//! Every Plug and Play device, with its driver and where it is physically connected.

use std::collections::HashMap;

use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Device_IDW, CM_Get_Parent, CM_Locate_DevNodeW, CM_LOCATE_DEVNODE_NORMAL,
    CM_LOCATE_DEVNODE_PHANTOM, CR_SUCCESS,
};

use crate::fmt;
use crate::model::{Fields, Section, Status};
use crate::wmiutil::{self, Row, RowExt};

#[derive(Clone, Debug, Default)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub class: String,
    pub manufacturer: String,
    pub problem_code: u32,
    pub present: bool,
    pub driver_version: Option<String>,
    pub driver_date: Option<String>,
    pub driver_provider: Option<String>,
    pub inf: Option<String>,
    pub signed: Option<bool>,
    pub signer: Option<String>,
    pub location: Option<String>,
    pub parent_ids: Vec<String>,
    pub hardware_ids: Vec<String>,
    pub service: Option<String>,
    pub bus: String,
    pub raw_entity: Row,
    pub raw_driver: Option<Row>,
}

#[derive(Default)]
pub struct DeviceIndex {
    pub map: HashMap<String, DeviceInfo>,
}

impl DeviceIndex {
    pub fn get(&self, id: &str) -> Option<&DeviceInfo> {
        self.map.get(&id.to_uppercase())
    }

    pub fn name_of(&self, id: &str) -> String {
        self.get(id).map(|d| d.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// Human-readable path from the system down to the device, skipping uninformative nodes.
    pub fn chain(&self, d: &DeviceInfo) -> Vec<String> {
        d.parent_ids
            .iter()
            .filter(|p| {
                let u = p.to_uppercase();
                !(u.starts_with("ROOT\\ACPI_HAL") || u.starts_with("ACPI_HAL\\") || u.starts_with("HTREE\\"))
            })
            .map(|p| self.name_of(p))
            .collect()
    }

    /// "Connection" and "Driver" sections plus short summary facts for an item.
    pub fn describe(&self, id: &str) -> Option<(Vec<crate::model::Field>, Vec<Section>)> {
        let d = self.get(id)?;
        let chain = self.chain(d);
        let mut summary = Fields::new();
        summary.text("Connected via", chain.last().cloned());
        summary.text("Location", d.location.as_deref().map(friendly_location));

        let mut conn = Fields::new();
        conn.text("Bus", Some(bus_name(&d.bus)));
        conn.text("Location", d.location.as_deref().map(friendly_location));
        conn.text("Raw location", d.location.clone());
        conn.text("Connected via", chain.last().cloned());
        conn.text("Full connection path", (!chain.is_empty()).then(|| chain.join("  ›  ")));
        conn.text("Device instance ID", Some(d.id.clone()));
        conn.text("Parent instance ID", d.parent_ids.last().cloned());
        conn.text("Hardware IDs", (!d.hardware_ids.is_empty()).then(|| d.hardware_ids.join(", ")));

        let mut drv = Fields::new();
        drv.text("Device status", Some(problem_text(d.problem_code)));
        drv.text("Driver provider", d.driver_provider.clone());
        drv.text("Driver version", d.driver_version.clone());
        drv.text("Driver date", d.driver_date.clone());
        drv.text("INF file", d.inf.clone());
        drv.yes_no("Digitally signed", d.signed);
        drv.text("Signer", d.signer.clone());
        drv.text("Service", d.service.clone());

        let sections = vec![
            Section::new("Connection", conn.into_vec()),
            Section::new("Driver", drv.into_vec()),
        ];
        Some((summary.into_vec(), sections))
    }
}

pub fn collect() -> Result<DeviceIndex, String> {
    let con = wmiutil::connect("ROOT\\CIMV2")?;
    let entities = wmiutil::query(&con, "SELECT * FROM Win32_PnPEntity")?;
    let drivers = wmiutil::query_opt(&con, "SELECT * FROM Win32_PnPSignedDriver");

    let mut drv_map: HashMap<String, Row> = HashMap::new();
    for d in drivers {
        if let Some(id) = d.s("DeviceID") {
            drv_map.insert(id.to_uppercase(), d);
        }
    }

    let mut map = HashMap::new();
    for e in entities {
        let Some(id) = e.s("DeviceID").or_else(|| e.s("PNPDeviceID")) else { continue };
        let key = id.to_uppercase();
        if key.starts_with("HTREE\\") {
            continue; // Windows' internal root node, not a real device
        }
        let drv = drv_map.remove(&key);
        let bus = id.split('\\').next().unwrap_or("").to_uppercase();
        let info = DeviceInfo {
            name: e.s("Name").or_else(|| e.s("Description")).unwrap_or_else(|| id.clone()),
            class: e.s("PNPClass").or_else(|| drv.as_ref().and_then(|d| d.s("DeviceClass"))).unwrap_or_default(),
            manufacturer: e.s("Manufacturer").unwrap_or_default(),
            problem_code: e.u("ConfigManagerErrorCode").unwrap_or(0) as u32,
            present: e.b("Present").unwrap_or(true),
            driver_version: drv.as_ref().and_then(|d| d.s("DriverVersion")),
            driver_date: drv.as_ref().and_then(|d| d.s("DriverDate")).map(|s| fmt::wmi_datetime(&s).unwrap_or(s)),
            driver_provider: drv.as_ref().and_then(|d| d.s("DriverProviderName")),
            inf: drv.as_ref().and_then(|d| d.s("InfName")),
            signed: drv.as_ref().and_then(|d| d.b("IsSigned")),
            signer: drv.as_ref().and_then(|d| d.s("Signer")),
            location: drv.as_ref().and_then(|d| d.s("Location")),
            parent_ids: parent_ids(&id),
            hardware_ids: e.list("HardwareID"),
            service: e.s("Service"),
            bus,
            raw_entity: e,
            raw_driver: drv,
            id,
        };
        map.insert(key, info);
    }
    Ok(DeviceIndex { map })
}

/// Walks up the device tree using the Configuration Manager API. Root-most first.
fn parent_ids(id: &str) -> Vec<String> {
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = Vec::new();
    unsafe {
        let mut inst = 0u32;
        let mut cr = CM_Locate_DevNodeW(&mut inst, PCWSTR(wide.as_ptr()), CM_LOCATE_DEVNODE_NORMAL);
        if cr != CR_SUCCESS {
            cr = CM_Locate_DevNodeW(&mut inst, PCWSTR(wide.as_ptr()), CM_LOCATE_DEVNODE_PHANTOM);
        }
        if cr != CR_SUCCESS {
            return out;
        }
        let mut cur = inst;
        for _ in 0..24 {
            let mut parent = 0u32;
            if CM_Get_Parent(&mut parent, cur, 0) != CR_SUCCESS {
                break;
            }
            let mut buf = [0u16; 512];
            if CM_Get_Device_IDW(parent, &mut buf, 0) != CR_SUCCESS {
                break;
            }
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            let s = String::from_utf16_lossy(&buf[..len]);
            if s.to_uppercase().starts_with("HTREE\\ROOT") {
                break;
            }
            out.push(s);
            cur = parent;
        }
    }
    out.reverse();
    out
}

/// "Port_#0003.Hub_#0002" → "USB port 3 on hub 2"; "PCI bus 0, device 20, function 3" stays as is.
pub fn friendly_location(raw: &str) -> String {
    let r = raw.trim();
    if let Some(rest) = r.strip_prefix("Port_#") {
        let mut parts = rest.split(".Hub_#");
        let port = parts.next().and_then(|p| p.parse::<u32>().ok());
        let hub = parts.next().and_then(|h| h.parse::<u32>().ok());
        if let (Some(p), Some(h)) = (port, hub) {
            return format!("USB port {p} on hub {h}");
        }
    }
    r.to_string()
}

pub fn bus_name(enumerator: &str) -> String {
    match enumerator {
        "PCI" => "PCI / PCI Express",
        "USB" => "USB",
        "USB4" => "USB4 / Thunderbolt",
        "USBSTOR" => "USB storage",
        "HID" => "Human interface device (HID)",
        "ACPI" => "ACPI (built into motherboard)",
        "SCSI" => "Storage (SCSI / NVMe / SATA)",
        "STORAGE" => "Storage volume",
        "NVME" => "NVMe",
        "HDAUDIO" => "High Definition Audio",
        "INTELAUDIO" => "Intel Smart Sound",
        "BTH" | "BTHENUM" | "BTHLE" | "BTHLEDEVICE" => "Bluetooth",
        "DISPLAY" => "Display",
        "MONITOR" => "Monitor",
        "SWD" => "Software device",
        "ROOT" => "System (virtual/root)",
        "SW" => "Software device",
        "UEFI" => "UEFI firmware",
        "TPM" => "Trusted Platform Module",
        other => other,
    }
    .to_string()
}

/// Device Manager error codes in plain English.
pub fn problem_text(code: u32) -> String {
    match code {
        0 => "Working properly",
        1 => "Not configured correctly (code 1)",
        3 => "Driver may be corrupted or the system is low on memory (code 3)",
        10 => "Device cannot start (code 10)",
        12 => "Not enough free resources (code 12)",
        14 => "Needs a restart to work (code 14)",
        18 => "Drivers need to be reinstalled (code 18)",
        19 => "Configuration in the registry is damaged (code 19)",
        21 => "Windows is removing this device (code 21)",
        22 => "Device is disabled (code 22)",
        24 => "Device is not present or not working, or drivers are missing (code 24)",
        28 => "Drivers are not installed (code 28)",
        29 => "Disabled by firmware (code 29)",
        31 => "Not working properly; Windows cannot load its drivers (code 31)",
        32 => "Driver service is disabled (code 32)",
        33 => "Cannot determine required resources (code 33)",
        34 => "Needs manual configuration (code 34)",
        35 => "Firmware lacks information for this device (code 35)",
        37 => "Driver failed to initialise (code 37)",
        38 => "Previous driver copy still in memory (code 38)",
        39 => "Driver is corrupted or missing (code 39)",
        40 => "Driver service registry information is invalid (code 40)",
        41 => "Driver loaded but the hardware was not found (code 41)",
        42 => "Duplicate device detected (code 42)",
        43 => "Windows stopped this device because it reported problems (code 43)",
        44 => "An application or service shut this device down (code 44)",
        45 => "Device is not currently connected (code 45)",
        46 => "Windows is shutting down (code 46)",
        47 => "Prepared for safe removal (code 47)",
        48 => "Driver blocked because of known problems (code 48)",
        49 => "System registry too large (code 49)",
        50 => "Cannot apply all properties (code 50)",
        51 => "Waiting on another device to start (code 51)",
        52 => "Driver digital signature could not be verified (code 52)",
        53 => "Reserved by the Windows kernel debugger (code 53)",
        54 => "Failed and is being reset (code 54)",
        _ => return format!("Problem code {code}"),
    }
    .to_string()
}

pub fn problem_status(code: u32) -> Status {
    match code {
        0 => Status::Ok,
        22 | 45 | 47 => Status::Info,
        14 | 21 | 46 | 51 => Status::Warn,
        _ => Status::Error,
    }
}
