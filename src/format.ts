/** Shared, dependency-free formatters. */

const pad = (n: number) => String(n).padStart(2, "0");

/** `2026-11-23 10:36` — used for tooltips and metadata rows. */
export function fmtDateTime(value: string): string {
  if (!value) return "—";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "—";
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Short human delta: `just now`, `4m ago`, `yesterday`, … */
export function fmtRelative(value: string): string {
  if (!value) return "Never";
  const t = Date.parse(value);
  if (Number.isNaN(t)) return "—";
  const seconds = Math.max(0, Math.floor((Date.now() - t) / 1000));
  if (seconds < 45) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days}d ago`;
  return fmtDateTime(value).slice(0, 10);
}

/** Elapsed duration for a live run: `18s`, `3m 05s`, `1h 12m`. */
export function fmtElapsed(startedAt: string, endedAt?: string): string {
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return "—";
  const parsedEnd = endedAt ? Date.parse(endedAt) : Date.now();
  const end = Number.isNaN(parsedEnd) ? Date.now() : parsedEnd;
  const seconds = Math.max(0, Math.floor((end - start) / 1000));
  if (seconds >= 3600) return `${Math.floor(seconds / 3600)}h ${pad(Math.floor((seconds % 3600) / 60))}m`;
  if (seconds >= 60) return `${Math.floor(seconds / 60)}m ${pad(seconds % 60)}s`;
  return `${seconds}s`;
}
