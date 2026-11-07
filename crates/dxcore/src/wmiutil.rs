//! Thin helpers over WMI that turn every result into plain JSON maps.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use wmi::{Variant, WMIConnection};

use crate::fmt;
use crate::model::{Field, Section};

pub type Row = BTreeMap<String, Value>;

pub fn connect(namespace: &str) -> Result<WMIConnection, String> {
    WMIConnection::with_namespace_path(namespace).map_err(|e| format!("WMI {namespace}: {e}"))
}

pub fn query(con: &WMIConnection, wql: &str) -> Result<Vec<Row>, String> {
    let rows: Vec<HashMap<String, Variant>> =
        con.raw_query(wql).map_err(|e| format!("{wql}: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|m| m.into_iter().map(|(k, v)| (k, to_json(&v))).collect())
        .collect())
}

/// Runs a query, returning an empty list when the class does not exist on this PC.
pub fn query_opt(con: &WMIConnection, wql: &str) -> Vec<Row> {
    query(con, wql).unwrap_or_default()
}

/// Shortcut: connect to a namespace and query it, ignoring failures.
pub fn ns_query(namespace: &str, wql: &str) -> Vec<Row> {
    match connect(namespace) {
        Ok(c) => query_opt(&c, wql),
        Err(_) => Vec::new(),
    }
}

fn to_json(v: &Variant) -> Value {
    match v {
        Variant::Empty | Variant::Null => Value::Null,
        Variant::String(s) => Value::String(s.clone()),
        Variant::I1(n) => (*n).into(),
        Variant::I2(n) => (*n).into(),
        Variant::I4(n) => (*n).into(),
        Variant::I8(n) => (*n).into(),
        Variant::UI1(n) => (*n).into(),
        Variant::UI2(n) => (*n).into(),
        Variant::UI4(n) => (*n).into(),
        Variant::UI8(n) => (*n).into(),
        Variant::R4(n) => serde_json::json!(*n),
        Variant::R8(n) => serde_json::json!(*n),
        Variant::Bool(b) => Value::Bool(*b),
        Variant::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        Variant::Unknown(_) | Variant::Object(_) => Value::String("(object)".into()),
    }
}

pub trait RowExt {
    fn s(&self, k: &str) -> Option<String>;
    fn u(&self, k: &str) -> Option<u64>;
    fn f(&self, k: &str) -> Option<f64>;
    fn b(&self, k: &str) -> Option<bool>;
    fn list(&self, k: &str) -> Vec<String>;
    fn nums(&self, k: &str) -> Vec<u64>;
}

impl RowExt for Row {
    fn s(&self, k: &str) -> Option<String> {
        match self.get(k)? {
            Value::String(s) => {
                let t = s.trim().trim_matches('\0').trim();
                if t.is_empty() { None } else { Some(t.to_string()) }
            }
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(fmt::yes_no(*b).to_string()),
            _ => None,
        }
    }
    fn u(&self, k: &str) -> Option<u64> {
        match self.get(k)? {
            Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
    fn f(&self, k: &str) -> Option<f64> {
        match self.get(k)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
    fn b(&self, k: &str) -> Option<bool> {
        match self.get(k)? {
            Value::Bool(b) => Some(*b),
            Value::Number(n) => n.as_u64().map(|v| v != 0),
            _ => None,
        }
    }
    fn list(&self, k: &str) -> Vec<String> {
        match self.get(k) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .collect(),
            Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
            _ => vec![],
        }
    }
    fn nums(&self, k: &str) -> Vec<u64> {
        match self.get(k) {
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_u64()).collect(),
            _ => vec![],
        }
    }
}

/// Decode WMI fields stored as arrays of UTF-16 code units (monitor names, serials).
pub fn u16_string(row: &Row, k: &str) -> Option<String> {
    let n = row.nums(k);
    let s: String = n
        .into_iter()
        .take_while(|c| *c != 0)
        .filter_map(|c| char::from_u32(c as u32))
        .collect();
    let t = s.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn display_value(v: &Value) -> Option<(String, Value)> {
    match v {
        Value::Null => None,
        Value::String(s) => {
            let t = s.trim().trim_matches('\0').trim();
            if t.is_empty() {
                return None;
            }
            if let Some(d) = fmt::wmi_datetime(t) {
                return Some((d, Value::String(t.to_string())));
            }
            Some((t.to_string(), Value::String(t.to_lowercase())))
        }
        Value::Bool(b) => Some((fmt::yes_no(*b).to_string(), Value::Bool(*b))),
        Value::Number(n) => Some((n.to_string(), Value::Number(n.clone()))),
        Value::Array(a) => {
            if a.is_empty() {
                return None;
            }
            let parts: Vec<String> = a.iter().filter_map(|x| display_value(x).map(|p| p.0)).collect();
            if parts.is_empty() {
                return None;
            }
            let joined = parts.join(", ");
            Some((joined.clone(), Value::String(joined.to_lowercase())))
        }
        Value::Object(_) => None,
    }
}

/// Every non-empty property of a WMI object, alphabetically. Shown in Advanced mode.
pub fn raw_section(title: &str, row: &Row) -> Section {
    let skip = ["CreationClassName", "SystemCreationClassName", "SystemName", "__PATH"];
    let fields = row
        .iter()
        .filter(|(k, _)| !k.starts_with("__") && !skip.contains(&k.as_str()))
        .filter_map(|(k, v)| {
            display_value(v).map(|(display, sort)| Field {
                key: format!("{title}.{k}"),
                label: k.clone(),
                display,
                sort,
            })
        })
        .collect();
    Section { title: title.to_string(), raw: true, fields }
}
