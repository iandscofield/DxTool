//! Prints a short inventory summary. Run with: cargo run -p dxcore --example inventory [--json]

fn main() {
    let inv = dxcore::inventory::collect();
    if std::env::args().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&inv).unwrap());
        return;
    }
    println!("{} · admin={} · {} ms", inv.computer_name, inv.is_admin, inv.elapsed_ms);
    for c in &inv.categories {
        println!(
            "\n== {} ({} items, {} ms){}",
            c.title,
            c.items.len(),
            c.elapsed_ms,
            c.error.as_deref().map(|e| format!(" ERROR: {e}")).unwrap_or_default()
        );
        for it in c.items.iter().take(4) {
            let facts: Vec<String> = it.summary.iter().take(6).map(|f| format!("{}={}", f.label, f.display)).collect();
            println!("  [{:?}] {} — {}", it.status, it.name, facts.join(" | "));
        }
    }
}
