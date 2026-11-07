// Tests for engine/sfx.rs: the game's one-shot sounds on their buses (BUG-265), through the app's
// own AudioManager on kira's mock backend (no sound card, no sound, the numbers the app hands kira).

use std::collections::BTreeMap;
use std::path::Path;

use humanity_core::audio::sound_levels;
use humanity_core::audio::sounds::SoundCatalog;
use humanity_core::audio::AudioManager;

use super::{play_one, NOTICE_BUS};

fn repo() -> &'static Path {
    Path::new(env!("HUMANITY_REPO_ROOT"))
}

/// THE GAME'S CHIME AND SETTINGS' PLAY AGREE, AND THE CLICKS NEVER SILENCE IT (BUG-265): with the
/// UI Volume at 0, the sound-effects volume at 0.3 and the Notifications volume at 0.8, each chime
/// (the notification, the chat message, the bright "success") plays from the game's queue at
/// exactly the amplitude Settings > Audio > Sound levels' Play gives it, both through the
/// notifications volume (moving it moves both), and audible; a notice whose chosen sound is an
/// interface file (the "soft" menu-open sound) still plays, on the notifications bus; the item
/// pickup, an action's sound catalogued on the interface bus, follows the UI Volume to silence.
///
/// RED first (2026-11-07) against today's drain (`play_one` as lib.rs's queue was: every sound
/// through `play_sound_vol`, the sound-effects bus) and today's catalogue (the chimes on "ui"):
/// "sfx.notification: the game's queue plays it at Play's level (left: 0.029528577553540915,
/// right: 0.0)": the game on the sound-effects volume, Play on the UI Volume at 0.
#[test]
fn a_chime_plays_at_its_play_level_on_the_notifications_volume() {
    sound_levels::use_on_this_thread(BTreeMap::new());
    let catalog = SoundCatalog::load(&repo().join("data"));
    let assets = repo().join("assets");
    let mut audio = AudioManager::without_device();
    audio.set_data_dir(&repo().join("data"));
    audio.set_master_volume(0.5);
    audio.set_sfx_volume(0.3);
    audio.set_ui_volume(0.0);
    audio.set_notifications_volume(0.8);
    for id in ["sfx.notification", "sfx.chat_message", "sfx.success"] {
        let play = crate::gui::pages::settings::sound_levels::play(id, &assets, &mut audio, &catalog).expect("Play plays it");
        let game = play_one(&mut audio, &catalog, &assets, id, "", None).expect("the game plays it");
        assert_eq!(game, play, "{id}: the game's queue plays it at Play's level");
        assert_eq!(catalog.bus_or_default(id), NOTICE_BUS, "{id} is catalogued as a notice's chime");
        let notice = play_one(&mut audio, &catalog, &assets, id, "", Some(NOTICE_BUS)).expect("a notice plays it");
        assert_eq!(notice, play, "{id}: a notice plays it at Play's level");
        assert!(game > 0.01, "{id}: with the UI Volume at 0 the chime still sounds ({game})");
        audio.set_notifications_volume(0.4);
        let half = play_one(&mut audio, &catalog, &assets, id, "", None).unwrap();
        let half_play = crate::gui::pages::settings::sound_levels::play(id, &assets, &mut audio, &catalog).unwrap();
        assert!((half / game - 0.5).abs() < 1e-12 && half == half_play, "{id}: the Notifications volume moves the game and Play alike ({half} of {game})");
        audio.set_notifications_volume(0.8);
    }
    let soft = play_one(&mut audio, &catalog, &assets, "sfx.menu_open", "", Some(NOTICE_BUS)).unwrap();
    assert!(soft > 0.01, "a notice's soft sound plays on the notifications bus with the UI Volume at 0 ({soft})");
    let pickup = play_one(&mut audio, &catalog, &assets, "sfx.inventory_pickup", "", None).unwrap();
    assert_eq!(pickup, 0.0, "the item pickup follows the UI Volume, its catalogue bus");
}

/// Every bus in the catalogue is one the mixer has a slider for, and an unknown bus is the
/// sound-effects volume: "ambient" follows the ambience volume (BUG-271: it fell through to the
/// sound-effects volume, and the ambience volume was never read). RED first (2026-11-07) with
/// `bus_volume` lacking its "ambient" arm: "the ambient bus is the ambience volume (left: 0.3,
/// right: 0.6)".
#[test]
fn every_bus_has_its_own_volume() {
    let mut audio = AudioManager::without_device();
    audio.set_sfx_volume(0.3);
    audio.set_ui_volume(0.2);
    audio.set_notifications_volume(0.9);
    audio.set_music_volume(0.7);
    audio.set_ambience_volume(0.6);
    assert_eq!(audio.bus_volume("ambient"), 0.6, "the ambient bus is the ambience volume");
    assert_eq!(audio.bus_volume("notifications"), 0.9);
    assert_eq!(audio.bus_volume("ui"), 0.2);
    assert_eq!(audio.bus_volume("music"), 0.7);
    assert_eq!(audio.bus_volume("sfx"), 0.3);
    assert_eq!(audio.bus_volume("voice"), 0.3, "voice has no slider of its own: the sound-effects volume");
    let catalog = SoundCatalog::load(&repo().join("data"));
    for (id, e) in catalog.entries() {
        assert!(["ambient", "music", "sfx", "voice", "ui", "notifications"].contains(&e.bus.as_str()), "{id}: bus {:?} has no slider", e.bus);
    }
}
