import type { TerminalInfo } from "./types";

const LABELS: Record<string, string> = {
  powershell: "PowerShell",
  cmd: "CMD",
  gitbash: "Git Bash",
  terminal: "Windows Terminal",
};

const ACCENTS: Record<string, string> = {
  powershell: "is-powershell",
  cmd: "is-cmd",
  gitbash: "is-gitbash",
  terminal: "is-terminal",
};

/** Human name for a terminal id, including Windows Terminal profiles. */
export function terminalLabelById(id: string): string {
  if (id.startsWith("terminal:")) return `WT: ${id.slice("terminal:".length)}`;
  return LABELS[id] ?? id;
}

export function terminalLabel(term: TerminalInfo): string {
  return terminalLabelById(term.id) || term.name;
}

/** CSS modifier used for the terminal colour dot. */
export function terminalAccent(id: string): string {
  if (id.startsWith("terminal")) return ACCENTS.terminal;
  return ACCENTS[id] ?? "is-default";
}
