//! Writing reports to disk. The report itself is built as HTML by the interface;
//! PDFs are made by asking Microsoft Edge (built into Windows 10 and 11) to print it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::health::run_hidden;
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
    let args = [
        "--headless=new".to_string(),
        "--disable-gpu".to_string(),
        "--no-first-run".to_string(),
        "--disable-extensions".to_string(),
        "--no-pdf-header-footer".to_string(),
        format!("--user-data-dir={}", profile.display()),
        format!("--print-to-pdf={}", pdf.display()),
        file_url(html),
    ];
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = run_hidden(&edge.to_string_lossy(), &argv, Duration::from_secs(90));
    let _ = std::fs::remove_dir_all(&profile);
    result?;
    match std::fs::metadata(pdf) {
        Ok(m) if m.len() > 0 => Ok(()),
        _ => Err("Edge did not produce a PDF. Save as HTML instead and print it from a browser.".into()),
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
        // Keep only the PDF the user asked for.
        let _ = std::fs::remove_file(&html_path);
        r?;
        Ok(pdf_path)
    } else {
        Ok(html_path)
    }
}
