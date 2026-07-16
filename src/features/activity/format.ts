export function formatCompletion(completion: number) {
  return `${Math.round(Math.min(1, Math.max(0, completion)) * 100)}%`;
}

export function formatMisses(misses: number) {
  return `${misses} ${misses === 1 ? "miss" : "misses"}`;
}
