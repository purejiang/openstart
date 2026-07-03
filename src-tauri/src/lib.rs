pub mod storage;
pub mod terminal;
pub mod autostart;
pub mod settings;
pub mod commands;
pub mod updater;

use commands::AppDataDir;
use storage::Storage;
use tauri::Manager;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

#[tauri::command]
fn check_update() -> Result<updater::UpdateInfo, String> {
    updater::check_for_updates()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("openstart");

    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    let storage =
        Storage::new(&app_data_dir).expect("Failed to initialize database");

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

            // Auto-execute startup commands as tabs in one Terminal window
            let storage = app.state::<Storage>();
            if let Ok(auto_cmds) = storage.get_auto_start_commands() {
                if !auto_cmds.is_empty() {
                    let cmds: Vec<(String, String)> = auto_cmds
                        .iter()
                        .map(|c| {
                            let steps = commands::steps_for_command(c);
                            let shell = commands::shell_of(&c.terminal);
                            let script = commands::build_chained_script(&steps, shell);
                            let effective_cmd = if script.is_empty() { c.command.clone() } else { script };
                            (effective_cmd, c.terminal.clone())
                        })
                        .collect();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(startup_delay));
                        let _ = commands::spawn_terminal_batch(&cmds);
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
            commands::delete_command,
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
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
