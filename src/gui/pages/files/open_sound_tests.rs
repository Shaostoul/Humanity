// Child of files/open.rs (#[path]): the Files page's video gets its sound from the app's mixer.
// Needs a sound device, so ignored by default; run by hand after touching `sync_sound`:
// node scripts/lib-tests.js open::sound_tests --ignored --nocapture

use super::*;

/// THE PAGE'S VIDEO GETS ITS SOUND, AND MUTE SILENCES IT (2026-10-18; it had played silent): the
/// first sync hands the sound to the mixer at the master volume; Mute sends silence; Unmute sends the
/// master volume again; a changed master volume follows. Paused throughout, so nothing is heard.
/// Seen red 2026-10-18 with Mute ignored: "muted: silence" (left: Some(0.3)).
#[test]
#[ignore = "needs an audio output device; run: node scripts/lib-tests.js open::sound_tests --ignored --nocapture"]
fn the_pages_video_gets_its_sound_and_mute_silences_it() {
    if !crate::media::system::available() {
        println!("skipped: this copy of Windows has no Media Foundation");
        return;
    }
    let mut audio = match crate::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    audio.set_master_volume(0.3);
    let mp4 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media/moving-box-h264-aac.mp4");
    let mut opened = Opened::new(Item::default());
    opened.video.player = Some(crate::media::VideoPlayer::open(&mp4).expect("open"));
    sync_sound(&mut opened, &mut audio);
    assert!(opened.video.player.as_ref().unwrap().has_audio(), "the sound is attached");
    assert_eq!(opened.video.volume_sent, Some(0.3), "at the master volume");
    opened.video.muted = true;
    sync_sound(&mut opened, &mut audio);
    assert_eq!(opened.video.volume_sent, Some(0.0), "muted: silence");
    opened.video.muted = false;
    audio.set_master_volume(0.2);
    sync_sound(&mut opened, &mut audio);
    assert_eq!(opened.video.volume_sent, Some(0.2), "unmuted, at the master volume as it is now");
}
