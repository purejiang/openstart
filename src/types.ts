export interface TerminalInfo {
  id: string;
  name: string;
  path: string;
  available: boolean;
}

export interface Command {
  id: string;
  name: string;
  command: string;
  terminal: string;
  auto_start: boolean;
  group_name: string;
  created_at: string;
  updated_at: string;
}

export interface AppSettings {
  app_autostart: boolean;
  startup_delay_seconds: number;
}

export interface AppInfo {
  name: string;
  version: string;
  dataDir: string;
}
