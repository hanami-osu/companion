import {
  ExternalLink,
  LogOut,
  Play,
  Power,
  Radio,
  ShieldCheck,
  Square,
  X,
} from "lucide-react";

import { StatusMark } from "../../components/StatusMark";
import type { CompanionSnapshot } from "../../lib/types";

interface SettingsPanelProps {
  snapshot: CompanionSnapshot;
  pending: string | null;
  onClose: () => void;
  onTracking: (enabled: boolean) => void;
  onLaunchTosu: () => void;
  onStopTosu: () => void;
  onGrantMemoryAccess: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onOpenTosu: () => void;
  onOpenHanami: () => void;
}

const tosuLabels = {
  disabled: "Companion tracking paused",
  searching: "Looking for tosu",
  connecting: "Connecting to tosu",
  connected: "tosu connected",
  unavailable: "tosu unavailable",
  error: "tosu needs attention",
} as const;

export function SettingsPanel(props: SettingsPanelProps) {
  const { snapshot, pending } = props;
  const authenticated =
    snapshot.auth.state === "signed_in" || snapshot.auth.state === "refreshing";
  const connected = snapshot.tosu.connection === "connected";
  const externallyManaged = connected && !snapshot.tosu.processOwned;

  return (
    <aside
      className="absolute inset-0 z-20 flex animate-panel-in flex-col bg-canvas"
      aria-label="Settings"
    >
      <header className="flex h-[68px] shrink-0 items-center justify-between border-b border-line px-5">
        <div>
          <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-hanami">
            Hanami Companion
          </p>
          <h2 className="mt-1 text-sm font-semibold text-zinc-100">Settings</h2>
        </div>
        <button
          type="button"
          onClick={props.onClose}
          className="grid h-8 w-8 place-items-center rounded-md text-zinc-500 transition hover:bg-zinc-900 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami"
          aria-label="Close settings"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
        <SettingsSection title="tosu service">
          <SettingRow
            title={tosuLabels[snapshot.tosu.connection]}
            detail={
              externallyManaged
                ? "Running outside Companion. Companion will never stop this process."
                : snapshot.tosu.processOwned
                  ? "Started and owned by Companion."
                  : "tosu provides local osu! state to Companion."
            }
          >
            <span className="inline-flex items-center gap-2 text-[10px] font-medium text-zinc-400">
              <StatusMark
                active={connected}
                warning={snapshot.tosu.connection === "error"}
              />
              {connected ? "Online" : "Offline"}
            </span>
          </SettingRow>

          {snapshot.tosu.executableAvailable && snapshot.tosu.memoryAccess !== "not_required" && (
            <SettingRow
              title={
                snapshot.tosu.memoryAccess === "granted"
                  ? "osu! memory access granted"
                  : snapshot.tosu.memoryAccess === "required"
                    ? "osu! memory access required"
                    : "Memory access unavailable"
              }
              detail={
                snapshot.tosu.memoryAccess === "granted"
                  ? "tosu has the Linux capability needed to read osu!lazer state."
                  : snapshot.tosu.memoryAccess === "required"
                    ? "Authorize the installed tosu binary through the system privilege prompt."
                    : "Companion could not verify the native tosu binary or required system tools."
              }
            >
              <StatusMark
                active={snapshot.tosu.memoryAccess === "granted"}
                warning={snapshot.tosu.memoryAccess !== "granted"}
              />
            </SettingRow>
          )}

          {snapshot.tosu.memoryAccess === "required" && (
            <ActionRow
              icon={ShieldCheck}
              label="Grant memory access"
              detail="Uses the native system authorization prompt. Companion never runs as root."
              onClick={props.onGrantMemoryAccess}
              disabled={pending === "grant-memory-access"}
            />
          )}

          {snapshot.tosu.processOwned ? (
            <ActionRow
              icon={Square}
              label="Stop tosu"
              detail="Only the process started by Companion will be stopped."
              onClick={props.onStopTosu}
              disabled={pending === "stop-tosu"}
              tone="danger"
            />
          ) : !connected ? (
            <ActionRow
              icon={Power}
              label="Start tosu"
              detail="Launch the installed tosu application."
              onClick={props.onLaunchTosu}
              disabled={pending === "launch-tosu" || snapshot.tosu.memoryAccess === "required"}
            />
          ) : null}

          <ActionRow
            icon={ExternalLink}
            label="Open tosu dashboard"
            onClick={props.onOpenTosu}
            disabled={!connected}
          />

        </SettingsSection>

        <SettingsSection title="Companion behavior">
          <SettingRow
            title="Background tracking"
            detail="Pauses Companion's connection only. tosu keeps running."
          >
            <button
              type="button"
              role="switch"
              aria-label="Background tracking"
              aria-checked={snapshot.trackingEnabled}
              onClick={() => props.onTracking(!snapshot.trackingEnabled)}
              disabled={pending === "tracking"}
              className={`inline-flex h-8 min-w-[72px] items-center justify-center gap-1.5 rounded-md border px-2.5 text-[10px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:cursor-wait disabled:opacity-50 ${
                snapshot.trackingEnabled
                  ? "border-hanami/40 bg-hanami/10 text-pink-200"
                  : "border-zinc-800 bg-zinc-950 text-zinc-400"
              }`}
            >
              {snapshot.trackingEnabled ? (
                <Radio className="h-3.5 w-3.5" aria-hidden="true" />
              ) : (
                <Play className="h-3.5 w-3.5" aria-hidden="true" />
              )}
              {snapshot.trackingEnabled ? "On" : "Paused"}
            </button>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title="Hanami account">
          <SettingRow
            title={authenticated ? "Connected to Hanami" : "Not connected"}
            detail={
              snapshot.auth.isProduction
                ? "Production server"
                : `Local development server · ${snapshot.auth.baseUrl}`
            }
          >
            <StatusMark
              active={authenticated}
              warning={snapshot.auth.state === "error"}
            />
          </SettingRow>
          {authenticated ? (
            <ActionRow
              icon={LogOut}
              label="Disconnect Hanami"
              onClick={props.onDisconnect}
              disabled={pending === "disconnect-hanami"}
              tone="danger"
            />
          ) : (
            <ActionRow
              icon={Radio}
              label="Connect Hanami"
              detail="Opens authorization in your system browser."
              onClick={props.onConnect}
              disabled={pending === "connect-hanami"}
            />
          )}
          <ActionRow
            icon={ExternalLink}
            label={snapshot.auth.isProduction ? "Open Hanami website" : "Open local Hanami Web"}
            onClick={props.onOpenHanami}
          />
        </SettingsSection>

        <SettingsSection title="About & diagnostics">
          <dl className="divide-y divide-zinc-900 text-[11px]">
            <Diagnostic label="Version" value={snapshot.appVersion} />
            <Diagnostic label="tosu API" value={snapshot.tosu.connection} />
            <Diagnostic label="Memory access" value={snapshot.tosu.memoryAccess.replace("_", " ")} />
            <Diagnostic label="osu! client" value={snapshot.osu.client ?? "Not detected"} />
            <Diagnostic
              label="tosu owner"
              value={
                snapshot.tosu.processOwned
                  ? "Companion"
                  : connected
                    ? "External"
                    : "None"
              }
            />
            <Diagnostic label="Play uploads" value="Unavailable until the backend API exists" />
          </dl>
        </SettingsSection>
      </div>
    </aside>
  );
}

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="pt-5">
      <h3 className="mb-2 text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
        {title}
      </h3>
      <div className="border-y border-line">{children}</div>
    </section>
  );
}

function SettingRow({
  title,
  detail,
  children,
}: {
  title: string;
  detail: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-4 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-zinc-200">{title}</p>
        <p className="mt-0.5 text-[10px] leading-4 text-zinc-600">{detail}</p>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function ActionRow({
  icon: Icon,
  label,
  detail,
  onClick,
  disabled = false,
  tone = "default",
}: {
  icon: typeof Radio;
  label: string;
  detail?: string;
  onClick: () => void;
  disabled?: boolean;
  tone?: "default" | "danger";
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`flex w-full items-center gap-3 border-t border-zinc-900 py-3 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-hanami disabled:cursor-not-allowed disabled:text-zinc-700 ${
        tone === "danger" ? "text-rose-300 hover:text-rose-200" : "text-zinc-300 hover:text-white"
      }`}
    >
      <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
      <span className="min-w-0 flex-1">
        <span className="block text-xs">{label}</span>
        {detail && <span className="mt-0.5 block text-[10px] leading-4 text-zinc-600">{detail}</span>}
      </span>
    </button>
  );
}

function Diagnostic({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-4 py-2.5">
      <dt className="w-24 shrink-0 text-zinc-600">{label}</dt>
      <dd className="min-w-0 flex-1 text-right text-zinc-300">{value}</dd>
    </div>
  );
}
