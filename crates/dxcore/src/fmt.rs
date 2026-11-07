//! Human-friendly formatting helpers.

/// Binary units, labelled the way Windows labels them (1 GB = 1024^3 bytes).
pub fn bytes_bin(b: f64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    trim_num(v, i, U[i])
}

/// Decimal units, as printed on drive boxes (1 TB = 10^12 bytes).
pub fn bytes_dec(b: f64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    trim_num(v, i, U[i])
}

fn trim_num(v: f64, i: usize, unit: &str) -> String {
    if i == 0 {
        format!("{v:.0} {unit}")
    } else if v >= 100.0 {
        format!("{v:.0} {unit}")
    } else if v >= 10.0 {
        format!("{v:.1} {unit}")
    } else {
        format!("{v:.2} {unit}")
    }
}

pub fn mhz(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.2} GHz", v / 1000.0)
    } else {
        format!("{v:.0} MHz")
    }
}

pub fn bits_per_sec(v: f64) -> String {
    if v >= 1e9 {
        let g = v / 1e9;
        if g.fract() == 0.0 { format!("{g:.0} Gbps") } else { format!("{g:.1} Gbps") }
    } else if v >= 1e6 {
        format!("{:.0} Mbps", v / 1e6)
    } else if v >= 1e3 {
        format!("{:.0} Kbps", v / 1e3)
    } else {
        format!("{v:.0} bps")
    }
}

pub fn duration_secs(s: u64) -> String {
    let d = s / 86_400;
    let h = (s % 86_400) / 3600;
    let m = (s % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

pub fn percent(v: f64) -> String {
    format!("{v:.0}%")
}

/// WMI dates look like `20240115123456.000000-300`. Returns `2024-01-15 12:34`
/// (or just the date when the time part is midnight, as with driver and BIOS dates).
pub fn wmi_datetime(s: &str) -> Option<String> {
    let b = s.as_bytes();
    if b.len() < 14 || !b[..14].iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if b.len() > 14 && b[14] != b'.' {
        return None;
    }
    let (y, mo, d, h, mi) = (&s[0..4], &s[4..6], &s[6..8], &s[8..10], &s[10..12]);
    if y == "0000" {
        return None;
    }
    if h == "00" && mi == "00" && &s[12..14] == "00" {
        Some(format!("{y}-{mo}-{d}"))
    } else {
        Some(format!("{y}-{mo}-{d} {h}:{mi}"))
    }
}

/// Parse a WMI date into a chrono value (local offset is ignored; good enough for ages).
pub fn wmi_to_naive(s: &str) -> Option<chrono::NaiveDateTime> {
    if s.len() < 14 {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(&s[..14], "%Y%m%d%H%M%S").ok()
}

/// Memory module manufacturers are often reported as JEDEC codes such as `80CE`.
pub fn jedec_vendor(raw: &str) -> String {
    let t = raw.trim();
    let name = match t.to_uppercase().trim_start_matches("0X") {
        s if s.starts_with("80CE") || s.starts_with("CE00") => Some("Samsung"),
        s if s.starts_with("80AD") || s.starts_with("AD00") => Some("SK hynix"),
        s if s.starts_with("802C") || s.starts_with("2C00") => Some("Micron"),
        s if s.starts_with("859B") || s.starts_with("9B05") => Some("Crucial"),
        s if s.starts_with("0198") || s.starts_with("9801") => Some("Kingston"),
        s if s.starts_with("029E") || s.starts_with("9E02") => Some("Corsair"),
        s if s.starts_with("04CD") || s.starts_with("CD04") => Some("G.Skill"),
        s if s.starts_with("04CB") || s.starts_with("CB04") => Some("ADATA"),
        s if s.starts_with("0B4C") || s.starts_with("4C0B") => Some("Lexar"),
        s if s.starts_with("8551") || s.starts_with("5105") => Some("Qimonda"),
        s if s.starts_with("830B") || s.starts_with("0B83") => Some("Nanya"),
        s if s.starts_with("04F1") || s.starts_with("F104") => Some("TeamGroup"),
        s if s.starts_with("0443") || s.starts_with("4304") => Some("Ramaxel"),
        s if s.starts_with("8634") => Some("Smart Modular"),
        _ => None,
    };
    match name {
        Some(n) => format!("{n} ({t})"),
        None => t.to_string(),
    }
}

pub fn yes_no(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}
