//! Game check from the command line.
//!   cargo run -p dxcore --example games                 list installed games
//!   cargo run -p dxcore --example games -- check <name or Steam app id>
//!   cargo run -p dxcore --example games -- search <words>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => match dxcore::games::check_by_name(&args[1..].join(" ")) {
            Ok(c) => {
                println!("{} (app {}) → {}: {}", c.name, c.app_id, c.overall, c.summary);
                for p in &c.parts {
                    println!("  [{}] {}: yours {} | min {} | rec {} {}", p.verdict, p.part, p.yours, p.minimum, p.recommended, p.note);
                }
            }
            Err(e) => println!("Error: {e}"),
        },
        Some("check-as") => {
            let inv: dxcore::model::Inventory = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
            for id in &args[2..] {
                match dxcore::games::check_against(id.parse().unwrap(), &inv) {
                    Ok(c) => {
                        println!("{} → {}: {}", c.name, c.overall, c.summary);
                        for p in &c.parts {
                            println!("  [{}] {}: yours {} | min {} | rec {}", p.verdict, p.part, p.yours, p.minimum, p.recommended);
                        }
                    }
                    Err(e) => println!("Error: {e}"),
                }
            }
        }
        // Check many installed games and list requirement texts DxTool couldn't read.
        Some("audit") => {
            let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(40);
            for g in dxcore::games::installed().into_iter().filter(|g| g.steam_app_id.is_some()).take(n) {
                match dxcore::games::check(g.steam_app_id.unwrap(), false) {
                    Ok(c) => {
                        let bad: Vec<String> = c.parts.iter().filter(|p| (p.part == "Processor" || p.part == "Graphics") && !p.note.is_empty()).map(|p| format!("{}: min '{}' rec '{}'", p.part, p.minimum, p.recommended)).collect();
                        println!("{:9} {}{}", c.overall, c.name, if bad.is_empty() { String::new() } else { format!("
      UNREAD {}", bad.join("
      UNREAD ")) });
                    }
                    Err(e) => println!("ERROR    {}: {e}", g.name),
                }
                std::thread::sleep(std::time::Duration::from_millis(900));
            }
        }
        Some("search") => println!("{:?}", dxcore::games::search(&args[1..].join(" "))),
        _ => {
            for g in dxcore::games::installed() {
                println!("{:5} {:40} steam={:?} {:>6.1} GB {:?}", g.launcher, g.name, g.steam_app_id, g.size_bytes as f64 / 1e9, g.last_verdict);
            }
        }
    }
}
