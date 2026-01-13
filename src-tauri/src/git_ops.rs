// Git operations module - handles repository operations

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// Result of cloning a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResult {
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of committing and pushing changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPushResult {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub error: Option<String>,
}

fn get_temp_clone_dir() -> PathBuf {
    use dirs;
    let base = match dirs::home_dir() {
        Some(path) => {
            println!("[get_temp_clone_dir] Using home dir: {:?}", path);
            path.join(".alru-cache")
        }
        None => {
            let path = env::temp_dir().join("amplify-runtime-updater");
            println!(
                "[get_temp_clone_dir] Home dir not found, using temp dir: {:?}",
                path
            );
            path
        }
    };
    base
}

/// Generate a unique directory name for a repository clone
fn generate_clone_dir_name(url: &str, branch: &str) -> String {
    // Extract repo name from URL
    let repo_name = url
        .trim_end_matches(".git")
        .split('/')
        .last()
        .or_else(|| url.split(':').last())
        .unwrap_or("repo");

    // Create a unique name with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    format!("{}-{}-{}", repo_name, branch, timestamp)
}

/// Execute a git command in a specific directory
fn execute_git_command(args: &[&str], cwd: Option<&str>) -> Result<String, String> {
    let mut cmd = crate::command::create_clean_command("git");
    cmd.args(args);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git error: {}", stderr));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Failed to parse git output: {}", e))
}

/// Clone a repository to a temporary directory and checkout the specified branch.
///
/// # Arguments
/// * `url` - The repository URL to clone
/// * `branch` - The branch to checkout after cloning
///
/// # Returns
/// A CloneResult containing the path to the cloned repository
#[tauri::command]
pub async fn clone_repository(url: &str, branch: &str) -> Result<CloneResult, String> {
    // Create the base temp directory if it doesn't exist
    let base_dir = get_temp_clone_dir();
    std::fs::create_dir_all(&base_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Generate unique directory name for this clone
    let clone_dir_name = generate_clone_dir_name(url, branch);
    let clone_path = base_dir.join(&clone_dir_name);
    let clone_path_str = clone_path.to_string_lossy().to_string();

    // Clone the repository
    execute_git_command(&["clone", url, &clone_path_str], None).map_err(|e| {
        format!(
            "Failed to clone repository: {}. Please check the repository URL and your access permissions.",
            e
        )
    })?;

    // Checkout the specified branch
    execute_git_command(&["checkout", branch], Some(&clone_path_str)).map_err(|e| {
        // Clean up the cloned directory on checkout failure
        let _ = std::fs::remove_dir_all(&clone_path);
        format!(
            "Failed to checkout branch '{}': {}. Please verify the branch exists.",
            branch, e
        )
    })?;

    Ok(CloneResult {
        path: clone_path_str,
        success: true,
        error: None,
    })
}

/// Clean up a cloned repository by removing its directory.
///
/// # Arguments
/// * `path` - The path to the cloned repository to remove
///
/// # Returns
/// * `Ok(true)` - Repository cleaned up successfully
/// * `Err(String)` - Error message if cleanup fails
#[tauri::command]
pub async fn cleanup_repository(path: &str) -> Result<bool, String> {
    let repo_path = std::path::Path::new(path);

    if !repo_path.exists() {
        return Ok(true); // Already cleaned up
    }

    std::fs::remove_dir_all(repo_path)
        .map_err(|e| format!("Failed to clean up repository at {}: {}", path, e))?;

    Ok(true)
}

/// Stage all changes, commit with a message, and push to the remote branch.
///
/// # Arguments
/// * `path` - The path to the repository
/// * `message` - The commit message
///
/// # Returns
/// A CommitPushResult containing the commit hash if successful
#[tauri::command]
pub async fn commit_and_push(path: &str, message: &str) -> Result<CommitPushResult, String> {
    // Stage all changes
    execute_git_command(&["add", "-A"], Some(path)).map_err(|e| {
        format!(
            "Failed to stage changes: {}. Please check if the repository is valid.",
            e
        )
    })?;

    // Check if there are any changes to commit
    let status_output = execute_git_command(&["status", "--porcelain"], Some(path))?;
    if status_output.trim().is_empty() {
        return Ok(CommitPushResult {
            success: true,
            commit_hash: None,
            error: Some("No changes to commit".to_string()),
        });
    }

    // Commit the changes
    execute_git_command(&["commit", "-m", message], Some(path)).map_err(|e| {
        format!(
            "Failed to commit changes: {}. Please check your git configuration.",
            e
        )
    })?;

    // Get the commit hash
    let commit_hash = execute_git_command(&["rev-parse", "HEAD"], Some(path))
        .map(|s| s.trim().to_string())
        .ok();

    // Push to remote
    execute_git_command(&["push"], Some(path)).map_err(|e| {
        format!(
            "Failed to push changes: {}. Please check your remote access and try again.",
            e
        )
    })?;

    Ok(CommitPushResult {
        success: true,
        commit_hash,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_clone_dir_name() {
        let name = generate_clone_dir_name("https://github.com/user/my-repo.git", "main");
        assert!(name.starts_with("my-repo-main-"));

        let name2 = generate_clone_dir_name("git@github.com:user/another-repo", "develop");
        assert!(name2.starts_with("another-repo-develop-"));
    }

    #[test]
    fn test_get_temp_clone_dir() {
        let dir = get_temp_clone_dir();
        assert!(dir.ends_with("amplify-runtime-updater"));
    }
}
