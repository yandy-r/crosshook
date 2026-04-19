#[cfg(feature = "ts-rs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    crosshook_core::ts_rs_exports::export_ts_types()
}

#[cfg(not(feature = "ts-rs"))]
fn main() {
    eprintln!("Enable the `ts-rs` feature to run this exporter (cargo run -p crosshook-core --features ts-rs --bin ts_rs_export)");
    std::process::exit(1);
}
