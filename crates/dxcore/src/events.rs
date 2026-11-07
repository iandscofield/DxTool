//! Reads the Windows event logs through the Windows Event Log API (wevtapi).
//!
//! Each event is rendered as XML and the handful of fields we need are picked out of
//! it. The human-readable message is formatted with the publisher's own message table,
//! exactly like Event Viewer shows it.

use std::collections::HashMap;

use chrono::{DateTime, Local, Utc};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::EventLog::{
    EvtClose, EvtFormatMessage, EvtFormatMessageEvent, EvtNext, EvtOpenPublisherMetadata, EvtQuery,
    EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender, EvtRenderEventXml, EVT_HANDLE,
};

#[derive(Clone, Debug, Default)]
pub struct Event {
    pub channel: String,
    pub provider: String,
    pub id: u32,
    pub level: u8,
    pub record_id: u64,
    pub time: Option<DateTime<Utc>>,
    pub message: String,
    pub data: Vec<(String, String)>,
}

impl Event {
    pub fn data(&self, name: &str) -> Option<&str> {
        self.data.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn local_time(&self) -> String {
        self.time
            .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default()
    }

    pub fn timestamp(&self) -> i64 {
        self.time.map(|t| t.timestamp()).unwrap_or(0)
    }

    pub fn level_text(&self) -> &'static str {
        match self.level {
            1 => "Critical",
            2 => "Error",
            3 => "Warning",
            4 | 0 => "Information",
            _ => "Verbose",
        }
    }

    /// First line of the message, trimmed to a readable length.
    pub fn short_message(&self) -> String {
        let line = self.message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        if line.chars().count() > 220 {
            format!("{}…", line.chars().take(220).collect::<String>())
        } else {
            line.to_string()
        }
    }
}

struct Handle(EVT_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if self.0 .0 != 0 {
            unsafe {
                let _ = EvtClose(self.0);
            }
        }
    }
}

/// XPath fragment for "within the last N days".
pub fn within_days(days: u32) -> String {
    format!("TimeCreated[timediff(@SystemTime) <= {}]", days as u64 * 86_400_000)
}

/// XPath fragment matching any of the given event IDs.
pub fn ids(list: &[u32]) -> String {
    let parts: Vec<String> = list.iter().map(|i| format!("EventID={i}")).collect();
    format!("({})", parts.join(" or "))
}

/// XPath fragment matching any of the given provider names.
pub fn providers(list: &[&str]) -> String {
    let parts: Vec<String> = list.iter().map(|p| format!("@Name='{p}'")).collect();
    format!("Provider[{}]", parts.join(" or "))
}

/// Builds `*[System[a and b and c]]`.
pub fn system_filter(parts: &[String]) -> String {
    format!("*[System[{}]]", parts.join(" and "))
}

/// Runs an XPath query against a channel (for example "System"), newest first.
pub fn query(channel: &str, xpath: &str, max: usize) -> Result<Vec<Event>, String> {
    let q = unsafe {
        EvtQuery(
            None,
            &HSTRING::from(channel),
            &HSTRING::from(xpath),
            EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
        )
    }
    .map_err(|e| format!("Could not read the {channel} log: {}", e.message()))?;
    let q = Handle(q);

    let mut meta: HashMap<String, Option<Handle>> = HashMap::new();
    let mut out = Vec::new();
    let mut buf = [0isize; 32];
    'outer: while out.len() < max {
        let mut returned = 0u32;
        if unsafe { EvtNext(q.0, &mut buf, 2000, 0, &mut returned) }.is_err() || returned == 0 {
            break;
        }
        let handles: Vec<Handle> = buf[..returned as usize].iter().map(|h| Handle(EVT_HANDLE(*h))).collect();
        for h in &handles {
            if out.len() >= max {
                break 'outer;
            }
            let Some(xml) = render_xml(h.0) else { continue };
            let mut ev = parse_xml(&xml);
            if ev.channel.is_empty() {
                ev.channel = channel.to_string();
            }
            let pm = meta.entry(ev.provider.clone()).or_insert_with(|| open_publisher(&ev.provider));
            if let Some(pm) = pm {
                ev.message = format_message(pm.0, h.0).unwrap_or_default();
            }
            if ev.message.is_empty() {
                ev.message = ev.data.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("; ");
            }
            out.push(ev);
        }
    }
    Ok(out)
}

fn open_publisher(name: &str) -> Option<Handle> {
    if name.is_empty() {
        return None;
    }
    unsafe { EvtOpenPublisherMetadata(None, &HSTRING::from(name), PCWSTR::null(), 0, 0) }
        .ok()
        .map(Handle)
}

fn render_xml(h: EVT_HANDLE) -> Option<String> {
    let mut used = 0u32;
    let mut count = 0u32;
    let _ = unsafe { EvtRender(None, h, EvtRenderEventXml.0, 0, None, &mut used, &mut count) };
    if used == 0 {
        return None;
    }
    let mut buf = vec![0u16; used as usize / 2 + 1];
    unsafe {
        EvtRender(None, h, EvtRenderEventXml.0, (buf.len() * 2) as u32, Some(buf.as_mut_ptr() as *mut _), &mut used, &mut count)
    }
    .ok()?;
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

fn format_message(meta: EVT_HANDLE, ev: EVT_HANDLE) -> Option<String> {
    let mut used = 0u32;
    let _ = unsafe { EvtFormatMessage(Some(meta), Some(ev), 0, None, EvtFormatMessageEvent.0, None, &mut used) };
    if used == 0 {
        return None;
    }
    let mut buf = vec![0u16; used as usize + 1];
    // Some messages report "unresolved insert" errors but still fill the buffer.
    let _ = unsafe { EvtFormatMessage(Some(meta), Some(ev), 0, None, EvtFormatMessageEvent.0, Some(&mut buf), &mut used) };
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]).trim().to_string();
    (!s.is_empty()).then_some(s)
}

// ───────── Minimal XML picking (event XML has a fixed, simple shape) ─────────

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Value of `attr` on the first `<tag ...>` element.
fn attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag} "))?;
    let end = start + xml[start..].find('>')?;
    let el = &xml[start..end];
    let key = format!("{attr}=");
    let p = el.find(&key)? + key.len();
    let quote = el[p..].chars().next()?;
    let rest = &el[p + 1..];
    let q = rest.find(quote)?;
    Some(unescape(&rest[..q]))
}

/// Text inside the first `<tag>...</tag>` (the tag may carry attributes).
fn text(xml: &str, tag: &str) -> Option<String> {
    let open = xml.find(&format!("<{tag}>")).or_else(|| xml.find(&format!("<{tag} ")))?;
    let gt = open + xml[open..].find('>')?;
    if xml[..gt].ends_with('/') {
        return None;
    }
    let close = gt + xml[gt..].find(&format!("</{tag}>"))?;
    Some(unescape(&xml[gt + 1..close]))
}

fn parse_xml(xml: &str) -> Event {
    let mut ev = Event {
        provider: attr(xml, "Provider", "Name").unwrap_or_default(),
        id: text(xml, "EventID").and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        level: text(xml, "Level").and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        record_id: text(xml, "EventRecordID").and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        channel: text(xml, "Channel").unwrap_or_default(),
        time: attr(xml, "TimeCreated", "SystemTime")
            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&Utc)),
        ..Default::default()
    };
    // <Data Name='X'>value</Data> or unnamed <Data>value</Data>
    let mut rest = xml;
    let mut n = 0;
    while let Some(p) = rest.find("<Data") {
        rest = &rest[p + 5..];
        let Some(gt) = rest.find('>') else { break };
        let head = &rest[..gt];
        if head.ends_with('/') {
            rest = &rest[gt + 1..];
            continue;
        }
        let name = head
            .find("Name=")
            .and_then(|i| {
                let q = head[i + 5..].chars().next()?;
                let s = &head[i + 6..];
                s.find(q).map(|e| s[..e].to_string())
            })
            .unwrap_or_else(|| {
                n += 1;
                format!("Data{n}")
            });
        let body = &rest[gt + 1..];
        let Some(end) = body.find("</Data>") else { break };
        ev.data.push((name, unescape(&body[..end])));
        rest = &body[end..];
    }
    ev
}
