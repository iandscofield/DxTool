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
export type Accent = "blue" | "purple" | "red" | "green" | "orange";
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
  machine: MachineInfo;
  appVersion: string;
  saved: boolean;
}

export interface MachineInfo {
  id: string;
  computerName: string;
  manufacturer: string;
  model: string;
  serial: string;
  uuid: string;
  windows: string;
  cpu: string;
  memory: string;
  graphics: string[];
  drives: string[];
  motherboard: string;
  bios: string;
}

export interface ScanSummary {
  id: string;
  kind: ScanKind;
  startedAt: string;
  elapsedMs: number;
  score: number;
  grade: string;
  counts: Counts;
  isAdmin: boolean;
  checks: number;
}

export interface MachineRecord {
  info: MachineInfo;
  firstSeen: string;
  lastSeen: string;
  scans: ScanSummary[];
}

export interface Branding {
  /** Logo image as a data: URL. */
  logo: string | null;
  showLogo: boolean;
  company: string;
  showCompany: boolean;
  /** Phone, email or website shown under the company name. */
  contact: string;
  showContact: boolean;
  tech: string;
  showTech: boolean;
}

export interface AppSettings {
  branding: Branding;
}

export const defaultBranding: Branding = {
  logo: null,
  showLogo: true,
  company: "",
  showCompany: true,
  contact: "",
  showContact: true,
  tech: "",
  showTech: true,
};

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

// ───────── Fixes (crates/dxcore/src/model.rs) ─────────

export type Risk = "low" | "medium" | "high";

export interface FixInfo {
  id: string;
  title: string;
  group: string;
  description: string;
  changes: string;
  risk: Risk;
  needsAdmin: boolean;
  mayNeedRestart: boolean;
  duration: string;
}

export interface FixRequest {
  id: string;
  options: Record<string, unknown>;
}

export type StepStatus = "ok" | "warning" | "failed" | "skipped";

export interface FixStepResult {
  id: string;
  title: string;
  status: StepStatus;
  summary: string;
  details: string[];
  outputTail: string[];
  needsRestart: boolean;
  elapsedMs: number;
}

export interface FixRun {
  id: string;
  machineId: string;
  computerName: string;
  startedAt: string;
  elapsedMs: number;
  isAdmin: boolean;
  restorePoint: FixStepResult | null;
  restorePointSkipped: boolean;
  steps: FixStepResult[];
  needsRestart: boolean;
  appVersion: string;
}

export type FixEvent =
  | { type: "stepStarted"; id: string; title: string }
  | { type: "output"; id: string; line: string }
  | { type: "progress"; id: string; percent: number }
  | { type: "stepFinished"; result: FixStepResult };

export interface DriverUpdate {
  updateId: string;
  title: string;
  manufacturer: string;
  driverClass: string;
  hardwareId: string;
  newVersion: string;
  newDate: string;
  sizeBytes: number;
  deviceName: string;
  installedVersion: string;
  installedDate: string;
  warning: string;
}

export interface FixStepProgress {
  id: string;
  title: string;
  state: "waiting" | "running" | "done";
  percent?: number;
  lines: string[];
  result?: FixStepResult;
}

export interface FixState {
  requests: FixRequest[];
  steps: FixStepProgress[];
  startedAt: number;
}
