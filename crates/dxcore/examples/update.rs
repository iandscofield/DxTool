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
        // Act as an older portable copy at <program.exe> and update it from GitHub (without starting it).
        Some("install-as") if args.len() == 3 => {
            let r = dxcore::update::install_as(std::path::Path::new(&args[1]), &args[2], "portable", false, &|d, t| eprint!("\r{d} / {t} bytes   "));
            eprintln!();
            println!("{r:?}");
        }
        Some("replace") if args.len() == 3 => {
            println!("{:?}", dxcore::update::replace_exe(std::path::Path::new(&args[1]), std::path::Path::new(&args[2])));
        }
        _ => println!("usage: update check | update replace <program.exe> <new.exe>"),
    }
}
