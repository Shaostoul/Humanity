//! EVERY HOME VANTAGE STANDS IN THE ROOM IT NAMES (2026-11-15). scripts/home-vantages.json is where
//! scripts/photograph-home.js stands to photograph each room of the homestead, one row per room,
//! each naming its room by the label the shipped design gives it (data/homes/shipped/homestead.ron's
//! zones). When the quiet wing went upstairs (2026-10-20, the bedroom, dressing room, bathroom, wet
//! room and study each up 3.0 m with their rooms) the six rows kept their eye at 1.7 m, so each
//! photographed the ground-floor room under it: `10-bedroom` stood in the dining room, `15-study`
//! in the boot room. Nothing read the rows against the layout. Now each row's point is held to its
//! room: inside the zone of that label, its eye between that zone's floor and its top, and inside no
//! piece of the family's furniture (data/machines/home.ron) at the eye's height; a row for the whole
//! acre stands outside the home's box or over its roof.
//!
//! Seen red 2026-11-15 on the rows as they were: six, "10-bedroom (Bedroom) at (40.60, 1.70, 37.00)
//! is in Dining room, not Bedroom" first among them.

use crate::crowd_nav::Footprint;
use crate::machines::{MachineHome, ZoneRect};
use crate::ship_structure::HomeDesign;
use std::collections::{BTreeSet, HashMap};

/// The label the rows use for a vantage outside the home (the overviews).
const WHOLE_ACRE: &str = "the whole acre";

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"))
}

/// One row: its id, the room it names, and where the eye is (home metres).
struct Vantage {
    id: String,
    room: String,
    pos: (f32, f32, f32),
}

fn vantages(text: &str) -> Vec<Vantage> {
    let v: serde_json::Value = serde_json::from_str(text).expect("scripts/home-vantages.json parses");
    v["vantages"]
        .as_array()
        .expect("it lists vantages")
        .iter()
        .map(|r| {
            let p = r["pos"].as_array().expect("each vantage has a pos");
            let n = |i: usize| p[i].as_f64().expect("pos is three numbers") as f32;
            Vantage { id: r["id"].as_str().expect("an id").to_string(), room: r["room"].as_str().expect("a room").to_string(), pos: (n(0), n(1), n(2)) }
        })
        .collect()
}

/// What is wrong with each row against the design and its furniture.
fn problems(rows: &[Vantage], design: &HomeDesign, home: &MachineHome) -> Vec<String> {
    let b = &design.body;
    let zones = vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (b.width, b.depth, b.height) }];
    let in_home: BTreeSet<String> = home.all_instances().into_iter().filter(|i| i.zone == "home").map(|i| i.id).collect();
    let pieces: Vec<_> = home.placements(&HashMap::new(), Some(&zones)).into_iter().filter(|p| in_home.contains(&p.id)).collect();
    let mut out = Vec::new();
    for v in rows {
        let (x, y, z) = v.pos;
        let at = format!("{} ({}) at ({x:.2}, {y:.2}, {z:.2})", v.id, v.room);
        if v.room == WHOLE_ACRE {
            let inside = x > 0.0 && x < b.width && z > 0.0 && z < b.depth && y < b.height;
            if inside {
                out.push(format!("{at} is inside the home's box, not outside it"));
            }
            continue;
        }
        let named: Vec<_> = b.zones.iter().filter(|zn| zn.label == v.room).collect();
        if named.is_empty() {
            out.push(format!("{at} names a room the homestead has no zone labelled"));
            continue;
        }
        let holds = |zn: &crate::home_structure::Zone| zn.contains_xz(x, z) && y > zn.origin.1 && y <= zn.origin.1 + zn.size.1;
        if !named.iter().any(|zn| holds(zn)) {
            let actual: Vec<&str> = b.zones.iter().filter(|zn| holds(zn)).map(|zn| zn.label.as_str()).collect();
            out.push(format!("{at} is in {}, not {}", if actual.is_empty() { "no room".to_string() } else { actual.join(", ") }, v.room));
            continue;
        }
        for p in &pieces {
            let f = Footprint::of_placed(p);
            if f.contains(x, z) && y > f.y0 && y < f.y1 {
                out.push(format!("{at} is inside {} ({})", p.id, p.label));
            }
        }
    }
    out
}

fn shipped() -> (Vec<Vantage>, HomeDesign, MachineHome) {
    let rows = vantages(&std::fs::read_to_string(repo().join("scripts").join("home-vantages.json")).expect("scripts/home-vantages.json reads"));
    let text = std::fs::read_to_string(repo().join("data").join("homes").join("shipped").join("homestead.ron")).expect("the shipped homestead reads");
    let design: HomeDesign = ron::from_str(&text).expect("the shipped homestead parses");
    let home = MachineHome::load(&repo().join("data").join("machines").join("home.ron")).expect("data/machines/home.ron parses");
    (rows, design, home)
}

#[test]
fn every_home_vantage_stands_in_the_room_it_names() {
    let (rows, design, home) = shipped();
    assert!(rows.len() >= 20, "the vantages read: {}", rows.len());
    let p = problems(&rows, &design, &home);
    assert!(p.is_empty(), "{} vantage(s) out of their rooms:\n{}", p.len(), p.join("\n"));

    // The bedroom's row back at the eye it had: the dining room under it.
    let mut moved: Vec<Vantage> = rows.into_iter().filter(|v| v.id == "10-bedroom").collect();
    assert_eq!(moved.len(), 1, "the bedroom's row is listed");
    moved[0].pos.1 = 1.7;
    let red = problems(&moved, &design, &home);
    assert!(red.iter().any(|m| m.contains("10-bedroom") && m.contains("is in Dining room, not Bedroom")), "a vantage on the wrong storey is caught: {red:?}");
}
