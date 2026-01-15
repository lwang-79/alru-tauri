import { createSignal, onMount, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type {
  AmplifyApp,
  AmplifyBranch,
  LambdaFunction,
  NodeVersion,
} from "../types";
import { appState, setAppState, clearDownstreamState } from "../store/appStore";

interface AppSelectionStepProps {
  onComplete?: () => void;
  onBack?: () => void;
}

export function AppSelectionStep(props: AppSelectionStepProps) {
  const [isLoadingApps, setIsLoadingApps] = createSignal(false);
  const [isLoadingBranches, setIsLoadingBranches] = createSignal(false);
  const [isLoadingFunctions, setIsLoadingFunctions] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Load supported Node.js runtimes
  const loadSupportedRuntimes = async () => {
    try {
      const versions = await invoke<NodeVersion[]>("get_supported_runtimes");
      setAppState("runtimeInfo", "supportedVersions", versions);

      // Also get the target runtime
      const targetRuntime = await invoke<string>("get_target_runtime", {
        versions,
      });
      setAppState("runtimeInfo", "targetRuntime", targetRuntime);

      console.log(
        "Loaded supported versions:",
        versions.filter((v) => v.is_supported).map((v) => v.version),
      );
      console.log("Target runtime:", targetRuntime);
    } catch (e) {
      console.error("Failed to load supported runtimes:", e);
    }
  };

  // Load Amplify apps when component mounts
  const loadApps = async () => {
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!profile || !region) {
      setError("Please select an AWS profile and region first.");
      return;
    }

    setIsLoadingApps(true);
    setError(null);
    // Clear existing selections when reloading apps
    setAppState("amplifyResources", "selectedApp", null);
    setAppState("amplifyResources", "branches", []);
    setAppState("amplifyResources", "selectedBranch", null);
    setAppState("amplifyResources", "lambdaFunctions", []);

    try {
      const apps = await invoke<AmplifyApp[]>("list_amplify_apps", {
        profile,
        region,
      });
      // Replace with fresh data
      setAppState("amplifyResources", "apps", [...apps]);
    } catch (e) {
      setError(`Failed to load Amplify apps: ${e}`);
    } finally {
      setIsLoadingApps(false);
    }
  };

  // Load branches when an app is selected
  const loadBranches = async (appId: string) => {
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!profile || !region) return;

    setIsLoadingBranches(true);
    // Clear branches and related state first
    setAppState("amplifyResources", "branches", []);
    setAppState("amplifyResources", "selectedBranch", null);
    setAppState("amplifyResources", "lambdaFunctions", []);

    try {
      const branches = await invoke<AmplifyBranch[]>("list_amplify_branches", {
        profile,
        region,
        appId,
      });

      // Check branch protection for each branch
      const selectedApp = appState.amplifyResources.selectedApp;
      if (selectedApp && selectedApp.repository) {
        const branchesWithProtection = await Promise.all(
          branches.map(async (branch) => {
            try {
              const protection = await invoke<{
                is_protected: boolean;
                protection_info: string | null;
              }>("check_branch_protection", {
                repositoryUrl: selectedApp.repository,
                branchName: branch.branch_name,
              });

              return {
                ...branch,
                is_protected: protection.is_protected,
                protection_info: protection.protection_info,
              };
            } catch (e) {
              console.error(
                `Failed to check protection for branch ${branch.branch_name}:`,
                e,
              );
              return branch; // Return original branch if check fails
            }
          }),
        );

        setAppState("amplifyResources", "branches", [
          ...branchesWithProtection,
        ]);
      } else {
        // No repository URL, can't check protection
        setAppState("amplifyResources", "branches", [...branches]);
      }
    } catch (e) {
      setError(`Failed to load branches: ${e}`);
    } finally {
      setIsLoadingBranches(false);
    }
  };

  // Load Lambda functions when a branch is selected
  const loadLambdaFunctions = async (branch: AmplifyBranch) => {
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;
    const selectedApp = appState.amplifyResources.selectedApp;

    if (!profile || !region || !selectedApp) return;

    setIsLoadingFunctions(true);
    // Clear functions first
    setAppState("amplifyResources", "lambdaFunctions", []);

    try {
      // Get supported versions to determine outdated status
      const supportedVersions = appState.runtimeInfo.supportedVersions
        .filter((v: NodeVersion) => v.is_supported)
        .map((v: NodeVersion) => {
          const match = v.version.match(/v?(\d+)/);
          return match ? parseInt(match[1], 10) : 0;
        })
        .filter((v: number) => v > 0);

      console.log("Supported versions being passed:", supportedVersions);
      console.log("Backend environment name:", branch.backend_environment_name);

      const functions = await invoke<LambdaFunction[]>(
        "get_lambda_functions_with_status",
        {
          profile,
          region,
          appId: selectedApp.app_id,
          branchName: branch.branch_name,
          backendEnvironmentName: branch.backend_environment_name,
          supportedVersions,
        },
      );
      // Replace with fresh data
      setAppState("amplifyResources", "lambdaFunctions", [...functions]);
    } catch (e) {
      setError(`Failed to load Lambda functions: ${e}`);
    } finally {
      setIsLoadingFunctions(false);
    }
  };

  onMount(() => {
    loadSupportedRuntimes();

    // Skip re-fetching apps if data already exists in store
    if (appState.amplifyResources.apps.length > 0) {
      // Data already loaded, no need to fetch again
      return;
    }

    loadApps();
  });

  // Handle app selection
  const handleAppSelect = (app: AmplifyApp) => {
    const previousAppId = appState.amplifyResources.selectedApp?.app_id;

    // Only clear downstream state if app actually changed
    if (app.app_id !== previousAppId) {
      // Clear repository state since we're selecting a different app
      clearDownstreamState(2);
      // Clear any error message
      setError(null);
    }

    // Create a copy to avoid storing a proxy reference
    setAppState("amplifyResources", "selectedApp", { ...app });
    loadBranches(app.app_id);
  };

  // Handle branch selection
  const handleBranchSelect = (branch: AmplifyBranch) => {
    const previousBranchName =
      appState.amplifyResources.selectedBranch?.branch_name;

    // Only clear downstream state if branch actually changed
    if (branch.branch_name !== previousBranchName) {
      // Clear repository state since we're selecting a different branch
      clearDownstreamState(2);
    }

    // Allow selecting protected branches to view functions, but show warning
    if (branch.is_protected) {
      setError(
        `Cannot process protected branch "${branch.branch_name}". ${branch.protection_info || "This branch does not allow direct pushes."} Please select another branch.`,
      );
    } else {
      // Clear any previous error
      setError(null);
    }

    // Create a copy to avoid storing a proxy reference
    setAppState("amplifyResources", "selectedBranch", { ...branch });
    loadLambdaFunctions(branch);
  };

  const canContinue = () => {
    // Must have app and branch selected, and have functions that need updates
    const hasSelection =
      appState.amplifyResources.selectedApp !== null &&
      appState.amplifyResources.selectedBranch !== null;
    const hasFunctions = appState.amplifyResources.lambdaFunctions.length > 0;
    const hasOutdatedFunctions = getOutdatedCount() > 0;

    // Cannot continue if selected branch is protected
    const isProtectedBranch =
      appState.amplifyResources.selectedBranch?.is_protected || false;

    return (
      hasSelection && hasFunctions && hasOutdatedFunctions && !isProtectedBranch
    );
  };

  const handleContinue = () => {
    if (canContinue() && props.onComplete) {
      props.onComplete();
    }
  };

  const handleBack = () => {
    if (props.onBack) {
      props.onBack();
    }
  };

  const getOutdatedCount = () => {
    return appState.amplifyResources.lambdaFunctions.filter(
      (f: LambdaFunction) => f.is_outdated,
    ).length;
  };

  const getNonNodejsCount = () => {
    return appState.amplifyResources.lambdaFunctions.filter(
      (f: LambdaFunction) => !f.runtime.startsWith("nodejs"),
    ).length;
  };

  const isLoading = () =>
    isLoadingApps() || isLoadingBranches() || isLoadingFunctions();

  return (
    <div class="max-w-[800px] mx-auto app-selection-step">
      <h2 class="mb-2 text-2xl font-bold">Select Amplify App & Branch</h2>
      <p class="text-[#666] dark:text-[#999] mb-8">
        Choose the Amplify application and branch you want to update.
      </p>

      {/* Apps Section */}
      <div class="apps-container">
        <div class="flex flex-col gap-2 mb-6">
          <label for="app-select" class="font-semibold text-[0.95rem]">Amplify Application</label>
          <Show
            when={!isLoadingApps()}
            fallback={
              <div class="flex items-center gap-2 p-3 text-[#666] dark:text-[#aaa] text-[0.95rem]">
                <span class="w-4 h-4 border-2 border-[#e0e0e0] border-t-[#396cd8] rounded-full animate-spin"></span>
                Loading Amplify apps...
              </div>
            }
          >
            <Show
              when={appState.amplifyResources.apps.length > 0}
              fallback={
                <div class="text-center p-6 text-[#666] dark:text-[#aaa] bg-[#f5f5f5] dark:bg-[#333] rounded-lg text-sm">
                  No Amplify apps found in {appState.awsConfig.selectedRegion}.
                </div>
              }
            >
              <select
                id="app-select"
                class="p-3 border border-[#ccc] dark:border-[#444] rounded-md text-base bg-white dark:bg-[#2a2a2a] cursor-pointer transition-all duration-200 hover:border-[#396cd8] focus:outline-none focus:border-[#396cd8] focus:ring-3 focus:ring-[rgba(57,108,216,0.15)] disabled:bg-[#f5f5f5] disabled:cursor-not-allowed disabled:text-[#999]"
                value={appState.amplifyResources.selectedApp?.app_id || ""}
                onChange={(e) => {
                  const appId = e.currentTarget.value;
                  const app = appState.amplifyResources.apps.find(
                    (a: AmplifyApp) => a.app_id === appId,
                  );
                  if (app) {
                    handleAppSelect(app);
                  }
                }}
              >
                <option value="">Select an application...</option>
                <For each={appState.amplifyResources.apps}>
                  {(app) => (
                    <option value={app.app_id}>
                      {app.name} ({app.app_id})
                    </option>
                  )}
                </For>
              </select>
            </Show>
          </Show>
          <Show when={appState.amplifyResources.selectedApp?.repository}>
            <p class="text-[0.85rem] text-[#888] m-0">
              Repository:{" "}
              <a
                href={appState.amplifyResources.selectedApp?.repository}
                target="_blank"
                rel="noopener noreferrer"
                class="font-medium text-[#396cd8] hover:underline"
              >
                {appState.amplifyResources.selectedApp?.repository}
              </a>
            </p>
          </Show>
        </div>
      </div>

      {/* Branches Section */}
      <Show when={appState.amplifyResources.selectedApp}>
        <div class="mb-8">
          <h3 class="text-[1.1rem] font-semibold mb-4 text-[#333] dark:text-[#eee]">Branches</h3>
          <Show
            when={!isLoadingBranches()}
            fallback={
              <div class="flex items-center gap-2 p-3 text-[#666] dark:text-[#aaa] text-[0.95rem]">
                <span class="w-4 h-4 border-2 border-[#e0e0e0] border-t-[#396cd8] rounded-full animate-spin"></span>
                Loading branches...
              </div>
            }
          >
            <Show
              when={appState.amplifyResources.branches.length > 0}
              fallback={
                <div class="text-[#888] italic">
                  No branches found for this app.
                </div>
              }
            >
              <div class="flex flex-wrap gap-2 mb-4">
                <For each={appState.amplifyResources.branches}>
                  {(branch) => (
                    <button
                      class={`px-4 py-2 border-2 rounded-[20px] transition-all duration-200 text-[0.9rem] flex items-center gap-2 
                        ${appState.amplifyResources.selectedBranch?.branch_name === branch.branch_name
                          ? "border-[#396cd8] bg-[#396cd8] text-white"
                          : branch.is_protected
                            ? "border-[#f57c00] bg-[#fff8f0] dark:bg-[#3a2a1a] text-[#333] dark:text-[#ffb74d] hover:bg-[#ffe8d0] dark:hover:bg-[#4a3a2a]"
                            : "border-[#e0e0e0] dark:border-[#444] bg-white dark:bg-[#2a2a2a] text-[#333] dark:text-[#eee] hover:border-[#396cd8] hover:bg-[#f8faff] dark:hover:bg-[#333]"}`}
                      onClick={() => handleBranchSelect(branch)}
                      title={
                        branch.is_protected
                          ? branch.protection_info || "Protected branch"
                          : undefined
                      }
                    >
                      {branch.branch_name}
                      {branch.is_protected && (
                        <span class="text-[0.85rem]">🔒</span>
                      )}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </Show>

          {/* Error message for protected branch selection */}
          <Show when={error()}>
            <div class="p-4 rounded-lg my-4 border border-[#fcc] bg-[#fee] text-[#c00] dark:bg-[#3a1a1a] dark:text-[#ff6b6b] dark:border-[#f44336]">{error()}</div>
          </Show>
        </div>
      </Show>

      {/* Lambda Functions Section */}
      <Show when={appState.amplifyResources.selectedBranch}>
        <div class="mb-8">
          <h3 class="text-[1.1rem] font-semibold mb-4 text-[#333] dark:text-[#eee]">Lambda Functions Runtime Analysis</h3>
          <Show
            when={!isLoadingFunctions()}
            fallback={
              <div class="flex items-center gap-2 p-3 text-[#666] dark:text-[#aaa] text-[0.95rem]">
                <span class="w-4 h-4 border-2 border-[#e0e0e0] border-t-[#396cd8] rounded-full animate-spin"></span>
                Analyzing Lambda functions...
              </div>
            }
          >
            <Show
              when={appState.amplifyResources.lambdaFunctions.length > 0}
              fallback={
                <div class="text-center p-6 text-[#666] dark:text-[#aaa] bg-[#f5f5f5] dark:bg-[#333] rounded-lg text-sm">
                  No Lambda functions found for this branch.
                </div>
              }
            >
              <div class="flex flex-col gap-3">
                <For each={appState.amplifyResources.lambdaFunctions}>
                  {(func) => {
                    const isNodejs = func.runtime.startsWith("nodejs");

                    let statusClasses = "border-[#e0e0e0] dark:border-[#444]";
                    if (!isNodejs) {
                      statusClasses = "border-[#9e9e9e] bg-[#fafafa] dark:bg-[#2a2a2a] dark:border-[#757575]";
                    } else if (func.is_outdated) {
                      statusClasses = "border-[#f57c00] bg-[#fff8f0] dark:bg-[#3a2a1a] dark:border-[#f57c00]";
                    } else {
                      statusClasses = "border-[#4caf50] bg-[#f0f8f0] dark:bg-[#1a2a1a] dark:border-[#4caf50]";
                    }

                    return (
                      <div
                        class={`border rounded-lg p-4 flex justify-between items-center ${statusClasses}`}
                        title={func.description || undefined}
                      >
                        <div class="flex-1">
                          <div class="font-semibold text-[0.95rem] text-[#333] dark:text-[#eee] mb-1 flex flex-col items-start gap-2">
                            {func.name}
                            <Show
                              when={func.is_auto_managed}
                              fallback={
                                <span
                                  class="inline-block px-2 py-0.5 text-[0.7rem] bg-[#fbe9cb] text-[#e65100] dark:bg-[#3a2a1a] dark:text-[#ffb74d] rounded-[10px] font-medium"
                                  title="This function is custom function"
                                >
                                  Custom Function
                                </span>
                              }
                            >
                              <span
                                class="inline-block px-2 py-0.5 text-[0.7rem] bg-[#e3f2fd] text-[#1976d2] dark:bg-[#1a3a5c] dark:text-[#64b5f6] rounded-[10px] font-medium"
                                title="This function is auto-managed by Amplify and will be updated when you upgrade Amplify CLI (Gen1) or backend dependencies (Gen2)"
                              >
                                Auto Managed
                              </span>
                            </Show>
                          </div>
                        </div>
                        <div class="flex flex-col items-end gap-1">
                          <span class={`px-3 py-1 rounded-md text-[0.85rem] font-mono text-white 
                            ${!isNodejs ? "bg-[#9e9e9e]" : func.is_outdated ? "bg-[#f57c00]" : "bg-[#4caf50]"}`}>
                            {func.runtime}
                          </span>
                          <Show when={!isNodejs}>
                            <span class="text-[0.8rem] text-[#757575] dark:text-[#9e9e9e] italic">
                              <Show
                                when={!func.is_auto_managed}
                                fallback="Will update with Amplify upgrade"
                              >
                                Not a Node.js runtime
                              </Show>
                            </span>
                          </Show>
                          <Show
                            when={
                              isNodejs &&
                              func.is_outdated &&
                              appState.runtimeInfo.targetRuntime
                            }
                          >
                            <span class="text-[0.8rem] text-[#666] dark:text-[#aaa]">
                              <Show
                                when={func.is_auto_managed}
                                fallback={
                                  <>
                                    Recommended:{" "}
                                    <strong class="text-[#4caf50] dark:text-[#66bb6a]">
                                      {appState.runtimeInfo.targetRuntime}
                                    </strong>
                                  </>
                                }
                              >
                                Will update with Amplify upgrade
                              </Show>
                            </span>
                          </Show>
                          <Show when={isNodejs && !func.is_outdated}>
                            <span class="text-[0.8rem] text-[#2e7d32] dark:text-[#66bb6a]">
                              ✓ Supported
                            </span>
                          </Show>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>

              {/* Summary */}
              <div class="mt-4 p-3 bg-[#f5f5f5] dark:bg-[#2a2a2a] rounded-md flex flex-wrap gap-4 text-[0.9rem]">
                <Show when={getOutdatedCount() > 0}>
                  <span class="text-[#e65100] dark:text-[#ffb74d]">
                    {getOutdatedCount()} function(s) with outdated Node.js
                    runtime
                  </span>
                </Show>
                <Show when={getNonNodejsCount() > 0}>
                  <span class="text-[#757575] dark:text-[#9e9e9e]">
                    {getNonNodejsCount()} non-Node.js function(s)
                  </span>
                </Show>
                <Show
                  when={getOutdatedCount() === 0 && getNonNodejsCount() === 0}
                >
                  <span class="text-[#2e7d32] dark:text-[#66bb6a]">
                    ✓ All Node.js functions are using supported runtimes
                  </span>
                </Show>
              </div>
            </Show>
          </Show>
        </div>
      </Show>

      {/* Selection Summary */}
      <Show
        when={
          appState.amplifyResources.selectedApp &&
          appState.amplifyResources.selectedBranch
        }
      >
        <div class="flex items-center gap-3 p-4 bg-[#f0f8f0] border border-[#4caf50] rounded-lg mb-8">
          <span class="w-6 h-6 bg-[#4caf50] text-white rounded-full flex items-center justify-center font-bold text-sm">✓</span>
          <span class="dark:text-[#2e7d32]">
            Selected{" "}
            <strong>{appState.amplifyResources.selectedApp?.name}</strong> /{" "}
            <strong>
              {appState.amplifyResources.selectedBranch?.branch_name}
            </strong>
            <Show when={getOutdatedCount() > 0}>
              {" "}
              — {getOutdatedCount()} function(s) need runtime updates
            </Show>
          </span>
        </div>
      </Show>

      <div class="flex justify-between gap-4 mt-8">
        <button
          onClick={handleBack}
          class="bg-white dark:bg-[#2a2a2a] text-[#396cd8] dark:text-[#64b5f6] border border-[#396cd8] dark:border-[#1e3a8a] px-8 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#396cd8] dark:hover:bg-[#1e3a8a] hover:text-white dark:hover:text-white disabled:opacity-40 disabled:grayscale disabled:cursor-not-allowed shadow-sm active:scale-95"
          disabled={isLoadingBranches() || isLoadingFunctions()}
        >
          Back
        </button>
        <button
          onClick={handleContinue}
          class="bg-[#396cd8] dark:bg-[#3b82f6] text-white border-none px-12 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#2d5bb8] dark:hover:bg-[#2563eb] disabled:bg-[#eee] dark:disabled:bg-[#333] disabled:text-[#999] dark:disabled:text-[#666] disabled:cursor-not-allowed active:scale-95 flex items-center gap-2 group"
          disabled={!canContinue() || isLoading()}
        >
          Continue
          <svg class="w-5 h-5 transition-transform group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
          </svg>
        </button>
      </div>

      <Show
        when={
          !canContinue() &&
          !isLoading() &&
          appState.amplifyResources.selectedBranch
        }
      >
        <p class="p-4 rounded-lg my-4 border border-[#ffcc80] bg-[#fff3e0] text-[#f57c00] dark:bg-[#3a2a1a] dark:text-[#ffb74d] dark:border-[#f57c00]">
          <Show
            when={appState.amplifyResources.lambdaFunctions.length === 0}
            fallback="All functions are using supported runtimes. No updates needed."
          >
            No Lambda functions found for this branch.
          </Show>
        </p>
      </Show>

      <Show
        when={
          !appState.amplifyResources.selectedApp ||
          !appState.amplifyResources.selectedBranch
        }
      >
        <Show when={!isLoading()}>
          <p class="p-4 rounded-lg my-4 border border-[#ffcc80] bg-[#fff3e0] text-[#f57c00] dark:bg-[#3a2a1a] dark:text-[#ffb74d] dark:border-[#f57c00]">
            Please select an app and branch to continue.
          </p>
        </Show>
      </Show>
    </div>
  );
}

export default AppSelectionStep;
