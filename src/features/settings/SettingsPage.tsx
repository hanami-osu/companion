import { useEffect, useRef } from "react";
import { ArrowLeft } from "lucide-react";

import type { ActionName } from "../../app/useCompanion";
import type { CompanionSnapshot } from "../../lib/types";
import { AboutSettings } from "./AboutSettings";
import { AdvancedSettings } from "./AdvancedSettings";
import { GeneralSettings } from "./GeneralSettings";
import { HanamiSettings } from "./HanamiSettings";
import { TosuSettings } from "./TosuSettings";

export interface SettingsPageProps {
  snapshot: CompanionSnapshot;
  pending: ReadonlySet<ActionName>;
  actionError: { action: ActionName; message: string } | null;
  onClearError: () => void;
  onBack: () => void;
  onTracking: (enabled: boolean) => void;
  onTosuAutoStart: (enabled: boolean) => void;
  onLaunchTosu: () => void;
  onStopTosu: () => void;
  onSelectTosu: () => void;
  onResetTosu: () => void;
  onGrantMemoryAccess: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onOpenTosu: () => void;
  onOpenHanami: () => void;
  onOpenRepository: () => void;
}

export function SettingsPage(props: SettingsPageProps) {
  const backRef = useRef<HTMLButtonElement>(null);
  const { onBack } = props;

  useEffect(() => {
    backRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onBack();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onBack]);

  return (
    <main className="flex h-dvh min-h-0 flex-col bg-canvas text-zinc-100" aria-labelledby="settings-title">
      <header className="flex h-[68px] shrink-0 items-center gap-3 border-b border-line px-5">
        <button
          ref={backRef}
          type="button"
          onClick={props.onBack}
          className="grid h-8 w-8 place-items-center rounded-md text-zinc-500 transition hover:bg-zinc-900 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami"
          aria-label="Back to Companion"
        >
          <ArrowLeft className="h-4 w-4" aria-hidden="true" />
        </button>
        <div>
          <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-hanami">Hanami Companion</p>
          <h1 id="settings-title" className="mt-1 text-sm font-semibold text-zinc-100">Settings</h1>
        </div>
      </header>

      <div className="min-h-0 flex-1 space-y-6 overflow-y-auto px-5 py-5">
        {props.actionError && (
          <div className="flex items-start gap-3 border-l-2 border-rose-400 bg-rose-400/5 px-3 py-2.5 text-[10px] leading-4 text-rose-200" role="alert">
            <p className="min-w-0 flex-1">
              <span className="font-semibold capitalize">{props.actionError.action.replace(/-/g, " ")}:</span>{" "}
              {props.actionError.message}
            </p>
            <button type="button" onClick={props.onClearError} className="text-rose-300 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami">
              Dismiss
            </button>
          </div>
        )}
        <GeneralSettings snapshot={props.snapshot} pending={props.pending} onTracking={props.onTracking} />
        <TosuSettings
          snapshot={props.snapshot}
          pending={props.pending}
          onLaunch={props.onLaunchTosu}
          onStop={props.onStopTosu}
          onSelect={props.onSelectTosu}
          onReset={props.onResetTosu}
          onOpenDashboard={props.onOpenTosu}
          onAutoStart={props.onTosuAutoStart}
        />
        <HanamiSettings
          snapshot={props.snapshot}
          pending={props.pending}
          onConnect={props.onConnect}
          onDisconnect={props.onDisconnect}
          onOpen={props.onOpenHanami}
        />
        <AdvancedSettings snapshot={props.snapshot} pending={props.pending} onGrantMemoryAccess={props.onGrantMemoryAccess} />
        <AboutSettings snapshot={props.snapshot} pending={props.pending} onOpenRepository={props.onOpenRepository} />
      </div>
    </main>
  );
}
