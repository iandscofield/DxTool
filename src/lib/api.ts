import { Channel, invoke } from "@tauri-apps/api/core";
import type { GameCheck, InstalledGame, SearchHit, UpgradeReport, UpdateInfo, AvEngine, AvEvent, AvKind, AvStatus, ClaudeStatus, QuarantineItem, Threat, VirusScan, AppInfo, AppSettings, CheckInfo, DriverUpdate, FixEvent, FixInfo, FixRequest, FixRun, Inventory, MachineInfo, MachineRecord, ScanEvent, ScanKind, ScanReport } from "./types";
import { defaultBranding } from "./types";

/** True inside the DxTool window; false when the UI is opened in a normal browser during development. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Development-only: when previewing the UI in a browser, read a saved snapshot instead.
// Create it with: cargo run -p dxcore --example inventory -- --json > public/__mock/inventory.json
async function mock<T>(name: string): Promise<T> {
  const r = await fetch(`/__mock/${name}.json`);
  if (!r.ok) throw new Error("Not running inside DxTool and no development snapshot found.");
  return r.json();
}

export const api = {
  appInfo: () =>
    inTauri
      ? invoke<AppInfo>("app_info")
      : Promise.resolve<AppInfo>({ version: "dev", isAdmin: false, computerName: "Browser preview", firmware: "UEFI", debugBuild: true }),
  inventory: () => (inTauri ? invoke<Inventory>("get_inventory") : mock<Inventory>("inventory")),
  catalog: () => (inTauri ? invoke<CheckInfo[]>("scan_catalog") : mock<CheckInfo[]>("catalog")),
  runScan: (kind: ScanKind, checks: string[], onEvent: (e: ScanEvent) => void): Promise<ScanReport> => {
    if (!inTauri) return mockScan(kind, checks, onEvent);
    const ch = new Channel<ScanEvent>();
    ch.onmessage = onEvent;
    return invoke<ScanReport>("run_scan", { kind, checks, onEvent: ch });
  },
  currentMachine: () => (inTauri ? invoke<MachineInfo>("current_machine") : mock<ScanReport>("scan").then((r) => r.machine)),
  listMachines: async (): Promise<MachineRecord[]> => {
    if (inTauri) return invoke<MachineRecord[]>("list_machines");
    const r = await mock<ScanReport>("scan");
    const s = { id: r.id, kind: r.kind, startedAt: r.startedAt, elapsedMs: r.elapsedMs, score: r.score, grade: r.grade, counts: r.counts, isAdmin: r.isAdmin, checks: r.results.length };
    const older = { ...s, id: "20261020-101500", startedAt: "2026-10-20T10:15:00-07:00", score: 82, grade: "Good", counts: { error: 0, warn: 3, info: 6 } };
    return [{ info: r.machine, firstSeen: older.startedAt, lastSeen: r.startedAt, scans: [s, older] }];
  },
  loadScan: (machineId: string, scanId: string) => (inTauri ? invoke<ScanReport>("load_scan", { machineId, scanId }) : mock<ScanReport>("scan")),
  deleteScan: (machineId: string, scanId: string) => (inTauri ? invoke<void>("delete_scan", { machineId, scanId }) : Promise.resolve()),
  dataDir: () => (inTauri ? invoke<string>("data_dir") : Promise.resolve("(browser preview)")),
  loadSettings: async (): Promise<AppSettings> => {
    const raw: Partial<AppSettings> = inTauri ? await invoke<Partial<AppSettings>>("load_settings") : loadPref("settings", {});
    return { ...raw, branding: { ...defaultBranding, ...(raw.branding ?? {}) } };
  },
  saveSettings: (settings: AppSettings) => (inTauri ? invoke<void>("save_settings", { settings }) : Promise.resolve(savePref("settings", settings))),
  exportReport: (machineId: string, name: string, html: string, format: "html" | "pdf") =>
    inTauri ? invoke<string>("export_report", { machineId, name, html, format }) : Promise.reject("Saving reports only works inside DxTool"),
  openPath: (path: string) => (inTauri ? invoke<void>("open_path", { path }) : Promise.resolve()),
  showInFolder: (path: string) => (inTauri ? invoke<void>("show_in_folder", { path }) : Promise.resolve()),
  fixCatalog: () => (inTauri ? invoke<FixInfo[]>("fix_catalog") : mock<FixInfo[]>("fixes")),
  runFixes: (requests: FixRequest[], skipRestorePoint: boolean, onEvent: (e: FixEvent) => void): Promise<FixRun> => {
    if (!inTauri) return mockFixes(requests, skipRestorePoint, onEvent);
    const ch = new Channel<FixEvent>();
    ch.onmessage = onEvent;
    return invoke<FixRun>("run_fixes", { requests, skipRestorePoint, onEvent: ch });
  },
  checkDriverUpdates: (onLine: (l: string) => void): Promise<DriverUpdate[]> => {
    if (!inTauri) return mock<DriverUpdate[]>("drivers");
    const ch = new Channel<string>();
    ch.onmessage = onLine;
    return invoke<DriverUpdate[]>("check_driver_updates", { onLine: ch });
  },
  listFixRuns: (machineId: string) => (inTauri ? invoke<FixRun[]>("list_fix_runs", { machineId }) : Promise.resolve(loadPref<FixRun[]>("mockFixRuns", []))),
  ntfsDrives: () => (inTauri ? invoke<[string, string, string][]>("ntfs_drives") : Promise.resolve<[string, string, string][]>([["C:", "Windows", "4.00 TB"], ["D:", "Games", "4.00 TB"]])),
  restorePoints: () => (inTauri ? invoke<[number, string, string][]>("restore_points") : Promise.resolve<[number, string, string][]>([])),
  enableSystemProtection: () => (inTauri ? invoke<void>("enable_system_protection") : Promise.resolve()),
  openSystemRestore: () => (inTauri ? invoke<void>("open_system_restore") : Promise.resolve()),
  restartPc: () => (inTauri ? invoke<void>("restart_pc") : Promise.resolve()),
  cancelRestart: () => (inTauri ? invoke<void>("cancel_restart") : Promise.resolve()),
  relaunchAsAdmin: () => (inTauri ? invoke<void>("relaunch_as_admin") : Promise.reject("Only available inside DxTool")),

  avStatus: () => (inTauri ? invoke<AvStatus>("av_status") : Promise.resolve(mockAvStatus())),
  virusScan: (engine: AvEngine, kind: AvKind, paths: string[], onEvent: (e: AvEvent) => void): Promise<VirusScan> => {
    if (!inTauri) return mockVirusScan(engine, kind, paths, onEvent);
    const ch = new Channel<AvEvent>();
    ch.onmessage = onEvent;
    return invoke<VirusScan>("virus_scan", { engine, kind, paths, onEvent: ch });
  },
  avCancel: () => (inTauri ? invoke<void>("av_cancel") : Promise.resolve(void (mockCancel = true))),
  avUpdate: (engine: AvEngine, onEvent: (e: AvEvent) => void): Promise<string> => {
    if (!inTauri) return mockJob(onEvent, engine === "defender" ? "Updating Microsoft Defender's virus definitions" : "Checking for newer virus databases", "Definitions are up to date.");
    const ch = new Channel<AvEvent>();
    ch.onmessage = onEvent;
    return invoke<string>("av_update", { engine, onEvent: ch });
  },
  clamavInstall: (onEvent: (e: AvEvent) => void): Promise<string> => {
    if (!inTauri) return mockJob(onEvent, "Downloading ClamAV 1.5.4 (225 MB)", "ClamAV 1.5.4 is ready.").then((m) => (savePref("mockClam", true), m));
    const ch = new Channel<AvEvent>();
    ch.onmessage = onEvent;
    return invoke<string>("clamav_install", { onEvent: ch });
  },
  clamavRemove: () => (inTauri ? invoke<void>("clamav_remove") : Promise.resolve(savePref("mockClam", false))),
  virusScans: (machineId: string) => (inTauri ? invoke<VirusScan[]>("virus_scans", { machineId }) : Promise.resolve(loadPref<VirusScan[]>("mockVirus", []))),
  deleteVirusScan: (machineId: string, scanId: string) =>
    inTauri ? invoke<void>("delete_virus_scan", { machineId, scanId }) : Promise.resolve(savePref("mockVirus", loadPref<VirusScan[]>("mockVirus", []).filter((s) => s.id !== scanId))),
  defenderRemove: (machineId: string, scanId: string) =>
    inTauri ? invoke<VirusScan>("defender_remove", { machineId, scanId }) : sleep(1500).then(() => mockMark(scanId, (t) => t.engine === "defender" && t.status === "active", "quarantined", "Quarantined by Defender")),
  threatQuarantine: (machineId: string, scanId: string, path: string) =>
    inTauri
      ? invoke<QuarantineItem>("threat_quarantine", { machineId, scanId, path })
      : Promise.resolve(mockMark(scanId, (t) => t.path === path, "quarantined", "Moved to DxTool's quarantine")).then(() => {
          const item: QuarantineItem = { id: String(Date.now()), originalPath: path, threat: "Win.Test.EICAR_HDB-1", engine: "clamav", quarantinedAt: new Date().toISOString(), sizeBytes: 68 };
          savePref("mockQ", [item, ...loadPref<QuarantineItem[]>("mockQ", [])]);
          return item;
        }),
  threatDelete: (machineId: string, scanId: string, path: string) =>
    inTauri ? invoke<void>("threat_delete", { machineId, scanId, path }) : Promise.resolve(void mockMark(scanId, (t) => t.path === path, "removed", "Deleted permanently")),
  quarantineList: () => (inTauri ? invoke<QuarantineItem[]>("quarantine_list") : Promise.resolve(loadPref<QuarantineItem[]>("mockQ", []))),
  quarantineRestore: (id: string) =>
    inTauri ? invoke<string>("quarantine_restore", { id }) : Promise.resolve(savePref("mockQ", loadPref<QuarantineItem[]>("mockQ", []).filter((q) => q.id !== id))).then(() => "restored"),
  quarantineDelete: (id: string) =>
    inTauri ? invoke<void>("quarantine_delete", { id }) : Promise.resolve(savePref("mockQ", loadPref<QuarantineItem[]>("mockQ", []).filter((q) => q.id !== id))),
  openWindowsSecurity: () => (inTauri ? invoke<void>("open_windows_security") : Promise.resolve()),
  pickFolder: async (): Promise<string | null> => {
    if (!inTauri) return "C:\\Users\\Example\\Downloads";
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ directory: true, multiple: false, title: "Choose a folder or drive to scan" });
    return typeof r === "string" ? r : null;
  },
  pickFile: async (): Promise<string | null> => {
    if (!inTauri) return "C:\\Users\\Example\\Downloads\\setup.exe";
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ directory: false, multiple: false, title: "Choose a file to scan" });
    return typeof r === "string" ? r : null;
  },

  claudeStatus: () =>
    inTauri
      ? invoke<ClaudeStatus>("claude_status")
      : Promise.resolve<ClaudeStatus>({
          desktopInstalled: true,
          state: loadPref("mockClaude", false) ? "set_up" : "not_set_up",
          configuredCommand: "",
          configPath: "C:\\Users\\Example\\AppData\\Roaming\\Claude\\claude_desktop_config.json",
          exe: "C:\\Tools\\DxTool\\DxTool.exe",
          claudeCodeCommand: 'claude mcp add --scope user dxtool -- "C:\\Tools\\DxTool\\DxTool.exe" --mcp',
        }),
  gamesInstalled: (): Promise<InstalledGame[]> =>
    inTauri
      ? invoke<InstalledGame[]>("games_installed")
      : Promise.resolve([
          { id: "1091500", name: "Cyberpunk 2077", launcher: "Steam", steamAppId: 1091500, sizeBytes: 66.8e9, folder: "", lastVerdict: "recommended" },
          { id: "271590", name: "Grand Theft Auto V Legacy", launcher: "Steam", steamAppId: 271590, sizeBytes: 129e9, folder: "", lastVerdict: null },
          { id: "Fortnite", name: "Fortnite", launcher: "Epic", steamAppId: null, sizeBytes: 37.6e9, folder: "", lastVerdict: null },
        ]),
  gameCheck: (appId: number, refresh: boolean): Promise<GameCheck> =>
    inTauri
      ? invoke<GameCheck>("game_check", { appId, refresh })
      : new Promise((r) =>
          setTimeout(
            () =>
              r({
                appId,
                name: appId === 271590 ? "Grand Theft Auto V Legacy" : "Cyberpunk 2077",
                image: "",
                storeUrl: `https://store.steampowered.com/app/${appId}/`,
                overall: appId === 271590 ? "minimum" : "recommended",
                summary: appId === 271590 ? "Meets the minimum but not the recommended requirements for the graphics: expect to use low or medium settings." : "Meets the recommended requirements: expect it to run well, usually on high settings.",
                parts: [
                  { part: "Processor", yours: "13th Gen Intel Core i7-13700F", minimum: "Core i7-6700 or Ryzen 5 1600", recommended: "Core i7-12700 or Ryzen 7 7800X3D", verdict: "recommended", note: "" },
                  { part: "Graphics", yours: "NVIDIA GeForce RTX 4070 Ti", minimum: "GeForce GTX 1060 6GB or Radeon RX 580 8GB", recommended: "GeForce RTX 2060 SUPER or Radeon RX 5700 XT", verdict: appId === 271590 ? "minimum" : "recommended", note: "" },
                  { part: "Memory", yours: "64 GB", minimum: "12 GB RAM", recommended: "16 GB RAM", verdict: "recommended", note: "" },
                  { part: "Storage", yours: "Installed", minimum: "70 GB available space", recommended: "70 GB available space", verdict: "recommended", note: "Already installed." },
                  { part: "Windows", yours: "Windows 11 Pro", minimum: "64-bit Windows 10", recommended: "64-bit Windows 10", verdict: "recommended", note: "" },
                ],
                minimumText: "",
                recommendedText: "",
                fetchedAt: new Date().toISOString(),
                installed: true,
              }),
            500,
          ),
        ),
  gameSearch: (term: string): Promise<SearchHit[]> =>
    inTauri
      ? invoke<SearchHit[]>("game_search", { term })
      : Promise.resolve([
          { appId: 1086940, name: "Baldur's Gate 3", image: "" },
          { appId: 1091500, name: `${term} (preview result)`, image: "" },
        ]),
  gameFind: (name: string): Promise<SearchHit | null> => (inTauri ? invoke<SearchHit | null>("game_find", { name }) : Promise.resolve(null)),
  upgradeAdvice: (refresh: boolean): Promise<UpgradeReport> =>
    inTauri
      ? invoke<UpgradeReport>("upgrade_advice", { refresh })
      : new Promise((r) =>
          setTimeout(
            () =>
              r({
                generatedAt: new Date().toISOString(),
                pcName: "HP ProBook 450 G3 (preview)",
                pcKind: "laptop",
                limited: null,
                items: [
                  {
                    id: "win11_blocked",
                    area: "Windows",
                    priority: "high",
                    cost: "Expensive",
                    title: "This PC can't run Windows 11 yet",
                    why: "Windows 10 stopped getting free security updates in October 2025. It can't move to Windows 11 because the processor (Intel Core i5-6200U) is older than Windows 11 supports.",
                    how: "The choices: a newer PC, Microsoft's paid Extended Security Updates for Windows 10, or a free operating system such as Linux Mint.",
                  },
                  {
                    id: "memory_xmp",
                    area: "Memory",
                    priority: "high",
                    cost: "Free",
                    title: "Turn on XMP so the memory runs at its full 6000 MT/s",
                    why: "The memory is sold as 6000 MT/s but runs at 4000 MT/s, the safe default speed.",
                    how: "Open the BIOS setup, find XMP, choose Profile 1, then save and exit.",
                  },
                  {
                    id: "system_ssd",
                    area: "Storage",
                    priority: "high",
                    cost: "Low cost",
                    title: "Move Windows to an SSD",
                    why: "Windows is on a spinning hard drive (500 GB), the biggest cause of slow start-up.",
                    how: "Replace it with a 500GB SSD and copy Windows across with the SSD maker's free cloning tool.",
                    search: "500GB SSD",
                  },
                  {
                    id: "battery_bat0",
                    area: "Battery",
                    priority: "medium",
                    cost: "Moderate",
                    title: "Replace the battery: it holds only 62% of its original charge",
                    why: "The battery has worn down to 62% of its design capacity.",
                    how: "Search for a replacement battery for this laptop model.",
                    search: "HP TE04XL laptop battery",
                  },
                  {
                    id: "cpu_old",
                    area: "Processor",
                    priority: "low",
                    cost: "Expensive",
                    title: "The processor is getting old",
                    why: "The Intel Core i5-6200U is a generation 6 processor, from 2015 or earlier.",
                    how: "When a bigger upgrade is due, a newer PC is usually better value.",
                  },
                ],
                good: ["16 GB of memory is plenty for everyday use."],
              }),
            500,
          ),
        ),
  installKind: () => (inTauri ? invoke<string>("install_kind") : Promise.resolve("portable")),
  updateCheck: (): Promise<UpdateInfo> =>
    inTauri
      ? invoke<UpdateInfo>("update_check")
      : new Promise((r) =>
          setTimeout(
            () =>
              r({
                current: "0.5.1",
                latest: "0.6.0",
                newer: true,
                notes: ["### New", "- **Installer** that can also connect Claude Desktop.", "- **Updates** from GitHub, from Settings.", "", "### Fixed", "- Something small."].join("\n"),
                published: "2026-11-08T12:00:00Z",
                pageUrl: "https://github.com/iandscofield/DxTool/releases/tag/v0.6.0",
                installKind: "portable",
                downloadName: "DxTool.exe",
                downloadSize: 6_600_000,
              }),
            600,
          ),
        ),
  updateInstall: (onProgress: (p: { done: number; total: number }) => void): Promise<string> => {
    if (!inTauri) {
      return new Promise((resolve) => {
        let done = 0;
        const t = setInterval(() => {
          done += 1_100_000;
          onProgress({ done: Math.min(done, 6_600_000), total: 6_600_000 });
          if (done >= 6_600_000) {
            clearInterval(t);
            resolve("DxTool 0.6.0 is installed and is starting now.");
          }
        }, 400);
      });
    }
    const ch = new Channel<{ done: number; total: number }>();
    ch.onmessage = onProgress;
    return invoke<string>("update_install", { onProgress: ch });
  },
  claudeSetup: (remove: boolean) =>
    inTauri
      ? invoke<string>("claude_setup", { remove })
      : Promise.resolve(savePref("mockClaude", !remove)).then(() =>
          remove ? "DxTool was removed from Claude Desktop. Quit and reopen Claude Desktop to finish." : "DxTool was added to Claude Desktop. Quit Claude Desktop completely (right-click its tray icon → Quit) and open it again.",
        ),
  mcpTryTool: (name: string) =>
    inTauri
      ? invoke<string>("mcp_try_tool", { name })
      : Promise.resolve(`(${name}) This PC\nComputer name: Browser preview\nProcessor: 13th Gen Intel(R) Core(TM) i7-13700F\nMemory: 63.8 GB\n\nLatest health scan: 2026-11-07 20:09 (fast scan), score 100 (Excellent)`),
};

// ───────── Development-only virus scan stand-ins ─────────

let mockCancel = false;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function mockAvStatus(): AvStatus {
  const clam = loadPref("mockClam", false);
  return {
    isAdmin: false,
    defender: {
      available: true,
      reason: "",
      realtime: true,
      runningMode: "Normal",
      productVersion: "4.18.26100.4",
      engineVersion: "1.1.26100.4",
      signatureVersion: "1.463.184.0",
      signatureUpdated: new Date().toISOString(),
      signatureAgeDays: 0,
      quickScanAgeDays: 0,
      fullScanAgeDays: null,
      otherAntivirus: [],
    },
    clamav: {
      installed: clam,
      version: clam ? "1.5.4" : "",
      databases: clam,
      dbVersion: clam ? "27820" : "",
      dbDate: clam ? new Date().toISOString() : "",
      dbAgeDays: clam ? 0 : null,
      signatures: clam ? 3627519 : 0,
      folder: "C:\\Tools\\DxTool\\DxTool-Data\\clamav",
      sizeBytes: clam ? 312_000_000 : 0,
    },
  };
}

async function mockJob(onEvent: (e: AvEvent) => void, phase: string, done: string): Promise<string> {
  mockCancel = false;
  onEvent({ type: "phase", text: phase });
  for (let i = 0; i <= 20; i++) {
    if (mockCancel) throw new Error("Stopped by the user.");
    onEvent({ type: "progress", done: i * 11_000_000, total: 220_000_000, current: "" });
    await sleep(120);
  }
  return done;
}

function mockMark(scanId: string, match: (t: Threat) => boolean, status: Threat["status"], action: string): VirusScan {
  const list = loadPref<VirusScan[]>("mockVirus", []);
  const s = list.find((x) => x.id === scanId)!;
  s.threats = s.threats.map((t) => (match(t) ? { ...t, status, action } : t));
  savePref("mockVirus", list);
  return s;
}

async function mockVirusScan(engine: AvEngine, kind: AvKind, paths: string[], onEvent: (e: AvEvent) => void): Promise<VirusScan> {
  mockCancel = false;
  const started = new Date();
  const total = engine === "clamav" ? 2400 : 0;
  onEvent({ type: "phase", text: engine === "clamav" ? "Finding files to scan" : "Quick scan of the places malware usually hides" });
  await sleep(500);
  if (engine === "clamav") onEvent({ type: "phase", text: "Loading 3,627,519 virus signatures (this can take a minute)" });
  await sleep(700);
  if (engine === "clamav") onEvent({ type: "phase", text: "Scanning files" });
  const threats: Threat[] = [];
  let outcome: VirusScan["outcome"] = "clean";
  for (let i = 1; i <= 30; i++) {
    if (mockCancel) {
      outcome = "cancelled";
      break;
    }
    onEvent({ type: "progress", done: Math.round((total / 30) * i), total, current: `C:\\Users\\Example\\Downloads\\file-${i}.zip` });
    if (i === 18) {
      const t: Threat = {
        name: engine === "clamav" ? "Win.Test.EICAR_HDB-1" : "Virus:DOS/EICAR_Test_File",
        path: "C:\\Users\\Example\\Downloads\\eicar-test.txt",
        engine,
        severity: engine === "defender" ? "Severe" : "",
        status: "active",
        action: engine === "clamav" ? "Found, still on the PC" : "Found, not yet removed",
      };
      threats.push(t);
      onEvent({ type: "threat", threat: t });
    }
    await sleep(100);
  }
  if (outcome !== "cancelled") outcome = threats.length ? "threats" : "clean";
  const scan: VirusScan = {
    id: `${Date.now()}-${engine}`,
    machineId: "SPARTAN-9d7b5352",
    computerName: "Browser preview",
    engine,
    kind,
    targets: kind === "custom" ? paths : [kind === "full" ? "All drives" : "Common malware locations"],
    startedAt: started.toISOString(),
    elapsedMs: Date.now() - started.getTime(),
    filesScanned: engine === "clamav" ? total : 0,
    threats,
    outcome,
    message: outcome === "cancelled" ? "The scan was stopped before it finished." : threats.length ? `Found ${threats.length} threat.` : "No threats found.",
    signatureVersion: engine === "clamav" ? "ClamAV 1.5.4 · daily 27820" : "1.463.184.0",
    log: ["Scan starting...", "Scan finished."],
    appVersion: "dev",
  };
  savePref("mockVirus", [scan, ...loadPref<VirusScan[]>("mockVirus", [])]);
  return scan;
}

// Development-only: replays a saved scan with fake timing so the progress view can be previewed.
async function mockScan(kind: ScanKind, checks: string[], onEvent: (e: ScanEvent) => void): Promise<ScanReport> {
  const [report, cat] = await Promise.all([mock<ScanReport>("scan"), mock<CheckInfo[]>("catalog")]);
  const ids = cat.filter((c) => (kind === "fast" ? c.fast : kind === "full" ? !c.customOnly : checks.includes(c.id))).map((c) => c.id);
  const results = report.results.filter((r) => ids.includes(r.id));
  onEvent({ type: "started", checks: ids });
  await Promise.all(
    ids.map(async (id, i) => {
      await new Promise((r) => setTimeout(r, 80 * i));
      onEvent({ type: "checkStarted", id });
      await new Promise((r) => setTimeout(r, 300 + Math.random() * 1200));
      const r = results.find((x) => x.id === id);
      onEvent({ type: "checkFinished", id, status: r?.status ?? "info", summary: r?.summary ?? "Not in snapshot", elapsedMs: r?.elapsedMs ?? 0 });
    }),
  );
  return { ...report, kind, results, startedAt: new Date().toISOString() };
}

// Development-only: pretends to run fixes so the progress view can be previewed.
async function mockFixes(requests: FixRequest[], skip: boolean, onEvent: (e: FixEvent) => void): Promise<FixRun> {
  const cat = await mock<FixInfo[]>("fixes");
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const steps = [];
  const ids = [...(skip ? [] : ["restore_point"]), ...cat.filter((c) => requests.some((r) => r.id === c.id)).map((c) => c.id)];
  for (const id of ids) {
    const title = id === "restore_point" ? "Create a restore point" : cat.find((c) => c.id === id)!.title;
    onEvent({ type: "stepStarted", id, title });
    for (let p = 0; p <= 100; p += 20) {
      onEvent({ type: "progress", id, percent: p });
      onEvent({ type: "output", id, line: `Working… ${p}%` });
      await wait(150);
    }
    const result = { id, title, status: "ok" as const, summary: "Done (browser preview, nothing was changed).", details: [], outputTail: [], needsRestart: id === "sfc_repair", elapsedMs: 900 };
    onEvent({ type: "stepFinished", result });
    steps.push(result);
  }
  const rp = skip ? null : steps.shift()!;
  const run: FixRun = { id: String(Date.now()), machineId: "preview", computerName: "Browser preview", startedAt: new Date().toISOString(), elapsedMs: 3000, isAdmin: true, restorePoint: rp, restorePointSkipped: skip, steps, needsRestart: steps.some((s) => s.needsRestart), appVersion: "dev" };
  savePref("mockFixRuns", [run, ...loadPref<FixRun[]>("mockFixRuns", [])]);
  return run;
}

/** Read a value from localStorage, falling back safely if storage is unavailable. */
export function loadPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`dxtool:${key}`);
    return raw == null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function savePref<T>(key: string, value: T) {
  try {
    localStorage.setItem(`dxtool:${key}`, JSON.stringify(value));
  } catch {
    /* storage unavailable: preference just won't persist */
  }
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
