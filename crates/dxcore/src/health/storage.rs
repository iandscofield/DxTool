//! Storage checks: drive health counters, free space and storage errors in the event log.

use std::collections::BTreeMap;
use std::path::Path;

use super::{event_item, field, item, num_field, plural, Ctx};
use crate::events;
use crate::fmt;
use crate::inventory::{bus_type, health_text, media_type};
use crate::model::{CheckResult, Fields, Finding, Status};
use crate::wmiutil::{self, ns_query, raw_section, Row, RowExt};

const STORAGE: &str = "ROOT\\Microsoft\\Windows\\Storage";

pub fn drive_health(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let st = wmiutil::connect(STORAGE)?;
    let phys = wmiutil::query(&st, "SELECT * FROM MSFT_PhysicalDisk")?;
    let rel = wmiutil::query_opt(&st, "SELECT * FROM MSFT_StorageReliabilityCounter");
    let predict = ns_query("ROOT\\WMI", "SELECT * FROM MSStorageDriver_FailurePredictStatus");
    let w32 = ns_query("ROOT\\CIMV2", "SELECT Index, PNPDeviceID FROM Win32_DiskDrive");
    if rel.is_empty() && !ctx.is_admin {
        r.limited = Some("Wear level, temperature and error counters need administrator rights. Restart DxTool as administrator to see them.".into());
    }

    let mut bad = 0;
    for p in &phys {
        let num = p.s("DeviceId").unwrap_or_default();
        let name = p.s("FriendlyName").or_else(|| p.s("Model")).unwrap_or_else(|| format!("Disk {num}"));
        let rc = rel.iter().find(|x| x.s("DeviceId").as_deref() == Some(num.as_str()));
        let pnp = w32.iter().find(|w| w.s("Index").as_deref() == Some(num.as_str())).and_then(|w| w.s("PNPDeviceID"));
        let failing = pnp.as_ref().and_then(|id| {
            predict.iter().find(|x| {
                x.s("InstanceName").map(|n| n.to_uppercase().starts_with(&id.to_uppercase())).unwrap_or(false)
            })
        });
        let media = media_type(p.u("MediaType").unwrap_or(0));
        let is_hdd = media == "HDD";
        let (health, hstatus) = health_text(p.u("HealthStatus").unwrap_or(99));
        let ops: Vec<String> = p.list("OperationalStatus");

        let wear = rc.and_then(|x| x.f("Wear"));
        let temp = rc.and_then(|x| x.f("Temperature")).filter(|t| *t > 0.0 && *t < 150.0);
        let temp_max = rc.and_then(|x| x.f("TemperatureMax")).filter(|t| *t > 0.0 && *t < 150.0);
        let hours = rc.and_then(|x| x.f("PowerOnHours"));
        let cycles = rc.and_then(|x| x.f("StartStopCycleCount"));
        let read_unc = rc.and_then(|x| x.f("ReadErrorsUncorrected")).unwrap_or(0.0);
        let write_unc = rc.and_then(|x| x.f("WriteErrorsUncorrected")).unwrap_or(0.0);

        let mut f = Fields::new();
        f.text("Type", Some(media))
            .text("Interface", Some(bus_type(p.u("BusType").unwrap_or(0))))
            .opt_num("Capacity", p.f("Size"), fmt::bytes_dec)
            .text("Windows health", Some(health))
            .opt_num("Wear (life used)", wear, |w| format!("{w:.0}%"))
            .opt_num("Temperature", temp, |t| format!("{t:.0} °C"))
            .opt_num("Highest temperature", temp_max, |t| format!("{t:.0} °C"))
            .opt_num("Power-on time", hours, |h| format!("{:.0} h ({:.1} years)", h, h / 8766.0))
            .opt_num("Power cycles", cycles, |c| format!("{c:.0}"))
            .opt_num("Uncorrectable read errors", rc.and_then(|x| x.f("ReadErrorsUncorrected")), |c| format!("{c:.0}"))
            .opt_num("Uncorrectable write errors", rc.and_then(|x| x.f("WriteErrorsUncorrected")), |c| format!("{c:.0}"))
            .text("Failure predicted", failing.and_then(|x| x.b("PredictFailure")).map(|b| fmt::yes_no(b)))
            .text("Firmware", p.s("FirmwareVersion"))
            .text("Serial number", p.s("SerialNumber"));
        let mut it = item(format!("disk{num}"), name.clone(), format!("Disk {num}"), f.into_vec());
        it.status = hstatus;
        it.sections.push(raw_section("MSFT_PhysicalDisk", p));
        if let Some(x) = rc {
            it.sections.push(raw_section("MSFT_StorageReliabilityCounter", x));
        }

        let ev = it.summary.clone();
        let mut add = |f: Finding| {
            it.status = it.status.max(f.severity);
            r.findings.push(f.evidence(ev.clone()));
        };
        let backup = "Back up important files from this drive now, then plan to replace it.";
        if hstatus == Status::Error || ops.iter().any(|o| o == "5") || failing.and_then(|x| x.b("PredictFailure")) == Some(true) {
            add(Finding::new(format!("drive_failing_{num}"), Status::Error, format!("{name} is failing"), "The drive itself reports that it is failing or about to fail. Drives in this state can stop working at any time and take their files with them.").advice(backup));
        } else if hstatus == Status::Warn || ops.iter().any(|o| o == "3") {
            add(Finding::new(format!("drive_warning_{num}"), Status::Warn, format!("{name} reports a warning"), "Windows says this drive is degraded. It still works but is not fully healthy.").advice("Back up important files and keep an eye on this drive. Check the drive maker's tool for details."));
        }
        if let Some(w) = wear {
            if w >= 90.0 {
                add(Finding::new(format!("drive_wear_{num}"), Status::Error, format!("{name} is nearly worn out ({w:.0}% of its life used)"), "SSDs can only be written a certain amount. This one has used almost all of its rated life.").advice(backup));
            } else if w >= 70.0 {
                add(Finding::new(format!("drive_wear_{num}"), Status::Warn, format!("{name} has used {w:.0}% of its life"), "SSDs wear out gradually as data is written. This one is well into its rated life.").advice("Nothing urgent, but plan to replace it in the future and keep backups."));
            }
        }
        if let Some(t) = temp {
            let limit = if is_hdd { 55.0 } else { 70.0 };
            if t >= limit {
                add(Finding::new(format!("drive_hot_{num}"), Status::Warn, format!("{name} is running hot ({t:.0} °C)"), "High temperatures shorten a drive's life and can make it slow itself down.").advice("Improve airflow around the drive. NVMe SSDs often benefit from a heatsink."));
            }
        }
        let unc = read_unc + write_unc;
        if unc > 0.0 {
            let sev = if unc >= 50.0 { Status::Error } else { Status::Warn };
            add(Finding::new(format!("drive_errors_{num}"), sev, format!("{name} has had {unc:.0} errors it could not correct"), "The drive failed to read or write some data even after retrying. This often comes before failure.").advice(backup));
        }
        if it.status != Status::Ok {
            bad += 1;
            it.status_text = Some("Needs attention".into());
        }
        r.items.push(it);
    }
    r.items_label = Some("Drives".into());
    r.summary = if phys.is_empty() {
        "No drives found".into()
    } else if bad == 0 {
        format!("{}, all healthy", plural(phys.len(), "drive", "drives"))
    } else {
        format!("{bad} of {} need attention", plural(phys.len(), "drive", "drives"))
    };
    Ok(())
}

fn dir_size(path: &Path, cap: usize) -> (u64, usize) {
    let mut total = 0u64;
    let mut files = 0usize;
    let mut stack = vec![path.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            if files >= cap {
                return (total, files);
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else if let Ok(m) = e.metadata() {
                total += m.len();
                files += 1;
            }
        }
    }
    (total, files)
}

fn recycle_bin() -> Option<(u64, u64)> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.ok()?;
    Some((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
}

pub fn disk_space(_ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let rows = ns_query("ROOT\\CIMV2", "SELECT DeviceID, VolumeName, FileSystem, Size, FreeSpace FROM Win32_LogicalDisk WHERE DriveType = 3");
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let gb = 1_000_000_000.0;
    let mut low = 0;
    for d in &rows {
        let id = d.s("DeviceID").unwrap_or_default();
        let (Some(size), Some(free)) = (d.f("Size"), d.f("FreeSpace")) else { continue };
        if size <= 0.0 {
            continue;
        }
        let pct = free / size * 100.0;
        let is_sys = id.eq_ignore_ascii_case(&sys);
        let label = d.s("VolumeName").map(|v| format!("{id} ({v})")).unwrap_or(id.clone());
        let mut it = item(
            id.clone(),
            label.clone(),
            if is_sys { "Windows drive" } else { "" },
            vec![
                num_field("Free", fmt::bytes_dec(free), free),
                num_field("Free %", format!("{pct:.0}%"), pct),
                num_field("Size", fmt::bytes_dec(size), size),
                field("File system", d.s("FileSystem").unwrap_or_default()),
                field("Windows drive", fmt::yes_no(is_sys)),
            ],
        );
        let (sev, crit) = if is_sys {
            if pct < 5.0 || free < 5.0 * gb {
                (Some(Status::Error), true)
            } else if pct < 10.0 || free < 15.0 * gb {
                (Some(Status::Warn), false)
            } else {
                (None, false)
            }
        } else if pct < 5.0 {
            (Some(Status::Warn), false)
        } else {
            (None, false)
        };
        if let Some(sev) = sev {
            low += 1;
            it.status = sev;
            it.status_text = Some("Low on space".into());
            let detail = if is_sys {
                if crit {
                    "Windows is almost out of room. Updates can fail, the PC slows down, and programs may crash or refuse to save files."
                } else {
                    "Windows needs free space for updates, temporary files and virtual memory. Below about 10% things start to slow down and updates may fail."
                }
            } else {
                "This drive is almost full."
            };
            r.findings.push(
                Finding::new(format!("low_space_{}", id.trim_end_matches(':')), sev, format!("{label} is low on space ({} free, {pct:.0}%)", fmt::bytes_dec(free)), detail)
                    .advice("Clear temporary files and the Recycle Bin, uninstall programs you no longer use, or move large files (videos, photos, downloads) elsewhere.")
                    .fix("clear_temp")
                    .evidence(it.summary.clone()),
            );
        }
        r.items.push(it);
    }

    // Space that could be reclaimed
    let mut temp_total = 0u64;
    let mut temp_dirs = Vec::new();
    if let Ok(t) = std::env::var("TEMP") {
        temp_dirs.push(t);
    }
    if let Ok(w) = std::env::var("SystemRoot") {
        temp_dirs.push(format!("{w}\\Temp"));
    }
    for d in &temp_dirs {
        let (b, _) = dir_size(Path::new(d), 200_000);
        temp_total += b;
    }
    r.facts.push(num_field("Temporary files", fmt::bytes_dec(temp_total as f64), temp_total as f64));
    let mut reclaim = temp_total;
    if let Some((rb, n)) = recycle_bin() {
        r.facts.push(num_field("Recycle Bin", format!("{} ({} items)", fmt::bytes_dec(rb as f64), n), rb as f64));
        reclaim += rb;
    }
    if reclaim as f64 > 2.0 * gb {
        r.findings.push(
            Finding::new("reclaimable_space", Status::Info, format!("About {} could be freed", fmt::bytes_dec(reclaim as f64)), "Temporary files and the Recycle Bin are taking up space that could be cleared safely.")
                .advice("Use the temporary file clean-up in Fixes, or empty the Recycle Bin.")
                .fix("clear_temp")
                .evidence(r.facts.clone()),
        );
    }

    r.items_label = Some("Drives".into());
    r.summary = if low == 0 {
        format!("{} with enough free space", plural(r.items.len(), "drive", "drives"))
    } else {
        format!("{} low on space", plural(low, "drive is", "drives are"))
    };
    Ok(())
}

struct Known {
    severity: Status,
    title: &'static str,
    detail: &'static str,
    advice: &'static str,
    fix: Option<&'static str>,
}

fn known_storage_event(provider: &str, id: u32, level: u8) -> Known {
    let p = provider.to_lowercase();
    let disk = p == "disk";
    let ntfs = p.contains("ntfs");
    let back = "Back up important files, check the drive health section, and reseat or replace the drive's cable.";
    match (disk, ntfs, id) {
        (true, _, 7) => Known { severity: Status::Error, title: "Bad blocks found on a drive", detail: "Part of the drive could not be read. Bad blocks are physical damage and often get worse.", advice: back, fix: None },
        (true, _, 154) => Known { severity: Status::Error, title: "A drive reported a hardware error", detail: "The drive reported a hardware failure while reading or writing.", advice: back, fix: None },
        (true, _, 11) => Known { severity: Status::Warn, title: "Drive controller errors", detail: "Windows had trouble talking to a drive. Loose or faulty cables, failing USB enclosures or a failing drive can cause this.", advice: back, fix: None },
        (true, _, 51) => Known { severity: Status::Warn, title: "Errors while writing virtual memory to a drive", detail: "Windows had a problem writing memory pages to a drive.", advice: back, fix: None },
        (true, _, 153) => Known { severity: Status::Warn, title: "Drive operations had to be retried", detail: "Reads or writes did not complete the first time. Occasional retries on USB drives are common; frequent ones on internal drives are a warning sign.", advice: back, fix: None },
        (true, _, 158) => Known { severity: Status::Info, title: "Two drives have the same identity", detail: "Windows found drives with identical disk IDs. This normally happens with cloned drives or attached virtual disks, and Windows may keep one of them offline.", advice: "If a drive is missing, open Disk Management and bring it online. Nothing to do otherwise.", fix: None },
        (_, true, 55) | (_, true, 98) | (_, true, 137) => Known { severity: Status::Warn, title: "File system corruption detected", detail: "Windows found damage in the file system structure on a drive. This can lose or damage files.", advice: "Run a disk check (chkdsk) on the affected drive. Check the drive health section too.", fix: Some("chkdsk") },
        _ => Known {
            severity: if level <= 2 { Status::Warn } else { Status::Info },
            title: "Storage warnings",
            detail: "Windows logged storage-related warnings or errors.",
            advice: "Check the event details. If they repeat, check the drive health section and cables.",
            fix: None,
        },
    }
}

pub fn storage_errors(ctx: &Ctx, r: &mut CheckResult) -> Result<(), String> {
    let q = events::system_filter(&[
        events::providers(&["disk", "Ntfs", "Microsoft-Windows-Ntfs", "stornvme", "storahci", "nvme", "iaStorAC", "iaStorA", "iaStorAVC", "iaStorVD", "volmgr", "Microsoft-Windows-StorPort"]),
        "(Level=1 or Level=2 or Level=3)".into(),
        events::within_days(ctx.days),
    ]);
    let evs = events::query("System", &q, 500)?;
    // Map \Device\HarddiskN to drive names.
    let names: BTreeMap<String, String> = ns_query(STORAGE, "SELECT DeviceId, FriendlyName FROM MSFT_PhysicalDisk")
        .iter()
        .filter_map(|p: &Row| Some((p.s("DeviceId")?, p.s("FriendlyName")?)))
        .collect();
    // Messages name drives as "\\Device\\Harddisk2\\DR2" or "Disk 2".
    let drive_of = |msg: &str| -> Option<String> {
        let i = msg.find("Harddisk").map(|i| i + 8).or_else(|| msg.find("Disk ").map(|i| i + 5))?;
        let n: String = msg[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if n.is_empty() {
            return None;
        }
        Some(names.get(&n).cloned().unwrap_or(format!("Disk {n}")))
    };

    // Group by meaning so one noisy drive becomes one finding.
    let mut groups: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
    for (i, e) in evs.iter().enumerate() {
        groups.entry((e.provider.to_lowercase(), e.id)).or_default().push(i);
    }
    for ((prov, id), idxs) in &groups {
        let first = &evs[idxs[0]];
        let k = known_storage_event(prov, *id, first.level);
        let mut drives: Vec<String> = idxs.iter().filter_map(|i| drive_of(&evs[*i].message)).collect();
        drives.sort();
        drives.dedup();
        let n = idxs.len();
        // A couple of retries on a USB stick is not worth a warning.
        let sev = if *id == 153 && n < 5 { Status::Info } else { k.severity };
        let on = if drives.is_empty() { String::new() } else { format!(" on {}", drives.join(", ")) };
        let mut f = Finding::new(
            format!("storage_ev_{prov}_{id}"),
            sev,
            format!("{}{on} ({})", k.title, plural(n, "time", "times")),
            format!("{} Last seen {}.", k.detail, first.local_time()),
        )
        .advice(k.advice)
        .evidence(vec![field("Source", first.provider.clone()), field("Event ID", id.to_string()), field("Example", first.short_message())]);
        if let Some(fx) = k.fix {
            f = f.fix(fx);
        }
        r.findings.push(f);
    }
    for (i, e) in evs.iter().enumerate() {
        let drive = drive_of(&e.message).unwrap_or_default();
        r.items.push(event_item(e, i, known_storage_event(&e.provider, e.id, e.level).title, vec![field("Drive", drive)]));
    }
    r.items_label = Some("Storage events".into());
    r.summary = if evs.is_empty() {
        format!("No storage errors in the last {} days", ctx.days)
    } else {
        format!("{} in the last {} days", plural(evs.len(), "storage event", "storage events"), ctx.days)
    };
    Ok(())
}
