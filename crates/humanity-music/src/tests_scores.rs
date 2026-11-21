//! The three public-domain pieces the score player ships (docs/design/score-player.md section 16,
//! rung 2; converted by scripts/ly-to-mei.js from the Mutopia Project files the dated finding allows:
//! docs/reference/findings/2026-11-20-public-domain-scores.md, findings 16 and 20). Each is read and
//! timed by the real reader and held to something measured outside the code: its bar count as the
//! edition numbers it (the symphony's 502 bars with the exposition's repeat at bar 124), its length
//! at its written tempo worked out by hand from its bars, and its opening notes as anyone who knows the
//! piece knows them. Their loudness is held by tests_written's `the_shipped_pieces_sound_at_the_music_target`,
//! which renders every piece of the index through the engine.

use crate::mei::{read, MeiScore};
use crate::scores::load_piece;
use crate::tests::{data_dir, shipped};
use crate::written::Timeline;

fn file(id: &str) -> MeiScore {
    let text = std::fs::read_to_string(data_dir().join("music").join("scores").join(format!("{id}.mei"))).unwrap();
    read(&text).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The timeline as the game times it (the index's tempo before the file's first mark).
fn timeline(id: &str) -> Timeline {
    let lib = shipped();
    load_piece(&data_dir(), lib.score(id).unwrap_or_else(|| panic!("the index lists {id}"))).unwrap().timeline.as_ref().clone()
}

/// The first `n` sounding notes of a staff (MIDI), in order.
fn opening(s: &MeiScore, staff: u16, n: usize) -> Vec<f32> {
    s.notes.iter().filter(|x| x.staff == staff && !x.grace).take(n).map(|x| x.midi).collect()
}

/// Nothing skipped, no problem, and only notation the player does not sound (slurs, clefs) unplayed.
fn reads_whole(id: &str, s: &MeiScore) {
    assert!(s.skipped.is_empty(), "{id}: skipped {:?}", s.skipped);
    assert!(s.problems.is_empty(), "{id}: problems {:?}", s.problems);
    let unplayed: Vec<&String> = s.not_played.keys().filter(|k| !["slur", "clef"].contains(&k.as_str())).collect();
    assert!(unplayed.is_empty(), "{id}: not played {unplayed:?}");
    assert!(s.availability.starts_with("Public domain."), "{id}: {}", s.availability);
    assert_eq!(s.meiversion, "5.1+basic");
}

/// FÜR ELISE READS WHOLE AND PLAYS ITS 106 BARS, BOTH REPEATS, IN 156.25 S AT A QUARTER = 72: the
/// file's 106 written bars (a pickup, two pairs of endings) play as 127 (the first section of 8 bars
/// twice with its two endings, the second of 13 twice with its two, then 81 bars); by hand, 375/8
/// whole notes (2 x 22/8 + 2/8 + 3/8, then 2 x 39/8 + 6/8, then 80 bars of 3/8 and the last of 2/8)
/// at 72 quarter notes a minute last 156.25 s; and it opens E D sharp E D sharp E B D C A (76 75 76 75
/// 76 71 74 72 69). It passed on its first run; RED 2026-11-20 with the reader playing the bars in
/// the order written (repeats not expanded): "left: (106, 106), right: (106, 127)".
#[test]
fn fur_elise_reads_whole_and_plays_both_repeats() {
    let s = file("fur_elise");
    reads_whole("fur_elise", &s);
    assert_eq!(s.title, "Für Elise (Clavierstück in A minor, WoO 59)");
    assert_eq!(s.composer, "Ludwig van Beethoven");
    assert_eq!((s.written_bars, s.bars.len()), (106, 127));
    assert_eq!(opening(&s, 1, 9), vec![76.0, 75.0, 76.0, 75.0, 76.0, 71.0, 74.0, 72.0, 69.0]);
    let t = timeline("fur_elise");
    assert!((t.length_s - 156.25).abs() < 1e-6, "{} s", t.length_s);
}

/// THE MOUNTAIN KING READS WHOLE: 88 BARS, PIÙ VIVO FROM BAR 50, 147.74 S: Grieg's piano version has
/// no repeat, so its 88 bars play once; its first bar's fermata holds the opening chord twice its
/// whole note; by hand, bar 1 (8 quarters held) and bars 2 to 49 (192) at 138, bars 50 to 88 (156)
/// at 154, last 200 x 60/138 + 156 x 60/154 = 147.736 s; the chord is F sharp 3 and 4 (54, 66), the
/// theme climbs B C sharp D E F sharp D F sharp from B1 (35 37 38 40 42 38 42) over the left hand's B0
/// and F sharp 1 (23 30); the right hand's two voices are two layers. RED 2026-11-20 on its first
/// run, a reader bug it found (a "cresc." ending on a rest in a later bar never closed, fixed and held
/// by tests_written2's hairpin test): "mountain_king: problems [\"a line from 84.000 whole notes waits
/// for Id(\\\"r757\\\"), which never came: applied to the end\"]".
#[test]
fn the_mountain_king_reads_whole_and_speeds_up_at_bar_50() {
    let s = file("mountain_king");
    reads_whole("mountain_king", &s);
    assert_eq!(s.composer, "Edvard Grieg");
    assert_eq!((s.written_bars, s.bars.len()), (88, 88));
    assert_eq!(opening(&s, 1, 9), vec![54.0, 66.0, 35.0, 37.0, 38.0, 40.0, 42.0, 38.0, 42.0]);
    assert_eq!(opening(&s, 2, 4), vec![23.0, 30.0, 23.0, 30.0]);
    assert!(s.notes.iter().any(|n| n.staff == 1 && n.layer == 2), "the right hand's second voice");
    assert!(s.notes.iter().any(|n| n.grace), "the closing grace-note runs play");
    let t = timeline("mountain_king");
    assert_eq!((t.bars[48].qpm, t.bars[49].qpm), (138.0, 154.0));
    let want = 200.0 * 60.0 / 138.0 + 156.0 * 60.0 / 154.0;
    assert!((t.length_s - want).abs() < 1e-6, "{} s, want {want}", t.length_s);
}

/// THE SYMPHONY'S FIRST MOVEMENT READS WHOLE: 502 BARS, THE EXPOSITION (BARS 1 TO 124) TWICE, THE
/// OBOE'S ADAGIO CADENZA IN BAR 268: twelve staves; 626 bars played; violins I and the double basses
/// open with the four-note motto, G G G E flat then F F F D (67 67 67 63 65 65 65 62), the basses an
/// octave below where they read (43 43 43 39); the cadenza bar lasts three whole notes as the edition
/// lays it out and plays at 60 quarter notes a minute from the oboe's Adagio (its fourth beat). By hand, without the fermatas: 625 bars of
/// a half and the cadenza bar of three wholes are 1,262 quarters, 350.56 s at 216, and the Adagio's
/// eight quarters at 60 add 5.78 s, so the movement lasts more than 356.33 s and, with its fermatas
/// held, less than 400. RED 2026-11-20 on its first run, a converter bug it found (no layer written
/// out to the cadenza bar's line, so the reader took the bar as its longest layer, dropping the
/// source's closing half bar): "left: Frac { n: 5, d: 2 }, right: Frac { n: 3, d: 1 }".
#[test]
fn the_symphony_reads_whole_with_its_repeat_and_its_cadenza() {
    let s = file("beethoven_symphony5_1");
    reads_whole("beethoven_symphony5_1", &s);
    assert_eq!(s.staves.len(), 12);
    assert_eq!((s.written_bars, s.bars.len()), (502, 626));
    assert_eq!(s.bars[124].n, "1", "after bar 124 the exposition again");
    assert_eq!(opening(&s, 8, 8), vec![67.0, 67.0, 67.0, 63.0, 65.0, 65.0, 65.0, 62.0]);
    assert_eq!(opening(&s, 12, 4), vec![43.0, 43.0, 43.0, 39.0]);
    let cadenza = s.bars.iter().find(|b| b.n == "268").expect("bar 268");
    assert_eq!(cadenza.len, crate::mei::Frac::int(3));
    let t = timeline("beethoven_symphony5_1");
    let map = crate::written::TempoMap::with_holds(&s.tempos, 216.0, &s.fermatas);
    assert_eq!(map.qpm(cadenza.at.add(crate::mei::Frac::new(1, 2))), 60.0, "the Adagio cadenza");
    assert!(t.length_s > 356.33 && t.length_s < 400.0, "{} s", t.length_s);
}

/// EVERY WRITTEN PIECE CARRIES ITS CATALOGUE DETAILS (docs/design/music-page.md section 10: the
/// Written music list a person searches, sorts and filters): its era with words, the year it was
/// written, the composer's years, a key the page can name (a note and a mode with words), the tempo's
/// basis with words (and its finding, when it names one, on disk), a source page that is an https
/// address when it gives one, and its LENGTH as the game plays it once through, within a second of
/// the file's own timeline (so the list's Length and its sort by length are the music's, not a guess).
///
/// Seen RED 2026-11-20 with every `length_s` left at 0.0 in the index: "ode_to_joy: its length is 0
/// s in the index, the file plays 32 s", and the same for Für Elise (156.25 s), the Mountain King
/// (147.74 s) and the symphony (365.11 s).
#[test]
fn every_written_piece_carries_its_catalogue_details() {
    let lib = shipped();
    let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data_dir().join("i18n/en.json")).unwrap()).unwrap();
    let has = |key: &str| {
        let mut v = &en;
        for part in key.split('.') {
            v = &v[part];
        }
        v.as_str().is_some_and(|s| !s.is_empty())
    };
    let repo = data_dir().parent().unwrap().to_path_buf();
    let mut wrong = Vec::new();
    for e in &lib.scores {
        let id = &e.id;
        if !(has(&e.era_words) || !e.era.is_empty()) {
            wrong.push(format!("{id}: its era {:?} has no words", e.era_words));
        }
        if e.year == 0 || e.composer_years.is_empty() {
            wrong.push(format!("{id}: no year written or composer's years"));
        }
        if (0..12).all(|pc| crate::sheet::pc_name(pc, false) != e.key_root && crate::sheet::pc_name(pc, true) != e.key_root) ||!has(&format!("music.mode.{}", e.key_mode)) {
            wrong.push(format!("{id}: the key {:?} {:?} cannot be named", e.key_root, e.key_mode));
        }
        if !has(&e.tempo_words) {
            wrong.push(format!("{id}: the tempo's basis {:?} has no words", e.tempo_words));
        }
        if !e.tempo_finding.is_empty() && !repo.join(&e.tempo_finding).is_file() {
            wrong.push(format!("{id}: its tempo finding {} is not on disk", e.tempo_finding));
        }
        if !e.source_url.is_empty() && !e.source_url.starts_with("https://") {
            wrong.push(format!("{id}: its source {:?} is not an https address", e.source_url));
        }
        let plays = load_piece(&data_dir(), e).unwrap_or_else(|x| panic!("{id}: {x}")).timeline.length_s;
        if (e.length_s as f64 - plays).abs() > 1.0 {
            wrong.push(format!("{id}: its length is {} s in the index, the file plays {plays} s", e.length_s));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
