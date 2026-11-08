//! cargo run -p dxcore --example claude -- status|setup|remove
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("setup") => println!("{:?}", dxcore::mcp::claude_setup(false)),
        Some("remove") => println!("{:?}", dxcore::mcp::claude_setup(true)),
        _ => println!("{}", serde_json::to_string_pretty(&dxcore::mcp::claude_status()).unwrap()),
    }
}
