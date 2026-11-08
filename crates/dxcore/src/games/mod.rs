//! Game check: compares a game's minimum and recommended PC requirements with this PC.
//!
//! - Installed games are found offline (Steam app manifests, Epic Games manifests).
//! - Requirements come from Steam's public store data, only when asked, and are kept in
//!   `DxTool-Data/games/<app id>.json` so a game checked once can be re-checked offline.
//! - Processors and graphics cards are compared with rough performance scores (`perf`).

pub mod perf;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fixes::runner;
use crate::model::{Inventory, Item};

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct InstalledGame {
    /// Steam app id or Epic app name.
    pub id: String,
    pub name: String,
    /// "Steam" or "Epic".
    pub launcher: String,
    pub steam_app_id: Option<u32>,
    pub size_bytes: u64,
    pub folder: String,
    /// Result of the last check, if this game was checked before (from the cache).
    pub last_verdict: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PartCheck {
    /// Processor, Graphics, Memory, Storage, Windows.
    pub part: String,
    pub yours: String,
    pub minimum: String,
    pub recommended: String,
    /// "recommended", "minimum", "below" or "unknown".
    pub verdict: String,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GameCheck {
    pub app_id: u32,
    pub name: String,
    pub image: String,
    pub store_url: String,
    /// "recommended", "minimum", "below" or "unknown".
    pub overall: String,
    pub summary: String,
    pub parts: Vec<PartCheck>,
    pub minimum_text: String,
    pub recommended_text: String,
    /// When the requirements were downloaded from Steam.
    pub fetched_at: String,
    pub installed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchHit {
    pub app_id: u32,
    pub name: String,
    pub image: String,
}

// ───────────────────────── installed games ─────────────────────────

/// Values of `"key"  "value"` lines in Valve's text files (.vdf / .acf).
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let want = format!("\"{key}\"");
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix(&want)?.trim();
            let v = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(v.replace("\\\\", "\\"))
        })
        .collect()
}

fn steam_folder() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let from = |hive, path: &str, val: &str| RegKey::predef(hive).open_subkey(path).ok()?.get_value::<String, _>(val).ok();
    from(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath")
        .or_else(|| from(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"))
        .map(|p| PathBuf::from(p.replace('/', "\\")))
        .filter(|p| p.is_dir())
}

/// Steam tools and runtimes that aren't games.
fn steam_tool(name: &str, id: u32) -> bool {
    let n = name.to_lowercase();
    matches!(id, 228980 | 250820 | 1070560 | 1391110 | 1628350 | 1493710)
        || ["redistributable", "steamvr", "proton", "steam linux runtime", "dedicated server", " sdk", "steamworks"].iter().any(|k| n.contains(k))
}

fn steam_games() -> Vec<InstalledGame> {
    let Some(steam) = steam_folder() else { return Vec::new() };
    let mut libs = vec![steam.join("steamapps")];
    if let Ok(t) = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for p in vdf_values(&t, "path") {
            let lib = PathBuf::from(p).join("steamapps");
            if !libs.iter().any(|l| l.to_string_lossy().eq_ignore_ascii_case(&lib.to_string_lossy())) {
                libs.push(lib);
            }
        }
    }
    let mut out = Vec::new();
    for lib in libs {
        let Ok(rd) = fs::read_dir(&lib) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(t) = fs::read_to_string(e.path()) else { continue };
            let first = |k: &str| vdf_values(&t, k).into_iter().next().unwrap_or_default();
            let Ok(id) = first("appid").parse::<u32>() else { continue };
            let title = first("name");
            if title.is_empty() || steam_tool(&title, id) {
                continue;
            }
            out.push(InstalledGame {
                id: id.to_string(),
                name: title,
                launcher: "Steam".into(),
                steam_app_id: Some(id),
                size_bytes: first("SizeOnDisk").parse().unwrap_or(0),
                folder: lib.join("common").join(first("installdir")).to_string_lossy().into(),
                last_verdict: None,
            });
        }
    }
    out
}

fn epic_games() -> Vec<InstalledGame> {
    let base = std::env::var("ProgramData").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\ProgramData"));
    let Ok(rd) = fs::read_dir(base.join(r"Epic\EpicGamesLauncher\Data\Manifests")) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        if e.path().extension().and_then(|x| x.to_str()) != Some("item") {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&fs::read(e.path()).unwrap_or_default()) else { continue };
        let cats: Vec<&str> = v.get("AppCategories").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        // Only full games: skip engine plug-ins, add-ons and DLC.
        if !cats.contains(&"games") || cats.iter().any(|c| c.contains("addons") || c.contains("plugins")) {
            continue;
        }
        if v.get("MainGameAppName").and_then(Value::as_str).is_some_and(|m| !m.is_empty() && Some(m) != v.get("AppName").and_then(Value::as_str)) {
            continue;
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        out.push(InstalledGame {
            id: s("AppName"),
            name: s("DisplayName"),
            launcher: "Epic".into(),
            steam_app_id: None,
            size_bytes: v.get("InstallSize").and_then(Value::as_u64).unwrap_or(0),
            folder: s("InstallLocation"),
            last_verdict: None,
        });
    }
    out
}

/// Games installed through Steam and the Epic Games Launcher (offline).
pub fn installed() -> Vec<InstalledGame> {
    let mut v = steam_games();
    v.extend(epic_games());
    let names = name_map();
    for g in &mut v {
        if g.steam_app_id.is_none() {
            g.steam_app_id = names.get(&simple_name(&g.name)).copied();
        }
        if let Some(id) = g.steam_app_id {
            g.last_verdict = load_check(id).map(|c| c.overall);
        }
    }
    v.sort_by_key(|g| g.name.to_lowercase());
    v
}

// ───────────────────────── Steam store data ─────────────────────────

fn cache_dir() -> PathBuf {
    crate::history::data_dir().join("games")
}

fn system32(exe: &str) -> String {
    format!("{}\\System32\\{exe}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
}

fn get_json(url: &str) -> Result<Value, String> {
    let r = runner::run_cancellable(&system32("curl.exe"), &["-sS", "-L", "--fail", "--max-time", "20", "-H", "User-Agent: DxTool", url], None, Duration::from_secs(30), None, &|_| {});
    match r {
        Ok((0, lines)) => serde_json::from_str(&lines.join("\n")).map_err(|_| "Steam sent an unexpected reply.".to_string()),
        Ok((code, _)) => Err(match code {
            6 | 7 | 28 | 35 => "Couldn't reach Steam. Check the internet connection and try again.".to_string(),
            22 => "Steam refused the request (too many lookups in a short time). Wait a few minutes and try again.".to_string(),
            _ => format!("The lookup failed (curl code {code})."),
        }),
        Err(e) => Err(e),
    }
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Searches the Steam store by name (online).
pub fn search(term: &str) -> Result<Vec<SearchHit>, String> {
    let v = get_json(&format!("https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US", url_encode(term.trim())))?;
    Ok(v.get("items")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|i| {
                    Some(SearchHit {
                        app_id: i.get("id")?.as_u64()? as u32,
                        name: i.get("name")?.as_str()?.to_string(),
                        image: i.get("tiny_image").and_then(Value::as_str).unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// "The Witcher® 3: Wild Hunt" → "the witcher 3 wild hunt" (for matching Epic names to Steam).
fn simple_name(s: &str) -> String {
    s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Epic game name → Steam app id, remembered from earlier lookups.
fn name_map() -> std::collections::HashMap<String, u32> {
    fs::read(cache_dir().join("names.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn remember_name(name: &str, id: u32) {
    let mut m = name_map();
    m.insert(simple_name(name), id);
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(cache_dir().join("names.json"), serde_json::to_vec_pretty(&m).unwrap_or_default());
}

/// Finds the Steam app for a game name (online), e.g. for Epic games.
pub fn find_app(name: &str) -> Result<Option<SearchHit>, String> {
    let want = simple_name(name);
    let hits = search(name)?;
    let hit = hits
        .iter()
        .find(|h| simple_name(&h.name) == want)
        .or_else(|| hits.iter().filter(|h| simple_name(&h.name).starts_with(&want) || want.starts_with(&simple_name(&h.name))).min_by_key(|h| h.name.len()))
        .cloned();
    if let Some(h) = &hit {
        remember_name(name, h.app_id);
    }
    Ok(hit)
}

#[derive(Serialize, Deserialize, Default)]
struct Cached {
    name: String,
    image: String,
    minimum: String,
    recommended: String,
    fetched_at: String,
}

fn cached(app_id: u32) -> Option<Cached> {
    serde_json::from_slice(&fs::read(cache_dir().join(format!("{app_id}.json"))).ok()?).ok()
}

fn load_check(app_id: u32) -> Option<GameCheck> {
    serde_json::from_slice(&fs::read(cache_dir().join(format!("{app_id}.check.json"))).ok()?).ok()
}

fn fetch(app_id: u32) -> Result<Cached, String> {
    let v = get_json(&format!("https://store.steampowered.com/api/appdetails?appids={app_id}&cc=us&l=english&filters=basic"))?;
    let entry = v.get(app_id.to_string()).ok_or("Steam sent an unexpected reply.")?;
    if entry.get("success").and_then(Value::as_bool) != Some(true) {
        return Err("Steam has no store page for this game (it may have been removed, or be region-locked).".into());
    }
    let d = &entry["data"];
    let req = &d["pc_requirements"];
    let c = Cached {
        name: d.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        image: d.get("header_image").and_then(Value::as_str).unwrap_or("").to_string(),
        minimum: req.get("minimum").and_then(Value::as_str).unwrap_or("").to_string(),
        recommended: req.get("recommended").and_then(Value::as_str).unwrap_or("").to_string(),
        fetched_at: chrono::Local::now().to_rfc3339(),
    };
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(cache_dir().join(format!("{app_id}.json")), serde_json::to_vec_pretty(&c).unwrap_or_default());
    Ok(c)
}

// ───────────────────────── reading requirements ─────────────────────────

/// Steam's requirement HTML → (field, value) pairs, e.g. ("Processor", "Intel Core i5-4460").
pub fn parse_requirements(html: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for li in html.split("<li>").skip(1) {
        let li = li.split("</li>").next().unwrap_or(li);
        let Some(start) = li.find("<strong>") else { continue };
        let rest = &li[start + 8..];
        let Some(end) = rest.find("</strong>") else { continue };
        let key = strip_tags(&rest[..end]).trim().trim_end_matches(':').trim().to_string();
        let value = strip_tags(&rest[end + 9..]).trim().to_string();
        if !key.is_empty() && !value.is_empty() {
            out.push((key, value));
        }
    }
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                out.push(' ');
            }
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Plain text version of a requirements block, one "Field: value" per line.
fn plain(html: &str) -> String {
    parse_requirements(html).iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n")
}

fn get<'a>(pairs: &'a [(String, String)], keys: &[&str]) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| keys.iter().any(|w| k.eq_ignore_ascii_case(w))).map(|(_, v)| v.as_str())
}

/// "8 GB RAM" → bytes; "512 MB" → bytes.
fn size_of(text: &str) -> Option<f64> {
    let l = text.to_lowercase().replace(',', ".");
    let (unit, mult) = if let Some(i) = l.find("gb") { (i, GB) } else if let Some(i) = l.find("mb") { (i, GB / 1024.0) } else if let Some(i) = l.find("tb") { (i, GB * 1024.0) } else { return None };
    let head = l[..unit].trim_end();
    let num: String = head.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
    num.parse::<f64>().ok().map(|n| n * mult)
}

// ───────────────────────── this PC ─────────────────────────

struct Pc {
    cpu: String,
    cpu_score: Option<f64>,
    gpu: String,
    gpu_score: Option<f64>,
    ram: f64,
    free_space: f64,
    windows: String,
}

fn field<'a>(it: &'a Item, key: &str) -> Option<&'a crate::model::Field> {
    it.summary.iter().chain(it.sections.iter().filter(|s| !s.raw).flat_map(|s| s.fields.iter())).find(|f| f.key == key)
}

fn pc_from(inv: &Inventory) -> Pc {
    let items = |c: &str| inv.categories.iter().find(|x| x.id == c).map(|x| x.items.clone()).unwrap_or_default();
    let cpu = items("cpu").first().and_then(|i| field(i, "Model").map(|f| f.display.clone())).unwrap_or_default();
    let cpu_score = perf::cpu_scores(&cpu).first().map(|x| x.1);
    // The strongest graphics adapter that's in the table.
    let mut gpu = String::new();
    let mut gpu_score = None;
    for g in items("graphics") {
        let model = field(&g, "Model").map(|f| f.display.clone()).unwrap_or(g.name.clone());
        if let Some((_, s)) = perf::gpu_scores(&model).into_iter().max_by(|a, b| a.1.total_cmp(&b.1)) {
            if gpu_score.is_none_or(|b| s > b) {
                gpu_score = Some(s);
                gpu = model.replace("(R)", "").replace("(TM)", "");
            }
        } else if gpu.is_empty() && !model.to_lowercase().contains("virtual") {
            gpu = model;
        }
    }
    let ram = items("memory").iter().filter(|i| i.id.starts_with("dimm")).filter_map(|m| field(m, "Capacity").and_then(|f| f.sort.as_f64())).sum();
    let free_space = items("volumes")
        .iter()
        .filter(|v| field(v, "Type").is_some_and(|f| f.display == "Fixed"))
        .filter_map(|v| field(v, "Free").and_then(|f| f.sort.as_f64()))
        .fold(0.0, f64::max);
    let windows = items("system").iter().find(|i| i.id == "windows").and_then(|w| field(w, "Edition").map(|f| f.display.clone())).unwrap_or_default();
    Pc { cpu: crate::upgrades::clean_cpu(&cpu), cpu_score, gpu, gpu_score, ram, free_space, windows }
}

fn gb(b: f64) -> String {
    let g = b / GB;
    if g >= 10.0 || (g - g.round()).abs() < 0.05 { format!("{g:.0} GB") } else { format!("{g:.1} GB") }
}

/// Lowest score among the alternatives a requirement lists ("i5-4460 or FX-6300": either is enough).
fn need(found: &[(String, f64)]) -> Option<f64> {
    found.iter().map(|x| x.1).reduce(f64::min)
}

/// Compares a score with the minimum and recommended scores. Scores are rough, so being
/// within 10% of the recommended or 15% of the minimum counts as meeting it.
fn verdict(yours: Option<f64>, min: Option<f64>, rec: Option<f64>) -> &'static str {
    let Some(y) = yours else { return "unknown" };
    // Only a minimum given: well above it counts as running well.
    if let (Some(m), None) = (min, rec) {
        return if y >= m * 1.5 { "recommended" } else if y >= m * 0.85 { "minimum" } else { "below" };
    }
    if let Some(r) = rec {
        if y >= r * 0.9 {
            return "recommended";
        }
    }
    match min {
        Some(m) if y >= m * 0.85 => "minimum",
        Some(_) => "below",
        None if rec.is_some() => "minimum",
        None => "unknown",
    }
}

fn compare(pc: &Pc, c: &Cached, installed: bool) -> Vec<PartCheck> {
    let min = parse_requirements(&c.minimum);
    let rec = parse_requirements(&c.recommended);
    let mut parts = Vec::new();

    // Processor
    let (min_t, rec_t) = (get(&min, &["Processor", "CPU"]).unwrap_or(""), get(&rec, &["Processor", "CPU"]).unwrap_or(""));
    if !min_t.is_empty() || !rec_t.is_empty() {
        let score = |t: &str| need(&perf::cpu_scores(t)).or_else(|| perf::vague_cpu_score(t));
        let (m, r) = (score(min_t), score(rec_t));
        let mut v = verdict(pc.cpu_score, m, r);
        let mut note = String::new();
        if (m.is_none() && r.is_none()) && pc.cpu_score.is_some_and(|s| s >= 15.0) {
            v = "recommended";
            note = "The requirement doesn't name a specific processor, but this one is modern and well above typical game requirements.".into();
        } else if pc.cpu_score.is_none() {
            note = "DxTool doesn't know this processor's performance, so compare it yourself.".into();
        } else if m.is_none() && r.is_none() {
            note = "The requirement doesn't name a processor DxTool knows.".into();
        }
        parts.push(PartCheck { part: "Processor".into(), yours: pc.cpu.clone(), minimum: min_t.into(), recommended: rec_t.into(), verdict: v.into(), note });
    }

    // Graphics
    let (min_t, rec_t) = (get(&min, &["Graphics", "Video Card", "Video", "GPU"]).unwrap_or(""), get(&rec, &["Graphics", "Video Card", "Video", "GPU"]).unwrap_or(""));
    if !min_t.is_empty() || !rec_t.is_empty() {
        let score = |t: &str| need(&perf::gpu_scores(t)).or_else(|| perf::vague_gpu_score(t));
        let (m, r) = (score(min_t), score(rec_t));
        let mut v = verdict(pc.gpu_score, m, r);
        let mut note = String::new();
        if m.is_none() && r.is_none() && pc.gpu_score.is_some_and(|s| s >= 15.0) {
            v = "recommended";
            note = "The requirement doesn't name a specific graphics card, but this one is well above typical game requirements.".into();
        } else if pc.gpu_score.is_none() {
            note = "DxTool doesn't know this graphics chip's performance, so compare it yourself.".into();
        } else if m.is_none() && r.is_none() {
            note = "The requirement doesn't name a graphics card DxTool knows.".into();
        }
        parts.push(PartCheck { part: "Graphics".into(), yours: pc.gpu.clone(), minimum: min_t.into(), recommended: rec_t.into(), verdict: v.into(), note });
    }

    // Memory (exact)
    let (min_t, rec_t) = (get(&min, &["Memory", "RAM"]).unwrap_or(""), get(&rec, &["Memory", "RAM"]).unwrap_or(""));
    if !min_t.is_empty() || !rec_t.is_empty() {
        let v = if pc.ram > 0.0 { verdict(Some(pc.ram / GB), size_of(min_t).map(|b| b / GB / 0.9 * 0.97), size_of(rec_t).map(|b| b / GB / 0.9 * 0.97)) } else { "unknown" };
        parts.push(PartCheck { part: "Memory".into(), yours: gb(pc.ram), minimum: min_t.into(), recommended: rec_t.into(), verdict: v.into(), note: String::new() });
    }

    // Storage: only matters before installing.
    let (min_t, rec_t) = (get(&min, &["Storage", "Hard Drive", "Hard Disk Space", "Disk Space"]).unwrap_or(""), get(&rec, &["Storage", "Hard Drive", "Hard Disk Space", "Disk Space"]).unwrap_or(""));
    if !min_t.is_empty() || !rec_t.is_empty() {
        let need_b = size_of(min_t).or(size_of(rec_t));
        let (v, note) = if installed {
            ("recommended", "Already installed.".to_string())
        } else {
            match need_b {
                Some(n) if pc.free_space >= n => ("recommended", format!("{} free on the emptiest drive.", gb(pc.free_space))),
                Some(_) => ("below", format!("Only {} free on the emptiest drive.", gb(pc.free_space))),
                None => ("unknown", String::new()),
            }
        };
        let yours = if installed { "Installed".to_string() } else { format!("{} free", gb(pc.free_space)) };
        parts.push(PartCheck { part: "Storage".into(), yours, minimum: min_t.into(), recommended: rec_t.into(), verdict: v.into(), note });
    }

    // Windows version
    let (min_t, rec_t) = (get(&min, &["OS", "OS *", "Operating System"]).unwrap_or(""), get(&rec, &["OS", "OS *", "Operating System"]).unwrap_or(""));
    if !min_t.is_empty() {
        let wants11 = min_t.contains("11") && !min_t.contains("10");
        let on10 = pc.windows.contains("Windows 10");
        let v = if wants11 && on10 { "below" } else if pc.windows.is_empty() { "unknown" } else { "recommended" };
        parts.push(PartCheck { part: "Windows".into(), yours: pc.windows.replace("Microsoft ", ""), minimum: min_t.into(), recommended: rec_t.into(), verdict: v.into(), note: String::new() });
    }
    parts
}

fn overall(parts: &[PartCheck]) -> (String, String) {
    let known: Vec<&PartCheck> = parts.iter().filter(|p| p.verdict != "unknown").collect();
    let below: Vec<&str> = parts.iter().filter(|p| p.verdict == "below").map(|p| p.part.as_str()).collect();
    let unknown: Vec<&str> = parts.iter().filter(|p| p.verdict == "unknown" && matches!(p.part.as_str(), "Processor" | "Graphics")).map(|p| p.part.as_str()).collect();
    let list = |v: &[&str]| {
        let l: Vec<String> = v.iter().map(|s| s.to_lowercase()).collect();
        match l.len() {
            0 => String::new(),
            1 => l[0].clone(),
            n => format!("{} and {}", l[..n - 1].join(", "), l[n - 1]),
        }
    };
    if parts.is_empty() {
        return ("unknown".into(), "Steam doesn't list PC requirements for this game.".into());
    }
    if !below.is_empty() {
        return ("below".into(), format!("Below the minimum for the {}. The game may not start, or run very poorly.", list(&below)));
    }
    let extra = if unknown.is_empty() { String::new() } else { format!(" DxTool couldn't compare the {}, so check that yourself.", list(&unknown)) };
    if known.iter().all(|p| p.verdict == "recommended") && !known.is_empty() {
        return ("recommended".into(), format!("Meets the recommended requirements: expect it to run well, usually on high settings.{extra}"));
    }
    if known.is_empty() {
        return ("unknown".into(), "DxTool couldn't compare this game's requirements with this PC.".into());
    }
    let short: Vec<&str> = known.iter().filter(|p| p.verdict == "minimum").map(|p| p.part.as_str()).collect();
    ("minimum".into(), format!("Meets the minimum but not the recommended requirements for the {}: expect to use low or medium settings.{extra}", list(&short)))
}

/// Checks a Steam game against this PC. Uses the saved requirements unless `refresh` is set
/// or the game was never looked up (then Steam is asked, which needs the internet).
pub fn check(app_id: u32, refresh: bool) -> Result<GameCheck, String> {
    let c = match (refresh, cached(app_id)) {
        (false, Some(c)) => c,
        _ => fetch(app_id)?,
    };
    let result = check_with(app_id, c, &crate::upgrades::inventory())?;
    // Remembered so the game list can show each game's last result.
    let _ = fs::write(cache_dir().join(format!("{app_id}.check.json")), serde_json::to_vec_pretty(&result).unwrap_or_default());
    Ok(result)
}

/// Compares saved/downloaded requirements with a given inventory.
fn check_with(app_id: u32, c: Cached, inv: &Inventory) -> Result<GameCheck, String> {
    let pc = pc_from(inv);
    let installed = installed_ids().contains(&app_id);
    let parts = compare(&pc, &c, installed);
    let (overall, summary) = overall(&parts);
    let result = GameCheck {
        app_id,
        name: c.name.clone(),
        image: c.image.clone(),
        store_url: format!("https://store.steampowered.com/app/{app_id}/"),
        overall,
        summary,
        parts,
        minimum_text: plain(&c.minimum),
        recommended_text: plain(&c.recommended),
        fetched_at: c.fetched_at.clone(),
        installed,
    };
    Ok(result)
}

fn installed_ids() -> Vec<u32> {
    let names = name_map();
    let mut ids: Vec<u32> = steam_games().iter().filter_map(|g| g.steam_app_id).collect();
    ids.extend(epic_games().iter().filter_map(|g| names.get(&simple_name(&g.name)).copied()));
    ids
}

/// Checks a game against a saved inventory (used by the test tool for other PCs).
pub fn check_against(app_id: u32, inv: &Inventory) -> Result<GameCheck, String> {
    let c = match cached(app_id) {
        Some(c) => c,
        None => fetch(app_id)?,
    };
    check_with(app_id, c, inv)
}

/// Checks a game by name: an installed game's Steam id, or the best Steam search match.
pub fn check_by_name(name: &str) -> Result<GameCheck, String> {
    if let Ok(id) = name.trim().parse::<u32>() {
        return check(id, false);
    }
    let want = simple_name(name);
    if let Some(id) = installed().iter().find(|g| simple_name(&g.name) == want).and_then(|g| g.steam_app_id) {
        return check(id, false);
    }
    match find_app(name)? {
        Some(h) => check(h.app_id, false),
        None => {
            let hits = search(name)?;
            match hits.first() {
                Some(h) => check(h.app_id, false),
                None => Err(format!("Couldn't find \"{name}\" on Steam. DxTool reads game requirements from Steam's store, so games sold only elsewhere (such as Fortnite) can't be checked.")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GTA_MIN: &str = r#"<strong>Minimum:</strong><br><ul class="bb_ul"><li>Requires a 64-bit processor and operating system<br></li><li><strong>OS:</strong> Windows 10 64 Bit<br></li><li><strong>Processor:</strong> Intel Core 2 Quad CPU Q6600 @ 2.40GHz (4 CPUs) / AMD Phenom 9850 Quad-Core Processor (4 CPUs) @ 2.5GHz<br></li><li><strong>Memory:</strong> 4 GB RAM<br></li><li><strong>Graphics:</strong> NVIDIA 9800 GT 1GB / AMD HD 4870 1GB (DX 10, 10.1, 11)<br></li><li><strong>Storage:</strong> 125 GB available space<br></li></ul>"#;
    const GTA_REC: &str = r#"<strong>Recommended:</strong><br><ul class="bb_ul"><li><strong>OS:</strong> Windows 10 64 Bit<br></li><li><strong>Processor:</strong> Intel Core i5 3470 @ 3.2GHz (4 CPUs) / AMD X8 FX-8350 @ 4GHz (8 CPUs)<br></li><li><strong>Memory:</strong> 8 GB RAM<br></li><li><strong>Graphics:</strong> NVIDIA GTX 660 2GB / AMD HD 7870 2GB<br></li><li><strong>Storage:</strong> 125 GB available space<br></li></ul>"#;

    fn cached_gta() -> Cached {
        Cached { name: "GTA V".into(), minimum: GTA_MIN.into(), recommended: GTA_REC.into(), ..Default::default() }
    }

    #[test]
    fn parses_steam_html() {
        let p = parse_requirements(GTA_MIN);
        assert_eq!(get(&p, &["Memory"]), Some("4 GB RAM"));
        assert!(get(&p, &["Processor"]).unwrap().starts_with("Intel Core 2 Quad"));
        assert_eq!(size_of("125 GB available space"), Some(125.0 * GB));
        assert_eq!(size_of("512 MB RAM"), Some(0.5 * GB));
    }

    #[test]
    fn verdicts() {
        let strong = Pc { cpu: "i7-13700F".into(), cpu_score: Some(44.0), gpu: "RTX 4070 Ti".into(), gpu_score: Some(31.5), ram: 64.0 * GB, free_space: 900.0 * GB, windows: "Windows 11 Pro".into() };
        let parts = compare(&strong, &cached_gta(), true);
        assert!(parts.iter().all(|p| p.verdict == "recommended"), "{parts:?}");
        assert_eq!(overall(&parts).0, "recommended");

        let weak = Pc { cpu: "i5-2400".into(), cpu_score: Some(3.8), gpu: "HD 4000".into(), gpu_score: Some(0.5), ram: 4.0 * GB, free_space: 50.0 * GB, windows: "Windows 10 Home".into() };
        let parts = compare(&weak, &cached_gta(), false);
        let v = |n: &str| parts.iter().find(|p| p.part == n).unwrap().verdict.clone();
        assert_eq!(v("Processor"), "minimum");
        assert_eq!(v("Graphics"), "below");
        assert_eq!(v("Memory"), "minimum");
        assert_eq!(v("Storage"), "below");
        assert_eq!(overall(&parts).0, "below");
    }
}
