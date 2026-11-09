//! A Model Context Protocol (MCP) server so Claude can use DxTool's diagnostics.
//!
//! Started with `DxTool.exe --mcp`; Claude Desktop or Claude Code talk to it over
//! stdin/stdout using newline-delimited JSON-RPC 2.0. The tools only read and diagnose:
//! repairs and virus removal stay in the app, where the user sees and confirms them.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::model::{Inventory, ScanReport, Status};

const PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "DxTool is a Windows diagnostic tool running on the user's PC. \
Start with get_pc_overview. Run run_health_scan (fast takes seconds, full under a minute) to check drives, crashes, \
Windows Update, drivers, security and more; findings come with plain-English advice. Use get_hardware_inventory for \
detailed specs, get_upgrade_recommendations for what to upgrade or whether Windows 11 is possible, check_game to see whether a game will run, check_newest_drivers for graphics and network driver updates, and query_event_log to dig into crashes and errors. DxTool's repairs (SFC, DISM, chkdsk, driver updates, \
etc.) and virus removal are run by the user inside the DxTool app, which creates a restore point first: recommend them \
by name rather than giving manual commands when a DxTool fix exists. Explain things simply; the user may not be technical.";

fn tools() -> Value {
    json!([
        {
            "name": "get_pc_overview",
            "description": "Summary of this PC: model, CPU, memory, graphics, Windows version, plus the latest health scan score and its problems. Start here.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "run_health_scan",
            "description": "Runs a DxTool health scan now and saves it to the PC's history. 'fast' (a few seconds) covers the essentials; 'full' (up to a minute) adds crash dumps, drivers and deeper event log checks. Returns the score and every finding with advice.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["fast", "full"], "description": "Scan depth (default fast)." },
                    "include_details": { "type": "boolean", "description": "Also include each check's measured facts and data rows (longer)." }
                }
            }
        },
        {
            "name": "get_latest_health_scan",
            "description": "The most recent saved health scan for this PC (or another PC in DxTool's history), without running a new one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "machine_id": { "type": "string", "description": "From list_machines; defaults to this PC." },
                    "include_details": { "type": "boolean" }
                }
            }
        },
        {
            "name": "list_machines",
            "description": "PCs in DxTool's history, each with its saved scans (id, date, kind, score). Use with get_saved_scan to compare over time.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_saved_scan",
            "description": "A specific saved health scan.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "machine_id": { "type": "string" },
                    "scan_id": { "type": "string" },
                    "include_details": { "type": "boolean" }
                },
                "required": ["machine_id", "scan_id"]
            }
        },
        {
            "name": "get_upgrade_recommendations",
            "description": "DxTool's upgrade advice for this PC, worked out from its hardware: memory (amount, empty slots, XMP/EXPO switched off), moving Windows to an SSD, failing or full drives, Windows 11 readiness, worn laptop batteries, graphics and processor age. Each item has a priority, rough cost, why and how. Also lists what is already fine.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "check_newest_drivers",
            "description": "Compares this PC's graphics, Wi-Fi, Bluetooth and Ethernet drivers with the newest versions the makers publish: NVIDIA graphics (exact, from NVIDIA's driver service), Intel graphics/Wi-Fi/Bluetooth/Ethernet (from Intel's Driver & Support Assistant catalog), AMD graphics (installed version only, with AMD's page). Gives download links. Needs the internet. Other drivers come from Windows Update.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_installed_games",
            "description": "Games installed on this PC through Steam and the Epic Games Launcher, with size and the result of the last DxTool game check if there was one.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "check_game",
            "description": "Compares a game's minimum and recommended PC requirements (from Steam's store) with this PC's processor, graphics, memory, free space and Windows version. Works for any game sold on Steam, installed or not. Needs the internet the first time a game is checked.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "game": { "type": "string", "description": "Game name (e.g. 'Cyberpunk 2077') or Steam app id." }
                },
                "required": ["game"]
            }
        },
        {
            "name": "get_hardware_inventory",
            "description": "Detailed hardware and system inventory: every device with manufacturer, model, driver, connection and more. Takes a few seconds. Optionally limit to one category.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": { "type": "string", "description": "Category id or title, e.g. 'cpu', 'memory', 'storage', 'graphics', 'network', 'motherboard'. Omit for a summary of everything." },
                    "full_detail": { "type": "boolean", "description": "Include every detail section (long). Default false unless a category is given." }
                }
            }
        },
        {
            "name": "query_event_log",
            "description": "Reads Windows event log entries, newest first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "log": { "type": "string", "description": "Channel name: System (default), Application, Setup, or e.g. Microsoft-Windows-Kernel-PnP/Configuration." },
                    "level": { "type": "string", "enum": ["errors", "warnings", "all"], "description": "errors = critical+error, warnings = also warnings (default)." },
                    "days": { "type": "integer", "description": "How far back, 1-90 (default 7)." },
                    "event_id": { "type": "integer" },
                    "provider": { "type": "string", "description": "Event source name, e.g. 'disk' or 'Microsoft-Windows-WHEA-Logger'." },
                    "max": { "type": "integer", "description": "Maximum entries, up to 200 (default 40)." }
                }
            }
        },
        {
            "name": "get_antivirus_status",
            "description": "Microsoft Defender and ClamAV status (definitions age, real-time protection) plus recent DxTool virus scans and their detections.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_dxtool_fixes",
            "description": "The repairs available in DxTool's Fixes section (with risk level and what each changes), and the repairs already run on this PC.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

/// Serves MCP over stdin/stdout until the client disconnects.
pub fn serve(app_version: &str) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let out: Vec<Value> = batch.iter().filter_map(|m| handle(m, app_version)).collect();
                (!out.is_empty()).then(|| Value::Array(out))
            }
            Ok(msg) => handle(&msg, app_version),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Parse error: {e}") } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(stdout, "{r}");
            let _ = stdout.flush();
        }
    }
}

fn handle(msg: &Value, app_version: &str) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    // Notifications (no id) never get a reply.
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if PROTOCOLS.contains(&asked) { asked } else { PROTOCOLS[0] };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "dxtool", "title": "DxTool", "version": app_version },
                "instructions": INSTRUCTIONS
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let outcome = std::panic::catch_unwind(|| call_tool(&name, &args)).unwrap_or_else(|_| Err("The tool stopped unexpectedly.".into()));
            Ok(match outcome {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            })
        }
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}
fn arg_bool(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// Runs a tool and returns its text result.
pub fn call_tool(name: &str, a: &Value) -> Result<String, String> {
    match name {
        "get_pc_overview" => Ok(overview()),
        "run_health_scan" => {
            let kind = match arg_str(a, "kind") {
                Some("full") => "full",
                _ => "fast",
            };
            let mut r = crate::health::run_scan(kind, &[], &|_| {});
            r.saved = crate::history::save(&r).is_ok();
            Ok(scan_text(&r, arg_bool(a, "include_details")))
        }
        "get_latest_health_scan" => {
            let id = arg_str(a, "machine_id").map(String::from).unwrap_or_else(|| crate::machine::collect().id);
            let rec = crate::history::machines().into_iter().find(|m| m.info.id == id).ok_or("No saved scans for that PC. Use run_health_scan.")?;
            let latest = rec.scans.iter().max_by(|x, y| x.started_at.cmp(&y.started_at)).ok_or("No saved scans for that PC. Use run_health_scan.")?;
            load_scan(&rec.info.id, &latest.id, arg_bool(a, "include_details"))
        }
        "get_saved_scan" => load_scan(arg_str(a, "machine_id").ok_or("machine_id is required")?, arg_str(a, "scan_id").ok_or("scan_id is required")?, arg_bool(a, "include_details")),
        "list_machines" => Ok(machines_text()),
        "check_newest_drivers" => Ok(drivers_text(&crate::latest_drivers::check())),
        "list_installed_games" => Ok(games_text()),
        "check_game" => {
            let g = arg_str(a, "game").ok_or("Missing 'game'.")?;
            crate::games::check_by_name(g).map(|c| game_check_text(&c))
        }
        "get_upgrade_recommendations" => Ok(upgrades_text(&crate::upgrades::for_this_pc())),
        "get_hardware_inventory" => {
            let inv = crate::upgrades::inventory();
            let cat = arg_str(a, "category");
            let full = a.get("full_detail").and_then(Value::as_bool).unwrap_or(cat.is_some());
            inventory_text(&inv, cat, full)
        }
        "query_event_log" => events_text(a),
        "get_antivirus_status" => Ok(antivirus_text()),
        "list_dxtool_fixes" => Ok(fixes_text()),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn sev(s: Status) -> &'static str {
    match s {
        Status::Error => "Problem",
        Status::Warn => "Warning",
        Status::Info => "Note",
        Status::Ok => "OK",
    }
}

fn local(rfc: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|_| rfc.to_string())
}

fn fix_title(id: &str) -> String {
    crate::fixes::catalog().iter().find(|f| f.id == id).map(|f| f.title.to_string()).unwrap_or_else(|| id.to_string())
}

fn scan_text(r: &ScanReport, details: bool) -> String {
    let mut s = format!(
        "Health scan ({}) of {} at {}: score {}/100 ({}). {} problem(s), {} warning(s), {} note(s).{}\nScan id: {} · PC id: {}\n",
        r.kind,
        r.computer_name,
        local(&r.started_at),
        r.score,
        r.grade,
        r.counts.error,
        r.counts.warn,
        r.counts.info,
        if r.is_admin { "" } else { " DxTool was not running as administrator, so a few checks were limited." },
        r.id,
        r.machine.id
    );
    let mut findings: Vec<_> = r.results.iter().flat_map(|c| c.findings.iter().map(move |f| (c, f))).collect();
    findings.sort_by_key(|(_, f)| match f.severity {
        Status::Error => 0,
        Status::Warn => 1,
        Status::Info => 2,
        Status::Ok => 3,
    });
    if findings.is_empty() {
        s.push_str("\nNo findings: everything checked looks healthy.\n");
    } else {
        s.push_str("\nFindings:\n");
        for (c, f) in findings {
            s.push_str(&format!("- [{}] {} (check: {})\n  {}\n", sev(f.severity), f.title, c.title, f.detail));
            if let Some(a) = &f.advice {
                s.push_str(&format!("  What to do: {a}\n"));
            }
            if let Some(fx) = &f.fix {
                s.push_str(&format!("  DxTool fix: {}\n", fix_title(fx)));
            }
            let ev: Vec<String> = f.evidence.iter().filter(|e| !e.display.is_empty()).map(|e| format!("{}: {}", e.label, e.display)).collect();
            if !ev.is_empty() {
                s.push_str(&format!("  Evidence: {}\n", ev.join("; ")));
            }
        }
    }
    s.push_str("\nChecks:\n");
    for c in &r.results {
        s.push_str(&format!("- {} [{}]: {}", c.title, sev(c.status), c.summary));
        if let Some(l) = &c.limited {
            s.push_str(&format!(" (limited: {l})"));
        }
        if let Some(e) = &c.error {
            s.push_str(&format!(" (error: {e})"));
        }
        s.push('\n');
        if details {
            for f in &c.facts {
                s.push_str(&format!("    {}: {}\n", f.label, f.display));
            }
            for it in c.items.iter().take(30) {
                let sum: Vec<String> = it.summary.iter().map(|f| format!("{}: {}", f.label, f.display)).collect();
                s.push_str(&format!("    • {} {} {}\n", it.name, it.subtitle, sum.join("; ")));
            }
            if c.items.len() > 30 {
                s.push_str(&format!("    … and {} more rows\n", c.items.len() - 30));
            }
        }
    }
    s
}

fn load_scan(machine_id: &str, scan_id: &str, details: bool) -> Result<String, String> {
    let v = crate::history::load(machine_id, scan_id)?;
    let r: ScanReport = serde_json::from_value(v).map_err(|e| format!("Could not read that scan: {e}"))?;
    Ok(scan_text(&r, details))
}

fn machines_text() -> String {
    let list = crate::history::machines();
    if list.is_empty() {
        return "No PCs in DxTool's history yet. Use run_health_scan.".into();
    }
    let mut s = String::new();
    for m in list {
        s.push_str(&format!("{} (PC id {}): {} {}\n", m.info.computer_name, m.info.id, m.info.manufacturer, m.info.model));
        let mut scans = m.scans.clone();
        scans.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        for sc in scans.iter().take(25) {
            s.push_str(&format!("  - {} · {} scan · score {} ({}) · {}P/{}W · scan id {}\n", local(&sc.started_at), sc.kind, sc.score, sc.grade, sc.counts.error, sc.counts.warn, sc.id));
        }
        if scans.len() > 25 {
            s.push_str(&format!("  … {} older scans\n", scans.len() - 25));
        }
    }
    s
}

fn overview() -> String {
    let m = crate::machine::collect();
    let rows = [
        ("Computer name", m.computer_name.clone()),
        ("PC id (for history tools)", m.id.clone()),
        ("Manufacturer", m.manufacturer.clone()),
        ("Model", m.model.clone()),
        ("Serial number", m.serial.clone()),
        ("Windows", m.windows.clone()),
        ("Processor", m.cpu.clone()),
        ("Memory", m.memory.clone()),
        ("Graphics", m.graphics.join(", ")),
        ("Drives", m.drives.join("; ")),
        ("Motherboard", m.motherboard.clone()),
        ("BIOS", m.bios.clone()),
    ];
    let mut s: String = rows.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}: {v}\n")).collect();
    s.insert_str(0, "This PC\n");
    s.push_str(&format!("Running as administrator: {}\n", crate::fmt::yes_no(crate::platform::is_admin())));
    match crate::history::machines().into_iter().find(|r| r.info.id == m.id).and_then(|r| r.scans.into_iter().max_by(|a, b| a.started_at.cmp(&b.started_at)).map(|sc| (r.info.id.clone(), sc))) {
        Some((mid, sc)) => {
            s.push_str(&format!("\nLatest health scan: {} ({} scan), score {} ({})\n", local(&sc.started_at), sc.kind, sc.score, sc.grade));
            if let Ok(v) = crate::history::load(&mid, &sc.id) {
                if let Ok(r) = serde_json::from_value::<ScanReport>(v) {
                    let top: Vec<String> = r
                        .results
                        .iter()
                        .flat_map(|c| c.findings.iter())
                        .filter(|f| matches!(f.severity, Status::Error | Status::Warn))
                        .map(|f| format!("- [{}] {}", sev(f.severity), f.title))
                        .collect();
                    if top.is_empty() {
                        s.push_str("No problems or warnings in that scan.\n");
                    } else {
                        s.push_str(&top.join("\n"));
                        s.push('\n');
                    }
                }
            }
        }
        None => s.push_str("\nNo health scan saved yet for this PC. Use run_health_scan.\n"),
    }
    s
}

fn inventory_text(inv: &Inventory, cat: Option<&str>, full: bool) -> Result<String, String> {
    let cats: Vec<_> = match cat {
        Some(c) => {
            let c = c.to_lowercase();
            let v: Vec<_> = inv.categories.iter().filter(|x| x.id.to_lowercase() == c || x.title.to_lowercase().contains(&c)).collect();
            if v.is_empty() {
                let names: Vec<String> = inv.categories.iter().map(|x| format!("{} ({})", x.id, x.title)).collect();
                return Err(format!("No category matches '{c}'. Categories: {}", names.join(", ")));
            }
            v
        }
        None => inv.categories.iter().collect(),
    };
    let mut s = format!("Hardware inventory of {} (collected {})\n", inv.computer_name, local(&inv.collected_at));
    for c in cats {
        s.push_str(&format!("\n## {} [{}]\n", c.title, c.id));
        if let Some(e) = &c.error {
            s.push_str(&format!("(error: {e})\n"));
        }
        for it in &c.items {
            let st = it.status_text.clone().unwrap_or_default();
            s.push_str(&format!("- {}{}{}\n", it.name, if it.subtitle.is_empty() { String::new() } else { format!(" — {}", it.subtitle) }, if st.is_empty() { String::new() } else { format!(" [{st}]") }));
            let sum: Vec<String> = it.summary.iter().map(|f| format!("{}: {}", f.label, f.display)).collect();
            if !sum.is_empty() {
                s.push_str(&format!("  {}\n", sum.join("; ")));
            }
            if full {
                for sec in it.sections.iter().filter(|x| !x.raw) {
                    let f: Vec<String> = sec.fields.iter().map(|f| format!("{}: {}", f.label, f.display)).collect();
                    if !f.is_empty() {
                        s.push_str(&format!("  {}: {}\n", sec.title, f.join("; ")));
                    }
                }
            }
        }
    }
    Ok(s)
}

fn events_text(a: &Value) -> Result<String, String> {
    use crate::events;
    let log = arg_str(a, "log").unwrap_or("System");
    if log.contains(['\'', '"', '<', '>']) {
        return Err("Invalid log name.".into());
    }
    let days = a.get("days").and_then(Value::as_u64).unwrap_or(7).clamp(1, 90) as u32;
    let max = a.get("max").and_then(Value::as_u64).unwrap_or(40).clamp(1, 200) as usize;
    let mut parts = vec![events::within_days(days)];
    match arg_str(a, "level").unwrap_or("warnings") {
        "errors" => parts.push("(Level=1 or Level=2)".into()),
        "all" => {}
        _ => parts.push("(Level=1 or Level=2 or Level=3)".into()),
    }
    if let Some(id) = a.get("event_id").and_then(Value::as_u64) {
        parts.push(format!("EventID={id}"));
    }
    if let Some(p) = arg_str(a, "provider") {
        if p.contains(['\'', '"', '<', '>', ']', '[']) {
            return Err("Invalid provider name.".into());
        }
        parts.push(events::providers(&[p]));
    }
    let list = events::query(log, &events::system_filter(&parts), max)?;
    if list.is_empty() {
        return Ok(format!("No matching events in the {log} log in the last {days} day(s)."));
    }
    let mut s = format!("{} event(s) from the {log} log, newest first:\n", list.len());
    for e in list {
        let msg: String = e.message.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(600).collect();
        s.push_str(&format!("- {} · {} · {} · ID {} · {}\n", e.local_time(), e.level_text(), e.provider, e.id, if msg.is_empty() { e.short_message() } else { msg }));
    }
    Ok(s)
}

fn antivirus_text() -> String {
    let st = crate::av::status();
    let d = &st.defender;
    let mut s = String::from("Microsoft Defender: ");
    if d.available {
        s.push_str(&format!(
            "available, real-time protection {}, definitions {} ({}), last quick scan {}, last full scan {}\n",
            if d.realtime { "on" } else { "OFF" },
            d.signature_version,
            d.signature_age_days.map(|n| format!("{n} day(s) old")).unwrap_or_else(|| "age unknown".into()),
            d.quick_scan_age_days.map(|n| format!("{n} day(s) ago")).unwrap_or_else(|| "never".into()),
            d.full_scan_age_days.map(|n| format!("{n} day(s) ago")).unwrap_or_else(|| "never".into()),
        ));
    } else {
        s.push_str(&format!("not available for scanning: {}\n", d.reason));
    }
    if !d.other_antivirus.is_empty() {
        s.push_str(&format!("Other antivirus registered: {}\n", d.other_antivirus.join(", ")));
    }
    let c = &st.clamav;
    s.push_str(&if !c.installed {
        "ClamAV: not downloaded (optional second-opinion scanner in DxTool's Virus scan page).\n".to_string()
    } else if !c.databases {
        format!("ClamAV {}: downloaded, but its virus databases aren't.\n", c.version)
    } else {
        format!("ClamAV {}: databases version {} ({} old, {} signatures)\n", c.version, c.db_version, c.db_age_days.map(|n| format!("{n} day(s)")).unwrap_or_default(), c.signatures)
    });
    let scans = crate::av::scans(&crate::machine::collect().id);
    if scans.is_empty() {
        s.push_str("\nNo virus scans run from DxTool yet. The user can run them from the Virus scan page.\n");
    } else {
        s.push_str("\nRecent DxTool virus scans:\n");
        for v in scans.iter().take(10) {
            s.push_str(&format!("- {} · {} {} scan · {} · {}\n", local(&v.started_at), v.engine, v.kind, v.outcome, v.message));
            for t in &v.threats {
                s.push_str(&format!("    • {} in {} — {}\n", t.name, t.path, t.action));
            }
        }
    }
    s
}

fn fixes_text() -> String {
    let mut s = String::from("Repairs in DxTool's Fixes section (the user runs these in the app; a restore point is made first):\n");
    for f in crate::fixes::catalog() {
        s.push_str(&format!("- {} [{} risk{}]: {} Changes: {}\n", f.title, f.risk, if f.needs_admin { ", needs administrator" } else { "" }, f.description, f.changes));
    }
    let runs = crate::history::fix_runs(&crate::machine::collect().id);
    if !runs.is_empty() {
        s.push_str("\nRepairs already run on this PC:\n");
        for r in runs.iter().take(10) {
            let steps: Vec<String> = r.steps.iter().map(|x| format!("{} ({}: {})", x.title, x.status, x.summary)).collect();
            s.push_str(&format!("- {}: {}\n", local(&r.started_at), steps.join("; ")));
        }
    }
    s
}

// ───────────────────────── Setting up Claude ─────────────────────────

/// Where Claude Desktop keeps its settings (standard and Microsoft Store installs).
fn claude_desktop_configs() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        v.push(std::path::PathBuf::from(appdata).join("Claude").join("claude_desktop_config.json"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        for e in std::fs::read_dir(std::path::PathBuf::from(local).join("Packages")).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().starts_with("Claude_") {
                v.push(e.path().join(r"LocalCache\Roaming\Claude\claude_desktop_config.json"));
            }
        }
    }
    v
}

fn claude_desktop_installed() -> bool {
    let local = std::env::var("LOCALAPPDATA").map(std::path::PathBuf::from).ok();
    local.as_ref().is_some_and(|l| l.join("AnthropicClaude").is_dir() || l.join("Programs").join("Claude").is_dir())
        || claude_desktop_configs().iter().any(|p| p.parent().is_some_and(|d| d.is_dir()))
}

/// The command Claude should run to start DxTool's MCP server.
pub fn server_command() -> (String, Vec<String>) {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    (exe, vec!["--mcp".into()])
}

/// Whether Claude Desktop is installed and set up to use this copy of DxTool.
pub fn claude_status() -> Value {
    let (exe, _) = server_command();
    let mut state = "not_set_up";
    let mut configured = String::new();
    let mut config_path = String::new();
    for p in claude_desktop_configs() {
        if !p.parent().is_some_and(|d| d.is_dir()) {
            continue;
        }
        if config_path.is_empty() {
            config_path = p.to_string_lossy().into();
        }
        let Ok(v) = std::fs::read(&p).map_err(|_| ()).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|_| ())) else { continue };
        if let Some(cmd) = v.pointer("/mcpServers/dxtool/command").and_then(Value::as_str) {
            config_path = p.to_string_lossy().into();
            configured = cmd.into();
            state = if cmd.eq_ignore_ascii_case(&exe) { "set_up" } else { "moved" };
            break;
        }
    }
    let code_cmd = format!("claude mcp add --scope user dxtool -- \"{exe}\" --mcp");
    json!({
        "desktopInstalled": claude_desktop_installed(),
        "state": state,
        "configuredCommand": configured,
        "configPath": config_path,
        "exe": exe,
        "claudeCodeCommand": code_cmd,
    })
}

/// Removes DxTool from Claude Desktop only if it points to this copy. Used by the uninstaller,
/// so uninstalling doesn't break a connection made by another (e.g. portable) copy.
pub fn claude_remove_if_mine() -> Result<String, String> {
    if claude_status().get("state").and_then(Value::as_str) == Some("set_up") {
        claude_setup(true)
    } else {
        Ok("Claude Desktop isn't connected to this copy of DxTool.".into())
    }
}

/// Adds (or updates) DxTool in Claude Desktop's settings. A backup is kept.
pub fn claude_setup(remove: bool) -> Result<String, String> {
    let configs = claude_desktop_configs();
    let target = configs
        .iter()
        .find(|p| p.is_file())
        .or_else(|| configs.iter().find(|p| p.parent().is_some_and(|d| d.is_dir())))
        .cloned()
        .or_else(|| configs.first().cloned())
        .ok_or("Couldn't find where Claude Desktop keeps its settings.")?;
    let mut v: Value = match std::fs::read(&target) {
        Ok(b) if !b.iter().all(u8::is_ascii_whitespace) => serde_json::from_slice(&b).map_err(|_| format!("Claude Desktop's settings file isn't valid JSON, so DxTool left it alone: {}", target.display()))?,
        _ => json!({}),
    };
    if !v.is_object() {
        return Err("Claude Desktop's settings file has an unexpected format, so DxTool left it alone.".into());
    }
    if target.is_file() {
        let _ = std::fs::copy(&target, target.with_extension("json.dxtool-backup"));
    }
    let obj = v.as_object_mut().unwrap();
    let servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        return Err("Claude Desktop's mcpServers setting has an unexpected format, so DxTool left it alone.".into());
    }
    if remove {
        servers.as_object_mut().unwrap().remove("dxtool");
    } else {
        let (exe, args) = server_command();
        servers.as_object_mut().unwrap().insert("dxtool".into(), json!({ "command": exe, "args": args }));
    }
    if let Some(d) = target.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save Claude Desktop's settings: {e}"))?;
    Ok(if remove {
        "DxTool was removed from Claude Desktop. Quit and reopen Claude Desktop to finish.".into()
    } else {
        "DxTool was added to Claude Desktop. Quit Claude Desktop completely (right-click its tray icon → Quit) and open it again.".into()
    })
}

fn upgrades_text(r: &crate::model::UpgradeReport) -> String {
    let mut out = format!("Upgrade recommendations for {} ({})\n", r.pc_name, r.pc_kind);
    if r.items.is_empty() {
        out.push_str("\nNo upgrades needed: nothing stands out.\n");
    }
    for (i, u) in r.items.iter().enumerate() {
        out.push_str(&format!("\n{}. [{} priority · {} · {}] {}\n   Why: {}\n   How: {}\n", i + 1, u.priority, u.area, u.cost, u.title, u.why, u.how));
        if let Some(s) = &u.search {
            out.push_str(&format!("   Part to search for: {s}\n"));
        }
    }
    if !r.good.is_empty() {
        out.push_str("\nAlready fine:\n");
        for g in &r.good {
            out.push_str(&format!("- {g}\n"));
        }
    }
    if let Some(l) = &r.limited {
        out.push_str(&format!("\nNote: {l}\n"));
    }
    out
}

fn games_text() -> String {
    let games = crate::games::installed();
    if games.is_empty() {
        return "No Steam or Epic Games Launcher games found on this PC.".into();
    }
    let mut out = format!("{} installed games:\n", games.len());
    for g in games {
        out.push_str(&format!(
            "- {} ({}, {:.1} GB){}{}\n",
            g.name,
            g.launcher,
            g.size_bytes as f64 / 1e9,
            g.steam_app_id.map(|id| format!(", Steam app {id}")).unwrap_or_default(),
            g.last_verdict.map(|v| format!(", last check: {v}")).unwrap_or_default()
        ));
    }
    out
}

fn game_check_text(c: &crate::games::GameCheck) -> String {
    let mut out = format!("{} (Steam app {}): {}\n{}\n", c.name, c.app_id, c.overall, c.summary);
    for p in &c.parts {
        out.push_str(&format!("\n{} [{}]\n  This PC: {}\n  Minimum: {}\n  Recommended: {}\n", p.part, p.verdict, p.yours, p.minimum, p.recommended));
        if !p.note.is_empty() {
            out.push_str(&format!("  Note: {}\n", p.note));
        }
    }
    out.push_str("\nProcessor and graphics comparisons use rough benchmark scores; memory and storage are exact.\n");
    out
}

fn drivers_text(c: &crate::latest_drivers::DriverCheck) -> String {
    let mut out = String::new();
    if c.nothing_to_check {
        out.push_str("This PC has no NVIDIA, Intel or AMD graphics, Wi-Fi, Bluetooth or Ethernet devices that DxTool can look up. Windows Update covers the rest.\n");
    }
    for d in &c.items {
        out.push_str(&format!("\n{} ({} {}) [{}]\n  {}\n  Installed: {} ({})\n", d.device, d.vendor, d.kind, d.status, d.summary, d.installed_version, d.installed_date));
        if !d.latest_version.is_empty() {
            out.push_str(&format!("  Newest: {} {} ({})\n", d.latest_name, d.latest_version, d.latest_date));
        }
        if !d.download_url.is_empty() {
            out.push_str(&format!("  Download: {}\n", d.download_url));
        }
        if !d.details_url.is_empty() {
            out.push_str(&format!("  Page: {}\n", d.details_url));
        }
        if !d.note.is_empty() {
            out.push_str(&format!("  Note: {}\n", d.note));
        }
    }
    for p in &c.problems {
        out.push_str(&format!("\nProblem: {p}\n"));
    }
    out.push_str("\nOther drivers (chipset, audio, printers and so on) come from Windows Update: DxTool's 'Install driver updates' repair checks those.\n");
    out
}
