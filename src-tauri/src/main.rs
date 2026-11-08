// Hide the extra console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--mcp") {
        return dxtool_lib::run_mcp();
    }
    dxtool_lib::run()
}
