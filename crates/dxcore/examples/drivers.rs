//! Newest-drivers check on this PC (online): `cargo run -p dxcore --example drivers`.
fn main() {
    let c = dxcore::latest_drivers::check();
    println!("{}", serde_json::to_string_pretty(&c).unwrap());
}
