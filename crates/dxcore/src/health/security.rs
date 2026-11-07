//! Security checks: antivirus, firewall and device security settings.

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

use super::{field, item, num_field, plural, Ctx};
use crate::model::{CheckResult, Finding, Status};
use crate::platform;
use crate::wmiutil::{ns_query, raw_section, RowExt};

fn reg_dword(path: &str, name: &str) -> Option<u32> {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path).ok()?.get_value::<u32, _>(name).ok()
}

/// Security Center product state: (turned on, definitions up to date).
fn product_state(state: u64) -> (bool, bool) {
    let on = matches!((state >> 8) & 0xFF, 0x10 | 0x11);
    let current = state & 0xFF == 0x00;
    (on, current)
}

pub fn antivirus(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let products = ns_query("ROOT\\SecurityCenter2", "SELECT * FROM AntiVirusProduct");
    let mp = ns_query("ROOT\\Microsoft\\Windows\\Defender", "SELECT * FROM MSFT_MpComputerStatus");
    let mp = mp.first();

    let mut active: Vec<String> = Vec::new();
    for (i, p) in products.iter().enumerate() {
        let name = p.s("displayName").unwrap_or_else(|| "Antivirus".into());
        let (on, current) = product_state(p.u("productState").unwrap_or(0));
        if on {
            active.push(name.clone());
        }
        let mut it = item(
            format!("av{i}"),
            name.clone(),
            p.s("pathToSignedProductExe").unwrap_or_default(),
            vec![field("Turned on", if on { "Yes" } else { "No" }), field("Definitions", if current { "Up to date" } else { "Out of date" }), field("Last updated", p.s("timestamp").unwrap_or_default())],
        );
        it.status = if on && current { Status::Ok } else if on { Status::Warn } else { Status::Info };
        it.sections.push(raw_section("AntiVirusProduct", p));
        r.items.push(it);
    }
    let third_party: Vec<&String> = active.iter().filter(|n| !n.to_lowercase().contains("defender")).collect();

    let defender_on = mp.and_then(|m| m.b("RealTimeProtectionEnabled")).unwrap_or(false) && mp.and_then(|m| m.b("AntivirusEnabled")).unwrap_or(false);
    if let Some(m) = mp {
        let mode = m.s("AMRunningMode").unwrap_or_default();
        r.facts.push(field("Microsoft Defender", if defender_on { format!("On ({mode})") } else if mode.is_empty() { "Off".into() } else { format!("Off ({mode})") }));
        if let Some(age) = m.u("AntivirusSignatureAge").filter(|a| *a < 10_000) {
            r.facts.push(num_field("Definitions age", plural(age as usize, "day", "days"), age as f64));
            if defender_on && age > 7 {
                r.findings.push(
                    Finding::new("defender_old_defs", Status::Warn, format!("Virus definitions are {age} days old"), "Antivirus can only catch threats it knows about. Definitions normally update every day.")
                        .advice("Open Windows Security → Virus & threat protection → Protection updates and click 'Check for updates'."),
                );
            }
        }
        if let Some(q) = m.u("QuickScanAge").filter(|a| *a < 10_000) {
            r.facts.push(num_field("Last quick scan", format!("{} ago", plural(q as usize, "day", "days")), q as f64));
            if defender_on && q > 30 {
                r.findings.push(Finding::new("defender_no_scan", Status::Info, format!("No virus scan in {q} days"), "Real-time protection checks files as they are used, but a periodic scan catches anything that slipped through.").advice("Run a quick scan from Windows Security, or from the Security section of DxTool once virus scanning arrives."));
            }
        }
        if let Some(f) = m.u("FullScanAge").filter(|a| *a < 10_000) {
            r.facts.push(num_field("Last full scan", format!("{} ago", plural(f as usize, "day", "days")), f as f64));
        }
        if defender_on && m.b("IsTamperProtected") == Some(false) {
            r.findings.push(Finding::new("tamper_off", Status::Info, "Tamper Protection is off", "Tamper Protection stops malware from switching Defender off. It is on by default.").advice("Turn it on in Windows Security → Virus & threat protection settings."));
        }
    }
    if active.is_empty() && !defender_on {
        r.findings.push(
            Finding::new("no_av", Status::Error, "No antivirus protection is turned on", "Nothing is checking files and downloads for viruses and other malware.")
                .advice("Turn on real-time protection in Windows Security, or install and enable another antivirus."),
        );
    }
    if third_party.len() > 1 {
        r.findings.push(
            Finding::new("multiple_av", Status::Warn, format!("{} antivirus programs are running at once", third_party.len()), "Several antivirus programs fighting over the same files slows the PC down and can cause crashes.")
                .advice("Keep one antivirus and uninstall the others.")
                .evidence(vec![field("Programs", third_party.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))]),
        );
    }

    // Recent threat detections (may need administrator rights)
    let threats = ns_query("ROOT\\Microsoft\\Windows\\Defender", "SELECT * FROM MSFT_MpThreat");
    let detections = ns_query("ROOT\\Microsoft\\Windows\\Defender", "SELECT * FROM MSFT_MpThreatDetection");
    let mut active_threats = 0;
    for (i, d) in detections.iter().enumerate() {
        let tid = d.s("ThreatID").unwrap_or_default();
        let t = threats.iter().find(|t| t.s("ThreatID").as_deref() == Some(tid.as_str()));
        let name = t.and_then(|t| t.s("ThreatName")).unwrap_or_else(|| format!("Threat {tid}"));
        let is_active = t.and_then(|t| t.b("IsActive")).unwrap_or(false);
        if is_active {
            active_threats += 1;
        }
        let when = d.s("InitialDetectionTime").and_then(|s| crate::fmt::wmi_datetime(&s)).unwrap_or_default();
        let mut it = item(
            format!("threat{i}"),
            name.clone(),
            when.clone(),
            vec![
                field("When", when),
                field("Still active", crate::fmt::yes_no(is_active)),
                field("Removed", d.b("ActionSuccess").map(crate::fmt::yes_no).unwrap_or("Unknown")),
                field("Files", d.list("Resources").join(", ")),
                field("Found by", d.s("ProcessName").unwrap_or_default()),
            ],
        );
        it.status = if is_active { Status::Error } else { Status::Info };
        it.sections.push(raw_section("MSFT_MpThreatDetection", d));
        r.items.push(it);
    }
    if active_threats > 0 {
        r.findings.push(
            Finding::new("active_threats", Status::Error, format!("Microsoft Defender reports {} that still need action", plural(active_threats, "threat", "threats")), "Defender found malware or unwanted software that has not been fully removed.")
                .advice("Open Windows Security → Virus & threat protection → Protection history and remove or quarantine the threats, then run a full scan."),
        );
    } else if !detections.is_empty() {
        r.facts.push(num_field("Past detections (handled)", detections.len().to_string(), detections.len() as f64));
    }
    r.items_label = Some("Antivirus and detections".into());
    r.summary = if active.is_empty() && !defender_on {
        "No antivirus turned on".into()
    } else if !third_party.is_empty() {
        format!("Protected by {}", third_party.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
    } else {
        "Protected by Microsoft Defender".into()
    };
    Ok(())
}

pub fn firewall(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let profiles = ns_query("ROOT\\StandardCimv2", "SELECT Name, Enabled, DefaultInboundAction FROM MSFT_NetFirewallProfile");
    let third: Vec<String> = ns_query("ROOT\\SecurityCenter2", "SELECT displayName, productState FROM FirewallProduct")
        .iter()
        .filter(|p| product_state(p.u("productState").unwrap_or(0)).0)
        .filter_map(|p| p.s("displayName"))
        .collect();
    // Which profile the current network uses: 0 Public, 1 Private, 2 Domain
    let current: Vec<&str> = ns_query("ROOT\\StandardCimv2", "SELECT NetworkCategory FROM MSFT_NetConnectionProfile")
        .iter()
        .map(|c| match c.u("NetworkCategory").unwrap_or(0) {
            1 => "Private",
            2 => "Domain",
            _ => "Public",
        })
        .collect();
    let mut off = Vec::new();
    for (i, p) in profiles.iter().enumerate() {
        let name = p.s("Name").unwrap_or_default();
        let on = p.u("Enabled").unwrap_or(1) != 0;
        if !on {
            off.push(name.clone());
        }
        let in_use = current.contains(&name.as_str());
        let mut it = item(format!("fw{i}"), format!("{name} networks"), if in_use { "In use now" } else { "" }, vec![field("Firewall", if on { "On" } else { "Off" }), field("In use now", crate::fmt::yes_no(in_use))]);
        it.status = if on { Status::Ok } else { Status::Warn };
        r.items.push(it);
    }
    if !third.is_empty() {
        r.facts.push(field("Other firewall", third.join(", ")));
    }
    if !off.is_empty() && third.is_empty() {
        let in_use_off = off.iter().any(|o| current.contains(&o.as_str()));
        r.findings.push(
            Finding::new(
                "firewall_off",
                if in_use_off { Status::Error } else { Status::Warn },
                format!("Windows Firewall is off for {} networks", off.join(" and ")),
                if in_use_off { "The firewall is off on the network this PC is connected to, so other devices on that network can reach it more easily." } else { "The firewall is off for some network types. It would not protect the PC on those networks." },
            )
            .advice("Turn the firewall on in Windows Security → Firewall & network protection."),
        );
    }
    r.items_label = Some("Firewall profiles".into());
    r.summary = if off.is_empty() {
        "Firewall on for all networks".into()
    } else if !third.is_empty() {
        format!("Protected by {}", third.join(", "))
    } else {
        format!("Firewall off for {} networks", off.join(", "))
    };
    Ok(())
}

pub fn device_security(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let mut rows = Vec::new();
    let mut row = |name: &str, value: String, status: Status| {
        let mut it = item(name.to_lowercase().replace(' ', "_"), name, value.clone(), vec![field("Setting", name), field("State", value)]);
        it.status = status;
        rows.push(it);
    };
    let mut findings = Vec::new();

    // Secure Boot
    let firmware = platform::firmware_type();
    let sb = reg_dword("SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State", "UEFISecureBootEnabled");
    match (firmware, sb) {
        ("Legacy BIOS", _) => {
            row("Secure Boot", "Not available (legacy BIOS mode)".into(), Status::Warn);
            findings.push(Finding::new("legacy_bios", Status::Warn, "The PC starts in old legacy BIOS mode", "Legacy mode cannot use Secure Boot, which blocks malware that loads before Windows. Windows 11 requires UEFI mode.").advice("A technician can convert the drive with MBR2GPT and switch the BIOS to UEFI mode. Back up first."));
        }
        (_, Some(1)) => row("Secure Boot", "On".into(), Status::Ok),
        _ => {
            row("Secure Boot", "Off".into(), Status::Warn);
            findings.push(Finding::new("secure_boot_off", Status::Warn, "Secure Boot is off", "Secure Boot makes sure only trusted software runs while the PC starts, blocking a type of malware that hides below Windows.").advice("Turn Secure Boot on in the BIOS/UEFI settings (usually under Boot or Security)."));
        }
    }

    // TPM
    // Both of these wait 5 seconds and then fail without administrator rights.
    let tpm = if ctx.is_admin { ns_query("ROOT\\CIMV2\\Security\\MicrosoftTpm", "SELECT * FROM Win32_Tpm") } else { Vec::new() };
    if let Some(t) = tpm.first() {
        let enabled = t.b("IsEnabled_InitialValue").unwrap_or(false);
        let ver = t.s("SpecVersion").map(|v| v.split(',').next().unwrap_or("").trim().to_string()).unwrap_or_default();
        row("TPM", format!("{} · version {ver}", if enabled { "On" } else { "Off" }), if enabled { Status::Ok } else { Status::Warn });
        if !enabled {
            findings.push(Finding::new("tpm_off", Status::Warn, "The TPM security chip is turned off", "The TPM stores encryption keys for BitLocker and Windows Hello, and Windows 11 requires it.").advice("Turn on TPM (also called PTT on Intel or fTPM on AMD) in the BIOS settings."));
        }
    } else {
        let dev = ns_query("ROOT\\CIMV2", "SELECT Name FROM Win32_PnPEntity WHERE PNPClass = 'SecurityDevices'");
        match dev.iter().filter_map(|d| d.s("Name")).find(|n| n.contains("Trusted Platform")) {
            Some(n) => row("TPM", format!("Present ({n})"), Status::Ok),
            None if ctx.is_admin => {
                row("TPM", "Not found".into(), Status::Warn);
                findings.push(Finding::new("tpm_missing", Status::Warn, "No TPM security chip found", "The TPM stores encryption keys for BitLocker and Windows Hello, and Windows 11 requires it. Most PCs from 2016 onward have one that can be turned on in the BIOS.").advice("Look for TPM, PTT (Intel) or fTPM (AMD) in the BIOS settings and turn it on."));
            }
            None => row("TPM", "Unknown (needs administrator rights)".into(), Status::Info),
        }
    }

    // BitLocker (needs administrator rights)
    let vols = if ctx.is_admin { ns_query("ROOT\\CIMV2\\Security\\MicrosoftVolumeEncryption", "SELECT DriveLetter, ProtectionStatus, ConversionStatus FROM Win32_EncryptableVolume") } else { Vec::new() };
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    if vols.is_empty() {
        if !ctx.is_admin {
            row("BitLocker", "Unknown (needs administrator rights)".into(), Status::Info);
            r.limited = Some("BitLocker and TPM details need administrator rights.".into());
        } else {
            row("BitLocker", "Not available on this edition of Windows".into(), Status::Info);
        }
    } else {
        let mut sys_on = false;
        for v in &vols {
            let letter = v.s("DriveLetter").unwrap_or_default();
            let on = v.u("ProtectionStatus") == Some(1);
            if letter.eq_ignore_ascii_case(&sys) {
                sys_on = on;
            }
            if !letter.is_empty() {
                row(&format!("BitLocker {letter}"), if on { "Encrypted".into() } else { "Not encrypted".to_string() }, if on { Status::Ok } else { Status::Info });
            }
        }
        if sys_on {
            findings.push(Finding::new("bitlocker_on", Status::Info, format!("Drive {sys} is encrypted with BitLocker"), "Good for privacy if the PC is lost. Before BIOS updates, hardware changes or boot repairs, make sure the recovery key is available or the PC may ask for it.").advice("Check the recovery key is saved, for example at account.microsoft.com/devices/recoverykey."));
        } else {
            findings.push(Finding::new("bitlocker_off", Status::Info, format!("Drive {sys} is not encrypted"), "If this PC is lost or stolen, someone could remove the drive and read the files on it. This matters most for laptops.").advice("Turn on BitLocker or Device Encryption in Settings → Privacy & security, and save the recovery key somewhere safe."));
        }
    }

    // User Account Control
    let pol = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
    let lua = reg_dword(pol, "EnableLUA").unwrap_or(1);
    let consent = reg_dword(pol, "ConsentPromptBehaviorAdmin").unwrap_or(5);
    if lua == 0 {
        row("User Account Control", "Off".into(), Status::Error);
        findings.push(Finding::new("uac_off", Status::Error, "User Account Control is turned off", "Every program runs with full control of the PC, so malware can change anything without asking.").advice("Turn UAC back on in Control Panel → User Accounts → Change User Account Control settings, then restart."));
    } else if consent == 0 {
        row("User Account Control", "On, but never notifies".into(), Status::Warn);
        findings.push(Finding::new("uac_silent", Status::Warn, "User Account Control never asks for permission", "Programs can get administrator rights silently, which makes it easy for malware to take over.").advice("Move the UAC slider back to the default level."));
    } else {
        row("User Account Control", "On".into(), Status::Ok);
    }

    // Memory integrity (HVCI)
    match reg_dword("SYSTEM\\CurrentControlSet\\Control\\DeviceGuard\\Scenarios\\HypervisorEnforcedCodeIntegrity", "Enabled") {
        Some(1) => row("Memory integrity", "On".into(), Status::Ok),
        _ => {
            row("Memory integrity", "Off".into(), Status::Info);
            findings.push(Finding::new("hvci_off", Status::Info, "Memory integrity is off", "Memory integrity stops malicious code from getting into the core of Windows. Some older drivers and games are incompatible with it.").advice("Turn it on in Windows Security → Device security → Core isolation, if your drivers allow."));
        }
    }

    // Remote Desktop
    if reg_dword("SYSTEM\\CurrentControlSet\\Control\\Terminal Server", "fDenyTSConnections") == Some(0) {
        row("Remote Desktop", "On".into(), Status::Info);
        findings.push(Finding::new("rdp_on", Status::Info, "Remote Desktop is turned on", "Other computers can sign in to this one remotely. Fine if you use it; otherwise it is an unnecessary way in.").advice("Turn it off in Settings → System → Remote Desktop if you don't use it."));
    } else {
        row("Remote Desktop", "Off".into(), Status::Ok);
    }

    // SMB1
    let smb1 = reg_dword("SYSTEM\\CurrentControlSet\\Services\\mrxsmb10", "Start").map(|s| s != 4).unwrap_or(false);
    if smb1 {
        row("SMB1 file sharing", "On".into(), Status::Warn);
        findings.push(Finding::new("smb1_on", Status::Warn, "Old SMB1 file sharing is turned on", "SMB1 is a decades-old file sharing protocol abused by ransomware such as WannaCry. Very few devices still need it.").advice("Turn off 'SMB 1.0/CIFS File Sharing Support' in Windows Features, unless an old NAS or printer needs it."));
    } else {
        row("SMB1 file sharing", "Off".into(), Status::Ok);
    }

    let issues = findings.iter().filter(|f| f.severity >= Status::Warn).count();
    r.findings.extend(findings);
    r.items.extend(rows);
    r.items_label = Some("Security settings".into());
    r.summary = if issues == 0 { "Security settings look good".into() } else { format!("{} to review", plural(issues, "setting", "settings")) };
    Ok(())
}
