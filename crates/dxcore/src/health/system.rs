//! Windows checks: updates, system files, startup, resources, battery and activation.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{Local, NaiveDate};
use serde_json::json;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use super::{event_item, field, item, num_field, plural, run_hidden, Ctx};
use crate::events;
use crate::fmt;
use crate::model::{CheckResult, Field, Finding, Status};
use crate::wmiutil::{ns_query, RowExt};

const CIMV2: &str = "ROOT\\CIMV2";

fn hklm_exists(path: &str) -> bool {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path).is_ok()
}

fn sys32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
}

pub fn windows_update(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    // Pending restart
    let mut pending = Vec::new();
    if hklm_exists("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending") {
        pending.push("Windows components");
    }
    if hklm_exists("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired") {
        pending.push("Windows Update");
    }
    let pfro = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager")
        .ok()
        .and_then(|k| k.get_value::<Vec<String>, _>("PendingFileRenameOperations").ok())
        .map(|v| v.iter().any(|s| !s.trim().is_empty()))
        .unwrap_or(false);
    r.facts.push(field("Restart pending", if pending.is_empty() { if pfro { "Only for file changes" } else { "No" }.to_string() } else { format!("Yes ({})", pending.join(", ")) }));
    if !pending.is_empty() {
        r.findings.push(
            Finding::new("reboot_pending", Status::Warn, "A restart is needed to finish installing updates", "Updates have been downloaded or partly installed but only finish after a restart. Until then the PC may be missing fixes, and more updates can pile up.")
                .advice("Save your work and restart the PC (use Restart, not Shut down)."),
        );
    }

    // Update history from the event log
    let q = events::system_filter(&[
        events::providers(&["Microsoft-Windows-WindowsUpdateClient"]),
        events::ids(&[19, 20]),
        events::within_days(ctx.days.max(90)),
    ]);
    let evs = events::query("System", &q, 300).unwrap_or_default();
    let last_ok = evs.iter().find(|e| e.id == 19);
    let mut fails: BTreeMap<String, Vec<&events::Event>> = BTreeMap::new();
    let mut store_fails = 0;
    for e in evs.iter().filter(|e| e.id == 20) {
        // Microsoft Store app updates ("9WZDNCRFJBMP-...") retry by themselves; they are not Windows Update problems.
        if is_store_app(e.data("updateTitle").unwrap_or("")) {
            store_fails += 1;
            continue;
        }
        if e.time.map(|t| (chrono::Utc::now() - t).num_days() <= 30).unwrap_or(false) {
            fails.entry(e.data("updateTitle").unwrap_or("Unknown update").to_string()).or_default().push(e);
        }
    }

    // Installed hotfixes give a second opinion on the last install date.
    let qfe = ns_query(CIMV2, "SELECT HotFixID, Description, InstalledOn FROM Win32_QuickFixEngineering");
    let qfe_last = qfe
        .iter()
        .filter_map(|q| q.s("InstalledOn"))
        .filter_map(|s| NaiveDate::parse_from_str(&s, "%m/%d/%Y").ok())
        .max();
    let ev_last = last_ok.and_then(|e| e.time).map(|t| t.with_timezone(&Local).date_naive());
    let last = match (ev_last, qfe_last) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    if let Some(d) = last {
        let age = (Local::now().date_naive() - d).num_days();
        r.facts.push(Field { key: "Last update installed".into(), label: "Last update installed".into(), display: format!("{} ({})", d.format("%Y-%m-%d"), if age == 0 { "today".into() } else { format!("{age} days ago") }), sort: json!(age) });
        if age > 60 {
            r.findings.push(
                Finding::new("updates_old", if age > 120 { Status::Error } else { Status::Warn }, format!("No updates installed in {age} days"), "Windows normally installs security updates every month. Missing them leaves the PC open to known attacks.")
                    .advice("Open Settings → Windows Update and click 'Check for updates'. If updates keep failing, the Windows Update repair in Fixes can help.")
                    .fix("reset_windows_update"),
            );
        }
    }
    for (title, list) in &fails {
        let n = list.len();
        r.findings.push(
            Finding::new(
                format!("update_failed_{}", title.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()),
                if n >= 2 { Status::Warn } else { Status::Info },
                format!("An update failed to install ({})", plural(n, "time", "times")),
                format!("{title}. Last attempt {}. {}", list[0].local_time(), list[0].data("errorCode").map(|c| format!("Error code {c}.")).unwrap_or_default()),
            )
            .advice("Restart and try again. If the same update keeps failing, use the Windows Update repair in Fixes, then search the error code online.")
            .fix("reset_windows_update"),
        );
    }

    if store_fails > 0 {
        r.facts.push(num_field("Store app updates that failed (retried automatically)", store_fails.to_string(), store_fails as f64));
    }

    // Is the Windows Update service disabled?
    if let Some(s) = ns_query(CIMV2, "SELECT StartMode, State FROM Win32_Service WHERE Name = 'wuauserv'").first() {
        let mode = s.s("StartMode").unwrap_or_default();
        r.facts.push(field("Windows Update service", format!("{} ({})", s.s("State").unwrap_or_default(), mode)));
        if mode.eq_ignore_ascii_case("Disabled") {
            r.findings.push(
                Finding::new("wu_disabled", Status::Warn, "The Windows Update service is turned off", "With this service disabled the PC cannot receive security updates. Some 'tweak' tools turn it off.")
                    .advice("Set the Windows Update service back to Manual in Services, or use the Windows Update repair in Fixes.")
                    .fix("reset_windows_update"),
            );
        }
    }

    for (i, e) in evs.iter().enumerate() {
        let ok = e.id == 19;
        let store = is_store_app(e.data("updateTitle").unwrap_or(""));
        let kind = match (ok, store) {
            (true, false) => "Installed",
            (true, true) => "Store app updated",
            (false, false) => "Failed",
            (false, true) => "Store app update failed",
        };
        let mut it = event_item(e, i, kind, vec![field("Update", e.data("updateTitle").unwrap_or("").to_string())]);
        it.status = if ok { Status::Ok } else if store { Status::Info } else { Status::Warn };
        r.items.push(it);
    }
    if r.items.is_empty() {
        for (i, q) in qfe.iter().enumerate() {
            r.items.push(item(
                format!("qfe{i}"),
                q.s("HotFixID").unwrap_or_default(),
                q.s("Description").unwrap_or_default(),
                vec![field("Update", q.s("HotFixID").unwrap_or_default()), field("Kind", q.s("Description").unwrap_or_default()), field("Installed", q.s("InstalledOn").unwrap_or_default())],
            ));
        }
    }
    r.items_label = Some("Update history".into());
    r.summary = match last {
        Some(d) => format!("Last update installed {}", d.format("%Y-%m-%d")),
        None => "Could not find when updates were last installed".into(),
    };
    if !pending.is_empty() {
        r.summary.push_str(" · restart pending");
    }
    Ok(())
}

fn is_store_app(title: &str) -> bool {
    let id: Vec<char> = title.chars().take(13).collect();
    id.len() == 13 && id[12] == '-' && id[..12].iter().all(|c| c.is_ascii_alphanumeric())
}

pub fn system_files(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    if !ctx.is_admin {
        r.limited = Some("This check needs administrator rights. Restart DxTool as administrator to run it.".into());
        r.summary = "Skipped: needs administrator rights".into();
        return Ok(());
    }
    let (code, out) = run_hidden(&sys32("dism.exe"), &["/Online", "/Cleanup-Image", "/CheckHealth", "/English"], Duration::from_secs(180))?;
    let low = out.to_lowercase();
    r.facts.push(field("DISM exit code", code.to_string()));
    r.facts.push(field("DISM output", out.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("Deployment") && !l.starts_with("Version") && !l.starts_with("Image Version")).collect::<Vec<_>>().join(" ")));
    if low.contains("no component store corruption detected") {
        r.summary = "No damage found in the Windows component store".into();
    } else if low.contains("not repairable") {
        r.summary = "The Windows component store is damaged and cannot be repaired".into();
        r.findings.push(
            Finding::new("component_store_broken", Status::Error, "Windows' repair source is damaged beyond repair", "The component store Windows uses to repair itself is damaged in a way that cannot be fixed in place.")
                .advice("An in-place upgrade (reinstalling Windows while keeping files and programs) is usually needed. Back up first."),
        );
    } else if low.contains("repairable") {
        r.summary = "The Windows component store is damaged but repairable".into();
        r.findings.push(
            Finding::new("component_store_damaged", Status::Warn, "Windows' repair source is damaged", "The component store Windows uses to repair system files has damage. This can make updates fail and stop system file repairs from working.")
                .advice("Run the system image repair (DISM RestoreHealth) followed by the system file repair in Fixes.")
                .fix("dism_repair"),
        );
    } else {
        return Err(format!("DISM returned an unexpected result (code {code})."));
    }
    Ok(())
}

pub fn sfc_verify(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    if !ctx.is_admin {
        r.limited = Some("This check needs administrator rights. Restart DxTool as administrator to run it.".into());
        r.summary = "Skipped: needs administrator rights".into();
        return Ok(());
    }
    let (code, out) = run_hidden(&sys32("sfc.exe"), &["/verifyonly"], Duration::from_secs(45 * 60))?;
    let low = out.to_lowercase();
    r.facts.push(field("SFC exit code", code.to_string()));
    let tail: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains('%')).collect();
    r.facts.push(field("SFC output", tail.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" ")));
    if low.contains("did not find any integrity violations") {
        r.summary = "No damaged Windows files found".into();
    } else if low.contains("found integrity violations") {
        r.summary = "Damaged Windows files found".into();
        r.findings.push(
            Finding::new("sfc_violations", Status::Warn, "Some Windows system files are damaged", "System File Checker found Windows files that do not match the originals. Damaged system files can cause crashes and odd behaviour.")
                .advice("Run the system file repair in Fixes (DISM RestoreHealth, then SFC /scannow).")
                .fix("sfc_repair"),
        );
    } else if low.contains("could not perform") || low.contains("pending") {
        r.summary = "System File Checker could not run".into();
        r.findings.push(Finding::new("sfc_failed", Status::Info, "System File Checker could not finish", "This usually means a restart is pending or another repair is running.").advice("Restart the PC and run this check again."));
    } else {
        return Err(format!("System File Checker returned an unexpected result (code {code})."));
    }
    Ok(())
}

/// StartupApproved stores a binary value whose first byte is odd when the entry is disabled.
fn startup_disabled(name: &str) -> bool {
    let paths = [
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run",
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run32",
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\StartupFolder",
    ];
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for p in paths {
            if let Ok(k) = RegKey::predef(root).open_subkey(p) {
                for key in [name.to_string(), format!("{name}.lnk")] {
                    if let Ok(v) = k.get_raw_value(&key) {
                        if let Some(b) = v.bytes.first() {
                            return b & 1 == 1;
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn startup(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let rows = ns_query(CIMV2, "SELECT Name, Command, Location, User FROM Win32_StartupCommand");
    let mut enabled = 0;
    for (i, s) in rows.iter().enumerate() {
        let name = s.s("Name").unwrap_or_default();
        if name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        let off = startup_disabled(&name);
        if !off {
            enabled += 1;
        }
        let loc = s.s("Location").unwrap_or_default();
        let where_ = if loc.contains("Startup") { "Startup folder" } else if loc.starts_with("HKU") || loc.starts_with("HKCU") { "Registry (this user)" } else if loc.starts_with("HKLM") { "Registry (all users)" } else { "Other" };
        let mut it = item(
            format!("st{i}"),
            name.clone(),
            s.s("Command").unwrap_or_default(),
            vec![
                field("Enabled", if off { "No" } else { "Yes" }),
                field("Command", s.s("Command").unwrap_or_default()),
                field("Where", where_),
                field("User", s.s("User").unwrap_or_default()),
                field("Location", loc),
            ],
        );
        if off {
            it.status = Status::Info;
            it.status_text = Some("Disabled".into());
        }
        r.items.push(it);
    }
    r.facts.push(num_field("Programs starting with Windows", enabled.to_string(), enabled as f64));
    if enabled > 15 {
        r.findings.push(
            Finding::new("startup_many", if enabled > 25 { Status::Warn } else { Status::Info }, format!("{enabled} programs start with Windows"), "Every program that starts with Windows makes startup slower and uses memory in the background.")
                .advice("Open Task Manager → Startup apps and disable programs you don't need right away (updaters, launchers, chat apps). They still work when you open them.")
                .evidence(vec![]),
        );
    }

    // Boot time (this log normally needs administrator rights).
    let boot = events::query("Microsoft-Windows-Diagnostics-Performance/Operational", "*[System[EventID=100]]", 10).unwrap_or_default();
    if let Some(b) = boot.first() {
        if let Some(ms) = b.data("BootTime").and_then(|v| v.parse::<f64>().ok()) {
            let s = ms / 1000.0;
            r.facts.push(num_field("Last startup took", format!("{s:.0} seconds ({})", b.local_time()), s));
            let avg: Vec<f64> = boot.iter().filter_map(|e| e.data("BootTime")?.parse::<f64>().ok()).collect();
            if avg.len() > 1 {
                let a = avg.iter().sum::<f64>() / avg.len() as f64 / 1000.0;
                r.facts.push(num_field("Average startup", format!("{a:.0} seconds over {} boots", avg.len()), a));
            }
            if s > 90.0 {
                r.findings.push(
                    Finding::new("boot_slow", if s > 180.0 { Status::Warn } else { Status::Info }, format!("Startup is slow ({s:.0} seconds)"), "Windows measured how long it took from power-on until the desktop was ready to use.")
                        .advice("Disable unneeded startup programs. A hard drive (rather than an SSD), a nearly full drive or failing hardware also slows startup."),
                );
            }
        }
    }
    r.items_label = Some("Startup programs".into());
    r.summary = format!("{} start with Windows", plural(enabled, "program", "programs"));
    if let Some(f) = r.facts.iter().find(|f| f.label == "Last startup took") {
        r.summary.push_str(&format!(" · startup took {}", f.display.split(" (").next().unwrap_or("")));
    }
    Ok(())
}

pub fn resources(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let os = ns_query(CIMV2, "SELECT TotalVisibleMemorySize, FreePhysicalMemory, TotalVirtualMemorySize, FreeVirtualMemory, LastBootUpTime, NumberOfProcesses FROM Win32_OperatingSystem");
    let os = os.first().cloned().unwrap_or_default();
    let total = os.f("TotalVisibleMemorySize").unwrap_or(0.0) * 1024.0;
    let free = os.f("FreePhysicalMemory").unwrap_or(0.0) * 1024.0;
    let used_pct = if total > 0.0 { (total - free) / total * 100.0 } else { 0.0 };
    let vtotal = os.f("TotalVirtualMemorySize").unwrap_or(0.0) * 1024.0;
    let vfree = os.f("FreeVirtualMemory").unwrap_or(0.0) * 1024.0;
    let commit_pct = if vtotal > 0.0 { (vtotal - vfree) / vtotal * 100.0 } else { 0.0 };
    r.facts.push(num_field("Memory in use", format!("{} of {} ({used_pct:.0}%)", fmt::bytes_bin(total - free), fmt::bytes_bin(total)), used_pct));
    r.facts.push(num_field("Committed memory", format!("{} of {} ({commit_pct:.0}%)", fmt::bytes_bin(vtotal - vfree), fmt::bytes_bin(vtotal)), commit_pct));
    if let Some(n) = os.f("NumberOfProcesses") {
        r.facts.push(num_field("Running processes", format!("{n:.0}"), n));
    }
    if used_pct >= 90.0 || commit_pct >= 90.0 {
        r.findings.push(
            Finding::new("memory_full", Status::Warn, format!("Memory is nearly full ({:.0}% in use)", used_pct.max(commit_pct)), "When memory runs out Windows has to use the much slower drive instead, so everything slows down and programs may close.")
                .advice("Close programs and browser tabs you aren't using. If this happens often, more RAM would help."),
        );
    }

    let boot = os.s("LastBootUpTime").as_deref().and_then(fmt::wmi_to_naive);
    if let Some(b) = boot {
        let up = (Local::now().naive_local() - b).num_seconds().max(0) as u64;
        let days = up / 86_400;
        r.facts.push(num_field("Up for", fmt::duration_secs(up), up as f64));
        if days >= 14 {
            r.findings.push(
                Finding::new("uptime_long", if days >= 30 { Status::Warn } else { Status::Info }, format!("Not restarted in {days} days"), "Some updates only finish after a restart, and a fresh start clears out memory leaks and stuck programs. With Fast Startup turned on, Shut down does not count as a restart.")
                    .advice("Use Start → Power → Restart now and then."),
            );
        }
    }
    if let Some(l) = ns_query(CIMV2, "SELECT LoadPercentage FROM Win32_Processor").first().and_then(|p| p.f("LoadPercentage")) {
        r.facts.push(num_field("Processor load right now", format!("{l:.0}%"), l));
    }
    let pf = ns_query(CIMV2, "SELECT Name, AllocatedBaseSize, CurrentUsage, PeakUsage FROM Win32_PageFileUsage");
    if pf.is_empty() {
        r.findings.push(
            Finding::new("no_pagefile", Status::Info, "There is no page file", "Without a page file, programs can run out of memory sooner and Windows cannot save crash details after a blue screen.")
                .advice("Unless you turned it off on purpose, set virtual memory back to 'Automatically manage' in System → Advanced system settings."),
        );
    } else {
        for p in &pf {
            r.facts.push(field("Page file", format!("{} · {} MB, peak use {} MB", p.s("Name").unwrap_or_default(), p.s("AllocatedBaseSize").unwrap_or_default(), p.s("PeakUsage").unwrap_or_default())));
        }
    }

    // Programs using the most memory (grouped by name)
    let procs = ns_query(CIMV2, "SELECT Name, WorkingSetSize FROM Win32_Process");
    let mut agg: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for p in &procs {
        let n = p.s("Name").unwrap_or_default();
        let e = agg.entry(n).or_default();
        e.0 += p.f("WorkingSetSize").unwrap_or(0.0);
        e.1 += 1;
    }
    let mut top: Vec<(String, (f64, usize))> = agg.into_iter().collect();
    top.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));
    for (i, (name, (bytes, count))) in top.into_iter().take(20).enumerate() {
        let pct = if total > 0.0 { bytes / total * 100.0 } else { 0.0 };
        r.items.push(item(
            format!("proc{i}"),
            name.clone(),
            "",
            vec![
                num_field("Memory", fmt::bytes_bin(bytes), bytes),
                num_field("Share of RAM", format!("{pct:.1}%"), pct),
                num_field("Processes", count.to_string(), count as f64),
            ],
        ));
    }
    r.items_label = Some("Top programs by memory".into());
    r.summary = format!("{used_pct:.0}% of memory in use");
    if let Some(f) = r.facts.iter().find(|f| f.label == "Up for") {
        r.summary.push_str(&format!(" · up for {}", f.display));
    }
    Ok(())
}

pub fn battery_health(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let items = crate::inventory::battery()?;
    if items.is_empty() {
        r.summary = "No battery (desktop PC)".into();
        return Ok(());
    }
    for b in &items {
        if b.status != Status::Ok {
            r.findings.push(
                Finding::new(format!("battery_{}", b.id), b.status, b.status_text.clone().unwrap_or_else(|| "Battery is worn".into()), "Batteries lose capacity as they age and are charged. A worn battery runs out sooner and may shut the laptop off suddenly.")
                    .advice(if b.status == Status::Error { "Consider replacing the battery." } else { "Nothing urgent. Avoiding long periods at 100% or in heat slows further wear." })
                    .evidence(b.summary.clone()),
            );
        }
    }
    let health = items[0].summary.iter().find(|f| f.label == "Health").map(|f| f.display.clone());
    r.summary = match health {
        Some(h) => format!("Battery health {h}"),
        None => "Battery found (health not reported)".into(),
    };
    r.items = items;
    r.items_label = Some("Batteries".into());
    Ok(())
}

pub fn activation(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let rows = ns_query(
        CIMV2,
        "SELECT Name, Description, LicenseStatus, PartialProductKey, ProductKeyChannel, GracePeriodRemaining FROM SoftwareLicensingProduct WHERE ApplicationID = '55c92734-d682-4d71-983e-d6ec3f16059f' AND PartialProductKey IS NOT NULL",
    );
    let Some(p) = rows.iter().find(|p| p.s("Name").map(|n| n.starts_with("Windows")).unwrap_or(false)).or(rows.first()) else {
        return Err("Windows did not report any licence.".into());
    };
    let status = p.u("LicenseStatus").unwrap_or(99);
    let text = match status {
        0 => "Not activated",
        1 => "Activated",
        2 => "Initial grace period",
        3 => "Additional grace period",
        4 => "Non-genuine grace period",
        5 => "Not activated (notifications)",
        6 => "Extended grace period",
        _ => "Unknown",
    };
    let channel = p.s("ProductKeyChannel").unwrap_or_else(|| p.s("Description").unwrap_or_default());
    r.facts.push(field("Status", text));
    r.facts.push(field("Edition", p.s("Name").unwrap_or_default()));
    r.facts.push(field("Licence type", channel.clone()));
    r.facts.push(field("Product key ends in", p.s("PartialProductKey").unwrap_or_default()));
    if status != 1 {
        r.findings.push(
            Finding::new("not_activated", Status::Warn, format!("Windows is not activated ({text})"), "Unactivated Windows shows a watermark, limits personalisation and keeps reminding you to activate.")
                .advice("Open Settings → System → Activation. If this PC came with Windows, the troubleshooter there usually reactivates it."),
        );
    }
    r.summary = format!("{text} · {channel}");
    Ok(())
}
