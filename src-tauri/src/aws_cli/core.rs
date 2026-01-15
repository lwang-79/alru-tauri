use crate::command::create_clean_shell_command;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

/// AWS CLI profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsProfile {
    pub name: String,
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
#[allow(dead_code)] // Used in tests and get_profiles_from_files
pub fn parse_credentials_file(content: &str) -> Vec<String> {
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
#[allow(dead_code)] // Used in tests and get_profiles_from_files
pub fn parse_config_file(content: &str) -> Vec<String> {
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

/// Execute AWS CLI command with profile and region
pub fn execute_aws_command(args: &[&str], profile: &str, region: &str) -> Result<String, String> {
    let output = create_clean_shell_command("aws")
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

/// Extract repository name from a repository URL
/// e.g., "https://github.com/lwang-79/todo-gen2" -> "todo-gen2"
pub fn extract_repo_name(repository_url: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
