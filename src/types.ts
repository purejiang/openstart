export interface TerminalInfo {
  id: string;
  name: string;
  path: string;
  available: boolean;
}

export interface CommandStep {
  cmd: string;
  delay_sec: number;
  note?: string;
}

export interface Command {
  id: string;
  name: string;
  command: string;
  terminal: string;
  auto_start: boolean;
  group_name: string;
  steps: CommandStep[];
  note: string;
  last_executed_at: string;
  created_at: string;
  updated_at: string;
}

export interface AppSettings {
  app_autostart: boolean;
  startup_delay_seconds: number;
  close_to_tray: boolean;
  keep_terminal_open: boolean;
}

export interface AppInfo {
  name: string;
  version: string;
  dataDir: string;
}

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  download_url: string;
  release_notes: string;
}
