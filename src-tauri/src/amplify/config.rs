use crate::file_ops::{BuildConfigChange, BuildConfigLocation, BuildConfigUpdateResult};
use std::path::Path;

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
    use std::fs;
    use tempfile::TempDir;

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
