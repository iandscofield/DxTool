# DxTool Features

A plain-English list of what DxTool does. ✅ = working now, 🔜 = planned for version 1, 💭 = later idea.

---

## Overview page ✅
The first screen you see.
- Computer name, make and model, Windows version and build, how long the PC has been on.
- Boot mode (UEFI or legacy BIOS) and whether Secure Boot is on.
- Quick tiles for processor, memory, graphics, storage, displays, network, battery and motherboard. Click any tile to jump to its details.
- **Needs attention** list: anything the inventory flagged as a warning or problem (for example a device with a driver error, a nearly full drive, a worn battery).

## Hardware inventory ✅
Every piece of hardware Windows can see, in as much detail as Windows reports.

| Category | What it shows |
|---|---|
| System | Make, model, serial number, UUID, form factor, Windows edition/version/build, install date, uptime, BIOS/UEFI version and date, Secure Boot, motherboard model/revision/serial, case type, TPM chip version and state |
| Processor | Model, socket, cores, threads, base and current speed, load, cache sizes, virtualization support |
| Memory | Each RAM stick: **which slot it's in**, size, type (DDR4/DDR5…), rated and actual speed, maker, part number, serial number, voltage. Plus total slots and maximum supported memory |
| Graphics | Each graphics card: model, real video memory size, current resolution and refresh rate, driver version and date |
| Displays | Each monitor: model, maker, **how it's connected (HDMI, DisplayPort, DVI, built-in)**, screen size, serial number, manufacture date |
| Drives | Each physical drive: model, SSD or hard drive, NVMe/SATA/USB, capacity, Windows health status, firmware, serial number, partitions, which drive letters live on it |
| Volumes | Each drive letter: label, file system, size, free space, % used (warns when nearly full) |
| Network | Each adapter: Wi-Fi/Ethernet/virtual, connected or not, link speed, MAC address, IP addresses, gateway, DNS, DHCP, driver |
| Battery | Health (% of original capacity), design vs. current capacity, charge cycles, charge level, chemistry, serial number |
| Printers | Name, driver, port, default/network/shared |
| Ports | Physical connectors the motherboard reports (USB, audio, video, network…) |
| Expansion slots | Motherboard slots and whether each is in use |
| USB | Every USB controller, hub and device, with **the exact port and hub it's plugged into** and its vendor/product ID |
| Audio | Sound cards, speakers, microphones |
| Input | Keyboards, mice, touchpads |
| Cameras | Webcams and scanners |
| Bluetooth | Bluetooth radio and paired devices |
| All devices | Every device in Device Manager, including problem codes explained in plain English |

**Where things are plugged in:** every device shows its bus (PCI Express, USB, SATA/NVMe…), its location (for example "USB port 3 on hub 2" or "PCI bus 0, device 20"), what it's connected through, and the full path from the motherboard down to the device.

**Driver details:** for every device, its driver version, date, maker, INF file, whether it's digitally signed and who signed it.

## Simple and Advanced modes ✅
- **Simple** shows the important facts with friendly names.
- **Advanced** adds every raw property Windows reports (often 50+ per device), searchable, and lets you add any of them as a table column.
- The switch is in the top bar and in Settings. DxTool remembers your choice.

## Tables and cards ✅
Every list in DxTool can be shown as a **table** or as **cards**.
- **Search** across every column.
- **Filter** each column separately.
- **Sort** by clicking a column header (Shift+click to sort by several columns).
- **Choose columns**: show or hide any column, including every raw property in Advanced mode.
- **Reorder columns** by dragging headers; **resize** by dragging the edge (double-click to reset).
- **Compact** or comfortable row height.
- **Copy table** to the clipboard, ready to paste into Excel, Word or an email.
- Layouts are remembered per category.

## Detail panel ✅
Click any item to open a side panel with everything about it: summary, connection path, driver, partitions and (in Advanced mode) every raw property. Search inside it, copy everything with one click, or double-click any value to copy it.

## Other ✅
- Dark and light themes, or follow Windows.
- Banner when DxTool isn't running as administrator, with a one-click **Restart as administrator** button.
- Works fully offline.

---

## Coming in version 1 🔜

### Scans (Stage 3)
- **Fast Scan**: the most important checks in under a minute.
- **Full Scan**: every check.
- **Custom Scan**: pick which checks to run.
- Health score and a list of findings ranked by how serious they are, each with a plain-English explanation and suggested fix.

### Health checks (Stage 3)
- Drive health: wear level, temperature, power-on hours, read/write errors.
- Crash analysis: blue screens explained in plain English.
- Event log review: hardware errors, unexpected shutdowns, failed drivers.
- Windows health: pending updates, damaged system files, startup programs.
- Security status: Defender, firewall, TPM, Secure Boot, BitLocker.

### History (Stage 4)
- Every scan saved per machine; compare two scans to see what changed.

### Reports (Stage 4)
- HTML and PDF reports, readable by the PC owner and detailed enough for a technician.
- Your logo, company name and technician name, set once in Settings; each can be hidden.

### Fixes (Stage 5)
- Separate section, unlocked after a scan and a risk acknowledgement.
- Restore point created before any change.
- Clear temporary files, repair system files (SFC/DISM), update drivers, and more.

### Threat scanning and Claude (Stage 6)
- Virus scan using Microsoft Defender's engine.
- Second-opinion scan with ClamAV and downloadable threat databases.
- Connect DxTool to Claude (MCP) so Claude can run checks and explain results.

### Distribution
- Portable single program (no install needed), plus optional installer.
- Automatic updates from GitHub releases.

## Later 💭
- Bootable USB version: rescue files, find out why Windows won't start, fix boot problems, offline virus scan and removal.
- Mac support for the desktop app.
- Live temperature and fan sensors (needs a safe driver approach).
