// Child of audio/sound_levels.rs (#[path]): the person's level for each sound, held process-wide.
// Each test runs on its own thread's levels (`use_on_this_thread`), so none sees another's. Every
// test was seen failing on purpose once; the break and what it said are at the test.

use super::*;

const CLICK: &str = "sfx.button_click";

fn db_of(gain: f64) -> f64 {
    20.0 * gain.log10()
}

/// A LEVEL MOVES A SOUND BY EXACTLY ITS dB: -6 dB is an amplitude of 10^(-6/20), +12 dB of
/// 10^(12/20), each reading back as its dB to a billionth; a level is kept on the half-dB grid and
/// inside -24..+12, and a number that is not one is never kept. RED first (2026-11-06): with
/// `db_to_gain` dividing by 10 instead of 20 (a power ratio taken for an amplitude), "-6 dB moves
/// the click by -6 dB: -12.000 dB".
#[test]
fn a_level_moves_a_sound_by_exactly_its_db() {
    use_on_this_thread(BTreeMap::new());
    assert!(set_level(CLICK, -6.0), "a new level is a change");
    let g = gain(CLICK);
    assert!((db_of(g) + 6.0).abs() < 1e-9, "-6 dB moves the click by -6 dB: {:.3} dB", db_of(g));
    assert!((g - 10f64.powf(-6.0 / 20.0)).abs() < 1e-12, "the amplitude is 10^(-6/20): {g}");
    set_level(CLICK, 12.0);
    assert!((db_of(gain(CLICK)) - 12.0).abs() < 1e-9, "+12 dB: {:.3}", db_of(gain(CLICK)));
    assert!(!set_level(CLICK, 12.0), "the same level again is no change");
    set_level(CLICK, 3.26);
    assert_eq!(level_db(CLICK), 3.5, "kept on the half-dB grid");
    set_level(CLICK, 40.0);
    assert_eq!(level_db(CLICK), MAX_DB, "held at the top");
    set_level(CLICK, -99.0);
    assert_eq!(level_db(CLICK), MIN_DB, "held at the bottom");
    assert!(!set_level(CLICK, f32::NAN), "NaN is never kept");
    assert_eq!(level_db(CLICK), MIN_DB, "and leaves the level as it was");
}

/// RESET RETURNS A SOUND TO ITS LEVELLED DEFAULT: after Reset the sound's level is 0 dB, its gain
/// exactly 1.0 (the sound as "Even out loudness" and its authored volume leave it) and it is gone
/// from what the config saves; setting it to 0 does the same; "Reset all" empties the lot. RED
/// first (2026-11-06): with `set_level` keeping a 0 entry, "a level set back to 0 is not kept"
/// failed (the config would have kept {"sfx.crash": 0.0}).
#[test]
fn reset_returns_a_sound_to_its_levelled_default() {
    use_on_this_thread(BTreeMap::new());
    set_level(CLICK, -9.0);
    assert!(reset(CLICK), "it had a level");
    assert_eq!((level_db(CLICK), gain(CLICK)), (0.0, 1.0), "back to 0 dB, exactly the levelled default");
    assert!(!all().contains_key(CLICK), "and nothing kept for it");
    assert!(!reset(CLICK), "a second reset changes nothing");
    set_level("sfx.crash", 4.0);
    set_level("sfx.crash", 0.0);
    assert!(all().is_empty(), "a level set back to 0 is not kept: {:?}", all());
    set_level(CLICK, -3.0);
    set_level("sfx.crash", 2.0);
    assert!(any());
    assert!(reset_all(), "Reset all had levels to reset");
    assert!(!any() && all().is_empty(), "Reset all empties every level");
    assert!(!reset_all(), "and a second one changes nothing");
}

/// A SOUND WITHOUT A LEVEL PLAYS AT ITS DEFAULT: with the click turned down, every other sound
/// (and one no catalogue names) still answers 0 dB and an amplitude of exactly 1.0. RED first
/// (2026-11-06): with `level_db` answering the last level set for any sound, "the crash, never
/// set, plays at its default" failed (left: -6.0, right: 0.0).
#[test]
fn a_sound_without_a_level_plays_at_its_default() {
    use_on_this_thread(BTreeMap::new());
    set_level(CLICK, -6.0);
    assert_eq!(level_db("sfx.crash"), 0.0, "the crash, never set, plays at its default");
    assert_eq!(gain("sfx.crash"), 1.0);
    assert_eq!(gain("mod.a_sound_no_catalogue_names"), 1.0);
}

/// The config's levels come back as they were kept, cleaned: a 0 and a number that is not one are
/// dropped, one out of range is held at the limit. RED first (2026-11-06): with `restore` taking
/// the map as it came, "a restored 0 is not kept" failed.
#[test]
fn the_levels_come_back_as_the_config_kept_them() {
    use_on_this_thread(BTreeMap::new());
    let kept: BTreeMap<String, f32> = [("sfx.a", -6.0), ("sfx.b", 0.0), ("sfx.c", f32::NAN), ("sfx.d", 40.0)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    restore(&kept);
    let back = all();
    assert_eq!(back.get("sfx.a"), Some(&-6.0));
    assert!(!back.contains_key("sfx.b"), "a restored 0 is not kept");
    assert!(!back.contains_key("sfx.c"), "nor a NaN");
    assert_eq!(back.get("sfx.d"), Some(&MAX_DB), "held at the top");
}
