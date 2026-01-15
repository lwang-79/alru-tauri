import { createEffect, Show } from "solid-js";

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
        <div class={`mt-4 border border-[#e0e0e0] dark:border-[#444] rounded-md overflow-hidden ${props.className || ""}`}>
            <div class="flex justify-between items-center px-4 py-2 bg-[#f5f5f5] dark:bg-[#333] border-b border-[#e0e0e0] dark:border-b-[#444] text-[0.85rem] font-medium text-[#666] dark:text-[#aaa]">
                <span>{props.title}</span>
                <Show when={props.isRunning}>
                    <span class="text-[#4caf50] text-[0.8rem] animate-pulse">● Live</span>
                </Show>
            </div>
            <pre
                ref={preRef}
                class="m-0 p-4 bg-[#1e1e1e] text-[#d4d4d4] font-mono text-[0.8rem] overflow-y-auto whitespace-pre-wrap break-words"
                style={{ "max-height": props.maxHeight || "400px" }}
            >
                {props.output}
            </pre>
        </div>
    );
}
