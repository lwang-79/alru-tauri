use super::detector::PackageManager;
use crate::command::CommandExtClean;
use std::process::Command;

/// Installs dependencies using the appropriate package manager command with streaming output
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `package_manager` - The package manager to use
/// * `window` - Tauri window for emitting events
///
/// # Returns
/// * `Ok(true)` - Dependencies installed successfully
/// * `Err(String)` - Error message if installation fails
///
/// # Events Emitted
/// * `prepare-output` - Emitted with each line of output
/// * `prepare-status` - Emitted with status updates ("running", "completed", "failed")
#[tauri::command]
pub async fn install_dependencies_streaming(
    project_path: String,
    package_manager: PackageManager,
    window: tauri::Window,
) -> Result<bool, String> {
    let (cmd, args) = match package_manager {
        PackageManager::Npm => ("npm", vec!["install"]),
        PackageManager::Yarn => ("yarn", vec!["install"]),
        PackageManager::Pnpm => ("pnpm", vec!["install"]),
        PackageManager::Bun => ("bun", vec!["install"]),
    };

    use tauri::Emitter;
    let _ = window.emit("prepare-status", "running");
    let _ = window.emit(
        "prepare-output",
        format!("=== Installing Dependencies ===\n"),
    );
    let _ = window.emit("prepare-output", format!("Package Manager: {}\n", cmd));
    let _ = window.emit(
        "prepare-output",
        format!("Working directory: {}\n\n", project_path),
    );

    let mut command = Command::new(cmd).clean_env();
    command.args(&args).current_dir(&project_path);

    match crate::command::run_command_streaming(command, &window, "prepare-output") {
        Ok(true) => {
            let _ = window.emit(
                "prepare-output",
                "\n✔ Dependencies installed successfully!\n",
            );
            let _ = window.emit("prepare-status", "completed");
            Ok(true)
        }
        Ok(false) => {
            let _ = window.emit("prepare-output", "\n✗ Dependency installation failed!\n");
            let _ = window.emit("prepare-status", "failed");
            Err("Dependency installation failed".to_string())
        }
        Err(e) => {
            let _ = window.emit("prepare-output", format!("\n✗ Error: {}\n", e));
            let _ = window.emit("prepare-status", "failed");
            Err(e)
        }
    }
}

/// Installs dependencies using the appropriate package manager command
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `package_manager` - The package manager to use
///
/// # Returns
/// * `Ok(true)` - Dependencies installed successfully
/// * `Err(String)` - Error message if installation fails
#[tauri::command]
pub async fn install_dependencies(
    project_path: &str,
    package_manager: PackageManager,
) -> Result<bool, String> {
    let (cmd, args) = match package_manager {
        PackageManager::Npm => ("npm", vec!["install"]),
        PackageManager::Yarn => ("yarn", vec!["install"]),
        PackageManager::Pnpm => ("pnpm", vec!["install"]),
        PackageManager::Bun => ("bun", vec!["install"]),
    };

    // Log the command being executed
    println!(
        "[install_dependencies] Executing: {} {}",
        cmd,
        args.join(" ")
    );
    println!("[install_dependencies] Working directory: {}", project_path);

    let output = Command::new(cmd)
        .clean_env()
        .args(&args)
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", cmd, e))?;

    if output.status.success() {
        println!("[install_dependencies] Dependencies installed successfully");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[install_dependencies] Installation failed: {}", stderr);
        Err(format!("Dependency installation failed: {}", stderr))
    }
}

/// Upgrades @aws-amplify/backend and @aws-amplify/backend-cli to the latest versions.
/// This is required for Gen2 backends before building.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `package_manager` - The package manager to use
///
/// # Returns
/// * `Ok(true)` - Upgrade successful
/// * `Err(String)` - Error message if upgrade fails
#[tauri::command]
pub async fn upgrade_amplify_backend_packages(
    project_path: String,
    package_manager: PackageManager,
    window: tauri::Window,
) -> Result<bool, String> {
    let (cmd, args) = match package_manager {
        PackageManager::Npm => (
            "npm",
            vec![
                "install",
                "--save-dev",
                "@aws-amplify/backend@latest",
                "@aws-amplify/backend-cli@latest",
            ],
        ),
        PackageManager::Yarn => (
            "yarn",
            vec![
                "add",
                "--dev",
                "@aws-amplify/backend@latest",
                "@aws-amplify/backend-cli@latest",
            ],
        ),
        PackageManager::Pnpm => (
            "pnpm",
            vec![
                "add",
                "-D",
                "@aws-amplify/backend@latest",
                "@aws-amplify/backend-cli@latest",
            ],
        ),
        PackageManager::Bun => (
            "bun",
            vec![
                "add",
                "--dev",
                "@aws-amplify/backend@latest",
                "@aws-amplify/backend-cli@latest",
            ],
        ),
    };

    use tauri::Emitter;
    let _ = window.emit("prepare-status", "running");
    let _ = window.emit(
        "prepare-output",
        format!("=== Upgrading Amplify Backend Packages ===\n"),
    );
    let _ = window.emit(
        "prepare-output",
        format!("Command: {} {}\n", cmd, args.join(" ")),
    );
    let _ = window.emit(
        "prepare-output",
        format!("Working directory: {}\n\n", project_path),
    );

    let mut upgrade_cmd = Command::new(cmd).clean_env();
    upgrade_cmd.args(&args).current_dir(&project_path);

    let upgrade_success =
        crate::command::run_command_streaming(upgrade_cmd, &window, "prepare-output")?;

    if !upgrade_success {
        let _ = window.emit(
            "prepare-output",
            "\n✗ Failed to upgrade Amplify backend packages!\n",
        );
        let _ = window.emit("prepare-status", "failed");
        return Err("Failed to upgrade Amplify backend packages".to_string());
    }

    let _ = window.emit(
        "prepare-output",
        "\n✔ Package upgrade completed successfully!\n",
    );

    // Step 2: Run a full install to update all dependencies and the lock file
    let (install_cmd, install_args) = match package_manager {
        PackageManager::Npm => ("npm", vec!["install"]),
        PackageManager::Yarn => ("yarn", vec!["install"]),
        PackageManager::Pnpm => ("pnpm", vec!["install"]),
        PackageManager::Bun => ("bun", vec!["install"]),
    };

    let _ = window.emit(
        "prepare-output",
        format!("\n=== Running Full Dependency Install ===\n"),
    );
    let _ = window.emit(
        "prepare-output",
        format!("Command: {} {}\n\n", install_cmd, install_args.join(" ")),
    );

    let mut install_cmd_obj = Command::new(install_cmd).clean_env();
    install_cmd_obj
        .args(&install_args)
        .current_dir(&project_path);

    let install_success =
        crate::command::run_command_streaming(install_cmd_obj, &window, "prepare-output")?;

    if install_success {
        let _ = window.emit(
            "prepare-output",
            "\n✔ Full dependency install completed successfully!\n",
        );
        let _ = window.emit("prepare-status", "completed");
        Ok(true)
    } else {
        let _ = window.emit("prepare-output", "\n✗ Full dependency install failed!\n");
        let _ = window.emit("prepare-status", "failed");
        Err("Failed to update dependencies after package upgrade".to_string())
    }
}
