//! Android phone tools from the command line:
//!   cargo run -p dxcore --example phone -- install              (download Google's platform-tools)
//!   cargo run -p dxcore --example phone -- status
//!   cargo run -p dxcore --example phone -- info <serial>
//!   cargo run -p dxcore --example phone -- backup <serial> photos|messages <folder>
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("install") => println!("{:?}", dxcore::phone::install_tools(&|d, t| eprint!("\r{d} / {t}   "))),
        Some("status") => println!("{}", serde_json::to_string_pretty(&dxcore::phone::status()).unwrap()),
        Some("info") if a.len() == 2 => match dxcore::phone::info(&a[1]) {
            Ok(i) => println!("{}", serde_json::to_string_pretty(&i).unwrap()),
            Err(e) => println!("Error: {e}"),
        },
        Some("backup") if a.len() == 4 => println!("{:?}", dxcore::phone::backup(&a[1], &a[2], &a[3], &|e| eprintln!("{} {}/{} {}", e.phase, e.done, e.total, e.current))),
        Some("stop") => dxcore::phone::stop(),
        _ => eprintln!("usage: install | status | info <serial> | backup <serial> photos|messages <folder> | stop"),
    }
}
