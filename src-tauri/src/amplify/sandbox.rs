use crate::command::create_clean_shell_command;
use serde::{Deserialize, Serialize};
use std::process::Stdio;

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
    let mut child = create_clean_shell_command("npx")
        .args(["ampx", "sandbox", "--profile", &profile])
        .current_dir(&project_path)
        .env("AWS_REGION", &region)
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
) -> Result<crate::file_ops::BuildResult, String> {
    use crate::file_ops::BuildResult;

    let output = create_clean_shell_command("npx")
        .args(["ampx", "sandbox", "delete", "--profile", profile, "-y"])
        .current_dir(project_path)
        .env("AWS_REGION", region)
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
