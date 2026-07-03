use crate::storage::{Command, CommandStep, Storage};
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
    steps: Vec<CommandStep>,
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
) -> Result<(), String> {
    let mut existing = storage.get_command(&id)?;
    existing.name = name;
    existing.command = command;
    existing.terminal = terminal;
    existing.auto_start = auto_start;
    existing.group_name = group_name;
    existing.steps = steps;
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

/// Resolve the shell type string for a given terminal identifier.
pub fn shell_of(terminal: &str) -> &str {
    match terminal {
        "powershell" => "powershell",
        "cmd" => "cmd",
        "gitbash" => "gitbash",
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };
            if profile_name.is_empty() {
                "powershell"
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                profile
                    .as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell")
            }
        }
        _ => "powershell",
    }
}

/// Build a shell-chained script from command steps with delay separators.
/// The delay is attached to the step BEFORE it. The last step's delay is NOT used.
/// Uses `&&` for gitbash to avoid wt.exe parsing `;` as action separator.
pub fn build_chained_script(steps: &[CommandStep], shell: &str) -> String {
    if steps.is_empty() {
        return String::new();
    }

    let sep_fn = |delay: u64| -> String {
        match shell {
            // PowerShell: `& { ... }` wrapper keeps `;` safe from wt.exe even for WT profiles
            "powershell" => format!("; Start-Sleep -Seconds {}; ", delay),
            // CMD: `&` is NOT a wt.exe action separator, safe
            "cmd" => format!(" & timeout /t {} /nobreak >nul & ", delay),
            // Git Bash / unknown: use `&&` instead of `;` — avoids wt.exe splitting
            // `&&` in bash = run next only if previous succeeds (desired: stop chain on failure)
            _ => format!(" && sleep {} && ", delay),
        }
    };

    // Bare separator between commands when delay is 0 (still needed to avoid concatenation)
    let bare_sep: &str = match shell {
        "powershell" => "; ",
        "cmd" => " & ",
        _ => " && ",
    };

    let mut parts: Vec<String> = Vec::with_capacity(steps.len() * 2);
    for (i, step) in steps.iter().enumerate() {
        parts.push(step.cmd.clone());
        if i < steps.len() - 1 {
            if step.delay_sec > 0 {
                parts.push(sep_fn(step.delay_sec));
            } else {
                parts.push(bare_sep.to_string());
            }
        }
    }

    parts.concat()
}

/// Get the command steps for a command (backward compat: legacy commands use single step).
pub fn steps_for_command(cmd: &Command) -> Vec<CommandStep> {
    if cmd.steps.is_empty() {
        vec![CommandStep {
            cmd: cmd.command.clone(),
            delay_sec: 0,
        }]
    } else {
        cmd.steps.clone()
    }
}

/// Build (program, args) tuple for spawning a terminal command.
/// Kept behind spawn_terminal; extracted for reusability in multi-step execution.
pub fn build_spawn_args(command: &str, terminal: &str) -> Result<(String, Vec<String>), String> {
    match terminal {
        "powershell" => Ok((
            "powershell.exe".into(),
            vec!["-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
        )),
        "cmd" => Ok((
            "cmd.exe".into(),
            vec!["/k".into(), command.to_string()],
        )),
        "gitbash" => Ok((
            "bash.exe".into(),
            vec!["-i".into(), "-c".into(), command.to_string()],
        )),
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };

            if profile_name.is_empty() {
                Ok((
                    "wt.exe".into(),
                    vec!["powershell.exe".into(), "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
                ))
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                let shell = profile.as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell");

                match shell {
                    "gitbash" => {
                        let bash = crate::terminal::find_bash_path()
                            .unwrap_or_else(|| "bash".to_string());
                        Ok((
                            "wt.exe".into(),
                            vec![
                                "-p".into(), profile_name.to_string(), "--".into(),
                                bash, "-i".into(), "-c".into(), command.to_string(),
                            ],
                        ))
                    }
                    "cmd" => Ok((
                        "wt.exe".into(),
                        vec![
                            "-p".into(), profile_name.to_string(), "--".into(),
                            "cmd.exe".into(), "/k".into(), command.to_string(),
                        ],
                    )),
                    _ => Ok((
                        "wt.exe".into(),
                        vec![
                            "-p".into(), profile_name.to_string(), "--".into(),
                            "powershell.exe".into(), "-NoExit".into(), "-Command".into(),
                            format!("& {{{}}}", command),
                        ],
                    )),
                }
            }
        }
        _ => Err(format!("Unknown terminal type: {}", terminal)),
    }
}

/// Spawn a terminal process with the given command (no Tauri State needed).
pub fn spawn_terminal(command: &str, terminal: &str) -> Result<(), String> {
    let (program, args) = build_spawn_args(command, terminal)?;
    ProcessCommand::new(&program)
        .args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .spawn()
        .map_err(|e| format!("{}", e))?;
    Ok(())
}

/// Execute a command by its ID, resolving multi-step chains.
#[tauri::command]
pub fn execute_command_by_id(storage: State<'_, Storage>, id: String) -> Result<(), String> {
    let cmd = storage.get_command(&id)?;
    let steps = steps_for_command(&cmd);
    let shell = shell_of(&cmd.terminal);
    let script = build_chained_script(&steps, shell);
    if script.is_empty() {
        return Err("No steps defined".into());
    }
    spawn_terminal(&script, &cmd.terminal)
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
    // Resolve multi-step commands into chained scripts
    let resolved: Vec<(String, String)> = cmds
        .iter()
        .map(|c| {
            let steps = steps_for_command(c);
            let shell = shell_of(&c.terminal);
            let script = build_chained_script(&steps, shell);
            let effective_cmd = if script.is_empty() { c.command.clone() } else { script };
            (effective_cmd, c.terminal.clone())
        })
        .collect();
    // Check if all commands use Windows Terminal (terminal / terminal:*)
    let all_wt = cmds.iter().all(|c| c.terminal.starts_with("terminal"));
    if all_wt {
        spawn_terminal_batch(&resolved)
    } else {
        // Mixed terminals → spawn individually
        for (cmd, terminal) in &resolved {
            let _ = spawn_terminal(cmd, terminal);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_chained_script_powershell_with_delays() {
        let steps = vec![
            CommandStep { cmd: "echo a".into(), delay_sec: 2 },
            CommandStep { cmd: "echo b".into(), delay_sec: 3 },
            CommandStep { cmd: "echo c".into(), delay_sec: 0 },
        ];
        let script = build_chained_script(&steps, "powershell");
        assert!(script.contains("Start-Sleep -Seconds 2"), "powershell: should have delay after step 0, got: {script}");
        assert!(script.contains("Start-Sleep -Seconds 3"), "powershell: should have delay after step 1, got: {script}");
        assert!(!script.contains("Start-Sleep -Seconds 0"), "powershell: last step delay 0 should be absent, got: {script}");
        assert!(script.starts_with("echo a"), "powershell: should start with first step, got: {script}");
    }

    #[test]
    fn build_chained_script_cmd_with_delays() {
        let steps = vec![
            CommandStep { cmd: "dir".into(), delay_sec: 1 },
            CommandStep { cmd: "echo done".into(), delay_sec: 5 },
        ];
        let script = build_chained_script(&steps, "cmd");
        assert!(script.contains("timeout /t 1 /nobreak"), "cmd: should have delay after step 0, got: {script}");
        assert!(!script.contains("timeout /t 5"), "cmd: last step delay should be absent, got: {script}");
    }

    #[test]
    fn build_chained_script_gitbash_single_no_delay() {
        let steps = vec![
            CommandStep { cmd: "ls -la".into(), delay_sec: 0 },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert_eq!(script, "ls -la", "gitbash single step no delay should be just the command");
    }

    #[test]
    fn build_chained_script_gitbash_two_steps_no_delay() {
        let steps = vec![
            CommandStep { cmd: "echo a".into(), delay_sec: 0 },
            CommandStep { cmd: "echo b".into(), delay_sec: 0 },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert_eq!(script, "echo a && echo b", "gitbash two steps no delay should use && between");
    }

    #[test]
    fn build_chained_script_empty() {
        let steps: Vec<CommandStep> = vec![];
        let script = build_chained_script(&steps, "powershell");
        assert!(script.is_empty(), "empty steps should produce empty string");
    }

    #[test]
    fn build_chained_script_gitbash_with_delays() {
        let steps = vec![
            CommandStep { cmd: "echo x".into(), delay_sec: 1 },
            CommandStep { cmd: "echo y".into(), delay_sec: 2 },
            CommandStep { cmd: "echo z".into(), delay_sec: 0 },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert!(script.contains(" && sleep 1 && "), "gitbash: expected ' && sleep 1 && ', got: {script}");
        assert!(script.contains(" && sleep 2 && "), "gitbash: expected ' && sleep 2 && ', got: {script}");
        assert!(!script.contains("sleep 0"), "gitbash: last step delay 0 should not produce sleep");
    }

    #[test]
    fn shell_of_known_types() {
        assert_eq!(shell_of("powershell"), "powershell");
        assert_eq!(shell_of("cmd"), "cmd");
        assert_eq!(shell_of("gitbash"), "gitbash");
    }

    #[test]
    fn shell_of_terminal_defaults_to_powershell() {
        assert_eq!(shell_of("terminal"), "powershell");
    }

    #[test]
    fn shell_of_terminal_profile_unknown_defaults_to_powershell() {
        // Non-existent profile → fallback to powershell
        assert_eq!(shell_of("terminal:NonExistentProfile12345"), "powershell");
    }
}
