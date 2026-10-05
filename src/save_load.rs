//! Offline save/load lifecycle (v0.381) -- homes increment 3.
//!
//! Before this, the game persisted NOTHING between sessions: `persistence::*` was
//! wired only in tests, and entering the 3D world regenerates the homestead fresh.
//! This wires the minimal, correct first slice: the player's INVENTORY + SKILLS
//! (their actual progress) are captured into the active offline home on exit +
//! periodically, and applied back on startup, so your homestead progress sticks.
//!
//! Why apply at STARTUP (not on 3D-enter): the ECS player entity is the source of
//! truth and the systems tick every frame -- in the menu-driven loops AND in 3D --
//! so the player accumulates progress always. Applying on startup also makes the
//! exit-save SAFE: the player always carries the loaded state, so closing without
//! playing round-trips the save instead of overwriting it with an empty inventory.
//!
//! The body round-trips too (first-hour audit S1, 2026-10-04): health, the
//! vitals with the waste meter, the status effects still running, a death, and
//! the home's urine tank, so quitting no longer heals or refills anyone. Still
//! deferred: POSITION. A load stands you at the home's front door, because a
//! saved position means nothing without the frame it was taken in (the home
//! aboard, a planet's build site, a Dev trip) and the plot the home stood on,
//! which the next launch can change (docs/design/homes-as-profiles.md). Crops,
//! quests, vehicles, wallet and the world clock round-trip; the clock and the
//! offline catch-up are `resume_home` below.

use crate::ecs::components::Controllable;
use crate::persistence::{self, WorldSave};
use crate::systems::inventory::Inventory;
use crate::systems::skills::{PlayerSkills, SkillProgress};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Wall-clock seconds at the last periodic save (0 = not armed yet). A process
/// singleton, fine for a single-instance desktop app.
static LAST_SAVE_SECS: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where the player's home stands: the box (min, max) of the plot it stands on, in ship metres
/// (increment 1b of docs/design/ship-homes-and-logistics.md). The engine keeps it in the
/// DataStore under `HOME_FRAME_KEY`, current with the live ship (engine/home_plot.rs: the world
/// load publishes it, and every rebuild of the home republishes it when the home's plot
/// changed, carrying what the home holds along, `follow_home_box`), and every save records it
/// beside the pieces and vehicles, which are saved where they stand (`record_home_frame`).
///
/// Why the save records it (the third review of 1b): the save used to write the home's pieces
/// as if the home stood on the default plot, which only held while every world entry built the
/// home there first and the default plot never changed. A Dev who made another plot the
/// default stranded every piece 99 m from the home on the next launch, and increment 2, which
/// builds the home on the player's remembered plot, would have stranded them all. A save that
/// says where its home stood can be carried to wherever the home stands when it loads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HomeFrame {
    pub home: (glam::Vec3, glam::Vec3),
}

/// The DataStore key of the home's `HomeFrame`.
pub const HOME_FRAME_KEY: &str = "home_frame";

/// The DataStore key under which the save applied at startup leaves the box its home stood on
/// (`LoadedHomeBox`), for the world load to carry its pieces to the plot the home is built on
/// (engine/world_load.rs; the save is applied before the ship is assembled).
pub const LOADED_HOME_BOX_KEY: &str = "loaded_home_box";

/// The plot box a save's home stood on (`WorldSave::home_plot_box`), waiting in the DataStore
/// under `LOADED_HOME_BOX_KEY` for the world load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadedHomeBox(pub [[f32; 3]; 2]);

/// The frame a save records (`record_home_frame`): the one the engine keeps with the live
/// ship, else, before the world has loaded, the box of the save applied at startup, which its
/// pieces still stand in (`LoadedHomeBox`; it also stays for a session on the legacy layout,
/// engine/home_plot.rs `carry_loaded_box`). Round 4 of the 1b review: the save on quit and the
/// periodic save run before the world loads too, and recorded NO box while the pieces stood in
/// the saved plot's frame, so the next launch left them where they stood, outside the home.
pub fn frame_for_save(data: &crate::hot_reload::data_store::DataStore) -> Option<HomeFrame> {
    data.get::<HomeFrame>(HOME_FRAME_KEY).copied().or_else(|| {
        data.get::<LoadedHomeBox>(LOADED_HOME_BOX_KEY)
            .map(|b| HomeFrame { home: (glam::Vec3::from_array(b.0[0]), glam::Vec3::from_array(b.0[1])) })
    })
}

/// Write where the home stands into a save (`WorldSave::home_plot_box`), from the frame the
/// engine keeps (`HomeFrame`, `frame_for_save`); no frame (the legacy layout, with no save
/// waiting) records none. The pieces and vehicles stay where they stand.
pub fn record_home_frame(save: &mut WorldSave, frame: Option<&HomeFrame>) {
    save.home_plot_box = frame.map(|f| [f.home.0.to_array(), f.home.1.to_array()]);
}

/// The active offline home's save file. Progressive disclosure: one home for now
/// (the homes-as-profiles model). Multi-home selection comes with multiplayer.
pub fn active_home_path() -> PathBuf {
    persistence::saves_dir().join("offline_home.json")
}

/// Load the active offline home's save, if it exists + parses. None on first run.
pub fn load_active_home() -> Option<WorldSave> {
    load_home_at(&active_home_path())
}

/// `load_active_home` for a home at any path (tests give it a throwaway one).
fn load_home_at(path: &std::path::Path) -> Option<WorldSave> {
    if !path.exists() {
        return None;
    }
    match persistence::load_world(path) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("load_active_home: {e}");
            None
        }
    }
}

/// Extract the live player's progress (inventory + skills) into a WorldSave.
pub fn extract_world_save(world: &hecs::World) -> WorldSave {
    let mut save = WorldSave::new_offline("My Homestead", "fibonacci");
    save.timestamp = now_secs();
    // The player is the single Controllable entity.
    for (_e, (inv, skills, name, appearance, outfit, _ctrl)) in world
        .query::<(
            &Inventory,
            &PlayerSkills,
            &crate::ecs::components::Name,
            &crate::ecs::components::Appearance,
            &crate::ecs::components::Outfit,
            &Controllable,
        )>()
        .iter()
    {
        save.character_name = name.0.clone();
        // One (item_id, qty) per occupied slot; apply re-stacks via add_item.
        save.inventory = inv
            .slots
            .iter()
            .filter_map(|s| s.as_ref().map(|st| (st.item_id.clone(), st.quantity)))
            .collect();
        save.inventory_state = inv
            .slots
            .iter()
            .filter_map(|s| s.as_ref().map(|st| (st.wear, st.quality)))
            .collect();
        // Each stack's food clock (2026-10-04, first-hour audit S6).
        save.inventory_age = inv.slots.iter().filter_map(|s| s.as_ref().map(|st| st.age_s)).collect();
        save.skills = skills
            .skills
            .iter()
            .map(|(id, p)| (id.clone(), (p.level, p.xp)))
            .collect();
        // Avatar appearance + equipped outfit (v0.440).
        save.appearance = appearance.clone();
        save.outfit = outfit.clone();
        break;
    }
    // Deployed vehicles (economy Phase 2 Stage 1, v0.677): every Vehicle entity's
    // kind + pose, so a parked truck is still there after a restart. A truck a loaded save
    // left on the home's plot that is not the home's keeps its mark (ship homes 1b, round 4).
    use crate::engine::home_plot::{still_not_the_homes, NotTheHomes};
    save.deployed_vehicles = world
        .query::<(
            &crate::ecs::components::Vehicle,
            &crate::ecs::components::Transform,
            Option<&NotTheHomes>,
        )>()
        .iter()
        .map(|(_e, (v, t, mark))| crate::persistence::VehicleSave {
            item_id: v.item_id.clone(),
            position: t.position.to_array(),
            yaw: t.rotation.to_euler(glam::EulerRot::YXZ).0,
            outside_home: still_not_the_homes(mark, t.position),
        })
        .collect();
    // Wallet (v0.747, ladder rung 3): credits survive restarts.
    for (_e, (wallet, _ctrl)) in world
        .query::<(&crate::ecs::components::Wallet, &Controllable)>()
        .iter()
    {
        save.credits = wallet.credits;
        break;
    }
    // The body and the home's urine tank (first-hour audit S1, 2026-10-04): quitting used
    // to heal and refill everything.
    save.body = body_of(world);
    save.urine_tank_person_days = crate::systems::food::urine_tank_level(world);
    // The packs left where the player fell (the Death setting's Realistic mode, 2026-10-04),
    // each with what it holds and the play time it has counted.
    save.left_packs = crate::systems::death_pack::packs(world);
    // Quests (v0.748, ladder rung 4): the tracker round-trips, so progress
    // and completions survive restarts (was: reset fresh every session).
    for (_e, (tracker, _ctrl)) in world
        .query::<(&crate::systems::quests::QuestTracker, &Controllable)>()
        .iter()
    {
        save.quests = Some(tracker.clone());
        break;
    }
    // Crops (v0.863): the whole garden round-trips. planted_at is game-time
    // seconds, so it only means anything next to the clock it was read from:
    // save_active_home stores that clock in save.game_time, and resume_home
    // puts it back on load. (Until 2026-09-25 this comment claimed the clock
    // was saved when nothing wrote it, and every restart rewound the garden.)
    // Each crop with its soil (2026-09-26, N-P-K), and the soil the emptied
    // units remember, so a restart does not refill every unit.
    // And each crop's pollination record (2026-09-26, farming::pollination),
    // and a picked crop's picking state (farming::picking).
    use crate::ecs::components::{CropInstance, CropPicking, CropPollination, CropSoil};
    type Saved = (CropInstance, Option<CropSoil>, Option<CropPollination>, Option<CropPicking>);
    let crops: Vec<Saved> = world
        .query::<(&CropInstance, Option<&CropSoil>, Option<&CropPollination>, Option<&CropPicking>)>()
        .iter()
        .map(|(_e, (c, s, p, k))| (c.clone(), s.cloned(), p.cloned(), k.cloned()))
        .collect();
    save.crop_soil = crops.iter().map(|(_, s, _, _)| s.clone()).collect();
    save.crop_pollination = crops.iter().map(|(_, _, p, _)| p.clone()).collect();
    save.crop_picking = crops.iter().map(|(_, _, _, k)| k.clone()).collect();
    save.crops = crops.into_iter().map(|(c, _, _, _)| c).collect();
    save.soil_memory = world
        .query::<&crate::ecs::components::SoilMemory>()
        .iter()
        .next()
        .map(|(_e, m)| m.clone())
        .unwrap_or_default();
    // Builds (2026-09-25): finished structures AND scaffolds still going up.
    // Until now nothing wrote this field, so everything the player built was
    // gone after a restart even though its materials had been consumed.
    // A piece built on a planet carries its build site (2026-09-27): the
    // pose is then site-local, and the site says which body and where.
    use crate::systems::construction::{fires::FireFuel, Construction, DoorOpen, PlanetSite, Structure};
    let pose = |t: &crate::ecs::components::Transform| {
        (t.position.to_array(), t.rotation.to_array(), t.scale.to_array())
    };
    // A built fire keeps its fuel (BUG-153, 2026-10-05).
    for (_e, (s, t, site, open, mark, fire)) in world
        .query::<(&Structure, &crate::ecs::components::Transform, Option<&PlanetSite>, Option<&DoorOpen>, Option<&NotTheHomes>, Option<&FireFuel>)>()
        .iter()
    {
        let (position, rotation, scale) = pose(t);
        save.constructions.push(crate::persistence::ConstructionSave {
            blueprint_id: s.blueprint_id.clone(),
            position,
            rotation,
            scale,
            health: s.health,
            max_health: s.max_health,
            provides: s.provides.clone(),
            building: None,
            uid: s.uid,
            open: open.is_some(),
            site: site.cloned(),
            outside_home: still_not_the_homes(mark, t.position),
            fire_s: fire.map(|f| f.seconds_left),
        });
    }
    for (_e, (c, t, site, mark)) in world
        .query::<(&Construction, &crate::ecs::components::Transform, Option<&PlanetSite>, Option<&NotTheHomes>)>()
        .iter()
    {
        let (position, rotation, scale) = pose(t);
        save.constructions.push(crate::persistence::ConstructionSave {
            blueprint_id: c.blueprint_id.clone(),
            position,
            rotation,
            scale,
            health: 0.0,
            max_health: 0.0,
            provides: None,
            building: Some((c.progress, c.build_time)),
            uid: 0,
            open: false,
            site: site.cloned(),
            outside_home: still_not_the_homes(mark, t.position),
            fire_s: None,
        });
    }
    // The herd's yield timers, the asteroids as mined down, and the drone in
    // flight with its cargo (2026-09-27, offline progression). Each was
    // rebuilt fresh at every launch: every animal ready again, every asteroid
    // full again, and the ore in a flying drone's hold gone.
    save.herd = crate::systems::livestock::herd_timers(world);
    save.asteroids = Some(
        world
            .query::<&crate::ecs::components::AsteroidBody>()
            .iter()
            .map(|(_e, a)| a.clone())
            .collect(),
    );
    save.drone = world
        .query::<&crate::ecs::components::Drone>()
        .iter()
        .next()
        .map(|(_e, d)| d.clone());
    // What the home's machines hold (2026-09-27): each bank's charge, each
    // tank's litres, each vessel's contents, with any saved contents still
    // held for world entry (systems::machine_levels).
    save.machine_levels = crate::systems::machine_levels::levels(world);
    // The relay trades this backpack has settled (2026-10-02), in the same
    // save as the backpack they changed.
    save.settled_trades = settled_trades(world);
    // And the home's id with the fleet, and the gives whose items have left
    // the backpack and wait for a server's answer (2026-10-04, engine/fleet.rs):
    // saved with the backpack they left, so a give is never in two places.
    if let Some((_e, (ts, _))) = world.query::<(&crate::systems::inventory::TradeSettlements, &Controllable)>().iter().next() {
        save.home_id = ts.home_id.clone();
        save.fleet_held = ts.fleet_held.clone();
    }
    save
}

/// The ids of the trades the player's backpack has settled, sorted
/// (systems::inventory::TradeSettlements). Trades still queued to settle are
/// left out on purpose: their items have not moved yet, so a save that listed
/// them would lose them at a restart.
///
/// KNOWN GAP (2026-10-02): the ids live per SAVE, not per identity, so a
/// completed trade replays into any other home of the same identity whose save
/// does not list it (load that home, and the trade's items arrive there too).
/// That stays until a per-identity trade ledger exists.
fn settled_trades(world: &hecs::World) -> Vec<String> {
    world
        .query::<(&crate::systems::inventory::TradeSettlements, &Controllable)>()
        .iter()
        .next()
        .map(|(_e, (ts, _))| ts.settled.iter().cloned().collect())
        .unwrap_or_default()
}

/// Put a save's settled trades onto the player (2026-10-02). `rewound`: the
/// backpack was just replaced by the save's, so the save's set REPLACES the
/// live one (a trade settled after that save no longer has its items here and
/// must settle again). Otherwise the backpack was left alone and the save's
/// set is ADDED to the live one, and a trade still queued keeps its moves
/// unless the save already counts it settled.
///
/// "Must settle again" is done by `gui::pages::trade::tick`, which settles
/// every completed trade in hand that the backpack has not (2026-10-02, round
/// 2). The relay connects at the main menu, so a trade could settle there and
/// then be undone by Play loading the save; nothing settled it again, and the
/// items were lost. For the same reason a rewind DROPS the queued moves: they
/// were sized against the backpack before the load, and `tick` queues them
/// afresh against the one just loaded.
fn restore_settled_trades(world: &mut hecs::World, save: &WorldSave, rewound: bool) {
    let ids = &save.settled_trades;
    use crate::systems::inventory::TradeSettlements;
    let Some(player) = world.query::<(&Inventory, &Controllable)>().iter().next().map(|(e, _)| e) else {
        return;
    };
    if world.get::<&TradeSettlements>(player).is_err() {
        let _ = world.insert_one(player, TradeSettlements::default());
    }
    if let Ok(mut ts) = world.get::<&mut TradeSettlements>(player) {
        if rewound {
            ts.settled.clear();
            ts.pending.clear();
            // The fleet (2026-10-04): the save's home and the gives its backpack had
            // handed over, and the fleet's record of this home asked for again, so a
            // give the save does not list is settled out of the backpack just put back
            // (engine/fleet.rs). A fresh home (`rewound` false) keeps the live ones.
            ts.home_id = save.home_id.clone();
            ts.fleet_held = save.fleet_held.clone();
            ts.fleet_recheck = true;
        }
        ts.settled.extend(ids.iter().cloned());
        let done = ts.settled.clone();
        ts.pending.retain(|(id, _)| !done.contains(id));
    }
}

/// The player's body for the save (first-hour audit S1, 2026-10-04): health, the vitals,
/// the effects still running and the cause when dead. None when the world has no player
/// body (a test's bare world).
fn body_of(world: &hecs::World) -> Option<persistence::BodySave> {
    use crate::ecs::components::{Dead, Health, StatusEffects, Vitals};
    let mut q = world.query::<(&Health, &Vitals, &StatusEffects, Option<&Dead>, &Controllable)>();
    let (_e, (health, vitals, effects, dead, _)) = q.iter().next()?;
    Some(persistence::BodySave {
        health: health.clone(),
        vitals: vitals.clone(),
        effects: effects.clone(),
        dead: dead.map(|d| if d.cause.is_empty() { "injuries".to_string() } else { d.cause.clone() }),
    })
}

/// Put a save's body on the player (first-hour audit S1, 2026-10-04): as it was saved, dead
/// or alive. A save without one (from before it) gives the body a new character starts with.
/// The save is authoritative here as it is for the backpack, because the character picker
/// loads a save over a live player.
fn restore_body(world: &mut hecs::World, body: Option<&persistence::BodySave>) {
    use crate::ecs::components::{Dead, Health, StatusEffects, Vitals};
    let Some(p) = world.query::<(&Health, &Vitals, &StatusEffects, &Controllable)>().iter().next().map(|(e, _)| e)
    else {
        return;
    };
    let (health, vitals, effects, death) = match body {
        Some(b) => (b.health.clone(), b.vitals.clone(), b.effects.clone(), b.death()),
        None => (Health::default(), Vitals::default(), StatusEffects::default(), None),
    };
    if let Ok((h, v, fx)) = world.query_one_mut::<(&mut Health, &mut Vitals, &mut StatusEffects)>(p) {
        *h = health;
        *v = vitals;
        *fx = effects;
    }
    match death {
        Some(cause) => {
            let _ = world.insert_one(p, Dead { cause, ..Default::default() });
        }
        None => {
            let _ = world.remove_one::<Dead>(p);
        }
    }
}

/// Apply a loaded WorldSave's inventory + skills + vehicles + crops + the body
/// (first-hour audit S1) onto the live world. Position is left to the world
/// load: the home's front door. Idempotent; called at startup and on
/// character select.
pub fn apply_save_to_world(world: &mut hecs::World, save: &WorldSave) {
    // The live world now comes from a save on disk again, so a restored
    // snapshot that was waiting for this may be saved over (see
    // RESTORED_SAVE_WAITING). Released before the kind check: holding saves
    // for a load that can never apply would stop saving for the whole session.
    release_restore_hold();
    // Only offline homes are supported today.
    if save.kind != "offline" {
        return;
    }
    for (_e, (inv, skills, name, appearance, outfit, _ctrl)) in world.query_mut::<(
        &mut Inventory,
        &mut PlayerSkills,
        &mut crate::ecs::components::Name,
        &mut crate::ecs::components::Appearance,
        &mut crate::ecs::components::Outfit,
        &Controllable,
    )>() {
        if !save.character_name.is_empty() {
            name.0 = save.character_name.clone();
        }
        // Rebuild inventory: clear every slot, then add_item re-stacks.
        for slot in inv.slots.iter_mut() {
            *slot = None;
        }
        // GROW to fit before re-adding (v0.692 review fix): the v0.687 delivery
        // fix legitimately grows the backpack past its base 36 slots, so a save
        // can hold more stacks than Inventory::new(36) offers -- and add_item's
        // discarded overflow here silently ate the excess on the NEXT restart,
        // undoing the never-lose-a-haul guarantee one launch later. Mirror the
        // delivery-site pattern: ensure the slots, then land everything.
        // Each saved stack comes back exactly as it was, with its wear and
        // grade (2026-09-26) and its food's age (2026-10-04); a stack an
        // older save has no state for comes back unworn, ungraded and fresh.
        inv.ensure_slots(save.inventory.len());
        for (i, (item_id, qty)) in save.inventory.iter().enumerate() {
            let (wear, quality) = save.inventory_state.get(i).copied().unwrap_or((0, 0));
            let mut stack = crate::systems::inventory::ItemStack::new(item_id.clone(), *qty, (*qty).max(99));
            stack.wear = wear;
            stack.quality = quality;
            stack.age_s = save.inventory_age.get(i).copied().filter(|a| a.is_finite()).unwrap_or(0.0);
            inv.slots[i] = Some(stack);
        }
        // Rebuild skills.
        skills.skills.clear();
        for (id, (level, xp)) in &save.skills {
            skills
                .skills
                .insert(id.clone(), SkillProgress { level: *level, xp: *xp });
        }
        // Restore avatar appearance + outfit (v0.440).
        *appearance = save.appearance.clone();
        *outfit = save.outfit.clone();
        break;
    }
    // Wallet (v0.747): -1 = a pre-wallet save; keep the fresh-start default.
    if save.credits >= 0 {
        for (_e, (wallet, _ctrl)) in
            world.query_mut::<(&mut crate::ecs::components::Wallet, &Controllable)>()
        {
            wallet.credits = save.credits;
            break;
        }
    }
    // The body as it was left, and the home's urine tank (first-hour audit S1).
    restore_body(world, save.body.as_ref());
    crate::systems::food::set_urine_tank(world, save.urine_tank_person_days);
    // The packs left where the player fell, as saved: authoritative like the backpack they
    // came out of, and not advanced by the time away (a pack counts only play).
    crate::systems::death_pack::restore(world, &save.left_packs);
    // Settled trades (2026-10-02) travel with the backpack just rebuilt.
    restore_settled_trades(world, save, true);
    // Quests (v0.748): a saved tracker replaces the fresh spawn default
    // (which auto-accepted gs_first_steps); None keeps the fresh start.
    if let Some(saved) = &save.quests {
        for (_e, (tracker, _ctrl)) in
            world.query_mut::<(&mut crate::systems::quests::QuestTracker, &Controllable)>()
        {
            *tracker = saved.clone();
            break;
        }
    }
    // Deployed vehicles (economy Phase 2 Stage 1): the save is AUTHORITATIVE,
    // exactly like inventory above (clear every slot, then rebuild). Despawn
    // every existing Vehicle, then respawn the saved set. This matters because
    // this fn is NOT startup-only: the launcher's character select (lib.rs
    // "launcher_pending_load") re-applies a save onto the live world, so an
    // add-without-clear here would leak vehicles across saves, and the earlier
    // same-pose skip guard silently collapsed two identically-parked vehicles
    // (deploy twice without moving) into one on reload (v0.678 review fix).
    let existing: Vec<hecs::Entity> = world
        .query_mut::<&crate::ecs::components::Vehicle>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for vs in &save.deployed_vehicles {
        // Same tuple VehicleSystem::handle_deploy spawns, minus Name (the display
        // name lives in the kit registry, which this fn deliberately has no access
        // to; nothing reads a vehicle's Name yet — revisit when nameplates land).
        let vehicle = world.spawn((
            crate::ecs::components::Vehicle { item_id: vs.item_id.clone() },
            crate::ecs::components::Transform {
                position: glam::Vec3::from_array(vs.position),
                rotation: glam::Quat::from_rotation_y(vs.yaw),
                scale: glam::Vec3::ONE,
            },
            crate::ecs::components::Velocity::default(),
            crate::ecs::components::VehicleSeat {
                occupant_key: None,
                seat_type: "pilot".to_string(),
            },
        ));
        // Not the home's, though it stands where the home stood (ship homes 1b, round 4).
        if vs.outside_home {
            let at = glam::Vec3::from_array(vs.position);
            let _ = world.insert_one(vehicle, crate::engine::home_plot::NotTheHomes { at });
        }
    }
    // Crops (v0.863): save is AUTHORITATIVE, same clear-then-rebuild rule as
    // vehicles above (this fn re-applies on character select, not just boot).
    let existing: Vec<hecs::Entity> = world
        .query_mut::<&crate::ecs::components::CropInstance>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for (i, c) in save.crops.iter().enumerate() {
        let e = world.spawn((c.clone(),));
        if let Some(soil) = save.crop_soil.get(i).cloned().flatten() {
            let _ = world.insert_one(e, soil);
        }
        if let Some(pollination) = save.crop_pollination.get(i).cloned().flatten() {
            let _ = world.insert_one(e, pollination);
        }
        if let Some(picking) = save.crop_picking.get(i).cloned().flatten() {
            let _ = world.insert_one(e, picking);
        }
    }
    // One soil memory per world (2026-09-26): the save's replaces the live one.
    let old: Vec<hecs::Entity> = world
        .query::<&crate::ecs::components::SoilMemory>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in old {
        let _ = world.despawn(e);
    }
    world.spawn((save.soil_memory.clone(),));
    // Builds (2026-09-25): authoritative like crops and vehicles. The
    // ConstructionSystem finishes a restored scaffold on its own tick,
    // with the usual completion events, once progress reaches build_time.
    use crate::systems::construction::{Construction, Structure};
    let existing: Vec<hecs::Entity> = world
        .query_mut::<hecs::Or<&Structure, &Construction>>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for b in &save.constructions {
        let transform = crate::ecs::components::Transform {
            position: glam::Vec3::from_array(b.position),
            rotation: glam::Quat::from_array(b.rotation),
            scale: glam::Vec3::from_array(b.scale),
        };
        let piece = match b.building {
            Some((progress, build_time)) => world.spawn((
                transform,
                Construction {
                    blueprint_id: b.blueprint_id.clone(),
                    progress,
                    build_time,
                    builder_key: None,
                },
            )),
            None => world.spawn((
                transform,
                Structure {
                    blueprint_id: b.blueprint_id.clone(),
                    health: b.health,
                    max_health: b.max_health,
                    provides: b.provides.clone(),
                    uid: b.uid,
                },
            )),
        };
        // Back into its planet build site, when it was built on a planet.
        if let Some(site) = &b.site {
            let _ = world.insert_one(piece, site.clone());
        }
        // A door that was left open is open again.
        if b.open && b.building.is_none() {
            let _ = world.insert_one(piece, crate::systems::construction::DoorOpen);
        }
        // A built fire comes back with the fuel it had (BUG-153).
        if let (Some(fire_s), None) = (b.fire_s, b.building) {
            let _ = world.insert_one(piece, crate::systems::construction::fires::FireFuel { seconds_left: fire_s.max(0.0) });
        }
        // Not the home's, though it stands where the home stood (ship homes 1b, round 4).
        if b.outside_home {
            let at = glam::Vec3::from_array(b.position);
            let _ = world.insert_one(piece, crate::engine::home_plot::NotTheHomes { at });
        }
    }
    // Asteroids (2026-09-27): authoritative once recorded, so what was mined
    // stays mined and a mined-out asteroid stays gone. None (a save from
    // before) keeps the fresh set.
    use crate::ecs::components::{AsteroidBody, Drone};
    if let Some(saved) = &save.asteroids {
        let existing: Vec<hecs::Entity> = world.query_mut::<&AsteroidBody>().into_iter().map(|(e, _)| e).collect();
        for e in existing {
            let _ = world.despawn(e);
        }
        for a in saved {
            world.spawn((a.clone(),));
        }
    }
    // The drone in flight (2026-09-27): authoritative like the vehicles, and
    // its home is this player's entity now, not the one it was saved with.
    let existing: Vec<hecs::Entity> = world.query_mut::<&Drone>().into_iter().map(|(e, _)| e).collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    if let Some(d) = &save.drone {
        let home = world
            .query_mut::<(&Inventory, &Controllable)>()
            .into_iter()
            .next()
            .map(|(e, _)| e.to_bits().get());
        if let Some(home) = home {
            world.spawn((Drone { home, ..d.clone() },));
        }
    }
    // What the home's machines hold (2026-09-27): onto the machines by
    // instance id now, and a vessel's contents held until world entry spawns
    // the vessel (the menu-mode machines carry none). Not advanced by the
    // time away: a bank, a tank and a drum come back as saved.
    crate::systems::machine_levels::restore(world, &save.machine_levels);
}

/// A NEW player's starting kit: `starting_items` in data/world/player.ron
/// (the data dir first, the embedded copy otherwise). Empty when the file
/// is missing or does not parse, so a broken data file never blocks a boot.
pub fn starting_kit(data_dir: &std::path::Path) -> Vec<(String, u32)> {
    #[derive(serde::Deserialize)]
    struct PlayerDef {
        #[serde(default)]
        starting_items: Vec<(String, u32)>,
    }
    crate::embedded_data::read_data_or_embedded(data_dir, "world/player.ron")
        .and_then(|text| match ron::from_str::<PlayerDef>(&text) {
            Ok(def) => Some(def.starting_items),
            Err(e) => {
                log::warn!("world/player.ron did not parse, starting kit empty: {e}");
                None
            }
        })
        .unwrap_or_default()
}

/// Apply ONLY the character (name, look, outfit) from a save: the path for
/// "Start every session from the default home" and for a save that carries
/// no progress (`progress_saved == false`). The home, inventory, garden,
/// builds and clock stay the default.
pub fn apply_identity(world: &mut hecs::World, save: &WorldSave) {
    // Same as apply_save_to_world: the world now matches a save from disk.
    release_restore_hold();
    for (_e, (name, appearance, outfit, _ctrl)) in world.query_mut::<(
        &mut crate::ecs::components::Name,
        &mut crate::ecs::components::Appearance,
        &mut crate::ecs::components::Outfit,
        &Controllable,
    )>() {
        if !save.character_name.is_empty() {
            name.0 = save.character_name.clone();
        }
        *appearance = save.appearance.clone();
        *outfit = save.outfit.clone();
        break;
    }
    // Settled trades (2026-10-02) are kept even here. A fresh home discards
    // what a trade brought along with the rest of the session, as the setting
    // says; replaying every old trade into each fresh home instead would make
    // traded goods the one thing that carried over.
    restore_settled_trades(world, save, false);
}

/// The save to write when progress is NOT being kept: the existing save
/// with only the character replaced, so the progress on disk survives
/// untouched for when the setting is turned off. With no save yet, a
/// character-only save marked `progress_saved: false`. The settled trades
/// are added too (2026-10-02): they are not progress but a record of what the
/// relay already settled, kept so a fresh home never replays an old trade.
pub fn identity_only_save(existing: Option<WorldSave>, world: &hecs::World) -> WorldSave {
    let current = extract_world_save(world);
    let mut save = existing.unwrap_or_else(|| {
        let mut s = WorldSave::new_offline("My Homestead", "fibonacci");
        s.progress_saved = false;
        s
    });
    save.character_name = current.character_name;
    save.appearance = current.appearance;
    save.outfit = current.outfit;
    // And the settled trades, added to what the save had (see apply_identity).
    let mut settled = std::mem::take(&mut save.settled_trades);
    settled.extend(current.settled_trades);
    settled.sort();
    settled.dedup();
    save.settled_trades = settled;
    save
}

/// Extract + write the active offline home to disk. Logs on failure. `placed` is the
/// organize-layer container pool (GuiState-owned, not in the ECS world), persisted
/// alongside the world-derived save so container contents + transfers survive a restart.
/// `own_home` is the character's own home (`GuiState::own_home`, engine/own_home.rs): written
/// into a save that keeps progress as it is given, None for a character living in the default
/// home. A save that keeps only the character leaves the save's own home as it was on disk.
pub fn save_active_home(
    world: &hecs::World,
    placed: &[crate::systems::inventory::placed::PlacedItem],
    data: &crate::hot_reload::data_store::DataStore,
    keep_progress: bool,
    own_home: Option<&crate::persistence::SavedHome>,
) {
    save_home_at(&active_home_path(), world, placed, data, keep_progress, own_home);
}

/// `save_active_home` with the save file given, so a test can point it at a
/// throwaway path and see exactly what a save writes (or, while a restore is
/// waiting to load, that it writes nothing). The game only ever calls it
/// through `save_active_home`, with `active_home_path()`.
pub(crate) fn save_home_at(
    path: &std::path::Path,
    world: &hecs::World,
    placed: &[crate::systems::inventory::placed::PlacedItem],
    data: &crate::hot_reload::data_store::DataStore,
    keep_progress: bool,
    own_home: Option<&crate::persistence::SavedHome>,
) {
    if restored_save_waiting() {
        // Settings > Data restored a snapshot over the active home and asked
        // for it to be loaded into the world; until that load happens the
        // live world still holds what the restore replaced, and saving it now
        // would undo the restore. Both kinds of save are held, and so is the
        // snapshot a save would keep first (that happens inside save_world).
        log::warn!("save_active_home: a restored save is waiting to be loaded into the world, not saving over it");
        return;
    }
    if !keep_progress {
        // "Start every session from the default home": record the character,
        // leave any progress save exactly as it was.
        let save = identity_only_save(load_home_at(path), world);
        if let Err(e) = persistence::save_world(path, &save) {
            log::error!("save_active_home (character only) failed: {e}");
        }
        return;
    }
    let mut save = extract_world_save(world);
    // Where the home stood, beside its pieces and vehicles where they stand (`HomeFrame`).
    record_home_frame(&mut save, frame_for_save(data).as_ref());
    save.placed_items = Some(placed.to_vec());
    // The character's own home, as built outside the Dev mode (engine/own_home.rs), or none.
    save.home = own_home.cloned();
    // The world clock, from the TimeSystem's DataStore export. Crop
    // planted_at values are only meaningful against it.
    save.game_time = crate::systems::time::elapsed_now(data);
    save.progress_saved = true;
    // Craft batches in flight, from the CraftingSystem's export (the list
    // lives inside the system). Their inputs are already spent.
    save.crafts = data
        .get::<std::sync::Mutex<Vec<crate::systems::crafting::CraftSave>>>("active_crafts_export")
        .and_then(|m| m.lock().ok().map(|v| v.clone()))
        .unwrap_or_default();
    // The drone's standing order lives in the DataStore (2026-09-27).
    save.mining_order = crate::systems::mining::standing_order(data);
    // So does the ship supply ledger (systems::ship_power): the reactor's kWh.
    save.ship_supply = crate::systems::ship_power::ledger(data);
    // Before world entry the herd is not spawned yet and its saved timers
    // are still waiting for it: keep those rather than forget them.
    if save.herd.is_empty() {
        save.herd = crate::systems::livestock::pending_herd(data).unwrap_or_default();
    }
    if let Err(e) = persistence::save_world(path, &save) {
        log::error!("save_active_home failed: {e}");
    } else {
        log::info!(
            "Saved offline home: {} item stacks, {} skills",
            save.inventory.len(),
            save.skills.len()
        );
    }
}

// ── Restoring a save snapshot while the game runs (2026-10-03) ──────────────
//
// Settings > Data > "Save snapshots" can put an earlier copy of the active home
// back (persistence::restore_snapshot). The file on disk is then the restored
// home, but the live world still holds what it replaced, and the next periodic
// save (or the save on quit) would write that straight back over the restore.
// So a restore of the active home (1) asks for the restored file to be loaded
// into the world, through the same `launcher_pending_load` path the character
// picker uses (lib.rs applies it once the world is up, rewinding crafts and
// catching up the time since the snapshot like any time away), and (2) holds
// `save_active_home` off until that load has happened. The load ends in
// `apply_save_to_world` or `apply_identity`, which release the hold. Restored
// from the main menu before the world was ever entered and then quit: the hold
// keeps the quit-save off the restored file, and the next launch loads it.
//
// Thread-local on purpose: every save, every restore click and every apply
// runs on the one main (event loop) thread, and a thread-local keeps each
// test's hold its own, where a process-wide flag would be released at random
// by the many tests that call apply_save_to_world in parallel. If saving ever
// moves to another thread, this has to move with it.
thread_local! {
    static RESTORED_SAVE_WAITING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// True while a restored active home is waiting to be loaded into the world
/// (saves of the active home are held off until then). Settings shows it.
pub fn restored_save_waiting() -> bool {
    RESTORED_SAVE_WAITING.with(|w| w.get())
}

fn hold_saves_for_restore() {
    RESTORED_SAVE_WAITING.with(|w| w.set(true));
}

fn release_restore_hold() {
    RESTORED_SAVE_WAITING.with(|w| w.set(false));
}

/// Settings > Data "Restore": put `snapshot` back as the save at `save_path`
/// (persistence::restore_snapshot keeps a snapshot of what it replaces), and
/// when that save is the active home, load it into the running game. Returns
/// the line to show the player.
pub fn restore_snapshot_into_game(
    gui: &mut crate::gui::GuiState,
    snapshot: &std::path::Path,
    save_path: &std::path::Path,
) -> Result<String, String> {
    let restored = persistence::restore_snapshot(snapshot, save_path, persistence::now_ms())?;
    if save_path == active_home_path() {
        hold_saves_for_restore();
        // lib.rs finds the save by this name among the saves and applies it.
        gui.launcher_pending_load = Some(restored.name.clone());
        // With "Start every session from the default home" on (off by default
        // since 2026-10-04) the load applies only the character, so say so:
        // the restored home is safe on disk (a character-only save leaves the
        // progress in the file untouched), it just is not what you are playing.
        if gui.settings.fresh_world_each_launch {
            Ok("Restored on disk. Because \"Start every session from the default home\" is on (Settings > Gameplay), only your character is loaded now; turn it off and the restored home is what you play. The save it replaced is kept among the snapshots.".to_string())
        } else {
            Ok("Restored. Your home is being loaded back into the game; the save it replaced is kept among the snapshots.".to_string())
        }
    } else {
        Ok(format!(
            "Restored {}. The save it replaced is kept among the snapshots.",
            save_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        ))
    }
}

/// What a "Snapshot now" click did, for the line Settings shows.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotNow {
    /// A new copy was kept, at this path.
    Kept(PathBuf),
    /// The save has not changed since the last copy kept, so that copy
    /// already holds it and nothing new was taken (a new one would only push
    /// the oldest copy out).
    AlreadyKept,
    /// There is no save of the home yet, so there is nothing to keep.
    NotSavedYet,
}

/// Settings > Data "Snapshot now": keep a copy of the active home as it was
/// last saved (the game saves every two minutes and on quit), whatever the
/// spacing, unless the save is unchanged since the last copy.
pub fn snapshot_active_home_now() -> Result<SnapshotNow, String> {
    snapshot_home_now_at(&active_home_path(), persistence::now_ms())
}

/// `snapshot_active_home_now` for a save at any path and moment (tests).
fn snapshot_home_now_at(path: &std::path::Path, now_ms: u64) -> Result<SnapshotNow, String> {
    if !path.is_file() {
        return Ok(SnapshotNow::NotSavedYet);
    }
    // Forced, so the only way it keeps nothing for a save that exists is the
    // save being unchanged since the last copy (persistence::snapshot_save).
    Ok(match persistence::snapshot_save(path, &persistence::snapshots_root_for(path), now_ms, true)? {
        Some(kept) => SnapshotNow::Kept(kept),
        None => SnapshotNow::AlreadyKept,
    })
}

/// Whether the periodic save of the offline home is due: at most once per `interval_secs` of
/// wall-clock time. Call every frame from the main loop, and save when it says so; it
/// self-throttles. Robust to ANY exit path (in-app quit, crash, kill) where the graceful
/// close-save would not fire. A question rather than the save itself (2026-10-04) so the main
/// loop can bring the character's own home up to date first (engine/own_home.rs
/// `refresh_own_home`) only when a save is actually about to be written.
pub fn periodic_save_due(interval_secs: u64) -> bool {
    let now = now_secs();
    let last = LAST_SAVE_SECS.load(Ordering::Relaxed);
    if last == 0 {
        // First call: arm the timer; do NOT save immediately (avoids writing an
        // empty home before any play happens on a fresh first run).
        LAST_SAVE_SECS.store(now, Ordering::Relaxed);
        return false;
    }
    if now.saturating_sub(last) >= interval_secs {
        LAST_SAVE_SECS.store(now, Ordering::Relaxed);
        return true;
    }
    false
}

/// What `resume_home` did, for the log and the "while you were away" notice.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Resumed {
    /// The game clock the world resumes at, in game seconds.
    pub clock: f64,
    /// Game seconds of offline catch-up applied (0 when the toggle is off).
    pub away_secs: f64,
    /// Living crops that were aged by `away_secs`.
    pub crops_aged: usize,
    /// Builds still under construction that were advanced by `away_secs`.
    pub builds_advanced: usize,
    /// Craft batches in flight that were advanced by `away_secs`.
    pub crafts_advanced: usize,
    /// Homestead animals still regrowing their yield at the save that are
    /// ready to collect from again after `away_secs` (2026-09-27).
    pub animals_ready: usize,
    /// Hauls the drone brought home during `away_secs` (2026-09-27).
    pub drone_hauls: usize,
}

/// Offline progression (operator, 2026-09-21; docs/design/offline-progression.md).
/// Pure part of `resume_home`, so it can be tested without a DataStore.
///
/// The CLOCK always resumes where the save left it. It is never jumped forward
/// by the time away, because every system that reads the clock would then
/// advance offline by accident, and the design doc requires the opposite: a
/// system advances offline only by deliberately opting in here. The clock is
/// also never behind the newest crop (a crop cannot have been planted in the
/// future), which heals a save written before the clock was stored.
///
/// Opted in today (crops, and builds under construction):
/// - CROPS: when `offline_progression` is on, every living crop's planted_at
///   moves back by the time away, so its age grows by exactly that much and
///   the growth-speed setting applies to it like any other hour. Water and
///   health are integrated per tick and are NOT advanced: the character keeps
///   the garden watered while you are away (the doc's "offline upkeep"), so
///   nothing can die of thirst while nobody could have prevented it.
///
/// - BUILDS: scaffolds still going up advance by the time away (see below).
/// - CRAFTS: batches in flight count down by the time away and deliver on
///   the CraftingSystem's next tick (see `restored_crafts`).
/// - SOIL pH (2026-09-27): lime, sulfur and nitrifying ammonium that were
///   still reacting keep reacting. `resume_home` hands the time away to the
///   farming tick (`farming::soil_ph::hand_away_secs`), which steps it in
///   garden days.
/// - THE DRONE (2026-09-27, in `resume_home`): the trip in flight finishes and
///   a standing order keeps flying, bounded by the asteroid's real ore
///   (`mining::advance_away`).
/// - AUTOMATED MACHINES (2026-09-27): handed the time away, they run it
///   through on the inputs really on hand and the power the home could spare,
///   once they exist (`crafting::away`).
/// - LIVESTOCK (2026-09-27, in `resume_home`): each animal's yield timer
///   moves on by the time away, to one yield waiting, as when you are home
///   and do not collect (`livestock::timers_after_away`).
/// - FIRES (2026-10-05, BUG-153): a campfire left burning burns its fuel
///   down by the time away (`construction::fires::burn`), so it is out when
///   the player comes back the next day. Its logs were spent when they went
///   on the fire.
///
/// Deliberately not advanced: the body (saved since 2026-10-04, first-hour
/// audit S1, and put back exactly as it was left: the time away costs no
/// food, water or sleep and runs no effect's timer) and the urine tank, a pack
/// left where the player fell (2026-10-04, systems::death_pack: it stays for
/// minutes of PLAY, so quitting never loses it), and anything that consumes
/// or destroys. That includes garden PESTS
/// (2026-09-26): their pressure costs crop health and the player could not
/// have answered it while away, so it resumes where the save left it and the
/// character's upkeep kept them down in the meantime. Nor does a picked
/// crop's picking window (2026-09-26, farming::picking): its clock runs on
/// the tick, so produce the player could not have picked while away does
/// not pass over, and the window resumes where the save left it. A crop's soil draw is
/// not in that class: the growth made offline is paid for from its unit on
/// the first tick back (farming's uptake), which is the crop's own feeding.
///
/// Clock source: the device clock, because only offline single-player homes
/// are saved today. Multiplayer and MMO must use the SERVER clock instead
/// (the design doc's cheating section) when their saves exist.
///
/// The time away is counted on the one game clock (2026-09-27): real
/// seconds away times the player's `time_speed` setting, the same rate the
/// world ran at while they played, so a garden at 72x keeps growing at 72x
/// while the game is closed. (A dev hold, the F11 freeze or a sleep, is not
/// the setting and does not stretch it.) No cap: a returning player finding
/// a finished garden is the point (the doc's open question, unbounded until
/// something misbehaves).
pub fn catch_up_world(
    world: &mut hecs::World,
    save: &WorldSave,
    offline_progression: bool,
    time_speed: f32,
    now: u64,
) -> Resumed {
    let newest_planting = world
        .query_mut::<&crate::ecs::components::CropInstance>()
        .into_iter()
        .map(|(_e, c)| c.planted_at)
        .fold(0.0_f64, f64::max);
    let clock = save.game_time.max(newest_planting).max(0.0);
    // timestamp 0 = never stamped by a save, so there is no "away" to measure.
    // A clock set backwards gives zero, never negative growth.
    let away_secs = if offline_progression && save.timestamp > 0 {
        now.saturating_sub(save.timestamp) as f64 * f64::from(crate::systems::time::clamp_time_speed(time_speed))
    } else {
        0.0
    };
    let mut crops_aged = 0;
    let mut builds_advanced = 0;
    if away_secs > 0.0 {
        for (_e, crop) in world.query_mut::<&mut crate::ecs::components::CropInstance>() {
            if crop.growth_stage == crate::ecs::components::STAGE_DEAD {
                continue;
            }
            crop.planted_at -= away_secs;
            crops_aged += 1;
        }
        // BUILDS: a scaffold's materials were consumed when it started, so
        // finishing it offline consumes nothing (the doc's "reserve inputs up
        // front" rule). Progress is a countdown in seconds; it is capped at
        // build_time so the ConstructionSystem's next tick does the
        // completion itself, quest event, skill XP and all.
        for (_e, c) in world.query_mut::<&mut crate::systems::construction::Construction>() {
            c.progress = (c.progress as f64 + away_secs).min(c.build_time as f64) as f32;
            builds_advanced += 1;
        }
        // FIRES (BUG-153): a campfire left burning burns down while the game
        // is closed, as it would in life. Its fuel was spent when it went on
        // the fire, so this only takes away burning the player paid for.
        crate::systems::construction::fires::burn(world, away_secs.min(f64::from(f32::MAX)) as f32);
    }
    let crafts_advanced = if away_secs > 0.0 { save.crafts.len() } else { 0 };
    Resumed { clock, away_secs, crops_aged, builds_advanced, crafts_advanced, ..Default::default() }
}

/// The craft batches a save resumes with, each counted down by the time away
/// (floored at zero, which completes it on the next tick through the normal
/// delivery path, vehicle pad and machine vessel included). Their inputs were
/// spent when they started, so finishing them offline spends nothing; this is
/// the design doc's "reserve inputs up front" rule, already true of crafting.
pub fn restored_crafts(save: &WorldSave, away_secs: f64) -> Vec<crate::systems::crafting::CraftSave> {
    save.crafts
        .iter()
        .map(|c| {
            let mut c = c.clone();
            c.time_remaining = (c.time_remaining as f64 - away_secs).max(0.0) as f32;
            c
        })
        .collect()
}

/// The one-line "while you were away" notice, or None when nothing grew. The
/// catch-up must never be silent: a garden that jumped forward with no word
/// reads as a bug, not as the character having lived the hours.
pub fn away_notice(r: &Resumed) -> Option<String> {
    if r.away_secs < 60.0 {
        return None;
    }
    let mins = (r.away_secs / 60.0) as u64;
    let span = if mins >= 48 * 60 {
        format!("{} days", mins / (24 * 60))
    } else if mins >= 60 {
        format!("{} h {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    };
    let count = |n: usize, one: &str, many: &str| {
        if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
    };
    let mut parts = Vec::new();
    if r.crops_aged > 0 {
        parts.push(format!("{} kept growing", count(r.crops_aged, "plant", "plants")));
    }
    if r.builds_advanced > 0 {
        parts.push(format!("{} kept going up", count(r.builds_advanced, "build", "builds")));
    }
    if r.crafts_advanced > 0 {
        parts.push(format!("{} kept working", count(r.crafts_advanced, "craft", "crafts")));
    }
    if r.animals_ready > 0 {
        parts.push(format!("{} ready to collect from again", count(r.animals_ready, "animal was", "animals were")));
    }
    if r.drone_hauls > 0 {
        parts.push(format!("the drone brought home {}", count(r.drone_hauls, "haul", "hauls")));
    }
    if parts.is_empty() {
        return None;
    }
    let list = crate::systems::crafting::away::join_list(&parts);
    Some(format!("While you were away ({span}), {list}."))
}

/// The GUI half of applying a save (lib.rs, at startup and on character
/// select, right after `resume_home`): the home's storage comes back with the
/// save (a character select used to keep the previous home's Barn, which let
/// the rewound backpack and the kept Barn both hold the same goods), the
/// "while you were away" notice, and the Mining panel's "Keep mining" switch
/// set to the standing order the save carried (the frame bridge ends the
/// order while the switch is off). A save that never wrote a pool (None, a
/// fresh character) keeps the seeded default, as at startup; a pool saved
/// empty comes back empty.
pub fn after_resume(gui: &mut crate::gui::GuiState, save: &WorldSave, r: &Resumed) {
    if let Some(pool) = &save.placed_items {
        gui.placed_items = pool.clone();
    }
    if let Some(msg) = away_notice(r) {
        gui.pending_notices.push(msg);
    }
    gui.auto_mine_enabled = save.mining_order.is_some();
    gui.prev_auto_mine_enabled = gui.auto_mine_enabled;
    if save.mining_order.is_some() {
        gui.last_drone_order = save.mining_order.clone();
    }
    // The body came back with the save (first-hour audit S1): a save written dead comes
    // back to the death screen with its cause, and a living one clears it.
    gui.player_death_cause = save.body.as_ref().and_then(persistence::BodySave::death);
    // And what that death cost is worked out again from the world the save just made (the
    // pack it left, if any: engine/death_pack.rs after_tick), never a previous death's words.
    gui.death_pack.note = None;
}

/// Put the world clock back where `save` left it and apply the offline
/// catch-up. Call right after `apply_save_to_world` with the same save, at
/// startup and on character select, and after the config is loaded (the
/// toggle lives there). Idempotent per save: both inputs come from disk, so
/// re-applying the same save lands in the same place (`after_resume` puts
/// the home's storage back with it for the same reason). `home` is the
/// placed machine layout, whose Usage meter bounds the power the automated
/// machines may draw while away (`crafting::away::day_power_balance`).
pub fn resume_home(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    save: &WorldSave,
    offline_progression: bool,
    time_speed: f32,
    home: Option<&crate::machines::MachineHome>,
) -> Resumed {
    let mut r = catch_up_world(world, save, offline_progression, time_speed, now_secs());
    crate::systems::time::request_restore_elapsed(data, r.clock);
    // SOIL pH: what was still reacting when the player left kept reacting.
    // Handed over, not stepped here: the farming tick applies it under the
    // player's own Soil pH switch, which reaches the DataStore only after
    // this runs (farming::soil_ph::hand_away_secs).
    crate::systems::farming::soil_ph::hand_away_secs(data, r.away_secs);
    // THE DRONE: its standing order is the player's own setting, so it comes
    // back whether or not the time away counts; then the trip in flight and
    // any the order sends finish in the time away, out of the asteroid's real
    // ore, and each haul lands in home storage as it would have (BUG-150).
    crate::systems::mining::set_standing_order(data, save.mining_order.clone());
    // THE SHIP SUPPLY LEDGER comes back as saved; the time away adds to it
    // when the machines take it (crafting::away::meter_away_reactor).
    crate::systems::ship_power::restore(data, &save.ship_supply);
    let hauls = crate::systems::mining::advance_away(world, data, r.away_secs);
    r.drone_hauls = hauls.len();
    // LIVESTOCK: each animal's timer as saved, moved on by the time away (by
    // nothing when it does not count), onto the herd now or at world entry.
    r.animals_ready = data
        .get::<crate::systems::livestock::CreatureRegistry>("creature_registry")
        .map_or(0, |reg| crate::systems::livestock::readied_by_away(&save.herd, r.away_secs, reg));
    let timers = crate::systems::livestock::timers_after_away(&save.herd, r.away_secs);
    crate::systems::livestock::restore_herd(world, data, timers);
    // AUTOMATED MACHINES: handed the time away with what each was busy with
    // at the save, the home's spare power and the drone's hauls; the
    // CraftingSystem runs them through it once they exist (crafting::away).
    let work = (r.away_secs > 0.0).then(|| crate::systems::crafting::away::AwayWork {
        secs: r.away_secs,
        busy: save
            .crafts
            .iter()
            .filter(|c| c.auto)
            .filter_map(|c| c.machine_id.clone().map(|id| (id, f64::from(c.time_remaining))))
            .collect(),
        power_balance_w: home.map_or([0.0; 2], crate::systems::crafting::away::day_power_balance),
        hauls,
    });
    crate::systems::crafting::away::hand_over(data, work);
    // Craft batches go through the CraftingSystem's restore channel, which
    // it consumes after its rewind drop, so a character select replaces the
    // live batches with the saved ones instead of losing both.
    if let Some(slot) = data
        .get::<std::sync::Mutex<Option<Vec<crate::systems::crafting::CraftSave>>>>("restore_active_crafts")
    {
        if let Ok(mut s) = slot.lock() {
            *s = Some(restored_crafts(save, r.away_secs));
        }
    }
    log::info!(
        "Resumed home clock at game second {:.0}; offline catch-up {} ({:.0} s away, {} crops aged, {} builds and {} crafts advanced, {} animals ready again, {} drone hauls)",
        r.clock,
        if offline_progression { "on" } else { "off" },
        r.away_secs,
        r.crops_aged,
        r.builds_advanced,
        r.crafts_advanced,
        r.animals_ready,
        r.drone_hauls
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_apply_round_trips_inventory_and_skills() {
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("wood_plank_0", 40, 99);
        inv.add_item("steel_ingot_0", 8, 99);
        let mut skills = PlayerSkills::new();
        skills
            .skills
            .insert("farming".to_string(), SkillProgress { level: 3, xp: 450 });
        let mut appearance = crate::ecs::components::Appearance::default();
        appearance.skin_tone = [0.4, 0.3, 0.2];
        appearance.height_scale = 1.2;
        let mut outfit = crate::ecs::components::Outfit::default();
        outfit.equipped.insert("chest".to_string(), "work_jacket".to_string());
        world.spawn((
            Controllable,
            inv,
            skills,
            crate::ecs::components::Name("Astra".to_string()),
            appearance,
            outfit,
        ));

        let save = extract_world_save(&world);
        assert!(save.inventory.iter().any(|(id, q)| id == "wood_plank_0" && *q == 40));
        assert_eq!(save.skills.get("farming").copied(), Some((3, 450)));
        assert_eq!(save.character_name, "Astra");
        assert_eq!(save.appearance.skin_tone, [0.4, 0.3, 0.2]);
        assert_eq!(save.outfit.equipped.get("chest").map(|s| s.as_str()), Some("work_jacket"));

        // Wipe the live state, then apply the save back.
        for (_e, (inv, skills, _c)) in
            world.query_mut::<(&mut Inventory, &mut PlayerSkills, &Controllable)>()
        {
            for s in inv.slots.iter_mut() {
                *s = None;
            }
            skills.skills.clear();
        }
        apply_save_to_world(&mut world, &save);

        // Verify the restore via a fresh extract.
        let restored = extract_world_save(&world);
        let wood: u32 = restored
            .inventory
            .iter()
            .filter(|(id, _)| id == "wood_plank_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(wood, 40);
        assert_eq!(restored.skills.get("farming").copied(), Some((3, 450)));
        // Appearance + outfit survive the round-trip too (v0.440).
        assert_eq!(restored.character_name, "Astra");
        assert_eq!(restored.appearance.skin_tone, [0.4, 0.3, 0.2]);
        assert_eq!(restored.appearance.height_scale, 1.2);
        assert_eq!(restored.outfit.equipped.get("chest").map(|s| s.as_str()), Some("work_jacket"));
    }

    #[test]
    fn apply_ignores_non_offline_kind() {
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("wood_plank_0", 5, 99);
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));

        let mut save = WorldSave::new_offline("X", "fibonacci");
        save.kind = "server".to_string();
        save.inventory = vec![("steel_ingot_0".to_string(), 99)];
        apply_save_to_world(&mut world, &save); // should be a no-op

        let after = extract_world_save(&world);
        // Untouched: still the original wood, no injected steel.
        assert!(after.inventory.iter().any(|(id, q)| id == "wood_plank_0" && *q == 5));
        assert!(!after.inventory.iter().any(|(id, _)| id == "steel_ingot_0"));
    }


    /// Range-review fix (v0.692): a GROWN backpack (the v0.687 delivery fix
    /// legitimately expands past 36 slots) must round-trip the save -- the
    /// load path used to rebuild into Inventory::new(36) and discard
    /// add_item's overflow, silently eating the extra stacks one restart
    /// after the delivery rescued them.
    #[test]
    fn grown_backpack_saves_round_trip_without_losing_stacks() {
        let mut world = hecs::World::new();
        // 37 DISTINCT unstackable items: one more than the base 36 slots.
        let mut inv = Inventory::new(36);
        for i in 0..36 {
            inv.add_item(&format!("junk_{i}"), 1, 1);
        }
        inv.ensure_slots(37);
        inv.add_item("iron_ore_0", 1, 1); // the rescued haul
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("Hauler".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));

        let save = extract_world_save(&world);
        assert_eq!(save.inventory.len(), 37, "the grown backpack saved all 37 stacks");

        // Fresh world with the BASE 36-slot inventory, like a restart.
        let mut fresh = hecs::World::new();
        let player = fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &save);
        let inv = fresh.get::<&Inventory>(player).unwrap();
        let stacks = inv.slots.iter().filter(|s| s.is_some()).count();
        assert_eq!(stacks, 37, "all 37 stacks survived the restart");
        assert_eq!(inv.count_item("iron_ore_0"), 1, "the rescued haul survived");
    }

    /// A deployed vehicle survives the full extract -> apply round trip (economy
    /// Phase 2 Stage 1): the truck the player deployed is still parked where they
    /// left it after a restart, and a second apply doesn't stack a duplicate.
    #[test]
    fn deployed_vehicles_survive_the_save_round_trip() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        world.spawn((
            Vehicle { item_id: "truck_pickup_0".to_string() },
            Transform {
                position: glam::Vec3::new(12.0, 0.0, -7.5),
                rotation: glam::Quat::from_rotation_y(1.25),
                scale: glam::Vec3::ONE,
            },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
        ));

        let save = extract_world_save(&world);
        assert_eq!(save.deployed_vehicles.len(), 1);
        assert_eq!(save.deployed_vehicles[0].item_id, "truck_pickup_0");
        assert!((save.deployed_vehicles[0].yaw - 1.25).abs() < 1e-4);

        // Serde round trip (what actually hits disk), then apply to a FRESH world.
        let json = serde_json::to_string(&save).expect("serialize");
        let loaded: WorldSave = serde_json::from_str(&json).expect("deserialize");
        let mut fresh = hecs::World::new();
        fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &loaded);
        let vehicles: Vec<(String, glam::Vec3)> = fresh
            .query_mut::<(&Vehicle, &Transform)>()
            .into_iter()
            .map(|(_e, (v, t))| (v.item_id.clone(), t.position))
            .collect();
        assert_eq!(vehicles.len(), 1, "the parked truck came back");
        assert_eq!(vehicles[0].0, "truck_pickup_0");
        assert!((vehicles[0].1 - glam::Vec3::new(12.0, 0.0, -7.5)).length() < 1e-4);

        // Applying the same save again must NOT duplicate the vehicle.
        apply_save_to_world(&mut fresh, &loaded);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 1, "idempotent re-apply");

        // The save is authoritative (v0.678 review fix): applying a DIFFERENT
        // save clears vehicles that aren't in it — the launcher's character
        // switch must not leak one character's trucks into another's world.
        let empty = WorldSave::new_offline("Other", "fibonacci");
        apply_save_to_world(&mut fresh, &empty);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 0, "vehicles absent from the applied save are despawned");
    }

    /// Two vehicles deployed at the IDENTICAL pose (deploy twice without moving)
    /// must both come back after a restart. The v0.677 same-pose skip guard
    /// collapsed them to one — two kits paid, one truck restored (review fix).
    #[test]
    fn two_identically_parked_vehicles_both_survive_reload() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        let pose = Transform {
            position: glam::Vec3::new(3.0, 0.0, 9.0),
            rotation: glam::Quat::from_rotation_y(0.4),
            scale: glam::Vec3::ONE,
        };
        for _ in 0..2 {
            world.spawn((
                Vehicle { item_id: "rover_0".to_string() },
                pose.clone(),
                Velocity::default(),
                VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
            ));
        }

        let save = extract_world_save(&world);
        assert_eq!(save.deployed_vehicles.len(), 2);

        let mut fresh = hecs::World::new();
        fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &save);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 2, "both identically-parked rovers restore");

        // And an old save without the field loads with none (serde default).
        let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
            "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
            "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
            "weather_state":"clear"}"#;
        let old: WorldSave = serde_json::from_str(old_json).expect("old save loads");
        assert!(old.deployed_vehicles.is_empty());
    }

    /// Regression lock (v0.678, found by the pre-commit review): re-applying a
    /// STALE disk save after a deploy must rewind the WHOLE world consistently —
    /// the kit comes back to the inventory AND the truck despawns. The pre-fix
    /// additive vehicle loop let both exist at once (save-scum duplication).
    #[test]
    fn stale_reapply_rewinds_instead_of_duplicating() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("truck_pickup_kit_0", 1, 1);
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        // T0: periodic save fires while the kit is still in the backpack.
        let stale_disk_save = extract_world_save(&world);
        assert_eq!(stale_disk_save.deployed_vehicles.len(), 0);
        assert!(stale_disk_save.inventory.iter().any(|(id, _)| id == "truck_pickup_kit_0"));

        // T1: player deploys the kit (what handle_deploy does: consume + spawn).
        for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &Controllable)>() {
            inv.remove_item("truck_pickup_kit_0", 1);
        }
        world.spawn((
            Vehicle { item_id: "truck_pickup_0".to_string() },
            Transform { position: glam::Vec3::new(3.0, 0.0, -6.0), rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
        ));

        // T2: player clicks their character in the launcher (lib.rs
        // launcher_pending_load) -> apply_save_to_world with the STALE disk
        // save. Pre-v0.678 this DUPLICATED value: the inventory rebuild
        // resurrected the kit while the additive vehicle loop left the truck
        // standing — one kit became kit + truck via save-scumming. The save is
        // now authoritative for vehicles exactly as it is for inventory, so the
        // whole world rewinds consistently: kit back, truck gone.
        apply_save_to_world(&mut world, &stale_disk_save);

        let kit_count: u32 = world
            .query_mut::<(&Inventory, &Controllable)>()
            .into_iter()
            .map(|(_e, (inv, _c))| inv.count_item("truck_pickup_kit_0"))
            .sum();
        let truck_count = world.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(kit_count, 1, "kit restored from the stale save");
        assert_eq!(
            truck_count, 0,
            "the truck is NOT in the stale save — a consistent rewind removes it; \
             kit + truck coexisting was the save-scum duplication"
        );
    }

    /// Organize-layer container contents survive a save serde round-trip, and a
    /// A carried tool keeps its wear and grade across a save (2026-09-26):
    /// the save used to keep only ids and counts, so every restart renewed
    /// every tool and erased its grade.
    #[test]
    fn carried_tools_keep_their_wear_and_grade_across_a_save() {
        use crate::systems::inventory::Inventory;
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item_q("hammer_0", 1, 1, 6);
        inv.add_item_q("hammer_0", 1, 1, 1);
        inv.slots[0].as_mut().unwrap().wear = 150;
        world.spawn((inv, Controllable, crate::ecs::components::Name("Tester".into()), PlayerSkills::new(), crate::ecs::components::Appearance::default(), crate::ecs::components::Outfit::default()));
        let save = extract_world_save(&world);
        let back: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        let mut fresh = hecs::World::new();
        fresh.spawn((Inventory::new(8), Controllable, crate::ecs::components::Name("Tester".into()), PlayerSkills::new(), crate::ecs::components::Appearance::default(), crate::ecs::components::Outfit::default()));
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(u32, u8)> = fresh
            .query::<(&Inventory, &Controllable)>()
            .iter()
            .flat_map(|(_, (i, _))| i.slots.iter().flatten().map(|s| (s.wear, s.quality)).collect::<Vec<_>>())
            .collect();
        assert_eq!(got, vec![(150, 6), (0, 1)], "each hammer as it was");
    }

    /// The garden's soil survives a save (2026-09-26, N-P-K): each crop's
    /// store and what emptied units remember come back after a restart.
    #[test]
    fn crop_soil_and_soil_memory_survive_a_save() {
        use crate::ecs::components::{CropInstance, CropSoil, Npk, SoilMemory};
        let mut world = hecs::World::new();
        let crop = CropInstance {
            crop_def_id: "tomato".into(),
            growth_stage: "seedling".into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("bed_1".into()),
            tower_slot: Some(2),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        world.spawn((crop, CropSoil { store: Npk::new(1.0, 2.0, 3.0), uptake: 0.4 }));
        let mut memory = SoilMemory::default();
        memory.units.entry("bed_1".into()).or_default().insert(5, Npk::new(4.0, 5.0, 6.0));
        world.spawn((memory,));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let soils: Vec<CropSoil> = fresh.query::<(&CropInstance, &CropSoil)>().iter().map(|(_, (_, s))| s.clone()).collect();
        assert_eq!(soils.len(), 1);
        assert_eq!(soils[0].store, Npk::new(1.0, 2.0, 3.0));
        assert!((soils[0].uptake - 0.4).abs() < 1e-6);
        let mems: Vec<SoilMemory> = fresh.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert_eq!(mems.len(), 1, "one soil memory per world");
        assert_eq!(mems[0].units["bed_1"][&5], Npk::new(4.0, 5.0, 6.0));
    }

    /// Each grow room's air survives a save (2026-09-26, farming::humidity):
    /// its vapour, its fan's speed, what its crops were breathing out,
    /// whether the player was told it is humid, and its humidifier's output,
    /// litres and dry flag come back as they were. A save from before the
    /// field (no `rooms` in the soil memory) still loads, with no rooms, so
    /// each starts from the home's air, and one from before the humidifier
    /// (a room with no humidifier fields) loads with it idle. Seen red by
    /// marking `SoilMemory::rooms` `#[serde(skip)]` (the room came back
    /// empty), and by marking `RoomAir::humidifier` `#[serde(skip)]` (its
    /// output came back 0).
    #[test]
    fn grow_room_air_survives_a_save_and_old_saves_load() {
        use crate::ecs::components::{RoomAir, SoilMemory};
        let mut world = hecs::World::new();
        let mut memory = SoilMemory::default();
        let air = RoomAir {
            vapour_g_m3: 16.25,
            fan_speed: 0.4,
            breathed_l_day: 540.0,
            told: true,
            humidifier: 0.88,
            humidifier_l_day: 27.5,
            humidifier_dry: true,
            // Ship life support (2026-09-26): its carbon dioxide and its air
            // handlers come back too.
            co2_g_m3: 1.9,
            air_handler: 0.6,
            condensate_l_day: 360.0,
            ..Default::default()
        };
        memory.rooms.insert("room-greenhouse".into(), air);
        world.spawn((memory,));
        let text = serde_json::to_string(&extract_world_save(&world)).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let mems: Vec<SoilMemory> = fresh.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert_eq!(mems[0].rooms.get("room-greenhouse"), Some(&air), "the room's air came back");
        // A save from before the humidifier: the room has none of its fields.
        let mut pre: serde_json::Value = serde_json::from_str(&text).unwrap();
        let r = pre["soil_memory"]["rooms"]["room-greenhouse"].as_object_mut().unwrap();
        for k in ["humidifier", "humidifier_l_day", "humidifier_dry"] {
            assert!(r.remove(k).is_some(), "the save wrote {k}");
        }
        let back: WorldSave = serde_json::from_value(pre).unwrap();
        let mut before = hecs::World::new();
        apply_save_to_world(&mut before, &back);
        let mems: Vec<SoilMemory> = before.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        let want = RoomAir { humidifier: 0.0, humidifier_l_day: 0.0, humidifier_dry: false, ..air };
        assert_eq!(mems[0].rooms.get("room-greenhouse"), Some(&want), "a pre-humidifier room loads idle");
        // An older save: its soil memory has no `rooms` at all.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old["soil_memory"].as_object_mut().unwrap().remove("rooms");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        let mems: Vec<SoilMemory> = older.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert!(mems[0].rooms.is_empty(), "an old save loads with no room air");
    }

    /// A crop's pollination record survives a save (2026-09-26,
    /// farming::pollination): the flowering and pollinated days and what is
    /// left of a hand pollination come back on the same crop, and a crop
    /// that had none still has none. A save from before the field (no
    /// `crop_pollination`) still loads, its crops with no record. Seen red
    /// by not writing `crop_pollination` in `extract_world_save`.
    #[test]
    fn crop_pollination_survives_a_save() {
        use crate::ecs::components::{CropInstance, CropPollination};
        let mut world = hecs::World::new();
        let crop = |plant: &str| CropInstance {
            crop_def_id: plant.into(),
            growth_stage: "flower".into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("ntower_3".into()),
            tower_slot: Some(1),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let rec = CropPollination { flowering_days: 4.5, pollinated_days: 2.25, hand_days_left: 1.5 };
        world.spawn((crop("tomato"), rec.clone()));
        world.spawn((crop("lettuce"),));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(String, Option<CropPollination>)> = fresh
            .query::<(&CropInstance, Option<&CropPollination>)>()
            .iter()
            .map(|(_, (c, p))| (c.crop_def_id.clone(), p.cloned()))
            .collect();
        assert_eq!(got.len(), 2);
        for (plant, p) in got {
            match plant.as_str() {
                "tomato" => assert_eq!(p, Some(rec.clone()), "the tomato's record came back"),
                _ => assert_eq!(p, None, "the lettuce still has none"),
            }
        }
        // An older save: strip the field, and it still loads.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("crop_pollination");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        assert_eq!(older.query::<&CropInstance>().iter().count(), 2);
        assert_eq!(older.query::<&CropPollination>().iter().count(), 0);
    }

    /// A picked crop's picking state survives a save (2026-09-26,
    /// farming::picking): how long it has been ripe, the picks taken, its
    /// season roll and the fraction it carries come back on the same crop,
    /// and a crop that had none still has none. A save from before the field
    /// (no `crop_picking`) still loads, its crops with no state (a ripe picked
    /// crop then starts its window afresh). Seen red by not writing
    /// `crop_picking` in `extract_world_save`.
    #[test]
    fn crop_picking_survives_a_save() {
        use crate::ecs::components::{CropInstance, CropPicking};
        let mut world = hecs::World::new();
        let crop = |plant: &str, stage: &str| CropInstance {
            crop_def_id: plant.into(),
            growth_stage: stage.into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("ntower_3".into()),
            tower_slot: Some(1),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let rec = CropPicking { days_ripe: 12.25, next_pick: 7, taken: 6, roll: Some(0.625), carry: 0.375 };
        world.spawn((crop("tomato", "ripe"), rec.clone()));
        world.spawn((crop("lettuce", "mature"),));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(String, Option<CropPicking>)> = fresh
            .query::<(&CropInstance, Option<&CropPicking>)>()
            .iter()
            .map(|(_, (c, p))| (c.crop_def_id.clone(), p.cloned()))
            .collect();
        assert_eq!(got.len(), 2);
        for (plant, p) in got {
            match plant.as_str() {
                "tomato" => assert_eq!(p, Some(rec.clone()), "the tomato's picking came back"),
                _ => assert_eq!(p, None, "the lettuce still has none"),
            }
        }
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("crop_picking");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        assert_eq!(older.query::<&CropInstance>().iter().count(), 2);
        assert_eq!(older.query::<&CropPicking>().iter().count(), 0);
    }

    /// A pool saved EMPTY comes back empty, and a save that never wrote a
    /// pool keeps the live one (2026-09-27, review of the sleep and offline
    /// batch). Seen red with the old `is_empty` check on a Vec: the planks
    /// stashed after the empty save stayed beside the rewound backpack.
    #[test]
    fn a_pool_saved_empty_comes_back_empty() {
        let plank = crate::systems::inventory::placed::PlacedItem {
            key: "wood_plank_0".into(),
            name: "Wood Plank".into(),
            qty: 5,
            container: "2/0".into(),
            ..Default::default()
        };
        let mut gui = crate::gui::GuiState::default();
        gui.placed_items = vec![plank.clone()];
        let mut save = WorldSave::new_offline("Test", "fibonacci");
        save.placed_items = Some(Vec::new());
        let save: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        after_resume(&mut gui, &save, &Resumed::default());
        assert!(gui.placed_items.is_empty(), "the empty pool came back empty: {:?}", gui.placed_items);

        gui.placed_items = vec![plank];
        let fresh = WorldSave::new_offline("Test", "fibonacci");
        after_resume(&mut gui, &fresh, &Resumed::default());
        assert_eq!(gui.placed_items.len(), 1, "a save with no pool keeps the live one");
    }

    /// A save with no `placed_items` field loads with no pool (serde default),
    /// so the places spine's seed stays.
    #[test]
    fn placed_items_persist_and_old_saves_default_empty() {
        let mut save = WorldSave::new_offline("Test", "fibonacci");
        save.placed_items = Some(vec![
            crate::systems::inventory::placed::PlacedItem {
                key: "ice_axe_0".into(),
                name: "Ice Axe".into(),
                qty: 1,
                container: "1/0/0".into(),
                ..Default::default()
            },
            crate::systems::inventory::placed::PlacedItem {
                key: "iron_ore_0".into(),
                name: "Iron Ore".into(),
                qty: 5,
                container: "2/0".into(),
                ..Default::default()
            },
        ]);
        let json = serde_json::to_string(&save).expect("serialize");
        let back: WorldSave = serde_json::from_str(&json).expect("deserialize");
        let pool = back.placed_items.as_ref().expect("the pool came back");
        assert_eq!(pool.len(), 2);
        assert_eq!(pool[0].key, "ice_axe_0");
        assert_eq!(pool[1].qty, 5);
        assert_eq!(pool[1].container, "2/0");

        // A pre-v0.517 save JSON that lacks the field -> empty pool, no error.
        let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
            "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
            "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
            "weather_state":"clear"}"#;
        let old: WorldSave = serde_json::from_str(old_json).expect("old save loads");
        assert!(old.placed_items.is_none(), "a save without the field has no pool");
    }

    fn crop(planted_at: f64, stage: &str) -> crate::ecs::components::CropInstance {
        crate::ecs::components::CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: stage.to_string(),
            planted_at,
            water_level: 0.8,
            health: 90.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    /// Offline catch-up ages living crops by exactly the time away, leaves the
    /// dead alone, never touches water or health, and does nothing when off.
    #[test]
    fn offline_catch_up_ages_living_crops_only_when_on() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.game_time = 6000.0;
        save.timestamp = 1_000;
        save.crops = vec![crop(5000.0, "seedling"), crop(4000.0, crate::ecs::components::STAGE_DEAD)];
        let now = 1_000 + 3_600;

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, true, 1.0, now);
        assert_eq!(r, Resumed { clock: 6000.0, away_secs: 3600.0, crops_aged: 1, ..Default::default() });
        let mut got: Vec<(f64, f32, f32)> = world
            .query_mut::<&crate::ecs::components::CropInstance>()
            .into_iter()
            .map(|(_e, c)| (c.planted_at, c.water_level, c.health))
            .collect();
        got.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert_eq!(got, vec![(1400.0, 0.8, 90.0), (4000.0, 0.8, 90.0)], "living aged, dead untouched, upkeep untouched");

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, false, 1.0, now);
        assert_eq!(r, Resumed { clock: 6000.0, ..Default::default() });

        // At time speed 72 the hour away is 72 game hours, the rate the world
        // ran at while played (the one clock, 2026-09-27). Red check, run:
        // leaving out the time speed ages the crop one hour, not 72.
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, true, 72.0, now);
        assert_eq!(r.away_secs, 72.0 * 3600.0);
    }

    /// A crop cannot have been planted in the future, so the clock never
    /// resumes behind the newest planting (a save from before the clock was
    /// stored carries game_time 0 next to crops planted at game second 5000).
    #[test]
    fn clock_never_resumes_behind_the_newest_crop() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.crops = vec![crop(5000.0, "seedling"), crop(1200.0, "seedling")];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, false, 1.0, 0).clock, 5000.0);
    }

    /// No stamp means no measurable absence; a clock set backwards means zero
    /// growth, never negative.
    #[test]
    fn unstamped_saves_and_backwards_clocks_grant_nothing() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.crops = vec![crop(100.0, "seedling")];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, true, 1.0, 9_999).away_secs, 0.0, "timestamp 0");

        save.timestamp = 5_000;
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, true, 1.0, 4_000).away_secs, 0.0, "clock went backwards");
    }

    /// Builds survive a restart (2026-09-25: nothing wrote save.constructions,
    /// so every structure was discarded at exit after its materials were spent).
    /// A finished Structure and a half-built scaffold both round-trip with pose
    /// and state, and re-applying replaces rather than duplicates.
    #[test]
    fn builds_survive_the_save_round_trip() {
        use crate::systems::construction::{Construction, Structure};
        let tf = |x: f32| crate::ecs::components::Transform {
            position: glam::Vec3::new(x, 0.0, 3.0),
            rotation: glam::Quat::from_rotation_y(0.5),
            scale: glam::Vec3::new(2.0, 3.0, 0.2),
        };
        let mut world = hecs::World::new();
        world.spawn((
            tf(1.0),
            Structure {
                blueprint_id: "wooden_wall".to_string(),
                health: 80.0,
                max_health: 100.0,
                provides: Some("shelter".to_string()),
                uid: 4,
            },
        ));
        world.spawn((
            tf(5.0),
            Construction {
                blueprint_id: "wooden_door".to_string(),
                progress: 4.0,
                build_time: 10.0,
                builder_key: None,
            },
        ));
        let save = extract_world_save(&world);
        assert_eq!(save.constructions.len(), 2);

        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &save);
        apply_save_to_world(&mut fresh, &save); // re-apply must not duplicate
        let structures: Vec<(String, f32, f32, Option<String>, glam::Vec3)> = fresh
            .query::<(&Structure, &crate::ecs::components::Transform)>()
            .iter()
            .map(|(_e, (s, t))| (s.blueprint_id.clone(), s.health, s.max_health, s.provides.clone(), t.scale))
            .collect();
        assert_eq!(
            structures,
            vec![("wooden_wall".to_string(), 80.0, 100.0, Some("shelter".to_string()), glam::Vec3::new(2.0, 3.0, 0.2))]
        );
        let scaffolds: Vec<(String, f32, f32, f32)> = fresh
            .query::<(&Construction, &crate::ecs::components::Transform)>()
            .iter()
            .map(|(_e, (c, t))| (c.blueprint_id.clone(), c.progress, c.build_time, t.position.x))
            .collect();
        assert_eq!(scaffolds, vec![("wooden_door".to_string(), 4.0, 10.0, 5.0)]);
        // And through JSON, the way it reaches disk.
        let json = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&json).unwrap();
        assert_eq!(back.constructions, save.constructions);
    }

    /// A door left open is open after a save and a load (2026-09-28), a shut
    /// one stays shut, and a record from before doors (no `open`) loads shut.
    /// Red check, run: writing `open: false` for every structure fails the
    /// first assertion.
    #[test]
    fn an_open_door_stays_open_across_a_save() {
        use crate::systems::construction::{DoorOpen, Structure};
        let piece = |uid: u32| Structure {
            blueprint_id: "wood_wall_door".to_string(),
            health: 150.0,
            max_health: 150.0,
            provides: Some("shelter".to_string()),
            uid,
        };
        let tf = |x: f32| crate::ecs::components::Transform {
            position: glam::Vec3::new(x, 0.0, 0.0),
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::new(4.0, 3.0, 0.2),
        };
        let mut world = hecs::World::new();
        let open = world.spawn((tf(0.0), piece(1)));
        world.insert_one(open, DoorOpen).unwrap();
        world.spawn((tf(8.0), piece(2)));
        let json = serde_json::to_string(&extract_world_save(&world)).unwrap();
        let back: WorldSave = serde_json::from_str(&json).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let mut states: Vec<(u32, bool)> =
            fresh.query::<(&Structure, Option<&DoorOpen>)>().iter().map(|(_e, (s, o))| (s.uid, o.is_some())).collect();
        states.sort();
        assert_eq!(states, vec![(1, true), (2, false)]);
        let old: crate::persistence::ConstructionSave = serde_json::from_str(
            r#"{"blueprint_id":"wood_wall_door","position":[0,0,0],"rotation":[0,0,0,1],"health":150}"#,
        )
        .unwrap();
        assert!(!old.open, "a record from before doors loads shut");
    }

    /// A BUILT FIRE KEEPS ITS FUEL ACROSS A SAVE AND BURNS DOWN WHILE AWAY
    /// (BUG-153, 2026-10-05). A campfire with 50 minutes of burning left comes
    /// back from the save (through JSON) with 50 minutes, a wall beside it
    /// comes back with no fuel at all, and 20 minutes away leaves 30; a day
    /// away leaves it out. Red check, run: writing `fire_s: None` for every
    /// structure brings the campfire back with no fuel and the first
    /// assertion fails.
    #[test]
    fn a_fire_keeps_its_fuel_across_a_save_and_burns_down_while_away() {
        use crate::systems::construction::{fires::FireFuel, Structure};
        let piece = |id: &str, uid: u32| Structure { blueprint_id: id.to_string(), health: 30.0, max_health: 30.0, provides: None, uid };
        let tf = |x: f32| crate::ecs::components::Transform { position: glam::Vec3::new(x, 0.0, 0.0), ..Default::default() };
        let mut world = hecs::World::new();
        world.spawn((tf(0.0), piece("campfire", 1), FireFuel { seconds_left: 3000.0 }));
        world.spawn((tf(8.0), piece("wood_wall", 2)));
        let json = serde_json::to_string(&extract_world_save(&world)).unwrap();
        let mut back: WorldSave = serde_json::from_str(&json).unwrap();
        back.timestamp = 1_000;
        let fuel = |w: &hecs::World| {
            let mut f: Vec<(u32, Option<f32>)> =
                w.query::<(&Structure, Option<&FireFuel>)>().iter().map(|(_e, (s, f))| (s.uid, f.map(|f| f.seconds_left))).collect();
            f.sort_by_key(|(uid, _)| *uid);
            f
        };
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        assert_eq!(fuel(&fresh), vec![(1, Some(3000.0)), (2, None)], "the fire's fuel comes back, the wall has none");
        catch_up_world(&mut fresh, &back, true, 1.0, 1_000 + 1_200);
        assert_eq!(fuel(&fresh), vec![(1, Some(1800.0)), (2, None)], "20 minutes away burned 20 minutes");
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        catch_up_world(&mut fresh, &back, true, 1.0, 1_000 + 86_400);
        assert_eq!(fuel(&fresh), vec![(1, Some(0.0)), (2, None)], "a day away: out");
    }

    /// A wall the player turned is built turned, saved turned, and comes back
    /// turned (2026-09-27). The whole path: a build request with a quarter
    /// turn through the ConstructionSystem (materials, the timed build,
    /// completion), the save as written on exit, JSON, and the restore. Red
    /// check, run: the ConstructionSystem spawning with `Quat::IDENTITY`, as
    /// it did before placement could turn anything, fails the first
    /// assertion.
    #[test]
    fn a_turned_wall_is_saved_turned_and_comes_back_turned() {
        use crate::ecs::components::{Controllable, Transform};
        use crate::systems::construction::{placement, BlueprintRegistry, BuildRequest, ConstructionSystem, Structure};
        use crate::systems::inventory::Inventory;
        use crate::ecs::systems::System;
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let wall = reg.get("wood_wall").unwrap().clone();
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("blueprint_registry", reg);
        let ghost = placement::placement_pose(&wall, glam::Vec3::new(2.0, 0.0, 0.0), 1, &hecs::World::new(), data.get::<BlueprintRegistry>("blueprint_registry").unwrap(), None);
        let request = BuildRequest::new("wood_wall", ghost);
        data.insert("build_request", std::sync::Mutex::new(vec![request]));
        data.insert("build_status", std::sync::Mutex::new(String::new()));
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        for (id, qty) in &wall.materials {
            inv.add_item(id, *qty, 99);
        }
        world.spawn((inv, Controllable));
        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);
        sys.tick(&mut world, wall.build_time + 1.0, &data);
        let turned = placement::quarter_turn(1);
        let built: Vec<glam::Quat> = world.query::<(&Structure, &Transform)>().iter().map(|(_, (_, t))| t.rotation).collect();
        assert_eq!(built.len(), 1);
        // Same rotation: |q1 . q2| is 1 (acos-based angle_between is too
        // coarse in f32 to tell an exact match from a 0.02 degree miss).
        let same = |q: glam::Quat| q.dot(turned).abs() > 1.0 - 1e-6;
        assert!(same(built[0]), "built turned: {:?}", built[0]);

        let save = extract_world_save(&world);
        let back: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let restored: Vec<Transform> = fresh.query::<(&Structure, &Transform)>().iter().map(|(_, (_, t))| t.clone()).collect();
        assert_eq!(restored.len(), 1);
        assert!(same(restored[0].rotation), "restored turned: {:?}", restored[0].rotation);
        let (lo, hi) = placement::world_aabb(&restored[0]);
        assert!((hi.z - lo.z - 4.0).abs() < 1e-3, "still runs north-south: {lo} {hi}");
    }

    /// A piece built on a planet comes back on the planet, in the same build
    /// site, to the same place (2026-09-27, BUG-102): the site's body and its
    /// f64 origin go through the save as written on exit, JSON, and the
    /// restore, beside the site-local pose, for a finished piece and a
    /// scaffold alike; a home piece in the same save stays in the home frame.
    /// Red check, run: leaving `site` out of the extracted save (the
    /// `site.cloned()` lines as None) restores the wall with no site, drawn
    /// in the home frame at the station, and the first assertion fails.
    #[test]
    fn a_planet_pieces_site_round_trips_a_save() {
        use crate::ecs::components::Transform;
        use crate::systems::construction::{Construction, PlanetSite, Structure};
        // A site in Silverdale, WA: an origin 6,366 km from Earth's centre
        // with metre-scale detail the f64 must keep exactly.
        let site = PlanetSite {
            body: "earth".into(),
            origin: glam::DVec3::new(-2_297_531.123_456, 4_718_021.654_321, 3_607_714.987_654),
        };
        let pose = Transform {
            position: glam::Vec3::new(2.0, 0.37, -2.0),
            rotation: glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            scale: glam::Vec3::new(4.0, 3.0, 0.2),
        };
        let mut world = hecs::World::new();
        world.spawn((
            pose.clone(),
            Structure { blueprint_id: "wood_wall".into(), health: 150.0, max_health: 150.0, provides: Some("shelter".into()), uid: 3 },
            site.clone(),
        ));
        world.spawn((
            pose.clone(),
            Construction { blueprint_id: "roof".into(), progress: 2.0, build_time: 6.0, builder_key: None },
            site.clone(),
        ));
        world.spawn((
            Transform::default(),
            Structure { blueprint_id: "bed".into(), health: 60.0, max_health: 60.0, provides: Some("rest".into()), uid: 4 },
        ));
        let save = extract_world_save(&world);
        let back: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let walls: Vec<(Transform, PlanetSite)> = fresh
            .query::<(&Structure, &Transform, &PlanetSite)>()
            .iter()
            .map(|(_e, (_, t, s))| (t.clone(), s.clone()))
            .collect();
        assert_eq!(walls.len(), 1, "the wall comes back in its site");
        assert_eq!(walls[0].1.body, "earth");
        assert!((walls[0].1.origin - site.origin).length() < 1e-6, "origin to a micrometre: {}", walls[0].1.origin);
        assert_eq!(walls[0].0.position, pose.position);
        assert_eq!(walls[0].0.rotation, pose.rotation);
        assert_eq!(fresh.query::<(&Construction, &PlanetSite)>().iter().count(), 1, "the scaffold too");
        let home: Vec<String> = fresh
            .query::<hecs::Without<&Structure, &PlanetSite>>()
            .iter()
            .map(|(_e, s)| s.blueprint_id.clone())
            .collect();
        assert_eq!(home, vec!["bed".to_string()], "a home piece stays in the home frame");
    }

    /// A built chest and what is in it survive a restart (2026-09-27): the
    /// chest comes back with the SAME uid, so its places-tree node comes back
    /// at the same path, and the item filed there is in it again. Red check:
    /// with `apply_save_to_world` restoring uid 0 instead of the saved one,
    /// the restored chest is no store until the ConstructionSystem numbers
    /// it afresh, and the first assertion after the restart fails.
    #[test]
    fn a_built_chest_keeps_its_contents_through_a_restart() {
        use crate::systems::construction::{uses, BlueprintRegistry, Structure};
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/blueprints/basic.ron"
        )))
        .unwrap();
        let chest = reg.get("storage_chest").unwrap();
        let mut world = hecs::World::new();
        world.spawn((
            crate::ecs::components::Transform {
                position: glam::Vec3::new(2.0, 0.0, -4.0),
                rotation: glam::Quat::IDENTITY,
                scale: glam::Vec3::from_array(chest.size),
            },
            Structure {
                blueprint_id: chest.id.clone(),
                health: chest.health,
                max_health: chest.health,
                provides: chest.provides.clone(),
                uid: 3,
            },
        ));
        // The player stashes planks in it (the Inventory page files them
        // under the chest's path), then the home is saved as on exit.
        let path = uses::built_stores(&world, Some(&reg))[0].0.clone();
        let mut save = extract_world_save(&world);
        save.placed_items = Some(vec![crate::systems::inventory::placed::PlacedItem {
            key: "wood_plank_0".into(),
            name: "Wood Plank".into(),
            qty: 5,
            container: path.clone(),
            ..Default::default()
        }]);
        let back: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();

        // Next launch: the world comes back, the tree is rebuilt from it.
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let stores = uses::built_stores(&fresh, Some(&reg));
        assert_eq!(stores, vec![(path.clone(), "Storage Chest".to_string())]);
        let mut places = crate::gui::load_places(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data")));
        crate::gui::sync_built_stores(&mut places, &stores);
        assert!(crate::gui::collect_containers(&places).iter().any(|(p, l)| *p == path && l == "Storage Chest"));
        let inside: Vec<_> = back.placed_items.iter().flatten().filter(|p| p.container == path).collect();
        assert_eq!(inside.len(), 1);
        assert_eq!((inside[0].key.as_str(), inside[0].qty), ("wood_plank_0", 5));
    }

    /// A scaffold's materials were spent when it started, so time away may
    /// finish it; progress is capped at build_time so the ConstructionSystem's
    /// own tick does the completion (quest event, XP) rather than this pass.
    #[test]
    fn offline_catch_up_finishes_scaffolds_without_skipping_completion() {
        use crate::systems::construction::Construction;
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.timestamp = 1_000;
        save.constructions = vec![crate::persistence::ConstructionSave {
            blueprint_id: "wooden_door".to_string(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            health: 0.0,
            max_health: 0.0,
            provides: None,
            building: Some((4.0, 10.0)),
            uid: 0,
            open: false,
            site: None,
            outside_home: false,
            fire_s: None,
        }];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, true, 1.0, 1_000 + 3_600);
        assert_eq!(r.builds_advanced, 1);
        let (_e, c) = world.query_mut::<&Construction>().into_iter().next().unwrap();
        assert_eq!(c.progress, 10.0, "capped at build_time, still a Construction for the tick to complete");

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, false, 1.0, 1_000 + 3_600).builds_advanced, 0);
        let (_e, c) = world.query_mut::<&Construction>().into_iter().next().unwrap();
        assert_eq!(c.progress, 4.0, "toggle off leaves the scaffold where it was");
    }

    /// Craft batches resume counted down by the time away, floored at zero
    /// (which completes them on the next tick through the normal delivery).
    #[test]
    fn restored_crafts_count_down_by_the_time_away() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        let batch = |t: f32| crate::systems::crafting::CraftSave {
            recipe_id: "smelt_iron".to_string(),
            time_remaining: t,
            auto: true,
            machine_id: Some("smelter_1".to_string()),
            pad: None,
        };
        save.crafts = vec![batch(7.0), batch(9_000.0)];
        let r: Vec<f32> = restored_crafts(&save, 3_600.0).iter().map(|c| c.time_remaining).collect();
        assert_eq!(r, vec![0.0, 5_400.0]);
        let r: Vec<f32> = restored_crafts(&save, 0.0).iter().map(|c| c.time_remaining).collect();
        assert_eq!(r, vec![7.0, 9_000.0], "no time away, no change");
    }

    /// A restored active home holds saves off until it is loaded into the
    /// world, and loading it (either way lib.rs loads a save: with its
    /// progress, or the character only) releases the hold. Without the
    /// release the game would never save again that session; without the
    /// hold the next periodic save would write the pre-restore world back
    /// over the restore.
    ///
    /// Red check (2026-10-03): with the `release_restore_hold();` line
    /// removed from `apply_identity`, this failed with `loading the
    /// character only releases the hold`. Restored byte for byte, it passes.
    #[test]
    fn a_restored_home_holds_saves_until_it_is_loaded() {
        // Thread-local, so this test's hold is its own (see RESTORED_SAVE_WAITING).
        assert!(!restored_save_waiting());
        let save = WorldSave::new_offline("Restored", "fibonacci");
        let mut world = hecs::World::new();

        hold_saves_for_restore();
        assert!(restored_save_waiting(), "a restore holds saves");
        apply_identity(&mut world, &save);
        assert!(!restored_save_waiting(), "loading the character only releases the hold");

        hold_saves_for_restore();
        apply_save_to_world(&mut world, &save);
        assert!(!restored_save_waiting(), "loading the whole save releases the hold");

        // A save of another kind is ignored by the apply, but must still
        // release: holding for a load that can never happen would stop
        // saving for the rest of the session.
        let mut server = WorldSave::new_offline("Elsewhere", "fibonacci");
        server.kind = "server".to_string();
        hold_saves_for_restore();
        apply_save_to_world(&mut world, &server);
        assert!(!restored_save_waiting(), "an ignored save still releases the hold");
    }

    /// ROUND 4 of the 1b review, finding 4: a save written before the world loads (on quit
    /// from the main menu, or the periodic save, neither of which waits for the world) records
    /// the box of the save applied at startup, which its pieces still stand in (`frame_for_save`).
    /// It recorded none, so the next launch left every piece where it stood, outside the home.
    /// Once the world has loaded, the frame kept with the live ship is the one recorded.
    ///
    /// Seen red 2026-10-03 with `frame_for_save` reading only the published frame (the
    /// db551f530 save): "a save written before the world loaded says its home stood nowhere:
    /// None".
    #[test]
    fn a_save_before_the_world_loads_keeps_the_box_its_pieces_stand_in() {
        let root = std::env::temp_dir().join(format!("hos_save_before_world_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("saves").join("offline_home.json");
        let mut world = hecs::World::new();
        world.spawn((
            crate::ecs::components::Transform { position: glam::Vec3::new(20.0, 0.0, 129.0), rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
            crate::systems::construction::Structure { blueprint_id: "chest".into(), health: 1.0, max_health: 1.0, provides: None, uid: 9 },
        ));
        let mut data = crate::hot_reload::data_store::DataStore::new();
        let p2 = [[0.0, 0.0, 99.0], [55.0, 12.0, 188.0]];
        data.insert(LOADED_HOME_BOX_KEY, LoadedHomeBox(p2));
        save_home_at(&path, &world, &[], &data, true, None);
        let saved = persistence::load_world(&path).unwrap().home_plot_box;
        assert_eq!(saved, Some(p2), "a save written before the world loaded says its home stood nowhere: {saved:?}");
        // The world loaded: the frame kept with the live ship wins.
        let p1 = (glam::Vec3::ZERO, glam::Vec3::new(55.0, 12.0, 89.0));
        data.insert(HOME_FRAME_KEY, HomeFrame { home: p1 });
        save_home_at(&path, &world, &[], &data, true, None);
        assert_eq!(persistence::load_world(&path).unwrap().home_plot_box, Some([p1.0.to_array(), p1.1.to_array()]));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The hold itself: while a restored home waits to be loaded, neither
    /// kind of save (the periodic or quit save with progress, and the
    /// character-only save) writes anything, not even the snapshot a save
    /// keeps first. Once the load releases the hold, the same save writes,
    /// which proves the held saves above could have written and did not.
    ///
    /// Before this test the hold had no test that could fail: the reviewer
    /// removed the whole `if restored_save_waiting() { ...; return; }` guard
    /// and every test in this lane still passed (review finding, 2026-10-03).
    ///
    /// Red check (2026-10-03): with that guard removed from `save_home_at`,
    /// this failed with `a held save writes nothing over the restored home`.
    /// Restored byte for byte, it passes.
    #[test]
    fn a_held_save_writes_nothing() {
        // Thread-local, so this test's hold is its own (see RESTORED_SAVE_WAITING).
        assert!(!restored_save_waiting());
        let root = std::env::temp_dir().join(format!("hos_held_save_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("saves").join("offline_home.json");
        let slot = persistence::snapshots_root_for(&path).join("offline_home");
        // The restored home, as the restore left it on disk.
        let mut restored = WorldSave::new_offline("Restored home", "fibonacci");
        restored.progress_saved = true;
        persistence::save_world(&path, &restored).unwrap();
        let restored_bytes = std::fs::read(&path).unwrap();
        // The live world still holds what the restore replaced: here a world
        // with nothing in it, whose save would be a different home entirely.
        let mut world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();

        hold_saves_for_restore();
        save_home_at(&path, &world, &[], &data, true, None);
        save_home_at(&path, &world, &[], &data, false, None);
        assert!(
            std::fs::read(&path).unwrap() == restored_bytes,
            "a held save writes nothing over the restored home"
        );
        assert!(persistence::list_snapshots(&slot).is_empty(), "a held save keeps no snapshot either");
        assert!(restored_save_waiting(), "a save does not release the hold, only a load does");

        // The load releases the hold, and the same save now writes: the
        // restored home is kept as a snapshot first, then saved over.
        apply_save_to_world(&mut world, &restored);
        save_home_at(&path, &world, &[], &data, true, None);
        assert_eq!(persistence::load_world(&path).unwrap().name, "My Homestead", "after the load, the save writes");
        let kept = persistence::list_snapshots(&slot);
        assert_eq!(kept.len(), 1);
        assert!(std::fs::read(&kept[0].path).unwrap() == restored_bytes, "the restored home was kept first");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// "Snapshot now" says what it did: nothing to keep before the first
    /// save, a new copy for a changed save, and "already kept" for a save
    /// unchanged since the last copy (where it used to keep a duplicate and
    /// push the oldest copy out).
    ///
    /// Red check (2026-10-03): with the "Unchanged since the last copy" loop
    /// removed from `persistence::snapshot_save_protecting`, this failed with
    /// `a second click on an unchanged home says it is already kept`, `left:
    /// Kept(...), right: AlreadyKept`. Restored byte for byte, it passes.
    #[test]
    fn snapshot_now_says_when_the_home_is_already_kept() {
        let root = std::env::temp_dir().join(format!("hos_snapshot_now_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("saves").join("offline_home.json");
        let t = 1_759_400_000_000u64;
        assert_eq!(snapshot_home_now_at(&path, t).unwrap(), SnapshotNow::NotSavedYet);

        persistence::save_world(&path, &WorldSave::new_offline("Home", "fibonacci")).unwrap();
        assert!(matches!(snapshot_home_now_at(&path, t + 1_000).unwrap(), SnapshotNow::Kept(_)));
        assert_eq!(
            snapshot_home_now_at(&path, t + 2_000).unwrap(),
            SnapshotNow::AlreadyKept,
            "a second click on an unchanged home says it is already kept"
        );
        // The home changes (written without the snapshot a save would keep):
        // the next click keeps it.
        let mut changed = WorldSave::new_offline("Home", "fibonacci");
        changed.character_name = "Changed".to_string();
        persistence::write_atomic(&path, serde_json::to_string_pretty(&changed).unwrap().as_bytes()).unwrap();
        assert!(matches!(snapshot_home_now_at(&path, t + 3_000).unwrap(), SnapshotNow::Kept(_)));
        let slot = persistence::snapshots_root_for(&path).join("offline_home");
        assert_eq!(persistence::list_snapshots(&slot).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// "Start every session from the default home" (2026-09-25): writing
    /// keeps an existing progress save untouched apart from the character,
    /// and with no save yet writes a character-only save that is marked as
    /// carrying no progress.
    #[test]
    fn identity_only_save_leaves_progress_alone() {
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(8),
            PlayerSkills::new(),
            crate::ecs::components::Name("Astra".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        let mut existing = WorldSave::new_offline("t", "fibonacci");
        existing.character_name = "Old Name".into();
        existing.inventory = vec![("wood_plank_0".into(), 40)];
        existing.crops = vec![crop(5.0, "seedling")];
        existing.game_time = 999.0;
        let out = identity_only_save(Some(existing), &world);
        assert_eq!(out.character_name, "Astra", "the character is recorded");
        assert_eq!(out.inventory, vec![("wood_plank_0".to_string(), 40)], "progress untouched");
        assert_eq!(out.crops.len(), 1);
        assert_eq!(out.game_time, 999.0);
        assert!(out.progress_saved);

        let fresh = identity_only_save(None, &world);
        assert_eq!(fresh.character_name, "Astra");
        assert!(!fresh.progress_saved, "a character-only save says so");
        assert!(fresh.inventory.is_empty() && fresh.crops.is_empty());
    }

    /// The shipped starting kit parses and every item in it is a real item,
    /// so a new player never starts with a name that resolves to nothing.
    #[test]
    fn the_starting_kit_is_real_items() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let kit = starting_kit(&dir);
        assert!(kit.iter().any(|(id, _)| id.starts_with("seed_")), "a new player can plant: {kit:?}");
        let items = crate::systems::inventory::ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .unwrap();
        for (id, qty) in &kit {
            assert!(items.items.contains_key(id), "starting item {id} is not in items.csv");
            assert!(*qty > 0);
        }
    }

    fn rock(iron: f32) -> crate::ecs::components::AsteroidBody {
        crate::ecs::components::AsteroidBody {
            id: "rock".into(),
            name: "Rock".into(),
            classification: "M".into(),
            ores: vec![("iron_ore_0".into(), iron)],
            position: [0.0, 0.0, 0.0],
        }
    }

    /// The asteroids as mined down, the drone in flight with its cargo, the
    /// herd's timers and the standing order all survive a save (2026-09-27:
    /// every one of them was rebuilt fresh at each launch, and the ore in a
    /// flying drone's hold was lost). Re-applying replaces, and the drone's
    /// home becomes the new player entity. Seen red with the saved asteroids
    /// not spawned by `apply_save_to_world` (none left at all).
    #[test]
    fn the_asteroids_the_drone_and_the_herd_survive_a_save() {
        use crate::ecs::components::{AsteroidBody, Drone, DronePhase};
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((rock(12.0),));
        world.spawn((Drone {
            home: 1,
            target: "rock".into(),
            manifest: vec![("iron_ore_0".into(), 4)],
            phase: DronePhase::Returning,
            phase_time: 1.0,
            cargo: vec![("iron_ore_0".into(), 4)],
            home_pos: [0.0; 3],
            target_pos: [0.0; 3],
        },));
        world.spawn((
            crate::systems::livestock::HerdSlot("chicken#0".into()),
            crate::ecs::components::Harvestable { resource: "egg_0".into(), amount: 1.0, regrow_time: 300.0, time_since_harvest: 40.0 },
        ));
        let mut save = extract_world_save(&world);
        assert_eq!(save.herd, vec![("chicken#0".to_string(), 40.0)]);
        // Through JSON, as on disk (2026-09-27 review: this test once
        // extracted and applied in memory only), with the standing order,
        // which the home save writes beside the world.
        save.mining_order = Some(("rock".into(), vec![("iron_ore_0".into(), 2)]));
        let save: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        assert_eq!(save.mining_order, Some(("rock".to_string(), vec![("iron_ore_0".to_string(), 2)])));
        assert_eq!(save.herd, vec![("chicken#0".to_string(), 40.0)]);

        // The next launch: a fresh player and the fresh, full asteroid.
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((rock(120.0),));
        apply_save_to_world(&mut world, &save);
        apply_save_to_world(&mut world, &save);
        let ores: Vec<f32> = world.query::<&AsteroidBody>().iter().map(|(_, a)| a.ores[0].1).collect();
        assert_eq!(ores, vec![12.0], "mined down, and one of it");
        let drones: Vec<Drone> = world.query::<&Drone>().iter().map(|(_, d)| d.clone()).collect();
        assert_eq!(drones.len(), 1);
        assert_eq!(drones[0].cargo, vec![("iron_ore_0".to_string(), 4)], "the ore in its hold");
        assert_eq!(drones[0].home, player.to_bits().get(), "home is this player now");
    }

    /// A return runs the home through the time away, end to end: the drone's
    /// standing order mines the asteroid out (three hauls of 2), the smelter
    /// smelts each haul from the moment it landed (three ingots, one coal
    /// each), and the hen's timer waits for the herd with the hour added.
    /// Since BUG-150 the hauls land in home storage and the coal is taken from
    /// there; the backpack is not touched.
    /// Seen red with `mining::advance_away` returning at once (no hauls, so no
    /// ore and no ingots).
    #[test]
    fn a_return_runs_the_drone_the_machines_and_the_herd_through_the_time_away() {
        use crate::ecs::components::{AutoRefine, MachineInstanceId};
        use crate::hot_reload::data_store::DataStore;
        use std::sync::Mutex;
        let mut data = DataStore::new();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let read = |f: &str| std::fs::read(root.join(f)).unwrap();
        data.insert("recipe_registry", crate::systems::crafting::RecipeRegistry::from_csv(&read("recipes.csv")).unwrap());
        data.insert("item_registry", crate::systems::inventory::ItemRegistry::from_csv(&read("items.csv")).unwrap());
        data.insert("creature_registry", crate::systems::livestock::CreatureRegistry::from_csv(&read("creatures.csv")).unwrap());
        data.insert("auto_mine_order", Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
        data.insert("player_notices", Mutex::new(Vec::<String>::new()));
        // The smelter's coal waits in home storage, the only store the home's
        // machines draw on (BUG-150); the drone files each haul there too.
        data.insert("home_stock", Mutex::new(std::collections::HashMap::from([("coal_0".to_string(), 3u32)])));
        data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
        crate::systems::crafting::register(&mut data);
        crate::systems::livestock::register(&mut data);

        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.timestamp = now_secs() - 3600;
        save.asteroids = Some(vec![rock(6.0)]);
        save.mining_order = Some(("rock".into(), vec![("iron_ore_0".into(), 2)]));
        save.herd = vec![("chicken#0".into(), 100.0)];

        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((AutoRefine { recipe_id: "smelt_iron".into(), keep: None }, MachineInstanceId("smelter_1".into())));
        apply_save_to_world(&mut world, &save);
        let r = resume_home(&mut world, &data, &save, true, 1.0, None);
        assert_eq!((r.drone_hauls, r.animals_ready), (3, 1));
        assert!((r.away_secs - 3600.0).abs() <= 2.0, "{}", r.away_secs);
        let pending = crate::systems::livestock::pending_herd(&data).expect("waits for the herd");
        assert!(pending[0].1 >= 3700.0, "{pending:?}");

        crate::ecs::systems::System::tick(&mut crate::systems::crafting::CraftingSystem::new(), &mut world, 0.0, &data);
        let ingots: u32 = data
            .get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id == "iron_ingot_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(ingots, 3);
        let ore_filed: u32 = data
            .get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id == "iron_ore_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(ore_filed, 0, "every haul was smelted");
        let coal_left = data.get::<Mutex<std::collections::HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap()["coal_0"];
        assert_eq!(coal_left, 0, "one coal from home storage for each ingot");
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.slots.iter().flatten().count(), 0, "the backpack is not touched");
        assert_eq!(
            away_notice(&r).unwrap(),
            "While you were away (1 h 0 min), 1 animal was ready to collect from again and the drone brought home 3 hauls."
        );
    }

    /// With the toggle off nothing moves on, but what was saved comes back as
    /// saved: the drone stays where it was and the hen's timer is unchanged.
    #[test]
    fn with_offline_progression_off_the_state_comes_back_as_saved() {
        use crate::hot_reload::data_store::DataStore;
        let mut data = DataStore::new();
        data.insert("auto_mine_order", std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
        crate::systems::crafting::register(&mut data);
        crate::systems::livestock::register(&mut data);
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.timestamp = now_secs() - 3600;
        save.asteroids = Some(vec![rock(6.0)]);
        save.mining_order = Some(("rock".into(), vec![("iron_ore_0".into(), 2)]));
        save.herd = vec![("chicken#0".into(), 100.0)];
        // A drone in flight at the save (2026-09-27 review: the doc promised
        // the drone stays where it was, and the save held none).
        let flying = crate::ecs::components::Drone {
            home: 1,
            target: "rock".into(),
            manifest: vec![("iron_ore_0".into(), 2)],
            phase: crate::ecs::components::DronePhase::Returning,
            phase_time: 1.0,
            cargo: vec![("iron_ore_0".into(), 2)],
            home_pos: [0.0; 3],
            target_pos: [0.0; 3],
        };
        save.drone = Some(flying.clone());
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        apply_save_to_world(&mut world, &save);
        let r = resume_home(&mut world, &data, &save, false, 1.0, None);
        assert_eq!((r.away_secs, r.drone_hauls, r.animals_ready), (0.0, 0, 0));
        let drones: Vec<crate::ecs::components::Drone> =
            world.query::<&crate::ecs::components::Drone>().iter().map(|(_, d)| d.clone()).collect();
        assert_eq!(drones.len(), 1, "the drone is still out");
        assert_eq!((drones[0].phase_time, drones[0].cargo.clone()), (1.0, flying.cargo.clone()), "where it was, hold and all");
        assert_eq!(crate::systems::livestock::pending_herd(&data), Some(vec![("chicken#0".to_string(), 100.0)]));
        assert_eq!(crate::systems::mining::standing_order(&data), save.mining_order, "the order is the player's own setting");
        assert!(crate::systems::crafting::away::take(&data).is_none(), "nothing handed to the machines");
    }

    /// The machine catalog the game ships (data/machines/home.ron).
    fn shipped_home() -> crate::machines::MachineHome {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join("home.ron");
        crate::machines::MachineHome::load(&path).expect("home.ron parses")
    }

    fn shipped_containers() -> crate::systems::inventory::containers::ContainerRegistry {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("containers");
        crate::systems::inventory::containers::ContainerRegistry::from_bytes(
            &std::fs::read(root.join("types.csv")).unwrap(),
            &std::fs::read(root.join("content_classes.ron")).unwrap(),
        )
        .unwrap()
    }

    /// Spawn the home's battery bank, water tank, backup genset (with its
    /// fuel drum) and grain silo the way the game does: in menu mode (no
    /// container registry, so no vessels) or at world entry (with it).
    fn spawn_machines(
        world: &mut hecs::World,
        home: &crate::machines::MachineHome,
        containers: Option<&crate::systems::inventory::containers::ContainerRegistry>,
    ) {
        let empty = std::collections::HashMap::new();
        for (id, machine) in
            [("bank_0", "battery_bank"), ("tank_0", "water_tank"), ("genset_0", "generator_portable"), ("silo_0", "grain_silo")]
        {
            let inst = crate::machines::MachineInstance {
                id: id.to_string(),
                machine: machine.to_string(),
                room: "room-plant".to_string(),
                offset: (0.0, 0.0, 0.0),
                rotation: 0.0,
                zone: "home".to_string(),
                screen_source: None,
            };
            crate::engine::home_spawn::spawn_home_machine_entity(world, &inst, &home.catalog[machine], &empty, &empty, None, containers);
        }
    }

    /// (charge as a share of the bank, litres as a share of the tank).
    fn bank_and_tank(world: &hecs::World) -> (f32, f32) {
        use crate::ecs::components::{Battery, WaterTank};
        let b: Vec<f32> = world.query::<&Battery>().iter().map(|(_, b)| b.charge_wh / b.capacity_wh).collect();
        let t: Vec<f32> = world.query::<&WaterTank>().iter().map(|(_, t)| t.liters / t.capacity_l).collect();
        assert_eq!((b.len(), t.len()), (1, 1), "one bank, one tank");
        (b[0], t[0])
    }

    fn set_bank_and_tank(world: &mut hecs::World, charge: f32, water: f32) {
        for (_e, b) in world.query_mut::<&mut crate::ecs::components::Battery>() {
            b.charge_wh = b.capacity_wh * charge;
        }
        for (_e, t) in world.query_mut::<&mut crate::ecs::components::WaterTank>() {
            t.liters = t.capacity_l * water;
        }
    }

    /// A battery bank's charge and a water tank's litres survive a save
    /// through JSON and a restore onto the next launch's fresh spawn, which
    /// starts both at half (2026-09-27: nothing saved them, so every restart
    /// undid the night's discharge or the day's charge). Re-applying lands in
    /// the same place, and a return an hour later with offline progression on
    /// finds them as saved: neither moves on by the time away
    /// (docs/design/offline-progression.md). Seen red by not calling
    /// `machine_levels::restore` in `apply_save_to_world` (both came back at
    /// the spawn half); the offline half pins the decision.
    #[test]
    fn a_banks_charge_and_a_tanks_litres_survive_a_save() {
        let home = shipped_home();
        let mut world = hecs::World::new();
        spawn_machines(&mut world, &home, None);
        assert_eq!(bank_and_tank(&world), (0.5, 0.5), "the spawn levels");
        set_bank_and_tank(&mut world, 0.85, 0.2);
        let mut save = extract_world_save(&world);
        save.timestamp = now_secs() - 3600;
        let save: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();

        let mut fresh = hecs::World::new();
        spawn_machines(&mut fresh, &home, None);
        apply_save_to_world(&mut fresh, &save);
        apply_save_to_world(&mut fresh, &save);
        assert_eq!(bank_and_tank(&fresh), (0.85, 0.2), "the bank and the tank as saved");

        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("auto_mine_order", std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
        crate::systems::crafting::register(&mut data);
        crate::systems::livestock::register(&mut data);
        let r = resume_home(&mut fresh, &data, &save, true, 1.0, None);
        assert!(r.away_secs >= 3600.0, "an hour away counted: {}", r.away_secs);
        assert_eq!(bank_and_tank(&fresh), (0.85, 0.2), "not advanced by the time away");
    }

    /// What a vessel holds (the genset's fuel drum, with what it remembers)
    /// survives a save, and since the menu-mode machines the save lands on
    /// carry no vessel, it waits for world entry: a save written before then
    /// still has it, and world entry puts it in the new drum, with the bank's
    /// charge carried across the respawn too (2026-09-27: every vessel came
    /// back empty, which destroyed what was stored in it, and world entry
    /// reset every bank to half). Seen red with `machine_levels::levels`
    /// leaving out the held contents (the save written in the menu had no
    /// drum, so the fuel was lost at the next restart).
    #[test]
    fn a_vessels_contents_survive_a_save_and_wait_for_world_entry() {
        use crate::ecs::components::{HomeMachine, MachineInstanceId};
        use crate::systems::machine_levels::{self, HeldMachineLevels};
        use crate::systems::inventory::containers::Container;
        let (home, reg) = (shipped_home(), shipped_containers());
        let mut world = hecs::World::new();
        spawn_machines(&mut world, &home, Some(&reg));
        let mut drum = None;
        for (_e, (id, c)) in world.query_mut::<(&MachineInstanceId, &mut Container)>() {
            if id.0 == "genset_0" {
                c.current_content_item = Some("fuel_refined_0".into());
                c.current_qty = 20;
                c.used_liters = 20.0;
                c.last_content = Some("fuel_refined_0".into());
                c.toxic_from = Some("fuel_refined_0".into());
                drum = Some(c.clone());
            }
        }
        let drum = drum.expect("the genset has its drum");
        let save: WorldSave = serde_json::from_str(&serde_json::to_string(&extract_world_save(&world)).unwrap()).unwrap();

        // The next launch: menu-mode machines, no vessels, take the save.
        let mut fresh = hecs::World::new();
        spawn_machines(&mut fresh, &home, None);
        apply_save_to_world(&mut fresh, &save);
        assert_eq!(fresh.query::<&Container>().iter().count(), 0, "no vessels in the menu");
        let vessel_in = |s: &WorldSave| s.machine_levels.iter().find(|l| l.id == "genset_0").and_then(|l| l.vessel.clone());
        assert_eq!(vessel_in(&extract_world_save(&fresh)), Some(drum.clone()), "a save before world entry keeps it");
        set_bank_and_tank(&mut fresh, 0.3, 0.6);

        // World entry, as load_world does it: take, respawn, put back.
        let carried = machine_levels::take_all(&mut fresh);
        let old: Vec<hecs::Entity> = fresh.query::<&HomeMachine>().iter().map(|(e, _)| e).collect();
        for e in old {
            let _ = fresh.despawn(e);
        }
        spawn_machines(&mut fresh, &home, Some(&reg));
        assert!(machine_levels::apply(&mut fresh, &carried).is_empty(), "every level found its machine");
        let vessels: Vec<(String, Container)> =
            fresh.query::<(&MachineInstanceId, &Container)>().iter().map(|(_, (id, c))| (id.0.clone(), c.clone())).collect();
        let genset = vessels.iter().find(|(id, _)| id == "genset_0").map(|(_, c)| c.clone());
        assert_eq!(genset, Some(drum), "the fuel is back in its drum");
        let silo = vessels.iter().find(|(id, _)| id == "silo_0").map(|(_, c)| c.clone()).unwrap();
        assert!(silo.is_empty(), "the silo held nothing and holds nothing");
        assert_eq!(bank_and_tank(&fresh), (0.3, 0.6), "carried across the respawn");
        assert_eq!(fresh.query::<&HeldMachineLevels>().iter().count(), 0, "nothing left waiting");
    }

    /// A save from before the field (no `machine_levels`) still loads, and
    /// every machine keeps its spawn level. Seen red by removing
    /// `#[serde(default)]` from `WorldSave::machine_levels` (the older save
    /// then failed to parse).
    #[test]
    fn a_save_without_machine_levels_keeps_the_spawn_levels() {
        let home = shipped_home();
        let mut world = hecs::World::new();
        spawn_machines(&mut world, &home, None);
        set_bank_and_tank(&mut world, 0.9, 0.1);
        let mut old = serde_json::to_value(extract_world_save(&world)).unwrap();
        assert!(old.as_object_mut().unwrap().remove("machine_levels").is_some(), "the save wrote it");
        let save: WorldSave = serde_json::from_value(old).unwrap();
        assert!(save.machine_levels.is_empty());
        let mut fresh = hecs::World::new();
        spawn_machines(&mut fresh, &home, None);
        apply_save_to_world(&mut fresh, &save);
        assert_eq!(bank_and_tank(&fresh), (0.5, 0.5), "the spawn levels");
        assert_eq!(fresh.query::<&crate::systems::machine_levels::HeldMachineLevels>().iter().count(), 0);
    }

    #[test]
    fn away_notice_says_how_long_and_how_many() {
        let r = |away_secs, crops_aged| Resumed { away_secs, crops_aged, ..Default::default() };
        assert_eq!(away_notice(&r(30.0, 5)), None, "under a minute is not worth a word");
        assert_eq!(away_notice(&r(7200.0, 0)), None, "nothing grew");
        assert_eq!(away_notice(&r(600.0, 1)).unwrap(), "While you were away (10 min), 1 plant kept growing.");
        assert_eq!(away_notice(&r(29_520.0, 12)).unwrap(), "While you were away (8 h 12 min), 12 plants kept growing.");
        assert_eq!(away_notice(&r(3.0 * 86_400.0, 2)).unwrap(), "While you were away (3 days), 2 plants kept growing.");
        let both = Resumed { away_secs: 600.0, crops_aged: 3, builds_advanced: 1, ..Default::default() };
        assert_eq!(
            away_notice(&both).unwrap(),
            "While you were away (10 min), 3 plants kept growing and 1 build kept going up."
        );
        let all = Resumed { away_secs: 600.0, crops_aged: 2, crafts_advanced: 1, animals_ready: 3, drone_hauls: 1, ..Default::default() };
        assert_eq!(
            away_notice(&all).unwrap(),
            "While you were away (10 min), 2 plants kept growing, 1 craft kept working, 3 animals were ready to collect from again and the drone brought home 1 haul."
        );
    }

    /// FINDING 3 (2026-10-02): the trades a backpack has settled are saved
    /// with that backpack, so a restart can neither settle one twice nor lose
    /// one. Seen red: with the `save.settled_trades = settled_trades(world)`
    /// line removed from `extract_world_save`, the save carried no record of
    /// t-1 (the first assert), so a restart would settle it again.
    #[test]
    fn settled_trades_ride_the_save_with_the_backpack() {
        use crate::systems::inventory::{TradeSettlements, TransferOp};
        let player = |world: &mut hecs::World| {
            world.spawn((
                Controllable,
                Inventory::new(16),
                PlayerSkills::new(),
                crate::ecs::components::Name("Astra".to_string()),
                crate::ecs::components::Appearance::default(),
                crate::ecs::components::Outfit::default(),
            ))
        };
        let settled = |world: &hecs::World, e| world.get::<&TradeSettlements>(e).map(|t| (*t).clone()).unwrap_or_default();
        let mut world = hecs::World::new();
        let p = player(&mut world);
        let mut ts = TradeSettlements::default();
        ts.settled.insert("t-1".into());
        // Queued but not yet applied: its items have not moved, so it must
        // not be saved as settled (it settles again after a restart).
        ts.pending.push(("t-2".into(), vec![TransferOp { item_id: "rope_0".into(), qty: 1, add: true, ..Default::default() }]));
        world.insert_one(p, ts).unwrap();

        let save = extract_world_save(&world);
        assert_eq!(save.settled_trades, vec!["t-1".to_string()]);
        // Through the file format and back.
        let text = serde_json::to_string(&save).unwrap();
        let save: WorldSave = serde_json::from_str(&text).unwrap();

        // A restart: a fresh player, the save applied.
        let mut fresh = hecs::World::new();
        let q = player(&mut fresh);
        apply_save_to_world(&mut fresh, &save);
        assert_eq!(settled(&fresh, q).settled.iter().collect::<Vec<_>>(), vec!["t-1"]);
        // The relay's list brings t-1 back as completed: nothing moves again.
        let mut gs = crate::gui::GuiState::default();
        gs.profile_public_key = "bob".into();
        gs.trades.push(crate::gui::GuiTrade {
            id: "t-1".into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: "completed".into(),
            initiator_items: vec![crate::gui::GuiTradeItem {
                name: "Hammer".into(),
                quantity: 1,
                reference_id: Some("hammer_0".into()),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(crate::gui::pages::trade::settle_completed(&mut gs, &mut fresh, "t-1", |_: &str| true).is_none());
        assert!(settled(&fresh, q).pending.is_empty());

        // A save from before the field loads with none settled.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("settled_trades");
        let old: WorldSave = serde_json::from_value(old).unwrap();
        assert!(old.settled_trades.is_empty());

        // A fresh home ("Start every session from the default home") keeps
        // the record too, added to what this session settled, so an old trade
        // is never replayed into a new default backpack.
        let mut live = hecs::World::new();
        let r = player(&mut live);
        let mut ts = TradeSettlements::default();
        ts.settled.insert("t-5".into());
        live.insert_one(r, ts).unwrap();
        apply_identity(&mut live, &save);
        assert_eq!(settled(&live, r).settled.iter().collect::<Vec<_>>(), vec!["t-1", "t-5"]);
        let kept = identity_only_save(Some(save.clone()), &live);
        assert_eq!(kept.settled_trades, vec!["t-1".to_string(), "t-5".to_string()]);
    }

    /// THE FLEET'S HELD GIVES AND THE HOME'S ID RIDE THE SAVE WITH THE BACKPACK (the fleet
    /// ledger's review, 2026-10-04, findings 1, 2, 3 and 12): a give whose items left the
    /// backpack is saved with it, so a restart neither loses those items nor gives them twice;
    /// the home's id travels with the save, so a save loaded back asks the fleet for ITS gives
    /// (and marks the fleet's record to be asked for again); a fresh home ("Start every session
    /// from the default home") keeps the live ones.
    ///
    /// Seen red 2026-10-04 with the fleet lines taken out of `extract_world_save`: "the save
    /// carries the home's id / left: \"\" / right: \"home-a\"".
    #[test]
    fn fleet_held_gives_and_the_home_id_ride_the_save() {
        use crate::systems::inventory::{FleetHeld, TradeSettlements};
        let player = |world: &mut hecs::World| {
            world.spawn((
                Controllable,
                Inventory::new(16),
                PlayerSkills::new(),
                crate::ecs::components::Name("Astra".to_string()),
                crate::ecs::components::Appearance::default(),
                crate::ecs::components::Outfit::default(),
            ))
        };
        let ts_of = |world: &hecs::World, e| world.get::<&TradeSettlements>(e).map(|t| (*t).clone()).unwrap_or_default();
        let mut world = hecs::World::new();
        let p = player(&mut world);
        let held = FleetHeld { give_id: "g-1".into(), server: "wss://here".into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty: 3, ..Default::default() };
        world.insert_one(p, TradeSettlements { home_id: "home-a".into(), fleet_held: vec![held.clone()], ..Default::default() }).unwrap();

        let save = extract_world_save(&world);
        assert_eq!(save.home_id, "home-a", "the save carries the home's id / left: {:?} / right: \"home-a\"", save.home_id);
        assert_eq!(save.fleet_held, vec![held.clone()]);
        let text = serde_json::to_string(&save).unwrap();
        let save: WorldSave = serde_json::from_str(&text).unwrap();

        // Loaded back over another home's backpack: the save's home, its held give, and the
        // fleet's record to be asked for again.
        let mut other = hecs::World::new();
        let q = player(&mut other);
        other.insert_one(q, TradeSettlements { home_id: "home-b".into(), ..Default::default() }).unwrap();
        apply_save_to_world(&mut other, &save);
        let t = ts_of(&other, q);
        assert_eq!((t.home_id.as_str(), t.fleet_held.len(), t.fleet_recheck), ("home-a", 1, true), "{t:?}");

        // A fresh home keeps the live ones.
        let mut live = hecs::World::new();
        let r = player(&mut live);
        live.insert_one(r, TradeSettlements { home_id: "home-fresh".into(), ..Default::default() }).unwrap();
        apply_identity(&mut live, &save);
        let t = ts_of(&live, r);
        assert_eq!((t.home_id.as_str(), t.fleet_held.len(), t.fleet_recheck), ("home-fresh", 0, false), "{t:?}");

        // A save from before the fields loads with none.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("home_id");
        old.as_object_mut().unwrap().remove("fleet_held");
        let old: WorldSave = serde_json::from_value(old).unwrap();
        assert!(old.home_id.is_empty() && old.fleet_held.is_empty());
    }
}

/// The player's body in the save (first-hour audit S1, 2026-10-04).
#[cfg(test)]
#[path = "save_load_body_tests.rs"]
mod body_tests;
