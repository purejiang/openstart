import { invoke } from "@tauri-apps/api/core";
import type { Command, CommandStep, TerminalInfo, AppSettings, AppInfo, UpdateInfo } from "./types";

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
  steps: CommandStep[],
  note: string = "",
): Promise<Command> {
  return invoke("add_command", { name, command, terminal, autoStart, groupName, steps, note });
}

export function updateCommand(
  id: string,
  name: string,
  command: string,
  terminal: string,
  autoStart: boolean,
  groupName: string,
  steps: CommandStep[],
  note: string = "",
): Promise<void> {
  return invoke("update_command", { id, name, command, terminal, autoStart, groupName, steps, note });
}

export function executeCommandById(id: string, useExistingWindow?: boolean, groupName?: string): Promise<void> {
  return invoke("execute_command_by_id", { id, useExistingWindow: useExistingWindow ?? false, groupName: groupName ?? null });
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
  closeToTray: boolean,
  keepTerminalOpen: boolean,
): Promise<void> {
  return invoke("update_settings", { appAutostart, startupDelaySeconds, closeToTray, keepTerminalOpen });
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
}

export function checkUpdate(): Promise<UpdateInfo> {
  return invoke("check_update");
}

export function readLogs(maxLines?: number): Promise<string> {
  return invoke("read_logs", { maxLines: maxLines ?? null });
}

export function markCommandExecuted(id: string): Promise<void> {
  return invoke("mark_command_executed", { id });
}
