use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

const LOG_FILE: &str = "openstart.log";
const MAX_CMD_PREVIEW: usize = 80;

/// Initialize the log file by creating it, storing the path, and writing a startup header.
pub fn init_logger(app_data_dir: &Path) {
    let log_path = app_data_dir.join(LOG_FILE);
    let _ = fs::create_dir_all(app_data_dir);
    let mut file = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    let _ = writeln!(file, "[{}] ──── OpenStart session started ────", now);
    let _ = LOG_PATH.set(log_path);
}

/// Append a spawn event to the log file. Uses the path stored by `init_logger`.
pub fn log_spawn(terminal: &str, command_preview: &str, success: bool, error: Option<&str>) {
    let log_path = match LOG_PATH.get() {
        Some(p) => p,
        None => return,
    };
    let mut file = match OpenOptions::new().create(true).append(true).open(log_path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    let preview = truncate(command_preview, MAX_CMD_PREVIEW);
    if success {
        let _ = writeln!(file, "[{}] SPAWN | terminal={} | cmd={} | OK", now, terminal, preview);
    } else {
        let err = error.unwrap_or("unknown error");
        let _ = writeln!(file, "[{}] SPAWN | terminal={} | cmd={} | FAIL: {}", now, terminal, preview, err);
    }
}

/// Read the last `max_lines` lines from the log file.
pub fn read_logs(app_data_dir: &Path, max_lines: usize) -> Result<String, String> {
    let log_path = app_data_dir.join(LOG_FILE);
    let content = fs::read_to_string(&log_path).map_err(|e| format!("Failed to read log: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return Ok(content);
    }
    let start = lines.len() - max_lines;
    Ok(lines[start..].join("\n"))
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
