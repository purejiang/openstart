use crate::storage::{Command, CommandStep, Storage};
use crate::terminal;
use crate::autostart;
use crate::script;
use crate::settings::{self, AppSettings};
use crate::logger;
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
    steps: Vec<CommandStep>,
    note: String,
) -> Result<Command, String> {
    let now = Utc::now().to_rfc3339();
    let cmd = Command {
        id: Uuid::new_v4().to_string(),
        name,
        command,
        terminal,
        auto_start,
        group_name,
        steps,
        created_at: now.clone(),
        updated_at: now,
        note,
        last_executed_at: String::new(),
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
    steps: Vec<CommandStep>,
    note: String,
) -> Result<(), String> {
    let mut existing = storage.get_command(&id)?;
    existing.name = name;
    existing.command = command;
    existing.terminal = terminal;
    existing.auto_start = auto_start;
    existing.group_name = group_name;
    existing.steps = steps;
    existing.note = note;
    storage.update_command(&existing)
}

/// Delete a command by id.
#[tauri::command]
pub fn delete_command(storage: State<'_, Storage>, id: String) -> Result<(), String> {
    storage.delete_command(&id)
}

/// Mark a command as executed (updates last_executed_at timestamp).
#[tauri::command]
pub fn mark_command_executed(storage: State<'_, Storage>, id: String) -> Result<(), String> {
    storage.update_last_executed(&id)
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
    script::spawn_terminal(&command, &terminal)
        .map_err(|e| format!("Failed to execute '{}' in {} terminal: {}", name, terminal, e))
}

/// Execute a command by its ID, resolving multi-step chains.
#[tauri::command(rename_all = "camelCase")]
pub fn execute_command_by_id(
    storage: State<'_, Storage>,
    id: String,
    use_existing_window: Option<bool>,
    group_name: Option<String>,
) -> Result<(), String> {
    let cmd = storage.get_command(&id)?;
    let steps = script::steps_for_command(&cmd);
    let shell = script::shell_of(&cmd.terminal);
    let chained = script::build_chained_script(&steps, shell);
    if chained.is_empty() {
        return Err("No steps defined".into());
    }
    let use_existing = use_existing_window.unwrap_or(false);
    script::spawn_terminal_ex(&chained, &cmd.terminal, use_existing, group_name.as_deref())?;
    let _ = storage.update_last_executed(&id);
    Ok(())
}

/// Execute all commands in a group. If all are WT profiles → tabs in one window.
#[tauri::command]
pub fn execute_group(storage: State<'_, Storage>, group_name: String) -> Result<(), String> {
    let cmds = storage.get_group_commands(&group_name)?;
    if cmds.is_empty() {
        return Ok(());
    }
    // Collect IDs for execution tracking
    let cmd_ids: Vec<String> = cmds.iter().map(|c| c.id.clone()).collect();
    // Resolve multi-step commands into chained scripts
    let resolved: Vec<(String, String)> = cmds
        .iter()
        .map(|c| {
            let steps = script::steps_for_command(c);
            let shell = script::shell_of(&c.terminal);
            let chained = script::build_chained_script(&steps, shell);
            let effective_cmd = if chained.is_empty() { c.command.clone() } else { chained };
            (effective_cmd, c.terminal.clone())
        })
        .collect();
    // Check if all commands use Windows Terminal (terminal / terminal:*)
    let all_wt = cmds.iter().all(|c| c.terminal.starts_with("terminal"));
    if all_wt {
        script::spawn_terminal_batch(&resolved, &group_name)?;
        for id in &cmd_ids {
            let _ = storage.update_last_executed(id);
        }
        Ok(())
    } else {
        // Mixed terminals → spawn individually (logged by spawn_terminal_ex)
        logger::log_spawn("group", &format!("batch {}: {} commands", group_name, resolved.len()), true, None);
        for (i, (cmd, terminal)) in resolved.iter().enumerate() {
            if script::spawn_terminal(cmd, terminal).is_ok() {
                let _ = storage.update_last_executed(&cmd_ids[i]);
            }
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
    close_to_tray: bool,
    keep_terminal_open: bool,
) -> Result<(), String> {
    // Debug: log the received value
    logger::log_spawn("settings", &format!("keep_terminal_open={}", keep_terminal_open), true, None);

    let s = AppSettings {
        app_autostart,
        startup_delay_seconds,
        close_to_tray,
        keep_terminal_open,
    };
    settings::save_settings(&data_dir.0, &s)?;
    // Ensure runtime flag is synced (belt and suspenders)
    settings::set_keep_open(keep_terminal_open);

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
