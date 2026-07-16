import type { CompanionSnapshot } from "../lib/types";

const DEFAULT_MESSAGE = "Uploads unavailable in this prototype";

export function selectStatusMessage(snapshot: CompanionSnapshot, actionError: string | null): string {
    if (actionError) return actionError;
    if (snapshot.auth.state === "error" && snapshot.auth.message) return snapshot.auth.message;
    if (["error", "stale"].includes(snapshot.tosu.connection) && snapshot.tosu.message) {
        return snapshot.tosu.message;
    }
    if (snapshot.auth.state === "refreshing" && snapshot.auth.message) return snapshot.auth.message;
    return snapshot.tosu.message ?? snapshot.auth.message ?? DEFAULT_MESSAGE;
}
