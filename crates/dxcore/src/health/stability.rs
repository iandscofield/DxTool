//! Stability checks: blue screens, unexpected shutdowns, hardware errors, devices,
//! drivers, services and app crashes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Local, Utc};
use serde_json::json;

use super::bugcheck;
use super::{event_item, field, item, num_field, plural, Ctx};
use crate::devices::{problem_status, problem_text};
use crate::events::{self, Event};
use crate::fmt;
use crate::model::{CheckResult, Field, Finding, Status};
use crate::wmiutil::{ns_query, raw_section, RowExt};

struct Crash {
    time: DateTime<Utc>,
    code: u32,
    params: Vec<String>,
    dump: Option<String>,
    source: &'static str,
}

fn dt_field(label: &str, t: DateTime<Utc>) -> Field {
    Field {
        key: label.into(),
        label: label.into(),
        display: t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
        sort: json!(t.timestamp()),
    }
}

pub fn crashes(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let days = ctx.days;
    let bsod_q = events::system_filter(&[
        events::providers(&["Microsoft-Windows-WER-SystemErrorReporting", "BugCheck"]),
        "EventID=1001".into(),
        events::within_days(days),
    ]);
    let kp_q = events::system_filter(&[events::providers(&["Microsoft-Windows-Kernel-Power"]), "EventID=41".into(), events::within_days(days)]);
    let bsod_ev = events::query("System", &bsod_q, 200)?;
    let kp_ev = events::query("System", &kp_q, 200).unwrap_or_default();

    let mut list: Vec<Crash> = Vec::new();
    for e in &bsod_ev {
        let Some(t) = e.time else { continue };
        let text = e.data("param1").map(str::to_string).unwrap_or_else(|| e.message.clone());
        let Some((code, params)) = bugcheck::parse_from_text(&text) else { continue };
        let dump = e.data("param2").map(str::to_string).filter(|s| s.to_lowercase().ends_with(".dmp"));
        list.push(Crash { time: t, code, params, dump, source: "Event log" });
    }
    let near = |list: &[Crash], t: DateTime<Utc>| list.iter().position(|c| (c.time - t).num_minutes().abs() <= 15);

    let mut shutdowns: Vec<&Event> = Vec::new();
    let mut held_button = 0;
    for e in &kp_ev {
        let Some(t) = e.time else { continue };
        let code = e.data("BugcheckCode").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) as u32;
        if code != 0 {
            if near(&list, t).is_none() {
                list.push(Crash { time: t, code, params: vec![], dump: None, source: "Event log (Kernel-Power)" });
            }
        } else {
            if e.data("PowerButtonTimestamp").map(|v| v != "0").unwrap_or(false) {
                held_button += 1;
            }
            shutdowns.push(e);
        }
    }

    // Crash dump files (the folder normally needs administrator rights).
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let mut dump_files: Vec<(PathBuf, DateTime<Utc>, u64)> = Vec::new();
    match std::fs::read_dir(format!("{root}\\Minidump")) {
        Ok(rd) => {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x.eq_ignore_ascii_case("dmp")).unwrap_or(false) {
                    if let Ok(m) = e.metadata() {
                        let t: DateTime<Utc> = m.modified().map(DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now());
                        dump_files.push((p, t, m.len()));
                    }
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            r.limited = Some("Crash dump files can only be read with administrator rights. Crashes are still found from the event log.".into());
        }
        Err(_) => {}
    }
    let full_dump = PathBuf::from(format!("{root}\\MEMORY.DMP"));
    if let Ok(m) = std::fs::metadata(&full_dump) {
        r.facts.push(num_field("Full memory dump", format!("{} · {}", fmt::bytes_dec(m.len() as f64), m.modified().map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()), m.len() as f64));
    }
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    for (p, t, _) in &dump_files {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if let Some(i) = near(&list, *t) {
            list[i].dump.get_or_insert(p.to_string_lossy().to_string());
        } else if *t >= cutoff {
            if let Some((code, params)) = bugcheck::read_dump_header(p) {
                list.push(Crash { time: *t, code, params: params.iter().map(|v| format!("{v:#x}")).collect(), dump: Some(p.to_string_lossy().to_string()), source: "Crash dump file" });
            }
        }
        let _ = name;
    }
    r.facts.push(num_field("Crash dump files", dump_files.len().to_string(), dump_files.len() as f64));
    list.sort_by(|a, b| b.time.cmp(&a.time));

    // Findings: one per distinct stop code.
    let mut by_code: BTreeMap<u32, Vec<&Crash>> = BTreeMap::new();
    for c in &list {
        by_code.entry(c.code).or_default().push(c);
    }
    let week_ago = Utc::now() - chrono::Duration::days(7);
    for (code, cs) in &by_code {
        let sc = bugcheck::lookup(*code);
        let n = cs.len();
        let recent = cs.iter().any(|c| c.time >= week_ago);
        let sev = if n >= 2 || recent { Status::Error } else { Status::Warn };
        let fix = if matches!(code, 0xEF | 0xC000_021A) { Some("sfc_repair") } else if sc.advice.contains("Update drivers") { Some("update_drivers") } else { None };
        let mut f = Finding::new(
            format!("bsod_{code:x}"),
            sev,
            format!("Blue screen: {} ({})", sc.name, plural(n, "time", "times")),
            format!("{} Stop code {code:#010X}. Last crash {}.", sc.meaning, cs[0].time.with_timezone(&Local).format("%Y-%m-%d %H:%M")),
        )
        .advice(sc.advice)
        .evidence(vec![
            field("Stop code", format!("{code:#010X}")),
            field("Name", sc.name),
            field("Occurrences", n.to_string()),
            field("Parameters", cs[0].params.join(", ")),
            field("Dump file", cs.iter().find_map(|c| c.dump.clone()).unwrap_or_default()),
        ]);
        if let Some(fx) = fix {
            f = f.fix(fx);
        }
        r.findings.push(f);
    }
    if !shutdowns.is_empty() {
        let n = shutdowns.len();
        let recent = shutdowns[0].time.map(|t| t >= week_ago).unwrap_or(false);
        let sev = if n >= 5 { Status::Error } else if n >= 2 || recent { Status::Warn } else { Status::Info };
        let held = if held_button > 0 { format!(" In {} the power button was held down.", plural(held_button, "case", "cases")) } else { String::new() };
        r.findings.push(
            Finding::new(
                "unexpected_shutdown",
                sev,
                format!("The PC shut off unexpectedly {}", plural(n, "time", "times")),
                format!("Windows restarted without shutting down properly first. This happens after power cuts, holding the power button, the PC freezing, or the power supply or overheating cutting power.{held} Last time {}.", shutdowns[0].local_time()),
            )
            .advice("If nobody cut the power or held the button, check temperatures and dust, make sure cables are firm, and consider testing the power supply. Freezes before the shutdown point to drivers or hardware.")
            .evidence(vec![field("Occurrences", n.to_string()), field("Power button held", held_button.to_string())]),
        );
    }

    for (i, c) in list.iter().enumerate() {
        let sc = bugcheck::lookup(c.code);
        let mut it = item(
            format!("crash{i}"),
            format!("Blue screen: {}", sc.name),
            c.time.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
            vec![
                dt_field("When", c.time),
                field("Stop code", format!("{:#010X}", c.code)),
                field("Name", sc.name),
                field("Meaning", sc.meaning),
                field("Dump file", c.dump.clone().unwrap_or_default()),
                field("Found in", c.source),
            ],
        );
        it.status = Status::Error;
        it.sections.push(crate::model::Section::new(
            "What to do",
            vec![field("Advice", sc.advice), field("Parameters", c.params.join(", "))],
        ));
        r.items.push(it);
    }
    for (i, e) in shutdowns.iter().enumerate() {
        let held = e.data("PowerButtonTimestamp").map(|v| v != "0").unwrap_or(false);
        let mut it = event_item(e, i, "Unexpected shutdown", vec![field("Meaning", if held { "Power button was held down" } else { "Lost power, froze or was reset" })]);
        it.status = Status::Warn;
        r.items.push(it);
    }
    r.items_label = Some("Crashes and shutdowns".into());
    r.summary = match (list.len(), shutdowns.len()) {
        (0, 0) => format!("No crashes in the last {days} days"),
        (b, s) => {
            let mut parts = vec![];
            if b > 0 {
                parts.push(plural(b, "blue screen", "blue screens"));
            }
            if s > 0 {
                parts.push(plural(s, "unexpected shutdown", "unexpected shutdowns"));
            }
            format!("{} in the last {days} days", parts.join(" and "))
        }
    };
    Ok(())
}

pub fn hardware_errors(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let since = events::within_days(ctx.days);
    let whea = events::query(
        "System",
        &events::system_filter(&[events::providers(&["Microsoft-Windows-WHEA-Logger"]), "(Level=1 or Level=2 or Level=3)".into(), since.clone()]),
        300,
    )?;
    let tdr = events::query("System", &events::system_filter(&[events::providers(&["Display"]), "EventID=4101".into(), since.clone()]), 200).unwrap_or_default();
    let throttle = events::query(
        "System",
        &events::system_filter(&[events::providers(&["Microsoft-Windows-Kernel-Processor-Power"]), "EventID=37".into(), since.clone()]),
        200,
    )
    .unwrap_or_default();
    let memdiag = events::query("System", &events::system_filter(&[events::providers(&["Microsoft-Windows-MemoryDiagnostics-Results"])]), 5).unwrap_or_default();

    // WHEA: split fatal from corrected, and by component.
    let component = |e: &Event| -> String {
        e.message
            .lines()
            .find_map(|l| l.trim().strip_prefix("Component:").map(|s| s.trim().to_string()))
            .or_else(|| e.data("ErrorSource").map(|s| format!("Error source {s}")))
            .unwrap_or_else(|| "Hardware".into())
    };
    let mut fatal: BTreeMap<String, Vec<&Event>> = BTreeMap::new();
    let mut corrected: BTreeMap<String, Vec<&Event>> = BTreeMap::new();
    for e in &whea {
        let target = if e.level <= 2 { &mut fatal } else { &mut corrected };
        target.entry(component(e)).or_default().push(e);
    }
    for (comp, evs) in &fatal {
        r.findings.push(
            Finding::new(
                format!("whea_fatal_{}", comp.to_lowercase().replace(' ', "_")),
                Status::Error,
                format!("Serious hardware error: {comp} ({})", plural(evs.len(), "time", "times")),
                format!("The hardware reported an error it could not correct. This usually comes with a blue screen or sudden restart. Last seen {}.", evs[0].local_time()),
            )
            .advice("Remove any overclocking, check temperatures, update the BIOS and test the memory. If it continues, the part named may be failing.")
            .evidence(vec![field("Component", comp.clone()), field("Example", evs[0].short_message())]),
        );
    }
    for (comp, evs) in &corrected {
        let n = evs.len();
        let pcie = comp.to_lowercase().contains("pci");
        let sev = if (pcie && n < 50) || (!pcie && n < 3) { Status::Info } else { Status::Warn };
        let detail = if pcie {
            "A PCI Express link (graphics card, NVMe drive or other card) had errors that the hardware fixed by itself. A few are harmless; many can mean a loose card, a bad riser cable or a power-saving incompatibility."
        } else {
            "The hardware caught and fixed an error by itself. Occasional ones are harmless; repeated ones can mean an unstable overclock, overheating or a part starting to fail."
        };
        r.findings.push(
            Finding::new(
                format!("whea_corrected_{}", comp.to_lowercase().replace(' ', "_")),
                sev,
                format!("Corrected hardware errors: {comp} ({})", plural(n, "time", "times")),
                format!("{detail} Last seen {}.", evs[0].local_time()),
            )
            .advice(if pcie { "Reseat the card or drive involved. Updating the BIOS and chipset drivers often helps." } else { "Remove overclocking and check temperatures. Update the BIOS." })
            .evidence(vec![field("Component", comp.clone()), field("Example", evs[0].short_message())]),
        );
    }
    if !tdr.is_empty() {
        let n = tdr.len();
        let driver = tdr[0].message.split_whitespace().nth(2).unwrap_or("").to_string();
        r.findings.push(
            Finding::new(
                "gpu_tdr",
                if n >= 3 { Status::Warn } else { Status::Info },
                format!("The graphics driver crashed and recovered {}", plural(n, "time", "times")),
                format!("The screen may have gone black or flickered for a moment. Windows restarted the graphics driver ({driver}). Last time {}.", tdr[0].local_time()),
            )
            .advice("Update or cleanly reinstall the graphics driver, remove any graphics overclock and check graphics card temperatures.")
            .fix("update_drivers")
            .evidence(vec![field("Example", tdr[0].short_message())]),
        );
    }
    if throttle.len() >= 5 {
        r.findings.push(
            Finding::new(
                "cpu_throttle",
                if throttle.len() >= 50 { Status::Warn } else { Status::Info },
                format!("The processor was slowed down by the firmware {}", plural(throttle.len(), "time", "times")),
                "The BIOS limited the processor's speed, usually because of heat or power limits. The PC runs slower while this happens.",
            )
            .advice("Clean dust from fans and heatsinks, make sure vents are not blocked, and on laptops use the charger and a hard surface.")
            .evidence(vec![field("Example", throttle[0].short_message())]),
        );
    }
    if let Some(m) = memdiag.first() {
        let msg = m.message.to_lowercase();
        if msg.contains("hardware error") || (msg.contains("detected") && !msg.contains("no errors")) {
            r.findings.push(
                Finding::new("memdiag_failed", Status::Error, "Windows Memory Diagnostic found memory errors", format!("The last memory test ({}) found problems with the RAM.", m.local_time()))
                    .advice("Replace the faulty memory module. Testing one stick at a time finds which one.")
                    .evidence(vec![field("Result", m.short_message())]),
            );
        } else {
            r.facts.push(field("Last memory test", format!("{}: {}", m.local_time(), m.short_message())));
        }
    }

    let mut i = 0;
    for e in &whea {
        r.items.push(event_item(e, i, if e.level <= 2 { "Hardware error" } else { "Corrected hardware error" }, vec![field("Component", component(e))]));
        i += 1;
    }
    for e in &tdr {
        r.items.push(event_item(e, i, "Graphics driver reset", vec![field("Component", "Graphics")]));
        i += 1;
    }
    for e in throttle.iter().take(50) {
        r.items.push(event_item(e, i, "Processor throttled", vec![field("Component", "Processor")]));
        i += 1;
    }
    r.items.sort_by(|a, b| {
        let t = |it: &crate::model::Item| it.summary.first().and_then(|f| f.sort.as_i64()).unwrap_or(0);
        t(b).cmp(&t(a))
    });
    r.items_label = Some("Hardware events".into());
    let total = whea.len() + tdr.len();
    r.summary = if total == 0 {
        format!("No hardware errors in the last {} days", ctx.days)
    } else {
        format!("{} in the last {} days", plural(total, "hardware event", "hardware events"), ctx.days)
    };
    Ok(())
}

pub fn device_problems(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let rows = ns_query("ROOT\\CIMV2", "SELECT * FROM Win32_PnPEntity WHERE ConfigManagerErrorCode <> 0");
    let mut disabled = 0;
    for d in &rows {
        let code = d.u("ConfigManagerErrorCode").unwrap_or(0) as u32;
        let name = d.s("Name").or_else(|| d.s("Description")).unwrap_or_else(|| "Unknown device".into());
        let id = d.s("PNPDeviceID").unwrap_or_default();
        let status = problem_status(code);
        let text = problem_text(code);
        let mut it = item(
            id.clone(),
            name.clone(),
            d.s("PNPClass").unwrap_or_default(),
            vec![
                field("Problem", text.clone()),
                num_field("Code", code.to_string(), code as f64),
                field("Class", d.s("PNPClass").unwrap_or_default()),
                field("Manufacturer", d.s("Manufacturer").unwrap_or_default()),
                field("Device ID", id.clone()),
            ],
        );
        it.status = status;
        it.status_text = Some(text.clone());
        it.sections.push(raw_section("Win32_PnPEntity", d));
        if status == Status::Info {
            disabled += 1;
        } else {
            let (advice, fix) = match code {
                28 | 18 | 39 | 31 | 1 => ("Install or reinstall the driver for this device, from Windows Update or the device maker's website.", Some("update_drivers")),
                10 | 43 => ("Unplug and reconnect the device (or restart the PC), then update its driver. If it keeps failing, the device itself may be faulty.", Some("update_drivers")),
                52 => ("The driver's signature could not be verified. Install a signed driver from the device maker.", Some("update_drivers")),
                14 => ("Restart the PC.", None),
                _ => ("Open Device Manager for details, try updating the driver, or remove the device and restart so Windows sets it up again.", Some("update_drivers")),
            };
            let mut f = Finding::new(format!("device_{}", it.id), status, format!("{name}: {text}"), "Device Manager reports that this device is not working properly.")
                .advice(advice)
                .evidence(it.summary.clone());
            if let Some(fx) = fix {
                f = f.fix(fx);
            }
            r.findings.push(f);
        }
        r.items.push(it);
    }
    r.items_label = Some("Devices with problems".into());
    let problems = rows.len() - disabled;
    r.summary = match (problems, disabled) {
        (0, 0) => "All devices working".into(),
        (0, d) => format!("All devices working ({} disabled)", plural(d, "device", "devices")),
        (p, _) => format!("{} not working properly", plural(p, "device is", "devices are")),
    };
    Ok(())
}

fn wmi_date(s: &str) -> Option<chrono::NaiveDate> {
    fmt::wmi_to_naive(s).map(|d| d.date())
}

pub fn drivers(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let rows = ns_query(
        "ROOT\\CIMV2",
        "SELECT DeviceName, DeviceClass, DriverProviderName, DriverVersion, DriverDate, IsSigned, Signer, InfName, DeviceID, Manufacturer FROM Win32_PnPSignedDriver",
    );
    let today = Local::now().date_naive();
    let mut unsigned = Vec::new();
    for d in &rows {
        let Some(name) = d.s("DeviceName") else { continue };
        let provider = d.s("DriverProviderName").unwrap_or_default();
        if d.b("IsSigned") == Some(false) {
            unsigned.push(name.clone());
        }
        if provider.eq_ignore_ascii_case("Microsoft") || provider.is_empty() {
            continue;
        }
        let date = d.s("DriverDate").as_deref().and_then(wmi_date);
        let age_days = date.map(|dt| (today - dt).num_days()).unwrap_or(0);
        let mut it = item(
            d.s("DeviceID").unwrap_or_else(|| name.clone()),
            name.clone(),
            provider.clone(),
            vec![
                field("Class", d.s("DeviceClass").unwrap_or_default()),
                field("Provider", provider.clone()),
                field("Version", d.s("DriverVersion").unwrap_or_default()),
                Field {
                    key: "Date".into(),
                    label: "Date".into(),
                    display: date.map(|x| x.format("%Y-%m-%d").to_string()).unwrap_or_default(),
                    sort: json!(date.map(|x| x.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()).unwrap_or(0)),
                },
                num_field("Age", if age_days > 0 { format!("{:.1} years", age_days as f64 / 365.25) } else { String::new() }, age_days as f64),
                field("Signed", d.b("IsSigned").map(fmt::yes_no).unwrap_or("Unknown")),
                field("Signer", d.s("Signer").unwrap_or_default()),
                field("INF file", d.s("InfName").unwrap_or_default()),
            ],
        );
        if d.b("IsSigned") == Some(false) {
            it.status = Status::Warn;
            it.status_text = Some("Unsigned".into());
        }
        r.items.push(it);
    }
    r.items.sort_by(|a, b| a.name.cmp(&b.name));

    // Graphics driver age
    for g in ns_query("ROOT\\CIMV2", "SELECT Name, DriverVersion, DriverDate, PNPDeviceID FROM Win32_VideoController") {
        let name = g.s("Name").unwrap_or_default();
        let lname = name.to_lowercase();
        if lname.contains("basic display") || lname.contains("remote") || lname.contains("virtual") || lname.contains("parsec") || lname.contains("meta") {
            continue;
        }
        let Some(date) = g.s("DriverDate").as_deref().and_then(wmi_date) else { continue };
        let days = (today - date).num_days();
        r.facts.push(field(&format!("{name} driver"), format!("{} ({})", g.s("DriverVersion").unwrap_or_default(), date.format("%Y-%m-%d"))));
        if days > 365 {
            r.findings.push(
                Finding::new(
                    format!("gpu_driver_old_{}", lname.replace(' ', "_")),
                    if days > 730 { Status::Warn } else { Status::Info },
                    format!("The {name} driver is {:.1} years old", days as f64 / 365.25),
                    "Newer graphics drivers fix crashes, improve game performance and patch security issues.",
                )
                .advice("Download the latest driver from NVIDIA, AMD or Intel, or from the laptop maker's website.")
                .fix("update_drivers"),
            );
        }
    }
    if !unsigned.is_empty() {
        unsigned.sort();
        unsigned.dedup();
        r.findings.push(
            Finding::new("unsigned_drivers", Status::Info, format!("{} installed", plural(unsigned.len(), "unsigned driver is", "unsigned drivers are")), "Unsigned drivers have not been verified by Microsoft. Most are just old, but they can be unstable or, rarely, malicious.")
                .advice("Check where these drivers came from and replace them with signed versions from the device maker.")
                .evidence(vec![field("Devices", unsigned.join(", "))]),
        );
    }
    r.items_label = Some("Third-party drivers".into());
    r.summary = format!("{} from other companies{}", plural(r.items.len(), "driver", "drivers"), if unsigned.is_empty() { ", all signed".to_string() } else { format!(", {} unsigned", unsigned.len()) });
    Ok(())
}

fn service_name(e: &Event) -> String {
    let key = if matches!(e.id, 7009 | 7011) { "param2" } else { "param1" };
    e.data(key).map(str::to_string).unwrap_or_else(|| "Unknown service".into())
}

pub fn service_failures(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let q = events::system_filter(&[
        events::providers(&["Service Control Manager"]),
        events::ids(&[7000, 7001, 7009, 7011, 7022, 7023, 7024, 7031, 7034]),
        events::within_days(ctx.days),
    ]);
    let evs = events::query("System", &q, 400)?;
    let mut by: BTreeMap<String, Vec<&Event>> = BTreeMap::new();
    for e in &evs {
        let svc = service_name(e);
        by.entry(svc).or_default().push(e);
    }
    for (svc, list) in &by {
        let n = list.len();
        let crashed = list.iter().any(|e| matches!(e.id, 7031 | 7034));
        let what = if crashed { "stopped unexpectedly" } else { "failed to start" };
        r.findings.push(
            Finding::new(
                format!("svc_{}", svc.to_lowercase().replace(' ', "_")),
                // Failed starts are usually harmless (launchers, removed programs); repeated crashes are not.
                if crashed && n >= 5 { Status::Warn } else { Status::Info },
                format!("{svc} {what} ({})", plural(n, "time", "times")),
                format!("A background service {what}. {} Last time {}.", if crashed { "Windows usually restarts it automatically." } else { "Features that depend on it may not work." }, list[0].local_time()),
            )
            .advice("If this service belongs to a program you removed, it is safe to ignore. Otherwise reinstall or update the program it belongs to.")
            .evidence(vec![field("Example", list[0].short_message())]),
        );
    }
    for (i, e) in evs.iter().enumerate() {
        let kind = match e.id {
            7031 | 7034 => "Service crashed",
            7009 | 7011 | 7022 => "Service timed out",
            _ => "Service failed",
        };
        r.items.push(event_item(e, i, kind, vec![field("Service", service_name(e))]));
    }
    r.items_label = Some("Service events".into());
    r.summary = if by.is_empty() {
        format!("No service failures in the last {} days", ctx.days)
    } else {
        format!("{} had problems in the last {} days", plural(by.len(), "service", "services"), ctx.days)
    };
    Ok(())
}

pub fn app_crashes(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let q = events::system_filter(&[
        events::providers(&["Application Error", "Application Hang"]),
        events::ids(&[1000, 1002]),
        events::within_days(ctx.days),
    ]);
    let evs = events::query("Application", &q, 500)?;
    struct Agg<'a> {
        crashes: usize,
        hangs: usize,
        modules: BTreeMap<String, usize>,
        last: &'a Event,
    }
    let mut by: BTreeMap<String, Agg> = BTreeMap::new();
    for e in &evs {
        let app = e.data("AppName").or(e.data("param1")).unwrap_or("Unknown program").to_string();
        let a = by.entry(app.to_lowercase()).or_insert(Agg { crashes: 0, hangs: 0, modules: BTreeMap::new(), last: e });
        if e.id == 1002 {
            a.hangs += 1;
        } else {
            a.crashes += 1;
            if let Some(m) = e.data("ModuleName").or(e.data("param4")) {
                *a.modules.entry(m.to_string()).or_default() += 1;
            }
        }
    }
    let mut ranked: Vec<(&String, &Agg)> = by.iter().collect();
    ranked.sort_by(|a, b| (b.1.crashes + b.1.hangs).cmp(&(a.1.crashes + a.1.hangs)));
    for (_, a) in &ranked {
        let total = a.crashes + a.hangs;
        let app = a.last.data("AppName").or(a.last.data("param1")).unwrap_or("Unknown program");
        let top_module = a.modules.iter().max_by_key(|(_, n)| **n).map(|(m, _)| m.clone()).unwrap_or_default();
        let mut parts = vec![];
        if a.crashes > 0 {
            parts.push(format!("crashed {}", plural(a.crashes, "time", "times")));
        }
        if a.hangs > 0 {
            parts.push(format!("stopped responding {}", plural(a.hangs, "time", "times")));
        }
        let mut summary = vec![
            field("Program", app),
            num_field("Crashes", a.crashes.to_string(), a.crashes as f64),
            num_field("Hangs", a.hangs.to_string(), a.hangs as f64),
            field("Most common faulting file", top_module.clone()),
        ];
        summary.insert(1, Field { key: "Last".into(), label: "Last".into(), display: a.last.local_time(), sort: json!(a.last.timestamp()) });
        let mut it = item(format!("app_{app}"), app, parts.join(", "), summary);
        it.status = if total >= 5 { Status::Warn } else { Status::Info };
        r.items.push(it);
        if total >= 5 {
            let hint = if !top_module.is_empty() && !top_module.eq_ignore_ascii_case(app) {
                format!(" Most crashes happened in {top_module}, which may belong to a driver, plug-in or another program.")
            } else {
                String::new()
            };
            r.findings.push(
                Finding::new(format!("app_{}", app.to_lowercase()), Status::Warn, format!("{app} {}", parts.join(" and ")), format!("This program has been unstable in the last {} days.{hint}", ctx.days))
                    .advice("Update or reinstall the program. If the faulting file belongs to a graphics driver or plug-in, update that instead."),
            );
        }
    }
    r.items_label = Some("Programs".into());
    r.summary = if evs.is_empty() {
        format!("No program crashes in the last {} days", ctx.days)
    } else {
        format!("{} had {} in the last {} days", plural(by.len(), "program", "programs"), plural(evs.len(), "crash or hang", "crashes or hangs"), ctx.days)
    };
    Ok(())
}
