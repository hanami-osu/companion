import { describe, expect, it } from "vitest";

import { artworkReducer } from "./artworkState";

describe("artwork retry state", () => {
    it("allows a failed URL to be attempted again after reconnection", () => {
        const initial = {
            url: "http://127.0.0.1:24050/files/beatmap/background",
            retryKey: "stale",
            failed: false,
            attempts: 0,
            generation: 0,
        };
        const failed = artworkReducer(initial, { type: "failed" });
        expect(failed.failed).toBe(true);

        const reconnected = artworkReducer(failed, {
            type: "source",
            url: initial.url,
            retryKey: "connected",
        });
        expect(reconnected.failed).toBe(false);
        expect(reconnected.attempts).toBe(0);
        expect(reconnected.generation).toBe(1);
    });
});
