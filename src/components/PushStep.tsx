import {
  createSignal,
  Show,
  For,
  onCleanup,
  createEffect,
  onMount,
} from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { CommitPushResult, AmplifyJob, AmplifyJobDetails } from "../types";
import {
  appState,
  setAppState,
  checkAndResetPushStepIfNeeded,
  updatePushStepContext,
} from "../store/appStore";
import { CleanupDialog } from "./CleanupDialog";
import "./shared.css";
import "./PushStep.css";
import { AmplifyJobStatus } from "./common/AmplifyJobStatus";
import { EnvVarChangesList } from "./common/EnvVarChangesList";

interface PushStepProps {
  onComplete?: () => void;
  onBack?: () => void;
}

export function PushStep(props: PushStepProps) {
  // Use store state instead of local signals for persistence
  const pushStatus = () => appState.pushStep.status;
  const setPushStatus = (status: typeof appState.pushStep.status) =>
    setAppState("pushStep", "status", status);

  const pushError = () => appState.pushStep.error;
  const setPushError = (error: string | null) =>
    setAppState("pushStep", "error", error);

  const commitHash = () => appState.pushStep.commitHash;
  const setCommitHash = (hash: string | null) =>
    setAppState("pushStep", "commitHash", hash);

  const amplifyJob = () => appState.pushStep.amplifyJob;
  const setAmplifyJob = (job: AmplifyJobDetails | null) =>
    setAppState("pushStep", "amplifyJob", job);

  const jobCheckError = () => appState.pushStep.jobCheckError;
  const setJobCheckError = (error: string | null) =>
    setAppState("pushStep", "jobCheckError", error);

  // Add state to track when we're checking for jobs with retries
  const [checkingForJob, setCheckingForJob] = createSignal(false);

  const lastFailedJob = () => appState.pushStep.lastFailedJob;
  const setLastFailedJob = (job: AmplifyJobDetails | null) =>
    setAppState("pushStep", "lastFailedJob", job);

  const retryingJob = () => appState.pushStep.retryingJob;
  const setRetryingJob = (retrying: boolean) =>
    setAppState("pushStep", "retryingJob", retrying);

  // Local signals for dialogs and temporary state (these don't need persistence)
  const [showCleanupDialog, setShowCleanupDialog] = createSignal(false);
  let jobCheckInterval: number | null = null;

  // Environment variable revert functionality (local state)
  const [showRevertDialog, setShowRevertDialog] = createSignal(false);
  const [revertInProgress, setRevertInProgress] = createSignal(false);

  // Build spec revert functionality (local state)
  const [showBuildSpecRevertDialog, setShowBuildSpecRevertDialog] =
    createSignal(false);
  const [revertBuildSpecInProgress, setRevertBuildSpecInProgress] =
    createSignal(false);

  // Check if state should be reset on mount and when navigating to this step
  onMount(() => {
    checkAndResetPushStepIfNeeded();
    updatePushStepContext();
  });

  // Get environment variable changes from app state
  const getEnvVarChanges = () => appState.repository.envVarChanges;

  // Show confirmation dialog
  const handleInitiatePush = () => {
    setPushStatus("confirming");
  };

  // Cancel push
  const handleCancelPush = () => {
    setPushStatus("pending");
  };

  // Confirm and execute push
  const handleConfirmPush = async () => {
    const clonePath = appState.repository.clonePath;

    if (!clonePath) {
      setPushError("No repository path found");
      setPushStatus("failed");
      return;
    }

    setPushStatus("running");
    setPushError(null);
    setCommitHash(null);

    try {
      // Create commit message based on changes
      const targetRuntime = appState.runtimeInfo.targetRuntime;
      const backendType = appState.repository.backendType;
      const commitMessage = `chore: Update Lambda runtime to ${targetRuntime}

- Updated ${backendType} backend runtime configurations
- Upgraded Amplify packages to latest versions
${appState.repository.changes.length > 0 ? `- Modified ${appState.repository.changes.length} file(s)` : ""}

This update ensures Lambda functions use supported Node.js runtimes.`;

      const result = await invoke<CommitPushResult>("commit_and_push", {
        path: clonePath,
        message: commitMessage,
      });

      if (result.success) {
        setCommitHash(result.commit_hash);
        setPushStatus("success");

        // Check for Amplify job (only if there's a commit hash)
        const hash = result.commit_hash;
        if (hash) {
          checkForAmplifyJob(hash);
        } else {
          // No commit hash means no changes were committed
          setJobCheckError(
            "No changes were committed, so no deployment job was triggered",
          );

          // Check if the last job failed - if so, we should retry it
          checkLastJobStatus();
        }
      } else {
        setPushError(result.error || "Push failed");
        setPushStatus("failed");
      }
    } catch (e) {
      setPushError(String(e));
      setPushStatus("failed");
    }
  };

  // Check the last job status to see if we should offer a retry
  const checkLastJobStatus = async () => {
    console.log("[checkLastJobStatus] Starting to check last job status");
    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    console.log(
      "[checkLastJobStatus] App:",
      selectedApp?.name,
      "Branch:",
      selectedBranch?.branch_name,
    );

    if (!selectedApp || !selectedBranch || !profile || !region) {
      console.log("[checkLastJobStatus] Missing required info, aborting");
      return;
    }

    try {
      console.log("[checkLastJobStatus] Calling get_latest_amplify_job...");
      const lastJob = await invoke<AmplifyJobDetails | null>(
        "get_latest_amplify_job",
        {
          profile,
          region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
        },
      );

      console.log("[checkLastJobStatus] Last job result:", lastJob);

      if (lastJob) {
        console.log("[checkLastJobStatus] Last job status:", lastJob.status);
        // Set lastFailedJob for both FAILED and SUCCEED status
        // FAILED: deployment failed, need to retry
        // SUCCEED: functions might have been updated outside Amplify, need to redeploy
        if (lastJob.status === "FAILED" || lastJob.status === "SUCCEED") {
          console.log("[checkLastJobStatus] Setting lastFailedJob:", lastJob);
          setLastFailedJob(lastJob);
        } else {
          console.log(
            "[checkLastJobStatus] Last job status is not FAILED or SUCCEED, it's:",
            lastJob.status,
          );
        }
      } else {
        console.log("[checkLastJobStatus] No last job found");
      }
    } catch (e) {
      console.error("[checkLastJobStatus] Failed to check last job status:", e);
    }
  };

  // Retry the last failed job
  const handleRetryJob = async () => {
    const lastJob = lastFailedJob();
    if (!lastJob) return;

    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!selectedApp || !selectedBranch || !profile || !region) {
      return;
    }

    setRetryingJob(true);

    try {
      const newJobId = await invoke<string>("start_amplify_job", {
        profile,
        region,
        appId: selectedApp.app_id,
        branchName: selectedBranch.branch_name,
        jobType: "RETRY",
        jobId: lastJob.job_id,
      });

      // Get the new job details
      const jobDetails = await invoke<AmplifyJobDetails>("get_amplify_job", {
        profile,
        region,
        appId: selectedApp.app_id,
        branchName: selectedBranch.branch_name,
        jobId: newJobId,
      });

      setAmplifyJob(jobDetails);
      setJobCheckError(null);
      setLastFailedJob(null);

      // Start polling for job status updates
      startJobStatusPolling(newJobId);
    } catch (e) {
      console.error("Failed to retry job:", e);
      setJobCheckError(`Failed to retry job: ${String(e)}`);
    } finally {
      setRetryingJob(false);
    }
  };

  // Check for Amplify job after push with retry logic
  const checkForAmplifyJob = async (commitId: string) => {
    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!selectedApp || !selectedBranch || !profile || !region) {
      return;
    }

    setCheckingForJob(true);
    setJobCheckError(null);

    const maxRetries = 3;
    const retryDelays = [5000, 10000, 15000]; // 5s, 10s, 15s for attempts 1, 2, 3

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        // Wait before each attempt (including the first one)
        const currentDelay = retryDelays[attempt - 1];
        console.log(
          `[checkForAmplifyJob] Waiting ${currentDelay}ms before attempt ${attempt}/${maxRetries}...`,
        );

        // Update UI to show waiting status
        setJobCheckError(
          `Waiting ${currentDelay / 1000}s before checking for deployment job... (attempt ${attempt}/${maxRetries})`,
        );

        await new Promise((resolve) => setTimeout(resolve, currentDelay));

        console.log(
          `[checkForAmplifyJob] Attempt ${attempt}/${maxRetries}: Looking for job with commit ${commitId}...`,
        );

        // Update UI to show we're now checking
        setJobCheckError(
          `Looking for deployment job... (attempt ${attempt}/${maxRetries})`,
        );

        const jobs = await invoke<AmplifyJob[]>("list_amplify_jobs", {
          profile,
          region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
          commitId,
        });

        if (jobs.length > 0) {
          // Found a job, get its details
          const job = jobs[0];
          console.log(
            `[checkForAmplifyJob] Found job on attempt ${attempt}:`,
            job.job_id,
          );
          const jobDetails = await invoke<AmplifyJobDetails>(
            "get_amplify_job",
            {
              profile,
              region,
              appId: selectedApp.app_id,
              branchName: selectedBranch.branch_name,
              jobId: job.job_id,
            },
          );

          setAmplifyJob(jobDetails);
          setJobCheckError(null);
          setCheckingForJob(false);

          // Start polling for job status updates every 10 seconds
          startJobStatusPolling(job.job_id);
          return; // Job found, exit retry loop
        } else {
          console.log(
            `[checkForAmplifyJob] No job found on attempt ${attempt}/${maxRetries}`,
          );

          // If this is the last attempt, show final error message
          if (attempt === maxRetries) {
            console.log("[checkForAmplifyJob] No job found after all retries");
            setJobCheckError(
              "No Amplify job found for this commit after multiple attempts. The job may take longer to appear.",
            );
          }
        }
      } catch (e) {
        console.error(`[checkForAmplifyJob] Attempt ${attempt} failed:`, e);

        // If this is the last attempt, show error
        if (attempt === maxRetries) {
          console.error("[checkForAmplifyJob] All retry attempts failed:", e);
          setJobCheckError(String(e));
        }
      }
    }

    setCheckingForJob(false);
  };

  // Poll job status every 10 seconds
  const startJobStatusPolling = (jobId: string) => {
    // Clear any existing interval
    if (jobCheckInterval !== null) {
      clearInterval(jobCheckInterval);
    }

    jobCheckInterval = window.setInterval(async () => {
      const selectedApp = appState.amplifyResources.selectedApp;
      const selectedBranch = appState.amplifyResources.selectedBranch;
      const profile = appState.awsConfig.selectedProfile;
      const region = appState.awsConfig.selectedRegion;

      if (!selectedApp || !selectedBranch || !profile || !region) {
        return;
      }

      try {
        const jobDetails = await invoke<AmplifyJobDetails>("get_amplify_job", {
          profile,
          region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
          jobId,
        });

        setAmplifyJob(jobDetails);

        // Stop polling if job is in a terminal state
        if (["SUCCEED", "FAILED", "CANCELLED"].includes(jobDetails.status)) {
          if (jobCheckInterval !== null) {
            clearInterval(jobCheckInterval);
            jobCheckInterval = null;
          }
        }
      } catch (e) {
        console.error("Failed to update job status:", e);
      }
    }, 10000); // 10 seconds
  };

  // Cleanup interval on component unmount
  onCleanup(() => {
    if (jobCheckInterval !== null) {
      clearInterval(jobCheckInterval);
    }
  });

  // Debug: Log when lastFailedJob changes
  createEffect(() => {
    const job = lastFailedJob();
    console.log("[createEffect] lastFailedJob changed:", job);
  });

  // Revert environment variables (excluding _LIVE_UPDATES and _CUSTOM_IMAGE)
  const handleRevertEnvVars = async () => {
    const changes = getEnvVarChanges();
    const revertableChanges = changes.filter(
      (change) =>
        change.key !== "_LIVE_UPDATES" && change.key !== "_CUSTOM_IMAGE",
    );

    if (revertableChanges.length === 0) {
      return;
    }

    setRevertInProgress(true);

    try {
      const selectedApp = appState.amplifyResources.selectedApp;
      const selectedBranch = appState.amplifyResources.selectedBranch;
      const profile = appState.awsConfig.selectedProfile;
      const region = appState.awsConfig.selectedRegion;

      if (!selectedApp || !selectedBranch || !profile || !region) {
        throw new Error(
          "Missing required information for environment variable revert",
        );
      }

      // Group changes by level (app vs branch)
      const appChanges = revertableChanges.filter((c) => c.level === "app");
      const branchChanges = revertableChanges.filter(
        (c) => c.level === "branch",
      );

      // Revert app-level changes
      if (appChanges.length > 0) {
        // Step 1: Get CURRENT environment variables from AWS (not from stale app state)
        const currentAppEnvVars = await invoke<Record<string, string>>(
          "get_current_app_env_vars",
          {
            profile,
            region,
            appId: selectedApp.app_id,
          },
        );

        // Step 2: Replace only the revertable variables with their old values
        // Keep everything else unchanged (including _LIVE_UPDATES and _CUSTOM_IMAGE)
        for (const change of appChanges) {
          currentAppEnvVars[change.key] = change.old_value;
        }

        // Step 3: Update AWS with the modified environment variables
        await invoke("update_app_env_vars", {
          profile,
          region,
          appId: selectedApp.app_id,
          envVars: currentAppEnvVars,
        });
      }

      // Revert branch-level changes
      if (branchChanges.length > 0) {
        // Step 1: Get CURRENT environment variables from AWS (not from stale app state)
        const currentBranchEnvVars = await invoke<Record<string, string>>(
          "get_current_branch_env_vars",
          {
            profile,
            region,
            appId: selectedApp.app_id,
            branchName: selectedBranch.branch_name,
          },
        );

        // Step 2: Replace only the revertable variables with their old values
        // Keep everything else unchanged (including _LIVE_UPDATES and _CUSTOM_IMAGE)
        for (const change of branchChanges) {
          currentBranchEnvVars[change.key] = change.old_value;
        }

        // Step 3: Update AWS with the modified environment variables
        await invoke("update_branch_env_vars", {
          profile,
          region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
          envVars: currentBranchEnvVars,
        });
      }

      // Clear only the reverted changes from state, keep non-revertible ones
      const remainingChanges = getEnvVarChanges().filter(
        (change) =>
          change.key === "_LIVE_UPDATES" || change.key === "_CUSTOM_IMAGE",
      );
      setAppState("repository", "envVarChanges", remainingChanges);
      setShowRevertDialog(false);
    } catch (e) {
      console.error("Failed to revert environment variables:", e);
      // Could add error state here if needed
    } finally {
      setRevertInProgress(false);
    }
  };

  const handleCancelRevert = () => {
    setShowRevertDialog(false);
  };

  // Revert build spec to original
  const handleRevertBuildSpec = async () => {
    const originalBuildSpec = appState.repository.originalBuildSpec;

    if (!originalBuildSpec) {
      console.error("No original build spec found");
      return;
    }

    setRevertBuildSpecInProgress(true);

    try {
      const selectedApp = appState.amplifyResources.selectedApp;
      const profile = appState.awsConfig.selectedProfile;
      const region = appState.awsConfig.selectedRegion;

      if (!selectedApp || !profile || !region) {
        throw new Error("Missing required information for build spec revert");
      }

      // Call the revert_build_spec Tauri command
      await invoke<boolean>("revert_build_spec", {
        profile,
        region,
        appId: selectedApp.app_id,
        originalBuildSpec,
      });

      // Clear build config change and original build spec from state
      setAppState("repository", "buildConfigChange", null);
      setAppState("repository", "originalBuildSpec", null);
      setShowBuildSpecRevertDialog(false);
    } catch (e) {
      console.error("Failed to revert build spec:", e);
      // Could add error state here if needed
    } finally {
      setRevertBuildSpecInProgress(false);
    }
  };

  const handleCancelBuildSpecRevert = () => {
    setShowBuildSpecRevertDialog(false);
  };


  const handleBack = () => {
    if (props.onBack) {
      props.onBack();
    }
  };

  const handleFinish = () => {
    // Show cleanup dialog if repository exists
    if (appState.repository.clonePath) {
      setShowCleanupDialog(true);
    } else if (props.onComplete) {
      props.onComplete();
    }
  };

  const handleCleanupClose = () => {
    setShowCleanupDialog(false);
    // Don't call props.onComplete() after cleanup since cleanup resets the state
    // The CleanupDialog already handles navigation to the appropriate step
  };

  return (
    <div class="step-container wide push-step">
      <h2>Push Changes</h2>
      <p class="step-description">
        Review and push your changes to trigger the Amplify deployment.
      </p>

      {/* Summary Section */}
      <div class="summary-balanced">
        <h3>Changes Summary</h3>
        <div class="summary-row-balanced">
          <div class="summary-pair-balanced half-width">
            <span class="summary-pair-label-balanced">App:</span>
            <span class="summary-pair-value-balanced">
              {appState.amplifyResources.selectedApp?.name}
            </span>
          </div>
          <div class="summary-pair-balanced half-width">
            <span class="summary-pair-label-balanced">Branch:</span>
            <span class="summary-pair-value-balanced">
              {appState.amplifyResources.selectedBranch?.branch_name}
            </span>
          </div>
        </div>
        <div class="summary-row-balanced">
          <div class="summary-pair-balanced half-width">
            <span class="summary-pair-label-balanced">Target Runtime:</span>
            <span class="badge-balanced runtime">
              {appState.runtimeInfo.targetRuntime}
            </span>
          </div>
          <div class="summary-pair-balanced half-width">
            <span class="summary-pair-label-balanced">Backend Type:</span>
            <span class="badge-balanced type">
              {appState.repository.backendType === "Gen2" ? "Gen 2" : "Gen 1"}
            </span>
          </div>
        </div>
        <div class="summary-row-balanced">
          <div class="summary-pair-balanced full-width">
            <span class="summary-pair-label-balanced">Files Modified:</span>
            <span class="summary-pair-value-balanced">
              {appState.repository.changes.length > 0
                ? `${appState.repository.changes.length} file(s)`
                : "No manual changes (updated via package upgrade)"}
            </span>
          </div>
        </div>
      </div>

      {/* Push Action Section */}
      <div class="push-action-section">
        <Show when={pushStatus() === "pending"}>
          <div class="push-ready">
            <div class="ready-icon">✓</div>
            <h3>Ready to Push</h3>
            <p>
              Your changes are ready to be committed and pushed to the remote
              repository. This will trigger an Amplify deployment with the
              updated runtime configuration.
            </p>
            <button class="push-button" onClick={handleInitiatePush}>
              Push Changes
            </button>
          </div>
        </Show>

        <Show when={pushStatus() === "confirming"}>
          <div class="confirmation-dialog">
            <div class="confirmation-icon">⚠️</div>
            <h3>Confirm Push</h3>
            <p>
              Are you sure you want to push these changes? This will commit and
              push to the{" "}
              <strong>
                {appState.amplifyResources.selectedBranch?.branch_name}
              </strong>{" "}
              branch and trigger an Amplify deployment.
            </p>
            <div class="confirmation-actions">
              <button class="cancel-button" onClick={handleCancelPush}>
                Cancel
              </button>
              <button class="confirm-button" onClick={handleConfirmPush}>
                Confirm & Push
              </button>
            </div>
          </div>
        </Show>

        <Show when={pushStatus() === "running"}>
          <div class="push-progress">
            <span class="spinner"></span>
            <h3>Pushing Changes...</h3>
            <p>Committing and pushing to remote repository</p>
          </div>
        </Show>

        <Show when={pushStatus() === "success"}>
          <div
            class={`push-result ${commitHash() ? "push-success" : "push-warning"}`}
          >
            <Show when={commitHash()}>
              <div class="success-icon">✓</div>
              <h3>Successfully Pushed!</h3>
            </Show>
            <Show when={!commitHash()}>
              <div class="warning-icon">⚠️</div>
              <h3>Push Skipped</h3>
            </Show>
            <Show when={commitHash()}>
              <div class="commit-info">
                <span class="commit-label">Commit:</span>
                <code class="commit-hash">{commitHash()}</code>
              </div>
            </Show>

            {/* Amplify Job Status */}
            <Show when={amplifyJob()}>
              <AmplifyJobStatus job={amplifyJob()!} />
            </Show>

            <Show when={jobCheckError() && !commitHash()}>
              <div class="job-check-info">
                <p class="info-message">
                  <Show when={checkingForJob()}>
                    <span class="spinner-small"></span>
                  </Show>
                  {jobCheckError()}
                </p>
              </div>
            </Show>

            <Show when={jobCheckError() && commitHash()}>
              <div class="job-check-error">
                <p class="error-hint">
                  <Show when={checkingForJob()}>
                    <span class="spinner-small"></span>
                  </Show>
                  {jobCheckError()}
                </p>
              </div>
            </Show>

            {/* Environment Variable Changes */}
            <EnvVarChangesList changes={getEnvVarChanges()}>
              <Show
                when={
                  getEnvVarChanges().filter(
                    (c) =>
                      c.key !== "_LIVE_UPDATES" && c.key !== "_CUSTOM_IMAGE",
                  ).length > 0
                }
              >
                <button
                  class="revert-env-vars-button"
                  onClick={() => setShowRevertDialog(true)}
                >
                  Revert Environment Variables
                </button>
                <p class="revert-note">
                  Note: _LIVE_UPDATES and _CUSTOM_IMAGE changes cannot be
                  reverted
                </p>
              </Show>
            </EnvVarChangesList>

            {/* Build Config Changes */}
            <Show
              when={
                appState.repository.buildConfigChange &&
                appState.repository.buildConfigChange.location === "Cloud"
              }
            >
              <div class="build-config-change-section">
                <h4>Build Configuration Changes</h4>
                <div class="build-config-change-item">
                  <div class="build-config-change-header">
                    <span class="build-config-label">Location:</span>
                    <span class="build-config-value">
                      Cloud (AWS Amplify buildSpec)
                    </span>
                  </div>
                  <div class="build-config-change-values">
                    <div class="build-config-old">
                      <span class="build-config-label">Old Command:</span>
                      <code class="build-config-command">
                        {appState.repository.buildConfigChange?.old_command}
                      </code>
                    </div>
                    <span class="build-config-arrow">→</span>
                    <div class="build-config-new">
                      <span class="build-config-label">New Command:</span>
                      <code class="build-config-command">
                        {appState.repository.buildConfigChange?.new_command}
                      </code>
                    </div>
                  </div>
                </div>
                <button
                  class="revert-build-spec-button"
                  onClick={() => setShowBuildSpecRevertDialog(true)}
                >
                  Revert Build Configuration
                </button>
                <p class="revert-note">
                  This will restore the original buildSpec in AWS Amplify
                </p>
              </div>
            </Show>

            <Show when={commitHash()}>
              <div class="success-message">
                <p>
                  <strong>Next Steps:</strong>
                </p>
                <ul>
                  <li>
                    Monitor the deployment in the{" "}
                    <a
                      href={`https://${appState.awsConfig.selectedRegion}.console.aws.amazon.com/amplify/apps/${appState.amplifyResources.selectedApp?.app_id}/branches/${appState.amplifyResources.selectedBranch?.branch_name}/deployments`}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      AWS Amplify Console
                    </a>
                  </li>
                  <li>
                    Verify Lambda functions are using the new runtime after
                    deployment completes
                  </li>
                </ul>
              </div>
            </Show>

            <Show when={!commitHash()}>
              <div class="success-message">
                {/* Show "What happened" only if no job is being tracked */}
                <Show when={!amplifyJob()}>
                  <p>
                    <strong>What happened:</strong>
                  </p>
                  <ul>
                    <li>All runtime configurations were already up to date</li>
                    <li>Environment variables were updated as needed</li>
                    <li>
                      No file changes were required, so no commit was created
                    </li>
                  </ul>
                  <Show when={lastFailedJob()}>
                    <Show when={lastFailedJob()?.status === "FAILED"}>
                      <div class="failed-job-notice">
                        <p class="failed-job-text">
                          <strong>Note:</strong> The last deployment job (ID:{" "}
                          {lastFailedJob()?.job_id}) failed. This might be why
                          your Lambda functions haven't been updated yet.
                        </p>
                        <div class="failed-job-actions">
                          <button
                            class="retry-job-button"
                            onClick={handleRetryJob}
                            disabled={retryingJob()}
                          >
                            {retryingJob()
                              ? "Retrying Job..."
                              : "Retry Failed Deployment"}
                          </button>
                          <a
                            href={`https://${appState.awsConfig.selectedRegion}.console.aws.amazon.com/amplify/apps/${appState.amplifyResources.selectedApp?.app_id}/branches/${appState.amplifyResources.selectedBranch?.branch_name}/deployments`}
                            target="_blank"
                            rel="noopener noreferrer"
                            class="console-link-button"
                          >
                            View in AWS Console
                          </a>
                        </div>
                      </div>
                    </Show>
                    <Show when={lastFailedJob()?.status === "SUCCEED"}>
                      <div class="failed-job-notice">
                        <p class="failed-job-text">
                          <strong>Note:</strong> The last deployment job (ID:{" "}
                          {lastFailedJob()?.job_id}) succeeded, but your Lambda
                          functions might have been updated outside of Amplify
                          after that deployment. You can trigger a new
                          deployment to ensure the runtime configurations are
                          applied.
                        </p>
                        <button
                          class="retry-job-button"
                          onClick={handleRetryJob}
                          disabled={retryingJob()}
                        >
                          {retryingJob()
                            ? "Starting Deployment..."
                            : "Trigger New Deployment"}
                        </button>
                      </div>
                    </Show>
                  </Show>
                  <Show when={!lastFailedJob()}>
                    <p class="no-deployment-note">
                      Since no code changes were made, your Lambda functions
                      should already be using the correct runtime versions.
                    </p>
                  </Show>
                </Show>

                {/* Show "Next Steps" if a job is being tracked (after retry) */}
                <Show when={amplifyJob()}>
                  <p>
                    <strong>Next Steps:</strong>
                  </p>
                  <ul>
                    <li>
                      Monitor the deployment in the{" "}
                      <a
                        href={`https://${appState.awsConfig.selectedRegion}.console.aws.amazon.com/amplify/apps/${appState.amplifyResources.selectedApp?.app_id}/branches/${appState.amplifyResources.selectedBranch?.branch_name}/deployments`}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        AWS Amplify Console
                      </a>
                    </li>
                    <li>
                      Verify Lambda functions are using the new runtime after
                      deployment completes
                    </li>
                  </ul>
                </Show>
              </div>
            </Show>
          </div>
        </Show>

        <Show when={pushStatus() === "failed"}>
          <div class="push-error">
            <div class="error-icon">✗</div>
            <h3>Push Failed</h3>
            <div class="error-details">
              <pre class="error-message">{pushError()}</pre>
            </div>
            <p class="error-hint">
              Common issues: authentication problems, network errors, or
              conflicts with remote branch. Ensure you have push access to the
              repository.
            </p>
            <button class="retry-button" onClick={handleInitiatePush}>
              Retry Push
            </button>
          </div>
        </Show>
      </div>

      {/* Actions */}
      <div class="actions">
        <button
          onClick={handleBack}
          class="secondary-button"
          disabled={pushStatus() === "running"}
        >
          Back
        </button>
        <Show when={pushStatus() === "success"}>
          <button onClick={handleFinish} class="primary-button">
            Clean Up
          </button>
        </Show>
      </div>

      {/* Revert Confirmation Dialog */}
      <Show when={showRevertDialog()}>
        <div class="revert-dialog-overlay">
          <div class="revert-dialog">
            <h3>Revert Environment Variables</h3>
            <p>
              Are you sure you want to revert the environment variable changes?
              This will restore the original values.
            </p>
            <div class="revert-changes-list">
              <For
                each={getEnvVarChanges().filter(
                  (c) => c.key !== "_LIVE_UPDATES" && c.key !== "_CUSTOM_IMAGE",
                )}
              >
                {(change) => (
                  <div class="revert-change-item">
                    <span class="revert-level-badge">{change.level}</span>
                    <code class="revert-key">{change.key}</code>
                    <span class="revert-arrow">←</span>
                    <span class="revert-value">{change.old_value}</span>
                  </div>
                )}
              </For>
            </div>
            <div class="revert-dialog-actions">
              <button
                class="cancel-revert-button"
                onClick={handleCancelRevert}
                disabled={revertInProgress()}
              >
                Cancel
              </button>
              <button
                class="confirm-revert-button"
                onClick={handleRevertEnvVars}
                disabled={revertInProgress()}
              >
                {revertInProgress() ? "Reverting..." : "Confirm Revert"}
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Build Spec Revert Confirmation Dialog */}
      <Show when={showBuildSpecRevertDialog()}>
        <div class="revert-dialog-overlay">
          <div class="revert-dialog">
            <h3>Revert Build Configuration</h3>
            <p>
              Are you sure you want to revert the build configuration changes?
              This will restore the original buildSpec in AWS Amplify.
            </p>
            <div class="revert-changes-list">
              <Show when={appState.repository.buildConfigChange}>
                <div class="revert-change-item">
                  <span class="revert-level-badge">cloud</span>
                  <code class="revert-key">Build Command</code>
                  <span class="revert-arrow">←</span>
                  <span class="revert-value">
                    {appState.repository.buildConfigChange?.old_command}
                  </span>
                </div>
              </Show>
            </div>
            <div class="revert-dialog-actions">
              <button
                class="cancel-revert-button"
                onClick={handleCancelBuildSpecRevert}
                disabled={revertBuildSpecInProgress()}
              >
                Cancel
              </button>
              <button
                class="confirm-revert-button"
                onClick={handleRevertBuildSpec}
                disabled={revertBuildSpecInProgress()}
              >
                {revertBuildSpecInProgress()
                  ? "Reverting..."
                  : "Confirm Revert"}
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Cleanup Dialog */}
      <CleanupDialog
        show={showCleanupDialog()}
        onClose={handleCleanupClose}
        resetToStep={2}
      />
    </div>
  );
}

export default PushStep;
