import type { CompanionSnapshot } from "../lib/types";

export type ActionName =
    | "tracking"
    | "tosu-auto-start"
    | "launch-tosu"
    | "stop-tosu"
    | "select-tosu"
    | "reset-tosu"
    | "grant-memory-access"
    | "connect-hanami"
    | "disconnect-hanami"
    | "open-tosu"
    | "open-hanami"
    | "open-repository";

const conflictingTosuActions = new Set<ActionName>([
    "tracking",
    "tosu-auto-start",
    "launch-tosu",
    "stop-tosu",
    "select-tosu",
    "reset-tosu",
    "grant-memory-access",
]);

export function hasPendingTosuOperation(pending: ReadonlySet<ActionName>): boolean {
    return [...conflictingTosuActions].some((action) => pending.has(action));
}

export function isActionBlocked(action: ActionName, pending: ReadonlySet<ActionName>): boolean {
    if (pending.has(action)) return true;
    return conflictingTosuActions.has(action) && hasPendingTosuOperation(pending);
}

export type TosuPrimaryAction = "select" | "launch" | "stop" | null;

export function getTosuPrimaryAction(snapshot: CompanionSnapshot): TosuPrimaryAction {
    if (snapshot.tosu.processOwned) return "stop";
    if (!snapshot.trackingEnabled || ["connecting", "connected", "stale"].includes(snapshot.tosu.connection)) {
        return null;
    }
    if (!snapshot.tosu.executableAvailable) return "select";
    return "launch";
}
