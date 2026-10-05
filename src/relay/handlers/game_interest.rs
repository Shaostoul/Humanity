//! Who is in view of whom, and so who is sent what (increment 4 of
//! docs/design/ship-homes-and-logistics.md, section 5.10: "Deliver game events by zone, or to the
//! people involved, not to every socket").
//!
//! Until this increment every position update, every crew member's move and every line of crew
//! chatter went to EVERY socket on the server, chat-only ones included: about 45 KB/s per socket
//! at 12 players from positions alone (docs/design/shared-building.md section 5). Now a mover's
//! news goes only to the players in the shared world who have it IN VIEW: within
//! `in_view_m` (data/ship/shared_world.ron `delivery`), and out of view again past
//! `out_of_view_m`, the gap keeping someone walking along the edge from flickering.
//!
//! WHY BY DISTANCE AND NOT BY ROOM. The design says "by zone". The relay's rooms are the ship's
//! zones and the labelled volumes in them (ship_world.rs), but a room is not a line of sight: the
//! walker in its corridor is seen from the Commons through the open door (the co-presence rig's
//! meeting judges exactly that, `meet_walker_through_its_corridor`), and a person across First
//! Street's glass lid would vanish at the zone's edge. A distance with a gap is what "nearby"
//! means for both, and costs one pass over the movers per move. On the first test ship every
//! place is within 210 m of every other, so with the shipped 250 m everyone aboard sees everyone,
//! and nothing a player sees changes; what changes is that nothing goes to sockets that are not
//! in the shared world.
//!
//! COMING INTO VIEW AND GOING OUT. A mover that comes into a player's view is sent to that player
//! whole (`game_in_view`: its snapshot entry, as in the welcome), because nothing about it reached
//! them while it was away and a crew member dwelling at a chore sends no moves; one that goes out
//! of view is taken off their screen (`game_out_of_view`), or it would stand frozen where it was
//! last seen. A player's welcome lists only the movers in their view (`snapshot_for`), and their
//! join is announced only to the players they are in view of.

use super::game_state::{EntitySnapshot, GameWorld};
use glam::Vec3;
use std::collections::HashSet;

/// Who sees whom: (viewer, seen) pairs, the viewer always a player, the seen a player or a crew
/// member. Never saved: every player is reaped on a restart, and a join re-judges its own.
#[derive(Debug, Clone, Default)]
pub struct Interest {
    seen: HashSet<(u64, u64)>,
}

/// One pair that changed: `viewer` now has `seen` in view (`in_view`), or no longer.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewChange {
    pub viewer: u64,
    pub seen: u64,
    pub in_view: bool,
}

impl Interest {
    /// Forget everyone `id` saw and everyone who saw `id` (they left the world).
    pub fn forget(&mut self, id: u64) {
        self.seen.retain(|(a, b)| *a != id && *b != id);
    }

    /// True when `viewer` has `seen` in view.
    pub fn sees(&self, viewer: u64, seen: u64) -> bool {
        self.seen.contains(&(viewer, seen))
    }

    /// Judge one pair at distance `d`: in view within `in_m`, out past `out_m`, unchanged between.
    fn judge(&mut self, viewer: u64, seen: u64, d: f32, in_m: f32, out_m: f32, out: &mut Vec<ViewChange>) {
        let now = self.seen.contains(&(viewer, seen));
        if !now && d <= in_m {
            self.seen.insert((viewer, seen));
            out.push(ViewChange { viewer, seen, in_view: true });
        } else if now && d > out_m {
            self.seen.remove(&(viewer, seen));
            out.push(ViewChange { viewer, seen, in_view: false });
        }
    }
}

/// True for a crew member (an entity with a chore loop, game_state.rs `populate_ship_entities`).
fn is_crew(e: &super::game_state::GameEntity) -> bool {
    e.components.get("chore_agent").is_some()
}

/// What anyone else is told of entity `e`'s components (a welcome, `game_in_view`, an AI's
/// `game_query_entity` or `game_interact`): all of them, except where a player lives. A player's
/// entity carries their plot (`home_plot`: id, origin and size, set on every join by
/// ship_world.rs `set_home_plot`, and the plot their explore quest names) for the relay's own
/// rules; nobody else is told it (the design, section 5.10: "No endpoint lists who lives
/// where"; the review of increment 4, P7). Nothing in the game reads another player's plot: a
/// neighbour's home is drawn from the ship file's plots (src/ship/neighbours.rs).
pub fn components_seen_by_others(e: &super::game_state::GameEntity) -> serde_json::Value {
    let mut c = e.components.clone();
    if let Some(o) = c.as_object_mut() {
        o.remove("home_plot");
        if let Some(q) = o.get_mut("current_quest").and_then(|q| q.as_object_mut()) {
            q.remove("home_plot");
        }
    }
    c
}

impl GameWorld {
    /// Re-judge every pair `moved` (a player or a crew member) is in, after it moved or arrived:
    /// a player against every other player (both ways) and every crew member; a crew member
    /// against every player. Returns the pairs that changed.
    pub fn rejudge_view(&mut self, moved: u64) -> Vec<ViewChange> {
        let (in_m, out_m) = (self.rules.delivery.in_view_m, self.rules.delivery.out_of_view_m);
        let Some(me) = self.entities.get(&moved) else { return Vec::new() };
        let at = Vec3::from(me.position);
        let me_player = me.entity_type == "player";
        if !me_player && !is_crew(me) {
            return Vec::new();
        }
        let others: Vec<(u64, f32, bool)> = self
            .entities
            .iter()
            .filter(|(id, e)| **id != moved && (e.entity_type == "player" || is_crew(e)))
            .map(|(id, e)| (*id, at.distance(Vec3::from(e.position)), e.entity_type == "player"))
            .collect();
        let mut out = Vec::new();
        for (other, d, other_player) in others {
            if me_player {
                self.interest.judge(moved, other, d, in_m, out_m, &mut out);
            }
            if other_player {
                self.interest.judge(other, moved, d, in_m, out_m, &mut out);
            }
        }
        out
    }

    /// The keys of the players who have `seen` in view: who its moves go to.
    pub fn viewer_keys(&self, seen: u64) -> HashSet<String> {
        self.entities
            .iter()
            .filter(|(id, e)| e.entity_type == "player" && self.interest.sees(**id, seen))
            .filter_map(|(_, e)| e.owner.clone())
            .collect()
    }

    /// The welcome's world snapshot for player `viewer`: every entity but the movers (players and
    /// crew) out of their view; themselves always, with their own plot, and every other player
    /// without theirs (`components_seen_by_others`).
    pub fn snapshot_for(&self, viewer: u64) -> Vec<EntitySnapshot> {
        self.snapshot()
            .into_iter()
            .filter(|s| {
                let mover = self.entities.get(&s.entity_id).is_some_and(|e| e.entity_type == "player" || is_crew(e));
                !mover || s.entity_id == viewer || self.interest.sees(viewer, s.entity_id)
            })
            .map(|mut s| {
                if s.entity_id != viewer {
                    if let Some(e) = self.entities.get(&s.entity_id) {
                        s.components = components_seen_by_others(e);
                    }
                }
                s
            })
            .collect()
    }

    /// One entity as the welcome's snapshot lists it, for a `game_in_view` (always about someone
    /// else: a player never comes into their own view).
    pub fn entity_entry_json(&self, id: u64) -> Option<serde_json::Value> {
        let e = self.entities.get(&id)?;
        Some(serde_json::json!({
            "entity_id": id,
            "entity_type": e.entity_type,
            "position": e.position,
            "rotation": e.rotation,
            "components": components_seen_by_others(e),
        }))
    }

    /// The messages `changes` sends, as (player key, message): each viewer is sent the mover that
    /// came into view (`game_in_view`, its snapshot entry) or the id of the one that went out
    /// (`game_out_of_view`). `skip_viewer` is left out (a joiner, whose welcome lists its view).
    pub fn view_messages(&self, changes: &[ViewChange], skip_viewer: Option<u64>) -> Vec<(String, serde_json::Value)> {
        changes
            .iter()
            .filter(|c| Some(c.viewer) != skip_viewer)
            .filter_map(|c| {
                let key = self.entities.get(&c.viewer)?.owner.clone()?;
                let msg = if c.in_view {
                    serde_json::json!({ "type": "game_in_view", "entity": self.entity_entry_json(c.seen)? })
                } else {
                    serde_json::json!({ "type": "game_out_of_view", "entity_id": c.seen })
                };
                Some((key, msg))
            })
            .collect()
    }
}

/// Send one game message to the players named (the relay's send loop delivers it to their sockets
/// only, relay.rs `RelayMessage::GameTo`). Nothing goes out to no one.
pub fn send_to(state: &crate::relay::relay::RelayState, to: HashSet<String>, msg: &serde_json::Value) {
    if to.is_empty() {
        return;
    }
    let _ = state.broadcast_tx.send(crate::relay::relay::RelayMessage::GameTo { to: std::sync::Arc::new(to), message: format!("__game__:{msg}") });
}

/// Send each (player key, message) pair (`GameWorld::view_messages`).
pub fn send_each(state: &crate::relay::relay::RelayState, msgs: Vec<(String, serde_json::Value)>) {
    for (key, msg) in msgs {
        send_to(state, HashSet::from([key]), &msg);
    }
}

impl GameWorld {
    /// What the crew's chore events of one tick send, and to whom: each event (`game_npc_update`)
    /// to the players who have that crew member in view, after the move it reports is re-judged
    /// (a player it just came near is sent it whole, one it left takes it off their screen).
    pub fn npc_deliveries(&mut self, events: Vec<super::game_state::NpcChoreEvent>) -> Vec<(HashSet<String>, serde_json::Value)> {
        let mut out = Vec::new();
        for ev in events {
            let changes = self.rejudge_view(ev.entity_id);
            out.extend(self.view_messages(&changes, None).into_iter().map(|(k, m)| (HashSet::from([k]), m)));
            let payload = serde_json::json!({
                "type": "game_npc_update",
                "entity_id": ev.entity_id,
                "name": ev.name,
                "position": ev.position,
                "chore_id": ev.chore_id,
                "chore_label": ev.chore_label,
                "chore_state": ev.chore_state,
                "room_id": ev.room_id,
            });
            out.push((self.viewer_keys(ev.entity_id), payload));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved(world: &mut GameWorld, id: u64, to: [f32; 3]) -> Vec<ViewChange> {
        world.update_position(id, to, [0.0, 0.0, 0.0, 1.0]);
        world.rejudge_view(id)
    }

    /// TWO PLAYERS FAR APART ARE NOT SENT EACH OTHER'S MOVES: with a 50 m view (60 m to lose it)
    /// the players at the two front doors, 99 m apart, see nothing of each other; walking to 39.5 m
    /// brings each into the other's view (both ways, in one re-judging); back to 54.5 m keeps them
    /// (the gap); 69.5 m takes them out. Seen red 2026-10-04 with `viewer_keys` answering every
    /// player in the world, as the relay delivered before: "a player 99 m away was sent the
    /// move".
    #[test]
    fn players_out_of_view_are_not_sent_each_others_moves() {
        let mut world = GameWorld::new();
        world.rules.delivery = crate::ship::moves::DeliveryRules { in_view_m: 50.0, out_of_view_m: 60.0 };
        let a = world.spawn_player("e11e00aa", [53.5, 1.7, 40.5]);
        let b = world.spawn_player("e11e00bb", [53.5, 1.7, 139.5]);
        world.rejudge_view(a);
        world.rejudge_view(b);
        assert!(!world.viewer_keys(b).contains("e11e00aa"), "a player 99 m away was sent the move");
        // (Near the Commons, crew members come into b's view too; only the pair matters here.)
        let pair = |cs: Vec<ViewChange>| -> Vec<ViewChange> { cs.into_iter().filter(|c| (c.viewer, c.seen) == (a, b) || (c.viewer, c.seen) == (b, a)).collect() };
        assert!(pair(moved(&mut world, b, [53.5, 1.7, 95.0])).is_empty(), "54.5 m is not yet in view");
        let came = pair(moved(&mut world, b, [53.5, 1.7, 80.0]));
        assert_eq!(came.len(), 2, "both see each other at 39.5 m: {came:?}");
        assert!(came.iter().all(|c| c.in_view));
        assert!(world.viewer_keys(b).contains("e11e00aa") && world.viewer_keys(a).contains("e11e00bb"));
        assert!(pair(moved(&mut world, b, [53.5, 1.7, 95.0])).is_empty(), "54.5 m keeps them in view (the gap)");
        let went = pair(moved(&mut world, b, [53.5, 1.7, 110.0]));
        assert_eq!(went.len(), 2, "69.5 m takes both out: {went:?}");
        assert!(went.iter().all(|c| !c.in_view));
        // What each is sent: the other's whole entry coming in, its id going out.
        let msgs = world.view_messages(&came, None);
        assert!(msgs.iter().any(|(k, m)| k == "e11e00aa" && m["type"] == "game_in_view" && m["entity"]["entity_id"] == b));
        let gone = world.view_messages(&went, None);
        assert!(gone.iter().any(|(k, m)| k == "e11e00aa" && m["type"] == "game_out_of_view" && m["entity_id"] == b));
    }

    /// THE CREW ARE IN VIEW ONLY NEAR THEM: with a 50 m view, a player at p2's door is sent no
    /// crew member in their welcome (every one works the Commons, more than 60 m off) but is sent
    /// themselves and every thing that does not move; walking into the Commons brings the crew
    /// into view, each sent whole with the lines it says. Seen red 2026-10-04 with `snapshot_for`
    /// listing everything: "no crew member in the far player's welcome" (left: 6).
    #[test]
    fn the_crew_are_in_view_only_near_them() {
        let mut world = GameWorld::new();
        world.rules.delivery = crate::ship::moves::DeliveryRules { in_view_m: 50.0, out_of_view_m: 60.0 };
        let p = world.spawn_player("e11e00cc", [53.5, 1.7, 139.5]);
        world.rejudge_view(p);
        let snap = world.snapshot_for(p);
        let crew_in = |snap: &[EntitySnapshot]| snap.iter().filter(|s| s.components.get("chore_agent").is_some()).count();
        assert_eq!(crew_in(&snap), 0, "no crew member in the far player's welcome");
        assert!(snap.iter().any(|s| s.entity_id == p), "the player is in their own welcome");
        assert!(snap.iter().any(|s| s.entity_type == "food_store"), "things that do not move are always listed");
        let came = moved(&mut world, p, [80.0, 1.7, 50.0]);
        let msgs = world.view_messages(&came, None);
        let crew: Vec<&serde_json::Value> = msgs.iter().filter(|(_, m)| m["type"] == "game_in_view" && m["entity"]["components"].get("dialog").is_some()).map(|(_, m)| m).collect();
        assert!(crew.len() >= 5, "the crew came into view in the Commons: {}", crew.len());
        assert!(crew_in(&world.snapshot_for(p)) >= 5);
    }

    /// ON THE SHIPPED SHIP EVERYONE ABOARD SEES EVERYONE: with the shipped view, the two places
    /// aboard farthest apart (the farthest two corners of every zone, plot and corridor of the
    /// relay's own ship file, measured, not picked: First Street's far end and the far corner of
    /// p1, about 209 m) are in each other's view, and two players standing there see each other,
    /// so nothing anyone sees changes on this ship. (The review of increment 4, R3: the test
    /// used to stand two players at a hand-picked pair 193.6 m apart, which a view of 200 m
    /// passed while it hid the real farthest pair.)
    ///
    /// Seen red 2026-10-04 with the shipped file's view at 150 m: "assertion failed:
    /// world.viewer_keys(b).contains(\"e11e00dd\") && world.viewer_keys(a).contains(\"e11e00ee\")";
    /// and, the farthest pair measured, with the view at 200 m (which the hand-picked pair
    /// passed): "the farthest two corners aboard, zone street-1 at [75, 4, 195] and plot p1 at
    /// [0, 0, 0], are 209.0 m apart, past the 200 m view".
    #[test]
    fn on_the_shipped_ship_everyone_aboard_sees_everyone() {
        use crate::ship::ship_space::{tube_box, Aabb};
        use crate::ship::ship_structure::{HomeDesign, ShipStructure};
        let mut world = GameWorld::new();
        let ship = ShipStructure::load_ship_file(std::path::Path::new("data")).expect("the relay's ship file");
        let mut boxes: Vec<(String, Aabb)> = Vec::new();
        for z in &ship.zones {
            let o = z.origin_vec();
            boxes.push((format!("zone {}", z.id), (o, o + Vec3::new(z.body.width, z.body.height, z.body.depth))));
        }
        for p in &ship.plots {
            boxes.push((format!("plot {}", p.id), p.aabb()));
            if let Ok(g) = ship.plot_door_tube(p, HomeDesign::built_in_ref(&p.kind)) {
                boxes.push((format!("plot {}'s corridor", p.id), tube_box(&g)));
            }
        }
        for c in &ship.corridors {
            if let Ok(g) = ship.corridor_geometry(c) {
                boxes.push((format!("corridor {} to {}", c.from_zone, c.to_zone), tube_box(&g)));
            }
        }
        let corners = |(lo, hi): Aabb| [0u8, 1, 2, 3, 4, 5, 6, 7].map(|i| Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }));
        let all: Vec<(String, Vec3)> = boxes.iter().flat_map(|(n, b)| corners(*b).map(|c| (n.clone(), c))).collect();
        let (mut far, mut pair) = (0.0_f32, None);
        for (i, (na, a)) in all.iter().enumerate() {
            for (nb, b) in &all[i + 1..] {
                if a.distance(*b) > far {
                    far = a.distance(*b);
                    pair = Some((na.clone(), *a, nb.clone(), *b));
                }
            }
        }
        let (na, a_at, nb, b_at) = pair.expect("the ship has places");
        let view = world.rules.delivery.in_view_m;
        assert!(far <= view, "the farthest two corners aboard, {na} at {a_at} and {nb} at {b_at}, are {far:.1} m apart, past the {view} m view");
        // Two players standing there (at eye height over each corner's floor) see each other.
        let a = world.spawn_player("e11e00dd", [a_at.x, 1.7, a_at.z]);
        let b = world.spawn_player("e11e00ee", [b_at.x, 1.7, b_at.z]);
        world.rejudge_view(a);
        assert!(world.viewer_keys(b).contains("e11e00dd") && world.viewer_keys(a).contains("e11e00ee"));
        assert_eq!(world.snapshot_for(a).len(), world.snapshot().len(), "the welcome lists everything");
    }

    /// NOBODY IS TOLD WHO LIVES WHERE (the review of increment 4, P7; the design, section 5.10:
    /// "No endpoint lists who lives where"). A player's entity carries their plot (`home_plot`:
    /// id, origin and size, and the plot their explore quest names) for the relay's own rules, and
    /// the welcome's snapshot and `game_in_view` sent it, with their name, to every other player.
    /// Nothing in the game reads another player's plot (a neighbour's home is drawn from the
    /// ship file's plots, src/ship/neighbours.rs). Now another player's entry says nothing of it;
    /// a player's own entry keeps it.
    ///
    /// Seen red 2026-10-04 on the code before the fix: "a's welcome says where b lives: {...,
    /// \"current_quest\":{... \"home_plot\":\"p2\" ...}, ..., \"home_plot\":{\"id\":\"p2\",
    /// \"origin\":[0.0,0.0,99.0],\"size\":[55.0,3.0,89.0]}, ..., \"name\":\"Bea\", ...}".
    #[test]
    fn nobody_is_told_who_lives_where() {
        let mut world = GameWorld::new();
        let a = world.spawn_player("e11e00f1", [53.5, 1.7, 40.5]);
        let b = world.spawn_player("e11e00f2", [53.5, 1.7, 139.5]);
        let (p1, p2) = (world.ship_plots.plot("p1").cloned().expect("p1"), world.ship_plots.plot("p2").cloned().expect("p2"));
        world.set_home_plot(a, Some(&p1));
        world.set_home_plot(b, Some(&p2));
        if let Some(o) = world.entities.get_mut(&b).and_then(|e| e.components.as_object_mut()) {
            o.insert("name".to_string(), serde_json::json!("Bea"));
        }
        world.rejudge_view(a);
        world.rejudge_view(b);
        let says_where = |c: &serde_json::Value| c.get("home_plot").is_some_and(|p| !p.is_null()) || c.get("current_quest").and_then(|q| q.get("home_plot")).is_some_and(|p| !p.is_null());
        let snap = world.snapshot_for(a);
        let theirs = snap.iter().find(|s| s.entity_id == b).expect("b is in a's welcome");
        assert!(!says_where(&theirs.components), "a's welcome says where b lives: {}", theirs.components);
        let own = snap.iter().find(|s| s.entity_id == a).expect("a is in their own welcome");
        assert!(own.components.get("home_plot").is_some_and(|p| p["id"] == "p1"), "a's own entry keeps their plot");
        let came = world.entity_entry_json(b).expect("b's entry");
        assert!(!says_where(&came["components"]), "game_in_view says where b lives: {}", came["components"]);
        assert_eq!(came["components"]["name"], "Bea", "their name is still said");
    }
}
