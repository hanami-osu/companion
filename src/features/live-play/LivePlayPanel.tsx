import { BeatmapArtwork } from "../../components/BeatmapArtwork";
import type { LivePlay, NowPlaying } from "../../lib/types";
import { ModList } from "../tosu/ModList";

interface LivePlayPanelProps {
  beatmap: NowPlaying;
  play: LivePlay;
}

const scoreFormatter = new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 });

export function LivePlayPanel({ beatmap, play }: LivePlayPanelProps) {
  return (
    <section className="animate-content-in py-5" aria-labelledby="live-play-heading">
      <div className="flex gap-4">
        <BeatmapArtwork
          url={beatmap.backgroundUrl}
          alt={`${beatmap.artist} — ${beatmap.title}`}
          className="h-[92px] w-[92px] shrink-0 rounded-lg"
        />
        <div className="min-w-0 flex-1 pt-0.5">
          <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-hanami">Live play</p>
          <h2 id="live-play-heading" className="mt-2 truncate text-[17px] font-semibold tracking-tight text-zinc-50">
            {beatmap.title || "Untitled beatmap"}
          </h2>
          <p className="mt-0.5 truncate text-xs text-zinc-400">{beatmap.artist || "Unknown artist"}</p>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] font-semibold text-zinc-300">
            <span className="truncate">[{beatmap.difficulty || "Unknown difficulty"}]</span>
            <ModList mods={beatmap.mods} />
          </div>
        </div>
      </div>

      <div className="mt-5 h-1 overflow-hidden rounded-full bg-zinc-800" aria-label={`${Math.round(play.progress * 100)}% complete`}>
        <div className="h-full rounded-full bg-hanami transition-[width] duration-150" style={{ width: `${play.progress * 100}%` }} />
      </div>

      <div className="mt-5 grid grid-cols-4 divide-x divide-line border-y border-line py-3">
        <Stat label="Score" value={scoreFormatter.format(play.score)} />
        <Stat label="Accuracy" value={`${play.accuracy.toFixed(2)}%`} />
        <Stat label="Combo" value={`${scoreFormatter.format(play.combo)}×`} />
        <Stat label="Misses" value={scoreFormatter.format(play.hits.misses)} />
      </div>

      <div className="mt-3 flex items-center justify-between text-xs">
        <span className="text-muted">{play.playerName ?? "Local player"}</span>
        <span className="font-semibold text-zinc-200">
          {play.currentPp == null ? "PP unavailable" : `${Math.round(play.currentPp)} pp`}
          {play.maximumPp != null && <span className="font-normal text-zinc-600"> / {Math.round(play.maximumPp)}</span>}
        </span>
      </div>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 px-2 first:pl-0 last:pr-0">
      <p className="truncate text-[9px] font-bold uppercase tracking-[0.12em] text-zinc-600">{label}</p>
      <p className="mt-1 truncate text-[13px] font-semibold tabular-nums text-zinc-100">{value}</p>
    </div>
  );
}
