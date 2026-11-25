//! Windows process inspection helpers.
//!
//! Responsibilities:
//! - PID liveness checks with PID-reuse protection (creation time compare).
//! - Process-tree termination (`taskkill /T /F`, no console window).
//! - Windows Terminal shell PID discovery.
//!
//! Windows Terminal launches the actual tab shell from an already-running
//! `WindowsTerminal.exe` server, so the `wt.exe` PID we spawn is only a
//! short-lived launcher. To find the real shell process we inject a unique
//! marker (`OSRUN-<run-id>`) into the shell command line and scan process
//! command lines for it. The scan prefers processes whose parent is a terminal
//! host (`WindowsTerminal.exe` / `OpenConsole.exe` / `conhost.exe`).

use std::collections::HashMap;

use windows::core::{PCWSTR, PWSTR};
use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, GetProcessTimes, OpenProcess, CREATE_NEW_CONSOLE,
    PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
};

const STILL_ACTIVE: u32 = 259;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
const STATUS_BUFFER_TOO_SMALL: i32 = 0xC000_0023u32 as i32;
const STATUS_BUFFER_OVERFLOW: i32 = 0x8000_0005u32 as i32;
const ERROR_ACCESS_DENIED_HRESULT: i32 = 0x8007_0005u32 as i32;

/// Layout of `UNICODE_STRING` returned by `ProcessCommandLineInformation`.
/// Defined locally so we do not depend on which windows-rs module exports it.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[derive(Debug, Clone)]
struct ProcEntry {
    pid: u32,
    parent_pid: u32,
    exe: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    pid: u32,
    started: u64,
    terminal_parent: bool,
}

fn filetime_to_unix(ft: FILETIME) -> u64 {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ticks
        .saturating_div(10_000_000)
        .saturating_sub(11_644_473_600)
}

fn is_access_denied(err: &windows::core::Error) -> bool {
    err.code().0 == ERROR_ACCESS_DENIED_HRESULT
}

unsafe fn process_creation_time(handle: HANDLE) -> Option<u64> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
    Some(filetime_to_unix(creation))
}

unsafe fn command_line_of(handle: HANDLE) -> Option<String> {
    let mut buffer = vec![0u8; 4096];
    let mut returned: u32 = 0;
    let mut status = NtQueryInformationProcess(
        handle,
        ProcessCommandLineInformation,
        buffer.as_mut_ptr() as *mut _,
        buffer.len() as u32,
        &mut returned,
    );

    let mut tries = 0;
    while status.0 != 0 && tries < 3 {
        let growable = status.0 == STATUS_INFO_LENGTH_MISMATCH
            || status.0 == STATUS_BUFFER_TOO_SMALL
            || status.0 == STATUS_BUFFER_OVERFLOW;
        if !growable {
            break;
        }
        let target = if returned as usize > buffer.len() {
            returned as usize
        } else {
            buffer.len().saturating_mul(2)
        };
        if target == 0 || target > 1_048_576 {
            break;
        }
        buffer.resize(target, 0);
        status = NtQueryInformationProcess(
            handle,
            ProcessCommandLineInformation,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
        );
        tries += 1;
    }

    if status.0 != 0 {
        return None;
    }

    // The buffer starts with a UNICODE_STRING whose `Buffer` points into the
    // same allocation, followed by the UTF-16 command line.
    let unicode = &*(buffer.as_ptr() as *const UnicodeString);
    if unicode.buffer.is_null() || unicode.length == 0 {
        return None;
    }
    let words = (unicode.length / 2) as usize;
    let slice = std::slice::from_raw_parts(unicode.buffer as *const u16, words);
    Some(String::from_utf16_lossy(slice))
}

/// Read a process command line + creation time, skipping processes older than
/// `since` (Unix seconds) by more than 10 seconds. `since == 0` disables the
/// time filter.
unsafe fn query_process(pid: u32, since: u64) -> Option<(String, u64)> {
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let created = process_creation_time(handle);
    if since > 0 {
        if let Some(created) = created {
            if created.saturating_add(10) < since {
                let _ = CloseHandle(handle);
                return None;
            }
        }
    }
    let cmd = command_line_of(handle);
    let _ = CloseHandle(handle);
    cmd.map(|line| (line, created.unwrap_or(0)))
}

fn snapshot() -> Result<Vec<ProcEntry>, String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| format!("CreateToolhelp32Snapshot failed: {}", e))?;
        if snap.is_invalid() {
            return Err("CreateToolhelp32Snapshot returned an invalid handle".to_string());
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();

        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                processes.push(ProcEntry {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    exe: String::from_utf16_lossy(&entry.szExeFile[..end]),
                });
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snap);
        Ok(processes)
    }
}

/// Creation time of `pid` as Unix seconds (UTC). `None` if the process no
/// longer exists or cannot be queried.
pub fn start_time_of(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let created = process_creation_time(handle);
        let _ = CloseHandle(handle);
        created
    }
}

/// Is `pid` still running?
///
/// When `expected_started_at` is provided it is compared with the current
/// process creation time; a mismatch means the PID was recycled and the
/// original process is gone. Access-denied is treated as alive (we cannot
/// prove it exited, and treating it as dead could allow duplicate launches).
pub fn is_alive(pid: u32, expected_started_at: Option<u64>) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => handle,
            Err(err) => return is_access_denied(&err),
        };

        let mut exit_code: u32 = 0;
        let exit_ok = GetExitCodeProcess(handle, &mut exit_code).is_ok();
        let created = process_creation_time(handle);
        let _ = CloseHandle(handle);

        if !exit_ok {
            // Could not query; be conservative and treat it as running.
            return true;
        }
        if exit_code != STILL_ACTIVE {
            return false;
        }
        if let (Some(expected), Some(actual)) = (expected_started_at, created) {
            if expected != actual {
                return false;
            }
        }
        true
    }
}

/// Forcefully terminate a process and all of its descendants.
/// Uses `taskkill /T /F` so the whole command tree (e.g. shell -> node) dies.
pub fn kill_tree(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID 0".to_string());
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to run taskkill: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "taskkill failed ({}): {} {}",
        output.status, stderr, stdout
    ))
}

// ── Console process spawning ──────────────────────────────────────────

/// Append one argument to a Windows command line using the standard
/// `CommandLineToArgvW` / CRT quoting rules.
fn push_quoted_arg(line: &mut String, arg: &str) {
    let needs_quotes = arg.is_empty() || arg.chars().any(|ch| ch == ' ' || ch == '\t' || ch == '"');

    if !needs_quotes {
        line.push_str(arg);
        return;
    }

    line.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                // Escape the quote and double the preceding backslashes.
                for _ in 0..(backslashes * 2 + 1) {
                    line.push('\\');
                }
                line.push('"');
                backslashes = 0;
            }
            _ => {
                for _ in 0..backslashes {
                    line.push('\\');
                }
                backslashes = 0;
                line.push(ch);
            }
        }
    }
    // Trailing backslashes must be doubled before the closing quote.
    for _ in 0..(backslashes * 2) {
        line.push('\\');
    }
    line.push('"');
}

fn build_command_line(program: &str, args: &[String]) -> Vec<u16> {
    let mut line = String::new();
    push_quoted_arg(&mut line, program);
    for arg in args {
        line.push(' ');
        push_quoted_arg(&mut line, arg);
    }
    line.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Start a console application in its own new console window.
///
/// `std::process::Command` sets `STARTF_USESTDHANDLES` even for inherited
/// stdio, which makes the child inherit the caller's console/pipe handles.
/// For a CLI invocation that means the terminal's output leaks into the
/// caller's console and a piped caller can block until the terminal closes.
/// Using `CreateProcessW` without `STARTF_USESTDHANDLES` lets the new console
/// own the child's std handles.
pub fn spawn_new_console(program: &str, args: &[String]) -> Result<u32, String> {
    let mut command_line = build_command_line(program, args);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            // Let CreateProcess parse the quoted program from the command line
            // and use its normal PATH/System32 search (a bare `lpApplicationName`
            // is not searched and fails with ERROR_FILE_NOT_FOUND).
            PCWSTR::null(),
            PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NEW_CONSOLE,
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )
        .map_err(|e| format!("CreateProcessW failed: {}", e))?;

        let pid = info.dwProcessId;
        let _ = CloseHandle(info.hProcess);
        let _ = CloseHandle(info.hThread);
        Ok(pid)
    }
}

/// Find shell PIDs for the given `(marker, run_started_at_unix)` specs.
///
/// Returns `marker -> (pid, process_created_at_unix)`. Only processes whose
/// command line contains the marker and whose creation time is close to (or
/// after) the run start are considered. Processes parented by a terminal host
/// are preferred; if no such candidate exists, a unique marker match is
/// accepted as a fallback.
pub fn discover_markers(specs: &[(String, u64)]) -> HashMap<String, (u32, u64)> {
    let mut found = HashMap::new();
    if specs.is_empty() {
        return found;
    }

    let processes = match snapshot() {
        Ok(processes) => processes,
        Err(_) => return found,
    };
    let by_pid: HashMap<u32, &ProcEntry> = processes.iter().map(|p| (p.pid, p)).collect();
    let min_since = specs
        .iter()
        .map(|(_, since)| *since)
        .min()
        .unwrap_or(0)
        .saturating_sub(10);
    let self_pid = std::process::id();

    let mut candidates: HashMap<String, Vec<Candidate>> = HashMap::new();

    for process in &processes {
        if process.pid <= 4 || process.pid == self_pid || process.pid == 0 {
            continue;
        }
        let exe_lower = process.exe.to_lowercase();
        // The marker also appears in the `wt.exe` launcher command line, and in
        // our own process if a run command happens to echo it. Skip non-shells.
        if matches!(
            exe_lower.as_str(),
            "openstart.exe" | "wt.exe" | "windowsterminal.exe" | "conhost.exe" | "openconsole.exe"
        ) {
            continue;
        }

        let Some((cmdline, created)) = (unsafe { query_process(process.pid, min_since) }) else {
            continue;
        };

        for (marker, since) in specs {
            if !cmdline.contains(marker.as_str()) {
                continue;
            }
            if created != 0 && created.saturating_add(10) < *since {
                continue;
            }
            let parent_exe = by_pid
                .get(&process.parent_pid)
                .map(|parent| parent.exe.to_lowercase())
                .unwrap_or_default();
            let terminal_parent = matches!(
                parent_exe.as_str(),
                "windowsterminal.exe" | "openconsole.exe" | "conhost.exe"
            );
            candidates
                .entry(marker.clone())
                .or_default()
                .push(Candidate {
                    pid: process.pid,
                    started: created,
                    terminal_parent,
                });
        }
    }

    for (marker, mut list) in candidates {
        // Prefer a shell whose parent is a terminal host; otherwise earliest.
        list.sort_by_key(|candidate| (!candidate.terminal_parent, candidate.started));
        if let Some(best) = list.first() {
            found.insert(marker, (best.pid, best.started));
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn current_process_is_alive() {
        let pid = std::process::id();
        let started = start_time_of(pid).expect("own process should expose a start time");
        assert!(is_alive(pid, Some(started)));
        assert!(is_alive(pid, None));
        // A mismatched creation time means the PID was reused.
        assert!(!is_alive(pid, Some(started.saturating_add(10_000))));
    }

    #[test]
    fn dead_child_is_not_alive() {
        let mut child = Command::new("cmd.exe")
            .args(["/C", "exit 0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd.exe");
        let pid = child.id();
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!is_alive(pid, None));
    }

    #[test]
    fn command_line_marker_is_discoverable() {
        let marker = format!("OSRUN-test-{}", std::process::id());
        let mut child = Command::new("cmd.exe")
            .args(["/C", &format!("ping -n 30 127.0.0.1 >nul & rem {}", marker)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd.exe");
        std::thread::sleep(std::time::Duration::from_millis(250));

        assert!(is_alive(child.id(), None));
        let found = discover_markers(&[(marker.clone(), 0)]);
        let _ = kill_tree(child.id());
        let _ = child.wait();

        assert!(
            found.contains_key(&marker),
            "spawned process command line should be discoverable by marker"
        );
    }

    #[test]
    fn command_line_quoting_matches_windows_rules() {
        let args = vec![
            "C:\\Program Files\\x.exe".to_string(),
            "a\"b".to_string(),
            "C:\\dir\\".to_string(),
            String::new(),
            "plain".to_string(),
        ];
        let wide = build_command_line("prog.exe", &args);
        let line = String::from_utf16(&wide[..wide.len() - 1]).unwrap();
        assert_eq!(
            line,
            r#"prog.exe "C:\Program Files\x.exe" "a\"b" C:\dir\ "" plain"#
        );
    }

    #[test]
    fn kill_tree_terminates_child() {
        let mut child = Command::new("cmd.exe")
            .args(["/C", "ping -n 30 127.0.0.1 >nul"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd.exe");
        let pid = child.id();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(is_alive(pid, None));

        kill_tree(pid).expect("taskkill should succeed");
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert!(!is_alive(pid, None));
        let _ = child.wait();
    }
}
