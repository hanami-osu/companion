import { ExternalLink } from "lucide-react";

import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot } from "../../lib/types";
import { SettingRow, SettingsSection, secondaryButton } from "./SettingRow";

export function AboutSettings({
    snapshot,
    pending,
    onOpenRepository,
}: {
    snapshot: CompanionSnapshot;
    pending: ReadonlySet<ActionName>;
    onOpenRepository: () => void;
}) {
    return (
        <SettingsSection title="About">
            <SettingRow title="Version">{snapshot.appVersion}</SettingRow>
            <SettingRow title="License">MIT</SettingRow>
            <div className="py-3">
                <button
                    type="button"
                    className={secondaryButton}
                    onClick={onOpenRepository}
                    disabled={pending.has("open-repository")}
                >
                    <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" /> Repository
                </button>
            </div>
        </SettingsSection>
    );
}
