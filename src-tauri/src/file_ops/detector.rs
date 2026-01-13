use serde::{Deserialize, Serialize};
use std::path::Path;

/// Backend type (Gen1 or Gen2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendType {
    Gen1,
    Gen2,
}

/// Package manager type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
    Bun,
}

/// Detects the package manager used in a project by checking for lock files.
/// Priority order: bun.lockb > pnpm-lock.yaml > yarn.lock > package-lock.json
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(PackageManager)` - The detected package manager
/// * `Err(String)` - Error message if no lock file is found
#[tauri::command]
pub async fn detect_package_manager(project_path: &str) -> Result<PackageManager, String> {
    detect_package_manager_sync(project_path)
}

/// Synchronous version of package manager detection for use in other functions
pub fn detect_package_manager_sync(project_path: &str) -> Result<PackageManager, String> {
    let path = Path::new(project_path);

    // Check for lock files in priority order (most specific first)
    if path.join("bun.lockb").exists() {
        return Ok(PackageManager::Bun);
    }

    if path.join("pnpm-lock.yaml").exists() {
        return Ok(PackageManager::Pnpm);
    }

    if path.join("yarn.lock").exists() {
        return Ok(PackageManager::Yarn);
    }

    if path.join("package-lock.json").exists() {
        return Ok(PackageManager::Npm);
    }

    Err("No package manager lock file found. Expected one of: package-lock.json, yarn.lock, pnpm-lock.yaml, or bun.lockb".to_string())
}

/// Detects the backend type (Gen1 or Gen2) by checking package.json for @aws-amplify/backend
///
/// # Arguments
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(BackendType)` - Gen2 if @aws-amplify/backend is in devDependencies, Gen1 otherwise
/// * `Err(String)` - Error message if package.json cannot be read or parsed
#[tauri::command]
pub async fn detect_backend_type(project_path: &str) -> Result<BackendType, String> {
    detect_backend_type_sync(project_path)
}

/// Synchronous version of backend type detection for use in other functions
pub fn detect_backend_type_sync(project_path: &str) -> Result<BackendType, String> {
    let package_json_path = Path::new(project_path).join("package.json");

    let content = std::fs::read_to_string(&package_json_path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;

    detect_backend_type_from_content(&content)
}

/// Detects backend type from package.json content string
/// This is separated for easier testing
pub fn detect_backend_type_from_content(content: &str) -> Result<BackendType, String> {
    let package_json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    // Check devDependencies for @aws-amplify/backend
    if let Some(dev_deps) = package_json.get("devDependencies") {
        if dev_deps.get("@aws-amplify/backend").is_some() {
            return Ok(BackendType::Gen2);
        }
    }

    // Also check regular dependencies as a fallback
    if let Some(deps) = package_json.get("dependencies") {
        if deps.get("@aws-amplify/backend").is_some() {
            return Ok(BackendType::Gen2);
        }
    }

    Ok(BackendType::Gen1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_npm_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Npm);
    }

    #[test]
    fn test_detect_yarn_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("yarn.lock"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Yarn);
    }

    #[test]
    fn test_detect_pnpm_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Pnpm);
    }

    #[test]
    fn test_detect_bun_package_manager() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bun.lockb"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Bun);
    }

    #[test]
    fn test_no_lock_file_returns_error() {
        let temp_dir = TempDir::new().unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_bun_takes_priority_over_npm() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();
        fs::write(temp_dir.path().join("bun.lockb"), "").unwrap();

        let result = detect_package_manager_sync(temp_dir.path().to_str().unwrap());
        assert_eq!(result.unwrap(), PackageManager::Bun);
    }

    #[test]
    fn test_detect_gen2_backend_from_dev_dependencies() {
        let content = r#"{
            "name": "test-project",
            "devDependencies": {
                "@aws-amplify/backend": "^1.0.0",
                "@aws-amplify/backend-cli": "^1.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen2);
    }

    #[test]
    fn test_detect_gen2_backend_from_dependencies() {
        let content = r#"{
            "name": "test-project",
            "dependencies": {
                "@aws-amplify/backend": "^1.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen2);
    }

    #[test]
    fn test_detect_gen1_backend_without_amplify_backend() {
        let content = r#"{
            "name": "test-project",
            "devDependencies": {
                "aws-amplify": "^5.0.0"
            }
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen1);
    }

    #[test]
    fn test_detect_gen1_backend_empty_package_json() {
        let content = r#"{
            "name": "test-project"
        }"#;

        let result = detect_backend_type_from_content(content);
        assert_eq!(result.unwrap(), BackendType::Gen1);
    }

    #[test]
    fn test_invalid_json_returns_error() {
        let content = "not valid json";

        let result = detect_backend_type_from_content(content);
        assert!(result.is_err());
    }

    // Property-based tests
    proptest! {
        /// Property 5: Package Manager Detection
        #[test]
        fn prop_package_manager_detection(lock_file_type in 0u8..4) {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            // Create exactly one lock file based on the random type
            let (lock_file, expected_pm) = match lock_file_type {
                0 => ("package-lock.json", PackageManager::Npm),
                1 => ("yarn.lock", PackageManager::Yarn),
                2 => ("pnpm-lock.yaml", PackageManager::Pnpm),
                _ => ("bun.lockb", PackageManager::Bun),
            };

            fs::write(path.join(lock_file), "").unwrap();

            let result = detect_package_manager_sync(path.to_str().unwrap());

            prop_assert!(result.is_ok(), "Detection should succeed when lock file exists");
            prop_assert_eq!(result.unwrap(), expected_pm,
                "Expected {:?} for lock file {}", expected_pm, lock_file);
        }

        /// Property test: No lock file should return an error
        #[test]
        fn prop_no_lock_file_returns_error(random_files in prop::collection::vec("[a-z]+\\.[a-z]+", 0..5)) {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            // Create random files that are NOT lock files
            for file_name in random_files {
                // Skip if the random name happens to match a lock file
                if file_name == "package-lock.json" || file_name == "yarn.lock"
                    || file_name == "pnpm-lock.yaml" || file_name == "bun.lockb" {
                    continue;
                }
                let _ = fs::write(path.join(&file_name), "");
            }

            let result = detect_package_manager_sync(path.to_str().unwrap());

            // If no lock file was created, detection should fail
            let has_lock_file = path.join("package-lock.json").exists()
                || path.join("yarn.lock").exists()
                || path.join("pnpm-lock.yaml").exists()
                || path.join("bun.lockb").exists();

            if !has_lock_file {
                prop_assert!(result.is_err(), "Detection should fail when no lock file exists");
            }
        }

        /// Property test: Priority order is maintained (bun > pnpm > yarn > npm)
        #[test]
        fn prop_package_manager_priority(
            has_npm in any::<bool>(),
            has_yarn in any::<bool>(),
            has_pnpm in any::<bool>(),
            has_bun in any::<bool>()
        ) {
            // Skip if no lock files would be created
            prop_assume!(has_npm || has_yarn || has_pnpm || has_bun);

            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path();

            if has_npm {
                fs::write(path.join("package-lock.json"), "{}").unwrap();
            }
            if has_yarn {
                fs::write(path.join("yarn.lock"), "").unwrap();
            }
            if has_pnpm {
                fs::write(path.join("pnpm-lock.yaml"), "").unwrap();
            }
            if has_bun {
                fs::write(path.join("bun.lockb"), "").unwrap();
            }

            let result = detect_package_manager_sync(path.to_str().unwrap());
            prop_assert!(result.is_ok());

            let detected = result.unwrap();

            // Verify priority: bun > pnpm > yarn > npm
            let expected = if has_bun {
                PackageManager::Bun
            } else if has_pnpm {
                PackageManager::Pnpm
            } else if has_yarn {
                PackageManager::Yarn
            } else {
                PackageManager::Npm
            };

            prop_assert_eq!(detected, expected,
                "Priority not maintained: has_npm={}, has_yarn={}, has_pnpm={}, has_bun={}, expected {:?}, got {:?}",
                has_npm, has_yarn, has_pnpm, has_bun, expected, detected);
        }
    }

    // Property-based tests for Backend Type Detection
    proptest! {
        /// Property 6: Backend Type Detection
        #[test]
        fn prop_backend_type_detection_gen2_in_dev_deps(
            project_name in "[a-z][a-z0-9-]{0,20}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+"
        ) {
            let content = format!(r#"{{
                "name": "{}",
                "devDependencies": {{
                    "@aws-amplify/backend": "^{}"
                }}
            }}"#, project_name, version);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen2,
                "Should detect Gen2 when @aws-amplify/backend is in devDependencies");
        }

        /// Property test: Gen2 detection from regular dependencies
        #[test]
        fn prop_backend_type_detection_gen2_in_deps(
            project_name in "[a-z][a-z0-9-]{0,20}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+"
        ) {
            let content = format!(r#"{{
                "name": "{}",
                "dependencies": {{
                    "@aws-amplify/backend": "^{}"
                }}
            }}"#, project_name, version);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen2,
                "Should detect Gen2 when @aws-amplify/backend is in dependencies");
        }

        /// Property test: Gen1 detection when @aws-amplify/backend is absent
        #[test]
        fn prop_backend_type_detection_gen1_without_amplify_backend(
            project_name in "[a-z][a-z0-9-]{0,20}",
            other_dep in "[a-z][a-z0-9-]{0,20}"
        ) {
            // Ensure the other_dep is not @aws-amplify/backend
            prop_assume!(other_dep != "aws-amplify/backend");

            let content = format!(r#"{{
                "name": "{}",
                "devDependencies": {{
                    "{}": "^1.0.0"
                }}
            }}"#, project_name, other_dep);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen1,
                "Should detect Gen1 when @aws-amplify/backend is not present");
        }

        /// Property test: Gen1 detection with empty dependencies
        #[test]
        fn prop_backend_type_detection_gen1_empty_deps(
            project_name in "[a-z][a-z0-9-]{0,20}"
        ) {
            let content = format!(r#"{{
                "name": "{}"
            }}"#, project_name);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");
            prop_assert_eq!(result.unwrap(), BackendType::Gen1,
                "Should detect Gen1 when no dependencies are present");
        }

        /// Property test: devDependencies takes precedence (Gen2 detection)
        #[test]
        fn prop_backend_type_detection_dev_deps_precedence(
            project_name in "[a-z][a-z0-9-]{0,20}",
            has_in_dev_deps in any::<bool>(),
            has_in_deps in any::<bool>()
        ) {
            let dev_deps = if has_in_dev_deps {
                r#""devDependencies": { "@aws-amplify/backend": "^1.0.0" },"#
            } else {
                r#""devDependencies": { "other-package": "^1.0.0" },"#
            };

            let deps = if has_in_deps {
                r#""dependencies": { "@aws-amplify/backend": "^1.0.0" }"#
            } else {
                r#""dependencies": { "other-package": "^1.0.0" }"#
            };

            let content = format!(r#"{{
                "name": "{}",
                {}
                {}
            }}"#, project_name, dev_deps, deps);

            let result = detect_backend_type_from_content(&content);
            prop_assert!(result.is_ok(), "Should parse valid package.json");

            let expected = if has_in_dev_deps || has_in_deps {
                BackendType::Gen2
            } else {
                BackendType::Gen1
            };

            prop_assert_eq!(result.unwrap(), expected,
                "Backend type should be Gen2 if @aws-amplify/backend is in either devDependencies or dependencies");
        }
    }
}
