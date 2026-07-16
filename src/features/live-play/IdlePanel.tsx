import { Gamepad2 } from "lucide-react";

import { BeatmapArtwork } from "../../components/BeatmapArtwork";
import type { NowPlaying, TosuConnection } from "../../lib/types";
import { ModList } from "../tosu/ModList";

interface IdlePanelProps {
    connection: TosuConnection;
    osuRunning: boolean;
    beatmap: NowPlaying | null;
}

export function IdlePanel({ connection, osuRunning, beatmap }: IdlePanelProps) {
    const heading = connection !== "connected" ? "Connect to tosu" : osuRunning ? "Waiting for a play" : "Ready for plays";
    const copy =
        connection !== "connected"
            ? "Start tosu, then Companion will connect to its local v2 service automatically."
            : osuRunning
              ? "Choose a beatmap and start playing."
              : "tosu is connected. Open osu! and Companion will pick up your next play.";

    return (
        <section className="animate-content-in py-5" aria-labelledby="idle-heading">
            {connection === "connected" && beatmap ? (
                <div className="flex items-center gap-4">
                    <BeatmapArtwork url={beatmap.backgroundUrl} alt={`${beatmap.artist} — ${beatmap.title}`} className="h-[76px] w-[76px] shrink-0 rounded-lg opacity-80" />
                    <div className="min-w-0">
                        <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-zinc-600">Selected beatmap</p>
                        <h2 id="idle-heading" className="mt-1.5 truncate text-[15px] font-semibold text-zinc-100">
                            {beatmap.title}
                        </h2>
                        <p className="mt-0.5 truncate text-xs text-muted">
                            {beatmap.artist} · [{beatmap.difficulty}]
                        </p>
                        <div className="mt-2">
                            <ModList mods={beatmap.mods} />
                        </div>
                    </div>
                </div>
            ) : (
                <div className="flex min-h-32 items-center gap-4">
                    <div className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-zinc-900">
                        <Gamepad2 className="h-5 w-5 text-zinc-500" aria-hidden="true" />
                    </div>
                    <div>
                        <h2 id="idle-heading" className="text-base font-semibold tracking-tight text-zinc-100">
                            {heading}
                        </h2>
                        <p className="mt-1 max-w-[285px] text-xs leading-5 text-muted">{copy}</p>
                    </div>
                </div>
            )}
        </section>
    );
}
