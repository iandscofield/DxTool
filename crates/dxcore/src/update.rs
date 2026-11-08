//! Updates from DxTool's GitHub releases. Nothing happens unless asked: `check` looks up
//! the latest release, `install` downloads it, checks its SHA-256 against the release's
//! SHA256SUMS.txt and then either swaps the program file (portable copy) or runs the
//! installer quietly (installed copy).
//!
//! Release files: `DxTool.exe` (portable), `DxTool_<version>_x64-setup.exe` (installer)
//! and `SHA256SUMS.txt` (`<hash>  <file name>` lines).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::fixes::runner;

pub const REPO: &str = "iandscofield/DxTool";
const PORTABLE_ASSET: &str = "DxTool.exe";
const SUMS_ASSET: &str = "SHA256SUMS.txt";

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    /// Release notes (plain text / Markdown).
    pub notes: String,
    pub published: String,
    pub page_url: String,
    /// "portable" or "installed".
    pub install_kind: String,
    pub download_name: String,
    pub download_size: u64,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// "installed" when the Windows installer put this copy here (it leaves uninstall.exe next to it).
pub fn install_kind() -> &'static str {
    let installed = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("uninstall.exe").is_file())).unwrap_or(false);
    if installed { "installed" } else { "portable" }
}

/// "v1.2.3" / "1.2.3" → (1, 2, 3). Anything after a '-' (e.g. "-beta") is ignored.
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>());
    let v = (it.next()?.ok()?, it.next().unwrap_or(Ok(0)).ok()?, it.next().unwrap_or(Ok(0)).ok()?);
    Some(v)
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn system32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

fn work_dir() -> PathBuf {
    std::env::temp_dir().join("dxtool-update")
}

/// Downloads `url` to `to` with Windows' curl.exe, calling `progress(done, total)` twice a second.
fn download(url: &str, to: &Path, total: u64, progress: &(dyn Fn(u64, u64) + Sync)) -> Result<(), String> {
    let _ = fs::remove_file(to);
    let stop = AtomicBool::new(false);
    let out = to.to_string_lossy().to_string();
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            while !stop.load(Ordering::SeqCst) {
                progress(fs::metadata(to).map(|m| m.len()).unwrap_or(0), total);
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        let r = runner::run_cancellable(
            &system32("curl.exe"),
            &["-sS", "-L", "--fail", "--retry", "2", "-H", "User-Agent: DxTool", "-H", "Accept: application/vnd.github+json", "-o", &out, url],
            None,
            Duration::from_secs(30 * 60),
            None,
            &|_| {},
        );
        stop.store(true, Ordering::SeqCst);
        r
    });
    match result {
        Ok((0, _)) => Ok(()),
        Ok((code, lines)) => {
            let _ = fs::remove_file(to);
            Err(match code {
                6 | 7 | 28 | 35 => "Couldn't reach GitHub. Check the internet connection and try again.".to_string(),
                22 => "GitHub refused the request (the release may not exist yet, or too many checks in a short time). Try again later.".to_string(),
                _ => format!("The download failed (curl code {code}). {}", lines.join(" ")).trim().to_string(),
            })
        }
        Err(e) => {
            let _ = fs::remove_file(to);
            Err(e)
        }
    }
}

fn latest_release() -> Result<Value, String> {
    let dir = work_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("release.json");
    download(&format!("https://api.github.com/repos/{REPO}/releases/latest"), &file, 0, &|_, _| {})?;
    let v = serde_json::from_slice(&fs::read(&file).map_err(|e| e.to_string())?).map_err(|_| "GitHub sent an unexpected reply.".to_string());
    let _ = fs::remove_file(&file);
    v
}

fn asset_for(release: &Value, kind: &str) -> Option<(String, String, u64)> {
    let assets = release.get("assets")?.as_array()?;
    let wanted = |n: &str| if kind == "installed" { n.ends_with("_x64-setup.exe") } else { n == PORTABLE_ASSET };
    let a = assets.iter().find(|a| a.get("name").and_then(Value::as_str).is_some_and(wanted))?;
    Some((
        a.get("name")?.as_str()?.to_string(),
        a.get("browser_download_url")?.as_str()?.to_string(),
        a.get("size").and_then(Value::as_u64).unwrap_or(0),
    ))
}

fn sums_url(release: &Value) -> Option<String> {
    release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|a| a.get("name").and_then(Value::as_str) == Some(SUMS_ASSET))?
        .get("browser_download_url")?
        .as_str()
        .map(String::from)
}

fn info_from(release: &Value) -> UpdateInfo {
    let s = |k: &str| release.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let kind = install_kind();
    let latest = s("tag_name").trim_start_matches(['v', 'V']).to_string();
    let (name, _, size) = asset_for(release, kind).unwrap_or_default();
    UpdateInfo {
        current: current_version().into(),
        newer: is_newer(&latest, current_version()),
        latest,
        notes: s("body"),
        published: s("published_at"),
        page_url: s("html_url"),
        install_kind: kind.into(),
        download_name: name,
        download_size: size,
    }
}

/// Looks up the latest release on GitHub.
pub fn check() -> Result<UpdateInfo, String> {
    Ok(info_from(&latest_release()?))
}

fn sha256_of(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
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

/// The expected hash of `name` in a SHA256SUMS.txt file (`<hash>  <name>` or `<hash> *<name>`).
fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        (file.eq_ignore_ascii_case(name) && hash.len() == 64).then(|| hash.to_lowercase())
    })
}

/// Downloads and checks the newest version. Returns the downloaded file.
fn fetch(progress: &(dyn Fn(u64, u64) + Sync)) -> Result<(UpdateInfo, PathBuf), String> {
    let release = latest_release()?;
    let info = info_from(&release);
    if !info.newer {
        return Err(format!("DxTool {} is already the newest version.", info.current));
    }
    let (name, url, size) = asset_for(&release, &info.install_kind).ok_or("The newest release doesn't include a file for this kind of DxTool copy yet. Download it from the release page instead.")?;
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    let sums = sums_url(&release).ok_or("The newest release has no checksum file, so DxTool won't install it automatically.")?;
    if !url.starts_with(&prefix) || !sums.starts_with(&prefix) {
        return Err("Unexpected download location; stopped for safety.".into());
    }
    let dir = work_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let sums_file = dir.join(SUMS_ASSET);
    download(&sums, &sums_file, 0, &|_, _| {})?;
    let expected = expected_hash(&fs::read_to_string(&sums_file).unwrap_or_default(), &name).ok_or("The checksum file doesn't list this download, so DxTool won't install it.")?;
    let file = dir.join(&name);
    download(&url, &file, size, progress)?;
    let got = sha256_of(&file)?;
    if got != expected {
        let _ = fs::remove_file(&file);
        return Err("The download didn't match its checksum (it may be damaged). Nothing was changed. Try again.".into());
    }
    Ok((info, file))
}

/// Downloads and installs the newest version. On success the caller should close DxTool
/// straight away: the new version (or the installer) is already starting.
pub fn install(progress: &(dyn Fn(u64, u64) + Sync)) -> Result<String, String> {
    let (info, file) = fetch(progress)?;
    if info.install_kind == "installed" {
        // /UPDATE keeps settings and shortcuts, /P shows only a progress bar, /R restarts DxTool after.
        std::process::Command::new(&file).args(["/UPDATE", "/P", "/R"]).spawn().map_err(|e| format!("Couldn't start the installer: {e}"))?;
        return Ok(format!("Installing DxTool {}. DxTool will close and reopen by itself.", info.latest));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    replace_exe(&exe, &file)?;
    std::process::Command::new(&exe).spawn().map_err(|e| format!("DxTool {} is installed, but couldn't be started: {e}. Open it again by hand.", info.latest))?;
    Ok(format!("DxTool {} is installed and is starting now.", info.latest))
}

fn old_path(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "DxTool".into());
    exe.with_file_name(format!("{stem}.old.exe"))
}

/// Swaps the running program file for `new`: Windows lets a running exe be renamed, so the
/// old one becomes `<name>.old.exe` (removed next time DxTool starts).
pub fn replace_exe(exe: &Path, new: &Path) -> Result<(), String> {
    let old = old_path(exe);
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old).map_err(|e| format!("Couldn't replace DxTool's program file ({e}). Is the folder read-only?"))?;
    if let Err(e) = fs::copy(new, exe) {
        let _ = fs::rename(&old, exe);
        return Err(format!("Couldn't copy the new version into place ({e}). The old version was kept."));
    }
    let _ = fs::remove_file(new);
    Ok(())
}

/// Removes what an earlier update left behind. Called when DxTool starts.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_path(&exe);
        // The old copy may still be closing; try a few times in the background.
        std::thread::spawn(move || {
            for _ in 0..10 {
                if !old.exists() || fs::remove_file(&old).is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    }
    let _ = fs::remove_dir_all(work_dir());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.6.0", "0.5.1"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.5.1", "0.5.1"));
        assert!(!is_newer("0.5.0", "0.5.1"));
        assert!(!is_newer("garbage", "0.5.1"));
        assert!(is_newer("0.10.0", "0.9.0"));
    }

    #[test]
    fn sums() {
        let h = "a".repeat(64);
        let text = format!("{h}  DxTool.exe\n{}  *DxTool_0.6.0_x64-setup.exe\n", "b".repeat(64));
        assert_eq!(expected_hash(&text, "DxTool.exe"), Some(h));
        assert_eq!(expected_hash(&text, "DxTool_0.6.0_x64-setup.exe"), Some("b".repeat(64)));
        assert_eq!(expected_hash(&text, "other.exe"), None);
    }
}
