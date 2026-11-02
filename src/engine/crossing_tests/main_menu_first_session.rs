//! From gui/pages/main_menu.rs (`first_session_tests`): the first session starts alone in the
//! player's own home, through the engine's join gate (`home_plot`) and save slots (`save_slot`).

//! The first session starts alone in the player's own home, and a relaunch keeps it there
//! (first-hour audit 2026-10-04, docs/design/first-hour-audit-2026-10-04.md: Blocker 1 and
//! Friction 10). A new player who typed a name and was online used to be put in the live
//! shared world a few seconds after Enter: chat connects to united-humanity.us by itself, and
//! with `copresence_solo` false (its default, never saved) the join gate joined its world,
//! on a 72x clock and possibly as a guest with no home. Only the WORLD join changes here;
//! chat still connects as it always has.

use crate::gui::pages::main_menu::{finish_onboarding, skip_server_step, skip_setup};
use crate::config::AppConfig;
use crate::engine::home_plot::{join_step, JoinGate, JoinStep};
use crate::gui::{GuiPage, GuiState, LauncherWhere};

/// The join gate once the player stands in the loaded world, connected and identified,
/// aboard, with the server's own save live: everything a join waits for except the player's
/// own solo choice.
fn in_world_gate(solo: bool) -> JoinGate {
    JoinGate { in_world: true, joined: false, identified: true, solo, aboard: true, refused_here: false, world_loaded: true, has_ship: true, slot_ready: true }
}

/// The app closed and opened again: the config written from `state`, read back as JSON,
/// applied to a fresh state the way boot applies it (lib.rs, `apply_to_gui_state`).
fn relaunch(state: &GuiState) -> GuiState {
    let json = serde_json::to_string(&AppConfig::from_gui_state(state)).expect("config serializes");
    let back: AppConfig = serde_json::from_str(&json).expect("config parses");
    let mut fresh = GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    fresh
}

/// "Enter HumanityOS" after skipping the server step: into the world, alone, in the
/// player's own home, and that home is the pairing Play repeats.
///
/// Seen red 2026-10-04 on 8e400d7ed: "a new player is joined to the shared world without
/// being asked: copresence_solo is false".
#[test]
fn entering_from_onboarding_starts_alone_in_your_own_home() {
    let mut state = GuiState::default();
    assert!(!state.server_connected, "the setup: no server chosen on the server step");
    finish_onboarding(&mut state);
    assert!(state.onboarding_complete);
    assert_eq!(state.active_page, GuiPage::None, "Enter goes into the world");
    assert!(state.copresence_solo, "a new player is joined to the shared world without being asked: copresence_solo is false");
    assert_eq!(join_step(&in_world_gate(state.copresence_solo)), JoinStep::Wait, "the join gate joins the shared world");
    assert_eq!(state.launcher_last_world, "home:", "no pairing recorded, so Play opens the picker: {:?}", state.launcher_last_world);
    assert_eq!(state.launcher_where_kind, LauncherWhere::Home);
    // And it outlives the session.
    let back = relaunch(&state);
    assert!(back.copresence_solo, "after a relaunch the game joins the shared world");
}

/// The welcome step's "Skip setup (offline mode)" goes into the world with no identity;
/// one made later in Settings must not quietly join the shared world either.
///
/// Seen red 2026-10-04 on 8e400d7ed: "skipping setup leaves the shared world one identity
/// away: copresence_solo is false".
#[test]
fn skipping_setup_starts_alone_in_your_own_home() {
    let mut state = GuiState::default();
    skip_setup(&mut state);
    assert!(state.onboarding_complete);
    assert_eq!(state.active_page, GuiPage::None);
    assert!(state.copresence_solo, "skipping setup leaves the shared world one identity away: copresence_solo is false");
    assert_eq!(state.launcher_last_world, "home:", "{:?}", state.launcher_last_world);
}

/// "Skip (stay offline)" after a successful Connect: the player chose not to use the
/// server, so Enter must not treat the earlier reachability check as a choice of its world.
///
/// Seen red 2026-10-04 on 8e400d7ed: "Skip then Enter lands on Chat, not in the world".
#[test]
fn skip_stay_offline_after_connecting_still_starts_alone() {
    let mut state = GuiState::default();
    state.onboarding_step = 1;
    state.server_connected = true; // Connect reached the server
    skip_server_step(&mut state);
    assert_eq!(state.onboarding_step, 2, "Skip moves on to the identity step");
    finish_onboarding(&mut state);
    assert_eq!(state.active_page, GuiPage::None, "Skip then Enter lands on {:?}, not in the world", state.active_page);
    assert!(state.copresence_solo, "Skip (stay offline) after a Connect joins the shared world: copresence_solo is false");
    assert_eq!(state.launcher_last_world, "home:");
}

/// The one exception: the player pressed Connect, reached the server and pressed
/// Continue. They chose that server, so its shared world stays theirs (and Enter opens
/// Chat, as before); no home pairing is recorded over that choice. Green on 8e400d7ed
/// too: it guards the exception, so the fix cannot overreach. Broken on purpose 2026-10-04
/// with the home start applied on this path as well: "a player who chose a server is kept
/// out of its world".
/// The first five minutes (gui/first_minutes.rs) begin when the first-run setup ends,
/// on both of its paths, and a tour already finished is not restarted by it.
#[test]
fn finishing_the_first_run_starts_the_first_minutes_tour() {
    for connected in [false, true] {
        let mut state = GuiState::default();
        state.server_connected = connected;
        assert!(!state.first_minutes.showing());
        finish_onboarding(&mut state);
        assert!(state.first_minutes.showing(), "connected={connected}: the tour starts");
        assert_eq!(state.first_minutes.step, 0);
    }
    let mut state = GuiState::default();
    state.first_minutes.start();
    state.first_minutes.done = true;
    finish_onboarding(&mut state);
    assert!(!state.first_minutes.showing(), "a finished tour stays finished");
}

#[test]
fn connecting_and_continuing_keeps_the_servers_shared_world() {
    let mut state = GuiState::default();
    state.server_connected = true;
    finish_onboarding(&mut state);
    assert_eq!(state.active_page, GuiPage::Chat);
    assert!(!state.copresence_solo, "a player who chose a server is kept out of its world");
    assert_eq!(join_step(&in_world_gate(state.copresence_solo)), JoinStep::Join);
    assert!(state.launcher_last_world.is_empty(), "a home pairing was recorded over the server choice: {:?}", state.launcher_last_world);
}

/// RELAUNCH (Friction 10): the player's last pairing was their own home. After a restart
/// Esc (or the hub's Enter World) drops into the world: the join gate must keep out of the
/// shared world, and Play must go straight home, never through the picker or onto the server.
///
/// Seen red 2026-10-04 on 8e400d7ed (boot never reading the solo start back from the
/// pairing): "after a relaunch, Esc into the world joins the shared world: copresence_solo
/// is false".
#[test]
fn a_relaunch_keeps_a_home_player_out_of_the_shared_world() {
    let mut state = GuiState::default();
    state.onboarding_complete = true;
    state.launcher_where_kind = LauncherWhere::Home;
    state.copresence_solo = true;
    state.record_pairing();
    let mut back = relaunch(&state);
    assert_eq!(back.launcher_last_world, "home:", "the pairing is in the config");
    assert!(back.copresence_solo, "after a relaunch, Esc into the world joins the shared world: copresence_solo is false");
    assert_eq!(join_step(&in_world_gate(back.copresence_solo)), JoinStep::Wait);
    // Play: the recorded pairing, not the picker, and still alone.
    assert!(back.apply_last_pairing(), "Play opens the picker");
    assert!(back.copresence_solo, "Play joins the shared world");
    assert_eq!(back.launcher_where_kind, LauncherWhere::Home);
}

/// The other pairings keep what they always did: a server pairing is that server's shared
/// world, and no pairing at all (a config from before pairings, a rig's fresh folder) keeps
/// the shared default the copresence rigs rely on. Green on 8e400d7ed too: a guard, so
/// the solo start read back at boot stays confined to home pairings. Broken on purpose
/// 2026-10-04 with solo read back for anything but a server pairing: "no pairing comes back
/// solo, which would keep every rig out of the shared world".
#[test]
fn a_relaunch_after_a_server_or_no_pairing_keeps_the_shared_world() {
    let mut state = GuiState::default();
    state.launcher_where_kind = LauncherWhere::Server;
    state.launcher_selected_server = Some("__connected__".to_string());
    state.record_pairing();
    let back = relaunch(&state);
    assert_eq!(back.launcher_last_world, "server:__connected__");
    assert!(!back.copresence_solo, "a server pairing comes back solo");
    let back = relaunch(&GuiState::default());
    assert!(back.launcher_last_world.is_empty());
    assert!(!back.copresence_solo, "no pairing comes back solo, which would keep every rig out of the shared world");
}

/// A RELAUNCH ON A SERVER PAIRING LOADS THAT SERVER'S SAVE (ship homes increment 6, plan 3.6):
/// the player's last pairing was a server, where they live in a Cabin with its water. After a
/// restart the boot (engine/save_slot.rs `boot`, from the config as it was written) makes that
/// server's save the live one, never your own home's (whose gold stays home), so the player is
/// spawned as a Cabin and its save is what the startup applies. A home pairing boots your own
/// home's save.
///
/// Seen red 2026-10-12 with `boot` reading offline_home.json for the server pairing too (its
/// `open_slot` given your own home's slot, as the boot before increment 6 always read): "a
/// relaunch on a server pairing loaded your own home's save: [(\"gold_ingot_0\", 7)]".
#[test]
fn a_relaunch_on_a_server_pairing_loads_that_servers_save() {
    use crate::persistence::{save_world, slot_path, SaveSlot, WorldSave};
    let data_dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let saves = humanity_core::test_temp::dir("relaunch_server_save");
    let mut state = GuiState::default();
    state.onboarding_complete = true;
    state.profile_public_key = "AB".repeat(32);
    state.server_url = "https://server.example/".into();
    state.launcher_where_kind = LauncherWhere::Server;
    state.launcher_selected_server = Some("__connected__".to_string());
    state.record_pairing();
    let config: AppConfig = serde_json::from_str(&serde_json::to_string(&AppConfig::from_gui_state(&state)).unwrap()).unwrap();
    // Your own home, with its gold; the server's save, a Cabin with its water.
    let mut own = WorldSave::new_offline("My Homestead", "homestead");
    own.inventory = vec![("gold_ingot_0".into(), 7)];
    own.progress_saved = true;
    save_world(&slot_path(&saves, &SaveSlot::Offline), &own).unwrap();
    let slot = SaveSlot::Server { server: "https://server.example".into(), identity: "ab".repeat(32) };
    let mut theirs = WorldSave::new_offline("My Cabin", "cabin");
    theirs.kind = "server".into();
    theirs.household = "solo".into();
    theirs.inventory = vec![("water_purified_0".into(), 2)];
    theirs.progress_saved = true;
    save_world(&slot_path(&saves, &slot), &theirs).unwrap();

    let boot = crate::engine::save_slot::boot(&saves, &data_dir, Some(&config));
    assert_eq!(boot.slot, slot, "the server's save is the live one");
    let save = boot.save.expect("the server's save was read");
    assert_eq!(save.inventory, theirs.inventory, "a relaunch on a server pairing loaded your own home's save: {:?}", save.inventory);
    let active = crate::persistence::live_save::active_home();
    assert_eq!((active.start.as_str(), active.household.as_str(), active.path.clone()), ("cabin", "solo", slot_path(&saves, &slot)));
    assert!(crate::save_load::applies_progress(&save, true), "a server's save keeps its progress whatever the fresh-home setting says");

    // A home pairing boots your own home.
    state.launcher_where_kind = LauncherWhere::Home;
    state.record_pairing();
    let home: AppConfig = serde_json::from_str(&serde_json::to_string(&AppConfig::from_gui_state(&state)).unwrap()).unwrap();
    let boot = crate::engine::save_slot::boot(&saves, &data_dir, Some(&home));
    assert_eq!(boot.slot, SaveSlot::Offline);
    assert_eq!(boot.save.map(|s| s.inventory), Some(own.inventory));
    assert_eq!(crate::persistence::live_save::active_home().start, "homestead");
    crate::persistence::live_save::set_active_home(crate::persistence::live_save::ActiveHome::offline());
}
