import { Show, For } from "solid-js";
import { appState } from "./store/appStore"; // Still needed for isOperationRunning check in JSX (button disabled state)
import { PrerequisitesStep } from "./components/PrerequisitesStep";
import { ProfileRegionStep } from "./components/ProfileRegionStep";
import { AppSelectionStep } from "./components/AppSelectionStep";
import { CloneUpdateStep } from "./components/CloneUpdateStep";
import { PushStep } from "./components/PushStep";
import { CleanupDialog } from "./components/CleanupDialog";
import { useWizard } from "./hooks/useWizard";
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
    <main class="flex flex-col h-screen w-full mx-auto p-0 overflow-hidden">
      <header class="text-center p-4 bg-[#f6f6f6] dark:bg-[#1a1a1a] flex-shrink-0 [WebkitAppRegion:drag] max-w-[900px] mx-auto w-full">
        <h1 class="m-0 mb-2 text-[1.8rem] text-[#1a1a1a] dark:text-[#f6f6f6] font-bold [WebkitAppRegion:no-drag]">
          Amplify Lambda Runtime Updater
        </h1>
        <p class="text-[#666] dark:text-[#999] m-0 [WebkitAppRegion:no-drag]">
          Update Lambda Node.js runtimes in your Amplify projects
        </p>
      </header>

      {/* Step Indicator */}
      <nav class="flex justify-center gap-2 pb-4 flex-wrap bg-[#f6f6f6] dark:bg-[#1a1a1a] flex-shrink-0 [WebkitAppRegion:no-drag] max-w-[900px] mx-auto w-full">
        <For each={steps()}>
          {(step, index) => {
            const stepIndex = index();
            const isDisabled = () =>
              !steps()[stepIndex].isEnabled ||
              appState.repository.isOperationRunning;

            const isActive = () => currentStep() === stepIndex;
            const isComplete = () => step.isComplete;

            return (
              <button
                class={`flex items-center gap-2 px-4 py-2 border rounded-lg cursor-pointer transition-all duration-200 
                  ${isActive() ? "border-[#396cd8] dark:border-[#5b8def] bg-[#f0f5ff] dark:bg-[#2a3a5a]" : "border-[#ddd] dark:border-[#444] bg-white dark:bg-[#2a2a2a]"} 
                  ${isComplete() ? "border-[#22c55e]" : ""} 
                  ${isDisabled() ? "opacity-50 dark:opacity-40 cursor-not-allowed pointer-events-none bg-[#f5f5f5] dark:bg-[#1a1a1a]" : "hover:border-[#396cd8] dark:hover:border-[#5b8def]"}
                `}
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
                <span
                  class={`flex items-center justify-center w-6 h-6 rounded-full text-[0.75rem] font-semibold 
                    ${isActive() ? "bg-[#396cd8] dark:bg-[#5b8def] text-white" : isComplete() ? "bg-[#22c55e] text-white" : "bg-[#e5e5e5] dark:bg-[#444] text-[#0f0f0f] dark:text-[#f6f6f6]"}
                  `}
                >
                  {isComplete() ? "✓" : stepIndex + 1}
                </span>
                <span class="text-[0.875rem] font-medium leading-none">
                  {step.title}
                </span>
              </button>
            );
          }}
        </For>
      </nav>

      {/* Step Content Wrapper - Scrollable area */}
      <div class="flex-1 overflow-y-auto overflow-x-hidden w-full bg-[#f6f6f6] dark:bg-[#1a1a1a]">
        <div class="max-w-[900px] mx-auto mb-8 p-8 min-h-[calc(100vh-200px)] bg-white dark:bg-[#2a2a2a] [contain:layout] shadow-sm rounded-b-lg">
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
