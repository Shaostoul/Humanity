// Drawing the reaction pictures ahead, gradually (2026-11-20, docs/design/emoji.md "Drawn ahead,
// gradually"). The panel's first opening on the real chat page is
// `pages/chat/emoji_tests.rs` `the_panels_first_opening_draws_no_picture_on_the_frame`. Each test was
// seen failing once on purpose; the break and what it said are recorded at the test.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::mutex::RwLock;

use super::outline::Outline;
use super::pack::{pack_version, PackPictures, WarmKey};
use super::warm::{self, GAP_MS};

fn pack_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("assets").join("emoji")
}

fn textures() -> Arc<RwLock<egui::epaint::TextureManager>> {
    Arc::new(RwLock::new(egui::epaint::TextureManager::default()))
}

fn keys(files: &[&str]) -> Vec<WarmKey> {
    files.iter().map(|f| WarmKey { set: "openmoji".into(), file: (*f).into(), px: 24, outline: Outline::NONE }).collect()
}

/// DRAWING AHEAD IS GRADUAL, AND SO ARE THE UPLOADS: handed eight pictures, the worker draws them one
/// at a time with its rest after each (so the eight take at least seven rests, never a burst), the
/// same list is not handed over twice, each frame's upload takes at most the number it is allowed,
/// nothing is drawn on the asking thread, and in the end all eight are kept.
///
/// Seen red 2026-11-20 with the worker's rest taken out (no sleep in `draw_ahead`): "drawn in a
/// burst: 8 pictures in 8 ms (at least 105 ms wanted)".
#[test]
fn drawing_ahead_is_gradual_and_so_are_the_uploads() {
    let files = ["1F600", "1F602", "1F44D", "1F525", "1F331", "1F389", "1F64F", "1F622"];
    let pics = PackPictures::new(pack_dir(), textures());
    let version = pack_version();
    let started = Instant::now();
    assert!(pics.ensure_ahead(version, [1, 2, 3, 4, 5], || keys(&files)), "the list is handed to the worker");
    assert!(!pics.ensure_ahead(version, [1, 2, 3, 4, 5], || panic!("the same list is not made again")), "not handed over twice");
    // The worker's pace, read from what it drew.
    while pics.drawn_ahead() < files.len() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(1));
    }
    let took = started.elapsed();
    assert_eq!(pics.drawn_ahead(), files.len(), "each drawn once, ahead");
    let least = Duration::from_millis(GAP_MS * (files.len() as u64 - 1));
    assert!(took >= least, "drawn in a burst: {} pictures in {} ms (at least {} ms wanted)", files.len(), took.as_millis(), least.as_millis());
    // The frames' uploads, a few at a time.
    std::thread::sleep(Duration::from_millis(GAP_MS * 2)); // the last one handed over
    let mut uploaded = 0;
    let mut most = 0;
    for _ in 0..files.len() {
        let n = pics.upload_ahead(version, 3, Duration::from_millis(2));
        most = most.max(n);
        uploaded += n;
    }
    assert_eq!(uploaded, files.len(), "all eight were uploaded");
    assert!(most <= 3, "a frame uploads at most what it is allowed ({most})");
    assert_eq!(pics.drawn(), 0, "nothing was drawn on the asking thread");
    assert!(keys(&files).iter().all(|k| pics.is_kept(k, version)), "all kept");
    // Asked for now, each comes from what was kept: still nothing drawn on the asking thread.
    for f in files {
        assert!(pics.picture_with("openmoji", f, 24, Outline::NONE, true, version).is_some());
    }
    assert_eq!(pics.drawn(), 0, "the drawn-ahead pictures are the ones shown");
}

/// A PICTURE ASKED FOR BEFORE ITS UPLOAD USES THE DRAWN-AHEAD ONE: one the worker finished but no
/// frame uploaded yet is taken from the waiting list when the text asks for it, not drawn again.
///
/// Seen red 2026-11-20 with `picture_with` ignoring the waiting list: "a waiting picture is used,
/// not drawn again" (left: 1, right: 0).
#[test]
fn a_waiting_picture_is_used_not_drawn_again() {
    let pics = PackPictures::new(pack_dir(), textures());
    let version = pack_version();
    pics.ensure_ahead(version, [9, 9, 9, 9, 9], || keys(&["1F600"]));
    let started = Instant::now();
    while pics.waiting_len() == 0 && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(pics.waiting_len(), 1, "drawn ahead, waiting for a frame");
    assert!(pics.picture_with("openmoji", "1F600", 24, Outline::NONE, true, version).is_some());
    assert_eq!(pics.drawn(), 0, "a waiting picture is used, not drawn again");
    assert_eq!(pics.waiting_len(), 0, "and taken off the waiting list");
}

/// WITH "TURN OFF ALL REUSE" NOTHING IS DRAWN AHEAD, AND THE START DELAY HOLDS: on a context with the
/// pack's pictures, `warm::frame_with` with the switch off hands the worker nothing; with it on,
/// nothing before the start delay and the list after it; off again, the worker stops and what waits
/// is let go.
///
/// Seen red 2026-11-20 with `frame_with` not checking the switch: the first frame with it off
/// handed the worker its list (left: `Step { started: true, uploaded: 0 }`, right: the default).
#[test]
fn with_reuse_off_nothing_is_drawn_ahead() {
    let ctx = egui::Context::default();
    let pack = super::pack::pack_source(&ctx, &pack_dir()).expect("the shipped pack installs");
    let ahead = pack.pictures.as_pack().expect("the pack's pictures").clone();
    super::install(&ctx, Some(Arc::new(pack)));
    for _ in 0..3 {
        assert_eq!(warm::frame_with(&ctx, 16.0, 0.0, false), warm::Step::default(), "with reuse off nothing is handed to the worker");
    }
    std::thread::sleep(Duration::from_millis(GAP_MS * 4));
    assert_eq!(ahead.drawn_ahead(), 0, "with reuse off nothing is drawn ahead ({} drawn)", ahead.drawn_ahead());
    // On, and the start delay respected: nothing before it, the list handed over after it.
    assert_eq!(warm::frame_with(&ctx, 16.0, 3600.0, true), warm::Step::default(), "nothing before the start delay");
    assert!(warm::frame_with(&ctx, 16.0, 0.0, true).started, "the list is handed over once the delay has passed");
    // Off again: the worker stops and what waits is let go.
    warm::frame_with(&ctx, 16.0, 0.0, false);
    std::thread::sleep(Duration::from_millis(GAP_MS * 4));
    assert_eq!(ahead.waiting_len(), 0, "with reuse off, nothing waits");
}
