import { describe, expect, it } from "vitest";

import { initialSnapshot } from "../lib/types";
import { getTosuPrimaryAction, hasPendingTosuOperation, isActionBlocked, type ActionName } from "./actions";

describe("tosu action policy", () => {
    it("blocks overlapping lifecycle operations without blocking Hanami or dashboard actions", () => {
        const pending = new Set<ActionName>(["launch-tosu"]);
        expect(hasPendingTosuOperation(pending)).toBe(true);
        expect(isActionBlocked("tracking", pending)).toBe(true);
        expect(isActionBlocked("select-tosu", pending)).toBe(true);
        expect(isActionBlocked("grant-memory-access", pending)).toBe(true);
        expect(isActionBlocked("open-tosu", pending)).toBe(false);
        expect(isActionBlocked("connect-hanami", pending)).toBe(false);
    });

    it("chooses only actions Rust can currently accept", () => {
        const snapshot = structuredClone(initialSnapshot);
        expect(getTosuPrimaryAction(snapshot)).toBe("select");

        snapshot.tosu.connection = "connected";
        expect(getTosuPrimaryAction(snapshot)).toBeNull();

        snapshot.tosu.executableAvailable = true;
        snapshot.tosu.connection = "unavailable";
        expect(getTosuPrimaryAction(snapshot)).toBe("launch");

        for (const connection of ["connecting", "connected", "stale"] as const) {
            snapshot.tosu.connection = connection;
            expect(getTosuPrimaryAction(snapshot)).toBeNull();
        }

        snapshot.tosu.processOwned = true;
        expect(getTosuPrimaryAction(snapshot)).toBe("stop");
        snapshot.tosu.processOwned = false;
        snapshot.trackingEnabled = false;
        expect(getTosuPrimaryAction(snapshot)).toBeNull();
    });
});
