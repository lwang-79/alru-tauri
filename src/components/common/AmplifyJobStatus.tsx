import { Show } from "solid-js";
import type { AmplifyJobDetails } from "../../types";
import "./AmplifyJobStatus.css";

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

    return (
        <div class="job-status-section">
            <h4>Amplify Deployment Job</h4>
            <div class="job-info">
                <div class="job-detail">
                    <span class="job-label">Job ID:</span>
                    <code class="job-value">{props.job.job_id}</code>
                </div>
                <div class="job-detail">
                    <span class="job-label">Status:</span>
                    <div class="job-status-container">
                        <span
                            class={`job-status-badge ${props.job.status.toLowerCase()}`}
                        >
                            {props.job.status}
                        </span>
                        <Show when={props.job.status === "RUNNING"}>
                            <span class="spinner-small job-status-spinner"></span>
                        </Show>
                    </div>
                </div>
                <Show when={props.job.start_time}>
                    <div class="job-detail">
                        <span class="job-label">Started:</span>
                        <span class="job-value">
                            {formatLocalDateTime(props.job.start_time!)}
                        </span>
                    </div>
                </Show>
                <Show when={props.job.end_time}>
                    <div class="job-detail">
                        <span class="job-label">Ended:</span>
                        <span class="job-value">
                            {formatLocalDateTime(props.job.end_time!)}
                        </span>
                    </div>
                </Show>
            </div>
        </div>
    );
}
