//! Upgrade recommendations, worked out offline from the hardware inventory.
//!
//! Each rule looks at a few inventory facts and, when something would clearly help,
//! adds an [`Upgrade`]: what to do, why, roughly how much it costs, and a shop search
//! phrase for the part. Things that are already fine go in `good` so the page can
//! reassure as well as advise.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::model::{Inventory, Item, Upgrade, UpgradeReport};

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

// ───────────────────────── inventory helpers ─────────────────────────

fn items<'a>(inv: &'a Inventory, cat: &str) -> &'a [Item] {
    inv.categories.iter().find(|c| c.id == cat).map(|c| c.items.as_slice()).unwrap_or(&[])
}

fn item<'a>(inv: &'a Inventory, cat: &str, id: &str) -> Option<&'a Item> {
    items(inv, cat).iter().find(|i| i.id == id)
}

/// A curated field (summary first, then the friendly sections).
fn field<'a>(it: &'a Item, key: &str) -> Option<&'a crate::model::Field> {
    it.summary.iter().chain(it.sections.iter().filter(|s| !s.raw).flat_map(|s| s.fields.iter())).find(|f| f.key == key)
}

fn text<'a>(it: &'a Item, key: &str) -> Option<&'a str> {
    field(it, key).map(|f| f.display.as_str())
}

fn num(it: &Item, key: &str) -> Option<f64> {
    field(it, key).and_then(|f| f.sort.as_f64())
}

fn gb(bytes: f64) -> String {
    let g = bytes / GB;
    if g >= 10.0 || (g - g.round()).abs() < 0.05 { format!("{g:.0} GB") } else { format!("{g:.1} GB") }
}

/// Smallest common memory stick size (in bytes) that is at least `bytes`.
fn stick_size(bytes: f64) -> f64 {
    [4.0, 8.0, 16.0, 32.0, 48.0, 64.0].iter().map(|g| g * GB).find(|&s| s >= bytes - 0.1 * GB).unwrap_or(64.0 * GB)
}

/// "Intel(R) Core(TM) i5-6200U CPU @ 2.30GHz" → "Intel Core i5-6200U".
pub fn clean_cpu(name: &str) -> String {
    let n = name.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    let n = n.split(" @ ").next().unwrap_or(&n).trim().trim_end_matches(" CPU").trim();
    n.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ───────────────────────── facts worth parsing ─────────────────────────

/// Speed a memory kit is sold as (its XMP/EXPO profile), read from common part numbers:
/// Corsair CMK32GX5M2B6000C36, G.Skill F5-6000J3036F16G, Kingston KF560C36BBE-16,
/// Crucial CP16G60C36U5B / BL2K16G32C16U4B, TeamGroup FF3D516G6000C38A.
pub fn kit_speed(part: &str) -> Option<u32> {
    let p = part.trim().to_uppercase();
    let digits_after = |s: &str, n: usize| -> Option<u32> {
        let d: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        (d.len() == n).then(|| d.parse().ok()).flatten()
    };
    let plausible = |v: u32| (1600..=10000).contains(&v).then_some(v);
    if p.starts_with("CM") {
        // Corsair: ...GX5M2B6000C36 / ...GX4M2D3600C18 → the 4 digits before the last 'C'.
        let i = p.rfind('C')?;
        let before: String = p[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        return before.parse().ok().and_then(plausible);
    }
    if let Some(rest) = p.strip_prefix("F5-").or_else(|| p.strip_prefix("F4-")).or_else(|| p.strip_prefix("F3-")) {
        return digits_after(rest, 4).and_then(plausible);
    }
    if let Some(rest) = p.strip_prefix("KF5").or_else(|| p.strip_prefix("KF4")).or_else(|| p.strip_prefix("KF3")) {
        // Kingston Fury: KF560C36 → 56 × 100; KF432C16 → 3200.
        let d: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return match d.len() {
            2 => d.parse::<u32>().ok().map(|v| v * 100).and_then(plausible),
            3 => d[..2].parse::<u32>().ok().map(|v| v * 100).and_then(plausible),
            _ => None,
        };
    }
    if p.starts_with("CP") || p.starts_with("BL") {
        // Crucial / Ballistix: ...16G60C36... → 6000, ...16G32C16... → 3200.
        let i = p.find('G')?;
        let d: String = p[i + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
        return (d.len() == 2).then(|| d.parse::<u32>().ok().map(|v| v * 100)).flatten().and_then(plausible);
    }
    if p.starts_with("FF") || p.starts_with("TF") || p.starts_with("TLZ") {
        // TeamGroup T-Force: FF3D516G6000C38A → the 4 digits before the last 'C'.
        let i = p.rfind('C')?;
        let d: String = p[..i].chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        return d.parse().ok().and_then(plausible);
    }
    None
}

#[derive(Debug, PartialEq)]
pub enum Win11Cpu {
    Supported,
    Unsupported,
    Unknown,
}

/// Intel generation from names like "Intel(R) Core(TM) i5-8250U" (8) or
/// "13th Gen Intel(R) Core(TM) i7-13700F" (13). None for other Intel lines.
pub fn intel_core_gen(name: &str) -> Option<u32> {
    let n = name.to_lowercase();
    let i = ["i3-", "i5-", "i7-", "i9-"].iter().find_map(|k| n.find(k).map(|i| i + 3))?;
    let d: String = n[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
    crate::games::perf::intel_gen_from_model(&d)
}

/// Whether the processor is on Microsoft's Windows 11 list (rule of thumb: Intel 8th gen
/// and newer, AMD Ryzen 2000 and newer).
pub fn win11_cpu(name: &str) -> Win11Cpu {
    let n = name.to_lowercase();
    if n.contains("core(tm) ultra") || n.contains("core ultra") || n.contains("ryzen ai") || n.contains("snapdragon") {
        return Win11Cpu::Supported;
    }
    if let Some(g) = intel_core_gen(name) {
        return if g >= 8 { Win11Cpu::Supported } else { Win11Cpu::Unsupported };
    }
    // New Intel naming without the "i": "Intel(R) Core(TM) 5 120U", "Intel Core 7 150U".
    if n.contains("intel") && ["core(tm) 3 ", "core(tm) 5 ", "core(tm) 7 ", "core 3 ", "core 5 ", "core 7 "].iter().any(|k| n.contains(k)) {
        return Win11Cpu::Supported;
    }
    if let Some(i) = n.find("ryzen") {
        // "AMD Ryzen 7 5800X" → model 5800 → series 5. Series 1 is unsupported.
        let tail = &n[i + 5..];
        let model = tail.split_whitespace().nth(1).unwrap_or("");
        let d: String = model.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Some(series) = d.chars().next().and_then(|c| c.to_digit(10)) {
            return if d.len() >= 4 && series >= 2 { Win11Cpu::Supported } else if series == 1 { Win11Cpu::Unsupported } else { Win11Cpu::Unknown };
        }
    }
    if n.contains("amd fx") || n.contains("phenom") || n.contains("athlon ii") || n.contains("core(tm)2") || n.contains("core 2") || n.contains("pentium(r) d") {
        return Win11Cpu::Unsupported;
    }
    Win11Cpu::Unknown
}

// ───────────────────────── the rules ─────────────────────────

struct Ctx<'a> {
    inv: &'a Inventory,
    laptop: bool,
    tpm_unknown: bool,
    out: Vec<Upgrade>,
    good: Vec<String>,
}

impl Ctx<'_> {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, id: &str, area: &str, priority: &str, cost: &str, title: String, why: String, how: String, search: Option<String>) {
        self.out.push(Upgrade { id: id.into(), area: area.into(), priority: priority.into(), cost: cost.into(), title, why, how, search });
    }
}

fn memory(c: &mut Ctx) {
    let mods: Vec<&Item> = items(c.inv, "memory").iter().filter(|i| i.id.starts_with("dimm")).collect();
    let arr = item(c.inv, "memory", "memarray");
    let total: f64 = mods.iter().filter_map(|m| num(m, "Capacity")).sum();
    if total <= 0.0 {
        return;
    }
    let slots = arr.and_then(|a| num(a, "Total slots")).map(|v| v as usize).filter(|&s| s >= mods.len() && s > 0);
    let max = arr.and_then(|a| num(a, "Maximum supported")).filter(|&m| m >= total);
    let kind = mods.iter().find_map(|m| text(m, "Type")).unwrap_or("").to_string();
    let speed = mods.iter().filter_map(|m| num(m, "Rated speed")).fold(0.0, f64::max);
    let running = mods.iter().filter_map(|m| num(m, "Running at")).filter(|v| *v > 0.0).fold(f64::MAX, f64::min);
    let module_size = mods.iter().filter_map(|m| num(m, "Capacity")).fold(0.0, f64::max);
    let free = slots.map(|s| s - mods.len()).unwrap_or(0);
    let laptop_note = if c.laptop { " On laptops, check first that the memory isn't soldered to the board (the maker's manual or a quick search for the model will say)." } else { "" };
    let kind_txt = if kind.is_empty() { "memory".to_string() } else { kind.clone() };
    let speed_txt = if speed > 0.0 { format!("-{speed:.0}") } else { String::new() };
    let stick = if c.laptop { "SO-DIMM laptop memory" } else { "desktop memory" };

    // 1. Memory sold faster than it runs (XMP/EXPO switched off). Free.
    if running < f64::MAX {
        let rated_kit = mods.iter().filter_map(|m| text(m, "Part number")).filter_map(kit_speed).max();
        if let Some(k) = rated_kit {
            if (k as f64) > running + 200.0 && !c.laptop {
                let ddr5_four = mods.len() >= 4 && kind == "DDR5";
                let profile = if c.inv.categories.iter().flat_map(|x| x.items.iter()).any(|i| i.id == "cpu0" && text(i, "Manufacturer").is_some_and(|m| m.contains("AMD"))) { "EXPO (or DOCP)" } else { "XMP" };
                c.add(
                    "memory_xmp",
                    "Memory",
                    "high",
                    "Free",
                    format!("Turn on {profile} so the memory runs at its full {k} MT/s"),
                    format!("The memory installed is sold as {k} MT/s, but it is running at {running:.0} MT/s, the safe default speed. Memory only runs at the speed on the box when the {profile} profile is switched on in the BIOS. Faster memory helps games and anything that uses integrated graphics."),
                    format!("Restart, open the BIOS setup (usually Delete or F2 at start-up), find {profile} or \"memory profile\", choose Profile 1, then save and exit. If the PC won't start afterwards, it resets itself after a few tries, or clear the BIOS settings.{}", if ddr5_four { " With four DDR5 sticks many PCs can't hold the full speed: if games crash or the PC freezes afterwards, set the memory speed to 5600 or 5200 MT/s instead (still much faster than now)." } else { "" }),
                    None,
                );
            }
        }
    }

    // 2. Not enough memory.
    let need = if total < 7.5 * GB { Some(("high", 16.0)) } else if total < 15.5 * GB { Some(("medium", 16.0)) } else { None };
    if let Some((priority, target)) = need {
        let target_b = target * GB;
        let why = if total < 7.5 * GB {
            format!("This PC has {} of memory. Windows 10 and 11 with a web browser open already use most of that, so the PC slows down by using the drive instead.", gb(total))
        } else {
            format!("This PC has {} of memory. That works for light use, but a browser with many tabs, Office and Teams together use more, and 16 GB is now the comfortable amount.", gb(total))
        };
        let how;
        let search;
        let add = target_b - total;
        let per_free = if free > 0 { stick_size((add / free as f64).max(module_size)) } else { 0.0 };
        // Adding a stick much bigger than the ones there makes a lopsided mix: suggest a matched kit instead.
        let lopsided = per_free > module_size * 2.0 + 0.1 * GB;
        if free > 0 && !lopsided && max.is_none_or(|m| m >= target_b) {
            let per = per_free;
            let n = (add / per).ceil().clamp(1.0, free as f64) as usize;
            how = format!(
                "There {} {free} empty memory slot{}. Add {n} × {} {kind_txt}{speed_txt} {stick}, ideally the same speed as what's there, to reach about {}.{laptop_note}",
                if free == 1 { "is" } else { "are" },
                if free == 1 { "" } else { "s" },
                gb(per),
                gb(total + per * n as f64)
            );
            search = Some(format!("{} {kind_txt}{speed_txt} {stick}", gb(per)).trim().to_string());
        } else if slots.is_some() && max.is_none_or(|m| m >= target_b) {
            let n = slots.unwrap().clamp(1, 2);
            let per = stick_size(target_b / n as f64);
            how = format!("{}replace the current memory with a {n} × {} {kind_txt}{speed_txt} kit ({} total). Matched pairs run fastest (dual-channel).{laptop_note}", if free == 0 { "All memory slots are full, so " } else { "For the best result, " }, gb(per), gb(per * n as f64));
            search = Some(format!("{} ({n}x{}) {kind_txt}{speed_txt} {stick}", gb(per * n as f64), gb(per)));
        } else {
            how = format!("Add or replace memory to reach 16 GB, using {kind_txt} {stick}.{} {laptop_note}", max.map(|m| format!(" The motherboard supports up to {}.", gb(m))).unwrap_or_default());
            search = Some(format!("16GB {kind_txt}{speed_txt} {stick}"));
        }
        c.add("memory_more", "Memory", priority, "Low cost", format!("Add memory: {} → {}", gb(total), gb(target_b)), why, how, search);
    } else {
        c.good.push(format!("{} of memory is plenty for everyday use.", gb(total)));
    }

    // 3. One stick only: single-channel halves memory bandwidth.
    if mods.len() == 1 && slots.is_some_and(|s| s >= 2) && need.is_none() {
        c.add(
            "memory_dual",
            "Memory",
            "low",
            "Low cost",
            "Add a second matching memory stick for dual-channel speed".into(),
            format!("There's only one memory stick, so the memory runs in single-channel mode at half its possible speed. Built-in graphics and many games feel this the most."),
            format!("Add a second {} {kind_txt}{speed_txt} {stick}, the same size and speed as the one installed.{laptop_note}", gb(module_size)),
            Some(format!("{} {kind_txt}{speed_txt} {stick}", gb(module_size))),
        );
    }
}

fn storage(c: &mut Ctx) {
    let disks = items(c.inv, "storage");
    let system = disks.iter().find(|d| text(d, "System drive") == Some("Yes") || text(d, "Windows boots from this drive") == Some("Yes") || text(d, "Drive letters").is_some_and(|l| l.split_whitespace().any(|x| x.eq_ignore_ascii_case("C:"))));

    for d in disks {
        let health = text(d, "Health").unwrap_or("");
        if health == "Warning" || health == "Unhealthy" {
            let name = text(d, "Model").unwrap_or(&d.name);
            c.add(
                &format!("drive_failing_{}", d.id),
                "Storage",
                "high",
                "Moderate",
                format!("Replace the failing drive: {name}"),
                format!("Windows reports this drive's health as \"{health}\". Drives that start reporting problems often fail completely soon after."),
                "Back up anything important from it today, then replace it. For the drive Windows is on, a new SSD can be cloned from the old one, or Windows reinstalled.".into(),
                Some(format!("{} SSD", text(d, "Capacity").unwrap_or("1 TB"))),
            );
        }
    }

    if let Some(sys) = system {
        let ty = text(sys, "Type").unwrap_or("");
        let cap = num(sys, "Capacity").unwrap_or(0.0);
        let cap_txt = text(sys, "Capacity").unwrap_or("").to_string();
        if ty.contains("HDD") {
            let size = if cap > 900e9 { "1TB" } else { "500GB" };
            c.add(
                "system_ssd",
                "Storage",
                "high",
                "Low cost",
                "Move Windows to an SSD".into(),
                format!("Windows is on a spinning hard drive ({cap_txt}). This is the single biggest cause of slow start-up and programs taking ages to open. An SSD makes the whole PC feel several times faster."),
                format!(
                    "Replace it with a {size} SSD and copy Windows across with the free cloning tool from the SSD's maker (Samsung, Crucial and others provide one).{}",
                    if c.laptop { " Most laptops take a 2.5-inch SATA SSD or an M.2 SSD; check which one yours has." } else { " Desktops can usually take an M.2 NVMe SSD (fastest) if the motherboard has an M.2 slot, or a 2.5-inch SATA SSD." }
                ),
                Some(format!("{size} SSD")),
            );
        } else if ty.contains("SSD") {
            c.good.push(format!("Windows is on an SSD ({}).", text(sys, "Interface").map(|i| format!("{i}, {cap_txt}")).unwrap_or(cap_txt.clone())));
        }
        if cap > 0.0 && cap < 200e9 && !ty.contains("HDD") {
            c.add(
                "system_small",
                "Storage",
                "low",
                "Low cost",
                format!("A bigger drive: {cap_txt} is small for Windows"),
                "Windows updates alone need 20 GB or more of free space, so a drive this size fills up quickly.".into(),
                "Replace it with a 500 GB or 1 TB SSD (cloning keeps everything), or add a second drive for files.".into(),
                Some("1TB SSD".into()),
            );
        }
    }

    // Free space on C:
    if let Some(v) = items(c.inv, "volumes").iter().find(|v| text(v, "Drive").is_some_and(|d| d.eq_ignore_ascii_case("C:"))) {
        let size = num(v, "Size").unwrap_or(0.0);
        let free = num(v, "Free").unwrap_or(0.0);
        if size > 0.0 && (free < 20.0 * GB || free / size < 0.1) {
            c.add(
                "system_full",
                "Storage",
                "medium",
                "Free",
                format!("Free up space on C: (only {} left)", gb(free)),
                "With less than about 10% free, Windows updates can fail and the PC slows down.".into(),
                "Run Temporary files clean-up in DxTool's Fixes, empty the Recycle Bin, and move videos and photos to another drive. If it fills up again, a bigger SSD is the long-term answer.".into(),
                None,
            );
        }
    }
}

fn windows11(c: &mut Ctx) {
    let Some(win) = item(c.inv, "system", "windows") else { return };
    let edition = text(win, "Edition").unwrap_or(&win.name).to_string();
    if !edition.contains("Windows 10") {
        return;
    }
    let cpu = items(c.inv, "cpu").first().and_then(|i| text(i, "Model")).unwrap_or("").to_string();
    let cpu_ok = win11_cpu(&cpu);
    let firmware = item(c.inv, "system", "firmware");
    let uefi = firmware.and_then(|f| text(f, "Boot mode")).map(|m| m.contains("UEFI"));
    let tpm = item(c.inv, "system", "tpm").and_then(|t| text(t, "Specification")).map(|s| s.trim().to_string());
    let ram = items(c.inv, "memory").iter().filter(|i| i.id.starts_with("dimm")).filter_map(|m| num(m, "Capacity")).sum::<f64>();

    let mut blockers = Vec::new();
    let mut unknown = Vec::new();
    match cpu_ok {
        Win11Cpu::Unsupported => blockers.push(format!("the processor ({}) is older than Windows 11 supports", clean_cpu(&cpu))),
        Win11Cpu::Unknown => unknown.push("the processor model".to_string()),
        Win11Cpu::Supported => {}
    }
    match tpm.as_deref() {
        Some(s) if s.starts_with('2') => {}
        Some(s) if !s.is_empty() => blockers.push(format!("the TPM security chip is version {s}, and Windows 11 needs 2.0")),
        _ => {
            unknown.push("the TPM security chip (DxTool needs administrator rights to read it)".to_string());
        }
    }
    if uefi == Some(false) {
        blockers.push("the PC starts in old legacy BIOS mode instead of UEFI (this one can be fixed)".into());
    }
    if ram > 0.0 && ram < 3.9 * GB {
        blockers.push(format!("it has {} of memory, and Windows 11 needs 4 GB", gb(ram)));
    }

    c.tpm_unknown = blockers.is_empty() && tpm.as_deref().is_none_or(str::is_empty);
    let eol = "Windows 10 stopped getting free security updates in October 2025, so this PC gets no fixes for new security holes.";
    if blockers.is_empty() && unknown.is_empty() {
        c.add(
            "win11_upgrade",
            "Windows",
            "high",
            "Free",
            "Upgrade to Windows 11 for free".into(),
            format!("{eol} This PC meets Windows 11's requirements."),
            "Back up first, then open Settings → Windows Update and choose \"Upgrade to Windows 11\". Programs and files are kept.".into(),
            None,
        );
    } else if blockers.is_empty() {
        c.add(
            "win11_check",
            "Windows",
            "high",
            "Free",
            "Check whether this PC can move to Windows 11".into(),
            format!("{eol} DxTool couldn't confirm {}.", unknown.join(" or ")),
            "Run Microsoft's PC Health Check app, or restart DxTool as administrator to check again. If it passes, upgrade from Settings → Windows Update.".into(),
            None,
        );
    } else {
        let only_fixable = blockers.iter().all(|b| b.contains("legacy BIOS") || b.contains("memory"));
        c.add(
            "win11_blocked",
            "Windows",
            "high",
            if only_fixable { "Low cost" } else { "Expensive" },
            "This PC can't run Windows 11 yet".into(),
            format!("{eol} It can't move to Windows 11 because {}.", blockers.join(", and ")),
            if only_fixable {
                "These can be fixed: a technician can switch the PC to UEFI mode (MBR2GPT, after a backup) and memory can be added. Then upgrade from Settings → Windows Update.".into()
            } else {
                "The choices: a newer PC, Microsoft's paid Extended Security Updates for Windows 10, or moving to a free operating system such as Linux Mint for a PC used mostly for the web.".into()
            },
            None,
        );
    }
}

fn processor(c: &mut Ctx) {
    let Some(cpu) = items(c.inv, "cpu").first() else { return };
    let name = text(cpu, "Model").unwrap_or("").to_string();
    if let Some(g) = intel_core_gen(&name) {
        if g <= 6 {
            c.add(
                "cpu_old",
                "Processor",
                "low",
                "Expensive",
                "The processor is getting old".into(),
                format!("The {} is a generation {g} Intel processor, from 2015 or earlier. Memory and an SSD help a lot, but it will feel slow with modern websites and apps.", clean_cpu(&name)),
                "When a bigger upgrade is due, a newer PC is usually better value than a new processor and motherboard.".into(),
                None,
            );
        } else if g >= 12 {
            c.good.push(format!("The processor ({}) is modern.", clean_cpu(&name)));
        }
    }
}

fn graphics(c: &mut Ctx) {
    let gpus: Vec<&Item> = items(c.inv, "graphics").iter().filter(|g| !text(g, "Bus").is_some_and(|b| b.contains("virtual"))).collect();
    let real: Vec<&Item> = gpus.into_iter().filter(|g| !text(g, "Model").unwrap_or(&g.name).to_lowercase().contains("virtual") && !text(g, "Model").unwrap_or(&g.name).contains("Basic Display")).collect();
    let discrete = real.iter().find(|g| {
        let m = text(g, "Model").unwrap_or(&g.name).to_lowercase();
        m.contains("geforce") || m.contains("rtx") || m.contains("gtx") || m.contains("radeon rx") || m.contains("arc a") || m.contains("arc b") || m.contains("quadro") || m.contains("radeon pro")
    });
    match discrete {
        Some(g) => {
            let vram = num(g, "Video memory").unwrap_or(0.0);
            let model = text(g, "Model").unwrap_or(&g.name).to_string();
            if vram > 0.0 && vram < 3.9 * GB {
                c.add(
                    "gpu_vram",
                    "Graphics",
                    "low",
                    "Expensive",
                    format!("A newer graphics card for modern games ({} on the {model})", gb(vram)),
                    "Most new games want at least 6 to 8 GB of video memory. With less, they need low settings or won't start.".into(),
                    if c.laptop { "Laptop graphics can't be upgraded. For gaming, this would mean a newer laptop.".into() } else { "Only worth it for gaming or video work. Check the power supply's wattage and the space in the case before buying.".into() },
                    (!c.laptop).then(|| "graphics card 8GB".into()),
                );
            } else {
                c.good.push(format!("The graphics card ({model}{}) is fine for games and video.", if vram > 0.0 { format!(", {}", gb(vram)) } else { String::new() }));
            }
        }
        None if !c.laptop && !real.is_empty() => c.add(
            "gpu_none",
            "Graphics",
            "low",
            "Moderate",
            "Add a graphics card, if you play games or edit video".into(),
            "This PC uses the graphics built into the processor. That's fine for office work, the web and video, but too slow for most 3D games.".into(),
            "Only needed for gaming or video work. Check that the case has room and the power supply is strong enough before buying.".into(),
            Some("graphics card".into()),
        ),
        None => {}
    }
}

fn battery(c: &mut Ctx) {
    for b in items(c.inv, "battery") {
        let Some(h) = num(b, "Health") else { continue };
        if h < 70.0 {
            let model = text(b, "Model").map(|m| format!(" ({m})")).unwrap_or_default();
            c.add(
                &format!("battery_{}", b.id),
                "Battery",
                if h < 50.0 { "high" } else { "medium" },
                "Moderate",
                format!("Replace the battery: it holds only {h:.0}% of its original charge"),
                format!("The battery{model} has worn down to {h:.0}% of its design capacity, so it runs out much sooner than when it was new. Below about 50% they can also shut the laptop down suddenly."),
                "Search for a replacement battery for this laptop model, or ask the maker. Many laptop batteries can be swapped by a technician in under an hour.".into(),
                Some(format!("{} laptop battery", text(b, "Model").unwrap_or("replacement"))),
            );
        } else {
            c.good.push(format!("The battery is healthy ({h:.0}% of its original capacity)."));
        }
    }
}

/// Works out upgrade recommendations from an inventory.
pub fn recommend(inv: &Inventory) -> UpgradeReport {
    let computer = item(inv, "system", "computer");
    let form = computer.and_then(|c| text(c, "Form factor")).unwrap_or("").to_lowercase();
    let laptop = ["laptop", "notebook", "portable", "convertible", "detachable", "tablet", "handheld"].iter().any(|k| form.contains(k)) || !items(inv, "battery").is_empty();
    let mut c = Ctx { inv, laptop, tpm_unknown: false, out: Vec::new(), good: Vec::new() };
    windows11(&mut c);
    storage(&mut c);
    memory(&mut c);
    battery(&mut c);
    graphics(&mut c);
    processor(&mut c);
    let rank = |p: &str| match p {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    c.out.sort_by_key(|u| rank(&u.priority));
    UpgradeReport {
        generated_at: chrono::Local::now().to_rfc3339(),
        pc_kind: if laptop { "laptop" } else { "desktop" }.into(),
        pc_name: computer.map(|c| c.name.clone()).unwrap_or_default(),
        limited: c.tpm_unknown.then(|| "The Windows 11 check couldn't read the TPM security chip without administrator rights. Restart DxTool as administrator for a full answer.".to_string()),
        items: c.out,
        good: c.good,
    }
}

// ───────────────────────── inventory cache ─────────────────────────

static CACHE: Mutex<Option<(Instant, Inventory)>> = Mutex::new(None);

/// The hardware inventory, reusing one collected in the last few minutes.
pub fn inventory() -> Inventory {
    if let Some((t, inv)) = CACHE.lock().unwrap().as_ref() {
        if t.elapsed() < Duration::from_secs(300) {
            return inv.clone();
        }
    }
    let inv = crate::inventory::collect();
    remember(&inv);
    inv
}

/// Keeps a freshly collected inventory for [`inventory`].
pub fn remember(inv: &Inventory) {
    *CACHE.lock().unwrap() = Some((Instant::now(), inv.clone()));
}

/// Upgrade recommendations for this PC.
pub fn for_this_pc() -> UpgradeReport {
    recommend(&inventory())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_speeds() {
        assert_eq!(kit_speed("CMH32GX5M2E6000C36"), Some(6000));
        assert_eq!(kit_speed("CMK16GX4M2B3200C16"), Some(3200));
        assert_eq!(kit_speed("F5-6000J3038F16G"), Some(6000));
        assert_eq!(kit_speed("F4-3600C16-8GVKC"), Some(3600));
        assert_eq!(kit_speed("KF560C36BBE-16"), Some(6000));
        assert_eq!(kit_speed("KF432C16BB/8"), Some(3200));
        assert_eq!(kit_speed("CP16G60C36U5B.M8D1"), Some(6000));
        assert_eq!(kit_speed("BL16G32C16U4B.16FE"), Some(3200));
        assert_eq!(kit_speed("FF3D516G6000C38A"), Some(6000));
        assert_eq!(kit_speed("M378A1K43CB2-CTD"), None); // Samsung OEM: no XMP
        assert_eq!(kit_speed("HMA81GU6CJR8N-VK"), None);
    }

    #[test]
    fn win11() {
        assert_eq!(win11_cpu("13th Gen Intel(R) Core(TM) i7-13700F"), Win11Cpu::Supported);
        assert_eq!(win11_cpu("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz"), Win11Cpu::Supported);
        assert_eq!(win11_cpu("Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz"), Win11Cpu::Unsupported);
        assert_eq!(win11_cpu("Intel(R) Core(TM) i5-4590 CPU @ 3.30GHz"), Win11Cpu::Unsupported);
        assert_eq!(win11_cpu("Intel(R) Core(TM) Ultra 7 155H"), Win11Cpu::Supported);
        assert_eq!(win11_cpu("Intel(R) Core(TM) 5 120U"), Win11Cpu::Supported);
        assert_eq!(win11_cpu("AMD Ryzen 7 5800X 8-Core Processor"), Win11Cpu::Supported);
        assert_eq!(win11_cpu("AMD Ryzen 5 1600 Six-Core Processor"), Win11Cpu::Unsupported);
        assert_eq!(win11_cpu("Intel(R) Celeron(R) N4020 CPU"), Win11Cpu::Unknown);
        assert_eq!(intel_core_gen("Intel(R) Core(TM) i5-10400"), Some(10));
        assert_eq!(intel_core_gen("Intel(R) Core(TM) i5-1135G7 @ 2.40GHz"), Some(11));
        assert_eq!(win11_cpu("Intel(R) Core(TM) i7-1065G7"), Win11Cpu::Supported);
        assert_eq!(intel_core_gen("Intel(R) Core(TM) i7 CPU 920"), None);
    }
}
