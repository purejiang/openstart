use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "OpenStart";
const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn enable_app_autostart() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_path_str = exe_path
        .to_str()
        .ok_or_else(|| "Executable path contains invalid Unicode characters".to_string())?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

    run_key
        .set_value(APP_NAME, &exe_path_str)
        .map_err(|e| format!("Failed to set autostart registry value: {}", e))?;

    Ok(())
}

pub fn disable_app_autostart() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

    match run_key.delete_value(APP_NAME) {
        Ok(()) => Ok(()),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Failed to remove autostart registry value: {}", e)),
    }
}

pub fn is_app_autostart_enabled() -> Result<bool, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY_PATH, KEY_READ)
        .map_err(|e| format!("Failed to open Run key: {}", e))?;

    match run_key.get_value::<String, _>(APP_NAME) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
