// AWS CLI module - handles all AWS CLI interactions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// AWS CLI profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsProfile {
    pub name: String,
}

/// Amplify application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyApp {
    pub app_id: String,
    pub name: String,
    pub repository: String,
    pub environment_variables: HashMap<String, String>,
}

/// Amplify branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyBranch {
    pub branch_name: String,
    pub stack_arn: String,
    pub backend_environment_name: String,
    pub environment_variables: HashMap<String, String>,
    pub is_protected: bool,
    pub protection_info: Option<String>,
}

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

/// Amplify job summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyJob {
    pub job_id: String,
    pub commit_id: String,
    pub status: String,
    pub start_time: Option<String>,
}

/// Amplify job details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyJobDetails {
    pub job_id: String,
    pub commit_id: String,
    pub status: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// Get the AWS config directory path
fn get_aws_config_dir() -> Option<PathBuf> {
    // Check AWS_CONFIG_FILE env var first
    if let Ok(config_file) = env::var("AWS_CONFIG_FILE") {
        let path = PathBuf::from(config_file);
        return path.parent().map(|p| p.to_path_buf());
    }

    // Default to ~/.aws
    dirs::home_dir().map(|home| home.join(".aws"))
}

/// Parse AWS credentials file to extract profile names
fn parse_credentials_file(content: &str) -> Vec<String> {
    let mut profiles = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let profile_name = &trimmed[1..trimmed.len() - 1];
            profiles.push(profile_name.to_string());
        }
    }

    profiles
}

/// Parse AWS config file to extract profile names
/// Config file uses [profile name] format except for default
fn parse_config_file(content: &str) -> Vec<String> {
    let mut profiles = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = &trimmed[1..trimmed.len() - 1];
            if section == "default" {
                profiles.push("default".to_string());
            } else if let Some(profile_name) = section.strip_prefix("profile ") {
                profiles.push(profile_name.to_string());
            }
        }
    }

    profiles
}

/// Get AWS profiles from credentials and config files
pub fn get_profiles_from_files() -> Vec<String> {
    let mut profiles = Vec::new();

    if let Some(aws_dir) = get_aws_config_dir() {
        // Parse credentials file
        let credentials_path = aws_dir.join("credentials");
        if let Ok(content) = fs::read_to_string(&credentials_path) {
            profiles.extend(parse_credentials_file(&content));
        }

        // Parse config file
        let config_path = aws_dir.join("config");
        if let Ok(content) = fs::read_to_string(&config_path) {
            profiles.extend(parse_config_file(&content));
        }
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    profiles.retain(|p| seen.insert(p.clone()));

    profiles
}

/// Tauri command to get available AWS CLI profiles
#[tauri::command]
pub async fn get_aws_profiles() -> Result<Vec<AwsProfile>, String> {
    let profiles = get_profiles_from_files();

    if profiles.is_empty() {
        return Err(
            "No AWS profiles found. Please configure AWS CLI with 'aws configure'.".to_string(),
        );
    }

    Ok(profiles
        .into_iter()
        .map(|name| AwsProfile { name })
        .collect())
}

/// Parse the region from AWS config file for a specific profile
/// Supports both `[profile name]` format (standard) and `[name]` format (some configs)
fn parse_region_from_config(content: &str, profile_name: &str) -> Option<String> {
    let mut in_target_section = false;

    // Build possible section headers
    // For "default": only "[default]"
    // For others: "[profile name]" (standard) or "[name]" (alternative)
    let section_headers: Vec<String> = if profile_name == "default" {
        vec!["[default]".to_string()]
    } else {
        vec![
            format!("[profile {}]", profile_name),
            format!("[{}]", profile_name),
        ]
    };

    for line in content.lines() {
        let trimmed = line.trim();

        // Check if we're entering a new section
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_target_section = section_headers.iter().any(|h| trimmed == h);
            continue;
        }

        // If we're in the target section, look for region
        if in_target_section {
            if let Some(region_value) = trimmed.strip_prefix("region") {
                let region_value = region_value.trim();
                if let Some(region) = region_value.strip_prefix('=') {
                    return Some(region.trim().to_string());
                }
            }
        }
    }

    None
}

/// Get the default region for a specific AWS profile
/// Reads from ~/.aws/config file
#[tauri::command]
pub async fn get_profile_region(profile: &str) -> Result<Option<String>, String> {
    let aws_dir =
        get_aws_config_dir().ok_or_else(|| "Could not find AWS config directory".to_string())?;

    let config_path = aws_dir.join("config");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read AWS config file: {}", e))?;

    Ok(parse_region_from_config(&content, profile))
}

/// Static list of AWS regions where Amplify is available
const AWS_REGIONS: &[&str] = &[
    "us-east-1",      // N. Virginia
    "us-east-2",      // Ohio
    "us-west-1",      // N. California
    "us-west-2",      // Oregon
    "ap-east-1",      // Hong Kong
    "ap-south-1",     // Mumbai
    "ap-northeast-1", // Tokyo
    "ap-northeast-2", // Seoul
    "ap-northeast-3", // Osaka
    "ap-southeast-1", // Singapore
    "ap-southeast-2", // Sydney
    "ca-central-1",   // Canada
    "eu-central-1",   // Frankfurt
    "eu-west-1",      // Ireland
    "eu-west-2",      // London
    "eu-west-3",      // Paris
    "eu-north-1",     // Stockholm
    "eu-south-1",     // Milan
    "me-south-1",     // Bahrain
    "sa-east-1",      // São Paulo
];

/// Tauri command to get available AWS regions
#[tauri::command]
pub fn get_aws_regions() -> Vec<String> {
    AWS_REGIONS.iter().map(|s| s.to_string()).collect()
}

use std::process::Command;

/// Response structure for AWS Amplify list-apps command
#[derive(Debug, Deserialize)]
struct AmplifyListAppsResponse {
    apps: Vec<AmplifyAppRaw>,
}

/// Raw Amplify app from AWS CLI response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyAppRaw {
    app_id: String,
    name: String,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    environment_variables: Option<HashMap<String, String>>,
}

/// Execute AWS CLI command with profile and region
fn execute_aws_command(args: &[&str], profile: &str, region: &str) -> Result<String, String> {
    let output = Command::new("aws")
        .args(args)
        .arg("--profile")
        .arg(profile)
        .arg("--region")
        .arg(region)
        .arg("--output")
        .arg("json")
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AWS CLI error: {}", stderr));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Failed to parse AWS CLI output: {}", e))
}

/// Tauri command to list Amplify apps in a region
#[tauri::command]
pub async fn list_amplify_apps(profile: &str, region: &str) -> Result<Vec<AmplifyApp>, String> {
    let output = execute_aws_command(&["amplify", "list-apps"], profile, region)?;

    let response: AmplifyListAppsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify apps response: {}", e))?;

    let apps = response
        .apps
        .into_iter()
        .map(|raw| AmplifyApp {
            app_id: raw.app_id,
            name: raw.name,
            repository: raw.repository.unwrap_or_default(),
            environment_variables: raw.environment_variables.unwrap_or_default(),
        })
        .collect();

    Ok(apps)
}

/// Response structure for AWS Amplify list-branches command
#[derive(Debug, Deserialize)]
struct AmplifyListBranchesResponse {
    branches: Vec<AmplifyBranchRaw>,
}

/// Raw Amplify branch from AWS CLI response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyBranchRaw {
    branch_name: String,
    #[serde(default)]
    backend: Option<AmplifyBackend>,
    #[serde(default)]
    backend_environment_arn: Option<String>,
    #[serde(default)]
    environment_variables: Option<HashMap<String, String>>,
}

/// Backend info for Amplify branch
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyBackend {
    #[serde(default)]
    stack_arn: Option<String>,
}

/// Tauri command to list branches for an Amplify app
#[tauri::command]
pub async fn list_amplify_branches(
    profile: &str,
    region: &str,
    app_id: &str,
) -> Result<Vec<AmplifyBranch>, String> {
    let output = execute_aws_command(
        &["amplify", "list-branches", "--app-id", app_id],
        profile,
        region,
    )?;

    let response: AmplifyListBranchesResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify branches response: {}", e))?;

    let branches = response
        .branches
        .into_iter()
        .map(|raw| {
            // Extract backend environment name from ARN
            // e.g., "arn:aws:amplify:ap-southeast-2:123456:apps/xxx/backendenvironments/prod" -> "prod"
            let backend_env_name = raw
                .backend_environment_arn
                .as_ref()
                .and_then(|arn| arn.split('/').last())
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw.branch_name.clone());

            AmplifyBranch {
                branch_name: raw.branch_name,
                stack_arn: raw.backend.and_then(|b| b.stack_arn).unwrap_or_default(),
                backend_environment_name: backend_env_name,
                environment_variables: raw.environment_variables.unwrap_or_default(),
                is_protected: false, // Will be checked from frontend
                protection_info: None,
            }
        })
        .collect();

    Ok(branches)
}

/// Environment variable change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarChange {
    pub level: String, // "app" or "branch"
    pub key: String,
    pub old_value: String,
    pub new_value: String,
}

/// Result of updating app environment variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEnvVarResult {
    pub success: bool,
    pub updated: bool,
    pub message: String,
    pub changes: Vec<EnvVarChange>,
}

/// Tauri command to update _CUSTOM_IMAGE environment variable for Gen2 Amplify apps
/// Removes _CUSTOM_IMAGE if it's set to an amplify:xx image to use default latest image
#[tauri::command]
pub async fn update_custom_image_env_var(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    current_env_vars: std::collections::HashMap<String, String>,
    branch_env_vars: std::collections::HashMap<String, String>,
) -> Result<UpdateEnvVarResult, String> {
    let mut update_messages = Vec::new();
    let mut changes = Vec::new();
    let mut app_updated = false;
    let mut branch_updated = false;

    println!("Current app env vars: {:?}", current_env_vars);
    println!("Current branch env vars: {:?}", branch_env_vars);

    // ===== Check and update APP-level _CUSTOM_IMAGE =====
    let mut app_needs_update = false;
    let mut updated_app_env_vars = current_env_vars.clone();

    // Check if _CUSTOM_IMAGE needs updating at app level
    match current_env_vars.get("_CUSTOM_IMAGE") {
        Some(value) => {
            // Check if it's an amplify:xx image that should be removed
            if value.contains("amplify:") {
                // Remove the _CUSTOM_IMAGE to use default latest image
                updated_app_env_vars.remove("_CUSTOM_IMAGE");
                update_messages.push(format!(
                    "App: _CUSTOM_IMAGE removed (was: {}), now using default latest Amplify image",
                    value
                ));
                changes.push(EnvVarChange {
                    level: "app".to_string(),
                    key: "_CUSTOM_IMAGE".to_string(),
                    old_value: value.clone(),
                    new_value: "".to_string(),
                });
                app_needs_update = true;
            } else {
                update_messages.push(format!("App: _CUSTOM_IMAGE is already set to {}", value));
            }
        }
        None => {
            update_messages
                .push("App: _CUSTOM_IMAGE not set, using default latest Amplify image".to_string());
        }
    }

    // ===== Check and update BRANCH-level _CUSTOM_IMAGE =====
    let mut branch_needs_update = false;
    let mut updated_branch_env_vars = branch_env_vars.clone();

    // Check if _CUSTOM_IMAGE needs updating at branch level
    match branch_env_vars.get("_CUSTOM_IMAGE") {
        Some(value) => {
            // Check if it's an amplify:xx image that should be removed
            if value.contains("amplify:") {
                // Remove the _CUSTOM_IMAGE to use default latest image
                updated_branch_env_vars.remove("_CUSTOM_IMAGE");
                update_messages.push(format!("Branch: _CUSTOM_IMAGE removed (was: {}), now using default latest Amplify image", value));
                changes.push(EnvVarChange {
                    level: "branch".to_string(),
                    key: "_CUSTOM_IMAGE".to_string(),
                    old_value: value.clone(),
                    new_value: "".to_string(),
                });
                branch_needs_update = true;
            } else {
                update_messages.push(format!("Branch: _CUSTOM_IMAGE is already set to {}", value));
            }
        }
        None => {
            update_messages.push(
                "Branch: _CUSTOM_IMAGE not set, using default latest Amplify image".to_string(),
            );
        }
    }

    // ===== Apply updates to AWS =====
    if app_needs_update {
        println!(
            "Updating app-level env vars. Removed _CUSTOM_IMAGE. New env vars: {:?}",
            updated_app_env_vars
        );

        // Get current environment variables from AWS to ensure we have the latest state
        let current_aws_env_vars = match get_current_app_env_vars(profile, region, app_id).await {
            Ok(vars) => vars,
            Err(e) => {
                eprintln!("Failed to get current app environment variables: {}", e);
                return Err(format!(
                    "Failed to get current app environment variables: {}",
                    e
                ));
            }
        };

        println!("Current AWS app env vars: {:?}", current_aws_env_vars);

        // Create final env vars by starting with current AWS state and applying our changes
        let mut final_env_vars = current_aws_env_vars;

        // Remove _CUSTOM_IMAGE if it exists
        let was_removed = final_env_vars.remove("_CUSTOM_IMAGE").is_some();
        if was_removed {
            println!("Removed _CUSTOM_IMAGE from final env vars");
        } else {
            println!("_CUSTOM_IMAGE was not present in current AWS env vars");
        }

        // If final env vars is empty, AWS Amplify requires a space key-value pair
        if final_env_vars.is_empty() {
            final_env_vars.insert(" ".to_string(), "".to_string());
            println!("Final env vars is empty, using space key-value pair for AWS compatibility");
        }

        println!("Final app env vars to send to AWS: {:?}", final_env_vars);

        match update_app_env_vars(profile, region, app_id, final_env_vars).await {
            Ok(_) => {
                app_updated = true;
                println!("Successfully updated app-level environment variables");

                // Verify the update by checking current state
                match get_current_app_env_vars(profile, region, app_id).await {
                    Ok(verification_vars) => {
                        println!(
                            "Verification - Current app env vars after update: {:?}",
                            verification_vars
                        );
                        if verification_vars.contains_key("_CUSTOM_IMAGE") {
                            println!("WARNING: _CUSTOM_IMAGE still exists after update!");
                        } else {
                            println!(
                                "SUCCESS: _CUSTOM_IMAGE has been removed from app-level env vars"
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed to verify app env vars after update: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to update app-level environment variables: {}", e);
                return Err(format!(
                    "Failed to update app-level environment variables: {}",
                    e
                ));
            }
        }
    }

    if branch_needs_update {
        println!(
            "Updating branch-level env vars. Removed _CUSTOM_IMAGE. New env vars: {:?}",
            updated_branch_env_vars
        );

        // Get current environment variables from AWS to ensure we have the latest state
        let current_aws_branch_env_vars =
            match get_current_branch_env_vars(profile, region, app_id, branch_name).await {
                Ok(vars) => vars,
                Err(e) => {
                    eprintln!("Failed to get current branch environment variables: {}", e);
                    return Err(format!(
                        "Failed to get current branch environment variables: {}",
                        e
                    ));
                }
            };

        println!(
            "Current AWS branch env vars: {:?}",
            current_aws_branch_env_vars
        );

        // Create final env vars by starting with current AWS state and applying our changes
        let mut final_branch_env_vars = current_aws_branch_env_vars;

        // Remove _CUSTOM_IMAGE if it exists
        let was_removed = final_branch_env_vars.remove("_CUSTOM_IMAGE").is_some();
        if was_removed {
            println!("Removed _CUSTOM_IMAGE from final branch env vars");
        } else {
            println!("_CUSTOM_IMAGE was not present in current AWS branch env vars");
        }

        // If final env vars is empty, AWS Amplify requires a space key-value pair
        if final_branch_env_vars.is_empty() {
            final_branch_env_vars.insert(" ".to_string(), "".to_string());
            println!(
                "Final branch env vars is empty, using space key-value pair for AWS compatibility"
            );
        }

        println!(
            "Final branch env vars to send to AWS: {:?}",
            final_branch_env_vars
        );

        match update_branch_env_vars(profile, region, app_id, branch_name, final_branch_env_vars)
            .await
        {
            Ok(_) => {
                branch_updated = true;
                println!("Successfully updated branch-level environment variables");

                // Verify the update by checking current state
                match get_current_branch_env_vars(profile, region, app_id, branch_name).await {
                    Ok(verification_vars) => {
                        println!(
                            "Verification - Current branch env vars after update: {:?}",
                            verification_vars
                        );
                        if verification_vars.contains_key("_CUSTOM_IMAGE") {
                            println!("WARNING: _CUSTOM_IMAGE still exists in branch after update!");
                        } else {
                            println!("SUCCESS: _CUSTOM_IMAGE has been removed from branch-level env vars");
                        }
                    }
                    Err(e) => {
                        println!("Failed to verify branch env vars after update: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to update branch-level environment variables: {}", e);
                return Err(format!(
                    "Failed to update branch-level environment variables: {}",
                    e
                ));
            }
        }
    }

    // Return result
    let success = true; // Always successful, even if no changes needed
    let updated = app_updated || branch_updated;
    let message = if update_messages.is_empty() {
        "_CUSTOM_IMAGE is already properly configured".to_string()
    } else {
        update_messages.join("\n")
    };

    Ok(UpdateEnvVarResult {
        success,
        updated,
        message,
        changes,
    })
}

/// Tauri command to update environment variables for an Amplify app
/// Updates _LIVE_UPDATES for @aws-amplify/cli to latest version (only if needed)
/// Also checks for AMPLIFY_BACKEND_PULL_ONLY at both app and branch level and changes it from "true" to "false" if present
#[tauri::command]
pub async fn update_live_updates_env_var(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    current_env_vars: std::collections::HashMap<String, String>,
    branch_env_vars: std::collections::HashMap<String, String>,
) -> Result<UpdateEnvVarResult, String> {
    use crate::file_ops::{check_if_live_updates_needs_update, update_live_updates_to_version};

    let mut update_messages = Vec::new();
    let mut changes = Vec::new();
    let mut app_updated = false;
    let mut branch_updated = false;

    // ===== Check and update APP-level environment variables =====
    let mut app_needs_update = false;
    let mut updated_app_env_vars = current_env_vars.clone();

    // Check if _LIVE_UPDATES needs updating (app level) - optimized logic
    let live_updates_json = current_env_vars.get("_LIVE_UPDATES").map(|s| s.as_str());
    let live_updates_target_version = match check_if_live_updates_needs_update(live_updates_json) {
        Ok(Some(version)) => Some(version),
        Ok(None) => None, // No update needed
        Err(e) => {
            // If we can't check, try to fetch latest version as fallback
            eprintln!("Warning: Failed to check _LIVE_UPDATES: {}", e);
            None
        }
    };

    // Update _LIVE_UPDATES if needed (app level)
    if let Some(target_version) = live_updates_target_version {
        if let Some(live_updates) = current_env_vars.get("_LIVE_UPDATES") {
            let updated_live_updates =
                update_live_updates_to_version(live_updates, &target_version)?;
            updated_app_env_vars.insert("_LIVE_UPDATES".to_string(), updated_live_updates.clone());
            update_messages.push(format!(
                "App: _LIVE_UPDATES → @aws-amplify/cli version set to {}",
                target_version
            ));
            app_needs_update = true;
        }
    }

    // Check if AMPLIFY_BACKEND_PULL_ONLY needs updating (app level)
    let app_pull_only_needs_update = match current_env_vars.get("AMPLIFY_BACKEND_PULL_ONLY") {
        Some(value) => value == "true",
        None => false,
    };

    if app_pull_only_needs_update {
        let old_value = current_env_vars
            .get("AMPLIFY_BACKEND_PULL_ONLY")
            .unwrap()
            .clone();
        updated_app_env_vars.insert("AMPLIFY_BACKEND_PULL_ONLY".to_string(), "false".to_string());
        update_messages.push("App: AMPLIFY_BACKEND_PULL_ONLY → true to false".to_string());
        changes.push(EnvVarChange {
            level: "app".to_string(),
            key: "AMPLIFY_BACKEND_PULL_ONLY".to_string(),
            old_value,
            new_value: "false".to_string(),
        });
        app_needs_update = true;
    }

    // Check if _CUSTOM_IMAGE needs to be deleted (app level only)
    // _CUSTOM_IMAGE scenarios:
    // 1. Doesn't exist → OK, using default latest Amplify image
    // 2. Starts with "amplify:" → Delete it (legacy Amplify image)
    // 3. Starts with something else → Show info but don't change (custom image, customer's responsibility)
    let app_custom_image_action = match current_env_vars.get("_CUSTOM_IMAGE") {
        Some(value) if value.starts_with("amplify:") => "delete", // Legacy Amplify image
        Some(_) => "custom",                                      // Custom image, don't touch
        None => "none",                                           // Not set, using default
    };

    if app_custom_image_action == "delete" {
        let old_value = current_env_vars.get("_CUSTOM_IMAGE").unwrap().clone();
        updated_app_env_vars.remove("_CUSTOM_IMAGE");
        update_messages.push(format!(
            "App: _CUSTOM_IMAGE → deleted (was: {}, now using default latest Amplify image)",
            old_value
        ));
        // Note: NOT adding to changes array - this should not be reverted
        app_needs_update = true;
    }

    // Update app environment variables if needed
    if app_needs_update {
        // If updated env vars is empty, AWS Amplify requires a space key-value pair
        if updated_app_env_vars.is_empty() {
            updated_app_env_vars.insert(" ".to_string(), "".to_string());
        }

        let env_vars_json = serde_json::to_string(&updated_app_env_vars)
            .map_err(|e| format!("Failed to serialize app environment variables: {}", e))?;

        let output = Command::new("aws")
            .args([
                "amplify",
                "update-app",
                "--app-id",
                app_id,
                "--environment-variables",
                &env_vars_json,
                "--profile",
                profile,
                "--region",
                region,
            ])
            .output()
            .map_err(|e| format!("Failed to execute AWS CLI for app update: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Failed to update app environment variables: {}",
                stderr
            ));
        }
        app_updated = true;
    }

    // ===== Check and update BRANCH-level environment variables =====
    let mut branch_needs_update = false;
    let mut updated_branch_env_vars = branch_env_vars.clone();

    // Check if AMPLIFY_BACKEND_PULL_ONLY needs updating (branch level)
    let branch_pull_only_needs_update = match branch_env_vars.get("AMPLIFY_BACKEND_PULL_ONLY") {
        Some(value) => value == "true",
        None => false,
    };

    if branch_pull_only_needs_update {
        let old_value = branch_env_vars
            .get("AMPLIFY_BACKEND_PULL_ONLY")
            .unwrap()
            .clone();
        updated_branch_env_vars
            .insert("AMPLIFY_BACKEND_PULL_ONLY".to_string(), "false".to_string());
        update_messages.push(format!(
            "Branch ({}): AMPLIFY_BACKEND_PULL_ONLY → true to false",
            branch_name
        ));
        changes.push(EnvVarChange {
            level: "branch".to_string(),
            key: "AMPLIFY_BACKEND_PULL_ONLY".to_string(),
            old_value,
            new_value: "false".to_string(),
        });
        branch_needs_update = true;
    }

    // Update branch environment variables if needed
    if branch_needs_update {
        // If updated env vars is empty, AWS Amplify requires a space key-value pair
        if updated_branch_env_vars.is_empty() {
            updated_branch_env_vars.insert(" ".to_string(), "".to_string());
        }

        let env_vars_json = serde_json::to_string(&updated_branch_env_vars)
            .map_err(|e| format!("Failed to serialize branch environment variables: {}", e))?;

        let output = Command::new("aws")
            .args([
                "amplify",
                "update-branch",
                "--app-id",
                app_id,
                "--branch-name",
                branch_name,
                "--environment-variables",
                &env_vars_json,
                "--profile",
                profile,
                "--region",
                region,
            ])
            .output()
            .map_err(|e| format!("Failed to execute AWS CLI for branch update: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Failed to update branch environment variables: {}",
                stderr
            ));
        }
        branch_updated = true;
    }

    // Return result
    if !app_updated && !branch_updated {
        let mut message = String::new();

        // Check _LIVE_UPDATES (app level only)
        if current_env_vars.contains_key("_LIVE_UPDATES") {
            message.push_str("@aws-amplify/cli is already set to latest");
        }

        // Check AMPLIFY_BACKEND_PULL_ONLY (both levels)
        if current_env_vars.contains_key("AMPLIFY_BACKEND_PULL_ONLY")
            || branch_env_vars.contains_key("AMPLIFY_BACKEND_PULL_ONLY")
        {
            if !message.is_empty() {
                message.push_str("\n");
            }
            message.push_str("AMPLIFY_BACKEND_PULL_ONLY is already set to false or not present");
        }

        // Check _CUSTOM_IMAGE status (app level only)
        // Three scenarios:
        // 1. Doesn't exist → OK, using default latest Amplify image
        // 2. Starts with "amplify:" → Should have been deleted (but we're in the no-update path, so it wasn't)
        // 3. Starts with something else → Custom image, show info
        match current_env_vars.get("_CUSTOM_IMAGE") {
            None => {
                // Not set, using default latest Amplify image
                if !message.is_empty() {
                    message.push_str("\n");
                }
                message.push_str("_CUSTOM_IMAGE not set, using default latest Amplify image");
            }
            Some(value) if value.starts_with("amplify:") => {
                // This shouldn't happen in the no-update path, but handle it
                // (it means the deletion logic didn't trigger, which is a bug)
            }
            Some(value) => {
                // Custom image
                if !message.is_empty() {
                    message.push_str("\n");
                }
                message.push_str(&format!(
                    "_CUSTOM_IMAGE is set to custom image: {} (ensure it has required dependencies)",
                    value
                ));
            }
        }

        if message.is_empty() {
            message = "No environment variable updates needed".to_string();
        }
        return Ok(UpdateEnvVarResult {
            success: true,
            updated: false,
            message,
            changes: vec![],
        });
    }

    let message = format!(
        "Updated environment variables:\n{}",
        update_messages.join("\n")
    );

    Ok(UpdateEnvVarResult {
        success: true,
        updated: true,
        message,
        changes,
    })
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
        Err(e) => {
            println!("Strategy 1 error: {}", e);
        }
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
            Err(e) => {
                println!("Strategy 2 JSON parse error: {}", e);
            }
        },
        Err(e) => {
            println!("Strategy 2 error: {}", e);
        }
    }

    // Strategy 3: Final fallback - list all functions and filter by name pattern
    get_lambda_functions_by_name_pattern(profile, region, app_id, branch_name).await
}

/// Get app by app ID
async fn get_app_by_id(profile: &str, region: &str, app_id: &str) -> Result<AmplifyApp, String> {
    let apps = list_amplify_apps(profile, region).await?;
    apps.into_iter()
        .find(|app| app.app_id == app_id)
        .ok_or_else(|| format!("App with ID {} not found", app_id))
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
            Err(e) => {
                println!("  Error getting details: {}", e);
            }
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
        return tag.value.to_lowercase() == repo_name.to_lowercase();
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

/// Extract repository name from a repository URL
/// e.g., "https://github.com/lwang-79/todo-gen2" -> "todo-gen2"
fn extract_repo_name(repository_url: &str) -> String {
    // Handle various URL formats:
    // https://github.com/user/repo
    // https://github.com/user/repo.git
    // git@github.com:user/repo.git
    let url = repository_url.trim_end_matches(".git");

    // Split by '/' and get the last part
    url.split('/')
        .last()
        .or_else(|| url.split(':').last()) // Handle git@github.com:user/repo format
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
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

/// Response structure for AWS Amplify list-jobs command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyListJobsResponse {
    job_summaries: Vec<AmplifyJobSummary>,
}

/// Job summary from list-jobs response
#[derive(Debug, Deserialize)]
struct AmplifyJobSummary {
    #[serde(alias = "job_id", alias = "jobId", alias = "JobId")]
    job_id: String,
    #[serde(alias = "commit_id", alias = "commitId", alias = "CommitId")]
    commit_id: String,
    #[serde(alias = "Status")]
    status: String,
    #[serde(
        default,
        alias = "start_time",
        alias = "startTime",
        alias = "StartTime"
    )]
    start_time: Option<String>,
}

/// Response structure for AWS Amplify get-job command
#[derive(Debug, Deserialize)]
struct AmplifyGetJobResponse {
    job: AmplifyJobWrapper,
}

/// Job wrapper from get-job response
#[derive(Debug, Deserialize)]
struct AmplifyJobWrapper {
    summary: AmplifyJobDetail,
}

/// Job detail from get-job response
#[derive(Debug, Deserialize)]
struct AmplifyJobDetail {
    #[serde(alias = "job_id", alias = "jobId", alias = "JobId")]
    job_id: String,
    #[serde(alias = "commit_id", alias = "commitId", alias = "CommitId")]
    commit_id: String,
    #[serde(alias = "Status")]
    status: String,
    #[serde(
        default,
        alias = "start_time",
        alias = "startTime",
        alias = "StartTime"
    )]
    start_time: Option<String>,
    #[serde(default, alias = "end_time", alias = "endTime", alias = "EndTime")]
    end_time: Option<String>,
}

/// Tauri command to list Amplify jobs for a specific commit
#[tauri::command]
pub async fn list_amplify_jobs(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    commit_id: &str,
) -> Result<Vec<AmplifyJob>, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "list-jobs",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--max-results",
            "10",
        ],
        profile,
        region,
    )?;

    let response: AmplifyListJobsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify jobs response: {}", e))?;

    // Filter jobs by commit ID
    let jobs: Vec<AmplifyJob> = response
        .job_summaries
        .into_iter()
        .filter(|job| job.commit_id == commit_id)
        .map(|job| AmplifyJob {
            job_id: job.job_id,
            commit_id: job.commit_id,
            status: job.status,
            start_time: job.start_time,
        })
        .collect();

    Ok(jobs)
}

/// Tauri command to get details of a specific Amplify job
#[tauri::command]
pub async fn get_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    job_id: &str,
) -> Result<AmplifyJobDetails, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "get-job",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--job-id",
            job_id,
        ],
        profile,
        region,
    )?;

    let response: AmplifyGetJobResponse = serde_json::from_str(&output).map_err(|e| {
        println!("[get_amplify_job] Parse error: {}", e);
        format!("Failed to parse Amplify job response: {}", e)
    })?;

    Ok(AmplifyJobDetails {
        job_id: response.job.summary.job_id,
        commit_id: response.job.summary.commit_id,
        status: response.job.summary.status,
        start_time: response.job.summary.start_time,
        end_time: response.job.summary.end_time,
    })
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
    fn test_extract_repo_name() {
        // Standard GitHub HTTPS URL
        assert_eq!(
            extract_repo_name("https://github.com/lwang-79/todo-gen2"),
            "todo-gen2"
        );
        // GitHub URL with .git suffix
        assert_eq!(
            extract_repo_name("https://github.com/user/my-app.git"),
            "my-app"
        );
        // Git SSH URL
        assert_eq!(
            extract_repo_name("git@github.com:user/my-repo.git"),
            "my-repo"
        );
        // CodeCommit URL
        assert_eq!(
            extract_repo_name("https://git-codecommit.us-east-1.amazonaws.com/v1/repos/my-project"),
            "my-project"
        );
        // Empty string
        assert_eq!(extract_repo_name(""), "");
        // Just repo name
        assert_eq!(extract_repo_name("simple-repo"), "simple-repo");
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

    #[test]
    fn test_parse_credentials_file() {
        let content = r#"
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[production]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
"#;
        let profiles = parse_credentials_file(content);
        assert_eq!(profiles, vec!["default", "production"]);
    }

    #[test]
    fn test_parse_config_file() {
        let content = r#"
[default]
region = us-east-1

[profile development]
region = us-west-2

[profile production]
region = eu-west-1
"#;
        let profiles = parse_config_file(content);
        assert_eq!(profiles, vec!["default", "development", "production"]);
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
/// Tauri command to update app-level environment variables
#[tauri::command]
pub async fn update_app_env_vars(
    profile: &str,
    region: &str,
    app_id: &str,
    mut env_vars: std::collections::HashMap<String, String>,
) -> Result<bool, String> {
    // If env vars is empty, AWS Amplify requires a space key-value pair
    if env_vars.is_empty() {
        env_vars.insert(" ".to_string(), "".to_string());
    }

    let env_vars_json = serde_json::to_string(&env_vars)
        .map_err(|e| format!("Failed to serialize app environment variables: {}", e))?;

    // Log the exact command being executed
    let command_args = [
        "amplify",
        "update-app",
        "--app-id",
        app_id,
        "--environment-variables",
        &env_vars_json,
        "--profile",
        profile,
        "--region",
        region,
    ];
    println!("Executing AWS CLI command: aws {}", command_args.join(" "));
    println!("Environment variables JSON being sent: {}", env_vars_json);

    let output = Command::new("aws")
        .args(command_args)
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for app update: {}", e))?;

    // Log the full response
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("AWS CLI stdout: {}", stdout);
    println!("AWS CLI stderr: {}", stderr);
    println!("AWS CLI exit code: {}", output.status.code().unwrap_or(-1));

    if !output.status.success() {
        return Err(format!(
            "Failed to update app environment variables: {}",
            stderr
        ));
    }

    Ok(true)
}

/// Tauri command to update branch-level environment variables
#[tauri::command]
pub async fn update_branch_env_vars(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    mut env_vars: std::collections::HashMap<String, String>,
) -> Result<bool, String> {
    // If env vars is empty, AWS Amplify requires a space key-value pair
    if env_vars.is_empty() {
        env_vars.insert(" ".to_string(), "".to_string());
    }

    let env_vars_json = serde_json::to_string(&env_vars)
        .map_err(|e| format!("Failed to serialize branch environment variables: {}", e))?;

    // Log the exact command being executed
    let command_args = [
        "amplify",
        "update-branch",
        "--app-id",
        app_id,
        "--branch-name",
        branch_name,
        "--environment-variables",
        &env_vars_json,
        "--profile",
        profile,
        "--region",
        region,
    ];
    println!("Executing AWS CLI command: aws {}", command_args.join(" "));
    println!("Environment variables JSON being sent: {}", env_vars_json);

    let output = Command::new("aws")
        .args(command_args)
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for branch update: {}", e))?;

    // Log the full response
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("AWS CLI stdout: {}", stdout);
    println!("AWS CLI stderr: {}", stderr);
    println!("AWS CLI exit code: {}", output.status.code().unwrap_or(-1));

    if !output.status.success() {
        return Err(format!(
            "Failed to update branch environment variables: {}",
            stderr
        ));
    }

    Ok(true)
}

/// Tauri command to get current app-level environment variables from AWS
#[tauri::command]
pub async fn get_current_app_env_vars(
    profile: &str,
    region: &str,
    app_id: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let output = execute_aws_command(&["amplify", "get-app", "--app-id", app_id], profile, region)?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse get-app response: {}", e))?;

    let env_vars = response["app"]["environmentVariables"]
        .as_object()
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(env_vars)
}

/// Tauri command to get current branch-level environment variables from AWS
#[tauri::command]
pub async fn get_current_branch_env_vars(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "get-branch",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
        ],
        profile,
        region,
    )?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse get-branch response: {}", e))?;

    let env_vars = response["branch"]["environmentVariables"]
        .as_object()
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(env_vars)
}

/// Tauri command to start/retry an Amplify job
#[tauri::command]
pub async fn start_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    job_type: &str,
    job_id: Option<&str>,
) -> Result<String, String> {
    let mut args = vec![
        "amplify",
        "start-job",
        "--app-id",
        app_id,
        "--branch-name",
        branch_name,
        "--job-type",
        job_type,
    ];

    // Add job-id if provided (for RETRY)
    let job_id_string;
    if let Some(id) = job_id {
        job_id_string = id.to_string();
        args.push("--job-id");
        args.push(&job_id_string);
    }

    let output = execute_aws_command(&args, profile, region)?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse start-job response: {}", e))?;

    let new_job_id = response["jobSummary"]["jobId"]
        .as_str()
        .ok_or_else(|| "Failed to get job ID from response".to_string())?
        .to_string();

    Ok(new_job_id)
}

/// Tauri command to get the most recent job for a branch
#[tauri::command]
pub async fn get_latest_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
) -> Result<Option<AmplifyJobDetails>, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "list-jobs",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--max-results",
            "1",
        ],
        profile,
        region,
    )?;

    let response: AmplifyListJobsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse list-jobs response: {}", e))?;

    if response.job_summaries.is_empty() {
        println!("[get_latest_amplify_job] No jobs found, returning None");
        return Ok(None);
    }

    let latest_job = &response.job_summaries[0];

    // Return job details directly from the list-jobs response
    // We have all the info we need, no need to call get-job which has parsing issues
    let job_details = AmplifyJobDetails {
        job_id: latest_job.job_id.clone(),
        commit_id: latest_job.commit_id.clone(),
        status: latest_job.status.clone(),
        start_time: latest_job.start_time.clone(),
        end_time: None, // list-jobs doesn't include end_time, but we don't need it for retry
    };

    Ok(Some(job_details))
}

/// Get the buildSpec for an Amplify app
/// Calls `aws amplify get-app --app-id <id>` and extracts the buildSpec field
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
///
/// # Returns
/// The buildSpec string from the app configuration
pub fn get_app_build_spec(profile: &str, region: &str, app_id: &str) -> Result<String, String> {
    let output = execute_aws_command(&["amplify", "get-app", "--app-id", app_id], profile, region)?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse get-app response: {}", e))?;

    let build_spec = response["app"]["buildSpec"]
        .as_str()
        .ok_or_else(|| "buildSpec field not found in app configuration".to_string())?
        .to_string();

    Ok(build_spec)
}

/// Update the buildSpec for an Amplify app
/// Calls `aws amplify update-app --app-id <id> --build-spec <spec>`
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
/// * `build_spec` - The new buildSpec YAML string
///
/// # Returns
/// `Ok(true)` on success, `Err` with error message on failure
pub fn update_app_build_spec(
    profile: &str,
    region: &str,
    app_id: &str,
    build_spec: &str,
) -> Result<bool, String> {
    let output = Command::new("aws")
        .args([
            "amplify",
            "update-app",
            "--app-id",
            app_id,
            "--build-spec",
            build_spec,
            "--profile",
            profile,
            "--region",
            region,
        ])
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for buildSpec update: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to update app buildSpec: {}", stderr));
    }

    Ok(true)
}

/// Revert the buildSpec for an Amplify app to its original value
/// This is a wrapper around update_app_build_spec for clarity
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
/// * `original_build_spec` - The original buildSpec YAML string to restore
///
/// # Returns
/// `Ok(true)` on success, `Err` with error message on failure
#[tauri::command]
pub fn revert_build_spec(
    profile: &str,
    region: &str,
    app_id: &str,
    original_build_spec: &str,
) -> Result<bool, String> {
    update_app_build_spec(profile, region, app_id, original_build_spec)
}
