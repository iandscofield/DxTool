//! Runs a health scan and prints the findings.
//! cargo run -p dxcore --example scan [fast|full|<check ids...>] [--json]

fn main() {
    if std::env::args().any(|a| a == "--catalog") {
        println!("{}", serde_json::to_string_pretty(&dxcore::health::catalog()).unwrap());
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--json" && a != "--save").collect();
    let json = std::env::args().any(|a| a == "--json");
    let kind = match args.first().map(String::as_str) {
        None | Some("fast") => "fast",
        Some("full") => "full",
        _ => "custom",
    };
    let quiet = json;
    let report = dxcore::health::run_scan(kind, &args, &|e| {
        if !quiet {
            if let dxcore::model::ScanEvent::CheckFinished { id, status, summary, elapsed_ms } = e {
                eprintln!("  {id:<18} {status:?} {elapsed_ms:>6} ms  {summary}");
            }
        }
    });
    let mut report = report;
    if std::env::args().any(|a| a == "--save") {
        match dxcore::history::save(&report) {
            Ok(p) => { report.saved = true; eprintln!("saved to {}", p.display()); }
            Err(e) => eprintln!("save failed: {e}"),
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }
    println!("\nScore {} ({}) · admin={} · {} ms · {:?}", report.score, report.grade, report.is_admin, report.elapsed_ms, report.counts);
    for r in &report.results {
        if let Some(e) = &r.error { println!("  ! {}: {e}", r.id); }
        if let Some(l) = &r.limited { println!("  ~ {}: {l}", r.id); }
        for f in &r.findings {
            println!("  [{:?}] {} — {}", f.severity, f.title, f.detail);
        }
    }
}
