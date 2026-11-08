//! cargo run -p dxcore --example fix -- <fix id> [json options] [--no-restore-point]
//! cargo run -p dxcore --example fix -- check-drivers
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(String::as_str) == Some("check-drivers") {
        match dxcore::fixes::drivers::check(&|l| eprintln!("  | {l}")) {
            Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
            Err(e) => println!("ERROR: {e}"),
        }
        return;
    }
    let req = dxcore::model::FixRequest {
        id: a[0].clone(),
        options: a.get(1).filter(|s| !s.starts_with("--")).map(|s| serde_json::from_str(s).unwrap()).unwrap_or(serde_json::Value::Null),
    };
    let skip = a.iter().any(|x| x == "--no-restore-point");
    let run = dxcore::fixes::run_fixes(&[req], skip, &|e| match e {
        dxcore::model::FixEvent::Output { line, .. } => eprintln!("  | {line}"),
        dxcore::model::FixEvent::Progress { percent, .. } => eprintln!("  | {percent:.0}%"),
        dxcore::model::FixEvent::StepFinished { result } => eprintln!("[{}] {}: {}", result.status, result.title, result.summary),
        _ => {}
    });
    println!("{}", serde_json::to_string_pretty(&run).unwrap());
}
