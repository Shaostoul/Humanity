//! Build data/planets/earth_heightmap.bin from the ETOPO 2022 15-arc-second
//! tiles (dev tool; run on the dev machine, ship only the output).
//!
//!   cargo run --release --example build_earth_grid            # full build
//!   cargo run --release --example build_earth_grid -- --probe # decode 1 tile, print stats
//!
//! Reads the 288 GeoTIFF tiles fetched by scripts/download-etopo2022.js
//! (ext_data/etopo2022_15s/, 15x15 deg each, 3600x3600 float32 metres),
//! box-averages them into a 7200x3600 (0.05 deg, ~5.5 km cells) global grid
//! (12x12 source samples per output cell), and writes the HOSHGT1 format
//! `src/terrain/planet_heightmap.rs` loads. Replaces the old ETOPO1-derived
//! 3600x1800 grid: 4x the cells AND the true peak window is restored
//! (Everest stops being clamped at the old averaged max of 6394.5 m).
//!
//! Output quantization window: -11000..8900 m (Challenger Deep ~-10935,
//! Everest 8849). Vertical quantum ~0.30 m - far below any cell's real
//! variance at this resolution.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

const TILE_DEG: i32 = 15;
const TILE_PX: usize = 3600; // 15 deg / 15 arc-sec
const OUT_W: usize = 7200; // 0.05 deg cells
const OUT_H: usize = 3600;
const MIN_M: f32 = -11000.0;
const MAX_M: f32 = 8900.0;

fn tile_name(lat_top: i32, lon_west: i32) -> String {
    let lat = if lat_top > 0 {
        format!("N{:02}", lat_top)
    } else if lat_top == 0 {
        "N00".to_string()
    } else {
        format!("S{:02}", -lat_top)
    };
    let lon = if lon_west < 0 {
        format!("W{:03}", -lon_west)
    } else {
        format!("E{:03}", lon_west)
    };
    format!("ETOPO_2022_v1_15s_{lat}{lon}_surface.tif")
}

fn decode_tile(path: &PathBuf) -> Result<Vec<f32>, String> {
    let file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(file))
        .map_err(|e| format!("tiff decoder: {e}"))?;
    let (w, h) = dec.dimensions().map_err(|e| format!("dimensions: {e}"))?;
    if (w as usize, h as usize) != (TILE_PX, TILE_PX) {
        return Err(format!("unexpected tile size {w}x{h}"));
    }
    match dec.read_image().map_err(|e| format!("read_image: {e}"))? {
        tiff::decoder::DecodingResult::F32(v) => Ok(v),
        other => Err(format!("unexpected sample format (not F32): {:?} variant", variant_name(&other))),
    }
}

fn variant_name(r: &tiff::decoder::DecodingResult) -> &'static str {
    use tiff::decoder::DecodingResult::*;
    match r {
        U8(_) => "U8",
        U16(_) => "U16",
        U32(_) => "U32",
        U64(_) => "U64",
        I8(_) => "I8",
        I16(_) => "I16",
        I32(_) => "I32",
        I64(_) => "I64",
        F32(_) => "F32",
        F64(_) => "F64",
    }
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tiles_dir = root.join("ext_data/etopo2022_15s");
    let probe = std::env::args().any(|a| a == "--probe");

    if probe {
        // Decode one land tile and print stats - fast sanity check that the
        // TIFF layout matches expectations before the full build.
        let p = tiles_dir.join(tile_name(75, -30)); // Greenland region (early in download order)
        let px = decode_tile(&p).expect("probe tile decodes");
        let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0f64);
        for &v in &px {
            lo = lo.min(v);
            hi = hi.max(v);
            sum += v as f64;
        }
        println!(
            "probe {}: {} px, min {lo:.1} m, max {hi:.1} m, mean {:.1} m",
            p.file_name().unwrap().to_string_lossy(),
            px.len(),
            sum / px.len() as f64
        );
        return;
    }

    // Full build: accumulate every tile into the output grid.
    let mut acc = vec![0.0f64; OUT_W * OUT_H];
    let mut cnt = vec![0u32; OUT_W * OUT_H];
    let mut tiles_done = 0usize;
    let scale = TILE_PX / (TILE_DEG as usize * OUT_W / 360); // 3600 / 300 = 12 src px per out cell side

    let mut lat_top = 90;
    while lat_top > -90 {
        let mut lon_west = -180;
        while lon_west < 180 {
            let path = tiles_dir.join(tile_name(lat_top, lon_west));
            let px = match decode_tile(&path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("SKIP {}: {e}", path.display());
                    lon_west += TILE_DEG;
                    continue;
                }
            };
            // Output region this tile covers.
            let out_x0 = ((lon_west + 180) as usize * OUT_W) / 360;
            let out_y0 = ((90 - lat_top) as usize * OUT_H) / 180;
            for (i, &m) in px.iter().enumerate() {
                let sx = i % TILE_PX;
                let sy = i / TILE_PX;
                let ox = out_x0 + sx / scale;
                let oy = out_y0 + sy / scale;
                let oi = oy * OUT_W + ox;
                acc[oi] += m as f64;
                cnt[oi] += 1;
            }
            tiles_done += 1;
            if tiles_done % 24 == 0 {
                println!("accumulated {tiles_done}/288 tiles...");
            }
            lon_west += TILE_DEG;
        }
        lat_top -= TILE_DEG;
    }
    println!("tiles accumulated: {tiles_done}/288");
    assert!(tiles_done == 288, "refusing to write a partial globe");

    // Quantize + write HOSHGT1 (same header the JS builder writes; the Rust
    // loader's quantize_meters test pins the round-trip).
    let out_path = root.join("data/planets/earth_heightmap.bin");
    let mut out = Vec::with_capacity(23 + OUT_W * OUT_H * 2);
    out.extend_from_slice(b"HOSHGT1");
    out.extend_from_slice(&(OUT_W as u32).to_le_bytes());
    out.extend_from_slice(&(OUT_H as u32).to_le_bytes());
    out.extend_from_slice(&MIN_M.to_le_bytes());
    out.extend_from_slice(&MAX_M.to_le_bytes());
    let (mut real_lo, mut real_hi) = (f32::MAX, f32::MIN);
    for i in 0..OUT_W * OUT_H {
        let m = (acc[i] / cnt[i].max(1) as f64) as f32;
        real_lo = real_lo.min(m);
        real_hi = real_hi.max(m);
        let q = humanity_engine::terrain::planet_heightmap::quantize_meters(m, MIN_M, MAX_M);
        out.extend_from_slice(&q.to_le_bytes());
    }
    let mut f = fs::File::create(&out_path).expect("create output");
    f.write_all(&out).expect("write output");
    println!(
        "wrote {} ({:.1} MB, {}x{}, data range {real_lo:.1}..{real_hi:.1} m)",
        out_path.display(),
        out.len() as f64 / 1048576.0,
        OUT_W,
        OUT_H
    );

    // Sanity probes the operator cares about.
    let hm = humanity_engine::terrain::planet_heightmap::PlanetHeightmap::load(&out_path)
        .expect("reload output");
    println!("Fuji (35.36N 138.73E): {:.0} m", hm.sample_meters_latlon(35.36, 138.73));
    println!("Everest (27.99N 86.93E): {:.0} m", hm.sample_meters_latlon(27.99, 86.93));
    println!("Challenger Deep (11.37N 142.59E): {:.0} m", hm.sample_meters_latlon(11.37, 142.59));
    println!("Death Valley (36.23N -116.82E): {:.0} m", hm.sample_meters_latlon(36.23, -116.82));
}
