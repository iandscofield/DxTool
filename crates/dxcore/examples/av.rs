//! cargo run -p dxcore --example av -- status
//! cargo run -p dxcore --example av -- scan defender|clamav quick|full|custom [paths...]
//! cargo run -p dxcore --example av -- update defender|clamav
//! cargo run -p dxcore --example av -- install-clamav
use dxcore::model::AvEvent;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ev = |e: AvEvent| match e {
        AvEvent::Phase { text } => println!("== {text}"),
        AvEvent::Output { line } => println!("   {line}"),
        AvEvent::Progress { done, total, current } => println!("   .. {done}/{total} {current}"),
        AvEvent::Threat { threat } => println!("!! {} {} ({})", threat.name, threat.path, threat.action),
    };
    match args.first().map(String::as_str) {
        Some("status") => println!("{}", serde_json::to_string_pretty(&dxcore::av::status()).unwrap()),
        Some("scan") => {
            let r = dxcore::av::scan(&args[1], &args[2], &args[3..], &ev);
            match r {
                Ok(s) => println!("{}", serde_json::to_string_pretty(&s).unwrap()),
                Err(e) => println!("ERR {e}"),
            }
        }
        Some("update") if args[1] == "defender" => println!("{:?}", dxcore::av::defender::update(&ev)),
        Some("update") => println!("{:?}", dxcore::av::clamav::update(&ev)),
        Some("remove-defender") => match dxcore::av::defender::remove_active(&args[1], &args[2]) {
            Ok(s) => println!("{}", serde_json::to_string_pretty(&s.threats).unwrap()),
            Err(e) => println!("ERR {e}"),
        },
        Some("quarantine") => println!("{:?}", dxcore::av::quarantine(&args[1], &args[2], &args[3])),
        Some("qlist") => println!("{}", serde_json::to_string_pretty(&dxcore::av::quarantine_list()).unwrap()),
        Some("restore") => println!("{:?}", dxcore::av::restore(&args[1])),
        Some("qdelete") => println!("{:?}", dxcore::av::delete_quarantined(&args[1])),
        Some("install-clamav") => println!("{:?}", dxcore::av::clamav::install(&ev)),
        _ => eprintln!("see source for usage"),
    }
}
