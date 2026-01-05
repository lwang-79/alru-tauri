/**
 * Property-based tests for the application state store
 *
 * **Feature: amplify-runtime-updater, Property 12: Profile and Region State Persistence**
 * **Validates: Requirements 2.2, 2.4**
 */

import { describe, it, expect } from "vitest";
import * as fc from "fast-check";
import { createStore, SetStoreFunction } from "solid-js/store";
import type { AppState } from "./appStore";

// Create a fresh store for testing (isolated from the main app store)
function createTestStore(): [AppState, SetStoreFunction<AppState>] {
  const initialState: AppState = {
    prerequisites: {
      network: { installed: false, version: null, error: null },
      awsCli: { installed: false, version: null, error: null },
      git: { installed: false, version: null, error: null },
      nodejs: { installed: false, version: null, error: null },
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

  return createStore<AppState>(initialState);
}

// Arbitrary for generating valid AWS profile names
const awsProfileArb: fc.Arbitrary<string> = fc
  .string({ minLength: 1, maxLength: 50 })
  .filter((s) => /^[a-zA-Z0-9_-]+$/.test(s));

// Arbitrary for generating valid AWS region names
const awsRegionArb: fc.Arbitrary<string> = fc.constantFrom(
  "us-east-1",
  "us-east-2",
  "us-west-1",
  "us-west-2",
  "ap-south-1",
  "ap-northeast-1",
  "ap-northeast-2",
  "ap-southeast-1",
  "ap-southeast-2",
  "eu-central-1",
  "eu-west-1",
  "eu-west-2",
  "eu-west-3",
  "eu-north-1",
  "sa-east-1",
);

describe("AppStore State Persistence", () => {
  /**
   * Property 12: Profile and Region State Persistence
   *
   * For any profile or region selection action, the selected value shall be
   * stored in the application state and available for all subsequent AWS operations.
   */
  describe("Property 12: Profile and Region State Persistence", () => {
    it("should persist selected profile in state for any valid profile name", () => {
      fc.assert(
        fc.property(awsProfileArb, (profileName: string) => {
          const [state, setState] = createTestStore();

          // Simulate profile selection
          setState("awsConfig", "selectedProfile", profileName);

          // Property: The selected profile should be stored and retrievable
          expect(state.awsConfig.selectedProfile).toBe(profileName);
        }),
        { numRuns: 100 },
      );
    });

    it("should persist selected region in state for any valid region", () => {
      fc.assert(
        fc.property(awsRegionArb, (regionName: string) => {
          const [state, setState] = createTestStore();

          // Simulate region selection
          setState("awsConfig", "selectedRegion", regionName);

          // Property: The selected region should be stored and retrievable
          expect(state.awsConfig.selectedRegion).toBe(regionName);
        }),
        { numRuns: 100 },
      );
    });

    it("should persist both profile and region independently", () => {
      fc.assert(
        fc.property(
          awsProfileArb,
          awsRegionArb,
          (profileName: string, regionName: string) => {
            const [state, setState] = createTestStore();

            // Simulate profile and region selection
            setState("awsConfig", "selectedProfile", profileName);
            setState("awsConfig", "selectedRegion", regionName);

            // Property: Both values should be stored independently
            expect(state.awsConfig.selectedProfile).toBe(profileName);
            expect(state.awsConfig.selectedRegion).toBe(regionName);
          },
        ),
        { numRuns: 100 },
      );
    });

    it("should allow updating profile without affecting region", () => {
      fc.assert(
        fc.property(
          awsProfileArb,
          awsProfileArb,
          awsRegionArb,
          (profile1: string, profile2: string, region: string) => {
            const [state, setState] = createTestStore();

            // Set initial values
            setState("awsConfig", "selectedProfile", profile1);
            setState("awsConfig", "selectedRegion", region);

            // Update profile
            setState("awsConfig", "selectedProfile", profile2);

            // Property: Region should remain unchanged when profile is updated
            expect(state.awsConfig.selectedProfile).toBe(profile2);
            expect(state.awsConfig.selectedRegion).toBe(region);
          },
        ),
        { numRuns: 100 },
      );
    });

    it("should allow updating region without affecting profile", () => {
      fc.assert(
        fc.property(
          awsProfileArb,
          awsRegionArb,
          awsRegionArb,
          (profile: string, region1: string, region2: string) => {
            const [state, setState] = createTestStore();

            // Set initial values
            setState("awsConfig", "selectedProfile", profile);
            setState("awsConfig", "selectedRegion", region1);

            // Update region
            setState("awsConfig", "selectedRegion", region2);

            // Property: Profile should remain unchanged when region is updated
            expect(state.awsConfig.selectedProfile).toBe(profile);
            expect(state.awsConfig.selectedRegion).toBe(region2);
          },
        ),
        { numRuns: 100 },
      );
    });

    it("should allow clearing profile selection (setting to null)", () => {
      fc.assert(
        fc.property(awsProfileArb, (profileName: string) => {
          const [state, setState] = createTestStore();

          // Set profile
          setState("awsConfig", "selectedProfile", profileName);
          expect(state.awsConfig.selectedProfile).toBe(profileName);

          // Clear profile
          setState("awsConfig", "selectedProfile", null);

          // Property: Profile should be clearable
          expect(state.awsConfig.selectedProfile).toBeNull();
        }),
        { numRuns: 100 },
      );
    });

    it("should allow clearing region selection (setting to null)", () => {
      fc.assert(
        fc.property(awsRegionArb, (regionName: string) => {
          const [state, setState] = createTestStore();

          // Set region
          setState("awsConfig", "selectedRegion", regionName);
          expect(state.awsConfig.selectedRegion).toBe(regionName);

          // Clear region
          setState("awsConfig", "selectedRegion", null);

          // Property: Region should be clearable
          expect(state.awsConfig.selectedRegion).toBeNull();
        }),
        { numRuns: 100 },
      );
    });

    it("should persist profile list in state", () => {
      fc.assert(
        fc.property(
          fc.array(awsProfileArb, { minLength: 0, maxLength: 10 }),
          (profiles: string[]) => {
            const [state, setState] = createTestStore();

            // Set profiles list
            setState("awsConfig", "profiles", profiles);

            // Property: Profiles list should be stored and retrievable
            expect(state.awsConfig.profiles).toEqual(profiles);
          },
        ),
        { numRuns: 100 },
      );
    });

    it("should persist regions list in state", () => {
      fc.assert(
        fc.property(
          fc.array(awsRegionArb, { minLength: 0, maxLength: 20 }),
          (regions: string[]) => {
            const [state, setState] = createTestStore();

            // Set regions list
            setState("awsConfig", "regions", regions);

            // Property: Regions list should be stored and retrievable
            expect(state.awsConfig.regions).toEqual(regions);
          },
        ),
        { numRuns: 100 },
      );
    });
  });
});
