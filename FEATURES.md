# DxTool Features

A plain-English list of what DxTool does. ✅ = working now, 🔜 = planned for version 1, 💭 = later idea.

---

## Overview page ✅
The first screen you see.
- Computer name, make and model, Windows version and build, how long the PC has been on.
- Boot mode (UEFI or legacy BIOS) and whether Secure Boot is on.
- Quick tiles for processor, memory, graphics, storage, displays, network, battery and motherboard. Click any tile to jump to its details.
- **Health card**: run a Fast scan with one click, or see the latest health score.
- **Needs attention** list: anything the inventory or the latest health scan flagged as a warning or problem (for example a failing drive, a blue screen, a device with a driver error, a nearly full drive, a worn battery).

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

## Scans ✅
Scans only look. They never change anything on the PC.
- **Fast scan**: 14 of the most important checks, looking back 30 days. Takes a few seconds.
- **Full scan**: 19 checks, looking back 90 days. Usually well under a minute.
- **Custom scan**: tick exactly which checks to run, including the slow **deep system file check** (5 to 15 minutes). DxTool remembers your picks.
- **Live progress**: watch each check tick off as it finishes. You can keep using the rest of DxTool while a scan runs.
- Checks that need administrator rights say so. Without admin rights they still run but give partial results, and DxTool tells you which ones.

## Health ✅
The results of the latest scan.
- **Health score** from 0 to 100, with a grade (Excellent, Good, Fair, Poor) and a one-line verdict.
- **Findings**, most serious first: **Problems** (red), **Warnings** (amber) and **Notes** (blue, for your information only; they don't lower the score). Filter by type.
- Click any finding for **what it means** in plain English, **what to do** about it, and the supporting facts for a technician. Findings that a future one-click repair will handle are marked.
- **All checks**: every check with its one-line result. Click one to see the facts it measured and a full table of what it found (drives, crashes, events, drivers, startup programs and so on). The tables work like every other table in DxTool.

### What the checks look at

| Area | Check | What it finds |
|---|---|---|
| Storage | Drive health | Drives that report failing or degraded, SSD wear (% of life used), temperature, power-on hours, power cycles, errors the drive couldn't correct, failure prediction *(full detail needs admin)* |
| Storage | Free space | Drives low on space (stricter for the Windows drive), size of temporary files and the Recycle Bin |
| Storage | Drive and file system errors | Bad blocks, drive controller errors, retried reads and writes, file system corruption, duplicate disk IDs, with the drive named where possible |
| Stability | Blue screens and unexpected shutdowns | Every blue screen with its **stop code explained in plain English** (about 50 common codes) and advice; crash dump files; times the PC lost power, froze or the power button was held |
| Stability | Hardware errors | Processor, memory and PCI Express errors reported by the hardware (serious vs. self-corrected), graphics driver crashes and recoveries, processor slowed by heat or power limits, Windows Memory Diagnostic results |
| Stability | Device problems | Devices that aren't working, are missing drivers or are disabled, with advice per problem code |
| Stability | Drivers | Unsigned drivers, how old the graphics driver is, and a list of every third-party driver with version, date and age |
| Stability | Service failures | Background services that failed to start or crashed, grouped by service |
| Stability | App crashes | Programs that crashed or froze, how often, and the file most often involved |
| Windows | Windows Update | When updates were last installed, a pending restart, updates that keep failing (Store app updates are separated out), a disabled Update service |
| Windows | Windows system image | Asks Windows whether its repair source is damaged (DISM) *(needs admin)* |
| Windows | Deep system file check | System File Checker in verify-only mode *(Custom scan only, needs admin)* |
| Windows | Startup programs and boot time | Every program that starts with Windows (enabled or disabled), too many startup programs, how long startup takes |
| Windows | Memory and performance | Memory in use, committed memory, uptime (reminds you to restart), processor load, page file, and the programs using the most memory |
| Windows | Battery | Battery wear compared with its original capacity (laptops) |
| Windows | Windows activation | Activated or not, edition, licence type, last 5 characters of the product key |
| Security | Antivirus | Which antivirus is on, definition age, last quick and full scans, Tamper Protection, more than one antivirus running, threats Defender found |
| Security | Firewall | Firewall on or off for each network type, and which one the current network uses |
| Security | Device security | Secure Boot, TPM, BitLocker, User Account Control, memory integrity, Remote Desktop, old SMB1 file sharing |
| Network | Network and internet | Internet access as Windows sees it, addresses, router address problems, slow wired links, weak Wi-Fi, proxy settings, custom hosts file entries. DxTool doesn't contact the internet for this |

## History ✅
Every scan is saved automatically, grouped by PC.
- **Every PC you've scanned** in one list, with its make, model, serial number and latest score. "This PC" is marked.
- **Health score over time** as a small chart.
- **All scans** for a PC: date, type, score, problems, warnings, notes and whether it ran as administrator. **Open** any scan to see its full results, make a **Report** from it, or **Delete** it (asks first).
- **Compare two scans**: tick two and click Compare to see the score change, what's **new**, what's been **fixed**, what's **still there**, and which checks changed.
- DxTool reopens the latest scan of this PC when it starts.
- **Portable:** everything lives in a `DxTool-Data` folder next to DxTool. Run DxTool from a USB stick and the history of every PC you scan goes with it. (If that folder can't be written to, DxTool uses `%LOCALAPPDATA%\DxTool` instead.)

## Reports ✅
Turn any scan into a report you can give to the PC's owner.
- **Live preview** that updates as you change options.
- **Save as PDF or HTML.** Reports are saved in `DxTool-Data\reports\<PC>`, with **Open** and **Show in folder** buttons. PDFs are made by Microsoft Edge, which is built into Windows, so nothing extra is needed.
- **Written for the owner first:** the score and a one-line verdict, then "What needs attention" with what to do about each item.
- **Detailed enough for a technician:** optional technical section with evidence, measured facts and detail tables for every check.
- **Options per report:** title, who it's prepared for, your own technician notes, and whether to include computer details, notes, the list of checks and the technical section.
- **Branding:** your logo, company name, contact details and technician name at the top. Set them once in **Settings**; each can be hidden in Settings or per report.
- The footer records the DxTool version, scan type, whether it ran as administrator, and which checks were partial.

## Security page ✅
The antivirus, firewall and device security results from the latest scan, all in one place. Virus scanning is coming in Stage 6.

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
- **Settings are saved in the DxTool-Data folder**, so they travel with DxTool on a USB stick.
- Banner when DxTool isn't running as administrator, with a one-click **Restart as administrator** button.
- Works fully offline.

---

## Coming in version 1 🔜

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
- Naming the exact driver behind a blue screen by reading the full crash dump.
