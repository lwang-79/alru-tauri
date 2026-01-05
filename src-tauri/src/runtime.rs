// Runtime detection module - fetches and parses Node.js release schedule

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node.js release schedule URL
const NODEJS_RELEASE_SCHEDULE_URL: &str =
    "https://raw.githubusercontent.com/nodejs/Release/main/schedule.json";

/// Node.js version information from release schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    pub version: String,
    pub lts: Option<String>,
    pub start: String,
    pub end: String,
    pub is_supported: bool,
}

/// Raw schedule entry from the Node.js release schedule JSON
#[derive(Debug, Clone, Deserialize)]
struct RawScheduleEntry {
    start: String,
    end: String,
    #[serde(default)]
    codename: Option<String>,
}

/// Check if a Node.js major version number is an LTS candidate.
/// LTS versions are those with even major version numbers.
///
/// # Arguments
/// * `major_version` - The major version number to check
///
/// # Returns
/// `true` if the major version is even (LTS candidate), `false` otherwise
pub fn is_lts_candidate(major_version: u32) -> bool {
    major_version % 2 == 0
}

/// Extract the major version number from a version string.
/// Handles formats like "v20", "20", "v20.10.0", "20.10.0"
///
/// # Arguments
/// * `version_str` - The version string to parse
///
/// # Returns
/// The major version number if parsing succeeds, None otherwise
pub fn extract_major_version(version_str: &str) -> Option<u32> {
    let trimmed = version_str.trim().trim_start_matches('v');
    let major_str = trimmed.split('.').next()?;
    major_str.parse().ok()
}

/// Check if a version string represents an LTS candidate.
/// Combines version extraction and LTS check.
///
/// # Arguments
/// * `version_str` - The version string to check (e.g., "v20", "18.19.0")
///
/// # Returns
/// `true` if the version is an LTS candidate, `false` otherwise
pub fn is_version_lts_candidate(version_str: &str) -> Option<bool> {
    extract_major_version(version_str).map(is_lts_candidate)
}

/// Check if a version is currently supported based on start and end dates.
/// A version is supported if the current date is after the start date and before the end date.
///
/// # Arguments
/// * `start_date` - The start date string in YYYY-MM-DD format
/// * `end_date` - The end date string in YYYY-MM-DD format
///
/// # Returns
/// `true` if the version is currently supported, `false` otherwise
pub fn is_version_supported(start_date: &str, end_date: &str) -> bool {
    is_version_supported_at_date(start_date, end_date, Utc::now().date_naive())
}

/// Check if a version is supported at a specific reference date.
/// A version is supported if the reference date is after the start date and before the end date.
///
/// # Arguments
/// * `start_date` - The start date string in YYYY-MM-DD format
/// * `end_date` - The end date string in YYYY-MM-DD format
/// * `reference_date` - The date to check against
///
/// # Returns
/// `true` if the version is supported at the reference date, `false` otherwise
pub fn is_version_supported_at_date(
    start_date: &str,
    end_date: &str,
    reference_date: NaiveDate,
) -> bool {
    let start = match NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return false,
    };
    let end = match NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return false,
    };

    reference_date >= start && reference_date < end
}

/// Parse the Node.js release schedule JSON into a list of NodeVersion structs.
/// Filters for LTS versions (even major numbers) and calculates support status.
///
/// # Arguments
/// * `json_str` - The raw JSON string from the release schedule
///
/// # Returns
/// A vector of NodeVersion structs, or an error message
pub fn parse_release_schedule(json_str: &str) -> Result<Vec<NodeVersion>, String> {
    let schedule: HashMap<String, RawScheduleEntry> =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut versions: Vec<NodeVersion> = schedule
        .into_iter()
        .filter_map(|(version_key, entry)| {
            // Extract major version and check if it's an LTS candidate
            let major = extract_major_version(&version_key)?;
            if !is_lts_candidate(major) {
                return None;
            }

            let is_supported = is_version_supported(&entry.start, &entry.end);

            Some(NodeVersion {
                version: version_key,
                lts: entry.codename,
                start: entry.start,
                end: entry.end,
                is_supported,
            })
        })
        .collect();

    // Sort by major version number (descending - newest first)
    versions.sort_by(|a, b| {
        let a_major = extract_major_version(&a.version).unwrap_or(0);
        let b_major = extract_major_version(&b.version).unwrap_or(0);
        b_major.cmp(&a_major)
    });

    Ok(versions)
}

/// Get the target runtime - the second oldest supported LTS version.
/// This provides a balance between stability and modern features.
/// If there's only one supported version, it returns that version.
///
/// # Arguments
/// * `versions` - A list of NodeVersion structs
///
/// # Returns
/// The Lambda runtime string (e.g., "nodejs22.x") for the second oldest supported LTS version
pub fn get_oldest_supported_lts(versions: &[NodeVersion]) -> Option<String> {
    let mut supported: Vec<(u32, &NodeVersion)> = versions
        .iter()
        .filter(|v| v.is_supported)
        .filter_map(|v| {
            let major = extract_major_version(&v.version)?;
            Some((major, v))
        })
        .collect();

    // Sort by major version (ascending - oldest first)
    supported.sort_by_key(|(major, _)| *major);

    // Get the second oldest, or the oldest if there's only one
    let target_version = if supported.len() >= 2 {
        supported.get(1) // Second oldest
    } else {
        supported.first() // Only one version available
    };

    target_version.map(|(major, _)| format!("nodejs{}.x", major))
}

/// Fetch the Node.js release schedule from the official source.
/// This is a blocking HTTP request.
///
/// # Returns
/// The raw JSON string from the release schedule, or an error message
pub fn fetch_release_schedule() -> Result<String, String> {
    let response = reqwest::blocking::get(NODEJS_RELEASE_SCHEDULE_URL)
        .map_err(|e| format!("Failed to fetch release schedule: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch release schedule: HTTP {}",
            response.status()
        ));
    }

    response
        .text()
        .map_err(|e| format!("Failed to read response body: {}", e))
}

/// Tauri command to get supported Node.js runtimes.
/// Fetches the release schedule and returns parsed versions.
#[tauri::command]
pub async fn get_supported_runtimes() -> Result<Vec<NodeVersion>, String> {
    // Run blocking HTTP request in a separate thread
    let json_str = tokio::task::spawn_blocking(fetch_release_schedule)
        .await
        .map_err(|e| format!("Task failed: {}", e))??;

    parse_release_schedule(&json_str)
}

/// Tauri command to get the target runtime for updates.
/// Returns the oldest supported LTS version as a Lambda runtime string.
#[tauri::command]
pub fn get_target_runtime(versions: Vec<NodeVersion>) -> Result<String, String> {
    get_oldest_supported_lts(&versions).ok_or_else(|| "No supported LTS versions found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use proptest::prelude::*;

    // Unit tests for basic functionality
    #[test]
    fn test_is_lts_candidate_even() {
        assert!(is_lts_candidate(20));
        assert!(is_lts_candidate(18));
        assert!(is_lts_candidate(22));
        assert!(is_lts_candidate(0));
    }

    #[test]
    fn test_is_lts_candidate_odd() {
        assert!(!is_lts_candidate(19));
        assert!(!is_lts_candidate(21));
        assert!(!is_lts_candidate(17));
        assert!(!is_lts_candidate(1));
    }

    #[test]
    fn test_extract_major_version() {
        assert_eq!(extract_major_version("v20"), Some(20));
        assert_eq!(extract_major_version("20"), Some(20));
        assert_eq!(extract_major_version("v20.10.0"), Some(20));
        assert_eq!(extract_major_version("18.19.0"), Some(18));
        assert_eq!(extract_major_version("invalid"), None);
    }

    #[test]
    fn test_is_version_supported_at_date() {
        let ref_date = NaiveDate::from_ymd_opt(2025, 1, 5).unwrap();

        // Version that started and hasn't ended
        assert!(is_version_supported_at_date(
            "2024-01-01",
            "2026-01-01",
            ref_date
        ));

        // Version that hasn't started yet
        assert!(!is_version_supported_at_date(
            "2025-06-01",
            "2027-01-01",
            ref_date
        ));

        // Version that has already ended
        assert!(!is_version_supported_at_date(
            "2023-01-01",
            "2024-12-31",
            ref_date
        ));

        // Version that ends exactly on reference date (not supported - end is exclusive)
        assert!(!is_version_supported_at_date(
            "2024-01-01",
            "2025-01-05",
            ref_date
        ));

        // Version that starts exactly on reference date (supported - start is inclusive)
        assert!(is_version_supported_at_date(
            "2025-01-05",
            "2026-01-01",
            ref_date
        ));
    }

    #[test]
    fn test_parse_release_schedule() {
        let json = r#"{
            "v20": {
                "start": "2023-04-18",
                "lts": "2023-10-24",
                "maintenance": "2024-10-22",
                "end": "2026-04-30",
                "codename": "Iron"
            },
            "v21": {
                "start": "2023-10-17",
                "end": "2024-06-01"
            },
            "v22": {
                "start": "2024-04-24",
                "lts": "2024-10-29",
                "maintenance": "2025-10-21",
                "end": "2027-04-30",
                "codename": "Jod"
            }
        }"#;

        let versions = parse_release_schedule(json).unwrap();

        // Should only include even versions (LTS candidates)
        assert!(versions.iter().all(|v| {
            let major = extract_major_version(&v.version).unwrap();
            major % 2 == 0
        }));

        // v21 should not be included
        assert!(!versions.iter().any(|v| v.version == "v21"));

        // v20 and v22 should be included
        assert!(versions.iter().any(|v| v.version == "v20"));
        assert!(versions.iter().any(|v| v.version == "v22"));
    }

    #[test]
    fn test_get_oldest_supported_lts() {
        let versions = vec![
            NodeVersion {
                version: "v22".to_string(),
                lts: Some("Jod".to_string()),
                start: "2024-04-24".to_string(),
                end: "2027-04-30".to_string(),
                is_supported: true,
            },
            NodeVersion {
                version: "v20".to_string(),
                lts: Some("Iron".to_string()),
                start: "2023-04-18".to_string(),
                end: "2026-04-30".to_string(),
                is_supported: true,
            },
            NodeVersion {
                version: "v18".to_string(),
                lts: Some("Hydrogen".to_string()),
                start: "2022-04-19".to_string(),
                end: "2025-04-30".to_string(),
                is_supported: false, // Expired
            },
        ];

        // Should return the second oldest supported version (v22), not the oldest (v20)
        let target = get_oldest_supported_lts(&versions);
        assert_eq!(target, Some("nodejs22.x".to_string()));
    }

    #[test]
    fn test_get_oldest_supported_lts_single_version() {
        let versions = vec![
            NodeVersion {
                version: "v20".to_string(),
                lts: Some("Iron".to_string()),
                start: "2023-04-18".to_string(),
                end: "2026-04-30".to_string(),
                is_supported: true,
            },
            NodeVersion {
                version: "v18".to_string(),
                lts: Some("Hydrogen".to_string()),
                start: "2022-04-19".to_string(),
                end: "2025-04-30".to_string(),
                is_supported: false, // Expired
            },
        ];

        // With only one supported version, should return that version
        let target = get_oldest_supported_lts(&versions);
        assert_eq!(target, Some("nodejs20.x".to_string()));
    }

    #[test]
    fn test_get_oldest_supported_lts_empty() {
        let versions: Vec<NodeVersion> = vec![];
        assert_eq!(get_oldest_supported_lts(&versions), None);
    }

    #[test]
    fn test_get_oldest_supported_lts_none_supported() {
        let versions = vec![NodeVersion {
            version: "v18".to_string(),
            lts: Some("Hydrogen".to_string()),
            start: "2022-04-19".to_string(),
            end: "2024-04-30".to_string(),
            is_supported: false,
        }];

        assert_eq!(get_oldest_supported_lts(&versions), None);
    }

    // Property-based tests
    // **Feature: amplify-runtime-updater, Property 1: LTS Version Identification**
    // **Validates: Requirements 3.2**
    proptest! {
        /// Property 1: LTS Version Identification
        /// For any Node.js version number, the system shall correctly identify it
        /// as an LTS candidate if and only if the major version number is even.
        #[test]
        fn prop_lts_version_identification(major_version in 0u32..1000) {
            let is_lts = is_lts_candidate(major_version);
            let is_even = major_version % 2 == 0;

            // Property: is_lts_candidate returns true iff major version is even
            prop_assert_eq!(is_lts, is_even,
                "LTS identification failed for version {}: expected {}, got {}",
                major_version, is_even, is_lts);
        }

        /// Property test for version string parsing and LTS identification
        /// Ensures that version strings with even major versions are identified as LTS
        #[test]
        fn prop_version_string_lts_identification(major in 0u32..100, minor in 0u32..100, patch in 0u32..100) {
            let version_str = format!("v{}.{}.{}", major, minor, patch);
            let result = is_version_lts_candidate(&version_str);

            prop_assert!(result.is_some(), "Failed to parse version string: {}", version_str);
            prop_assert_eq!(result.unwrap(), major % 2 == 0,
                "LTS identification failed for version string {}", version_str);
        }

        /// Property test for version string without 'v' prefix
        #[test]
        fn prop_version_string_no_prefix_lts_identification(major in 0u32..100, minor in 0u32..100, patch in 0u32..100) {
            let version_str = format!("{}.{}.{}", major, minor, patch);
            let result = is_version_lts_candidate(&version_str);

            prop_assert!(result.is_some(), "Failed to parse version string: {}", version_str);
            prop_assert_eq!(result.unwrap(), major % 2 == 0,
                "LTS identification failed for version string {}", version_str);
        }

        /// **Feature: amplify-runtime-updater, Property 2: Runtime Support Status Calculation**
        /// **Validates: Requirements 3.3**
        /// For any Node.js version with start and end dates, and any reference date,
        /// the system shall correctly determine the version is supported if and only if
        /// the reference date is after the start date and before the end date.
        #[test]
        fn prop_runtime_support_status_calculation(
            start_year in 2020u32..2030,
            start_month in 1u32..13,
            start_day in 1u32..29,
            end_year in 2020u32..2030,
            end_month in 1u32..13,
            end_day in 1u32..29,
            ref_year in 2020u32..2030,
            ref_month in 1u32..13,
            ref_day in 1u32..29
        ) {
            let start_date = format!("{:04}-{:02}-{:02}", start_year, start_month, start_day);
            let end_date = format!("{:04}-{:02}-{:02}", end_year, end_month, end_day);
            let reference_date = NaiveDate::from_ymd_opt(ref_year as i32, ref_month, ref_day).unwrap();

            let start = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d").unwrap();
            let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d").unwrap();

            let is_supported = is_version_supported_at_date(&start_date, &end_date, reference_date);

            // Property: version is supported iff reference_date >= start AND reference_date < end
            let expected = reference_date >= start && reference_date < end;

            prop_assert_eq!(is_supported, expected,
                "Support status calculation failed for start={}, end={}, ref={}: expected {}, got {}",
                start_date, end_date, reference_date, expected, is_supported);
        }

        /// **Feature: amplify-runtime-updater, Property 3: Second Oldest Supported LTS Selection**
        /// **Validates: Requirements 3.4**
        /// For any non-empty list of supported Node.js versions, the system shall select
        /// the version with the second smallest major version number as the target runtime.
        /// If only one version is supported, it shall select that version.
        #[test]
        fn prop_oldest_supported_lts_selection(
            versions in prop::collection::vec(
                (10u32..30, prop::bool::ANY),
                1..10
            )
        ) {
            // Create NodeVersion structs from the generated data
            let node_versions: Vec<NodeVersion> = versions
                .iter()
                .filter(|(major, _)| major % 2 == 0) // Only even versions (LTS)
                .map(|(major, is_supported)| NodeVersion {
                    version: format!("v{}", major),
                    lts: Some(format!("LTS{}", major)),
                    start: "2023-01-01".to_string(),
                    end: "2027-01-01".to_string(),
                    is_supported: *is_supported,
                })
                .collect();

            let result = get_oldest_supported_lts(&node_versions);

            // Find the expected second oldest supported version (or oldest if only one)
            let mut supported_majors: Vec<u32> = node_versions
                .iter()
                .filter(|v| v.is_supported)
                .filter_map(|v| extract_major_version(&v.version))
                .collect();

            supported_majors.sort();

            let expected_major = if supported_majors.len() >= 2 {
                supported_majors.get(1).copied() // Second oldest
            } else {
                supported_majors.first().copied() // Only one version
            };

            match (result, expected_major) {
                (Some(runtime), Some(expected)) => {
                    let expected_runtime = format!("nodejs{}.x", expected);
                    prop_assert_eq!(&runtime, &expected_runtime,
                        "Second oldest LTS selection failed: expected {}, got {}",
                        expected_runtime, runtime);
                }
                (None, None) => {
                    // Both are None - correct behavior when no supported versions
                }
                (Some(runtime), None) => {
                    prop_assert!(false, "Got runtime {} but expected None", runtime);
                }
                (None, Some(expected)) => {
                    prop_assert!(false, "Got None but expected nodejs{}.x", expected);
                }
            }
        }
    }
}
