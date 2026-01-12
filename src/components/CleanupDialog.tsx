import { Show, createSignal, batch } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { appState, setAppState, resetPushStepState } from "../store/appStore";
import "./shared.css";
import "./CleanupDialog.css";

interface CleanupDialogProps {
  show: boolean;
  onClose: () => void;
  resetToStep?: number; // Optional: which step to navigate to after cleanup
}

export function CleanupDialog(props: CleanupDialogProps) {
  const [cleanupInProgress, setCleanupInProgress] = createSignal(false);
  const [deleteSandboxOnCleanup, setDeleteSandboxOnCleanup] =
    createSignal(true);
  const [sandboxDeletionInProgress, setSandboxDeletionInProgress] =
    createSignal(false);

  // Helper function to generate CloudFormation monitoring URL
  const getCloudFormationMonitoringUrl = () => {
    const region = appState.awsConfig.selectedRegion;

    // Generate the CloudFormation console URL with filtering
    return `https://console.aws.amazon.com/cloudformation/home?region=${region}#/stacks/stackinfo?filteringText=sandbox&filteringStatus=active&viewNested=false`;
  };

  const handleCleanupConfirm = async (shouldCleanup: boolean) => {
    if (shouldCleanup && appState.repository.clonePath) {
      setCleanupInProgress(true);
      try {
        // Delete sandbox if it was deployed and user opted to delete it
        if (appState.repository.sandboxDeployed && deleteSandboxOnCleanup()) {
          const profile = appState.awsConfig.selectedProfile;
          const region = appState.awsConfig.selectedRegion;
          if (profile && region) {
            try {
              // Start sandbox deletion and show monitoring link
              setSandboxDeletionInProgress(true);

              await invoke("delete_gen2_sandbox", {
                projectPath: appState.repository.clonePath,
                profile: profile,
                region: region,
              });

              setSandboxDeletionInProgress(false);
            } catch (e) {
              console.error("Failed to delete sandbox:", e);
              setSandboxDeletionInProgress(false);
              // Continue with repository cleanup even if sandbox deletion fails
            }
          }
        }

        await invoke("cleanup_repository", {
          path: appState.repository.clonePath,
        });

        // Reset all state in a batch to ensure UI updates properly
        batch(() => {
          console.log("[Cleanup] Starting state reset in batch");
          console.log("[Cleanup] Step 2 before reset:", {
            isEnabled: appState.wizard.steps[2].isEnabled,
            isComplete: appState.wizard.steps[2].isComplete,
          });
          console.log("[Cleanup] Step 3 before reset:", {
            isEnabled: appState.wizard.steps[3].isEnabled,
            isComplete: appState.wizard.steps[3].isComplete,
          });
          console.log("[Cleanup] Step 4 before reset:", {
            isEnabled: appState.wizard.steps[4].isEnabled,
            isComplete: appState.wizard.steps[4].isComplete,
          });

          // Reset repository state - this clears the cloned repository and all related state
          setAppState("repository", {
            clonePath: null,
            packageManager: null,
            backendType: null,
            changes: [],
            buildStatus: "pending",
            sandboxDeployed: false,
            gen2SandboxEnabled: false,
            gen2SandboxStatus: "pending",
            gen2BuildVerificationStatus: "pending",
            isOperationRunning: false,
            envVarChanges: [],
            buildConfigChange: null,
            originalBuildSpec: null,
            operationStatus: {
              cloneComplete: false,
              prepareComplete: false,
              updateComplete: false,
              upgradeComplete: false,
              buildConfigComplete: false,
              buildComplete: false,
              envVarComplete: false,
              gen2EnvVarComplete: false,
            },
          });

          // Reset amplify resources - clear selections but keep apps list
          setAppState("amplifyResources", "selectedApp", null);
          setAppState("amplifyResources", "selectedBranch", null);
          setAppState("amplifyResources", "branches", []);
          setAppState("amplifyResources", "lambdaFunctions", []);

          // Reset Push step state since all upstream data is being cleared
          resetPushStepState();

          // Reset step 2 (App Selection) - mark as not complete but keep enabled
          setAppState("wizard", "steps", 2, "isComplete", false);

          // Reset step 3 (Clone & Update) - mark as not complete AND disabled
          // Users can only access it by clicking Continue from App Selection step
          setAppState("wizard", "steps", 3, "isComplete", false);
          setAppState("wizard", "steps", 3, "isEnabled", false);

          // Reset step 4 (Push) - mark as not complete AND disabled
          setAppState("wizard", "steps", 4, "isComplete", false);
          setAppState("wizard", "steps", 4, "isEnabled", false);

          console.log("[Cleanup] Step 2 after reset:", {
            isEnabled: appState.wizard.steps[2].isEnabled,
            isComplete: appState.wizard.steps[2].isComplete,
          });
          console.log("[Cleanup] Step 3 after reset:", {
            isEnabled: appState.wizard.steps[3].isEnabled,
            isComplete: appState.wizard.steps[3].isComplete,
          });
          console.log("[Cleanup] Step 4 after reset:", {
            isEnabled: appState.wizard.steps[4].isEnabled,
            isComplete: appState.wizard.steps[4].isComplete,
          });

          // If resetToStep is provided and current step is after step 2, navigate to that step
          if (
            props.resetToStep !== undefined &&
            appState.wizard.currentStep > 2
          ) {
            console.log(
              `[Cleanup] Navigating from step ${appState.wizard.currentStep} to step ${props.resetToStep}`,
            );
            setAppState("wizard", "currentStep", props.resetToStep);

            // Scroll to top after cleanup navigation
            setTimeout(() => {
              const scrollableWrapper = document.querySelector(
                ".scrollable-wrapper",
              );
              if (scrollableWrapper) {
                scrollableWrapper.scrollTop = 0;
              }
            }, 0);
          }

          console.log("[Cleanup] Batch complete, final states:");
          console.log("  Current step:", appState.wizard.currentStep);
          console.log("  Step 2 (App Selection):", {
            isEnabled: appState.wizard.steps[2].isEnabled,
            isComplete: appState.wizard.steps[2].isComplete,
          });
          console.log("  Step 3 (Clone & Update):", {
            isEnabled: appState.wizard.steps[3].isEnabled,
            isComplete: appState.wizard.steps[3].isComplete,
          });
          console.log("  Step 4 (Push Changes):", {
            isEnabled: appState.wizard.steps[4].isEnabled,
            isComplete: appState.wizard.steps[4].isComplete,
          });
        });
      } catch (e) {
        console.error("Failed to cleanup repository:", e);
      } finally {
        setCleanupInProgress(false);
      }
    }

    setDeleteSandboxOnCleanup(true); // Reset for next time
    setSandboxDeletionInProgress(false); // Reset state
    props.onClose();
  };

  return (
    <Show when={props.show}>
      <div class="cleanup-dialog-overlay">
        <div class="cleanup-dialog">
          <h3>Clean up cloned repository?</h3>
          <p>
            A repository has been cloned to your system. Would you like to clean
            it up?
          </p>
          <Show when={appState.repository.sandboxDeployed}>
            <div class="cleanup-dialog-checkbox">
              <label class="cleanup-checkbox-label">
                <input
                  type="checkbox"
                  checked={deleteSandboxOnCleanup()}
                  onChange={(e) =>
                    setDeleteSandboxOnCleanup(e.currentTarget.checked)
                  }
                  disabled={cleanupInProgress()}
                />
                <span>Also delete the deployed sandbox</span>
              </label>
              <p class="cleanup-dialog-hint">
                This will run <code>ampx sandbox delete</code> to remove the
                sandbox resources from AWS.
              </p>
            </div>
          </Show>

          {/* Sandbox Deletion Monitoring Link */}
          <Show when={sandboxDeletionInProgress()}>
            <div class="cleanup-monitoring-section">
              <p class="cleanup-monitoring-text">
                <span class="spinner-small"></span>
                Deleting sandbox ... This may take a while.
              </p>
              <Show when={getCloudFormationMonitoringUrl()}>
                <a
                  href={getCloudFormationMonitoringUrl()!}
                  target="_blank"
                  rel="noopener noreferrer"
                  class="cleanup-monitoring-link"
                >
                  Monitor Deletion in AWS Console
                </a>
              </Show>
            </div>
          </Show>

          <Show when={cleanupInProgress() && !sandboxDeletionInProgress()}>
            <div class="cleanup-monitoring-section">
              <p class="cleanup-monitoring-text">
                <span class="spinner-small"></span>
                Cleaning up cloned repository ...
              </p>
            </div>
          </Show>

          <div class="cleanup-dialog-actions">
            <button
              class="cleanup-secondary-button"
              onClick={() => handleCleanupConfirm(false)}
              disabled={cleanupInProgress()}
            >
              Keep Repository
            </button>
            <button
              class="cleanup-primary-button"
              onClick={() => handleCleanupConfirm(true)}
              disabled={cleanupInProgress()}
            >
              <Show when={cleanupInProgress()}>
                <span class="cleanup-spinner-small"></span>
                Cleaning Up...
              </Show>
              <Show when={!cleanupInProgress()}>Clean Up</Show>
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}

export default CleanupDialog;
