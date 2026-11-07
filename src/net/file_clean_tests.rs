//! Tests for file_clean.rs: a file is cleaned on the sender's device with the relay's own rules,
//! the attach step's look agrees with the clean, and a big file is cleaned in a copy, never itself.

use super::*;
use humanity_protocol::describe_metadata::describe;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/describe").join(name)).expect("the fixture is there")
}

/// Pictures and videos that say where they were taken come out saying nothing; a video keeps its
/// length. Seen red 2026-10-11 with `clean_bytes` not treating a picture as one (it fell through to
/// the rule for other files): the assertion "{file}: {c:?}" failed on jpeg-phone.jpg (not changed).
#[test]
fn a_phone_photo_and_a_phone_video_leave_saying_nothing() {
    for (file, name) in [("jpeg-phone.jpg", "IMG_0001.JPG"), ("png-exif.png", "a.png"), ("webp-exif.webp", "a.webp"), ("mov-apple.mov", "a.mov"), ("3gp-loci.3gp", "a.3gp"), ("mp4-xmp-uuid.mp4", "a.mp4")] {
        let before = fixture(file);
        assert!(describe(&before, name).has_location, "{file}: test setup, it says where");
        let (after, c) = clean_bytes(before.clone(), name);
        assert!(c.changed && !c.unchecked, "{file}: {c:?}");
        let said: Vec<&str> = describe(&after, name).details.iter().map(|d| d.key).collect();
        assert!(said.is_empty(), "{file}: still says {said:?}");
        if !file.ends_with(".jpg") && !file.ends_with(".png") && !file.ends_with(".webp") {
            assert_eq!(after.len(), before.len(), "{file}: a video keeps its length");
        }
    }
}

/// A GIF LEAVES WITHOUT ITS COMMENT AND ITS XMP, IN EVERY UPLOAD PATH (2026-11-07, BUG-263): the
/// desktop's own clean runs the relay's GIF strip, so the looping animation (the shared fixture,
/// also the website's) comes out exactly as the strip gives it, its loop and frames untouched,
/// saying nothing; a cleaned copy of a big one (Add to Files) says nothing either; a GIF cut off
/// part way could not be checked, and the look agrees; a whole GIF with nothing in it goes as it is.
///
/// Seen red 2026-11-07 with GIF passed through as before (`picture_type` without "gif" and
/// `clean_bytes` sending it on as it came): "gif-comment-xmp.gif: Cleaned { changed: false,
/// unchecked: false }".
#[test]
fn a_gif_leaves_without_its_comment_and_xmp() {
    let before = fixture("gif-comment-xmp.gif");
    let d = describe(&before, "anim.gif");
    assert!(d.has_location && d.details.iter().any(|x| x.key == "comment"), "test setup: it says where, and has a comment");
    let (after, c) = clean_bytes(before.clone(), "anim.gif");
    assert!(c.changed && !c.unchecked, "gif-comment-xmp.gif: {c:?}");
    assert_eq!(Some(after.clone()), humanity_protocol::strip_metadata::try_strip_image_metadata("image/gif", &before), "the relay's own strip, to the byte");
    assert!(describe(&after, "anim.gif").details.is_empty(), "it leaves saying nothing");
    assert!(after.windows(11).any(|w| w == b"NETSCAPE2.0"), "the loop stays");
    let dir = std::env::temp_dir().join(format!("hum_test_gif_copy_{}", std::process::id()));
    let src = dir.join("anim.gif");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&src, &before).unwrap();
    let (copy, cc) = clean_copy(&src, "anim.gif", &dir).unwrap();
    let copy = copy.expect("a cleaned copy");
    assert!(cc.changed && !cc.unchecked);
    assert_eq!(std::fs::read(&copy).unwrap(), after, "the copy is the same cleaned GIF");
    assert_eq!(std::fs::read(&src).unwrap(), before, "the person's own file is untouched");
    let _ = std::fs::remove_dir_all(&dir);
    for file in ["gif-cut.gif", "gif-junk.gif"] {
        let b = fixture(file);
        let (out, c) = clean_bytes(b.clone(), "g.gif");
        assert!(c.unchecked && out == b, "{file} could not be checked and goes on as it came: {c:?}");
        assert!(look_bytes(&b, "g.gif").unchecked, "{file}: the look says so too");
    }
    let plain = fixture("gif.gif");
    assert_eq!(clean_bytes(plain.clone(), "g.gif"), (plain, Cleaned::default()), "a GIF with nothing in it goes as it is");
}

/// A HEIC photo (an iPhone's, built byte by byte by the HEIF strip's own test builder) is cleaned on
/// the desktop with the relay's HEIF strip: its Exif item zeroed in place. Seen red 2026-10-11 with
/// the HEIF strip handed nothing to strip: the assertion "{c:?}" failed (not changed, unchecked).
#[test]
fn a_heic_photo_is_cleaned_in_place() {
    let built = humanity_protocol::strip_heif_metadata::build::iphone_heic();
    let place = humanity_protocol::strip_heif_metadata::build::PLACE;
    let has = |b: &[u8]| b.windows(place.len()).any(|w| w == place);
    assert!(has(&built.bytes), "test setup");
    let (after, c) = clean_bytes(built.bytes.clone(), "IMG_0005.HEIC");
    assert!(c.changed && !c.unchecked, "{c:?}");
    assert!(!has(&after), "the place is gone");
    assert_eq!(after.len(), built.bytes.len());
}

/// Files that could not be checked (answer 5), by the relay's own verdict: a picture that cannot
/// be walked comes back as it was; a video whose boxes stop early or that has no movie box; a ".jpg"
/// that is not a JPEG. Anything else (a text file, a whole GIF) is fine as it is.
/// Seen red 2026-10-11 with the no-movie case answered from the video strip alone (`unchecked:
/// !v.stopped_early.is_empty()`): "{file} could not be checked" failed (the movie with no movie box).
#[test]
fn what_could_not_be_checked() {
    for (file, name) in [("jpeg-lost-sync.jpg", "a.jpg"), ("png-cut.png", "a.png"), ("mp4-broken-box.mp4", "a.mp4"), ("mp4-no-moov.mp4", "a.mp4"), ("mp4-cut-140.mp4", "a.mp4"), ("named-jpeg-not-one.jpg", "IMG_0002.JPG")] {
        let before = fixture(file);
        let (after, c) = clean_bytes(before.clone(), name);
        assert!(c.unchecked, "{file} could not be checked");
        assert!(look_bytes(&before, name).unchecked, "{file}: the look says so too");
        if file.starts_with("jpeg") || file.starts_with("png") || file.starts_with("named") {
            assert_eq!(after, before, "{file}: a picture that cannot be walked is given back as it came");
        }
    }
    for (file, name) in [("other.txt", "notes.txt"), ("gif.gif", "g.gif"), ("webm.webm", "c.webm"), ("jpeg-clean.jpg", "a.jpg"), ("mp4-clean.mp4", "a.mp4")] {
        let b = fixture(file);
        assert!(!clean_bytes(b.clone(), name).1.unchecked, "{file} is fine as it is");
        assert!(!look_bytes(&b, name).unchecked, "{file}: the look agrees");
    }
}

/// The look from disk (what the attach step shows) agrees with the clean on every shared fixture.
/// Seen red 2026-10-11 with the look calling a GIF unchecked: "gif.gif".
#[test]
fn the_look_from_disk_agrees_with_the_clean() {
    let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/describe");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name == "expected.json" {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let look = look_file(&p, &name).unwrap();
        assert_eq!(look.unchecked, clean_bytes(bytes.clone(), &name).1.unchecked, "{name}");
        assert_eq!(look.description, describe(&bytes, &name), "{name}");
        n += 1;
    }
    assert!(n >= 50);
}

/// A big file is cleaned in a COPY: the person's own file is never written; a file with nothing to
/// remove is not copied at all; one that could not be checked is not copied either; and the sweep
/// takes only copies nothing uses. Seen red 2026-10-11 with `clean_copy` stripping `path` itself:
/// "the person's own file is never written".
#[test]
fn a_big_file_is_cleaned_in_a_copy_and_its_own_file_is_never_written() {
    let work = humanity_core::test_temp::dir("clean_copy");
    let out = work.join("cleaned");
    let own = work.join("IMG_0003.MOV");
    let before = fixture("mov-apple.mov");
    std::fs::write(&own, &before).unwrap();
    let (copy, c) = clean_copy(&own, "IMG_0003.MOV", &out).unwrap();
    let copy = copy.expect("a copy");
    assert!(c.changed && !c.unchecked);
    assert_eq!(std::fs::read(&own).unwrap(), before, "the person's own file is never written");
    assert!(describe(&std::fs::read(&copy).unwrap(), "x.mov").details.is_empty(), "the copy says nothing");
    assert!(copy.starts_with(&out));

    let clean = work.join("clean.mp4");
    std::fs::write(&clean, fixture("mp4-clean.mp4")).unwrap();
    assert_eq!(clean_copy(&clean, "clean.mp4", &out).unwrap(), (None, Cleaned::default()), "nothing to remove: no copy");
    let broken = work.join("broken.mp4");
    std::fs::write(&broken, fixture("mp4-broken-box.mp4")).unwrap();
    assert_eq!(clean_copy(&broken, "broken.mp4", &out).unwrap(), (None, Cleaned { changed: false, unchecked: true }));
    let photo = work.join("p.jpg");
    std::fs::write(&photo, fixture("jpeg-phone.jpg")).unwrap();
    let (pc, _) = clean_copy(&photo, "p.jpg", &out).unwrap();
    let pc = pc.expect("a cleaned picture is a copy too");

    assert_eq!(sweep_cleaned(&out, &[copy.clone()], std::time::Duration::from_secs(3600)), 0, "nothing idle yet");
    assert_eq!(sweep_cleaned(&out, &[copy.clone()], std::time::Duration::ZERO), 1, "the copy nothing uses goes");
    assert!(copy.exists() && !pc.exists());
}
