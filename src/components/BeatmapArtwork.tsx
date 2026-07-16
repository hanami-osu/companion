import { useState } from "react";

interface BeatmapArtworkProps {
  url: string | null;
  alt: string;
  className?: string;
}

export function BeatmapArtwork({ url, alt, className = "" }: BeatmapArtworkProps) {
  const [failedUrl, setFailedUrl] = useState<string | null>(null);

  if (!url || failedUrl === url) {
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
      key={url}
      src={url}
      alt={alt}
      className={`object-cover ${className}`}
      decoding="async"
      onError={() => setFailedUrl(url)}
    />
  );
}
