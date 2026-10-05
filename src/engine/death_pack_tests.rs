//! The Death setting's two modes (2026-10-04, the operator's decision; systems::death_pack and
//! engine/death_pack.rs; docs/design/death-and-your-pack.md): what each mode does to the
//! backpack, where a pack lies, taking it back with E, the save, the clock that counts only
//! play, the marker, and the death screen's words.
//!
//! A child of `engine::death_pack` (`use super::*`), in its own file so the module stays small.

use super::*;
use crate::ecs::components::{Appearance, Controllable, Health, Name, Outfit, StatusEffects, Vitals};
use crate::systems::death_pack::{DeathRules, TakeBack};
use crate::systems::inventory::{Inventory, ItemRegistry, ItemStack};
use crate::systems::skills::PlayerSkills;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The rules as shipped (data/world/death.ron).
fn rules() -> DeathRules {
    DeathRules::load(&data_dir())
}

fn items() -> ItemRegistry {
    ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).expect("items.csv")
}

/// A player as lib.rs spawns one, wearing a shirt, trousers and a large backpack.
fn player_world() -> (hecs::World, hecs::Entity) {
    let mut world = hecs::World::new();
    let mut outfit = Outfit::default();
    for (slot, item) in [("chest", "shirt_0"), ("legs", "trousers_0"), ("back", "backpack_large_0")] {
        outfit.equipped.insert(slot.into(), item.into());
    }
    let p = world.spawn((
        Controllable,
        Inventory::new(36),
        PlayerSkills::new(),
        Name("Astra".into()),
        Appearance::default(),
        outfit,
        Health::default(),
        Vitals::default(),
        StatusEffects::default(),
    ));
    (world, p)
}

/// A stack as it would be carried: `qty` of `id`, worn `wear` uses, grade `quality`, food
/// aged `age_s`.
fn stack(id: &str, qty: u32, wear: u32, quality: u8, age_s: f64) -> ItemStack {
    let mut s = ItemStack::new(id.into(), qty, 20);
    s.wear = wear;
    s.quality = quality;
    s.age_s = age_s;
    s
}

/// What a player carries into the deaths below: a worn graded hammer, three old rations and
/// five planks (9 items).
fn carry(world: &mut hecs::World, p: hecs::Entity) {
    let mut inv = world.get::<&mut Inventory>(p).unwrap();
    inv.slots[0] = Some(stack("hammer_0", 1, 3, 2, 0.0));
    inv.slots[1] = Some(stack("ration_basic_0", 3, 0, 0, 5_000.0));
    inv.slots[2] = Some(stack("wood_plank_0", 5, 0, 0, 0.0));
}

/// A backpack's or a pack's stacks, as (item, count, wear, grade, age).
fn stacks<'a>(it: impl Iterator<Item = &'a ItemStack>) -> Vec<(String, u32, u32, u8, f64)> {
    it.map(|s| (s.item_id.clone(), s.quantity, s.wear, s.quality, s.age_s)).collect()
}

fn backpack(world: &hecs::World, p: hecs::Entity) -> Vec<(String, u32, u32, u8, f64)> {
    stacks(world.get::<&Inventory>(p).unwrap().slots.iter().flatten())
}

/// Where the deaths below leave their pack: the Kitchen's floor, where they fell.
fn kitchen() -> PackSpot {
    PackSpot { place: PackPlace::Aboard { at: [26.0, 0.0, 40.0] }, place_words: "in the Kitchen".into(), landing: Landing::WhereYouFell }
}

/// SIMPLIFIED KEEPS THE BACKPACK ON DEATH (the default, the death screen's old "Nothing was
/// lost"): the backpack is exactly as it was, stack for stack, and no pack is left anywhere,
/// even with a place to leave one.
///
/// Seen red 2026-10-04 with `on_death`'s mode check inverted (a Simplified death taking the
/// Realistic path): "Simplified: nothing is lost / left: Left { items: 9, place_words: \"in
/// the Kitchen\", landing: WhereYouFell } / right: NothingLost".
#[test]
fn simplified_keeps_the_backpack_on_death() {
    let (mut world, p) = player_world();
    carry(&mut world, p);
    let before = backpack(&world, p);
    let note = crate::systems::death_pack::on_death(&mut world, DeathMode::Simplified, Some(kitchen()));
    assert_eq!(note, DeathNote::NothingLost, "Simplified: nothing is lost");
    assert_eq!(backpack(&world, p), before, "the backpack is as it was");
    assert!(dp::packs(&world).is_empty(), "no pack is left anywhere");
}

/// REALISTIC LEAVES THE BACKPACK WHERE YOU FELL AND KEEPS WHAT YOU WEAR: every stack goes into
/// one pack at the place the engine found (here the Kitchen's floor), each with its wear, grade
/// and food age; the backpack is empty; the outfit is untouched; the death screen's note says
/// how many and where. A second death with nothing in the backpack leaves nothing.
///
/// Seen red 2026-10-04 with `on_death`'s mode check inverted, so a Realistic death did what
/// every death did before (nothing): "Realistic: the pack's note / left: NothingLost / right:
/// Left { items: 9, place_words: \"in the Kitchen\", landing: WhereYouFell }".
#[test]
fn realistic_leaves_the_backpack_where_you_fell_and_keeps_what_you_wear() {
    let (mut world, p) = player_world();
    carry(&mut world, p);
    let before = backpack(&world, p);
    let worn = world.get::<&Outfit>(p).unwrap().equipped.clone();
    let note = dp::on_death(&mut world, DeathMode::Realistic, Some(kitchen()));
    assert_eq!(
        note,
        DeathNote::Left { items: 9, place_words: "in the Kitchen".into(), landing: Landing::WhereYouFell },
        "Realistic: the pack's note"
    );
    assert!(backpack(&world, p).is_empty(), "the backpack is empty: {:?}", backpack(&world, p));
    let packs = dp::packs(&world);
    assert_eq!(packs.len(), 1, "one pack");
    assert_eq!(packs[0].place, kitchen().place, "it lies where you fell");
    assert_eq!(stacks(packs[0].items.iter()), before, "every stack, with its wear, grade and age");
    assert!(packs[0].fresh, "the death on screen's pack");
    assert_eq!(world.get::<&Outfit>(p).unwrap().equipped, worn, "what you wear stays on you");

    let again = dp::on_death(&mut world, DeathMode::Realistic, Some(kitchen()));
    assert_eq!(again, DeathNote::NothingCarried, "an empty backpack leaves nothing");
    assert_eq!(dp::packs(&world).len(), 1, "and no second pack");
    // Out of the world there is nowhere to leave it, so nothing is lost.
    let (mut world, p) = player_world();
    carry(&mut world, p);
    assert_eq!(dp::on_death(&mut world, DeathMode::Realistic, None), DeathNote::NothingLost);
    assert_eq!(backpack(&world, p).len(), 3);
}

/// ABOARD, A PACK LIES WHERE YOU FELL, OR ON THE NEAREST FLOOR FROM OPEN SPACE: standing in a
/// room it lies under the feet, on the floor the walk had (a stair top), kept off the walls;
/// on the upper of two stacked rooms, on the upper floor; just outside a room's box (a doorway)
/// on its floor, as where you fell; outside every room by more (on the hull, in space) on the
/// nearest room's floor, with how far that was, said as open space, or as no ground near when
/// the player died on a body with no ground to walk to. The room's name is the words.
///
/// Seen red 2026-10-04 with `aboard_spot` answering the feet wherever they were (no placing at
/// all): "kept off the wall / left: 0.05 / right: 0.4".
#[test]
fn aboard_a_pack_lies_where_you_fell_or_on_the_nearest_floor() {
    let r = rules();
    let (clear, slack, open) = (r.wall_clearance_m, r.room_slack_m, r.open_space_m);
    let rooms = [
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 3.0, 10.0)),
        (Vec3::new(20.0, 0.0, 0.0), Vec3::new(30.0, 3.0, 10.0)),
        (Vec3::new(0.0, 3.0, 0.0), Vec3::new(10.0, 6.0, 10.0)),
    ];
    let spot = |feet: Vec3, floor: f32| dp::aboard_spot(&rooms, feet, floor, clear, slack, open).unwrap();
    let s = spot(Vec3::new(5.0, 0.0, 5.0), 0.0);
    assert_eq!(s, dp::AboardSpot { at: Vec3::new(5.0, 0.0, 5.0), room: 0, moved_m: None }, "where you fell");
    assert_eq!(spot(Vec3::new(5.0, 1.2, 5.0), 1.2).at.y, 1.2, "on the stair top the walk had under the feet");
    assert_eq!(spot(Vec3::new(0.05, 0.0, 5.0), 0.0).at.x, clear, "kept off the wall");
    let s = spot(Vec3::new(5.0, 3.0, 5.0), 3.0);
    assert_eq!((s.room, s.at.y), (2, 3.0), "the upper storey's floor");
    let s = spot(Vec3::new(10.3, 0.0, 5.0), 0.0);
    assert_eq!((s.room, s.moved_m), (0, None), "a doorway just outside a room's box is where you fell: {s:?}");
    let s = spot(Vec3::new(14.0, 0.0, 5.0), 0.0);
    assert_eq!(s.room, 0, "between the rooms is open space: it moves to the nearest floor: {s:?}");
    assert_eq!(s.at, Vec3::new(10.0 - clear, 0.0, 5.0));
    let moved = s.moved_m.expect("between the rooms is open space: it moves to the nearest floor");
    assert!((moved - (4.0 + clear)).abs() < 1e-4, "{moved}");
    let s = spot(Vec3::new(25.0, 20.0, 5.0), 0.0);
    assert_eq!((s.room, s.at.y), (1, 0.0), "far over the hull, the floor below: {s:?}");
    assert!(s.moved_m.is_some_and(|m| m > 19.0), "and it says how far: {s:?}");
    assert!(dp::aboard_spot(&[], Vec3::ZERO, 0.0, clear, slack, open).is_none(), "no rooms, no floor");
    // Why it lies there, as the death screen tells it.
    assert_eq!(aboard_landing(None, false), Landing::WhereYouFell);
    assert_eq!(aboard_landing(None, true), Landing::WhereYouFell, "standing in a room is where you fell");
    assert_eq!(aboard_landing(Some(12.0), false), Landing::FromOpenSpace { dist_m: 12.0 });
    assert_eq!(aboard_landing(Some(12.0), true), Landing::FromNoGround { dist_m: 12.0 }, "a body with no ground near");
    assert_eq!(room_words("Kitchen"), "in the Kitchen");
    assert_eq!(room_words("room_7"), "aboard the ship", "a room with no name of its own");
}

/// ON A PLANET, A PACK LIES ON THE GROUND WHERE YOU FELL, OR ON THE NEAREST DRY GROUND: on dry
/// ground under the feet; from the air, on the ground below, saying how far it fell; over deep
/// water, on the nearest dry ground and how far that is along the surface (here the shore is
/// 5 km off in every direction), a little past the water's edge; with no dry ground in reach of
/// the search, nowhere (the engine then uses the ship's nearest floor).
///
/// Seen red 2026-10-04 with `ground_spot` ignoring the water: "over deep water it lands on the
/// nearest dry ground / left: WhereYouFell".
#[test]
fn on_a_planet_a_pack_lies_on_the_ground_or_the_nearest_dry_ground() {
    let r = rules();
    let radius = 6_371_000.0_f64;
    let up = DVec3::Y;
    // A round sea 5 km across the surface around the pole, dry ground beyond.
    let sea = |d: DVec3| (radius, d.angle_between(up) * radius < 5_000.0);
    let dry = |_: DVec3| (radius, false);
    let eye = 1.7;
    let s = dp::ground_spot(up * (radius + eye), eye, &r, dry).unwrap();
    assert_eq!(s.landing, Landing::WhereYouFell, "on dry ground, under the feet");
    assert!((s.at - up * radius).length() < 1e-6, "on the ground under the feet: {s:?}");
    let s = dp::ground_spot(up * (radius + 500.0 + eye), eye, &r, dry).unwrap();
    match s.landing {
        Landing::FromAir { drop_m } => assert!((drop_m - 500.0).abs() < 1e-6, "{drop_m}"),
        other => panic!("from the air it falls to the ground below: {other:?}"),
    }
    let s = dp::ground_spot(up * (radius + eye), eye, &r, sea).unwrap();
    let Landing::FromDeepWater { dist_m } = s.landing else { panic!("over deep water it lands on the nearest dry ground / left: {:?}", s.landing) };
    assert!(!sea(s.at.normalize()).1, "on dry ground");
    let along = s.at.normalize().angle_between(up) * radius;
    assert!((along - dist_m).abs() < 0.01, "the distance is along the surface: {along} vs {dist_m}");
    assert!((5_000.0..5_000.0 + r.shore.onto_land_m + 0.01).contains(&dist_m), "at the shore, a little past it: {dist_m}");
    assert!((s.at.length() - radius).abs() < 1e-6, "on the ground's radius");
    let all_sea = |_: DVec3| (radius, true);
    assert!(dp::ground_spot(up * (radius + eye), eye, &r, all_sea).is_none(), "no dry ground in reach");
}

/// TAKING IT BACK WITH E: as much as the backpack holds goes back in, each stack as it was
/// (the hammer's wear and grade), and the rest stays in the pack; with the backpack full,
/// nothing moves and the pack keeps it all; once there is room, the rest comes back and the
/// emptied pack is gone. The toast says which.
///
/// Seen red 2026-10-04 with `take_back` moving nothing (there was nothing to take back
/// before): "the hammer and 7 planks fit, 3 planks do not / left: TakeBack { taken: 0, left:
/// 11 } / right: TakeBack { taken: 8, left: 3 }".
#[test]
fn e_takes_back_what_fits_and_leaves_the_rest() {
    let reg = items();
    let (mut world, p) = player_world();
    let pack = world.spawn((LeftPack {
        items: vec![stack("hammer_0", 1, 3, 2, 0.0), stack("wood_plank_0", 10, 0, 0, 0.0)],
        place: kitchen().place,
        place_words: "in the Kitchen".into(),
        landing: Landing::WhereYouFell,
        played_s: 0.0,
        warned: false,
        fresh: false,
    },));
    // A 65 L backpack: the hammer (0.29 L) and 7 of the 8.17 L planks fit (57.5 L), 3 do not.
    let r = dp::take_back(&mut world, pack, Some(&reg)).unwrap();
    assert_eq!(r, TakeBack { taken: 8, left: 3 }, "the hammer and 7 planks fit, 3 planks do not");
    let carried = backpack(&world, p);
    assert!(carried.contains(&("hammer_0".into(), 1, 3, 2, 0.0)), "the hammer as it was: {carried:?}");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("wood_plank_0"), 7);
    assert_eq!(world.get::<&LeftPack>(pack).unwrap().count(), 3, "the rest stays in the pack");
    assert_eq!(take_back_words(r), "You took back 8 items. 3 items are still in your pack: your backpack has no room for them.");

    let r = dp::take_back(&mut world, pack, Some(&reg)).unwrap();
    assert_eq!(r, TakeBack { taken: 0, left: 3 }, "a full backpack takes nothing");
    assert_eq!(take_back_words(r), "Your backpack has no room: all 3 items are still in your pack.");

    {
        let mut inv = world.get::<&mut Inventory>(p).unwrap();
        inv.remove_item("wood_plank_0", 7);
        inv.volume_current_l = reg.volume_for("hammer_0");
    }
    let r = dp::take_back(&mut world, pack, Some(&reg)).unwrap();
    assert_eq!(r, TakeBack { taken: 3, left: 0 }, "with room, the rest");
    assert!(world.get::<&LeftPack>(pack).is_err(), "an emptied pack is gone");
    assert_eq!(take_back_words(r), "You took back everything in your pack: 3 items.");
}

/// E reaches a pack within the rules' reach, faced within their cone, and nothing else.
/// Seen red 2026-10-04 with `faces` taking any direction: "looking away".
#[test]
fn e_reaches_a_pack_in_front_within_reach() {
    let r = rules();
    let eye = DVec3::new(0.0, 1.7, 0.0);
    let ahead = DVec3::Z;
    // Standing over it, looking down at it.
    let at = DVec3::new(0.0, 0.25, 1.2);
    let look_at = (at - eye).normalize();
    assert!(faces(eye, look_at, at, r.reach_m, r.facing_cos).is_some(), "looking at it, in reach");
    assert!(faces(eye, -look_at, at, r.reach_m, r.facing_cos).is_none(), "looking away");
    assert!(faces(eye, ahead, DVec3::new(0.0, 0.25, r.reach_m + 3.0), r.reach_m, r.facing_cos).is_none(), "out of reach");
}

/// Through the file format and back, as a save on disk is.
fn round_trip(save: &crate::persistence::WorldSave) -> crate::persistence::WorldSave {
    serde_json::from_str(&serde_json::to_string(save).unwrap()).unwrap()
}

/// THE PACK SURVIVES A SAVE AND A LOAD: a pack left on Earth's ground, 90 s into its time,
/// comes back after the save, through the file format, onto another player: where it lies,
/// every stack as it was, the play time it had counted, and the death screen it belongs to.
/// The save is authoritative: a pack the world had that the save does not is gone. A save
/// from before packs loads with none.
///
/// Seen red 2026-10-04 with the save not writing the packs: "the pack comes back with the
/// save, and only it / left: 0 / right: 1".
#[test]
fn the_pack_survives_a_save_and_a_load() {
    let (mut world, p) = player_world();
    carry(&mut world, p);
    let before = backpack(&world, p);
    let place = PackPlace::Ground { body: "earth".into(), at: [1_234.5, 6_370_000.25, -987.0] };
    let spot = PackSpot { place: place.clone(), place_words: "on Earth".into(), landing: Landing::FromDeepWater { dist_m: 230.0 } };
    dp::on_death(&mut world, DeathMode::Realistic, Some(spot));
    dp::count_down(&mut world, 90.0, &rules());
    world.insert_one(p, crate::ecs::components::Dead { cause: "hypothermia".into(), ..Default::default() }).unwrap();
    let save = round_trip(&crate::save_load::extract_world_save(&world));

    let (mut fresh, _q) = player_world();
    fresh.spawn((LeftPack { items: vec![stack("hammer_0", 1, 0, 0, 0.0)], place: kitchen().place, place_words: String::new(), landing: Landing::WhereYouFell, played_s: 5.0, warned: false, fresh: false },));
    crate::save_load::apply_save_to_world(&mut fresh, &save);
    let packs = dp::packs(&fresh);
    assert_eq!(packs.len(), 1, "the pack comes back with the save, and only it");
    assert_eq!(packs[0].place, place, "where it lies");
    assert_eq!(stacks(packs[0].items.iter()), before, "every stack as it was");
    assert_eq!(packs[0].played_s, 90.0, "the play time it had counted");
    assert_eq!(
        dp::note_for_loaded_death(&fresh, DeathMode::Realistic),
        DeathNote::Left { items: 9, place_words: "on Earth".into(), landing: Landing::FromDeepWater { dist_m: 230.0 } },
        "the death screen it belongs to, after a quit on it"
    );
    // A save written before packs existed.
    let mut old = serde_json::to_value(&save).unwrap();
    old.as_object_mut().unwrap().remove("left_packs");
    let old: crate::persistence::WorldSave = serde_json::from_value(old).unwrap();
    assert!(old.left_packs.is_empty());
}

/// IT GOES AFTER THE DATA FILE'S TIME OF PLAY, AND NOT WHILE THE GAME IS CLOSED: the clock
/// counts play (the engine runs it only in the world and alive); the warning comes once,
/// `warn_minutes_left` before the end; a save loaded a day later, with the offline catch-up
/// on, has not counted the day; then the rest of `keep_minutes_of_play` takes it, with a
/// notice that says what was lost.
///
/// Seen red 2026-10-04 with `count_down` counting nothing (packs stayed forever): "the
/// warning: [] / left: 0 / right: 1".
#[test]
fn it_goes_after_the_data_files_time_of_play_and_not_while_the_game_is_closed() {
    let r = rules();
    let (keep, warn) = (r.keep_s(), r.warn_s());
    assert!(keep > warn && warn > 0.0, "the shipped file keeps a pack longer than its warning");
    let (mut world, p) = player_world();
    carry(&mut world, p);
    dp::on_death(&mut world, DeathMode::Realistic, Some(kitchen()));
    assert!(dp::count_down(&mut world, keep - warn - 1.0, &r).is_empty(), "nothing yet");
    let warned = dp::count_down(&mut world, 2.0, &r);
    assert_eq!(warned.len(), 1, "the warning: {warned:?}");
    assert!(matches!(&warned[0], PackEvent::Warn { items: 9, .. }), "{warned:?}");
    assert_eq!(
        pack_notice(&warned[0], r.keep_minutes_of_play),
        format!("Your pack in the Kitchen will be gone in {} of play. It holds 9 items.", minutes_words(r.warn_minutes_left))
    );
    assert!(dp::count_down(&mut world, 1.0, &r).is_empty(), "warned once");
    let played = dp::packs(&world)[0].played_s;

    let mut save = crate::save_load::extract_world_save(&world);
    save.timestamp = 1_000;
    let (mut fresh, _q) = player_world();
    crate::save_load::apply_save_to_world(&mut fresh, &save);
    crate::save_load::catch_up_world(&mut fresh, &save, true, 1.0, 1_000 + 86_400);
    assert_eq!(dp::packs(&fresh)[0].played_s, played, "a day with the game closed did not count");

    assert!(dp::count_down(&mut fresh, keep - played - 0.5, &r).is_empty(), "half a second left");
    let gone = dp::count_down(&mut fresh, 1.0, &r);
    assert_eq!(gone, vec![PackEvent::Gone { place_words: "in the Kitchen".into(), items: 9 }], "the rest of the time takes it");
    assert!(dp::packs(&fresh).is_empty(), "it is gone");
    assert_eq!(
        pack_notice(&gone[0], r.keep_minutes_of_play),
        format!("Your pack in the Kitchen is gone: it lay there for {} of play. The 9 items in it are lost.", minutes_words(r.keep_minutes_of_play))
    );
}

/// The HUD as lib.rs draws it, through `cam`, on a 1280 by 900 screen: the second frame's
/// shapes.
fn hud_shapes(state: &crate::gui::GuiState, cam: &crate::renderer::camera::Camera) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
            ..Default::default()
        };
        shapes = ctx
            .run(input, |ctx| crate::gui::pages::hud::draw(ctx, &theme, state, cam.yaw, cam.view_projection_matrix(), cam.position))
            .shapes;
    }
    shapes
}

fn pack_at(place: PackPlace, played_s: f64) -> LeftPack {
    LeftPack { items: vec![stack("hammer_0", 1, 0, 0, 0.0)], place, place_words: String::new(), landing: Landing::WhereYouFell, played_s, warned: false, fresh: false }
}

/// THE MARKER POINTS AT IT: a pack aboard 120 m away, ahead and to the right, is marked at its
/// own middle (its place plus the home's offset in render space, half its height up) with its
/// distance, and the HUD draws "Your pack · 120 m" on the right of the screen, where it is;
/// behind the player on the left, the marker is pinned to the left edge. A pack on a planet is
/// marked through that body's frame; two packs are numbered, newest first.
///
/// Seen red 2026-10-04 with `pack_markers` marking nothing: "the pack is marked / left: 0 /
/// right: 1".
#[test]
fn the_marker_points_at_the_pack() {
    use crate::gui::screen_surface::find_text_in_shapes;
    let half = rules().look.size_m.1 * 0.5;
    let mut cam = crate::renderer::camera::Camera::new();
    cam.far = 500.0;
    cam.aspect = 1280.0 / 900.0;
    cam.position = Vec3::new(0.0, 1.7, 0.0);
    // 120 m to the pack's middle, 30 degrees right of ahead.
    let dir = (cam.forward() * 30f32.to_radians().cos() + cam.right() * 30f32.to_radians().sin()).normalize();
    let mid = cam.position + dir * 120.0;
    let at = mid - Vec3::Y * half;
    let markers = pack_markers(&[pack_at(PackPlace::Aboard { at: at.to_array() }, 10.0)], Vec3::ZERO, &[], cam.position, half);
    assert_eq!(markers.len(), 1, "the pack is marked");
    assert_eq!(markers[0].0, "Your pack");
    assert!((markers[0].1 - mid).length() < 1e-3 && (markers[0].2 - 120.0).abs() < 1e-3, "at its middle, 120 m: {markers:?}");
    let mut state = crate::gui::GuiState::default();
    state.target_markers = markers;
    let found = find_text_in_shapes(&hud_shapes(&state, &cam), "Your pack").expect("the HUD marks the pack: none was drawn");
    assert_eq!(found.text, "Your pack · 120 m");
    assert!(found.rect.center().x > 640.0 + 40.0, "on the right, where the pack is: {:?}", found.rect);

    // Behind, on the left: pinned to the left edge.
    let behind_left = (-cam.forward() * 2.0 - cam.right()).normalize();
    let at = cam.position + behind_left * 30.0 - Vec3::Y * half;
    state.target_markers = pack_markers(&[pack_at(PackPlace::Aboard { at: at.to_array() }, 0.0)], Vec3::ZERO, &[], cam.position, half);
    let found = find_text_in_shapes(&hud_shapes(&state, &cam), "Your pack").expect("behind, still marked");
    assert!(found.rect.center().x < 320.0, "on the left edge, the side to turn: {:?}", found.rect);

    // The home drawn somewhere else in render space (off the station): the pack moves with it.
    let off = Vec3::new(500.0, -20.0, 3_000.0);
    let m = pack_markers(&[pack_at(PackPlace::Aboard { at: [1.0, 0.0, 2.0] }, 0.0)], off, &[], cam.position, half);
    assert!((m[0].1 - (Vec3::new(1.0, half, 2.0) + off)).length() < 1e-3, "{m:?}");

    // On Earth's ground, through the body's frame this frame; none when the body is not placed.
    let ground = DVec3::new(0.0, 6_371_000.0, 0.0);
    let frames = vec![("earth".to_string(), DVec3::new(0.0, -6_371_001.7, 0.0), DQuat::IDENTITY)];
    let on_earth = pack_at(PackPlace::Ground { body: "earth".into(), at: ground.to_array() }, 0.0);
    let m = pack_markers(std::slice::from_ref(&on_earth), Vec3::ZERO, &frames, Vec3::new(0.0, 0.0, 0.0), half);
    assert!((m[0].1 - Vec3::new(0.0, half - 1.7, 0.0)).length() < 1e-3, "under the feet, from the body's frame: {m:?}");
    assert!(pack_markers(&[on_earth], Vec3::ZERO, &[], Vec3::ZERO, half).is_empty(), "no frame, no marker");

    // Two packs: numbered, the newest first.
    let older = pack_at(PackPlace::Aboard { at: [0.0, 0.0, 50.0] }, 300.0);
    let newer = pack_at(PackPlace::Aboard { at: [0.0, 0.0, 9.0] }, 20.0);
    let m = pack_markers(&[older, newer], Vec3::ZERO, &[], Vec3::ZERO, half);
    let labels: Vec<(&str, f32)> = m.iter().map(|(l, p, _)| (l.as_str(), p.z)).collect();
    assert_eq!(labels, vec![("Your pack 1", 9.0), ("Your pack 2", 50.0)]);
}

/// THE DEATH SCREEN'S WORDS FOR EACH MODE: Simplified says what it always said, nothing was
/// lost; Realistic says what stayed behind in the pack, where it lies (and why somewhere else
/// when the player fell where no one can stand, and how far), that what they wear and have
/// equipped stays on them, the key that takes it back and how long it stays; an empty
/// backpack says nothing was left. Drawn: the card shows the mode's words and not the other's.
///
/// Seen red 2026-10-04 with the death screen's one fixed line for every mode: "Realistic: what
/// stayed, where, the key and the time / left: [\"You wake in the respawner. Nothing was lost,
/// but the body remembers: keep fed, hydrated, warm, and breathing.\"]".
#[test]
fn the_death_screen_says_what_each_mode_costs() {
    use crate::gui::pages::hud::death_screen_lines;
    let r = rules();
    let keep = minutes_words(r.keep_minutes_of_play);
    let simplified = vec![
        "You wake in the respawner. Nothing was lost, but the body remembers: keep fed, hydrated, warm, and breathing.".to_string(),
    ];
    assert_eq!(death_screen_lines(Some(&DeathNote::NothingLost), &r, "E"), simplified, "Simplified: the words it always said");
    assert_eq!(death_screen_lines(None, &r, "E"), simplified, "no note yet reads as nothing lost");
    let left = |landing: Landing, words: &str| {
        death_screen_lines(Some(&DeathNote::Left { items: 9, place_words: words.into(), landing }), &r, "E")
    };
    assert_eq!(
        left(Landing::WhereYouFell, "in the Kitchen"),
        vec![
            "You wake in the respawner. Everything in your backpack, 9 items, stays behind in your pack. What you wear and what you have equipped stay on you.".to_string(),
            "It lies where you fell, in the Kitchen.".to_string(),
            format!("It is marked on your screen. Walk to it and press E to take back what fits. It stays for {keep} of play, then it is gone."),
        ],
        "Realistic: what stayed, where, the key and the time"
    );
    assert_eq!(
        left(Landing::FromOpenSpace { dist_m: 12.0 }, "in the Corridor")[1],
        "You died in open space, where no one can walk, so it lies on the nearest floor you can walk to, 12 m away, in the Corridor."
    );
    assert_eq!(
        left(Landing::FromDeepWater { dist_m: 230.0 }, "on Earth")[1],
        "You died in deep water, so it lies on the nearest dry ground, 230 m away, on Earth."
    );
    assert_eq!(
        left(Landing::FromAir { drop_m: 2_000.0 }, "on Earth")[1],
        "You died above the ground, so it fell to the ground below you, 2.0 km down, on Earth."
    );
    assert_eq!(
        left(Landing::FromNoGround { dist_m: 12_000_000.0 }, "in the Kitchen")[1],
        "There was no ground you could walk to near where you died, so it lies on the ship's nearest floor, 12,000 km away, in the Kitchen."
    );
    assert_eq!(
        death_screen_lines(Some(&DeathNote::NothingCarried), &r, "E")[0],
        "You wake in the respawner. Your backpack was empty, so nothing was left where you fell. What you wear and what you have equipped stay on you."
    );

    // Drawn on the death screen.
    use crate::gui::screen_surface::find_text_in_shapes;
    let card = |note: DeathNote| {
        let mut state = crate::gui::GuiState::default();
        state.player_death_cause = Some("dehydration".into());
        state.death_pack.note = Some(note);
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut shapes = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
                ..Default::default()
            };
            shapes = ctx.run(input, |ctx| crate::gui::pages::hud::draw_death_screen(ctx, &theme, &mut state)).shapes;
        }
        shapes
    };
    let shapes = card(DeathNote::Left { items: 9, place_words: "in the Kitchen".into(), landing: Landing::WhereYouFell });
    assert!(find_text_in_shapes(&shapes, "It lies where you fell, in the Kitchen.").is_some(), "Realistic drawn");
    assert!(find_text_in_shapes(&shapes, "Nothing was lost").is_none(), "and not Simplified's words");
    let shapes = card(DeathNote::NothingLost);
    assert!(find_text_in_shapes(&shapes, "Nothing was lost").is_some(), "Simplified drawn");
    assert!(find_text_in_shapes(&shapes, "stays behind in your pack").is_none());
}

/// The Settings hint says exactly what each mode does, with the data file's time.
/// Seen red 2026-10-04 with the hint's time written in as "60 minutes": "the time is the
/// file's".
#[test]
fn the_settings_hint_says_what_each_mode_does_with_the_files_time() {
    let mut r = rules();
    let hint = crate::gui::pages::settings::death_hint(&r, "E");
    for part in ["Simplified: you keep everything you carried", "Realistic: everything in your backpack stays where you fell", "what you wear and what you have equipped stay on you", "press E", "open space or deep water", "never while the game is closed", "only you can see or take your pack"] {
        assert!(hint.contains(part), "the hint says {part:?}: {hint}");
    }
    assert!(hint.contains(&format!("{} of play", minutes_words(r.keep_minutes_of_play))), "{hint}");
    r.keep_minutes_of_play = 90.0;
    assert!(crate::gui::pages::settings::death_hint(&r, "E").contains("90 minutes of play"), "the time is the file's");
}

/// The shipped rules parse, the copy built into the exe is the same file, and the numbers make
/// sense together (a pack in reach of a standing player's eyes, a warning inside the time).
#[test]
fn the_shipped_death_rules_parse() {
    let text = std::fs::read_to_string(data_dir().join("world/death.ron")).unwrap();
    let disk = DeathRules::from_ron(&text).expect("data/world/death.ron parses");
    let built_in = DeathRules::from_ron(crate::embedded_data::WORLD_DEATH_RON).expect("the built-in copy parses");
    assert_eq!(disk, built_in, "the built-in copy is the file");
    assert_eq!(rules(), disk, "load reads it");
    assert!(disk.keep_minutes_of_play > disk.warn_minutes_left && disk.warn_minutes_left >= 0.0);
    assert!(disk.reach_m > f64::from(crate::surface_walk::EYE_HEIGHT_M), "a pack at your feet is in reach");
    assert!(disk.facing_cos > 0.0 && disk.facing_cos < 1.0);
    assert!(disk.wall_clearance_m >= 0.0 && disk.room_slack_m >= 0.0 && disk.open_space_m > 0.0);
    assert!(disk.shore.ring_growth > 1.0 && disk.shore.bearings >= 4 && disk.shore.max_m > disk.shore.first_ring_m);
}
