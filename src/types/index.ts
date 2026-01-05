// TypeScript type definitions matching Rust types for IPC compatibility

// Prerequisites types
export interface ToolStatus {
  installed: boolean;
  version: string | null;
  error: string | null;
}

export interface PrerequisitesResult {
  network: ToolStatus;
  aws_cli: ToolStatus;
  git: ToolStatus;
  nodejs: ToolStatus;
  // Optional tools
  amplify_cli: ToolStatus;
  npm: ToolStatus;
  yarn: ToolStatus;
  pnpm: ToolStatus;
  bun: ToolStatus;
}

// AWS CLI types
export interface AwsProfile {
  name: string;
}

export interface AmplifyApp {
  app_id: string;
  name: string;
  repository: string;
  environment_variables: Record<string, string>;
}

export interface AmplifyBranch {
  branch_name: string;
  stack_arn: string;
  backend_environment_name: string;
  environment_variables: Record<string, string>;
  is_protected: boolean;
  protection_info: string | null;
}

export interface LambdaFunction {
  arn: string;
  name: string;
  friendly_name: string;
  runtime: string;
  description: string | null;
  is_outdated: boolean;
  is_auto_managed: boolean;
}

// Runtime types
export interface NodeVersion {
  version: string;
  lts: string | null;
  start: string;
  end: string;
  is_supported: boolean;
}

// File operations types
export type BackendType = "Gen1" | "Gen2";

export type PackageManager = "Npm" | "Yarn" | "Pnpm" | "Bun";

export interface FileChange {
  path: string;
  change_type: string;
  old_value: string;
  new_value: string;
}

export interface UpdateResult {
  changes: FileChange[];
  success: boolean;
  error: string | null;
}

export interface BuildResult {
  success: boolean;
  output: string;
  error: string | null;
}

// Upgrade result for Amplify CLI
export interface UpgradeResult {
  success: boolean;
  skipped: boolean;
  current_version: string | null;
  latest_version: string | null;
  message: string;
}

// Sandbox deployment result
export interface SandboxResult {
  success: boolean;
  output: string;
  error: string | null;
  status: "running" | "completed" | "timeout" | "failed";
}

// Git operations types
export interface CloneResult {
  path: string;
  success: boolean;
  error: string | null;
}

export interface CommitPushResult {
  success: boolean;
  commit_hash: string | null;
  error: string | null;
}

// Build status for UI
export type BuildStatus = "pending" | "running" | "success" | "failed";

// Update environment variable result
export interface EnvVarChange {
  level: string; // "app" or "branch"
  key: string;
  old_value: string;
  new_value: string;
}

export interface UpdateEnvVarResult {
  success: boolean;
  updated: boolean;
  message: string;
  changes: EnvVarChange[];
}

// Build configuration update types
export interface BuildConfigChange {
  location: "File" | "Cloud";
  old_command: string;
  new_command: string;
}

export interface BuildConfigUpdateResult {
  success: boolean;
  updated: boolean;
  change: BuildConfigChange | null;
  message: string;
  error: string | null;
  original_build_spec: string | null;
}

// Amplify job types
export interface AmplifyJob {
  job_id: string;
  commit_id: string;
  status: string;
  start_time: string | null;
}

export interface AmplifyJobDetails {
  job_id: string;
  commit_id: string;
  status: string;
  start_time: string | null;
  end_time: string | null;
}
