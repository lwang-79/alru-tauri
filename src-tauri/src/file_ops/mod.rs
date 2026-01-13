pub mod detector;
pub mod installer;
pub mod modifier;

pub use detector::*;
pub use installer::*;
pub use modifier::*;

use serde::{Deserialize, Serialize};

/// A file change made during update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: String,
    pub old_value: String,
    pub new_value: String,
}

/// Result of updating backend files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub changes: Vec<FileChange>,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of running a build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Build configuration location (File or Cloud)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BuildConfigLocation {
    File(String), // Path to amplify.yml
    Cloud,        // AWS cloud configuration
}

/// Build configuration change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfigChange {
    pub location: BuildConfigLocation,
    pub old_command: String,
    pub new_command: String,
}

/// Result of build config update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfigUpdateResult {
    pub success: bool,
    pub updated: bool,
    pub change: Option<BuildConfigChange>,
    pub message: String,
    pub error: Option<String>,
    pub original_build_spec: Option<String>,
}
