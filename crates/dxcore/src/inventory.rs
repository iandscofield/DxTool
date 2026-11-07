//! Full hardware and system inventory.
//!
//! Each category runs on its own thread (WMI queries are slow individually but run
//! well in parallel). Afterwards every item that maps to a Plug and Play device gets
//! "Connection" and "Driver" details attached from the device index.

use std::time::Instant;

use serde_json::json;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

use crate::devices::{self, DeviceIndex, DeviceInfo};
use crate::fmt;
use crate::model::{Category, Field, Fields, Inventory, Item, Section, Status};
use crate::platform;
use crate::wmiutil::{self, ns_query, raw_section, u16_string, Row, RowExt};

const CIMV2: &str = "ROOT\\CIMV2";
const STORAGE: &str = "ROOT\\Microsoft\\Windows\\Storage";
const STDCIMV2: &str = "ROOT\\StandardCimv2";
const WMI: &str = "ROOT\\WMI";

type Collector = fn() -> Result<Vec<Item>, String>;
type IndexCollector = fn(&DeviceIndex) -> Vec<Item>;

struct Spec {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    description: &'static str,
}

const fn spec(id: &'static str, title: &'static str, icon: &'static str, description: &'static str) -> Spec {
    Spec { id, title, icon, description }
}

pub fn collect() -> Inventory {
    let start = Instant::now();

    let direct: Vec<(Spec, Collector)> = vec![
        (spec("system", "System", "monitor", "Computer, Windows, firmware, motherboard, chassis and TPM"), system),
        (spec("cpu", "Processor", "cpu", "CPU model, cores, clocks and caches"), cpu),
        (spec("memory", "Memory", "memory-stick", "Installed RAM modules and the slot each one is in"), memory),
        (spec("graphics", "Graphics", "gpu", "Graphics cards and their drivers"), graphics),
        (spec("displays", "Displays", "screen", "Connected monitors and how they are plugged in"), displays),
        (spec("storage", "Drives", "hard-drive", "Physical drives: SSDs, hard drives, USB drives"), storage),
        (spec("volumes", "Volumes", "folder", "Drive letters, partitions and free space"), volumes),
        (spec("network", "Network", "network", "Network adapters, addresses and link speeds"), network),
        (spec("battery", "Battery", "battery", "Battery capacity, wear and charge cycles"), battery),
        (spec("printers", "Printers", "printer", "Installed printers and their ports"), printers),
        (spec("ports", "Ports", "plug", "Physical connectors reported by the motherboard"), ports),
        (spec("slots", "Expansion slots", "slots", "Motherboard expansion slots and whether they are in use"), slots),
    ];

    let indexed: Vec<(Spec, IndexCollector)> = vec![
        (spec("usb", "USB", "usb", "USB controllers, hubs and every USB device with its port"), usb),
        (spec("audio", "Audio", "speaker", "Sound cards, speakers and microphones"), audio),
        (spec("input", "Input", "keyboard", "Keyboards, mice, touchpads and pens"), input),
        (spec("cameras", "Cameras", "camera", "Webcams and imaging devices"), cameras),
        (spec("bluetooth", "Bluetooth", "bluetooth", "Bluetooth radios and paired devices"), bluetooth),
        (spec("devices", "All devices", "list", "Every device Windows knows about, with drivers and connection paths"), all_devices),
    ];

    let (index, index_err, mut cats) = std::thread::scope(|s| {
        let dev_handle = s.spawn(|| {
            let t = Instant::now();
            (devices::collect(), t.elapsed().as_millis() as u64)
        });
        let handles: Vec<_> = direct
            .iter()
            .map(|(sp, f)| {
                let f = *f;
                s.spawn(move || {
                    let t = Instant::now();
                    let r = std::panic::catch_unwind(f).unwrap_or_else(|_| Err("Collector crashed".into()));
                    (sp, r, t.elapsed().as_millis() as u64)
                })
            })
            .collect();
        let cats: Vec<Category> = handles
            .into_iter()
            .map(|h| {
                let (sp, r, ms) = h.join().expect("thread");
                make_category(sp, r, ms)
            })
            .collect();
        let (dev, _ms) = dev_handle.join().expect("device thread");
        match dev {
            Ok(i) => (i, None, cats),
            Err(e) => (DeviceIndex::default(), Some(e), cats),
        }
    });

    for c in cats.iter_mut() {
        for it in c.items.iter_mut() {
            enrich(it, &index);
        }
    }
    for (sp, f) in indexed.iter() {
        let t = Instant::now();
        let mut cat = make_category(sp, Ok(f(&index)), t.elapsed().as_millis() as u64);
        if let Some(e) = &index_err {
            cat.error = Some(e.clone());
        }
        cats.push(cat);
    }

    Inventory {
        collected_at: chrono::Local::now().to_rfc3339(),
        computer_name: platform::computer_name(),
        is_admin: platform::is_admin(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        categories: cats,
    }
}

fn make_category(sp: &Spec, r: Result<Vec<Item>, String>, ms: u64) -> Category {
    let (items, error) = match r {
        Ok(i) => (i, None),
        Err(e) => (vec![], Some(e)),
    };
    Category {
        id: sp.id.into(),
        title: sp.title.into(),
        icon: sp.icon.into(),
        description: sp.description.into(),
        items,
        error,
        elapsed_ms: ms,
    }
}

fn enrich(it: &mut Item, index: &DeviceIndex) {
    let Some(id) = it.pnp_ids.first() else { return };
    let Some((summary, sections)) = index.describe(id) else { return };
    for f in summary {
        if !it.summary.iter().any(|x| x.key == f.key) {
            it.summary.push(f);
        }
    }
    let insert_at = it.sections.iter().position(|s| s.raw).unwrap_or(it.sections.len());
    for (n, s) in sections.into_iter().enumerate() {
        it.sections.insert(insert_at + n, s);
    }
    if let Some(d) = index.get(id) {
        if d.problem_code != 0 && it.status < devices::problem_status(d.problem_code) {
            it.status = devices::problem_status(d.problem_code);
            it.status_text = Some(devices::problem_text(d.problem_code));
        }
    }
}

fn new_item(id: impl Into<String>, name: impl Into<String>, subtitle: impl Into<String>) -> Item {
    Item { id: id.into(), name: name.into(), subtitle: subtitle.into(), ..Default::default() }
}

/// CIM char16 drive letters arrive either as a number or a one-character string.
fn drive_letter(row: &Row, key: &str) -> Option<char> {
    let c = match row.get(key)? {
        serde_json::Value::Number(n) => char::from_u32(n.as_u64()? as u32)?,
        serde_json::Value::String(s) => s.trim().chars().next()?,
        _ => return None,
    };
    c.is_ascii_alphabetic().then(|| c.to_ascii_uppercase())
}

fn first(rows: &[Row]) -> Row {
    rows.first().cloned().unwrap_or_default()
}

fn hklm(path: &str) -> Option<RegKey> {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path).ok()
}

// ───────────────────────────── System ─────────────────────────────

fn chassis_name(code: u64) -> &'static str {
    match code {
        3 => "Desktop",
        4 => "Low-profile desktop",
        5 => "Pizza box",
        6 => "Mini tower",
        7 => "Tower",
        8 => "Portable",
        9 => "Laptop",
        10 => "Notebook",
        11 => "Handheld",
        12 => "Docking station",
        13 => "All-in-one",
        14 => "Sub-notebook",
        15 => "Space-saving",
        16 => "Lunch box",
        17 => "Main server chassis",
        23 => "Rack mount",
        24 => "Sealed-case PC",
        30 => "Tablet",
        31 => "Convertible",
        32 => "Detachable",
        33 => "IoT gateway",
        34 => "Embedded PC",
        35 => "Mini PC",
        36 => "Stick PC",
        _ => "Other",
    }
}

fn system() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let cs = first(&wmiutil::query(&con, "SELECT * FROM Win32_ComputerSystem")?);
    let csp = first(&wmiutil::query_opt(&con, "SELECT * FROM Win32_ComputerSystemProduct"));
    let os = first(&wmiutil::query_opt(&con, "SELECT * FROM Win32_OperatingSystem"));
    let bios = first(&wmiutil::query_opt(&con, "SELECT * FROM Win32_BIOS"));
    let bb = first(&wmiutil::query_opt(&con, "SELECT * FROM Win32_BaseBoard"));
    let enc = first(&wmiutil::query_opt(&con, "SELECT * FROM Win32_SystemEnclosure"));

    let chassis = enc.nums("ChassisTypes").first().map(|c| chassis_name(*c).to_string());
    let mut out = Vec::new();

    // Computer
    {
        let mut f = Fields::new();
        f.text("Manufacturer", cs.s("Manufacturer"))
            .text("Model", cs.s("Model"))
            .text("Family", cs.s("SystemFamily"))
            .text("SKU", cs.s("SystemSKUNumber"))
            .text("Serial number", bios.s("SerialNumber"))
            .text("UUID", csp.s("UUID"))
            .text("Form factor", chassis.clone())
            .text("Computer name", cs.s("DNSHostName").or_else(|| cs.s("Name")))
            .text(
                if cs.b("PartOfDomain") == Some(true) { "Domain" } else { "Workgroup" },
                cs.s("Domain").or_else(|| cs.s("Workgroup")),
            )
            .text("Signed-in user", cs.s("UserName"))
            .opt_num("Installed memory", cs.f("TotalPhysicalMemory"), fmt::bytes_bin)
            .opt_num("Processors (sockets)", cs.f("NumberOfProcessors"), |n| format!("{n:.0}"))
            .opt_num("Logical processors", cs.f("NumberOfLogicalProcessors"), |n| format!("{n:.0}"))
            .yes_no("Running in a virtual machine", cs.s("Model").map(|m| {
                let m = m.to_lowercase();
                m.contains("virtual") || m.contains("vmware") || m.contains("kvm")
            }))
            .yes_no("Hypervisor present", cs.b("HypervisorPresent"));
        let name = format!(
            "{} {}",
            cs.s("Manufacturer").unwrap_or_default(),
            cs.s("Model").unwrap_or_default()
        );
        let mut it = new_item("computer", name.trim(), "Computer");
        it.summary = f.into_vec();
        it.sections = vec![raw_section("Win32_ComputerSystem", &cs), raw_section("Win32_ComputerSystemProduct", &csp)];
        out.push(it);
    }

    // Windows
    {
        let cv = hklm("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        let display_ver: Option<String> = cv.as_ref().and_then(|k| k.get_value("DisplayVersion").ok());
        let ubr: Option<u32> = cv.as_ref().and_then(|k| k.get_value("UBR").ok());
        let edition: Option<String> = cv.as_ref().and_then(|k| k.get_value("EditionID").ok());
        let build = os.s("BuildNumber");
        let full_build = match (&build, ubr) {
            (Some(b), Some(u)) => Some(format!("{b}.{u}")),
            (b, _) => b.clone(),
        };
        let uptime = os.s("LastBootUpTime").and_then(|s| fmt::wmi_to_naive(&s)).map(|boot| {
            let secs = (chrono::Local::now().naive_local() - boot).num_seconds().max(0) as u64;
            secs
        });
        let mut f = Fields::new();
        f.text("Edition", os.s("Caption"))
            .text("Version", display_ver.clone())
            .text("Build", full_build.clone())
            .text("Edition ID", edition)
            .text("Architecture", os.s("OSArchitecture"))
            .text("Installed", os.s("InstallDate").and_then(|s| fmt::wmi_datetime(&s)))
            .text("Last boot", os.s("LastBootUpTime").and_then(|s| fmt::wmi_datetime(&s)))
            .opt_num("Uptime", uptime.map(|u| u as f64), |s| fmt::duration_secs(s as u64))
            .text("Registered owner", os.s("RegisteredUser"))
            .text("Organization", os.s("Organization"))
            .text("Product ID", os.s("SerialNumber"))
            .text("System drive", os.s("SystemDrive"))
            .text("Windows folder", os.s("WindowsDirectory"))
            .text("Boot device", os.s("BootDevice"))
            .text("Language", os.s("MUILanguages").or_else(|| os.list("MUILanguages").first().cloned()))
            .opt_num("Visible memory", os.f("TotalVisibleMemorySize").map(|k| k * 1024.0), fmt::bytes_bin)
            .opt_num("Free memory", os.f("FreePhysicalMemory").map(|k| k * 1024.0), fmt::bytes_bin)
            .opt_num("Page file size", os.f("SizeStoredInPagingFiles").map(|k| k * 1024.0), fmt::bytes_bin);
        let mut it = new_item(
            "windows",
            os.s("Caption").unwrap_or_else(|| "Windows".into()),
            format!("{} · build {}", display_ver.unwrap_or_default(), full_build.unwrap_or_default()),
        );
        it.summary = f.into_vec();
        it.sections = vec![raw_section("Win32_OperatingSystem", &os)];
        out.push(it);
    }

    // Firmware
    {
        let sb: Option<u32> = hklm("SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State")
            .and_then(|k| k.get_value("UEFISecureBootEnabled").ok());
        let mode = platform::firmware_type();
        let mut f = Fields::new();
        f.text("Vendor", bios.s("Manufacturer"))
            .text("Version", bios.s("SMBIOSBIOSVersion"))
            .text("Release date", bios.s("ReleaseDate").and_then(|s| fmt::wmi_datetime(&s)))
            .text("Boot mode", Some(mode))
            .text(
                "Secure Boot",
                Some(match (mode, sb) {
                    ("UEFI", Some(1)) => "On",
                    ("UEFI", Some(0)) => "Off",
                    ("Legacy BIOS", _) => "Not supported in legacy mode",
                    _ => "Unknown",
                }),
            )
            .text(
                "SMBIOS version",
                bios.u("SMBIOSMajorVersion").map(|m| format!("{m}.{}", bios.u("SMBIOSMinorVersion").unwrap_or(0))),
            )
            .text(
                "Firmware revision",
                bios.u("SystemBiosMajorVersion").map(|m| format!("{m}.{}", bios.u("SystemBiosMinorVersion").unwrap_or(0))),
            )
            .text(
                "Embedded controller",
                bios.u("EmbeddedControllerMajorVersion")
                    .filter(|v| *v != 255)
                    .map(|m| format!("{m}.{}", bios.u("EmbeddedControllerMinorVersion").unwrap_or(0))),
            );
        let mut it = new_item(
            "firmware",
            format!("{} {}", bios.s("Manufacturer").unwrap_or_default(), bios.s("SMBIOSBIOSVersion").unwrap_or_default()).trim(),
            format!("{mode} firmware"),
        );
        if mode == "UEFI" && sb == Some(0) {
            it.status = Status::Warn;
            it.status_text = Some("Secure Boot is turned off".into());
        }
        it.summary = f.into_vec();
        it.sections = vec![raw_section("Win32_BIOS", &bios)];
        out.push(it);
    }

    // Motherboard
    {
        let mut f = Fields::new();
        f.text("Manufacturer", bb.s("Manufacturer"))
            .text("Model", bb.s("Product"))
            .text("Revision", bb.s("Version"))
            .text("Serial number", bb.s("SerialNumber"))
            .text("Part number", bb.s("PartNumber"))
            .yes_no("Replaceable", bb.b("Replaceable"))
            .yes_no("Hot-swappable", bb.b("HotSwappable"));
        let mut it = new_item(
            "motherboard",
            format!("{} {}", bb.s("Manufacturer").unwrap_or_default(), bb.s("Product").unwrap_or_default()).trim(),
            "Motherboard",
        );
        it.summary = f.into_vec();
        it.sections = vec![raw_section("Win32_BaseBoard", &bb)];
        out.push(it);
    }

    // Chassis
    {
        let mut f = Fields::new();
        f.text("Type", chassis.clone())
            .text("Manufacturer", enc.s("Manufacturer"))
            .text("Serial number", enc.s("SerialNumber"))
            .text("Asset tag", enc.s("SMBIOSAssetTag"))
            .yes_no("Lock present", enc.b("LockPresent"));
        let mut it = new_item("chassis", chassis.unwrap_or_else(|| "Chassis".into()), "Case / chassis");
        it.summary = f.into_vec();
        it.sections = vec![raw_section("Win32_SystemEnclosure", &enc)];
        out.push(it);
    }

    // TPM (needs administrator)
    {
        // Without administrator rights this query waits 5 seconds and then fails, so skip it.
        let tpm = if platform::is_admin() { ns_query("ROOT\\CIMV2\\Security\\MicrosoftTpm", "SELECT * FROM Win32_Tpm") } else { Vec::new() };
        let mut it = new_item("tpm", "TPM security chip", "Trusted Platform Module");
        if let Some(t) = tpm.first() {
            let spec = t.s("SpecVersion").map(|s| s.split(',').next().unwrap_or("").trim().to_string());
            let mut f = Fields::new();
            f.text("Specification", spec.clone())
                .text("Manufacturer", t.s("ManufacturerIdTxt"))
                .text("Firmware version", t.s("ManufacturerVersion"))
                .yes_no("Enabled", t.b("IsEnabled_InitialValue"))
                .yes_no("Activated", t.b("IsActivated_InitialValue"))
                .yes_no("Owned", t.b("IsOwned_InitialValue"));
            it.name = format!("TPM {}", spec.unwrap_or_default()).trim().to_string();
            it.summary = f.into_vec();
            it.sections = vec![raw_section("Win32_Tpm", t)];
            if t.b("IsEnabled_InitialValue") == Some(false) {
                it.status = Status::Warn;
                it.status_text = Some("TPM is disabled".into());
            }
        } else if !platform::is_admin() {
            it.status = Status::Info;
            it.status_text = Some("Run DxTool as administrator to read TPM details".into());
        } else {
            it.status = Status::Warn;
            it.status_text = Some("No TPM found".into());
        }
        out.push(it);
    }

    Ok(out)
}

// ───────────────────────────── CPU ─────────────────────────────

fn cpu() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let cpus = wmiutil::query(&con, "SELECT * FROM Win32_Processor")?;
    let caches = wmiutil::query_opt(&con, "SELECT * FROM Win32_CacheMemory");
    Ok(cpus
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let arch = match c.u("Architecture") {
                Some(0) => "x86",
                Some(5) => "ARM",
                Some(9) => "x64",
                Some(12) => "ARM64",
                _ => "Unknown",
            };
            let mut f = Fields::new();
            f.text("Model", c.s("Name"))
                .text("Manufacturer", c.s("Manufacturer"))
                .text("Socket", c.s("SocketDesignation"))
                .opt_num("Cores", c.f("NumberOfCores"), |n| format!("{n:.0}"))
                .opt_num("Enabled cores", c.f("NumberOfEnabledCore"), |n| format!("{n:.0}"))
                .opt_num("Threads", c.f("NumberOfLogicalProcessors"), |n| format!("{n:.0}"))
                .opt_num("Base speed", c.f("MaxClockSpeed"), fmt::mhz)
                .opt_num("Current speed", c.f("CurrentClockSpeed"), fmt::mhz)
                .opt_num("Load", c.f("LoadPercentage"), fmt::percent)
                .opt_num("L2 cache", c.f("L2CacheSize").map(|k| k * 1024.0), fmt::bytes_bin)
                .opt_num("L3 cache", c.f("L3CacheSize").map(|k| k * 1024.0), fmt::bytes_bin)
                .text("Architecture", Some(arch))
                .yes_no("Virtualization enabled in firmware", c.b("VirtualizationFirmwareEnabled"))
                .yes_no("Second-level address translation", c.b("SecondLevelAddressTranslationExtensions"))
                .text("Processor ID", c.s("ProcessorId"))
                .text("Family / stepping", c.s("Description"))
                .text("Status", c.s("Status"));
            let mut it = new_item(format!("cpu{i}"), c.s("Name").unwrap_or_else(|| "Processor".into()), c.s("SocketDesignation").unwrap_or_default());
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_Processor", c));
            for (n, cache) in caches.iter().enumerate() {
                it.sections.push(raw_section(&format!("Win32_CacheMemory #{}", n + 1), cache));
            }
            it
        })
        .collect())
}

// ───────────────────────────── Memory ─────────────────────────────

fn memory_type(code: u64) -> Option<&'static str> {
    Some(match code {
        18 => "DDR",
        19 => "DDR2",
        20 => "DDR2 FB-DIMM",
        24 => "DDR3",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => return None,
    })
}

fn form_factor(code: u64) -> Option<&'static str> {
    Some(match code {
        8 => "DIMM",
        12 => "SO-DIMM",
        13 => "Soldered / on-board",
        _ => return None,
    })
}

fn memory() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let mods = wmiutil::query(&con, "SELECT * FROM Win32_PhysicalMemory")?;
    let arrays = wmiutil::query_opt(&con, "SELECT * FROM Win32_PhysicalMemoryArray");
    let mut out: Vec<Item> = mods
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let ty = m.u("SMBIOSMemoryType").and_then(memory_type).or_else(|| m.u("MemoryType").and_then(memory_type));
            let slot = m.s("DeviceLocator").unwrap_or_else(|| format!("Module {}", i + 1));
            let mut f = Fields::new();
            f.text("Slot", Some(slot.clone()))
                .text("Bank", m.s("BankLabel"))
                .opt_num("Capacity", m.f("Capacity"), fmt::bytes_bin)
                .text("Type", ty)
                .opt_num("Rated speed", m.f("Speed"), |v| format!("{v:.0} MT/s"))
                .opt_num("Running at", m.f("ConfiguredClockSpeed"), |v| format!("{v:.0} MT/s"))
                .text("Manufacturer", m.s("Manufacturer").map(|v| fmt::jedec_vendor(&v)))
                .text("Part number", m.s("PartNumber"))
                .text("Serial number", m.s("SerialNumber"))
                .text("Form factor", m.u("FormFactor").and_then(form_factor))
                .opt_num("Voltage", m.f("ConfiguredVoltage").filter(|v| *v > 0.0), |v| format!("{:.2} V", v / 1000.0))
                .opt_num("Data width", m.f("DataWidth"), |v| format!("{v:.0}-bit"));
            let name = format!(
                "{} {}",
                m.f("Capacity").map(fmt::bytes_bin).unwrap_or_default(),
                ty.unwrap_or("RAM")
            );
            let mut it = new_item(format!("dimm{i}"), name, slot);
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_PhysicalMemory", m));
            it
        })
        .collect();

    if let Some(a) = arrays.first() {
        let total: f64 = mods.iter().filter_map(|m| m.f("Capacity")).sum();
        let mut f = Fields::new();
        f.opt_num("Total slots", a.f("MemoryDevices"), |n| format!("{n:.0}"))
            .num("Slots used", mods.len().to_string(), mods.len() as f64)
            .num("Installed", fmt::bytes_bin(total), total)
            .opt_num("Maximum supported", a.f("MaxCapacityEx").filter(|k| *k > 0.0).or(a.f("MaxCapacity")).map(|k| k * 1024.0), fmt::bytes_bin);
        let mut it = new_item("memarray", "Memory slots overview", "Motherboard memory array");
        it.status = Status::Info;
        it.summary = f.into_vec();
        it.sections.push(raw_section("Win32_PhysicalMemoryArray", a));
        out.insert(0, it);
    }
    Ok(out)
}

// ───────────────────────────── Graphics ─────────────────────────────

fn gpu_vram(pnp_id: &str) -> Option<u64> {
    let class = hklm("SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}")?;
    let id = pnp_id.to_lowercase();
    for name in class.enum_keys().flatten() {
        let Ok(k) = class.open_subkey(&name) else { continue };
        let matching: String = k.get_value("MatchingDeviceId").unwrap_or_default();
        if matching.is_empty() || !id.starts_with(&matching.to_lowercase()) {
            continue;
        }
        if let Ok(v) = k.get_value::<u64, _>("HardwareInformation.qwMemorySize") {
            return Some(v);
        }
        if let Ok(v) = k.get_raw_value("HardwareInformation.MemorySize") {
            let b = &v.bytes;
            if b.len() >= 8 {
                return Some(u64::from_le_bytes(b[..8].try_into().ok()?));
            } else if b.len() >= 4 {
                return Some(u32::from_le_bytes(b[..4].try_into().ok()?) as u64);
            }
        }
    }
    None
}

fn graphics() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let gpus = wmiutil::query(&con, "SELECT * FROM Win32_VideoController")?;
    Ok(gpus
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let pnp = g.s("PNPDeviceID").unwrap_or_default();
            let vram = gpu_vram(&pnp).map(|v| v as f64).or_else(|| g.f("AdapterRAM"));
            let res = match (g.u("CurrentHorizontalResolution"), g.u("CurrentVerticalResolution")) {
                (Some(w), Some(h)) => Some(format!(
                    "{w} × {h}{}",
                    g.u("CurrentRefreshRate").map(|r| format!(" @ {r} Hz")).unwrap_or_default()
                )),
                _ => None,
            };
            let mut f = Fields::new();
            f.text("Model", g.s("Name"))
                .text("Manufacturer", g.s("AdapterCompatibility"))
                .opt_num("Video memory", vram, fmt::bytes_bin)
                .text("Chip", g.s("VideoProcessor"))
                .text("Current mode", res)
                .opt_num("Colour depth", g.f("CurrentBitsPerPixel"), |b| format!("{b:.0}-bit"))
                .text("Driver version", g.s("DriverVersion"))
                .text("Driver date", g.s("DriverDate").and_then(|d| fmt::wmi_datetime(&d)))
                .text("Status", g.s("Status"));
            let mut it = new_item(format!("gpu{i}"), g.s("Name").unwrap_or_else(|| "Graphics".into()), g.s("AdapterCompatibility").unwrap_or_default());
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_VideoController", g));
            it.pnp_ids = vec![pnp];
            it
        })
        .collect())
}

// ───────────────────────────── Displays ─────────────────────────────

fn edid_vendor(code: &str) -> String {
    let name = match code {
        "GSM" => "LG",
        "LGD" => "LG Display",
        "SAM" | "SEC" => "Samsung",
        "DEL" => "Dell",
        "ACR" => "Acer",
        "AUS" => "ASUS",
        "BNQ" => "BenQ",
        "HWP" | "HPN" => "HP",
        "LEN" => "Lenovo",
        "AOC" => "AOC",
        "PHL" => "Philips",
        "VSC" => "ViewSonic",
        "MSI" => "MSI",
        "BOE" => "BOE",
        "AUO" => "AU Optronics",
        "CMN" => "Innolux",
        "SHP" => "Sharp",
        "IVM" => "iiyama",
        "GBT" => "Gigabyte",
        "SNY" => "Sony",
        "APP" => "Apple",
        "MEI" => "Panasonic",
        "NEC" => "NEC",
        "HKC" => "HKC",
        "SDC" => "Samsung Display",
        "CSO" => "CSOT",
        "TMX" => "Tianma",
        "IVO" => "InfoVision",
        _ => return code.to_string(),
    };
    format!("{name} ({code})")
}

fn connection_tech(v: i64) -> &'static str {
    match v {
        -2 => "Uninitialised",
        -1 => "Other",
        0 => "VGA",
        1 => "S-Video",
        2 => "Composite",
        3 => "Component",
        4 => "DVI",
        5 => "HDMI",
        6 => "LVDS (built-in panel)",
        8 => "D-Jpn",
        9 => "SDI",
        10 => "DisplayPort",
        11 => "Embedded DisplayPort (built-in panel)",
        12 => "UDI",
        13 => "Embedded UDI",
        14 => "SDTV dongle",
        15 => "Miracast (wireless)",
        16 => "Indirect (wired)",
        17 => "Indirect (virtual)",
        18 => "DisplayPort over USB-C",
        0x8000_0000 => "Internal",
        _ => "Unknown",
    }
}

fn displays() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(WMI)?;
    let ids = wmiutil::query(&con, "SELECT * FROM WmiMonitorID")?;
    let basic = wmiutil::query_opt(&con, "SELECT * FROM WmiMonitorBasicDisplayParams");
    let conns = wmiutil::query_opt(&con, "SELECT * FROM WmiMonitorConnectionParams");
    let find = |rows: &[Row], inst: &str| rows.iter().find(|r| r.s("InstanceName").as_deref() == Some(inst)).cloned();

    Ok(ids
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let inst = m.s("InstanceName").unwrap_or_default();
            let b = find(&basic, &inst);
            let c = find(&conns, &inst);
            let vendor_code = u16_string(m, "ManufacturerName");
            let model = u16_string(m, "UserFriendlyName");
            let tech = c.as_ref().and_then(|c| c.get("VideoOutputTechnology")).and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)));
            let size = b.as_ref().and_then(|b| {
                let w = b.f("MaxHorizontalImageSize")?;
                let h = b.f("MaxVerticalImageSize")?;
                if w <= 0.0 || h <= 0.0 { return None; }
                Some(((w * w + h * h).sqrt() / 2.54, w, h))
            });
            let year = m.u("YearOfManufacture").filter(|y| *y > 1990);
            let mut f = Fields::new();
            f.text("Model", model.clone())
                .text("Manufacturer", vendor_code.as_deref().map(edid_vendor))
                .text("Connection", tech.map(|t| connection_tech(t).to_string()))
                .opt_num("Screen size", size.map(|s| s.0), |d| format!("{d:.1}\""))
                .text("Physical size", size.map(|s| format!("{:.0} × {:.0} cm", s.1, s.2)))
                .text("Product code", u16_string(m, "ProductCodeID"))
                .text("Serial number", u16_string(m, "SerialNumberID"))
                .text("Manufactured", year.map(|y| match m.u("WeekOfManufacture").filter(|w| *w > 0 && *w < 54) {
                    Some(w) => format!("{y}, week {w}"),
                    None => y.to_string(),
                }))
                .yes_no("Active", m.b("Active"));
            let name = model.clone().unwrap_or_else(|| format!("Display {}", i + 1));
            let mut it = new_item(format!("mon{i}"), name, tech.map(connection_tech).unwrap_or("Monitor"));
            it.summary = f.into_vec();
            it.sections.push(raw_section("WmiMonitorID", m));
            if let Some(b) = &b { it.sections.push(raw_section("WmiMonitorBasicDisplayParams", b)); }
            if let Some(c) = &c { it.sections.push(raw_section("WmiMonitorConnectionParams", c)); }
            let pnp = inst.strip_suffix("_0").unwrap_or(&inst).to_string();
            it.pnp_ids = vec![pnp];
            it
        })
        .collect())
}

// ───────────────────────────── Storage ─────────────────────────────

pub fn bus_type(code: u64) -> &'static str {
    match code {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        4 => "FireWire",
        5 => "SSA",
        6 => "Fibre Channel",
        7 => "USB",
        8 => "RAID",
        9 => "iSCSI",
        10 => "SAS",
        11 => "SATA",
        12 => "SD card",
        13 => "MMC",
        15 => "Virtual (file-backed)",
        16 => "Storage Spaces",
        17 => "NVMe",
        18 => "SCM",
        19 => "UFS",
        _ => "Unknown",
    }
}

pub fn media_type(code: u64) -> &'static str {
    match code {
        3 => "Hard drive (HDD)",
        4 => "Solid-state drive (SSD)",
        5 => "Storage-class memory",
        _ => "Unspecified",
    }
}

pub fn health_text(code: u64) -> (&'static str, Status) {
    match code {
        0 => ("Healthy", Status::Ok),
        1 => ("Warning", Status::Warn),
        2 => ("Unhealthy", Status::Error),
        _ => ("Unknown", Status::Info),
    }
}

fn partition_kind(p: &Row) -> String {
    let gpt = p.s("GptType").unwrap_or_default().to_lowercase();
    let kind = match gpt.trim_matches(|c| c == '{' || c == '}') {
        "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => "EFI system",
        "e3c9e316-0b5c-4db8-817d-f92df00215ae" => "Microsoft reserved",
        "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" => "Basic data",
        "de94bba4-06d1-4d40-a16a-bfd50179d6ac" => "Recovery",
        "5808c8aa-7e8f-42e0-85d2-e1e90434cfb3" => "Dynamic metadata",
        "af9b60a0-1431-4f62-bc68-3311714a69ad" => "Dynamic data",
        "" => return p.s("Type").unwrap_or_else(|| "Partition".into()),
        _ => "Other",
    };
    kind.to_string()
}

fn storage() -> Result<Vec<Item>, String> {
    let st = wmiutil::connect(STORAGE)?;
    let phys = wmiutil::query(&st, "SELECT * FROM MSFT_PhysicalDisk")?;
    let disks = wmiutil::query_opt(&st, "SELECT * FROM MSFT_Disk");
    let parts = wmiutil::query_opt(&st, "SELECT * FROM MSFT_Partition");
    let w32 = ns_query(CIMV2, "SELECT * FROM Win32_DiskDrive");

    let mut out: Vec<Item> = phys
        .iter()
        .map(|p| {
            let num = p.s("DeviceId").unwrap_or_default();
            let n: u64 = num.parse().unwrap_or(u64::MAX);
            let d = disks.iter().find(|d| d.u("Number") == Some(n));
            let w = w32.iter().find(|w| w.u("Index") == Some(n));
            let my_parts: Vec<&Row> = parts.iter().filter(|x| x.u("DiskNumber") == Some(n)).collect();
            let (health, status) = health_text(p.u("HealthStatus").unwrap_or(99));
            let media = media_type(p.u("MediaType").unwrap_or(0));
            let bus = bus_type(p.u("BusType").unwrap_or(0));
            let letters: Vec<String> = my_parts
                .iter()
                .filter_map(|x| drive_letter(x, "DriveLetter"))
                .map(|c| format!("{c}:"))
                .collect();
            let style = d.and_then(|d| d.u("PartitionStyle")).map(|s| match s {
                1 => "MBR",
                2 => "GPT",
                _ => "Raw (not initialised)",
            });
            let rpm = p.u("SpindleSpeed").filter(|r| *r > 0 && *r < 100_000);
            let mut f = Fields::new();
            f.text("Model", p.s("FriendlyName").or_else(|| p.s("Model")))
                .text("Type", Some(media))
                .text("Interface", Some(bus))
                .opt_num("Capacity", p.f("Size"), fmt::bytes_dec)
                .text("Health", Some(health))
                .text("Disk number", Some(num.clone()))
                .text("Drive letters", (!letters.is_empty()).then(|| letters.join(" ")))
                .text("Partition style", style)
                .yes_no("Windows boots from this drive", d.and_then(|d| d.b("IsBoot")))
                .yes_no("System drive", d.and_then(|d| d.b("IsSystem")))
                .text("Firmware", p.s("FirmwareVersion"))
                .text("Serial number", p.s("SerialNumber"))
                .text("Manufacturer", p.s("Manufacturer"))
                .opt_num("Spindle speed", rpm.map(|r| r as f64), |r| format!("{r:.0} RPM"))
                .opt_num("Sector size", p.f("PhysicalSectorSize"), |s| format!("{s:.0} bytes"))
                .text("Physical location", p.s("PhysicalLocation"))
                .text("Operational status", p.list("OperationalStatus").first().map(|s| match s.as_str() {
                    "2" => "OK".to_string(),
                    "3" => "Degraded".to_string(),
                    "5" => "Predictive failure".to_string(),
                    "10" => "Stopped".to_string(),
                    "53264" => "Online".to_string(),
                    o => o.to_string(),
                }));
            let name = p.s("FriendlyName").or_else(|| p.s("Model")).unwrap_or_else(|| format!("Disk {num}"));
            let mut it = new_item(format!("disk{num}"), name, format!("{} · {}", bus, p.f("Size").map(fmt::bytes_dec).unwrap_or_default()));
            it.status = status;
            if status != Status::Ok {
                it.status_text = Some(format!("Windows reports this drive as {health}"));
            }
            it.summary = f.into_vec();

            let mut pf = Vec::new();
            for (i, x) in my_parts.iter().enumerate() {
                let letter = drive_letter(x, "DriveLetter");
                let label = format!(
                    "{}{} · {}",
                    letter.map(|c| format!("{c}: ")).unwrap_or_default(),
                    partition_kind(x),
                    x.f("Size").map(fmt::bytes_bin).unwrap_or_default()
                );
                pf.push(Field {
                    key: format!("partition{i}"),
                    label: format!("Partition {}", x.u("PartitionNumber").unwrap_or(i as u64 + 1)),
                    display: label.clone(),
                    sort: json!(label),
                });
            }
            if !pf.is_empty() {
                it.sections.push(Section::new("Partitions", pf));
            }
            it.sections.push(raw_section("MSFT_PhysicalDisk", p));
            if let Some(d) = d { it.sections.push(raw_section("MSFT_Disk", d)); }
            if let Some(w) = w {
                it.sections.push(raw_section("Win32_DiskDrive", w));
                if let Some(id) = w.s("PNPDeviceID") { it.pnp_ids.push(id); }
            }
            it
        })
        .collect();
    out.sort_by_key(|i| i.id.trim_start_matches("disk").parse::<u32>().unwrap_or(999));
    Ok(out)
}

fn volumes() -> Result<Vec<Item>, String> {
    let st = wmiutil::connect(STORAGE)?;
    let vols = wmiutil::query(&st, "SELECT * FROM MSFT_Volume")?;
    let mut out: Vec<Item> = vols
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let letter = drive_letter(v, "DriveLetter").map(|c| format!("{c}:"));
            let size = v.f("Size").unwrap_or(0.0);
            let free = v.f("SizeRemaining").unwrap_or(0.0);
            let used_pct = if size > 0.0 { (size - free) / size * 100.0 } else { 0.0 };
            let (health, mut status) = health_text(v.u("HealthStatus").unwrap_or(99));
            let label = v.s("FileSystemLabel");
            let drive_type = match v.u("DriveType") {
                Some(2) => "Removable",
                Some(3) => "Fixed",
                Some(4) => "Network",
                Some(5) => "Optical",
                Some(6) => "RAM disk",
                _ => "Unknown",
            };
            let mut status_text = None;
            if letter.is_some() && size > 0.0 && used_pct >= 90.0 {
                let s = if used_pct >= 97.0 { Status::Error } else { Status::Warn };
                if s > status {
                    status = s;
                }
                status_text = Some(format!("Only {} free", fmt::bytes_bin(free)));
            }
            let mut f = Fields::new();
            f.text("Drive", Some(letter.clone().unwrap_or_else(|| "(no letter)".into())))
                .text("Label", label.clone())
                .text("File system", v.s("FileSystem"))
                .num("Size", fmt::bytes_bin(size), size)
                .num("Free", fmt::bytes_bin(free), free)
                .num("Used", fmt::percent(used_pct), used_pct)
                .text("Health", Some(health))
                .text("Type", Some(drive_type))
                .text("Path", v.s("Path"));
            let name = match (&letter, &label) {
                (Some(l), Some(lb)) => format!("{lb} ({l})"),
                (Some(l), None) => format!("Local disk ({l})"),
                (None, Some(lb)) => lb.clone(),
                (None, None) => "Hidden system volume".into(),
            };
            let mut it = new_item(format!("vol{i}"), name, format!("{} · {}", v.s("FileSystem").unwrap_or_default(), fmt::bytes_bin(size)));
            it.status = status;
            it.status_text = status_text;
            it.summary = f.into_vec();
            it.sections.push(raw_section("MSFT_Volume", v));
            it
        })
        .collect();
    out.sort_by(|a, b| {
        let la = a.summary.first().map(|f| f.display.clone()).unwrap_or_default();
        let lb = b.summary.first().map(|f| f.display.clone()).unwrap_or_default();
        la.starts_with('(').cmp(&lb.starts_with('(')).then(la.cmp(&lb))
    });
    Ok(out)
}

// ───────────────────────────── Network ─────────────────────────────

fn physical_medium(code: u64) -> &'static str {
    match code {
        1 | 9 => "Wi-Fi",
        8 => "Mobile broadband",
        10 => "Bluetooth",
        14 => "Ethernet",
        2 => "Cable modem",
        5 => "DSL",
        11 => "InfiniBand",
        _ => "Other",
    }
}

fn network() -> Result<Vec<Item>, String> {
    let std = wmiutil::connect(STDCIMV2)?;
    let adapters = wmiutil::query(&std, "SELECT * FROM MSFT_NetAdapter")?;
    let configs = ns_query(CIMV2, "SELECT * FROM Win32_NetworkAdapterConfiguration");
    let mut out: Vec<Item> = adapters
        .iter()
        .map(|a| {
            let idx = a.u("InterfaceIndex");
            let cfg = configs.iter().find(|c| c.u("InterfaceIndex") == idx);
            let ips = cfg.map(|c| c.list("IPAddress")).unwrap_or_default();
            let v4: Vec<&String> = ips.iter().filter(|i| i.contains('.')).collect();
            let v6: Vec<&String> = ips.iter().filter(|i| i.contains(':')).collect();
            let connected = a.u("MediaConnectState") == Some(1);
            let hardware = a.b("HardwareInterface").unwrap_or(false);
            let medium = physical_medium(a.u("NdisPhysicalMedium").unwrap_or(0));
            let speed = a.f("ReceiveLinkSpeed").or(a.f("Speed")).filter(|s| *s > 0.0 && connected);
            let mut f = Fields::new();
            f.text("Name", a.s("Name"))
                .text("Adapter", a.s("InterfaceDescription"))
                .text("Type", Some(if hardware { medium } else { "Virtual" }))
                .text("Status", Some(if connected { "Connected" } else if a.u("InterfaceAdminStatus") == Some(2) { "Disabled" } else { "Disconnected" }))
                .opt_num("Link speed", speed, fmt::bits_per_sec)
                .text("MAC address", a.s("PermanentAddress").map(|m| {
                    m.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join("-")
                }).or_else(|| a.s("MacAddress")))
                .text("IPv4 address", (!v4.is_empty()).then(|| v4.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")))
                .text("IPv6 address", (!v6.is_empty()).then(|| v6.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")))
                .text("Gateway", cfg.map(|c| c.list("DefaultIPGateway").join(", ")))
                .text("DNS servers", cfg.map(|c| c.list("DNSServerSearchOrder").join(", ")))
                .yes_no("DHCP", cfg.and_then(|c| c.b("DHCPEnabled")))
                .text("DHCP server", cfg.and_then(|c| c.s("DHCPServer")))
                .yes_no("Physical adapter", Some(hardware))
                .text("Driver version", a.s("DriverVersionString"))
                .text("Driver date", a.s("DriverDate"))
                .text("Driver provider", a.s("DriverProvider"))
                .opt_num("MTU", a.f("MtuSize"), |m| format!("{m:.0}"));
            let mut it = new_item(
                format!("net{}", idx.unwrap_or(0)),
                a.s("Name").unwrap_or_else(|| "Adapter".into()),
                a.s("InterfaceDescription").unwrap_or_default(),
            );
            if !hardware {
                it.status = Status::Info;
            }
            it.summary = f.into_vec();
            it.sections.push(raw_section("MSFT_NetAdapter", a));
            if let Some(c) = cfg { it.sections.push(raw_section("Win32_NetworkAdapterConfiguration", c)); }
            if let Some(p) = a.s("PnPDeviceID") { it.pnp_ids.push(p); }
            it
        })
        .collect();
    out.sort_by_key(|i| (i.status != Status::Ok, i.name.clone()));
    Ok(out)
}

// ───────────────────────────── Battery ─────────────────────────────

pub(crate) fn battery() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(WMI)?;
    let stat = wmiutil::query_opt(&con, "SELECT * FROM BatteryStaticData");
    let full = wmiutil::query_opt(&con, "SELECT * FROM BatteryFullChargedCapacity");
    let cyc = wmiutil::query_opt(&con, "SELECT * FROM BatteryCycleCount");
    let st = wmiutil::query_opt(&con, "SELECT * FROM BatteryStatus");
    let w32 = ns_query(CIMV2, "SELECT * FROM Win32_Battery");
    if stat.is_empty() && w32.is_empty() {
        return Ok(vec![]);
    }
    let find = |rows: &[Row], inst: &str| rows.iter().find(|r| r.s("InstanceName").as_deref() == Some(inst)).cloned();
    let base: Vec<Row> = if stat.is_empty() { full.clone() } else { stat.clone() };
    Ok(base
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let inst = s.s("InstanceName").unwrap_or_default();
            let fc = find(&full, &inst).and_then(|r| r.f("FullChargedCapacity"));
            let design = s.f("DesignedCapacity").filter(|d| *d > 0.0);
            let cycles = find(&cyc, &inst).and_then(|r| r.f("CycleCount"));
            let status = find(&st, &inst);
            let w = w32.get(i).or(w32.first());
            let health = match (fc, design) {
                (Some(f), Some(d)) => Some((f / d * 100.0).min(100.0)),
                _ => None,
            };
            let chem = s.u("Chemistry").map(|c| {
                let b = (c as u32).to_le_bytes();
                String::from_utf8_lossy(&b).trim_matches(char::from(0)).trim().to_string()
            });
            let mut f = Fields::new();
            f.opt_num("Health", health, fmt::percent)
                .opt_num("Design capacity", design, |v| format!("{:.1} Wh", v / 1000.0))
                .opt_num("Full charge capacity", fc, |v| format!("{:.1} Wh", v / 1000.0))
                .opt_num("Charge cycles", cycles.filter(|c| *c > 0.0), |c| format!("{c:.0}"))
                .opt_num("Charge level", w.and_then(|w| w.f("EstimatedChargeRemaining")), fmt::percent)
                .text("Plugged in", status.as_ref().and_then(|r| r.b("PowerOnline")).map(|b| fmt::yes_no(b)))
                .text("Charging", status.as_ref().and_then(|r| r.b("Charging")).map(|b| fmt::yes_no(b)))
                .opt_num("Voltage", status.as_ref().and_then(|r| r.f("Voltage")).filter(|v| *v > 0.0), |v| format!("{:.2} V", v / 1000.0))
                .opt_num("Remaining capacity", status.as_ref().and_then(|r| r.f("RemainingCapacity")), |v| format!("{:.1} Wh", v / 1000.0))
                .text("Manufacturer", s.s("ManufactureName"))
                .text("Model", s.s("DeviceName"))
                .text("Serial number", s.s("SerialNumber"))
                .text("Chemistry", chem);
            let mut it = new_item(format!("bat{i}"), s.s("DeviceName").unwrap_or_else(|| "Battery".into()), s.s("ManufactureName").unwrap_or_default());
            if let Some(h) = health {
                if h < 50.0 {
                    it.status = Status::Error;
                    it.status_text = Some(format!("Battery holds only {h:.0}% of its original charge"));
                } else if h < 80.0 {
                    it.status = Status::Warn;
                    it.status_text = Some(format!("Battery has worn to {h:.0}% of its original capacity"));
                }
            }
            it.summary = f.into_vec();
            it.sections.push(raw_section("BatteryStaticData", s));
            if let Some(r) = status { it.sections.push(raw_section("BatteryStatus", &r)); }
            if let Some(w) = w { it.sections.push(raw_section("Win32_Battery", w)); }
            it
        })
        .collect())
}

// ───────────────────────────── Printers, ports, slots ─────────────────────────────

fn printers() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let ps = wmiutil::query(&con, "SELECT * FROM Win32_Printer")?;
    Ok(ps
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut f = Fields::new();
            f.text("Name", p.s("Name"))
                .text("Driver", p.s("DriverName"))
                .text("Port", p.s("PortName"))
                .yes_no("Default printer", p.b("Default"))
                .yes_no("Network printer", p.b("Network"))
                .yes_no("Shared", p.b("Shared"))
                .text("Share name", p.s("ShareName"))
                .text("Location", p.s("Location"))
                .yes_no("Offline", p.b("WorkOffline"))
                .text("Status", p.s("Status"));
            let mut it = new_item(format!("prn{i}"), p.s("Name").unwrap_or_default(), p.s("DriverName").unwrap_or_default());
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_Printer", p));
            if let Some(id) = p.s("PNPDeviceID") { it.pnp_ids.push(id); }
            it
        })
        .collect())
}

fn port_type(code: u64) -> &'static str {
    match code {
        0 => "None",
        1 => "Parallel (XT/AT)",
        2 => "Parallel (PS/2)",
        3 => "Parallel (ECP)",
        4 => "Parallel (EPP)",
        5 => "Parallel (ECP/EPP)",
        6 => "Serial (XT/AT)",
        7 => "Serial (16450)",
        8 => "Serial (16550)",
        9 => "Serial (16550A)",
        10 => "SCSI",
        11 => "MIDI",
        12 => "Joystick",
        13 => "Keyboard (PS/2)",
        14 => "Mouse (PS/2)",
        15 => "SSA SCSI",
        16 => "USB",
        17 => "FireWire",
        18..=20 => "PCMCIA",
        21 => "CardBus",
        22 => "Access bus",
        23 | 24 => "SCSI",
        28 => "Video",
        29 => "Audio",
        30 => "Modem",
        31 => "Network",
        32 => "SATA",
        33 => "SAS",
        34 => "MFDP (Multi-Function Display Port)",
        35 => "Thunderbolt",
        _ => "Other",
    }
}

fn ports() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let ps = wmiutil::query(&con, "SELECT * FROM Win32_PortConnector")?;
    Ok(ps
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let ext = p.s("ExternalReferenceDesignator");
            let int = p.s("InternalReferenceDesignator");
            let ty = p.u("PortType").map(port_type);
            let mut f = Fields::new();
            f.text("Port type", ty)
                .text("External label", ext.clone())
                .text("Internal label", int.clone())
                .text("Tag", p.s("Tag"));
            let name = ext.or(int).unwrap_or_else(|| format!("Port {}", i + 1));
            let mut it = new_item(format!("port{i}"), name, ty.unwrap_or("Port"));
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_PortConnector", p));
            it
        })
        .collect())
}

fn slots() -> Result<Vec<Item>, String> {
    let con = wmiutil::connect(CIMV2)?;
    let ss = wmiutil::query(&con, "SELECT * FROM Win32_SystemSlot")?;
    Ok(ss
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let usage = match s.u("CurrentUsage") {
                Some(3) => "Empty",
                Some(4) => "In use",
                Some(2) => "Unknown",
                _ => "Other",
            };
            let mut f = Fields::new();
            f.text("Slot", s.s("SlotDesignation"))
                .text("Usage", Some(usage))
                .text("Status", s.s("Status"))
                .yes_no("Supports hot-plug", s.b("SupportsHotPlug"))
                .text("Tag", s.s("Tag"));
            let mut it = new_item(format!("slot{i}"), s.s("SlotDesignation").unwrap_or_else(|| format!("Slot {}", i + 1)), usage);
            it.status = if usage == "In use" { Status::Ok } else { Status::Info };
            it.summary = f.into_vec();
            it.sections.push(raw_section("Win32_SystemSlot", s));
            it
        })
        .collect())
}

// ───────────────────────────── From the device index ─────────────────────────────

fn device_item(d: &DeviceInfo, index: &DeviceIndex, extra: impl FnOnce(&mut Fields)) -> Item {
    let chain = index.chain(d);
    let mut f = Fields::new();
    f.text("Name", Some(d.name.clone()))
        .text("Type", Some(d.class.clone()))
        .text("Manufacturer", Some(d.manufacturer.clone()))
        .text("Status", Some(devices::problem_text(d.problem_code)));
    extra(&mut f);
    f.text("Bus", Some(devices::bus_name(&d.bus)))
        .text("Location", d.location.as_deref().map(devices::friendly_location))
        .text("Connected via", chain.last().cloned())
        .text("Driver version", d.driver_version.clone())
        .text("Driver date", d.driver_date.clone())
        .text("Driver provider", d.driver_provider.clone());
    let mut it = new_item(d.id.clone(), d.name.clone(), d.class.clone());
    it.status = devices::problem_status(d.problem_code);
    if d.problem_code != 0 {
        it.status_text = Some(devices::problem_text(d.problem_code));
    }
    it.summary = f.into_vec();
    if let Some((_, sections)) = index.describe(&d.id) {
        it.sections.extend(sections);
    }
    it.sections.push(raw_section("Win32_PnPEntity", &d.raw_entity));
    if let Some(r) = &d.raw_driver {
        it.sections.push(raw_section("Win32_PnPSignedDriver", r));
    }
    it
}

fn from_index(index: &DeviceIndex, pred: impl Fn(&DeviceInfo) -> bool, extra: impl Fn(&DeviceInfo, &mut Fields)) -> Vec<Item> {
    let mut v: Vec<&DeviceInfo> = index.map.values().filter(|d| pred(d)).collect();
    v.sort_by(|a, b| a.class.cmp(&b.class).then(a.name.cmp(&b.name)));
    v.into_iter().map(|d| device_item(d, index, |f| extra(d, f))).collect()
}

fn vid_pid(id: &str) -> Option<String> {
    let u = id.to_uppercase();
    let vid = u.split("VID_").nth(1).map(|s| s.chars().take(4).collect::<String>())?;
    let pid = u.split("PID_").nth(1).map(|s| s.chars().take(4).collect::<String>())?;
    Some(format!("{vid}:{pid}"))
}

fn usb(index: &DeviceIndex) -> Vec<Item> {
    from_index(
        index,
        |d| matches!(d.bus.as_str(), "USB" | "USB4" | "USBSTOR") || d.class.eq_ignore_ascii_case("USB"),
        |d, f| {
            f.text("Vendor:Product ID", vid_pid(&d.id));
        },
    )
}

fn audio(index: &DeviceIndex) -> Vec<Item> {
    from_index(index, |d| matches!(d.class.as_str(), "MEDIA" | "AudioEndpoint"), |_, _| {})
}

fn input(index: &DeviceIndex) -> Vec<Item> {
    from_index(
        index,
        |d| {
            let n = d.name.to_lowercase();
            matches!(d.class.as_str(), "Keyboard" | "Mouse")
                || (d.class == "HIDClass" && (n.contains("touch pad") || n.contains("touchpad") || n.contains("touch screen") || n.contains("pen ")))
        },
        |_, _| {},
    )
}

fn cameras(index: &DeviceIndex) -> Vec<Item> {
    from_index(index, |d| matches!(d.class.as_str(), "Camera" | "Image"), |_, _| {})
}

fn bluetooth(index: &DeviceIndex) -> Vec<Item> {
    from_index(index, |d| d.class == "Bluetooth" || d.bus.starts_with("BTH"), |_, _| {})
}

fn all_devices(index: &DeviceIndex) -> Vec<Item> {
    let mut v = from_index(index, |_| true, |_, _| {});
    v.sort_by(|a, b| b.status.cmp(&a.status));
    v
}
