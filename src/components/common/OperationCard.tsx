import { Show, type JSX } from "solid-js";
import "./OperationCard.css";

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
    return (
        <div class={`operation-card ${props.status} ${props.className || ""}`}>
            <div class="operation-header">
                <div class="operation-number">{props.stepNumber}</div>
                <div class="operation-info">
                    <h3>{props.title}</h3>
                    <div class="operation-description">{props.description}</div>
                </div>
                <div class="operation-status">
                    <Show when={props.status === "pending"}>
                        <Show
                            when={props.onAction}
                            fallback={
                                <Show when={props.pendingLabel}>
                                    <span class="status-indicator pending">
                                        {props.pendingLabel}
                                    </span>
                                </Show>
                            }
                        >
                            <button class="action-button" onClick={props.onAction}>
                                {props.actionLabel || "Start"}
                            </button>
                        </Show>
                    </Show>
                    <Show when={props.status === "running"}>
                        <span class="status-indicator running">
                            <span class="spinner-small"></span>
                            {props.runningLabel || "Running..."}
                        </span>
                    </Show>
                    <Show when={props.status === "success"}>
                        <span class="status-indicator success">
                            {props.successLabel || "✓ Completed"}
                        </span>
                    </Show>
                    <Show when={props.status === "failed"}>
                        <span class="status-indicator failed">
                            {props.failedLabel || "✗ Failed"}
                        </span>
                    </Show>
                </div>
            </div>

            <Show when={props.error}>
                <div
                    class={`operation-error ${props.isPermissionError ? "permission-error" : ""}`}
                >
                    <pre class="error-message-text">{props.error}</pre>
                </div>
            </Show>

            {props.renderExtra?.()}

            <Show when={props.status === "failed" && props.onAction}>
                <div class="operation-retry-row">
                    <button class="retry-link" onClick={props.onAction}>
                        Retry {props.actionLabel ? props.actionLabel.replace("Start ", "") : "Action"}
                    </button>
                </div>
            </Show>

            {props.children}
        </div>
    );
}
