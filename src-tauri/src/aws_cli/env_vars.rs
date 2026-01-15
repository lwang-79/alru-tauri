use super::core::execute_aws_command;
use crate::amplify::{check_if_live_updates_needs_update, update_live_updates_to_version};
use serde::{Deserialize, Serialize};

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
        // Get current environment variables from AWS to ensure we have the latest state
        let current_aws_env_vars = match get_current_app_env_vars(profile, region, app_id).await {
            Ok(vars) => vars,
            Err(e) => {
                // Error is returned to caller
                return Err(format!(
                    "Failed to get current app environment variables: {}",
                    e
                ));
            }
        };

        // Create final env vars by starting with current AWS state and applying our changes
        let mut final_env_vars = current_aws_env_vars;

        // Remove _CUSTOM_IMAGE if it exists
        final_env_vars.remove("_CUSTOM_IMAGE");

        // If final env vars is empty, AWS Amplify requires a space key-value pair
        if final_env_vars.is_empty() {
            final_env_vars.insert(" ".to_string(), "".to_string());
        }

        match update_app_env_vars(profile, region, app_id, final_env_vars).await {
            Ok(_) => {
                app_updated = true;
            }
            Err(e) => {
                // Error is returned to caller
                return Err(format!(
                    "Failed to update app-level environment variables: {}",
                    e
                ));
            }
        }
    }

    if branch_needs_update {
        // Get current environment variables from AWS to ensure we have the latest state
        let current_aws_branch_env_vars =
            match get_current_branch_env_vars(profile, region, app_id, branch_name).await {
                Ok(vars) => vars,
                Err(e) => {
                    // Error is returned to caller
                    return Err(format!(
                        "Failed to get current branch environment variables: {}",
                        e
                    ));
                }
            };

        // Create final env vars by starting with current AWS state and applying our changes
        let mut final_branch_env_vars = current_aws_branch_env_vars;

        // Remove _CUSTOM_IMAGE if it exists
        final_branch_env_vars.remove("_CUSTOM_IMAGE");

        // If final env vars is empty, AWS Amplify requires a space key-value pair
        if final_branch_env_vars.is_empty() {
            final_branch_env_vars.insert(" ".to_string(), "".to_string());
        }

        match update_branch_env_vars(profile, region, app_id, branch_name, final_branch_env_vars)
            .await
        {
            Ok(_) => {
                branch_updated = true;
            }
            Err(e) => {
                // Error is returned to caller
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

        let output = crate::command::create_clean_shell_command("aws")
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

        let output = crate::command::create_clean_shell_command("aws")
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

    let output = crate::command::create_clean_shell_command("aws")
        .args(command_args)
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for app update: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
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

    let output = crate::command::create_clean_shell_command("aws")
        .args(command_args)
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for branch update: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
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
