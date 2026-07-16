import { useState } from "react";
import { Settings2, X } from "lucide-react";

import { StatusMark } from "../components/StatusMark";
import { RecentActivity } from "../features/activity/RecentActivity";
import { HanamiStatusRow } from "../features/auth/HanamiStatusRow";
import { IdlePanel } from "../features/live-play/IdlePanel";
import { LivePlayPanel } from "../features/live-play/LivePlayPanel";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import { TosuStatusRow } from "../features/tosu/TosuStatusRow";
import { companionCommands } from "../lib/companion";
import { useCompanion } from "./useCompanion";

export function CompanionApp() {
    const { snapshot, loading, pending, actionError, clearError, run } = useCompanion();
    const [settingsOpen, setSettingsOpen] = useState(false);
    const activePlay = snapshot.osu.state === "gameplay" && snapshot.livePlay && snapshot.nowPlaying ? { beatmap: snapshot.nowPlaying, play: snapshot.livePlay } : null;
    const overall = loading ? "Starting" : activePlay ? "Live play" : snapshot.tosu.connection === "connected" ? (snapshot.osu.running ? "Ready" : "Waiting for osu!") : "Setup needed";

    const action = (name: string, callback: () => Promise<void>) => () => void run(name, callback);

    return (
        <div className="relative flex h-dvh min-h-0 flex-col overflow-hidden bg-canvas text-zinc-100">
            <header className="flex h-[68px] shrink-0 items-center justify-between border-b border-line px-5">
                <div>
                    <p className="text-[10px] font-bold uppercase tracking-[0.17em] text-hanami">Hanami</p>
                    <h1 className="mt-1 text-[17px] font-semibold tracking-[-0.025em] text-white">Companion</h1>
                </div>
                <div className="flex items-center gap-3">
                    <span className="inline-flex items-center gap-2 text-[11px] font-medium text-zinc-400" role="status" aria-live="polite">
                        <StatusMark active={snapshot.tosu.connection === "connected"} warning={snapshot.tosu.connection === "error"} />
                        {overall}
                    </span>
                    <button
                        type="button"
                        onClick={() => setSettingsOpen(true)}
                        className="grid h-8 w-8 place-items-center rounded-md text-zinc-500 transition hover:bg-zinc-900 hover:text-zinc-100"
                        aria-label="Open settings"
                    >
                        <Settings2 className="h-4 w-4" aria-hidden="true" />
                    </button>
                </div>
            </header>

            <div className="min-h-0 flex-1 overflow-y-auto px-5">
                <div aria-live="polite">
                    <TosuStatusRow
                        snapshot={snapshot}
                        pending={pending}
                        onLaunch={action("launch-tosu", companionCommands.launchTosu)}
                        onResumeTracking={action("tracking", () => companionCommands.setTracking(true))}
                        onGrantMemoryAccess={action("grant-memory-access", companionCommands.grantTosuMemoryAccess)}
                    />
                    <HanamiStatusRow state={snapshot.auth.state} pending={pending} onConnect={action("connect-hanami", companionCommands.connectHanami)} />
                </div>

                {actionError && (
                    <div className="mt-4 flex items-start gap-3 border-l-2 border-rose-400 bg-rose-400/5 px-3 py-2.5 text-[11px] leading-4 text-rose-200" role="alert">
                        <p className="min-w-0 flex-1">{actionError}</p>
                        <button type="button" onClick={clearError} className="text-rose-300 hover:text-white" aria-label="Dismiss error">
                            <X className="h-3.5 w-3.5" aria-hidden="true" />
                        </button>
                    </div>
                )}

                {activePlay ? (
                    <LivePlayPanel beatmap={activePlay.beatmap} play={activePlay.play} />
                ) : (
                    <IdlePanel connection={snapshot.tosu.connection} osuRunning={snapshot.osu.running} beatmap={snapshot.nowPlaying} />
                )}

                <RecentActivity plays={snapshot.recentActivity} />
            </div>

            <footer className="flex min-h-10 shrink-0 items-center justify-between border-t border-line px-5 text-[10px] text-zinc-600">
                <span>{snapshot.trackingEnabled ? "Tracking in background" : "Tracking paused"}</span>
                <span className="max-w-[220px] truncate text-right">{snapshot.tosu.message ?? snapshot.auth.message ?? "Uploads unavailable in this prototype"}</span>
            </footer>

            {settingsOpen && (
                <SettingsPanel
                    snapshot={snapshot}
                    pending={pending}
                    onClose={() => setSettingsOpen(false)}
                    onTracking={(enabled) => void run("tracking", () => companionCommands.setTracking(enabled))}
                    onLaunchTosu={action("launch-tosu", companionCommands.launchTosu)}
                    onStopTosu={action("stop-tosu", companionCommands.stopTosu)}
                    onGrantMemoryAccess={action("grant-memory-access", companionCommands.grantTosuMemoryAccess)}
                    onConnect={action("connect-hanami", companionCommands.connectHanami)}
                    onDisconnect={action("disconnect-hanami", companionCommands.disconnectHanami)}
                    onOpenTosu={action("open-tosu", companionCommands.openTosu)}
                    onOpenHanami={action("open-hanami", companionCommands.openHanami)}
                />
            )}
        </div>
    );
}
