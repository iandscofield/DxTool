//! Try the updater from the command line.
//!   cargo run -p dxcore --example update -- check
//!   cargo run -p dxcore --example update -- replace <program.exe> <new.exe>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => match dxcore::update::check() {
            Ok(i) => println!("{}", serde_json::to_string_pretty(&i).unwrap()),
            Err(e) => println!("Error: {e}"),
        },
        Some("replace") if args.len() == 3 => {
            println!("{:?}", dxcore::update::replace_exe(std::path::Path::new(&args[1]), std::path::Path::new(&args[2])));
        }
        _ => println!("usage: update check | update replace <program.exe> <new.exe>"),
    }
}
