//! Writing reports to disk. The report itself is built as HTML by the interface;
//! PDFs are made by asking Microsoft Edge (built into Windows 10 and 11) to print it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::history;

/// Turns free text into a safe file name.
fn file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '(' | ')') { c } else { '-' })
        .collect();
    let s = s.trim().trim_matches('-').to_string();
    if s.is_empty() { "DxTool report".into() } else { s.chars().take(100).collect() }
}

fn find_edge() -> Option<PathBuf> {
    let key = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\msedge.exe";
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Ok(k) = RegKey::predef(root).open_subkey(key) {
            if let Ok(p) = k.get_value::<String, _>("") {
                let p = PathBuf::from(p.trim_matches('"'));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .map(|base| PathBuf::from(base).join("Microsoft\\Edge\\Application\\msedge.exe"))
        .find(|p| p.exists())
}

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let enc: String = s
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            c => c.to_string(),
        })
        .collect();
    format!("file:///{enc}")
}

/// Prints an HTML file to PDF with Edge in headless mode (nothing appears on screen).
pub fn html_to_pdf(html: &Path, pdf: &Path) -> Result<(), String> {
    let edge = find_edge().ok_or("Microsoft Edge was not found, so a PDF can't be made. Save as HTML instead and print it from a browser.")?;
    let profile = std::env::temp_dir().join(format!("dxtool-pdf-{}", std::process::id()));
    let _ = std::fs::remove_file(pdf);
    clean_old_profiles();
    let args = [
        "--headless=new".to_string(),
        // Without this, Edge started from an elevated (administrator) DxTool relaunches
        // itself as a normal user and the first process exits before the PDF exists.
        "--do-not-de-elevate".to_string(),
        "--disable-gpu".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--no-pdf-header-footer".to_string(),
        format!("--user-data-dir={}", profile.display()),
        format!("--print-to-pdf={}", pdf.display()),
        file_url(html),
    ];
    let outcome = run_edge(&edge, &args, pdf);
    remove_dir_retry(&profile);
    outcome
}

/// Starts Edge (no window, no pipes that child processes could hold open) and waits
/// until the PDF has been written completely, even if Edge hands the work to another process.
fn run_edge(edge: &Path, args: &[String], pdf: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Instant;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = Command::new(edge)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Could not start Microsoft Edge to make the PDF: {e}"))?;
    let start = Instant::now();
    let limit = Duration::from_secs(90);
    let mut exited = None;
    let mut last_len = 0u64;
    let mut stable = 0;
    while start.elapsed() < limit {
        if exited.is_none() {
            if let Ok(Some(s)) = child.try_wait() {
                exited = Some(s);
            }
        }
        // Done once the file exists and has stopped growing.
        if let Ok(m) = std::fs::metadata(pdf) {
            let len = m.len();
            if len > 0 && len == last_len {
                stable += 1;
                if stable >= 3 && exited.is_some() {
                    return Ok(());
                }
                // Edge's own process may linger a little after writing; that's fine.
                if stable >= 10 {
                    let _ = child.kill();
                    return Ok(());
                }
            } else {
                stable = 0;
            }
            last_len = len;
        } else if exited.is_some() && start.elapsed() > Duration::from_secs(30) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = child.kill();
    let code = exited.and_then(|s| s.code()).map(|c| format!(" (Edge exit code {c})")).unwrap_or_default();
    Err(format!(
        "Microsoft Edge did not produce the PDF{code}. Save as HTML instead and use Print → Save as PDF in any browser."
    ))
}

/// Edge sometimes keeps files open for a moment after finishing.
fn remove_dir_retry(dir: &Path) {
    for _ in 0..10 {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Removes temporary Edge profiles left behind by earlier runs.
fn clean_old_profiles() {
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("dxtool-pdf-") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

/// Saves a report for a machine as HTML or PDF and returns the file's path.
pub fn export(machine_id: &str, name: &str, html: &str, format: &str) -> Result<PathBuf, String> {
    let dir = history::reports_dir(machine_id)?;
    let base = file_name(name);
    let html_path = dir.join(format!("{base}.html"));
    std::fs::write(&html_path, html).map_err(|e| format!("Could not save the report: {e}"))?;
    if format == "pdf" {
        let pdf_path = dir.join(format!("{base}.pdf"));
        let tmp = std::env::temp_dir().join(format!("dxtool-report-{}.html", std::process::id()));
        std::fs::write(&tmp, html).map_err(|e| e.to_string())?;
        let r = html_to_pdf(&tmp, &pdf_path);
        let _ = std::fs::remove_file(&tmp);
        match r {
            Ok(()) => {
                // Keep only the PDF the user asked for.
                let _ = std::fs::remove_file(&html_path);
                Ok(pdf_path)
            }
            // Keep the HTML so the report isn't lost.
            Err(e) => Err(format!("{e} The HTML version was saved here: {}", html_path.display())),
        }
    } else {
        Ok(html_path)
    }
}
