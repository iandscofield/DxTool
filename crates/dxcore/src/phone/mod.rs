//! Android phones over USB, through Google's Android Debug Bridge (adb).
//!
//! adb isn't bundled: [`install_tools`] downloads Google's "SDK Platform-Tools" when the
//! user clicks, checks it against the SHA-1 Google publishes, and unpacks it into
//! `DxTool-Data/android`. The phone needs USB debugging switched on and must allow this
//! PC once. Then [`info`] reads diagnostics and [`backup`] copies photos and messages.

pub mod backup;
pub mod info;

use std::fs;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::fixes::runner;

pub use backup::{backup, cancel_backup, default_folder, BackupEvent, BackupResult};
pub use info::{info, PhoneInfo};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const REPO: &str = "https://dl.google.com/android/repository/";
pub const TERMS_URL: &str = "https://developer.android.com/studio/terms";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhoneDevice {
    pub serial: String,
    /// "device" (ready), "unauthorized" (waiting for Allow on the phone), "offline", ...
    pub state: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhoneStatus {
    pub tools_installed: bool,
    pub tools_version: String,
    pub devices: Vec<PhoneDevice>,
    /// Set when adb couldn't be started.
    pub problem: Option<String>,
}

fn tools_dir() -> PathBuf {
    crate::history::data_dir().join("android")
}

pub fn adb_path() -> PathBuf {
    tools_dir().join("platform-tools").join("adb.exe")
}

fn system32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

/// Version of the installed tools ("37.0.1"), from source.properties.
fn tools_version() -> String {
    fs::read_to_string(tools_dir().join("platform-tools").join("source.properties"))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("Pkg.Revision=").map(|v| v.trim().to_string())))
        .unwrap_or_default()
}

/// Runs adb and returns its raw standard output (exit code, stdout, stderr).
/// Raw bytes matter: message text can contain line breaks.
pub fn adb_raw(args: &[&str], timeout: Duration) -> Result<(i32, Vec<u8>, String), String> {
    let exe = adb_path();
    if !exe.exists() {
        return Err("The Android tools aren't downloaded yet.".into());
    }
    if args.first() != Some(&"kill-server") {
        start_server(&exe);
    }
    let mut child = Command::new(&exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Couldn't start adb: {e}"))?;
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let out_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = so.read_to_end(&mut v);
        v
    });
    let err_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = se.read_to_end(&mut v);
        v
    });
    let start = std::time::Instant::now();
    let code = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s.code().unwrap_or(-1);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The phone stopped answering. Check the cable, unlock the phone and try again.".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Collect the output, but never hang if something still holds the pipes open.
    let out = join_timeout(out_t);
    let err = String::from_utf8_lossy(&join_timeout(err_t)).trim().to_string();
    Ok((code, out, err))
}

fn join_timeout(h: std::thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    let start = std::time::Instant::now();
    while !h.is_finished() {
        if start.elapsed() > Duration::from_secs(5) {
            return Vec::new();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    h.join().unwrap_or_default()
}

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);

/// Starts adb's background service with no pipes attached. Started from a command with
/// captured output, the long-running service would inherit those pipes and the command
/// would never seem to finish.
pub fn start_server(exe: &Path) {
    if SERVER_STARTED.load(Ordering::SeqCst) {
        return;
    }
    SERVER_STARTED.store(run_isolated(exe, "start-server", 30_000), Ordering::SeqCst);
}

/// Runs a program without letting it inherit any of DxTool's open handles (std's Command
/// always passes inheritable handles on, which a long-running child would keep open).
fn run_isolated(exe: &Path, args: &str, wait_ms: u32) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{CreateProcessW, GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW as NO_WINDOW, PROCESS_INFORMATION, STARTUPINFOW};
    let mut cmd: Vec<u16> = format!("\"{}\" {args}", exe.display()).encode_utf16().chain([0]).collect();
    let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        if CreateProcessW(None, Some(PWSTR(cmd.as_mut_ptr())), None, None, false, NO_WINDOW, None, None, &si, &mut pi).is_err() {
            return false;
        }
        let _ = WaitForSingleObject(pi.hProcess, wait_ms);
        let mut code = 1u32;
        let ok = GetExitCodeProcess(pi.hProcess, &mut code).is_ok() && code == 0;
        let _ = CloseHandle(pi.hThread);
        let _ = CloseHandle(pi.hProcess);
        ok
    }
}

/// `adb -s SERIAL shell <cmd>` as text.
pub fn shell(serial: &str, cmd: &str, timeout: Duration) -> Result<String, String> {
    let (code, out, err) = adb_raw(&["-s", serial, "shell", cmd], timeout)?;
    let text = String::from_utf8_lossy(&out).replace("\r\n", "\n");
    if code != 0 && text.trim().is_empty() {
        return Err(if err.is_empty() { format!("The phone returned an error (code {code}).") } else { err });
    }
    Ok(text)
}

/// Parses `adb devices -l`.
pub fn parse_devices(text: &str) -> Vec<PhoneDevice> {
    text.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let serial = w.next()?.to_string();
            let state = w.next()?.to_string();
            let rest: Vec<&str> = w.collect();
            let model = rest.iter().find_map(|t| t.strip_prefix("model:")).unwrap_or("").replace('_', " ");
            Some(PhoneDevice { serial, state, model })
        })
        .collect()
}

/// Whether the tools are there, and which phones are connected (starts adb's background service).
pub fn status() -> PhoneStatus {
    let mut s = PhoneStatus { tools_installed: adb_path().exists(), tools_version: tools_version(), ..Default::default() };
    if !s.tools_installed {
        return s;
    }
    match adb_raw(&["devices", "-l"], Duration::from_secs(30)) {
        Ok((_, out, err)) => {
            let text = String::from_utf8_lossy(&out);
            s.devices = parse_devices(&text);
            if s.devices.is_empty() && !text.contains("List of devices") {
                s.problem = Some(format!("adb didn't start properly. {err}").trim().to_string());
            }
        }
        Err(e) => s.problem = Some(e),
    }
    s
}

/// Stops adb's background service (so the DxTool-Data folder can be moved or deleted).
pub fn stop() {
    if adb_path().exists() {
        run_isolated(&adb_path(), "kill-server", 10_000);
        SERVER_STARTED.store(false, Ordering::SeqCst);
    }
}

// ───────────────────────── downloading the tools ─────────────────────────

/// (file name, SHA-1, size) of the stable Windows platform-tools in Google's repository list.
pub fn find_archive(xml: &str) -> Option<(String, String, u64)> {
    for block in xml.split("<remotePackage ").skip(1) {
        if !block.starts_with("path=\"platform-tools\"") || !block.contains("<channelRef ref=\"channel-0\"/>") {
            continue;
        }
        for a in block.split("<archive>").skip(1) {
            if !a.contains("<host-os>windows</host-os>") {
                continue;
            }
            let tag = |t: &str| {
                let open = a.find(&format!("<{t}"))?;
                let start = open + a[open..].find('>')? + 1;
                let end = start + a[start..].find(&format!("</{t}>"))?;
                Some(a[start..end].trim().to_string())
            };
            return Some((tag("url")?, tag("checksum")?, tag("size")?.parse().ok()?));
        }
    }
    None
}

fn sha1_of(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn curl(args: &[&str], secs: u64) -> Result<(), String> {
    let mut all = vec!["-sS", "-L", "--fail", "--retry", "2", "-H", "User-Agent: DxTool"];
    all.extend_from_slice(args);
    match runner::run_cancellable(&system32("curl.exe"), &all, None, Duration::from_secs(secs), None, &|_| {})? {
        (0, _) => Ok(()),
        (6 | 7 | 28 | 35, _) => Err("Couldn't reach Google's download server. Check the internet connection and try again.".into()),
        (code, l) => Err(format!("The download failed (curl code {code}). {}", l.join(" ")).trim().to_string()),
    }
}

/// Downloads Google's platform-tools (about 8 MB), checks its SHA-1, and unpacks it.
/// The user agrees to Google's terms in the app before this runs.
pub fn install_tools(progress: &(dyn Fn(u64, u64) + Sync)) -> Result<String, String> {
    let dir = tools_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let list = dir.join("repository.xml");
    curl(&["-o", &list.to_string_lossy(), &format!("{REPO}repository2-3.xml")], 120)?;
    let xml = fs::read_to_string(&list).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&list);
    let (file, sha1, size) = find_archive(&xml).ok_or("Google's download list didn't include the Android tools for Windows. Try again later.")?;
    if file.contains('/') || file.contains('\\') || !file.ends_with(".zip") {
        return Err("Google's download list looked wrong, so nothing was downloaded.".into());
    }
    let zip = dir.join(&file);
    let watching = AtomicBool::new(false);
    let r = std::thread::scope(|s| {
        s.spawn(|| {
            while !watching.load(Ordering::SeqCst) {
                progress(fs::metadata(&zip).map(|m| m.len()).unwrap_or(0), size);
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        let r = curl(&["-o", &zip.to_string_lossy(), &format!("{REPO}{file}")], 30 * 60);
        watching.store(true, Ordering::SeqCst);
        r
    });
    r?;
    progress(size, size);
    let got = sha1_of(&zip)?;
    if !got.eq_ignore_ascii_case(&sha1) {
        let _ = fs::remove_file(&zip);
        return Err("The download didn't match Google's checksum, so it was deleted and not used. Try again.".into());
    }
    stop();
    let old = dir.join("platform-tools");
    if old.exists() {
        fs::remove_dir_all(&old).map_err(|_| "The old Android tools are in use. Close other programs using adb and try again.".to_string())?;
    }
    let (code, lines) = runner::run_cancellable(&system32("tar.exe"), &["-xf", &zip.to_string_lossy(), "-C", &dir.to_string_lossy()], None, Duration::from_secs(120), None, &|_| {})?;
    let _ = fs::remove_file(&zip);
    if code != 0 || !adb_path().exists() {
        return Err(format!("Unpacking the Android tools failed. {}", lines.join(" ")).trim().to_string());
    }
    Ok(format!("Android tools {} are ready.", tools_version()))
}

/// Turns a phone name into something safe for a folder name.
pub fn folder_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let t = t.trim().trim_end_matches('.').to_string();
    if t.is_empty() { "Phone".into() } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices() {
        let d = parse_devices("* daemon started successfully\nList of devices attached\n1A2B3C4D       device usb:1-4 product:panther model:Pixel_7 device:panther transport_id:2\nR58N12345      unauthorized usb:1-3 transport_id:3\n\n");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].model, "Pixel 7");
        assert_eq!(d[0].state, "device");
        assert_eq!(d[1].state, "unauthorized");
        assert!(parse_devices("List of devices attached\n\n").is_empty());
    }

    #[test]
    fn archive() {
        let xml = r#"<x><remotePackage path="platform-tools"><uses-license ref="android-sdk-preview-license"/><channelRef ref="channel-2"/><archives>
<archive><complete><size>1</size><checksum type="sha1">aaa</checksum><url>platform-tools_r38-win.zip</url></complete><host-os>windows</host-os></archive></archives></remotePackage>
<remotePackage path="platform-tools"><channelRef ref="channel-0"/><archives>
<archive><complete><size>9</size><checksum type="sha1">bbb</checksum><url>platform-tools_r37-linux.zip</url></complete><host-os>linux</host-os></archive>
<archive><complete><size>8044989</size><checksum type="sha1">e03e78</checksum><url>platform-tools_r37.0.1-win.zip</url></complete><host-os>windows</host-os></archive></archives></remotePackage></x>"#;
        assert_eq!(find_archive(xml), Some(("platform-tools_r37.0.1-win.zip".into(), "e03e78".into(), 8044989)));
    }

    #[test]
    fn folders() {
        assert_eq!(folder_name("Galaxy S23: Ultra"), "Galaxy S23_ Ultra");
        assert_eq!(folder_name(""), "Phone");
    }
}
