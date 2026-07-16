import type { CompanionMod, JsonValue } from "../../lib/types";

interface ModListProps {
    mods: CompanionMod[];
}

const modClassName = "rounded bg-hanami/15 px-1.5 py-0.5 text-[10px] font-semibold text-hanami";
const noMod: CompanionMod = { acronym: "NM", settings: {} };

export function ModList({ mods }: ModListProps) {
    const displayedMods = mods.length === 0 ? [noMod] : mods;

    return (
        <div
            className="flex min-w-0 flex-wrap items-center gap-1"
            aria-label={`Selected mods: ${displayedMods.map((mod) => describeMod(mod)).join(", ")}`}
        >
            {displayedMods.map((mod) => (
                <span key={mod.acronym} className={modClassName} title={describeMod(mod)}>
                    {mod.acronym}
                </span>
            ))}
        </div>
    );
}

function describeMod(mod: CompanionMod) {
    const settings = Object.entries(mod.settings);
    if (settings.length === 0) return mod.acronym;
    return `${mod.acronym} (${settings.map(([key, value]) => `${formatKey(key)}: ${formatValue(value)}`).join(", ")})`;
}

function formatKey(key: string) {
    return key.replace(/_/g, " ");
}

function formatValue(value: JsonValue) {
    if (typeof value === "string") return value;
    return JSON.stringify(value);
}
