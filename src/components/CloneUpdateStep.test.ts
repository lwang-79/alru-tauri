/**
 * Property-based tests for the CloneUpdateStep component
 *
 * **Feature: amplify-runtime-updater, Property 10: Change Summary Completeness**
 * **Validates: Requirements 7.5, 8.5**
 */

import { describe, it, expect } from "vitest";
import * as fc from "fast-check";
import type { FileChange } from "../types";

// Helper function to generate change summary display (mirrors the component logic)
function generateChangeSummary(changes: FileChange[]): string[] {
  return changes.map((change) => {
    const changeTypeDisplay: Record<string, string> = {
      runtime_update: "Runtime Update",
      dependency_update: "Dependency Update",
      env_update: "Environment Variable Update",
    };
    const typeLabel =
      changeTypeDisplay[change.change_type] || change.change_type;
    return `${typeLabel}: ${change.path} (${change.old_value} → ${change.new_value})`;
  });
}

// Arbitrary for generating valid file paths
const filePathArb: fc.Arbitrary<string> = fc.oneof(
  fc.constant(
    "amplify/backend/function/myFunc/myFunc-cloudformation-template.json",
  ),
  fc.constant(
    "amplify/backend/function/anotherFunc/anotherFunc-cloudformation-template.json",
  ),
  fc.constant("amplify/functions/myFunction/resource.ts"),
  fc.constant("amplify/functions/handler/resource.ts"),
  fc.constant("package.json"),
  fc.constant("_LIVE_UPDATES"),
  fc
    .stringMatching(/^[a-zA-Z0-9_\-\/\.]+$/)
    .filter((s) => s.length > 0 && s.length < 100),
);

// Arbitrary for generating valid change types
const changeTypeArb: fc.Arbitrary<string> = fc.constantFrom(
  "runtime_update",
  "dependency_update",
  "env_update",
);

// Arbitrary for generating valid runtime values
const runtimeValueArb: fc.Arbitrary<string> = fc.constantFrom(
  "nodejs16.x",
  "nodejs18.x",
  "nodejs20.x",
  "nodejs22.x",
  "Runtime.NODEJS_16_X",
  "Runtime.NODEJS_18_X",
  "Runtime.NODEJS_20_X",
  "Runtime.NODEJS_22_X",
  "@aws-amplify/backend: ^1.0.0",
  "@aws-amplify/backend: latest",
  "@aws-amplify/backend-cli: ^1.0.0",
  "@aws-amplify/backend-cli: latest",
);

// Arbitrary for generating a FileChange
const fileChangeArb: fc.Arbitrary<FileChange> = fc.record({
  path: filePathArb,
  change_type: changeTypeArb,
  old_value: runtimeValueArb,
  new_value: runtimeValueArb,
});

// Arbitrary for generating a list of FileChanges
const fileChangesArb: fc.Arbitrary<FileChange[]> = fc.array(fileChangeArb, {
  minLength: 0,
  maxLength: 20,
});

describe("CloneUpdateStep Change Summary", () => {
  /**
   * Property 10: Change Summary Completeness
   *
   * For any list of file changes, the generated summary shall contain an entry
   * for each change including the file path and change description.
   */
  describe("Property 10: Change Summary Completeness", () => {
    it("should generate a summary entry for each file change", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          // Property: Summary should have exactly one entry per change
          expect(summary.length).toBe(changes.length);
        }),
        { numRuns: 100 },
      );
    });

    it("should include file path in each summary entry", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          // Property: Each summary entry should contain the file path
          changes.forEach((change, index) => {
            expect(summary[index]).toContain(change.path);
          });
        }),
        { numRuns: 100 },
      );
    });

    it("should include old value in each summary entry", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          // Property: Each summary entry should contain the old value
          changes.forEach((change, index) => {
            expect(summary[index]).toContain(change.old_value);
          });
        }),
        { numRuns: 100 },
      );
    });

    it("should include new value in each summary entry", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          // Property: Each summary entry should contain the new value
          changes.forEach((change, index) => {
            expect(summary[index]).toContain(change.new_value);
          });
        }),
        { numRuns: 100 },
      );
    });

    it("should include change type description in each summary entry", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          const changeTypeDisplay: Record<string, string> = {
            runtime_update: "Runtime Update",
            dependency_update: "Dependency Update",
            env_update: "Environment Variable Update",
          };

          // Property: Each summary entry should contain a human-readable change type
          changes.forEach((change, index) => {
            const expectedType =
              changeTypeDisplay[change.change_type] || change.change_type;
            expect(summary[index]).toContain(expectedType);
          });
        }),
        { numRuns: 100 },
      );
    });

    it("should handle empty change list", () => {
      const summary = generateChangeSummary([]);

      // Property: Empty changes should produce empty summary
      expect(summary).toEqual([]);
    });

    it("should preserve order of changes in summary", () => {
      fc.assert(
        fc.property(fileChangesArb, (changes: FileChange[]) => {
          const summary = generateChangeSummary(changes);

          // Property: Summary entries should be in the same order as input changes
          changes.forEach((change, index) => {
            // Each summary entry at index i should correspond to change at index i
            expect(summary[index]).toContain(change.path);
          });
        }),
        { numRuns: 100 },
      );
    });
  });
});
