use super::env::fetch_latest_amplify_cli_version;
use crate::command::create_clean_shell_command;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Result of upgrading Amplify CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub success: bool,
    pub skipped: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub message: String,
}

/// Runs `amplify pull` command with streaming output.
/// For Gen1 apps, uses headless mode with environment variables.
/// For Gen2 apps, uses the standard command.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `app_id` - The Amplify app ID
/// * `env_name` - The Amplify environment name (typically the branch name)
/// * `backend_type` - The backend type ("Gen1" or "Gen2")
/// * `profile_name` - AWS profile name (required for Gen1 headless mode)
/// * `window` - Tauri window for emitting events
///
/// # Returns
/// * `Ok(true)` - Pull successful
/// * `Err(String)` - Error message if pull fails
///
/// # Events Emitted
/// * `prepare-output` - Emitted with each line of output
/// * `prepare-status` - Emitted with status updates
#[tauri::command]
pub async fn amplify_pull_streaming(
    project_path: String,
    app_id: String,
    env_name: String,
    profile_name: String,
    window: tauri::Window,
) -> Result<bool, String> {
    // Emit initial status
    let _ = window.emit(
        "prepare-output",
        format!("\n=== Pulling Amplify Project ===\n"),
    );
    let _ = window.emit("prepare-output", format!("App ID: {}\n", app_id));
    let _ = window.emit("prepare-output", format!("Environment: {}\n\n", env_name));

    // For Gen1, use headless mode with environment variables
    let _ = window.emit("prepare-output", "Using headless mode for Gen1 app...\n");

    // Create environment variables for headless mode
    let aws_cloudformation_config = format!(
        r#"{{"configLevel":"project","useProfile":true,"profileName":"{}"}}"#,
        profile_name
    );
    let amplify_config = format!(
        r#"{{"appId":"{}","envName":"{}","defaultEditor":"code"}}"#,
        app_id, env_name
    );
    let providers_config = format!(r#"{{"awscloudformation":{}}}"#, aws_cloudformation_config);

    let _ = window.emit(
        "prepare-output",
        format!(
            "Executing command: amplify pull --amplify '{}' --providers '{}' --yes\n\n",
            amplify_config, providers_config
        ),
    );

    let mut command = create_clean_shell_command("amplify");
    command
        .args([
            "pull",
            "--amplify",
            &amplify_config,
            "--providers",
            &providers_config,
            "--yes",
        ])
        .current_dir(&project_path);

    match crate::command::run_command_streaming(command, &window, "prepare-output") {
        Ok(true) => {
            let _ = window.emit(
                "prepare-output",
                "\n✔ Amplify pull completed successfully!\n",
            );
            Ok(true)
        }
        Ok(false) => {
            let _ = window.emit("prepare-output", "\n✗ Amplify pull failed!\n");
            Err("Amplify pull failed".to_string())
        }
        Err(e) => {
            let _ = window.emit("prepare-output", format!("\n✗ Error: {}\n", e));
            Err(e)
        }
    }
}

/// Runs `amplify env checkout <envName> --yes` with streaming output.
/// This switches to the correct Amplify environment.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `env_name` - The Amplify environment name
/// * `window` - Tauri window for emitting events
///
/// # Returns
/// * `Ok(true)` - Environment checkout successful
/// * `Err(String)` - Error message if checkout fails
///
/// # Events Emitted
/// * `prepare-output` - Emitted with each line of output
/// * `prepare-status` - Emitted with status updates
#[tauri::command]
pub async fn amplify_env_checkout_streaming(
    project_path: String,
    env_name: String,
    window: tauri::Window,
) -> Result<bool, String> {
    use tauri::Emitter;

    // Emit initial status
    let _ = window.emit(
        "prepare-output",
        format!("\n=== Checking out Amplify Environment ===\n"),
    );
    let _ = window.emit("prepare-output", format!("Environment: {}\n\n", env_name));

    let mut command = create_clean_shell_command("amplify");
    command
        .args(["env", "checkout", &env_name, "--yes"])
        .current_dir(&project_path);

    match crate::command::run_command_streaming(command, &window, "prepare-output") {
        Ok(true) => {
            let _ = window.emit(
                "prepare-output",
                "\n✔ Environment checkout completed successfully!\n",
            );
            Ok(true)
        }
        Ok(false) => {
            let _ = window.emit("prepare-output", "\n✗ Environment checkout failed!\n");
            Err("Amplify environment checkout failed. Available environments may differ from branch names.".to_string())
        }
        Err(e) => {
            let _ = window.emit("prepare-output", format!("\n✗ Error: {}\n", e));
            Err(e)
        }
    }
}

/// Runs `amplify pull` command to pull the existing Amplify project.
/// For Gen1 apps, uses headless mode with environment variables.
/// For Gen2 apps, uses the standard command.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `app_id` - The Amplify app ID
/// * `env_name` - The Amplify environment name (typically the branch name)
/// * `backend_type` - The backend type ("Gen1" or "Gen2")
/// * `profile_name` - AWS profile name (required for Gen1 headless mode)
///
/// # Returns
/// * `Ok(true)` - Pull successful
/// * `Err(String)` - Error message if pull fails
#[tauri::command]
pub async fn amplify_pull(
    project_path: &str,
    app_id: &str,
    env_name: &str,
    profile_name: &str,
) -> Result<bool, String> {
    // For Gen1, use headless mode with environment variables
    let aws_cloudformation_config = format!(
        r#"{{"configLevel":"project","useProfile":true,"profileName":"{}"}}"#,
        profile_name
    );
    let amplify_config = format!(
        r#"{{"appId":"{}","envName":"{}","defaultEditor":"code"}}"#,
        app_id, env_name
    );
    let providers_config = format!(r#"{{"awscloudformation":{}}}"#, aws_cloudformation_config);

    let output = create_clean_shell_command("amplify")
        .args([
            "pull",
            "--amplify",
            &amplify_config,
            "--providers",
            &providers_config,
            "--yes",
        ])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to execute amplify pull (headless): {}", e))?;

    if output.status.success() {
        println!("[amplify_pull] Successfully pulled Amplify project");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined_output = format!("{}{}", stdout, stderr);

        Err(format!("Amplify pull failed: {}", combined_output))
    }
}

/// Runs `amplify env checkout <envName>` to switch to the correct Amplify environment.
/// This is required for Gen1 backends to ensure the correct CloudFormation templates are present.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `env_name` - The Amplify environment name (typically the branch name)
///
/// # Returns
/// * `Ok(true)` - Environment checkout successful
/// * `Err(String)` - Error message if checkout fails
#[tauri::command]
pub async fn amplify_env_checkout(project_path: &str, env_name: &str) -> Result<bool, String> {
    // Run amplify env checkout
    let output = create_clean_shell_command("amplify")
        .args(["env", "checkout", env_name, "--yes"])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to execute amplify env checkout: {}", e))?;

    if output.status.success() {
        println!(
            "[amplify_env_checkout] Successfully checked out environment {}",
            env_name
        );
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if the error is because the env doesn't exist
        let combined_output = format!("{}{}", stdout, stderr);
        if combined_output.contains("doesn't exist") || combined_output.contains("does not exist") {
            return Err(format!(
                "Amplify environment '{}' does not exist. Available environments may differ from branch names.",
                env_name
            ));
        }

        Err(format!("Amplify env checkout failed: {}", combined_output))
    }
}

/// Gets the currently installed version of Amplify CLI
fn get_installed_amplify_cli_version() -> Option<String> {
    // Try direct check with --version
    let output = create_clean_shell_command("amplify")
        .arg("--version")
        .output();

    let output = match output {
        Ok(out) => {
            if out.status.success() {
                Ok(out)
            } else {
                // Try via shell if direct failed
                crate::command::run_clean_sh_c("amplify --version")
            }
        }
        Err(_) => {
            // Try via shell if direct failed
            crate::command::run_clean_sh_c("amplify --version")
        }
    };

    if let Ok(out) = output {
        if out.status.success() {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }

    None
}

// Trait etc moved to crate::command

/// Upgrades the Amplify CLI to the latest version globally.
/// This is required for Gen1 backends before building.
/// Skips upgrade if already at the latest version.
///
/// # Returns
/// * `Ok(UpgradeResult)` - Result with upgrade status
/// * `Err(String)` - Error message if upgrade fails
#[tauri::command]
pub async fn upgrade_amplify_cli() -> Result<UpgradeResult, String> {
    // Get current installed version
    let current_version = get_installed_amplify_cli_version();
    println!("[amplify_upgrade] Current version: {:?}", current_version);

    // Get latest version from npm
    let latest_version = fetch_latest_amplify_cli_version()?;
    println!("[amplify_upgrade] Latest version: {}", latest_version);

    // Check if upgrade is needed
    if let Some(ref current) = current_version {
        if current == &latest_version {
            return Ok(UpgradeResult {
                success: true,
                skipped: true,
                current_version: Some(current.clone()),
                latest_version: Some(latest_version),
                message: format!("Amplify CLI is already at the latest version ({})", current),
            });
        }
    }

    // Perform upgrade
    let output = create_clean_shell_command("npm")
        .args(["install", "-g", "@aws-amplify/cli@latest"])
        .output()
        .map_err(|e| format!("Failed to execute npm install: {}", e))?;

    if output.status.success() {
        Ok(UpgradeResult {
            success: true,
            skipped: false,
            current_version,
            latest_version: Some(latest_version),
            message: "Amplify CLI upgraded successfully".to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to upgrade Amplify CLI: {}", stderr))
    }
}
