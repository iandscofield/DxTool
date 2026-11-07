//! Data shapes sent to the user interface.

use serde::Serialize;
use serde_json::Value;

/// Overall condition of an item.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Default, PartialOrd, Ord)]
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
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub key: String,
    pub label: String,
    pub display: String,
    pub sort: Value,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    /// Raw sections hold every property Windows reports; the UI shows them in Advanced mode.
    pub raw: bool,
    pub fields: Vec<Field>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub description: String,
    pub items: Vec<Item>,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Clone, Copy, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub error: u32,
    pub warn: u32,
    pub info: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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
}

/// Live progress sent to the UI while a scan runs.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ScanEvent {
    Started { checks: Vec<String> },
    CheckStarted { id: String },
    CheckFinished { id: String, status: Status, summary: String, elapsed_ms: u64 },
}
