//! Terminal spawning, chain-script building, and argument construction.
//!
//! Extracted from `commands.rs` so the Tauri command layer stays thin.
//! All logic here is side-effecting only through `spawn_*` functions; the
//! `build_*` helpers are pure and unit-tested.

use crate::logger;
use crate::process;
use crate::settings;
use crate::storage::{Command, CommandStep};
use std::process::Command as ProcessCommand;

/// Resolve the shell type string for a given terminal identifier.
pub fn shell_of(terminal: &str) -> &str {
    match terminal {
        "powershell" => "powershell",
        "cmd" => "cmd",
        "gitbash" => "gitbash",
        "wsl" => "wsl",
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') {
                p
            } else {
                ""
            };
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

/// A spawned terminal process. `pid` is the direct child PID (standalone
/// shells) or the short-lived `wt.exe` launcher PID (Windows Terminal — the
/// real tab shell PID is resolved separately via marker scanning).
#[derive(Debug, Clone)]
pub struct Spawned {
    pub pid: Option<u32>,
    pub program: String,
}

/// Inject a unique marker into a command so the real shell process can later be
/// found by scanning process command lines.
///
/// The marker only contains `[A-Za-z0-9-]`, so it is safe inside single quotes,
/// double quotes and `set "..."` assignments. Uses `&&` for POSIX shells so it
/// never introduces a bare `;` that Windows Terminal could treat as an action
/// separator; PowerShell launches through WT are base64-encoded instead (see
/// [`wt_powershell_command`]) so their `;` separator never reaches `wt.exe`.
pub fn inject_marker(command: &str, shell: &str, marker: &str) -> String {
    match shell {
        "cmd" => format!("set \"OSRUN={}\" & {}", marker, command),
        "gitbash" | "wsl" => format!("OSRUN='{}' && {}", marker, command),
        // powershell / unknown profiles
        _ => format!("$null = '{}'; {}", marker, command),
    }
}

// ── Windows Terminal safe PowerShell payload ──────────────────────────

const BASE64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_bytes(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_TABLE[((n >> 18) & 63) as usize] as char);
        out.push(BASE64_TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Build the `-Command` argument for a PowerShell invocation that travels
/// through Windows Terminal.
///
/// `wt.exe` splits its command line on `;` before the shell ever sees it — even
/// inside quotes. Any `;` in the payload (marker injection, multi-step chains or
/// a user's own PowerShell command) therefore made WT open an extra tab trying
/// to launch the truncated fragment. The script is base64-encoded (UTF-8) and
/// decoded via `Invoke-Expression`, so the command line only contains characters
/// WT passes through untouched. The PID marker rides along as a trailing comment,
/// keeping it visible to process-command-line scanning without a `;`.
fn wt_powershell_command(script: &str, marker: Option<&str>) -> String {
    let encoded = base64_bytes(script.as_bytes());
    let mut argument = format!(
        "Invoke-Expression ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}')))",
        encoded
    );
    if let Some(marker) = marker.filter(|value| !value.is_empty()) {
        argument.push_str(" # ");
        argument.push_str(marker);
    }
    argument
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
fn shell_invocation(
    shell: &str,
    command: &str,
    keep: bool,
    wt: bool,
    marker: Option<&str>,
) -> (String, Vec<String>) {
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
            let bash = crate::terminal::find_bash_path().unwrap_or_else(|| fallback.to_string());
            let sep = if wt { "\\;" } else { ";" };
            let cmd = if keep {
                // For WT launches use a child `bash` instead of `exec bash`:
                // the outer shell keeps the marker in its command line, which
                // lets us resolve/track its PID. The PID is unchanged by exec,
                // but the marker would disappear from the command line.
                if wt {
                    format!("{}{} bash", command, sep)
                } else {
                    format!("{}{} exec bash", command, sep)
                }
            } else {
                command.to_string()
            };
            (bash, vec!["-c".into(), cmd])
        }
        "wsl" => {
            // Run through wsl.exe so Windows CWD is mapped to /mnt/... in WSL.
            // The WSL init (/bin/sh) parses the command before bash, so we escape
            // the `;` as `\;` to prevent sh from splitting `command; exec bash`
            // into two separate commands (the second being the builtin `exec`).
            let cmd = if keep {
                format!("{}\\; exec bash", command)
            } else {
                command.to_string()
            };
            (
                "wsl.exe".into(),
                vec!["--".into(), "bash".into(), "-lic".into(), cmd],
            )
        }
        // powershell (default for unknown profiles)
        _ => {
            let mut args: Vec<String> = Vec::new();
            if keep {
                args.push("-NoExit".into());
            }
            args.push("-Command".into());
            if wt {
                // Never let a `;` reach wt.exe — encode the whole script instead.
                args.push(wt_powershell_command(&format!("& {{{}}}", command), marker));
            } else {
                args.push(format!("& {{{}}}", command));
            }
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
    marker: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let keep = settings::is_keep_open();

    match terminal {
        "powershell" | "cmd" | "gitbash" | "wsl" => {
            let wt = matches!(mode, SpawnMode::WtTab);
            let effective = match marker {
                Some(marker) if !marker.is_empty() => inject_marker(command, terminal, marker),
                _ => command.to_string(),
            };
            let (program, inner) = shell_invocation(terminal, &effective, keep, wt, marker);
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
            let profile_name = if let Some((_, p)) = t.split_once(':') {
                p
            } else {
                ""
            };
            let shell = if profile_name.is_empty() {
                "powershell"
            } else {
                let profile = crate::terminal::find_wt_profile(profile_name);
                profile
                    .as_ref()
                    .map(|p| crate::terminal::profile_shell_type(p))
                    .unwrap_or("powershell")
            };
            let effective = match marker {
                Some(marker) if !marker.is_empty() => inject_marker(command, shell, marker),
                _ => command.to_string(),
            };
            // Everything under `terminal*` runs inside wt.exe.
            let (shell_program, shell_args) = shell_invocation(shell, &effective, keep, true, marker);

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
    build_spawn_args_tracked(command, terminal, use_existing_window, group_name, None)
}

/// Same as [`build_spawn_args`] but optionally injects a PID-discovery marker
/// (used for Windows Terminal launches).
pub fn build_spawn_args_tracked(
    command: &str,
    terminal: &str,
    use_existing_window: bool,
    group_name: Option<&str>,
    marker: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    build_terminal_args(
        command,
        terminal,
        use_existing_window,
        group_name,
        SpawnMode::Standalone,
        marker,
    )
}

/// Build args for a single wt.exe `new-tab` segment (no program prefix).
pub fn build_wt_tab_args(command: &str, terminal: &str) -> Result<Vec<String>, String> {
    build_wt_tab_args_tracked(command, terminal, None)
}

/// Same as [`build_wt_tab_args`] but optionally injects a PID-discovery marker.
pub fn build_wt_tab_args_tracked(
    command: &str,
    terminal: &str,
    marker: Option<&str>,
) -> Result<Vec<String>, String> {
    let (_program, args) =
        build_terminal_args(command, terminal, false, None, SpawnMode::WtTab, marker)?;
    Ok(args)
}

// ── Spawning ──────────────────────────────────────────────────────────

/// Spawn a terminal process with the given command (no Tauri State needed).
pub fn spawn_terminal(command: &str, terminal: &str) -> Result<(), String> {
    spawn_terminal_ex(command, terminal, false, None)
}

/// Spawn with optional tab-in-existing-window for WT terminals.
pub fn spawn_terminal_ex(
    command: &str,
    terminal: &str,
    use_existing_window: bool,
    group_name: Option<&str>,
) -> Result<(), String> {
    spawn_terminal_ex_tracked(command, terminal, use_existing_window, group_name, None).map(|_| ())
}

/// Spawn a terminal process and return its PID.
///
/// `marker` is only used for Windows Terminal launches; the direct child PID of
/// `wt.exe` is a short-lived launcher, so the real shell PID is resolved later
/// by scanning for the marker.
pub fn spawn_terminal_ex_tracked(
    command: &str,
    terminal: &str,
    use_existing_window: bool,
    group_name: Option<&str>,
    marker: Option<&str>,
) -> Result<Spawned, String> {
    let (program, args) =
        build_spawn_args_tracked(command, terminal, use_existing_window, group_name, marker)?;
    let cmd_preview = if command.len() > 80 {
        &command[..80]
    } else {
        command
    };
    let keep = settings::is_keep_open();

    let spawned = if program == "wt.exe" {
        // GUI app: no console. The launcher exits immediately anyway; its PID is
        // only informational (WT runs are tracked through their marker).
        ProcessCommand::new(&program)
            .args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .spawn()
            .map(|child| Some(child.id()))
            .map_err(|e| format!("{}", e))
    } else {
        // Console apps must own a fresh console, otherwise inherited stdout
        // handles leak the shell output into the invoking CLI console (and can
        // keep a piped caller blocked until the terminal closes).
        process::spawn_new_console(&program, &args).map(Some)
    };

    match spawned {
        Ok(pid) => {
            logger::log_spawn(
                terminal,
                &format!("keep={} | {}", keep, cmd_preview),
                true,
                None,
            );
            Ok(Spawned { pid, program })
        }
        Err(error) => {
            logger::log_spawn(
                terminal,
                &format!("keep={} | {}", keep, cmd_preview),
                false,
                Some(&error),
            );
            Err(error)
        }
    }
}

/// Spawn multiple commands as tabs in a single Windows Terminal window.
/// If `group_name` is provided, the WT window is named `OS:<group_name>` for later targeting.
pub fn spawn_terminal_batch(commands: &[(String, String)], group_name: &str) -> Result<(), String> {
    let tracked: Vec<(String, String, String)> = commands
        .iter()
        .map(|(command, terminal)| (command.clone(), terminal.clone(), String::new()))
        .collect();
    spawn_terminal_batch_tracked(&tracked, group_name).map(|_| ())
}

/// Spawn multiple commands as WT tabs, injecting a marker per tab and returning
/// the `wt.exe` launcher PID.
pub fn spawn_terminal_batch_tracked(
    commands: &[(String, String, String)],
    group_name: &str,
) -> Result<Spawned, String> {
    if commands.is_empty() {
        return Err("No commands to launch".to_string());
    }
    let mut all_args: Vec<String> = vec![];
    if !group_name.is_empty() {
        all_args.push("--window".into());
        all_args.push(format!("OS:{}", group_name));
    }
    for (i, (cmd, term, marker)) in commands.iter().enumerate() {
        if i > 0 {
            all_args.push(";".into());
        }
        let marker = if marker.is_empty() {
            None
        } else {
            Some(marker.as_str())
        };
        all_args.append(&mut build_wt_tab_args_tracked(cmd, term, marker)?);
    }
    // Log the batch — use first command as preview
    let preview = {
        let (cmd, term, _) = &commands[0];
        let p = if cmd.len() > 80 {
            &cmd[..80]
        } else {
            cmd.as_str()
        };
        (term.clone(), p.to_string())
    };
    let result = ProcessCommand::new("wt.exe").args(&all_args).spawn();
    match result {
        Ok(child) => {
            logger::log_spawn(&preview.0, &preview.1, true, None);
            Ok(Spawned {
                pid: Some(child.id()),
                program: "wt.exe".to_string(),
            })
        }
        Err(e) => {
            logger::log_spawn(&preview.0, &preview.1, false, Some(&e.to_string()));
            Err(format!("{}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CommandStep;

    #[test]
    fn build_chained_script_powershell_with_delays() {
        let steps = vec![
            CommandStep {
                cmd: "echo a".into(),
                delay_sec: 2,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo b".into(),
                delay_sec: 3,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo c".into(),
                delay_sec: 0,
                note: String::new(),
            },
        ];
        let script = build_chained_script(&steps, "powershell");
        assert!(
            script.contains("Start-Sleep -Seconds 2"),
            "powershell: should have delay after step 0, got: {script}"
        );
        assert!(
            script.contains("Start-Sleep -Seconds 3"),
            "powershell: should have delay after step 1, got: {script}"
        );
        assert!(
            !script.contains("Start-Sleep -Seconds 0"),
            "powershell: last step delay 0 should be absent, got: {script}"
        );
        assert!(
            script.starts_with("echo a"),
            "powershell: should start with first step, got: {script}"
        );
    }

    #[test]
    fn build_chained_script_cmd_with_delays() {
        let steps = vec![
            CommandStep {
                cmd: "dir".into(),
                delay_sec: 1,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo done".into(),
                delay_sec: 5,
                note: String::new(),
            },
        ];
        let script = build_chained_script(&steps, "cmd");
        assert!(
            script.contains("timeout /t 1 /nobreak"),
            "cmd: should have delay after step 0, got: {script}"
        );
        assert!(
            !script.contains("timeout /t 5"),
            "cmd: last step delay should be absent, got: {script}"
        );
    }

    #[test]
    fn build_chained_script_gitbash_single_no_delay() {
        let steps = vec![CommandStep {
            cmd: "ls -la".into(),
            delay_sec: 0,
            note: String::new(),
        }];
        let script = build_chained_script(&steps, "gitbash");
        assert_eq!(
            script, "ls -la",
            "gitbash single step no delay should be just the command"
        );
    }

    #[test]
    fn build_chained_script_gitbash_two_steps_no_delay() {
        let steps = vec![
            CommandStep {
                cmd: "echo a".into(),
                delay_sec: 0,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo b".into(),
                delay_sec: 0,
                note: String::new(),
            },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert_eq!(
            script, "echo a && echo b",
            "gitbash two steps no delay should use && between"
        );
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
            CommandStep {
                cmd: "echo x".into(),
                delay_sec: 1,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo y".into(),
                delay_sec: 2,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo z".into(),
                delay_sec: 0,
                note: String::new(),
            },
        ];
        let script = build_chained_script(&steps, "gitbash");
        assert!(
            script.contains(" && sleep 1 && "),
            "gitbash: expected ' && sleep 1 && ', got: {script}"
        );
        assert!(
            script.contains(" && sleep 2 && "),
            "gitbash: expected ' && sleep 2 && ', got: {script}"
        );
        assert!(
            !script.contains("sleep 0"),
            "gitbash: last step delay 0 should not produce sleep"
        );
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
            CommandStep {
                cmd: "echo x".into(),
                delay_sec: 1,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo y".into(),
                delay_sec: 2,
                note: String::new(),
            },
        ];
        let script = build_chained_script(&steps, "wsl");
        assert!(
            script.contains(" && sleep 1 && "),
            "wsl: expected ' && sleep 1 && ', got: {script}"
        );
        assert!(
            !script.contains("sleep 2"),
            "wsl: last step delay should be absent, got: {script}"
        );
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

    #[test]
    fn shell_invocation_wsl_uses_login_interactive_shell() {
        // keep_open=true: command gets the "; exec bash" suffix (escaped as \; for sh)
        let (prog, args) = shell_invocation("wsl", "opencode", true, false, None);
        assert_eq!(prog, "wsl.exe", "wsl program should be wsl.exe");
        assert_eq!(args[0], "--", "first arg should be --");
        assert_eq!(args[1], "bash", "second arg should be bash");
        assert_eq!(args[2], "-lic",
            "wsl should use login+interactive shell (-lic) so ~/.profile AND ~/.bashrc both load; got args: {args:?}");
        let cmd = &args[3];
        assert!(
            cmd.contains("\\; exec bash"),
            "keep_open=true should append escaped '; exec bash'; got cmd: {cmd}"
        );
        assert!(
            cmd.starts_with("opencode"),
            "cmd should start with the user command; got: {cmd}"
        );

        // keep_open=false: command passed through unchanged
        let (prog2, args2) = shell_invocation("wsl", "echo hi", false, false, None);
        assert_eq!(prog2, "wsl.exe");
        assert_eq!(
            args2[2], "-lic",
            "login+interactive flag must be present even when keep_open=false; got args: {args2:?}"
        );
        assert_eq!(
            args2[3], "echo hi",
            "keep_open=false should pass command unchanged; got: {}",
            args2[4]
        );
    }

    #[test]
    fn inject_marker_per_shell() {
        let marker = "OSRUN-abc123def456";
        assert_eq!(
            inject_marker("echo hi", "powershell", marker),
            format!("$null = '{}'; echo hi", marker)
        );
        assert_eq!(
            inject_marker("echo hi", "cmd", marker),
            format!("set \"OSRUN={}\" & echo hi", marker)
        );
        assert_eq!(
            inject_marker("echo hi", "gitbash", marker),
            format!("OSRUN='{}' && echo hi", marker)
        );
        assert_eq!(
            inject_marker("echo hi", "wsl", marker),
            format!("OSRUN='{}' && echo hi", marker)
        );
    }

    #[test]
    fn wt_tab_args_include_marker() {
        let marker = "OSRUN-abc123def456";
        let args = build_wt_tab_args_tracked("echo hi", "terminal", Some(marker)).unwrap();
        let joined = args.join(" ");
        assert!(
            joined.contains(marker),
            "WT tab args should keep the marker visible for PID discovery, got: {joined}"
        );
        assert!(
            joined.contains("FromBase64String"),
            "WT PowerShell payload must be base64-encoded, got: {joined}"
        );
        assert!(
            !joined.contains("echo hi"),
            "the raw command must not leak into the WT command line, got: {joined}"
        );
    }

    #[test]
    fn base64_encoder_matches_reference_values() {
        assert_eq!(base64_bytes(b""), "");
        assert_eq!(base64_bytes(b"f"), "Zg==");
        assert_eq!(base64_bytes(b"fo"), "Zm8=");
        assert_eq!(base64_bytes(b"foo"), "Zm9v");
        assert_eq!(base64_bytes(b"foob"), "Zm9vYg==");
        assert_eq!(base64_bytes("& {echo hi}".as_bytes()), "JiB7ZWNobyBoaX0=");
        assert_eq!(base64_bytes("中文".as_bytes()), "5Lit5paH");
    }

    #[test]
    fn wt_powershell_payload_has_no_wt_separator() {
        let marker = "OSRUN-abc123def456";
        let argument = wt_powershell_command("& {echo hi}", Some(marker));
        assert!(
            !argument.contains(';'),
            "payload must not contain ';' because wt.exe would split it: {argument}"
        );
        assert!(
            argument.contains("JiB7ZWNobyBoaX0="),
            "payload should be the base64 of '& {{echo hi}}', got: {argument}"
        );
        assert!(
            argument.ends_with(&format!("# {marker}")),
            "marker should ride along as a trailing comment, got: {argument}"
        );
    }

    #[test]
    fn wt_powershell_args_never_expose_semicolons() {
        let marker = "OSRUN-feedface0001";
        let steps = vec![
            CommandStep {
                cmd: "echo a".into(),
                delay_sec: 2,
                note: String::new(),
            },
            CommandStep {
                cmd: "echo b".into(),
                delay_sec: 0,
                note: String::new(),
            },
        ];
        let chained = build_chained_script(&steps, "powershell");
        assert!(
            chained.contains(';'),
            "powershell chains legitimately use ';' before encoding: {chained}"
        );

        let args = build_wt_tab_args_tracked(&chained, "terminal:PowerShell", Some(marker)).unwrap();
        assert!(
            args.iter().all(|arg| !arg.contains(';')),
            "no argument passed to wt.exe may contain ';': {args:?}"
        );
        assert!(
            args.iter().any(|arg| arg.contains(marker)),
            "WT args should keep the marker for PID discovery: {args:?}"
        );

        // Same for a standalone launch into an existing WT window (`-w 0`).
        let (program, standalone) =
            build_spawn_args_tracked("echo hi; echo bye", "terminal:PowerShell", true, None, Some(marker))
                .unwrap();
        assert_eq!(program, "wt.exe");
        assert!(
            standalone.iter().all(|arg| !arg.contains(';')),
            "standalone WT launch args may not contain ';': {standalone:?}"
        );
    }

    #[test]
    fn standalone_powershell_keeps_plain_command_wrapper() {
        let (_program, args) = shell_invocation("powershell", "echo hi", true, false, None);
        assert_eq!(args, vec!["-NoExit", "-Command", "& {echo hi}"]);
    }

    #[test]
    fn shell_invocation_gitbash_wt_keep_uses_child_bash() {
        // WT keep-open must keep the outer bash alive (it carries the marker),
        // so use a child `bash` rather than `exec bash`.
        let (prog, args) = shell_invocation("gitbash", "echo hi", true, true, None);
        assert!(
            prog.to_lowercase().contains("bash"),
            "expected a bash program, got {prog}"
        );
        let cmd = &args[1];
        assert!(
            cmd.contains("\\; bash"),
            "WT keep-open should use a child bash, got: {cmd}"
        );
        assert!(
            !cmd.contains("exec bash"),
            "WT keep-open must not exec over the marker-carrying shell, got: {cmd}"
        );

        // Standalone gitbash launches keep the existing `exec bash` behavior.
        let (_prog, direct) = shell_invocation("gitbash", "echo hi", true, false, None);
        assert!(
            direct[1].contains("; exec bash"),
            "standalone gitbash should keep 'exec bash', got: {}",
            direct[1]
        );
    }
}
