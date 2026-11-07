# DxTool Changes

Newest first. Written in plain English: what changed and why it matters.

---

## Version 0.2.0 · Stage 3: health checks and scans · 2026-11-07

### New
- **Scans page** with three kinds of scan:
  - **Fast**: 14 checks, last 30 days, a few seconds.
  - **Full**: 19 checks, last 90 days.
  - **Custom**: pick your own checks, including a slow deep system file check.
  - Scans only look; they change nothing.
- **Live progress** while a scan runs. You can move around the app while it works.
- **Health page** with a 0–100 health score and a grade. Findings come most serious first, each with what it means, what to do, and the facts behind it.
- **19 health checks** covering:
  - **Storage:** drive health and wear, free space, drive and file system errors.
  - **Stability:** blue screens, unexpected shutdowns, hardware errors, device problems, drivers, service failures and app crashes.
  - **Windows:** Windows Update, the system image, startup programs and boot time, memory and performance, battery and activation.
  - **Security:** antivirus, firewall and device security.
  - **Network:** connection and internet.
- **Blue screens explained:** about 50 common stop codes translated into plain English with advice. Crash dump files are read when DxTool runs as administrator.
- **Security page** showing antivirus, firewall and device security results.
- **Overview** now has a health card (run a Fast scan, or see the latest score). The "Needs attention" list includes scan findings.
- The sidebar shows the latest health score, and "running" while a scan is in progress.

### Improved
- **The hardware inventory is about 5 seconds faster without administrator rights.** Windows makes the TPM query wait 5 seconds before refusing it, so DxTool now skips it unless it has admin rights.

### How the score works
- Each problem (red) takes off 18 points.
- Each warning (amber) takes off 6 points.
- Notes (blue) are for information and don't count.
- No single check can take off more than 30 points, so one noisy area can't sink the whole score.

### Known limitations
- **Only the latest scan is kept, until DxTool closes.** Saved history per machine comes in Stage 4.
- **Some checks give partial results without administrator rights:** drive wear and temperature, crash dump files, the system image check, TPM and BitLocker.
- **Tested only without administrator rights.** I couldn't test the administrator-only parts: it isn't possible yet in the development version, and Windows asks you, not me, to approve it.
- **The one-click repairs mentioned in findings arrive in Stage 5** (Fixes).
- **Tested on one PC so far.**

---

## Version 0.1.0 · Stage 1: app foundation and hardware inventory · 2026-11-07

### New
- **The DxTool app.** A modern Windows desktop app with a sidebar, dark and light themes, and Simple/Advanced modes.
- **Overview page.** Shows this PC at a glance: model, Windows version, uptime, Secure Boot, and tiles for processor, memory, graphics, storage, displays, network, battery and motherboard. A "Needs attention" list shows anything flagged.
- **Hardware page with 18 categories.** System, Processor, Memory, Graphics, Displays, Drives, Volumes, Network, Battery, Printers, Ports, Expansion slots, USB, Audio, Input, Cameras, Bluetooth and All devices.
- **Where everything is plugged in.** Each device shows its bus, its exact location (such as "USB port 3 on hub 2"), what it's connected through, and the full connection path.
- **Driver details for every device:** version, date, maker, INF file, digital signature.
- **Device Manager problem codes in plain English** (for example "Code 43: Windows stopped this device because it reported problems").
- **Powerful tables:** search, per-column filters, sorting, show/hide/reorder/resize columns, compact rows, copy to clipboard. Layouts are remembered.
- **Card view** for categories where a table doesn't fit well (System, Processor, Graphics, Displays, Battery).
- **Detail panel** for any item, with search, copy-all, and every raw Windows property in Advanced mode.
- **Restart as administrator** button, since some details (TPM, battery data) need admin rights.

### Under the hood
- Built with Tauri 2 (Rust engine + React interface). The app is small and fast.
- The diagnostic engine (`crates/dxcore`) is separate from the interface so it can be reused by a future command-line tool, the Claude connection and the bootable version.
- Hardware categories are read in parallel; a full inventory takes about 5 seconds.

### Known limitations
- Health scans, history, reports, fixes and security scanning are not built yet (coming in stages 3–6).
- Temperatures and fan speeds are not shown yet.
- Tested on one PC so far (Windows 11, Intel Core i7-13700F, NVIDIA RTX 4070 Ti, MSI Z690 board).
