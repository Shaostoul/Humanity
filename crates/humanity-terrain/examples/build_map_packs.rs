//! Build whole-Earth map packs from the sources on the data drive (whole-Earth maps rung 2;
//! docs/design/whole-earth-maps.md section 8.2). Developer tooling: `just map-packs [args]`.
//!
//!   cargo run --release -p humanity-terrain --features map-build --example build_map_packs -- \
//!       --tiles home --etopo C:\Humanity\ext_data\etopo2022_15s
//!
//! `--tiles home` (the packs holding data/maps/places.ron's home places, the default),
//! `washington` (every pack wholly inside the fetched GEDTM30 window) or a comma list of pack
//! names (`f07-213301`). Writes data/maps/packs/<name>.hmpack; each pack is built twice and must
//! match byte for byte, every chunk must decode, and every named place inside must pass its gates.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    if let Err(e) = humanity_terrain::map_build::run(&args, root) {
        eprintln!("build_map_packs: {e}");
        std::process::exit(1);
    }
}
