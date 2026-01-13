use super::{FileChange, UpdateResult};
use crate::amplify::env::check_live_updates_version;
use regex::Regex;
use std::path::Path;
use walkdir::WalkDir;

/// Finds all resource.ts files recursively in a project directory.
/// Excludes node_modules and other common non-source directories.
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Vec<String>` - List of paths to resource.ts files found
pub fn find_resource_ts_files(project_path: &str) -> Vec<String> {
    let path = Path::new(project_path);
    let mut resource_files = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            // Skip common non-source directories
            let name = e.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                "node_modules" | ".git" | "dist" | "build" | ".amplify" | "cdk.out"
            )
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy();
            if file_name == "resource.ts" {
                if let Some(path_str) = entry.path().to_str() {
                    resource_files.push(path_str.to_string());
                }
            }
        }
    }

    resource_files
}

/// Converts a Node.js version string (e.g., "nodejs20.x") to the Runtime enum format (e.g., "NODEJS_20_X")
pub fn version_to_runtime_enum(version: &str) -> Option<String> {
    // Parse version like "nodejs20.x" or "nodejs18.x"
    let re = Regex::new(r"nodejs(\d+)\.x").ok()?;
    let caps = re.captures(version)?;
    let major = caps.get(1)?.as_str();
    Some(format!("NODEJS_{}_X", major))
}

/// Extracts the major version number from a runtime string.
/// Supports formats like "nodejs20.x", "NODEJS_20_X", "Runtime.NODEJS_20_X"
fn extract_major_version(runtime: &str) -> Option<u32> {
    // Try to match "nodejs20.x" format
    if let Ok(re) = Regex::new(r"nodejs(\d+)\.x") {
        if let Some(caps) = re.captures(runtime) {
            if let Some(major) = caps.get(1) {
                return major.as_str().parse().ok();
            }
        }
    }

    // Try to match "NODEJS_20_X" or "Runtime.NODEJS_20_X" format
    if let Ok(re) = Regex::new(r"NODEJS_(\d+)_X") {
        if let Some(caps) = re.captures(runtime) {
            if let Some(major) = caps.get(1) {
                return major.as_str().parse().ok();
            }
        }
    }

    None
}

/// Checks if the current runtime version is older than the target runtime.
/// Returns true if current is older (should be updated), false otherwise.
fn is_runtime_older(current: &str, target: &str) -> bool {
    match (
        extract_major_version(current),
        extract_major_version(target),
    ) {
        (Some(current_ver), Some(target_ver)) => current_ver < target_ver,
        _ => false, // If we can't parse versions, don't update
    }
}

/// Updates runtime definitions in a resource.ts file content.
/// Replaces patterns like `Runtime.NODEJS_18_X` and `runtime: 18` with the target runtime.
/// Only updates runtimes that are older than the target (won't downgrade).
///
/// # Arguments
/// * `content` - The content of the resource.ts file
/// * `target_runtime` - The target runtime version (e.g., "nodejs20.x")
///
/// # Returns
/// * `(String, Vec<FileChange>)` - Updated content and list of changes made
pub fn update_runtime_in_resource_ts(
    content: &str,
    target_runtime: &str,
    file_path: &str,
) -> (String, Vec<FileChange>) {
    let mut changes = Vec::new();

    // Extract target major version
    let target_major = match extract_major_version(target_runtime) {
        Some(v) => v,
        None => return (content.to_string(), changes),
    };

    // Convert target runtime to enum format (e.g., "nodejs20.x" -> "NODEJS_20_X")
    let target_enum = match version_to_runtime_enum(target_runtime) {
        Some(e) => e,
        None => return (content.to_string(), changes),
    };

    let mut result = content.to_string();

    // Pattern 1: Match Runtime.NODEJS_XX_X
    let enum_re = match Regex::new(r"Runtime\.NODEJS_(\d+)_X") {
        Ok(r) => r,
        Err(_) => return (content.to_string(), changes),
    };

    // Pattern 2: Match runtime: XX (numeric format)
    // This pattern matches "runtime: 18" or "runtime:18" with optional whitespace
    let numeric_re = match Regex::new(r"runtime:\s*(\d+)") {
        Ok(r) => r,
        Err(_) => return (content.to_string(), changes),
    };

    // Process enum format (Runtime.NODEJS_XX_X)
    let new_enum_value = format!("Runtime.{}", target_enum);
    for cap in enum_re.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let old_value = full_match.as_str();

        // Only update if the current runtime is older than the target
        if old_value != new_enum_value && is_runtime_older(old_value, target_runtime) {
            changes.push(FileChange {
                path: file_path.to_string(),
                change_type: "runtime_update".to_string(),
                old_value: old_value.to_string(),
                new_value: new_enum_value.clone(),
            });
        }
    }

    // Perform selective replacement for enum format - only replace older versions
    if !changes.is_empty() {
        result = enum_re
            .replace_all(&result, |caps: &regex::Captures| {
                let full_match = caps.get(0).unwrap().as_str();
                if is_runtime_older(full_match, target_runtime) {
                    new_enum_value.clone()
                } else {
                    full_match.to_string()
                }
            })
            .to_string();
    }

    // Process numeric format (runtime: XX)
    let new_numeric_value = format!("runtime: {}", target_major);
    for cap in numeric_re.captures_iter(content) {
        if let Some(version_match) = cap.get(1) {
            let old_version: u32 = match version_match.as_str().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Only update if the current runtime is older than the target
            if old_version < target_major {
                let full_match = cap.get(0).unwrap();
                let old_value = full_match.as_str();

                changes.push(FileChange {
                    path: file_path.to_string(),
                    change_type: "runtime_update".to_string(),
                    old_value: old_value.to_string(),
                    new_value: new_numeric_value.clone(),
                });
            }
        }
    }

    // Perform selective replacement for numeric format - only replace older versions
    result = numeric_re
        .replace_all(&result, |caps: &regex::Captures| {
            if let Some(version_match) = caps.get(1) {
                if let Ok(old_version) = version_match.as_str().parse::<u32>() {
                    if old_version < target_major {
                        return new_numeric_value.clone();
                    }
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        })
        .to_string();

    (result, changes)
}

/// Updates @aws-amplify/backend and @aws-amplify/backend-cli dependencies to latest versions.
///
/// # Arguments
/// * `package_json_content` - The content of package.json
/// * `file_path` - Path to the package.json file (for change tracking)
///
/// # Returns
/// * `(String, Vec<FileChange>)` - Updated content and list of changes made
pub fn update_amplify_dependencies(
    package_json_content: &str,
    file_path: &str,
) -> Result<(String, Vec<FileChange>), String> {
    let mut package_json: serde_json::Value = serde_json::from_str(package_json_content)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    let mut changes = Vec::new();
    let latest_version = "latest";

    // Update devDependencies
    if let Some(dev_deps) = package_json.get_mut("devDependencies") {
        if let Some(dev_deps_obj) = dev_deps.as_object_mut() {
            // Update @aws-amplify/backend
            if let Some(current_version) = dev_deps_obj.get("@aws-amplify/backend") {
                let current_str = current_version.as_str().unwrap_or("");
                if current_str != latest_version {
                    changes.push(FileChange {
                        path: file_path.to_string(),
                        change_type: "dependency_update".to_string(),
                        old_value: format!("@aws-amplify/backend: {}", current_str),
                        new_value: format!("@aws-amplify/backend: {}", latest_version),
                    });
                    dev_deps_obj.insert(
                        "@aws-amplify/backend".to_string(),
                        serde_json::Value::String(latest_version.to_string()),
                    );
                }
            }

            // Update @aws-amplify/backend-cli
            if let Some(current_version) = dev_deps_obj.get("@aws-amplify/backend-cli") {
                let current_str = current_version.as_str().unwrap_or("");
                if current_str != latest_version {
                    changes.push(FileChange {
                        path: file_path.to_string(),
                        change_type: "dependency_update".to_string(),
                        old_value: format!("@aws-amplify/backend-cli: {}", current_str),
                        new_value: format!("@aws-amplify/backend-cli: {}", latest_version),
                    });
                    dev_deps_obj.insert(
                        "@aws-amplify/backend-cli".to_string(),
                        serde_json::Value::String(latest_version.to_string()),
                    );
                }
            }
        }
    }

    // Also check regular dependencies
    if let Some(deps) = package_json.get_mut("dependencies") {
        if let Some(deps_obj) = deps.as_object_mut() {
            // Update @aws-amplify/backend
            if let Some(current_version) = deps_obj.get("@aws-amplify/backend") {
                let current_str = current_version.as_str().unwrap_or("");
                if current_str != latest_version {
                    changes.push(FileChange {
                        path: file_path.to_string(),
                        change_type: "dependency_update".to_string(),
                        old_value: format!("@aws-amplify/backend: {}", current_str),
                        new_value: format!("@aws-amplify/backend: {}", latest_version),
                    });
                    deps_obj.insert(
                        "@aws-amplify/backend".to_string(),
                        serde_json::Value::String(latest_version.to_string()),
                    );
                }
            }

            // Update @aws-amplify/backend-cli
            if let Some(current_version) = deps_obj.get("@aws-amplify/backend-cli") {
                let current_str = current_version.as_str().unwrap_or("");
                if current_str != latest_version {
                    changes.push(FileChange {
                        path: file_path.to_string(),
                        change_type: "dependency_update".to_string(),
                        old_value: format!("@aws-amplify/backend-cli: {}", current_str),
                        new_value: format!("@aws-amplify/backend-cli: {}", latest_version),
                    });
                    deps_obj.insert(
                        "@aws-amplify/backend-cli".to_string(),
                        serde_json::Value::String(latest_version.to_string()),
                    );
                }
            }
        }
    }

    let updated_content = serde_json::to_string_pretty(&package_json)
        .map_err(|e| format!("Failed to serialize package.json: {}", e))?;

    Ok((updated_content, changes))
}

/// Finds all CloudFormation template files (*-cloudformation-template.json) in backend/function/ directories.
/// This is used for Gen1 Amplify backend updates.
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Vec<String>` - List of paths to CloudFormation template files found
pub fn find_cloudformation_templates(project_path: &str) -> Vec<String> {
    let path = Path::new(project_path);
    let backend_function_path = path.join("amplify").join("backend").join("function");
    let mut template_files = Vec::new();

    // If the backend/function directory doesn't exist, return empty
    if !backend_function_path.exists() {
        return template_files;
    }

    for entry in WalkDir::new(&backend_function_path)
        .into_iter()
        .filter_entry(|e| {
            // Skip node_modules and other non-source directories
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "node_modules" | ".git" | "dist" | "build")
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy();
            if file_name.ends_with("-cloudformation-template.json") {
                if let Some(path_str) = entry.path().to_str() {
                    template_files.push(path_str.to_string());
                }
            }
        }
    }

    template_files
}

/// Updates the runtime in a CloudFormation template JSON content.
/// Looks for Resources.*.Properties.Runtime and updates nodejs versions.
/// Only updates runtimes that are older than the target (won't downgrade).
///
/// # Arguments
/// * `content` - The JSON content of the CloudFormation template
/// * `target_runtime` - The target runtime version (e.g., "nodejs20.x")
/// * `file_path` - Path to the file (for change tracking)
///
/// # Returns
/// * `Result<(String, Vec<FileChange>), String>` - Updated content and list of changes made
pub fn update_runtime_in_cloudformation_template(
    content: &str,
    target_runtime: &str,
    file_path: &str,
) -> Result<(String, Vec<FileChange>), String> {
    let mut template: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse CloudFormation template: {}", e))?;

    let mut changes = Vec::new();

    // Navigate to Resources
    if let Some(resources) = template.get_mut("Resources") {
        if let Some(resources_obj) = resources.as_object_mut() {
            for (resource_name, resource) in resources_obj.iter_mut() {
                // Check if this resource has Properties.Runtime
                if let Some(properties) = resource.get_mut("Properties") {
                    if let Some(runtime) = properties.get_mut("Runtime") {
                        if let Some(runtime_str) = runtime.as_str() {
                            // Check if it's a nodejs runtime that needs updating
                            // Only update if the current runtime is older than the target
                            if runtime_str.starts_with("nodejs")
                                && runtime_str != target_runtime
                                && is_runtime_older(runtime_str, target_runtime)
                            {
                                changes.push(FileChange {
                                    path: file_path.to_string(),
                                    change_type: "runtime_update".to_string(),
                                    old_value: format!("{}: {}", resource_name, runtime_str),
                                    new_value: format!("{}: {}", resource_name, target_runtime),
                                });
                                *runtime = serde_json::Value::String(target_runtime.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let updated_content = serde_json::to_string_pretty(&template)
        .map_err(|e| format!("Failed to serialize CloudFormation template: {}", e))?;

    Ok((updated_content, changes))
}

/// Finds all Lambda function handler files in Gen1 backend/function directories.
/// For each function, looks for index.ts first, then index.js if index.ts doesn't exist.
/// Finds all Lambda function handler files in Gen1 backend/function directories.
/// For each function, looks for index.js only (TypeScript files are compiled to JS for deployment).
/// This matches the typical Amplify Gen1 function structure: amplify/backend/function/<functionName>/src/index.js
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Vec<String>` - List of paths to handler files found (one per function)
pub fn find_gen1_function_handlers(project_path: &str) -> Vec<String> {
    let path = Path::new(project_path);
    let backend_function_path = path.join("amplify").join("backend").join("function");
    let mut handler_files = Vec::new();

    // If the backend/function directory doesn't exist, return empty
    if !backend_function_path.exists() {
        return handler_files;
    }

    // Iterate through each function directory
    if let Ok(entries) = std::fs::read_dir(&backend_function_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let function_path = entry.path();

            // Skip if not a directory
            if !function_path.is_dir() {
                continue;
            }

            // Skip node_modules and other non-function directories
            if let Some(dir_name) = function_path.file_name() {
                let name = dir_name.to_string_lossy();
                if matches!(name.as_ref(), "node_modules" | ".git" | "dist" | "build") {
                    continue;
                }
            }

            let src_path = function_path.join("src");

            // Check if src directory exists
            if !src_path.exists() || !src_path.is_dir() {
                continue;
            }

            // Look for index.js only (TypeScript is compiled to JS for deployment)
            let index_js = src_path.join("index.js");

            if index_js.exists() {
                if let Some(path_str) = index_js.to_str() {
                    handler_files.push(path_str.to_string());
                }
            }
        }
    }

    handler_files
}

/// Adds a comment to a Lambda function handler file to trigger CloudFormation update.
/// This is necessary for Gen1 because Amplify CLI only regenerates CloudFormation
/// when there are code changes in the function.
///
/// # Arguments
/// * `content` - The content of the handler file
/// * `target_runtime` - The target runtime version (for the comment)
///
/// # Returns
/// * `String` - Updated content with comment added at the top
pub fn add_runtime_update_comment(content: &str, target_runtime: &str) -> String {
    use chrono::Utc;
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let comment = format!(
        "// Runtime updated to {} on {}\n",
        target_runtime, timestamp
    );

    // Check if a similar comment already exists (to avoid duplicates)
    if content.contains("// Runtime updated to") {
        // Replace existing comment
        let re = Regex::new(r"// Runtime updated to .+ on .+\n").unwrap();
        re.replace(content, comment.as_str()).to_string()
    } else {
        // Add new comment at the top
        format!("{}{}", comment, content)
    }
}

/// Updates a Gen1 Amplify backend to use the target Node.js runtime.
/// This function:
/// 1. Finds all CloudFormation template files in backend/function/
/// 2. Updates runtime definitions in each template
/// 3. Adds comments to function handler files to trigger CloudFormation regeneration
/// 4. Checks and updates _LIVE_UPDATES environment variable if needed
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `target_runtime` - The target runtime version (e.g., "nodejs20.x")
/// * `app_env_vars` - Environment variables from the Amplify app (contains _LIVE_UPDATES)
///
/// # Returns
/// * `UpdateResult` - Contains list of changes made and success status
#[tauri::command]
pub async fn update_gen1_backend(
    project_path: &str,
    target_runtime: &str,
    app_env_vars: std::collections::HashMap<String, String>,
) -> Result<UpdateResult, String> {
    let mut all_changes = Vec::new();

    // Step 1: Find all CloudFormation template files
    let template_files = find_cloudformation_templates(project_path);

    // Step 2: Update runtime in each CloudFormation template
    for file_path in &template_files {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        let (updated_content, changes) =
            update_runtime_in_cloudformation_template(&content, target_runtime, file_path)?;

        if !changes.is_empty() {
            std::fs::write(file_path, &updated_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path, e))?;
            all_changes.extend(changes);
        }
    }

    // Step 2.5: Add comments to function handler files to trigger CloudFormation regeneration
    // COMMENTED OUT FOR TESTING - Testing if this is actually required for Gen1 runtime updates
    // This is necessary because Amplify CLI only regenerates CloudFormation when there are code changes
    /*
    let handler_files = find_gen1_function_handlers(project_path);
    for file_path in &handler_files {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        let updated_content = add_runtime_update_comment(&content, target_runtime);

        // Only write if content actually changed
        if updated_content != content {
            std::fs::write(file_path, &updated_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path, e))?;

            all_changes.push(FileChange {
                path: file_path.to_string(),
                change_type: "code_comment_update".to_string(),
                old_value: "No runtime comment".to_string(),
                new_value: format!("Added runtime update comment for {}", target_runtime),
            });
        }
    }
    */

    // Step 3: Check _LIVE_UPDATES if present (but don't add to changes - handled separately in Step 6)
    // This is just for validation purposes
    if let Some(live_updates) = app_env_vars.get("_LIVE_UPDATES") {
        match check_live_updates_version(live_updates) {
            Ok(_is_latest) => {
                // Validation passed, env var update will be handled in Step 6
            }
            Err(e) => {
                // Log warning but continue - _LIVE_UPDATES might have unexpected format
                eprintln!("Warning: Could not parse _LIVE_UPDATES: {}", e);
            }
        }
    }

    Ok(UpdateResult {
        changes: all_changes,
        success: true,
        error: None,
    })
}

/// Updates a Gen2 Amplify backend to use the target Node.js runtime.
/// This function:
/// 1. Finds all resource.ts files in the project
/// 2. Updates runtime definitions in each file
/// 3. Updates @aws-amplify/backend and @aws-amplify/backend-cli dependencies
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `target_runtime` - The target runtime version (e.g., "nodejs20.x")
///
/// # Returns
/// * `UpdateResult` - Contains list of changes made and success status
#[tauri::command]
pub async fn update_gen2_backend(
    project_path: &str,
    target_runtime: &str,
) -> Result<UpdateResult, String> {
    let mut all_changes = Vec::new();

    // Step 1: Find all resource.ts files
    let resource_files = find_resource_ts_files(project_path);

    // Step 2: Update runtime in each resource.ts file
    for file_path in &resource_files {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        let (updated_content, changes) =
            update_runtime_in_resource_ts(&content, target_runtime, file_path);

        if !changes.is_empty() {
            std::fs::write(file_path, &updated_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path, e))?;
            all_changes.extend(changes);
        }
    }

    // Note: package.json dependency updates (@aws-amplify/backend, @aws-amplify/backend-cli)
    // are handled separately by upgrade_amplify_backend_packages function

    Ok(UpdateResult {
        changes: all_changes,
        success: true,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    // Unit tests for runtime transformation
    #[test]
    fn test_version_to_runtime_enum() {
        assert_eq!(
            version_to_runtime_enum("nodejs20.x"),
            Some("NODEJS_20_X".to_string())
        );
        assert_eq!(
            version_to_runtime_enum("nodejs18.x"),
            Some("NODEJS_18_X".to_string())
        );
        assert_eq!(
            version_to_runtime_enum("nodejs22.x"),
            Some("NODEJS_22_X".to_string())
        );
        assert_eq!(version_to_runtime_enum("invalid"), None);
    }

    #[test]
    fn test_update_runtime_in_resource_ts_basic() {
        let content = r#"
import { defineFunction } from '@aws-amplify/backend';

export const myFunction = defineFunction({
  name: 'my-function',
  runtime: Runtime.NODEJS_18_X,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        assert!(updated.contains("Runtime.NODEJS_20_X"));
        assert!(!updated.contains("Runtime.NODEJS_18_X"));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_value, "Runtime.NODEJS_18_X");
        assert_eq!(changes[0].new_value, "Runtime.NODEJS_20_X");
    }

    #[test]
    fn test_update_runtime_in_resource_ts_multiple() {
        let content = r#"
export const func1 = defineFunction({
  runtime: Runtime.NODEJS_16_X,
});
export const func2 = defineFunction({
  runtime: Runtime.NODEJS_18_X,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        assert!(updated.contains("Runtime.NODEJS_20_X"));
        assert!(!updated.contains("Runtime.NODEJS_16_X"));
        assert!(!updated.contains("Runtime.NODEJS_18_X"));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_update_runtime_in_resource_ts_no_change_needed() {
        let content = r#"
export const myFunction = defineFunction({
  runtime: Runtime.NODEJS_20_X,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        assert_eq!(updated, content);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_update_runtime_in_resource_ts_no_downgrade() {
        // Test that newer runtimes are NOT downgraded to the target
        let content = r#"
export const myFunction = defineFunction({
  runtime: Runtime.NODEJS_22_X,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        // Should NOT change - nodejs22 is newer than nodejs20
        assert!(updated.contains("Runtime.NODEJS_22_X"));
        assert!(!updated.contains("Runtime.NODEJS_20_X"));
        assert!(changes.is_empty());
    }

    #[test]
    fn test_update_runtime_in_resource_ts_mixed_versions() {
        // Test with a mix of older and newer runtimes
        let content = r#"
export const func1 = defineFunction({
  runtime: Runtime.NODEJS_18_X,
});
export const func2 = defineFunction({
  runtime: Runtime.NODEJS_22_X,
});
export const func3 = defineFunction({
  runtime: Runtime.NODEJS_16_X,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        // Should update 18 and 16 to 20, but leave 22 unchanged
        assert!(updated.contains("Runtime.NODEJS_20_X")); // Updated from 18 and 16
        assert!(updated.contains("Runtime.NODEJS_22_X")); // Should remain unchanged
        assert!(!updated.contains("Runtime.NODEJS_18_X")); // Should be updated
        assert!(!updated.contains("Runtime.NODEJS_16_X")); // Should be updated
        assert_eq!(changes.len(), 2); // Only 2 changes (18 and 16)
    }

    #[test]
    fn test_extract_major_version() {
        assert_eq!(extract_major_version("nodejs20.x"), Some(20));
        assert_eq!(extract_major_version("nodejs18.x"), Some(18));
        assert_eq!(extract_major_version("nodejs22.x"), Some(22));
        assert_eq!(extract_major_version("NODEJS_20_X"), Some(20));
        assert_eq!(extract_major_version("Runtime.NODEJS_18_X"), Some(18));
        assert_eq!(extract_major_version("invalid"), None);
    }

    #[test]
    fn test_update_runtime_numeric_format_basic() {
        let content = r#"
export const bedrockApi = defineFunction({
  name: "myKbBedrockApi",
  entry: "./handler.ts",
  timeoutSeconds: 30,
  runtime: 18,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs22.x", "test/resource.ts");

        assert!(updated.contains("runtime: 22"));
        assert!(!updated.contains("runtime: 18"));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_value, "runtime: 18");
        assert_eq!(changes[0].new_value, "runtime: 22");
    }

    #[test]
    fn test_update_runtime_numeric_format_multiple() {
        let content = r#"
export const func1 = defineFunction({
  runtime: 16,
});
export const func2 = defineFunction({
  runtime: 18,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        assert!(updated.contains("runtime: 20"));
        assert!(!updated.contains("runtime: 16"));
        assert!(!updated.contains("runtime: 18"));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_update_runtime_numeric_format_no_downgrade() {
        let content = r#"
export const myFunction = defineFunction({
  runtime: 22,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        // Should NOT change - 22 is newer than 20
        assert!(updated.contains("runtime: 22"));
        assert!(!updated.contains("runtime: 20"));
        assert!(changes.is_empty());
    }

    #[test]
    fn test_update_runtime_numeric_format_mixed() {
        let content = r#"
export const func1 = defineFunction({
  runtime: 18,
});
export const func2 = defineFunction({
  runtime: 22,
});
export const func3 = defineFunction({
  runtime: 16,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        // Should update 18 and 16 to 20, but leave 22 unchanged
        assert!(updated.contains("runtime: 20")); // Updated from 18 and 16
        assert!(updated.contains("runtime: 22")); // Should remain unchanged
        assert_eq!(changes.len(), 2); // Only 2 changes (18 and 16)
    }

    #[test]
    fn test_update_runtime_mixed_enum_and_numeric() {
        let content = r#"
export const func1 = defineFunction({
  runtime: Runtime.NODEJS_18_X,
});
export const func2 = defineFunction({
  runtime: 16,
});
"#;
        let (updated, changes) =
            update_runtime_in_resource_ts(content, "nodejs20.x", "test/resource.ts");

        // Should update both enum and numeric formats
        assert!(updated.contains("Runtime.NODEJS_20_X"));
        assert!(updated.contains("runtime: 20"));
        assert!(!updated.contains("Runtime.NODEJS_18_X"));
        assert!(!updated.contains("runtime: 16"));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_extract_major_version_extended() {
        assert_eq!(extract_major_version("nodejs20.x"), Some(20));
        assert_eq!(extract_major_version("nodejs18.x"), Some(18));
        assert_eq!(extract_major_version("nodejs22.x"), Some(22));
        assert_eq!(extract_major_version("NODEJS_20_X"), Some(20));
        assert_eq!(extract_major_version("Runtime.NODEJS_18_X"), Some(18));
        assert_eq!(extract_major_version("invalid"), None);
    }

    #[test]
    fn test_is_runtime_older() {
        // Test with nodejs format
        assert!(is_runtime_older("nodejs18.x", "nodejs20.x"));
        assert!(is_runtime_older("nodejs16.x", "nodejs20.x"));
        assert!(!is_runtime_older("nodejs22.x", "nodejs20.x"));
        assert!(!is_runtime_older("nodejs20.x", "nodejs20.x"));

        // Test with Runtime enum format
        assert!(is_runtime_older("Runtime.NODEJS_18_X", "nodejs20.x"));
        assert!(!is_runtime_older("Runtime.NODEJS_22_X", "nodejs20.x"));
    }

    #[test]
    fn test_find_resource_ts_files() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(path.join("amplify/functions/func1")).unwrap();
        fs::create_dir_all(path.join("amplify/functions/func2")).unwrap();
        fs::create_dir_all(path.join("node_modules/some-package")).unwrap();

        // Create resource.ts files
        fs::write(path.join("amplify/functions/func1/resource.ts"), "").unwrap();
        fs::write(path.join("amplify/functions/func2/resource.ts"), "").unwrap();
        // This should be excluded (in node_modules)
        fs::write(path.join("node_modules/some-package/resource.ts"), "").unwrap();

        let files = find_resource_ts_files(path.to_str().unwrap());

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.contains("func1/resource.ts")));
        assert!(files.iter().any(|f| f.contains("func2/resource.ts")));
        assert!(!files.iter().any(|f| f.contains("node_modules")));
    }

    // Property-based tests
    proptest! {
        /// Property 7: Runtime Update Transformation
        #[test]
        fn prop_runtime_update_preserves_non_runtime_content(
            prefix in "[a-zA-Z0-9\\s\\n\\{\\}\\(\\)\\[\\];:,\\.='\"\\-_/\\*\\+@#\\$%&!\\?<>\\|\\\\~`^]+",
            suffix in "[a-zA-Z0-9\\s\\n\\{\\}\\(\\)\\[\\];:,\\.='\"\\-_/\\*\\+@#\\$%&!\\?<>\\|\\\\~`^]+",
            old_version in 14u8..20,
            new_version in 20u8..24
        ) {
            // Ensure we're actually updating (old != new)
            prop_assume!(old_version != new_version);

            let runtime_pattern = format!("Runtime.NODEJS_{}_X", old_version);
            let content = format!("{}{}{}", prefix, runtime_pattern, suffix);
            let target_runtime = format!("nodejs{}.x", new_version);

            let (updated, changes) = update_runtime_in_resource_ts(&content, &target_runtime, "test.ts");

            // The runtime should be updated
            let expected_runtime = format!("Runtime.NODEJS_{}_X", new_version);
            prop_assert!(updated.contains(&expected_runtime),
                "Updated content should contain new runtime: {}", expected_runtime);

            // The old runtime should be gone
            prop_assert!(!updated.contains(&runtime_pattern),
                "Updated content should not contain old runtime: {}", runtime_pattern);

            // Non-runtime content should be preserved
            prop_assert!(updated.starts_with(&prefix),
                "Prefix should be preserved");
            prop_assert!(updated.ends_with(&suffix),
                "Suffix should be preserved");

            // Changes should be recorded
            prop_assert!(!changes.is_empty(),
                "Changes should be recorded when runtime is updated");
            prop_assert_eq!(&changes[0].old_value, &runtime_pattern,
                "Old value should match original runtime");
            prop_assert_eq!(&changes[0].new_value, &expected_runtime,
                "New value should match target runtime");
        }

        /// Property test: No changes when runtime is already at target version
        #[test]
        fn prop_runtime_update_no_change_when_current(
            prefix in "[a-zA-Z0-9\\s\\n\\{\\}\\(\\)\\[\\];:,\\.='\"\\-_]+",
            suffix in "[a-zA-Z0-9\\s\\n\\{\\}\\(\\)\\[\\];:,\\.='\"\\-_]+",
            version in 18u8..24
        ) {
            let runtime_pattern = format!("Runtime.NODEJS_{}_X", version);
            let content = format!("{}{}{}", prefix, runtime_pattern, suffix);
            let target_runtime = format!("nodejs{}.x", version);

            let (updated, changes) = update_runtime_in_resource_ts(&content, &target_runtime, "test.ts");

            // Content should be unchanged
            prop_assert_eq!(updated, content,
                "Content should be unchanged when runtime is already at target version");

            // No changes should be recorded
            prop_assert!(changes.is_empty(),
                "No changes should be recorded when runtime is already at target version");
        }

        /// Property test: Multiple runtimes are all updated
        #[test]
        fn prop_runtime_update_multiple_runtimes(
            num_runtimes in 1usize..5,
            old_version in 14u8..18,
            new_version in 20u8..24
        ) {
            prop_assume!(old_version != new_version);

            let runtime_pattern = format!("Runtime.NODEJS_{}_X", old_version);
            let content = (0..num_runtimes)
                .map(|i| format!("func{}: {}", i, runtime_pattern))
                .collect::<Vec<_>>()
                .join("\n");
            let target_runtime = format!("nodejs{}.x", new_version);

            let (updated, changes) = update_runtime_in_resource_ts(&content, &target_runtime, "test.ts");

            // All old runtimes should be replaced
            prop_assert!(!updated.contains(&runtime_pattern),
                "All old runtimes should be replaced");

            // New runtime should appear the same number of times
            let expected_runtime = format!("Runtime.NODEJS_{}_X", new_version);
            let count = updated.matches(&expected_runtime).count();
            prop_assert_eq!(count, num_runtimes,
                "New runtime should appear {} times, found {}", num_runtimes, count);

            // Changes should be recorded for each runtime
            prop_assert_eq!(changes.len(), num_runtimes,
                "Should record {} changes, found {}", num_runtimes, changes.len());
        }

        /// Property test: Content without runtime patterns is unchanged
        #[test]
        fn prop_runtime_update_no_runtime_unchanged(
            content in "[a-zA-Z0-9\\s\\n\\{\\}\\(\\)\\[\\];:,\\.='\"\\-_]+",
            new_version in 20u8..24
        ) {
            // Ensure content doesn't accidentally contain a runtime pattern
            prop_assume!(!content.contains("Runtime.NODEJS_"));

            let target_runtime = format!("nodejs{}.x", new_version);

            let (updated, changes) = update_runtime_in_resource_ts(&content, &target_runtime, "test.ts");

            // Content should be unchanged
            prop_assert_eq!(updated, content,
                "Content without runtime patterns should be unchanged");

            // No changes should be recorded
            prop_assert!(changes.is_empty(),
                "No changes should be recorded when no runtime patterns exist");
        }
    }

    // ==================== Gen1 Backend Tests ====================

    // Unit tests for CloudFormation template search
    #[test]
    fn test_find_cloudformation_templates() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create Gen1 directory structure
        fs::create_dir_all(path.join("amplify/backend/function/myFunc1")).unwrap();
        fs::create_dir_all(path.join("amplify/backend/function/myFunc2")).unwrap();
        fs::create_dir_all(path.join("amplify/backend/function/myFunc1/node_modules")).unwrap();

        // Create CloudFormation template files
        fs::write(
            path.join("amplify/backend/function/myFunc1/myFunc1-cloudformation-template.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            path.join("amplify/backend/function/myFunc2/myFunc2-cloudformation-template.json"),
            "{}",
        )
        .unwrap();
        // This should be excluded (in node_modules)
        fs::write(
            path.join(
                "amplify/backend/function/myFunc1/node_modules/test-cloudformation-template.json",
            ),
            "{}",
        )
        .unwrap();

        let files = find_cloudformation_templates(path.to_str().unwrap());

        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|f| f.contains("myFunc1-cloudformation-template.json")));
        assert!(files
            .iter()
            .any(|f| f.contains("myFunc2-cloudformation-template.json")));
        assert!(!files.iter().any(|f| f.contains("node_modules")));
    }

    #[test]
    fn test_find_cloudformation_templates_no_backend_dir() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Don't create the backend/function directory
        let files = find_cloudformation_templates(path.to_str().unwrap());

        assert!(files.is_empty());
    }

    // Unit tests for CloudFormation runtime update
    #[test]
    fn test_update_runtime_in_cloudformation_template_basic() {
        let content = r#"{
            "Resources": {
                "LambdaFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "Runtime": "nodejs18.x",
                        "Handler": "index.handler"
                    }
                }
            }
        }"#;

        let (updated, changes) =
            update_runtime_in_cloudformation_template(content, "nodejs20.x", "test.json").unwrap();

        assert!(updated.contains("nodejs20.x"));
        assert!(!updated.contains("nodejs18.x"));
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_value.contains("nodejs18.x"));
        assert!(changes[0].new_value.contains("nodejs20.x"));
    }

    #[test]
    fn test_update_runtime_in_cloudformation_template_multiple_functions() {
        let content = r#"{
            "Resources": {
                "Function1": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "Runtime": "nodejs16.x"
                    }
                },
                "Function2": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "Runtime": "nodejs18.x"
                    }
                }
            }
        }"#;

        let (updated, changes) =
            update_runtime_in_cloudformation_template(content, "nodejs20.x", "test.json").unwrap();

        assert!(updated.contains("nodejs20.x"));
        assert!(!updated.contains("nodejs16.x"));
        assert!(!updated.contains("nodejs18.x"));
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_update_runtime_in_cloudformation_template_no_change_needed() {
        let content = r#"{
            "Resources": {
                "LambdaFunction": {
                    "Properties": {
                        "Runtime": "nodejs20.x"
                    }
                }
            }
        }"#;

        let (updated, changes) =
            update_runtime_in_cloudformation_template(content, "nodejs20.x", "test.json").unwrap();

        // Parse both to compare (formatting may differ)
        let original: serde_json::Value = serde_json::from_str(content).unwrap();
        let updated_json: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(original, updated_json);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_update_runtime_in_cloudformation_template_no_downgrade() {
        // Test that newer runtimes are NOT downgraded to the target
        let content = r#"{
            "Resources": {
                "LambdaFunction": {
                    "Properties": {
                        "Runtime": "nodejs22.x"
                    }
                }
            }
        }"#;

        let (updated, changes) =
            update_runtime_in_cloudformation_template(content, "nodejs20.x", "test.json").unwrap();

        // Should NOT change - nodejs22 is newer than nodejs20
        let updated_json: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(
            updated_json["Resources"]["LambdaFunction"]["Properties"]["Runtime"],
            "nodejs22.x"
        );
        assert!(changes.is_empty());
    }

    #[test]
    fn test_update_runtime_in_cloudformation_template_mixed_versions() {
        // Test with a mix of older and newer runtimes
        let content = r#"{
            "Resources": {
                "OldFunction": {
                    "Properties": {
                        "Runtime": "nodejs18.x"
                    }
                },
                "NewerFunction": {
                    "Properties": {
                        "Runtime": "nodejs22.x"
                    }
                },
                "VeryOldFunction": {
                    "Properties": {
                        "Runtime": "nodejs16.x"
                    }
                }
            }
        }"#;

        let (updated, changes) =
            update_runtime_in_cloudformation_template(content, "nodejs20.x", "test.json").unwrap();

        let updated_json: serde_json::Value = serde_json::from_str(&updated).unwrap();

        // OldFunction and VeryOldFunction should be updated to nodejs20.x
        assert_eq!(
            updated_json["Resources"]["OldFunction"]["Properties"]["Runtime"],
            "nodejs20.x"
        );
        assert_eq!(
            updated_json["Resources"]["VeryOldFunction"]["Properties"]["Runtime"],
            "nodejs20.x"
        );
        // NewerFunction should remain at nodejs22.x
        assert_eq!(
            updated_json["Resources"]["NewerFunction"]["Properties"]["Runtime"],
            "nodejs22.x"
        );
        // Only 2 changes (OldFunction and VeryOldFunction)
        assert_eq!(changes.len(), 2);
    }
}
