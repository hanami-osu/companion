import { Link2 } from "lucide-react";

import { StatusMark } from "../../components/StatusMark";
import type { AuthState } from "../../lib/types";

interface HanamiStatusRowProps {
  state: AuthState;
  pending: string | null;
  onConnect: () => void;
}

const labels: Record<AuthState, string> = {
  signed_out: "Not connected",
  opening_browser: "Opening browser",
  waiting_for_approval: "Waiting for approval",
  exchanging_code: "Finishing sign-in",
  signed_in: "Connected",
  refreshing: "Refreshing",
  error: "Needs attention",
};

export function HanamiStatusRow({ state, pending, onConnect }: HanamiStatusRowProps) {
  const connected = state === "signed_in" || state === "refreshing";
  const inProgress = ["opening_browser", "waiting_for_approval", "exchanging_code"].includes(state);

  return (
    <section className="flex min-h-16 items-center gap-3 border-b border-line py-3" aria-label="Hanami account">
      <Link2 className="h-4 w-4 shrink-0 text-zinc-500" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <h2 className="text-[13px] font-semibold text-zinc-100">Hanami</h2>
          <span className="inline-flex items-center gap-1.5 text-[11px] text-zinc-400">
            <StatusMark active={connected} warning={state === "error"} />
            {labels[state]}
          </span>
        </div>
        <p className="mt-0.5 text-[11px] leading-4 text-muted">Browser sign-in for your Hanami connection.</p>
      </div>
      {!connected && !inProgress && (
        <button
          type="button"
          onClick={onConnect}
          disabled={pending === "connect-hanami"}
          className="h-8 rounded-md border border-zinc-700 px-2.5 text-[11px] font-semibold text-zinc-200 transition hover:border-zinc-500 hover:text-white disabled:opacity-50"
        >
          Connect
        </button>
      )}
    </section>
  );
}
