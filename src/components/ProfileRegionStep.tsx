import { createSignal, onMount, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { AwsProfile } from "../types";
import { appState, setAppState, clearDownstreamState } from "../store/appStore";

interface ProfileRegionStepProps {
  onComplete?: () => void;
  onBack?: () => void;
}

export function ProfileRegionStep(props: ProfileRegionStepProps) {
  const [isLoadingProfiles, setIsLoadingProfiles] = createSignal(false);
  const [isLoadingRegions, setIsLoadingRegions] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const loadProfileRegion = async (profile: string) => {
    try {
      const region = await invoke<string | null>("get_profile_region", {
        profile,
      });
      if (region && appState.awsConfig.regions.includes(region)) {
        setAppState("awsConfig", "selectedRegion", region);
      }
    } catch (e) {
      // Silently fail - region will use default fallback
      console.warn(`Failed to get region for profile ${profile}:`, e);
    }
  };

  const loadProfiles = async () => {
    setIsLoadingProfiles(true);
    setError(null);

    try {
      const profiles = await invoke<AwsProfile[]>("get_aws_profiles");
      const profileNames = profiles.map((p) => p.name);
      setAppState("awsConfig", "profiles", profileNames);

      // Set default profile if not already selected
      if (!appState.awsConfig.selectedProfile && profileNames.length > 0) {
        // Prefer "default" profile, otherwise use the first one
        const defaultProfile = profileNames.includes("default")
          ? "default"
          : profileNames[0];
        setAppState("awsConfig", "selectedProfile", defaultProfile);

        // Load region for the default profile after regions are loaded
        // We'll call this after loadRegions completes
      }
    } catch (e) {
      setError(`Failed to load AWS profiles: ${e}`);
    } finally {
      setIsLoadingProfiles(false);
    }
  };

  const loadRegions = async () => {
    setIsLoadingRegions(true);

    try {
      const regions = await invoke<string[]>("get_aws_regions");
      setAppState("awsConfig", "regions", regions);

      // Set default region if not already selected
      if (!appState.awsConfig.selectedRegion && regions.length > 0) {
        // Default to us-east-1 if available (will be overridden by profile region if available)
        const defaultRegion = regions.includes("us-east-1")
          ? "us-east-1"
          : regions[0];
        setAppState("awsConfig", "selectedRegion", defaultRegion);
      }
    } catch (e) {
      setError(`Failed to load AWS regions: ${e}`);
    } finally {
      setIsLoadingRegions(false);
    }
  };

  onMount(async () => {
    await loadProfiles();
    await loadRegions();

    // After both profiles and regions are loaded, set region from profile config
    const selectedProfile = appState.awsConfig.selectedProfile;
    if (selectedProfile) {
      await loadProfileRegion(selectedProfile);
    }
  });

  const handleProfileChange = async (event: Event) => {
    const target = event.target as HTMLSelectElement;
    const value = target.value || null;
    const previousValue = appState.awsConfig.selectedProfile;

    if (value !== previousValue) {
      setAppState("awsConfig", "selectedProfile", value);
      // Clear downstream state when profile changes (affects app list)
      clearDownstreamState(1);

      // Update region based on the new profile's config
      if (value) {
        await loadProfileRegion(value);
      }
    }
  };

  const handleRegionChange = (event: Event) => {
    const target = event.target as HTMLSelectElement;
    const value = target.value || null;
    const previousValue = appState.awsConfig.selectedRegion;

    if (value !== previousValue) {
      setAppState("awsConfig", "selectedRegion", value);
      // Clear downstream state when region changes (affects app list)
      clearDownstreamState(1);
    }
  };

  const canContinue = () => {
    return (
      appState.awsConfig.selectedProfile !== null &&
      appState.awsConfig.selectedRegion !== null
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

  const isLoading = () => isLoadingProfiles() || isLoadingRegions();

  return (
    <div class="max-w-[800px] mx-auto opacity-1 animate-[fadeIn_0.1s_ease-in] profile-region-step">
      <h2 class="text-2xl font-bold text-[#333] dark:text-[#eee] mb-2 text-center">AWS Profile & Region</h2>
      <p class="text-[#666] dark:text-[#aaa] mb-8 text-center leading-relaxed max-w-[600px] mx-auto">
        Select your AWS profile and region to access your Amplify applications.
      </p>

      <Show when={error()}>
        <div class="bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800/30 rounded-xl p-5 mb-8 flex items-center justify-between gap-4 text-red-800 dark:text-red-300 shadow-sm shadow-red-500/5">
          <div class="flex items-center gap-3">
            <svg class="w-6 h-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <p class="text-[0.9rem] font-medium leading-relaxed m-0">{error()}</p>
          </div>
          <button onClick={loadProfiles} class="bg-white dark:bg-[#333] text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800/50 px-4 py-2 rounded-lg font-bold text-[0.8rem] cursor-pointer transition-all hover:bg-red-50 dark:hover:bg-red-900/20 active:scale-95 shrink-0">
            Retry
          </button>
        </div>
      </Show>

      <div class="bg-white dark:bg-[#2a2a2a] rounded-2xl border border-[#eee] dark:border-[#444] p-8 shadow-sm flex flex-col gap-8 mb-8">
        {/* Profile Group */}
        <div class="flex flex-col gap-3">
          <label for="profile-select" class="text-[0.95rem] font-bold text-[#333] dark:text-[#eee] tracking-tight ml-1">AWS Profile</label>
          <Show
            when={!isLoadingProfiles()}
            fallback={
              <div class="flex items-center gap-3 py-3 px-4 bg-[#f8f9fa] dark:bg-[#333] rounded-xl border border-[#eee] dark:border-[#444]">
                <span class="w-4 h-4 border-2 border-[#eee] dark:border-[#444] border-t-[#396cd8] dark:border-t-[#3b82f6] rounded-full animate-spin"></span>
                <span class="text-[0.9rem] text-[#666] dark:text-[#999]">Loading profiles...</span>
              </div>
            }
          >
            <div class="relative group">
              <select
                id="profile-select"
                class="w-full appearance-none bg-[#f8f9fa] dark:bg-[#333] border-none text-[1.1rem] text-[#333] dark:text-[#eee] font-bold py-3.5 px-5 rounded-xl cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#396cd8] dark:focus:ring-[#3b82f6] transition-all disabled:opacity-50 disabled:cursor-not-allowed shadow-inner"
                value={appState.awsConfig.selectedProfile || ""}
                onChange={handleProfileChange}
                disabled={appState.awsConfig.profiles.length === 0}
              >
                <option value="">Select a profile...</option>
                <For each={appState.awsConfig.profiles}>
                  {(profile) => <option value={profile}>{profile}</option>}
                </For>
              </select>
              <div class="absolute inset-y-0 right-4 flex items-center pointer-events-none text-[#999] group-hover:text-[#396cd8] transition-colors">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
              </div>
            </div>
          </Show>
          <p class="m-0 ml-1 text-[0.8rem] text-[#888] dark:text-[#666] italic flex items-center gap-2">
            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
            </svg>
            Profiles are loaded from ~/.aws/credentials and ~/.aws/config
          </p>
        </div>

        {/* Region Group */}
        <div class="flex flex-col gap-3">
          <label for="region-select" class="text-[0.95rem] font-bold text-[#333] dark:text-[#eee] tracking-tight ml-1">AWS Region</label>
          <Show
            when={!isLoadingRegions()}
            fallback={
              <div class="flex items-center gap-3 py-3 px-4 bg-[#f8f9fa] dark:bg-[#333] rounded-xl border border-[#eee] dark:border-[#444]">
                <span class="w-4 h-4 border-2 border-[#eee] dark:border-[#444] border-t-[#396cd8] dark:border-t-[#3b82f6] rounded-full animate-spin"></span>
                <span class="text-[0.9rem] text-[#666] dark:text-[#999]">Loading regions...</span>
              </div>
            }
          >
            <div class="relative group">
              <select
                id="region-select"
                class="w-full appearance-none bg-[#f8f9fa] dark:bg-[#333] border-none text-[1.1rem] text-[#333] dark:text-[#eee] font-bold py-3.5 px-5 rounded-xl cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#396cd8] dark:focus:ring-[#3b82f6] transition-all disabled:opacity-50 disabled:cursor-not-allowed shadow-inner"
                value={appState.awsConfig.selectedRegion || ""}
                onChange={handleRegionChange}
                disabled={appState.awsConfig.regions.length === 0}
              >
                <option value="">Select a region...</option>
                <For each={appState.awsConfig.regions}>
                  {(region) => <option value={region}>{region}</option>}
                </For>
              </select>
              <div class="absolute inset-y-0 right-4 flex items-center pointer-events-none text-[#999] group-hover:text-[#396cd8] transition-colors">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
                </svg>
              </div>
            </div>
          </Show>
          <p class="m-0 ml-1 text-[0.8rem] text-[#888] dark:text-[#666] leading-relaxed">
            Select the target region where your Amplify application is hosted
          </p>
          <div class="bg-amber-50 dark:bg-amber-950/10 border border-amber-200 dark:border-amber-800/30 rounded-xl p-4 flex items-start gap-3 mt-2">
            <svg class="w-5 h-5 text-amber-500 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <p class="m-0 text-[0.85rem] text-amber-800 dark:text-amber-300 leading-relaxed font-medium">
              If the selected region differs from your profile's default configuration, <code class="bg-amber-100 dark:bg-amber-900/30 px-1 rounded text-amber-900 dark:text-amber-200">amplify pull</code> operations for Gen1 apps may fail.
            </p>
          </div>
        </div>
      </div>

      <Show
        when={
          appState.awsConfig.selectedProfile &&
          appState.awsConfig.selectedRegion
        }
      >
        <div class="flex items-center gap-3 bg-blue-50/50 dark:bg-blue-900/10 border border-blue-100 dark:border-blue-800/20 py-3.5 px-6 rounded-full mb-8 shadow-sm">
          <div class="w-6 h-6 flex items-center justify-center bg-blue-500 rounded-full text-white text-[0.85rem] font-bold">✓</div>
          <span class="text-[0.95rem] text-[#333] dark:text-[#eee]">
            Active Configuration: <strong class="text-[#396cd8] dark:text-[#3b82f6]">{appState.awsConfig.selectedProfile}</strong> {" "}
            in <strong class="text-[#396cd8] dark:text-[#3b82f6]">{appState.awsConfig.selectedRegion}</strong>
          </span>
        </div>
      </Show>

      <div class="flex items-center justify-between gap-4 mt-8">
        <button onClick={handleBack} class="bg-white dark:bg-[#2a2a2a] text-[#396cd8] dark:text-[#64b5f6] border border-[#396cd8] dark:border-[#1e3a8a] px-8 py-3 rounded-xl font-bold cursor-pointer transition-all duration-200 hover:bg-[#396cd8] dark:hover:bg-[#1e3a8a] hover:text-white dark:hover:text-white active:scale-95 shadow-sm">
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

      <Show when={!canContinue() && !isLoading()}>
        <div class="mt-[-2rem] mb-12 flex justify-center animate-pulse">
          <p class="px-5 py-2.5 bg-blue-50 dark:bg-blue-900/10 text-[#1e40af] dark:text-[#93c5fd] rounded-full text-xs font-semibold border border-blue-100 dark:border-blue-800/30">
            Please select both a profile and region to continue.
          </p>
        </div>
      </Show>
    </div>
  );
}

export default ProfileRegionStep;
