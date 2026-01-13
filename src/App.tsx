import { Show, For } from "solid-js";
import { appState } from "./store/appStore"; // Still needed for isOperationRunning check in JSX (button disabled state)
import { PrerequisitesStep } from "./components/PrerequisitesStep";
import { ProfileRegionStep } from "./components/ProfileRegionStep";
import { AppSelectionStep } from "./components/AppSelectionStep";
import { CloneUpdateStep } from "./components/CloneUpdateStep";
import { PushStep } from "./components/PushStep";
import { CleanupDialog } from "./components/CleanupDialog";
import { useWizard } from "./hooks/useWizard";
import "./App.css";

function App() {
  const {
    currentStep,
    steps,
    handleStepComplete,
    goToPreviousStep,
    goToStep,
    showCleanupDialog,
    handleCleanupClose,
  } = useWizard();

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
                  if (isDisabled()) {
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
