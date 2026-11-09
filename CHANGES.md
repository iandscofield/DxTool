# DxTool Changes

Newest first. Written in plain English: what changed and why it matters.

---

## Version 0.11.0 · Extra Features 1: Android phones · 2026-11-08

### New
- **Phone page** (Sidebar → Tools → Phone) for Android phones connected by USB.
  - **Android tools on click:** downloads Google's adb (about 8 MB) after you agree to Google's terms, checked against Google's checksum.
  - **Setup help:** how to switch on USB debugging, and a live "looking for phones" that notices the phone and the "Allow" step.
  - **Diagnostics:** battery (level, Android's health verdict, temperature, charge cycles and remaining capacity where reported), storage, security patch age, Android version, memory, uptime, with plain-English warnings.
  - **Photo and video backup** to Pictures\Phone backups (or any folder), copying only new files each time, with progress and Stop.
  - **Text message backup** as a readable page, a spreadsheet and a data file. If the phone blocks this, DxTool copies the backup made by the SMS Backup & Restore app instead.
- **Claude** can read a connected phone's diagnostics.
- That finishes Extra Features 1.

### Fixed during testing
- Starting the Android tools froze the check. Their background service kept DxTool's connections open, which in the full app could also have frozen other tasks running at the same moment. DxTool now starts that service completely separately.

### Tested
- **Android tools download on this PC:** Google's list gave version 37.0.1 (8 MB). It matched the checksum, unpacked and started. With no phone connected, the page and Claude both say so correctly, and stopping the service works.
- **The Phone page** in the preview browser through every step: download, connected phone, diagnostics, backup progress and results.
- **Automatic tests** for reading the phone's answers: device list, Google's download list, battery report (including capacity and cycles), storage, memory, the photo list (hidden thumbnail folders skipped, Windows-unsafe file names fixed) and text messages (including line breaks and commas inside messages).
- **Not yet tested with a real phone:** the diagnostics and backups need an Android phone with USB debugging switched on.

---

## Version 0.10.0 · Extra Features 1: Part prices · 2026-11-08

### New
- **Price search buttons on the Upgrades page.** Every suggestion that means buying something (an SSD, more memory, a graphics card) has buttons for **PCPartPicker**, **Amazon**, **Newegg** and **eBay**.
- **Prices for this PC's parts:** a new card at the bottom of the Upgrades page listing the main parts with the same buttons: the computer or laptop model, processor, graphics card, memory, drives, motherboard and monitors.
  - Memory is searched by its exact part number (for example "Corsair CMH32GX5M2E6000C36"), so it finds the same kit.
  - Processor names are cleaned up the way shops list them ("Intel Core i7-13700F").
  - Identical drives are listed once, with a count.
- DxTool doesn't read prices itself, as agreed: the buttons open the shop's search in your browser.
- **Claude** gets the parts list and price-search links with the upgrade advice.

### Tested
- **This PC:** listed the MSI computer, i7-13700F, RTX 4070 Ti, the Corsair memory kit (by part number), 2 × Samsung 990 PRO 4TB and an MSI M450, the MSI PRO Z690-A WIFI motherboard and both monitors (LG 27GL850, Samsung Odyssey G70NC). Virtual displays were left out.
- **The made-up old laptop:** no graphics card or motherboard listed (they can't be swapped), and the processor marked "for reference only".
- **Automatic tests** for maker names ("Micro-Star International" → MSI) and placeholder names like "To Be Filled By O.E.M.".
- **The Upgrades page** in the preview browser, and Claude's upgrade tool on the real program.

---

## Version 0.9.0 · Extra Features 1: Newest drivers · 2026-11-08

### New
- **Drivers page** (Sidebar → Tools → Drivers). Click **Check for newer drivers** to compare the most important drivers with the newest from the makers:
  - **NVIDIA** graphics cards: the exact newest driver for the card, straight from NVIDIA.
  - **Intel** graphics, Wi-Fi, Bluetooth and Ethernet: matched by device ID against Intel's own driver list.
  - **AMD** graphics cards: the installed driver's version and age, with a button to AMD's driver page (AMD offers no way to look it up).
- **Download** buttons open the maker's own download in the browser. You run the file yourself, and the page explains how.
- **Other drivers** button: opens the existing Windows Update driver repair for everything else.
- The last check is saved, so it shows again offline.
- **Claude** can check for newer drivers.

### Tested
- **This PC:**
  - NVIDIA RTX 4070 Ti: found that 621.14 (4 November) is out and this PC has 621.01.
  - Intel Wi-Fi 6E AX211: 24.70 is out, this PC has 23.30 (from January 2024).
  - Intel Bluetooth: 24.70 is out, this PC has 24.40.
  - The download links work: NVIDIA's is about 1 GB, Intel's Wi-Fi one 54 MB.
- **NVIDIA lookups** for an old GTX 680 (gives its final driver, 475.14, from 2024), a laptop RTX 4070 (gives the laptop download) and a made-up card (handled as "no driver listed").
- **Automatic tests** for NVIDIA's version numbering, version comparison and Intel device matching, including skipping old "Historical" packages and packages for the other Windows version.
- **The Drivers page** in the preview browser, and Claude's drivers tool on the real program.

---

## Version 0.8.0 · Extra Features 1: Game check · 2026-11-08

### New
- **Games page** (Sidebar → Tools → Games).
  - Lists games installed through Steam and the Epic Games Launcher.
  - Checks any Steam game against this PC's processor, graphics card, memory, free space and Windows version.
  - Each result is **Runs well**, **Low settings**, **Too slow** or **Can't tell**, with a part-by-part table and the Steam store link.
- **Check all** for every unchecked installed game.
- **Results are saved**, so games can be looked at again offline. Only searching and first checks use the internet.
- **Claude** can list installed games and check whether a game will run.
- **Upgrades page:** laptop processors from the 10th generation on (for example i5-1135G7) were mistaken for 1st-generation chips in the Windows 11 check. Fixed.

### Tested
- **This PC:** found all 170 installed games (168 from Steam, 2 from Epic) and left out tools and runtimes.
- **70 of them checked against Steam,** including GTA V, Cyberpunk 2077, Battlefield 6, Baldur's Gate 3 and Helldivers 2. All run well on this PC, as expected. The only "Can't tell" results are games whose requirements say nothing useful ("N/A").
  - The audit found and fixed games that list only minimum requirements: a powerful PC was being rated "Low settings".
  - It also added readings for vague requirements ("Intel Core i5 3rd gen", "2 GB dedicated memory", "Integrated graphics") and odd spellings ("RTX2060", "R5700").
- **The made-up old laptop** (6th-gen i5 laptop chip, Intel HD 520, 4 GB):
  - GTA V: low settings;
  - Among Us: low settings;
  - Cyberpunk 2077 and Baldur's Gate 3: too slow, on processor, graphics and memory.
- **Automatic tests** for reading Steam's requirement pages and the processor and graphics names.
- **The Games page** in the preview browser, and Claude's game tools on the real program.

---

## Version 0.7.0 · Extra Features 1: Upgrade recommendations · 2026-11-08

### New
- **Upgrades page** (in the sidebar under Tools). Upgrade advice worked out from the PC's own hardware, with no internet needed.
  - Covers memory, drives, Windows 11 readiness, laptop batteries, graphics and processor age.
  - Each suggestion has a priority, a rough cost, why it helps and what to do, plus the part to look for.
  - Also lists what's already good.
- **Spots memory running below its rated speed** (the XMP/EXPO setting switched off in the BIOS), by reading the memory's part number. Supported brands: Corsair, G.Skill, Kingston Fury, Crucial and TeamGroup. **This PC has it:** its DDR5-6000 memory runs at 4000.
- **Claude** can ask DxTool for the same upgrade advice.

### Tested
- **This PC:** the only suggestion is turning on XMP. Everything else is correctly listed as already good: NVMe SSD, 64 GB, RTX 4070 Ti, 13th-gen processor.
- **A made-up old Windows 10 laptop** (6th-gen i5, 4 GB, hard drive, worn battery, nearly full C:) gets, in order:
  - "can't run Windows 11" with the reason;
  - move to an SSD;
  - a 2 × 8 GB memory kit;
  - replace the battery;
  - free up space;
  - "processor getting old".
- **Automatic tests** for reading memory part numbers and the Windows 11 processor rule.
- **The page in the preview browser,** and the Claude tool on the real program.
- **Updating from 0.6.0 to 0.7.0, using the real GitHub releases:**
  - **Portable:** the published 0.6.0 DxTool.exe was updated by DxTool's own update code while it was running. It downloaded and checked 0.7.0, then swapped the program file.
  - **Installed:** the published 0.6.0 installer, then the 0.7.0 installer in DxTool's quiet update mode. Saved data was kept, and the test copy was removed afterwards.

---

## Version 0.6.0 · Stage 7: Downloads, installer and updates · 2026-11-08

This completes the version 1 plan.

### New
- **Releases on GitHub.** Each new version is built on GitHub's own computers and published on the project's Releases page, with:
  - **DxTool.exe:** the portable version, no installing needed;
  - **an installer** for people who prefer a Start menu shortcut and an uninstall entry;
  - **a checksum file** DxTool uses to make sure an update downloaded correctly.
- **The installer:**
  - installs for your Windows account, with no administrator password needed;
  - asks whether to connect DxTool to Claude Desktop, if Claude Desktop is installed;
  - on uninstall, also disconnects Claude, and can delete DxTool's saved data if you tick the box.
- **Updates** in Settings → Updates.
  - **Check for updates** shows the new version's notes.
  - **Download and install** checks the download, then DxTool closes and reopens as the new version.
  - Works for both the portable and the installed version, and keeps all your saved scans and settings.
  - Optional **Check when DxTool starts** setting, off by default (DxTool stays offline unless you ask).
- **The DxTool project on GitHub is now public**, as planned for a free, open-source tool.

### Good to know
- **Windows SmartScreen** may say "Windows protected your PC" the first time you run a downloaded copy, because DxTool isn't code-signed yet. Choose **More info → Run anyway**. Updates downloaded by DxTool itself don't trigger this.
- **Updating while Claude Desktop is using DxTool** closes Claude's copy of DxTool. Restart Claude Desktop afterwards.

### Tested
- **The installer, on this PC, in a test folder:**
  - a quiet install put the program, Start menu shortcut and Apps entry in place;
  - an update over it kept the saved data;
  - the uninstall removed everything except the data folder (deleting that is the checkbox's job);
  - the test copy was then deleted.
- **Portable update:** swapping the program file while an older copy was still running (the way Claude Desktop keeps it open); the new copy then started as 0.6.0.
- **Update check:** gives a clear message when no release can be found. Version comparison and checksum reading have automatic tests.
- **The new Settings card and the "new version available" note**, in the preview browser.

### Not tested yet
- **The Claude question in the installer:** Claude Desktop isn't installed on this PC. Try the installer on a PC that has it.
- **A real update from one GitHub release to the next:** this needs two published releases. The first real test will be updating 0.6.0 to the next version.

---

## Version 0.5.1 · ClamAV tested on a real PC · 2026-11-08

### Fixed
- **ClamAV quick scan was far too slow.** On this PC it took **2 hours** and went through 58,000 files, because unpacked programs in Downloads (one AI tool alone had 27,000 files) and big installers were all included. It now looks only at the file types that can carry viruses, near the top of Downloads, Desktop and temp, skips files over 20 MB, and checks programs and scripts (not app libraries) in AppData and ProgramData. The same PC now takes **24 minutes** for about 1,500 files.
- **Files Defender had already caught were reported as "no threats".** When Defender blocks a file, ClamAV can't open it. DxTool now lists such a file as a threat "Detected by Microsoft Defender" and says how many files that happened to.
- **"Remove with Defender"** now finds each file even when Defender reports it under a different threat name, and looks back far enough to find older detections.
- On Defender's threats, DxTool's own Quarantine/Delete buttons are hidden; use **Remove with Defender** for those.
- The database update log no longer fills with download progress lines.

### Good to know
- **ClamAV is slow on Windows:** about half a second per small file, and up to a minute for a large installer or PDF. Loading its 3.6 million signatures takes 1 to 2 minutes before any file is scanned. Defender's own checks on every file ClamAV opens probably add to this.

### Tested
- **ClamAV, end to end, on this PC:** download and install (ClamAV 1.5.4, 3.6 million signatures), database update, quick scans (no threats on this PC), a harmless home-made test signature to check that a detection is reported and can be quarantined, and a file Defender had already blocked.

### Not tested yet
- **Connecting your real Claude Desktop.** Try **Connect to Claude Desktop** in Settings, then fully quit and reopen Claude Desktop.

---

## Version 0.5.0 · Stage 6: Virus scanning and Claude · 2026-11-07

### New
- **Virus scan page.**
  - **Microsoft Defender:** Quick, Full, folder or single-file scans, plus a button to update its definitions.
  - **ClamAV second opinion:** a free extra scanner DxTool downloads only when you click, with its virus databases.
  - Live progress, threats shown as they're found, and a Stop button.
- **Choose what to do with each threat:**
  - Remove with Defender.
  - Quarantine: DxTool keeps a scrambled copy that can be restored later.
  - Delete permanently, after a confirmation.
- **DxTool quarantine** list with Restore and Delete.
- **Virus scan history** for each PC. Reports can now include a **Virus scans** section.
- **Claude AI assistant.**
  - **Settings → Claude AI assistant** connects DxTool to Claude Desktop in one click, or gives a command for Claude Code.
  - Claude can then run health scans and read the results, hardware details, event logs and antivirus status, and explain them in plain English.
  - Claude can't run repairs or delete files; those stay in DxTool.
- The Security page links to the new Virus scan page.

### Good to know
- **Defender without administrator rights:** it finds threats, but sometimes leaves them in place. "Remove with Defender" then quarantines them, which worked here without administrator rights.
- **ClamAV's first download is large:** about 450 MB (the program, then the virus databases). ClamAV's servers limit how often databases can be downloaded; if they ask DxTool to wait, it says so.
- **ClamAV needs memory:** a scan uses about 1.5 GB while it runs, and the first minute is spent loading its 3.6 million signatures.

### Tested
- **Defender:** found the standard harmless antivirus test file (EICAR) and reported it correctly. "Remove with Defender" quarantined it. One detection covering two files now lists both.
- **DxTool's quarantine:** moving a file in, refusing to act twice, and restoring it, using a harmless test file.
- **Claude connection:** Claude's start-up and tool messages, sent to the finished program. Connecting and disconnecting, tested on a copy of Claude Desktop's settings: other settings are kept and a backup is made.
- **The new screens,** in the preview browser.

### Not tested yet
- **ClamAV itself.** My safety rules stop me from running programs downloaded from the internet, so I couldn't run ClamAV's download, database update or scan here. That code follows ClamAV's documented commands. **Please try "Download ClamAV" and a ClamAV quick scan**, and tell me if anything fails.
- **Connecting your real Claude Desktop.** I didn't change your real Claude settings. Try **Connect to Claude Desktop** in Settings, then fully quit and reopen Claude Desktop.

---

## Version 0.4.0 · Stage 5: Fixes · 2026-11-07

### New
- **Fixes section.** It unlocks after this PC has been scanned and you've ticked two boxes acknowledging the risks.
- **A restore point is created before every run.** If Windows can't make one, nothing runs. DxTool explains why and offers two ways forward: turn on System Protection, or continue without a restore point after a second warning.
- **Eight fixes:**
  - Clear temporary files.
  - Repair the Windows system image (DISM).
  - Repair Windows system files (SFC).
  - Check a drive for errors (chkdsk).
  - Reset Windows Update.
  - Clear the DNS cache.
  - Reset network settings.
  - Install driver updates.
- **Every fix shows "What it changes"** and its risk level. A confirmation screen lists everything before it runs.
- **Driver updates from Windows Update:** the installed driver sits next to the newest available.
  - Offers that are older than what's installed are flagged and can't be selected.
  - BIOS/firmware updates carry an extra warning.
  - On this PC, Windows Update offered a BIOS update from 2023 while the installed BIOS is from 2026. DxTool now catches that.
- **Live progress** for each step, with the repair tool's own output for technicians.
- **Restart button** when a fix needs a restart (30-second countdown you can cancel).
- **"Undo with System Restore"** opens Windows' restore wizard.
- **Fix log per PC.** Reports can include a "Repairs carried out" section for fixes run after the scan.
- **"Fix this" buttons** on Health and Security findings jump straight to the matching fix. The Fixes page also shows "Suggested by your scan".

### Safety details
- Temporary file clean-up only ever touches the exact, well-known temp folders, and never follows links out of them.
- Emptying the Recycle Bin is off by default and is called out as permanent in the confirmation.
- Windows normally makes at most one restore point a day and silently skips the rest. DxTool briefly lifts that limit so a new one is really created, then puts the setting back.

### Known limitations
- **I could only test parts that don't need administrator rights:**
  - Clearing a test temp folder.
  - Checking Windows Update for drivers.
  - The restore point rule (fixes don't run without one).

  The repairs themselves and creating restore points need administrator rights, which I can't get. **Please try a low-risk fix first**, such as clearing the DNS cache, before the long repairs.
- Installing driver updates accepts the driver makers' licence terms on your behalf, as Windows Update does.

---

## Version 0.3.2 · PDF export fix · 2026-11-07

### Fixed
- **Saving a report as PDF failed when DxTool was running as administrator.** Edge (which makes the PDF) restarts itself as a normal user when started from an administrator program. DxTool now stops that and waits for the PDF to finish writing.
- If a PDF still can't be made, the HTML version is kept, and the message says where it is so you can print it from a browser.
- Leftover temporary files from making PDFs are now cleaned up.

---

## Version 0.3.1 · Color themes · 2026-11-07

### New
- **Color themes** in Settings → Appearance: blue (the original), purple, red, green, and orange & black. Orange & black also turns dark mode pure black. Each theme works in both light and dark mode, and DxTool remembers your choice.

### Planned
- **Extra Features 1 added to the plan** (from the requirements document), to be built after version 1. Online lookups only happen when you click.
  - **Driver versions:** the installed driver next to the newest available. This joins Stage 5's driver updates.
  - **Part prices.**
  - **Upgrade recommendations.**
  - **Game requirement checks.**
  - **Phone diagnostics.**
  - **Phone photo and text backups.**

---

## Version 0.3.0 · Stage 4: history and reports · 2026-11-07

### New
- **Every scan is saved automatically**, per PC, in a `DxTool-Data` folder next to DxTool. Carry DxTool on a USB stick and the history of every PC goes with it.
- **History page:**
  - Every PC you've scanned, with its make, model and serial number.
  - A chart of its health score over time.
  - Every scan, which you can open again, make a report from, or delete.
- **Compare two scans:** see the score change, what's new, what's been fixed and what's still there.
- **DxTool reopens this PC's latest scan when it starts**, so results don't vanish when you close it.
- **Reports page** with a live preview. Save as **PDF** or **HTML**, then open the file or show it in its folder.
- **Reports are written for the PC owner first** (score, verdict, what needs attention and what to do). There's an optional technical section for a technician.
- **Per-report options:** title, "prepared for", technician notes, and which sections to include.
- **Report branding in Settings:** logo, company name, contact details and technician name. Each can be hidden in Settings or per report.
- **Health page:** a "Make a report" button. A banner shows when you're looking at an older saved scan or one from another PC.
- **Settings** now shows where the data folder is, with a button to open it.

### Improved
- Virtual display adapters (from VR or remote desktop software) are left out of the computer's graphics list in history and reports.

### Known limitations
- **Reports are always saved into the DxTool-Data folder.** Use "Show in folder" to copy or email them. There's no "Save as…" to choose another location yet.
- **PDFs need Microsoft Edge**, which every Windows 10 and 11 PC has unless someone removed it. If it's missing, save as HTML and print from any browser.
- **Saved scans include full details**, roughly 1 MB each, so a few hundred scans take a few hundred MB.

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
