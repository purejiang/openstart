// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use app_lib::autostart;
use app_lib::runner::{self, LaunchOptions, StopSummary};
use app_lib::storage::{Command, Run, Storage};

fn main() {
    let args: Vec<String> = env::args().collect();

    // CLI mode: `openstart --cli <subcommand>`
    if args.len() >= 2 && args[1] == "--cli" {
        // Release builds are GUI-subsystem apps. Attach to the launching
        // terminal so CLI output is visible, then restore the code page.
        let original_codepage = attach_parent_console();
        let code = run_cli(&args[2..]);
        restore_console_codepage(original_codepage);
        std::process::exit(code);
    }

    // Single-instance guard: prevent multiple GUI instances.
    if !ensure_single_instance() {
        return;
    }

    app_lib::run();
}

// ── Single instance ───────────────────────────────────────────────────

fn ensure_single_instance() -> bool {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    unsafe {
        // A session-local mutex works for standard users too; the previous
        // `Global\` name required SeCreateGlobalPrivilege and could panic.
        let mutex = match CreateMutexW(
            None,
            true,
            windows::core::w!("Local\\OpenStart_SingleInstance"),
        ) {
            Ok(handle) => handle,
            // Do not block startup if the guard cannot be created.
            Err(_) => return true,
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(hwnd) = FindWindowW(None, windows::core::w!("OpenStart")) {
                if !hwnd.0.is_null() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                }
            }
            return false;
        }

        // Handle intentionally lives for the whole process (HANDLE has no Drop).
        let _ = mutex;
        true
    }
}

// ── Console attach for CLI mode ───────────────────────────────────────

fn attach_parent_console() -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        AttachConsole, GetConsoleOutputCP, GetConsoleWindow, GetStdHandle, SetConsoleOutputCP,
        SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    unsafe {
        // Debug builds already own a console. For a release GUI-subsystem app
        // launched from a console, attach to the parent console; when stdout
        // was redirected (pipe/file) keep those valid handles instead of
        // replacing them with CONOUT$.
        let has_console = !GetConsoleWindow().0.is_null();
        if has_console {
            return None;
        }
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            // No parent console (e.g. launched from Explorer or a pipe-only
            // parent); leave any inherited handles untouched.
            return None;
        }

        let original = GetConsoleOutputCP();
        let has_stdout = GetStdHandle(STD_OUTPUT_HANDLE).is_ok();

        if !has_stdout {
            if let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
            {
                let handle = HANDLE(file.as_raw_handle());
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
                let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
                std::mem::forget(file);
            }
        }

        // UTF-8 so command names in any language render correctly.
        let _ = SetConsoleOutputCP(65001);
        Some(original)
    }
}

fn restore_console_codepage(original: Option<u32>) {
    if let Some(codepage) = original {
        if codepage != 0 {
            unsafe {
                let _ = windows::Win32::System::Console::SetConsoleOutputCP(codepage);
            }
        }
    }
}

// ── CLI entry ─────────────────────────────────────────────────────────

fn app_data_dir() -> PathBuf {
    app_lib::settings::app_data_dir()
}

fn open_storage(dir: &Path) -> Result<Storage, String> {
    let storage = Storage::new(dir)?;
    // Keep CLI-only usage bounded too (GUI also prunes on startup).
    let _ = storage.prune_runs();
    Ok(storage)
}

fn run_cli(args: &[String]) -> i32 {
    if args.is_empty() {
        print_cli_help();
        return 0;
    }

    let data_dir = app_data_dir();
    if let Err(error) = std::fs::create_dir_all(&data_dir) {
        eprintln!("Error: failed to create app data directory: {}", error);
        return 1;
    }
    // Keep CLI launches consistent with GUI settings (notably keep_terminal_open).
    let _ = app_lib::settings::load_settings(&data_dir);

    match args[0].as_str() {
        "list" => cmd_list(&data_dir),
        "add" => cmd_add(&data_dir, &args[1..]),
        "delete" => cmd_delete(&data_dir, &args[1..]),
        "run" => cmd_run(&data_dir, &args[1..]),
        "group" => cmd_group(&data_dir, &args[1..]),
        "run-all" => cmd_run_all(&data_dir, &args[1..]),
        "exec" => cmd_exec(&data_dir, &args[1..]),
        "status" => cmd_status(&data_dir, &args[1..]),
        "stop" => cmd_stop(&data_dir, &args[1..]),
        "enable" => match autostart::enable_app_autostart() {
            Ok(()) => {
                println!("App auto-start enabled.");
                0
            }
            Err(error) => {
                eprintln!("Error: {}", error);
                1
            }
        },
        "disable" => match autostart::disable_app_autostart() {
            Ok(()) => {
                println!("App auto-start disabled.");
                0
            }
            Err(error) => {
                eprintln!("Error: {}", error);
                1
            }
        },
        "help" | "--help" | "-h" => {
            print_cli_help();
            0
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            print_cli_help();
            1
        }
    }
}

fn split_force(args: &[String]) -> (Vec<String>, bool) {
    let mut positionals = Vec::new();
    let mut force = false;
    for arg in args {
        if arg == "--force" {
            force = true;
        } else {
            positionals.push(arg.clone());
        }
    }
    (positionals, force)
}

fn split_auto(args: &[String]) -> (Vec<String>, bool) {
    let mut positionals = Vec::new();
    let mut auto_start = false;
    for arg in args {
        if arg == "--auto" {
            auto_start = true;
        } else {
            positionals.push(arg.clone());
        }
    }
    (positionals, auto_start)
}

fn exit_code_for(error: &runner::RunError) -> i32 {
    if error.code == "already_running" {
        2
    } else {
        1
    }
}

// ── list / add / delete (unchanged behavior) ─────────────────────────

fn cmd_list(data_dir: &Path) -> i32 {
    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    match storage.list_commands() {
        Ok(commands) => {
            if commands.is_empty() {
                println!("No commands saved.");
            } else {
                for cmd in &commands {
                    let mut tags = Vec::new();
                    if cmd.auto_start {
                        tags.push("[auto]");
                    }
                    if !cmd.enabled {
                        tags.push("[disabled]");
                    }
                    let tags = tags.join(" ");
                    println!(
                        "{} {} | {} | {} -> {}",
                        cmd.id, tags, cmd.name, cmd.terminal, cmd.command
                    );
                }
            }
            0
        }
        Err(error) => {
            eprintln!("Error: {}", error);
            1
        }
    }
}

fn cmd_add(data_dir: &Path, args: &[String]) -> i32 {
    let (positionals, auto_start) = split_auto(args);
    if positionals.len() < 3 {
        eprintln!("Usage: openstart --cli add <name> <command> <terminal> [--auto]");
        return 1;
    }
    let name = positionals[0].clone();
    let command = positionals[1].clone();
    let terminal = positionals[2].clone();

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    let cmd = Command {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        command,
        terminal,
        auto_start,
        enabled: true,
        group_name: String::new(),
        steps: vec![],
        created_at: now.clone(),
        updated_at: now,
        note: String::new(),
        last_executed_at: String::new(),
    };
    match storage.add_command(&cmd) {
        Ok(()) => {
            println!("Added command: {} ({})", cmd.name, cmd.id);
            0
        }
        Err(error) => {
            eprintln!("Error: {}", error);
            1
        }
    }
}

fn cmd_delete(data_dir: &Path, args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: openstart --cli delete <id>");
        return 1;
    }
    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };
    match storage.delete_command(&args[0]) {
        Ok(()) => {
            println!("Deleted command: {}", args[0]);
            0
        }
        Err(error) => {
            eprintln!("Error: {}", error);
            1
        }
    }
}

// ── run / group / run-all / exec ─────────────────────────────────────

fn cmd_run(data_dir: &Path, args: &[String]) -> i32 {
    let (positionals, force) = split_force(args);
    let Some(id) = positionals.first() else {
        eprintln!("Usage: openstart --cli run <id> [--force]");
        return 1;
    };

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };
    let command = match storage.get_command(id) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    let options = LaunchOptions::cli(force);
    match runner::launch_command(&storage, &command, &options) {
        Ok(run) => {
            print_run_started(&run);
            0
        }
        Err(error) => {
            eprintln!("Error: {}", error.message);
            exit_code_for(&error)
        }
    }
}

fn cmd_group(data_dir: &Path, args: &[String]) -> i32 {
    let (positionals, force) = split_force(args);
    if positionals.is_empty() {
        eprintln!("Usage: openstart --cli group <name> [--force]");
        return 1;
    }
    let group = positionals.join(" ");

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    let options = LaunchOptions::cli(force);
    match runner::launch_group(&storage, &group, &options) {
        Ok(outcome) => {
            if outcome.runs.is_empty() && outcome.skipped.is_empty() {
                println!("No commands in group \"{}\".", group);
                return 0;
            }
            for name in &outcome.skipped {
                println!("Skipped: {}", name);
            }
            for run in &outcome.runs {
                print_run_started(run);
            }
            if outcome
                .runs
                .iter()
                .any(|run| run.status == app_lib::storage::RUN_FAILED)
            {
                1
            } else {
                0
            }
        }
        Err(error) => {
            eprintln!("Error: {}", error.message);
            exit_code_for(&error)
        }
    }
}

fn cmd_run_all(data_dir: &Path, args: &[String]) -> i32 {
    let (_, force) = split_force(args);

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };
    let commands = match storage.get_auto_start_commands() {
        Ok(commands) => commands,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };
    if commands.is_empty() {
        println!("No auto-start commands.");
        return 0;
    }

    let mut options = LaunchOptions::cli(force);
    // Launch everything first, then resolve WT PIDs once.
    options.resolve_timeout = Duration::ZERO;

    let mut run_ids = Vec::new();
    let mut blocked = 0;
    let mut failed = false;

    for command in &commands {
        match runner::launch_command(&storage, command, &options) {
            Ok(run) => run_ids.push(run.id),
            Err(error) if error.code == "already_running" => {
                println!("Skipped \"{}\": {}", command.name, error.message);
                blocked += 1;
            }
            Err(error) => {
                eprintln!("Failed \"{}\": {}", command.name, error.message);
                failed = true;
            }
        }
    }

    if !run_ids.is_empty() {
        match runner::resolve_runs(&storage, &run_ids, Duration::from_millis(2500)) {
            Ok(runs) => {
                for run in &runs {
                    print_run_started(run);
                }
            }
            Err(error) => {
                eprintln!("Error: {}", error.message);
                failed = true;
            }
        }
    }

    if failed {
        1
    } else if blocked > 0 {
        2
    } else {
        0
    }
}

fn cmd_exec(data_dir: &Path, args: &[String]) -> i32 {
    let (positionals, force) = split_force(args);
    if positionals.len() < 2 {
        eprintln!("Usage: openstart --cli exec <terminal> <command...> [--force]");
        eprintln!("Example: openstart --cli exec gitbash \"cd /path && some-cmd\"");
        return 1;
    }
    let terminal = positionals[0].clone();
    let command = positionals[1..].join(" ");

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    let options = LaunchOptions::cli(force);
    match runner::launch_adhoc(&storage, &command, &command, &terminal, &options) {
        Ok(run) => {
            print_run_started(&run);
            0
        }
        Err(error) => {
            eprintln!("Error: {}", error.message);
            exit_code_for(&error)
        }
    }
}

// ── status / stop ─────────────────────────────────────────────────────

fn cmd_status(data_dir: &Path, args: &[String]) -> i32 {
    let mut show_all = false;
    let mut run_id: Option<String> = None;
    for arg in args {
        match arg.as_str() {
            "--all" => show_all = true,
            other if other.starts_with("--") => {
                eprintln!("Unknown option: {}", other);
                return 1;
            }
            other => {
                if run_id.is_none() {
                    run_id = Some(other.to_string());
                } else {
                    eprintln!("Usage: openstart --cli status [<run-id>] [--all]");
                    return 1;
                }
            }
        }
    }

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };
    if let Err(error) = storage.refresh_runs() {
        eprintln!("Error: {}", error);
        return 1;
    }

    if let Some(run_id) = run_id {
        match storage.get_run(&run_id) {
            Ok(Some(run)) => {
                print_run_detail(&run);
                0
            }
            Ok(None) => {
                eprintln!("Error: run '{}' was not found", run_id);
                1
            }
            Err(error) => {
                eprintln!("Error: {}", error);
                1
            }
        }
    } else {
        let runs = if show_all {
            storage.list_runs(100)
        } else {
            storage.active_runs()
        };
        match runs {
            Ok(runs) => {
                if runs.is_empty() {
                    if show_all {
                        println!("No runs recorded.");
                    } else {
                        println!("No active runs.");
                    }
                } else {
                    print_runs(&runs);
                }
                print_autostart_status();
                0
            }
            Err(error) => {
                eprintln!("Error: {}", error);
                1
            }
        }
    }
}

fn cmd_stop(data_dir: &Path, args: &[String]) -> i32 {
    let mut run_id: Option<String> = None;
    let mut command_id: Option<String> = None;
    let mut group: Option<String> = None;
    let mut stop_everything = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--all" => stop_everything = true,
            "--command" => {
                index += 1;
                match args.get(index) {
                    Some(value) => command_id = Some(value.clone()),
                    None => {
                        eprintln!("Error: --command requires a command id");
                        return 1;
                    }
                }
            }
            "--group" => {
                index += 1;
                match args.get(index) {
                    Some(value) => group = Some(value.clone()),
                    None => {
                        eprintln!("Error: --group requires a group name");
                        return 1;
                    }
                }
            }
            other if other.starts_with("--") => {
                eprintln!("Unknown option: {}", other);
                return 1;
            }
            other => {
                if run_id.is_none() {
                    run_id = Some(other.to_string());
                } else {
                    eprintln!("Usage: openstart --cli stop <run-id> | --command <id> | --group <name> | --all");
                    return 1;
                }
            }
        }
        index += 1;
    }

    let storage = match open_storage(data_dir) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 1;
        }
    };

    if stop_everything {
        match runner::stop_all_internal(&storage) {
            Ok(summary) => {
                print_stop_summary(&summary);
                if summary.failed.is_empty() {
                    0
                } else {
                    1
                }
            }
            Err(error) => {
                eprintln!("Error: {}", error.message);
                1
            }
        }
    } else if let Some(group) = group {
        match runner::stop_group_internal(&storage, &group) {
            Ok(summary) => {
                print_stop_summary(&summary);
                if summary.failed.is_empty() {
                    0
                } else {
                    1
                }
            }
            Err(error) => {
                eprintln!("Error: {}", error.message);
                1
            }
        }
    } else if let Some(command_id) = command_id {
        let active = match storage.active_runs() {
            Ok(active) => active,
            Err(error) => {
                eprintln!("Error: {}", error);
                return 1;
            }
        };
        let mut summary = StopSummary::default();
        for run in active
            .iter()
            .filter(|run| run.target_kind == "command" && run.target_id == command_id)
        {
            match runner::stop_run_internal(&storage, &run.id) {
                Ok(_) => summary.stopped += 1,
                Err(error) => summary
                    .failed
                    .push(format!("{}: {}", run.target_name, error.message)),
            }
        }
        if summary.stopped == 0 && summary.failed.is_empty() {
            println!("No active runs for command {}.", command_id);
        } else {
            print_stop_summary(&summary);
        }
        if summary.failed.is_empty() {
            0
        } else {
            1
        }
    } else if let Some(run_id) = run_id {
        match runner::stop_run_internal(&storage, &run_id) {
            Ok(run) => {
                if run.status == app_lib::storage::RUN_STOPPED {
                    println!(
                        "Stopped run {} (pid {}, \"{}\").",
                        run.id,
                        pid_label(&run),
                        run.target_name
                    );
                } else {
                    println!("Run {} is not running (status: {}).", run.id, run.status);
                }
                0
            }
            Err(error) => {
                eprintln!("Error: {}", error.message);
                1
            }
        }
    } else {
        eprintln!("Usage: openstart --cli stop <run-id> | --command <id> | --group <name> | --all");
        1
    }
}

// ── Formatting helpers ────────────────────────────────────────────────

fn pid_label(run: &Run) -> String {
    run.pid
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "pending".to_string())
}

fn print_run_started(run: &Run) {
    println!(
        "Started \"{}\" (run {}, pid {}) in {}",
        run.target_name,
        run.id,
        pid_label(run),
        run.terminal
    );
    if run.status == app_lib::storage::RUN_STARTING && run.pid.is_none() {
        println!(
            "Run is starting; check with: openstart --cli status {}",
            run.id
        );
    } else {
        println!("Status: {}", run.status);
    }
}

fn print_run_detail(run: &Run) {
    println!("Run:      {}", run.id);
    println!("Target:   {} ({})", run.target_name, run.target_kind);
    println!(
        "Group:    {}",
        if run.group_name.is_empty() {
            "-"
        } else {
            &run.group_name
        }
    );
    println!("Terminal: {}", run.terminal);
    println!("Origin:   {}", run.origin);
    println!("PID:      {}", pid_label(run));
    println!(
        "Status:   {}{}",
        run.status,
        if run.message.is_empty() {
            String::new()
        } else {
            format!(" ({})", run.message)
        }
    );
    println!("Started:  {}", run.started_at);
    println!(
        "Ended:    {}",
        if run.ended_at.is_empty() {
            "-"
        } else {
            &run.ended_at
        }
    );
    println!("Uptime:   {}", run_elapsed(run));
}

fn print_runs(runs: &[Run]) {
    println!(
        "{:<14} {:<9} {:<8} {:<22} {:<20} {}",
        "ID", "STATUS", "PID", "TARGET", "TERMINAL", "UPTIME"
    );
    for run in runs {
        let mut line = format!(
            "{:<14} {:<9} {:<8} {:<22} {:<20} {}",
            run.id,
            run.status,
            pid_label(run),
            truncate(&run.target_name, 22),
            truncate(&run.terminal, 20),
            run_elapsed(run)
        );
        if !run.message.is_empty() {
            line.push_str(&format!("  | {}", run.message));
        }
        println!("{}", line);
    }
}

fn print_autostart_status() {
    if let Ok(enabled) = autostart::is_app_autostart_enabled() {
        println!();
        println!(
            "App auto-start: {}",
            if enabled { "ENABLED" } else { "DISABLED" }
        );
    }
}

fn print_stop_summary(summary: &StopSummary) {
    println!("Stopped {} run(s).", summary.stopped);
    for failure in &summary.failed {
        eprintln!("Failed: {}", failure);
    }
}

fn run_elapsed(run: &Run) -> String {
    let Ok(start) = chrono::DateTime::parse_from_rfc3339(&run.started_at) else {
        return "-".to_string();
    };
    let end = if run.ended_at.is_empty() {
        chrono::Utc::now()
    } else {
        chrono::DateTime::parse_from_rfc3339(&run.ended_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now())
    };
    let seconds = end
        .signed_duration_since(start.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0) as u64;
    runner::format_duration(seconds)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut truncated: String = value.chars().take(max_chars.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

fn print_cli_help() {
    println!("OpenStart - Startup Command Launcher");
    println!();
    println!("USAGE:");
    println!("  openstart --cli <subcommand> [args]");
    println!();
    println!("SUBCOMMANDS:");
    println!("  list                          List all saved commands");
    println!("  add <name> <cmd> <terminal> [--auto]  Add a command");
    println!("  delete <id>                   Delete a command by ID");
    println!("  run <id> [--force]            Execute a saved command by ID");
    println!("  group <name> [--force]        Execute every command in a group");
    println!("  run-all [--force]             Execute all auto-start commands");
    println!("  exec <terminal> <cmd...> [--force]  Execute ad-hoc (no save)");
    println!("  status [<run-id>] [--all]     Show active runs / run details / history");
    println!("  stop <run-id>                 Stop a tracked run");
    println!("       --command <id>           Stop active runs of a command");
    println!("       --group <name>           Stop active runs of a group");
    println!("       --all                    Stop every active run");
    println!("  enable / disable              Register/unregister app auto-start");
    println!("  help                          Show this help");
    println!();
    println!("A run starts only once; repeating it while active is rejected (exit 2).");
    println!("EXIT CODES: 0 ok, 1 error, 2 already running");
    println!();
    println!("TERMINALS: powershell, cmd, gitbash, terminal, terminal:<name>");
}
