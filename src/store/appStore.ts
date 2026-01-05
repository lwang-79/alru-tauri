// SolidJS state store for the Amplify Runtime Updater application

import { createStore } from "solid-js/store";
import type {
  ToolStatus,
  AmplifyApp,
  AmplifyBranch,
  LambdaFunction,
  NodeVersion,
  PackageManager,
  FileChange,
  BuildStatus,
  BackendType,
  EnvVarChange,
  BuildConfigChange,
} from "../types";

// Step definition for wizard navigation
export interface Step {
  id: string;
  title: string;
  isComplete: boolean;
  isEnabled: boolean;
}

// Application state interface
export interface AppState {
  // Prerequisites
  prerequisites: {
    network: ToolStatus;
    awsCli: ToolStatus;
    git: ToolStatus;
    nodejs: ToolStatus;
    // Optional tools
    amplifyCli: ToolStatus;
    npm: ToolStatus;
    yarn: ToolStatus;
    pnpm: ToolStatus;
    bun: ToolStatus;
  };

  // AWS Configuration
  awsConfig: {
    profiles: string[];
    selectedProfile: string | null;
    regions: string[];
    selectedRegion: string | null;
  };

  // Runtime Information
  runtimeInfo: {
    supportedVersions: NodeVersion[];
    targetRuntime: string | null;
  };

  // Amplify Resources
  amplifyResources: {
    apps: AmplifyApp[];
    selectedApp: AmplifyApp | null;
    branches: AmplifyBranch[];
    selectedBranch: AmplifyBranch | null;
    lambdaFunctions: LambdaFunction[];
  };

  // Repository State
  repository: {
    clonePath: string | null;
    packageManager: PackageManager | null;
    backendType: BackendType | null;
    changes: FileChange[];
    buildStatus: BuildStatus;
    sandboxDeployed: boolean;
    gen2SandboxEnabled: boolean; // Whether user enabled sandbox deployment option
    gen2SandboxStatus: BuildStatus; // Status of sandbox deployment (6a)
    gen2BuildVerificationStatus: BuildStatus; // Status of build verification (6b)
    isOperationRunning: boolean; // Whether any operation is currently running
    envVarChanges: EnvVarChange[];
    buildConfigChange: BuildConfigChange | null;
    originalBuildSpec: string | null;
    // Operation completion status for state persistence
    operationStatus: {
      cloneComplete: boolean;
      prepareComplete: boolean;
      updateComplete: boolean;
      upgradeComplete: boolean;
      buildConfigComplete: boolean;
      buildComplete: boolean;
      envVarComplete: boolean;
      gen2EnvVarComplete: boolean;
    };
  };

  // Push Step State (preserved during navigation, reset on data changes)
  pushStep: {
    status: "pending" | "confirming" | "running" | "success" | "failed";
    error: string | null;
    commitHash: string | null;
    amplifyJob: any | null; // AmplifyJobDetails type
    jobCheckError: string | null;
    lastFailedJob: any | null; // AmplifyJobDetails type
    retryingJob: boolean;
    // Track what this state is based on to detect when it should be reset
    basedOnAppId: string | null;
    basedOnBranchName: string | null;
    basedOnClonePath: string | null;
  };

  // Wizard Navigation
  wizard: {
    currentStep: number;
    steps: Step[];
  };
}

// Initial state
const initialState: AppState = {
  prerequisites: {
    network: { installed: false, version: null, error: null },
    awsCli: { installed: false, version: null, error: null },
    git: { installed: false, version: null, error: null },
    nodejs: { installed: false, version: null, error: null },
    // Optional tools
    amplifyCli: { installed: false, version: null, error: null },
    npm: { installed: false, version: null, error: null },
    yarn: { installed: false, version: null, error: null },
    pnpm: { installed: false, version: null, error: null },
    bun: { installed: false, version: null, error: null },
  },

  awsConfig: {
    profiles: [],
    selectedProfile: null,
    regions: [],
    selectedRegion: null,
  },

  runtimeInfo: {
    supportedVersions: [],
    targetRuntime: null,
  },

  amplifyResources: {
    apps: [],
    selectedApp: null,
    branches: [],
    selectedBranch: null,
    lambdaFunctions: [],
  },

  repository: {
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
  },

  pushStep: {
    status: "pending",
    error: null,
    commitHash: null,
    amplifyJob: null,
    jobCheckError: null,
    lastFailedJob: null,
    retryingJob: false,
    basedOnAppId: null,
    basedOnBranchName: null,
    basedOnClonePath: null,
  },

  wizard: {
    currentStep: 0,
    steps: [
      {
        id: "prerequisites",
        title: "Prerequisites",
        isComplete: false,
        isEnabled: true,
      },
      {
        id: "profile-region",
        title: "AWS Profile & Region",
        isComplete: false,
        isEnabled: false,
      },
      {
        id: "app-selection",
        title: "App Selection",
        isComplete: false,
        isEnabled: false,
      },
      {
        id: "clone-update",
        title: "Clone & Update",
        isComplete: false,
        isEnabled: false,
      },
      {
        id: "push",
        title: "Push Changes",
        isComplete: false,
        isEnabled: false,
      },
    ],
  },
};

// Create the store
const [appState, setAppState] = createStore<AppState>(initialState);

// Helper function to clear state for steps after a given step index
// Step 0: Prerequisites
// Step 1: Profile & Region -> clears: amplifyResources, repository
// Step 2: App Selection -> clears: repository
// Step 3: Clone & Update -> clears: (nothing downstream except push state)
export function clearDownstreamState(fromStepIndex: number) {
  if (fromStepIndex <= 1) {
    // Clear App Selection state (step 2) and beyond
    setAppState("amplifyResources", {
      apps: [],
      selectedApp: null,
      branches: [],
      selectedBranch: null,
      lambdaFunctions: [],
    });
  }

  if (fromStepIndex <= 2) {
    // Clear Clone & Update state (step 3) and beyond
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
  }

  if (fromStepIndex <= 3) {
    // Clear Push step state (step 4) when upstream data changes
    resetPushStepState();
  }

  // Mark downstream steps as incomplete and disabled
  const steps = appState.wizard.steps;
  for (let i = fromStepIndex + 1; i < steps.length; i++) {
    setAppState("wizard", "steps", i, "isComplete", false);
    if (i > fromStepIndex + 1) {
      setAppState("wizard", "steps", i, "isEnabled", false);
    }
  }
}

// Reset Push step state to initial values
export function resetPushStepState() {
  setAppState("pushStep", {
    status: "pending",
    error: null,
    commitHash: null,
    amplifyJob: null,
    jobCheckError: null,
    lastFailedJob: null,
    retryingJob: false,
    basedOnAppId: null,
    basedOnBranchName: null,
    basedOnClonePath: null,
  });
}

// Check if Push step state should be reset based on current context
export function checkAndResetPushStepIfNeeded() {
  const currentAppId = appState.amplifyResources.selectedApp?.app_id || null;
  const currentBranchName =
    appState.amplifyResources.selectedBranch?.branch_name || null;
  const currentClonePath = appState.repository.clonePath;

  const pushState = appState.pushStep;

  // Reset if any of the key dependencies have changed
  if (
    pushState.basedOnAppId !== currentAppId ||
    pushState.basedOnBranchName !== currentBranchName ||
    pushState.basedOnClonePath !== currentClonePath
  ) {
    console.log("[PushStep] Resetting state due to upstream changes:", {
      oldAppId: pushState.basedOnAppId,
      newAppId: currentAppId,
      oldBranchName: pushState.basedOnBranchName,
      newBranchName: currentBranchName,
      oldClonePath: pushState.basedOnClonePath,
      newClonePath: currentClonePath,
    });
    resetPushStepState();
  }
}

// Update Push step state tracking to current context
export function updatePushStepContext() {
  const currentAppId = appState.amplifyResources.selectedApp?.app_id || null;
  const currentBranchName =
    appState.amplifyResources.selectedBranch?.branch_name || null;
  const currentClonePath = appState.repository.clonePath;

  setAppState("pushStep", "basedOnAppId", currentAppId);
  setAppState("pushStep", "basedOnBranchName", currentBranchName);
  setAppState("pushStep", "basedOnClonePath", currentClonePath);
}

export { appState, setAppState };
