//! Health checks and scans.
//!
//! Each check inspects one area of the PC and turns what it finds into plain-English
//! findings with a severity. A scan runs a set of checks in parallel and combines the
//! findings into a 0-100 health score.

mod bugcheck;
mod network;
mod security;
mod stability;
mod storage;
mod system;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::events::Event;
use crate::model::{CheckInfo, CheckResult, Counts, Field, Item, ScanEvent, ScanReport, Status};
use crate::platform;

/// Shared settings for one scan.
pub struct Ctx {
    pub is_admin: bool,
    /// How far back to look in the event logs.
    pub days: u32,
    pub full: bool,
}

type CheckFn = fn(&Ctx, &mut CheckResult) -> Result<(), String>;

const fn info(
    id: &'static str,
    title: &'static str,
    group: &'static str,
    description: &'static str,
    fast: bool,
    needs_admin: bool,
    duration: &'static str,
) -> CheckInfo {
    CheckInfo { id, title, group, description, fast, custom_only: false, needs_admin, duration }
}

const CHECKS: &[(CheckInfo, CheckFn)] = &[
    // Storage
    (info("drive_health", "Drive health", "Storage", "Wear level, temperature, power-on hours and uncorrectable errors reported by each drive", true, true, "a few seconds"), storage::drive_health),
    (info("disk_space", "Free space", "Storage", "Free space on every drive, plus temporary files and the Recycle Bin", true, false, "a few seconds"), storage::disk_space),
    (info("storage_errors", "Drive and file system errors", "Storage", "Bad blocks, controller errors and file system corruption logged by Windows", true, false, "a few seconds"), storage::storage_errors),
    // Stability
    (info("crashes", "Blue screens and unexpected shutdowns", "Stability", "Blue screen crashes with their stop codes explained, crash dump files, and times the PC lost power or was forced off", true, false, "a few seconds"), stability::crashes),
    (info("hardware_errors", "Hardware errors", "Stability", "Processor, memory and PCIe errors reported by the hardware, graphics driver resets, and processor throttling", true, false, "a few seconds"), stability::hardware_errors),
    (info("device_problems", "Device problems", "Stability", "Devices that Device Manager reports as not working, missing drivers or disabled", true, false, "a few seconds"), stability::device_problems),
    (info("drivers", "Drivers", "Stability", "Unsigned drivers, the age of the graphics driver, and every third-party driver installed", false, false, "a few seconds"), stability::drivers),
    (info("service_failures", "Service failures", "Stability", "Windows services that failed to start or crashed", false, false, "a few seconds"), stability::service_failures),
    (info("app_crashes", "App crashes", "Stability", "Programs that crashed or stopped responding, and the files involved", false, false, "a few seconds"), stability::app_crashes),
    // Windows
    (info("windows_update", "Windows Update", "Windows", "When updates were last installed, failed updates, and whether a restart is pending", true, false, "a few seconds"), system::windows_update),
    (info("system_files", "Windows system image", "Windows", "Asks Windows whether its component store (the source used to repair system files) is damaged", false, true, "about 10 seconds"), system::system_files),
    (info("startup", "Startup programs and boot time", "Windows", "Programs that start with Windows and how long startup takes", true, false, "a few seconds"), system::startup),
    (info("resources", "Memory and performance", "Windows", "Memory use, uptime, processor load, page file and the programs using the most memory", true, false, "a few seconds"), system::resources),
    (info("battery_health", "Battery", "Windows", "Battery wear compared with its original capacity (laptops)", true, false, "a second"), system::battery_health),
    (info("activation", "Windows activation", "Windows", "Whether Windows is activated and which licence it uses", false, false, "a few seconds"), system::activation),
    // Security
    (info("antivirus", "Antivirus", "Security", "Antivirus status, virus definition age, last scan and recent threat detections", true, false, "a few seconds"), security::antivirus),
    (info("firewall", "Firewall", "Security", "Whether the firewall is on for each network type", true, false, "a second"), security::firewall),
    (info("device_security", "Device security", "Security", "Secure Boot, TPM, BitLocker, User Account Control, memory integrity, Remote Desktop and SMB1", true, false, "a few seconds"), security::device_security),
    // Network
    (info("network", "Network and internet", "Network", "Internet connectivity as Windows sees it, addresses, link speed, Wi-Fi signal, proxy and hosts file", true, false, "a few seconds"), network::network),
    // Custom only
    (CheckInfo {
        id: "sfc_verify",
        title: "Deep system file check",
        group: "Windows",
        description: "Runs System File Checker in verify-only mode to find damaged Windows files (changes nothing)",
        fast: false,
        custom_only: true,
        needs_admin: true,
        duration: "5 to 15 minutes",
    }, system::sfc_verify),
];

/// Every available check, for the Custom scan picker.
pub fn catalog() -> Vec<CheckInfo> {
    CHECKS.iter().map(|(i, _)| i.clone()).collect()
}

/// Which checks a scan of this kind runs.
pub fn checks_for(kind: &str, selected: &[String]) -> Vec<&'static str> {
    CHECKS
        .iter()
        .filter(|(i, _)| match kind {
            "fast" => i.fast,
            "full" => !i.custom_only,
            _ => selected.iter().any(|s| s == i.id),
        })
        .map(|(i, _)| i.id)
        .collect()
}

/// Runs a scan. `on_event` receives live progress and must be thread-safe.
pub fn run_scan(kind: &str, selected: &[String], on_event: &(dyn Fn(ScanEvent) + Sync)) -> ScanReport {
    let start = Instant::now();
    let started_at = chrono::Local::now();
    let ctx = Ctx { is_admin: platform::is_admin(), days: if kind == "fast" { 30 } else { 90 }, full: kind != "fast" };
    let ids = checks_for(kind, selected);
    on_event(ScanEvent::Started { checks: ids.iter().map(|s| s.to_string()).collect() });

    let mut machine = crate::model::MachineInfo::default();
    let mut results: Vec<CheckResult> = std::thread::scope(|s| {
        let m = s.spawn(crate::machine::collect);
        let handles: Vec<_> = ids
            .iter()
            .filter_map(|id| CHECKS.iter().find(|(i, _)| i.id == *id))
            .map(|(meta, f)| {
                let ctx = &ctx;
                s.spawn(move || {
                    on_event(ScanEvent::CheckStarted { id: meta.id.to_string() });
                    let r = run_one(meta, *f, ctx);
                    on_event(ScanEvent::CheckFinished {
                        id: r.id.clone(),
                        status: r.status,
                        summary: r.summary.clone(),
                        elapsed_ms: r.elapsed_ms,
                    });
                    r
                })
            })
            .collect();
        let out = handles.into_iter().filter_map(|h| h.join().ok()).collect();
        machine = m.join().unwrap_or_default();
        out
    });
    let order = |id: &str| CHECKS.iter().position(|(i, _)| i.id == id).unwrap_or(usize::MAX);
    results.sort_by_key(|r| order(&r.id));

    let (score, counts) = score(&results);
    ScanReport {
        id: started_at.format("%Y%m%d-%H%M%S").to_string(),
        kind: kind.to_string(),
        started_at: started_at.to_rfc3339(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        computer_name: platform::computer_name(),
        is_admin: ctx.is_admin,
        score,
        grade: grade(score).to_string(),
        counts,
        results,
        machine,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        saved: false,
    }
}

fn run_one(meta: &CheckInfo, f: CheckFn, ctx: &Ctx) -> CheckResult {
    let t = Instant::now();
    let mut r = CheckResult { id: meta.id.into(), title: meta.title.into(), group: meta.group.into(), ..Default::default() };
    // A panic in one check must never take down the whole scan.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ctx, &mut r)));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => r.error = Some(e),
        Err(_) => r.error = Some("This check stopped unexpectedly.".into()),
    }
    r.findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    r.status = r.findings.iter().map(|f| f.severity).max().unwrap_or(Status::Ok);
    if r.error.is_some() && r.status == Status::Ok {
        r.status = Status::Info;
        if r.summary.is_empty() {
            r.summary = "Could not complete this check".into();
        }
    }
    r.elapsed_ms = t.elapsed().as_millis() as u64;
    r
}

/// 100 minus penalties: 18 per problem, 6 per warning, capped at 30 per check.
fn score(results: &[CheckResult]) -> (u32, Counts) {
    let mut c = Counts::default();
    let mut penalty = 0u32;
    for r in results {
        let mut p = 0u32;
        for f in &r.findings {
            match f.severity {
                Status::Error => {
                    c.error += 1;
                    p += 18;
                }
                Status::Warn => {
                    c.warn += 1;
                    p += 6;
                }
                Status::Info => c.info += 1,
                Status::Ok => {}
            }
        }
        penalty += p.min(30);
    }
    (100u32.saturating_sub(penalty), c)
}

pub fn grade(score: u32) -> &'static str {
    match score {
        90.. => "Excellent",
        75..=89 => "Good",
        50..=74 => "Fair",
        _ => "Poor",
    }
}

// ───────────────────────────── Shared helpers ─────────────────────────────

pub(crate) fn item(id: impl Into<String>, name: impl Into<String>, subtitle: impl Into<String>, summary: Vec<Field>) -> Item {
    Item { id: id.into(), name: name.into(), subtitle: subtitle.into(), summary, ..Default::default() }
}

pub(crate) fn field(label: &str, display: impl Into<String>) -> Field {
    let d: String = display.into();
    Field { key: label.into(), label: label.into(), sort: json!(d.to_lowercase()), display: d }
}

pub(crate) fn num_field(label: &str, display: impl Into<String>, sort: f64) -> Field {
    Field { key: label.into(), label: label.into(), display: display.into(), sort: json!(sort) }
}

/// A date/time column that sorts chronologically.
pub(crate) fn time_field(label: &str, ev: &Event) -> Field {
    Field { key: label.into(), label: label.into(), display: ev.local_time(), sort: json!(ev.timestamp()) }
}

pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Turns a list of events into table rows with the full message as a detail section.
pub(crate) fn event_item(ev: &Event, idx: usize, kind: &str, extra: Vec<Field>) -> Item {
    let mut summary = vec![time_field("When", ev)];
    summary.extend(extra);
    summary.push(field("Source", ev.provider.clone()));
    summary.push(num_field("Event ID", ev.id.to_string(), ev.id as f64));
    summary.push(field("Level", ev.level_text()));
    summary.push(field("Message", ev.short_message()));
    let mut it = item(format!("ev{}-{idx}", ev.record_id), kind, ev.local_time(), summary);
    it.status = match ev.level {
        1 | 2 => Status::Error,
        3 => Status::Warn,
        _ => Status::Info,
    };
    let mut fields = vec![field("Log", ev.channel.clone()), field("Record", ev.record_id.to_string()), field("Full message", ev.message.clone())];
    for (k, v) in &ev.data {
        if !v.trim().is_empty() {
            fields.push(Field { key: format!("data.{k}"), label: k.clone(), display: v.clone(), sort: json!(v.to_lowercase()) });
        }
    }
    it.sections.push(crate::model::Section { title: "Event details".into(), raw: false, fields });
    it
}

/// Runs a console program without showing a window and returns (exit code, output).
pub fn run_hidden(exe: &str, args: &[&str], timeout: Duration) -> Result<(i32, String), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Could not start {exe}: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let t = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code().unwrap_or(-1),
            Ok(None) if t.elapsed() > timeout => {
                let _ = child.kill();
                return Err(format!("{exe} took too long and was stopped"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(150)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let bytes = reader.join().unwrap_or_default();
    Ok((code, decode_console(&bytes)))
}

/// Console tools write either UTF-16 (sfc) or the OEM code page (most others).
fn decode_console(b: &[u8]) -> String {
    let zeros = b.iter().skip(1).step_by(2).take(400).filter(|c| **c == 0).count();
    let sample = b.len().min(800) / 2;
    if sample > 0 && zeros * 10 > sample * 6 {
        let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u).replace('\0', "")
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}
