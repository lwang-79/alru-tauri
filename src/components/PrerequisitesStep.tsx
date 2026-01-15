import { createSignal, onMount, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { PrerequisitesResult, ToolStatus } from "../types";
import { appState, setAppState } from "../store/appStore";

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
    <div class="max-w-[800px] mx-auto opacity-1 animate-[fadeIn_0.1s_ease-in] prerequisites-step">
      <h2 class="text-2xl font-bold text-[#333] dark:text-[#eee] mb-2 text-center">Prerequisites Check</h2>
      <p class="text-[#666] dark:text-[#aaa] mb-8 text-center leading-relaxed max-w-[600px] mx-auto">
        Verifying that required tools are installed on your system.
      </p>

      <Show when={isLoading()}>
        <div class="flex flex-col items-center justify-center p-12 bg-white dark:bg-[#2a2a2a] rounded-2xl border border-[#eee] dark:border-[#444] shadow-sm mb-8 animate-pulse">
          <span class="w-10 h-10 border-4 border-[#eee] dark:border-[#444] border-t-[#396cd8] dark:border-t-[#3b82f6] rounded-full animate-spin mb-4"></span>
          <span class="text-[#666] dark:text-[#aaa] font-medium tracking-tight">Checking prerequisites...</span>
        </div>
      </Show>

      <Show when={error()}>
        <div class="bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800/30 rounded-xl p-5 mb-8 flex items-center justify-between gap-4 text-red-800 dark:text-red-300 shadow-sm shadow-red-500/5">
          <div class="flex items-center gap-3">
            <svg class="w-6 h-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <p class="text-[0.9rem] font-medium leading-relaxed m-0">{error()}</p>
          </div>
          <button onClick={checkPrerequisites} class="bg-white dark:bg-[#333] text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800/50 px-4 py-2 rounded-lg font-bold text-[0.8rem] cursor-pointer transition-all hover:bg-red-50 dark:hover:bg-red-900/20 active:scale-95 shrink-0">
            Retry Check
          </button>
        </div>
      </Show>

      <Show when={!isLoading() && !error()}>
        <div class="flex flex-col gap-4 mb-10">
          <For each={TOOLS}>
            {(tool) => {
              const status = () => getToolStatus(tool);
              return (
                <div
                  class={`p-5 rounded-2xl border transition-all duration-300 flex flex-col gap-4 ${status().installed ? "bg-green-50/30 dark:bg-green-900/5 border-green-100 dark:border-green-800/30" : "bg-red-50/30 dark:bg-red-900/5 border-red-100 dark:border-red-800/30"}`}
                >
                  <div class="flex items-center gap-4">
                    <span
                      class={`flex items-center justify-center w-8 h-8 rounded-full text-[0.9rem] font-bold ${status().installed ? "bg-[#e8f5e9] dark:bg-[#1b5e20] text-[#2e7d32] dark:text-[#a5d6a7]" : "bg-[#fbe9e7] dark:bg-[#b71c1c] text-[#c62828] dark:text-[#ef9a9a]"}`}
                    >
                      {status().installed ? "✓" : "✗"}
                    </span>
                    <span class="text-[1.1rem] font-bold text-[#333] dark:text-[#eee] grow">{tool.name}</span>
                    <Show when={status().installed && status().version}>
                      <span class="text-[0.85rem] font-mono font-bold bg-white/60 dark:bg-black/20 px-2 py-0.5 rounded border border-[#0000000a] dark:border-[#ffffff0a] text-[#666] dark:text-[#bbb]">
                        {tool.key === "network"
                          ? status().version
                          : `v${status().version}`}
                      </span>
                    </Show>
                  </div>

                  <Show when={!status().installed}>
                    <div class="mt-1 pt-4 border-t border-red-100 dark:border-red-800/20 animate-[slideDown_0.2s_ease-out]">
                      <p class="m-0 text-[0.9rem] text-[#666] dark:text-[#aaa] leading-relaxed mb-3">{tool.installGuide}</p>
                      <Show when={tool.installUrl}>
                        <a
                          href={tool.installUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          class="inline-flex items-center gap-1.5 text-[#396cd8] dark:text-[#64b5f6] text-[0.85rem] font-bold hover:underline"
                        >
                          Installation Guide
                          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                          </svg>
                        </a>
                      </Show>
                      <Show when={status().error}>
                        <div class="mt-3 p-3 bg-red-100/30 dark:bg-red-950/20 rounded-lg border border-red-200/20 dark:border-red-800/10">
                          <p class="m-0 text-[0.8rem] text-red-600 dark:text-red-400 font-mono italic">{status().error}</p>
                        </div>
                      </Show>
                    </div>
                  </Show>
                </div>
              );
            }}
          </For>
        </div>

        {/* Optional Tools Section */}
        <div class="mt-12 mb-8 pt-8 border-t border-[#eee] dark:border-[#444]">
          <h3 class="text-[1.1rem] font-bold text-[#666] dark:text-[#aaa] mb-2 uppercase tracking-tight">Optional Tools</h3>
          <p class="text-[0.85rem] text-[#888] mb-6">
            These tools may be required depending on your project configuration.
          </p>
          <div class="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
            <For each={OPTIONAL_TOOLS}>
              {(tool) => {
                const status = () => getOptionalToolStatus(tool.stateKey);
                return (
                  <div
                    class={`border rounded-xl p-3 flex flex-col gap-1 transition-all duration-200 ${status().installed ? "bg-green-50/20 dark:bg-green-900/10 border-green-100 dark:border-green-800/20" : "bg-[#fafafa] dark:bg-[#2a2a2a] border-[#e0e0e0] dark:border-[#444]"}`}
                  >
                    <div class="flex items-center gap-2">
                      <span
                        class={`w-2 h-2 rounded-full shrink-0 ${status().installed ? "bg-[#4caf50]" : "bg-[#bdbdbd] dark:bg-[#666]"}`}
                      ></span>
                      <span class="text-[0.9rem] font-bold text-[#333] dark:text-[#eee] truncate grow">{tool.name}</span>
                      <Show when={status().installed && status().version}>
                        <span class="text-[0.75rem] text-[#666] dark:text-[#aaa] font-mono">
                          {status().version}
                        </span>
                      </Show>
                    </div>
                    <p class="m-0 text-[0.7rem] text-[#888] dark:text-[#666] leading-tight grow">{tool.description}</p>
                  </div>
                );
              }}
            </For>
          </div>
        </div>

        <div class="flex items-center justify-between gap-4 mt-8">
          <button onClick={checkPrerequisites} class="bg-white dark:bg-[#2a2a2a] text-[#396cd8] dark:text-[#64b5f6] border border-[#396cd8] dark:border-[#1e3a8a] px-8 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#396cd8] dark:hover:bg-[#1e3a8a] hover:text-white dark:hover:text-white active:scale-95 shadow-sm">
            Check Again
          </button>
          <button
            onClick={handleContinue}
            class="bg-[#396cd8] dark:bg-[#3b82f6] text-white border-none px-12 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#2d5bb8] dark:hover:bg-[#2563eb] disabled:bg-[#eee] dark:disabled:bg-[#333] disabled:text-[#999] dark:disabled:text-[#666] disabled:cursor-not-allowed active:scale-95 flex items-center gap-2 group"
            disabled={!allPrerequisitesMet()}
          >
            Continue
            <svg class="w-5 h-5 transition-transform group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
            </svg>
          </button>
        </div>

        <Show when={!allPrerequisitesMet()}>
          <div class="mt-[-2rem] mb-12 animate-pulse">
            <Show when={!appState.prerequisites.network.installed}>
              <p class="px-5 py-3 bg-red-50 dark:bg-red-900/10 text-red-700 dark:text-red-400 rounded-xl text-[0.85rem] font-medium border border-red-100 dark:border-red-800/30 text-center">
                <span class="font-bold">Network Connection Required:</span> Some features require internet connectivity. Local tools can still be verified.
              </p>
            </Show>
            <Show when={appState.prerequisites.network.installed}>
              <p class="px-5 py-3 bg-blue-50 dark:bg-blue-900/10 text-[#1e40af] dark:text-[#93c5fd] rounded-xl text-[0.85rem] font-medium border border-blue-100 dark:border-blue-800/30 text-center">
                Please install all required tools before continuing.
              </p>
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  );
}

export default PrerequisitesStep;
