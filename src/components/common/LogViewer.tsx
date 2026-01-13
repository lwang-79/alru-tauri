import { createEffect, Show } from "solid-js";
import "./LogViewer.css";

interface LogViewerProps {
    output: string;
    isRunning?: boolean;
    title: string;
    className?: string;
    maxHeight?: string;
}

export function LogViewer(props: LogViewerProps) {
    let preRef: HTMLPreElement | undefined;

    createEffect(() => {
        // Access the output to track dependency
        void props.output;
        // Auto-scroll to bottom when output changes
        if (preRef) {
            preRef.scrollTop = preRef.scrollHeight;
        }
    });

    return (
        <div class={`log-viewer-container ${props.className || ""}`}>
            <div class="log-viewer-header">
                <span>{props.title}</span>
                <Show when={props.isRunning}>
                    <span class="live-indicator">● Live</span>
                </Show>
            </div>
            <pre
                ref={preRef}
                class="log-viewer-content"
                style={{ "max-height": props.maxHeight || "400px" }}
            >
                {props.output}
            </pre>
        </div>
    );
}
