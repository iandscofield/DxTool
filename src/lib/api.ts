import { Channel, invoke } from "@tauri-apps/api/core";
import type { AppInfo, AppSettings, CheckInfo, Inventory, MachineInfo, MachineRecord, ScanEvent, ScanKind, ScanReport } from "./types";
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
  relaunchAsAdmin: () => (inTauri ? invoke<void>("relaunch_as_admin") : Promise.reject("Only available inside DxTool")),
};

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
