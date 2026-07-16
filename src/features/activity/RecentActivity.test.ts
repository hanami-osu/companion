import { describe, expect, it } from "vitest";

import { formatCompletion, formatMisses } from "./format";

describe("recent activity formatting", () => {
    it("pluralizes misses correctly", () => {
        expect(formatMisses(0)).toBe("0 misses");
        expect(formatMisses(1)).toBe("1 miss");
        expect(formatMisses(2)).toBe("2 misses");
    });

    it("clamps completion to a percentage", () => {
        expect(formatCompletion(0.426)).toBe("43%");
        expect(formatCompletion(2)).toBe("100%");
    });
});
