# DxTool Changes

Newest first. Written in plain English: what changed and why it matters.

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
