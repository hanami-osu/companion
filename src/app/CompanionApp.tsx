import { useCallback, useRef, useState } from "react";
import { Settings2, X } from "lucide-react";

import { StatusMark } from "../components/StatusMark";
import { RecentActivity } from "../features/activity/RecentActivity";
import { HanamiStatusRow } from "../features/auth/HanamiStatusRow";
import { IdlePanel } from "../features/live-play/IdlePanel";
import { LivePlayPanel } from "../features/live-play/LivePlayPanel";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TosuStatusRow } from "../features/tosu/TosuStatusRow";
import { companionCommands } from "../lib/companion";
import { selectStatusMessage } from "./statusMessage";
import { useCompanion, type ActionName } from "./useCompanion";

export function CompanionApp() {
    const { snapshot, loading, pending, actionError, latestActionError, clearError, run } = useCompanion();
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [settingsTarget, setSettingsTarget] = useState<"advanced" | null>(null);
    const settingsButtonRef = useRef<HTMLButtonElement>(null);
    const activePlay =
        snapshot.tosu.connection === "connected" &&
        snapshot.osu.state === "gameplay" &&
        snapshot.livePlay &&
        snapshot.nowPlaying
            ? { beatmap: snapshot.nowPlaying, play: snapshot.livePlay }
            : null;
    const overall = loading
        ? "Starting"
        : activePlay
          ? "Live play"
          : snapshot.tosu.connection === "connected"
            ? snapshot.osu.running
                ? "Ready"
                : "Ready for plays"
            : "Setup needed";

    const action = (name: ActionName, callback: () => Promise<void>) => () => void run(name, callback);
    const closeSettings = useCallback(() => {
        setSettingsOpen(false);
        setSettingsTarget(null);
        requestAnimationFrame(() => settingsButtonRef.current?.focus());
    }, []);
    const openSettings = useCallback((target: "advanced" | null = null) => {
        setSettingsTarget(target);
        setSettingsOpen(true);
    }, []);

    if (settingsOpen) {
        return (
            <SettingsPage
                snapshot={snapshot}
                pending={pending}
                actionError={latestActionError}
                onClearError={clearError}
                onBack={closeSettings}
                onTracking={(enabled) => void run("tracking", () => companionCommands.setTracking(enabled))}
                onTosuAutoStart={(enabled) =>
                    void run("tosu-auto-start", () => companionCommands.setTosuAutoStart(enabled))
                }
                onLaunchTosu={action("launch-tosu", companionCommands.launchTosu)}
                onStopTosu={action("stop-tosu", companionCommands.stopTosu)}
                onSelectTosu={action("select-tosu", companionCommands.selectTosu)}
                onResetTosu={action("reset-tosu", companionCommands.resetTosu)}
                onGrantMemoryAccess={action("grant-memory-access", companionCommands.grantTosuMemoryAccess)}
                onConnect={action("connect-hanami", companionCommands.connectHanami)}
                onDisconnect={action("disconnect-hanami", companionCommands.disconnectHanami)}
                onOpenTosu={action("open-tosu", companionCommands.openTosu)}
                onOpenHanami={action("open-hanami", companionCommands.openHanami)}
                onOpenRepository={action("open-repository", companionCommands.openRepository)}
                focusAdvanced={settingsTarget === "advanced"}
            />
        );
    }

    return (
        <div className="relative flex h-dvh min-h-0 flex-col overflow-hidden bg-canvas text-zinc-100">
            <header className="flex h-[68px] shrink-0 items-center justify-between border-b border-line px-5">
                <p className="text-[10px] font-bold uppercase tracking-[0.17em] text-hanami">Hanami Companion</p>
                <div className="flex items-center gap-3">
                    <span
                        className="inline-flex items-center gap-2 text-[11px] font-medium text-zinc-400"
                        role="status"
                        aria-live="polite"
                    >
                        <StatusMark
                            active={snapshot.tosu.connection === "connected"}
                            warning={snapshot.tosu.connection === "error" || snapshot.tosu.connection === "stale"}
                        />
                        {overall}
                    </span>
                    <button
                        ref={settingsButtonRef}
                        type="button"
                        onClick={() => openSettings()}
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
                        onSelectExecutable={action("select-tosu", companionCommands.selectTosu)}
                        onTroubleshoot={() => openSettings("advanced")}
                    />
                    <HanamiStatusRow
                        state={snapshot.auth.state}
                        pending={pending}
                        onConnect={action("connect-hanami", companionCommands.connectHanami)}
                    />
                </div>

                {actionError && (
                    <div
                        className="mt-4 flex items-start gap-3 border-l-2 border-rose-400 bg-rose-400/5 px-3 py-2.5 text-[11px] leading-4 text-rose-200"
                        role="alert"
                    >
                        <p className="min-w-0 flex-1">{actionError}</p>
                        <button
                            type="button"
                            onClick={clearError}
                            className="text-rose-300 hover:text-white"
                            aria-label="Dismiss error"
                        >
                            <X className="h-3.5 w-3.5" aria-hidden="true" />
                        </button>
                    </div>
                )}

                {activePlay ? (
                    <LivePlayPanel
                        beatmap={activePlay.beatmap}
                        play={activePlay.play}
                        connection={snapshot.tosu.connection}
                    />
                ) : (
                    <IdlePanel
                        connection={snapshot.tosu.connection}
                        osuRunning={snapshot.osu.running}
                        beatmap={snapshot.nowPlaying}
                    />
                )}

                <RecentActivity plays={snapshot.recentActivity} />
            </div>

            <footer className="flex min-h-10 shrink-0 items-center justify-between border-t border-line px-5 text-[10px] text-zinc-600">
                <span>{snapshot.trackingEnabled ? "Tracking in background" : "Tracking paused"}</span>
                <span className="max-w-[220px] truncate text-right">{selectStatusMessage(snapshot, actionError)}</span>
            </footer>
        </div>
    );
}
