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
    /// crew) out of their view; themselves always.
    pub fn snapshot_for(&self, viewer: u64) -> Vec<EntitySnapshot> {
        self.snapshot()
            .into_iter()
            .filter(|s| {
                let mover = self.entities.get(&s.entity_id).is_some_and(|e| e.entity_type == "player" || is_crew(e));
                !mover || s.entity_id == viewer || self.interest.sees(viewer, s.entity_id)
            })
            .collect()
    }

    /// One entity as the welcome's snapshot lists it, for a `game_in_view`.
    pub fn entity_entry_json(&self, id: u64) -> Option<serde_json::Value> {
        let e = self.entities.get(&id)?;
        Some(serde_json::json!({
            "entity_id": id,
            "entity_type": e.entity_type,
            "position": e.position,
            "rotation": e.rotation,
            "components": e.components,
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
    /// into view, each sent whole with the lines it says.
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

    /// ON THE SHIPPED SHIP EVERYONE ABOARD SEES EVERYONE: with the shipped 250 m view, players at
    /// the farthest two corners of the ship's places (p2's far corner and the Commons' far one,
    /// about 195 m apart) are in each other's view, so nothing anyone sees changes on this ship.
    #[test]
    fn on_the_shipped_ship_everyone_aboard_sees_everyone() {
        let mut world = GameWorld::new();
        let a = world.spawn_player("e11e00dd", [0.5, 1.7, 187.5]);
        let b = world.spawn_player("e11e00ee", [98.5, 1.7, 20.5]);
        world.rejudge_view(a);
        assert!(world.viewer_keys(b).contains("e11e00dd") && world.viewer_keys(a).contains("e11e00ee"));
        assert_eq!(world.snapshot_for(a).len(), world.snapshot().len(), "the welcome lists everything");
    }
}
