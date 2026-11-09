//! The pack format's unit tests (whole-earth-maps.md 9.2). Each was seen red against a deliberate
//! mutation of `map_pack.rs` on 2026-11-09; the mutation and what it printed are in its comment.

use super::*;

fn manifest() -> PackManifest {
    PackManifest {
        format: "HOSMPAK1".into(),
        spec: "docs/design/whole-earth-maps.md".into(),
        tile: (7, 6, 0b10_01_11_00_01_10),
        build_id: String::new(),
        builder: "test".into(),
        levels: vec![6, 9, 12],
        lattice_segments: LATTICE_SEGMENTS,
        layers: vec![LayerInfo {
            name: "land_h".into(),
            source: "a test".into(),
            snapshot: "2026-11-09".into(),
            licence: "CC-BY-4.0".into(),
            licence_url: "https://creativecommons.org/licenses/by/4.0/".into(),
            notice: "test".into(),
        }],
        notices: vec!["ODbL".into()],
    }
}

fn sample_pack() -> (Vec<u8>, Vec<i32>, Vec<WaterRing>, Vec<MapWay>, Vec<MapBuilding>) {
    let n = crate::map_tiles::lattice_len(LATTICE_SEGMENTS);
    let dm: Vec<i32> = (0..n as i32).map(|i| if i == 7 { NODATA } else { 44_127 - i * 3 + (i % 5) * 17 }).collect();
    let rings = vec![
        WaterRing { kind: WaterRingKind::Sea, feature: 0, name: Some("Dyes Inlet".into()), points: vec![(476_400_000, -1_226_950_000), (476_400_100, -1_226_940_000), (476_410_000, -1_226_945_000)], clip: vec![false, true, false] },
        WaterRing { kind: WaterRingKind::Island, feature: 12_345, name: None, points: vec![(1, 2), (3, -4), (-5, 6), (7, 8), (9, 10), (11, 12), (13, 14), (15, 16), (17, 18)], clip: vec![true, false, false, false, false, false, false, false, true] },
    ];
    let ways = vec![MapWay { osm_id: 99, class: 3, bridge: true, tunnel: false, layer: -1, width_m: 7.5, name: Some("Silverdale Way NW".into()), points: vec![(476_440_000, -1_226_900_000), (476_441_000, -1_226_901_234)], start_clipped: false, end_clipped: true }];
    let bs = vec![MapBuilding { osm_id: 7, height_m: 12.5, min_height_m: 0.0, levels: 3, ring: vec![(0, 0), (10, 0), (10, 10), (0, 10)] }];
    let mut w = PackWriter::new();
    w.add(ChunkKind::LandH, 12, 4095, encode_lattice(&dm));
    w.add(ChunkKind::Water, 12, 4095, encode_water(&rings));
    w.add(ChunkKind::Ways, 12, 4095, encode_ways(&ways));
    w.add(ChunkKind::Buildings, 12, 4095, encode_buildings(&bs));
    w.add(ChunkKind::Cover, 12, 1, Vec::new());
    let mut m = manifest();
    m.build_id = w.content_hash();
    (w.finish(&m).expect("writes"), dm, rings, ways, bs)
}

/// Every layer survives the file: write, open, read back, equal. Seen red 2026-11-09 with
/// `put_points` writing absolute coordinates while `get_points` still added deltas: the running
/// sum leaves i32, the water chunk refuses to decode, and `pack.water(4095).unwrap()` panics on None.
#[test]
fn a_pack_round_trips_every_layer() {
    let (bytes, dm, rings, ways, bs) = sample_pack();
    let pack = Pack::open(bytes).expect("opens");
    assert_eq!(pack.patch(), PatchId { face: 7, depth: 6, path: 0b10_01_11_00_01_10 });
    assert_eq!(pack.count(ChunkKind::Cover), 0, "an empty chunk is not written");
    let back = decode_lattice(&pack.chunk(ChunkKind::LandH, 12, 4095).unwrap(), dm.len()).unwrap();
    assert_eq!(back, dm, "the lattice round trips, NODATA included");
    let m = pack.heights(ChunkKind::LandH, 12, 4095).unwrap();
    assert!(m[7].is_nan() && (m[0] - 4412.7).abs() < 1e-3, "decimetres to metres, NODATA to NaN: {} {}", m[0], m[7]);
    assert_eq!(pack.water(4095).unwrap(), rings, "water rings round trip");
    assert_eq!(pack.ways(4095).unwrap(), ways, "ways round trip");
    assert_eq!(pack.buildings(4095).unwrap(), bs, "buildings round trip");
    assert!(pack.water(4094).is_none(), "a tile with no chunk has none");
}

/// The same input writes the same bytes, and the build id changes with any chunk (the builder's
/// byte-identical rebuild rests on this). Seen red 2026-11-09 with `finish` skipping its sort:
/// "a pack written from chunks added in another order is byte-identical".
#[test]
fn the_same_input_writes_the_same_bytes() {
    let (a, ..) = sample_pack();
    let (b, ..) = sample_pack();
    assert_eq!(a, b);
    let two = |first_ways: bool| {
        let mut w = PackWriter::new();
        if first_ways {
            w.add(ChunkKind::Ways, 12, 4095, encode_ways(&[]));
        }
        w.add(ChunkKind::LandH, 12, 4095, encode_lattice(&[1, 2, 3]));
        if !first_ways {
            w.add(ChunkKind::Ways, 12, 4095, encode_ways(&[]));
        }
        w
    };
    assert_eq!(two(true).finish(&manifest()).unwrap(), two(false).finish(&manifest()).unwrap(), "a pack written from chunks added in another order is byte-identical");
    let h1 = two(true).content_hash();
    assert_eq!(h1, two(false).content_hash());
    let mut w3 = PackWriter::new();
    w3.add(ChunkKind::LandH, 12, 4095, encode_lattice(&[1, 2, 4]));
    w3.add(ChunkKind::Ways, 12, 4095, encode_ways(&[]));
    assert_ne!(w3.content_hash(), h1, "one changed sample changes the build id");
}

/// A broken file is refused, never half-read. Seen red 2026-11-09 with the chunk-range check
/// removed: "a chunk past the end of the file refuses the pack".
#[test]
fn a_broken_pack_is_refused() {
    let (bytes, ..) = sample_pack();
    assert!(Pack::open(bytes[..bytes.len() - 1].to_vec()).is_err(), "a chunk past the end of the file refuses the pack");
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert!(Pack::open(bad).is_err(), "no magic");
    assert!(Pack::open(b"HOSMPAK1".to_vec()).is_err(), "no manifest");
    assert_eq!(decode_lattice(&encode_lattice(&[1, 2, 3]), 2), None, "a lattice of the wrong length");
    assert_eq!(decode_water(&[1, 9]), None, "an unknown ring kind");
}

/// Varints at their limits, and metres to decimetres. Seen red 2026-11-09 with `unzig` written
/// `(v >> 1) as i64 ^ (v & 1) as i64`: "-1 survives the zigzag; left: Some(1), right: Some(-1)".
#[test]
fn varints_and_decimetres() {
    for v in [0i64, 1, -1, 63, -64, 64, 1 << 40, -(1 << 40), i64::MAX, i64::MIN] {
        let mut out = Vec::new();
        put_svarint(&mut out, v);
        let mut pos = 0;
        assert_eq!(get_svarint(&out, &mut pos), Some(v), "{v} survives the zigzag");
        assert_eq!(pos, out.len());
    }
    assert_eq!(get_varint(&[0x80], &mut 0), None, "a varint cut short");
    assert_eq!(metres_to_dm(4412.74), 44127);
    assert_eq!(metres_to_dm(-0.05), -1, "rounded, not truncated (-0.05 m is -0.5 dm, away from zero)");
    assert_eq!(metres_to_dm(f64::NAN), NODATA);
}

/// The file name names the tile and comes back to it. Seen red 2026-11-09 with the digits written
/// deepest level first: "one digit per level from the root face down; left: f07-103312.hmpack".
#[test]
fn pack_names_round_trip() {
    let id = PatchId { face: 7, depth: 6, path: 0b01_00_11_11_01_10 };
    let name = pack_file_name(&id);
    assert_eq!(name, "f07-213301.hmpack", "one digit per level from the root face down");
    assert_eq!(parse_pack_file_name(&name), Some(id), "{name}: the name comes back to its tile");
    assert_eq!(parse_pack_file_name("f07-213301"), Some(id));
    assert_eq!(parse_pack_file_name("f20-0"), None, "there are 20 faces");
    assert_eq!(parse_pack_file_name("f07-2134"), None, "a digit is 0 to 3");
}
