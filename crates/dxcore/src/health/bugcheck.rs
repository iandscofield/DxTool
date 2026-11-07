//! Blue screen stop codes explained in plain English.

pub struct StopCode {
    pub name: &'static str,
    pub meaning: &'static str,
    pub advice: &'static str,
}

const DRIVER: &str = "Update drivers, especially graphics, network, Wi-Fi and chipset drivers, from the PC or part maker's website. If it started after installing a program or device, remove it.";
const RAM: &str = "Test the memory with Windows Memory Diagnostic or MemTest86. If memory overclocking (XMP/EXPO) is turned on in the BIOS, try turning it off.";
const DISK: &str = "Back up important files now. Check the drive health section, reseat or replace the drive's cable, and update the drive's firmware.";
const HW: &str = "Usually hardware: remove any overclocking, check temperatures and dust, update the BIOS, and test the memory. A weak or failing power supply can also cause this.";
const GPU: &str = "Do a clean reinstall of the graphics driver, check graphics card temperatures and remove any graphics overclock. If it keeps happening the graphics card may be failing.";

pub fn lookup(code: u32) -> StopCode {
    // Codes with the 0x10000000 flag are the same error reported differently.
    let c = if code & 0x1000_0000 != 0 && code < 0xC000_0000 { code & 0x0FFF_FFFF } else { code };
    let (name, meaning, advice) = match c {
        0x0A => ("IRQL_NOT_LESS_OR_EQUAL", "A driver or part of Windows tried to use memory it was not allowed to.", DRIVER),
        0x18 => ("REFERENCE_BY_POINTER", "A driver kept using something Windows had already released.", DRIVER),
        0x19 => ("BAD_POOL_HEADER", "Windows' internal memory bookkeeping was damaged, usually by a driver.", DRIVER),
        0x1A => ("MEMORY_MANAGEMENT", "Windows found its memory management in a broken state. Faulty RAM is the most common cause.", RAM),
        0x1E => ("KMODE_EXCEPTION_NOT_HANDLED", "A driver hit an error it could not handle.", DRIVER),
        0x24 => ("NTFS_FILE_SYSTEM", "The file system on a drive has a problem.", "Run a disk check (chkdsk) on the drive, and check the drive health section."),
        0x3B => ("SYSTEM_SERVICE_EXCEPTION", "A driver or Windows service crashed while doing work for a program. Graphics drivers are a common cause.", DRIVER),
        0x4E => ("PFN_LIST_CORRUPT", "Windows' list of memory pages was damaged, by faulty RAM or a driver.", RAM),
        0x50 => ("PAGE_FAULT_IN_NONPAGED_AREA", "Something tried to read memory that did not exist. Causes include faulty RAM, a bad driver or antivirus software.", RAM),
        0x77 => ("KERNEL_STACK_INPAGE_ERROR", "Windows could not read part of itself back from the drive.", DISK),
        0x7A => ("KERNEL_DATA_INPAGE_ERROR", "Windows could not read data it needed from the drive. This often points to a failing drive or loose cable.", DISK),
        0x7B => ("INACCESSIBLE_BOOT_DEVICE", "Windows lost access to the drive it starts from.", "Check the drive health section. If the storage mode (AHCI/RAID/VMD) was changed in the BIOS, change it back."),
        0x7E => ("SYSTEM_THREAD_EXCEPTION_NOT_HANDLED", "A driver crashed inside Windows.", DRIVER),
        0x7F => ("UNEXPECTED_KERNEL_MODE_TRAP", "The processor hit an error Windows could not recover from, often caused by hardware or overheating.", HW),
        0x8E => ("KERNEL_MODE_EXCEPTION_NOT_HANDLED", "A driver or Windows component hit an error it could not handle.", DRIVER),
        0x9C => ("MACHINE_CHECK_EXCEPTION", "The processor reported a hardware error.", HW),
        0x9F => ("DRIVER_POWER_STATE_FAILURE", "A driver did not respond properly when the PC went to sleep, woke up or shut down.", "Update drivers (network, Wi-Fi, graphics, USB and chipset are the usual suspects) and the BIOS. Unplugging USB devices one at a time can find the culprit."),
        0xA0 => ("INTERNAL_POWER_ERROR", "Windows' power management hit a fatal error, often while hibernating.", "Update the BIOS and chipset drivers. Check there is enough free space on the system drive for hibernation."),
        0xA5 => ("ACPI_BIOS_ERROR", "The BIOS gave Windows information it could not use.", "Update the BIOS from the motherboard or PC maker's website."),
        0xBE => ("ATTEMPTED_WRITE_TO_READONLY_MEMORY", "A driver tried to change memory that is protected.", DRIVER),
        0xC2 => ("BAD_POOL_CALLER", "A driver asked Windows for memory in an invalid way.", DRIVER),
        0xC4 => ("DRIVER_VERIFIER_DETECTED_VIOLATION", "Driver Verifier (a testing tool) caught a driver misbehaving.", "Driver Verifier is turned on. A technician can read the crash dump to see which driver it caught, then turn Verifier off with 'verifier /reset'."),
        0xC5 => ("DRIVER_CORRUPTED_EXPOOL", "A driver damaged Windows' memory pool.", DRIVER),
        0xD1 => ("DRIVER_IRQL_NOT_LESS_OR_EQUAL", "A driver tried to use memory it was not allowed to. Network and Wi-Fi drivers are frequent causes.", DRIVER),
        0xD5 => ("DRIVER_PAGE_FAULT_IN_FREED_SPECIAL_POOL", "A driver used memory after giving it back.", DRIVER),
        0xE2 | 0xDEAD_DEAD => ("MANUALLY_INITIATED_CRASH", "Someone deliberately triggered this crash, usually for troubleshooting.", "Nothing to fix unless nobody meant to do this."),
        0xEF => ("CRITICAL_PROCESS_DIED", "A process Windows cannot run without stopped. Causes include damaged system files, a failing drive or bad drivers.", "Run the system file repair in Fixes, check the drive health section, and update drivers."),
        0xF4 => ("CRITICAL_OBJECT_TERMINATION", "An essential Windows process ended unexpectedly. This often points to a drive or storage problem.", DISK),
        0xFC => ("ATTEMPTED_EXECUTE_OF_NOEXECUTE_MEMORY", "A driver tried to run code from memory that holds data.", DRIVER),
        0x101 => ("CLOCK_WATCHDOG_TIMEOUT", "One of the processor cores stopped responding.", HW),
        0x109 => ("CRITICAL_STRUCTURE_CORRUPTION", "Core Windows code or data was changed. Faulty RAM, a bad driver or malware can cause this.", RAM),
        0x10D => ("WDF_VIOLATION", "A driver built on Microsoft's driver framework broke its rules. USB, Thunderbolt and keyboard/mouse software drivers are common causes.", DRIVER),
        0x113 => ("VIDEO_DXGKRNL_FATAL_ERROR", "The Windows graphics system hit a fatal error.", GPU),
        0x116 => ("VIDEO_TDR_FAILURE", "The graphics driver stopped responding and could not be restarted.", GPU),
        0x117 => ("VIDEO_TDR_TIMEOUT_DETECTED", "The graphics driver stopped responding.", GPU),
        0x119 => ("VIDEO_SCHEDULER_INTERNAL_ERROR", "The graphics scheduler hit a fatal error.", GPU),
        0x124 => ("WHEA_UNCORRECTABLE_ERROR", "The hardware reported an error it could not correct. Usually the processor, memory, overheating, overclocking or the power supply.", HW),
        0x12B => ("FAULTY_HARDWARE_CORRUPTED_PAGE", "Windows found memory that changed by itself. This points to faulty RAM.", RAM),
        0x133 => ("DPC_WATCHDOG_VIOLATION", "A driver took far too long to finish its work. Storage drivers and SSD firmware are common causes.", "Update SSD firmware, storage (SATA/NVMe/RST) and chipset drivers, then other drivers."),
        0x139 => ("KERNEL_SECURITY_CHECK_FAILURE", "Windows detected damaged internal data, usually from a driver or faulty RAM.", DRIVER),
        0x13A => ("KERNEL_MODE_HEAP_CORRUPTION", "A driver damaged Windows' memory heap.", DRIVER),
        0x144 => ("BUGCODE_USB3_DRIVER", "The USB 3 driver hit a fatal error, often triggered by a USB device.", "Unplug USB devices and hubs to find the one causing it, and update chipset and USB drivers."),
        0x14F => ("PDC_WATCHDOG_TIMEOUT", "A component did not respond while the PC was going into or out of standby.", "Update the BIOS, chipset and Wi-Fi drivers."),
        0x154 => ("UNEXPECTED_STORE_EXCEPTION", "Windows could not read compressed memory back, often because of a failing drive or faulty RAM.", DISK),
        0x1D3 => ("DRIVER_PNP_WATCHDOG", "A driver took too long while a device was being set up.", DRIVER),
        0xC000_021A => ("STATUS_SYSTEM_PROCESS_TERMINATED", "A core Windows process failed, often because of damaged system files.", "Run the system file repair in Fixes. If it started after an update or new program, undo that."),
        _ => ("", "", ""),
    };
    if name.is_empty() {
        StopCode {
            name: "Uncommon stop code",
            meaning: "This stop code is less common. A technician can open the crash dump with WinDbg to see which driver caused it.",
            advice: "Update drivers and Windows, and note when the crash happens (gaming, sleep, startup) to narrow it down.",
        }
    } else {
        StopCode { name, meaning, advice }
    }
}

/// Finds the stop code in a message like "The bugcheck was: 0x0000009f (0x..., ...)".
pub fn parse_from_text(s: &str) -> Option<(u32, Vec<String>)> {
    let p = s.to_lowercase().find("0x")?;
    let rest = &s[p + 2..];
    let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    let code = u64::from_str_radix(&hex, 16).ok()? as u32;
    let params = rest
        .find('(')
        .and_then(|o| rest[o + 1..].find(')').map(|c| &rest[o + 1..o + 1 + c]))
        .map(|inner| inner.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_default();
    Some((code, params))
}

/// Reads the stop code and parameters from a minidump or full memory dump header.
pub fn read_dump_header(path: &std::path::Path) -> Option<(u32, [u64; 4])> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = [0u8; 0x60];
    f.read_exact(&mut h).ok()?;
    let u32_at = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().unwrap());
    let u64_at = |o: usize| u64::from_le_bytes(h[o..o + 8].try_into().unwrap());
    if &h[0..8] == b"PAGEDU64" {
        Some((u32_at(0x38), [u64_at(0x40), u64_at(0x48), u64_at(0x50), u64_at(0x58)]))
    } else if &h[0..8] == b"PAGEDUMP" {
        Some((u32_at(0x20), [u32_at(0x24) as u64, u32_at(0x28) as u64, u32_at(0x2C) as u64, u32_at(0x30) as u64]))
    } else {
        None
    }
}
