//! Runs console tools without a window and streams their output line by line.

use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Splits a growing byte stream into lines. Handles UTF-16 (sfc) and the console code
/// page (most tools), and treats '\r' as a line break so progress updates come through.
struct LineSplitter {
    buf: Vec<u8>,
    utf16: Option<bool>,
    text: String,
}

impl LineSplitter {
    fn new() -> Self {
        Self { buf: Vec::new(), utf16: None, text: String::new() }
    }

    fn push(&mut self, bytes: &[u8], out: &mut Vec<String>) {
        self.buf.extend_from_slice(bytes);
        if self.utf16.is_none() && self.buf.len() >= 8 {
            let zeros = self.buf.iter().skip(1).step_by(2).take(64).filter(|b| **b == 0).count();
            let n = (self.buf.len() / 2).min(64);
            self.utf16 = Some(zeros * 10 > n * 6);
        }
        match self.utf16 {
            Some(true) => {
                let even = self.buf.len() & !1;
                let units: Vec<u16> = self.buf[..even].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                self.buf.drain(..even);
                self.text.push_str(&String::from_utf16_lossy(&units).replace('\0', ""));
            }
            Some(false) => {
                // Keep an incomplete UTF-8 sequence for the next chunk.
                let cut = match std::str::from_utf8(&self.buf) {
                    Ok(_) => self.buf.len(),
                    Err(e) if e.error_len().is_none() => e.valid_up_to(),
                    Err(_) => self.buf.len(),
                };
                self.text.push_str(&String::from_utf8_lossy(&self.buf[..cut]));
                self.buf.drain(..cut);
            }
            None => return,
        }
        while let Some(i) = self.text.find(['\r', '\n']) {
            let line: String = self.text[..i].trim().to_string();
            self.text.drain(..=i);
            if !line.is_empty() {
                out.push(line);
            }
        }
    }

    fn finish(&mut self, out: &mut Vec<String>) {
        if self.utf16.is_none() && !self.buf.is_empty() {
            self.utf16 = Some(false);
            let b = std::mem::take(&mut self.buf);
            self.push(&b, out);
        }
        let rest = self.text.trim().to_string();
        if !rest.is_empty() {
            out.push(rest);
        }
        self.text.clear();
    }
}

/// Runs a program, calling `on_line` for every line of output (stdout and stderr).
/// Returns the exit code and all lines.
pub fn run_streaming(exe: &str, args: &[&str], stdin_text: Option<&str>, timeout: Duration, on_line: &dyn Fn(&str)) -> Result<(i32, Vec<String>), String> {
    run_cancellable(exe, args, stdin_text, timeout, None, on_line)
}

/// Message returned when a run is stopped through the cancel flag.
pub const CANCELLED: &str = "Stopped by the user.";

/// Like `run_streaming`, but stops the program when `cancel` becomes true.
pub fn run_cancellable(
    exe: &str,
    args: &[&str],
    stdin_text: Option<&str>,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    on_line: &dyn Fn(&str),
) -> Result<(i32, Vec<String>), String> {
    let mut child = Command::new(exe)
        .args(args)
        .stdin(if stdin_text.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Could not start {exe}: {e}"))?;
    if let (Some(text), Some(mut si)) = (stdin_text, child.stdin.take()) {
        let _ = si.write_all(text.as_bytes());
    }
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let (etx, erx) = mpsc::channel::<Vec<u8>>();
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let r1 = std::thread::spawn(move || {
        let mut b = [0u8; 4096];
        while let Ok(n) = so.read(&mut b) {
            if n == 0 || tx.send(b[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let r2 = std::thread::spawn(move || {
        let mut b = [0u8; 4096];
        while let Ok(n) = se.read(&mut b) {
            if n == 0 || etx.send(b[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let mut out = LineSplitter::new();
    let mut err = LineSplitter::new();
    let mut lines = Vec::new();
    let start = Instant::now();
    let mut code = None;
    loop {
        let mut fresh = Vec::new();
        while let Ok(chunk) = rx.try_recv() {
            out.push(&chunk, &mut fresh);
        }
        while let Ok(chunk) = erx.try_recv() {
            err.push(&chunk, &mut fresh);
        }
        for l in fresh {
            on_line(&l);
            lines.push(l);
        }
        if code.is_none() && cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
            let _ = child.kill();
            return Err(CANCELLED.into());
        }
        if code.is_none() {
            match child.try_wait() {
                Ok(Some(s)) => code = Some(s.code().unwrap_or(-1)),
                Ok(None) if start.elapsed() > timeout => {
                    let _ = child.kill();
                    return Err(format!("{exe} took longer than {} minutes and was stopped.", timeout.as_secs() / 60));
                }
                Ok(None) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        if code.is_some() && r1.is_finished() && r2.is_finished() {
            break;
        }
        // Child processes can keep the pipes open after the main process ends; don't wait forever.
        if code.is_some() && start.elapsed() > timeout {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut fresh = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        out.push(&chunk, &mut fresh);
    }
    while let Ok(chunk) = erx.try_recv() {
        err.push(&chunk, &mut fresh);
    }
    out.finish(&mut fresh);
    err.finish(&mut fresh);
    for l in fresh {
        on_line(&l);
        lines.push(l);
    }
    Ok((code.unwrap_or(-1), lines))
}

/// Runs a Windows PowerShell script (passed encoded, so quoting can't break it).
pub fn powershell(script: &str, timeout: Duration, on_line: &dyn Fn(&str)) -> Result<(i32, Vec<String>), String> {
    use base64::Engine;
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let exe = format!("{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
    run_streaming(&exe, &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded], None, timeout, on_line)
}
