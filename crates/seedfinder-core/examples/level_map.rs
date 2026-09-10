//! Prints the shared map JSON document; see `docs/level-map-format.md`.

#[cfg(feature = "json-query")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("expected a level-map JSON request")?;
    let request = shpd_seedfinder_core::level_map::json::decode_request(&input)?;
    println!("{}", request.generate_document()?);
    Ok(())
}

#[cfg(not(feature = "json-query"))]
fn main() {
    eprintln!("enable --features json-query to run this example");
    std::process::exit(1);
}
