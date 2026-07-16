import { describe, expect, it } from "vitest";

import { actionReducer, initialActionState } from "./useCompanion";

describe("actionReducer", () => {
  it("keeps unrelated actions pending when one finishes", () => {
    let state = actionReducer(initialActionState, { type: "started", action: "launch-tosu" });
    state = actionReducer(state, { type: "started", action: "connect-hanami" });
    state = actionReducer(state, { type: "succeeded", action: "launch-tosu" });

    expect(state.pending.has("launch-tosu")).toBe(false);
    expect(state.pending.has("connect-hanami")).toBe(true);
  });

  it("associates an error with the action that failed", () => {
    const state = actionReducer(initialActionState, {
      type: "failed",
      action: "disconnect-hanami",
      message: "offline",
    });

    expect(state.errors["disconnect-hanami"]).toBe("offline");
    expect(state.latestError?.action).toBe("disconnect-hanami");
  });
});
