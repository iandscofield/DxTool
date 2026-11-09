//! Phone backups to the PC: photos and videos (only new files each time) and text messages.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;

use super::{adb_path, folder_name, shell};
use crate::fixes::runner;

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Asks a running backup to stop after the current file.
pub fn cancel_backup() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupEvent {
    /// "listing", "copying", "messages".
    pub phase: String,
    pub done: u64,
    pub total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    /// "photos" or "messages".
    pub kind: String,
    pub folder: String,
    pub copied: u64,
    pub skipped: u64,
    pub failed: u64,
    pub bytes: u64,
    pub messages: u64,
    pub cancelled: bool,
    pub summary: String,
    pub errors: Vec<String>,
}

/// The user's Pictures folder + "Phone backups" (follows OneDrive or a moved Pictures folder).
pub fn default_folder() -> String {
    use winreg::enums::HKEY_CURRENT_USER;
    let pics = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders")
        .and_then(|k| k.get_value::<String, _>("My Pictures"))
        .unwrap_or_else(|_| format!("{}\\Pictures", std::env::var("USERPROFILE").unwrap_or_default()));
    format!("{pics}\\Phone backups")
}

fn phone_folder(serial: &str, dest: &str) -> Result<PathBuf, String> {
    let props = shell(serial, "getprop ro.product.manufacturer; getprop ro.product.model", Duration::from_secs(30))?;
    let mut l = props.lines().map(str::trim).filter(|l| !l.is_empty());
    let (maker, model) = (l.next().unwrap_or(""), l.next().unwrap_or(""));
    if model.is_empty() {
        return Err("The phone didn't answer. Unlock it and check it still allows USB debugging from this PC.".into());
    }
    let maker = maker.chars().next().map(|c| c.to_uppercase().collect::<String>() + &maker[c.len_utf8()..]).unwrap_or_default();
    let name = if model.to_lowercase().starts_with(&maker.to_lowercase()) { model.to_string() } else { format!("{maker} {model}") };
    let dir = Path::new(dest).join(folder_name(&name));
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create the backup folder {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Starts a backup: kind "photos" or "messages", into `dest` (a subfolder per phone is made).
pub fn backup(serial: &str, kind: &str, dest: &str, on_event: &dyn Fn(BackupEvent)) -> Result<BackupResult, String> {
    CANCEL.store(false, Ordering::SeqCst);
    super::start_server(&adb_path());
    let dir = phone_folder(serial, dest)?;
    match kind {
        "photos" => photos(serial, &dir, on_event),
        "messages" => messages(serial, &dir, on_event),
        _ => Err(format!("Unknown backup type {kind}.")),
    }
}

// ───────────────────────── photos and videos ─────────────────────────

const PHOTO_FOLDERS: &str = "/sdcard/DCIM /sdcard/Pictures /sdcard/Movies";

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteFile {
    pub size: u64,
    pub path: String,
}

/// Parses `stat -c '%s|%Y|%n'` lines.
pub fn parse_listing(text: &str) -> Vec<RemoteFile> {
    text.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '|');
            let size = p.next()?.trim().parse().ok()?;
            let _mtime = p.next()?;
            let path = p.next()?.trim_end().to_string();
            (path.starts_with("/sdcard/") && !path.split('/').any(|c| c.starts_with('.'))).then_some(RemoteFile { size, path })
        })
        .collect()
}

/// "/sdcard/DCIM/Camera/a:b.jpg" → "DCIM\Camera\a_b.jpg" (safe for Windows).
pub fn local_rel(remote: &str) -> PathBuf {
    let rel = remote.strip_prefix("/sdcard/").unwrap_or(remote);
    rel.split('/')
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .map(|c| {
            let s: String = c.chars().map(|ch| if "<>:\"\\|?*".contains(ch) || ch.is_control() { '_' } else { ch }).collect();
            s.trim_end_matches(['.', ' ']).to_string()
        })
        .filter(|c| !c.is_empty())
        .collect()
}

fn photos(serial: &str, dir: &Path, on_event: &dyn Fn(BackupEvent)) -> Result<BackupResult, String> {
    on_event(BackupEvent { phase: "listing".into(), current: "Listing photos and videos on the phone…".into(), ..Default::default() });
    let listing = shell(serial, &format!("find {PHOTO_FOLDERS} -type f -exec stat -c '%s|%Y|%n' {{}} + 2>/dev/null"), Duration::from_secs(10 * 60))?;
    let files = parse_listing(&listing);
    let bytes_total: u64 = files.iter().map(|f| f.size).sum();
    let mut r = BackupResult { kind: "photos".into(), folder: dir.display().to_string(), ..Default::default() };
    let adb = adb_path().to_string_lossy().to_string();
    let mut bytes_done = 0u64;
    for (i, f) in files.iter().enumerate() {
        if CANCEL.load(Ordering::SeqCst) {
            r.cancelled = true;
            break;
        }
        let local = dir.join(local_rel(&f.path));
        let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        on_event(BackupEvent { phase: "copying".into(), done: i as u64, total: files.len() as u64, bytes_done, bytes_total, current: name.clone() });
        if fs::metadata(&local).is_ok_and(|m| m.len() == f.size) {
            r.skipped += 1;
            bytes_done += f.size;
            continue;
        }
        if let Some(p) = local.parent() {
            let _ = fs::create_dir_all(p);
        }
        let part = local.with_extension(format!("{}.part", local.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()));
        let res = runner::run_cancellable(&adb, &["-s", serial, "pull", "-a", &f.path, &part.to_string_lossy()], None, Duration::from_secs(30 * 60), Some(&CANCEL), &|_| {});
        match res {
            Ok((0, _)) if fs::metadata(&part).is_ok_and(|m| m.len() == f.size) => {
                let _ = fs::remove_file(&local);
                if fs::rename(&part, &local).is_ok() {
                    r.copied += 1;
                    r.bytes += f.size;
                } else {
                    r.failed += 1;
                    r.errors.push(format!("{name}: couldn't be saved"));
                }
            }
            Ok((_, lines)) => {
                let _ = fs::remove_file(&part);
                if CANCEL.load(Ordering::SeqCst) {
                    r.cancelled = true;
                    break;
                }
                r.failed += 1;
                r.errors.push(format!("{name}: {}", lines.last().cloned().unwrap_or_else(|| "copy failed".into())));
                if lines.iter().any(|l| l.contains("no devices") || l.contains("not found") && l.contains("device")) {
                    return Err("The phone was disconnected. Connect it again and run the backup again: files already copied are kept and skipped.".into());
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&part);
                if e == runner::CANCELLED || CANCEL.load(Ordering::SeqCst) {
                    r.cancelled = true;
                    break;
                }
                r.failed += 1;
                r.errors.push(format!("{name}: {e}"));
            }
        }
        bytes_done += f.size;
    }
    on_event(BackupEvent { phase: "copying".into(), done: files.len() as u64, total: files.len() as u64, bytes_done: bytes_total, bytes_total, current: String::new() });
    r.errors.truncate(20);
    r.summary = if files.is_empty() {
        "No photos or videos were found in the phone's camera, Pictures or Movies folders.".into()
    } else {
        format!(
            "{}Copied {} new file{} ({:.1} GB); {} already backed up{}.",
            if r.cancelled { "Stopped. " } else { "" },
            r.copied,
            if r.copied == 1 { "" } else { "s" },
            r.bytes as f64 / 1e9,
            r.skipped,
            if r.failed > 0 { format!("; {} couldn't be copied", r.failed) } else { String::new() }
        )
    };
    Ok(r)
}

// ───────────────────────── text messages ─────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sms {
    pub thread_id: String,
    pub address: String,
    pub contact: String,
    /// Milliseconds since 1970.
    pub date: i64,
    /// true = sent from the phone, false = received.
    pub sent: bool,
    pub body: String,
}

/// Parses `content query --uri content://sms --projection _id:thread_id:address:date:type:read:body`.
/// The body comes last so line breaks inside messages are kept.
pub fn parse_sms(text: &str) -> Vec<Sms> {
    let text = format!("\n{}", text.replace("\r\n", "\n"));
    let mut out = Vec::new();
    for row in text.split("\nRow: ").skip(1) {
        let row = row.split_once(' ').map(|(_, r)| r).unwrap_or(row);
        let field = |key: &str, next: &str| -> Option<String> {
            let start = row.find(&format!("{key}="))? + key.len() + 1;
            let end = if next.is_empty() { row.len() } else { start + row[start..].find(&format!(", {next}="))? };
            Some(row[start..end].to_string())
        };
        let body = field("body", "").unwrap_or_default();
        let body = body.strip_suffix('\n').unwrap_or(&body).to_string();
        let kind = field("type", "read").unwrap_or_default();
        out.push(Sms {
            thread_id: field("thread_id", "address").unwrap_or_default(),
            address: field("address", "date").filter(|a| a != "NULL").unwrap_or_default(),
            contact: String::new(),
            date: field("date", "type").and_then(|d| d.parse().ok()).unwrap_or(0),
            sent: kind == "2" || kind == "4" || kind == "5" || kind == "6",
            body,
        });
    }
    out
}

/// Last 10 digits of a phone number, for matching contacts.
fn number_key(n: &str) -> String {
    let d: String = n.chars().filter(|c| c.is_ascii_digit()).collect();
    d[d.len().saturating_sub(10)..].to_string()
}

/// Contact names by number, if the phone lets adb read them.
fn contacts(serial: &str) -> HashMap<String, String> {
    let Ok(text) = shell(serial, "content query --uri content://com.android.contacts/data/phones --projection data1:display_name", Duration::from_secs(60)) else { return HashMap::new() };
    text.lines()
        .filter_map(|l| {
            let num = l.split("data1=").nth(1)?.split(", display_name=").next()?;
            let name = l.split(", display_name=").nth(1)?.trim();
            Some((number_key(num), name.to_string()))
        })
        .filter(|(k, n)| k.len() >= 7 && !n.is_empty() && n != "NULL")
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn when(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
}

/// A readable page: one section per conversation, newest conversation first.
pub fn messages_html(phone: &str, msgs: &[Sms]) -> String {
    let mut threads: Vec<(String, Vec<&Sms>)> = Vec::new();
    for m in msgs {
        let key = if m.thread_id.is_empty() { m.address.clone() } else { m.thread_id.clone() };
        match threads.iter_mut().find(|t| t.0 == key) {
            Some(t) => t.1.push(m),
            None => threads.push((key, vec![m])),
        }
    }
    threads.sort_by_key(|t| std::cmp::Reverse(t.1.iter().map(|m| m.date).max().unwrap_or(0)));
    let mut h = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Text messages: {p}</title><style>\
body{{font-family:Segoe UI,Arial,sans-serif;max-width:760px;margin:24px auto;padding:0 16px;color:#1f2933}}\
h1{{font-size:22px}}h2{{font-size:16px;margin:32px 0 8px;border-bottom:1px solid #d9dee5;padding-bottom:6px}}\
.m{{max-width:75%;padding:8px 12px;border-radius:14px;margin:6px 0;white-space:pre-wrap;word-wrap:break-word}}\
.in{{background:#eef1f5}}.out{{background:#d8e8ff;margin-left:auto}}.t{{font-size:11px;color:#6b7785;margin-top:2px}}\
nav a{{display:block;font-size:13px}}</style></head><body><h1>Text messages from {p}</h1>\
<p>{n} messages in {c} conversations, backed up by DxTool on {d}. Picture messages (MMS) aren't included.</p><nav>",
        p = esc(phone),
        n = msgs.len(),
        c = threads.len(),
        d = chrono::Local::now().format("%Y-%m-%d %H:%M")
    );
    let label = |ms: &[&Sms]| {
        let m = ms[0];
        if m.contact.is_empty() { m.address.clone() } else { format!("{} ({})", m.contact, m.address) }
    };
    for (i, (_, ms)) in threads.iter().enumerate() {
        h.push_str(&format!("<a href=\"#t{i}\">{} · {} messages</a>", esc(&label(ms)), ms.len()));
    }
    h.push_str("</nav>");
    for (i, (_, ms)) in threads.iter_mut().enumerate() {
        ms.sort_by_key(|m| m.date);
        h.push_str(&format!("<h2 id=\"t{i}\">{}</h2>", esc(&label(ms))));
        for m in ms.iter() {
            h.push_str(&format!("<div class=\"m {}\">{}<div class=\"t\">{}</div></div>", if m.sent { "out" } else { "in" }, esc(&m.body), when(m.date)));
        }
    }
    h.push_str("</body></html>");
    h
}

fn csv_cell(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn messages(serial: &str, dir: &Path, on_event: &dyn Fn(BackupEvent)) -> Result<BackupResult, String> {
    on_event(BackupEvent { phase: "messages".into(), current: "Reading text messages…".into(), ..Default::default() });
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
    let out = dir.join("Messages").join(&stamp);
    let mut r = BackupResult { kind: "messages".into(), folder: out.display().to_string(), ..Default::default() };
    let text = shell(serial, "content query --uri content://sms --projection _id:thread_id:address:date:type:read:body --sort 'date ASC'", Duration::from_secs(10 * 60)).unwrap_or_else(|e| e);
    let denied = text.contains("Permission Denial") || text.contains("SecurityException") || text.contains("Error while accessing provider");
    if !denied && (text.contains("Row: ") || text.contains("No result found")) {
        let mut msgs = parse_sms(&text);
        on_event(BackupEvent { phase: "messages".into(), current: format!("Saving {} messages…", msgs.len()), ..Default::default() });
        let names = contacts(serial);
        for m in &mut msgs {
            if let Some(n) = names.get(&number_key(&m.address)) {
                m.contact = n.clone();
            }
        }
        fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let phone = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        fs::write(out.join("messages.html"), messages_html(&phone, &msgs)).map_err(|e| e.to_string())?;
        fs::write(out.join("messages.json"), serde_json::to_string_pretty(&msgs).unwrap_or_default()).map_err(|e| e.to_string())?;
        let mut csv = String::from("\u{feff}Date,Direction,Number,Contact,Message\r\n");
        for m in &msgs {
            csv.push_str(&format!("{},{},{},{},{}\r\n", when(m.date), if m.sent { "Sent" } else { "Received" }, csv_cell(&m.address), csv_cell(&m.contact), csv_cell(&m.body)));
        }
        fs::write(out.join("messages.csv"), csv).map_err(|e| e.to_string())?;
        r.messages = msgs.len() as u64;
        r.copied = 3;
        r.summary = if msgs.is_empty() {
            "The phone has no text messages to back up.".into()
        } else {
            format!("Saved {} text messages: messages.html to read, messages.csv for Excel, messages.json for other programs. Picture messages (MMS) aren't included.", msgs.len())
        };
        return Ok(r);
    }

    // The phone doesn't let adb read messages: copy backups made by "SMS Backup & Restore" instead.
    let list = shell(serial, "ls /sdcard/SMSBackupRestore/ 2>/dev/null", Duration::from_secs(30)).unwrap_or_default();
    let files: Vec<&str> = list.lines().map(str::trim).filter(|f| f.ends_with(".xml")).collect();
    if files.is_empty() {
        return Err("This phone doesn't let the PC read text messages directly (newer Android versions block it). \
To back them up: install the free app \"SMS Backup & Restore\" on the phone, make a backup to the phone's storage, then click Back up messages again and DxTool will copy that backup file here."
            .into());
    }
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let adb = adb_path().to_string_lossy().to_string();
    for f in files {
        let local = out.join(local_rel(f));
        match runner::run_cancellable(&adb, &["-s", serial, "pull", "-a", &format!("/sdcard/SMSBackupRestore/{f}"), &local.to_string_lossy()], None, Duration::from_secs(10 * 60), Some(&CANCEL), &|_| {}) {
            Ok((0, _)) => r.copied += 1,
            _ => {
                r.failed += 1;
                r.errors.push(format!("{f}: copy failed"));
            }
        }
    }
    r.summary = format!(
        "This phone doesn't let the PC read messages directly, so DxTool copied {} backup file{} made by the SMS Backup & Restore app. That app can restore them to a phone; the files are also readable XML.",
        r.copied,
        if r.copied == 1 { "" } else { "s" }
    );
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing() {
        let f = parse_listing("3145728|1730000000|/sdcard/DCIM/Camera/PXL_1.jpg\n10|1|/sdcard/DCIM/.thumbnails/x.jpg\n99|1|/sdcard/Pictures/Screenshots/Shot | 2.png\nbad line\n");
        assert_eq!(f, vec![RemoteFile { size: 3145728, path: "/sdcard/DCIM/Camera/PXL_1.jpg".into() }, RemoteFile { size: 99, path: "/sdcard/Pictures/Screenshots/Shot | 2.png".into() }]);
        assert_eq!(local_rel("/sdcard/DCIM/Camera/a:b?.jpg"), PathBuf::from("DCIM").join("Camera").join("a_b_.jpg"));
        assert_eq!(local_rel("/sdcard/../x/./y."), PathBuf::from("x").join("y"));
    }

    #[test]
    fn sms() {
        let text = "Row: 0 _id=1, thread_id=3, address=+15551234567, date=1730000000000, type=1, read=1, body=Hi, are you free?\nCall me\nRow: 1 _id=2, thread_id=3, address=+15551234567, date=1730000100000, type=2, read=1, body=Yes = sure\n";
        let m = parse_sms(text);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].body, "Hi, are you free?\nCall me");
        assert!(!m[0].sent);
        assert_eq!(m[1].body, "Yes = sure");
        assert!(m[1].sent);
        assert_eq!(m[0].date, 1730000000000);
        assert_eq!(number_key("+1 (555) 123-4567"), "5551234567");
        let html = messages_html("Pixel 7", &m);
        assert!(html.contains("2 messages in 1 conversations"));
        assert!(html.contains("Call me"));
        assert!(parse_sms("No result found.\n").is_empty());
    }
}
