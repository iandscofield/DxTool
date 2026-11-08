//! Virus scanning: Microsoft Defender (built into Windows) and ClamAV (downloaded on
//! request as a second opinion), plus DxTool's own quarantine for ClamAV detections.
//!
//! Data layout inside the DxTool data folder:
//!
//! ```text
//! clamav/bin/       clamscan, freshclam and their DLLs
//! clamav/db/        virus databases (main, daily, bytecode)
//! quarantine/       <id>.dxq scrambled copies + index.json
//! machines/<id>/virus/<scan id>.json   VirusScan
//! ```

pub mod clamav;
pub mod defender;
mod walk;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::{AvEvent, AvStatus, QuarantineItem, VirusScan};
use crate::{history, platform};

/// Set to stop the running scan or download.
pub(crate) static CANCEL: AtomicBool = AtomicBool::new(false);
static BUSY: AtomicBool = AtomicBool::new(false);

/// Held while a scan or download runs; only one may run at a time.
pub(crate) struct BusyGuard;

impl BusyGuard {
    pub(crate) fn acquire() -> Result<Self, String> {
        if BUSY.swap(true, Ordering::SeqCst) {
            return Err("Another virus scan or download is already running.".into());
        }
        CANCEL.store(false, Ordering::SeqCst);
        Ok(BusyGuard)
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// Asks the running scan or download to stop.
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub(crate) fn cancelled() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

pub fn status() -> AvStatus {
    let (defender, clamav) = std::thread::scope(|s| {
        let d = s.spawn(defender::status);
        let c = s.spawn(clamav::status);
        (d.join().unwrap_or_default(), c.join().unwrap_or_default())
    });
    AvStatus { defender, clamav, is_admin: platform::is_admin() }
}

/// Runs a scan with the chosen engine and saves it in the machine's history.
pub fn scan(engine: &str, kind: &str, paths: &[String], on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<VirusScan, String> {
    let _busy = BusyGuard::acquire()?;
    let now = chrono::Local::now();
    let machine = crate::machine::collect();
    let mut scan = VirusScan {
        id: format!("{}-{}", now.format("%Y%m%d-%H%M%S"), engine),
        machine_id: machine.id.clone(),
        computer_name: machine.computer_name.clone(),
        engine: engine.into(),
        kind: kind.into(),
        started_at: now.to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        ..Default::default()
    };
    if kind == "custom" && paths.is_empty() {
        return Err("Choose at least one file, folder or drive to scan.".into());
    }
    let t = std::time::Instant::now();
    match engine {
        "defender" => defender::scan(&mut scan, paths, on_event),
        "clamav" => clamav::scan(&mut scan, paths, on_event),
        _ => return Err(format!("Unknown scanner: {engine}")),
    }
    scan.elapsed_ms = t.elapsed().as_millis() as u64;
    if scan.outcome.is_empty() {
        scan.outcome = if scan.threats.is_empty() { "clean" } else { "threats" }.into();
    }
    let _ = save_scan(&scan);
    Ok(scan)
}

// ───────────────────────── History ─────────────────────────

fn scans_dir(machine_id: &str) -> Result<PathBuf, String> {
    if machine_id.is_empty() || machine_id.contains(['/', '\\', ':', '.']) {
        return Err("Invalid id".into());
    }
    Ok(history::data_dir().join("machines").join(machine_id).join("virus"))
}

pub fn save_scan(scan: &VirusScan) -> Result<PathBuf, String> {
    let dir = scans_dir(&scan.machine_id)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create the virus scan folder: {e}"))?;
    let path = dir.join(format!("{}.json", scan.id.replace(['/', '\\', ':', '.'], "_")));
    fs::write(&path, serde_json::to_vec_pretty(scan).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save the virus scan: {e}"))?;
    Ok(path)
}

/// Every virus scan for a machine, newest first.
pub fn scans(machine_id: &str) -> Vec<VirusScan> {
    let Ok(dir) = scans_dir(machine_id) else { return vec![] };
    let mut v: Vec<VirusScan> = fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok()).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    v
}

pub fn delete_scan(machine_id: &str, scan_id: &str) -> Result<(), String> {
    let p = scans_dir(machine_id)?.join(format!("{}.json", scan_id.replace(['/', '\\', ':', '.'], "_")));
    fs::remove_file(p).map_err(|e| format!("Could not delete the virus scan: {e}"))
}

// ───────────────────────── Acting on detections ─────────────────────────

fn quarantine_dir() -> PathBuf {
    history::data_dir().join("quarantine")
}

fn read_index() -> Vec<QuarantineItem> {
    fs::read(quarantine_dir().join("index.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write_index(items: &[QuarantineItem]) -> Result<(), String> {
    let dir = quarantine_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("index.json"), serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?).map_err(|e| format!("Could not update the quarantine list: {e}"))
}

pub fn quarantine_list() -> Vec<QuarantineItem> {
    let mut v = read_index();
    v.sort_by(|a, b| b.quarantined_at.cmp(&a.quarantined_at));
    v
}

const MAGIC: &[u8] = b"DXQ1";
const KEY: u8 = 0x5A;

/// Scrambles the bytes so the quarantined copy can't run or be picked up again by antivirus.
fn scramble(data: &[u8]) -> Vec<u8> {
    data.iter().map(|b| b ^ KEY).collect()
}

fn friendly_io(e: &std::io::Error, what: &str) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("{what}: access denied. Restart DxTool as administrator and try again."),
        std::io::ErrorKind::NotFound => format!("{what}: the file is no longer there."),
        _ if e.raw_os_error() == Some(225) => format!("{what}: Windows is blocking it because Microsoft Defender detected it too. Use \"Remove with Defender\" instead."),
        _ if e.raw_os_error() == Some(32) => format!("{what}: the file is in use. Close the program using it, or restart the PC, and try again."),
        _ => format!("{what}: {e}"),
    }
}

/// Changes what happened to a threat in a saved scan, after the user acted on it.
fn mark_threat(machine_id: &str, scan_id: &str, path: &str, status: &str, action: &str) -> Result<(), String> {
    let mut list = scans(machine_id);
    let scan = list.iter_mut().find(|s| s.id == scan_id).ok_or("That virus scan is no longer in the history.")?;
    for t in scan.threats.iter_mut().filter(|t| t.path.eq_ignore_ascii_case(path)) {
        t.status = status.into();
        t.action = action.into();
    }
    save_scan(scan).map(|_| ())
}

/// Only files listed as active threats in a saved scan can be acted on.
fn check_threat(machine_id: &str, scan_id: &str, path: &str) -> Result<String, String> {
    let scan = scans(machine_id).into_iter().find(|s| s.id == scan_id).ok_or("That virus scan is no longer in the history.")?;
    let t = scan
        .threats
        .iter()
        .find(|t| t.path.eq_ignore_ascii_case(path))
        .ok_or("That file isn't a detection in this scan.")?;
    if t.status != "active" {
        return Err("That detection has already been dealt with.".into());
    }
    Ok(t.name.clone())
}

/// Moves a detected file into DxTool's quarantine (it can be restored later).
pub fn quarantine(machine_id: &str, scan_id: &str, path: &str) -> Result<QuarantineItem, String> {
    let threat = check_threat(machine_id, scan_id, path)?;
    let data = fs::read(path).map_err(|e| friendly_io(&e, "Could not read the file"))?;
    let now = chrono::Local::now();
    let item = QuarantineItem {
        id: format!("{}-{:08x}", now.format("%Y%m%d%H%M%S"), (now.timestamp_subsec_nanos() ^ data.len() as u32)),
        original_path: path.into(),
        threat,
        engine: scans(machine_id).into_iter().find(|s| s.id == scan_id).map(|s| s.engine).unwrap_or_default(),
        quarantined_at: now.to_rfc3339(),
        size_bytes: data.len() as u64,
    };
    let dir = quarantine_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let q = dir.join(format!("{}.dxq", item.id));
    let mut out = MAGIC.to_vec();
    out.extend(scramble(&data));
    fs::write(&q, out).map_err(|e| format!("Could not write to the quarantine folder: {e}"))?;
    if let Err(e) = fs::remove_file(path) {
        let _ = fs::remove_file(&q);
        return Err(friendly_io(&e, "Could not move the file"));
    }
    let mut idx = read_index();
    idx.push(item.clone());
    write_index(&idx)?;
    let _ = mark_threat(machine_id, scan_id, path, "quarantined", "Moved to DxTool's quarantine");
    Ok(item)
}

/// Permanently deletes a detected file.
pub fn delete_threat(machine_id: &str, scan_id: &str, path: &str) -> Result<(), String> {
    check_threat(machine_id, scan_id, path)?;
    fs::remove_file(path).map_err(|e| friendly_io(&e, "Could not delete the file"))?;
    let _ = mark_threat(machine_id, scan_id, path, "removed", "Deleted permanently");
    Ok(())
}

/// Puts a quarantined file back where it was.
pub fn restore(id: &str) -> Result<String, String> {
    let mut idx = read_index();
    let pos = idx.iter().position(|i| i.id == id).ok_or("That item is no longer in quarantine.")?;
    let item = idx[pos].clone();
    let q = quarantine_dir().join(format!("{}.dxq", item.id));
    let raw = fs::read(&q).map_err(|e| format!("Could not read the quarantined copy: {e}"))?;
    if !raw.starts_with(MAGIC) {
        return Err("The quarantined copy is damaged.".into());
    }
    let target = PathBuf::from(&item.original_path);
    if target.exists() {
        return Err(format!("A file already exists at {}. Move it first.", item.original_path));
    }
    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&target, scramble(&raw[MAGIC.len()..])).map_err(|e| friendly_io(&e, "Could not restore the file"))?;
    let _ = fs::remove_file(&q);
    idx.remove(pos);
    write_index(&idx)?;
    Ok(item.original_path)
}

/// Deletes a quarantined file for good.
pub fn delete_quarantined(id: &str) -> Result<(), String> {
    let mut idx = read_index();
    let pos = idx.iter().position(|i| i.id == id).ok_or("That item is no longer in quarantine.")?;
    let q = quarantine_dir().join(format!("{}.dxq", idx[pos].id));
    if q.exists() {
        fs::remove_file(&q).map_err(|e| format!("Could not delete the quarantined copy: {e}"))?;
    }
    idx.remove(pos);
    write_index(&idx)
}

/// Opens Windows Security's protection history, where Defender's quarantine lives.
pub fn open_windows_security() -> Result<(), String> {
    platform::open_path("windowsdefender://threathistory")
}
