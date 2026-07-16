import { describe, expect, it } from "vitest";

import { initialSnapshot } from "../lib/types";
import { selectStatusMessage } from "./statusMessage";

describe("selectStatusMessage", () => {
    it("prioritizes action and authentication errors over ordinary tosu information", () => {
        const snapshot = structuredClone(initialSnapshot);
        snapshot.tosu.message = "tosu is not running";
        snapshot.auth.state = "error";
        snapshot.auth.message = "Stored session could not be restored";
        expect(selectStatusMessage(snapshot, null)).toBe("Stored session could not be restored");
        expect(selectStatusMessage(snapshot, "Launch failed")).toBe("Launch failed");
    });

    it("keeps a stale tosu warning above lower-priority auth information", () => {
        const snapshot = structuredClone(initialSnapshot);
        snapshot.tosu.connection = "stale";
        snapshot.tosu.message = "tosu stopped responding";
        snapshot.auth.message = "Connected to Hanami";
        expect(selectStatusMessage(snapshot, null)).toBe("tosu stopped responding");
    });
});
