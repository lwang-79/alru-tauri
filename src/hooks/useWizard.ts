import { createSignal } from "solid-js";
import {
    appState,
    setAppState,
    checkAndResetPushStepIfNeeded,
    updatePushStepContext,
} from "../store/appStore";

export function useWizard() {
    // Cleanup dialog state
    const [showCleanupDialog, setShowCleanupDialog] = createSignal(false);
    const [pendingNavigation, setPendingNavigation] = createSignal<{
        callback: (() => void) | null;
    }>({ callback: null });

    // Function to scroll the step content to top
    const scrollToTop = () => {
        const scrollableWrapper = document.querySelector(".scrollable-wrapper");
        if (scrollableWrapper) {
            scrollableWrapper.scrollTop = 0;
        }
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

    const handleStepComplete = (stepIndex: number) => {
        // Mark current step as complete
        setAppState("wizard", "steps", stepIndex, "isComplete", true);

        // Enable next step if exists
        if (stepIndex + 1 < appState.wizard.steps.length) {
            // Always enable the next step when completing a step
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

    const goToPreviousStep = () => {
        const current = appState.wizard.currentStep;
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
        const step = appState.wizard.steps[stepIndex];
        console.log(
            `[goToStep] Called with index ${stepIndex}`,
            `| enabled: ${step.isEnabled}`,
            `| complete: ${step.isComplete}`,
            `| current: ${appState.wizard.currentStep}`,
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

        if (stepIndex === appState.wizard.currentStep) {
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
        if (stepIndex < appState.wizard.currentStep) {
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

    return {
        currentStep: () => appState.wizard.currentStep,
        steps: () => appState.wizard.steps,
        handleStepComplete,
        goToPreviousStep,
        goToStep,
        showCleanupDialog,
        handleCleanupClose,
    };
}
