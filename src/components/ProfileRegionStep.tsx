import { createSignal, onMount, Show, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { AwsProfile } from "../types";
import { appState, setAppState, clearDownstreamState } from "../store/appStore";
import "./shared.css";

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
    <div class="step-container profile-region-step">
      <h2>AWS Profile & Region</h2>
      <p class="step-description">
        Select your AWS profile and region to access your Amplify applications.
      </p>

      <Show when={error()}>
        <div class="message error message-with-actions">
          {error()}
          <button onClick={loadProfiles} class="retry-button">
            Retry
          </button>
        </div>
      </Show>

      <div class="form-container">
        <div class="form-group">
          <label for="profile-select">AWS Profile</label>
          <Show
            when={!isLoadingProfiles()}
            fallback={
              <div class="loading-inline">
                <span class="spinner-small"></span>
                Loading profiles...
              </div>
            }
          >
            <select
              id="profile-select"
              value={appState.awsConfig.selectedProfile || ""}
              onChange={handleProfileChange}
              disabled={appState.awsConfig.profiles.length === 0}
            >
              <option value="">Select a profile...</option>
              <For each={appState.awsConfig.profiles}>
                {(profile) => <option value={profile}>{profile}</option>}
              </For>
            </select>
          </Show>
          <p class="field-hint">
            Profiles are loaded from ~/.aws/credentials and ~/.aws/config
          </p>
        </div>

        <div class="form-group">
          <label for="region-select">AWS Region</label>
          <Show
            when={!isLoadingRegions()}
            fallback={
              <div class="loading-inline">
                <span class="spinner-small"></span>
                Loading regions...
              </div>
            }
          >
            <select
              id="region-select"
              value={appState.awsConfig.selectedRegion || ""}
              onChange={handleRegionChange}
              disabled={appState.awsConfig.regions.length === 0}
            >
              <option value="">Select a region...</option>
              <For each={appState.awsConfig.regions}>
                {(region) => <option value={region}>{region}</option>}
              </For>
            </select>
          </Show>
          <p class="field-hint">
            Select the region where your Amplify app is deployed
          </p>
          <p class="message warning">
            If the selected region is different from the profile's configured
            region, amplify pull may fail for Gen 1 App.
          </p>
        </div>
      </div>

      <Show
        when={
          appState.awsConfig.selectedProfile &&
          appState.awsConfig.selectedRegion
        }
      >
        <div class="selection-summary">
          <span class="summary-icon">✓</span>
          <span>
            Using profile <strong>{appState.awsConfig.selectedProfile}</strong>{" "}
            in <strong>{appState.awsConfig.selectedRegion}</strong>
          </span>
        </div>
      </Show>

      <div class="actions">
        <button onClick={handleBack} class="secondary-button">
          Back
        </button>
        <button
          onClick={handleContinue}
          class="primary-button"
          disabled={!canContinue() || isLoading()}
        >
          Continue
        </button>
      </div>

      <Show when={!canContinue() && !isLoading()}>
        <p class="message warning">
          Please select both a profile and region to continue.
        </p>
      </Show>
    </div>
  );
}

export default ProfileRegionStep;
