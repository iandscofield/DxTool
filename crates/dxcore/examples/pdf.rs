//! cargo run -p dxcore --example pdf -- <machine id> <html file> [pdf|html]
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let html = std::fs::read_to_string(&a[1]).unwrap();
    let t = std::time::Instant::now();
    match dxcore::report::export(&a[0], "Test report", &html, a.get(2).map(String::as_str).unwrap_or("pdf")) {
        Ok(p) => println!("{} ({} ms)", p.display(), t.elapsed().as_millis()),
        Err(e) => println!("ERROR: {e}"),
    }
}
