//! Upgrade recommendations.
//!   cargo run -p dxcore --example upgrades                 (this PC)
//!   cargo run -p dxcore --example upgrades -- inventory.json (a saved inventory)
fn main() {
    let report = match std::env::args().nth(1) {
        Some(path) => {
            let inv: dxcore::model::Inventory = serde_json::from_slice(&std::fs::read(&path).expect("read file")).expect("inventory JSON");
            dxcore::upgrades::recommend(&inv)
        }
        None => dxcore::upgrades::for_this_pc(),
    };
    println!("{} ({})", report.pc_name, report.pc_kind);
    for u in &report.items {
        println!("\n[{}] [{}] [{}] {}\n  WHY: {}\n  HOW: {}", u.priority, u.area, u.cost, u.title, u.why, u.how);
        if let Some(s) = &u.search {
            println!("  SEARCH: {s}");
        }
    }
    println!("\nGOOD:");
    for g in &report.good {
        println!("  - {g}");
    }
    if let Some(l) = &report.limited {
        println!("\nLIMITED: {l}");
    }
    println!("
PARTS:");
    for p in &report.parts {
        println!("  - {}: {}  [search: {}] {}", p.part, p.name, p.search, p.note);
    }
}
