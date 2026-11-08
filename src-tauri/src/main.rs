// Hide the extra console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--mcp") {
        return dxtool_lib::run_mcp();
    }
    // Used by the installer and uninstaller: connect / disconnect Claude Desktop without opening a window.
    if has("--setup-claude") || has("--remove-claude") {
        let r = if has("--setup-claude") { dxcore::mcp::claude_setup(false) } else { dxcore::mcp::claude_remove_if_mine() };
        std::process::exit(if r.is_ok() { 0 } else { 1 });
    }
    dxtool_lib::run()
}
