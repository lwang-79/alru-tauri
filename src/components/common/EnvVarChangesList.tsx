import { For, Show } from "solid-js";
import type { EnvVarChange } from "../../types";
import "./EnvVarChangesList.css";

interface EnvVarChangesListProps {
    changes: EnvVarChange[];
    title?: string;
    children?: any;
}

export function EnvVarChangesList(props: EnvVarChangesListProps) {
    return (
        <Show when={props.changes.length > 0}>
            <div class="env-var-changes-section">
                <Show when={props.title}>
                    <h4>{props.title}</h4>
                </Show>
                <Show when={!props.title}>
                    <h4>Environment Variable Changes</h4>
                </Show>
                <div class="env-var-changes-optimized">
                    <For each={props.changes}>
                        {(change) => (
                            <div class="env-var-change-optimized">
                                <div class="env-var-change-line">
                                    <span class="env-var-scope">
                                        {change.level.toUpperCase()}:
                                    </span>
                                    <code class="env-var-name">{change.key}</code>
                                    <Show when={change.old_value && change.new_value}>
                                        <span class="env-var-action">updated</span>
                                        <span class="env-var-from-to">
                                            <span class="env-var-old">{change.old_value}</span>
                                            <span class="env-var-separator">→</span>
                                            <span class="env-var-new">{change.new_value}</span>
                                        </span>
                                    </Show>
                                    <Show when={change.old_value && !change.new_value}>
                                        <span class="env-var-action removed">removed</span>
                                        <span class="env-var-old-only">
                                            was {change.old_value}
                                        </span>
                                    </Show>
                                    <Show when={!change.old_value && change.new_value}>
                                        <span class="env-var-action added">added</span>
                                        <span class="env-var-new-only">{change.new_value}</span>
                                    </Show>
                                </div>
                            </div>
                        )}
                    </For>
                </div>
                {props.children}
            </div>
        </Show>
    );
}
