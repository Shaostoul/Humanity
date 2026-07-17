//! Numeric proof of the terrain tile tier: sample Mount Fuji through the
//! SAME TerrainTiles sampler the patch mesher + ground clamp use, and print
//! a west-east elevation transect over the summit. A cone profile here IS
//! what the mesh renders (drawn == sampled).
//!
//!   cargo run --release --example probe_fuji

use humanity_engine::terrain::planet_heightmap::PlanetHeightmap;
use humanity_engine::terrain::terrain_tiles::TerrainTiles;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut tiles = TerrainTiles::new(root.join("data/planets/earth_tiles"));
    tiles.ensure_region(35.36, 138.73);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !tiles.poll() {
        assert!(std::time::Instant::now() < deadline, "tiles never arrived");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // Give the neighbors a moment too (poll returns on first arrival).
    std::thread::sleep(std::time::Duration::from_millis(500));
    tiles.poll();

    let base = PlanetHeightmap::load(&root.join("data/planets/earth_heightmap.bin"))
        .expect("base grid loads");

    let summit_tile = tiles.sample_meters_smooth(35.3606, 138.7274);
    println!(
        "Fuji summit  base-grid: {:>6.0} m   tile-tier: {:>6.0} m   (real: 3776 m)",
        base.sample_meters_latlon_smooth(35.3606, 138.7274),
        summit_tile.unwrap_or(f32::NAN)
    );
    println!("\nWest-east transect at lat 35.36 (each step 0.02 deg ~ 1.8 km):");
    print!("base: ");
    for i in -8..=8 {
        let lon = 138.7274 + i as f32 * 0.02;
        print!("{:>5.0}", base.sample_meters_latlon_smooth(35.3606, lon));
    }
    println!();
    print!("tile: ");
    for i in -8..=8 {
        let lon = 138.7274 + i as f32 * 0.02;
        print!("{:>5.0}", tiles.sample_meters_smooth(35.3606, lon).unwrap_or(f32::NAN));
    }
    println!();
}
