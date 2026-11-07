import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, Inventory } from "./types";

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
  relaunchAsAdmin: () => (inTauri ? invoke<void>("relaunch_as_admin") : Promise.reject("Only available inside DxTool")),
};

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
