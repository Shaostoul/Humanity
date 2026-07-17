//! Convert the ETOPO 2022 GeoTIFF tiles into the game's streamable tile bins
//! (dev tool; the output is the DOWNLOADABLE high-detail terrain tier).
//!
//!   cargo run --release --example build_earth_tiles
//!
//! Input:  ext_data/etopo2022_15s/*.tif  (288 tiles, 15x15 deg, 3600x3600 f32)
//! Output: data/planets/earth_tiles/N45W135.bin etc. (gitignored - ~7.5 GB
//!         total; players fetch tiles like the ultra star catalog, the dev
//!         machine generates them locally)
//!
//! Each output tile is a standard HOSHGT1 grid (the planet_heightmap format)
//! with width = height = 3600 and the SAME fixed quantization window as the
//! global base grid (-11000..8900 m), so a tile sample and a base-grid sample
//! are directly comparable in the shared normalized 0..1 domain. The tile's
//! geographic placement is carried by its FILENAME (north-west corner), not
//! the header - `terrain::terrain_tiles` derives placement from the name.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

const TILE_PX: usize = 3600;
const MIN_M: f32 = -11000.0;
const MAX_M: f32 = 8900.0;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = root.join("ext_data/etopo2022_15s");
    let out_dir = root.join("data/planets/earth_tiles");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let mut done = 0usize;
    let mut lat_top = 90i32;
    while lat_top > -90 {
        let mut lon_west = -180i32;
        while lon_west < 180 {
            let lat_s = if lat_top > 0 {
                format!("N{:02}", lat_top)
            } else if lat_top == 0 {
                "N00".to_string()
            } else {
                format!("S{:02}", -lat_top)
            };
            let lon_s = if lon_west < 0 {
                format!("W{:03}", -lon_west)
            } else {
                format!("E{:03}", lon_west)
            };
            let src = src_dir.join(format!("ETOPO_2022_v1_15s_{lat_s}{lon_s}_surface.tif"));
            let dst = out_dir.join(format!("{lat_s}{lon_s}.bin"));
            lon_west += 15;

            if dst.exists() {
                done += 1;
                continue; // resumable
            }
            let file = fs::File::open(&src).expect("open tile (download incomplete?)");
            let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(file))
                .expect("tiff decoder");
            let px = match dec.read_image().expect("read tile") {
                tiff::decoder::DecodingResult::F32(v) => v,
                _ => panic!("tile is not F32: {}", src.display()),
            };
            assert_eq!(px.len(), TILE_PX * TILE_PX, "unexpected tile size");

            let mut out = Vec::with_capacity(23 + TILE_PX * TILE_PX * 2);
            out.extend_from_slice(b"HOSHGT1");
            out.extend_from_slice(&(TILE_PX as u32).to_le_bytes());
            out.extend_from_slice(&(TILE_PX as u32).to_le_bytes());
            out.extend_from_slice(&MIN_M.to_le_bytes());
            out.extend_from_slice(&MAX_M.to_le_bytes());
            for &m in &px {
                let q = humanity_engine::terrain::planet_heightmap::quantize_meters(m, MIN_M, MAX_M);
                out.extend_from_slice(&q.to_le_bytes());
            }
            let mut f = fs::File::create(&dst).expect("create tile bin");
            f.write_all(&out).expect("write tile bin");
            done += 1;
            if done % 24 == 0 {
                println!("converted {done}/288 tiles...");
            }
        }
        lat_top -= 15;
    }
    println!("tile conversion complete: {done}/288 -> {}", out_dir.display());
}
