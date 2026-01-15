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
import { LogViewer } from "./common/LogViewer";
import { OperationCard } from "./common/OperationCard";

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
        // Gen1: Install dependencies first, then Upgrade CLI, then Amplify setup (Pull & Checkout)
        // This matches the original proven workflow
        console.log(
          "[handlePrepare] Gen1: Starting with dependency installation",
        );

        // Step 1: Install dependencies with streaming
        await invoke<boolean>("install_dependencies_streaming", {
          projectPath: clonePath,
          packageManager: packageManager,
        });

        console.log(
          "[handlePrepare] Gen1: Dependencies installed, proceeding with CLI check",
        );

        // Step 2: Upgrade Amplify CLI globally for Gen1
        // We do this before pull to ensure we have the latest CLI features
        const upgradeResult = await invoke<UpgradeResult>("upgrade_amplify_cli");
        if (upgradeResult.skipped) {
          setUpgradeMessage(upgradeResult.message);
        } else {
          setUpgradeMessage(`Upgraded to version ${upgradeResult.latest_version}`);
        }

        console.log(
          "[handlePrepare] Gen1: CLI checked/upgraded, proceeding with Amplify setup",
        );

        // Step 3: Run amplify pull (this also initializes the environment)
        if (selectedBranch && selectedApp) {
          try {
            // Run amplify pull to initialize the project and checkout the env
            await invoke<boolean>("amplify_pull_streaming", {
              projectPath: clonePath,
              appId: selectedApp.app_id,
              envName: selectedBranch.backend_environment_name,
              profileName: appState.awsConfig.selectedProfile,
            });
          } catch (e) {
            // If amplify commands fail, show a warning
            console.warn("Amplify setup warning:", e);
            setPrepareError(
              `Warning: ${String(e)}\n\nYou may need to manually run the amplify pull command if there are issues.`,
            );
            // Don't mark as complete if critical setup fails, or maybe we should?
            // The previous logic allowed it to proceed with warning.
            // But if pull fails, usually we can't build.
            // Let's stick to the previous pattern: return early but don't hard-fail?
            // User request implies this Step MUST work.
            // If pull fails here, we should probably STOP and let them retry.
            setPrepareStatus("failed");
            return;
          }
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
    <div class="max-w-[800px] mx-auto opacity-1 animate-[fadeIn_0.1s_ease-in] clone-update-step">
      <h2 class="text-2xl font-bold text-[#333] dark:text-[#eee] mb-2 text-center">Clone & Update</h2>
      <p class="text-[#666] dark:text-[#aaa] mb-8 text-center leading-relaxed max-w-[600px] mx-auto">
        Clone the repository, detect project configuration, and update runtime
        settings.
      </p>

      {/* Selected App/Branch Summary */}
      <div class="bg-white dark:bg-[#2a2a2a] rounded-xl p-5 border border-[#eee] dark:border-[#444] shadow-sm mb-8 flex flex-wrap gap-8 items-center justify-between">
        <div class="flex flex-wrap gap-8">
          <div class="flex flex-col">
            <span class="text-[0.7rem] font-bold text-[#999] dark:text-[#666] uppercase tracking-wider">App</span>
            <span class="text-[0.95rem] font-semibold text-[#333] dark:text-[#eee]">
              {appState.amplifyResources.selectedApp?.name}
            </span>
          </div>
          <div class="flex flex-col">
            <span class="text-[0.7rem] font-bold text-[#999] dark:text-[#666] uppercase tracking-wider">Branch</span>
            <span class="text-[0.95rem] font-semibold text-[#333] dark:text-[#eee]">
              {appState.amplifyResources.selectedBranch?.branch_name}
            </span>
          </div>
        </div>
        <div class="flex flex-col items-end">
          <span class="text-[0.7rem] font-bold text-[#999] dark:text-[#666] uppercase tracking-wider">Target Runtime</span>
          <span class="px-3 py-0.5 bg-[#e3f2fd] dark:bg-[#1a3a5c] text-[#1976d2] dark:text-[#64b5f6] rounded-full text-[0.8rem] font-bold border border-[#bbdefb] dark:border-[#1a3a5c]">
            {appState.runtimeInfo.targetRuntime}
          </span>
        </div>
      </div>

      {/* Operations */}
      <div class="flex flex-col gap-4">
        {/* Step 1: Clone Repository & Detect Configuration */}
        {/* Step 1: Clone Repository & Detect Configuration */}
        <OperationCard
          stepNumber={1}
          title="Clone Repository"
          description="Clone the repository and detect project configuration"
          status={cloneStatus()}
          onAction={handleClone}
          actionLabel="Clone"
          runningLabel="Cloning..."
          successLabel="✓ Cloned"
          failedLabel="✗ Failed"
          error={cloneError()}
          isPermissionError={
            !!cloneError() && isPermissionError(cloneError()!)
          }
        >
          <Show
            when={cloneStatus() === "success" && appState.repository.clonePath}
          >
            <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
              <h4 class="text-[0.9rem] font-bold text-[#555] dark:text-[#ccc] mb-4 uppercase tracking-tight">Repository Details</h4>
              <div class="flex flex-wrap gap-x-8 gap-y-4 mb-4">
                <div class="flex items-center gap-2">
                  <span class="text-[0.85rem] text-[#666] dark:text-[#aaa]">Package Manager:</span>
                  <span class="px-2 py-0.5 bg-[#f5f5f5] dark:bg-[#333] text-[#333] dark:text-[#eee] rounded text-[0.75rem] font-bold border border-[#ddd] dark:border-[#555]">
                    {getPackageManagerDisplay(
                      appState.repository.packageManager,
                    )}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-[0.85rem] text-[#666] dark:text-[#aaa]">Backend Type:</span>
                  <span class="px-2 py-0.5 bg-[#f5f5f5] dark:bg-[#333] text-[#333] dark:text-[#eee] rounded text-[0.75rem] font-bold border border-[#ddd] dark:border-[#555]">
                    {getBackendTypeDisplay(appState.repository.backendType)}
                  </span>
                </div>
              </div>
              <div class="flex items-center gap-2 bg-[#fafafa] dark:bg-[#222] p-2 rounded-lg border border-[#f0f0f0] dark:border-[#333]">
                <span class="text-[0.8rem] text-[#999] dark:text-[#666] font-mono shrink-0 ml-1 leading-none uppercase tracking-tighter">Path:</span>
                <code class="text-[0.85rem] text-[#444] dark:text-[#ccc] px-2 py-0.5 rounded break-all grow truncate">
                  {appState.repository.clonePath}
                </code>
                <button
                  class="flex items-center justify-center w-8 h-8 shrink-0 p-0 border border-[#ddd] dark:border-[#555] rounded-md bg-white dark:bg-[#444] text-[#666] dark:text-[#aaa] cursor-pointer transition-all duration-200 hover:bg-[#f5f5f5] dark:hover:bg-[#333] hover:text-[#333] dark:hover:text-white shadow-sm"
                  onClick={copyPathToClipboard}
                  title={pathCopied() ? "Copied!" : "Copy path"}
                >
                  <Show
                    when={pathCopied()}
                    fallback={
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
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
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                    >
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                  </Show>
                </button>
              </div>
            </div>
          </Show>
        </OperationCard>

        {/* Step 2: Prepare Project */}
        <Show when={cloneStatus() === "success"}>
          <OperationCard
            stepNumber={2}
            title="Prepare Project"
            description={
              <>
                Install dependencies, upgrade Amplify packages
                <Show when={appState.repository.backendType === "Gen1"}>
                  {" "}
                  and pull Amplify environment
                </Show>
              </>
            }
            status={prepareStatus()}
            onAction={handlePrepare}
            actionLabel="Prepare"
            runningLabel="Preparing..."
            successLabel="✓ Prepared"
            failedLabel="✗ Failed"
            error={prepareError()}
            isPermissionError={true}
          >
            <Show when={prepareOutput()}>
              <LogViewer
                output={prepareOutput()}
                title="Preparation Output"
                isRunning={prepareStatus() === "running"}
              />
            </Show>
            <Show when={prepareStatus() === "success" && upgradeMessage()}>
              <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                <div class="bg-[#f0f9ff] dark:bg-[#07253d] border border-[#bae6fd] dark:border-[#1e3a8a] rounded-lg p-3 flex items-start gap-3 text-blue-800 dark:text-blue-300">
                  <svg class="w-5 h-5 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <p class="m-0 text-[0.85rem] leading-relaxed">{upgradeMessage()}</p>
                </div>
              </div>
            </Show>
          </OperationCard>
        </Show>

        {/* Step 3: Update Runtime */}
        <Show when={prepareStatus() === "success"}>
          <OperationCard
            stepNumber={3}
            title="Update Runtime"
            description={`Update Lambda runtime configurations to ${appState.runtimeInfo.targetRuntime}`}
            status={updateStatus()}
            onAction={handleUpdate}
            actionLabel="Update"
            runningLabel="Updating..."
            successLabel="✓ Updated"
            failedLabel="✗ Failed"
            error={updateError()}
          >
            <Show
              when={
                updateStatus() === "success" &&
                appState.repository.changes.length > 0
              }
            >
              <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                <h4 class="text-[0.9rem] font-bold text-[#555] dark:text-[#ccc] mb-3 uppercase tracking-tight">Changes Applied</h4>
                <div class="flex flex-col gap-3">
                  <For each={appState.repository.changes}>
                    {(change: FileChange) => (
                      <div class="bg-[#fafafa] dark:bg-[#333] rounded-lg p-3 border border-[#f0f0f0] dark:border-[#444] shadow-sm">
                        <div class="flex items-center gap-3 mb-2">
                          <span class="px-2 py-0.5 bg-[#e3f2fd] dark:bg-[#1a3a5c] text-[#1976d2] dark:text-[#64b5f6] rounded text-[0.65rem] font-bold uppercase tracking-wide whitespace-nowrap">
                            {getChangeTypeDisplay(change.change_type)}
                          </span>
                          <code class="text-[0.8rem] text-[#666] dark:text-[#aaa] truncate italic grow">
                            {change.path.split("/").pop()}
                          </code>
                        </div>
                        <div class="flex items-center gap-2 text-[0.85rem] bg-white dark:bg-[#222] p-2 rounded border border-[#f0f0f0] dark:border-[#111]">
                          <span class="text-[#f44336] dark:text-[#ef5350] line-through font-mono opacity-60 text-[0.8rem]">{change.old_value}</span>
                          <span class="text-[#999] dark:text-[#666]">
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                              <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                            </svg>
                          </span>
                          <span class="text-[#4caf50] dark:text-[#81c784] font-bold font-mono text-[0.85rem]">{change.new_value}</span>
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
              <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                <p class="m-0 text-[#666] dark:text-[#aaa] italic text-[0.85rem] text-center py-6 bg-[#f8f9fa] dark:bg-[#222] rounded-lg border border-dashed border-[#ddd] dark:border-[#555]">
                  No outdated runtimes manually configured. Runtimes will be updated with the latest amplify backend version.
                </p>
              </div>
            </Show>
          </OperationCard>
        </Show>

        {/* Step 4: Update Build Configuration (Gen2 only, required) */}
        <Show
          when={
            updateStatus() === "success" &&
            appState.repository.backendType === "Gen2"
          }
        >
          <OperationCard
            stepNumber={4}
            title="Update Build Configuration"
            description="Update Amplify build command to use pipeline-deploy"
            status={buildConfigStatus()}
            onAction={handleBuildConfigUpdate}
            actionLabel="Update"
            runningLabel="Updating..."
            successLabel="✓ Updated"
            failedLabel="✗ Failed"
            error={buildConfigError()}
          >
            <Show
              when={buildConfigStatus() === "success" && buildConfigMessage()}
            >
              <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                <div class="bg-[#f0f9ff] dark:bg-[#07253d] border border-[#bae6fd] dark:border-[#1e3a8a] rounded-lg p-3 flex items-start gap-3 text-blue-800 dark:text-blue-300">
                  <svg class="w-5 h-5 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <p class="m-0 text-[0.85rem] leading-relaxed">{buildConfigMessage()}</p>
                </div>
              </div>
            </Show>
            {/* Build Configuration Changes Display */}
            <Show when={appState.repository.buildConfigChange}>
              <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                <h4 class="text-[0.9rem] font-bold text-[#555] dark:text-[#ccc] mb-4 uppercase tracking-tight">Configuration Changes</h4>
                <div class="bg-white dark:bg-[#333] rounded-xl p-5 border border-[#f0f0f0] dark:border-[#444] shadow-sm">
                  <div class="flex items-center gap-3 mb-5 pb-3 border-b border-[#f5f5f5] dark:border-[#444]">
                    <span class="text-[0.7rem] font-bold text-[#999] dark:text-[#666] uppercase tracking-widest leading-none">Location</span>
                    <span class="text-[0.9rem] font-bold text-[#333] dark:text-[#eee]">
                      {appState.repository.buildConfigChange?.location ===
                        "Cloud"
                        ? "AWS Cloud (amplify.yml)"
                        : appState.repository.buildConfigChange?.location}
                    </span>
                  </div>
                  <div class="space-y-4">
                    <div class="flex flex-col gap-2">
                      <span class="text-[0.7rem] font-bold text-[#c62828] dark:text-[#ef5350] uppercase tracking-widest leading-none">Old Command</span>
                      <code class="text-[0.8rem] bg-red-50 dark:bg-red-950/20 text-red-600 dark:text-red-400 p-3 rounded-lg font-mono break-all line-through decoration-red-400/50 decoration-2">
                        {appState.repository.buildConfigChange?.old_command}
                      </code>
                    </div>
                    <div class="flex flex-col gap-2 pt-2 border-t border-[#f5f5f5] dark:border-[#444]">
                      <span class="text-[0.7rem] font-bold text-[#2e7d32] dark:text-[#81c784] uppercase tracking-widest leading-none">New Command</span>
                      <code class="text-[0.85rem] bg-green-50 dark:bg-green-950/20 text-green-700 dark:text-green-300 p-3 rounded-lg font-mono break-all font-bold border border-green-200 dark:border-green-800/50 shadow-sm shadow-green-600/5">
                        {appState.repository.buildConfigChange?.new_command}
                      </code>
                    </div>
                  </div>
                </div>
              </div>
            </Show>
          </OperationCard>
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
            <OperationCard
              stepNumber={5}
              title="Update Environment Variables"
              description="Remove legacy _CUSTOM_IMAGE variable to use default Amplify image"
              status={gen2EnvVarStatus()}
              onAction={handleGen2EnvVarUpdate}
              actionLabel="Update"
              runningLabel="Updating..."
              successLabel="✓ Updated"
              failedLabel="✗ Failed"
              error={gen2EnvVarError()}
            >
              <Show
                when={gen2EnvVarStatus() === "success" && gen2EnvVarMessage()}
              >
                <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                  <h4 class="text-[0.9rem] font-bold text-[#555] dark:text-[#ccc] mb-3 uppercase tracking-tight">Environment Changes</h4>
                  <div class="bg-[#fafafa] dark:bg-[#222] rounded-lg border border-[#f0f0f0] dark:border-[#333] divide-y divide-[#f0f0f0] dark:divide-[#333]">
                    {gen2EnvVarMessage()
                      ?.split("\n")
                      .filter((line) => line.trim())
                      .map((line) => (
                        <div class="p-3 font-mono text-[0.85rem] text-[#444] dark:text-[#bbb] flex items-center gap-3">
                          <span class="w-1.5 h-1.5 rounded-full bg-blue-500 shrink-0"></span>
                          {line}
                        </div>
                      ))}
                  </div>
                </div>
              </Show>
            </OperationCard>
          </Show>

          <Show
            when={
              appState.repository.backendType === "Gen2" &&
              gen2EnvVarStatus() === "success"
            }
          >
            <div class="bg-gradient-to-br from-white to-[#f8faff] dark:from-[#2a2a2a] dark:to-[#1e293b] border border-[#dbeafe] dark:border-[#1e3a8a] rounded-2xl p-6 shadow-sm relative overflow-hidden group">
              <div class="absolute top-0 right-0 w-32 h-32 bg-blue-100/30 dark:bg-blue-900/10 rounded-full blur-3xl -mr-16 -mt-16 pointer-events-none transition-transform group-hover:scale-125 duration-500"></div>
              <label class="flex items-center gap-4 cursor-pointer relative z-10 select-none">
                <input
                  type="checkbox"
                  class="w-6 h-6 rounded-md accent-[#396cd8] cursor-pointer transition-transform active:scale-90"
                  checked={gen2SandboxEnabled()}
                  onChange={(e) =>
                    setGen2SandboxEnabled(e.currentTarget.checked)
                  }
                />
                <span class="text-[1.05rem] font-bold text-[#333] dark:text-[#eee] tracking-tight">Deploy sandbox & run build test (recommended)</span>
              </label>
              <p class="m-0 mt-3 pl-10 text-[0.9rem] text-[#666] dark:text-[#94a3b8] leading-relaxed relative z-10 italic">
                Enable this to deploy an ephemeral sandbox and verify the frontend build.
              </p>
            </div>
          </Show>

          <Show
            when={
              appState.repository.backendType === "Gen2" &&
              gen2SandboxEnabled() &&
              gen2EnvVarStatus() === "success"
            }
          >
            <OperationCard
              stepNumber="6a"
              title="Deploy Sandbox"
              description="Deploy Gen2 sandbox environment for testing"
              status={sandboxStatus()}
              onAction={handleSandboxDeploy}
              actionLabel="Deploy"
              runningLabel="Deploying..."
              successLabel="✓ Deployed"
              failedLabel="✗ Failed"
              error={sandboxError()}
            >
              <Show when={sandboxOutput()}>
                <LogViewer
                  output={sandboxOutput()}
                  title="Deployment Output"
                  isRunning={sandboxStatus() === "running"}
                />
              </Show>
            </OperationCard>
          </Show>

          {/* Build Verification (Required for Gen1, Optional step 5b for Gen2 when sandbox enabled) */}
          <Show
            when={
              appState.repository.backendType === "Gen1" ||
              (appState.repository.backendType === "Gen2" &&
                gen2SandboxEnabled())
            }
          >
            <OperationCard
              stepNumber={
                appState.repository.backendType === "Gen1" ? 5 : "6b"
              }
              title="Build Verification"
              description={
                appState.repository.backendType === "Gen1"
                  ? "Verify amplify build and frontend build"
                  : "Verify frontend build against sandbox schema"
              }
              status={buildStatus()}
              onAction={
                (appState.repository.backendType === "Gen1" ||
                  (appState.repository.backendType === "Gen2" &&
                    sandboxStatus() === "success"))
                  ? handleBuild
                  : undefined
              }
              actionLabel="Start Build"
              pendingLabel={
                (appState.repository.backendType === "Gen2" &&
                  sandboxStatus() !== "success")
                  ? "Waiting for sandbox..."
                  : undefined
              }
              runningLabel="Building..."
              successLabel="✓ Build Successful"
              failedLabel="✗ Build Failed"
              error={buildError()}
            >
              <Show when={buildOutput()}>
                <LogViewer
                  output={buildOutput()}
                  title="Build Log"
                  isRunning={buildStatus() === "running"}
                />
              </Show>
            </OperationCard>
          </Show>

          {/* Step 6: Update Environment Variable (Gen1 only, after build passes) */}
          <Show
            when={
              appState.repository.backendType === "Gen1" &&
              buildStatus() === "success"
            }
          >
            <OperationCard
              stepNumber={6}
              title="Update Environment Variables"
              description="Configure AWS Amplify build image settings to Amazon Linux 2023"
              status={envVarStatus()}
              onAction={handleEnvVarUpdate}
              actionLabel="Update"
              runningLabel="Updating..."
              successLabel="✓ Settings Updated"
              failedLabel="✗ Update Failed"
              error={envVarError()}
            >
              <Show when={envVarStatus() === "success" && envVarMessage()}>
                <div class="mt-4 pt-4 border-t border-[#f0f0f0] dark:border-[#444]">
                  <h4 class="text-[0.9rem] font-bold text-[#555] dark:text-[#ccc] mb-3 uppercase tracking-tight">Configuration Applied</h4>
                  <div class="bg-[#fafafa] dark:bg-[#222] rounded-lg border border-[#f0f0f0] dark:border-[#333] divide-y divide-[#f0f0f0] dark:divide-[#333]">
                    {envVarMessage()
                      ?.split("\n")
                      .filter((line) => line.trim())
                      .map((line) => (
                        <div class="p-3 font-mono text-[0.85rem] text-[#444] dark:text-[#bbb] flex items-center gap-3">
                          <span class="w-1.5 h-1.5 rounded-full bg-blue-500 shrink-0"></span>
                          {line}
                        </div>
                      ))}
                  </div>
                </div>
              </Show>
            </OperationCard>
          </Show>
        </Show>
      </div>

      {/* Actions */}
      <div class="flex items-center justify-between gap-4 mt-8">
        <button
          onClick={handleBack}
          class="bg-white dark:bg-[#2a2a2a] text-[#396cd8] dark:text-[#64b5f6] border border-[#396cd8] dark:border-[#1e3a8a] px-8 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#396cd8] dark:hover:bg-[#1e3a8a] hover:text-white dark:hover:text-white disabled:opacity-40 disabled:grayscale disabled:cursor-not-allowed shadow-sm active:scale-95"
          disabled={isAnyOperationRunning()}
        >
          Back
        </button>
        <button
          onClick={handleContinue}
          class="bg-[#396cd8] dark:bg-[#3b82f6] text-white border-none px-12 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#2d5bb8] dark:hover:bg-[#2563eb] disabled:bg-[#eee] dark:disabled:bg-[#333] disabled:text-[#999] dark:disabled:text-[#666] disabled:cursor-not-allowed active:scale-95 flex items-center gap-2 group"
          disabled={!canContinue()}
        >
          Continue
          <svg class="w-5 h-5 transition-transform group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default CloneUpdateStep;
