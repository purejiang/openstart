use serde::{Deserialize, Serialize};

/// Represents a detected terminal on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// Unique identifier (e.g. "powershell", "cmd", "gitbash", "terminal:Git Bash")
    pub id: String,
    /// Human-readable display name
    pub name: String,
    /// Path to the terminal executable, or empty string if built-in
    pub path: String,
    /// Whether the terminal is available on this system
    pub available: bool,
}

// ── WT settings.json parsing ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WtSettings {
    profiles: WtProfiles,
}

#[derive(Debug, Deserialize)]
struct WtProfiles {
    #[serde(default)]
    list: Vec<WtProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WtProfile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub hidden: bool,
    pub commandline: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Find Windows Terminal settings.json (packaged + unpackaged paths).
fn find_wt_settings() -> Option<std::path::PathBuf> {
    // Packaged (Microsoft Store)
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let packaged = std::path::PathBuf::from(&local)
        .join("Packages")
        .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
        .join("LocalState")
        .join("settings.json");
    if packaged.is_file() {
        return Some(packaged);
    }
    // Unpackaged / preview
    let unpackaged = std::path::PathBuf::from(&local)
        .join("Microsoft")
        .join("Windows Terminal")
        .join("settings.json");
    if unpackaged.is_file() {
        return Some(unpackaged);
    }
    None
}

/// Read all non-hidden profiles from WT settings.json.
pub fn read_wt_profiles() -> Vec<WtProfile> {
    let path = match find_wt_settings() {
        Some(p) => p,
        None => return vec![],
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let settings: WtSettings = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    settings
        .profiles
        .list
        .into_iter()
        .filter(|p| !p.hidden && !p.name.is_empty())
        .collect()
}

/// Infer shell type from a WT profile's commandline/name.
/// Returns "powershell", "cmd", "gitbash", or "wsl".
pub fn profile_shell_type(profile: &WtProfile) -> &'static str {
    let cmd = profile.commandline.as_deref().unwrap_or("");
    let cmd_lower = cmd.to_lowercase();
    let name_lower = profile.name.to_lowercase();

    if cmd_lower.contains("powershell") || cmd_lower.contains("pwsh")
        || name_lower.contains("powershell") || name_lower.contains("pwsh")
    {
        "powershell"
    } else if name_lower.contains("ubuntu") || name_lower.contains("debian")
        || name_lower.contains("wsl")
    {
        // WSL profiles must run through wsl.exe so Windows → WSL directory
        // mapping (CWD → /mnt/...) is preserved. Running bash directly inside
        // WSL starts in the WSL home dir and breaks relative paths.
        "wsl"
    } else if cmd_lower.contains("bash") || cmd_lower.contains("git")
        || name_lower.contains("bash") || name_lower.contains("git bash")
    {
        "gitbash"
    } else if cmd_lower.contains("cmd.exe")
        || name_lower.contains("command prompt") || name_lower.contains("cmd")
    {
        "cmd"
    } else {
        // Default for unknown profiles
        "powershell"
    }
}

/// Look up a WT profile by name.
pub fn find_wt_profile(name: &str) -> Option<WtProfile> {
    read_wt_profiles().into_iter().find(|p| p.name == name)
}

// ── Executable lookup ─────────────────────────────────────────────────

/// Check whether an executable exists on the system PATH.
pub fn find_in_path(exe_name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let full_name = if exe_name.ends_with(ext) {
        exe_name.to_string()
    } else {
        format!("{}{}", exe_name, ext)
    };
    for dir in path_var.split(';') {
        let candidate = std::path::Path::new(dir).join(&full_name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn file_exists(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

fn terminal(id: &str, name: &str, path: Option<String>) -> TerminalInfo {
    match path {
        Some(p) => TerminalInfo {
            id: id.to_string(),
            name: name.to_string(),
            path: p,
            available: true,
        },
        None => TerminalInfo {
            id: id.to_string(),
            name: name.to_string(),
            path: String::new(),
            available: false,
        },
    }
}

// ── Main detection ────────────────────────────────────────────────────

/// Detect all terminals available on the system.
pub fn detect_terminals() -> Vec<TerminalInfo> {
    let mut terminals = Vec::new();

    // ── PowerShell ──────────────────────────────────────────────────
    let ps_path = find_in_path("pwsh.exe").or_else(|| find_in_path("powershell.exe"));
    let ps_name = match &ps_path {
        Some(p) if p.to_lowercase().contains("pwsh") => "PowerShell",
        _ => "Windows PowerShell",
    };
    if ps_path.is_some() {
        terminals.push(terminal("powershell", ps_name, ps_path));
    } else {
        let sys_ps = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
        if file_exists(sys_ps) {
            terminals.push(terminal("powershell", "Windows PowerShell", Some(sys_ps.to_string())));
        } else {
            terminals.push(terminal("powershell", "Windows PowerShell", None));
        }
    }

    // ── CMD ─────────────────────────────────────────────────────────
    let cmd_path = find_in_path("cmd.exe").unwrap_or_else(|| "cmd.exe".to_string());
    terminals.push(terminal("cmd", "Command Prompt", Some(cmd_path)));

    // ── Git Bash ────────────────────────────────────────────────────
    let git_bash = find_bash_path();
    terminals.push(terminal("gitbash", "Git Bash", git_bash));

    // ── Windows Terminal + sub-profiles (dynamic from settings.json) ─
    let wt_path = find_in_path("wt.exe");
    if wt_path.is_some() {
        terminals.push(terminal("terminal", "Windows Terminal", wt_path));

        // Read WT config and add one entry per non-hidden profile
        for profile in read_wt_profiles() {
            terminals.push(TerminalInfo {
                id: format!("terminal:{}", profile.name),
                name: format!("Windows Terminal: {}", profile.name),
                path: String::new(),
                available: true,
            });
        }
    }

    terminals
}

// ── Bash path finder ──────────────────────────────────────────────────

/// Find bash.exe for Git Bash — searches common install paths first,
/// then PATH (skipping WSL/System32 entries).
pub fn find_bash_path() -> Option<String> {
    for drive in &["C", "D", "E", "F"] {
        let path = format!("{}:\\Program Files\\Git\\bin\\bash.exe", drive);
        if std::path::Path::new(&path).is_file() {
            return Some(path);
        }
    }
    for drive in &["C", "D"] {
        let path = format!("{}:\\Program Files (x86)\\Git\\bin\\bash.exe", drive);
        if std::path::Path::new(&path).is_file() {
            return Some(path);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(';') {
            let lower = dir.to_lowercase();
            if lower.contains("system32") || lower.contains("windowsapps") {
                continue;
            }
            let candidate = std::path::Path::new(dir).join("bash.exe");
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}
