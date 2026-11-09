//! Phone diagnostics: battery, storage, memory, Android version and security updates.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub level: Option<u32>,
    /// "Charging", "Discharging", "Full", "Not charging".
    pub status: String,
    /// Android's own verdict: "Good", "Overheating", "Dead", "Over voltage", "Failure", "Cold".
    pub health: String,
    pub temperature_c: Option<f64>,
    pub voltage_v: Option<f64>,
    /// "AC charger", "USB", "Wireless" or empty.
    pub plugged: String,
    pub cycle_count: Option<u32>,
    /// Remaining capacity compared with new, when the phone reports both.
    pub health_percent: Option<f64>,
    pub capacity_mah: Option<f64>,
    pub design_mah: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// "ok", "warning" or "problem".
    pub level: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhoneInfo {
    pub serial: String,
    pub manufacturer: String,
    pub model: String,
    /// "Galaxy S23" where the phone reports a marketing name.
    pub name: String,
    pub android: String,
    pub sdk: u32,
    pub security_patch: String,
    pub battery: Battery,
    pub storage_total: u64,
    pub storage_free: u64,
    pub memory_total: u64,
    pub memory_available: u64,
    pub uptime_hours: f64,
    pub user_apps: u32,
    pub findings: Vec<Finding>,
    pub collected_at: String,
}

impl PhoneInfo {
    /// "Samsung Galaxy S23 (SM-S911U)".
    pub fn title(&self) -> String {
        let maker = capitalise(&self.manufacturer);
        let name = if self.name.is_empty() { self.model.clone() } else { self.name.clone() };
        let mut t = if name.to_lowercase().starts_with(&maker.to_lowercase()) { name.clone() } else { format!("{maker} {name}") };
        if !self.name.is_empty() && self.name != self.model {
            t.push_str(&format!(" ({})", self.model));
        }
        t.trim().to_string()
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

const SCRIPT: &str = "echo @@props; getprop; \
echo @@battery; dumpsys battery; \
echo @@cap; dumpsys batterystats 2>/dev/null | grep -i 'battery capacity'; \
echo @@sys; for f in cycle_count battery_cycle charge_full charge_full_design; do echo \"$f=$(cat /sys/class/power_supply/battery/$f 2>/dev/null)\"; done; \
echo @@df; df -k /data; \
echo @@mem; cat /proc/meminfo; \
echo @@up; cat /proc/uptime; \
echo @@apps; pm list packages -3 2>/dev/null | wc -l";

/// Splits the script's output into its "@@name" sections.
fn sections(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for part in text.split("@@").skip(1) {
        let (name, body) = part.split_once('\n').unwrap_or((part, ""));
        out.insert(name.trim().to_string(), body.to_string());
    }
    out
}

/// `[ro.product.model]: [Pixel 7]` lines.
pub fn parse_props(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once("]: [")?;
            Some((k.trim().trim_start_matches('[').to_string(), v.trim().trim_end_matches(']').to_string()))
        })
        .collect()
}

/// "  level: 85" lines from `dumpsys battery`.
fn kv(text: &str) -> HashMap<String, String> {
    text.lines().filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))).collect()
}

pub fn parse_battery(battery: &str, cap: &str, sys: &str) -> Battery {
    let m = kv(battery);
    let num = |k: &str| m.get(k).and_then(|v| v.parse::<f64>().ok());
    let mut b = Battery {
        level: num("level").map(|v| v as u32),
        status: match num("status").map(|v| v as u32) {
            Some(2) => "Charging",
            Some(3) => "Discharging",
            Some(4) => "Not charging",
            Some(5) => "Full",
            _ => "",
        }
        .into(),
        health: match num("health").map(|v| v as u32) {
            Some(2) => "Good",
            Some(3) => "Overheating",
            Some(4) => "Dead",
            Some(5) => "Over voltage",
            Some(6) => "Failure",
            Some(7) => "Cold",
            _ => "",
        }
        .into(),
        temperature_c: num("temperature").map(|t| t / 10.0),
        voltage_v: num("voltage").map(|v| if v > 100.0 { v / 1000.0 } else { v }),
        ..Default::default()
    };
    let on = |k: &str| m.get(k).is_some_and(|v| v == "true");
    b.plugged = if on("ac powered") {
        "AC charger"
    } else if on("usb powered") {
        "USB"
    } else if on("wireless powered") {
        "Wireless"
    } else {
        ""
    }
    .into();

    // /sys files (readable on many phones): cycle count and µAh capacities.
    let s: HashMap<&str, f64> = sys.lines().filter_map(|l| l.split_once('=')).filter_map(|(k, v)| Some((k.trim(), v.trim().parse::<f64>().ok()?))).collect();
    b.cycle_count = s.get("cycle_count").or(s.get("battery_cycle")).filter(|c| **c > 0.0 && **c < 10000.0).map(|c| *c as u32);
    let full = s.get("charge_full").map(|v| v / 1000.0).filter(|v| *v > 500.0);
    let design = s.get("charge_full_design").map(|v| v / 1000.0).filter(|v| *v > 500.0);
    // batterystats: "Estimated battery capacity: 4,890 mAh", "Last learned battery capacity: 4,560 mAh".
    let mah = |label: &str| {
        cap.lines().find(|l| l.to_lowercase().contains(label)).and_then(|l| {
            let n: String = l.split(':').nth(1)?.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            n.parse::<f64>().ok().filter(|v| *v > 500.0)
        })
    };
    let (cap_now, cap_new) = match (full, design) {
        (Some(f), Some(d)) => (Some(f), Some(d)),
        _ => (mah("last learned battery capacity").or(mah("max learned battery capacity")), mah("estimated battery capacity")),
    };
    b.capacity_mah = cap_now;
    b.design_mah = cap_new;
    if let (Some(n), Some(d)) = (cap_now, cap_new) {
        let p = n / d * 100.0;
        if (20.0..=110.0).contains(&p) {
            b.health_percent = Some(p.min(100.0));
        }
    }
    b
}

/// (total, free) bytes from `df -k /data`.
pub fn parse_df(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().find(|l| l.trim_end().ends_with("/data"))?;
    let n: Vec<u64> = line.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    (n.len() >= 3).then(|| (n[0] * 1024, n[2] * 1024))
}

/// (total, available) bytes from /proc/meminfo.
pub fn parse_meminfo(text: &str) -> (u64, u64) {
    let get = |k: &str| text.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) * 1024;
    (get("MemTotal:"), get("MemAvailable:"))
}

fn months_since(date: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?;
    Some((chrono::Local::now().date_naive() - d).num_days() / 30)
}

fn gb(b: u64) -> String {
    let g = b as f64 / 1e9;
    if g >= 10.0 { format!("{g:.0} GB") } else { format!("{g:.1} GB") }
}

fn finding(level: &str, title: impl Into<String>, detail: impl Into<String>) -> Finding {
    Finding { level: level.into(), title: title.into(), detail: detail.into() }
}

/// Plain-English findings from the measurements.
pub fn assess(p: &PhoneInfo) -> Vec<Finding> {
    let mut f = Vec::new();
    let b = &p.battery;
    match b.health.as_str() {
        "Dead" | "Failure" | "Over voltage" => f.push(finding("problem", format!("Android reports the battery as \"{}\"", b.health), "The battery should be replaced. A failing battery can swell; if the back or screen is lifting, stop charging it and get it looked at.")),
        "Overheating" => f.push(finding("warning", "The battery is overheating", "Unplug it, take it out of its case and let it cool. If it keeps happening, the battery or charger may be faulty.")),
        _ => {}
    }
    match b.health_percent {
        Some(h) if h < 80.0 => f.push(finding("warning", format!("The battery holds about {h:.0}% of its original charge"), "Below 80% the phone runs out noticeably sooner and may shut down suddenly in the cold. A battery replacement is usually much cheaper than a new phone.")),
        Some(h) => f.push(finding("ok", format!("The battery holds about {h:.0}% of its original charge"), "That's healthy.")),
        None => {}
    }
    if let Some(c) = b.cycle_count {
        if c >= 800 {
            f.push(finding("warning", format!("The battery has been through {c} charge cycles"), "Most phone batteries are made to keep 80% of their capacity for about 500 to 1,000 cycles."));
        }
    }
    if let Some(t) = b.temperature_c {
        if t >= 45.0 {
            f.push(finding("warning", format!("The battery is hot ({t:.0} °C)"), "Phones slow down and charge slowly when hot. Let it cool, and avoid charging under a pillow or in the sun."));
        }
    }
    if p.storage_total > 0 {
        let used = 1.0 - p.storage_free as f64 / p.storage_total as f64;
        if used >= 0.95 {
            f.push(finding("problem", format!("Storage is almost full ({} free)", gb(p.storage_free)), "With less than 5% free, apps crash, photos can't be saved and updates fail. Back up photos and videos, then delete them from the phone, or remove apps you don't use."));
        } else if used >= 0.85 {
            f.push(finding("warning", format!("Storage is getting full ({} free of {})", gb(p.storage_free), gb(p.storage_total)), "Backing up and removing old videos is the quickest way to free space."));
        } else {
            f.push(finding("ok", format!("{} of {} storage is free", gb(p.storage_free), gb(p.storage_total)), ""));
        }
    }
    match months_since(&p.security_patch) {
        Some(m) if m >= 12 => f.push(finding("problem", format!("No security update for about {m} months"), format!("The last security patch is from {}. The phone has probably stopped getting updates, so new security holes stay open. Check Settings → System → Software update; if nothing comes, avoid banking on this phone or plan to replace it.", p.security_patch))),
        Some(m) if m >= 4 => f.push(finding("warning", format!("The last security update is {m} months old"), format!("Security patch {}. Check Settings → System → Software update for a newer one.", p.security_patch))),
        Some(_) => f.push(finding("ok", "Security updates are recent", format!("Security patch {}.", p.security_patch))),
        None => {}
    }
    if p.sdk > 0 && p.sdk < 29 {
        f.push(finding("warning", format!("Android {} is old", p.android), "Many apps (banking especially) are dropping support for Android versions this old."));
    }
    if p.uptime_hours >= 24.0 * 14.0 {
        f.push(finding("warning", format!("The phone hasn't restarted in {:.0} days", p.uptime_hours / 24.0), "Restarting now and then clears out slowdowns and finishes installing updates."));
    }
    f
}

/// Reads diagnostics from the phone with this serial (it must be connected and allowed).
pub fn info(serial: &str) -> Result<PhoneInfo, String> {
    let text = super::shell(serial, SCRIPT, Duration::from_secs(90))?;
    let s = sections(&text);
    let get = |k: &str| s.get(k).map(String::as_str).unwrap_or("");
    let props = parse_props(get("props"));
    let prop = |k: &str| props.get(k).cloned().unwrap_or_default();
    let (storage_total, storage_free) = parse_df(get("df")).unwrap_or((0, 0));
    let (memory_total, memory_available) = parse_meminfo(get("mem"));
    let name = ["ro.product.marketname", "ro.config.marketing_name", "ro.product.vendor.marketname", "ro.vendor.oplus.market.name"].iter().map(|k| prop(k)).find(|v| !v.is_empty()).unwrap_or_default();
    let mut p = PhoneInfo {
        serial: serial.into(),
        manufacturer: prop("ro.product.manufacturer"),
        model: prop("ro.product.model"),
        name,
        android: prop("ro.build.version.release"),
        sdk: prop("ro.build.version.sdk").parse().unwrap_or(0),
        security_patch: prop("ro.build.version.security_patch"),
        battery: parse_battery(get("battery"), get("cap"), get("sys")),
        storage_total,
        storage_free,
        memory_total,
        memory_available,
        uptime_hours: get("up").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 3600.0,
        user_apps: get("apps").trim().parse().unwrap_or(0),
        collected_at: chrono::Local::now().to_rfc3339(),
        ..Default::default()
    };
    if p.model.is_empty() {
        return Err("The phone didn't answer. Unlock it and check it still allows USB debugging from this PC.".into());
    }
    p.findings = assess(&p);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BATTERY: &str = "Current Battery Service state:\n  AC powered: false\n  USB powered: true\n  Wireless powered: false\n  status: 2\n  health: 2\n  present: true\n  level: 85\n  scale: 100\n  voltage: 4102\n  temperature: 291\n  technology: Li-ion\n";

    #[test]
    fn battery() {
        let b = parse_battery(BATTERY, "  Estimated battery capacity: 4,355 mAh\n  Last learned battery capacity: 3,600 mAh\n", "cycle_count=412\nbattery_cycle=\ncharge_full=\ncharge_full_design=\n");
        assert_eq!(b.level, Some(85));
        assert_eq!(b.status, "Charging");
        assert_eq!(b.health, "Good");
        assert_eq!(b.plugged, "USB");
        assert!((b.temperature_c.unwrap() - 29.1).abs() < 0.01);
        assert!((b.voltage_v.unwrap() - 4.102).abs() < 0.001);
        assert_eq!(b.cycle_count, Some(412));
        assert!((b.health_percent.unwrap() - 82.66).abs() < 0.1);
        // /sys capacities win over batterystats.
        let b = parse_battery(BATTERY, "", "charge_full=3900000\ncharge_full_design=4500000\n");
        assert!((b.health_percent.unwrap() - 86.67).abs() < 0.1);
    }

    #[test]
    fn storage_memory_props() {
        assert_eq!(parse_df("Filesystem     1K-blocks     Used Available Use% Mounted on\n/dev/block/dm-48 115249236 98765432 16352348  86% /data\n"), Some((115249236 * 1024, 16352348 * 1024)));
        assert_eq!(parse_meminfo("MemTotal:        7654321 kB\nMemFree:          123456 kB\nMemAvailable:    2345678 kB\n"), (7654321 * 1024, 2345678 * 1024));
        let p = parse_props("[ro.product.model]: [SM-S911U]\n[ro.build.version.security_patch]: [2026-10-01]\n");
        assert_eq!(p["ro.product.model"], "SM-S911U");
        let s = sections("@@props\n[a]: [b]\n@@df\nline\n");
        assert_eq!(s["df"], "line\n");
    }

    #[test]
    fn findings_and_title() {
        let p = PhoneInfo {
            manufacturer: "samsung".into(),
            model: "SM-S911U".into(),
            name: "Galaxy S23".into(),
            security_patch: "2024-01-01".into(),
            storage_total: 128_000_000_000,
            storage_free: 3_000_000_000,
            battery: Battery { health_percent: Some(76.0), ..Default::default() },
            ..Default::default()
        };
        assert_eq!(p.title(), "Samsung Galaxy S23 (SM-S911U)");
        let f = assess(&p);
        assert!(f.iter().any(|x| x.level == "problem" && x.title.contains("Storage is almost full")));
        assert!(f.iter().any(|x| x.level == "problem" && x.title.contains("No security update")));
        assert!(f.iter().any(|x| x.level == "warning" && x.title.contains("76%")));
    }
}
