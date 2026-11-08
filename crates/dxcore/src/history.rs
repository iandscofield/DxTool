//! Saved scans, per machine, in the DxTool data folder.
//!
//! Layout (portable: the folder sits next to DxTool.exe when possible):
//!
//! ```text
//! DxTool-Data/
//!   settings.json
//!   machines/<machine id>/machine.json   MachineRecord: identity + scan summaries
//!   machines/<machine id>/scans/<scan id>.json   full ScanReport
//!   reports/<machine id>/<file>.html|pdf
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::{MachineRecord, ScanReport, ScanSummary};

/// Next to the program if that folder is writable (portable use), otherwise %LOCALAPPDATA%\DxTool.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("DxTool-Data"))) {
        if fs::create_dir_all(&dir).is_ok() {
            let probe = dir.join(".write-test");
            if fs::write(&probe, b"ok").is_ok() {
                let _ = fs::remove_file(probe);
                return dir;
            }
        }
    }
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("DxTool");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn machines_dir() -> PathBuf {
    data_dir().join("machines")
}

fn safe(id: &str) -> Result<&str, String> {
    if id.is_empty() || id.contains(['/', '\\', ':', '.']) {
        Err("Invalid id".into())
    } else {
        Ok(id)
    }
}

fn read_record(dir: &Path) -> Option<MachineRecord> {
    serde_json::from_slice(&fs::read(dir.join("machine.json")).ok()?).ok()
}

fn write_record(dir: &Path, rec: &MachineRecord) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(rec).map_err(|e| e.to_string())?;
    fs::write(dir.join("machine.json"), json).map_err(|e| format!("Could not save history: {e}"))
}

pub fn summary_of(r: &ScanReport) -> ScanSummary {
    ScanSummary {
        id: r.id.clone(),
        kind: r.kind.clone(),
        started_at: r.started_at.clone(),
        elapsed_ms: r.elapsed_ms,
        score: r.score,
        grade: r.grade.clone(),
        counts: r.counts,
        is_admin: r.is_admin,
        checks: r.results.len(),
    }
}

/// Saves a finished scan and updates the machine's history.
pub fn save(report: &ScanReport) -> Result<PathBuf, String> {
    let dir = machines_dir().join(safe(&report.machine.id)?);
    fs::create_dir_all(dir.join("scans")).map_err(|e| format!("Could not create the history folder: {e}"))?;
    let path = dir.join("scans").join(format!("{}.json", safe(&report.id)?));
    fs::write(&path, serde_json::to_vec(report).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save the scan: {e}"))?;

    let mut rec = read_record(&dir).unwrap_or_else(|| MachineRecord { first_seen: report.started_at.clone(), ..Default::default() });
    rec.info = report.machine.clone();
    rec.last_seen = report.started_at.clone();
    rec.scans.retain(|s| s.id != report.id);
    rec.scans.push(summary_of(report));
    rec.scans.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    write_record(&dir, &rec)?;
    Ok(path)
}

/// Every PC with saved scans, most recently scanned first.
pub fn machines() -> Vec<MachineRecord> {
    let mut out: Vec<MachineRecord> = fs::read_dir(machines_dir())
        .map(|rd| rd.flatten().filter_map(|e| read_record(&e.path())).collect())
        .unwrap_or_default();
    out.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    out
}

/// A saved scan, as raw JSON (it is passed straight to the interface).
pub fn load(machine_id: &str, scan_id: &str) -> Result<Value, String> {
    let path = machines_dir().join(safe(machine_id)?).join("scans").join(format!("{}.json", safe(scan_id)?));
    let bytes = fs::read(&path).map_err(|e| format!("Could not open that scan: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("That scan file is damaged: {e}"))
}

pub fn delete(machine_id: &str, scan_id: &str) -> Result<(), String> {
    let dir = machines_dir().join(safe(machine_id)?);
    let _ = fs::remove_file(dir.join("scans").join(format!("{}.json", safe(scan_id)?)));
    if let Some(mut rec) = read_record(&dir) {
        rec.scans.retain(|s| s.id != scan_id);
        if rec.scans.is_empty() {
            let _ = fs::remove_dir_all(&dir);
        } else {
            write_record(&dir, &rec)?;
        }
    }
    Ok(())
}

pub fn load_settings() -> Value {
    fs::read(data_dir().join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Object(Default::default()))
}

pub fn save_settings(v: &Value) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    fs::write(data_dir().join("settings.json"), json).map_err(|e| format!("Could not save settings: {e}"))
}

/// Where reports for a machine are saved.
pub fn reports_dir(machine_id: &str) -> Result<PathBuf, String> {
    let dir = data_dir().join("reports").join(safe(machine_id)?);
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create the reports folder: {e}"))?;
    Ok(dir)
}

/// Saves a fix run in the machine's history.
pub fn save_fix_run(run: &crate::model::FixRun) -> Result<PathBuf, String> {
    let dir = machines_dir().join(safe(&run.machine_id)?).join("fixes");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create the fix log folder: {e}"))?;
    let path = dir.join(format!("{}.json", safe(&run.id)?));
    fs::write(&path, serde_json::to_vec_pretty(run).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save the fix log: {e}"))?;
    Ok(path)
}

/// Every fix run for a machine, newest first.
pub fn fix_runs(machine_id: &str) -> Vec<crate::model::FixRun> {
    let Ok(id) = safe(machine_id) else { return vec![] };
    let mut v: Vec<crate::model::FixRun> = fs::read_dir(machines_dir().join(id).join("fixes"))
        .map(|rd| rd.flatten().filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok()).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    v
}
