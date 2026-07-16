import { FolderOpen, Play, Power, Radio, ShieldCheck } from "lucide-react";

import { getTosuPrimaryAction, isActionBlocked, type ActionName } from "../../app/actions";
import { StatusMark } from "../../components/StatusMark";
import type { CompanionSnapshot } from "../../lib/types";

interface TosuStatusRowProps {
    snapshot: CompanionSnapshot;
    pending: ReadonlySet<ActionName>;
    onLaunch: () => void;
    onResumeTracking: () => void;
    onSelectExecutable: () => void;
    onTroubleshoot: () => void;
}

const labels = {
    disabled: "Tracking disabled",
    searching: "Searching",
    connecting: "Connecting",
    connected: "Connected",
    stale: "Connection stale",
    unavailable: "Unavailable",
    error: "Needs attention",
} as const;

export function TosuStatusRow({
    snapshot,
    pending,
    onLaunch,
    onResumeTracking,
    onSelectExecutable,
    onTroubleshoot,
}: TosuStatusRowProps) {
    const connected = snapshot.tosu.connection === "connected";
    const primaryAction = getTosuPrimaryAction(snapshot);
    const action =
        snapshot.tosu.memoryAccess === "possibly_required"
            ? { label: "Troubleshoot", icon: ShieldCheck, onClick: onTroubleshoot, pending: false }
            : !snapshot.trackingEnabled
              ? {
                    label: "Resume tracking",
                    icon: Play,
                    onClick: onResumeTracking,
                    pending: isActionBlocked("tracking", pending),
                }
              : primaryAction === "select"
                ? {
                      label: "Select executable",
                      icon: FolderOpen,
                      onClick: onSelectExecutable,
                      pending: isActionBlocked("select-tosu", pending),
                  }
                : primaryAction === "launch"
                  ? {
                        label: "Start tosu",
                        icon: Power,
                        onClick: onLaunch,
                        pending: isActionBlocked("launch-tosu", pending),
                    }
                  : null;
    const ActionIcon = action?.icon;

    return (
        <section className="flex min-h-16 items-center gap-3 border-b border-line py-3" aria-label="tosu connection">
            <Radio className="h-4 w-4 shrink-0 text-zinc-500" aria-hidden="true" />
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                    <h2 className="text-[13px] font-semibold text-zinc-100">tosu</h2>
                    <span className="inline-flex items-center gap-1.5 text-[11px] text-zinc-400">
                        <StatusMark
                            active={connected}
                            warning={
                                snapshot.tosu.connection === "error" ||
                                snapshot.tosu.connection === "stale" ||
                                snapshot.tosu.memoryAccess === "possibly_required"
                            }
                        />
                        {labels[snapshot.tosu.connection]}
                    </span>
                </div>
                <p className="mt-0.5 truncate text-[11px] leading-4 text-muted">
                    {snapshot.tosu.memoryAccess === "possibly_required"
                        ? "tosu reported a Linux memory-access problem."
                        : "Provides local osu! state to Companion."}
                </p>
            </div>
            {action && ActionIcon && (
                <button
                    type="button"
                    onClick={action.onClick}
                    disabled={action.pending}
                    className="inline-flex h-8 items-center gap-1.5 rounded-md bg-zinc-100 px-2.5 text-[11px] font-semibold text-zinc-950 transition hover:bg-white disabled:opacity-50"
                >
                    <ActionIcon className="h-3.5 w-3.5" aria-hidden="true" />
                    {action.label}
                </button>
            )}
        </section>
    );
}
