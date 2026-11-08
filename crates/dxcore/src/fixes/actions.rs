//! The individual repairs.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::runner::run_streaming;
use super::{step, tail};
use crate::fmt;
use crate::model::{FixInfo, FixStepResult};

type Emit<'a> = &'a dyn Fn(&str);
type Progress<'a> = &'a dyn Fn(f64);

fn win() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())
}

fn sys32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", win())
}

fn opt_bool(o: &Value, k: &str, default: bool) -> bool {
    o.get(k).and_then(Value::as_bool).unwrap_or(default)
}

/// Pulls a percentage like "45.2%" out of a progress line.
fn percent(line: &str) -> Option<f64> {
    let i = line.find('%')?;
    let head = &line[..i];
    let start = head.rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map(|p| p + 1).unwrap_or(0);
    head[start..].parse::<f64>().ok().filter(|p| (0.0..=100.0).contains(p))
}

/// Shows progress lines as a percentage instead of flooding the log with them.
fn streaming<'a>(emit: Emit<'a>, progress: Progress<'a>) -> impl Fn(&str) + 'a {
    move |line: &str| match percent(line) {
        Some(p) if line.len() < 80 => progress(p),
        _ => emit(line),
    }
}

pub fn run(info: &FixInfo, o: &Value, emit: Emit, progress: Progress) -> FixStepResult {
    let mut s = step(info.id, info.title);
    let r = match info.id {
        "clear_temp" => clear_temp(o, emit, &mut s),
        "dism_repair" => dism_repair(emit, progress, &mut s),
        "sfc_repair" => sfc_repair(emit, progress, &mut s),
        "chkdsk" => chkdsk(o, emit, progress, &mut s),
        "reset_windows_update" => reset_windows_update(emit, &mut s),
        "flush_dns" => flush_dns(emit, &mut s),
        "network_reset" => network_reset(emit, &mut s),
        "update_drivers" => super::drivers::install(o, emit, progress, &mut s),
        _ => Err("Unknown fix".into()),
    };
    if let Err(e) = r {
        s.status = "failed".into();
        s.summary = e;
    }
    s
}

// ───────────────────────────── Clean up ─────────────────────────────

/// Deletes everything inside `dir` that isn't in use. Returns (bytes freed, files deleted, files skipped).
fn empty_dir(dir: &Path) -> (u64, u64, u64) {
    let (mut freed, mut deleted, mut skipped) = (0u64, 0u64, 0u64);
    let Ok(rd) = std::fs::read_dir(dir) else { return (0, 0, 0) };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            // Never follow links out of the temp folder.
            if std::fs::remove_file(&p).is_err() {
                let _ = std::fs::remove_dir(&p);
            }
            continue;
        }
        if ft.is_dir() {
            let (f, d, s) = empty_dir(&p);
            freed += f;
            deleted += d;
            skipped += s;
            let _ = std::fs::remove_dir(&p);
        } else {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&p).is_ok() {
                freed += len;
                deleted += 1;
            } else {
                skipped += 1;
            }
        }
    }
    (freed, deleted, skipped)
}

fn clear_temp(o: &Value, emit: Emit, s: &mut FixStepResult) -> Result<(), String> {
    let admin = crate::platform::is_admin();
    let mut targets: Vec<(&str, PathBuf, bool)> = Vec::new();
    if opt_bool(o, "userTemp", true) {
        if let Ok(t) = std::env::var("TEMP") {
            targets.push(("Your temporary files", PathBuf::from(t), false));
        }
    }
    if opt_bool(o, "windowsTemp", true) {
        targets.push(("Windows temporary files", PathBuf::from(format!("{}\\Temp", win())), true));
    }
    if opt_bool(o, "errorReports", true) {
        let pd = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
        targets.push(("Old error reports", PathBuf::from(format!("{pd}\\Microsoft\\Windows\\WER\\ReportArchive")), true));
        targets.push(("Queued error reports", PathBuf::from(format!("{pd}\\Microsoft\\Windows\\WER\\ReportQueue")), true));
    }
    let mut total = 0u64;
    for (label, dir, needs_admin) in targets {
        if needs_admin && !admin {
            s.details.push(format!("{label}: skipped (needs administrator rights)"));
            continue;
        }
        // Safety: only ever clear these exact well-known folders.
        let d = dir.to_string_lossy().to_lowercase();
        if !(d.ends_with("\\temp") || d.contains("\\wer\\report")) {
            s.details.push(format!("{label}: skipped (unexpected folder {d})"));
            continue;
        }
        emit(&format!("Clearing {label} in {}…", dir.display()));
        let (freed, deleted, skipped) = empty_dir(&dir);
        total += freed;
        let line = format!("{label}: freed {} ({deleted} files deleted, {skipped} in use and skipped)", fmt::bytes_dec(freed as f64));
        emit(&line);
        s.details.push(line);
    }
    if opt_bool(o, "recycleBin", false) {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO};
        let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
        let size = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.map(|_| info.i64Size.max(0) as u64).unwrap_or(0);
        emit("Emptying the Recycle Bin…");
        let r = unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) };
        let line = if r.is_ok() || size == 0 {
            total += size;
            format!("Recycle Bin: emptied ({})", fmt::bytes_dec(size as f64))
        } else {
            "Recycle Bin: could not be emptied".to_string()
        };
        emit(&line);
        s.details.push(line);
    }
    s.summary = format!("Freed {}.", fmt::bytes_dec(total as f64));
    Ok(())
}

// ───────────────────────────── Repair Windows ─────────────────────────────

fn dism_repair(emit: Emit, progress: Progress, s: &mut FixStepResult) -> Result<(), String> {
    emit("Running DISM /Online /Cleanup-Image /RestoreHealth. Keep the PC on; this can take a while.");
    let (code, lines) = run_streaming(&sys32("dism.exe"), &["/Online", "/Cleanup-Image", "/RestoreHealth", "/English"], None, Duration::from_secs(90 * 60), &streaming(emit, progress))?;
    s.output_tail = tail(&lines);
    let text = lines.join("\n").to_lowercase();
    if code == 0 && text.contains("completed successfully") {
        s.summary = "The Windows component store is healthy or was repaired.".into();
    } else if text.contains("0x800f081f") || text.contains("source files could not be found") {
        s.status = "failed".into();
        s.summary = "DISM couldn't download the clean files it needs (error 0x800f081f). Connect to the internet and try again, or a technician can repair from Windows installation media.".into();
    } else {
        s.status = "failed".into();
        s.summary = format!("DISM finished with error code {code}. See the output for details.");
    }
    Ok(())
}

fn sfc_repair(emit: Emit, progress: Progress, s: &mut FixStepResult) -> Result<(), String> {
    emit("Running System File Checker (sfc /scannow). Keep the PC on; this can take a while.");
    let (code, lines) = run_streaming(&sys32("sfc.exe"), &["/scannow"], None, Duration::from_secs(90 * 60), &streaming(emit, progress))?;
    s.output_tail = tail(&lines);
    let text = lines.join(" ").to_lowercase();
    if text.contains("did not find any integrity violations") {
        s.summary = "No damaged system files were found.".into();
    } else if text.contains("successfully repaired") {
        s.summary = "Damaged system files were found and repaired. Restart the PC to finish.".into();
        s.needs_restart = true;
    } else if text.contains("unable to fix some") {
        s.status = "warning".into();
        s.summary = "Some damaged files could not be repaired. Run the DISM repair first, then this again. Details are in C:\\Windows\\Logs\\CBS\\CBS.log.".into();
    } else if text.contains("pending") || text.contains("could not perform") {
        s.status = "failed".into();
        s.summary = "System File Checker couldn't run, usually because a restart is pending. Restart and try again.".into();
    } else {
        s.status = "failed".into();
        s.summary = format!("System File Checker finished with code {code}. See the output for details.");
    }
    Ok(())
}

fn reset_windows_update(emit: Emit, s: &mut FixStepResult) -> Result<(), String> {
    let services = ["wuauserv", "bits", "cryptsvc", "msiserver"];
    let net = sys32("net.exe");
    let stop_all = || {
        for svc in services {
            emit(&format!("Stopping {svc}…"));
            let _ = run_streaming(&net, &["stop", svc, "/y"], None, Duration::from_secs(90), &|_| {});
        }
    };
    stop_all();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let folders = [format!("{}\\SoftwareDistribution", win()), format!("{}\\System32\\catroot2", win())];
    let mut ok = true;
    for f in &folders {
        let from = PathBuf::from(f);
        if !from.exists() {
            continue;
        }
        let to = PathBuf::from(format!("{f}.dxtool-{stamp}"));
        let mut renamed = false;
        // Windows Update can restart itself on demand; retry a few times.
        for attempt in 0..4 {
            if std::fs::rename(&from, &to).is_ok() {
                renamed = true;
                break;
            }
            if attempt < 3 {
                std::thread::sleep(Duration::from_secs(2));
                stop_all();
            }
        }
        let line = if renamed { format!("Renamed {f} → {}", to.display()) } else { format!("Could not rename {f} (still in use)") };
        ok &= renamed;
        emit(&line);
        s.details.push(line);
    }
    for svc in ["cryptsvc", "bits", "wuauserv", "msiserver"] {
        emit(&format!("Starting {svc}…"));
        let _ = run_streaming(&net, &["start", svc], None, Duration::from_secs(90), &|_| {});
    }
    if ok {
        s.summary = "Windows Update was reset. Open Settings → Windows Update and check for updates. The old folders were kept with '.dxtool-' in their names and can be deleted later.".into();
    } else {
        s.status = "warning".into();
        s.summary = "Windows Update's services were restarted, but a folder was in use and couldn't be reset. Restart the PC and run this fix again.".into();
    }
    Ok(())
}

// ───────────────────────────── Drives ─────────────────────────────

fn chkdsk(o: &Value, emit: Emit, progress: Progress, s: &mut FixStepResult) -> Result<(), String> {
    let drive = o.get("drive").and_then(Value::as_str).unwrap_or("C:").trim().to_uppercase();
    let valid = drive.len() == 2 && drive.ends_with(':') && drive.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    if !valid {
        return Err(format!("'{drive}' is not a drive letter."));
    }
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let exe = sys32("chkdsk.exe");
    emit(&format!("Scanning {drive} for file system errors (online scan, nothing is changed)…"));
    let (code, lines) = run_streaming(&exe, &[&drive, "/scan"], None, Duration::from_secs(60 * 60), &streaming(emit, progress))?;
    s.output_tail = tail(&lines);
    let text = lines.join(" ").to_lowercase();
    if text.contains("found no problems") || (code == 0 && !text.contains("found problems") && !text.contains("corrupt")) {
        s.summary = format!("{drive} has no file system errors.");
        return Ok(());
    }
    if text.contains("cannot open volume") || text.contains("is not available for raw drives") || text.contains("not supported") {
        s.status = "failed".into();
        s.summary = format!("{drive} can't be checked with an online scan (it may not be an NTFS drive).");
        return Ok(());
    }
    s.details.push(format!("Scan of {drive} found problems."));
    if drive == system {
        emit("Problems found. Scheduling a repair of the Windows drive for the next restart…");
        let (_, l2) = run_streaming(&exe, &[&drive, "/spotfix"], Some("Y\r\n"), Duration::from_secs(120), emit)?;
        s.output_tail = tail(&[lines, l2].concat());
        s.status = "warning".into();
        s.needs_restart = true;
        s.summary = format!("{drive} has file system errors. A repair will run the next time the PC restarts (it may take a while during startup).");
    } else {
        emit(&format!("Problems found. Repairing {drive} now (it will be briefly disconnected)…"));
        let (c2, l2) = run_streaming(&exe, &[&drive, "/spotfix"], Some("Y\r\n"), Duration::from_secs(60 * 60), &streaming(emit, progress))?;
        s.output_tail = tail(&[lines, l2.clone()].concat());
        if c2 == 0 {
            s.summary = format!("File system errors on {drive} were repaired.");
        } else {
            s.status = "warning".into();
            s.summary = format!("chkdsk tried to repair {drive} (code {c2}). Check the output, and back up the drive if errors remain.");
        }
    }
    Ok(())
}

// ───────────────────────────── Network ─────────────────────────────

fn flush_dns(emit: Emit, s: &mut FixStepResult) -> Result<(), String> {
    let (code, lines) = run_streaming(&sys32("ipconfig.exe"), &["/flushdns"], None, Duration::from_secs(30), emit)?;
    s.output_tail = tail(&lines);
    if code == 0 {
        s.summary = "The DNS cache was cleared.".into();
    } else {
        s.status = "failed".into();
        s.summary = format!("ipconfig returned code {code}.");
    }
    Ok(())
}

fn network_reset(emit: Emit, s: &mut FixStepResult) -> Result<(), String> {
    let netsh = sys32("netsh.exe");
    let mut all = Vec::new();
    let mut failed = Vec::new();
    for (label, args) in [("Winsock reset", vec!["winsock", "reset"]), ("TCP/IP reset", vec!["int", "ip", "reset"]), ("IPv6 reset", vec!["int", "ipv6", "reset"])] {
        emit(&format!("{label}…"));
        let (code, lines) = run_streaming(&netsh, &args, None, Duration::from_secs(60), emit)?;
        all.extend(lines);
        if code != 0 {
            failed.push(label);
        }
    }
    let _ = flush_dns(emit, &mut step("flush_dns", ""));
    s.output_tail = tail(&all);
    s.needs_restart = true;
    if failed.is_empty() {
        s.summary = "Network settings were reset. Restart the PC to finish.".into();
    } else {
        s.status = "warning".into();
        s.summary = format!("Network settings were reset, but {} reported errors (often harmless). Restart the PC to finish.", failed.join(" and "));
    }
    Ok(())
}
