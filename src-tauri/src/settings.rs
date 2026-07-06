use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Runtime flag for whether to keep terminal windows open after command execution.
/// Default: true. Updated when settings are loaded or changed.
static KEEP_OPEN: AtomicBool = AtomicBool::new(true);

pub fn set_keep_open(keep: bool) {
    KEEP_OPEN.store(keep, Ordering::Relaxed);
}

pub fn is_keep_open() -> bool {
    KEEP_OPEN.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub app_autostart: bool,
    #[serde(default = "default_delay")]
    pub startup_delay_seconds: u64,
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    #[serde(default = "default_keep_open")]
    pub keep_terminal_open: bool,
}

fn default_delay() -> u64 {
    3
}

fn default_close_to_tray() -> bool {
    true
}

fn default_keep_open() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_autostart: false,
            startup_delay_seconds: default_delay(),
            close_to_tray: default_close_to_tray(),
            keep_terminal_open: default_keep_open(),
        }
    }
}

/// Load settings from `openstart_settings.json` in the app data directory.
/// Returns defaults if file doesn't exist or can't be parsed.
/// Also syncs the runtime keep_open flag.
pub fn load_settings(app_data_dir: &Path) -> AppSettings {
    let path = app_data_dir.join("openstart_settings.json");
    let settings: AppSettings = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    };
    set_keep_open(settings.keep_terminal_open);
    settings
}

/// Save settings to `openstart_settings.json`.
/// Also syncs the runtime keep_open flag.
pub fn save_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = app_data_dir.join("openstart_settings.json");
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;
    set_keep_open(settings.keep_terminal_open);
    Ok(())
}
