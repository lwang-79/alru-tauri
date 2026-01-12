// File operations module - handles project analysis and file modifications

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

/// Backend type (Gen1 or Gen2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendType {
    Gen1,
    Gen2,
}

/// Package manager type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
    Bun,
}

/// A file change made during update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: String,
    pub old_value: String,
    pub new_value: String,
}

/// Result of updating backend files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub changes: Vec<FileChange>,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of running a build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Build configuration location (File or Cloud)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BuildConfigLocation {
    File(String), // Path to amplify.yml
    Cloud,        // AWS cloud configuration
}

/// Build configuration change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfigChange {
    pub location: BuildConfigLocation,
    pub old_command: String,
    pub new_command: String,
}

/// Result of build config update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfigUpdateResult {
    pub success: bool,
    pub updated: bool,
    pub change: Option<BuildConfigChange>,
    pub message: String,
    pub error: Option<String>,
    pub original_build_spec: Option<String>,
}

/// Detects the package manager used in a project by checking for lock files.
/// Priority order: bun.lockb > pnpm-lock.yaml > yarn.lock > package-lock.json
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(PackageManager)` - The detected package manager
/// * `Err(String)` - Error message if no lock file is found
#[tauri::command]
pub async fn detect_package_manager(project_path: &str) -> Result<PackageManager, String> {
    detect_package_manager_sync(project_path)
}

/// Synchronous version of package manager detection for use in other functions
pub fn detect_package_manager_sync(project_path: &str) -> Result<PackageManager, String> {
    let path = Path::new(project_path);

    // Check for lock files in priority order (most specific first)
    if path.join("bun.lockb").exists() {
        return Ok(PackageManager::Bun);
    }

    if path.join("pnpm-lock.yaml").exists() {
        return Ok(PackageManager::Pnpm);
    }

    if path.join("yarn.lock").exists() {
        return Ok(PackageManager::Yarn);
    }

    if path.join("package-lock.json").exists() {
        return Ok(PackageManager::Npm);
    }

    Err("No package manager lock file found. Expected one of: package-lock.json, yarn.lock, pnpm-lock.yaml, or bun.lockb".to_string())
}

/// Detects the backend type (Gen1 or Gen2) by checking package.json for @aws-amplify/backend
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(BackendType)` - Gen2 if @aws-amplify/backend is in devDependencies, Gen1 otherwise
/// * `Err(String)` - Error message if package.json cannot be read or parsed
#[tauri::command]
pub async fn detect_backend_type(project_path: &str) -> Result<BackendType, String> {
    detect_backend_type_sync(project_path)
}

/// Synchronous version of backend type detection for use in other functions
pub fn detect_backend_type_sync(project_path: &str) -> Result<BackendType, String> {
    let package_json_path = Path::new(project_path).join("package.json");

    let content = std::fs::read_to_string(&package_json_path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;

    detect_backend_type_from_content(&content)
}

/// Detects backend type from package.json content string
/// This is separated for easier testing
pub fn detect_backend_type_from_content(content: &str) -> Result<BackendType, String> {
    let package_json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    // Check devDependencies for @aws-amplify/backend
    if let Some(dev_deps) = package_json.get("devDependencies") {
        if dev_deps.get("@aws-amplify/backend").is_some() {
            return Ok(BackendType::Gen2);
        }
    }

    // Also check regular dependencies as a fallback
    if let Some(deps) = package_json.get("dependencies") {
        if deps.get("@aws-amplify/backend").is_some() {
            return Ok(BackendType::Gen2);
        }
    }

    Ok(BackendType::Gen1)
}

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
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use tauri::Emitter;

    let (cmd, args) = match package_manager {
        PackageManager::Npm => ("npm", vec!["install"]),
        PackageManager::Yarn => ("yarn", vec!["install"]),
        PackageManager::Pnpm => ("pnpm", vec!["install"]),
        PackageManager::Bun => ("bun", vec!["install"]),
    };

    // Channel for output lines from reader threads
    let (tx, rx) = mpsc::channel::<String>();

    // Emit initial status
    let _ = window.emit(
        "prepare-output",
        format!("=== Installing Dependencies ===\n"),
    );
    let _ = window.emit("prepare-output", format!("Package Manager: {}\n", cmd));
    let _ = window.emit(
        "prepare-output",
        format!("Working directory: {}\n\n", project_path),
    );
    let _ = window.emit("prepare-status", "running");

    // Spawn the install process
    let mut child = Command::new(cmd)
        .args(&args)
        .current_dir(&project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start {} {}: {}", cmd, args.join(" "), e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let tx_stdout = tx.clone();
    let tx_stderr = tx;

    // Thread to read stdout
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stdout.send(line);
        }
    });

    // Thread to read stderr
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stderr.send(line);
        }
    });

    let mut all_output = String::new();

    // Read output and emit to frontend
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(line) => {
                let _ = window.emit("prepare-output", format!("{}\n", line));
                all_output.push_str(&line);
                all_output.push('\n');
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if process has finished
                if let Ok(Some(status)) = child.try_wait() {
                    // Process finished, drain remaining output
                    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        let _ = window.emit("prepare-output", format!("{}\n", line));
                        all_output.push_str(&line);
                        all_output.push('\n');
                    }

                    if status.success() {
                        let _ = window.emit(
                            "prepare-output",
                            "\n✔ Dependencies installed successfully!\n",
                        );
                        let _ = window.emit("prepare-status", "completed");
                        break;
                    } else {
                        let _ =
                            window.emit("prepare-output", "\n✗ Dependency installation failed!\n");
                        let _ = window.emit("prepare-status", "failed");
                        break;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Wait for reader threads to finish
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    // Get final exit status
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    if status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Dependency installation failed with exit code: {}",
            status.code().unwrap_or(-1)
        ))
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
    backend_type: String,
    profile_name: String,
    window: tauri::Window,
) -> Result<bool, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use tauri::Emitter;

    // First check if amplify CLI is available
    let amplify_check = Command::new("amplify").arg("--version").output();
    if amplify_check.is_err() {
        return Err(
            "Amplify CLI is not installed. Please install it with: npm install -g @aws-amplify/cli"
                .to_string(),
        );
    }

    // Channel for output lines from reader threads
    let (tx, rx) = mpsc::channel::<String>();

    // Emit initial status
    let _ = window.emit(
        "prepare-output",
        format!("\n=== Pulling Amplify Project ===\n"),
    );
    let _ = window.emit("prepare-output", format!("App ID: {}\n", app_id));
    let _ = window.emit("prepare-output", format!("Environment: {}\n", env_name));
    let _ = window.emit(
        "prepare-output",
        format!("Backend Type: {}\n\n", backend_type),
    );

    // Create the command based on backend type
    let mut child = if backend_type == "Gen1" {
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

        Command::new("amplify")
            .args([
                "pull",
                "--amplify",
                &amplify_config,
                "--providers",
                &providers_config,
                "--yes",
            ])
            .env("AWSCLOUDFORMATIONCONFIG", &aws_cloudformation_config)
            .env("AMPLIFY", &amplify_config)
            .env("PROVIDERS", &providers_config)
            .current_dir(&project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start amplify pull (headless): {}", e))?
    } else {
        // For Gen2, use standard command
        let _ = window.emit("prepare-output", "Using standard mode for Gen2 app...\n");

        Command::new("amplify")
            .args(["pull", "--appId", &app_id, "--envName", &env_name, "--yes"])
            .current_dir(&project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start amplify pull: {}", e))?
    };

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let tx_stdout = tx.clone();
    let tx_stderr = tx;

    // Thread to read stdout
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stdout.send(line);
        }
    });

    // Thread to read stderr
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stderr.send(line);
        }
    });

    let mut all_output = String::new();

    // Read output and emit to frontend
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(line) => {
                let _ = window.emit("prepare-output", format!("{}\n", line));
                all_output.push_str(&line);
                all_output.push('\n');
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if process has finished
                if let Ok(Some(status)) = child.try_wait() {
                    // Process finished, drain remaining output
                    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        let _ = window.emit("prepare-output", format!("{}\n", line));
                        all_output.push_str(&line);
                        all_output.push('\n');
                    }

                    if status.success() {
                        let _ = window.emit(
                            "prepare-output",
                            "\n✔ Amplify pull completed successfully!\n",
                        );
                        break;
                    } else {
                        let _ = window.emit("prepare-output", "\n✗ Amplify pull failed!\n");
                        break;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Wait for reader threads to finish
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    // Get final exit status
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    if status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Amplify pull failed with exit code: {}",
            status.code().unwrap_or(-1)
        ))
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
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use tauri::Emitter;

    // First check if amplify CLI is available
    let amplify_check = Command::new("amplify").arg("--version").output();
    if amplify_check.is_err() {
        return Err(
            "Amplify CLI is not installed. Please install it with: npm install -g @aws-amplify/cli"
                .to_string(),
        );
    }

    // Channel for output lines from reader threads
    let (tx, rx) = mpsc::channel::<String>();

    // Emit initial status
    let _ = window.emit(
        "prepare-output",
        format!("\n=== Checking out Amplify Environment ===\n"),
    );
    let _ = window.emit("prepare-output", format!("Environment: {}\n\n", env_name));

    // Spawn the amplify env checkout process
    let mut child = Command::new("amplify")
        .args(["env", "checkout", &env_name, "--yes"])
        .current_dir(&project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start amplify env checkout: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let tx_stdout = tx.clone();
    let tx_stderr = tx;

    // Thread to read stdout
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stdout.send(line);
        }
    });

    // Thread to read stderr
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stderr.send(line);
        }
    });

    let mut all_output = String::new();

    // Read output and emit to frontend
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(line) => {
                let _ = window.emit("prepare-output", format!("{}\n", line));
                all_output.push_str(&line);
                all_output.push('\n');
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if process has finished
                if let Ok(Some(status)) = child.try_wait() {
                    // Process finished, drain remaining output
                    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        let _ = window.emit("prepare-output", format!("{}\n", line));
                        all_output.push_str(&line);
                        all_output.push('\n');
                    }

                    if status.success() {
                        let _ = window.emit(
                            "prepare-output",
                            "\n✔ Environment checkout completed successfully!\n",
                        );
                        break;
                    } else {
                        let _ = window.emit("prepare-output", "\n✗ Environment checkout failed!\n");
                        break;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Wait for reader threads to finish
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    // Get final exit status
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    if status.success() {
        Ok(true)
    } else {
        // Check if the error is because the env doesn't exist
        if all_output.contains("doesn't exist") || all_output.contains("does not exist") {
            return Err(format!(
                "Amplify environment '{}' does not exist. Available environments may differ from branch names.",
                env_name
            ));
        }
        Err(format!(
            "Amplify env checkout failed with exit code: {}",
            status.code().unwrap_or(-1)
        ))
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
    backend_type: &str,
    profile_name: &str,
) -> Result<bool, String> {
    // First check if amplify CLI is available
    let amplify_check = Command::new("amplify").arg("--version").output();

    if amplify_check.is_err() {
        return Err(
            "Amplify CLI is not installed. Please install it with: npm install -g @aws-amplify/cli"
                .to_string(),
        );
    }

    // Run amplify pull command based on backend type
    let output = if backend_type == "Gen1" {
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

        Command::new("amplify")
            .args([
                "pull",
                "--amplify",
                &amplify_config,
                "--providers",
                &providers_config,
                "--yes",
            ])
            .env("AWSCLOUDFORMATIONCONFIG", &aws_cloudformation_config)
            .env("AMPLIFY", &amplify_config)
            .env("PROVIDERS", &providers_config)
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute amplify pull (headless): {}", e))?
    } else {
        // For Gen2, use standard command
        Command::new("amplify")
            .args(["pull", "--appId", app_id, "--envName", env_name, "--yes"])
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute amplify pull: {}", e))?
    };

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
    // First check if amplify CLI is available
    let amplify_check = Command::new("amplify").arg("--version").output();

    if amplify_check.is_err() {
        return Err(
            "Amplify CLI is not installed. Please install it with: npm install -g @aws-amplify/cli"
                .to_string(),
        );
    }

    // Run amplify env checkout
    let output = Command::new("amplify")
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

/// Represents a package entry in the _LIVE_UPDATES environment variable
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveUpdateEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub pkg: String,
    #[serde(rename = "type")]
    pub pkg_type: String,
    pub version: String,
}

/// Checks if the _LIVE_UPDATES environment variable needs updating for @aws-amplify/cli.
/// Returns None if no update is needed (missing entry, already latest, or already at latest version).
/// Returns Some(latest_version) if an update is needed.
///
/// # Arguments
/// * `live_updates_json` - The JSON string from _LIVE_UPDATES environment variable (can be None if not present)
///
/// # Returns
/// * `Ok(Option<String>)` - None if no update needed, Some(version) if update needed
/// * `Err(String)` - Error if JSON parsing or version fetching fails
pub fn check_if_live_updates_needs_update(
    live_updates_json: Option<&str>,
) -> Result<Option<String>, String> {
    // If _LIVE_UPDATES doesn't exist, no update needed (default is latest)
    let json = match live_updates_json {
        Some(j) => j,
        None => return Ok(None),
    };

    let entries: Vec<LiveUpdateEntry> = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse _LIVE_UPDATES JSON: {}", e))?;

    // Find @aws-amplify/cli entry
    let cli_entry = entries.iter().find(|e| e.pkg == "@aws-amplify/cli");

    // If not found, no update needed (default is latest)
    let current_version = match cli_entry {
        Some(entry) => &entry.version,
        None => return Ok(None),
    };

    // If already "latest", no update needed
    if current_version == "latest" {
        return Ok(None);
    }

    // Fetch the latest version from npm
    let latest_version = fetch_latest_amplify_cli_version()?;

    // If current version equals latest version, no update needed
    if current_version == &latest_version {
        return Ok(None);
    }

    // Update needed - return the latest version
    Ok(Some(latest_version))
}

/// Fetches the latest version of @aws-amplify/cli from npm registry
///
/// # Returns
/// * `Ok(String)` - The latest version number (e.g., "14.2.1")
/// * `Err(String)` - Error if fetching fails
pub fn fetch_latest_amplify_cli_version() -> Result<String, String> {
    let output = Command::new("npm")
        .args(["view", "@aws-amplify/cli", "version"])
        .output()
        .map_err(|e| format!("Failed to execute npm command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to fetch latest version: {}", stderr));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if version.is_empty() {
        return Err("Failed to parse version from npm output".to_string());
    }

    Ok(version)
}

/// Updates the _LIVE_UPDATES environment variable to set @aws-amplify/cli version to a specific version.
///
/// # Arguments
/// * `live_updates_json` - The JSON string from _LIVE_UPDATES environment variable
/// * `target_version` - The version to set (e.g., "14.2.1")
///
/// # Returns
/// * `Ok(String)` - Updated JSON string with @aws-amplify/cli version set to target_version
/// * `Err(String)` - Error if JSON parsing fails
pub fn update_live_updates_to_version(
    live_updates_json: &str,
    target_version: &str,
) -> Result<String, String> {
    let mut entries: Vec<LiveUpdateEntry> = serde_json::from_str(live_updates_json)
        .map_err(|e| format!("Failed to parse _LIVE_UPDATES JSON: {}", e))?;

    let mut found = false;
    for entry in entries.iter_mut() {
        if entry.pkg == "@aws-amplify/cli" {
            entry.version = target_version.to_string();
            found = true;
            break;
        }
    }

    // If @aws-amplify/cli entry doesn't exist, add it
    if !found {
        entries.push(LiveUpdateEntry {
            name: Some("Amplify CLI".to_string()),
            pkg: "@aws-amplify/cli".to_string(),
            pkg_type: "npm".to_string(),
            version: target_version.to_string(),
        });
    }

    serde_json::to_string(&entries)
        .map_err(|e| format!("Failed to serialize _LIVE_UPDATES JSON: {}", e))
}

/// Checks if the _LIVE_UPDATES environment variable contains @aws-amplify/cli with version "latest".
///
/// # Arguments
/// * `live_updates_json` - The JSON string from _LIVE_UPDATES environment variable
///
/// # Returns
/// * `Ok(bool)` - true if @aws-amplify/cli version is "latest", false otherwise
/// * `Err(String)` - Error if JSON parsing fails
pub fn check_live_updates_version(live_updates_json: &str) -> Result<bool, String> {
    let entries: Vec<LiveUpdateEntry> = serde_json::from_str(live_updates_json)
        .map_err(|e| format!("Failed to parse _LIVE_UPDATES JSON: {}", e))?;

    for entry in entries {
        if entry.pkg == "@aws-amplify/cli" {
            return Ok(entry.version == "latest");
        }
    }

    // If @aws-amplify/cli is not found, consider it as not having "latest"
    Ok(false)
}

/// Updates the _LIVE_UPDATES environment variable to set @aws-amplify/cli version to "latest".
///
/// # Arguments
/// * `live_updates_json` - The JSON string from _LIVE_UPDATES environment variable
///
/// # Returns
/// * `Ok(String)` - Updated JSON string with @aws-amplify/cli version set to "latest"
/// * `Err(String)` - Error if JSON parsing fails
pub fn update_live_updates_version(live_updates_json: &str) -> Result<String, String> {
    let mut entries: Vec<LiveUpdateEntry> = serde_json::from_str(live_updates_json)
        .map_err(|e| format!("Failed to parse _LIVE_UPDATES JSON: {}", e))?;

    let mut found = false;
    for entry in entries.iter_mut() {
        if entry.pkg == "@aws-amplify/cli" {
            entry.version = "latest".to_string();
            found = true;
            break;
        }
    }

    // If @aws-amplify/cli entry doesn't exist, add it
    if !found {
        entries.push(LiveUpdateEntry {
            name: Some("Amplify CLI".to_string()),
            pkg: "@aws-amplify/cli".to_string(),
            pkg_type: "npm".to_string(),
            version: "latest".to_string(),
        });
    }

    serde_json::to_string(&entries)
        .map_err(|e| format!("Failed to serialize _LIVE_UPDATES JSON: {}", e))
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

/// Result of upgrading Amplify CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub success: bool,
    pub skipped: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub message: String,
}

/// Gets the latest version of @aws-amplify/cli from npm registry
fn get_latest_amplify_cli_version() -> Result<String, String> {
    let output = Command::new("npm")
        .args(["view", "@aws-amplify/cli", "version"])
        .output()
        .map_err(|e| format!("Failed to get latest Amplify CLI version: {}", e))?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(version)
    } else {
        Err("Failed to get latest Amplify CLI version from npm".to_string())
    }
}

/// Gets the currently installed version of Amplify CLI
fn get_installed_amplify_cli_version() -> Option<String> {
    let output = Command::new("amplify").arg("-v").output().ok()?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !version.is_empty() {
            Some(version)
        } else {
            None
        }
    } else {
        None
    }
}

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

    // Get latest version from npm
    let latest_version = get_latest_amplify_cli_version()?;

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
    let output = Command::new("npm")
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
    project_path: &str,
    package_manager: PackageManager,
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

    // Log the command being executed
    println!(
        "[upgrade_amplify_backend_packages] Executing: {} {}",
        cmd,
        args.join(" ")
    );
    println!(
        "[upgrade_amplify_backend_packages] Working directory: {}",
        project_path
    );

    let output = Command::new(cmd)
        .args(&args)
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", cmd, e))?;

    if output.status.success() {
        println!("[upgrade_amplify_backend_packages] Package upgrade completed successfully");

        // Step 2: Run a full install to update all dependencies and the lock file
        // This ensures that any packages depending on the updated Amplify packages
        // are also updated in the lock file
        let (install_cmd, install_args) = match package_manager {
            PackageManager::Npm => ("npm", vec!["install"]),
            PackageManager::Yarn => ("yarn", vec!["install"]),
            PackageManager::Pnpm => ("pnpm", vec!["install"]),
            PackageManager::Bun => ("bun", vec!["install"]),
        };

        println!(
            "[upgrade_amplify_backend_packages] Executing full install: {} {}",
            install_cmd,
            install_args.join(" ")
        );

        let install_output = Command::new(install_cmd)
            .args(&install_args)
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute {} install: {}", install_cmd, e))?;

        if install_output.status.success() {
            println!("[upgrade_amplify_backend_packages] Full install completed successfully");
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&install_output.stderr);
            Err(format!(
                "Failed to update dependencies after package upgrade: {}",
                stderr
            ))
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "[upgrade_amplify_backend_packages] Package upgrade failed: {}",
            stderr
        );
        Err(format!(
            "Failed to upgrade Amplify backend packages: {}",
            stderr
        ))
    }
}

/// Runs the build command for the project using the appropriate package manager.
/// For Gen1 backends, runs `amplify build` for backend and `npm run build` for frontend.
/// For Gen2 backends, only runs frontend build.
/// Streams output via Tauri events for live display.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `package_manager` - The package manager to use
/// * `backend_type` - The backend type (Gen1 or Gen2)
/// * `window` - Tauri window for emitting events
///
/// # Returns
/// * `BuildResult` - Contains success status, output, and any error message
///
/// # Events Emitted
/// * `build-output` - Emitted with each line of output
/// * `build-status` - Emitted with status updates ("running", "completed", "failed")
#[tauri::command]
pub async fn run_build(
    project_path: String,
    package_manager: PackageManager,
    backend_type: BackendType,
    window: tauri::Window,
) -> Result<BuildResult, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use tauri::Emitter;

    let mut all_output = String::new();

    let _ = window.emit("build-status", "running");

    // Helper function to run a command with streaming output
    let run_command_streaming = |cmd: &str,
                                 args: &[&str],
                                 working_dir: &str,
                                 window: &tauri::Window,
                                 output: &mut String|
     -> Result<bool, String> {
        let _ = window.emit("build-output", format!("$ {} {}\n", cmd, args.join(" ")));

        let (tx, rx) = mpsc::channel::<String>();

        let mut child = Command::new(cmd)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to execute {}: {}", cmd, e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

        let tx_stdout = tx.clone();
        let tx_stderr = tx;

        // Thread to read stdout
        let stdout_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stdout.send(line);
            }
        });

        // Thread to read stderr
        let stderr_handle = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stderr.send(line);
            }
        });

        // Read output and emit events
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    let _ = window.emit("build-output", format!("{}\n", line));
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Check if process has exited
                    if let Ok(Some(_)) = child.try_wait() {
                        // Drain remaining messages
                        while let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
                            let _ = window.emit("build-output", format!("{}\n", line));
                            output.push_str(&line);
                            output.push('\n');
                        }
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        // Wait for threads to finish
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        // Get exit status
        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for process: {}", e))?;
        Ok(status.success())
    };

    // Step 1: Run backend build for Gen1 only
    if backend_type == BackendType::Gen1 {
        let _ = window.emit("build-output", "\n=== Running Amplify Build (Gen1) ===\n");
        all_output.push_str("\n=== Running Amplify Build (Gen1) ===\n");

        let success = run_command_streaming(
            "amplify",
            &["build"],
            &project_path,
            &window,
            &mut all_output,
        )?;

        if !success {
            let _ = window.emit("build-output", "\n❌ Backend build failed\n");
            let _ = window.emit("build-status", "failed");
            return Ok(BuildResult {
                success: false,
                output: all_output,
                error: Some("Backend build failed".to_string()),
            });
        }
        let _ = window.emit("build-output", "\n✔ Backend build completed\n");
    }

    // Step 2: Run frontend build
    let _ = window.emit("build-output", "\n=== Running Frontend Build ===\n");
    all_output.push_str("\n=== Running Frontend Build ===\n");

    let (cmd, args): (&str, Vec<&str>) = match package_manager {
        PackageManager::Npm => ("npm", vec!["run", "build"]),
        PackageManager::Yarn => ("yarn", vec!["build"]),
        PackageManager::Pnpm => ("pnpm", vec!["build"]),
        PackageManager::Bun => ("bun", vec!["run", "build"]),
    };

    let success = run_command_streaming(cmd, &args, &project_path, &window, &mut all_output)?;

    if success {
        let _ = window.emit("build-output", "\n✔ Frontend build completed\n");
        let _ = window.emit("build-status", "completed");
        Ok(BuildResult {
            success: true,
            output: all_output,
            error: None,
        })
    } else {
        let _ = window.emit("build-output", "\n❌ Frontend build failed\n");
        let _ = window.emit("build-status", "failed");
        Ok(BuildResult {
            success: false,
            output: all_output,
            error: Some("Frontend build failed".to_string()),
        })
    }
}

/// Result of sandbox deployment with streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub status: String, // "running", "completed", "timeout", "failed"
}

/// Deploys a Gen2 sandbox for testing.
/// This runs `npx ampx sandbox --profile <profile>` with AWS_REGION override and monitors for completion.
/// The command is a long-running process that watches for file changes.
/// Completion is detected when output contains "Watching for file changes" and "File written:"
/// with no new output for 10 seconds.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `profile` - AWS profile to use
/// * `region` - AWS region to override profile default
/// * `window` - Tauri window for emitting events
///
/// # Returns
/// * `SandboxResult` - Contains success status, output, and any error message
///
/// # Events Emitted
/// * `sandbox-output` - Emitted with each line of output
/// * `sandbox-status` - Emitted with status updates ("running", "completed", "timeout", "failed")
#[tauri::command]
pub async fn deploy_gen2_sandbox(
    project_path: String,
    profile: String,
    region: String,
    window: tauri::Window,
) -> Result<SandboxResult, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::Emitter;

    // Channel for output lines from reader threads
    let (tx, rx) = mpsc::channel::<String>();

    // Emit initial status
    let _ = window.emit("sandbox-output", "=== Deploying Gen2 Sandbox ===\n");
    let _ = window.emit("sandbox-output", format!("Profile: {}\n", profile));
    let _ = window.emit("sandbox-output", format!("Region: {}\n", region));
    let _ = window.emit(
        "sandbox-output",
        format!("Working directory: {}\n\n", project_path),
    );
    let _ = window.emit("sandbox-status", "running");

    // Spawn the sandbox process with AWS_REGION override
    // Clear environment variables that might cause npx to detect wrong package manager
    let mut child = Command::new("npx")
        .args(["ampx", "sandbox", "--profile", &profile])
        .current_dir(&project_path)
        .env("AWS_REGION", &region)
        .env_remove("npm_config_user_agent")
        .env_remove("npm_execpath")
        .env_remove("BUN_INSTALL")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start npx ampx sandbox: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let tx_stdout = tx.clone();
    let tx_stderr = tx;

    // Thread to read stdout
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stdout.send(line);
        }
    });

    // Thread to read stderr
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stderr.send(line);
        }
    });

    // State tracking
    let mut all_output = String::new();
    let mut last_lines: Vec<String> = Vec::new();
    let mut last_output_time = Instant::now();
    let mut deployment_complete = false;
    let mut deployment_failed = false;
    let mut error_message: Option<String> = None;

    let completion_timeout = Duration::from_secs(10); // 10 seconds of no output after completion pattern
                                                      // Removed max timeout - let sandbox deployment run until completion
                                                      // let max_timeout = Duration::from_secs(600); // 10 minutes max
                                                      // let start_time = Instant::now();

    loop {
        // Try to receive output with a short timeout
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                // Emit the line to the frontend
                let _ = window.emit("sandbox-output", format!("{}\n", line));

                // Store output
                all_output.push_str(&line);
                all_output.push('\n');

                // Track last lines for pattern matching
                last_lines.push(line.clone());
                if last_lines.len() > 5 {
                    last_lines.remove(0);
                }

                last_output_time = Instant::now();

                // Check for error patterns
                if line.contains("Error:") || line.contains("error:") || line.contains("FAILED") {
                    deployment_failed = true;
                    error_message = Some(line);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No output, check conditions
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Channels closed, threads finished
                break;
            }
        }

        // Check if process has exited unexpectedly
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() && !deployment_complete {
                let _ = window.emit("sandbox-status", "failed");
                deployment_failed = true;
            }
            // Process exited, wait a bit for remaining output then break
            thread::sleep(Duration::from_millis(500));
            // Drain remaining messages
            while let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
                let _ = window.emit("sandbox-output", format!("{}\n", line));
                all_output.push_str(&line);
                all_output.push('\n');
                last_lines.push(line.clone());
                if last_lines.len() > 5 {
                    last_lines.remove(0);
                }
            }
            break;
        }

        // Check for deployment failure
        if deployment_failed {
            let _ = window.emit("sandbox-status", "failed");
            let _ = child.kill();
            break;
        }

        // Check for completion pattern
        let has_watching = last_lines
            .iter()
            .any(|l| l.contains("Watching for file changes"));
        let has_file_written = last_lines.iter().any(|l| l.contains("File written:"));

        if has_watching && has_file_written {
            let elapsed_since_output = last_output_time.elapsed();
            if elapsed_since_output >= completion_timeout {
                // Deployment completed successfully
                deployment_complete = true;
                let _ = window.emit(
                    "sandbox-output",
                    "\n✔ Sandbox deployment completed successfully!\n",
                );
                let _ = window.emit("sandbox-status", "completed");
                let _ = child.kill();
                break;
            }
        }

        // Removed max timeout check - let deployment run until completion
        // if start_time.elapsed() >= max_timeout {
        //     let _ = window.emit(
        //         "sandbox-output",
        //         "\n⚠ Deployment is taking longer than expected (10 minutes).\n",
        //     );
        //     let _ = window.emit(
        //         "sandbox-output",
        //         "Please monitor the AWS CloudFormation console for deployment status.\n",
        //     );
        //     let _ = window.emit("sandbox-status", "timeout");
        //     let _ = child.kill();
        //     break;
        // }
    }

    // Wait for reader threads to finish
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if deployment_complete {
        Ok(SandboxResult {
            success: true,
            output: all_output,
            error: None,
            status: "completed".to_string(),
        })
    } else if deployment_failed {
        Ok(SandboxResult {
            success: false,
            output: all_output,
            error: error_message,
            status: "failed".to_string(),
        })
    } else {
        // This case should now only occur if deployment actually failed, not timed out
        Ok(SandboxResult {
            success: false,
            output: all_output,
            error: Some(
                "Deployment failed or was interrupted. Please check AWS CloudFormation console."
                    .to_string(),
            ),
            status: "failed".to_string(),
        })
    }
}

/// Deletes a Gen2 sandbox.
/// This runs `npx ampx sandbox delete --profile <profile> -y` with AWS_REGION override.
///
/// # Arguments
/// * `project_path` - Path to the project directory
/// * `profile` - AWS profile to use
/// * `region` - AWS region to override profile default
///
/// # Returns
/// * `BuildResult` - Contains success status, output, and any error message
#[tauri::command]
pub async fn delete_gen2_sandbox(
    project_path: &str,
    profile: &str,
    region: &str,
) -> Result<BuildResult, String> {
    let output = Command::new("npx")
        .args(["ampx", "sandbox", "delete", "--profile", profile, "-y"])
        .current_dir(project_path)
        .env("AWS_REGION", region)
        .env_remove("npm_config_user_agent")
        .env_remove("npm_execpath")
        .env_remove("BUN_INSTALL")
        .output()
        .map_err(|e| format!("Failed to execute npx ampx sandbox delete: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(BuildResult {
            success: true,
            output: stdout,
            error: None,
        })
    } else {
        Ok(BuildResult {
            success: false,
            output: stdout,
            error: Some(stderr),
        })
    }
}

// ==================== Gen2 Build Configuration Update Functions ====================

/// Checks if amplify.yml exists in the repository root.
///
/// # Arguments
/// * `repo_path` - Path to the repository directory
///
/// # Returns
/// * `bool` - true if amplify.yml exists, false otherwise
pub fn check_amplify_yml_exists(repo_path: &str) -> bool {
    let amplify_yml_path = Path::new(repo_path).join("amplify.yml");
    amplify_yml_path.exists()
}

/// Reads the amplify.yml file content.
///
/// # Arguments
/// * `repo_path` - Path to the repository directory
///
/// # Returns
/// * `Ok(String)` - File content as string
/// * `Err(String)` - Error message if file cannot be read
pub fn read_amplify_yml(repo_path: &str) -> Result<String, String> {
    let amplify_yml_path = Path::new(repo_path).join("amplify.yml");

    std::fs::read_to_string(&amplify_yml_path)
        .map_err(|e| format!("Failed to read amplify.yml: {}", e))
}

/// Parses build commands from amplify.yml YAML content.
/// Navigates to backend.phases.build.commands and filters out commented lines.
///
/// # Arguments
/// * `yaml_content` - The YAML content as a string
///
/// # Returns
/// * `Ok(Vec<String>)` - List of active (non-commented) build commands
/// * `Err(String)` - Error message if parsing fails
pub fn parse_build_commands(yaml_content: &str) -> Result<Vec<String>, String> {
    // Parse YAML content
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    // Navigate to backend.phases.build.commands
    let commands = yaml
        .get("backend")
        .and_then(|b| b.get("phases"))
        .and_then(|p| p.get("build"))
        .and_then(|b| b.get("commands"))
        .and_then(|c| c.as_sequence())
        .ok_or_else(|| "Could not find backend.phases.build.commands in YAML".to_string())?;

    // Extract commands and filter out commented lines
    let mut result = Vec::new();
    for command in commands {
        if let Some(cmd_str) = command.as_str() {
            let trimmed = cmd_str.trim();
            // Filter out lines starting with #
            if !trimmed.starts_with('#') && !trimmed.is_empty() {
                result.push(cmd_str.to_string());
            }
        }
    }

    Ok(result)
}

/// Checks if a build command needs updating from "generate outputs" to "pipeline-deploy".
///
/// # Arguments
/// * `command` - The build command string
///
/// # Returns
/// * `bool` - true if command needs updating, false otherwise
pub fn needs_build_command_update(command: &str) -> bool {
    // Return false if already using pipeline-deploy
    if command.contains("npx ampx pipeline-deploy") {
        return false;
    }

    // Return true if using the old generate outputs command
    if command.contains("npx ampx generate outputs") {
        return true;
    }

    // Otherwise, no update needed
    false
}

/// Updates a build command from "generate outputs" to "pipeline-deploy".
/// Also replaces "--out-dir" with "--outputs-out-dir".
///
/// # Arguments
/// * `command` - The build command string
///
/// # Returns
/// * `String` - Updated command string
pub fn update_build_command(command: &str) -> String {
    let mut updated = command.to_string();

    // Replace the command
    updated = updated.replace("npx ampx generate outputs", "npx ampx pipeline-deploy");

    // Replace the parameter
    updated = updated.replace("--out-dir", "--outputs-out-dir");

    updated
}

/// Updates Gen2 build configuration from "generate outputs" to "pipeline-deploy".
/// Checks if amplify.yml exists in the repository root. If it exists, updates the file.
/// If not, retrieves and updates the cloud buildSpec via AWS API.
///
/// # Arguments
/// * `repo_path` - Path to the repository directory
/// * `profile` - AWS profile to use
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
///
/// # Returns
/// * `Ok(BuildConfigUpdateResult)` - Result with update status and changes
/// * `Err(String)` - Error message if operation fails
#[tauri::command]
pub async fn update_gen2_build_config(
    repo_path: &str,
    profile: &str,
    region: &str,
    app_id: &str,
) -> Result<BuildConfigUpdateResult, String> {
    use crate::aws_cli::{get_app_build_spec, update_app_build_spec};

    // Step 1: Check if amplify.yml exists
    if check_amplify_yml_exists(repo_path) {
        // Step 2a: Process local file
        let yaml_content = read_amplify_yml(repo_path)?;
        let commands = parse_build_commands(&yaml_content)?;

        // Check if any ampx command exists
        let has_pipeline_deploy = commands
            .iter()
            .any(|cmd| cmd.contains("npx ampx pipeline-deploy"));
        let has_generate_outputs = commands
            .iter()
            .any(|cmd| cmd.contains("npx ampx generate outputs"));

        if !has_pipeline_deploy && !has_generate_outputs {
            return Ok(BuildConfigUpdateResult {
                success: false,
                updated: false,
                change: None,
                message: "Configuration error".to_string(),
                error: Some(
                    "Gen2 build configuration error: No ampx deployment command found. \
                     Please check your build configuration manually."
                        .to_string(),
                ),
                original_build_spec: None,
            });
        }

        // Find command that needs updating
        for command in &commands {
            if needs_build_command_update(command) {
                let new_command = update_build_command(command);

                // Update the YAML content
                let updated_yaml = yaml_content.replace(command, &new_command);

                // Write back to file
                let amplify_yml_path = Path::new(repo_path).join("amplify.yml");
                std::fs::write(&amplify_yml_path, &updated_yaml)
                    .map_err(|e| format!("Failed to write amplify.yml: {}", e))?;

                return Ok(BuildConfigUpdateResult {
                    success: true,
                    updated: true,
                    change: Some(BuildConfigChange {
                        location: BuildConfigLocation::File(
                            amplify_yml_path.to_string_lossy().to_string(),
                        ),
                        old_command: command.clone(),
                        new_command,
                    }),
                    message: "Updated amplify.yml build command".to_string(),
                    error: None,
                    original_build_spec: None,
                });
            }
        }

        // No updates needed
        return Ok(BuildConfigUpdateResult {
            success: true,
            updated: false,
            change: None,
            message: "Build command already correct in amplify.yml".to_string(),
            error: None,
            original_build_spec: None,
        });
    } else {
        // Step 2b: Process cloud buildSpec
        let build_spec = get_app_build_spec(profile, region, app_id)?;
        let commands = parse_build_commands(&build_spec)?;

        // Check if any ampx command exists
        let has_pipeline_deploy = commands
            .iter()
            .any(|cmd| cmd.contains("npx ampx pipeline-deploy"));
        let has_generate_outputs = commands
            .iter()
            .any(|cmd| cmd.contains("npx ampx generate outputs"));

        if !has_pipeline_deploy && !has_generate_outputs {
            return Ok(BuildConfigUpdateResult {
                success: false,
                updated: false,
                change: None,
                message: "Configuration error".to_string(),
                error: Some(
                    "Gen2 build configuration error: No ampx deployment command found. \
                     Please check your build configuration manually."
                        .to_string(),
                ),
                original_build_spec: None,
            });
        }

        // Find command that needs updating
        for command in &commands {
            if needs_build_command_update(command) {
                let new_command = update_build_command(command);

                // Update the buildSpec
                let updated_spec = build_spec.replace(command, &new_command);

                // Update via AWS API
                update_app_build_spec(profile, region, app_id, &updated_spec)?;

                return Ok(BuildConfigUpdateResult {
                    success: true,
                    updated: true,
                    change: Some(BuildConfigChange {
                        location: BuildConfigLocation::Cloud,
                        old_command: command.clone(),
                        new_command,
                    }),
                    message: "Updated cloud buildSpec".to_string(),
                    error: None,
                    original_build_spec: Some(build_spec),
                });
            }
        }

        // No updates needed
        return Ok(BuildConfigUpdateResult {
            success: true,
            updated: false,
            change: None,
            message: "Build command already correct in cloud buildSpec".to_string(),
            error: None,
            original_build_spec: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_npm_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Npm);
    }

    #[test]
    fn test_detect_yarn_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("yarn.lock"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Yarn);
    }

    #[test]
    fn test_detect_pnpm_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Pnpm);
    }

    #[test]
    fn test_detect_bun_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bun.lockb"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Bun);
    }

    #[test]
    fn test_no_lock_file_returns_error() {
        let temp_dir = TempDir::new().unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_bun_takes_priority_over_npm() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();
        fs::write(temp_dir.path().join("bun.lockb"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Bun);
    }

    #[test]
    fn test_detect_gen2_backend_from_dev_dependencies() {
        let content = r#"{
            "name": "test-project",
            "devDependencies": {
                "@aws-amplify/backend": "^1.0.0",
                "@aws-amplify/backend-cli": "^1.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen2);
    }

    #[test]
    fn test_detect_gen2_backend_from_dependencies() {
        let content = r#"{
            "name": "test-project",
            "dependencies": {
                "@aws-amplify/backend": "^1.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen2);
    }

    #[test]
    fn test_detect_gen1_backend_without_amplify_backend() {
        let content = r#"{
            "name": "test-project",
            "devDependencies": {
                "aws-amplify": "^5.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen1);
    }

    #[test]
    fn test_detect_gen1_backend_empty_package_json() {
        let content = r#"{
            "name": "test-project"
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen1);
    }

    #[test]
    fn test_invalid_json_returns_error() {
        let content = "not valid json";

        let result = detect_backend_type_from_content(content);
        assert!(result.is_err());
    }

    // Property-based tests
    // **Feature: amplify-runtime-updater, Property 5: Package Manager Detection**
    // **Validates: Requirements 6.3**
    proptest! {
        /// Property 5: Package Manager Detection
        /// For any project directory containing exactly one of the lock files
        /// (package-lock.json, yarn.lock, pnpm-lock.yaml, bun.lockb),
        /// the system shall detect the corresponding package manager.
        #[test]
        fn prop_package_manager_detection(lock_file_type in 0u8..4) {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            // Create exactly one lock file based on the random type
            let (lock_file, expected_pm) = match lock_file_type {
                0 => ("package-lock.json", PackageManager::Npm),
                1 => ("yarn.lock", PackageManager::Yarn),
                2 => ("pnpm-lock.yaml", PackageManager::Pnpm),
                _ => ("bun.lockb", PackageManager::Bun),
            };

            fs::write(path.join(lock_file), "").unwrap();

            let result = detect_package_manager_sync(path.to_str().unwrap());

            prop_assert!(result.is_ok(), "Detection should succeed when lock file exists");
            prop_assert_eq!(result.unwrap(), expected_pm,
                "Expected {:?} for lock file {}", expected_pm, lock_file);
        }

        /// Property test: No lock file should return an error
        #[test]
        fn prop_no_lock_file_returns_error(random_files in prop::collection::vec("[a-z]+\\.[a-z]+", 0..5)) {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            // Create random files that are NOT lock files
            for file_name in random_files {
                // Skip if the random name happens to match a lock file
                if file_name == "package-lock.json" || file_name == "yarn.lock"
                    || file_name == "pnpm-lock.yaml" || file_name == "bun.lockb" {
                    continue;
                }
                let _ = fs::write(path.join(&file_name), "");
            }

            let result = detect_package_manager_sync(path.to_str().unwrap());

            // If no lock file was created, detection should fail
            let has_lock_file = path.join("package-lock.json").exists()
                || path.join("yarn.lock").exists()
                || path.join("pnpm-lock.yaml").exists()
                || path.join("bun.lockb").exists();

            if !has_lock_file {
                prop_assert!(result.is_err(), "Detection should fail when no lock file exists");
            }
        }

        /// Property test: Priority order is maintained (bun > pnpm > yarn > npm)
        #[test]
        fn prop_package_manager_priority(
            has_npm in any::<bool>(),
            has_yarn in any::<bool>(),
            has_pnpm in any::<bool>(),
            has_bun in any::<bool>()
        ) {
            // Skip if no lock files would be created
            prop_assume!(has_npm || has_yarn || has_pnpm || has_bun);

            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            if has_npm {
                fs::write(path.join("package-lock.json"), "{}").unwrap();
            }
            if has_yarn {
                fs::write(path.join("yarn.lock"), "").unwrap();
            }
            if has_pnpm {
                fs::write(path.join("pnpm-lock.yaml"), "").unwrap();
            }
            if has_bun {
                fs::write(path.join("bun.lockb"), "").unwrap();
            }

            let result = detect_package_manager_sync(path.to_str().unwrap());
            prop_assert!(result.is_ok());

            let detected = result.unwrap();

            // Verify priority: bun > pnpm > yarn > npm
            let expected = if has_bun {
                PackageManager::Bun
            } else if has_pnpm {
                PackageManager::Pnpm
            } else if has_yarn {
                PackageManager::Yarn
            } else {
                PackageManager::Npm
            };

            prop_assert_eq!(detected, expected,
                "Priority not maintained: has_npm={}, has_yarn={}, has_pnpm={}, has_bun={}, expected {:?}, got {:?}",
                has_npm, has_yarn, has_pnpm, has_bun, expected, detected);
        }
    }

    // **Feature: amplify-runtime-updater, Property 6: Backend Type Detection**
    // **Validates: Requirements 6.5**
    proptest! {
        /// Property 6: Backend Type Detection
        /// For any package.json content, the system shall identify the backend as Gen2
        /// if @aws-amplify/backend exists in devDependencies, and Gen1 otherwise.
        #[test]
        fn prop_backend_type_detection_gen2_in_dev_deps(
            project_name in "[a-z][a-z0-9-]{0,20}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+"
        ) {
            let content = format!(r#"{{
                "name": "{}",
                "devDependencies": {{
                    "@aws-amplify/backend": "^{}"
                }}
            }}"#, project_name, version);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen2,
                "Should detect Gen2 when @aws-amplify/backend is in devDependencies");
        }

        /// Property test: Gen2 detection from regular dependencies
        #[test]
        fn prop_backend_type_detection_gen2_in_deps(
            project_name in "[a-z][a-z0-9-]{0,20}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+"
        ) {
            let content = format!(r#"{{
                "name": "{}",
                "dependencies": {{
                    "@aws-amplify/backend": "^{}"
                }}
            }}"#, project_name, version);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen2,
                "Should detect Gen2 when @aws-amplify/backend is in dependencies");
        }

        /// Property test: Gen1 detection when @aws-amplify/backend is absent
        #[test]
        fn prop_backend_type_detection_gen1_without_amplify_backend(
            project_name in "[a-z][a-z0-9-]{0,20}",
            other_dep in "[a-z][a-z0-9-]{0,20}"
        ) {
            // Ensure the other_dep is not @aws-amplify/backend
            prop_assume!(other_dep != "aws-amplify/backend");

            let content = format!(r#"{{
                "name": "{}",
                "devDependencies": {{
                    "{}": "^1.0.0"
                }}
            }}"#, project_name, other_dep);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen1,
                "Should detect Gen1 when @aws-amplify/backend is not present");
        }

        /// Property test: Gen1 detection with empty dependencies
        #[test]
        fn prop_backend_type_detection_gen1_empty_deps(
            project_name in "[a-z][a-z0-9-]{0,20}"
        ) {
            let content = format!(r#"{{
                "name": "{}"
            }}"#, project_name);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen1,
                "Should detect Gen1 when no dependencies are present");
        }

        /// Property test: devDependencies takes precedence (Gen2 detection)
        #[test]
        fn prop_backend_type_detection_dev_deps_precedence(
            project_name in "[a-z][a-z0-9-]{0,20}",
            has_in_dev_deps in any::<bool>(),
            has_in_deps in any::<bool>()
        ) {
            let dev_deps = if has_in_dev_deps {
                r#""devDependencies": { "@aws-amplify/backend": "^1.0.0" },"#
            } else {
                r#""devDependencies": { "other-package": "^1.0.0" },"#
            };

            let deps = if has_in_deps {
                r#""dependencies": { "@aws-amplify/backend": "^1.0.0" }"#
            } else {
                r#""dependencies": { "other-package": "^1.0.0" }"#
            };

            let content = format!(r#"{{
                "name": "{}",
                {}
                {}
            }}"#, project_name, dev_deps, deps);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");

            let expected = if has_in_dev_deps || has_in_deps {
                BackendType::Gen2
            } else {
                BackendType::Gen1
            };

            prop_assert_eq!(result.unwrap(), expected,
                "Backend type should be Gen2 if @aws-amplify/backend is in either devDependencies or dependencies");
        }
    }

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

    // **Feature: amplify-runtime-updater, Property 7: Runtime Update Transformation**
    // **Validates: Requirements 7.2, 8.2**
    proptest! {
        /// Property 7: Runtime Update Transformation
        /// For any file content (resource.ts or CloudFormation template) containing an outdated
        /// runtime definition, applying the runtime update transformation shall produce content
        /// with the target runtime while preserving all other content unchanged.
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

    // Unit tests for _LIVE_UPDATES check
    #[test]
    fn test_check_live_updates_version_is_latest() {
        let json = r#"[{"pkg":"@aws-amplify/cli","type":"npm","version":"latest"}]"#;
        let result = check_live_updates_version(json).unwrap();
        assert!(result);
    }

    #[test]
    fn test_check_live_updates_version_not_latest() {
        let json = r#"[{"pkg":"@aws-amplify/cli","type":"npm","version":"12.0.0"}]"#;
        let result = check_live_updates_version(json).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_check_live_updates_version_missing_cli() {
        let json = r#"[{"pkg":"other-package","type":"npm","version":"latest"}]"#;
        let result = check_live_updates_version(json).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_check_live_updates_version_empty_array() {
        let json = r#"[]"#;
        let result = check_live_updates_version(json).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_check_live_updates_version_invalid_json() {
        let json = "not valid json";
        let result = check_live_updates_version(json);
        assert!(result.is_err());
    }

    // Unit tests for _LIVE_UPDATES update
    #[test]
    fn test_update_live_updates_version_basic() {
        let json = r#"[{"pkg":"@aws-amplify/cli","type":"npm","version":"12.0.0"}]"#;
        let result = update_live_updates_version(json).unwrap();

        let entries: Vec<LiveUpdateEntry> = serde_json::from_str(&result).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pkg, "@aws-amplify/cli");
        assert_eq!(entries[0].version, "latest");
    }

    #[test]
    fn test_update_live_updates_version_adds_if_missing() {
        let json = r#"[{"pkg":"other-package","type":"npm","version":"1.0.0"}]"#;
        let result = update_live_updates_version(json).unwrap();

        let entries: Vec<LiveUpdateEntry> = serde_json::from_str(&result).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|e| e.pkg == "@aws-amplify/cli" && e.version == "latest"));
    }

    #[test]
    fn test_update_live_updates_version_preserves_other_entries() {
        let json = r#"[{"pkg":"@aws-amplify/cli","type":"npm","version":"12.0.0"},{"pkg":"node","type":"nvm","version":"18"}]"#;
        let result = update_live_updates_version(json).unwrap();

        let entries: Vec<LiveUpdateEntry> = serde_json::from_str(&result).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|e| e.pkg == "@aws-amplify/cli" && e.version == "latest"));
        assert!(entries.iter().any(|e| e.pkg == "node" && e.version == "18"));
    }

    // **Feature: amplify-runtime-updater, Property 8: Live Updates Version Check**
    // **Validates: Requirements 8.3**
    proptest! {
        /// Property 8: Live Updates Version Check
        /// For any _LIVE_UPDATES JSON string, the system shall correctly identify
        /// whether the @aws-amplify/cli version is set to "latest".
        #[test]
        fn prop_live_updates_version_check_latest(
            other_packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{0,15}", "[a-z]+", "[0-9]+\\.[0-9]+\\.[0-9]+"),
                0..3
            )
        ) {
            // Build JSON with @aws-amplify/cli set to "latest"
            let mut entries: Vec<LiveUpdateEntry> = other_packages
                .iter()
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            entries.push(LiveUpdateEntry {
                name: None,
                pkg: "@aws-amplify/cli".to_string(),
                pkg_type: "npm".to_string(),
                version: "latest".to_string(),
            });

            let json = serde_json::to_string(&entries).unwrap();
            let result = check_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should parse valid JSON");
            prop_assert!(result.unwrap(), "Should return true when @aws-amplify/cli version is 'latest'");
        }

        /// Property test: Returns false when @aws-amplify/cli version is not "latest"
        #[test]
        fn prop_live_updates_version_check_not_latest(
            version in "[0-9]+\\.[0-9]+\\.[0-9]+",
            other_packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{0,15}", "[a-z]+", "[0-9]+\\.[0-9]+\\.[0-9]+"),
                0..3
            )
        ) {
            // Ensure version is not "latest"
            prop_assume!(version != "latest");

            let mut entries: Vec<LiveUpdateEntry> = other_packages
                .iter()
                .map(|(pkg, pkg_type, ver)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: ver.clone(),
                })
                .collect();

            entries.push(LiveUpdateEntry {
                name: None,
                pkg: "@aws-amplify/cli".to_string(),
                pkg_type: "npm".to_string(),
                version: version.clone(),
            });

            let json = serde_json::to_string(&entries).unwrap();
            let result = check_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should parse valid JSON");
            prop_assert!(!result.unwrap(), "Should return false when @aws-amplify/cli version is not 'latest'");
        }

        /// Property test: Returns false when @aws-amplify/cli is not present
        #[test]
        fn prop_live_updates_version_check_missing_cli(
            packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{0,15}", "[a-z]+", "[a-z0-9\\.]+"),
                0..5
            )
        ) {
            // Ensure no package is @aws-amplify/cli
            let entries: Vec<LiveUpdateEntry> = packages
                .iter()
                .filter(|(pkg, _, _)| pkg != "@aws-amplify/cli" && !pkg.contains("aws-amplify"))
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            let json = serde_json::to_string(&entries).unwrap();
            let result = check_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should parse valid JSON");
            prop_assert!(!result.unwrap(), "Should return false when @aws-amplify/cli is not present");
        }
    }

    // **Feature: amplify-runtime-updater, Property 9: Live Updates Version Update**
    // **Validates: Requirements 8.4**
    proptest! {
        /// Property 9: Live Updates Version Update
        /// For any _LIVE_UPDATES JSON string where @aws-amplify/cli version is not "latest",
        /// applying the update transformation shall produce a valid JSON string with version set to "latest".
        #[test]
        fn prop_live_updates_version_update_sets_latest(
            old_version in "[0-9]+\\.[0-9]+\\.[0-9]+",
            other_packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{0,15}", "[a-z]+", "[a-z0-9\\.]+"),
                0..3
            )
        ) {
            // Ensure old_version is not "latest"
            prop_assume!(old_version != "latest");

            let mut entries: Vec<LiveUpdateEntry> = other_packages
                .iter()
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            entries.push(LiveUpdateEntry {
                name: None,
                pkg: "@aws-amplify/cli".to_string(),
                pkg_type: "npm".to_string(),
                version: old_version.clone(),
            });

            let json = serde_json::to_string(&entries).unwrap();
            let result = update_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should successfully update JSON");

            let updated_json = result.unwrap();
            let updated_entries: Vec<LiveUpdateEntry> = serde_json::from_str(&updated_json).unwrap();

            // Find @aws-amplify/cli entry
            let cli_entry = updated_entries.iter().find(|e| e.pkg == "@aws-amplify/cli");
            prop_assert!(cli_entry.is_some(), "@aws-amplify/cli should be present");
            prop_assert_eq!(&cli_entry.unwrap().version, "latest",
                "@aws-amplify/cli version should be 'latest'");
        }

        /// Property test: Update preserves other package entries
        #[test]
        fn prop_live_updates_version_update_preserves_others(
            old_version in "[0-9]+\\.[0-9]+\\.[0-9]+",
            other_packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{1,15}", "[a-z]+", "[a-z0-9\\.]+"),
                1..4
            )
        ) {
            prop_assume!(old_version != "latest");

            // Filter out any packages that might conflict with @aws-amplify/cli
            let filtered_packages: Vec<_> = other_packages
                .iter()
                .filter(|(pkg, _, _)| !pkg.contains("aws-amplify"))
                .cloned()
                .collect();

            prop_assume!(!filtered_packages.is_empty());

            let mut entries: Vec<LiveUpdateEntry> = filtered_packages
                .iter()
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            entries.push(LiveUpdateEntry {
                name: None,
                pkg: "@aws-amplify/cli".to_string(),
                pkg_type: "npm".to_string(),
                version: old_version,
            });

            let json = serde_json::to_string(&entries).unwrap();
            let result = update_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should successfully update JSON");

            let updated_json = result.unwrap();
            let updated_entries: Vec<LiveUpdateEntry> = serde_json::from_str(&updated_json).unwrap();

            // Verify all other packages are preserved
            for (pkg, pkg_type, version) in &filtered_packages {
                let found = updated_entries.iter().find(|e| &e.pkg == pkg);
                prop_assert!(found.is_some(), "Package {} should be preserved", pkg);
                let entry = found.unwrap();
                prop_assert_eq!(&entry.pkg_type, pkg_type, "Package type should be preserved");
                prop_assert_eq!(&entry.version, version, "Package version should be preserved");
            }
        }

        /// Property test: Update adds @aws-amplify/cli if missing
        #[test]
        fn prop_live_updates_version_update_adds_if_missing(
            packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{1,15}", "[a-z]+", "[a-z0-9\\.]+"),
                0..4
            )
        ) {
            // Filter out @aws-amplify/cli
            let entries: Vec<LiveUpdateEntry> = packages
                .iter()
                .filter(|(pkg, _, _)| !pkg.contains("aws-amplify"))
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            let json = serde_json::to_string(&entries).unwrap();
            let result = update_live_updates_version(&json);

            prop_assert!(result.is_ok(), "Should successfully update JSON");

            let updated_json = result.unwrap();
            let updated_entries: Vec<LiveUpdateEntry> = serde_json::from_str(&updated_json).unwrap();

            // @aws-amplify/cli should now be present with version "latest"
            let cli_entry = updated_entries.iter().find(|e| e.pkg == "@aws-amplify/cli");
            prop_assert!(cli_entry.is_some(), "@aws-amplify/cli should be added");
            prop_assert_eq!(&cli_entry.unwrap().version, "latest",
                "@aws-amplify/cli version should be 'latest'");
        }

        /// Property test: Round-trip - update then check should return true
        #[test]
        fn prop_live_updates_round_trip(
            old_version in "[0-9]+\\.[0-9]+\\.[0-9]+",
            other_packages in prop::collection::vec(
                ("[a-z][a-z0-9-]{0,15}", "[a-z]+", "[a-z0-9\\.]+"),
                0..3
            )
        ) {
            prop_assume!(old_version != "latest");

            let mut entries: Vec<LiveUpdateEntry> = other_packages
                .iter()
                .map(|(pkg, pkg_type, version)| LiveUpdateEntry {
                    name: None,
                    pkg: pkg.clone(),
                    pkg_type: pkg_type.clone(),
                    version: version.clone(),
                })
                .collect();

            entries.push(LiveUpdateEntry {
                name: None,
                pkg: "@aws-amplify/cli".to_string(),
                pkg_type: "npm".to_string(),
                version: old_version,
            });

            let json = serde_json::to_string(&entries).unwrap();

            // First, check should return false (not latest)
            let check_before = check_live_updates_version(&json).unwrap();
            prop_assert!(!check_before, "Check before update should return false");

            // Update
            let updated_json = update_live_updates_version(&json).unwrap();

            // After update, check should return true
            let check_after = check_live_updates_version(&updated_json).unwrap();
            prop_assert!(check_after, "Check after update should return true");
        }
    }

    // ==================== Gen2 Build Configuration Tests ====================

    #[test]
    fn test_check_amplify_yml_exists_true() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create amplify.yml file
        fs::write(path.join("amplify.yml"), "version: 1").unwrap();

        let result = check_amplify_yml_exists(path.to_str().unwrap());
        assert!(result, "Should return true when amplify.yml exists");
    }

    #[test]
    fn test_check_amplify_yml_exists_false() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let result = check_amplify_yml_exists(path.to_str().unwrap());
        assert!(
            !result,
            "Should return false when amplify.yml does not exist"
        );
    }

    #[test]
    fn test_read_amplify_yml_success() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let content =
            "version: 1\nbackend:\n  phases:\n    build:\n      commands:\n        - npm ci";
        fs::write(path.join("amplify.yml"), content).unwrap();

        let result = read_amplify_yml(path.to_str().unwrap());
        assert!(result.is_ok(), "Should successfully read amplify.yml");
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_read_amplify_yml_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let result = read_amplify_yml(path.to_str().unwrap());
        assert!(
            result.is_err(),
            "Should return error when amplify.yml does not exist"
        );
        assert!(result.unwrap_err().contains("Failed to read amplify.yml"));
    }

    #[test]
    fn test_parse_build_commands_basic() {
        let yaml = r#"
version: 1
backend:
  phases:
    build:
      commands:
        - npm ci
        - npx ampx generate outputs --branch $AWS_BRANCH --app-id $AWS_APP_ID --out-dir amplify_outputs
"#;

        let result = parse_build_commands(yaml);
        assert!(result.is_ok(), "Should successfully parse YAML");

        let commands = result.unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "npm ci");
        assert!(commands[1].contains("npx ampx generate outputs"));
    }

    #[test]
    fn test_parse_build_commands_filters_comments() {
        let yaml = r#"
version: 1
backend:
  phases:
    build:
      commands:
        - npm ci
        - # This is a commented command
        - npx ampx generate outputs --branch $AWS_BRANCH
        - # Another comment
"#;

        let result = parse_build_commands(yaml);
        assert!(result.is_ok(), "Should successfully parse YAML");

        let commands = result.unwrap();
        assert_eq!(commands.len(), 2, "Should filter out commented lines");
        assert_eq!(commands[0], "npm ci");
        assert!(commands[1].contains("npx ampx generate outputs"));
    }

    #[test]
    fn test_parse_build_commands_missing_path() {
        let yaml = r#"
version: 1
frontend:
  phases:
    build:
      commands:
        - npm run build
"#;

        let result = parse_build_commands(yaml);
        assert!(
            result.is_err(),
            "Should return error when backend.phases.build.commands is missing"
        );
    }

    #[test]
    fn test_parse_build_commands_invalid_yaml() {
        let yaml = "this is not valid yaml: [[[";

        let result = parse_build_commands(yaml);
        assert!(result.is_err(), "Should return error for invalid YAML");
    }

    #[test]
    fn test_needs_build_command_update_true() {
        let command = "npx ampx generate outputs --branch $AWS_BRANCH --app-id $AWS_APP_ID --out-dir amplify_outputs";

        let result = needs_build_command_update(command);
        assert!(result, "Should return true for 'generate outputs' command");
    }

    #[test]
    fn test_needs_build_command_update_false_already_updated() {
        let command = "npx ampx pipeline-deploy --branch $AWS_BRANCH --app-id $AWS_APP_ID --outputs-out-dir amplify_outputs";

        let result = needs_build_command_update(command);
        assert!(!result, "Should return false for 'pipeline-deploy' command");
    }

    #[test]
    fn test_needs_build_command_update_false_other_command() {
        let command = "npm ci";

        let result = needs_build_command_update(command);
        assert!(!result, "Should return false for unrelated commands");
    }

    #[test]
    fn test_update_build_command_basic() {
        let command = "npx ampx generate outputs --branch $AWS_BRANCH --app-id $AWS_APP_ID --out-dir amplify_outputs";

        let result = update_build_command(command);

        assert!(
            result.contains("npx ampx pipeline-deploy"),
            "Should replace 'generate outputs' with 'pipeline-deploy'"
        );
        assert!(
            result.contains("--outputs-out-dir"),
            "Should replace '--out-dir' with '--outputs-out-dir'"
        );
        assert!(
            !result.contains("generate outputs"),
            "Should not contain old command"
        );
        assert!(
            !result.contains("--out-dir "),
            "Should not contain old parameter (with space after)"
        );
    }

    #[test]
    fn test_update_build_command_preserves_other_params() {
        let command = "npx ampx generate outputs --branch $AWS_BRANCH --app-id $AWS_APP_ID --out-dir amplify_outputs --profile myprofile";

        let result = update_build_command(command);

        assert!(
            result.contains("--branch $AWS_BRANCH"),
            "Should preserve --branch parameter"
        );
        assert!(
            result.contains("--app-id $AWS_APP_ID"),
            "Should preserve --app-id parameter"
        );
        assert!(
            result.contains("--profile myprofile"),
            "Should preserve --profile parameter"
        );
        assert!(
            result.contains("--outputs-out-dir amplify_outputs"),
            "Should update --out-dir to --outputs-out-dir"
        );
    }

    #[test]
    fn test_update_build_command_no_out_dir() {
        let command = "npx ampx generate outputs --branch $AWS_BRANCH --app-id $AWS_APP_ID";

        let result = update_build_command(command);

        assert!(
            result.contains("npx ampx pipeline-deploy"),
            "Should replace command"
        );
        assert!(
            result.contains("--branch $AWS_BRANCH"),
            "Should preserve parameters"
        );
        assert!(
            !result.contains("--out-dir"),
            "Should not add --out-dir if not present"
        );
    }

    #[test]
    fn test_update_build_command_already_updated() {
        let command =
            "npx ampx pipeline-deploy --branch $AWS_BRANCH --outputs-out-dir amplify_outputs";

        let result = update_build_command(command);

        // Should be idempotent - running on already updated command should not break it
        assert_eq!(result, command, "Should not change already updated command");
    }
}
