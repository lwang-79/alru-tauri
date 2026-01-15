use crate::command::create_clean_shell_command;
use serde::{Deserialize, Serialize};

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
    let output = create_clean_shell_command("npm")
        .args(["view", "@aws-amplify/cli", "version"])
        .output()
        .map_err(|e| format!("Failed to execute npm command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to fetch latest version: {}", stderr));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    println!(
        "[fetch_latest_amplify_cli_version] Latest version: {}",
        version
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
}
