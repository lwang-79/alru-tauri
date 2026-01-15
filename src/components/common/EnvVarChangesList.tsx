import { For, Show } from "solid-js";
import type { EnvVarChange } from "../../types";

interface EnvVarChangesListProps {
    changes: EnvVarChange[];
    title?: string;
    children?: any;
}

export function EnvVarChangesList(props: EnvVarChangesListProps) {
    return (
        <Show when={props.changes.length > 0}>
            <div class="mt-6 p-4 bg-[#f8f9fa] dark:bg-[#2a2a2a] rounded-lg border border-[#e9ecef] dark:border-[#444]">
                <Show when={props.title}>
                    <h4 class="m-0 mb-4 text-[#495057] dark:text-[#eee] text-base font-semibold">{props.title}</h4>
                </Show>
                <Show when={!props.title}>
                    <h4 class="m-0 mb-4 text-[#495057] dark:text-[#eee] text-base font-semibold">Environment Variable Changes</h4>
                </Show>
                <div class="flex flex-col gap-2 mb-4">
                    <For each={props.changes}>
                        {(change) => (
                            <div class="p-3 bg-white dark:bg-[#333] rounded-md border border-[#dee2e6] dark:border-[#444]">
                                <div class="flex items-center gap-3 flex-wrap text-sm leading-[1.4] dark:text-[#eee]">
                                    <span class="text-[0.75rem] font-semibold text-[#6c757d] dark:text-[#aaa] uppercase min-w-[40px]">
                                        {change.level.toUpperCase()}:
                                    </span>
                                    <code class="font-mono text-sm bg-[#f8f9fa] dark:bg-[#444] px-2 py-1 rounded text-[#495057] dark:text-[#ddd] font-semibold">{change.key}</code>
                                    <Show when={change.old_value && change.new_value}>
                                        <span class="font-medium px-2 py-1 rounded text-[0.75rem] lowercase bg-[#cfe2ff] text-[#084298] dark:bg-[#1a3a5c] dark:text-[#64b5f6]">updated</span>
                                        <span class="flex items-center gap-2 flex-wrap">
                                            <span class="font-mono bg-[#f8d7da] text-[#721c24] dark:bg-[#4a1f1f] dark:text-[#e57373] px-1.5 py-0.5 rounded text-[0.8rem] line-through">{change.old_value}</span>
                                            <span class="text-[#6c757d] dark:text-[#aaa] font-bold">→</span>
                                            <span class="font-mono bg-[#d4edda] text-[#155724] dark:bg-[#1b3a24] dark:text-[#81c784] px-1.5 py-0.5 rounded text-[0.8rem] font-medium">{change.new_value}</span>
                                        </span>
                                    </Show>
                                    <Show when={change.old_value && !change.new_value}>
                                        <span class="font-medium px-2 py-1 rounded text-[0.75rem] lowercase bg-[#f8d7da] text-[#721c24] dark:bg-[#4a1f1f] dark:text-[#e57373]">removed</span>
                                        <span class="font-mono text-[#6c757d] dark:text-[#aaa] text-[0.8rem] italic">
                                            was {change.old_value}
                                        </span>
                                    </Show>
                                    <Show when={!change.old_value && change.new_value}>
                                        <span class="font-medium px-2 py-1 rounded text-[0.75rem] lowercase bg-[#d4edda] text-[#155724] dark:bg-[#1b3a24] dark:text-[#81c784]">added</span>
                                        <span class="font-mono bg-[#d4edda] text-[#155724] dark:bg-[#1b3a24] dark:text-[#81c784] px-1.5 py-0.5 rounded text-[0.8rem] font-medium">{change.new_value}</span>
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
