use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub app_autostart: bool,
    #[serde(default = "default_delay")]
    pub startup_delay_seconds: u64,
}

fn default_delay() -> u64 {
    3
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_autostart: false,
            startup_delay_seconds: default_delay(),
        }
    }
}

/// Load settings from `openstart_settings.json` in the app data directory.
/// Returns defaults if file doesn't exist or can't be parsed.
pub fn load_settings(app_data_dir: &Path) -> AppSettings {
    let path = app_data_dir.join("openstart_settings.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

/// Save settings to `openstart_settings.json`.
pub fn save_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = app_data_dir.join("openstart_settings.json");
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;
    Ok(())
}
