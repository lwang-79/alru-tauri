use super::core::execute_aws_command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Amplify branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyBranch {
    pub branch_name: String,
    pub stack_arn: String,
    pub backend_environment_name: String,
    pub environment_variables: HashMap<String, String>,
    pub is_protected: bool,
    pub protection_info: Option<String>,
}

/// Response structure for AWS Amplify list-branches command
#[derive(Debug, Deserialize)]
struct AmplifyListBranchesResponse {
    branches: Vec<AmplifyBranchRaw>,
}

/// Raw Amplify branch from AWS CLI response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyBranchRaw {
    branch_name: String,
    #[serde(default)]
    backend: Option<AmplifyBackend>,
    #[serde(default)]
    backend_environment_arn: Option<String>,
    #[serde(default)]
    environment_variables: Option<HashMap<String, String>>,
}

/// Backend info for Amplify branch
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyBackend {
    #[serde(default)]
    stack_arn: Option<String>,
}

/// Tauri command to list branches for an Amplify app
#[tauri::command]
pub async fn list_amplify_branches(
    profile: &str,
    region: &str,
    app_id: &str,
) -> Result<Vec<AmplifyBranch>, String> {
    let output = execute_aws_command(
        &["amplify", "list-branches", "--app-id", app_id],
        profile,
        region,
    )?;

    let response: AmplifyListBranchesResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify branches response: {}", e))?;

    let branches = response
        .branches
        .into_iter()
        .map(|raw| {
            // Extract backend environment name from ARN
            // e.g., "arn:aws:amplify:ap-southeast-2:123456:apps/xxx/backendenvironments/prod" -> "prod"
            let backend_env_name = raw
                .backend_environment_arn
                .as_ref()
                .and_then(|arn| arn.split('/').last())
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw.branch_name.clone());

            AmplifyBranch {
                branch_name: raw.branch_name,
                stack_arn: raw.backend.and_then(|b| b.stack_arn).unwrap_or_default(),
                backend_environment_name: backend_env_name,
                environment_variables: raw.environment_variables.unwrap_or_default(),
                is_protected: false, // Will be checked from frontend
                protection_info: None,
            }
        })
        .collect();

    Ok(branches)
}
