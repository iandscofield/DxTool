//! Repairs. Every run starts with a System Restore point, streams live output, and is
//! logged in the machine's history.

mod actions;
pub mod drivers;
mod runner;

use std::time::Instant;

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

use crate::model::{FixEvent, FixInfo, FixRequest, FixRun, FixStepResult};
use crate::{history, platform};

pub use runner::{powershell, run_streaming};

const fn fix(
    id: &'static str,
    title: &'static str,
    group: &'static str,
    description: &'static str,
    changes: &'static str,
    risk: &'static str,
    needs_admin: bool,
    may_need_restart: bool,
    duration: &'static str,
) -> FixInfo {
    FixInfo { id, title, group, description, changes, risk, needs_admin, may_need_restart, duration }
}

/// Every fix, in the order they run when several are picked.
pub const FIXES: &[FixInfo] = &[
    fix(
        "clear_temp",
        "Clear temporary files",
        "Clean up",
        "Deletes temporary files that programs and Windows leave behind, to free up space. Files that are in use are skipped.",
        "Deletes files in the temporary folders you choose. Optionally empties the Recycle Bin, which permanently deletes what's in it.",
        "low",
        false,
        false,
        "under a minute",
    ),
    fix(
        "dism_repair",
        "Repair the Windows system image (DISM)",
        "Repair Windows",
        "Repairs the component store that Windows uses as the source for fixing its own files. Downloads clean copies from Windows Update if needed, so it works best online.",
        "Replaces damaged Windows component files with clean copies from Microsoft.",
        "low",
        true,
        false,
        "10 to 30 minutes",
    ),
    fix(
        "sfc_repair",
        "Repair Windows system files (SFC)",
        "Repair Windows",
        "System File Checker scans every protected Windows file and replaces damaged ones. Runs after the DISM repair when both are picked.",
        "Replaces damaged protected Windows files with clean copies.",
        "low",
        true,
        true,
        "10 to 20 minutes",
    ),
    fix(
        "chkdsk",
        "Check a drive for file system errors",
        "Drives",
        "Scans a drive's file system while Windows keeps running. If problems are found, a repair is scheduled for the next restart (for the Windows drive) or run straight away (for other drives).",
        "Scanning changes nothing. If errors are found, the repair fixes file system structures; for drives other than C: it briefly disconnects the drive, so close any files open on it first.",
        "medium",
        true,
        true,
        "a few minutes",
    ),
    fix(
        "reset_windows_update",
        "Reset Windows Update",
        "Repair Windows",
        "Fixes updates that keep failing or get stuck by clearing Windows Update's download cache and restarting its services.",
        "Stops the Windows Update, BITS, Cryptographic and Installer services, renames the SoftwareDistribution and catroot2 folders (kept as backups, not deleted), then starts the services again. Update history may look empty afterwards.",
        "medium",
        true,
        false,
        "about a minute",
    ),
    fix(
        "flush_dns",
        "Clear the DNS cache",
        "Network",
        "Clears Windows' memory of website addresses. Helps when a site won't load or loads an old version.",
        "Empties the DNS resolver cache. Nothing else changes.",
        "low",
        true,
        false,
        "a second",
    ),
    fix(
        "network_reset",
        "Reset network settings",
        "Network",
        "Resets Winsock and TCP/IP to factory defaults. Fixes many 'connected but no internet' problems caused by broken network software.",
        "Resets the Winsock catalog and TCP/IP settings. Manually set (static) IP addresses, and some VPN or network filter software, may need setting up again. Needs a restart.",
        "medium",
        true,
        true,
        "a few seconds",
    ),
    fix(
        "update_drivers",
        "Install driver updates",
        "Drivers",
        "Installs the driver updates you pick from Windows Update.",
        "Downloads and installs the selected drivers from Windows Update, accepting the driver makers' licence terms. The old driver stays on the PC and can be rolled back in Device Manager.",
        "medium",
        true,
        true,
        "a few minutes",
    ),
];

pub fn catalog() -> Vec<FixInfo> {
    FIXES.to_vec()
}

/// Shared helper for building a step result.
pub(crate) fn step(id: &str, title: &str) -> FixStepResult {
    FixStepResult { id: id.into(), title: title.into(), status: "ok".into(), ..Default::default() }
}

/// Runs the requested fixes in catalog order, after creating a restore point.
pub fn run_fixes(requests: &[FixRequest], skip_restore_point: bool, on_event: &(dyn Fn(FixEvent) + Sync)) -> FixRun {
    let start = Instant::now();
    let now = chrono::Local::now();
    let machine = crate::machine::collect();
    let mut run = FixRun {
        id: now.format("%Y%m%d-%H%M%S").to_string(),
        machine_id: machine.id.clone(),
        computer_name: machine.computer_name.clone(),
        started_at: now.to_rfc3339(),
        is_admin: platform::is_admin(),
        restore_point_skipped: skip_restore_point,
        app_version: env!("CARGO_PKG_VERSION").into(),
        ..Default::default()
    };
    let mut ordered: Vec<&FixRequest> = requests.iter().filter(|r| FIXES.iter().any(|f| f.id == r.id)).collect();
    ordered.sort_by_key(|r| FIXES.iter().position(|f| f.id == r.id));

    let mut abort = false;
    if !skip_restore_point {
        on_event(FixEvent::StepStarted { id: "restore_point".into(), title: "Create a restore point".into() });
        let names: Vec<&str> = ordered.iter().filter_map(|r| FIXES.iter().find(|f| f.id == r.id)).map(|f| f.title).collect();
        let rp = create_restore_point(&format!("DxTool: before {}", names.join(", ")), on_event);
        abort = rp.status == "failed";
        on_event(FixEvent::StepFinished { result: rp.clone() });
        run.restore_point = Some(rp);
    }

    for r in ordered {
        let info = FIXES.iter().find(|f| f.id == r.id).unwrap();
        if abort {
            let mut s = step(info.id, info.title);
            s.status = "skipped".into();
            s.summary = "Not run because the restore point could not be created.".into();
            on_event(FixEvent::StepFinished { result: s.clone() });
            run.steps.push(s);
            continue;
        }
        on_event(FixEvent::StepStarted { id: info.id.into(), title: info.title.into() });
        let t = Instant::now();
        let mut s = if info.needs_admin && !run.is_admin {
            let mut s = step(info.id, info.title);
            s.status = "skipped".into();
            s.summary = "Needs administrator rights. Restart DxTool as administrator to run this fix.".into();
            s
        } else {
            let emit = |line: &str| on_event(FixEvent::Output { id: info.id.into(), line: line.into() });
            let progress = |p: f64| on_event(FixEvent::Progress { id: info.id.into(), percent: p });
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| actions::run(info, &r.options, &emit, &progress)));
            match outcome {
                Ok(s) => s,
                Err(_) => {
                    let mut s = step(info.id, info.title);
                    s.status = "failed".into();
                    s.summary = "This fix stopped unexpectedly.".into();
                    s
                }
            }
        };
        s.elapsed_ms = t.elapsed().as_millis() as u64;
        run.needs_restart |= s.needs_restart;
        on_event(FixEvent::StepFinished { result: s.clone() });
        run.steps.push(s);
    }
    run.elapsed_ms = start.elapsed().as_millis() as u64;
    let _ = history::save_fix_run(&run);
    run
}

// ───────────────────────────── Restore points ─────────────────────────────

const SR_KEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore";

fn restore_point_count() -> Option<usize> {
    let rows = crate::wmiutil::ns_query("ROOT\\DEFAULT", "SELECT SequenceNumber FROM SystemRestore");
    Some(rows.len())
}

fn create_restore_point(description: &str, on_event: &(dyn Fn(FixEvent) + Sync)) -> FixStepResult {
    let t = Instant::now();
    let mut s = step("restore_point", "Create a restore point");
    if !platform::is_admin() {
        s.status = "failed".into();
        s.summary = "Creating a restore point needs administrator rights. Restart DxTool as administrator.".into();
        return s;
    }
    let emit = |l: &str| on_event(FixEvent::Output { id: "restore_point".into(), line: l.into() });
    emit("Asking Windows to create a restore point. This can take a minute…");

    // Windows normally allows only one restore point per 24 hours and silently skips
    // extra ones. Lift that limit for this one call, then put it back.
    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(SR_KEY, winreg::enums::KEY_READ | winreg::enums::KEY_WRITE).ok();
    let previous: Option<u32> = key.as_ref().and_then(|k| k.get_value("SystemRestorePointCreationFrequency").ok());
    if let Some(k) = &key {
        let _ = k.set_value("SystemRestorePointCreationFrequency", &0u32);
    }
    let before = restore_point_count();
    let desc: String = description.chars().filter(|c| *c != '\'' && *c != '"' && *c != '`').take(240).collect();
    let script = format!(
        "$ErrorActionPreference='Stop'; Checkpoint-Computer -Description '{desc}' -RestorePointType 'MODIFY_SETTINGS'; 'Restore point created.'"
    );
    let result = powershell(&script, std::time::Duration::from_secs(300), &emit);
    if let Some(k) = &key {
        match previous {
            Some(v) => {
                let _ = k.set_value("SystemRestorePointCreationFrequency", &v);
            }
            None => {
                let _ = k.delete_value("SystemRestorePointCreationFrequency");
            }
        }
    }
    let after = restore_point_count();
    s.elapsed_ms = t.elapsed().as_millis() as u64;
    match result {
        Ok((0, lines)) => {
            s.output_tail = tail(&lines);
            if matches!((before, after), (Some(b), Some(a)) if a > b) || before.is_none() {
                s.summary = format!("Restore point created: \"{desc}\". You can undo the fixes with System Restore.");
            } else {
                s.status = "warning".into();
                s.summary = "Windows reported success but no new restore point appeared. An earlier one may be reused.".into();
            }
        }
        Ok((_, lines)) => {
            let text = lines.join(" ").to_lowercase();
            s.output_tail = tail(&lines);
            s.status = "failed".into();
            s.summary = if text.contains("turned off") || text.contains("disabled") || text.contains("is not enabled") || text.contains("service cannot be started") {
                "System Protection (System Restore) is turned off for the Windows drive, so a restore point can't be made.".into()
            } else {
                format!("Windows could not create a restore point: {}", lines.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_default())
            };
        }
        Err(e) => {
            s.status = "failed".into();
            s.summary = e;
        }
    }
    s
}

/// Turns on System Protection for the Windows drive (only when the user asks).
pub fn enable_system_protection() -> Result<(), String> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let script = format!("$ErrorActionPreference='Stop'; Enable-ComputerRestore -Drive '{drive}\\'; 'System Protection turned on.'");
    match powershell(&script, std::time::Duration::from_secs(120), &|_| {})? {
        (0, _) => Ok(()),
        (_, lines) => Err(format!("Could not turn on System Protection: {}", lines.join(" "))),
    }
}

/// Restore points on this PC, newest first: (sequence, description, created).
pub fn restore_points() -> Vec<(u64, String, String)> {
    use crate::wmiutil::RowExt;
    let mut v: Vec<(u64, String, String)> = crate::wmiutil::ns_query("ROOT\\DEFAULT", "SELECT SequenceNumber, Description, CreationTime FROM SystemRestore")
        .iter()
        .map(|r| {
            (
                r.u("SequenceNumber").unwrap_or(0),
                r.s("Description").unwrap_or_default(),
                r.s("CreationTime").and_then(|t| crate::fmt::wmi_datetime(&t)).unwrap_or_default(),
            )
        })
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v
}

/// Opens the Windows System Restore wizard.
pub fn open_system_restore() -> Result<(), String> {
    let exe = format!("{}\\System32\\rstrui.exe", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
    platform::open_path(&exe)
}

/// Restarts the PC after 30 seconds (cancel with `shutdown /a`).
pub fn restart_pc() -> Result<(), String> {
    std::process::Command::new("shutdown.exe")
        .args(["/r", "/t", "30", "/c", "DxTool is restarting this PC to finish repairs."])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn cancel_restart() -> Result<(), String> {
    std::process::Command::new("shutdown.exe").arg("/a").spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// NTFS drives that chkdsk can scan online: (letter, label, size).
pub fn ntfs_drives() -> Vec<(String, String, String)> {
    use crate::wmiutil::RowExt;
    crate::wmiutil::ns_query("ROOT\\CIMV2", "SELECT DeviceID, VolumeName, Size, FileSystem FROM Win32_LogicalDisk WHERE DriveType = 3")
        .iter()
        .filter(|d| d.s("FileSystem").map(|f| f.eq_ignore_ascii_case("NTFS")).unwrap_or(false))
        .map(|d| (d.s("DeviceID").unwrap_or_default(), d.s("VolumeName").unwrap_or_default(), d.f("Size").map(crate::fmt::bytes_dec).unwrap_or_default()))
        .collect()
}

pub(crate) fn tail(lines: &[String]) -> Vec<String> {
    let keep: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    keep[keep.len().saturating_sub(25)..].iter().map(|s| s.to_string()).collect()
}
