use super::core::execute_aws_command;
use serde::{Deserialize, Serialize};

/// Amplify job summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyJob {
    pub job_id: String,
    pub commit_id: String,
    pub status: String,
    pub start_time: Option<String>,
}

/// Amplify job details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifyJobDetails {
    pub job_id: String,
    pub commit_id: String,
    pub status: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// Response structure for AWS Amplify list-jobs command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplifyListJobsResponse {
    job_summaries: Vec<AmplifyJobSummary>,
}

/// Job summary from list-jobs response
#[derive(Debug, Deserialize)]
struct AmplifyJobSummary {
    #[serde(alias = "job_id", alias = "jobId", alias = "JobId")]
    job_id: String,
    #[serde(alias = "commit_id", alias = "commitId", alias = "CommitId")]
    commit_id: String,
    #[serde(alias = "Status")]
    status: String,
    #[serde(
        default,
        alias = "start_time",
        alias = "startTime",
        alias = "StartTime"
    )]
    start_time: Option<String>,
}

/// Response structure for AWS Amplify get-job command
#[derive(Debug, Deserialize)]
struct AmplifyGetJobResponse {
    job: AmplifyJobWrapper,
}

/// Job wrapper from get-job response
#[derive(Debug, Deserialize)]
struct AmplifyJobWrapper {
    summary: AmplifyJobDetail,
}

/// Job detail from get-job response
#[derive(Debug, Deserialize)]
struct AmplifyJobDetail {
    #[serde(alias = "job_id", alias = "jobId", alias = "JobId")]
    job_id: String,
    #[serde(alias = "commit_id", alias = "commitId", alias = "CommitId")]
    commit_id: String,
    #[serde(alias = "Status")]
    status: String,
    #[serde(
        default,
        alias = "start_time",
        alias = "startTime",
        alias = "StartTime"
    )]
    start_time: Option<String>,
    #[serde(default, alias = "end_time", alias = "endTime", alias = "EndTime")]
    end_time: Option<String>,
}

/// Tauri command to list Amplify jobs for a specific commit
#[tauri::command]
pub async fn list_amplify_jobs(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    commit_id: &str,
) -> Result<Vec<AmplifyJob>, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "list-jobs",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--max-results",
            "10",
        ],
        profile,
        region,
    )?;

    let response: AmplifyListJobsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify jobs response: {}", e))?;

    // Filter jobs by commit ID
    let jobs: Vec<AmplifyJob> = response
        .job_summaries
        .into_iter()
        .filter(|job| job.commit_id == commit_id)
        .map(|job| AmplifyJob {
            job_id: job.job_id,
            commit_id: job.commit_id,
            status: job.status,
            start_time: job.start_time,
        })
        .collect();

    Ok(jobs)
}

/// Tauri command to get details of a specific Amplify job
#[tauri::command]
pub async fn get_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    job_id: &str,
) -> Result<AmplifyJobDetails, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "get-job",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--job-id",
            job_id,
        ],
        profile,
        region,
    )?;

    let response: AmplifyGetJobResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse Amplify job response: {}", e))?;

    Ok(AmplifyJobDetails {
        job_id: response.job.summary.job_id,
        commit_id: response.job.summary.commit_id,
        status: response.job.summary.status,
        start_time: response.job.summary.start_time,
        end_time: response.job.summary.end_time,
    })
}

/// Tauri command to start/retry an Amplify job
#[tauri::command]
pub async fn start_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
    job_type: &str,
    job_id: Option<&str>,
) -> Result<String, String> {
    let mut args = vec![
        "amplify",
        "start-job",
        "--app-id",
        app_id,
        "--branch-name",
        branch_name,
        "--job-type",
        job_type,
    ];

    // Add job-id if provided (for RETRY)
    let job_id_string;
    if let Some(id) = job_id {
        job_id_string = id.to_string();
        args.push("--job-id");
        args.push(&job_id_string);
    }

    let output = execute_aws_command(&args, profile, region)?;

    let response: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse start-job response: {}", e))?;

    let new_job_id = response["jobSummary"]["jobId"]
        .as_str()
        .ok_or_else(|| "Failed to get job ID from response".to_string())?
        .to_string();

    Ok(new_job_id)
}

/// Tauri command to get the most recent job for a branch
#[tauri::command]
pub async fn get_latest_amplify_job(
    profile: &str,
    region: &str,
    app_id: &str,
    branch_name: &str,
) -> Result<Option<AmplifyJobDetails>, String> {
    let output = execute_aws_command(
        &[
            "amplify",
            "list-jobs",
            "--app-id",
            app_id,
            "--branch-name",
            branch_name,
            "--max-results",
            "1",
        ],
        profile,
        region,
    )?;

    let response: AmplifyListJobsResponse = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse list-jobs response: {}", e))?;

    if response.job_summaries.is_empty() {
        return Ok(None);
    }

    let latest_job = &response.job_summaries[0];

    // Return job details directly from the list-jobs response
    // We have all the info we need, no need to call get-job which has parsing issues
    let job_details = AmplifyJobDetails {
        job_id: latest_job.job_id.clone(),
        commit_id: latest_job.commit_id.clone(),
        status: latest_job.status.clone(),
        start_time: latest_job.start_time.clone(),
        end_time: None, // list-jobs doesn't include end_time, but we don't need it for retry
    };

    Ok(Some(job_details))
}
