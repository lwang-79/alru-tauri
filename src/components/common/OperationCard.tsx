import { Show, type JSX } from "solid-js";

export type OperationStatus = "pending" | "running" | "success" | "failed";

interface OperationCardProps {
    stepNumber: string | number;
    title: string;
    description: string | JSX.Element;
    status: OperationStatus;
    onAction?: () => void;
    actionLabel?: string;
    runningLabel?: string;
    successLabel?: string;
    failedLabel?: string;
    pendingLabel?: string;
    error?: string | null;
    isPermissionError?: boolean;
    className?: string;
    children?: JSX.Element;
    renderExtra?: () => JSX.Element; // For additional content in the card body before children
}

export function OperationCard(props: OperationCardProps) {
    const statusClasses = () => {
        switch (props.status) {
            case "pending":
                return "border-l-[#999] dark:border-l-[#666]";
            case "running":
                return "border-l-[#396cd8] dark:border-l-[#5c8ce6] bg-[#f8faff] dark:bg-[#1a2a3a]";
            case "success":
                return "border-l-[#2e7d32] bg-[#f7fff7] dark:bg-[#1a2a1a] border-l-[#2e7d32] dark:border-l-[#4caf50]";
            case "failed":
                return "border-l-[#c62828] dark:border-l-[#f44336] bg-[#fff8f8] dark:bg-[#3a1a1a]";
            default:
                return "";
        }
    };

    return (
        <div
            class={`bg-white dark:bg-[#2a2a2a] border border-[#e0e0e0] dark:border-[#444] rounded-lg p-5 transition-all duration-300 relative overflow-hidden border-l-4 ${statusClasses()} ${props.className || ""}`}
        >
            <div class="flex items-start gap-4">
                <div
                    class={`w-7 h-7 rounded-full flex items-center justify-center font-semibold text-[0.9rem] flex-shrink-0 mt-[0.1rem]
                        ${props.status === "running" ? "bg-[#396cd8] dark:bg-[#5c8ce6] text-white" :
                            props.status === "success" ? "bg-[#2e7d32] dark:bg-[#4caf50] text-white" :
                                props.status === "failed" ? "bg-[#c62828] dark:bg-[#f44336] text-white" :
                                    "bg-[#eee] dark:bg-[#444] text-[#555] dark:text-[#aaa]"}
                    `}
                >
                    {props.stepNumber}
                </div>
                <div class="flex-1">
                    <h3 class="m-0 mb-1 text-base font-semibold dark:text-[#eee]">{props.title}</h3>
                    <div class="m-0 text-[0.85rem] text-[#666] dark:text-[#aaa]">{props.description}</div>
                </div>
                <div class="flex-shrink-0">
                    <Show when={props.status === "pending"}>
                        <Show
                            when={props.onAction}
                            fallback={
                                <Show when={props.pendingLabel}>
                                    <span class="flex items-center gap-2 text-[0.9rem] font-medium italic text-[#666] dark:text-[#aaa]">
                                        {props.pendingLabel}
                                    </span>
                                </Show>
                            }
                        >
                            <button
                                class="bg-[#396cd8] dark:bg-[#5c8ce6] text-white border-none px-5 py-2 rounded-md font-medium cursor-pointer transition-colors duration-200 hover:bg-[#2d5bb8] dark:hover:bg-[#4a7ad4]"
                                onClick={props.onAction}
                            >
                                {props.actionLabel || "Start"}
                            </button>
                        </Show>
                    </Show>
                    <Show when={props.status === "running"}>
                        <span class="flex items-center gap-2 text-[0.9rem] font-medium text-[#396cd8] dark:text-[#5c8ce6]">
                            <span class="w-4 h-4 border-2 border-[#e0e0e0] border-t-[#396cd8] rounded-full animate-spin inline-block"></span>
                            {props.runningLabel || "Running..."}
                        </span>
                    </Show>
                    <Show when={props.status === "success"}>
                        <span class="flex items-center gap-2 text-[0.9rem] font-medium text-[#2e7d32] dark:text-[#66bb6a]">
                            {props.successLabel || "✓ Completed"}
                        </span>
                    </Show>
                    <Show when={props.status === "failed"}>
                        <span class="flex items-center gap-2 text-[0.9rem] font-medium text-[#c62828] dark:text-[#ef5350]">
                            {props.failedLabel || "✗ Failed"}
                        </span>
                    </Show>
                </div>
            </div>

            <Show when={props.error}>
                <div
                    class={`mt-4 p-3 rounded-md text-[0.9rem] flex items-start justify-between gap-4 
                        ${props.isPermissionError ? "flex-col" : ""} 
                        bg-[#fee] dark:bg-[#3a1a1a] text-[#c00] dark:text-[#ff6b6b]`}
                >
                    <pre class="m-0 whitespace-pre-wrap break-words font-sans text-[0.85rem] leading-relaxed flex-1">{props.error}</pre>
                </div>
            </Show>

            {props.renderExtra?.()}

            <Show when={props.status === "failed" && props.onAction}>
                <div class="flex justify-center mt-2">
                    <button
                        class="bg-none border-none text-[#c00] dark:text-[#ff6b6b] underline cursor-pointer text-[0.9rem] flex-shrink-0 hover:text-[#a00] dark:hover:text-[#ff8a8a]"
                        onClick={props.onAction}
                    >
                        Retry {props.actionLabel ? props.actionLabel.replace("Start ", "") : "Action"}
                    </button>
                </div>
            </Show>

            {props.children}
        </div>
    );
}
