import { Show, For, createSignal } from "solid-js";
import {
  appState,
  setAppState,
  checkAndResetPushStepIfNeeded,
  updatePushStepContext,
} from "./store/appStore";
import { PrerequisitesStep } from "./components/PrerequisitesStep";
import { ProfileRegionStep } from "./components/ProfileRegionStep";
import { AppSelectionStep } from "./components/AppSelectionStep";
import { CloneUpdateStep } from "./components/CloneUpdateStep";
import { PushStep } from "./components/PushStep";
import { CleanupDialog } from "./components/CleanupDialog";
import "./App.css";

function App() {
  const currentStep = () => appState.wizard.currentStep;
  const steps = () => appState.wizard.steps;

  // Function to scroll the step content to top
  const scrollToTop = () => {
    const scrollableWrapper = document.querySelector(".scrollable-wrapper");
    if (scrollableWrapper) {
      scrollableWrapper.scrollTop = 0;
    }
  };

  // Cleanup dialog state
  const [showCleanupDialog, setShowCleanupDialog] = createSignal(false);
  const [pendingNavigation, setPendingNavigation] = createSignal<{
    callback: (() => void) | null;
  }>({ callback: null });

  const handleStepComplete = (stepIndex: number) => {
    // Mark current step as complete
    setAppState("wizard", "steps", stepIndex, "isComplete", true);

    // Enable next step if exists
    if (stepIndex + 1 < steps().length) {
      // Always enable the next step when completing a step
      // (The cleanup process will disable steps 3 and 4, but completing step 2 should re-enable step 3)
      setAppState("wizard", "steps", stepIndex + 1, "isEnabled", true);
      setAppState("wizard", "currentStep", stepIndex + 1);

      // If moving to Push step (step 4), update context
      if (stepIndex + 1 === 4) {
        updatePushStepContext();
      }

      // Scroll to top when navigating to next step
      setTimeout(scrollToTop, 0);
    }
  };

  // Handle cleanup confirmation
  const handleCleanupClose = () => {
    // CleanupDialog component already handles all state cleanup
    // We just need to handle the navigation callback here
    setShowCleanupDialog(false);
    const nav = pendingNavigation();
    if (nav.callback) {
      nav.callback();
    }
    setPendingNavigation({ callback: null });
  };

  // Check if navigation requires cleanup dialog (only when repo exists)
  const navigateWithCleanupCheck = (
    targetStep: number,
    callback: () => void,
  ) => {
    // If navigating to an earlier step (before Clone & Update) and repository exists, ask for cleanup
    if (targetStep < 3 && appState.repository.clonePath) {
      setShowCleanupDialog(true);
      setPendingNavigation({ callback });
    } else {
      // No cleanup needed, just navigate (don't clear state when just viewing)
      callback();
    }
  };

  const goToPreviousStep = () => {
    const current = currentStep();
    if (current > 0) {
      // Block navigation if any operation is running
      if (appState.repository.isOperationRunning) {
        console.log(`[goToPreviousStep] Blocked - operation is running`);
        return;
      }

      const targetStep = current - 1;
      // Always check for cleanup when navigating to earlier steps
      navigateWithCleanupCheck(targetStep, () => {
        setAppState("wizard", "currentStep", targetStep);
        // Scroll to top when navigating to previous step
        setTimeout(scrollToTop, 0);
      });
    }
  };

  const goToStep = (stepIndex: number) => {
    const step = steps()[stepIndex];
    console.log(
      `[goToStep] Called with index ${stepIndex}`,
      `| enabled: ${step.isEnabled}`,
      `| complete: ${step.isComplete}`,
      `| current: ${currentStep()}`,
      `| repoPath: ${appState.repository.clonePath}`,
      `| operationRunning: ${appState.repository.isOperationRunning}`,
    );

    // Block navigation if any operation is running
    if (appState.repository.isOperationRunning) {
      console.log(`[goToStep] Blocked - operation is running`);
      return;
    }

    if (!step.isEnabled) {
      console.log(
        `[goToStep] Step ${stepIndex} is DISABLED, blocking navigation`,
      );
      return;
    }

    if (stepIndex === currentStep()) {
      console.log(`[goToStep] Already on step ${stepIndex}, ignoring`);
      return; // Already on this step
    }

    // If repository was cleaned up and trying to navigate to Push step without a repo, block it
    // But allow navigation to Clone & Update step (step 3) to start a new workflow
    const repoCleanedUp =
      !appState.repository.clonePath &&
      appState.repository.operationStatus.cloneComplete === false;

    if (stepIndex === 4 && repoCleanedUp) {
      console.log(
        `[goToStep] Step ${stepIndex} blocked - no repository exists for Push step`,
      );
      return; // Can't go to Push step without a repository
    }

    // If navigating backwards, check for cleanup
    if (stepIndex < currentStep()) {
      console.log(`[goToStep] Navigating backwards, checking for cleanup`);
      navigateWithCleanupCheck(stepIndex, () => {
        setAppState("wizard", "currentStep", stepIndex);
        // Scroll to top when navigating backwards
        setTimeout(scrollToTop, 0);
      });
      return;
    }

    console.log(`[goToStep] Navigating to step ${stepIndex}`);
    setAppState("wizard", "currentStep", stepIndex);

    // If navigating to Push step (step 4), check and update context
    if (stepIndex === 4) {
      checkAndResetPushStepIfNeeded();
      updatePushStepContext();
    }

    // Scroll to top when navigating to any step
    setTimeout(scrollToTop, 0);
  };

  return (
    <main class="app-container">
      <header class="app-header">
        <h1>Amplify Lambda Runtime Updater</h1>
        <p class="app-subtitle">
          Update Lambda Node.js runtimes in your Amplify projects
        </p>
      </header>

      {/* Step Indicator */}
      <nav class="step-indicator">
        <For each={steps()}>
          {(step, index) => {
            const stepIndex = index();
            const isDisabled = () =>
              !steps()[stepIndex].isEnabled ||
              appState.repository.isOperationRunning;

            return (
              <button
                class={`step-item ${currentStep() === stepIndex ? "active" : ""} ${step.isComplete ? "complete" : ""} ${isDisabled() ? "disabled" : ""}`}
                onClick={(e) => {
                  console.log(
                    `[Button Click] Step ${stepIndex} clicked`,
                    `| enabled: ${steps()[stepIndex].isEnabled}`,
                    `| operationRunning: ${appState.repository.isOperationRunning}`,
                    `| disabled attr: ${e.currentTarget.disabled}`,
                  );
                  if (isDisabled()) {
                    console.log(
                      `[Button Click] Step ${stepIndex} is disabled, ignoring click`,
                    );
                    e.preventDefault();
                    e.stopPropagation();
                    return;
                  }
                  goToStep(stepIndex);
                }}
                disabled={isDisabled()}
                aria-disabled={isDisabled()}
              >
                <span class="step-number">
                  {step.isComplete ? "✓" : stepIndex + 1}
                </span>
                <span class="step-title">{step.title}</span>
              </button>
            );
          }}
        </For>
      </nav>

      {/* Step Content Wrapper - Scrollable area */}
      <div class="scrollable-wrapper">
        <div class="step-content">
          <Show when={currentStep() === 0}>
            <PrerequisitesStep onComplete={() => handleStepComplete(0)} />
          </Show>

          <Show when={currentStep() === 1}>
            <ProfileRegionStep
              onComplete={() => handleStepComplete(1)}
              onBack={goToPreviousStep}
            />
          </Show>

          <Show when={currentStep() === 2}>
            <AppSelectionStep
              onComplete={() => handleStepComplete(2)}
              onBack={goToPreviousStep}
            />
          </Show>

          <Show when={currentStep() === 3}>
            <CloneUpdateStep
              onComplete={() => handleStepComplete(3)}
              onBack={goToPreviousStep}
            />
          </Show>

          <Show when={currentStep() === 4}>
            <PushStep
              onComplete={() => handleStepComplete(4)}
              onBack={goToPreviousStep}
            />
          </Show>
        </div>
      </div>

      {/* Cleanup Confirmation Dialog */}
      <CleanupDialog show={showCleanupDialog()} onClose={handleCleanupClose} />
    </main>
  );
}

export default App;
