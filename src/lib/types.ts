export type TosuConnection =
  | "disabled"
  | "searching"
  | "connecting"
  | "connected"
  | "stale"
  | "unavailable"
  | "error";
export type TosuMemoryAccess =
  | "not_applicable"
  | "unknown"
  | "available"
  | "possibly_required"
  | "unavailable";

export type OsuState = "menu" | "song_select" | "gameplay" | "results" | "unknown";
export type Ruleset = "osu" | "taiko" | "catch" | "mania" | "unknown";
export type AuthState =
  | "signed_out"
  | "opening_browser"
  | "waiting_for_approval"
  | "exchanging_code"
  | "signed_in"
  | "refreshing"
  | "error";

export interface BeatmapSummary {
  beatmapId: number | null;
  beatmapSetId: number | null;
  checksum: string | null;
  artist: string;
  title: string;
  difficulty: string;
  mapper: string;
  ruleset: Ruleset;
  backgroundUrl: string | null;
  maxCombo: number | null;
}

export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface CompanionMod {
  acronym: string;
  settings: Record<string, JsonValue>;
}

export interface NowPlaying extends BeatmapSummary {
  mods: CompanionMod[];
}

export interface HitCounts {
  great: number;
  ok: number;
  meh: number;
  katu: number;
  geki: number;
  misses: number;
}

export interface LivePlay {
  playerName: string | null;
  score: number;
  accuracy: number;
  combo: number;
  maximumCombo: number | null;
  hits: HitCounts;
  currentPp: number | null;
  maximumPp: number | null;
  progress: number;
  elapsedSeconds: number;
  failed: boolean;
  rank: string | null;
}

export type PlayOutcome = "passed" | "failed" | "retried" | "quit";

export interface RecentPlay {
  id: string;
  scoreId: number | null;
  beatmap: BeatmapSummary;
  startedAt: string;
  endedAt: string;
  playerName: string | null;
  score: number;
  accuracy: number;
  combo: number;
  misses: number;
  mods: CompanionMod[];
  pp: number | null;
  completion: number | null;
  outcome: PlayOutcome;
  rank: string | null;
}

export interface CompanionSnapshot {
  trackingEnabled: boolean;
  tosu: {
    connection: TosuConnection;
    processOwned: boolean;
    executableAvailable: boolean;
    executablePath: string | null;
    executableConfigured: boolean;
    autoStart: boolean;
    port: number;
    memoryAccess: TosuMemoryAccess;
    message: string | null;
  };
  osu: {
    running: boolean;
    state: OsuState;
    client: string | null;
  };
  nowPlaying: NowPlaying | null;
  livePlay: LivePlay | null;
  recentActivity: RecentPlay[];
  auth: {
    state: AuthState;
    message: string | null;
    baseUrl: string;
    isProduction: boolean;
  };
  appVersion: string;
  uploadAvailable: boolean;
}

export const initialSnapshot: CompanionSnapshot = {
  trackingEnabled: true,
  tosu: {
    connection: "searching",
    processOwned: false,
    executableAvailable: false,
    executablePath: null,
    executableConfigured: false,
    autoStart: true,
    port: 24050,
    memoryAccess: "unknown",
    message: null,
  },
  osu: { running: false, state: "unknown", client: null },
  nowPlaying: null,
  livePlay: null,
  recentActivity: [],
  auth: {
    state: "signed_out",
    message: null,
    baseUrl: "https://hanami.yorunoken.com",
    isProduction: true,
  },
  appVersion: "0.1.0",
  uploadAvailable: false,
};
