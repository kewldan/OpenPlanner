//! Пишет синтетический скан квартиры в GLB — фикстура для e2e.
//!
//! `cargo run -p op-scan --example synthetic_scan -- apps/desktop/e2e/fixtures/apartment.glb`

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("apartment.glb"), PathBuf::from);
    // Поворот 0,2 рад: проверяет выравнивание плана по осям.
    let glb = op_scan::io::write_glb(&op_scan::synth::mesh(&op_scan::synth::apartment(0.2)));
    std::fs::write(&path, glb)
}
