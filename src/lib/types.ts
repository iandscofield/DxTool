// Shapes returned by the Rust engine (crates/dxcore/src/model.rs).

export type Status = "ok" | "info" | "warn" | "error";

export interface Field {
  key: string;
  label: string;
  display: string;
  sort: string | number | boolean | null;
}

export interface Section {
  title: string;
  raw: boolean;
  fields: Field[];
}

export interface Item {
  id: string;
  name: string;
  subtitle: string;
  status: Status;
  statusText: string | null;
  summary: Field[];
  sections: Section[];
}

export interface Category {
  id: string;
  title: string;
  icon: string;
  description: string;
  items: Item[];
  error: string | null;
  elapsedMs: number;
}

export interface Inventory {
  collectedAt: string;
  computerName: string;
  isAdmin: boolean;
  elapsedMs: number;
  categories: Category[];
}

export interface AppInfo {
  version: string;
  isAdmin: boolean;
  computerName: string;
  firmware: string;
  debugBuild: boolean;
}

export type Mode = "simple" | "advanced";
export type ThemePref = "system" | "dark" | "light";
export type Page =
  | "overview"
  | "hardware"
  | "health"
  | "scans"
  | "history"
  | "reports"
  | "fixes"
  | "security"
  | "settings";
