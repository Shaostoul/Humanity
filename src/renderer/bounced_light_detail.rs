//! BOUNCED LIGHT DETAIL (2026-10-20): Settings > Graphics > Light and sky.
//!
//! The operator asked whether the room light probes' budget could be changed
//! on the fly. It can: a level is the two numbers `room_probes::lattice_for`
//! works to (the home's probe spacing and the ship's probe cap,
//! `ProbeBudget`), the levels are data (data/gui/bounced_light_detail.json),
//! and `room_probes_gpu::RoomGi::set_budget` lays the lattice and the atlas
//! out again the next frame (`engine::room_gi` hands the level over). The
//! frame's work does not change with the level (`MAX_UPDATES_PER_FRAME`
//! probes an update whatever the count); the memory does, and the far rooms
//! take longer to relight at a finer level.
//!
//! THIS FILE is the levels, the memory each takes (its cap x
//! `BYTES_PER_PROBE`, plus the ambient row) and the words the Settings page
//! shows, and the graphics card's own memory for the warning beside them
//! (`note_adapter`, recorded at renderer start). docs/design/room-gi.md,
//! "Bounced light detail".
//!
//! Since 2026-10-21 a level's cap is the probes RESIDENT at once (probe
//! streaming: the rooms nearest the player hold them), and the lowest level,
//! "Ambient", has a cap of 0: no probes, one colour per room from its own
//! lamps and windows (`room_probes::room_ambient`).

use super::room_probes::{
    AtlasLayout, ProbeBudget, ATLAS_WIDTH, DEPTH_TILE, IRR_TILE, MAX_PROBE_SPACING_M, MAX_TOTAL_PROBES, PROBE_SPACING_M,
};

/// Video memory one probe takes in the atlas: its irradiance tile and its
/// depth tile, borders included, at 8 bytes a texel (`room_probes_gpu::ATLAS_FORMAT`,
/// Rgba16Float): (10 x 10 + 18 x 18) x 8 = 3,392 bytes.
pub const BYTES_PER_PROBE: u64 = ((IRR_TILE * IRR_TILE + DEPTH_TILE * DEPTH_TILE) * 8) as u64;

/// The atlas's AMBIENT ROW (`AtlasLayout::amb_y0`): one texel per room for
/// its ambient colour, one atlas row wide, at every level. 64,800 bytes.
pub const AMBIENT_ROW_BYTES: u64 = ATLAS_WIDTH as u64 * 8;

/// The video memory a probe cap takes: `max_probes` x `BYTES_PER_PROBE`. The
/// atlas holds exactly its level's cap (`room_probes_gpu::RoomGi::relayout`);
/// its last rows are padded to the atlas width, at most one more row of each
/// tile kind (1.8 MB), and the ambient row comes on top (`AMBIENT_ROW_BYTES`).
pub fn probe_memory_bytes(max_probes: u32) -> u64 {
    max_probes as u64 * BYTES_PER_PROBE
}

/// "106 MB", or "63 KB" under a megabyte (the Ambient level's whole atlas).
pub fn memory_words(bytes: u64) -> String {
    if bytes < 1 << 20 {
        format!("{} KB", grouped((bytes + 512) >> 10))
    } else {
        mb_words(bytes)
    }
}

/// `n` with thousands separators ("32,768").
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// "106 MB": whole megabytes of 1,048,576 bytes, rounded to the nearest, the
/// unit the Performance page's graphics-memory pie uses (`widgets::pie::fmt_bytes`).
pub fn mb_words(bytes: u64) -> String {
    format!("{} MB", grouped((bytes + (1 << 19)) >> 20))
}

/// Where the levels are read from (relative to the data folder's parent, as
/// data/gui/graphics_presets.json is).
pub const DETAIL_LEVELS_PATH: &str = "data/gui/bounced_light_detail.json";
/// The level a new install, or a saved id the file no longer lists, gets.
pub const DEFAULT_DETAIL: &str = "high";
/// The finest spacing a level may ask for. Finer than this the home alone
/// outgrows any cap the atlas can hold, so the lattice would coarsen it anyway.
pub const MIN_DETAIL_SPACING_M: f32 = 0.25;

/// One Bounced light detail level: the `ProbeBudget` it lays the probes out to.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct DetailLevel {
    pub id: String,
    pub name: String,
    /// How far apart every room's probes aim to be (metres). Unused by the
    /// Ambient level, which may leave it out.
    #[serde(default = "default_spacing")]
    pub spacing_m: f32,
    /// The most probes held at once, by the rooms nearest the player. 0 is
    /// the Ambient level: no probes, one colour per room.
    pub max_probes: u32,
    /// A short line saying what the level looks like.
    #[serde(default)]
    pub note: String,
}

fn default_spacing() -> f32 {
    PROBE_SPACING_M
}

impl DetailLevel {
    pub fn budget(&self) -> ProbeBudget {
        ProbeBudget { spacing_m: self.spacing_m, max_probes: self.max_probes }
    }

    /// Video memory at this level: its cap's probes and the ambient row.
    pub fn memory_bytes(&self) -> u64 {
        probe_memory_bytes(self.max_probes) + AMBIENT_ROW_BYTES
    }

    /// What the Settings page says beside the level: its cap and its memory,
    /// both from the level's own numbers, then its note.
    pub fn words(&self) -> String {
        let note = if self.note.is_empty() { String::new() } else { format!(" {}", self.note) };
        if self.max_probes == 0 {
            return format!("no probes, about {} of video memory.{note}", memory_words(self.memory_bytes()));
        }
        format!(
            "up to {} probes near you, about {} of video memory.{note}",
            grouped(self.max_probes as u64),
            memory_words(self.memory_bytes())
        )
    }
}

/// The design's level, for an install whose levels file is missing or empty.
fn built_in_detail() -> DetailLevel {
    DetailLevel {
        id: DEFAULT_DETAIL.to_string(),
        name: "High".to_string(),
        spacing_m: PROBE_SPACING_M,
        max_probes: MAX_TOTAL_PROBES,
        note: String::new(),
    }
}

/// The most probes one atlas can hold: its height must stay within the
/// 8,192 texels `wgpu::Limits::default()` guarantees (the width is fixed at
/// `room_probes::ATLAS_WIDTH`). About 156,000.
pub fn atlas_max_probes() -> u32 {
    let limit = wgpu::Limits::default().max_texture_dimension_2d;
    let (mut lo, mut hi) = (1u32, 1u32 << 22);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if AtlasLayout::for_probes(mid).height <= limit {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

#[derive(serde::Deserialize)]
struct DetailLevelsFile {
    #[serde(default)]
    levels: Vec<DetailLevel>,
}

/// The levels in `text` (the levels file; None when it could not be read),
/// each held to what the renderer can do: spacing within
/// `MIN_DETAIL_SPACING_M..=MAX_PROBE_SPACING_M`, the cap within one atlas (or
/// 0, the Ambient level), a repeated id dropped. Nothing usable gives the
/// design's level alone, so the setting always has a level to stand on.
pub fn parse_detail_levels(text: Option<&str>) -> Vec<DetailLevel> {
    let parsed = text.and_then(|t| match serde_json::from_str::<DetailLevelsFile>(t) {
        Ok(f) => Some(f.levels),
        Err(e) => {
            log::warn!("[RoomGI] {DETAIL_LEVELS_PATH} does not parse ({e}); Bounced light detail offers High only");
            None
        }
    });
    let most = atlas_max_probes();
    let mut levels: Vec<DetailLevel> = Vec::new();
    for mut l in parsed.unwrap_or_default() {
        if l.id.is_empty() || levels.iter().any(|k| k.id == l.id) {
            continue;
        }
        let spacing = if l.spacing_m.is_finite() { l.spacing_m } else { PROBE_SPACING_M };
        let spacing = spacing.clamp(MIN_DETAIL_SPACING_M, MAX_PROBE_SPACING_M);
        // 0 stays 0: the Ambient level holds no probes at all.
        let probes = if l.max_probes == 0 { 0 } else { l.max_probes.clamp(8, most) };
        if spacing != l.spacing_m || probes != l.max_probes {
            log::warn!(
                "[RoomGI] detail level {}: {} m / {} probes held to {spacing} m / {probes}",
                l.id,
                l.spacing_m,
                l.max_probes
            );
        }
        l.spacing_m = spacing;
        l.max_probes = probes;
        if l.name.is_empty() {
            l.name = l.id.clone();
        }
        levels.push(l);
    }
    if levels.is_empty() {
        levels.push(built_in_detail());
    }
    levels
}

/// Every level, read once from `DETAIL_LEVELS_PATH` (a change to the file
/// takes effect at the next start).
pub fn detail_levels() -> &'static [DetailLevel] {
    static LEVELS: std::sync::OnceLock<Vec<DetailLevel>> = std::sync::OnceLock::new();
    LEVELS.get_or_init(|| {
        let text = std::fs::read_to_string(DETAIL_LEVELS_PATH).ok();
        if text.is_none() {
            log::warn!("[RoomGI] {DETAIL_LEVELS_PATH} not found; Bounced light detail offers High only");
        }
        parse_detail_levels(text.as_deref())
    })
}

/// True when `id` names a level the file lists.
pub fn is_detail_level(id: &str) -> bool {
    detail_levels().iter().any(|l| l.id == id)
}

/// The level `id` names, or `DEFAULT_DETAIL`'s for an id the file does not
/// list (the first level when it does not list that either).
pub fn detail_level(id: &str) -> &'static DetailLevel {
    let levels = detail_levels();
    levels.iter().find(|l| l.id == id).or_else(|| levels.iter().find(|l| l.id == DEFAULT_DETAIL)).unwrap_or(&levels[0])
}

/// What the graphics card the game runs on says about its memory, recorded
/// once at renderer start (`note_adapter`). wgpu 24's `AdapterInfo` carries
/// no memory size, so on Windows it comes from the card's driver install
/// record (`HardwareInformation.qwMemorySize`, matched by PCI vendor and
/// device id), which is what Windows' own tools read.
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterMemory {
    pub name: String,
    /// An integrated GPU: it uses the computer's main memory, so it has no
    /// fixed amount of its own to measure a level against.
    pub integrated: bool,
    /// Its own memory in bytes, when the driver's record says.
    pub dedicated_bytes: Option<u64>,
}

static ADAPTER: std::sync::OnceLock<AdapterMemory> = std::sync::OnceLock::new();

/// Record the adapter the renderer was given (renderer start, once).
pub fn note_adapter(info: &wgpu::AdapterInfo) {
    let integrated = info.device_type == wgpu::DeviceType::IntegratedGpu;
    let dedicated_bytes = if integrated { None } else { dedicated_memory(info.vendor, info.device) };
    log::info!(
        "[RoomGI] graphics card {} ({:?}): {}",
        info.name,
        info.device_type,
        dedicated_bytes.map(|b| format!("{} of its own memory", mb_words(b))).unwrap_or_else(|| "no memory size reported".to_string())
    );
    let _ = ADAPTER.set(AdapterMemory { name: info.name.clone(), integrated, dedicated_bytes });
}

/// The adapter `note_adapter` recorded; None before the renderer starts (and
/// in the headless page snapshots, which have no renderer).
pub fn adapter_memory() -> Option<&'static AdapterMemory> {
    ADAPTER.get()
}

/// A level using more than this share of the card's own memory is warned
/// about: one lighting feature taking a twentieth of the card starves the
/// terrain, the meshes and the textures that need the rest.
pub const MEMORY_WARN_SHARE: f64 = 0.05;

/// The card line under the setting, and a warning when `level` would use
/// more than `MEMORY_WARN_SHARE` of the card's own memory.
pub fn memory_check(level: &DetailLevel, adapter: Option<&AdapterMemory>) -> (String, Option<String>) {
    let Some(a) = adapter else {
        return ("The graphics card has not reported its memory yet, so no level is checked against it.".to_string(), None);
    };
    if a.integrated {
        return (
            format!("{} shares your computer's main memory, so there is no fixed amount to check a level against.", a.name),
            None,
        );
    }
    let Some(card) = a.dedicated_bytes.filter(|b| *b > 0) else {
        return (format!("{} has not reported how much memory it has, so no level is checked against it.", a.name), None);
    };
    let used = level.memory_bytes();
    let share = used as f64 / card as f64;
    let line = format!("{}: {} of its own memory; {} uses {:.1}% of it.", a.name, mb_words(card), level.name, share * 100.0);
    let warn = (share > MEMORY_WARN_SHARE).then(|| {
        format!(
            "{} would use {}, more than a twentieth of this card's {}. The rest of the world needs that memory, and the game may stutter when the card runs short: a lower level gives it back.",
            level.name,
            mb_words(used),
            mb_words(card)
        )
    });
    (line, warn)
}

/// The memory of the display adapter record whose matching id names this
/// PCI vendor and device (`pci\ven_10de&dev_2786`), from (matching id,
/// bytes) pairs. Pure, so it is tested without a registry.
pub fn pick_dedicated(records: &[(String, u64)], vendor: u32, device: u32) -> Option<u64> {
    let want = format!("ven_{vendor:04x}&dev_{device:04x}");
    records.iter().find(|(id, b)| *b > 0 && id.to_ascii_lowercase().contains(&want)).map(|(_, b)| *b)
}

/// Windows' display adapter install records (the Display class key), read
/// only: each one's matching id and its memory (`HardwareInformation.qwMemorySize`,
/// or the 32-bit `HardwareInformation.MemorySize` an older driver writes).
#[cfg(all(windows, feature = "native"))]
fn display_adapter_records() -> Vec<(String, u64)> {
    use crate::games::registry::{open, Root};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_QWORD};
    const CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let qword = |sub: &str, value: &str| -> Option<u64> {
        let (path, value) = (wide(&format!(r"{CLASS}\{sub}")), wide(value));
        let mut v: u64 = 0;
        let mut size: u32 = 8;
        // SAFETY: both strings are NUL-ended and outlive the call; `v` holds the 8 bytes `size` says.
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(path.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_QWORD,
                None,
                Some(&mut v as *mut u64 as *mut core::ffi::c_void),
                Some(&mut size),
            )
        };
        (r == ERROR_SUCCESS && size == 8).then_some(v)
    };
    let Some(class) = open(Root::LocalMachine, CLASS) else { return Vec::new() };
    class
        .subkeys()
        .into_iter()
        .filter_map(|sub| {
            let key = class.open(&sub)?;
            let id = key.string("MatchingDeviceId")?;
            let bytes = qword(&sub, "HardwareInformation.qwMemorySize")
                .or_else(|| key.dword("HardwareInformation.MemorySize").map(u64::from))?;
            Some((id, bytes))
        })
        .collect()
}

#[cfg(all(windows, feature = "native"))]
fn dedicated_memory(vendor: u32, device: u32) -> Option<u64> {
    pick_dedicated(&display_adapter_records(), vendor, device)
}

#[cfg(not(all(windows, feature = "native")))]
fn dedicated_memory(_vendor: u32, _device: u32) -> Option<u64> {
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use super::super::room_probes::{lattice_for, probe_counts, RoomBox};
    use glam::Vec3;

    /// A room box with mid-grey faces.
    pub(crate) fn room(min: Vec3, max: Vec3) -> RoomBox {
        RoomBox { min, max, refl: [[0.5; 3]; 6], lid_transmittance: 0.0 }
    }

    /// Six 10 x 3 x 10 m rooms of a home beside eight 40 x 3 x 40 m rooms of
    /// the rest of the ship: every level's budget holds far fewer probes than
    /// the whole, so residency, not the lattice, decides who holds them.
    pub(crate) fn ship() -> Vec<RoomBox> {
        let mut out: Vec<RoomBox> = (0..6)
            .map(|i| {
                let x = -20.0 - i as f32 * 12.0;
                room(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 10.0, 3.0, 10.0))
            })
            .collect();
        out.extend((0..8).map(|i| room(Vec3::new(i as f32 * 50.0, 0.0, 0.0), Vec3::new(i as f32 * 50.0 + 40.0, 3.0, 40.0))));
        out
    }

    /// The shipped levels file: Ambient (no probes) and then Low to Ultra, High
    /// exactly the design (today's `PROBE_SPACING_M` and `MAX_TOTAL_PROBES`,
    /// so the default changes nothing), the probe levels ever finer and ever
    /// bigger, and every cap's atlas within the 8,192-texel texture limit.
    /// Seen red first (2026-10-21) with the levels file without the Ambient level: "data/gui/bounced_light_detail.json".
    #[test]
    fn the_shipped_levels_are_ambient_then_low_to_ultra_with_high_the_design() {
        let levels = detail_levels();
        let ids: Vec<&str> = levels.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["ambient", "low", "medium", "high", "ultra"], "{DETAIL_LEVELS_PATH}");
        let ambient = detail_level("ambient");
        assert_eq!(ambient.max_probes, 0, "the Ambient level holds no probes");
        assert!(ambient.budget().ambient_only());
        let high = detail_level("high");
        assert_eq!(high.budget(), ProbeBudget::default(), "High is today's 1 m and 32,768");
        assert_eq!(detail_level(DEFAULT_DETAIL).id, "high");
        assert_eq!(detail_level("no such level").id, "high", "an unknown id gets the default");
        for w in levels[1..].windows(2) {
            assert!(w[1].spacing_m < w[0].spacing_m && w[1].max_probes > w[0].max_probes, "{} then {}", w[0].id, w[1].id);
        }
        for l in levels {
            let atlas = AtlasLayout::for_probes(l.max_probes);
            assert!(atlas.height <= 8192, "{}: an atlas {} texels tall", l.id, atlas.height);
        }
        assert_eq!(ProbeBudget::default().per_room(), super::super::room_probes::MAX_PROBES_PER_ROOM);
    }

    /// The words beside each level give its cap and its memory, and the
    /// memory is the cap x 3,392 bytes (a probe's 10 x 10 irradiance tile and
    /// 18 x 18 depth tile at 8 bytes a texel) plus the ambient row's 64,800,
    /// never a number typed in. The real atlas for the cap is at most one more
    /// row of each tile kind (its last rows are padded out to the atlas
    /// width): 1.8 MB. The Ambient level's is the ambient row alone, 63 KB.
    ///
    /// Seen red 2026-10-20 with `words` printing the memory of half the cap:
    /// "low: \"up to 8,192 probes, about 13 MB of video memory. Probes 2 m
    /// apart: soft, broad bounced light.\" lacks 27 MB".
    #[test]
    fn every_level_s_memory_words_are_its_cap_times_3392_bytes() {
        assert_eq!(BYTES_PER_PROBE, 3392);
        assert_eq!(AMBIENT_ROW_BYTES, 64_800);
        assert_eq!(super::super::room_probes_gpu::ATLAS_FORMAT.block_copy_size(None), Some(8), "the atlas is 8 bytes a texel");
        assert_eq!(probe_memory_bytes(MAX_TOTAL_PROBES), 111_149_056);
        assert_eq!(mb_words(111_149_056), "106 MB");
        assert_eq!(mb_words(12_878_610_432), "12,282 MB");
        assert_eq!(memory_words(64_800), "63 KB");
        for l in detail_levels() {
            let bytes = l.max_probes as u64 * 3392 + 64_800;
            let words = l.words();
            println!("{:<7} {:>5.2} m  {:>6} probes  {}", l.name, l.spacing_m, l.max_probes, words);
            assert!(words.contains(&memory_words(bytes)), "{}: {words:?} lacks {}", l.id, memory_words(bytes));
            if l.max_probes > 0 {
                assert!(words.contains(&grouped(l.max_probes as u64)), "{}: {words:?} lacks the cap", l.id);
            } else {
                assert!(words.starts_with("no probes"), "{}: {words:?}", l.id);
            }
            let real = AtlasLayout::for_probes(l.max_probes).texel_count() as u64 * 8;
            let one_row_each = ((IRR_TILE + DEPTH_TILE) * super::super::room_probes::ATLAS_WIDTH) as u64 * 8;
            println!("        the atlas at the cap: {real} B, {} more than the words", real - bytes);
            assert!(real >= bytes && real - bytes <= one_row_each, "{}: atlas {real} B against {bytes} B", l.id);
        }
        assert_eq!(AtlasLayout::for_probes(0).texel_count() as u64 * 8, AMBIENT_ROW_BYTES);
    }

    /// Each level's spacing reaches every room that fits its per-room cap (a
    /// quarter of its budget) and no room's lattice depends on how many other
    /// rooms there are (probe streaming: the budget is for the rooms near the
    /// player, `room_probes::Residency`), and no two probe levels lay the
    /// same lattice out.
    ///
    /// Seen red 2026-10-20 with `lattice_for` reading the constants instead of
    /// `budget` (the design spacing and cap): "low: home room 0 [11, 4, 11]
    /// against [6, 3, 6] at 2 m".
    #[test]
    fn each_level_s_spacing_and_cap_reach_the_lattice() {
        let rooms = ship();
        let mut seen: Vec<Vec<[u32; 3]>> = Vec::new();
        for l in detail_levels().iter().filter(|l| l.max_probes > 0) {
            let lat = lattice_for(&rooms, l.budget());
            for (i, r) in rooms.iter().enumerate() {
                let want = probe_counts(r.size(), l.spacing_m);
                let n = want[0] * want[1] * want[2];
                if n <= l.budget().per_room() {
                    assert_eq!(lat.counts[i], want, "{}: room {i} {:?} against {want:?} at {} m", l.id, lat.counts[i], l.spacing_m);
                } else {
                    assert!(lat.probes_in(i) <= l.budget().per_room(), "{}: room {i} over its own cap", l.id);
                }
                // The same room alone gets the same lattice.
                assert_eq!(lattice_for(std::slice::from_ref(r), l.budget()).counts[0], lat.counts[i], "{}: room {i}", l.id);
            }
            assert!(!seen.contains(&lat.counts), "{}: the same lattice as a level before it", l.id);
            seen.push(lat.counts);
        }
        // And a level's numbers, not the file's: a made-up budget reaches it too.
        let lat = lattice_for(&rooms, ProbeBudget { spacing_m: 0.5, max_probes: 100_000 });
        assert_eq!(lat.counts[0], [21, 7, 21]);
    }

    /// The levels file is held to what the renderer can do, and a missing or
    /// broken file still leaves the design's level to stand on.
    #[test]
    fn a_levels_file_is_held_to_what_the_renderer_can_hold() {
        assert_eq!(parse_detail_levels(None), vec![built_in_detail()]);
        assert_eq!(parse_detail_levels(Some("not json")), vec![built_in_detail()]);
        assert_eq!(parse_detail_levels(Some(r#"{"levels":[]}"#)), vec![built_in_detail()]);
        let text = r#"{"levels":[
            {"id":"tiny","name":"","spacing_m":0.01,"max_probes":4},
            {"id":"huge","name":"Huge","spacing_m":500.0,"max_probes":99000000},
            {"id":"tiny","name":"Again","spacing_m":1.0,"max_probes":1000}
        ]}"#;
        let got = parse_detail_levels(Some(text));
        assert_eq!(got.len(), 2, "a repeated id is dropped: {got:?}");
        assert_eq!((got[0].name.as_str(), got[0].spacing_m, got[0].max_probes), ("tiny", MIN_DETAIL_SPACING_M, 8));
        assert_eq!((got[1].spacing_m, got[1].max_probes), (MAX_PROBE_SPACING_M, atlas_max_probes()));
        let most = atlas_max_probes();
        assert!(AtlasLayout::for_probes(most).height <= 8192 && AtlasLayout::for_probes(most + 1).height > 8192, "{most}");
        // A cap of 0 is the Ambient level and stays 0 (any other cap is held
        // to at least 8); a level may leave its spacing out. Seen red first
        // (2026-10-21) with a cap of 0 held to at least 8: "[DetailLevel { id: 'flat', name: 'Flat', spacing_m: 1.0, max_probes: 8, note: '' }]".
        let got = parse_detail_levels(Some(r#"{"levels":[{"id":"flat","name":"Flat","max_probes":0}]}"#));
        assert_eq!((got[0].max_probes, got[0].spacing_m), (0, PROBE_SPACING_M), "{got:?}");
    }

    /// The card line and the warning: no warning without a card's own memory
    /// to measure against; a 12 GB card takes Ultra easily; a 2 GB card is
    /// warned at High (106 MB, 5.2%) and not at Medium (53 MB, 2.6%).
    #[test]
    fn a_level_too_big_for_the_card_is_warned_about() {
        let card = |bytes: Option<u64>, integrated: bool| AdapterMemory { name: "Test GPU".into(), integrated, dedicated_bytes: bytes };
        let (high, medium, ultra) = (detail_level("high"), detail_level("medium"), detail_level("ultra"));
        assert_eq!(memory_check(high, None).1, None);
        assert_eq!(memory_check(ultra, Some(&card(Some(512 << 20), true))).1, None, "an integrated GPU has no own memory to measure");
        assert_eq!(memory_check(ultra, Some(&card(None, false))).1, None);
        let (line, warn) = memory_check(ultra, Some(&card(Some(12_878_610_432), false)));
        assert!(line.contains("12,282 MB") && line.contains("1.7%"), "{line}");
        assert_eq!(warn, None);
        let two_gb = card(Some(2048 << 20), false);
        let warn = memory_check(high, Some(&two_gb)).1.expect("High on a 2 GB card is warned about");
        assert!(warn.contains("106 MB") && warn.contains("2,048 MB"), "{warn}");
        assert_eq!(memory_check(medium, Some(&two_gb)).1, None);
    }

    /// The card's record is the one whose matching id names its PCI vendor
    /// and device, letter case aside; another card's, or a virtual screen's,
    /// is not.
    #[test]
    fn the_card_s_memory_is_its_own_record_s() {
        let records: Vec<(String, u64)> = vec![
            (r"Root\MetaVirtualScreenDriver".to_string(), 1 << 30),
            (r"pci\ven_8086&dev_3e92".to_string(), 1 << 30),
            (r"PCI\VEN_10DE&DEV_2786&SUBSYS_51611462".to_string(), 12_878_610_432),
        ];
        assert_eq!(pick_dedicated(&records, 0x10de, 0x2786), Some(12_878_610_432));
        assert_eq!(pick_dedicated(&records, 0x8086, 0x3e92), Some(1 << 30));
        assert_eq!(pick_dedicated(&records, 0x1002, 0x73bf), None);
        assert_eq!(pick_dedicated(&[(r"pci\ven_10de&dev_2786".into(), 0)], 0x10de, 0x2786), None, "no size is no size");
    }

    /// This computer's display adapter records, read the way `note_adapter`
    /// reads them (KEY_READ only). Each record names a device and a size, and
    /// looking a PCI card up by its own ids finds its own record.
    #[cfg(all(windows, feature = "native"))]
    #[test]
    fn this_computer_s_display_adapter_records_read() {
        let records = display_adapter_records();
        for (id, bytes) in &records {
            println!("{id}: {} ({bytes} bytes)", mb_words(*bytes));
            assert!(*bytes > 0 && !id.is_empty());
            let lower = id.to_ascii_lowercase();
            let hex = |key: &str| lower.split(key).nth(1).and_then(|t| u32::from_str_radix(&t[..4.min(t.len())], 16).ok());
            if let (Some(v), Some(d)) = (hex("ven_"), hex("dev_")) {
                assert!(pick_dedicated(&records, v, d).is_some(), "{id} is found by its own ids");
            }
        }
    }
}
