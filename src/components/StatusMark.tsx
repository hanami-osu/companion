interface StatusMarkProps {
  active?: boolean;
  warning?: boolean;
}

export function StatusMark({ active = false, warning = false }: StatusMarkProps) {
  const tone = warning ? "bg-amber-400" : active ? "bg-hanami" : "bg-zinc-600";
  return <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${tone}`} aria-hidden="true" />;
}
