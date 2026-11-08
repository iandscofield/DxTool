//! Data shapes sent to the user interface.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Overall condition of an item.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Ok,
    Info,
    Warn,
    Error,
}

/// One labelled value. `sort` holds a raw number or string so tables sort correctly
/// ("900 MB" before "16 GB").
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub display: String,
    pub sort: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Section {
    pub title: String,
    /// Raw sections hold every property Windows reports; the UI shows them in Advanced mode.
    pub raw: bool,
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub status: Status,
    pub status_text: Option<String>,
    /// Curated, friendly facts. These become the default table columns.
    pub summary: Vec<Field>,
    pub sections: Vec<Section>,
    /// Plug and Play device IDs used to attach connection and driver details.
    #[serde(skip)]
    pub pnp_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub description: String,
    pub items: Vec<Item>,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Inventory {
    pub collected_at: String,
    pub computer_name: String,
    pub is_admin: bool,
    pub elapsed_ms: u64,
    pub categories: Vec<Category>,
}

/// Small builder for lists of fields that skips anything empty.
#[derive(Default)]
pub struct Fields(pub Vec<Field>);

impl Fields {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn text(&mut self, label: &str, value: Option<impl Into<String>>) -> &mut Self {
        if let Some(v) = value {
            let v: String = v.into();
            let t = v.trim();
            if !t.is_empty() {
                self.0.push(Field {
                    key: label.to_string(),
                    label: label.to_string(),
                    display: t.to_string(),
                    sort: Value::String(t.to_lowercase()),
                });
            }
        }
        self
    }

    pub fn num(&mut self, label: &str, display: impl Into<String>, sort: f64) -> &mut Self {
        let d: String = display.into();
        if !d.is_empty() {
            self.0.push(Field {
                key: label.to_string(),
                label: label.to_string(),
                display: d,
                sort: serde_json::json!(sort),
            });
        }
        self
    }

    pub fn opt_num(&mut self, label: &str, v: Option<f64>, f: impl Fn(f64) -> String) -> &mut Self {
        if let Some(n) = v {
            self.num(label, f(n), n);
        }
        self
    }

    pub fn yes_no(&mut self, label: &str, v: Option<bool>) -> &mut Self {
        if let Some(b) = v {
            self.text(label, Some(if b { "Yes" } else { "No" }));
        }
        self
    }

    pub fn into_vec(self) -> Vec<Field> {
        self.0
    }
}

impl Section {
    pub fn new(title: &str, fields: Vec<Field>) -> Self {
        Self { title: title.into(), raw: false, fields }
    }
}

// ───────────────────────────── Health scans ─────────────────────────────

/// Static description of one health check, shown in the Custom scan picker.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    /// Included in a Fast scan.
    pub fast: bool,
    /// Only run when picked in a Custom scan (very slow checks).
    pub custom_only: bool,
    /// Gives full results only with administrator rights.
    pub needs_admin: bool,
    /// Rough duration hint for the user, e.g. "a few seconds".
    pub duration: &'static str,
}

/// Something a check noticed, explained in plain English.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Finding {
    pub id: String,
    pub severity: Status,
    pub title: String,
    /// What this means, for a non-expert.
    pub detail: String,
    /// What to do about it.
    pub advice: Option<String>,
    /// Id of a repair in the Fixes section that addresses this (Stage 5).
    pub fix: Option<String>,
    /// Supporting facts for a technician.
    pub evidence: Vec<Field>,
}

impl Finding {
    pub fn new(id: impl Into<String>, severity: Status, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { id: id.into(), severity, title: title.into(), detail: detail.into(), advice: None, fix: None, evidence: vec![] }
    }
    pub fn advice(mut self, a: impl Into<String>) -> Self {
        self.advice = Some(a.into());
        self
    }
    pub fn fix(mut self, f: &str) -> Self {
        self.fix = Some(f.into());
        self
    }
    pub fn evidence(mut self, e: Vec<Field>) -> Self {
        self.evidence = e;
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckResult {
    pub id: String,
    pub title: String,
    pub group: String,
    pub status: Status,
    /// One-line outcome, e.g. "3 drives, all healthy".
    pub summary: String,
    pub findings: Vec<Finding>,
    /// Key facts measured by the check.
    pub facts: Vec<Field>,
    /// Detailed rows (drives, crashes, events...) shown as a table.
    pub items: Vec<Item>,
    /// What the rows are, e.g. "Crash events".
    pub items_label: Option<String>,
    /// Set when the check could not run fully (for example it needs administrator rights).
    pub limited: Option<String>,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub error: u32,
    pub warn: u32,
    pub info: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanReport {
    pub id: String,
    /// "fast", "full" or "custom".
    pub kind: String,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub computer_name: String,
    pub is_admin: bool,
    /// 0 to 100.
    pub score: u32,
    /// "Excellent", "Good", "Fair" or "Poor".
    pub grade: String,
    pub counts: Counts,
    pub results: Vec<CheckResult>,
    /// Which PC this scan was run on (scans from many PCs can live in one DxTool folder).
    pub machine: MachineInfo,
    pub app_version: String,
    /// Whether the scan was written to the history folder.
    pub saved: bool,
}

/// Identity and key specs of a PC, stored with every scan so history and reports stand alone.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineInfo {
    /// Stable folder-safe id: computer name plus part of the motherboard UUID.
    pub id: String,
    pub computer_name: String,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub uuid: String,
    pub windows: String,
    pub cpu: String,
    pub memory: String,
    pub graphics: Vec<String>,
    pub drives: Vec<String>,
    pub motherboard: String,
    pub bios: String,
}

/// One line in a machine's scan history.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanSummary {
    pub id: String,
    pub kind: String,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub score: u32,
    pub grade: String,
    pub counts: Counts,
    pub is_admin: bool,
    pub checks: usize,
}

/// A PC that has scans in the history folder.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineRecord {
    pub info: MachineInfo,
    pub first_seen: String,
    pub last_seen: String,
    pub scans: Vec<ScanSummary>,
}

/// Live progress sent to the UI while a scan runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ScanEvent {
    Started { checks: Vec<String> },
    CheckStarted { id: String },
    CheckFinished { id: String, status: Status, summary: String, elapsed_ms: u64 },
}

// ───────────────────────────── Fixes ─────────────────────────────

/// A repair DxTool can run. Shown on the Fixes page.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FixInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    /// Exactly what it changes, shown before the user confirms.
    pub changes: &'static str,
    /// "low", "medium" or "high".
    pub risk: &'static str,
    pub needs_admin: bool,
    pub may_need_restart: bool,
    pub duration: &'static str,
}

/// One fix the user picked, with its options (for example which drive to check).
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FixRequest {
    pub id: String,
    #[serde(default)]
    pub options: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FixStepResult {
    pub id: String,
    pub title: String,
    /// "ok", "warning", "failed" or "skipped".
    pub status: String,
    pub summary: String,
    pub details: Vec<String>,
    /// Last lines of the tool's output, for a technician.
    pub output_tail: Vec<String>,
    pub needs_restart: bool,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FixRun {
    pub id: String,
    pub machine_id: String,
    pub computer_name: String,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub is_admin: bool,
    /// The restore point step (always first unless the user chose to skip it).
    pub restore_point: Option<FixStepResult>,
    pub restore_point_skipped: bool,
    pub steps: Vec<FixStepResult>,
    pub needs_restart: bool,
    pub app_version: String,
}

/// Live progress while fixes run.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum FixEvent {
    StepStarted { id: String, title: String },
    Output { id: String, line: String },
    Progress { id: String, percent: f64 },
    StepFinished { result: FixStepResult },
}

/// A driver update offered by Windows Update, matched to the installed driver.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DriverUpdate {
    pub update_id: String,
    pub title: String,
    pub manufacturer: String,
    pub driver_class: String,
    pub hardware_id: String,
    pub new_version: String,
    pub new_date: String,
    pub size_bytes: u64,
    pub device_name: String,
    pub installed_version: String,
    pub installed_date: String,
    /// Plain-English caution, e.g. when the offered driver is older than the installed one.
    pub warning: String,
}

impl DriverUpdate {
    /// BIOS/UEFI firmware updates carry more risk than ordinary drivers.
    pub fn is_firmware(&self) -> bool {
        self.driver_class.eq_ignore_ascii_case("Firmware")
    }
}

// ───────────────────────── Virus scanning ─────────────────────────

/// Microsoft Defender's state, as far as scanning from DxTool is concerned.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DefenderStatus {
    /// True when DxTool can ask Defender to scan.
    pub available: bool,
    /// Why scanning isn't available, in plain English.
    pub reason: String,
    pub realtime: bool,
    pub running_mode: String,
    pub product_version: String,
    pub engine_version: String,
    pub signature_version: String,
    pub signature_updated: String,
    pub signature_age_days: Option<i64>,
    pub quick_scan_age_days: Option<i64>,
    pub full_scan_age_days: Option<i64>,
    /// Other antivirus products registered with Windows Security.
    pub other_antivirus: Vec<String>,
}

/// The ClamAV copy DxTool downloads into its data folder.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClamStatus {
    pub installed: bool,
    pub version: String,
    /// True once the virus databases have been downloaded.
    pub databases: bool,
    pub db_version: String,
    pub db_date: String,
    pub db_age_days: Option<i64>,
    pub signatures: u64,
    pub folder: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AvStatus {
    pub defender: DefenderStatus,
    pub clamav: ClamStatus,
    pub is_admin: bool,
}

/// One detection from a virus scan.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Threat {
    pub name: String,
    pub path: String,
    /// "defender" or "clamav".
    pub engine: String,
    /// Defender's severity (Low, Moderate, High, Severe) when known.
    pub severity: String,
    /// "active" (still on disk), "quarantined", "removed", "allowed" or "failed".
    pub status: String,
    /// Plain-English description of what happened to it.
    pub action: String,
}

/// A finished (or stopped) virus scan, saved with the machine's history.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VirusScan {
    pub id: String,
    pub machine_id: String,
    pub computer_name: String,
    pub engine: String,
    /// "quick", "full" or "custom".
    pub kind: String,
    pub targets: Vec<String>,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub files_scanned: u64,
    pub threats: Vec<Threat>,
    /// "clean", "threats", "failed" or "cancelled".
    pub outcome: String,
    pub message: String,
    pub signature_version: String,
    pub log: Vec<String>,
    pub app_version: String,
}

/// A file DxTool moved into its quarantine folder.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct QuarantineItem {
    pub id: String,
    pub original_path: String,
    pub threat: String,
    pub engine: String,
    pub quarantined_at: String,
    pub size_bytes: u64,
}

/// Live progress for virus scans, database downloads and installs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AvEvent {
    /// A new phase, e.g. "Loading virus databases".
    Phase { text: String },
    Output { line: String },
    /// `total` is 0 when the amount of work isn't known.
    Progress { done: u64, total: u64, current: String },
    Threat { threat: Threat },
}

/// One upgrade recommendation.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub id: String,
    /// Memory, Storage, Windows, Graphics, Battery, Processor.
    pub area: String,
    /// "high", "medium" or "low".
    pub priority: String,
    /// "Free", "Low cost", "Moderate" or "Expensive".
    pub cost: String,
    pub title: String,
    pub why: String,
    pub how: String,
    /// Shop search phrase for the part, when there is one to buy.
    pub search: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeReport {
    pub generated_at: String,
    pub pc_name: String,
    /// "desktop" or "laptop".
    pub pc_kind: String,
    pub items: Vec<Upgrade>,
    /// Things that are already fine.
    pub good: Vec<String>,
    pub limited: Option<String>,
}
