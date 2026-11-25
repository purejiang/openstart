pub mod autostart;
pub mod commands;
pub mod logger;
pub mod process;
pub mod runner;
pub mod script;
pub mod settings;
pub mod storage;
pub mod terminal;
pub mod updater;

use commands::AppDataDir;
use storage::Storage;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri::State;

#[tauri::command]
fn check_update() -> Result<updater::UpdateInfo, String> {
    updater::check_for_updates()
}

#[tauri::command]
fn read_logs(data_dir: State<'_, AppDataDir>, max_lines: Option<usize>) -> Result<String, String> {
    logger::read_logs(&data_dir.0, max_lines.unwrap_or(200))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = settings::app_data_dir();

    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    logger::init_logger(&app_data_dir);

    let storage = Storage::new(&app_data_dir).expect("Failed to initialize database");

    // Clone before moving into AppDataDir state
    let close_data_dir = app_data_dir.clone();

    // Load settings for startup delay
    let app_settings = settings::load_settings(&app_data_dir);
    let startup_delay = app_settings.startup_delay_seconds;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(storage)
        .manage(AppDataDir(app_data_dir))
        .setup(move |app| {
            // System tray
            let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Auto-execute startup commands as tabs in one Terminal window.
            // All launches go through the run registry so duplicates are skipped
            // and the GUI can show their PID/status.
            let storage = app.state::<Storage>();
            let _ = storage.prune_runs();
            if let Ok(auto_cmds) = storage.get_auto_start_commands() {
                if !auto_cmds.is_empty() {
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(startup_delay));
                        let storage = handle.state::<Storage>();
                        match crate::runner::launch_auto_start(
                            storage.inner(),
                            &auto_cmds,
                            &crate::runner::LaunchOptions::auto_start(),
                        ) {
                            Ok(outcome) => {
                                for skipped in &outcome.skipped {
                                    logger::log_spawn(
                                        "autostart",
                                        &format!("skipped (already running): {}", skipped),
                                        true,
                                        None,
                                    );
                                }
                            }
                            Err(error) => logger::log_spawn(
                                "autostart",
                                &format!("auto-start failed: {}", error.message),
                                false,
                                Some(&error.message),
                            ),
                        }
                    });
                }
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_terminals,
            commands::list_commands,
            commands::add_command,
            commands::update_command,
            commands::set_command_enabled,
            commands::delete_command,
            commands::mark_command_executed,
            commands::execute_command,
            commands::execute_command_by_id,
            commands::execute_group,
            commands::delete_group,
            commands::get_auto_start_commands,
            commands::toggle_app_autostart,
            commands::get_app_autostart_status,
            commands::get_settings,
            commands::update_settings,
            commands::get_app_info,
            check_update,
            read_logs,
            runner::list_runs,
            runner::stop_run,
            runner::stop_group_runs,
            runner::stop_all_runs,
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let settings = settings::load_settings(&close_data_dir);
                if settings.close_to_tray {
                    let _ = window.hide();
                    api.prevent_close();
                }
                // else: close proceeds normally → app exits
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
