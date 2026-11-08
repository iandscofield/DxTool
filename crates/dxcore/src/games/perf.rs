//! Rough performance scores for processors and graphics cards, used to compare a game's
//! requirements with this PC. Scores are approximate multi-thread CPU and 3D benchmark
//! figures in thousands (they follow the shape of popular benchmark charts); only the
//! comparison between two parts matters, not the number itself.

/// Lower-case words with punctuation turned into spaces; brand words removed.
pub fn norm(s: &str) -> Vec<String> {
    let lower = s.to_lowercase().replace("(r)", " ").replace("(tm)", " ").replace("®", " ").replace("™", " ");
    let cleaned: String = lower.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { ' ' }).collect();
    const DROP: &[&str] = &["nvidia", "geforce", "amd", "ati", "radeon", "intel", "graphics", "card", "video", "or", "better", "with", "series", "gpu", "dedicated", "equivalent"];
    let mut out = Vec::new();
    for w in cleaned.split_whitespace().map(|w| w.trim_matches('.')) {
        if w.is_empty() || DROP.contains(&w) {
            continue;
        }
        let letters: String = w.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let num = &w[letters.len()..];
        if ["rtx", "gtx", "rx", "gt", "hd", "uhd"].contains(&letters.as_str()) && num.len() >= 3 && num.chars().all(|c| c.is_ascii_digit()) {
            out.push(letters);
            out.push(num.to_string());
            continue;
        }
        if letters == "r" && num.len() == 4 && num.starts_with(['5', '6', '7', '9']) && num.chars().all(|c| c.is_ascii_digit()) {
            out.push("rx".into());
            out.push(num.to_string());
            continue;
        }
        let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
        let tail = &w[digits.len()..];
        if digits.len() >= 3 && ["ti", "xt", "xtx", "super"].contains(&tail) {
            out.push(digits);
            out.push(tail.to_string());
        } else {
            out.push(w.to_string());
        }
    }
    out
}

// ───────────────────────── graphics ─────────────────────────

/// (pattern words, score). Matched as whole words; the longest pattern wins.
const GPUS: &[(&str, f64)] = &[
    // NVIDIA, oldest first
    ("8800 gt", 0.6), ("8800 gts", 0.6), ("9600 gt", 0.45), ("9800 gt", 0.65), ("9800 gtx", 0.75), ("gts 250", 0.8), ("gtx 260", 0.95), ("gtx 275", 1.1), ("gtx 285", 1.2),
    ("gt 430", 0.45), ("gt 440", 0.55), ("gts 450", 1.0), ("gtx 460", 1.9), ("gtx 470", 2.3), ("gtx 480", 2.7), ("gtx 550 ti", 1.5), ("gtx 560", 2.3), ("gtx 560 ti", 2.6), ("gtx 570", 3.1), ("gtx 580", 3.6),
    ("gt 630", 0.6), ("gt 640", 0.9), ("gt 710", 0.6), ("gt 730", 0.9), ("gt 1030", 2.6),
    ("gtx 650", 1.8), ("gtx 650 ti", 2.6), ("gtx 660", 3.8), ("gtx 660 ti", 4.3), ("gtx 670", 5.3), ("gtx 680", 5.8), ("gtx 690", 6.5),
    ("gtx 750", 3.2), ("gtx 750 ti", 3.9), ("gtx 760", 4.9), ("gtx 770", 6.0), ("gtx 780", 7.8), ("gtx 780 ti", 8.8), ("gtx titan", 8.0),
    ("gtx 950", 5.2), ("gtx 960", 6.0), ("gtx 970", 9.6), ("gtx 980", 11.0), ("gtx 980 ti", 13.5), ("gtx titan x", 13.5),
    ("gtx 1050", 4.9), ("gtx 1050 ti", 6.3), ("gtx 1060", 9.8), ("gtx 1070", 13.5), ("gtx 1070 ti", 14.6), ("gtx 1080", 15.4), ("gtx 1080 ti", 18.5),
    ("gtx 1630", 4.6), ("gtx 1650", 7.8), ("gtx 1650 super", 10.0), ("gtx 1660", 11.7), ("gtx 1660 super", 12.7), ("gtx 1660 ti", 12.9),
    ("rtx 2060", 14.0), ("rtx 2060 super", 16.3), ("rtx 2070", 16.2), ("rtx 2070 super", 18.0), ("rtx 2080", 19.0), ("rtx 2080 super", 19.6), ("rtx 2080 ti", 21.8),
    ("rtx 3050", 13.0), ("rtx 3060", 17.0), ("rtx 3060 ti", 20.0), ("rtx 3070", 22.4), ("rtx 3070 ti", 23.5), ("rtx 3080", 24.6), ("rtx 3080 ti", 26.3), ("rtx 3090", 26.5), ("rtx 3090 ti", 29.0),
    ("rtx 4060", 19.6), ("rtx 4060 ti", 22.5), ("rtx 4070", 26.9), ("rtx 4070 super", 30.0), ("rtx 4070 ti", 31.5), ("rtx 4070 ti super", 32.3), ("rtx 4080", 34.6), ("rtx 4080 super", 35.0), ("rtx 4090", 38.5),
    ("rtx 5050", 18.0), ("rtx 5060", 23.0), ("rtx 5060 ti", 25.0), ("rtx 5070", 30.0), ("rtx 5070 ti", 33.5), ("rtx 5080", 37.0), ("rtx 5090", 41.0),
    // AMD
    ("hd 4850", 0.75), ("hd 4870", 0.85), ("hd 5670", 0.7), ("hd 5750", 1.0), ("hd 5770", 1.1), ("hd 5850", 1.9), ("hd 5870", 2.2), ("hd 6670", 0.8), ("hd 6750", 1.1), ("hd 6770", 1.2), ("hd 6850", 2.1), ("hd 6870", 2.5), ("hd 6950", 3.0), ("hd 6970", 3.3),
    ("hd 7750", 2.0), ("hd 7770", 2.6), ("hd 7790", 3.2), ("hd 7850", 4.3), ("hd 7870", 5.0), ("hd 7950", 5.6), ("hd 7970", 6.5),
    ("r7 250", 1.6), ("r7 260x", 3.3), ("r7 265", 4.0), ("r7 370", 4.1), ("r9 270", 4.6), ("r9 270x", 5.4), ("r9 280", 6.0), ("r9 280x", 6.8), ("r9 285", 6.6), ("r9 290", 8.7), ("r9 290x", 9.0), ("r9 380", 6.5), ("r9 380x", 7.0), ("r9 390", 9.5), ("r9 390x", 10.0), ("r9 fury", 11.5), ("r9 fury x", 12.5),
    ("rx 460", 3.9), ("rx 470", 7.8), ("rx 480", 8.6), ("rx 550", 2.8), ("rx 560", 3.8), ("rx 570", 8.0), ("rx 580", 8.8), ("rx 590", 9.5), ("vega 56", 12.6), ("vega 64", 13.5), ("radeon vii", 16.5),
    ("rx 5500", 9.0), ("rx 5500 xt", 9.3), ("rx 5600 xt", 12.8), ("rx 5700", 13.8), ("rx 5700 xt", 15.6),
    ("rx 6400", 6.5), ("rx 6500 xt", 7.4), ("rx 6600", 14.2), ("rx 6600 xt", 15.4), ("rx 6650 xt", 16.0), ("rx 6700", 17.0), ("rx 6700 xt", 18.3), ("rx 6750 xt", 19.5), ("rx 6800", 21.6), ("rx 6800 xt", 23.0), ("rx 6900 xt", 24.5), ("rx 6950 xt", 25.5),
    ("rx 7600", 16.5), ("rx 7600 xt", 17.0), ("rx 7700 xt", 22.0), ("rx 7800 xt", 24.0), ("rx 7900 gre", 25.0), ("rx 7900 xt", 28.5), ("rx 7900 xtx", 30.5),
    ("rx 9060 xt", 20.0), ("rx 9070", 28.0), ("rx 9070 xt", 31.0),
    ("vega 3", 1.0), ("vega 6", 1.5), ("vega 7", 1.8), ("vega 8", 1.9), ("vega 11", 2.4), ("680m", 6.0), ("780m", 7.0), ("890m", 8.5),
    // Intel
    ("hd 2000", 0.25), ("hd 3000", 0.35), ("hd 4000", 0.5), ("hd 4400", 0.6), ("hd 4600", 0.7), ("hd 5500", 0.8), ("hd 520", 0.9), ("hd 530", 1.0), ("hd 620", 1.0), ("hd 630", 1.1),
    ("uhd 600", 0.4), ("uhd 605", 0.5), ("uhd 610", 0.8), ("uhd 620", 1.0), ("uhd 630", 1.3), ("uhd 730", 1.3), ("uhd 750", 1.5), ("uhd 770", 1.6), ("uhd", 1.0),
    ("iris plus", 1.6), ("iris xe", 2.7), ("arc a310", 4.5), ("arc a380", 5.8), ("arc a580", 13.0), ("arc a750", 14.0), ("arc a770", 15.0), ("arc b570", 15.5), ("arc b580", 17.0), ("arc 130v", 5.0), ("arc 140v", 6.0),
];

/// Scores of every graphics card named in `text` (several when a requirement says "X or Y").
pub fn gpu_scores(text: &str) -> Vec<(String, f64)> {
    let words = norm(text);
    let mut used = vec![false; words.len()];
    let mut patterns: Vec<(Vec<&str>, f64, &str)> = GPUS.iter().map(|(p, s)| (p.split(' ').collect(), *s, *p)).collect();
    for (p, s) in GPUS {
        let w: Vec<&str> = p.split(' ').collect();
        if (w[0] == "gtx" || w[0] == "rtx") && w.get(1).is_some_and(|n| n.len() == 4) {
            patterns.push((w[1..].to_vec(), *s, p));
        }
    }
    patterns.sort_by_key(|(p, _, _)| std::cmp::Reverse(p.len()));
    let mut found = Vec::new();
    let mobile = words.iter().any(|w| w == "laptop" || w == "mobile" || w == "max" || w == "notebook");
    for (pat, score, label) in &patterns {
        let n = pat.len();
        if n > words.len() {
            continue;
        }
        let mut i = 0;
        while i + n <= words.len() {
            if (0..n).all(|k| !used[i + k] && words[i + k] == pat[k]) {
                (i..i + n).for_each(|k| used[k] = true);
                // A bare "uhd" only counts when nothing more specific matched.
                if *label == "uhd" && !found.is_empty() {
                    i += n;
                    continue;
                }
                let s = if mobile && !label.starts_with("uhd") && !label.starts_with("iris") && !label.starts_with("hd ") { score * 0.8 } else { *score };
                found.push((label.to_uppercase().replace("RX ", "RX ").replace("VEGA", "Vega"), s));
                i += n;
            } else {
                i += 1;
            }
        }
    }
    found
}

// ───────────────────────── processors ─────────────────────────

/// Desktop scores by Intel Core generation 1–14, per tier (i3, i5, i7, i9). 0 = no such chip.
const INTEL: [(u32, [f64; 14]); 4] = [
    (3, [1.5, 1.9, 2.3, 3.2, 3.5, 4.1, 4.4, 6.1, 6.4, 8.8, 11.0, 14.2, 15.0, 15.5]),
    (5, [2.6, 4.0, 4.6, 5.0, 5.4, 6.0, 6.6, 9.2, 9.6, 12.3, 17.0, 19.5, 25.5, 26.0]),
    (7, [3.0, 5.3, 6.4, 7.2, 7.8, 8.9, 9.6, 13.8, 14.5, 19.5, 25.0, 34.5, 46.0, 53.0]),
    (9, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 15.0, 18.5, 23.5, 25.0, 41.0, 59.0, 60.0]),
];

/// Generation from an Intel Core model number: 750 → 1, 2500 → 2, 8700 → 8, 1165 → 11,
/// 1235 → 12, 13700 → 13.
pub fn intel_gen_from_model(digits: &str) -> Option<u32> {
    match digits.len() {
        3 => Some(1),
        4 if digits.starts_with('1') => digits[..2].parse().ok().filter(|g| (10..=14).contains(g)),
        4 => digits[..1].parse().ok(),
        5 => digits[..2].parse().ok(),
        _ => None,
    }
}

fn intel_core(tier: u32, model: &str) -> Option<f64> {
    let digits: String = model.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix = model[digits.len()..].to_lowercase();
    let gen = intel_gen_from_model(&digits)?;
    let row = INTEL.iter().find(|(t, _)| *t == tier)?.1;
    let mut base = *row.get(gen.clamp(1, 14) as usize - 1)?;
    if base == 0.0 {
        base = row.iter().copied().find(|v| *v > 0.0)?;
    }
    // Unlocked i5s from 12th gen on have many more cores than the plain ones.
    if tier == 5 && gen >= 12 && suffix.starts_with('k') {
        base *= 1.45;
    }
    // Mobile chips: U/Y and the 10th/11th gen "G" parts are low-power; H parts are close to desktop.
    let factor = if suffix.starts_with("hx") {
        0.95
    } else if suffix.starts_with('h') {
        0.75
    } else if suffix.starts_with('u') || suffix.starts_with('y') || suffix.starts_with('g') || suffix.starts_with('m') {
        // Low-power laptop chips: dual-core until 7th gen, quad-core from 8th; from 12th gen
        // the i7/i9 laptop parts have far fewer big cores than the desktop ones.
        if gen <= 7 {
            0.5
        } else if gen <= 11 {
            0.6
        } else if tier >= 7 {
            0.42
        } else {
            0.65
        }
    } else if suffix.starts_with('t') {
        0.8
    } else {
        1.0
    };
    Some(base * factor)
}

fn ryzen(tier: u32, model: &str) -> Option<f64> {
    let digits: String = model.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix = model[digits.len()..].to_lowercase();
    let series: u32 = digits.chars().next()?.to_digit(10)?;
    if digits.len() != 4 {
        return None;
    }
    let base = match (tier, series) {
        (3, 1) => 6.4, (3, 2) => 7.0, (3, 3) => 11.5, (3, 4) => 11.0, (3, 5) => 15.0, (3, 7) => 17.0, (3, _) => 15.0,
        (5, 1) => 12.4, (5, 2) => 13.2, (5, 3) => 17.8, (5, 4) => 16.0, (5, 5) => 21.9, (5, 7) => 28.7, (5, 8) => 25.0, (5, 9) => 30.0, (5, _) => 22.0,
        (7, 1) => 14.6, (7, 2) => 17.5, (7, 3) => 22.7, (7, 4) => 20.0, (7, 5) => 28.0, (7, 7) => 36.0, (7, 8) => 31.0, (7, 9) => 37.0, (7, _) => 28.0,
        (9, s) => {
            let sixteen = digits.ends_with("50");
            match s {
                3 => if sixteen { 39.0 } else { 32.0 },
                5 => if sixteen { 46.0 } else { 39.0 },
                7 => if sixteen { 63.0 } else { 52.0 },
                _ => if sixteen { 66.0 } else { 54.0 },
            }
        }
        _ => return None,
    };
    let factor = if suffix.starts_with('u') { 0.55 } else if suffix.starts_with('h') { 0.8 } else if suffix.starts_with('g') && series < 7 { 0.9 } else { 1.0 };
    Some(base * factor)
}

/// "4th gen" / "3rd gen" within the next few words → 4 / 3.
fn ordinal_gen(w: &[String], from: usize) -> Option<u32> {
    w.iter().skip(from).take(4).find_map(|t| {
        let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        let rest = &t[d.len()..];
        (!d.is_empty() && ["st", "nd", "rd", "th"].contains(&rest)).then(|| d.parse().ok()).flatten()
    }).filter(|g| (1..=14).contains(g))
}

/// A rough graphics score when a requirement only gives video memory ("2 GB dedicated
/// memory", "DX11 GPU (4 GB)") or says integrated graphics is enough.
pub fn vague_gpu_score(text: &str) -> Option<f64> {
    let l = text.to_lowercase();
    if l.contains("integrated") || l.contains("onboard") {
        return Some(0.8);
    }
    let unit = l.find("gb").map(|i| (i, 1.0)).or_else(|| l.find("mb").map(|i| (i, 1.0 / 1024.0)))?;
    let head = l[..unit.0].trim_end();
    let num: String = head.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
    let gb = num.parse::<f64>().ok()? * unit.1;
    (gb > 0.0 && gb <= 24.0).then_some((gb * 2.2).max(0.4))
}

/// Processors named in `text`, each with a score. Several when a requirement says "X / Y".
pub fn cpu_scores(text: &str) -> Vec<(String, f64)> {
    let w = norm(text);
    let mut found: Vec<(String, f64)> = Vec::new();
    let next_num = |i: usize| w.get(i).filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit())).cloned();
    let mut i = 0;
    while i < w.len() {
        let t = w[i].as_str();
        // Intel Core i3/i5/i7/i9 with a model number: "i5 4460", "i7 13700k" (from "i7-13700K").
        if t.len() == 2 && t.starts_with('i') && matches!(&t[1..], "3" | "5" | "7" | "9") {
            let tier: u32 = t[1..].parse().unwrap();
            if let Some(model) = next_num(i + 1) {
                if let Some(s) = intel_core(tier, &model) {
                    found.push((format!("Core i{tier}-{}", model.to_uppercase()), s));
                    i += 2;
                    continue;
                }
            }
        }
        // "Intel Core i5" without a model number, maybe with "(4th gen)": assume 4th gen if not said.
        if t.len() == 2 && t.starts_with('i') && matches!(&t[1..], "3" | "5" | "7" | "9") {
            let tier: u32 = t[1..].parse().unwrap();
            let gen = ordinal_gen(&w, i + 1).unwrap_or(4);
            if let Some(s) = intel_core(tier, &format!("{gen}000")) {
                found.push((format!("Core i{tier} ({gen}th gen or similar)"), s));
                i += 1;
                continue;
            }
        }
        // Core Ultra 5/7/9 and Ryzen AI 5/7/9.
        if (t == "ultra" || t == "ai") && i + 1 < w.len() {
            if let Ok(tier) = w[i + 1].parse::<u32>() {
                let rest = w.get(i + 2).map(String::as_str).unwrap_or("");
                let rest = if rest == "hx" || rest == "h" { w.get(i + 3).map(String::as_str).unwrap_or("") } else { rest };
                let suffix: String = rest.chars().skip_while(|c| c.is_ascii_digit()).collect();
                let s = if t == "ai" {
                    match tier { 5 => 26.0, 7 => 30.0, _ => 35.0 }
                } else if suffix.starts_with("hx") {
                    match tier { 5 => 40.0, 7 => 52.0, _ => 60.0 }
                } else if suffix.starts_with('k') || suffix.is_empty() && rest.len() == 3 {
                    match tier { 5 => 43.0, 7 => 59.0, _ => 67.0 }
                } else if suffix.starts_with('h') {
                    match tier { 5 => 22.0, 7 => 26.0, _ => 30.0 }
                } else {
                    match tier { 5 => 16.0, 7 => 19.0, _ => 21.0 }
                };
                if matches!(tier, 5 | 7 | 9) {
                    found.push((format!("{} {tier} {}", if t == "ai" { "Ryzen AI" } else { "Core Ultra" }, rest.to_uppercase()).trim().to_string(), s));
                    i += 3;
                    continue;
                }
            }
        }
        // "r5 3600" shorthand for Ryzen.
        if t.len() == 2 && t.starts_with('r') && matches!(&t[1..], "3" | "5" | "7" | "9") {
            if let Some(model) = next_num(i + 1).filter(|m| m.len() >= 4) {
                if let Some(s) = ryzen(t[1..].parse().unwrap(), &model) {
                    found.push((format!("Ryzen {} {}", &t[1..], model.to_uppercase()), s));
                    i += 2;
                    continue;
                }
            }
        }
        // Ryzen 3/5/7/9 with a model number.
        if t == "ryzen" && i + 2 < w.len() {
            if let (Ok(tier), Some(model)) = (w[i + 1].parse::<u32>(), next_num(i + 2)) {
                if let Some(s) = ryzen(tier, &model) {
                    found.push((format!("Ryzen {tier} {}", model.to_uppercase()), s));
                    i += 3;
                    continue;
                }
            }
        }
        // "Ryzen 5" without a model number, maybe with "2nd gen": assume the first generation.
        if t == "ryzen" && i + 1 < w.len() && next_num(i + 2).is_none_or(|m| m.len() < 4) {
            if let Ok(tier) = w[i + 1].parse::<u32>() {
                let series = ordinal_gen(&w, i + 2).unwrap_or(1);
                if let Some(s) = ryzen(tier, &format!("{series}600")) {
                    found.push((format!("Ryzen {tier} (series {series}000 or similar)"), s));
                    i += 2;
                    continue;
                }
            }
        }
        // AMD FX-4300 / 6300 / 8350 / 9590.
        if t == "fx" {
            if let Some(model) = next_num(i + 1) {
                let s = match model.chars().next() {
                    Some('4') => 3.0,
                    Some('6') => 4.3,
                    Some('8') => 5.9,
                    Some('9') => 6.5,
                    _ => 0.0,
                };
                if s > 0.0 {
                    found.push((format!("FX-{model}"), s));
                    i += 2;
                    continue;
                }
            }
        }
        // Core 2: Q6600, Q9550, E8400…
        if t.len() == 5 && (t.starts_with('q') || t.starts_with('e')) && t[1..].chars().all(|c| c.is_ascii_digit()) {
            let d = t.as_bytes()[1];
            let s = if t.starts_with('q') {
                match d { b'9' => 2.3, b'8' => 1.9, _ => 1.7 }
            } else {
                match d { b'8' => 1.2, b'7' => 1.0, b'6' => 0.85, _ => 0.8 }
            };
            found.push((format!("Core 2 {}", t.to_uppercase()), s));
            i += 1;
            continue;
        }
        if t == "phenom" {
            let ii = w.get(i + 1).is_some_and(|x| x == "ii");
            let six = w.iter().skip(i).take(4).any(|x| x == "x6" || x.starts_with("1090") || x.starts_with("1100"));
            let s = if !ii { 1.8 } else if six { 4.5 } else { 3.0 };
            found.push((if ii { "Phenom II".into() } else { "Phenom".into() }, s));
            i += 2;
            continue;
        }
        if t == "athlon" {
            let nxt = w.get(i + 1).map(String::as_str).unwrap_or("");
            let s = if nxt == "64" { 0.8 } else if nxt == "ii" { 2.0 } else if nxt.contains("ge") || nxt.contains("3000g") { 3.2 } else { 1.5 };
            found.push(("Athlon".into(), s));
            i += 2;
            continue;
        }
        if t == "pentium" {
            let nxt = w.get(i + 1).map(String::as_str).unwrap_or("");
            let s = if nxt.starts_with('g') && nxt.len() == 5 {
                match nxt.as_bytes()[1] { b'6' | b'7' => 4.6, b'5' => 3.9, b'4' => 3.6, _ => 2.0 }
            } else if nxt == "gold" { 4.0 } else { 1.0 };
            found.push(("Pentium".into(), s));
            i += 2;
            continue;
        }
        i += 1;
    }
    found
}

/// A rough score for vague requirements like "Quad-core 2.5 GHz" or "Dual Core 2.4GHz".
pub fn vague_cpu_score(text: &str) -> Option<f64> {
    let l = text.to_lowercase();
    let cores = if l.contains("octa") || l.contains("8 core") || l.contains("8-core") || l.contains("eight") {
        8.0
    } else if l.contains("hexa") || l.contains("six") || l.contains("6 core") || l.contains("6-core") {
        6.0
    } else if l.contains("quad") || l.contains("4 core") || l.contains("4-core") || l.contains("4 cpus") {
        4.0
    } else if l.contains("dual") || l.contains("2 core") || l.contains("2-core") {
        2.0
    } else {
        return None;
    };
    let ghz = l.find("ghz").and_then(|i| {
        let head = l[..i].trim_end();
        let num: String = head.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
        num.parse::<f64>().ok()
    });
    Some(cores * ghz.unwrap_or(2.5) * 0.25)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(v: &[(String, f64)]) -> f64 {
        v.iter().map(|x| x.1).fold(0.0, f64::max)
    }

    #[test]
    fn gpus() {
        let v = gpu_scores("NVIDIA GTX 660 2GB / AMD HD 7870 2GB");
        assert_eq!(v.len(), 2);
        assert!(best(&gpu_scores("NVIDIA GeForce RTX 4070 Ti")) > 31.0);
        assert!((best(&gpu_scores("GeForce GTX 1070 Ti")) - 14.6).abs() < 0.01);
        assert!((best(&gpu_scores("AMD Radeon R9 290X 4GB")) - 9.0).abs() < 0.01);
        assert!(best(&gpu_scores("Intel(R) UHD Graphics 770")) > 1.5);
        assert!(best(&gpu_scores("Intel(R) Iris(R) Xe Graphics")) > 2.0);
        assert!(best(&gpu_scores("NVIDIA 9800 GT 1GB / AMD HD 4870 1GB (DX 10, 10.1, 11)")) < 1.0);
        assert!(best(&gpu_scores("NVIDIA GeForce RTX 3060 Laptop GPU")) < 17.0);
        assert!(gpu_scores("Meta Virtual Monitor").is_empty());
        assert_eq!(gpu_scores("Nvidia RTX 3060Ti").len(), 1);
        assert_eq!(gpu_scores("Nvidia RTX 3060Ti, AMD Radeon RX 6700-XT").len(), 2);
        assert!(gpu_scores("Nvidia 2060 Super / RX 5700 XT").iter().any(|g| (g.1 - 16.3).abs() < 0.01));
    }

    #[test]
    fn cpus() {
        assert!(best(&cpu_scores("13th Gen Intel(R) Core(TM) i7-13700F")) > 40.0);
        assert!((best(&cpu_scores("Intel Core i5 3470 @ 3.2GHz (4 CPUs) / AMD X8 FX-8350 @ 4GHz (8 CPUs)")) - 5.9).abs() < 0.01);
        assert_eq!(cpu_scores("Intel Core 2 Quad CPU Q6600 @ 2.40GHz (4 CPUs) / AMD Phenom 9850 Quad-Core Processor").len(), 2);
        assert!(best(&cpu_scores("AMD Ryzen 5 3600")) > 17.0);
        assert!(best(&cpu_scores("Intel(R) Core(TM) i5-1135G7 @ 2.40GHz")) < 11.0);
        assert!(best(&cpu_scores("Intel(R) Core(TM) i5-1135G7 @ 2.40GHz")) > 5.0);
        assert!(best(&cpu_scores("Intel(R) Core(TM) Ultra 7 155H")) > 20.0);
        assert!(best(&cpu_scores("AMD Ryzen 9 7950X 16-Core Processor")) > 60.0);
        assert!(best(&cpu_scores("Intel Core i7-8700K or AMD Ryzen 5 3600X")) > 13.0);
        assert_eq!(intel_gen_from_model("1165"), Some(11));
        assert!(best(&cpu_scores("Intel i7 8700K / AMD r5 3600")) > 17.0);
        assert_eq!(intel_gen_from_model("8250"), Some(8));
        assert!(vague_cpu_score("Dual Core 2.4 GHz").unwrap() < 1.5);
        assert!(!cpu_scores("Intel Core i3 (4th gen) / AMD Ryzen 3 of equivalent").is_empty());
        assert!((best(&cpu_scores("i5 3rd gen / RYZEN 5 2nd gen")) - 13.2).abs() < 0.01);
        assert!((best(&cpu_scores("Intel Core i5")) - 5.0).abs() < 0.01);
        assert!((best(&cpu_scores("Intel Core i3 2GHz or equivalent")) - 3.2).abs() < 0.01);
        assert!((vague_gpu_score("2 GB Dedicated Memory").unwrap() - 4.4).abs() < 0.01);
        assert!(vague_gpu_score("256mb Graphics card").unwrap() < 1.0);
        assert_eq!(gpu_scores("GeForce RTX2060 4GB or Radeon R5700").len(), 2);
    }
}
