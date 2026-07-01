import { invoke } from "@tauri-apps/api/core";
import type { Command, TerminalInfo, AppSettings, AppInfo } from "./types";

export function detectTerminals(): Promise<TerminalInfo[]> {
  return invoke("detect_terminals");
}

export function listCommands(): Promise<Command[]> {
  return invoke("list_commands");
}

export function addCommand(
  name: string,
  command: string,
  terminal: string,
  autoStart: boolean,
  groupName: string,
): Promise<Command> {
  return invoke("add_command", { name, command, terminal, autoStart, groupName });
}

export function updateCommand(
  id: string,
  name: string,
  command: string,
  terminal: string,
  autoStart: boolean,
  groupName: string,
): Promise<void> {
  return invoke("update_command", { id, name, command, terminal, autoStart, groupName });
}

export function deleteCommand(id: string): Promise<void> {
  return invoke("delete_command", { id });
}

export function executeCommand(
  name: string,
  command: string,
  terminal: string,
): Promise<void> {
  return invoke("execute_command", { name, command, terminal });
}

export function getAutoStartCommands(): Promise<Command[]> {
  return invoke("get_auto_start_commands");
}

export function toggleAppAutostart(enable: boolean): Promise<void> {
  return invoke("toggle_app_autostart", { enable });
}

export function getAppAutostartStatus(): Promise<boolean> {
  return invoke("get_app_autostart_status");
}

export function executeGroup(groupName: string): Promise<void> {
  return invoke("execute_group", { groupName });
}

export function deleteGroup(groupName: string): Promise<number> {
  return invoke("delete_group", { groupName });
}

export function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export function updateSettings(
  appAutostart: boolean,
  startupDelaySeconds: number,
): Promise<void> {
  return invoke("update_settings", { appAutostart, startupDelaySeconds });
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
}
