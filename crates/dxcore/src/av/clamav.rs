//! ClamAV as a second-opinion scanner. Nothing is bundled: on request DxTool downloads
//! the official Windows build from ClamAV's GitHub releases into its data folder,
//! keeps only the files it needs, and downloads the virus databases with freshclam.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::walk;
use crate::fixes::runner::{self, powershell};
use crate::model::{AvEvent, ClamStatus, Threat, VirusScan};

const RELEASES: &str = "https://api.github.com/repos/Cisco-Talos/clamav/releases/latest";

pub fn folder() -> PathBuf {
    crate::history::data_dir().join("clamav")
}
fn bin() -> PathBuf {
    folder().join("bin")
}
fn db() -> PathBuf {
    folder().join("db")
}

fn system32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Header of a .cvd/.cld database: "ClamAV-VDB:build time:version:sigs:flevel:md5:dsig:builder:stime".
struct DbHeader {
    version: String,
    sigs: u64,
    time: Option<chrono::DateTime<chrono::Local>>,
}

fn db_header(name: &str) -> Option<DbHeader> {
    let path = ["cld", "cvd"].iter().map(|x| db().join(format!("{name}.{x}"))).find(|p| p.is_file())?;
    let mut buf = [0u8; 512];
    use std::io::Read;
    fs::File::open(path).ok()?.read_exact(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let f: Vec<&str> = text.trim_end_matches(['\0', ' ']).split(':').collect();
    if f.first() != Some(&"ClamAV-VDB") || f.len() < 4 {
        return None;
    }
    let time = f
        .get(8)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|t| t.with_timezone(&chrono::Local));
    Some(DbHeader { version: f[2].trim().into(), sigs: f[3].trim().parse().unwrap_or(0), time })
}

pub fn status() -> ClamStatus {
    let mut st = ClamStatus { folder: folder().to_string_lossy().into(), ..Default::default() };
    st.installed = bin().join("clamscan.exe").is_file() && bin().join("freshclam.exe").is_file();
    if !st.installed {
        return st;
    }
    st.version = fs::read_to_string(bin().join("version.txt")).unwrap_or_default().trim().to_string();
    let daily = db_header("daily");
    let main = db_header("main");
    st.databases = daily.is_some() && main.is_some();
    if let Some(d) = &daily {
        st.db_version = d.version.clone();
        if let Some(t) = d.time {
            st.db_date = t.to_rfc3339();
            st.db_age_days = Some((chrono::Local::now() - t).num_days().max(0));
        }
    }
    st.signatures = [main, daily, db_header("bytecode")].iter().flatten().map(|h| h.sigs).sum();
    st.size_bytes = dir_size(&folder());
    st
}

/// Calls `on_event` with the size of `path` every half second until `stop` is set.
fn watch_size(path: &Path, total: u64, label: &str, stop: &AtomicBool, on_event: &(dyn Fn(AvEvent) + Sync)) {
    while !stop.load(Ordering::SeqCst) {
        let done = if path.is_dir() { dir_size(path) } else { fs::metadata(path).map(|m| m.len()).unwrap_or(0) };
        on_event(AvEvent::Progress { done, total, current: label.into() });
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Downloads a file with Windows' own curl.exe, reporting progress from the file size.
fn download(url: &str, to: &Path, total: u64, on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<(), String> {
    let _ = fs::remove_file(to);
    let stop = AtomicBool::new(false);
    let out = to.to_string_lossy().to_string();
    let result = std::thread::scope(|s| {
        s.spawn(|| watch_size(to, total, "", &stop, on_event));
        let r = runner::run_cancellable(
            &system32("curl.exe"),
            &["-sS", "-L", "--fail", "--retry", "2", "-H", "User-Agent: DxTool", "-o", &out, url],
            None,
            Duration::from_secs(3 * 3600),
            Some(&super::CANCEL),
            &|_| {},
        );
        stop.store(true, Ordering::SeqCst);
        r
    });
    match result {
        Ok((0, _)) => Ok(()),
        Ok((code, lines)) => {
            let _ = fs::remove_file(to);
            Err(format!("The download failed (curl code {code}). {}", lines.join(" ")).trim().to_string())
        }
        Err(e) => {
            let _ = fs::remove_file(to);
            Err(e)
        }
    }
}

/// Downloads ClamAV, keeps the scanner, updater and their DLLs, then downloads the databases.
pub fn install(on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<String, String> {
    let _busy = super::BusyGuard::acquire()?;
    let dir = folder();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;

    on_event(AvEvent::Phase { text: "Finding the latest version of ClamAV".into() });
    let meta_file = dir.join("release.json");
    download(RELEASES, &meta_file, 0, on_event)?;
    let meta: Value = serde_json::from_slice(&fs::read(&meta_file).map_err(|e| e.to_string())?).map_err(|_| "GitHub sent an unexpected reply.".to_string())?;
    let _ = fs::remove_file(&meta_file);
    let arch = if cfg!(target_arch = "aarch64") { ".win.arm64.zip" } else { ".win.x64.zip" };
    let asset = meta
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|x| x.get("name").and_then(Value::as_str).is_some_and(|n| n.ends_with(arch))))
        .ok_or("The latest ClamAV release has no Windows download.")?;
    let url = asset.get("browser_download_url").and_then(Value::as_str).ok_or("Missing download link.")?;
    if !url.starts_with("https://github.com/Cisco-Talos/clamav/releases/download/") {
        return Err("Unexpected download location; stopped for safety.".into());
    }
    let size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
    let version = meta.get("tag_name").and_then(Value::as_str).unwrap_or("").trim_start_matches("clamav-").to_string();

    on_event(AvEvent::Phase { text: format!("Downloading ClamAV {version} ({} MB)", size / 1_000_000) });
    let zip = dir.join("clamav-download.zip");
    download(url, &zip, size, on_event)?;
    let got = fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
    if size > 0 && got != size {
        let _ = fs::remove_file(&zip);
        return Err(format!("The download was incomplete ({got} of {size} bytes). Try again."));
    }

    on_event(AvEvent::Phase { text: "Unpacking the scanner".into() });
    let staging = dir.join("bin.new");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let q = |p: &Path| p.to_string_lossy().replace('\'', "''");
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$dest = '{dest}'
$z = [IO.Compression.ZipFile]::OpenRead('{zip}')
try {{
  foreach ($e in $z.Entries) {{
    $parts = $e.FullName -split '/', 2
    if ($parts.Count -lt 2 -or $parts[1] -eq '' -or $e.FullName.EndsWith('/')) {{ continue }}
    $rel = $parts[1]
    $isTool = ($rel -notmatch '/') -and ($rel -match '\.(exe|dll)$') -and ($rel -notmatch '^(clambc|sigtool|clamd|clamdscan|clamdtop|clamsubmit|clamconf)\.exe$')
    if ($isTool -or $rel -like 'certs/*' -or $rel -like 'COPYING*') {{
      $out = Join-Path $dest ($rel -replace '/', '\')
      New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
      [IO.Compression.ZipFileExtensions]::ExtractToFile($e, $out, $true)
      "Unpacked $rel"
    }}
  }}
}} finally {{ $z.Dispose() }}"#,
        dest = q(&staging),
        zip = q(&zip)
    );
    let emit = |l: &str| on_event(AvEvent::Output { line: l.into() });
    let r = powershell(&script, Duration::from_secs(900), &emit);
    let _ = fs::remove_file(&zip);
    match r {
        Ok((0, _)) if staging.join("clamscan.exe").is_file() && staging.join("freshclam.exe").is_file() => {}
        Ok((_, lines)) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("Couldn't unpack ClamAV. {}", crate::fixes::tail(&lines).join(" ")));
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    }
    let _ = fs::write(staging.join("version.txt"), &version);
    let _ = fs::remove_dir_all(bin());
    fs::rename(&staging, bin()).map_err(|e| format!("Couldn't finish installing ClamAV: {e}"))?;

    update_inner(on_event)?;
    Ok(format!("ClamAV {version} is ready."))
}

/// Downloads or refreshes the virus databases.
pub fn update(on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<String, String> {
    let _busy = super::BusyGuard::acquire()?;
    update_inner(on_event)?;
    let st = status();
    Ok(format!("Virus databases are up to date (version {}).", st.db_version))
}

fn update_inner(on_event: &(dyn Fn(AvEvent) + Sync)) -> Result<(), String> {
    if !bin().join("freshclam.exe").is_file() {
        return Err("ClamAV isn't downloaded yet.".into());
    }
    let first = !status().databases;
    on_event(AvEvent::Phase {
        text: if first { "Downloading the virus databases (about 230 MB the first time)".into() } else { "Checking for newer virus databases".into() },
    });
    fs::create_dir_all(db()).map_err(|e| e.to_string())?;
    let conf = folder().join("freshclam.conf");
    fs::write(&conf, "# Written by DxTool\r\nDatabaseMirror database.clamav.net\r\n").map_err(|e| e.to_string())?;
    let exe = bin().join("freshclam.exe").to_string_lossy().to_string();
    let conf_arg = format!("--config-file={}", conf.display());
    let db_arg = format!("--datadir={}", db().display());
    let stop = AtomicBool::new(false);
    let result = std::thread::scope(|s| {
        s.spawn(|| watch_size(&db(), 0, "", &stop, on_event));
        let emit = |l: &str| {
            if !(l.starts_with("Time:") && l.contains("ETA:")) {
                on_event(AvEvent::Output { line: l.into() });
            }
        };
        let r = runner::run_cancellable(&exe, &[&conf_arg, &db_arg, "--stdout"], None, Duration::from_secs(3 * 3600), Some(&super::CANCEL), &emit);
        stop.store(true, Ordering::SeqCst);
        r
    });
    let (code, lines) = result?;
    // 0 = updated, 1 = already up to date.
    if (code == 0 || code == 1) && status().databases {
        return Ok(());
    }
    let text = lines.join("\n").to_lowercase();
    let hint = if text.contains("cool-down") || text.contains("429") || text.contains("rate limit") {
        "ClamAV's download servers asked DxTool to wait before trying again (they limit how often databases can be downloaded). Try again in a few hours."
    } else if text.contains("can't connect") || text.contains("couldn't resolve") || text.contains("connection") {
        "Couldn't reach ClamAV's download servers. Check the internet connection."
    } else if text.contains("403") {
        "ClamAV's download servers refused the request (they block some countries and networks)."
    } else {
        "freshclam couldn't download the databases."
    };
    Err(format!("{hint} (code {code}) {}", crate::fixes::tail(&lines).into_iter().rev().take(3).collect::<Vec<_>>().join(" | ")))
}

/// Deletes the downloaded ClamAV copy and its databases.
pub fn remove() -> Result<(), String> {
    let _busy = super::BusyGuard::acquire()?;
    fs::remove_dir_all(folder()).map_err(|e| format!("Couldn't remove ClamAV: {e}"))
}

fn is_file_line(l: &str) -> bool {
    let b = l.as_bytes();
    (b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\') || l.starts_with(r"\\")
}

pub fn scan(scan: &mut VirusScan, paths: &[String], on_event: &(dyn Fn(AvEvent) + Sync)) {
    let st = status();
    if !st.installed || !st.databases {
        scan.outcome = "failed".into();
        scan.message = "ClamAV and its virus databases need to be downloaded first.".into();
        return;
    }
    scan.signature_version = format!("ClamAV {} · daily {}", st.version, st.db_version);
    let roots = match scan.kind.as_str() {
        "full" => walk::full_roots(),
        "custom" => paths.iter().map(walk::Root::all).collect(),
        _ => walk::quick_roots(),
    };
    scan.targets = match scan.kind.as_str() {
        "full" => roots.iter().map(|r| r.path.to_string_lossy().to_string()).collect(),
        "custom" => paths.to_vec(),
        _ => vec!["Downloads, Desktop, temp folders, startup items and programs in AppData/ProgramData".into()],
    };

    on_event(AvEvent::Phase { text: "Finding files to scan".into() });
    let exclude = vec![crate::history::data_dir()];
    let files = walk::collect(&roots, &exclude, &|n, dir| on_event(AvEvent::Progress { done: n, total: 0, current: dir.into() }));
    if super::cancelled() {
        scan.outcome = "cancelled".into();
        scan.message = "The scan was stopped before it started.".into();
        return;
    }
    if files.is_empty() {
        scan.outcome = "clean".into();
        scan.message = "There were no files to scan in the chosen places.".into();
        return;
    }
    let total = files.len() as u64;
    let list = std::env::temp_dir().join(format!("dxtool-clamscan-{}.txt", std::process::id()));
    let body: String = files.iter().map(|p| format!("{}\n", p.display())).collect();
    if let Err(e) = fs::write(&list, body) {
        scan.outcome = "failed".into();
        scan.message = format!("Couldn't write the file list: {e}");
        return;
    }

    on_event(AvEvent::Phase { text: format!("Loading {} virus signatures (this can take a minute)", crate::fmt::thousands(st.signatures)) });
    on_event(AvEvent::Progress { done: 0, total, current: String::new() });
    let exe = bin().join("clamscan.exe").to_string_lossy().to_string();
    let db_arg = format!("--database={}", db().display());
    let list_arg = format!("--file-list={}", list.display());
    let done = std::sync::atomic::AtomicU64::new(0);
    let threats = std::sync::Mutex::new(Vec::<Threat>::new());
    let errors = std::sync::Mutex::new(Vec::<String>::new());
    let unreadable = std::sync::Mutex::new(std::collections::HashSet::<String>::new());
    let last = std::sync::Mutex::new(Instant::now());
    let started = AtomicBool::new(false);
    let on_line = |l: &str| {
        if !is_file_line(l) {
            // "LibClamAV Warning: Can't open file X: 225" = Windows blocked it because Defender detected a virus.
            if let Some(rest) = l.strip_prefix("LibClamAV Warning: Can't open file ") {
                if let Some(path) = rest.strip_suffix(": 225") {
                    let t = Threat {
                        name: "Detected by Microsoft Defender".into(),
                        path: path.into(),
                        engine: "defender".into(),
                        status: "active".into(),
                        action: "Windows is blocking this file because Defender detected a threat in it".into(),
                        ..Default::default()
                    };
                    on_event(AvEvent::Threat { threat: t.clone() });
                    threats.lock().unwrap().push(t);
                }
            } else if let Some(rest) = l.strip_prefix("WARNING: Can't open file ") {
                // clamscan's own line for a file it couldn't read: still counts towards progress.
                if let Some((path, _)) = rest.rsplit_once(": ") {
                    unreadable.lock().unwrap().insert(path.to_lowercase());
                }
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                on_event(AvEvent::Progress { done: n, total, current: String::new() });
            } else if l.contains("ERROR") {
                errors.lock().unwrap().push(l.to_string());
            }
            return;
        }
        if !started.swap(true, Ordering::SeqCst) {
            on_event(AvEvent::Phase { text: "Scanning files".into() });
        }
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(rest) = l.strip_suffix(" FOUND") {
            if let Some((path, name)) = rest.rsplit_once(": ") {
                let t = Threat {
                    name: name.trim().into(),
                    path: path.into(),
                    engine: "clamav".into(),
                    status: "active".into(),
                    action: "Found, still on the PC".into(),
                    ..Default::default()
                };
                on_event(AvEvent::Threat { threat: t.clone() });
                threats.lock().unwrap().push(t);
            }
        } else if l.ends_with("ERROR") {
            errors.lock().unwrap().push(l.to_string());
        }
        let mut lt = last.lock().unwrap();
        if lt.elapsed() > Duration::from_millis(150) || n == total {
            *lt = Instant::now();
            let path = l.rsplit_once(": ").map(|(p, _)| p).unwrap_or(l);
            on_event(AvEvent::Progress { done: n, total, current: path.into() });
        }
    };
    let secs = if scan.kind == "full" { 48 * 3600 } else { 12 * 3600 };
    let result = runner::run_cancellable(&exe, &[&db_arg, &list_arg, "--stdout"], None, Duration::from_secs(secs), Some(&super::CANCEL), &on_line);
    let _ = fs::remove_file(&list);
    scan.threats = threats.into_inner().unwrap();
    let errors = errors.into_inner().unwrap();
    let blocked: std::collections::HashSet<String> = scan.threats.iter().filter(|t| t.engine == "defender").map(|t| t.path.to_lowercase()).collect();
    let unread = unreadable.into_inner().unwrap().difference(&blocked).count();
    let scanned = done.load(Ordering::SeqCst);
    match result {
        Ok((code, lines)) => {
            scan.files_scanned = lines
                .iter()
                .find_map(|l| l.strip_prefix("Scanned files:"))
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(scanned);
            let log: Vec<String> = lines.iter().filter(|l| !is_file_line(l)).cloned().collect();
            scan.log = log[log.len().saturating_sub(200)..].to_vec();
            // 0 = clean, 1 = threats, 2 = some error (often just unreadable files).
            if code == 2 && scanned == 0 {
                scan.outcome = "failed".into();
                scan.message = format!("ClamAV couldn't run the scan. {}", errors.last().cloned().unwrap_or_default());
            } else {
                let found = scan.threats.iter().filter(|t| t.engine == "clamav").count();
                let plural = |n: usize| if n == 1 { "" } else { "s" };
                scan.message = match found {
                    0 => "ClamAV found no threats.".into(),
                    n => format!("ClamAV found {n} threat{}.", plural(n)),
                };
                let nb = blocked.len();
                if nb > 0 {
                    scan.message.push_str(&format!(" {nb} more file{} couldn't be scanned because Microsoft Defender has already detected a threat in {}.", plural(nb), if nb == 1 { "it" } else { "them" }));
                }
                if unread > 0 {
                    scan.message.push_str(&format!(" {unread} file{} couldn't be read (usually because Windows is using them).", if unread == 1 { "" } else { "s" }));
                }
            }
        }
        Err(e) => {
            scan.files_scanned = scanned;
            if e == runner::CANCELLED {
                scan.outcome = "cancelled".into();
                scan.message = format!("The scan was stopped after {} of {} files.", crate::fmt::thousands(scanned), crate::fmt::thousands(total));
            } else {
                scan.outcome = "failed".into();
                scan.message = e;
            }
        }
    }
}
