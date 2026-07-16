import { useEffect, useReducer } from "react";

import { artworkReducer } from "./artworkState";

interface BeatmapArtworkProps {
    url: string | null;
    alt: string;
    className?: string;
    retryKey?: string;
}

const MAX_AUTOMATIC_RETRIES = 2;
const RETRY_DELAY_MS = 3_000;

export function BeatmapArtwork({ url, alt, className = "", retryKey = "default" }: BeatmapArtworkProps) {
    const [state, dispatch] = useReducer(artworkReducer, {
        url,
        retryKey,
        failed: false,
        attempts: 0,
        generation: 0,
    });

    useEffect(() => dispatch({ type: "source", url, retryKey }), [url, retryKey]);

    useEffect(() => {
        if (!state.failed || state.attempts >= MAX_AUTOMATIC_RETRIES) return;
        const timeout = window.setTimeout(() => dispatch({ type: "retry" }), RETRY_DELAY_MS);
        return () => window.clearTimeout(timeout);
    }, [state.attempts, state.failed]);

    const failed = state.url === url && state.retryKey === retryKey && state.failed;

    if (!url || failed) {
        return (
            <div
                className={`grid place-items-center bg-zinc-900 text-[10px] font-semibold uppercase tracking-[0.2em] text-zinc-600 ${className}`}
                aria-label="Beatmap artwork unavailable"
            >
                osu!
            </div>
        );
    }

    return (
        <img
            key={`${url}-${state.generation}`}
            src={url}
            alt={alt}
            className={`object-cover ${className}`}
            decoding="async"
            onError={() => dispatch({ type: "failed" })}
        />
    );
}
