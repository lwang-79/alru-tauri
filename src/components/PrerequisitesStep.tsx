import { createSignal, onMount, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { PrerequisitesResult, ToolStatus } from "../types";
import { appState, setAppState } from "../store/appStore";
import "./shared.css";
import "./PrerequisitesStep.css";

interface ToolInfo {
  name: string;
  key: "network" | "awsCli" | "git" | "nodejs";
  stateKey: "network" | "aws_cli" | "git" | "nodejs";
  installUrl: string;
  installGuide: string;
}

interface OptionalToolInfo {
  name: string;
  stateKey: "amplify_cli" | "npm" | "yarn" | "pnpm" | "bun";
  description: string;
}

const TOOLS: ToolInfo[] = [
  {
    name: "Network Connection",
    key: "network" as any,
    stateKey: "network" as any,
    installUrl: "",
    installGuide: "Check your internet connection and try again",
  },
  {
    name: "AWS CLI",
    key: "awsCli",
    stateKey: "aws_cli",
    installUrl:
      "https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html",
    installGuide: "Install AWS CLI v2 from the official AWS documentation",
  },
  {
    name: "Git",
    key: "git",
    stateKey: "git",
    installUrl: "https://git-scm.com/downloads",
    installGuide: "Download and install Git from git-scm.com",
  },
  {
    name: "Node.js",
    key: "nodejs",
    stateKey: "nodejs",
    installUrl: "https://nodejs.org/",
    installGuide: "Install Node.js LTS version from nodejs.org",
  },
];

const OPTIONAL_TOOLS: OptionalToolInfo[] = [
  {
    name: "Amplify CLI",
    stateKey: "amplify_cli",
    description: "For Amplify Gen1 App",
  },
  {
    name: "npm",
    stateKey: "npm",
    description: "Package manager",
  },
  {
    name: "yarn",
    stateKey: "yarn",
    description: "Package manager",
  },
  {
    name: "pnpm",
    stateKey: "pnpm",
    description: "Package manager",
  },
  {
    name: "bun",
    stateKey: "bun",
    description: "Package manager",
  },
];

interface PrerequisitesStepProps {
  onComplete?: () => void;
}

export function PrerequisitesStep(props: PrerequisitesStepProps) {
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  const checkPrerequisites = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const result = await invoke<PrerequisitesResult>("check_prerequisites");

      setAppState("prerequisites", {
        network: result.network,
        awsCli: result.aws_cli,
        git: result.git,
        nodejs: result.nodejs,
        // Optional tools
        amplifyCli: result.amplify_cli,
        npm: result.npm,
        yarn: result.yarn,
        pnpm: result.pnpm,
        bun: result.bun,
      });
    } catch (e) {
      // Even if the check fails, show the UI with error state
      // This makes the app more resilient to network issues
      console.error("Prerequisites check failed:", e);
      setError(`Failed to check prerequisites: ${e}`);

      // Set a basic error state so the UI can still be displayed
      setAppState("prerequisites", {
        network: {
          installed: false,
          version: null,
          error: "Failed to check network connectivity",
        },
        awsCli: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        git: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        nodejs: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        amplifyCli: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        npm: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        yarn: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        pnpm: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
        bun: {
          installed: false,
          version: null,
          error: "Check failed - unable to verify installation",
        },
      });
    } finally {
      setIsLoading(false);
    }
  };

  onMount(() => {
    // Always show the UI, even if prerequisites haven't been checked yet
    // This makes the app more resilient to network issues
    const prereqs = appState.prerequisites;
    const alreadyChecked =
      prereqs.network.installed ||
      prereqs.awsCli.installed ||
      prereqs.git.installed ||
      prereqs.nodejs.installed ||
      prereqs.network.error ||
      prereqs.awsCli.error ||
      prereqs.git.error ||
      prereqs.nodejs.error ||
      prereqs.npm.installed ||
      prereqs.npm.error;

    if (alreadyChecked) {
      // Prerequisites were already checked, just show the results
      setIsLoading(false);
    } else {
      // Always show the UI first, then check prerequisites
      setIsLoading(false);
      // Start checking in the background
      setTimeout(() => checkPrerequisites(), 100);
    }
  });

  const allPrerequisitesMet = () => {
    const prereqs = appState.prerequisites;
    // Network is required for app functionality, plus all required local tools
    const networkOk = prereqs.network.installed;
    const requiredToolsOk =
      prereqs.awsCli.installed &&
      prereqs.git.installed &&
      prereqs.nodejs.installed;

    return networkOk && requiredToolsOk;
  };

  const getToolStatus = (tool: ToolInfo): ToolStatus => {
    return appState.prerequisites[tool.key];
  };

  const getOptionalToolStatus = (
    stateKey: OptionalToolInfo["stateKey"],
  ): ToolStatus => {
    const keyMap: Record<
      OptionalToolInfo["stateKey"],
      keyof typeof appState.prerequisites
    > = {
      amplify_cli: "amplifyCli",
      npm: "npm",
      yarn: "yarn",
      pnpm: "pnpm",
      bun: "bun",
    };
    return appState.prerequisites[keyMap[stateKey]];
  };

  const handleContinue = () => {
    if (allPrerequisitesMet() && props.onComplete) {
      props.onComplete();
    }
  };

  return (
    <div class="step-container prerequisites-step">
      <h2>Prerequisites Check</h2>
      <p class="step-description">
        Verifying that required tools are installed on your system.
      </p>

      <Show when={isLoading()}>
        <div class="loading">
          <span class="spinner"></span>
          Checking prerequisites...
        </div>
      </Show>

      <Show when={error()}>
        <div class="message error message-with-actions">
          {error()}
          <button onClick={checkPrerequisites} class="retry-button">
            Retry
          </button>
        </div>
      </Show>

      <Show when={!isLoading() && !error()}>
        <div class="tools-list">
          <For each={TOOLS}>
            {(tool) => {
              const status = () => getToolStatus(tool);
              return (
                <div
                  class={`tool-item ${status().installed ? "installed" : "missing"}`}
                >
                  <div class="tool-header">
                    <span
                      class={`status-icon ${status().installed ? "check" : "x"}`}
                    >
                      {status().installed ? "✓" : "✗"}
                    </span>
                    <span class="tool-name">{tool.name}</span>
                    <Show when={status().installed && status().version}>
                      <span class="tool-version">
                        {tool.key === "network"
                          ? status().version
                          : `v${status().version}`}
                      </span>
                    </Show>
                  </div>

                  <Show when={!status().installed}>
                    <div class="install-guidance">
                      <p>{tool.installGuide}</p>
                      <Show when={tool.installUrl}>
                        <a
                          href={tool.installUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          Installation Guide →
                        </a>
                      </Show>
                      <Show when={status().error}>
                        <p class="error-detail">{status().error}</p>
                      </Show>
                    </div>
                  </Show>
                </div>
              );
            }}
          </For>
        </div>

        {/* Optional Tools Section */}
        <div class="optional-tools-section">
          <h3>Optional Tools</h3>
          <p class="optional-description">
            These tools may be required depending on your project configuration.
          </p>
          <div class="optional-tools-grid">
            <For each={OPTIONAL_TOOLS}>
              {(tool) => {
                const status = () => getOptionalToolStatus(tool.stateKey);
                return (
                  <div
                    class={`optional-tool-item ${status().installed ? "installed" : "not-installed"}`}
                  >
                    <div class="optional-tool-header">
                      <span
                        class={`status-dot ${status().installed ? "green" : "gray"}`}
                      ></span>
                      <span class="optional-tool-name">{tool.name}</span>
                      <Show when={status().installed && status().version}>
                        <span class="optional-tool-version">
                          {status().version}
                        </span>
                      </Show>
                    </div>
                    <p class="optional-tool-description">{tool.description}</p>
                  </div>
                );
              }}
            </For>
          </div>
        </div>

        <div class="actions">
          <button onClick={checkPrerequisites} class="secondary-button">
            Re-check
          </button>
          <button
            onClick={handleContinue}
            class="primary-button"
            disabled={!allPrerequisitesMet()}
          >
            Continue
          </button>
        </div>

        <Show when={!allPrerequisitesMet()}>
          <Show when={!appState.prerequisites.network.installed}>
            <p class="message warning">
              <strong>Network Connection Required:</strong> Some features
              require internet connectivity. Please check your network
              connection and try again. Local tools can still be verified.
            </p>
          </Show>
          <Show when={appState.prerequisites.network.installed}>
            <p class="message warning">
              Please install all required tools before continuing.
            </p>
          </Show>
        </Show>
      </Show>
    </div>
  );
}

export default PrerequisitesStep;
