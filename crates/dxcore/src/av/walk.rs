//! Lists the files a ClamAV scan should look at.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const REPARSE_POINT: u32 = 0x400;

/// Programs and scripts (runnable files minus the libraries they load): used for AppData
/// and ProgramData, where apps keep thousands of DLLs that Defender watches anyway.
pub const STARTERS: &[&str] = &[
    "exe", "scr", "com", "pif", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "msi", "lnk", "jar", "cpl",
];

/// File types that can carry malware: runnable files plus archives, disk images, macro
/// documents, PDFs and web pages. Used for the quick scan of Downloads, Desktop and temp.
pub const RISKY: &[&str] = &[
    "exe", "dll", "scr", "com", "pif", "sys", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "msi", "lnk", "jar", "cpl",
    "zip", "rar", "7z", "cab", "iso", "img", "vhd", "docm", "xlsm", "pptm", "doc", "xls", "ppt", "rtf", "pdf", "html", "htm", "svg", "reg", "url", "msix",
    "appx",
];

/// Quick scans skip files bigger than this (large installers, games, AI models).
const QUICK_MAX_SIZE: u64 = 20 * 1024 * 1024;

/// Files Windows keeps locked that can't be read anyway.
const SKIP_NAMES: &[&str] = &["pagefile.sys", "hiberfil.sys", "swapfile.sys", "dumpstack.log.tmp"];

pub struct Root {
    pub path: PathBuf,
    pub max_depth: Option<usize>,
    /// Only these file types (lower-case extensions), or every file.
    pub only: Option<&'static [&'static str]>,
    pub max_size: Option<u64>,
}

impl Root {
    pub fn all(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), max_depth: None, only: None, max_size: None }
    }

    /// Quick-scan root: limited depth, only some file types, no huge files.
    fn quick(path: impl Into<PathBuf>, depth: usize, only: &'static [&'static str]) -> Self {
        Self { path: path.into(), max_depth: Some(depth), only: Some(only), max_size: Some(QUICK_MAX_SIZE) }
    }
}

/// Collects files under the roots, skipping links (so nothing is visited twice) and the
/// `exclude` folders. Calls `progress(count, current folder)` now and then.
pub fn collect(roots: &[Root], exclude: &[PathBuf], progress: &dyn Fn(u64, &str)) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut last = Instant::now();
    let excluded = |p: &Path| exclude.iter().any(|e| p.starts_with(e));
    for root in roots {
        let Ok(meta) = fs::symlink_metadata(&root.path) else { continue };
        if meta.is_file() {
            if seen.insert(root.path.to_string_lossy().to_lowercase()) {
                out.push(root.path.clone());
            }
            continue;
        }
        let mut stack = vec![(root.path.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if super::cancelled() {
                return out;
            }
            if excluded(&dir) {
                continue;
            }
            if last.elapsed() > Duration::from_millis(250) {
                progress(out.len() as u64, &dir.to_string_lossy());
                last = Instant::now();
            }
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let Ok(m) = e.metadata() else { continue };
                if m.file_attributes() & REPARSE_POINT != 0 {
                    continue;
                }
                let p = e.path();
                if m.is_dir() {
                    if root.max_depth.is_none_or(|d| depth < d) {
                        stack.push((p, depth + 1));
                    }
                } else if m.is_file() {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    if SKIP_NAMES.contains(&name.as_str()) {
                        continue;
                    }
                    if let Some(only) = root.only {
                        let ext = name.rsplit_once('.').map(|(_, x)| x).unwrap_or("");
                        if !only.contains(&ext) {
                            continue;
                        }
                    }
                    if root.max_size.is_some_and(|max| m.len() > max) {
                        continue;
                    }
                    if seen.insert(p.to_string_lossy().to_lowercase()) {
                        out.push(p);
                    }
                }
            }
        }
    }
    progress(out.len() as u64, "");
    out
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var(var).ok().filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Expands %VAR% references in a registry command line.
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        match after.find('%') {
            Some(b) => {
                let name = &after[..b];
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => out.push_str(&v),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[b + 1..];
            }
            None => {
                out.push_str(&rest[a..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The program file in a command line such as `"C:\x\y.exe" --flag` or `C:\x\y.exe -a`.
pub fn program_of(cmd: &str) -> Option<PathBuf> {
    let cmd = expand_env(cmd.trim());
    let path = if let Some(rest) = cmd.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        let lower = cmd.to_lowercase();
        match lower.find(".exe") {
            Some(i) => cmd[..i + 4].to_string(),
            None => cmd.split_whitespace().next()?.to_string(),
        }
    };
    let p = PathBuf::from(path);
    (p.is_absolute() && p.is_file()).then_some(p)
}

/// Programs set to start automatically from the Run keys.
fn autorun_programs() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let mut v = Vec::new();
    let keys = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\RunOnce"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\RunOnce"),
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"),
    ];
    for (hive, path) in keys {
        let Ok(k) = RegKey::predef(hive).open_subkey(path) else { continue };
        for (_, val) in k.enum_values().flatten() {
            if let Some(p) = program_of(&val.to_string()) {
                v.push(p);
            }
        }
    }
    v
}

/// Where malware usually lands: risky files near the top of Downloads, Desktop and the temp
/// folders, startup folders, programs that start automatically, and programs and scripts
/// in AppData and ProgramData. Files over 20 MB are skipped (they'd take far too long).
pub fn quick_roots() -> Vec<Root> {
    let mut v = Vec::new();
    let profile = env_path("USERPROFILE");
    if let Some(p) = &profile {
        v.push(Root::quick(p.join("Downloads"), 2, RISKY));
        v.push(Root::quick(p.join("Desktop"), 2, RISKY));
    }
    if let Some(t) = env_path("TEMP") {
        v.push(Root::quick(t, 2, RISKY));
    }
    if let Some(w) = env_path("SystemRoot") {
        v.push(Root::quick(w.join("Temp"), 2, RISKY));
    }
    if let Some(a) = env_path("APPDATA") {
        v.push(Root::all(a.join(r"Microsoft\Windows\Start Menu\Programs\Startup")));
        v.push(Root::quick(a, 3, STARTERS));
    }
    if let Some(l) = env_path("LOCALAPPDATA") {
        v.push(Root::quick(l, 3, STARTERS));
    }
    if let Some(pd) = env_path("ProgramData") {
        v.push(Root::all(pd.join(r"Microsoft\Windows\Start Menu\Programs\StartUp")));
        v.push(Root::quick(pd, 3, STARTERS));
    }
    v.extend(autorun_programs().into_iter().map(Root::all));
    v
}

/// The root of every fixed drive.
pub fn full_roots() -> Vec<Root> {
    let mut v: Vec<Root> = crate::fixes::ntfs_drives().into_iter().map(|(letter, _, _)| Root::all(format!("{letter}\\"))).collect();
    if v.is_empty() {
        v.push(Root::all(format!("{}\\", std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()))));
    }
    v
}
