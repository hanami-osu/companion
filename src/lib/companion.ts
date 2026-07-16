import { invoke } from "@tauri-apps/api/core";

import type { CompanionSnapshot } from "./types";

export const companionCommands = {
    snapshot: () => invoke<CompanionSnapshot>("get_snapshot"),
    setTracking: (enabled: boolean) => invoke<void>("set_tracking_enabled", { enabled }),
    setTosuAutoStart: (enabled: boolean) => invoke<void>("set_tosu_auto_start", { enabled }),
    launchTosu: () => invoke<void>("launch_tosu"),
    stopTosu: () => invoke<void>("stop_owned_tosu"),
    selectTosu: () => invoke<void>("select_tosu_executable"),
    resetTosu: () => invoke<void>("reset_tosu_executable"),
    grantTosuMemoryAccess: () => invoke<void>("grant_tosu_memory_access"),
    connectHanami: () => invoke<void>("connect_hanami"),
    disconnectHanami: () => invoke<void>("disconnect_hanami"),
    openTosu: () => invoke<void>("open_tosu_dashboard"),
    openHanami: () => invoke<void>("open_hanami_website"),
    openRepository: () => invoke<void>("open_repository"),
};

export function errorMessage(error: unknown): string {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    return "The action could not be completed.";
}
