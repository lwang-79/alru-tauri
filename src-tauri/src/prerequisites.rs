use crate::command::{create_clean_shell_command, CommandExtClean};
use serde::{Deserialize, Serialize};

/// Status of a required tool (AWS CLI, Git, Node.js)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// Result of checking all prerequisites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrerequisitesResult {
    pub network: ToolStatus,
    pub aws_cli: ToolStatus,
    pub git: ToolStatus,
    pub nodejs: ToolStatus,
    // Optional tools - absence won't block continuation
    pub amplify_cli: ToolStatus,
    pub npm: ToolStatus,
    pub yarn: ToolStatus,
    pub pnpm: ToolStatus,
    pub bun: ToolStatus,
}

impl ToolStatus {
    /// Create a successful tool status with version
    pub fn success(version: String) -> Self {
        Self {
            installed: true,
            version: Some(version),
            error: None,
        }
    }

    /// Create a failed tool status with error message
    pub fn failure(error: String) -> Self {
        Self {
            installed: false,
            version: None,
            error: Some(error),
        }
    }
}

/// Parse AWS CLI version from output string
/// Expected format: "aws-cli/2.15.0 Python/3.11.6 ..."
pub fn parse_aws_cli_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .next()
        .and_then(|s| s.strip_prefix("aws-cli/"))
        .map(|v| v.to_string())
}

/// Parse Git version from output string
/// Expected format: "git version 2.43.0"
pub fn parse_git_version(output: &str) -> Option<String> {
    output
        .strip_prefix("git version ")
        .map(|s| s.trim().to_string())
}

/// Parse Node.js version from output string
/// Expected format: "v20.10.0"
pub fn parse_nodejs_version(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.starts_with('v') {
        Some(trimmed[1..].to_string())
    } else {
        Some(trimmed.to_string())
    }
}

/// Check network connectivity by trying to reach a reliable endpoint
fn check_network() -> ToolStatus {
    // Try to connect to multiple reliable endpoints
    let endpoints = [
        "https://aws.amazon.com",
        "https://github.com",
        "https://npmjs.com",
    ];

    for endpoint in &endpoints {
        match std::process::Command::new("curl")
            .clean_env()
            .args(["-s", "-I", "--connect-timeout", "10", endpoint])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let response = String::from_utf8_lossy(&output.stdout).to_uppercase();
                    if response.contains("HTTP/")
                        && (response.contains(" 200")
                            || response.contains(" 301")
                            || response.contains(" 302"))
                    {
                        return ToolStatus::success("Connected".to_string());
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // If curl is not available or fails, try ping as fallback
    let ping_args = if cfg!(target_os = "windows") {
        ["-n", "1", "-w", "5000", "8.8.8.8"]
    } else {
        ["-c", "1", "-W", "5000", "8.8.8.8"]
    };

    match std::process::Command::new("ping")
        .clean_env()
        .args(ping_args)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                ToolStatus::success("Connected".to_string())
            } else {
                ToolStatus::failure(
                    "No internet connection detected. Please check your network connection."
                        .to_string(),
                )
            }
        }
        Err(_) => ToolStatus::failure(
            "Unable to verify network connection. Network tools not available.".to_string(),
        ),
    }
}

/// Check if AWS CLI is installed and get its version
fn check_aws_cli() -> ToolStatus {
    match create_clean_shell_command("aws").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // AWS CLI may output to stderr on some systems
                let stderr = String::from_utf8_lossy(&output.stderr);
                let version_str = if stdout.contains("aws-cli") {
                    stdout.to_string()
                } else {
                    stderr.to_string()
                };

                match parse_aws_cli_version(&version_str) {
                    Some(version) => ToolStatus::success(version),
                    None => ToolStatus::failure("Could not parse AWS CLI version".to_string()),
                }
            } else {
                ToolStatus::failure("AWS CLI command failed".to_string())
            }
        }
        Err(e) => ToolStatus::failure(format!("AWS CLI not found: {}", e)),
    }
}

/// Check if Git is installed and get its version
fn check_git() -> ToolStatus {
    match create_clean_shell_command("git").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match parse_git_version(&stdout) {
                    Some(version) => ToolStatus::success(version),
                    None => ToolStatus::failure("Could not parse Git version".to_string()),
                }
            } else {
                ToolStatus::failure("Git command failed".to_string())
            }
        }
        Err(e) => ToolStatus::failure(format!("Git not found: {}", e)),
    }
}

/// Check if Node.js is installed and get its version
fn check_nodejs() -> ToolStatus {
    match create_clean_shell_command("node").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match parse_nodejs_version(&stdout) {
                    Some(version) => ToolStatus::success(version),
                    None => ToolStatus::failure("Could not parse Node.js version".to_string()),
                }
            } else {
                ToolStatus::failure("Node.js command failed".to_string())
            }
        }
        Err(e) => ToolStatus::failure(format!("Node.js not found: {}", e)),
    }
}

/// Check if Amplify CLI is installed and get its version
fn check_amplify_cli() -> ToolStatus {
    match create_clean_shell_command("amplify").arg("-v").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !stdout.is_empty() {
                    ToolStatus::success(stdout)
                } else {
                    ToolStatus::failure("Could not parse Amplify CLI version".to_string())
                }
            } else {
                ToolStatus::failure("Amplify CLI command failed".to_string())
            }
        }
        Err(_) => ToolStatus::failure("Amplify CLI not installed".to_string()),
    }
}

/// Check if npm is installed and get its version
fn check_npm() -> ToolStatus {
    match create_clean_shell_command("npm").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolStatus::success(version)
            } else {
                ToolStatus::failure("npm command failed".to_string())
            }
        }
        Err(_) => ToolStatus::failure("npm not installed".to_string()),
    }
}

/// Check if yarn is installed and get its version
fn check_yarn() -> ToolStatus {
    match create_clean_shell_command("yarn").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolStatus::success(version)
            } else {
                ToolStatus::failure("yarn command failed".to_string())
            }
        }
        Err(_) => ToolStatus::failure("yarn not installed".to_string()),
    }
}

/// Check if pnpm is installed and get its version
fn check_pnpm() -> ToolStatus {
    match create_clean_shell_command("pnpm").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolStatus::success(version)
            } else {
                ToolStatus::failure("pnpm command failed".to_string())
            }
        }
        Err(_) => ToolStatus::failure("pnpm not installed".to_string()),
    }
}

/// Check if bun is installed and get its version
fn check_bun() -> ToolStatus {
    match create_clean_shell_command("bun").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolStatus::success(version)
            } else {
                ToolStatus::failure("bun command failed".to_string())
            }
        }
        Err(_) => ToolStatus::failure("bun not installed".to_string()),
    }
}

/// Check all prerequisites and return their status
#[tauri::command]
pub async fn check_prerequisites() -> Result<PrerequisitesResult, String> {
    // Check network connectivity first
    let network_status = check_network();

    Ok(PrerequisitesResult {
        network: network_status,
        // Always check local tools regardless of network - these are version checks only
        aws_cli: check_aws_cli(),
        git: check_git(),
        nodejs: check_nodejs(),
        amplify_cli: check_amplify_cli(),
        npm: check_npm(),
        yarn: check_yarn(),
        pnpm: check_pnpm(),
        bun: check_bun(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_aws_cli_version() {
        assert_eq!(
            parse_aws_cli_version("aws-cli/2.15.0 Python/3.11.6 Darwin/23.0.0"),
            Some("2.15.0".to_string())
        );
        assert_eq!(
            parse_aws_cli_version("aws-cli/1.27.0 Python/3.9.0"),
            Some("1.27.0".to_string())
        );
        assert_eq!(parse_aws_cli_version("invalid output"), None);
    }

    #[test]
    fn test_parse_git_version() {
        assert_eq!(
            parse_git_version("git version 2.43.0"),
            Some("2.43.0".to_string())
        );
        assert_eq!(
            parse_git_version("git version 2.39.3 (Apple Git-145)"),
            Some("2.39.3 (Apple Git-145)".to_string())
        );
        assert_eq!(parse_git_version("invalid output"), None);
    }

    #[test]
    fn test_parse_nodejs_version() {
        assert_eq!(
            parse_nodejs_version("v20.10.0"),
            Some("20.10.0".to_string())
        );
        assert_eq!(
            parse_nodejs_version("v18.19.0\n"),
            Some("18.19.0".to_string())
        );
        assert_eq!(parse_nodejs_version("22.0.0"), Some("22.0.0".to_string()));
    }
}
