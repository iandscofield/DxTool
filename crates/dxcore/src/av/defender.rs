//! Microsoft Defender scans through MpCmdRun.exe, with results read back from Defender's
//! own detection history so we know what it did with each threat.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::fixes::runner::{self, powershell};
use crate::model::{AvEvent, DefenderStatus, Threat, VirusScan};

/// The newest MpCmdRun.exe (platform updates install side by side under ProgramData).
pub fn mpcmdrun() -> Option<PathBuf> {
    let platform = std::env::var("ProgramData").map(|p| PathBuf::from(p).join(r"Microsoft\Windows Defender\Platform")).ok();
    let mut best: Option<(Vec<u64>, PathBuf)> = None;
    if let Some(dir) = platform {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let exe = e.path().join("MpCmdRun.exe");
            if !exe.is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let ver: Vec<u64> = name.split(['.', '-']).filter_map(|p| p.parse().ok()).collect();
            if best.as_ref().is_none_or(|(b, _)| ver > *b) {
                best = Some((ver, exe));
            }
        }
    }
    best.map(|(_, p)| p).or_else(|| {
        let p = PathBuf::from(std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())).join(r"Windows Defender\MpCmdRun.exe");
        p.is_file().then_some(p)
    })
}

/// Reads the JSON a PowerShell script printed after a "DXJSON" marker.
fn dxjson(lines: &[String]) -> Option<Value> {
    lines.iter().rev().find_map(|l| l.strip_prefix("DXJSON")).and_then(|j| serde_json::from_str(j).ok())
}

fn age(v: &Value) -> Option<i64> {
    v.as_i64().filter(|d| *d >= 0 && *d < 100_000)
}

pub fn status() -> DefenderStatus {
    let script = r#"
$o = @{}
try {
  $s = Get-MpComputerStatus -ErrorAction Stop
  $o.mode = [string]$s.AMRunningMode; $o.av = [bool]$s.AntivirusEnabled; $o.rt = [bool]$s.RealTimeProtectionEnabled
  $o.prod = [string]$s.AMProductVersion; $o.eng = [string]$s.AMEngineVersion; $o.sig = [string]$s.AntivirusSignatureVersion
  if ($s.AntivirusSignatureLastUpdated) { $o.sigdate = $s.AntivirusSignatureLastUpdated.ToString('o') }
  $o.sigage = [long]$s.AntivirusSignatureAge; $o.qage = [long]$s.QuickScanAge; $o.fage = [long]$s.FullScanAge
} catch { $o.err = $_.Exception.Message }
try { $o.others = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct -ErrorAction Stop | Where-Object { $_.displayName -notmatch 'Defender' } | ForEach-Object { [string]$_.displayName }) } catch { $o.others = @() }
'DXJSON' + ($o | ConvertTo-Json -Compress)
"#;
    let mut st = DefenderStatus::default();
    let lines = powershell(script, Duration::from_secs(60), &|_| {}).map(|(_, l)| l).unwrap_or_default();
    let Some(v) = dxjson(&lines) else {
        st.reason = "Couldn't read Microsoft Defender's status.".into();
        return st;
    };
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    st.other_antivirus = v.get("others").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    if !s("err").is_empty() {
        st.reason = if st.other_antivirus.is_empty() {
            "Microsoft Defender isn't running on this PC.".into()
        } else {
            format!("{} is protecting this PC instead of Microsoft Defender, so Defender scans aren't available. Use ClamAV for a second opinion.", st.other_antivirus.join(", "))
        };
        return st;
    }
    st.running_mode = s("mode");
    st.realtime = v.get("rt").and_then(Value::as_bool).unwrap_or(false);
    st.product_version = s("prod");
    st.engine_version = s("eng");
    st.signature_version = s("sig");
    st.signature_updated = s("sigdate");
    st.signature_age_days = v.get("sigage").and_then(age);
    st.quick_scan_age_days = v.get("qage").and_then(age);
    st.full_scan_age_days = v.get("fage").and_then(age);
    let enabled = v.get("av").and_then(Value::as_bool).unwrap_or(false);
    let normal = st.running_mode.eq_ignore_ascii_case("Normal") || st.running_mode.is_empty();
    st.available = mpcmdrun().is_some() && (enabled || normal);
    if !st.available {
        st.reason = if !st.other_antivirus.is_empty() {
            format!(
                "{} is protecting this PC, so Microsoft Defender is switched off ({}). Use ClamAV for a second opinion.",
                st.other_antivirus.join(", "),
                if st.running_mode.is_empty() { "disabled" } else { &st.running_mode }
            )
        } else if mpcmdrun().is_none() {
            "Microsoft Defender's scanner (MpCmdRun.exe) wasn't found.".into()
        } else {
            "Microsoft Defender's antivirus is turned off.".into()
        };
    }
    st
}

/// Downloads Defender's latest virus definitions from Microsoft.
pub fn update(on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<String, String> {
    let _busy = super::BusyGuard::acquire()?;
    let exe = mpcmdrun().ok_or("Microsoft Defender's scanner wasn't found.")?;
    on_event(AvEvent::Phase { text: "Updating Microsoft Defender's virus definitions".into() });
    let emit = |l: &str| on_event(AvEvent::Output { line: l.into() });
    let (code, lines) = runner::run_cancellable(&exe.to_string_lossy(), &["-SignatureUpdate"], None, Duration::from_secs(1800), Some(&super::CANCEL), &emit)?;
    if code == 0 {
        let ver = lines.iter().find_map(|l| l.strip_prefix("AntiVirus Signature Version:")).map(|s| s.trim().to_string()).unwrap_or_default();
        Ok(if ver.is_empty() { "Definitions are up to date.".into() } else { format!("Definitions are up to date (version {ver}).") })
    } else {
        Err(failure_text(&lines, code, "update the definitions"))
    }
}

fn failure_text(lines: &[String], code: i32, doing: &str) -> String {
    let hr = lines.iter().find(|l| l.contains("0x")).cloned().unwrap_or_default();
    let hint = if hr.contains("0x80070005") {
        " Access was denied. Restart DxTool as administrator and try again."
    } else if hr.contains("0x800106ba") || hr.contains("0x80508023") {
        " Defender's antivirus service isn't running, usually because another antivirus is installed."
    } else if hr.contains("0x8050800c") || hr.contains("0x80240438") || hr.contains("0x8024402c") {
        " Defender couldn't reach Microsoft. Check the internet connection."
    } else {
        ""
    };
    format!("Defender couldn't {doing} (code {code}).{hint}{}", if hr.is_empty() { String::new() } else { format!(" Details: {hr}") })
}

fn status_of(id: i64) -> (&'static str, &'static str) {
    match id {
        2 => ("removed", "Cleaned by Defender"),
        3 => ("quarantined", "Quarantined by Defender"),
        4 => ("removed", "Removed by Defender"),
        5 => ("allowed", "Allowed: someone chose to keep it"),
        6 => ("removed", "Blocked by Defender"),
        102 | 103 | 104 | 107 => ("failed", "Defender couldn't remove it"),
        _ => ("active", "Found, not yet removed"),
    }
}

fn severity(id: i64) -> &'static str {
    match id {
        1 => "Low",
        2 => "Moderate",
        4 => "High",
        5 => "Severe",
        _ => "",
    }
}

/// Defender lists resources as "file:_C:\path", "process:_pid:...", etc.
fn resource_path(res: &[String]) -> String {
    let pick = res
        .iter()
        .find(|r| r.starts_with("file:_") || r.starts_with("containerfile:_"))
        .or_else(|| res.first())
        .cloned()
        .unwrap_or_default();
    match pick.split_once(":_") {
        Some((_, rest)) => rest.to_string(),
        None => pick,
    }
}

/// Detections Defender recorded since the scan started.
fn detections_since(start: &str) -> Vec<Threat> {
    let script = format!(
        r#"
$since = [datetime]::Parse('{start}')
$names = @{{}}
try {{ Get-MpThreat -ErrorAction Stop | ForEach-Object {{ $names[[string]$_.ThreatID] = @([string]$_.ThreatName, [int]$_.SeverityID) }} }} catch {{}}
$r = @(Get-MpThreatDetection -ErrorAction SilentlyContinue | Where-Object {{ $_.InitialDetectionTime -ge $since -or $_.LastThreatStatusChangeTime -ge $since }} | ForEach-Object {{
  $n = $names[[string]$_.ThreatID]
  [pscustomobject]@{{ name = $(if ($n) {{ $n[0] }} else {{ 'Threat ' + $_.ThreatID }}); sev = $(if ($n) {{ $n[1] }} else {{ 0 }}); status = [int]$_.ThreatStatusID; res = @($_.Resources | ForEach-Object {{ [string]$_ }}) }}
}})
'DXJSON' + (ConvertTo-Json -InputObject $r -Compress -Depth 4)
"#
    );
    let lines = powershell(&script, Duration::from_secs(120), &|_| {}).map(|(_, l)| l).unwrap_or_default();
    let Some(Value::Array(items)) = dxjson(&lines) else { return vec![] };
    let mut out = Vec::new();
    for d in &items {
        let res: Vec<String> = d.get("res").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let (status, action) = status_of(d.get("status").and_then(Value::as_i64).unwrap_or(0));
        // One detection can cover several files; list each one.
        let mut paths: Vec<String> = res
            .iter()
            .filter(|r| r.starts_with("file:_") || r.starts_with("containerfile:_"))
            .map(|r| resource_path(std::slice::from_ref(r)))
            .collect();
        if paths.is_empty() {
            paths.push(resource_path(&res));
        }
        for path in paths {
            out.push(Threat {
                name: d.get("name").and_then(Value::as_str).unwrap_or("").into(),
                path,
                engine: "defender".into(),
                severity: severity(d.get("sev").and_then(Value::as_i64).unwrap_or(0)).into(),
                status: status.into(),
                action: action.into(),
            });
        }
    }
    out
}

/// Threats listed in MpCmdRun's own output ("Threat : name" then "file : path").
fn parse_output(lines: &[String]) -> Vec<Threat> {
    let mut v: Vec<Threat> = Vec::new();
    for l in lines {
        let Some((k, val)) = l.split_once(':') else { continue };
        let (k, val) = (k.trim(), val.trim());
        if k == "Threat" {
            v.push(Threat { name: val.into(), engine: "defender".into(), status: "active".into(), action: "Found by Defender. Check Windows Security for what happened to it.".into(), ..Default::default() });
        } else if (k == "file" || k == "containerfile") && v.last().is_some_and(|t| t.path.is_empty()) {
            v.last_mut().unwrap().path = val.into();
        }
    }
    v
}

pub fn scan(scan: &mut VirusScan, paths: &[String], on_event: &(dyn Fn(AvEvent) + Sync)) {
    let st = status();
    scan.signature_version = st.signature_version.clone();
    if !st.available {
        scan.outcome = "failed".into();
        scan.message = st.reason;
        return;
    }
    let exe = mpcmdrun().unwrap().to_string_lossy().to_string();
    let runs: Vec<(Vec<String>, String, u64)> = match scan.kind.as_str() {
        "full" => vec![(vec!["-Scan".into(), "-ScanType".into(), "2".into()], "Full scan of every drive (this can take an hour or more)".into(), 24 * 3600)],
        "custom" => paths
            .iter()
            .map(|p| (vec!["-Scan".into(), "-ScanType".into(), "3".into(), "-File".into(), p.clone()], format!("Scanning {p}"), 12 * 3600))
            .collect(),
        _ => vec![(vec!["-Scan".into(), "-ScanType".into(), "1".into()], "Quick scan of the places malware usually hides".into(), 4 * 3600)],
    };
    scan.targets = match scan.kind.as_str() {
        "custom" => paths.to_vec(),
        "full" => vec!["All drives".into()],
        _ => vec!["Memory, startup items and common malware locations".into()],
    };
    // Defender's detection times have one-second precision; start a little early.
    let since = (chrono::Local::now() - chrono::Duration::seconds(5)).to_rfc3339();
    let mut all_lines = Vec::new();
    for (i, (args, title, secs)) in runs.iter().enumerate() {
        on_event(AvEvent::Phase { text: title.clone() });
        on_event(AvEvent::Progress { done: i as u64, total: runs.len() as u64, current: String::new() });
        let emit = |l: &str| on_event(AvEvent::Output { line: l.into() });
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        match runner::run_cancellable(&exe, &a, None, Duration::from_secs(*secs), Some(&super::CANCEL), &emit) {
            Ok((code, lines)) => {
                all_lines.extend(lines.iter().cloned());
                // 0 = clean, 2 = threats found; anything else is a failure.
                if code != 0 && code != 2 {
                    scan.outcome = "failed".into();
                    scan.message = failure_text(&lines, code, "finish the scan");
                    break;
                }
            }
            Err(e) => {
                if e == runner::CANCELLED {
                    let _ = runner::run_streaming(&exe, &["-Scan", "-Cancel"], None, Duration::from_secs(60), &|_| {});
                    scan.outcome = "cancelled".into();
                    scan.message = "The scan was stopped before it finished.".into();
                } else {
                    scan.outcome = "failed".into();
                    scan.message = e;
                }
                break;
            }
        }
    }
    scan.log = crate::fixes::tail(&all_lines);
    on_event(AvEvent::Phase { text: "Reading the results from Defender".into() });
    let mut threats = detections_since(&since);
    if threats.is_empty() {
        threats = parse_output(&all_lines);
    }
    for t in &threats {
        on_event(AvEvent::Threat { threat: t.clone() });
    }
    scan.threats = threats;
    if scan.message.is_empty() {
        scan.message = match scan.threats.len() {
            0 => "Defender found no threats.".into(),
            n => format!("Defender found {n} threat{}.", if n == 1 { "" } else { "s" }),
        };
    }
}

/// Asks Defender to deal with every threat it found but hasn't removed yet, waits for it to
/// finish, and updates the saved scan with what happened.
pub fn remove_active(machine_id: &str, scan_id: &str) -> Result<VirusScan, String> {
    let mut scan = super::scans(machine_id).into_iter().find(|s| s.id == scan_id).ok_or("That virus scan is no longer in the history.")?;
    let script = "$ErrorActionPreference='Stop'; Remove-MpThreat; 'done'";
    match powershell(script, Duration::from_secs(600), &|_| {})? {
        (0, _) => {}
        (_, lines) => {
            let text = lines.join(" ");
            return Err(if text.contains("0x80070005") || text.to_lowercase().contains("denied") {
                "Defender needs administrator rights to remove these. Restart DxTool as administrator and try again.".into()
            } else {
                format!("Defender couldn't remove the threats: {}", text.chars().take(300).collect::<String>())
            });
        }
    }
    let since = chrono::DateTime::parse_from_rfc3339(&scan.started_at).map(|t| (t - chrono::Duration::seconds(5)).to_rfc3339()).unwrap_or_default();
    // Defender works in the background; give it up to 30 seconds.
    for _ in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let fresh = detections_since(&since);
        for t in scan.threats.iter_mut().filter(|t| t.engine == "defender" && t.status == "active") {
            if let Some(f) = fresh.iter().find(|f| f.path.eq_ignore_ascii_case(&t.path) && f.name == t.name) {
                t.status = f.status.clone();
                t.action = f.action.clone();
            }
        }
        if !scan.threats.iter().any(|t| t.engine == "defender" && t.status == "active") {
            break;
        }
    }
    super::save_scan(&scan)?;
    Ok(scan)
}
