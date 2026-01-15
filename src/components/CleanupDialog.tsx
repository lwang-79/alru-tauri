import { Show, createSignal, batch } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { appState, setAppState, resetPushStepState } from "../store/appStore";

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
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[1000]">
        <div class="bg-white dark:bg-[#2a2a2a] rounded-xl p-8 max-w-[500px] w-[90%] shadow-2xl">
          <h3 class="m-0 mb-4 text-xl font-semibold text-[#333] dark:text-[#eee]">Clean up cloned repository?</h3>
          <p class="m-0 mb-6 text-[#666] dark:text-[#aaa] leading-relaxed">
            A repository has been cloned to your system. Would you like to clean
            it up?
          </p>
          <Show when={appState.repository.sandboxDeployed}>
            <div class="mb-6 p-4 bg-[#f8f9fa] dark:bg-[#333] rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer font-medium text-[#333] dark:text-[#eee] mb-2">
                <input
                  type="checkbox"
                  class="w-[18px] h-[18px] cursor-pointer accent-[#396cd8]"
                  checked={deleteSandboxOnCleanup()}
                  onChange={(e) =>
                    setDeleteSandboxOnCleanup(e.currentTarget.checked)
                  }
                  disabled={cleanupInProgress()}
                />
                <span>Also delete the deployed sandbox</span>
              </label>
              <p class="m-0 pl-7 text-[0.8rem] text-[#666] dark:text-[#aaa]">
                This will run <code class="bg-[#e9ecef] dark:bg-[#444] px-1.5 py-0.5 rounded text-[0.75rem] dark:text-[#ddd]">ampx sandbox delete</code> to remove the
                sandbox resources from AWS.
              </p>
            </div>
          </Show>

          {/* Sandbox Deletion Monitoring Link */}
          <Show when={sandboxDeletionInProgress()}>
            <div class="mb-6 p-4 bg-[#fff3cd] dark:bg-[#3a3a1a] border border-[#ffeaa7] dark:border-[#5a5a2a] rounded-lg">
              <p class="text-[#856404] dark:text-[#d4b106] text-[0.9rem] flex items-center gap-2 m-0 mb-3">
                <span class="w-3 h-3 border-2 border-[#856404]/20 border-t-[#856404] rounded-full animate-spin"></span>
                Deleting sandbox ... This may take a while.
              </p>
              <Show when={getCloudFormationMonitoringUrl()}>
                <a
                  href={getCloudFormationMonitoringUrl()!}
                  target="_blank"
                  rel="noopener noreferrer"
                  class="inline-flex items-center px-3 py-2 text-[#0066cc] dark:text-[#64b5f6] border border-[#0066cc] dark:border-[#64b5f6] rounded font-medium text-[0.9rem] transition-all duration-200 hover:bg-[#0066cc] dark:hover:bg-[#64b5f6] hover:text-white dark:hover:text-[#1a1a1a] after:content-['↗'] after:ml-2 after:text-[0.8rem]"
                >
                  Monitor Deletion in AWS Console
                </a>
              </Show>
            </div>
          </Show>

          <Show when={cleanupInProgress() && !sandboxDeletionInProgress()}>
            <div class="mb-6 p-4 bg-[#fff3cd] dark:bg-[#3a3a1a] border border-[#ffeaa7] dark:border-[#5a5a2a] rounded-lg">
              <p class="text-[#856404] dark:text-[#d4b106] text-[0.9rem] flex items-center gap-2 m-0">
                <span class="w-3 h-3 border-2 border-[#856404]/20 border-t-[#856404] rounded-full animate-spin"></span>
                Cleaning up cloned repository ...
              </p>
            </div>
          </Show>

          <div class="flex justify-end gap-4">
            <button
              class="bg-transparent text-[#666] dark:text-[#aaa] border border-[#ccc] dark:border-[#555] px-6 py-3 rounded-md font-medium cursor-pointer transition-all duration-200 hover:bg-[#f5f5f5] dark:hover:bg-[#333] hover:border-[#999] dark:hover:border-[#666] disabled:opacity-60 disabled:cursor-not-allowed"
              onClick={() => handleCleanupConfirm(false)}
              disabled={cleanupInProgress()}
            >
              Keep Repository
            </button>
            <button
              class="bg-[#396cd8] dark:bg-[#5c8ce6] text-white border-none px-6 py-3 rounded-md font-medium cursor-pointer transition-all duration-200 hover:bg-[#2d5bb8] dark:hover:bg-[#4a7ad4] disabled:bg-[#ccc] dark:disabled:bg-[#444] dark:disabled:text-[#888] disabled:cursor-not-allowed flex items-center justify-center gap-2"
              onClick={() => handleCleanupConfirm(true)}
              disabled={cleanupInProgress()}
            >
              <Show when={cleanupInProgress()}>
                <span class="w-3 h-3 border-2 border-white/20 border-t-white rounded-full animate-spin"></span>
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
