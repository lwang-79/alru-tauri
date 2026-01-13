use crate::command::CommandExtClean;
use crate::file_ops::detector::{BackendType, PackageManager};
use crate::file_ops::BuildResult;
use std::process::Command;

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
    use tauri::Emitter;

    let mut all_output = String::new();

    let _ = window.emit("build-status", "running");

    // Helper function to run a command with streaming output
    let run_command_streaming = |cmd_name: &str,
                                 args: &[&str],
                                 working_dir: &str,
                                 window: &tauri::Window|
     -> Result<bool, String> {
        let _ = window.emit(
            "build-output",
            format!("$ {} {}\n", cmd_name, args.join(" ")),
        );

        let mut command = Command::new(cmd_name).clean_env();
        command.args(args).current_dir(working_dir);

        crate::command::run_command_streaming(command, window, "build-output")
    };

    // Step 1: Run backend build for Gen1 only
    if backend_type == BackendType::Gen1 {
        let _ = window.emit("build-output", "\n=== Running Amplify Build (Gen1) ===\n");
        all_output.push_str("\n=== Running Amplify Build (Gen1) ===\n");

        let success = run_command_streaming("amplify", &["build"], &project_path, &window)?;

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

    let success = run_command_streaming(cmd, &args, &project_path, &window)?;

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
