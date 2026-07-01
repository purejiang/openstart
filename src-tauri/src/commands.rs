use crate::storage::{Command, Storage};
use crate::terminal;
use crate::autostart;
use crate::settings::{self, AppSettings};
use std::process::Command as ProcessCommand;
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

/// Wrapper for app data directory path, managed as Tauri state.
pub struct AppDataDir(pub std::path::PathBuf);

/// Detect available terminals on the system.
#[tauri::command]
pub fn detect_terminals() -> Vec<terminal::TerminalInfo> {
    terminal::detect_terminals()
}

/// List all saved commands.
#[tauri::command]
pub fn list_commands(storage: State<'_, Storage>) -> Result<Vec<Command>, String> {
    storage.list_commands()
}

/// Add a new command (generates id + timestamps).
#[tauri::command(rename_all = "camelCase")]
pub fn add_command(
    storage: State<'_, Storage>,
    name: String,
    command: String,
    terminal: String,
    auto_start: bool,
    group_name: String,
) -> Result<Command, String> {
    let now = Utc::now().to_rfc3339();
    let cmd = Command {
        id: Uuid::new_v4().to_string(),
        name,
        command,
        terminal,
        auto_start,
        group_name,
        created_at: now.clone(),
        updated_at: now,
    };
    storage.add_command(&cmd)?;
    Ok(cmd)
}

/// Update an existing command.
#[tauri::command(rename_all = "camelCase")]
pub fn update_command(
    storage: State<'_, Storage>,
    id: String,
    name: String,
    command: String,
    terminal: String,
    auto_start: bool,
    group_name: String,
) -> Result<(), String> {
    let mut existing = storage.get_command(&id)?;
    existing.name = name;
    existing.command = command;
    existing.terminal = terminal;
    existing.auto_start = auto_start;
    existing.group_name = group_name;
    storage.update_command(&existing)
}

/// Delete a command by id.
#[tauri::command]
pub fn delete_command(storage: State<'_, Storage>, id: String) -> Result<(), String> {
    storage.delete_command(&id)
}

/// Execute a command in the specified terminal.
///
/// Terminal IDs:
/// - `powershell` — standalone PowerShell window
/// - `cmd`        — standalone CMD window
/// - `gitbash`    — standalone Git Bash window
/// - `terminal`           — Windows Terminal (defaults to PowerShell)
/// - `terminal:<name>`     — WT with named profile (dynamic from settings.json)
#[tauri::command]
pub fn execute_command(
    name: String,
    command: String,
    terminal: String,
) -> Result<(), String> {
    spawn_terminal(&command, &terminal)
        .map_err(|e| format!("Failed to execute '{}' in {} terminal: {}", name, terminal, e))
}

/// Spawn a terminal process with the given command (no Tauri State needed).
pub fn spawn_terminal(command: &str, terminal: &str) -> Result<(), String> {
    let (program, args): (String, Vec<String>) = match terminal {
        "powershell" => (
            "powershell.exe".into(),
            vec!["-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
        ),
        "cmd" => (
            "cmd.exe".into(),
            vec!["/k".into(), command.to_string()],
        ),
        "gitbash" => (
            "bash.exe".into(),
            vec!["-i".into(), "-c".into(), command.to_string()],
        ),
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };

            if profile_name.is_empty() {
                (
                    "wt.exe".into(),
                    vec!["powershell.exe".into(), "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
                )
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                let shell = profile.as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell");

                match shell {
                    "gitbash" => {
                        let bash = crate::terminal::find_bash_path()
                            .unwrap_or_else(|| "bash".to_string());
                        (
                            "wt.exe".into(),
                            vec![
                                "-p".into(), profile_name.to_string(), "--".into(),
                                bash, "-i".into(), "-c".into(), command.to_string(),
                            ],
                        )
                    }
                    "cmd" => (
                        "wt.exe".into(),
                        vec![
                            "-p".into(), profile_name.to_string(), "--".into(),
                            "cmd.exe".into(), "/k".into(), command.to_string(),
                        ],
                    ),
                    _ => (
                        "wt.exe".into(),
                        vec![
                            "-p".into(), profile_name.to_string(), "--".into(),
                            "powershell.exe".into(), "-NoExit".into(), "-Command".into(),
                            format!("& {{{}}}", command),
                        ],
                    ),
                }
            }
        }
        _ => return Err(format!("Unknown terminal type: {}", terminal)),
    };

    ProcessCommand::new(&program)
        .args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .spawn()
        .map_err(|e| format!("{}", e))?;

    Ok(())
}

/// Build args for a single wt.exe `new-tab` segment (no program prefix).
pub fn build_wt_tab_args(command: &str, terminal: &str) -> Result<Vec<String>, String> {
    match terminal {
        "powershell" => Ok(vec![
            "new-tab".into(), "powershell.exe".into(),
            "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command),
        ]),
        "cmd" => Ok(vec![
            "new-tab".into(), "cmd.exe".into(), "/k".into(), command.to_string(),
        ]),
        "gitbash" => {
            let bash = crate::terminal::find_bash_path().unwrap_or_else(|| "bash".to_string());
            Ok(vec!["new-tab".into(), bash, "-i".into(), "-c".into(), command.to_string()])
        }
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };
            if profile_name.is_empty() {
                Ok(vec!["new-tab".into(), "powershell.exe".into(),
                    "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)])
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                let shell = profile.as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell");
                match shell {
                    "gitbash" => {
                        let bash = crate::terminal::find_bash_path().unwrap_or_else(|| "bash".to_string());
                        Ok(vec!["new-tab".into(), bash, "-i".into(), "-c".into(), command.to_string()])
                    }
                    "cmd" => Ok(vec!["new-tab".into(), "cmd.exe".into(), "/k".into(), command.to_string()]),
                    _ => Ok(vec!["new-tab".into(), "powershell.exe".into(),
                        "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)]),
                }
            }
        }
        _ => Err(format!("Unknown terminal: {}", terminal)),
    }
}

/// Spawn multiple commands as tabs in a single Windows Terminal window.
pub fn spawn_terminal_batch(commands: &[(String, String)]) -> Result<(), String> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut all_args: Vec<String> = vec![];
    for (i, (cmd, term)) in commands.iter().enumerate() {
        if i > 0 {
            all_args.push(";".into());
        }
        all_args.append(&mut build_wt_tab_args(cmd, term)?);
    }
    ProcessCommand::new("wt.exe").args(&all_args).spawn()
        .map_err(|e| format!("{}", e))?;
    Ok(())
}

/// Execute all commands in a group. If all are WT profiles → tabs in one window.
#[tauri::command]
pub fn execute_group(storage: State<'_, Storage>, group_name: String) -> Result<(), String> {
    let cmds = storage.get_group_commands(&group_name)?;
    if cmds.is_empty() {
        return Ok(());
    }
    // Check if all commands use Windows Terminal (terminal / terminal:*)
    let all_wt = cmds.iter().all(|c| c.terminal.starts_with("terminal"));
    if all_wt {
        let batch: Vec<(String, String)> = cmds
            .iter()
            .map(|c| (c.command.clone(), c.terminal.clone()))
            .collect();
        spawn_terminal_batch(&batch)
    } else {
        // Mixed terminals → spawn individually
        for cmd in &cmds {
            let _ = spawn_terminal(&cmd.command, &cmd.terminal);
        }
        Ok(())
    }
}

/// Get all commands marked for auto-start.
#[tauri::command]
pub fn get_auto_start_commands(storage: State<'_, Storage>) -> Result<Vec<Command>, String> {
    storage.get_auto_start_commands()
}

/// Enable or disable app auto-start in Windows registry.
#[tauri::command]
pub fn toggle_app_autostart(enable: bool) -> Result<(), String> {
    if enable {
        autostart::enable_app_autostart()
    } else {
        autostart::disable_app_autostart()
    }
}

/// Check if app is registered for auto-start.
#[tauri::command]
pub fn get_app_autostart_status() -> Result<bool, String> {
    autostart::is_app_autostart_enabled()
}

// ── Settings ──────────────────────────────────────────────────────────

/// Delete all commands in a group.
#[tauri::command]
pub fn delete_group(storage: State<'_, Storage>, group_name: String) -> Result<usize, String> {
    storage.delete_group_commands(&group_name)
}

/// Get current app settings.
#[tauri::command]
pub fn get_settings(data_dir: State<'_, AppDataDir>) -> AppSettings {
    settings::load_settings(&data_dir.0)
}

/// Update app settings.
#[tauri::command(rename_all = "camelCase")]
pub fn update_settings(
    data_dir: State<'_, AppDataDir>,
    app_autostart: bool,
    startup_delay_seconds: u64,
) -> Result<(), String> {
    let s = AppSettings {
        app_autostart,
        startup_delay_seconds,
    };
    settings::save_settings(&data_dir.0, &s)?;

    // Sync registry
    if app_autostart {
        autostart::enable_app_autostart()
    } else {
        autostart::disable_app_autostart()
    }
}

/// Get app info (version, data dir).
#[tauri::command]
pub fn get_app_info(data_dir: State<'_, AppDataDir>) -> serde_json::Value {
    serde_json::json!({
        "name": "OpenStart",
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": data_dir.0.to_string_lossy(),
    })
}
