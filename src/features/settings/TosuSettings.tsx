import { ExternalLink, FolderOpen, Play, RotateCcw, Square } from "lucide-react";

import { getTosuPrimaryAction, isActionBlocked } from "../../app/actions";
import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot, TosuConnection } from "../../lib/types";
import { SettingRow, SettingsSection, primaryButton, secondaryButton } from "./SettingRow";

const statusLabels: Record<TosuConnection, string> = {
    disabled: "Tracking paused",
    searching: "Searching",
    connecting: "Connecting",
    connected: "Connected",
    stale: "Stale — reconnecting",
    unavailable: "Not running",
    error: "Error",
};

export function TosuSettings({
    snapshot,
    pending,
    onLaunch,
    onStop,
    onSelect,
    onReset,
    onOpenDashboard,
    onAutoStart,
}: {
    snapshot: CompanionSnapshot;
    pending: ReadonlySet<ActionName>;
    onLaunch: () => void;
    onStop: () => void;
    onSelect: () => void;
    onReset: () => void;
    onOpenDashboard: () => void;
    onAutoStart: (enabled: boolean) => void;
}) {
    const connected = snapshot.tosu.connection === "connected";
    const primaryAction = getTosuPrimaryAction(snapshot);
    const executablePath = snapshot.tosu.executablePath;
    const statusLabel = connected
        ? snapshot.tosu.processOwned
            ? "Connected · Companion-owned"
            : "Connected · External"
        : statusLabels[snapshot.tosu.connection];

    return (
        <SettingsSection title="tosu">
            <SettingRow title="Status">
                <span role="status">{statusLabel}</span>
            </SettingRow>
            <SettingRow
                title="Executable"
                description={
                    executablePath ? (
                        <span className="block max-w-[245px] truncate" title={executablePath}>
                            {compactExecutablePath(executablePath)}
                        </span>
                    ) : (
                        "Not found on PATH. Select tosu.exe or the native tosu binary."
                    )
                }
            >
                {executablePath ? (
                    <button
                        type="button"
                        className="text-zinc-400 underline-offset-4 hover:text-white hover:underline disabled:opacity-40"
                        onClick={onSelect}
                        disabled={isActionBlocked("select-tosu", pending) || snapshot.tosu.processOwned}
                    >
                        {snapshot.tosu.executableConfigured ? "Change" : "Select instead"}
                    </button>
                ) : (
                    "Not selected"
                )}
            </SettingRow>
            <SettingRow
                title="Launch tosu with Companion"
                description="Opt in to starting tosu after Companion confirms its local service is not already running."
            >
                <button
                    type="button"
                    role="switch"
                    aria-label="Launch tosu with Companion"
                    aria-checked={snapshot.tosu.autoStart}
                    onClick={() => onAutoStart(!snapshot.tosu.autoStart)}
                    disabled={isActionBlocked("tosu-auto-start", pending)}
                    className={`inline-flex h-8 min-w-[54px] items-center justify-center rounded-md px-2.5 text-[11px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:opacity-40 ${
                        snapshot.tosu.autoStart ? "bg-zinc-800 text-zinc-100" : "bg-zinc-950 text-zinc-500"
                    }`}
                >
                    {snapshot.tosu.autoStart ? "On" : "Off"}
                </button>
            </SettingRow>
            <div className="flex flex-wrap gap-2 py-3">
                {primaryAction === "stop" && (
                    <button
                        type="button"
                        className={secondaryButton}
                        onClick={onStop}
                        disabled={isActionBlocked("stop-tosu", pending)}
                    >
                        <Square className="h-3.5 w-3.5" aria-hidden="true" /> Stop tosu
                    </button>
                )}
                {primaryAction === "launch" && (
                    <button
                        type="button"
                        className={primaryButton}
                        onClick={onLaunch}
                        disabled={isActionBlocked("launch-tosu", pending)}
                    >
                        <Play className="h-3.5 w-3.5" aria-hidden="true" /> Launch tosu
                    </button>
                )}
                {primaryAction === "select" && (
                    <button
                        type="button"
                        className={primaryButton}
                        onClick={onSelect}
                        disabled={isActionBlocked("select-tosu", pending)}
                    >
                        <FolderOpen className="h-3.5 w-3.5" aria-hidden="true" /> Select executable
                    </button>
                )}
                {connected && (
                    <button
                        type="button"
                        className={secondaryButton}
                        onClick={onOpenDashboard}
                        disabled={pending.has("open-tosu")}
                    >
                        <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" /> Dashboard
                    </button>
                )}
                {snapshot.tosu.executableConfigured && !snapshot.tosu.processOwned && (
                    <button
                        type="button"
                        className="inline-flex h-8 items-center gap-1.5 px-1 text-[10px] text-zinc-600 hover:text-zinc-300 disabled:opacity-40"
                        onClick={onReset}
                        disabled={isActionBlocked("reset-tosu", pending)}
                    >
                        <RotateCcw className="h-3 w-3" aria-hidden="true" /> Use PATH discovery
                    </button>
                )}
            </div>
        </SettingsSection>
    );
}

function compactExecutablePath(path: string): string {
    if (path.length <= 44) return path;
    const separator = path.includes("\\") ? "\\" : "/";
    const parts = path.split(separator).filter(Boolean);
    const fileName = parts[parts.length - 1] ?? path;
    const root = path.startsWith(separator) ? separator : parts[0]?.endsWith(":") ? `${parts[0]}${separator}` : "";
    return `${root}…${separator}${fileName}`;
}
