import type { RecentPlay } from "../../lib/types";

interface RecentActivityProps {
    plays: RecentPlay[];
}

export function RecentActivity({ plays }: RecentActivityProps) {
    return (
        <section className="border-t border-line py-4" aria-labelledby="recent-heading">
            <div className="flex items-baseline justify-between">
                <h2 id="recent-heading" className="text-[11px] font-bold uppercase tracking-[0.14em] text-zinc-500">
                    Recent activity
                </h2>
                <span className="text-[10px] text-zinc-700">This session</span>
            </div>
            {plays.length === 0 ? (
                <p className="py-5 text-xs text-zinc-600">Gameplay attempts will appear here.</p>
            ) : (
                <ol className="mt-2 divide-y divide-zinc-900">
                    {plays.slice(0, 5).map((play) => (
                        <li key={play.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 py-2.5">
                            <div className="min-w-0">
                                <p className="truncate text-xs font-medium text-zinc-200">
                                    <span className={outcomeTone(play.outcome)}>{outcomeLabel(play)}</span>
                                    <span className="mx-2 text-zinc-700">·</span>
                                    {play.beatmap.title || "Unknown beatmap"}
                                </p>
                                <p className="mt-1 truncate text-[10px] text-zinc-600">
                                    <span className="tabular-nums text-zinc-500">{formatCompletion(play.completion)} complete</span>
                                    <span>
                                        {" "}
                                        · {play.accuracy.toFixed(2)}% · {play.combo}× · {play.misses} miss
                                    </span>
                                    {play.mods.length > 0 ? ` · +${play.mods.map((mod) => mod.acronym).join("")}` : ""}
                                </p>
                            </div>
                            <div className="text-right">
                                <p className="text-[11px] font-semibold tabular-nums text-zinc-300">{play.pp == null ? "—" : `${Math.round(play.pp)} pp`}</p>
                                <time dateTime={play.timestamp} className="mt-1 block text-[9px] text-zinc-700">
                                    {formatTime(play.timestamp)}
                                </time>
                            </div>
                        </li>
                    ))}
                </ol>
            )}
        </section>
    );
}

function formatCompletion(completion: number) {
    return `${Math.round(Math.min(1, Math.max(0, completion)) * 100)}%`;
}

function outcomeLabel(play: RecentPlay) {
    switch (play.outcome) {
        case "passed":
            return play.rank ?? "Pass";
        case "failed":
            return "F";
        case "retried":
            return "Retry";
        case "quit":
            return "Quit";
    }
}

function outcomeTone(outcome: RecentPlay["outcome"]) {
    switch (outcome) {
        case "failed":
            return "text-rose-400";
        case "retried":
            return "text-zinc-400";
        case "quit":
            return "text-zinc-500";
        case "passed":
            return "text-pink-300";
    }
}

function formatTime(timestamp: string) {
    const date = new Date(timestamp);
    return Number.isNaN(date.getTime()) ? "just now" : date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
