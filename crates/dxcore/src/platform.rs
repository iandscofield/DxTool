//! Process-level helpers: administrator checks, relaunching elevated, firmware type.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::SystemInformation::{GetFirmwareType, FIRMWARE_TYPE, FirmwareTypeBios, FirmwareTypeUefi};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

/// Starts a new elevated copy of this program (Windows shows the UAC prompt).
/// The caller should exit afterwards.
pub fn relaunch_as_admin() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let file = HSTRING::from(exe.as_os_str());
    let verb = HSTRING::from("runas");
    let r = unsafe {
        ShellExecuteW(None, &verb, &file, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
    };
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err("Administrator access was declined or could not be requested.".into())
    }
}

/// Opens a file or folder with its default program.
pub fn open_path(path: &str) -> Result<(), String> {
    let file = HSTRING::from(path);
    let verb = HSTRING::from("open");
    let r = unsafe { ShellExecuteW(None, &verb, &file, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if r.0 as isize > 32 { Ok(()) } else { Err(format!("Could not open {path}")) }
}

/// Opens File Explorer with the file selected.
pub fn show_in_folder(path: &str) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn firmware_type() -> &'static str {
    let mut t = FIRMWARE_TYPE::default();
    if unsafe { GetFirmwareType(&mut t) }.is_err() {
        return "Unknown";
    }
    if t == FirmwareTypeUefi {
        "UEFI"
    } else if t == FirmwareTypeBios {
        "Legacy BIOS"
    } else {
        "Unknown"
    }
}

pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "This PC".into())
}
