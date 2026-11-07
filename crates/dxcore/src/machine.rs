//! Identity and key specs of this PC, stored with every scan.

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

use crate::fmt;
use crate::inventory::media_type;
use crate::model::MachineInfo;
use crate::platform;
use crate::wmiutil::{self, ns_query, RowExt};

/// UUIDs that firmware vendors use when they didn't set a real one.
fn usable_uuid(u: &str) -> bool {
    let hex: String = u.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    hex.len() >= 8 && !hex.chars().all(|c| c == '0') && !hex.chars().all(|c| c.eq_ignore_ascii_case(&'f'))
}

/// Folder-safe machine id: computer name plus the first 8 characters of the UUID (or serial).
pub fn machine_id(name: &str, uuid: &str, serial: &str) -> String {
    let clean = |s: &str| -> String { s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect() };
    let mut id = clean(name);
    if id.is_empty() {
        id = "PC".into();
    }
    let tag = if usable_uuid(uuid) {
        uuid.chars().filter(|c| c.is_ascii_hexdigit()).take(8).collect::<String>().to_lowercase()
    } else {
        clean(serial).chars().take(8).collect::<String>().to_lowercase()
    };
    if tag.is_empty() { id } else { format!("{id}-{tag}") }
}

pub fn collect() -> MachineInfo {
    let Ok(con) = wmiutil::connect("ROOT\\CIMV2") else {
        let name = platform::computer_name();
        return MachineInfo { id: machine_id(&name, "", ""), computer_name: name, ..Default::default() };
    };
    let one = |q: &str| wmiutil::query_opt(&con, q).into_iter().next().unwrap_or_default();
    let cs = one("SELECT Manufacturer, Model, TotalPhysicalMemory FROM Win32_ComputerSystem");
    let csp = one("SELECT UUID FROM Win32_ComputerSystemProduct");
    let os = one("SELECT Caption, BuildNumber FROM Win32_OperatingSystem");
    let bios = one("SELECT SerialNumber, SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS");
    let bb = one("SELECT Manufacturer, Product FROM Win32_BaseBoard");
    let cpu = one("SELECT Name FROM Win32_Processor");
    let gpus = wmiutil::query_opt(&con, "SELECT Name FROM Win32_VideoController");
    let disks = ns_query("ROOT\\Microsoft\\Windows\\Storage", "SELECT FriendlyName, Size, MediaType, BusType FROM MSFT_PhysicalDisk");

    let display_ver: Option<String> = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .ok()
        .and_then(|k| k.get_value("DisplayVersion").ok());
    let windows = [
        os.s("Caption").map(|c| c.replace("Microsoft ", "")),
        display_ver,
        os.s("BuildNumber").map(|b| format!("build {b}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");

    let name = platform::computer_name();
    let uuid = csp.s("UUID").unwrap_or_default();
    let serial = bios.s("SerialNumber").unwrap_or_default();
    MachineInfo {
        id: machine_id(&name, &uuid, &serial),
        computer_name: name,
        manufacturer: cs.s("Manufacturer").unwrap_or_default(),
        model: cs.s("Model").unwrap_or_default(),
        serial,
        uuid,
        windows,
        cpu: cpu.s("Name").map(|n| n.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default(),
        memory: cs.f("TotalPhysicalMemory").map(fmt::bytes_bin).unwrap_or_default(),
        graphics: {
            let all: Vec<String> = gpus.iter().filter_map(|g| g.s("Name")).collect();
            let real: Vec<String> = all
                .iter()
                .filter(|n| {
                    let l = n.to_lowercase();
                    !["virtual", "basic display", "remote", "parsec", "mirror", "indirect"].iter().any(|w| l.contains(w))
                })
                .cloned()
                .collect();
            if real.is_empty() { all } else { real }
        },
        drives: disks
            .iter()
            .map(|d| {
                format!(
                    "{} ({}, {})",
                    d.s("FriendlyName").unwrap_or_else(|| "Drive".into()),
                    d.f("Size").map(fmt::bytes_dec).unwrap_or_default(),
                    media_type(d.u("MediaType").unwrap_or(0))
                )
            })
            .collect(),
        motherboard: [bb.s("Manufacturer"), bb.s("Product")].into_iter().flatten().collect::<Vec<_>>().join(" "),
        bios: [bios.s("SMBIOSBIOSVersion"), bios.s("ReleaseDate").and_then(|d| fmt::wmi_datetime(&d))]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
    }
}
