import { ExternalLink, Link2, LogOut } from "lucide-react";

import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot } from "../../lib/types";
import { SettingRow, SettingsSection, primaryButton, secondaryButton } from "./SettingRow";

export function HanamiSettings({
    snapshot,
    pending,
    onConnect,
    onDisconnect,
    onOpen,
}: {
    snapshot: CompanionSnapshot;
    pending: ReadonlySet<ActionName>;
    onConnect: () => void;
    onDisconnect: () => void;
    onOpen: () => void;
}) {
    const connected = snapshot.auth.state === "signed_in" || snapshot.auth.state === "refreshing";
    const authBusy = ["connect-hanami", "disconnect-hanami"].some((action) => pending.has(action as ActionName));

    return (
        <SettingsSection title="Hanami">
            <SettingRow title="Account" description={snapshot.auth.message ?? "Browser authorization with PKCE."}>
                {connected ? "Connected" : "Not connected"}
            </SettingRow>
            <SettingRow title="Server" description={!snapshot.auth.isProduction ? snapshot.auth.baseUrl : undefined}>
                {snapshot.auth.isProduction ? "Production" : "Development"}
            </SettingRow>
            <div className="flex flex-wrap gap-2 py-3">
                {connected ? (
                    <button type="button" className={secondaryButton} onClick={onDisconnect} disabled={authBusy}>
                        <LogOut className="h-3.5 w-3.5" aria-hidden="true" /> Disconnect
                    </button>
                ) : (
                    <button type="button" className={primaryButton} onClick={onConnect} disabled={authBusy}>
                        <Link2 className="h-3.5 w-3.5" aria-hidden="true" /> Connect Hanami
                    </button>
                )}
                <button
                    type="button"
                    className={secondaryButton}
                    onClick={onOpen}
                    disabled={pending.has("open-hanami")}
                >
                    <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" /> Open website
                </button>
            </div>
        </SettingsSection>
    );
}
