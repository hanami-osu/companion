import { Pause, Radio } from "lucide-react";

import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot } from "../../lib/types";
import { SettingRow, SettingsSection } from "./SettingRow";

export function GeneralSettings({
  snapshot,
  pending,
  onTracking,
}: {
  snapshot: CompanionSnapshot;
  pending: ReadonlySet<ActionName>;
  onTracking: (enabled: boolean) => void;
}) {
  return (
    <SettingsSection title="General">
      <SettingRow title="Background tracking" description="Pauses Companion's connection without stopping tosu.">
        <button
          type="button"
          role="switch"
          aria-checked={snapshot.trackingEnabled}
          onClick={() => onTracking(!snapshot.trackingEnabled)}
          disabled={pending.has("tracking")}
          className={`inline-flex h-8 min-w-[72px] items-center justify-center gap-1.5 rounded-md px-2.5 font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:opacity-40 ${
            snapshot.trackingEnabled
              ? "bg-zinc-800 text-zinc-100"
              : "bg-zinc-950 text-zinc-500"
          }`}
        >
          {snapshot.trackingEnabled ? <Radio className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
          {snapshot.trackingEnabled ? "On" : "Paused"}
        </button>
      </SettingRow>
      <SettingRow title="Close to tray" description="Closing the main window keeps background tracking active.">
        Always
      </SettingRow>
    </SettingsSection>
  );
}
