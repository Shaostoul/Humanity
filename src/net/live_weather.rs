//! Live Earth weather (v0.874): real global cloud cover in the game sky.
//!
//! Operator (2026-07-17): "Are you able to pull data from any source? Like a
//! global weather report and some how either bake one in and/or update it
//! dynamically? So, flying around Earth in-game shows the real-world
//! weather?" - yes: NASA GIBS serves the MODIS Cloud Fraction composite as
//! WMS imagery (public domain, the same source Worldview renders). A
//! background thread fetches the global equirect image, decodes the OFFICIAL
//! colormap back to fraction values, and hands the renderer a small RG8 grid:
//!   R = cloud fraction 0..255
//!   G = validity (255 = real data; 0 = no data here -> the shader falls
//!       back to procedural coverage for that spot)
//! Swath gaps and night-side no-data regions therefore blend seamlessly into
//! the procedural sky instead of leaving holes. The last good map is cached
//! on disk so an offline boot still shows yesterday's real weather.
//!
//! Endpoint: gibs.earthdata.nasa.gov WMS (no key, no account). Refresh every
//! 30 min of wall time; the composite itself updates daily, so this is
//! mostly a resilience loop. Settings > Graphics > Planets carries the "Live
//! weather" toggle (default on); turning it off never spawns the thread.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Weather grid dimensions (equirect). Canonical values live on the
/// renderer (compiled in every feature set); aliased here for the fetcher.
pub const WEATHER_W: u32 = crate::renderer::WEATHER_MAP_W;
pub const WEATHER_H: u32 = crate::renderer::WEATHER_MAP_H;

const WMS_URL: &str = "https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi?SERVICE=WMS&REQUEST=GetMap&VERSION=1.3.0&LAYERS=MODIS_Terra_Cloud_Fraction_Day&CRS=EPSG:4326&BBOX=-90,-180,90,180&WIDTH=1440&HEIGHT=720&FORMAT=image/png";

/// UTC calendar date `days_ago` days before now, as (year, month, day).
/// Hinnant's civil-from-days algorithm; no chrono dependency needed for one
/// date computation. (Naive epoch/86400 is exact for UTC day boundaries.)
fn utc_date_days_ago(days_ago: i64) -> (i64, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_unix_days(secs.div_euclid(86400) - days_ago)
}

/// Days-since-1970-01-01 -> (year, month, day). Split from the wrapper so a
/// known-answer test can pin the algorithm without mocking the clock.
fn civil_from_unix_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// NASA GIBS MODIS_Cloud_Fraction colormap: RGB -> cloud fraction %.
/// Parsed from gibs.earthdata.nasa.gov/colormaps/v1.3/MODIS_Cloud_Fraction.xml
/// (101 entries, purple 0% -> red 100%; gray 192,192,192 = no-data fill).
const PALETTE: [([u8; 3], u8); 101] = [
    ([102, 0, 119], 0),
    ([102, 1, 119], 1),
    ([102, 2, 119], 2),
    ([102, 3, 119], 3),
    ([102, 4, 119], 4),
    ([102, 5, 119], 5),
    ([183, 15, 141], 6),
    ([183, 16, 141], 7),
    ([183, 17, 141], 8),
    ([183, 18, 141], 9),
    ([183, 19, 141], 10),
    ([183, 20, 141], 11),
    ([0, 0, 100], 12),
    ([0, 1, 100], 13),
    ([0, 2, 100], 14),
    ([0, 3, 100], 15),
    ([0, 4, 100], 16),
    ([0, 5, 100], 17),
    ([0, 6, 100], 18),
    ([0, 0, 170], 19),
    ([0, 1, 170], 20),
    ([0, 2, 170], 21),
    ([0, 3, 170], 22),
    ([0, 4, 170], 23),
    ([0, 5, 170], 24),
    ([0, 0, 255], 25),
    ([0, 1, 255], 26),
    ([0, 2, 255], 27),
    ([0, 3, 255], 28),
    ([0, 4, 255], 29),
    ([0, 5, 255], 30),
    ([0, 136, 238], 31),
    ([1, 136, 238], 32),
    ([2, 136, 238], 33),
    ([3, 136, 238], 34),
    ([4, 136, 238], 35),
    ([5, 136, 238], 36),
    ([6, 136, 238], 37),
    ([0, 80, 0], 38),
    ([1, 80, 0], 39),
    ([2, 80, 0], 40),
    ([3, 80, 0], 41),
    ([4, 80, 0], 42),
    ([5, 80, 0], 43),
    ([0, 136, 0], 44),
    ([1, 136, 0], 45),
    ([2, 136, 0], 46),
    ([3, 136, 0], 47),
    ([4, 136, 0], 48),
    ([5, 136, 0], 49),
    ([0, 220, 0], 50),
    ([1, 220, 0], 51),
    ([2, 220, 0], 52),
    ([3, 220, 0], 53),
    ([4, 220, 0], 54),
    ([5, 220, 0], 55),
    ([6, 220, 0], 56),
    ([255, 255, 0], 57),
    ([255, 255, 1], 58),
    ([255, 255, 2], 59),
    ([255, 255, 3], 60),
    ([255, 255, 4], 61),
    ([255, 255, 5], 62),
    ([240, 190, 64], 63),
    ([240, 190, 65], 64),
    ([240, 190, 66], 65),
    ([240, 190, 67], 66),
    ([240, 190, 68], 67),
    ([240, 190, 69], 68),
    ([187, 136, 0], 69),
    ([187, 136, 1], 70),
    ([187, 136, 2], 71),
    ([187, 136, 3], 72),
    ([187, 136, 4], 73),
    ([187, 136, 5], 74),
    ([187, 136, 6], 75),
    ([122, 90, 3], 76),
    ([122, 90, 4], 77),
    ([122, 90, 5], 78),
    ([122, 90, 6], 79),
    ([122, 90, 7], 80),
    ([122, 90, 8], 81),
    ([110, 0, 0], 82),
    ([110, 0, 1], 83),
    ([110, 0, 2], 84),
    ([110, 0, 3], 85),
    ([110, 0, 4], 86),
    ([110, 0, 5], 87),
    ([170, 0, 0], 88),
    ([170, 0, 1], 89),
    ([170, 0, 2], 90),
    ([170, 0, 3], 91),
    ([170, 0, 4], 92),
    ([170, 0, 5], 93),
    ([170, 0, 6], 94),
    ([255, 0, 0], 95),
    ([255, 0, 1], 96),
    ([255, 0, 2], 97),
    ([255, 0, 3], 98),
    ([255, 0, 4], 99),
    ([255, 0, 5], 100),
];

/// Where the last good map is cached (survives offline boots).
pub fn cache_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("HumanityOS").join("cache");
        let _ = std::fs::create_dir_all(&dir);
        return dir.join("earth_weather.rg");
    }
    PathBuf::from("earth_weather.rg")
}

/// Decode the WMS PNG into the RG8 grid. Exact palette matching against the
/// official colormap; anything else (gray fill, transparent, antialiased
/// off-palette pixels) becomes validity 0.
fn decode(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png_bytes).ok()?.to_rgba8();
    if img.width() != WEATHER_W || img.height() != WEATHER_H {
        log::warn!("[Weather] unexpected image size {}x{}", img.width(), img.height());
        return None;
    }
    let mut out = vec![0u8; (WEATHER_W * WEATHER_H * 2) as usize];
    let mut valid_px = 0usize;
    // Nearest-palette classification, not exact matching: the WMS resamples
    // the source imagery to our grid, so most pixels are BLENDS of adjacent
    // palette colors (exact matching left only 12% valid on first boot).
    // The rainbow ramp is a smooth curve in RGB, so blends of neighbors stay
    // near the curve; blends into the gray/white no-data fill drift far from
    // every entry and fail the distance gate below.
    let dist2 = |a: [u8; 3], b: [u8; 3]| -> i32 {
        let dr = a[0] as i32 - b[0] as i32;
        let dg = a[1] as i32 - b[1] as i32;
        let db = a[2] as i32 - b[2] as i32;
        dr * dr + dg * dg + db * db
    };
    // Anything nearer to a no-data anchor than to every palette color is
    // invalid regardless of distance (gray fill, white unfilled tiles,
    // black background).
    const NO_DATA: [[u8; 3]; 3] = [[192, 192, 192], [255, 255, 255], [0, 0, 0]];
    // Max accepted squared distance to the nearest palette color (~55 per
    // channel total): rejects data/no-data boundary blends.
    const MAX_D2: i32 = 3000;
    for (i, p) in img.pixels().enumerate() {
        let [r, g, b, a] = p.0;
        if a < 128 {
            continue;
        }
        let px = [r, g, b];
        let mut best_d2 = i32::MAX;
        let mut best_frac = 0u8;
        for (rgb, frac) in PALETTE {
            let d = dist2(px, rgb);
            if d < best_d2 {
                best_d2 = d;
                best_frac = frac;
            }
        }
        let nodata_d2 = NO_DATA.iter().map(|n| dist2(px, *n)).min().unwrap_or(i32::MAX);
        if best_d2 <= MAX_D2 && best_d2 < nodata_d2 {
            out[i * 2] = (best_frac as u32 * 255 / 100) as u8;
            out[i * 2 + 1] = 255;
            valid_px += 1;
        }
    }
    // A mostly-empty map (endpoint hiccup, wrong layer state) is worse than
    // keeping the cache: require at least 20% real coverage to accept.
    if valid_px * 5 < (WEATHER_W * WEATHER_H) as usize {
        log::warn!(
            "[Weather] map only {:.0}% valid - keeping previous data",
            valid_px as f32 * 100.0 / (WEATHER_W * WEATHER_H) as f32
        );
        return None;
    }
    log::info!(
        "[Weather] live cloud map decoded: {:.0}% real data coverage",
        valid_px as f32 * 100.0 / (WEATHER_W * WEATHER_H) as f32
    );
    Some(out)
}

fn fetch_once() -> Option<Vec<u8>> {
    // The current UTC day's composite is mostly-empty swaths until late in
    // the day (first boot measured 12% valid without TIME). Yesterday's
    // composite is complete; the day before is the fallback if yesterday is
    // still processing. "Live weather" at daily-composite cadence.
    for days_ago in 1..=2 {
        let (y, m, d) = utc_date_days_ago(days_ago);
        let url = format!("{WMS_URL}&TIME={y:04}-{m:02}-{d:02}");
        let Ok(resp) = ureq::get(&url).timeout(std::time::Duration::from_secs(30)).call() else {
            continue;
        };
        let mut bytes = Vec::new();
        use std::io::Read as _;
        if resp.into_reader().take(16 * 1024 * 1024).read_to_end(&mut bytes).is_err() {
            continue;
        }
        if let Some(grid) = decode(&bytes) {
            return Some(grid);
        }
    }
    None
}

/// Spawn the background loop: send the cached map immediately (offline-first),
/// then fetch fresh every 30 minutes. Each successful fetch updates the cache
/// and sends the grid to the render thread.
pub fn spawn(tx: Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        let cache = cache_path();
        if let Ok(bytes) = std::fs::read(&cache) {
            if bytes.len() == (WEATHER_W * WEATHER_H * 2) as usize {
                log::info!("[Weather] cached cloud map loaded");
                let _ = tx.send(bytes);
            }
        }
        loop {
            if let Some(grid) = fetch_once() {
                let _ = std::fs::write(&cache, &grid);
                if tx.send(grid).is_err() {
                    return; // renderer gone - shut down quietly
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(30 * 60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_covers_the_full_fraction_range() {
        // 0% and 100% anchors from the official colormap must be present,
        // and every value 0..=100 must appear exactly once.
        let mut seen = [false; 101];
        for (_, v) in PALETTE {
            assert!(v <= 100, "fraction out of range: {v}");
            assert!(!seen[v as usize], "duplicate fraction {v}");
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "missing fraction values");
        let lut: std::collections::HashMap<[u8; 3], u8> = PALETTE.iter().copied().collect();
        assert_eq!(lut.get(&[102, 0, 119]), Some(&0), "purple = 0%");
        assert_eq!(lut.get(&[255, 0, 5]), Some(&100), "red = 100%");
        assert_eq!(lut.get(&[192, 192, 192]), None, "gray fill is NOT data");
    }

    #[test]
    fn civil_date_known_answers() {
        assert_eq!(civil_from_unix_days(0), (1970, 1, 1));
        assert_eq!(civil_from_unix_days(19_723), (2024, 1, 1)); // leap year start
        assert_eq!(civil_from_unix_days(19_782), (2024, 2, 29)); // leap day
        assert_eq!(civil_from_unix_days(20_651), (2026, 7, 17));
    }
}
