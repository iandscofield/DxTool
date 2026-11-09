//! Newest drivers: compares installed drivers with the newest versions the makers publish.
//!
//! - **NVIDIA** graphics: exact lookup by device ID from NVIDIA's driver service.
//! - **Intel** graphics, Wi-Fi, Bluetooth and Ethernet: matched by device ID against the
//!   catalog Intel's own Driver & Support Assistant downloads.
//! - **AMD** graphics: shows the installed driver and links to AMD's driver page.
//!
//! Everything else is left to Windows Update (Fixes → Install driver updates).
//! Only `check()` goes online, and only when the user asks.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fixes::runner;
use crate::wmiutil::{ns_query, RowExt};

pub const AMD_URL: &str = "https://www.amd.com/en/support/download/drivers.html";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DriverStatus {
    pub device: String,
    /// "NVIDIA", "Intel" or "AMD".
    pub vendor: String,
    /// "Graphics", "Wi-Fi", "Bluetooth", "Ethernet" or "Other".
    pub kind: String,
    pub installed_version: String,
    pub installed_date: String,
    pub latest_version: String,
    pub latest_date: String,
    pub latest_name: String,
    /// "update" (newer one available), "current", "newer" (installed is newer than the
    /// maker's public one) or "unknown".
    pub status: String,
    pub summary: String,
    pub download_url: String,
    pub download_size: u64,
    pub details_url: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DriverCheck {
    pub checked_at: String,
    pub items: Vec<DriverStatus>,
    /// Problems reaching a maker's service (the other makers' results still count).
    pub problems: Vec<String>,
    /// True when nothing was online-checked (no NVIDIA, Intel or AMD devices).
    pub nothing_to_check: bool,
}

// ───────────────────────── devices on this PC ─────────────────────────

#[derive(Debug, Clone)]
struct Device {
    name: String,
    class: String,
    /// Hardware IDs without the bus prefix, upper case: "VEN_10DE&DEV_2782&SUBSYS_…".
    hw_ids: Vec<String>,
    version: String,
    date: String,
}

impl Device {
    fn has(&self, vendor: &str) -> bool {
        self.hw_ids.iter().any(|h| h.starts_with(vendor))
    }
    fn dev_id(&self) -> Option<String> {
        let h = self.hw_ids.first()?;
        let i = h.find("DEV_")?;
        Some(h[i + 4..].chars().take(4).collect::<String>().to_lowercase())
    }
}

fn strip_bus(h: &str) -> String {
    h.split_once('\\').map(|(_, r)| r).unwrap_or(h).to_uppercase()
}

fn devices() -> Vec<Device> {
    let drivers: BTreeMap<String, (String, String)> = ns_query("ROOT\\CIMV2", "SELECT DeviceID, DriverVersion, DriverDate FROM Win32_PnPSignedDriver")
        .iter()
        .filter_map(|d| {
            Some((
                d.s("DeviceID")?.to_uppercase(),
                (
                    d.s("DriverVersion").unwrap_or_default(),
                    d.s("DriverDate").and_then(|x| crate::fmt::wmi_to_naive(&x)).map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_default(),
                ),
            ))
        })
        .collect();
    ns_query("ROOT\\CIMV2", "SELECT Name, PNPClass, PNPDeviceID, HardwareID FROM Win32_PnPEntity")
        .iter()
        .filter_map(|e| {
            let id = e.s("PNPDeviceID")?.to_uppercase();
            if !(id.starts_with("PCI\\") || id.starts_with("USB\\")) {
                return None;
            }
            let (version, date) = drivers.get(&id).cloned().unwrap_or_default();
            if version.is_empty() {
                return None;
            }
            let mut hw_ids: Vec<String> = e.list("HardwareID").iter().map(|h| strip_bus(h)).collect();
            if hw_ids.is_empty() {
                hw_ids.push(strip_bus(&id));
            }
            Some(Device { name: e.s("Name").unwrap_or_default(), class: e.s("PNPClass").unwrap_or_default(), hw_ids, version, date })
        })
        .collect()
}

fn is_laptop() -> bool {
    if !ns_query("ROOT\\CIMV2", "SELECT Name FROM Win32_Battery").is_empty() {
        return true;
    }
    // Chassis types 8–14, 30–32: portable, laptop, notebook, sub-notebook, tablet, convertible, detachable.
    ns_query("ROOT\\CIMV2", "SELECT ChassisTypes FROM Win32_SystemEnclosure")
        .iter()
        .flat_map(|r| r.get("ChassisTypes").and_then(Value::as_array).cloned().unwrap_or_default())
        .filter_map(|v| v.as_u64())
        .any(|t| (8..=14).contains(&t) || (30..=32).contains(&t))
}

fn windows_11() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|k| k.get_value::<String, _>("CurrentBuild"))
        .ok()
        .and_then(|b| b.parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
}

// ───────────────────────── versions ─────────────────────────

fn parts(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(0)).collect()
}

/// Compares dotted version numbers ("32.0.101.9150" > "32.0.101.6862").
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        match a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    Ordering::Equal
}

/// Windows' driver version for an NVIDIA driver → NVIDIA's own number: 32.0.16.2101 → 621.01.
pub fn nvidia_version(windows: &str) -> Option<String> {
    let p: Vec<&str> = windows.split('.').collect();
    if p.len() != 4 {
        return None;
    }
    let digits = format!("{}{:0>4}", p[2], p[3]);
    if digits.len() < 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let last = &digits[digits.len() - 5..];
    Some(format!("{}.{}", last[..3].trim_start_matches('0'), &last[3..]))
}

/// "update" / "current" / "newer" for installed vs latest.
fn status_of(installed: &str, latest: &str) -> &'static str {
    match cmp_versions(installed, latest) {
        Ordering::Less => "update",
        Ordering::Equal => "current",
        Ordering::Greater => "newer",
    }
}

fn months_since(date: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?;
    Some((chrono::Local::now().date_naive() - d).num_days() / 30)
}

// ───────────────────────── online helpers ─────────────────────────

fn data_dir() -> PathBuf {
    crate::history::data_dir().join("drivers")
}

fn system32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

fn curl_err(code: i32, who: &str) -> String {
    match code {
        6 | 7 | 28 | 35 => format!("Couldn't reach {who}. Check the internet connection and try again."),
        22 => format!("{who} refused the request. Try again later."),
        _ => format!("The {who} lookup failed (curl code {code})."),
    }
}

fn get_json(url: &str, who: &str) -> Result<Value, String> {
    match runner::run_cancellable(&system32("curl.exe"), &["-sS", "-L", "--fail", "--max-time", "25", "-H", "User-Agent: DxTool", url], None, Duration::from_secs(35), None, &|_| {})? {
        (0, lines) => serde_json::from_str(&lines.join("\n")).map_err(|_| format!("{who} sent an unexpected reply.")),
        (code, _) => Err(curl_err(code, who)),
    }
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ───────────────────────── NVIDIA ─────────────────────────

fn nvidia(dev: &Device, laptop: bool) -> Result<DriverStatus, String> {
    let id = dev.dev_id().ok_or("No device ID.")?;
    let installed = nvidia_version(&dev.version).unwrap_or_else(|| dev.version.clone());
    let q = format!(
        r#"{{"dIDa":["{id}_10de"],"osC":"10.0","osB":"64","is6":"1","lg":"1033","iLp":"{}","isDCH":"1","dch":"1","upCRD":"0","isCRD":"0","sbs":1}}"#,
        if laptop { 1 } else { 0 }
    );
    let url = format!("https://gfwsl.geforce.com/nvidia_web_services/controller.gfeclientcontent.NG.php/com.nvidia.services.GFEClientContent_NG.getDispDrvrByDevid/{}", url_encode(&q));
    let j = get_json(&url, "NVIDIA")?;
    let a = &j["DriverAttributes"];
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut d = DriverStatus {
        device: dev.name.clone(),
        vendor: "NVIDIA".into(),
        kind: "Graphics".into(),
        installed_version: installed.clone(),
        installed_date: dev.date.clone(),
        ..Default::default()
    };
    let latest = s(&a["Version"]);
    if latest.is_empty() {
        d.status = "unknown".into();
        d.summary = "NVIDIA doesn't list a current driver for this card. Windows Update may still offer one.".into();
        d.details_url = "https://www.nvidia.com/en-us/drivers/".into();
        return Ok(d);
    }
    d.latest_version = latest.clone();
    d.latest_date = s(&a["ReleaseDateTime"]).get(..10).unwrap_or("").to_string();
    d.latest_name = s(&a["Name"]);
    d.download_url = s(&a["DownloadURLAdmin"]);
    if d.download_url.is_empty() {
        d.download_url = s(&a["DownloadURL"]);
    }
    d.details_url = s(&a["DetailsURL"]);
    d.status = status_of(&installed, &latest).into();
    d.summary = match d.status.as_str() {
        "update" => format!("NVIDIA driver {latest} is out ({}). This PC has {installed}.", d.latest_date),
        "current" => format!("Up to date: {installed} is NVIDIA's newest {} for this card.", d.latest_name),
        _ => format!("This PC has {installed}, newer than NVIDIA's newest public {} ({latest}). Probably a Studio, beta or maker-supplied driver; that's fine.", d.latest_name),
    };
    if months_since(&d.latest_date).is_some_and(|m| m >= 12) {
        d.note = "NVIDIA no longer makes new drivers for this card; this is its final driver.".into();
    } else {
        d.note = "This is the Game Ready driver. If you use NVIDIA's Studio drivers instead, get those from NVIDIA's site or the NVIDIA app.".into();
    }
    Ok(d)
}

// ───────────────────────── Intel ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IntelPackage {
    #[serde(default, deserialize_with = "nullable")]
    name: String,
    #[serde(default, deserialize_with = "nullable")]
    display_release_date: String,
    #[serde(default, deserialize_with = "nullable")]
    url: String,
    #[serde(default)]
    intel_system_only: bool,
    #[serde(default)]
    board_numbers: Vec<Value>,
    #[serde(default)]
    model_numbers: Vec<Value>,
    #[serde(default)]
    files: Vec<IntelFile>,
    #[serde(default)]
    components: Vec<IntelComponent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IntelFile {
    #[serde(default, deserialize_with = "nullable")]
    url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    operating_systems: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct IntelComponent {
    #[serde(default, deserialize_with = "nullable")]
    category: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    detection_type: String,
    #[serde(default)]
    detection_values: Vec<String>,
}

/// Reads a string that may be `null` in Intel's data.
fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// Downloads Intel's Driver & Support Assistant catalog and reads its package list.
fn intel_catalog() -> Result<Vec<IntelPackage>, String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip = dir.join("intel-catalog.zip");
    let zs = zip.to_string_lossy().to_string();
    match runner::run_cancellable(&system32("curl.exe"), &["-sS", "-L", "--fail", "--compressed", "--max-time", "60", "-H", "User-Agent: DxTool", "-o", &zs, "https://dsadata.intel.com/data/en"], None, Duration::from_secs(75), None, &|_| {})? {
        (0, _) => {}
        (code, _) => return Err(curl_err(code, "Intel")),
    }
    let ds = dir.to_string_lossy().to_string();
    let file = "software-configurations.json";
    let _ = fs::remove_file(dir.join(file));
    runner::run_cancellable(&system32("tar.exe"), &["-xf", &zs, "-C", &ds, file], None, Duration::from_secs(30), None, &|_| {})?;
    let text = fs::read_to_string(dir.join(file)).map_err(|_| "Intel's driver list couldn't be opened.".to_string())?;
    serde_json::from_str(&text).map_err(|_| "Intel's driver list was in an unexpected format.".to_string())
}

/// Glob match where `*` is any run of characters and the pattern may match a prefix of the ID
/// (hardware IDs carry more detail, such as "&REV_1A", than the catalog lists).
fn glob_prefix(pat: &str, id: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match p.split_first() {
            None => true,
            Some((b'*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..])),
            Some((c, rest)) => s.first().is_some_and(|x| x.eq_ignore_ascii_case(c)) && go(rest, &s[1..]),
        }
    }
    go(pat.as_bytes(), id.as_bytes())
}

fn intel_kind(category: &str, class: &str) -> &'static str {
    match (category, class) {
        ("Graphics", _) | (_, "Display") => "Graphics",
        ("Wireless", _) => "Wi-Fi",
        ("Bluetooth", _) | (_, "Bluetooth") => "Bluetooth",
        ("LAN", _) => "Ethernet",
        _ => "Other",
    }
}

fn intel_checks(devs: &[&Device], catalog: &[IntelPackage], win11: bool) -> Vec<DriverStatus> {
    let os = if win11 { "windows-11" } else { "windows-10" };
    let usable: Vec<&IntelPackage> = catalog
        .iter()
        .filter(|p| !p.intel_system_only && p.board_numbers.is_empty() && p.model_numbers.is_empty() && !p.name.starts_with("Historical"))
        .filter(|p| p.files.iter().any(|f| f.operating_systems.iter().any(|o| o.starts_with(os) && o.ends_with("-64"))))
        .collect();
    let mut out = Vec::new();
    for dev in devs {
        // The newest matching (package, component version).
        let mut best: Option<(&IntelPackage, &IntelComponent, String)> = None;
        for p in &usable {
            for c in &p.components {
                let Some(v) = c.version.as_deref().filter(|v| !v.is_empty()) else { continue };
                if c.detection_type != "Default" || !c.detection_values.iter().any(|pat| dev.hw_ids.iter().any(|h| glob_prefix(pat, h))) {
                    continue;
                }
                if best.as_ref().is_none_or(|(_, _, bv)| cmp_versions(v, bv) == Ordering::Greater) {
                    best = Some((p, c, v.to_string()));
                }
            }
        }
        let Some((p, c, latest)) = best else { continue };
        let file = p.files.iter().find(|f| f.operating_systems.iter().any(|o| o.starts_with(os) && o.ends_with("-64")));
        let kind = intel_kind(&c.category, &dev.class);
        let name = p.name.replace(['®', '™', '*'], "").replace("  ", " ").trim().to_string();
        let mut d = DriverStatus {
            device: dev.name.clone(),
            vendor: "Intel".into(),
            kind: kind.into(),
            installed_version: dev.version.clone(),
            installed_date: dev.date.clone(),
            latest_version: latest.clone(),
            latest_date: p.display_release_date.get(..10).unwrap_or("").to_string(),
            latest_name: name.clone(),
            status: status_of(&dev.version, &latest).into(),
            download_url: file.map(|f| f.url.clone()).unwrap_or_default(),
            download_size: file.map(|f| f.size).unwrap_or(0),
            details_url: p.url.clone(),
            ..Default::default()
        };
        d.summary = match d.status.as_str() {
            "update" => format!("Intel driver {latest} is out ({}). This PC has {}.", d.latest_date, dev.version),
            "current" => format!("Up to date: {} is Intel's newest driver for this device.", dev.version),
            _ => format!("This PC has {}, newer than Intel's newest public driver ({latest}). Probably supplied by the PC maker or Windows Update; that's fine.", dev.version),
        };
        if kind == "Graphics" {
            d.note = "Laptop and PC makers sometimes customise Intel graphics drivers. If the maker's support site has its own driver for this model, that one is the safer choice.".into();
        }
        out.push(d);
    }
    out
}

// ───────────────────────── AMD ─────────────────────────

fn amd(dev: &Device) -> DriverStatus {
    let age = months_since(&dev.date);
    DriverStatus {
        device: dev.name.clone(),
        vendor: "AMD".into(),
        kind: "Graphics".into(),
        installed_version: dev.version.clone(),
        installed_date: dev.date.clone(),
        status: "unknown".into(),
        summary: match age {
            Some(m) if m >= 6 => format!("The installed driver is from {} (about {m} months old). AMD doesn't offer a way to look up the newest version, so check AMD's driver page or AMD Software: Adrenalin Edition.", dev.date),
            _ => "AMD doesn't offer a way to look up the newest version automatically. Check AMD's driver page or AMD Software: Adrenalin Edition.".into(),
        },
        details_url: AMD_URL.into(),
        note: "On AMD's page, use \"Auto-Detect and Install\" or pick the card by name.".into(),
        ..Default::default()
    }
}

// ───────────────────────── checking ─────────────────────────

fn graphics_or_network(d: &Device) -> bool {
    matches!(d.class.as_str(), "Display" | "Net" | "Bluetooth" | "System" | "ComputeAccelerator")
}

/// Checks this PC's NVIDIA, Intel and AMD drivers against the makers' newest (online).
pub fn check() -> DriverCheck {
    let devs = devices();
    let laptop = is_laptop();
    let mut out = DriverCheck { checked_at: chrono::Local::now().to_rfc3339(), ..Default::default() };

    // NVIDIA: one lookup per card model.
    let mut seen = Vec::new();
    for d in devs.iter().filter(|d| d.class == "Display" && d.has("VEN_10DE")) {
        let id = d.dev_id().unwrap_or_default();
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        match nvidia(d, laptop) {
            Ok(s) => out.items.push(s),
            Err(e) => out.problems.push(format!("NVIDIA: {e}")),
        }
    }

    // Intel: graphics, Wi-Fi, Bluetooth, Ethernet (Intel's own devices only).
    let intel: Vec<&Device> = devs.iter().filter(|d| (d.has("VEN_8086") || d.has("VID_8087")) && graphics_or_network(d)).collect();
    if !intel.is_empty() {
        match intel_catalog() {
            Ok(cat) => out.items.extend(intel_checks(&intel, &cat, windows_11())),
            Err(e) => out.problems.push(format!("Intel: {e}")),
        }
    }

    // AMD: link only.
    for d in devs.iter().filter(|d| d.class == "Display" && d.has("VEN_1002")) {
        out.items.push(amd(d));
    }

    out.nothing_to_check = out.items.is_empty() && out.problems.is_empty();
    let rank = |s: &str| match s {
        "update" => 0,
        "unknown" => 1,
        _ => 2,
    };
    out.items.sort_by_key(|i| (rank(&i.status), i.kind != "Graphics"));
    if let Ok(text) = serde_json::to_string_pretty(&out) {
        let _ = fs::create_dir_all(data_dir());
        let _ = fs::write(data_dir().join("last-check.json"), text);
    }
    out
}

/// The last saved check, for showing offline.
pub fn last() -> Option<DriverCheck> {
    serde_json::from_str(&fs::read_to_string(data_dir().join("last-check.json")).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(nvidia_version("32.0.16.2101").as_deref(), Some("621.01"));
        assert_eq!(nvidia_version("32.0.15.6094").as_deref(), Some("560.94"));
        assert_eq!(nvidia_version("31.0.15.3623").as_deref(), Some("536.23"));
        assert_eq!(nvidia_version("30.0.14.7514").as_deref(), Some("475.14"));
        assert_eq!(cmp_versions("621.01", "621.14"), Ordering::Less);
        assert_eq!(cmp_versions("32.0.101.9150", "32.0.101.6862"), Ordering::Greater);
        assert_eq!(cmp_versions("24.70.0.3", "24.70.0.3"), Ordering::Equal);
        assert_eq!(status_of("23.110.0.5", "24.70.0.3"), "update");
    }

    #[test]
    fn globbing() {
        assert!(glob_prefix("VEN_8086&DEV_2723&SUBSYS_*8086", "VEN_8086&DEV_2723&SUBSYS_00848086&REV_1A"));
        assert!(!glob_prefix("VEN_8086&DEV_2723&SUBSYS_*1A56", "VEN_8086&DEV_2723&SUBSYS_00848086&REV_1A"));
        assert!(glob_prefix("VID_8087&PID_0029", "VID_8087&PID_0029&REV_0001"));
        assert!(glob_prefix("VEN_8086&DEV_5690", "VEN_8086&DEV_5690&SUBSYS_30268086&REV_08"));
        assert!(!glob_prefix("VEN_8086&DEV_5690", "VEN_8086&DEV_5691"));
    }

    #[test]
    fn intel_matching() {
        let cat: Vec<IntelPackage> = serde_json::from_value(serde_json::json!([
            { "Name": "Intel® Arc™ Graphics - Windows*", "Version": "32.0.101.9150 WHQL Certified", "DisplayReleaseDate": "2026-10-02T00:00:00Z", "Url": "https://intel.example/arc",
              "IntelSystemOnly": false, "BoardNumbers": [], "ModelNumbers": [],
              "Files": [{ "Url": "https://dl.example/arc.exe", "Size": 900, "OperatingSystems": ["windows-11-24h2-64"] }],
              "Components": [{ "Category": "Graphics", "Version": "32.0.101.9150", "DetectionType": "Default", "DetectionValues": ["VEN_8086&DEV_5690"] }] },
            { "Name": "Historical Intel® Arc™ Graphics", "Version": "32.0.101.9999", "Files": [{ "Url": "x", "Size": 1, "OperatingSystems": ["windows-11-24h2-64"] }],
              "Components": [{ "Category": "Graphics", "Version": "32.0.101.9999", "DetectionType": "Default", "DetectionValues": ["VEN_8086&DEV_5690"] }] },
            { "Name": "Win10 only", "Version": "1", "Files": [{ "Url": "x", "Size": 1, "OperatingSystems": ["windows-10-22h2-64"] }],
              "Components": [{ "Category": "Graphics", "Version": "40.0.0.0", "DetectionType": "Default", "DetectionValues": ["VEN_8086&DEV_5690"] }] }
        ]))
        .unwrap();
        let dev = Device { name: "Intel Arc A770".into(), class: "Display".into(), hw_ids: vec!["VEN_8086&DEV_5690&SUBSYS_30268086&REV_08".into()], version: "32.0.101.6862".into(), date: "2025-06-01".into() };
        let r = intel_checks(&[&dev], &cat, true);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].latest_version, "32.0.101.9150");
        assert_eq!(r[0].status, "update");
        assert_eq!(r[0].latest_name, "Intel Arc Graphics - Windows");
        assert_eq!(r[0].download_url, "https://dl.example/arc.exe");
        // On Windows 10 only the Windows 10 package counts.
        assert_eq!(intel_checks(&[&dev], &cat, false)[0].latest_version, "40.0.0.0");
    }
}
