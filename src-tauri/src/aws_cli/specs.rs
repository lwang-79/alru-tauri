use super::core::execute_aws_command;

/// Get the buildSpec for an Amplify app
/// Calls `aws amplify get-app --app-id <id>` and extracts the buildSpec field
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
///
/// # Returns
/// The buildSpec string from the app configuration
pub fn get_app_build_spec(profile: &str, region: &str, app_id: &str) -> Result<String, String> {
    let output = execute_aws_command(&["amplify", "get-app", "--app-id", app_id], profile, region)?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse get-app response: {}", e))?;

    let build_spec = response["app"]["buildSpec"]
        .as_str()
        .ok_or_else(|| "buildSpec field not found in app configuration".to_string())?
        .to_string();

    Ok(build_spec)
}

/// Update the buildSpec for an Amplify app
/// Calls `aws amplify update-app --app-id <id> --build-spec <spec>`
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
/// * `build_spec` - The new buildSpec YAML string
///
/// # Returns
/// `Ok(true)` on success, `Err` with error message on failure
pub fn update_app_build_spec(
    profile: &str,
    region: &str,
    app_id: &str,
    build_spec: &str,
) -> Result<bool, String> {
    let output = crate::command::create_clean_command("aws")
        .args([
            "amplify",
            "update-app",
            "--app-id",
            app_id,
            "--build-spec",
            build_spec,
            "--profile",
            profile,
            "--region",
            region,
        ])
        .output()
        .map_err(|e| format!("Failed to execute AWS CLI for buildSpec update: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to update app buildSpec: {}", stderr));
    }

    Ok(true)
}

/// Revert the buildSpec for an Amplify app to its original value
/// This is a wrapper around update_app_build_spec for clarity
///
/// # Arguments
/// * `profile` - AWS CLI profile name
/// * `region` - AWS region
/// * `app_id` - Amplify app ID
/// * `original_build_spec` - The original buildSpec YAML string to restore
///
/// # Returns
/// `Ok(true)` on success, `Err` with error message on failure
#[tauri::command]
pub fn revert_build_spec(
    profile: &str,
    region: &str,
    app_id: &str,
    original_build_spec: &str,
) -> Result<bool, String> {
    update_app_build_spec(profile, region, app_id, original_build_spec)
}
