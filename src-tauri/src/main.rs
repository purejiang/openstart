// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    // CLI mode: `openstart --cli <subcommand>`
    if args.len() >= 2 && args[1] == "--cli" {
        run_cli(&args[2..]);
        return;
    }

    // GUI mode
    app_lib::run();
}

fn run_cli(args: &[String]) {
    if args.is_empty() {
        print_cli_help();
        return;
    }

    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("openstart");
    std::fs::create_dir_all(&app_data_dir).ok();

    match args[0].as_str() {
        "list" => {
            let storage = app_lib::storage::Storage::new(&app_data_dir)
                .expect("Failed to open database");
            match storage.list_commands() {
                Ok(cmds) => {
                    if cmds.is_empty() {
                        println!("No commands saved.");
                    } else {
                        for cmd in &cmds {
                            let auto = if cmd.auto_start { "[auto]" } else { "" };
                            println!("{} {} | {} | {} -> {}",
                                cmd.id, auto, cmd.name, cmd.terminal, cmd.command);
                        }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "add" => {
            if args.len() < 4 {
                eprintln!("Usage: openstart --cli add <name> <command> <terminal> [--auto]");
                return;
            }
            let name = args[1].clone();
            let command = args[2].clone();
            let terminal = args[3].clone();
            let auto_start = args.get(4).map_or(false, |a| a == "--auto");

            let storage = app_lib::storage::Storage::new(&app_data_dir)
                .expect("Failed to open database");
            let now = chrono::Utc::now().to_rfc3339();
            let cmd = app_lib::storage::Command {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                command,
                terminal,
                auto_start,
                group_name: String::new(),
                created_at: now.clone(),
                updated_at: now,
            };
            match storage.add_command(&cmd) {
                Ok(()) => println!("Added command: {} ({})", cmd.name, cmd.id),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "delete" => {
            if args.len() < 2 {
                eprintln!("Usage: openstart --cli delete <id>");
                return;
            }
            let storage = app_lib::storage::Storage::new(&app_data_dir)
                .expect("Failed to open database");
            match storage.delete_command(&args[1]) {
                Ok(()) => println!("Deleted command: {}", args[1]),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "run-all" => {
            let storage = app_lib::storage::Storage::new(&app_data_dir)
                .expect("Failed to open database");
            match storage.get_auto_start_commands() {
                Ok(cmds) => {
                    for cmd in &cmds {
                        println!("Executing: {} ({})", cmd.name, cmd.command);
                        execute_in_terminal(&cmd.command, &cmd.terminal);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "status" => {
            match app_lib::autostart::is_app_autostart_enabled() {
                Ok(enabled) => {
                    println!("App auto-start: {}", if enabled { "ENABLED" } else { "DISABLED" });
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "enable" => {
            match app_lib::autostart::enable_app_autostart() {
                Ok(()) => println!("App auto-start enabled."),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "disable" => {
            match app_lib::autostart::disable_app_autostart() {
                Ok(()) => println!("App auto-start disabled."),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        "exec" => {
            if args.len() < 3 {
                eprintln!("Usage: openstart --cli exec <terminal> <command>");
                eprintln!("Example: openstart --cli exec gitbash \"cd /path && some-cmd\"");
                return;
            }
            let terminal = &args[1];
            // Join remaining args as the command string (handles spaces without quoting)
            let command = args[2..].join(" ");
            println!("Executing in {}: {}", terminal, command);
            execute_in_terminal(&command, terminal);
        }
        "run" => {
            // Execute a saved command by id
            if args.len() < 2 {
                eprintln!("Usage: openstart --cli run <id>");
                return;
            }
            let storage = app_lib::storage::Storage::new(&app_data_dir)
                .expect("Failed to open database");
            match storage.get_command(&args[1]) {
                Ok(cmd) => {
                    println!("Executing: {} ({})", cmd.name, cmd.command);
                    execute_in_terminal(&cmd.command, &cmd.terminal);
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown subcommand: {}", args[0]);
            print_cli_help();
        }
    }
}

fn execute_in_terminal(command: &str, terminal: &str) {
    use std::process::Command;

    let (program, args): (&str, Vec<String>) = match terminal {
        "powershell" => (
            "powershell.exe",
            vec!["-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
        ),
        "cmd" => (
            "cmd.exe",
            vec!["/k".into(), command.to_string()],
        ),
        "gitbash" => (
            "bash.exe",
            vec!["-i".into(), "-c".into(), command.to_string()],
        ),
        t if t.starts_with("terminal") => {
            let profile_name = if let Some((_, p)) = t.split_once(':') { p } else { "" };
            if profile_name.is_empty() {
                (
                    "wt.exe",
                    vec!["powershell.exe".into(), "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
                )
            } else {
                let profile = app_lib::terminal::find_wt_profile(profile_name);
                let shell = profile.as_ref()
                    .map(|p| app_lib::terminal::profile_shell_type(p))
                    .unwrap_or("powershell");
                match shell {
                    "gitbash" => {
                        let bash = app_lib::terminal::find_bash_path()
                            .unwrap_or_else(|| "bash".to_string());
                        (
                            "wt.exe",
                            vec!["-p".into(), profile_name.to_string(), "--".into(), bash, "-i".into(), "-c".into(), command.to_string()],
                        )
                    }
                    "cmd" => (
                        "wt.exe",
                        vec!["-p".into(), profile_name.to_string(), "--".into(), "cmd.exe".into(), "/k".into(), command.to_string()],
                    ),
                    _ => (
                        "wt.exe",
                        vec!["-p".into(), profile_name.to_string(), "--".into(), "powershell.exe".into(), "-NoExit".into(), "-Command".into(), format!("& {{{}}}", command)],
                    ),
                }
            }
        }
        _ => {
            eprintln!("Unknown terminal: {}", terminal);
            return;
        }
    };

    match Command::new(program).args(&args).spawn() {
        Ok(_) => {}
        Err(e) => eprintln!("Failed to execute: {}", e),
    }
}

fn print_cli_help() {
    println!("OpenStart - Startup Command Launcher");
    println!();
    println!("USAGE:");
    println!("  openstart --cli <subcommand> [args]");
    println!();
    println!("SUBCOMMANDS:");
    println!("  list                  List all saved commands");
    println!("  add <name> <cmd> <t> [--auto]  Add a command (t=terminal)");
    println!("  exec <terminal> <cmd> Execute command directly (no save)");
    println!("  run <id>              Execute a saved command by ID");
    println!("  delete <id>           Delete a command by ID");
    println!("  run-all               Execute all auto-start commands");
    println!("  status                Show auto-start registry status");
    println!("  enable                Register app for auto-start");
    println!("  disable               Unregister app from auto-start");
    println!();
    println!("TERMINALS: powershell, cmd, gitbash, terminal, terminal:<name> (from WT profiles)");
}
