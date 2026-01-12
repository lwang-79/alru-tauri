import { createSignal, Show, For, onMount, createEffect } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CloneResult,
  PackageManager,
  BackendType,
  UpdateResult,
  BuildResult,
  BuildStatus,
  FileChange,
  SandboxResult,
  UpgradeResult,
  UpdateEnvVarResult,
  BuildConfigUpdateResult,
} from "../types";
import { appState, setAppState } from "../store/appStore";
import "./shared.css";
import "./CloneUpdateStep.css";

interface CloneUpdateStepProps {
  onComplete?: () => void;
  onBack?: () => void;
}

type OperationStatus = "pending" | "running" | "success" | "failed";

export function CloneUpdateStep(props: CloneUpdateStepProps) {
  // Operation states
  const [cloneStatus, setCloneStatus] =
    createSignal<OperationStatus>("pending");
  const [prepareStatus, setPrepareStatus] =
    createSignal<OperationStatus>("pending");
  const [updateStatus, setUpdateStatus] =
    createSignal<OperationStatus>("pending");

  // Build verification status - use store state for persistence (this is step 6b)
  const buildStatus = () => appState.repository.gen2BuildVerificationStatus;
  const setBuildStatus = (status: BuildStatus) =>
    setAppState("repository", "gen2BuildVerificationStatus", status);

  const [envVarStatus, setEnvVarStatus] =
    createSignal<OperationStatus>("pending");
  const [buildConfigStatus, setBuildConfigStatus] =
    createSignal<OperationStatus>("pending");
  const [gen2EnvVarStatus, setGen2EnvVarStatus] =
    createSignal<OperationStatus>("pending");

  // Error messages
  const [cloneError, setCloneError] = createSignal<string | null>(null);
  const [prepareError, setPrepareError] = createSignal<string | null>(null);
  const [prepareOutput, setPrepareOutput] = createSignal<string>("");
  const [updateError, setUpdateError] = createSignal<string | null>(null);
  const [upgradeMessage, setUpgradeMessage] = createSignal<string | null>(null);
  const [buildError, setBuildError] = createSignal<string | null>(null);
  const [envVarError, setEnvVarError] = createSignal<string | null>(null);
  const [envVarMessage, setEnvVarMessage] = createSignal<string | null>(null);
  const [buildConfigError, setBuildConfigError] = createSignal<string | null>(
    null,
  );
  const [buildConfigMessage, setBuildConfigMessage] = createSignal<
    string | null
  >(null);
  const [gen2EnvVarError, setGen2EnvVarError] = createSignal<string | null>(
    null,
  );
  const [gen2EnvVarMessage, setGen2EnvVarMessage] = createSignal<string | null>(
    null,
  );

  // Build output
  const [buildOutput, setBuildOutput] = createSignal<string>("");

  // Copy path feedback
  const [pathCopied, setPathCopied] = createSignal(false);

  // Gen2 sandbox option - use store state for persistence
  const gen2SandboxEnabled = () => appState.repository.gen2SandboxEnabled;
  const setGen2SandboxEnabled = (enabled: boolean) =>
    setAppState("repository", "gen2SandboxEnabled", enabled);

  // Gen2 sandbox status - use store state for persistence
  const sandboxStatus = () => appState.repository.gen2SandboxStatus;
  const setSandboxStatus = (status: BuildStatus) =>
    setAppState("repository", "gen2SandboxStatus", status);

  const [sandboxError, setSandboxError] = createSignal<string | null>(null);
  const [sandboxOutput, setSandboxOutput] = createSignal<string>("");

  // Function to check if any operation is currently running
  const isAnyOperationRunning = () => {
    return (
      cloneStatus() === "running" ||
      prepareStatus() === "running" ||
      updateStatus() === "running" ||
      buildConfigStatus() === "running" ||
      gen2EnvVarStatus() === "running" ||
      sandboxStatus() === "running" ||
      buildStatus() === "running" ||
      envVarStatus() === "running"
    );
  };

  // Sync running state to store for global access
  createEffect(() => {
    const isRunning = isAnyOperationRunning();
    setAppState("repository", "isOperationRunning", isRunning);
  });

  // Function to handle navigation - delegate to parent
  const handleBack = () => {
    // Don't allow navigation if any operation is running
    if (isAnyOperationRunning()) {
      return;
    }
    // Just call the parent's onBack - let App.tsx handle cleanup dialogs
    props.onBack?.();
  };

  // Restore operation states from store on mount
  onMount(() => {
    const opStatus = appState.repository.operationStatus;

    // Restore states based on what was completed before
    if (opStatus.cloneComplete) {
      setCloneStatus("success");
    }
    if (opStatus.prepareComplete) {
      setPrepareStatus("success");
    }
    if (opStatus.updateComplete) {
      setUpdateStatus("success");
    }
    if (opStatus.buildConfigComplete) {
      setBuildConfigStatus("success");
    }
    if (opStatus.buildComplete) {
      setBuildStatus("success");
    }
    if (opStatus.envVarComplete) {
      setEnvVarStatus("success");
    }
    if ((opStatus as any).gen2EnvVarComplete) {
      setGen2EnvVarStatus("success");
    }

    // Force layout recalculation to prevent empty/half-page rendering
    // This is a workaround for SolidJS reactive timing issues
    setTimeout(() => {
      const element = document.querySelector(".clone-update-step");
      if (element) {
        // Trigger a reflow by reading a layout property
        element.getBoundingClientRect();
      }
    }, 10);
  });

  // Auto-scroll prepare output when new content arrives
  createEffect(() => {
    const output = prepareOutput();
    if (output) {
      const pre = document.getElementById("prepare-output-pre");
      if (pre) {
        pre.scrollTop = pre.scrollHeight;
      }
    }
  });

  // Auto-scroll sandbox output when new content arrives
  createEffect(() => {
    const output = sandboxOutput();
    if (output) {
      const pre = document.getElementById("sandbox-output-pre");
      if (pre) {
        pre.scrollTop = pre.scrollHeight;
      }
    }
  });

  // Auto-scroll build output when new content arrives
  createEffect(() => {
    const output = buildOutput();
    if (output) {
      const pre = document.getElementById("build-output-pre");
      if (pre) {
        pre.scrollTop = pre.scrollHeight;
      }
    }
  });

  // Reset all local state when repository is cleaned up
  createEffect(() => {
    const clonePath = appState.repository.clonePath;

    // If clonePath becomes null, it means the repository was cleaned up
    // Reset all local signals to their initial state
    if (clonePath === null) {
      console.log(
        "[CloneUpdateStep] Repository cleaned up, resetting local state",
      );

      // Reset operation statuses
      setCloneStatus("pending");
      setPrepareStatus("pending");
      setUpdateStatus("pending");
      setEnvVarStatus("pending");
      setBuildConfigStatus("pending");
      setGen2EnvVarStatus("pending");

      // Reset error messages
      setCloneError(null);
      setPrepareError(null);
      setPrepareOutput("");
      setUpdateError(null);
      setUpgradeMessage(null);
      setBuildError(null);
      setEnvVarError(null);
      setEnvVarMessage(null);
      setBuildConfigError(null);
      setBuildConfigMessage(null);
      setGen2EnvVarError(null);
      setGen2EnvVarMessage(null);

      // Reset output
      setBuildOutput("");
      setSandboxError(null);
      setSandboxOutput("");

      // Reset copy feedback
      setPathCopied(false);
    }
  });

  // Check if error is a permission/authentication issue
  const isPermissionError = (error: string): boolean => {
    const permissionPatterns = [
      "Permission denied",
      "permission denied",
      "Authentication failed",
      "authentication failed",
      "could not read Username",
      "Could not read from remote repository",
      "fatal: repository",
      "not found",
      "access denied",
      "Access denied",
      "403",
      "401",
      "Invalid username or password",
      "Host key verification failed",
      "publickey",
    ];
    return permissionPatterns.some((pattern) => error.includes(pattern));
  };

  // Format clone error with helpful guidance
  const formatCloneError = (error: string, repoUrl: string): string => {
    if (isPermissionError(error)) {
      const isHttps = repoUrl.startsWith("https://");
      const isSsh = repoUrl.startsWith("git@") || repoUrl.includes("ssh://");

      let guidance = `Git clone failed due to permission/authentication issue.\n\n`;
      guidance += `Repository: ${repoUrl}\n\n`;
      guidance += `Please configure git access on your local machine:\n\n`;

      if (isHttps) {
        guidance += `For HTTPS repositories:\n`;
        guidance += `• Ensure you have access to the repository\n`;
        guidance += `• Configure git credentials: git config --global credential.helper store\n`;
        guidance += `• Or use a personal access token for authentication\n`;
        guidance += `• Try cloning manually first: git clone ${repoUrl}\n`;
      } else if (isSsh) {
        guidance += `For SSH repositories:\n`;
        guidance += `• Ensure your SSH key is added to your git provider\n`;
        guidance += `• Check SSH agent: ssh-add -l\n`;
        guidance += `• Add your key: ssh-add ~/.ssh/id_rsa (or your key path)\n`;
        guidance += `• Test connection: ssh -T git@github.com (or your provider)\n`;
        guidance += `• Try cloning manually first: git clone ${repoUrl}\n`;
      } else {
        guidance += `• Ensure you have access to the repository\n`;
        guidance += `• Try cloning manually first: git clone ${repoUrl}\n`;
      }

      return guidance;
    }
    return error;
  };

  // Clone repository and detect configuration
  const handleClone = async () => {
    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;

    if (!selectedApp || !selectedBranch) {
      setCloneError("No app or branch selected");
      return;
    }

    setCloneStatus("running");
    setCloneError(null);
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "cloneComplete", false);

    try {
      // Step 1: Clone repository
      const result = await invoke<CloneResult>("clone_repository", {
        url: selectedApp.repository,
        branch: selectedBranch.branch_name,
      });

      if (!result.success) {
        const formattedError = formatCloneError(
          result.error || "Clone failed",
          selectedApp.repository,
        );
        setCloneError(formattedError);
        setCloneStatus("failed");
        return;
      }

      setAppState("repository", "clonePath", result.path);

      // Step 2: Automatically detect configuration (non-mutation operation)
      try {
        // Detect package manager
        const packageManager = await invoke<PackageManager>(
          "detect_package_manager",
          {
            projectPath: result.path,
          },
        );
        setAppState("repository", "packageManager", packageManager);

        // Detect backend type
        const backendType = await invoke<BackendType>("detect_backend_type", {
          projectPath: result.path,
        });
        setAppState("repository", "backendType", backendType);

        setCloneStatus("success");
        setAppState("repository", "operationStatus", "cloneComplete", true);
      } catch (e) {
        setCloneError(
          `Clone succeeded but configuration detection failed: ${String(e)}`,
        );
        setCloneStatus("failed");
      }
    } catch (e) {
      const formattedError = formatCloneError(
        String(e),
        selectedApp.repository,
      );
      setCloneError(formattedError);
      setCloneStatus("failed");
    }
  };

  // Prepare project: install dependencies, setup Amplify env, and upgrade packages
  const handlePrepare = async () => {
    const clonePath = appState.repository.clonePath;
    const packageManager = appState.repository.packageManager;
    const backendType = appState.repository.backendType;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const selectedApp = appState.amplifyResources.selectedApp;

    if (!clonePath || !packageManager) {
      setPrepareError("Missing required information for preparation");
      return;
    }

    setPrepareStatus("running");
    setPrepareError(null);
    setPrepareOutput("");
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "prepareComplete", false);
    setAppState("repository", "operationStatus", "upgradeComplete", false);

    // Set up event listeners for streaming output
    let unlistenOutput: UnlistenFn | null = null;
    let unlistenStatus: UnlistenFn | null = null;

    try {
      // Listen for output events
      unlistenOutput = await listen<string>("prepare-output", (event) => {
        setPrepareOutput((prev) => prev + event.payload);
      });

      // Listen for status events
      unlistenStatus = await listen<string>("prepare-status", (event) => {
        if (event.payload === "completed") {
          // Don't set success yet, we may have more steps
        } else if (event.payload === "failed") {
          setPrepareStatus("failed");
        }
      });

      if (backendType === "Gen1") {
        // Gen1: Install dependencies first, then Amplify setup, then CLI upgrade
        console.log(
          "[handlePrepare] Gen1: Starting with dependency installation",
        );

        // Step 1: Install dependencies with streaming
        await invoke<boolean>("install_dependencies_streaming", {
          projectPath: clonePath,
          packageManager: packageManager,
        });

        console.log(
          "[handlePrepare] Gen1: Dependencies installed, proceeding with Amplify setup",
        );

        // Step 2: Run amplify pull and env checkout
        if (selectedBranch && selectedApp) {
          try {
            // First run amplify pull to initialize the project with streaming
            // Use backend_environment_name instead of branch_name as they can be different
            await invoke<boolean>("amplify_pull_streaming", {
              projectPath: clonePath,
              appId: selectedApp.app_id,
              envName: selectedBranch.backend_environment_name,
              backendType: backendType,
              profileName: appState.awsConfig.selectedProfile,
            });

            // Then run amplify env checkout to switch to the correct environment with streaming
            await invoke<boolean>("amplify_env_checkout_streaming", {
              projectPath: clonePath,
              envName: selectedBranch.backend_environment_name,
            });
          } catch (e) {
            // If amplify commands fail, show a warning but don't fail the whole step
            console.warn("Amplify setup warning:", e);
            setPrepareError(
              `Warning: ${String(e)}\n\nYou may need to manually run the amplify pull command if there are issues.`,
            );
            // Still mark as success since dependencies are installed
            setPrepareStatus("success");
            setAppState(
              "repository",
              "operationStatus",
              "prepareComplete",
              true,
            );
            setAppState(
              "repository",
              "operationStatus",
              "upgradeComplete",
              true,
            );
            return;
          }
        }

        // Step 3: Upgrade Amplify CLI globally for Gen1
        const result = await invoke<UpgradeResult>("upgrade_amplify_cli");
        if (result.skipped) {
          setUpgradeMessage(result.message);
        } else {
          setUpgradeMessage(`Upgraded to version ${result.latest_version}`);
        }
      } else {
        // Gen2: Upgrade Amplify packages first (includes full install), more efficient
        console.log(
          "[handlePrepare] Gen2: Starting with Amplify package upgrade",
        );

        // Step 1: Upgrade @aws-amplify/backend packages (includes full dependency install)
        await invoke<boolean>("upgrade_amplify_backend_packages", {
          projectPath: clonePath,
          packageManager: packageManager,
        });

        console.log("[handlePrepare] Gen2: Amplify package upgrade completed");

        // No need for separate install_dependencies call since upgrade_amplify_backend_packages
        // already runs a full install to update all dependencies and the lock file
      }

      setPrepareStatus("success");
      setAppState("repository", "operationStatus", "prepareComplete", true);
      // Also mark upgrade as complete since it's now part of prepare
      setAppState("repository", "operationStatus", "upgradeComplete", true);
    } catch (e) {
      setPrepareError(String(e));
      setPrepareStatus("failed");
      // Don't mark as complete if it failed
    } finally {
      // Clean up listeners
      if (unlistenOutput) unlistenOutput();
      if (unlistenStatus) unlistenStatus();
    }
  };

  // Update backend runtime
  const handleUpdate = async () => {
    const clonePath = appState.repository.clonePath;
    const backendType = appState.repository.backendType;
    const targetRuntime = appState.runtimeInfo.targetRuntime;
    const selectedApp = appState.amplifyResources.selectedApp;

    if (!clonePath || !backendType || !targetRuntime) {
      setUpdateError("Missing required information for update");
      return;
    }

    setUpdateStatus("running");
    setUpdateError(null);
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "updateComplete", false);

    try {
      let result: UpdateResult;

      if (backendType === "Gen2") {
        result = await invoke<UpdateResult>("update_gen2_backend", {
          projectPath: clonePath,
          targetRuntime: targetRuntime,
        });
      } else {
        result = await invoke<UpdateResult>("update_gen1_backend", {
          projectPath: clonePath,
          targetRuntime: targetRuntime,
          appEnvVars: selectedApp?.environment_variables || {},
        });
      }

      if (result.success) {
        setAppState("repository", "changes", result.changes);
        setUpdateStatus("success");
        setAppState("repository", "operationStatus", "updateComplete", true);
      } else {
        setUpdateError(result.error || "Update failed");
        setUpdateStatus("failed");
      }
    } catch (e) {
      setUpdateError(String(e));
      setUpdateStatus("failed");
    }
  };

  // Update Gen2 build configuration
  const handleBuildConfigUpdate = async () => {
    const clonePath = appState.repository.clonePath;
    const selectedApp = appState.amplifyResources.selectedApp;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!clonePath || !selectedApp || !profile || !region) {
      setBuildConfigError(
        "Missing required information for build config update",
      );
      return;
    }

    setBuildConfigStatus("running");
    setBuildConfigError(null);
    setBuildConfigMessage(null);
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "buildConfigComplete", false);

    try {
      const buildConfigResult = await invoke<BuildConfigUpdateResult>(
        "update_gen2_build_config",
        {
          repoPath: clonePath,
          profile: profile,
          region: region,
          appId: selectedApp.app_id,
        },
      );

      if (buildConfigResult.error) {
        // Display error prominently
        setBuildConfigError(buildConfigResult.error);
        setBuildConfigStatus("failed");
      } else {
        if (buildConfigResult.updated && buildConfigResult.change) {
          // Store build config change in app state
          setAppState(
            "repository",
            "buildConfigChange",
            buildConfigResult.change,
          );

          // If cloud buildSpec was updated, store original for potential revert
          if (
            buildConfigResult.change.location === "Cloud" &&
            buildConfigResult.original_build_spec
          ) {
            setAppState(
              "repository",
              "originalBuildSpec",
              buildConfigResult.original_build_spec,
            );
          }
        }

        setBuildConfigMessage(buildConfigResult.message);
        setBuildConfigStatus("success");
        setAppState(
          "repository",
          "operationStatus",
          "buildConfigComplete",
          true,
        );
      }
    } catch (e) {
      console.error("Build config update failed:", e);
      setBuildConfigError(`Build configuration update failed: ${String(e)}`);
      setBuildConfigStatus("failed");
    }
  };

  // Deploy Gen2 sandbox
  const handleSandboxDeploy = async () => {
    const clonePath = appState.repository.clonePath;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!clonePath || !profile || !region) {
      setSandboxError("Missing required information for sandbox deployment");
      return;
    }

    setSandboxStatus("running");
    setSandboxError(null);
    setSandboxOutput("");

    // Set up event listeners for streaming output
    let unlistenOutput: UnlistenFn | null = null;
    let unlistenStatus: UnlistenFn | null = null;

    try {
      // Listen for output events
      unlistenOutput = await listen<string>("sandbox-output", (event) => {
        setSandboxOutput((prev) => prev + event.payload);
      });

      // Listen for status events
      unlistenStatus = await listen<string>("sandbox-status", (event) => {
        if (event.payload === "completed") {
          setSandboxStatus("success");
          setAppState("repository", "sandboxDeployed", true);
        } else if (event.payload === "failed" || event.payload === "timeout") {
          setSandboxStatus("failed");
        }
      });

      // Start the sandbox deployment
      const result = await invoke<SandboxResult>("deploy_gen2_sandbox", {
        projectPath: clonePath,
        profile: profile,
        region: region,
      });

      // Final result handling
      if (result.success) {
        setSandboxStatus("success");
        setAppState("repository", "sandboxDeployed", true);
      } else {
        setSandboxError(result.error || "Sandbox deployment failed");
        setSandboxStatus("failed");
      }
    } catch (e) {
      setSandboxError(String(e));
      setSandboxStatus("failed");
    } finally {
      // Clean up listeners
      if (unlistenOutput) unlistenOutput();
      if (unlistenStatus) unlistenStatus();
    }
  };

  // Run build verification
  const handleBuild = async () => {
    const clonePath = appState.repository.clonePath;
    const packageManager = appState.repository.packageManager;
    const backendType = appState.repository.backendType;

    if (!clonePath || !packageManager || !backendType) {
      setBuildError("Missing required information for build");
      return;
    }

    setBuildStatus("running");
    setBuildError(null);
    setBuildOutput("");
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "buildComplete", false);

    // Set up event listeners for streaming output
    let unlistenOutput: UnlistenFn | null = null;
    let unlistenStatus: UnlistenFn | null = null;

    try {
      // Listen for output events
      unlistenOutput = await listen<string>("build-output", (event) => {
        setBuildOutput((prev) => prev + event.payload);
      });

      // Listen for status events
      unlistenStatus = await listen<string>("build-status", (event) => {
        if (event.payload === "completed") {
          setBuildStatus("success");
        } else if (event.payload === "failed") {
          setBuildStatus("failed");
        }
      });

      // Run build with backend type for appropriate backend build command
      const result = await invoke<BuildResult>("run_build", {
        projectPath: clonePath,
        packageManager: packageManager,
        backendType: backendType,
      });

      // Final result handling
      if (result.success) {
        setAppState("repository", "buildStatus", "success");
        setBuildStatus("success");
        setAppState("repository", "operationStatus", "buildComplete", true);
      } else {
        setBuildError(result.error || "Build failed");
        setAppState("repository", "buildStatus", "failed");
        setBuildStatus("failed");
      }
    } catch (e) {
      setBuildError(String(e));
      setAppState("repository", "buildStatus", "failed");
      setBuildStatus("failed");
    } finally {
      // Clean up listeners
      if (unlistenOutput) unlistenOutput();
      if (unlistenStatus) unlistenStatus();
    }
  };

  // Update environment variable for Gen1 apps
  const handleEnvVarUpdate = async () => {
    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!selectedApp || !selectedBranch || !profile || !region) {
      setEnvVarError(
        "Missing required information for environment variable update",
      );
      return;
    }

    setEnvVarStatus("running");
    setEnvVarError(null);
    setEnvVarMessage(null);
    // Clear complete status when starting
    setAppState("repository", "operationStatus", "envVarComplete", false);

    try {
      const result = await invoke<UpdateEnvVarResult>(
        "update_live_updates_env_var",
        {
          profile: profile,
          region: region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
          currentEnvVars: selectedApp.environment_variables,
          branchEnvVars: selectedBranch.environment_variables,
        },
      );

      if (result.success) {
        setEnvVarStatus("success");
        setEnvVarMessage(result.message);
        setAppState("repository", "operationStatus", "envVarComplete", true);

        // Store environment variable changes in app state for potential revert
        if (result.changes && result.changes.length > 0) {
          setAppState("repository", "envVarChanges", result.changes);
        }
      } else {
        setEnvVarError("Failed to update environment variable");
        setEnvVarStatus("failed");
      }
    } catch (e) {
      setEnvVarError(String(e));
      setEnvVarStatus("failed");
    }
  };

  // Update environment variable for Gen2 apps
  const handleGen2EnvVarUpdate = async () => {
    const selectedApp = appState.amplifyResources.selectedApp;
    const selectedBranch = appState.amplifyResources.selectedBranch;
    const profile = appState.awsConfig.selectedProfile;
    const region = appState.awsConfig.selectedRegion;

    if (!selectedApp || !selectedBranch || !profile || !region) {
      setGen2EnvVarError(
        "Missing required information for environment variable update",
      );
      return;
    }

    setGen2EnvVarStatus("running");
    setGen2EnvVarError(null);
    setGen2EnvVarMessage(null);
    // Clear complete status when starting
    setAppState(
      "repository",
      "operationStatus",
      "gen2EnvVarComplete" as any,
      false,
    );

    try {
      const result = await invoke<UpdateEnvVarResult>(
        "update_custom_image_env_var",
        {
          profile: profile,
          region: region,
          appId: selectedApp.app_id,
          branchName: selectedBranch.branch_name,
          currentEnvVars: selectedApp.environment_variables,
          branchEnvVars: selectedBranch.environment_variables,
        },
      );

      if (result.success) {
        setGen2EnvVarStatus("success");
        setGen2EnvVarMessage(result.message);
        setAppState(
          "repository",
          "operationStatus",
          "gen2EnvVarComplete" as any,
          true,
        );

        // Store environment variable changes in app state for potential revert
        if (result.changes && result.changes.length > 0) {
          // Append to existing changes or create new array
          const existingChanges = appState.repository.envVarChanges || [];
          setAppState("repository", "envVarChanges", [
            ...existingChanges,
            ...result.changes,
          ]);
        }
      } else {
        setGen2EnvVarError("Failed to update environment variable");
        setGen2EnvVarStatus("failed");
      }
    } catch (e) {
      setGen2EnvVarError(String(e));
      setGen2EnvVarStatus("failed");
    }
  };

  // For Gen2, user can continue without build test
  const canContinue = () => {
    // Don't allow continue if any operation is running
    if (isAnyOperationRunning()) {
      return false;
    }

    const backendType = appState.repository.backendType;
    // Gen1 requires build to pass and env var update
    if (backendType === "Gen1") {
      return buildStatus() === "success" && envVarStatus() === "success";
    }
    // Gen2 requires prepare (which includes upgrade), build config update, and env var update (build is optional)
    return (
      prepareStatus() === "success" &&
      buildConfigStatus() === "success" &&
      gen2EnvVarStatus() === "success"
    );
  };

  const handleContinue = () => {
    // Don't allow continue if any operation is running
    if (isAnyOperationRunning()) {
      return;
    }
    if (canContinue() && props.onComplete) {
      props.onComplete();
    }
  };

  const getPackageManagerDisplay = (pm: PackageManager | null) => {
    if (!pm) return "Unknown";
    const map: Record<PackageManager, string> = {
      Npm: "npm",
      Yarn: "yarn",
      Pnpm: "pnpm",
      Bun: "bun",
    };
    return map[pm] || pm;
  };

  const getBackendTypeDisplay = (bt: BackendType | null) => {
    if (!bt) return "Unknown";
    return bt === "Gen2" ? "Gen 2" : "Gen 1";
  };

  const getChangeTypeDisplay = (changeType: string) => {
    const map: Record<string, string> = {
      runtime_update: "Runtime Update",
      dependency_update: "Dependency Update",
      env_update: "Environment Variable Update",
      code_comment_update: "Code Comment Update",
    };
    return map[changeType] || changeType;
  };

  // Copy path to clipboard
  const copyPathToClipboard = async () => {
    const path = appState.repository.clonePath;
    if (path) {
      try {
        await navigator.clipboard.writeText(path);
        setPathCopied(true);
        setTimeout(() => setPathCopied(false), 2000);
      } catch (e) {
        console.error("Failed to copy path:", e);
      }
    }
  };

  return (
    <div class="step-container wide clone-update-step">
      <h2>Clone & Update</h2>
      <p class="step-description">
        Clone the repository, detect project configuration, and update runtime
        settings.
      </p>

      {/* Selected App/Branch Summary */}
      <div class="info-bar-balanced">
        <div class="info-bar-left-balanced">
          <div class="info-item-balanced">
            <span class="info-label-balanced">App:</span>
            <span class="info-value-balanced">
              {appState.amplifyResources.selectedApp?.name}
            </span>
          </div>
          <div class="info-item-balanced">
            <span class="info-label-balanced">Branch:</span>
            <span class="info-value-balanced">
              {appState.amplifyResources.selectedBranch?.branch_name}
            </span>
          </div>
        </div>
        <div class="info-bar-right-balanced">
          <div class="info-item-balanced">
            <span class="info-label-balanced">Target Runtime:</span>
            <span class="badge-balanced runtime">
              {appState.runtimeInfo.targetRuntime}
            </span>
          </div>
        </div>
      </div>

      {/* Operations */}
      <div class="operations-container">
        {/* Step 1: Clone Repository & Detect Configuration */}
        <div class={`operation-card ${cloneStatus()}`}>
          <div class="operation-header">
            <div class="operation-number">1</div>
            <div class="operation-info">
              <h3>Clone Repository</h3>
              <p>Clone the repository and detect project configuration</p>
            </div>
            <div class="operation-status">
              <Show when={cloneStatus() === "pending"}>
                <button class="action-button" onClick={handleClone}>
                  Clone
                </button>
              </Show>
              <Show when={cloneStatus() === "running"}>
                <span class="status-indicator running">
                  <span class="spinner-small"></span>
                  Cloning...
                </span>
              </Show>
              <Show when={cloneStatus() === "success"}>
                <span class="status-indicator success">✓ Cloned</span>
              </Show>
              <Show when={cloneStatus() === "failed"}>
                <span class="status-indicator failed">✗ Failed</span>
              </Show>
            </div>
          </div>
          <Show when={cloneError()}>
            <div class="operation-error permission-error">
              <pre class="error-message-text">{cloneError()}</pre>
            </div>
          </Show>
          <Show when={cloneStatus() === "failed"}>
            <div class="operation-retry-row">
              <button class="retry-link" onClick={handleClone}>
                Retry Clone
              </button>
            </div>
          </Show>
          <Show
            when={cloneStatus() === "success" && appState.repository.clonePath}
          >
            <div class="result-balanced">
              <h4>Repository Details</h4>
              <div class="result-row-balanced">
                <div class="result-item-balanced half-width">
                  <span class="result-item-label-balanced">
                    Package Manager:
                  </span>
                  <span class="badge-balanced type">
                    {getPackageManagerDisplay(
                      appState.repository.packageManager,
                    )}
                  </span>
                </div>
                <div class="result-item-balanced half-width">
                  <span class="result-item-label-balanced">Backend Type:</span>
                  <span class="badge-balanced type">
                    {getBackendTypeDisplay(appState.repository.backendType)}
                  </span>
                </div>
              </div>
              <div class="result-row-balanced">
                <div class="result-item-balanced full-width">
                  <span class="result-item-label-balanced">Path:</span>
                  <code class="result-item-value-balanced">
                    {appState.repository.clonePath}
                  </code>
                  <button
                    class="copy-button"
                    onClick={copyPathToClipboard}
                    title={pathCopied() ? "Copied!" : "Copy path"}
                  >
                    <Show
                      when={pathCopied()}
                      fallback={
                        <svg
                          width="16"
                          height="16"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2"
                        >
                          <rect
                            x="9"
                            y="9"
                            width="13"
                            height="13"
                            rx="2"
                            ry="2"
                          ></rect>
                          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                      }
                    >
                      <svg
                        width="16"
                        height="16"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                      >
                        <polyline points="20 6 9 17 4 12"></polyline>
                      </svg>
                    </Show>
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </div>

        {/* Step 2: Prepare Project */}
        <Show when={cloneStatus() === "success"}>
          <div class={`operation-card ${prepareStatus()}`}>
            <div class="operation-header">
              <div class="operation-number">2</div>
              <div class="operation-info">
                <h3>Prepare Project</h3>
                <p>
                  Install dependencies, upgrade Amplify packages
                  <Show when={appState.repository.backendType === "Gen1"}>
                    {" "}
                    and pull Amplify environment
                  </Show>
                </p>
              </div>
              <div class="operation-status">
                <Show when={prepareStatus() === "pending"}>
                  <button class="action-button" onClick={handlePrepare}>
                    Prepare
                  </button>
                </Show>
                <Show when={prepareStatus() === "running"}>
                  <span class="status-indicator running">
                    <span class="spinner-small"></span>
                    Preparing...
                  </span>
                </Show>
                <Show when={prepareStatus() === "success"}>
                  <span class="status-indicator success">✓ Prepared</span>
                </Show>
                <Show when={prepareStatus() === "failed"}>
                  <span class="status-indicator failed">✗ Failed</span>
                </Show>
              </div>
            </div>
            <Show when={prepareError()}>
              <div class="operation-error permission-error">
                <pre class="error-message-text">{prepareError()}</pre>
              </div>
            </Show>
            <Show when={prepareOutput()}>
              <div class="prepare-output-container">
                <div class="prepare-output-header">
                  <span>Preparation Output</span>
                  <Show when={prepareStatus() === "running"}>
                    <span class="live-indicator">● Live</span>
                  </Show>
                </div>
                <pre class="prepare-output-live" id="prepare-output-pre">
                  {prepareOutput()}
                </pre>
              </div>
            </Show>
            <Show when={prepareStatus() === "failed"}>
              <div class="operation-retry-row">
                <button class="retry-link" onClick={handlePrepare}>
                  Retry Prepare
                </button>
              </div>
            </Show>
            <Show when={prepareStatus() === "success" && upgradeMessage()}>
              <div class="operation-result">
                <p class="upgrade-message">{upgradeMessage()}</p>
              </div>
            </Show>
          </div>
        </Show>

        {/* Step 3: Update Runtime */}
        <Show when={prepareStatus() === "success"}>
          <div class={`operation-card ${updateStatus()}`}>
            <div class="operation-header">
              <div class="operation-number">3</div>
              <div class="operation-info">
                <h3>Update Runtime</h3>
                <p>
                  Update Lambda runtime configurations to{" "}
                  {appState.runtimeInfo.targetRuntime}
                </p>
              </div>
              <div class="operation-status">
                <Show when={updateStatus() === "pending"}>
                  <button class="action-button" onClick={handleUpdate}>
                    Update
                  </button>
                </Show>
                <Show when={updateStatus() === "running"}>
                  <span class="status-indicator running">
                    <span class="spinner-small"></span>
                    Updating...
                  </span>
                </Show>
                <Show when={updateStatus() === "success"}>
                  <span class="status-indicator success">✓ Updated</span>
                </Show>
                <Show when={updateStatus() === "failed"}>
                  <span class="status-indicator failed">✗ Failed</span>
                </Show>
              </div>
            </div>
            <Show when={updateError()}>
              <div class="operation-error">{updateError()}</div>
            </Show>
            <Show when={updateStatus() === "failed"}>
              <div class="operation-retry-row">
                <button class="retry-link" onClick={handleUpdate}>
                  Retry Update
                </button>
              </div>
            </Show>
            <Show
              when={
                updateStatus() === "success" &&
                appState.repository.changes.length > 0
              }
            >
              <div class="operation-result">
                <h4>Changes Made:</h4>
                <div class="changes-list">
                  <For each={appState.repository.changes}>
                    {(change: FileChange) => (
                      <div class="change-item">
                        <span class="change-type">
                          {getChangeTypeDisplay(change.change_type)}
                        </span>
                        <code class="change-path">{change.path}</code>
                        <div class="change-details">
                          <span class="old-value">{change.old_value}</span>
                          <span class="arrow">→</span>
                          <span class="new-value">{change.new_value}</span>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>
            <Show
              when={
                updateStatus() === "success" &&
                appState.repository.changes.length === 0
              }
            >
              <div class="operation-result">
                <p class="no-changes">
                  No outdated runtimes are manually configured. Runtimes will be
                  updated by upgrading to latest amplify backend version.
                </p>
              </div>
            </Show>
          </div>
        </Show>

        {/* Step 4: Update Build Configuration (Gen2 only, required) */}
        <Show
          when={
            updateStatus() === "success" &&
            appState.repository.backendType === "Gen2"
          }
        >
          <div class={`operation-card ${buildConfigStatus()}`}>
            <div class="operation-header">
              <div class="operation-number">4</div>
              <div class="operation-info">
                <h3>Update Build Configuration</h3>
                <p>Update Amplify build command to use pipeline-deploy</p>
              </div>
              <div class="operation-status">
                <Show when={buildConfigStatus() === "pending"}>
                  <button
                    class="action-button"
                    onClick={handleBuildConfigUpdate}
                  >
                    Update
                  </button>
                </Show>
                <Show when={buildConfigStatus() === "running"}>
                  <span class="status-indicator running">
                    <span class="spinner-small"></span>
                    Updating...
                  </span>
                </Show>
                <Show when={buildConfigStatus() === "success"}>
                  <span class="status-indicator success">✓ Updated</span>
                </Show>
                <Show when={buildConfigStatus() === "failed"}>
                  <span class="status-indicator failed">✗ Failed</span>
                </Show>
              </div>
            </div>
            <Show when={buildConfigError()}>
              <div class="operation-error">
                <pre class="error-message-text">{buildConfigError()}</pre>
              </div>
            </Show>
            <Show when={buildConfigStatus() === "failed"}>
              <div class="operation-retry-row">
                <button class="retry-link" onClick={handleBuildConfigUpdate}>
                  Retry Config
                </button>
              </div>
            </Show>
            <Show
              when={buildConfigStatus() === "success" && buildConfigMessage()}
            >
              <div class="operation-result">
                <p class="upgrade-message">{buildConfigMessage()}</p>
              </div>
            </Show>
            {/* Build Configuration Changes Display */}
            <Show when={appState.repository.buildConfigChange}>
              <div class="result-balanced">
                <h4>Build Configuration Updated</h4>
                <div class="result-row-balanced">
                  <div class="result-item-balanced half-width">
                    <span class="result-item-label-balanced">Location:</span>
                    <span class="result-item-value-balanced">
                      {appState.repository.buildConfigChange?.location ===
                      "Cloud"
                        ? "AWS Cloud Configuration"
                        : appState.repository.buildConfigChange?.location}
                    </span>
                  </div>
                </div>
                <div class="result-row-balanced">
                  <div class="result-item-balanced full-width">
                    <span class="result-item-label-balanced">Old Command:</span>
                    <span class="result-item-value-balanced old-value">
                      {appState.repository.buildConfigChange?.old_command}
                    </span>
                  </div>
                </div>
                <div class="result-row-balanced">
                  <div class="result-item-balanced full-width">
                    <span class="result-item-label-balanced">New Command:</span>
                    <span class="result-item-value-balanced new-value">
                      {appState.repository.buildConfigChange?.new_command}
                    </span>
                  </div>
                </div>
              </div>
            </Show>
          </div>
        </Show>

        {/* Step 5: Build Verification (Required for Gen1, Optional for Gen2) */}
        <Show
          when={
            (appState.repository.backendType === "Gen1" &&
              updateStatus() === "success") ||
            (appState.repository.backendType === "Gen2" &&
              buildConfigStatus() === "success")
          }
        >
          {/* Step 5: Update Environment Variables (Gen2 only, required) */}
          <Show
            when={
              appState.repository.backendType === "Gen2" &&
              buildConfigStatus() === "success"
            }
          >
            <div class={`operation-card ${gen2EnvVarStatus()}`}>
              <div class="operation-header">
                <div class="operation-number">5</div>
                <div class="operation-info">
                  <h3>Update Environment Variables</h3>
                  <p>
                    Remove legacy _CUSTOM_IMAGE variable to use default Amplify
                    image
                  </p>
                </div>
                <div class="operation-status">
                  <Show when={gen2EnvVarStatus() === "pending"}>
                    <button
                      class="action-button"
                      onClick={handleGen2EnvVarUpdate}
                    >
                      Update
                    </button>
                  </Show>
                  <Show when={gen2EnvVarStatus() === "running"}>
                    <span class="status-indicator running">
                      <span class="spinner-small"></span>
                      Updating...
                    </span>
                  </Show>
                  <Show when={gen2EnvVarStatus() === "success"}>
                    <span class="status-indicator success">✓ Updated</span>
                  </Show>
                  <Show when={gen2EnvVarStatus() === "failed"}>
                    <span class="status-indicator failed">✗ Failed</span>
                  </Show>
                </div>
              </div>
              <Show when={gen2EnvVarError()}>
                <div class="operation-error">
                  <pre class="error-message-text">{gen2EnvVarError()}</pre>
                </div>
              </Show>
              <Show when={gen2EnvVarStatus() === "failed"}>
                <div class="operation-retry-row">
                  <button class="retry-link" onClick={handleGen2EnvVarUpdate}>
                    Retry Env Vars
                  </button>
                </div>
              </Show>
              <Show
                when={gen2EnvVarStatus() === "success" && gen2EnvVarMessage()}
              >
                <div class="operation-result">
                  <h4>Environment Variable Changes</h4>
                  <div class="env-var-simple-list">
                    {gen2EnvVarMessage()
                      ?.split("\n")
                      .filter((line) => line.trim())
                      .map((line) => (
                        <div class="env-var-simple-item">{line}</div>
                      ))}
                  </div>
                </div>
              </Show>
            </div>
          </Show>

          {/* Gen2 Optional Build Test Section - Show only after all required operations are complete */}
          <Show
            when={
              appState.repository.backendType === "Gen2" &&
              gen2EnvVarStatus() === "success"
            }
          >
            <div class="optional-build-section">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={gen2SandboxEnabled()}
                  onChange={(e) =>
                    setGen2SandboxEnabled(e.currentTarget.checked)
                  }
                />
                <span>Deploy sandbox and run build test (optional)</span>
              </label>
              <p class="optional-hint">
                You can continue without testing. Enable this to deploy a
                sandbox environment and verify the build.
              </p>
            </div>
          </Show>

          {/* Gen2 Sandbox Deployment (Step 6a - only when enabled and all required operations complete) */}
          <Show
            when={
              appState.repository.backendType === "Gen2" &&
              gen2SandboxEnabled() &&
              gen2EnvVarStatus() === "success"
            }
          >
            <div class={`operation-card ${sandboxStatus()}`}>
              <div class="operation-header">
                <div class="operation-number">6a</div>
                <div class="operation-info">
                  <h3>Deploy Sandbox</h3>
                  <p>Deploy Gen2 sandbox environment for testing</p>
                </div>
                <div class="operation-status">
                  <Show when={sandboxStatus() === "pending"}>
                    <button class="action-button" onClick={handleSandboxDeploy}>
                      Deploy
                    </button>
                  </Show>
                  <Show when={sandboxStatus() === "running"}>
                    <span class="status-indicator running">
                      <span class="spinner-small"></span>
                      Deploying...
                    </span>
                  </Show>
                  <Show when={sandboxStatus() === "success"}>
                    <span class="status-indicator success">✓ Deployed</span>
                  </Show>
                  <Show when={sandboxStatus() === "failed"}>
                    <span class="status-indicator failed">✗ Failed</span>
                  </Show>
                </div>
              </div>
              <Show when={sandboxError()}>
                <div class="operation-error">
                  <pre class="error-output">{sandboxError()}</pre>
                </div>
              </Show>
              <Show when={sandboxStatus() === "failed"}>
                <div class="operation-retry-row">
                  <button class="retry-link" onClick={handleSandboxDeploy}>
                    Retry Deploy
                  </button>
                </div>
              </Show>
              <Show when={sandboxOutput()}>
                <div class="sandbox-output-container">
                  <div class="sandbox-output-header">
                    <span>Deployment Output</span>
                    <Show when={sandboxStatus() === "running"}>
                      <span class="live-indicator">● Live</span>
                    </Show>
                  </div>
                  <pre class="sandbox-output" id="sandbox-output-pre">
                    {sandboxOutput()}
                  </pre>
                </div>
              </Show>
            </div>
          </Show>

          {/* Build Verification (Required for Gen1, Optional step 5b for Gen2 when sandbox enabled) */}
          <Show
            when={
              appState.repository.backendType === "Gen1" ||
              (appState.repository.backendType === "Gen2" &&
                gen2SandboxEnabled())
            }
          >
            <div class={`operation-card ${buildStatus()}`}>
              <div class="operation-header">
                <div class="operation-number">
                  <Show when={appState.repository.backendType === "Gen1"}>
                    5
                  </Show>
                  <Show when={appState.repository.backendType === "Gen2"}>
                    6b
                  </Show>
                </div>
                <div class="operation-info">
                  <h3>Build Verification</h3>
                  <p>
                    <Show when={appState.repository.backendType === "Gen1"}>
                      Run amplify build and frontend build
                    </Show>
                    <Show when={appState.repository.backendType === "Gen2"}>
                      Run frontend build
                    </Show>
                  </p>
                </div>
                <div class="operation-status">
                  <Show when={buildStatus() === "pending"}>
                    <Show when={appState.repository.backendType === "Gen1"}>
                      <button class="action-button" onClick={handleBuild}>
                        Build
                      </button>
                    </Show>
                    <Show when={appState.repository.backendType === "Gen2"}>
                      <Show when={sandboxStatus() === "success"}>
                        <button class="action-button" onClick={handleBuild}>
                          Build
                        </button>
                      </Show>
                      <Show when={sandboxStatus() !== "success"}>
                        <span class="status-indicator pending">
                          Waiting for sandbox deployment
                        </span>
                      </Show>
                    </Show>
                  </Show>
                  <Show when={buildStatus() === "running"}>
                    <span class="status-indicator running">
                      <span class="spinner-small"></span>
                      Building...
                    </span>
                  </Show>
                  <Show when={buildStatus() === "success"}>
                    <span class="status-indicator success">✓ Build Passed</span>
                  </Show>
                  <Show when={buildStatus() === "failed"}>
                    <span class="status-indicator failed">✗ Build Failed</span>
                  </Show>
                </div>
              </div>
              <Show when={buildError()}>
                <div class="operation-error">
                  <pre class="error-output">{buildError()}</pre>
                </div>
              </Show>
              <Show when={buildStatus() === "failed"}>
                <div class="operation-retry-row">
                  <button class="retry-link" onClick={handleBuild}>
                    Retry Build
                  </button>
                </div>
              </Show>
              <Show when={buildOutput()}>
                <div class="build-output-container">
                  <div class="build-output-header">
                    <span>Build Output</span>
                    <Show when={buildStatus() === "running"}>
                      <span class="live-indicator">● Live</span>
                    </Show>
                  </div>
                  <pre class="build-output-live" id="build-output-pre">
                    {buildOutput()}
                  </pre>
                </div>
              </Show>
            </div>
          </Show>

          {/* Step 6: Update Environment Variable (Gen1 only, after build passes) */}
          <Show
            when={
              appState.repository.backendType === "Gen1" &&
              buildStatus() === "success"
            }
          >
            <div class={`operation-card ${envVarStatus()}`}>
              <div class="operation-header">
                <div class="operation-number">6</div>
                <div class="operation-info">
                  <h3>Update Environment Variables</h3>
                  <p>Update environment variables for Gen1 backend</p>
                </div>
                <div class="operation-status">
                  <Show when={envVarStatus() === "pending"}>
                    <button class="action-button" onClick={handleEnvVarUpdate}>
                      Update
                    </button>
                  </Show>
                  <Show when={envVarStatus() === "running"}>
                    <span class="status-indicator running">
                      <span class="spinner-small"></span>
                      Updating...
                    </span>
                  </Show>
                  <Show when={envVarStatus() === "success"}>
                    <span class="status-indicator success">✓ Updated</span>
                  </Show>
                  <Show when={envVarStatus() === "failed"}>
                    <span class="status-indicator failed">✗ Failed</span>
                  </Show>
                </div>
              </div>
              <Show when={envVarError()}>
                <div class="operation-error">
                  <pre class="error-message-text">{envVarError()}</pre>
                </div>
              </Show>
              <Show when={envVarStatus() === "failed"}>
                <div class="operation-retry-row">
                  <button class="retry-link" onClick={handleEnvVarUpdate}>
                    Retry Env Vars
                  </button>
                </div>
              </Show>
              <Show when={envVarStatus() === "success" && envVarMessage()}>
                <div class="operation-result">
                  <h4>Environment Variable Changes</h4>
                  <div class="env-var-simple-list">
                    {envVarMessage()
                      ?.split("\n")
                      .filter((line) => line.trim())
                      .map((line) => (
                        <div class="env-var-simple-item">{line}</div>
                      ))}
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
      </div>

      {/* Actions */}
      <div class="actions">
        <button
          onClick={handleBack}
          class="secondary-button"
          disabled={isAnyOperationRunning()}
        >
          Back
        </button>
        <button
          onClick={handleContinue}
          class="primary-button"
          disabled={!canContinue()}
        >
          Continue to Push
        </button>
      </div>

      <Show when={!canContinue() && buildStatus() !== "running"}>
        <p class="info-message">
          <Show when={appState.repository.backendType === "Gen1"}>
            Complete all steps above to continue to the push step.
          </Show>
          <Show when={appState.repository.backendType === "Gen2"}>
            Complete the prepare project and build configuration steps to
            continue. Build test is optional for Gen2.
          </Show>
          <Show when={!appState.repository.backendType}>
            Complete all steps above to continue to the push step.
          </Show>
        </p>
      </Show>

      {/* Cleanup Confirmation Dialog - Handled by App.tsx globally */}
    </div>
  );
}

export default CloneUpdateStep;
