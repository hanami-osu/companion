import { ChevronDown, ShieldCheck } from "lucide-react";

import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot, TosuMemoryAccess } from "../../lib/types";
import { secondaryButton, SettingsSection } from "./SettingRow";

const memoryLabels: Record<TosuMemoryAccess, string> = {
    not_applicable: "Not applicable",
    unknown: "Unknown — no failure reported",
    available: "Available",
    possibly_required: "Possibly required",
    unavailable: "Could not inspect",
};

export function AdvancedSettings({ snapshot, pending, onGrantMemoryAccess }: { snapshot: CompanionSnapshot; pending: ReadonlySet<ActionName>; onGrantMemoryAccess: () => void }) {
    return (
        <SettingsSection title="Advanced">
            <details className="group py-3">
                <summary className="flex cursor-pointer list-none items-center justify-between text-xs font-medium text-zinc-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami">
                    Diagnostics
                    <ChevronDown className="h-4 w-4 text-zinc-600 transition group-open:rotate-180" aria-hidden="true" />
                </summary>
                <dl className="mt-3 space-y-2 pl-1 text-[10px]">
                    <Diagnostic label="Connection" value={snapshot.tosu.connection} />
                    <Diagnostic label="Process owner" value={snapshot.tosu.processOwned ? "Companion" : "External or none"} />
                    <Diagnostic label="osu! client" value={snapshot.osu.client ?? "Not detected"} />
                    <Diagnostic label="Linux memory access" value={memoryLabels[snapshot.tosu.memoryAccess]} />
                    <Diagnostic label="Play uploads" value="Unavailable — backend endpoint not implemented" />
                </dl>
                {snapshot.tosu.memoryAccess === "possibly_required" && (
                    <div className="mt-3">
                        <p className="mb-2 text-[10px] leading-4 text-amber-200/80">
                            tosu reported a relevant memory-read failure. This advanced action applies ptrace capability to the resolved native binary after system authorization.
                        </p>
                        <button type="button" className={secondaryButton} onClick={onGrantMemoryAccess} disabled={pending.has("grant-memory-access")}>
                            <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" /> Grant ptrace access
                        </button>
                    </div>
                )}
            </details>
        </SettingsSection>
    );
}

function Diagnostic({ label, value }: { label: string; value: string }) {
    return (
        <div className="flex gap-4">
            <dt className="w-28 shrink-0 text-zinc-600">{label}</dt>
            <dd className="min-w-0 flex-1 break-words text-right text-zinc-400">{value.replace(/_/g, " ")}</dd>
        </div>
    );
}
