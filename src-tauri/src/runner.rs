//! Unified launch / duplicate-guard / status / stop orchestration.
//!
//! Every execution path (GUI single command, GUI group, CLI, app auto-start)
//! goes through this module so the `runs` registry is always up to date.

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::process;
use crate::script;
use crate::storage::{self, BeginRunError, Command, NewRun, Run, Storage, RUN_FAILED, RUN_STOPPED};

// ── Errors / results ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RunError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl RunError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            run_id: None,
            pid: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_target", message)
    }

    pub fn disabled(name: &str) -> Self {
        Self::new(
            "disabled",
            format!("\"{}\" is disabled. Enable it before launching.", name),
        )
    }

    pub fn no_enabled_commands(group_name: &str) -> Self {
        Self::new(
            "no_enabled_commands",
            format!(
                "Group \"{}\" has no enabled commands. Enable at least one before running the group.",
                group_name
            ),
        )
    }

    pub fn not_found(run_id: &str) -> Self {
        let mut error = Self::new("not_found", format!("Run '{}' was not found", run_id));
        error.run_id = Some(run_id.to_string());
        error
    }

    pub fn already_running(active: &[Run]) -> Self {
        let Some(first) = active.first() else {
            return Self::new("already_running", "Target is already running");
        };

        let mut message = format!(
            "\"{}\" is already running (run {}, pid {}, {})",
            first.target_name,
            first.id,
            first
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "pending".to_string()),
            elapsed_label(&first.started_at),
        );
        if active.len() > 1 {
            let names: Vec<&str> = active
                .iter()
                .skip(1)
                .take(2)
                .map(|run| run.target_name.as_str())
                .collect();
            message.push_str(&format!("; also running: {}", names.join(", ")));
        }
        message.push_str(&format!(
            ". Stop it first: openstart --cli stop {}",
            first.id
        ));

        Self {
            code: "already_running".to_string(),
            message,
            run_id: Some(first.id.clone()),
            pid: first.pid,
        }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RunError {}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StopSummary {
    pub stopped: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BatchOutcome {
    pub runs: Vec<Run>,
    /// Targets skipped because they were already running (auto-start only).
    pub skipped: Vec<String>,
}

/// Options controlling one launch.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub origin: String,
    pub use_existing_window: bool,
    pub group_name_for_window: Option<String>,
    pub force: bool,
    pub resolve_timeout: Duration,
}

impl LaunchOptions {
    pub fn gui(use_existing_window: bool, group_name_for_window: Option<String>) -> Self {
        Self {
            origin: "gui".to_string(),
            use_existing_window,
            group_name_for_window,
            force: false,
            resolve_timeout: Duration::ZERO,
        }
    }

    pub fn cli(force: bool) -> Self {
        Self {
            origin: "cli".to_string(),
            use_existing_window: false,
            group_name_for_window: None,
            force,
            resolve_timeout: Duration::from_millis(2500),
        }
    }

    pub fn auto_start() -> Self {
        Self {
            origin: "autostart".to_string(),
            use_existing_window: false,
            group_name_for_window: None,
            force: false,
            resolve_timeout: Duration::ZERO,
        }
    }
}

// ── IDs / markers / formatting ────────────────────────────────────────

/// Short run id: `r-` + 12 hex characters.
pub fn new_run_id() -> String {
    format!("r-{}", &uuid::Uuid::new_v4().simple().to_string()[..12])
}

/// Marker embedded in a Windows Terminal shell command line.
fn marker_for(run_id: &str) -> String {
    format!("OSRUN-{}", run_id.trim_start_matches("r-"))
}

/// Is this a Windows Terminal terminal id (`terminal` / `terminal:<profile>`)?
fn is_wt_terminal(terminal: &str) -> bool {
    terminal.starts_with("terminal")
}

pub fn format_duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

fn elapsed_label(started_at: &str) -> String {
    let seconds = chrono::DateTime::parse_from_rfc3339(started_at)
        .ok()
        .map(|dt| {
            chrono::Utc::now()
                .signed_duration_since(dt.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0) as u64
        })
        .unwrap_or(0);
    format_duration(seconds)
}

// ── Launch ────────────────────────────────────────────────────────────

fn build_chained(command: &Command) -> Result<String, RunError> {
    let steps = script::steps_for_command(command);
    let shell = script::shell_of(&command.terminal);
    let chained = script::build_chained_script(&steps, shell);
    if chained.trim().is_empty() {
        return Err(RunError::invalid(format!(
            "Command \"{}\" has no steps to execute",
            command.name
        )));
    }
    Ok(chained)
}

fn new_run(command: &Command, origin: &str, id: String) -> NewRun {
    let marker = if is_wt_terminal(&command.terminal) {
        marker_for(&id)
    } else {
        String::new()
    };
    NewRun {
        id,
        target_kind: "command".to_string(),
        target_id: command.id.clone(),
        target_name: command.name.clone(),
        group_name: command.group_name.clone(),
        terminal: command.terminal.clone(),
        origin: origin.to_string(),
        marker,
    }
}

fn new_adhoc_run(name: &str, command: &str, terminal: &str, origin: &str, id: String) -> NewRun {
    let marker = if is_wt_terminal(terminal) {
        marker_for(&id)
    } else {
        String::new()
    };
    NewRun {
        id,
        target_kind: "adhoc".to_string(),
        target_id: format!("{}\u{1f}{}", terminal, command),
        target_name: name.to_string(),
        group_name: String::new(),
        terminal: terminal.to_string(),
        origin: origin.to_string(),
        marker,
    }
}

fn map_begin_error(error: BeginRunError) -> RunError {
    match error {
        BeginRunError::Conflict(active) => RunError::already_running(&active),
        BeginRunError::Internal(message) => RunError::internal(message),
    }
}

/// Spawn one created run row and record its PID when it is a direct shell.
fn spawn_created_run(
    storage: &Storage,
    run: &Run,
    chained: &str,
    use_existing_window: bool,
    window_group: Option<&str>,
) -> Result<(), String> {
    let marker = if run.marker.is_empty() {
        None
    } else {
        Some(run.marker.as_str())
    };
    let spawned = script::spawn_terminal_ex_tracked(
        chained,
        &run.terminal,
        use_existing_window,
        window_group,
        marker,
    )?;

    // WT shells are spawned by WindowsTerminal; the wt.exe PID is a launcher.
    // Their real PID is resolved from the marker by `refresh_runs`.
    if run.marker.is_empty() {
        if let Some(pid) = spawned.pid {
            let started_at = process::start_time_of(pid);
            let _ = storage.set_run_started(&run.id, pid, started_at);
        }
    }
    Ok(())
}

fn stop_active_for(storage: &Storage, kind: &str, target_id: &str) {
    if let Ok(active) = storage.active_runs() {
        for run in active
            .into_iter()
            .filter(|run| run.target_kind == kind && run.target_id == target_id)
        {
            let _ = stop_run_internal(storage, &run.id);
        }
    }
}

fn wait_for_resolution(
    storage: &Storage,
    ids: &[String],
    timeout: Duration,
) -> Result<Vec<Run>, RunError> {
    let deadline = Instant::now() + timeout;
    loop {
        storage.refresh_runs().map_err(RunError::internal)?;

        let mut runs = Vec::new();
        let mut still_pending = false;
        for id in ids {
            if let Some(run) = storage.get_run(id).map_err(RunError::internal)? {
                if storage::is_active_status(&run.status)
                    && run.pid.is_none()
                    && !run.marker.is_empty()
                {
                    still_pending = true;
                }
                runs.push(run);
            }
        }

        if !still_pending || timeout.is_zero() || Instant::now() >= deadline {
            return Ok(runs);
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Launch one saved command. Returns the created run (possibly `starting` for
/// Windows Terminal runs until the PID is resolved by a later poll).
pub fn launch_command(
    storage: &Storage,
    command: &Command,
    options: &LaunchOptions,
) -> Result<Run, RunError> {
    if !command.enabled {
        return Err(RunError::disabled(&command.name));
    }

    let chained = build_chained(command)?;

    if options.force {
        stop_active_for(storage, "command", &command.id);
    }

    let run_id = new_run_id();
    let pending = new_run(command, &options.origin, run_id.clone());
    storage.begin_runs(vec![pending]).map_err(map_begin_error)?;

    let run = storage
        .get_run(&run_id)
        .map_err(RunError::internal)?
        .ok_or_else(|| RunError::internal("Run row disappeared after insert"))?;

    if let Err(error) = spawn_created_run(
        storage,
        &run,
        &chained,
        options.use_existing_window,
        options.group_name_for_window.as_deref(),
    ) {
        let _ = storage.finish_run(&run_id, RUN_FAILED, &error);
        return Err(RunError {
            code: "spawn_failed".to_string(),
            message: format!("Failed to launch \"{}\": {}", command.name, error),
            run_id: Some(run_id),
            pid: None,
        });
    }

    let _ = storage.update_last_executed(&command.id);

    let runs = wait_for_resolution(storage, &[run_id.clone()], options.resolve_timeout)?;
    runs.into_iter()
        .next()
        .ok_or_else(|| RunError::not_found(&run_id))
}

/// Launch an ad-hoc command from `openstart --cli exec` or the legacy
/// `execute_command` Tauri command. Deduplicated by terminal + command text.
pub fn launch_adhoc(
    storage: &Storage,
    name: &str,
    command: &str,
    terminal: &str,
    options: &LaunchOptions,
) -> Result<Run, RunError> {
    let key = format!("{}\u{1f}{}", terminal, command);
    if options.force {
        stop_active_for(storage, "adhoc", &key);
    }

    let run_id = new_run_id();
    let pending = new_adhoc_run(name, command, terminal, &options.origin, run_id.clone());
    storage.begin_runs(vec![pending]).map_err(map_begin_error)?;

    let run = storage
        .get_run(&run_id)
        .map_err(RunError::internal)?
        .ok_or_else(|| RunError::internal("Run row disappeared after insert"))?;

    if let Err(error) = spawn_created_run(
        storage,
        &run,
        command,
        options.use_existing_window,
        options.group_name_for_window.as_deref(),
    ) {
        let _ = storage.finish_run(&run_id, RUN_FAILED, &error);
        return Err(RunError {
            code: "spawn_failed".to_string(),
            message: format!("Failed to launch \"{}\": {}", name, error),
            run_id: Some(run_id),
            pid: None,
        });
    }

    let runs = wait_for_resolution(storage, &[run_id.clone()], options.resolve_timeout)?;
    runs.into_iter()
        .next()
        .ok_or_else(|| RunError::not_found(&run_id))
}

/// Launch a group of commands atomically (either all are started or the group
/// launch is rejected because a member is already active).
pub fn launch_group(
    storage: &Storage,
    group_name: &str,
    options: &LaunchOptions,
) -> Result<BatchOutcome, RunError> {
    let commands = storage
        .get_group_commands(group_name)
        .map_err(RunError::internal)?;
    if commands.is_empty() {
        return Ok(BatchOutcome::default());
    }

    // Disabled group members are skipped; a group with no enabled members
    // cannot be launched at all.
    let enabled: Vec<Command> = commands
        .into_iter()
        .filter(|command| command.enabled)
        .collect();
    if enabled.is_empty() {
        return Err(RunError::no_enabled_commands(group_name));
    }

    if options.force {
        let _ = stop_group_internal(storage, group_name);
    }

    launch_commands(storage, &enabled, options, group_name, false, false)
}

/// Auto-start: launch as many auto-start commands as possible as WT tabs,
/// skipping any whose target is already active. Never blocks the whole batch on
/// a single conflict.
pub fn launch_auto_start(
    storage: &Storage,
    commands: &[Command],
    options: &LaunchOptions,
) -> Result<BatchOutcome, RunError> {
    let enabled: Vec<Command> = commands
        .iter()
        .filter(|command| command.enabled)
        .cloned()
        .collect();
    launch_commands(storage, &enabled, options, "", true, true)
}

fn launch_commands(
    storage: &Storage,
    commands: &[Command],
    options: &LaunchOptions,
    window_group: &str,
    skip_conflicts: bool,
    force_wt_batch: bool,
) -> Result<BatchOutcome, RunError> {
    if commands.is_empty() {
        return Ok(BatchOutcome::default());
    }

    let mut remaining: Vec<&Command> = commands.iter().collect();
    let mut skipped: Vec<String> = Vec::new();
    let mut created: Vec<Run> = Vec::new();

    for attempt in 0..3 {
        if remaining.is_empty() {
            break;
        }
        let pending: Vec<NewRun> = remaining
            .iter()
            .map(|command| new_run(command, &options.origin, new_run_id()))
            .collect();
        match storage.begin_runs(pending) {
            Ok(runs) => {
                created = runs;
                break;
            }
            Err(BeginRunError::Conflict(active)) if skip_conflicts && attempt < 2 => {
                let active_ids: std::collections::HashSet<&str> =
                    active.iter().map(|run| run.target_id.as_str()).collect();
                let (conflict, keep): (Vec<&Command>, Vec<&Command>) = remaining
                    .into_iter()
                    .partition(|command| active_ids.contains(command.id.as_str()));
                skipped.extend(conflict.iter().map(|command| command.name.clone()));
                remaining = keep;
            }
            Err(error) => return Err(map_begin_error(error)),
        }
    }

    if created.is_empty() {
        return Ok(BatchOutcome {
            runs: Vec::new(),
            skipped,
        });
    }

    // Resolve the command + chained script for every created row. Commands with
    // no step are marked failed rather than silently dropped.
    let mut spawnable: Vec<(Run, &Command, String)> = Vec::new();
    for run in &created {
        let Some(command) = commands.iter().find(|command| command.id == run.target_id) else {
            let _ = storage.finish_run(&run.id, RUN_FAILED, "Command definition no longer exists");
            continue;
        };
        match build_chained(command) {
            Ok(chained) => spawnable.push((run.clone(), command, chained)),
            Err(error) => {
                let _ = storage.finish_run(&run.id, RUN_FAILED, &error.message);
            }
        }
    }

    if spawnable.is_empty() {
        let ids: Vec<String> = created.iter().map(|run| run.id.clone()).collect();
        let runs = wait_for_resolution(storage, &ids, Duration::ZERO)?;
        return Ok(BatchOutcome { runs, skipped });
    }

    let all_wt = force_wt_batch
        || spawnable
            .iter()
            .all(|(run, _command, _chained)| is_wt_terminal(&run.terminal));

    if all_wt {
        let tabs: Vec<(String, String, String)> = spawnable
            .iter()
            .map(|(run, command, chained)| {
                (
                    chained.clone(),
                    command.terminal.clone(),
                    run.marker.clone(),
                )
            })
            .collect();
        match script::spawn_terminal_batch_tracked(&tabs, window_group) {
            Ok(_) => {
                for (_, command, _) in &spawnable {
                    let _ = storage.update_last_executed(&command.id);
                }
            }
            Err(error) => {
                let mut first_run = None;
                for (run, _, _) in &spawnable {
                    let _ = storage.finish_run(&run.id, RUN_FAILED, &error);
                    if first_run.is_none() {
                        first_run = Some(run.id.clone());
                    }
                }
                return Err(RunError {
                    code: "spawn_failed".to_string(),
                    message: format!("Failed to launch group: {}", error),
                    run_id: first_run,
                    pid: None,
                });
            }
        }
    } else {
        for (run, command, chained) in &spawnable {
            match spawn_created_run(storage, run, chained, false, None) {
                Ok(()) => {
                    let _ = storage.update_last_executed(&command.id);
                }
                Err(error) => {
                    let _ = storage.finish_run(&run.id, RUN_FAILED, &error);
                }
            }
        }
    }

    let ids: Vec<String> = created.iter().map(|run| run.id.clone()).collect();
    let runs = wait_for_resolution(storage, &ids, options.resolve_timeout)?;
    Ok(BatchOutcome { runs, skipped })
}

// ── Status / stop ─────────────────────────────────────────────────────

pub fn list_runs_impl(storage: &Storage, limit: usize) -> Result<Vec<Run>, String> {
    storage.list_runs(limit)
}

/// Resolve pending Windows Terminal PIDs for the given runs, waiting up to
/// `timeout`. Used by the CLI so `run`/`run-all` can print real PIDs.
pub fn resolve_runs(
    storage: &Storage,
    ids: &[String],
    timeout: Duration,
) -> Result<Vec<Run>, RunError> {
    wait_for_resolution(storage, ids, timeout)
}

pub fn stop_run_internal(storage: &Storage, run_id: &str) -> Result<Run, RunError> {
    storage.refresh_runs().map_err(RunError::internal)?;
    let mut run = storage
        .get_run(run_id)
        .map_err(RunError::internal)?
        .ok_or_else(|| RunError::not_found(run_id))?;

    if !storage::is_active_status(&run.status) {
        return Ok(run);
    }

    // One more refresh attempt: a WT run may have just resolved its PID.
    if run.pid.is_none() {
        storage.refresh_runs().map_err(RunError::internal)?;
        run = storage
            .get_run(run_id)
            .map_err(RunError::internal)?
            .ok_or_else(|| RunError::not_found(run_id))?;
    }

    let Some(pid) = run.pid else {
        return Err(RunError {
            code: "pid_unavailable".to_string(),
            message: format!(
                "Run {} is still starting and its PID is not available yet. Try again in a few seconds.",
                run.id
            ),
            run_id: Some(run.id.clone()),
            pid: None,
        });
    };

    if !process::is_alive(pid, run.pid_started_at) {
        storage
            .finish_run(&run.id, storage::RUN_EXITED, "Process had already exited")
            .map_err(RunError::internal)?;
        return storage
            .get_run(run_id)
            .map_err(RunError::internal)?
            .ok_or_else(|| RunError::not_found(run_id));
    }

    process::kill_tree(pid).map_err(|error| RunError {
        code: "stop_failed".to_string(),
        message: format!(
            "Failed to stop \"{}\" (pid {}): {}",
            run.target_name, pid, error
        ),
        run_id: Some(run.id.clone()),
        pid: Some(pid),
    })?;

    // Wait briefly for the process tree to disappear.
    let deadline = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < deadline {
        if !process::is_alive(pid, run.pid_started_at) {
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }

    if process::is_alive(pid, run.pid_started_at) {
        return Err(RunError {
            code: "stop_failed".to_string(),
            message: format!(
                "Process {} is still running after taskkill; stop it manually",
                pid
            ),
            run_id: Some(run.id),
            pid: Some(pid),
        });
    }

    storage
        .finish_run(&run.id, RUN_STOPPED, "")
        .map_err(RunError::internal)?;
    storage
        .get_run(run_id)
        .map_err(RunError::internal)?
        .ok_or_else(|| RunError::not_found(run_id))
}

pub fn stop_group_internal(storage: &Storage, group_name: &str) -> Result<StopSummary, RunError> {
    let active = storage
        .active_runs_for_group(group_name)
        .map_err(RunError::internal)?;
    let mut summary = StopSummary::default();
    for run in active {
        match stop_run_internal(storage, &run.id) {
            Ok(_) => summary.stopped += 1,
            Err(error) => summary
                .failed
                .push(format!("{}: {}", run.target_name, error.message)),
        }
    }
    Ok(summary)
}

pub fn stop_all_internal(storage: &Storage) -> Result<StopSummary, RunError> {
    let active = storage.active_runs().map_err(RunError::internal)?;
    let mut summary = StopSummary::default();
    for run in active {
        match stop_run_internal(storage, &run.id) {
            Ok(_) => summary.stopped += 1,
            Err(error) => summary
                .failed
                .push(format!("{}: {}", run.target_name, error.message)),
        }
    }
    Ok(summary)
}

// ── Tauri commands ────────────────────────────────────────────────────

#[tauri::command]
pub fn list_runs(storage: State<'_, Storage>, limit: Option<usize>) -> Result<Vec<Run>, String> {
    list_runs_impl(storage.inner(), limit.unwrap_or(100))
}

#[tauri::command(rename_all = "camelCase")]
pub fn stop_run(storage: State<'_, Storage>, run_id: String) -> Result<Run, RunError> {
    stop_run_internal(storage.inner(), &run_id)
}

#[tauri::command(rename_all = "camelCase")]
pub fn stop_group_runs(
    storage: State<'_, Storage>,
    group_name: String,
) -> Result<StopSummary, RunError> {
    stop_group_internal(storage.inner(), &group_name)
}

#[tauri::command]
pub fn stop_all_runs(storage: State<'_, Storage>) -> Result<StopSummary, RunError> {
    stop_all_internal(storage.inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CommandStep;

    fn temp_storage() -> Storage {
        Storage::in_memory()
    }

    fn command(id: &str, group: &str, enabled: bool) -> Command {
        Command {
            id: id.to_string(),
            name: format!("Test {}", id),
            command: "echo hello".to_string(),
            terminal: "powershell".to_string(),
            auto_start: false,
            enabled,
            group_name: group.to_string(),
            steps: vec![CommandStep {
                cmd: "echo hello".to_string(),
                delay_sec: 0,
                note: String::new(),
            }],
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            note: String::new(),
            last_executed_at: String::new(),
        }
    }

    #[test]
    fn disabled_command_cannot_launch() {
        let storage = temp_storage();
        let cmd = command("c1", "", false);
        storage.add_command(&cmd).unwrap();

        let error = launch_command(&storage, &cmd, &LaunchOptions::gui(false, None))
            .expect_err("disabled command must not launch");
        assert_eq!(error.code, "disabled");
    }

    #[test]
    fn group_with_only_disabled_members_is_rejected() {
        let storage = temp_storage();
        storage.add_command(&command("c1", "g", false)).unwrap();
        storage.add_command(&command("c2", "g", false)).unwrap();

        let error = launch_group(&storage, "g", &LaunchOptions::gui(false, None))
            .expect_err("group with no enabled members must not launch");
        assert_eq!(error.code, "no_enabled_commands");
    }
}
