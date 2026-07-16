import { ExternalLink, FolderOpen, Play, RotateCcw, Square } from "lucide-react";

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

const tosuActions: ActionName[] = [
  "tracking",
  "tosu-auto-start",
  "launch-tosu",
  "stop-tosu",
  "select-tosu",
  "reset-tosu",
];

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
  const busy = tosuActions.some((action) => pending.has(action));
  const connected = snapshot.tosu.connection === "connected";

  return (
    <SettingsSection title="tosu">
      <SettingRow title="Status">
        <span role="status">{statusLabels[snapshot.tosu.connection]}</span>
      </SettingRow>
      <SettingRow
        title="Executable"
        description={snapshot.tosu.executablePath ?? "Not found on PATH. Select tosu.exe or the native tosu binary."}
      >
        {snapshot.tosu.executablePath
          ? snapshot.tosu.executableConfigured
            ? "Selected"
            : "Auto-detected"
          : "Not selected"}
      </SettingRow>
      <SettingRow title="Port" description="Local v2 API port used by Companion.">
        <span className="font-mono tabular-nums">{snapshot.tosu.port}</span>
      </SettingRow>
      <SettingRow title="Launch with Companion" description="Starts tosu only when its local service is not already available.">
        <button
          type="button"
          role="switch"
          aria-label="Launch tosu with Companion"
          aria-checked={snapshot.tosu.autoStart}
          onClick={() => onAutoStart(!snapshot.tosu.autoStart)}
          disabled={pending.has("tosu-auto-start")}
          className={`inline-flex h-8 min-w-[54px] items-center justify-center rounded-md px-2.5 text-[11px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:opacity-40 ${
            snapshot.tosu.autoStart ? "bg-zinc-800 text-zinc-100" : "bg-zinc-950 text-zinc-500"
          }`}
        >
          {snapshot.tosu.autoStart ? "On" : "Off"}
        </button>
      </SettingRow>
      <div className="flex flex-wrap gap-2 py-3">
        {snapshot.tosu.processOwned ? (
          <button type="button" className={secondaryButton} onClick={onStop} disabled={busy}>
            <Square className="h-3.5 w-3.5" aria-hidden="true" /> Stop tosu
          </button>
        ) : (
          <button
            type="button"
            className={primaryButton}
            onClick={onLaunch}
            disabled={busy || connected || !snapshot.trackingEnabled || !snapshot.tosu.executableAvailable}
          >
            <Play className="h-3.5 w-3.5" aria-hidden="true" /> Launch tosu
          </button>
        )}
        <button type="button" className={snapshot.tosu.executableAvailable ? secondaryButton : primaryButton} onClick={onSelect} disabled={busy || snapshot.tosu.processOwned}>
          <FolderOpen className="h-3.5 w-3.5" aria-hidden="true" /> Select executable
        </button>
        {snapshot.tosu.executableConfigured && (
          <button type="button" className={secondaryButton} onClick={onReset} disabled={busy || snapshot.tosu.processOwned}>
            <RotateCcw className="h-3.5 w-3.5" aria-hidden="true" /> Use PATH
          </button>
        )}
        <button type="button" className={secondaryButton} onClick={onOpenDashboard} disabled={!connected || pending.has("open-tosu")}>
          <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" /> Dashboard
        </button>
      </div>
    </SettingsSection>
  );
}
