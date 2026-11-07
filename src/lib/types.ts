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

// ───────── Health scans (crates/dxcore/src/model.rs) ─────────

export type ScanKind = "fast" | "full" | "custom";

export interface CheckInfo {
  id: string;
  title: string;
  group: string;
  description: string;
  fast: boolean;
  customOnly: boolean;
  needsAdmin: boolean;
  duration: string;
}

export interface Finding {
  id: string;
  severity: Status;
  title: string;
  detail: string;
  advice: string | null;
  fix: string | null;
  evidence: Field[];
}

export interface CheckResult {
  id: string;
  title: string;
  group: string;
  status: Status;
  summary: string;
  findings: Finding[];
  facts: Field[];
  items: Item[];
  itemsLabel: string | null;
  limited: string | null;
  error: string | null;
  elapsedMs: number;
}

export interface Counts {
  error: number;
  warn: number;
  info: number;
}

export interface ScanReport {
  id: string;
  kind: ScanKind;
  startedAt: string;
  elapsedMs: number;
  computerName: string;
  isAdmin: boolean;
  score: number;
  grade: string;
  counts: Counts;
  results: CheckResult[];
}

export type ScanEvent =
  | { type: "started"; checks: string[] }
  | { type: "checkStarted"; id: string }
  | { type: "checkFinished"; id: string; status: Status; summary: string; elapsedMs: number };

export interface CheckProgress {
  state: "waiting" | "running" | "done";
  status?: Status;
  summary?: string;
  elapsedMs?: number;
}

export interface ScanState {
  kind: ScanKind;
  order: string[];
  progress: Record<string, CheckProgress>;
  startedAt: number;
}
