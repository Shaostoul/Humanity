// Child of engine/screens.rs (#[path]): a wall screen's chat videos heard from the screen (BUG-258),
// through `place_page_sound`, the step `frame_surfaces` runs every frame for every page screen.
// Here, not beside the chat's other video tests, because they reach the engine (the screen's
// placement) and the gui crate may not (scripts/check-crate-layers.js); the engine reaches down
// to the chat's test helpers instead. Each test was seen failing once on purpose (recorded at it).

use crate::gui::pages::chat::inline_video::tests::{fetch_copies, media_fixture, Chat, Wall, A};
use crate::gui::widgets::video_view::{SoundPlace, VideoView};

/// A WALL SCREEN'S VIDEO IS HEARD FROM THE SCREEN, AT ITS VOLUME AND MUTE (BUG-258), the mix
/// decided without a sound device: a screen 10 m ahead is at 0.8 of full (the in-world screens'
/// falloff to silence at 50 m), on the world-sound bus under the master volume, centred; to the
/// listener's right it pans right; the chat's volume bar halves it, Mute silences it. The app
/// screen's set is centred at the master volume.
///
/// Seen red 2026-11-06 with `VideoView::mix` ignoring the place (the app's mix for both, as every
/// video was mixed before): "a screen 10 m ahead: master x sfx x 0.8, centred (got 0.5, 0.5)".
#[test]
fn a_wall_screens_video_is_mixed_from_the_screen_with_its_volume_and_mute() {
    use super::video::audio_placement;
    use glam::Vec3;
    let listener = (Vec3::new(0.0, 1.7, 0.0), Vec3::X);
    let place = |screen: Vec3| {
        let (falloff, pan) = audio_placement(screen, listener.0, listener.1);
        SoundPlace::World { falloff, pan }
    };
    let mut view = VideoView::default();
    let ahead = place(Vec3::new(0.0, 1.7, -10.0));
    let (v, p) = view.mix(ahead, 0.5, 0.4);
    assert!((v - 0.5 * 0.4 * 0.8).abs() < 1e-6 && (p - 0.5).abs() < 1e-6, "a screen 10 m ahead: master x sfx x 0.8, centred (got {v}, {p})");
    let (_, right) = view.mix(place(Vec3::new(10.0, 1.7, 0.0)), 0.5, 0.4);
    assert!(right > 0.75, "a screen to the right pans right: {right}");
    view.volume = 0.5;
    assert!((view.mix(ahead, 0.5, 0.4).0 - 0.08).abs() < 1e-6, "the volume bar halves it");
    view.muted = true;
    assert_eq!(view.mix(ahead, 0.5, 0.4).0, 0.0, "Mute silences it");
    view.muted = false;
    view.volume = 1.0;
    assert_eq!(view.mix(SoundPlace::App, 0.5, 0.4), (0.5, 0.5), "the app screen's: the master volume, centred");
}

/// THE ROW'S OWN VOLUME IS APPLIED ONCE (BUG-258, after 2026-11-06's `compose_mix` took the sound's
/// own volume as an argument, for the in-world screens' strip bar): a wall screen's chat video with
/// its row's volume bar at 0.5, the screen 10 m ahead, is mixed at exactly 0.5 x master x the world
/// bus x falloff, the same as `compose_mix` with that `own`, never 0.25 x (the row's volume going in
/// as `own` AND multiplied on top).
///
/// Seen red 2026-11-06 with `VideoView::mix` multiplying the row's volume on top of
/// `compose_mix(master, sfx, own, falloff)`: "the row's volume once: 0.5 x master x sfx x falloff
/// (got 0.0840, want 0.1680)" (the test above went red too: "the volume bar halves it").
#[test]
fn a_wall_screens_video_at_half_row_volume_is_mixed_at_half_not_a_quarter() {
    use glam::Vec3;
    let (falloff, pan) = super::video::audio_placement(Vec3::new(0.0, 1.7, -10.0), Vec3::new(0.0, 1.7, 0.0), Vec3::X);
    let (master, sfx) = (0.6, 0.7);
    let mut view = VideoView::default();
    view.volume = 0.5;
    let (got, _) = view.mix(SoundPlace::World { falloff, pan }, master, sfx);
    let want = 0.5 * master * sfx * falloff;
    assert!((got - want).abs() < 1e-12, "the row's volume once: 0.5 x master x sfx x falloff (got {got:.4}, want {want:.4})");
    assert_eq!(got, crate::media::compose_mix(master, sfx, 0.5, falloff), "the same sum the in-world screens use");
}

/// A WALL SCREEN'S PLAYING VIDEO IS HANDED TO THE MIXER, FROM THE SCREEN, AT ITS VOLUME AND MUTE
/// (BUG-258; they had always played silent, their Mute and volume doing nothing): the screens den's
/// wall screen plays the AV1 fixture (its Opus sound), and the engine's per-frame step for a page
/// screen (`engine::screens::place_page_sound`, the screen 10 m ahead of the listener) attaches
/// its sound at master x sfx x falloff, centred; the volume bar at half sends half; Mute sends
/// silence. The master is 0.02, so nothing is heard. Needs a sound device, so ignored by default:
/// `node scripts/lib-tests.js screens::sound_tests::a_wall_screens_playing_video --ignored --nocapture`.
///
/// Seen red 2026-11-06 with `place_page_sound` doing nothing for a page screen (as the engine's
/// loop skipped every screen without a provider): "its sound is handed to the mixer".
#[test]
#[ignore = "needs an audio output device; run: node scripts/lib-tests.js screens::sound_tests::a_wall_screens_playing_video --ignored --nocapture"]
fn a_wall_screens_playing_video_is_handed_to_the_mixer_with_its_volume_and_mute() {
    use glam::Vec3;
    let mut audio = match humanity_core::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    audio.set_master_volume(0.02);
    audio.set_sfx_volume(0.5);
    let mut chat = Chat::new("chat_video_wall_sound", &[&format!("on the wall {A}")], 1280.0, 720.0);
    let mut wall = Wall::new(fetch_copies(media_fixture("sliding-bar-av1-opus.mp4"), chat.dir.to_path_buf()));
    let a = Chat::url(A);
    for _ in 0..3 {
        wall.frame(&mut chat.state);
    }
    let at = wall.core.chat_videos().pictures[&a].center();
    wall.click(&mut chat.state, at);
    let (_, held) = wall.frames_until(&mut chat.state, |core, _| core.chat_videos().is_playing(&Chat::url(A)));
    assert!(held, "it plays on the wall");
    let (screen, listener, right) = (Vec3::new(0.0, 1.7, -10.0), Vec3::new(0.0, 1.7, 0.0), Vec3::X);
    let (falloff, pan) = super::video::audio_placement(screen, listener, right);
    let near = |got: Option<(f64, f64)>, want: (f64, f64)| got.is_some_and(|(v, p)| (v - want.0).abs() < 1e-9 && (p - want.1).abs() < 1e-9);

    super::place_page_sound(&mut wall.core, screen, listener, right, &mut audio);
    assert!(wall.core.chat_videos().player(&a).is_some_and(|p| p.has_audio()), "its sound is handed to the mixer");
    let sent = wall.core.chat_videos().mix_sent(&a);
    assert!(near(sent, (0.02 * 0.5 * falloff, pan)), "at master x sfx x falloff, from the screen: {sent:?}");
    wall.core.chat_videos().volume = 0.5;
    super::place_page_sound(&mut wall.core, screen, listener, right, &mut audio);
    let sent = wall.core.chat_videos().mix_sent(&a);
    assert!(near(sent, (0.02 * 0.5 * falloff * 0.5, pan)), "the volume bar at half: {sent:?}");
    wall.core.chat_videos().muted = true;
    super::place_page_sound(&mut wall.core, screen, listener, right, &mut audio);
    assert_eq!(wall.core.chat_videos().mix_sent(&a), Some((0.0, pan)), "Mute: silence");
}
