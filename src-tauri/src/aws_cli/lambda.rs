use super::apps::get_app_by_id;
use super::core::{execute_aws_command, extract_repo_name};
use serde::{Deserialize, Serialize};

/// Lambda function with runtime information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaFunction {
    pub arn: String,
    pub name: String,
    pub friendly_name: String,
    pub runtime: String,
    pub description: Option<String>,
    pub is_outdated: bool,
    pub is_auto_managed: bool,
}

/// Response structure for resourcegroupstaggingapi get-resources command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TaggingApiResponse {
    resource_tag_mapping_list: Vec<ResourceTagMapping>,
}

/// Resource tag mapping from tagging API
#[derive(Debug, Deserialize)]
struct ResourceTagMapping {
    #[serde(rename = "ResourceARN")]
    resource_arn: String,
    #[serde(rename = "Tags", default)]
    tags: Vec<ResourceTag>,
}

/// Tag from tagging API
#[derive(Debug, Deserialize)]
struct ResourceTag {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Value")]
    value: String,
}

/// Response structure for Lambda get-function command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LambdaGetFunctionResponse {
    configuration: LambdaConfiguration,
}

/// Lambda function configuration
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LambdaConfiguration {
    function_name: String,
    function_arn: String,
    runtime: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Extract a friendly name from a Lambda function name
/// Amplify functions often have names like "amplify-xxx-function-yyy"
fn extract_friendly_name(function_name: &str) -> String {
    // Try to extract a meaningful name from the function name
    // Common patterns: amplify-{appId}-{env}-function-{name}
    let parts: Vec<&str> = function_name.split('-').collect();

    // Look for "function" in the parts and take what comes after
    if let Some(pos) = parts.iter().position(|&p| p == "function") {
        if pos + 1 < parts.len() {
            return parts[pos + 1..].join("-");
        }
    }

    // Fallback to the full function name
    function_name.to_string()
}

/// Get Lambda function details by ARN
fn get_lambda_function_details(
    arn: &str,
    profile: &str,
    region: &str,
) -> Result<LambdaFunction, String> {
    let output = execute_aws_command(
        &["lambda", "get-function", "--function-name", arn],
        profile,
        region,
    )?;

    let response: LambdaGetFunctionResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Lambda function response: {}", e))?;

    let config = response.configuration;
    let friendly_name = extract_friendly_name(&config.function_name);

    Ok(LambdaFunction {
        arn: config.function_arn,
        name: config.function_name,
        friendly_name,
        runtime: config.runtime.unwrap_or_else(|| "unknown".to_string()),
        description: config.description,
        is_outdated: false,     // Will be set by the caller
        is_auto_managed: false, // Will be set by the caller based on tags
    })
}

/// Helper to get function details from a list of resource ARNs
fn get_function_details_from_arns(
    mappings: &[ResourceTagMapping],
    profile: &str,
    region: &str,
    app_name: &str,
) -> Result<Vec<LambdaFunction>, String> {
    let mut functions = Vec::new();
    for mapping in mappings {
        match get_lambda_function_details(&mapping.resource_arn, profile, region) {
            Ok(mut func) => {
                // Determine if auto-managed based on tags
                func.is_auto_managed = is_auto_managed_function(&mapping.tags, app_name);
                functions.push(func);
            }
            Err(_e) => {}
        }
    }
    Ok(functions)
}

/// Determine if a function is auto-managed by Amplify based on tags
/// - For Gen2: If "amplify:friendly-name" tag exists and equals repository name (case-insensitive), it's auto-managed
/// - For Gen1: If "amplify:friendly-name" doesn't exist, check "aws:cloudformation:logical-id"
///   - If logical-id is NOT "LambdaFunction", it's auto-managed
fn is_auto_managed_function(tags: &[ResourceTag], repo_name: &str) -> bool {
    // First check for amplify:friendly-name tag (Gen2)
    let friendly_name = tags.iter().find(|t| t.key == "amplify:friendly-name");
    if let Some(tag) = friendly_name {
        // If friendly-name equals repository name (case-insensitive), it's auto-managed
        return tag.value.to_lowercase().contains(&repo_name.to_lowercase());
    }

    // Fallback: check aws:cloudformation:logical-id (Gen1)
    let logical_id = tags
        .iter()
        .find(|t| t.key == "aws:cloudformation:logical-id");
    if let Some(tag) = logical_id {
        // If logical-id is NOT "LambdaFunction", it's auto-managed
        return tag.value != "LambdaFunction";
    }

    // Default: assume not auto-managed
    false
}

/// Response structure for Lambda list-functions command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LambdaListFunctionsResponse {
    functions: Vec<LambdaFunctionSummary>,
}

/// Lambda function summary from list-functions
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LambdaFunctionSummary {
    function_name: String,
    function_arn: String,
    runtime: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Fallback: Get Lambda functions by name pattern matching
/// This is used when tag-based discovery doesn't find functions
async fn get_lambda_functions_by_name_pattern(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
) -> Result<Vec<LambdaFunction>, String> {
    let output = execute_aws_command(&["lambda", "list-functions"], profile, region)?;

    let response: LambdaListFunctionsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Lambda list-functions response: {}", e))?;

    // Filter functions by name pattern - Amplify functions typically contain the app_id
    let app_id_lower = app_id.to_lowercase();
    let branch_lower = branch_name.to_lowercase();

    let functions: Vec<LambdaFunction> = response
        .functions
        .into_iter()
        .filter(|f| {
            let name_lower = f.function_name.to_lowercase();
            // Match functions that contain both app_id and branch name
            name_lower.contains(&app_id_lower) && name_lower.contains(&branch_lower)
        })
        .map(|f| LambdaFunction {
            arn: f.function_arn,
            friendly_name: extract_friendly_name(&f.function_name),
            name: f.function_name,
            runtime: f.runtime.unwrap_or_else(|| "unknown".to_string()),
            description: f.description,
            is_outdated: false,
            is_auto_managed: false, // Can't determine without tags in fallback mode
        })
        .collect();

    Ok(functions)
}

/// Tauri command to get Lambda functions for an Amplify app/branch
/// Uses resourcegroupstaggingapi to find functions tagged with the app ID and branch
#[tauri::command]
pub async fn get_lambda_functions(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    backend_environment_name: &str,
) -> Result<Vec<LambdaFunction>, String> {
    // Get app info - needed for auto-managed detection
    let app = get_app_by_id(profile, region, app_id).await?;
    let app_name = app.name.clone();
    let repo_name = extract_repo_name(&app.repository);

    // Strategy 1: Try amplify:app-id and amplify:branch-name tags first (Gen2)
    let amplify_app_filter = format!("Key=amplify:app-id,Values={}", app_id);
    let amplify_branch_filter = format!("Key=amplify:branch-name,Values={}", branch_name);

    let output = execute_aws_command(
        &[
            "resourcegroupstaggingapi",
            "get-resources",
            "--resource-type-filters",
            "lambda:function",
            "--tag-filters",
            &amplify_app_filter,
            &amplify_branch_filter,
        ],
        profile,
        region,
    );

    match &output {
        Ok(out) => {
            if let Ok(response) = serde_json::from_str::<TaggingApiResponse>(out) {
                if !response.resource_tag_mapping_list.is_empty() {
                    return get_function_details_from_arns(
                        &response.resource_tag_mapping_list,
                        profile,
                        region,
                        &repo_name,
                    );
                }
            }
        }
        Err(_e) => {}
    }

    // Strategy 2: Fallback to user:Application (app name) and user:Stack (backend env name) tags (Gen1)
    // Note: user:Stack uses the backend environment name, not the branch name

    let user_app_filter = format!("Key=user:Application,Values={}", app_name);
    let user_stack_filter = format!("Key=user:Stack,Values={}", backend_environment_name);

    let cmd_args = [
        "resourcegroupstaggingapi",
        "get-resources",
        "--resource-type-filters",
        "lambda:function",
        "--tag-filters",
        &user_app_filter,
        &user_stack_filter,
    ];

    let output = execute_aws_command(&cmd_args, profile, region);

    match &output {
        Ok(out) => match serde_json::from_str::<TaggingApiResponse>(out) {
            Ok(response) => {
                if !response.resource_tag_mapping_list.is_empty() {
                    return get_function_details_from_arns(
                        &response.resource_tag_mapping_list,
                        profile,
                        region,
                        &repo_name,
                    );
                }
            }
            Err(_e) => {}
        },
        Err(_e) => {}
    }

    // Strategy 3: Final fallback - list all functions and filter by name pattern
    get_lambda_functions_by_name_pattern(profile, region, app_id, branch_name).await
}

/// Extract the major version number from a Lambda runtime string.
/// Handles formats like "nodejs18.x", "nodejs20.x"
///
/// # Arguments
/// * `runtime` - The Lambda runtime string (e.g., "nodejs18.x")
///
/// # Returns
/// The major version number if parsing succeeds, None otherwise
pub fn extract_runtime_version(runtime: &str) -> Option<u32> {
    // Lambda runtimes are in format "nodejsXX.x"
    let stripped = runtime.strip_prefix("nodejs")?;
    let version_str = stripped.strip_suffix(".x")?;
    version_str.parse().ok()
}

/// Check if a Lambda runtime is a Node.js runtime
pub fn is_nodejs_runtime(runtime: &str) -> bool {
    runtime.starts_with("nodejs")
}

/// Check if a Lambda runtime is outdated compared to supported versions.
/// A runtime is outdated if it's a Node.js runtime and its version is not in the supported list.
/// Non-Node.js runtimes (Python, etc.) are NOT considered outdated - they're just not applicable.
///
/// # Arguments
/// * `runtime` - The Lambda runtime string (e.g., "nodejs18.x")
/// * `supported_versions` - List of supported major version numbers
///
/// # Returns
/// `true` if the runtime is a Node.js runtime that is outdated, `false` otherwise
pub fn is_runtime_outdated(runtime: &str, supported_versions: &[u32]) -> bool {
    // Only check Node.js runtimes - other runtimes are not applicable for this tool
    if !is_nodejs_runtime(runtime) {
        return false;
    }

    match extract_runtime_version(runtime) {
        Some(version) => !supported_versions.contains(&version),
        None => false, // Can't determine version, don't mark as outdated
    }
}

/// Mark Lambda functions as outdated based on supported runtime versions.
///
/// # Arguments
/// * `functions` - Mutable slice of Lambda functions to update
/// * `supported_versions` - List of supported major version numbers
pub fn mark_outdated_functions(functions: &mut [LambdaFunction], supported_versions: &[u32]) {
    for func in functions.iter_mut() {
        func.is_outdated = is_runtime_outdated(&func.runtime, supported_versions);
    }
}

/// Tauri command to get Lambda functions with outdated status for an Amplify app/branch.
/// Combines Lambda function discovery with runtime comparison.
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
/// * `branch_name` - Amplify branch name
/// * `backend_environment_name` - Backend environment name (from backendEnvironmentArn)
/// * `supported_versions` - List of supported Node.js major version numbers
///
/// # Returns
/// List of Lambda functions with is_outdated field set based on runtime comparison
#[tauri::command]
pub async fn get_lambda_functions_with_status(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    backend_environment_name: &str,
    supported_versions: Vec<u32>,
) -> Result<Vec<LambdaFunction>, String> {
    let mut functions = get_lambda_functions(
        profile,
        region,
        app_id,
        branch_name,
        backend_environment_name,
    )
    .await?;
    mark_outdated_functions(&mut functions, &supported_versions);
    Ok(functions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Unit tests
    #[test]
    fn test_extract_runtime_version() {
        assert_eq!(extract_runtime_version("nodejs18.x"), Some(18));
        assert_eq!(extract_runtime_version("nodejs20.x"), Some(20));
        assert_eq!(extract_runtime_version("nodejs22.x"), Some(22));
        assert_eq!(extract_runtime_version("python3.9"), None);
        assert_eq!(extract_runtime_version("invalid"), None);
    }

    #[test]
    fn test_is_runtime_outdated() {
        let supported = vec![20, 22];

        assert!(is_runtime_outdated("nodejs18.x", &supported)); // 18 not in supported
        assert!(!is_runtime_outdated("nodejs20.x", &supported)); // 20 is supported
        assert!(!is_runtime_outdated("nodejs22.x", &supported)); // 22 is supported
        assert!(!is_runtime_outdated("python3.9", &supported)); // Not a Node.js runtime - not applicable
        assert!(!is_runtime_outdated("python3.12", &supported)); // Not a Node.js runtime - not applicable
    }

    #[test]
    fn test_is_nodejs_runtime() {
        assert!(is_nodejs_runtime("nodejs18.x"));
        assert!(is_nodejs_runtime("nodejs20.x"));
        assert!(!is_nodejs_runtime("python3.9"));
        assert!(!is_nodejs_runtime("python3.12"));
        assert!(!is_nodejs_runtime("java11"));
    }

    #[test]
    fn test_mark_outdated_functions() {
        let mut functions = vec![
            LambdaFunction {
                arn: "arn:aws:lambda:us-east-1:123456789:function:test1".to_string(),
                name: "test1".to_string(),
                friendly_name: "test1".to_string(),
                runtime: "nodejs18.x".to_string(),
                description: None,
                is_outdated: false,
                is_auto_managed: false,
            },
            LambdaFunction {
                arn: "arn:aws:lambda:us-east-1:123456789:function:test2".to_string(),
                name: "test2".to_string(),
                friendly_name: "test2".to_string(),
                runtime: "nodejs20.x".to_string(),
                description: None,
                is_outdated: false,
                is_auto_managed: false,
            },
        ];

        let supported = vec![20, 22];
        mark_outdated_functions(&mut functions, &supported);

        assert!(functions[0].is_outdated); // nodejs18.x is outdated
        assert!(!functions[1].is_outdated); // nodejs20.x is supported
    }

    #[test]
    fn test_is_auto_managed_function() {
        // Test with amplify:friendly-name tag matching repo name (Gen2) - case insensitive
        let tags_with_friendly_name = vec![ResourceTag {
            key: "amplify:friendly-name".to_string(),
            value: "todo-gen2".to_string(),
        }];
        assert!(is_auto_managed_function(
            &tags_with_friendly_name,
            "todo-gen2"
        ));
        assert!(is_auto_managed_function(
            &tags_with_friendly_name,
            "TODO-GEN2"
        )); // case insensitive
        assert!(is_auto_managed_function(
            &tags_with_friendly_name,
            "Todo-Gen2"
        )); // case insensitive
        assert!(!is_auto_managed_function(
            &tags_with_friendly_name,
            "other-repo"
        ));

        // Test case insensitive with mixed case tag value
        let tags_mixed_case = vec![ResourceTag {
            key: "amplify:friendly-name".to_string(),
            value: "myKB".to_string(),
        }];
        assert!(is_auto_managed_function(&tags_mixed_case, "mykb"));
        assert!(is_auto_managed_function(&tags_mixed_case, "myKB"));
        assert!(is_auto_managed_function(&tags_mixed_case, "MYKB"));

        // Test with aws:cloudformation:logical-id tag (Gen1)
        let tags_with_logical_id_lambda = vec![ResourceTag {
            key: "aws:cloudformation:logical-id".to_string(),
            value: "LambdaFunction".to_string(),
        }];
        assert!(!is_auto_managed_function(
            &tags_with_logical_id_lambda,
            "myrepo"
        )); // LambdaFunction = custom function

        let tags_with_logical_id_other = vec![ResourceTag {
            key: "aws:cloudformation:logical-id".to_string(),
            value: "UpdateRolesWithIDPFunction".to_string(),
        }];
        assert!(is_auto_managed_function(
            &tags_with_logical_id_other,
            "myrepo"
        )); // Not LambdaFunction = auto-managed

        // Test with no relevant tags
        let empty_tags: Vec<ResourceTag> = vec![];
        assert!(!is_auto_managed_function(&empty_tags, "myrepo"));
    }

    #[test]
    fn test_extract_friendly_name() {
        assert_eq!(
            extract_friendly_name("amplify-d2xoh1ssr21q7d-main-function-myFunction"),
            "myFunction"
        );
        assert_eq!(
            extract_friendly_name("amplify-app-dev-function-api-handler"),
            "api-handler"
        );
        assert_eq!(extract_friendly_name("simple-function-name"), "name");
        assert_eq!(extract_friendly_name("no-function-keyword"), "keyword");
    }

    // Property-based tests
    // **Feature: amplify-runtime-updater, Property 4: Runtime Outdated Detection**
    // **Validates: Requirements 5.3**
    proptest! {
        /// Property 4: Runtime Outdated Detection
        /// For any Lambda function runtime string and list of supported runtime versions,
        /// the system shall correctly identify the function as outdated if and only if
        /// its runtime version is not in the supported list.
        #[test]
        fn prop_runtime_outdated_detection(
            runtime_version in 10u32..30,
            supported_versions in prop::collection::vec(10u32..30, 1..5)
        ) {
            let runtime = format!("nodejs{}.x", runtime_version);
            let is_outdated = is_runtime_outdated(&runtime, &supported_versions);

            // Property: runtime is outdated iff its version is NOT in supported_versions
            let expected_outdated = !supported_versions.contains(&runtime_version);

            prop_assert_eq!(is_outdated, expected_outdated,
                "Runtime outdated detection failed for runtime {} with supported {:?}: expected {}, got {}",
                runtime, supported_versions, expected_outdated, is_outdated);
        }

        /// Property test for non-Node.js runtime formats
        /// Non-Node.js runtimes should NOT be considered outdated (not applicable)
        #[test]
        fn prop_non_nodejs_runtime_not_outdated(
            prefix in "[a-z]{3,10}",
            version in 10u32..30,
            supported_versions in prop::collection::vec(10u32..30, 1..5)
        ) {
            // Create a non-Node.js runtime format
            let runtime = format!("{}{}.x", prefix, version);

            // Skip if it accidentally matches the nodejs format
            if runtime.starts_with("nodejs") {
                return Ok(());
            }

            let is_outdated = is_runtime_outdated(&runtime, &supported_versions);

            // Property: non-Node.js runtimes should NOT be considered outdated
            prop_assert!(!is_outdated,
                "Non-Node.js runtime {} should NOT be considered outdated", runtime);
        }

        /// Property test for version extraction round-trip
        #[test]
        fn prop_version_extraction_roundtrip(version in 10u32..100) {
            let runtime = format!("nodejs{}.x", version);
            let extracted = extract_runtime_version(&runtime);

            prop_assert_eq!(extracted, Some(version),
                "Version extraction failed for runtime {}: expected Some({}), got {:?}",
                runtime, version, extracted);
        }
    }
}
