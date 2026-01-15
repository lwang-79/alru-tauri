import { Show } from "solid-js";
import type { AmplifyJobDetails } from "../../types";

interface AmplifyJobStatusProps {
    job: AmplifyJobDetails;
}

export function AmplifyJobStatus(props: AmplifyJobStatusProps) {
    const formatLocalDateTime = (dateString: string) => {
        const date = new Date(dateString);

        return date.toLocaleString("en-US", {
            year: "numeric",
            month: "short", // This gives us "Jan", "Feb", "Mar", etc.
            day: "2-digit",
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
            hour12: true,
        });
    };

    const statusBadgeClasses = () => {
        switch (props.job.status.toUpperCase()) {
            case "PENDING":
                return "bg-[#fff3cd] text-[#856404] dark:bg-[#4a3c10] dark:text-[#ffd54f]";
            case "RUNNING":
                return "bg-[#cfe2ff] text-[#084298] dark:bg-[#1a3a5c] dark:text-[#64b5f6]";
            case "SUCCEED":
                return "bg-[#d1e7dd] text-[#0f5132] dark:bg-[#1b3a24] dark:text-[#81c784]";
            case "FAILED":
                return "bg-[#f8d7da] text-[#842029] dark:bg-[#4a1f1f] dark:text-[#e57373]";
            case "CANCELLED":
                return "bg-[#e2e3e5] text-[#41464b] dark:bg-[#333] dark:text-[#aaa]";
            default:
                return "bg-[#f5f5f5] text-[#666]";
        }
    };

    return (
        <div class="mt-6 p-4 bg-[#f8f9fa] dark:bg-[#2a2a2a] rounded-lg border border-[#e0e0e0] dark:border-[#444]">
            <h4 class="m-0 mb-4 text-base font-semibold text-[#333] dark:text-[#eee]">Amplify Deployment Job</h4>
            <div class="flex flex-col gap-3">
                <div class="flex items-center gap-2">
                    <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[80px]">Job ID:</span>
                    <code class="text-[#333] dark:text-[#eee]">{props.job.job_id}</code>
                </div>
                <div class="flex items-center gap-2">
                    <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[80px]">Status:</span>
                    <div class="flex items-center gap-2">
                        <span
                            class={`px-3 py-1 rounded-full text-sm font-medium uppercase ${statusBadgeClasses()}`}
                        >
                            {props.job.status}
                        </span>
                        <Show when={props.job.status === "RUNNING"}>
                            <span class="w-4 h-4 border-2 border-[rgba(8,66,152,0.3)] dark:border-[rgba(92,140,230,0.3)] border-t-[#084298] dark:border-t-[#5c8ce6] rounded-full animate-spin inline-block"></span>
                        </Show>
                    </div>
                </div>
                <Show when={props.job.start_time}>
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[80px]">Started:</span>
                        <span class="text-[#333] dark:text-[#eee]">
                            {formatLocalDateTime(props.job.start_time!)}
                        </span>
                    </div>
                </Show>
                <Show when={props.job.end_time}>
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-[#666] dark:text-[#aaa] min-w-[80px]">Ended:</span>
                        <span class="text-[#333] dark:text-[#eee]">
                            {formatLocalDateTime(props.job.end_time!)}
                        </span>
                    </div>
                </Show>
            </div>
        </div>
    );
}
