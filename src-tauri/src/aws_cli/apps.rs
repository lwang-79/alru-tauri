use super::core::execute_aws_command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Amplify application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyApp {
    pub app_id: String,
    pub name: String,
    pub repository: String,
    pub environment_variables: HashMap<String, String>,
}

/// Response structure for AWS Amplify list-apps command
#[derive(Debug, Deserialize)]
struct AmplifyListAppsResponse {
    apps: Vec<AmplifyAppRaw>,
}

/// Raw Amplify app from AWS CLI response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyAppRaw {
    app_id: String,
    name: String,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    environment_variables: Option<HashMap<String, String>>,
}

/// Tauri command to list Amplify apps in a region
#[tauri::command]
pub async fn list_amplify_apps(profile: &str, region: &str) -> Result<Vec<AmplifyApp>, String> {
    let output = execute_aws_command(&["amplify", "list-apps"], profile, region)?;

    let response: AmplifyListAppsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify apps response: {}", e))?;

    let apps = response
        .apps
        .into_iter()
        .map(|raw| AmplifyApp {
            app_id: raw.app_id,
            name: raw.name,
            repository: raw.repository.unwrap_or_default(),
            environment_variables: raw.environment_variables.unwrap_or_default(),
        })
        .collect();

    Ok(apps)
}

/// Get app by app ID
pub async fn get_app_by_id(
    profile: &str,
    region: &str,
    app_id: &str,
) -> Result<AmplifyApp, String> {
    let apps = list_amplify_apps(profile, region).await?;
    apps.into_iter()
        .find(|app| app.app_id == app_id)
        .ok_or_else(|| format!("App with ID {} not found", app_id))
}
