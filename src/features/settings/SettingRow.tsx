import type { ReactNode } from "react";

export function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-11 items-center gap-4 py-2">
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-zinc-200">{title}</p>
        {description && <p className="mt-0.5 text-[10px] leading-4 text-zinc-600">{description}</p>}
      </div>
      <div className="min-w-0 shrink-0 text-right text-[11px] text-zinc-400">{children}</div>
    </div>
  );
}

export function SettingsSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section aria-labelledby={`settings-${title.toLowerCase()}`}>
      <h2
        id={`settings-${title.toLowerCase()}`}
        className="mb-2 text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600"
      >
        {title}
      </h2>
      <div className="space-y-0.5">{children}</div>
    </section>
  );
}

export const secondaryButton =
  "inline-flex h-8 items-center justify-center gap-1.5 rounded-md bg-zinc-900 px-2.5 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:cursor-not-allowed disabled:opacity-40";

export const primaryButton =
  "inline-flex h-8 items-center justify-center gap-1.5 rounded-md bg-zinc-100 px-2.5 text-[11px] font-semibold text-zinc-950 transition hover:bg-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-hanami disabled:cursor-not-allowed disabled:opacity-40";
