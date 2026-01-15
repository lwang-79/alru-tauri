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
    <div class="max-w-[800px] mx-auto push-step">
      <h2 class="mb-2 text-2xl font-bold">Push Changes</h2>
      <p class="text-[#666] dark:text-[#999] mb-8">
        Review and push your changes to trigger the Amplify deployment.
      </p>

      {/* Summary Section */}
      <div class="bg-white dark:bg-[#2a2a2a] border border-[#e0e0e0] dark:border-[#444] rounded-lg p-6 mb-8">
        <h3 class="text-lg font-semibold mb-4 text-[#333] dark:text-[#eee]">Changes Summary</h3>
        <div class="flex flex-wrap gap-x-4 gap-y-3 mb-3">
          <div class="flex-1 basis-[calc(50%-1rem)] min-w-[200px] flex items-center gap-2">
            <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[120px]">App:</span>
            <span class="text-[#333] dark:text-[#eee]">
              {appState.amplifyResources.selectedApp?.name}
            </span>
          </div>
          <div class="flex-1 basis-[calc(50%-1rem)] min-w-[200px] flex items-center gap-2">
            <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[120px]">Branch:</span>
            <span class="text-[#333] dark:text-[#eee]">
              {appState.amplifyResources.selectedBranch?.branch_name}
            </span>
          </div>
        </div>
        <div class="flex flex-wrap gap-x-4 gap-y-3 mb-3">
          <div class="flex-1 basis-[calc(50%-1rem)] min-w-[200px] flex items-center gap-2">
            <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[120px]">Target Runtime:</span>
            <span class="px-3 py-1 bg-[#e0f2f1] text-[#00796b] dark:bg-[#1a2e2c] dark:text-[#4db6ac] rounded-md text-sm font-semibold font-mono">
              {appState.runtimeInfo.targetRuntime}
            </span>
          </div>
          <div class="flex-1 basis-[calc(50%-1rem)] min-w-[200px] flex items-center gap-2">
            <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[120px]">Backend Type:</span>
            <span class="px-3 py-1 bg-[#e3f2fd] text-[#1976d2] dark:bg-[#1a2a3a] dark:text-[#64b5f6] rounded-md text-sm font-semibold">
              {appState.repository.backendType === "Gen2" ? "Gen 2" : "Gen 1"}
            </span>
          </div>
        </div>
        <div class="flex flex-wrap gap-x-4 gap-y-3">
          <div class="w-full flex items-center gap-2">
            <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[120px]">Files Modified:</span>
            <span class="text-[#333] dark:text-[#eee]">
              {appState.repository.changes.length > 0
                ? `${appState.repository.changes.length} file(s)`
                : "No manual changes (updated via package upgrade)"}
            </span>
          </div>
        </div>
      </div>

      {/* Push Action Section */}
      <div class="bg-white dark:bg-[#2a2a2a] border border-[#e0e0e0] dark:border-[#444] rounded-lg p-8 mb-8 min-h-[300px] flex items-center justify-center">
        <Show when={pushStatus() === "pending"}>
          <div class="text-center max-w-[500px]">
            <div class="w-16 h-16 rounded-full bg-[#4caf50] text-white text-2xl flex items-center justify-center mx-auto mb-6">✓</div>
            <h3 class="m-0 mb-4 text-xl font-semibold text-[#333] dark:text-[#eee]">Ready to Push</h3>
            <p class="text-[#666] dark:text-[#aaa] leading-relaxed mb-8">
              Your changes are ready to be committed and pushed to the remote
              repository. This will trigger an Amplify deployment with the
              updated runtime configuration.
            </p>
            <button class="bg-[#4caf50] text-white border-none px-8 py-3.5 rounded-md text-base font-semibold cursor-pointer transition-all duration-200 hover:bg-[#45a049] shadow-md hover:shadow-lg active:transform active:scale-95" onClick={handleInitiatePush}>
              Push Changes
            </button>
          </div>
        </Show>

        <Show when={pushStatus() === "confirming"}>
          <div class="text-center max-w-[500px]">
            <div class="w-16 h-16 rounded-full bg-[#ff9800] text-white text-2xl flex items-center justify-center mx-auto mb-6">⚠️</div>
            <h3 class="m-0 mb-4 text-xl font-semibold text-[#333] dark:text-[#eee]">Confirm Push</h3>
            <p class="text-[#666] dark:text-[#aaa] leading-relaxed mb-8">
              Are you sure you want to push these changes? This will commit and
              push to the{" "}
              <strong class="text-[#333] dark:text-[#eee] font-bold">
                {appState.amplifyResources.selectedBranch?.branch_name}
              </strong>{" "}
              branch and trigger an Amplify deployment.
            </p>
            <div class="flex justify-center gap-4">
              <button class="bg-transparent text-[#666] dark:text-[#aaa] border border-[#ccc] dark:border-[#555] px-6 py-3 rounded-md font-semibold cursor-pointer transition-all duration-200 hover:bg-[#f5f5f5] dark:hover:bg-[#333]" onClick={handleCancelPush}>
                Cancel
              </button>
              <button class="bg-[#4caf50] text-white border-none px-6 py-3 rounded-md font-semibold cursor-pointer transition-all duration-200 hover:bg-[#45a049]" onClick={handleConfirmPush}>
                Confirm & Push
              </button>
            </div>
          </div>
        </Show>

        <Show when={pushStatus() === "running"}>
          <div class="text-center max-w-[500px]">
            <span class="w-10 h-10 border-4 border-[#e0e0e0] border-t-[#396cd8] rounded-full animate-spin inline-block mb-4"></span>
            <h3 class="m-0 mb-2 text-xl font-semibold text-[#333] dark:text-[#eee]">Pushing Changes...</h3>
            <p class="text-[#666] dark:text-[#aaa] m-0">Committing and pushing to remote repository</p>
          </div>
        </Show>

        <Show when={pushStatus() === "success"}>
          <div
            class={`text-center p-8 rounded-lg border transition-all duration-200 w-full ${commitHash() ? "bg-[#f0f8f0] border-[#4caf50] dark:bg-[#1a2a1a]" : "bg-[#fff8f0] border-[#f57c00] dark:bg-[#3a2a1a]"}`}
          >
            <Show when={commitHash()}>
              <div class="w-16 h-16 rounded-full bg-[#4caf50] text-white text-2xl flex items-center justify-center mx-auto mb-6">✓</div>
              <h3 class="text-[#15803d] dark:text-[#66bb6a] text-xl font-semibold m-4">Successfully Pushed!</h3>
            </Show>
            <Show when={!commitHash()}>
              <div class="w-16 h-16 rounded-full bg-[#f57c00] text-white text-2xl font-bold flex items-center justify-center mx-auto mb-6">⚠️</div>
              <h3 class="text-[#e65100] dark:text-[#ffb74d] text-xl font-semibold m-4">Push Skipped</h3>
            </Show>
            <Show when={commitHash()}>
              <div class="flex items-center justify-center gap-2 mb-6 p-3 bg-white/50 dark:bg-[#333]/50 rounded-md border border-black/5 dark:border-white/5">
                <span class="text-[0.85rem] text-[#666] dark:text-[#aaa] font-medium">Commit:</span>
                <code class="font-mono text-[0.85rem] bg-[#e0e0e0] dark:bg-[#444] px-2 py-1 rounded text-[#333] dark:text-[#eee]">{commitHash()}</code>
              </div>
            </Show>

            {/* Amplify Job Status */}
            <Show when={amplifyJob()}>
              <AmplifyJobStatus job={amplifyJob()!} />
            </Show>

            <Show when={jobCheckError() && !commitHash()}>
              <div class="mt-4 text-[#0066cc] dark:text-[#7dd3fc]">
                <p class="flex items-center justify-center gap-2 text-sm italic">
                  <Show when={checkingForJob()}>
                    <span class="w-3 h-3 border-2 border-[#e0f2fe] border-t-[#0ea5e9] rounded-full animate-spin"></span>
                  </Show>
                  {jobCheckError()}
                </p>
              </div>
            </Show>

            <Show when={jobCheckError() && commitHash()}>
              <div class="mt-4 text-[#f57c00] dark:text-[#ffb74d]">
                <p class="flex items-center justify-center gap-2 text-sm italic">
                  <Show when={checkingForJob()}>
                    <span class="w-3 h-3 border-2 border-[#fff7ed] border-t-[#f97316] rounded-full animate-spin"></span>
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
                <div class="mt-4">
                  <button
                    class="px-4 py-2 bg-[#ffc107] text-[#212529] border-none rounded-md text-[0.875rem] font-medium cursor-pointer transition-all duration-200 hover:bg-[#e0a800] hover:-translate-y-0.5"
                    onClick={() => setShowRevertDialog(true)}
                  >
                    Revert Environment Variables
                  </button>
                  <p class="mt-2 text-[0.75rem] text-[#6c757d] dark:text-[#aaa] italic">
                    Note: _LIVE_UPDATES and _CUSTOM_IMAGE changes cannot be
                    reverted
                  </p>
                </div>
              </Show>
            </EnvVarChangesList>

            {/* Build Config Changes */}
            <Show
              when={
                appState.repository.buildConfigChange &&
                appState.repository.buildConfigChange.location === "Cloud"
              }
            >
              <div class="mt-6 p-4 bg-[#f8f9fa] dark:bg-[#333] rounded-lg border border-[#e9ecef] dark:border-[#444] text-left">
                <h4 class="m-0 mb-4 text-[#495057] dark:text-[#eee] text-base font-semibold">Build Configuration Changes</h4>
                <div class="p-3 bg-white dark:bg-[#444] rounded-md border border-[#dee2e6] dark:border-[#555] mb-4">
                  <div class="flex items-center gap-2 mb-3">
                    <span class="font-medium text-[#6c757d] dark:text-[#aaa] text-[0.875rem]">Location:</span>
                    <span class="text-[#495057] dark:text-[#eee] text-[0.875rem]">
                      Cloud (AWS Amplify buildSpec)
                    </span>
                  </div>
                  <div class="flex flex-col gap-2">
                    <div class="flex flex-col gap-1">
                      <span class="font-medium text-[#6c757d] dark:text-[#aaa] text-[0.875rem]">Old Command:</span>
                      <code class="font-mono text-[0.8rem] p-2 rounded block break-all bg-[#f8d7da] text-[#721c24] border border-[#f5c6cb] dark:bg-[#4a1f1f] dark:text-[#e57373] dark:border-[#4a1f1f]">
                        {appState.repository.buildConfigChange?.old_command}
                      </code>
                    </div>
                    <span class="text-[#6c757d] dark:text-[#aaa] font-bold text-center my-1 select-none">↓</span>
                    <div class="flex flex-col gap-1">
                      <span class="font-medium text-[#6c757d] dark:text-[#aaa] text-[0.875rem]">New Command:</span>
                      <code class="font-mono text-[0.8rem] p-2 rounded block break-all bg-[#d4edda] text-[#155724] border border-[#c3e6cb] dark:bg-[#1b3a24] dark:text-[#81c784] dark:border-[#1b3a24]">
                        {appState.repository.buildConfigChange?.new_command}
                      </code>
                    </div>
                  </div>
                </div>
                <button
                  class="px-4 py-2 bg-[#ffc107] text-[#212529] border-none rounded-md text-[0.875rem] font-medium cursor-pointer transition-all duration-200 hover:bg-[#e0a800] hover:-translate-y-0.5"
                  onClick={() => setShowBuildSpecRevertDialog(true)}
                >
                  Revert Build Configuration
                </button>
                <p class="mt-2 text-[0.75rem] text-[#6c757d] dark:text-[#aaa] italic">
                  This will restore the original buildSpec in AWS Amplify
                </p>
              </div>
            </Show>

            <Show when={commitHash()}>
              <div class="mt-6 p-4 bg-[#e8f5e9] dark:bg-[#1a2e1a] rounded-lg text-left">
                <p class="m-0 mb-3 font-semibold text-[#2e7d32] dark:text-[#66bb6a]">
                  Next Steps:
                </p>
                <ul class="m-0 pl-6 text-[#666] dark:text-[#aaa] leading-relaxed list-disc">
                  <li class="mb-2">
                    Monitor the deployment in the{" "}
                    <a
                      href={`https://${appState.awsConfig.selectedRegion}.console.aws.amazon.com/amplify/apps/${appState.amplifyResources.selectedApp?.app_id}/branches/${appState.amplifyResources.selectedBranch?.branch_name}/deployments`}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="text-[#396cd8] dark:text-[#5c8ce6] font-medium hover:underline"
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
              <div class="mt-6 p-4 bg-[#e8f5e9] dark:bg-[#1a2e1a] rounded-lg text-left">
                {/* Show "What happened" only if no job is being tracked */}
                <Show when={!amplifyJob()}>
                  <p class="m-0 mb-3 font-semibold text-[#2e7d32] dark:text-[#66bb6a]">
                    What happened:
                  </p>
                  <ul class="m-0 pl-6 text-[#666] dark:text-[#aaa] leading-relaxed list-disc">
                    <li class="mb-1">All runtime configurations were already up to date</li>
                    <li class="mb-1">Environment variables were updated as needed</li>
                    <li>
                      No file changes were required, so no commit was created
                    </li>
                  </ul>
                  <Show when={lastFailedJob()}>
                    <Show when={lastFailedJob()?.status === "FAILED"}>
                      <div class="mt-4 p-4 bg-[#fff3cd] dark:bg-[#3a2e1a] border border-[#ffc107] dark:border-[#f57c00] rounded-lg">
                        <p class="m-0 mb-3 text-[#856404] dark:text-[#ffb74d] text-sm leading-relaxed">
                          <strong class="font-bold">Note:</strong> The last deployment job (ID:{" "}
                          {lastFailedJob()?.job_id}) failed. This might be why
                          your Lambda functions haven't been updated yet.
                        </p>
                        <div class="flex flex-wrap gap-3">
                          <button
                            class="px-4 py-2 bg-[#ff9800] text-white border-none rounded-md text-sm font-medium cursor-pointer transition-all duration-200 hover:bg-[#f57c00] disabled:opacity-60 disabled:cursor-not-allowed shadow-sm"
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
                            class="px-4 py-2 bg-[#396cd8] text-white rounded-md text-sm font-medium transition-all duration-200 hover:bg-[#2563eb] shadow-sm"
                          >
                            View in AWS Console
                          </a>
                        </div>
                      </div>
                    </Show>
                    <Show when={lastFailedJob()?.status === "SUCCEED"}>
                      <div class="mt-4 p-4 bg-[#fff3cd] dark:bg-[#3a2e1a] border border-[#ffc107] dark:border-[#f57c00] rounded-lg text-left">
                        <p class="m-0 mb-3 text-[#856404] dark:text-[#ffb74d] text-sm leading-relaxed">
                          <strong class="font-bold">Note:</strong> The last deployment job (ID:{" "}
                          {lastFailedJob()?.job_id}) succeeded, but your Lambda
                          functions might have been updated outside of Amplify
                          after that deployment. You can trigger a new
                          deployment to ensure the runtime configurations are
                          applied.
                        </p>
                        <button
                          class="px-4 py-2 bg-[#ff9800] text-white border-none rounded-md text-sm font-medium cursor-pointer transition-all duration-200 hover:bg-[#f57c00] disabled:opacity-60 disabled:cursor-not-allowed shadow-sm"
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
                    <p class="mt-4 p-3 bg-[#e7f3ff] dark:bg-[#1a2a3a] border border-[#b3d9ff] dark:border-[#396cd8] rounded-md text-[#0066cc] dark:text-[#7dd3fc] text-sm italic">
                      Since no code changes were made, your Lambda functions
                      should already be using the correct runtime versions.
                    </p>
                  </Show>
                </Show>

                {/* Show "Next Steps" if a job is being tracked (after retry) */}
                <Show when={amplifyJob()}>
                  <p class="m-0 mb-3 font-semibold text-[#2e7d32] dark:text-[#66bb6a]">
                    Next Steps:
                  </p>
                  <ul class="m-0 pl-6 text-[#666] dark:text-[#aaa] leading-relaxed list-disc">
                    <li class="mb-2">
                      Monitor the deployment in the{" "}
                      <a
                        href={`https://${appState.awsConfig.selectedRegion}.console.aws.amazon.com/amplify/apps/${appState.amplifyResources.selectedApp?.app_id}/branches/${appState.amplifyResources.selectedBranch?.branch_name}/deployments`}
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-[#396cd8] dark:text-[#5c8ce6] font-medium hover:underline"
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
          <div class="text-center max-w-[600px]">
            <div class="w-16 h-16 rounded-full bg-[#f44336] text-white text-2xl flex items-center justify-center mx-auto mb-6">✗</div>
            <h3 class="m-0 mb-4 text-[#c62828] dark:text-[#ef5350] text-xl font-semibold">Push Failed</h3>
            <div class="bg-[#ffebee] dark:bg-[#3a1a1a] p-4 rounded-lg text-left mb-4 overflow-hidden">
              <pre class="m-0 font-mono text-[0.85rem] text-[#c62828] dark:text-[#ff6b6b] whitespace-pre-wrap break-words">{pushError()}</pre>
            </div>
            <p class="text-[#666] dark:text-[#aaa] text-[0.9rem] leading-relaxed mb-6">
              Common issues: authentication problems, network errors, or
              conflicts with remote branch. Ensure you have push access to the
              repository.
            </p>
            <button class="bg-[#396cd8] text-white border-none px-6 py-2.5 rounded-md text-[0.95rem] font-medium cursor-pointer transition-all duration-200 hover:bg-[#2563eb]" onClick={handleInitiatePush}>
              Retry Push
            </button>
          </div>
        </Show>
      </div>

      {/* Actions */}
      <div class="flex justify-end gap-3 mt-8">
        <button
          onClick={handleBack}
          class="bg-transparent text-[#396cd8] border border-[#396cd8] px-6 py-2.5 rounded-md font-medium cursor-pointer transition-all duration-200 hover:bg-[#396cd8] hover:text-white disabled:opacity-60 disabled:cursor-not-allowed"
          disabled={pushStatus() === "running"}
        >
          Back
        </button>
        <Show when={pushStatus() === "success"}>
          <button onClick={handleFinish} class="bg-[#396cd8] text-white border-none px-6 py-2.5 rounded-md font-medium cursor-pointer transition-all duration-200 hover:bg-[#2563eb]">
            Clean Up
          </button>
        </Show>
      </div>

      {/* Revert Confirmation Dialog */}
      <Show when={showRevertDialog()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[1000]">
          <div class="bg-white dark:bg-[#2a2a2a] rounded-xl p-6 max-w-[500px] w-[90%] max-h-[80vh] overflow-y-auto shadow-2xl">
            <h3 class="m-0 mb-4 text-[#495057] dark:text-[#eee] text-xl font-semibold">Revert Environment Variables</h3>
            <p class="m-0 mb-4 text-[#6c757d] dark:text-[#aaa] leading-relaxed">
              Are you sure you want to revert the environment variable changes?
              This will restore the original values.
            </p>
            <div class="flex flex-col gap-2 mb-6 max-h-[200px] overflow-y-auto pr-1">
              <For
                each={getEnvVarChanges().filter(
                  (c) => c.key !== "_LIVE_UPDATES" && c.key !== "_CUSTOM_IMAGE",
                )}
              >
                {(change) => (
                  <div class="flex items-center gap-2 p-2 bg-[#f8f9fa] dark:bg-[#333] rounded-md text-[0.875rem]">
                    <span class="px-2 py-0.5 bg-[#6c757d] text-white rounded text-[0.75rem] font-medium uppercase">{change.level}</span>
                    <code class="font-mono bg-white dark:bg-[#444] px-1.5 py-0.5 rounded text-[#495057] dark:text-[#ddd]">{change.key}</code>
                    <span class="text-[#ffc107] font-bold">←</span>
                    <span class="text-[#28a745] dark:text-[#81c784] font-mono bg-[#d4edda] dark:bg-[#1b3a24] px-1.5 py-0.5 rounded">{change.old_value}</span>
                  </div>
                )}
              </For>
            </div>
            <div class="flex justify-end gap-3">
              <button
                class="px-4 py-2 bg-[#6c757d] text-white border-none rounded-md text-sm cursor-pointer transition-all duration-200 hover:bg-[#5a6268] disabled:opacity-60 disabled:cursor-not-allowed"
                onClick={handleCancelRevert}
                disabled={revertInProgress()}
              >
                Cancel
              </button>
              <button
                class="px-4 py-2 bg-[#ffc107] text-[#212529] border-none rounded-md text-sm font-medium cursor-pointer transition-all duration-200 hover:bg-[#e0a800] disabled:opacity-60 disabled:cursor-not-allowed"
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
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[1000]">
          <div class="bg-white dark:bg-[#2a2a2a] rounded-xl p-6 max-w-[500px] w-[90%] max-h-[80vh] overflow-y-auto shadow-2xl">
            <h3 class="m-0 mb-4 text-[#495057] dark:text-[#eee] text-xl font-semibold">Revert Build Configuration</h3>
            <p class="m-0 mb-4 text-[#6c757d] dark:text-[#aaa] leading-relaxed">
              Are you sure you want to revert the build configuration changes?
              This will restore the original buildSpec in AWS Amplify.
            </p>
            <div class="flex flex-col gap-2 mb-6">
              <Show when={appState.repository.buildConfigChange}>
                <div class="flex items-center gap-2 p-2 bg-[#f8f9fa] dark:bg-[#333] rounded-md text-[0.875rem]">
                  <span class="px-2 py-0.5 bg-[#6c757d] text-white rounded text-[0.75rem] font-medium uppercase">cloud</span>
                  <code class="font-mono bg-white dark:bg-[#444] px-1.5 py-0.5 rounded text-[#495057] dark:text-[#ddd]">Build Command</code>
                  <span class="text-[#ffc107] font-bold">←</span>
                  <span class="text-[#28a745] dark:text-[#81c784] font-mono bg-[#d4edda] dark:bg-[#1b3a24] px-1.5 py-0.5 rounded">
                    {appState.repository.buildConfigChange?.old_command}
                  </span>
                </div>
              </Show>
            </div>
            <div class="flex justify-end gap-3">
              <button
                class="px-4 py-2 bg-[#6c757d] text-white border-none rounded-md text-sm cursor-pointer transition-all duration-200 hover:bg-[#5a6268] disabled:opacity-60 disabled:cursor-not-allowed"
                onClick={handleCancelBuildSpecRevert}
                disabled={revertBuildSpecInProgress()}
              >
                Cancel
              </button>
              <button
                class="px-4 py-2 bg-[#ffc107] text-[#212529] border-none rounded-md text-sm font-medium cursor-pointer transition-all duration-200 hover:bg-[#e0a800] disabled:opacity-60 disabled:cursor-not-allowed"
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
