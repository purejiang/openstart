//! Terminal spawning, chain-script building, and argument construction.
//!
//! Extracted from `commands.rs` so the Tauri command layer stays thin.
//! All logic here is side-effecting only through `spawn_*` functions; the
//! `build_*` helpers are pure and unit-tested.

use crate::storage::{Command, CommandStep};
use crate::settings;
use crate::logger;
use std::process::Command as ProcessCommand;

/// Resolve the shell type string for a given terminal identifier.
pub fn shell_of(terminal: &str) -> &str {
    match terminal {
        "powershell" => "powershell",
        "cmd" => "cmd",
        "gitbash" => "gitbash",
        "wsl" => "wsl",
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
            note: String::new(),
        }]
    } else {
        cmd.steps.clone()
    }
}

// ── Argument building ─────────────────────────────────────────────────

/// Which flavor of arguments to produce for a resolved terminal command.
enum SpawnMode {
    /// Standalone process — returns `(program, args)` for `std::process::Command`.
    Standalone,
    /// A `wt.exe new-tab <shell> …` segment for batching into one WT window.
    WtTab,
}

/// Build the prefix args for opening a tab in an existing WT window.
/// If `group` is set, use `--window "OS:<name>"` to target the group's named window.
/// Otherwise, if `use_existing` is true, use `-w 0` (most recent window).
fn wt_tab_prefix(use_existing: bool, group: Option<&str>) -> Vec<String> {
    if let Some(name) = group {
        if !name.is_empty() {
            return vec!["--window".into(), format!("OS:{}", name), "new-tab".into()];
        }
    }
    if use_existing {
        vec!["-w".into(), "0".into(), "new-tab".into()]
    } else {
        vec![]
    }
}

/// Resolve the concrete shell program + args that actually run `command`.
///
/// `wt` = the command runs inside `wt.exe`, which changes gitbash `;` escaping
/// (needs `\;`) and the bash executable fallback (`bash` vs `bash.exe`).
fn shell_invocation(shell: &str, command: &str, keep: bool, wt: bool) -> (String, Vec<String>) {
    match shell {
        "cmd" => (
            "cmd.exe".into(),
            vec![
                if keep { "/k".into() } else { "/c".into() },
                command.to_string(),
            ],
        ),
        "gitbash" => {
            let fallback = if wt { "bash" } else { "bash.exe" };
            let bash = crate::terminal::find_bash_path()
                .unwrap_or_else(|| fallback.to_string());
            let sep = if wt { "\\;" } else { ";" };
            let cmd = if keep {
                format!("{}{} exec bash", command, sep)
            } else {
                command.to_string()
            };
            (bash, vec!["-c".into(), cmd])
        }
        "wsl" => {
            // Run through wsl.exe so Windows CWD is mapped to /mnt/... in WSL.
            // bash inside WSL then executes the chained command.
            let cmd = if keep {
                format!("{}; exec bash", command)
            } else {
                command.to_string()
            };
            ("wsl.exe".into(), vec!["--".into(), "bash".into(), "-c".into(), cmd])
        }
        // powershell (default for unknown profiles)
        _ => {
            let mut args: Vec<String> = Vec::new();
            if keep {
                args.push("-NoExit".into());
            }
            args.push("-Command".into());
            args.push(format!("& {{{}}}", command));
            ("powershell.exe".into(), args)
        }
    }
}

/// Unified terminal-argument builder. Both `build_spawn_args` and
/// `build_wt_tab_args` are thin wrappers over this so the per-terminal match
/// lives in exactly one place.
///
/// For `SpawnMode::Standalone` the returned program is the real executable.
/// For `SpawnMode::WtTab` the program is empty and the args are a `new-tab` segment.
fn build_terminal_args(
    command: &str,
    terminal: &str,
    use_existing_window: bool,
    group_name: Option<&str>,
    mode: SpawnMode,
) -> Result<(String, Vec<String>), String> {
    let keep = settings::is_keep_open();

    match terminal {
        "powershell" | "cmd" | "gitbash" | "wsl" => {
            let wt = matches!(mode, SpawnMode::WtTab);
            let (program, inner) = shell_invocation(terminal, command, keep, wt);
            match mode {
                SpawnMode::Standalone => Ok((program, inner)),
                SpawnMode::WtTab => {
                    let mut args = vec!["new-tab".to_string(), program];
                    args.extend(inner);
                    Ok((String::new(), args))
                }
            }
        }
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };
            let shell = if profile_name.is_empty() {
                "powershell"
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                profile
                    .as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell")
            };
            // Everything under `terminal*` runs inside wt.exe.
            let (shell_program, shell_args) = shell_invocation(shell, command, keep, true);

            match mode {
                SpawnMode::Standalone => {
                    let mut args = wt_tab_prefix(use_existing_window, group_name);
                    if !profile_name.is_empty() {
                        args.push("-p".into());
                        args.push(profile_name.to_string());
                        args.push("--".into());
                    }
                    args.push(shell_program);
                    args.extend(shell_args);
                    Ok(("wt.exe".into(), args))
                }
                SpawnMode::WtTab => {
                    let mut args = vec!["new-tab".to_string(), shell_program];
                    args.extend(shell_args);
                    Ok((String::new(), args))
                }
            }
        }
        _ => Err(format!("Unknown terminal type: {}", terminal)),
    }
}

/// Build (program, args) tuple for spawning a terminal command.
/// Kept behind spawn_terminal; extracted for reusability in multi-step execution.
pub fn build_spawn_args(
    command: &str,
    terminal: &str,
    use_existing_window: bool,
    group_name: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    build_terminal_args(command, terminal, use_existing_window, group_name, SpawnMode::Standalone)
}

/// Build args for a single wt.exe `new-tab` segment (no program prefix).
pub fn build_wt_tab_args(command: &str, terminal: &str) -> Result<Vec<String>, String> {
    let (_program, args) =
        build_terminal_args(command, terminal, false, None, SpawnMode::WtTab)?;
    Ok(args)
}

// ── Spawning ──────────────────────────────────────────────────────────

/// Spawn a terminal process with the given command (no Tauri State needed).
pub fn spawn_terminal(command: &str, terminal: &str) -> Result<(), String> {
    spawn_terminal_ex(command, terminal, false, None)
}

/// Spawn with optional tab-in-existing-window for WT terminals.
pub fn spawn_terminal_ex(command: &str, terminal: &str, use_existing_window: bool, group_name: Option<&str>) -> Result<(), String> {
    let (program, args) = build_spawn_args(command, terminal, use_existing_window, group_name)?;
    let cmd_preview = if command.len() > 80 { &command[..80] } else { command };

    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;

    let mut proc = ProcessCommand::new(&program);
    proc.args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    // GUI apps (wt.exe) don't need a console. Console apps (powershell/cmd/bash) do,
    // since the parent OpenStart process is a GUI app with no console of its own.
    if program != "wt.exe" {
        proc.creation_flags(CREATE_NEW_CONSOLE);
    }

    let result = proc.spawn()
        .map(|_| ())
        .map_err(|e| format!("{}", e));
    let keep = settings::is_keep_open();
    match &result {
        Ok(()) => logger::log_spawn(terminal, &format!("keep={} | {}", keep, cmd_preview), true, None),
        Err(e) => logger::log_spawn(terminal, &format!("keep={} | {}", keep, cmd_preview), false, Some(e)),
    }
    result
}

/// Spawn multiple commands as tabs in a single Windows Terminal window.
/// If `group_name` is provided, the WT window is named `OS:<group_name>` for later targeting.
pub fn spawn_terminal_batch(commands: &[(String, String)], group_name: &str) -> Result<(), String> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut all_args: Vec<String> = vec![];
    if !group_name.is_empty() {
        all_args.push("--window".into());
        all_args.push(format!("OS:{}", group_name));
    }
    for (i, (cmd, term)) in commands.iter().enumerate() {
        if i > 0 {
            all_args.push(";".into());
        }
        all_args.append(&mut build_wt_tab_args(cmd, term)?);
    }
    // Log the batch — use first command as preview
    let preview = if let Some((cmd, term)) = commands.first() {
        let p = if cmd.len() > 80 { &cmd[..80] } else { cmd.as_str() };
        (term.clone(), p.to_string())
    } else {
        return Ok(());
    };
    let result = ProcessCommand::new("wt.exe").args(&all_args).spawn()
        .map(|_| ())
        .map_err(|e| format!("{}", e));
    match &result {
        Ok(()) => logger::log_spawn(&preview.0, &preview.1, true, None),
        Err(ref e) => logger::log_spawn(&preview.0, &preview.1, false, Some(e)),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CommandStep;

    #[test]
    fn build_chained_script_powershell_with_delays() {
        let steps = vec![
            CommandStep { cmd: "echo a".into(), delay_sec: 2, note: String::new() },
            CommandStep { cmd: "echo b".into(), delay_sec: 3, note: String::new() },
            CommandStep { cmd: "echo c".into(), delay_sec: 0, note: String::new() },
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
            CommandStep { cmd: "dir".into(), delay_sec: 1, note: String::new() },
            CommandStep { cmd: "echo done".into(), delay_sec: 5, note: String::new() },
        ];
        let script = build_chained_script(&steps, "cmd");
        assert!(script.contains("timeout /t 1 /nobreak"), "cmd: should have delay after step 0, got: {script}");
        assert!(!script.contains("timeout /t 5"), "cmd: last step delay should be absent, got: {script}");
    }

    #[test]
    fn build_chained_script_gitbash_single_no_delay() {
        let steps = vec![
            CommandStep { cmd: "ls -la".into(), delay_sec: 0, note: String::new() },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert_eq!(script, "ls -la", "gitbash single step no delay should be just the command");
    }

    #[test]
    fn build_chained_script_gitbash_two_steps_no_delay() {
        let steps = vec![
            CommandStep { cmd: "echo a".into(), delay_sec: 0, note: String::new() },
            CommandStep { cmd: "echo b".into(), delay_sec: 0, note: String::new() },
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
            CommandStep { cmd: "echo x".into(), delay_sec: 1, note: String::new() },
            CommandStep { cmd: "echo y".into(), delay_sec: 2, note: String::new() },
            CommandStep { cmd: "echo z".into(), delay_sec: 0, note: String::new() },
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
    fn shell_of_wsl() {
        assert_eq!(shell_of("wsl"), "wsl");
    }

    #[test]
    fn build_chained_script_wsl_with_delays() {
        let steps = vec![
            CommandStep { cmd: "echo x".into(), delay_sec: 1, note: String::new() },
            CommandStep { cmd: "echo y".into(), delay_sec: 2, note: String::new() },
        ];
        let script = build_chained_script(&steps, "wsl");
        assert!(script.contains(" && sleep 1 && "), "wsl: expected ' && sleep 1 && ', got: {script}");
        assert!(!script.contains("sleep 2"), "wsl: last step delay should be absent, got: {script}");
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
