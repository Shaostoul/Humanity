//! The relay's speed check (increment 4 of docs/design/ship-homes-and-logistics.md, section
//! 5.10: "Replace the 100 m rule with a speed check ... A refused update sends the sender a
//! correction instead of freezing their stored position").
//!
//! THE RULE. Every player has an ALLOWANCE of distance, across the floor and up or down, that
//! builds up at the fastest a person can go (data/ship/shared_world.ron `moving`, with its margin
//! for uneven arrival) and is kept for at most `banked_s` seconds. A position update may move
//! them as far as the allowance holds, plus a fixed slack; it spends what it uses. One that moves
//! them farther is answered with a CORRECTION (`game_position_correction`): where the relay holds
//! them, which the game stands them back at. Nothing is relayed for it, and the player is never
//! left frozen, which is what the old rule did: it refused such an update without a word, kept
//! them where they were, and refused every later one too, so everyone else saw a statue while
//! the player walked on alone.
//!
//! THE CLOCK. The allowance builds on the RELAY's clock, not on the `timestamp` a game sends (the
//! design said "on the sender's own clock"): a game's own clock is whatever the game says it is,
//! and a modified one could claim ten seconds between two updates to move 250 m. The relay's clock
//! cannot be told anything; the margin and the banked seconds are what absorb updates that set
//! off evenly and arrive bunched.
//!
//! UPDATES ALREADY ON THEIR WAY. After a correction the game keeps sending from where it stood
//! until the correction reaches it. Each update carries the number of the last correction the
//! game applied (`correction`); one older than the newest correction sent is dropped quietly, and
//! the correction is sent again if the game has still not applied it after
//! `correction_resend_s`. A welcome forgives a pending correction (the game stands where the
//! welcome says).
//!
//! THE FAST MOVES THAT ARE REAL are told apart, each checked against what the relay knows:
//!   - spawning (a first join, Respawn, stepping out and back in) puts the player where the relay
//!     says, so their first update is near it: a fresh allowance (`MoveState::fresh`);
//!   - a reconnect that finds them still in the world (the 90 s grace) grants ONE move as far as
//!     they could have gone while away, at most `FAR_FROM_HELD_M` (`MoveState::rejoin`);
//!   - a transit link (`MoveDecl::Link`): in a shared zone it must be a link of the relay's own
//!     ship file, by its ids, and in the player's home both ends must be on their own plot; the
//!     walk to the entry pad and from the exit pad must fit the allowance;
//!   - shutting the build editor (`MoveDecl::Editor`): the build spot must be on their own plot,
//!     at most `FAR_FROM_HELD_M` from where the relay holds them;
//!   - driving (`MoveDecl::Vehicle`): that vehicle's own speed, from data/vehicles/kits.ron, when
//!     it is faster than on foot (no shipped vehicle is: the fastest is 12 m/s).

use crate::ship::moves::{floor_distance, MoveDecl, MovingRules, FAR_FROM_HELD_M};
use crate::ship::ship_space::Aabb;
use crate::ship::transit::TransitLink;
use glam::Vec3;

/// The relay's clock for the speed check, seconds since the first call: monotonic, and the same
/// clock for every player.
pub fn relay_now_s() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// One player's allowance and corrections (kept by entity id on `GameWorld::moves`, never
/// saved: a restart reaps every player, and their next join starts afresh).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveState {
    /// When the allowance was last brought up to date, relay seconds.
    pub at_s: f64,
    /// Distance across the floor the player may still move, metres.
    pub bank_h: f32,
    /// Distance up or down, metres.
    pub bank_v: f32,
    /// A one-shot allowance a reconnect grants (`rejoin`), spent by the next accepted move.
    pub grant_m: f32,
    /// How many corrections have been sent (the newest one's number).
    pub seq: u64,
    /// The newest correction the game has said it stood at.
    pub acked: u64,
    /// When the newest correction was sent, relay seconds.
    pub last_correction_s: f64,
    /// Why (the reason a resend repeats).
    pub last_reason: &'static str,
}

/// What to do with one position update.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Take it: the relay holds the player there now, and relays it.
    Accept,
    /// Send a correction (the newest, `seq`), and take nothing.
    Correct { reason: &'static str },
    /// Send the pending correction again, and take nothing.
    Resend,
    /// Quietly ignore it: an update sent before the game stood where it was corrected to.
    Drop,
}

/// What the relay knows to judge one move.
pub struct MoveContext<'a> {
    pub rules: &'a MovingRules,
    /// Where the relay holds the player, ship metres.
    pub held: Vec3,
    /// Where the update puts them.
    pub to: Vec3,
    /// What the update says the move was (None: walking).
    pub declared: Option<&'a MoveDecl>,
    /// The update's `correction`: the newest correction the game applied (absent reads as 0).
    pub applied: u64,
    /// The player's own plot box (None for a guest).
    pub own_plot: Option<Aabb>,
    /// The ship's transit links in its shared zones (the relay's ship file).
    pub links: &'a [TransitLink],
    /// The declared vehicle's speed, metres a second, when data/vehicles/kits.ron has it.
    pub vehicle_mps: Option<f32>,
}

/// How far from a transit pad's middle someone standing in it can be (a teleporter's footprint),
/// for a link in a home, which the relay has no copy of.
fn pad_reach_m() -> f32 {
    crate::ship::structure::structure_type("teleporter").map_or(1.0, |t| (t.size.0 * 0.5 + 0.05).hypot(t.size.2 * 0.5 + 0.05))
}

/// True when `p` stands on the box across the floor (x and z, 10 cm of slack).
fn on_plot(b: &Aabb, p: Vec3) -> bool {
    p.x >= b.0.x - 0.1 && p.x <= b.1.x + 0.1 && p.z >= b.0.z - 0.1 && p.z <= b.1.z + 0.1
}

impl MoveState {
    /// A player who has just been spawned where the relay put them: a full allowance.
    pub fn fresh(now_s: f64, rules: &MovingRules) -> Self {
        let (h, v) = Self::caps(rules, rules.on_foot_mps);
        MoveState { at_s: now_s, bank_h: h, bank_v: v, grant_m: 0.0, seq: 0, acked: 0, last_correction_s: f64::NEG_INFINITY, last_reason: "too_fast" }
    }

    /// A reconnect that found the player still in the world: one move as far as they could have
    /// gone since their last accepted update (on foot, with the margin), at most
    /// `FAR_FROM_HELD_M`, the same distance a welcome lets a game keep standing where it is
    /// (engine/home_plot.rs `stand_where_held`). A pending correction is forgiven: the welcome
    /// tells the game where it is held.
    pub fn rejoin(&mut self, now_s: f64, rules: &MovingRules) {
        let away_s = (now_s - self.at_s).max(0.0) as f32;
        self.grant_m = (rules.on_foot_mps * (1.0 + rules.jitter_margin) * away_s).min(FAR_FROM_HELD_M);
        self.acked = self.seq;
    }

    /// The most a player can bank across the floor and up or down at `speed_h`.
    fn caps(rules: &MovingRules, speed_h: f32) -> (f32, f32) {
        let k = (1.0 + rules.jitter_margin) * rules.banked_s;
        (speed_h * k, rules.vertical_mps * k)
    }

    /// Judge one update (see the top of this file), and keep the books: the allowance spent, a
    /// correction counted. The caller holds the player at `ctx.to` on `Accept` and leaves them
    /// where they are otherwise.
    pub fn check(&mut self, now_s: f64, ctx: &MoveContext) -> Verdict {
        let r = ctx.rules;
        // An update the game sent before it stood where it was last corrected to.
        if self.acked < self.seq {
            if ctx.applied >= self.seq {
                self.acked = self.seq;
            } else if now_s - self.last_correction_s >= f64::from(r.correction_resend_s) {
                self.last_correction_s = now_s;
                return Verdict::Resend;
            } else {
                return Verdict::Drop;
            }
        }
        // Bring the allowance up to date: it builds at the speed this move may go at.
        let speed_h = match ctx.declared {
            Some(MoveDecl::Vehicle { .. }) => r.on_foot_mps.max(ctx.vehicle_mps.unwrap_or(0.0)),
            _ => r.on_foot_mps,
        };
        let (cap_h, cap_v) = Self::caps(r, speed_h);
        let dt = (now_s - self.at_s).max(0.0) as f32;
        self.at_s = now_s;
        self.bank_h = (self.bank_h + speed_h * (1.0 + r.jitter_margin) * dt).min(cap_h);
        self.bank_v = (self.bank_v + r.vertical_mps * (1.0 + r.jitter_margin) * dt).min(cap_v);

        let up_down = (ctx.to.y - ctx.held.y).abs();
        // How far across the floor the move WALKED (a link's jump itself is free), or the reason
        // it is not a move anyone could make.
        let walked = match ctx.declared {
            Some(MoveDecl::Link { zone, from, to, from_at, to_at }) => {
                let ends = if zone == crate::ship::ship_structure::HOME_ZONE_ID {
                    // The player's own home: the relay has no copy of it, so both ends must be on
                    // their own plot.
                    match &ctx.own_plot {
                        Some(b) if on_plot(b, *from_at) && on_plot(b, *to_at) => Some((*from_at, *to_at, pad_reach_m())),
                        _ => return self.correct(now_s, r.correction_gap_s, "link_off_plot"),
                    }
                } else {
                    // A shared zone: a link of this relay's own ship file, by its ids; its ends
                    // are the relay's, whatever the update said.
                    match ctx.links.iter().find(|l| &l.zone == zone && &l.from == from && &l.to == to) {
                        Some(l) => Some((l.from_at, l.to_at, l.reach_m)),
                        None => return self.correct(now_s, r.correction_gap_s, "link_unknown"),
                    }
                };
                let (a, b, reach) = ends.expect("set above");
                (floor_distance(ctx.held, a) + floor_distance(b, ctx.to) - 2.0 * reach).max(0.0)
            }
            Some(MoveDecl::Editor) => {
                let far = floor_distance(ctx.held, ctx.to);
                return match &ctx.own_plot {
                    Some(b) if on_plot(b, ctx.to) && far <= FAR_FROM_HELD_M + r.slack_m => {
                        self.grant_m = 0.0;
                        Verdict::Accept
                    }
                    Some(b) if on_plot(b, ctx.to) => self.correct(now_s, r.correction_gap_s, "editor_too_far"),
                    _ => self.correct(now_s, r.correction_gap_s, "editor_off_plot"),
                };
            }
            _ => floor_distance(ctx.held, ctx.to),
        };
        let fits_h = walked <= self.bank_h + self.grant_m + r.slack_m;
        let fits_v = up_down <= self.bank_v + r.slack_m;
        if !(fits_h && fits_v) {
            return self.correct(now_s, r.correction_gap_s, "too_fast");
        }
        // Spend: the grant first (it is one-shot either way), then the allowance, which the slack
        // may take below zero but no further: the slack forgives one update's rounding, it is not
        // free distance on every update (at 15 a second, 1 m each would be 15 m/s more).
        let over_grant = (walked - self.grant_m).max(0.0);
        self.grant_m = 0.0;
        self.bank_h = (self.bank_h - over_grant).max(-r.slack_m);
        self.bank_v = (self.bank_v - up_down).max(-r.slack_m);
        Verdict::Accept
    }

    /// A move nobody could make: a new correction, unless one went out less than
    /// `correction_gap_s` ago (then the update is dropped, and the next one judged afresh).
    fn correct(&mut self, now_s: f64, gap_s: f32, reason: &'static str) -> Verdict {
        if now_s - self.last_correction_s < f64::from(gap_s) {
            return Verdict::Drop;
        }
        self.seq += 1;
        self.last_correction_s = now_s;
        self.last_reason = reason;
        Verdict::Correct { reason }
    }
}

/// The plain sentence a game shows with a correction, by its reason.
pub fn correction_sentence(reason: &str) -> &'static str {
    match reason {
        "link_unknown" => "The server put you back where it last saw you: that teleporter is not on this server's ship.",
        "link_off_plot" => "The server put you back where it last saw you: a teleporter in your home has to stand on your own plot.",
        "editor_off_plot" => "The server put you back where it last saw you: your build spot is not on your own plot, so walk there.",
        "editor_too_far" => "The server put you back where it last saw you: your build spot is more than 90 m away, so walk there.",
        _ => "The server put you back where it last saw you: that move was faster than anyone can go aboard.",
    }
}

/// The box of the plot the relay holds for a player, from their entity (`home_plot`, set on every
/// join by ship_world.rs `set_home_plot`). None for a guest.
pub fn own_plot_of(e: &super::game_state::GameEntity) -> Option<Aabb> {
    let hp = e.components.get("home_plot")?;
    let v3 = |k: &str| -> Option<Vec3> {
        let a = hp.get(k)?.as_array()?;
        Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
    };
    let o = v3("origin")?;
    Some((o, o + v3("size")?))
}

impl super::game_state::GameWorld {
    /// A player was just spawned (a first join, Respawn, stepping back in) where the relay put
    /// them: a fresh allowance. Or they reconnected and the relay still held them (`rejoin`): one
    /// move as far as they could have gone while away. handle_game_join calls it on every join.
    pub fn moves_on_join(&mut self, player_id: u64, rejoin: bool) {
        let now = relay_now_s();
        let rules = self.rules.moving.clone();
        match self.moves.get_mut(&player_id) {
            Some(m) if rejoin => m.rejoin(now, &rules),
            _ => {
                self.moves.insert(player_id, MoveState::fresh(now, &rules));
            }
        }
    }

    /// Judge one `game_position_update` of `player_id` to `to` (`raw` is the whole message, for
    /// its `moved` and `correction`). Returns what to do, and for a correction (or its resend) the
    /// message to send the player. The player's position is NOT changed here.
    pub fn judge_move(&mut self, player_id: u64, to: [f32; 3], raw: &serde_json::Value) -> (Verdict, Option<serde_json::Value>) {
        let Some(e) = self.entities.get(&player_id) else { return (Verdict::Drop, None) };
        let held = Vec3::from(e.position);
        let own_plot = own_plot_of(e);
        let declared = raw.get("moved").and_then(MoveDecl::from_json);
        let applied = raw.get("correction").and_then(|v| v.as_u64()).unwrap_or(0);
        let vehicle_mps = match &declared {
            Some(MoveDecl::Vehicle { vehicle }) => self.vehicle_speeds.get(vehicle).copied(),
            _ => None,
        };
        let now = relay_now_s();
        let rules = self.rules.moving.clone();
        let links = std::mem::take(&mut self.transit);
        let ctx = MoveContext { rules: &rules, held, to: Vec3::from(to), declared: declared.as_ref(), applied, own_plot, links: &links, vehicle_mps };
        let state = self.moves.entry(player_id).or_insert_with(|| MoveState::fresh(now, &rules));
        let verdict = state.check(now, &ctx);
        let (seq, last_reason) = (state.seq, state.last_reason);
        self.transit = links;
        let reason = match &verdict {
            Verdict::Correct { reason } => *reason,
            Verdict::Resend => last_reason,
            _ => return (verdict, None),
        };
        let msg = serde_json::json!({
            "type": "game_position_correction",
            "player_id": player_id,
            "position": [held.x, held.y, held.z],
            "seq": seq,
            "reason": reason,
            "message": correction_sentence(reason),
        });
        (verdict, Some(msg))
    }
}

/// Each vehicle's own speed by its item id, from data/vehicles/kits.ron on disk (no copy is built
/// in: no shipped vehicle is faster than on foot, so a relay without the file loses nothing).
pub fn vehicle_speeds(data_dir: &std::path::Path) -> std::collections::HashMap<String, f32> {
    let path = data_dir.join("vehicles").join("kits.ron");
    let kits = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| ron::from_str::<Vec<crate::systems::vehicles::VehicleKitDef>>(&t).map_err(|e| e.to_string()));
    match kits {
        Ok(kits) => kits.into_iter().filter(|k| k.speed_mps.is_finite() && k.speed_mps > 0.0).map(|k| (k.vehicle_item, k.speed_mps)).collect(),
        Err(e) => {
            tracing::warn!("Game: {} did not load ({e}); a move declared as driving goes at walking speed", path.display());
            Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::handlers::game_state::GameWorld;
    use crate::ship::moves::SharedWorldRules;

    /// The shipped rules: 25 m/s on foot with a quarter's margin (the allowance builds at
    /// 31.25 m/s and keeps at most 1.5 s of it, 46.9 m), 20 m/s up or down, 1 m of slack.
    fn rules() -> MovingRules {
        SharedWorldRules::parse(include_str!("../../../data/ship/shared_world.ron")).expect("the shipped rules").moving
    }

    fn ctx<'a>(r: &'a MovingRules, held: Vec3, to: Vec3) -> MoveContext<'a> {
        MoveContext { rules: r, held, to, declared: None, applied: 0, own_plot: None, links: &[], vehicle_mps: None }
    }

    /// Walk `n` updates `dt` apart (relay seconds, from `t0`), each `step` further on, judging
    /// each and holding the player where an accepted one put them (as the handler does). Returns
    /// the verdicts and where the relay holds them at the end.
    fn walk(s: &mut MoveState, r: &MovingRules, from: Vec3, step: Vec3, n: usize, t0: f64, dt: f64) -> (Vec<Verdict>, Vec3) {
        let mut held = from;
        let mut out = Vec::new();
        for i in 1..=n {
            let to = held + step;
            let v = s.check(t0 + dt * i as f64, &ctx(r, held, to));
            if v == Verdict::Accept {
                held = to;
            }
            out.push(v);
        }
        (out, held)
    }

    /// The box of plot p1 (data/blueprints/ship_structure.ron).
    fn p1() -> Aabb {
        (Vec3::ZERO, Vec3::new(55.0, 3.0, 89.0))
    }

    /// EVERY HONEST WALK IS TAKEN: a sprint at 15 updates a second for 20 s, the fastest walk the
    /// data allows (24.1 m/s) for 10 s, a second of updates held up by the network and then
    /// delivered all at once, a ladder climbed at 3 m/s and a fall from 8 m. Seen red 2026-10-04
    /// with the allowance kept for no time at all (`caps` times 0, so each update had only the
    /// slack): "the fastest legitimate walk was corrected".
    #[test]
    fn every_honest_walk_is_taken() {
        let r = rules();
        let start = Vec3::new(53.5, 1.7, 40.5);
        let mut s = MoveState::fresh(0.0, &r);
        let (v, at) = walk(&mut s, &r, start, Vec3::new(0.0, 0.0, 9.5 / 15.0), 300, 0.0, 1.0 / 15.0);
        assert!(v.iter().all(|v| *v == Verdict::Accept), "a sprint was corrected: {:?}", v.iter().position(|v| *v != Verdict::Accept));
        let (v, at) = walk(&mut s, &r, at, Vec3::new(24.1 / 15.0, 0.0, 0.0), 150, 20.0, 1.0 / 15.0);
        assert!(v.iter().all(|v| *v == Verdict::Accept), "the fastest legitimate walk was corrected");
        // A second with nothing arriving, then the second's fifteen updates at the same instant.
        let (v, at) = walk(&mut s, &r, at, Vec3::new(0.0, 0.0, 24.1 / 15.0), 15, 31.0, 0.0);
        assert!(v.iter().all(|v| *v == Verdict::Accept), "updates bunched by the network were corrected: {v:?}");
        let (v, at) = walk(&mut s, &r, at, Vec3::new(0.0, 3.0 / 15.0, 0.0), 30, 32.0, 1.0 / 15.0);
        assert!(v.iter().all(|v| *v == Verdict::Accept), "a ladder climb was corrected");
        let fall = s.check(35.0, &ctx(&r, at, at - Vec3::new(0.0, 8.0, 0.0)));
        assert_eq!(fall, Verdict::Accept, "a fall from 8 m in one update after a pause");
    }

    /// AN OVERSIZED JUMP IS CORRECTED, NEVER LEFT FROZEN: 60 m in one update (past the 46.9 m a
    /// player can bank) is answered with a correction; the updates already on their way from
    /// there are dropped quietly; the correction is sent again when the game has still not
    /// applied it 2 s later; and once the game says it stands where it was corrected to, its
    /// next step is taken.
    #[test]
    fn an_oversized_jump_is_corrected_never_frozen() {
        let r = rules();
        let held = Vec3::new(76.0, 1.7, 64.0);
        let far = held + Vec3::new(0.0, 0.0, 60.0);
        let mut s = MoveState::fresh(0.0, &r);
        let v = s.check(3.0, &ctx(&r, held, far));
        assert_eq!(v, Verdict::Correct { reason: "too_fast" }, "an oversized jump was answered with {v:?}");
        assert_eq!(s.seq, 1);
        for i in 1..10 {
            let stale = s.check(3.0 + 0.066 * f64::from(i), &ctx(&r, held, far + Vec3::new(0.0, 0.0, 0.1 * i as f32)));
            assert_eq!(stale, Verdict::Drop, "an update sent before the correction landed");
        }
        assert_eq!(s.check(5.1, &ctx(&r, held, far)), Verdict::Resend, "still not applied after 2 s: sent again");
        let mut back = ctx(&r, held, held + Vec3::new(0.0, 0.0, 0.6));
        back.applied = 1;
        assert_eq!(s.check(5.2, &back), Verdict::Accept, "the next step after the correction was taken");
        // Another oversized jump straight away waits out the gap, then is corrected too.
        let mut again = ctx(&r, held, far);
        again.applied = 1;
        assert_eq!(s.check(5.25, &again), Verdict::Drop, "inside the 0.5 s gap");
        assert_eq!(s.check(5.8, &again), Verdict::Correct { reason: "too_fast" });
        assert_eq!(s.seq, 2);
        // Straight up is checked too: 40 m at once.
        let mut t = MoveState::fresh(0.0, &r);
        assert_eq!(t.check(3.0, &ctx(&r, held, held + Vec3::new(0.0, 40.0, 0.0))), Verdict::Correct { reason: "too_fast" });
    }

    /// A TELEPORTER IS TAKEN BY ITS IDS: in a shared zone, a jump declared through a link of the
    /// relay's own ship, by its ids, from its entry pad, lands however far the pads are apart; a
    /// link the ship does not have, or a jump from nowhere near the pad, is corrected. In the
    /// player's own home (which the relay has no copy of), both ends must stand on their own
    /// plot.
    #[test]
    fn a_teleporter_is_taken_by_its_ids() {
        let r = rules();
        let link = TransitLink {
            zone: "commons".into(),
            from: "teleporter-1".into(),
            to: "teleporter-2".into(),
            from_at: Vec3::new(70.0, 0.0, 30.0),
            to_at: Vec3::new(90.0, 0.0, 70.0),
            from_yaw: 0.0,
            reach_m: 0.75,
        };
        let links = [link.clone()];
        let held = Vec3::new(70.2, 1.7, 30.1);
        let landed = Vec3::new(90.1, 1.7, 70.0);
        let decl = link.declaration();
        let judge = |held: Vec3, to: Vec3, d: &MoveDecl, plot: Option<Aabb>, bank: f32| {
            let mut s = MoveState::fresh(0.0, &r);
            s.bank_h = bank;
            s.check(0.0, &MoveContext { rules: &r, held, to, declared: Some(d), applied: 0, own_plot: plot, links: &links, vehicle_mps: None })
        };
        let v = judge(held, landed, &decl, None, 0.0);
        assert_eq!(v, Verdict::Accept, "a {:.1} m jump through the Commons' teleporter was corrected: {v:?}", held.distance(landed));
        let unknown = MoveDecl::Link { zone: "commons".into(), from: "teleporter-1".into(), to: "teleporter-9".into(), from_at: link.from_at, to_at: link.to_at };
        assert_eq!(judge(held, landed, &unknown, None, 46.0), Verdict::Correct { reason: "link_unknown" });
        let far_from_pad = Vec3::new(20.0, 1.7, 30.0);
        assert_eq!(judge(far_from_pad, landed, &decl, None, 46.0), Verdict::Correct { reason: "too_fast" }, "a jump from 50 m off the pad");
        // The player's own home: the ends must be on their own plot.
        let home = MoveDecl::Link { zone: "home".into(), from: "teleporter-1".into(), to: "teleporter-2".into(), from_at: Vec3::new(22.5, 0.0, 20.0), to_at: Vec3::new(31.0, 0.0, 80.0) };
        assert_eq!(judge(Vec3::new(22.5, 1.7, 20.2), Vec3::new(31.0, 1.7, 80.0), &home, Some(p1()), 0.0), Verdict::Accept);
        let off_plot = MoveDecl::Link { zone: "home".into(), from: "teleporter-1".into(), to: "teleporter-2".into(), from_at: Vec3::new(22.5, 0.0, 20.0), to_at: Vec3::new(80.0, 0.0, 60.0) };
        assert_eq!(judge(Vec3::new(22.5, 1.7, 20.2), Vec3::new(80.0, 1.7, 60.0), &off_plot, Some(p1()), 46.0), Verdict::Correct { reason: "link_off_plot" });
        assert_eq!(judge(Vec3::new(22.5, 1.7, 20.2), Vec3::new(31.0, 1.7, 80.0), &home, None, 46.0), Verdict::Correct { reason: "link_off_plot" }, "a guest has no home to jump in");
    }

    /// SHUTTING THE BUILD EDITOR LANDS ONLY ON YOUR OWN PLOT, at most 90 m from where the relay
    /// holds you, however little allowance is left.
    #[test]
    fn shutting_the_editor_lands_only_on_your_own_plot() {
        let r = rules();
        let judge = |held: Vec3, to: Vec3| {
            let mut s = MoveState::fresh(0.0, &r);
            s.bank_h = 0.0;
            s.check(0.0, &MoveContext { rules: &r, held, to, declared: Some(&MoveDecl::Editor), applied: 0, own_plot: Some(p1()), links: &[], vehicle_mps: None })
        };
        let corridor = Vec3::new(60.0, 1.7, 45.0);
        let door = Vec3::new(53.5, 1.7, 40.5);
        let deep = Vec3::new(40.0, 1.7, 30.0);
        let v = judge(corridor, deep);
        assert_eq!(v, Verdict::Accept, "shutting the editor {:.1} m from the corridor was corrected", corridor.distance(deep));
        assert_eq!(judge(corridor, door), Verdict::Accept);
        assert_eq!(judge(corridor, Vec3::new(80.0, 1.7, 60.0)), Verdict::Correct { reason: "editor_off_plot" }, "a build spot in the Commons");
        assert_eq!(judge(Vec3::new(70.0, 1.7, 190.0), door), Verdict::Correct { reason: "editor_too_far" }, "from the far end of First Street");
    }

    /// A VEHICLE GOES AT ITS OWN SPEED when that is faster than on foot (a 40 m/s one here; no
    /// shipped vehicle is), only while declared, and only a vehicle the relay knows. Ten seconds:
    /// on foot the banked allowance covers 40 m/s for about five. Seen red 2026-10-04 with the
    /// declared vehicle's speed ignored: "ten seconds at the car's 40 m/s".
    #[test]
    fn a_vehicle_goes_at_its_own_speed() {
        let r = rules();
        let car = MoveDecl::Vehicle { vehicle: "fast_car".into() };
        let drive = |declared: Option<&MoveDecl>, mps: Option<f32>| {
            let mut s = MoveState::fresh(0.0, &r);
            let mut held = Vec3::new(70.0, 1.7, 90.0);
            let mut verdicts = Vec::new();
            for i in 1..=150 {
                let to = held + Vec3::new(0.0, 0.0, 40.0 / 15.0);
                let v = s.check(f64::from(i) / 15.0, &MoveContext { rules: &r, held, to, declared, applied: 0, own_plot: None, links: &[], vehicle_mps: mps });
                if v == Verdict::Accept {
                    held = to;
                }
                verdicts.push(v);
            }
            verdicts
        };
        assert!(drive(Some(&car), Some(40.0)).iter().all(|v| *v == Verdict::Accept), "ten seconds at the car's 40 m/s");
        assert!(drive(None, None).iter().any(|v| *v != Verdict::Accept), "40 m/s on foot is corrected");
        assert!(drive(Some(&car), None).iter().any(|v| *v != Verdict::Accept), "an unknown vehicle goes at walking speed");
    }

    /// A RECONNECT MAY MOVE AS FAR AS THE TIME AWAY ALLOWS (at most 90 m), once; and it forgives a
    /// correction the game never got. Seen red 2026-10-04 with `rejoin` granting nothing: "after
    /// 5 s away, an 80 m first move was corrected".
    #[test]
    fn a_reconnect_may_move_as_far_as_the_time_away_allows() {
        let r = rules();
        let held = Vec3::new(70.0, 1.7, 100.0);
        let there = held + Vec3::new(0.0, 0.0, 80.0);
        let mut quick = MoveState::fresh(0.0, &r);
        quick.rejoin(1.0, &r);
        assert_eq!(quick.check(1.0, &ctx(&r, held, there)), Verdict::Correct { reason: "too_fast" }, "1 s away is not 80 m");
        let mut away = MoveState::fresh(0.0, &r);
        away.rejoin(5.0, &r);
        assert_eq!(away.check(5.0, &ctx(&r, held, there)), Verdict::Accept, "after 5 s away, an 80 m first move was corrected");
        assert_eq!(away.check(5.1, &ctx(&r, there, there + Vec3::new(0.0, 0.0, 80.0))), Verdict::Correct { reason: "too_fast" }, "the grant is spent");
        let mut pending = MoveState::fresh(0.0, &r);
        assert!(matches!(pending.check(1.0, &ctx(&r, held, there)), Verdict::Correct { .. }));
        pending.rejoin(30.0, &r);
        assert_eq!(pending.check(30.0, &ctx(&r, held, held + Vec3::new(0.5, 0.0, 0.0))), Verdict::Accept, "the welcome forgave the correction");
    }

    /// THE RELAY'S HALF ON A WORLD: a player spawned at their door is held there; a 60 m jump is
    /// answered with a correction to the door; the update that says it applied it is taken. Seen
    /// red 2026-10-04 with `judge_move` sending no message for a correction: "a correction
    /// message".
    #[test]
    fn a_world_answers_a_jump_with_where_it_holds_the_player() {
        let mut world = GameWorld::new();
        let door = [53.5, 1.7, 40.5];
        let id = world.spawn_player("e11e0004", door);
        world.moves_on_join(id, false);
        let (v, msg) = world.judge_move(id, [door[0], door[1], door[2] + 60.0], &serde_json::json!({}));
        assert_eq!(v, Verdict::Correct { reason: "too_fast" });
        let msg = msg.expect("a correction message");
        assert_eq!(msg["type"], "game_position_correction");
        assert_eq!(msg["position"], serde_json::json!(door), "it says where the relay holds them");
        assert_eq!(msg["seq"], 1);
        assert!(msg["message"].as_str().is_some_and(|m| !m.is_empty()));
        let (v, msg) = world.judge_move(id, [door[0], door[1], door[2] + 0.5], &serde_json::json!({ "correction": 1 }));
        assert_eq!((v, msg), (Verdict::Accept, None));
    }

    /// THE RELAY'S SHIP HAS ITS LINKS BY ID: today's ship has no teleporter aboard, so no jump in
    /// a shared zone is a link; a ship file with a teleporter pair in the Commons, named by ids,
    /// gives the relay both ways through it. Seen red 2026-10-04 with `transit_link` matching the
    /// zone and the entry pad only: "assertion failed: ship.transit_link(\"commons\",
    /// \"tp-west\", \"tp-north\").is_none()".
    #[test]
    fn the_relays_links_resolve_by_id() {
        assert!(GameWorld::new().transit.is_empty(), "the shipped ship has no teleporter in a shared zone");
        let mut ship = crate::ship::ship_structure::ShipStructure::load_ship_file(std::path::Path::new("data")).expect("the ship file");
        let commons = ship.zones.iter_mut().find(|z| z.id == "commons").expect("the Commons");
        let pad = |id: &str, pair: &str, x: f32, z: f32| crate::ship::home_structure::PlacedStructure { id: id.into(), type_id: "teleporter".into(), pos: (x, 0.0, z), rot_deg: 0.0, pair: Some(pair.into()) };
        commons.body.structures = vec![pad("tp-west", "tp-east", 3.0, 30.0), pad("tp-east", "tp-west", 30.0, 30.0)];
        let links = ship.transit_links();
        assert_eq!(links.len(), 2);
        let l = ship.transit_link("commons", "tp-west", "tp-east").expect("the link by its ids");
        assert_eq!(l.from_at, Vec3::new(68.0, 0.0, 50.0), "in ship metres: the Commons stands at (65, 0, 20)");
        assert!(ship.transit_link("commons", "tp-west", "tp-north").is_none());
    }

    /// AN OLD STORED WORLD LOADS UNDER THE SPEED CHECK: a database holding the world the previous
    /// code stored (increment 3's `game_world_snapshot_v10`) opens, its world restores with its
    /// players reaped as ghosts as always (their progress kept), and a player who joins afterwards
    /// starts with a fresh allowance at their door: a step is taken and an oversized jump is
    /// corrected. tests/fixtures/relay/ship_world_v10.json is EXACTLY what the previous code
    /// wrote, unedited: produced 2026-10-04 on 67a47bc65 (v0.1456.0) by `GameWorld::new()`, a
    /// player "e11e0002" spawned at p2's door with p2 set as their plot and moved to
    /// (70, 1.7, 120), 30 s of ticks, then `save_to_db`, and the stored blob copied out.
    #[test]
    fn an_old_stored_world_loads_under_the_speed_check() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_movecheck_v10_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");
        let blob = include_str!("../../../tests/fixtures/relay/ship_world_v10.json");
        let old: serde_json::Value = serde_json::from_str(blob).expect("the fixture parses");
        db.save_game_world(GameWorld::PERSIST_KEY, blob, old["game_time"].as_f64().unwrap(), old["next_entity_id"].as_u64().unwrap()).expect("store it");
        let mut world = GameWorld::new();
        assert!(world.restore_from_db(&db), "the stored world restores");
        assert_eq!(world.player_count(), 0, "its player was reaped as a ghost");
        assert!(db.load_player_progress("e11e0002").expect("query").is_some(), "and kept their progress");
        assert!(world.moves.is_empty(), "nothing of the speed check is stored");
        let door = [53.5, 1.7, 139.5];
        let id = world.spawn_player("e11e0002", door);
        world.moves_on_join(id, false);
        assert_eq!(world.judge_move(id, [53.5, 1.7, 140.0], &serde_json::json!({})).0, Verdict::Accept);
        world.update_position(id, [53.5, 1.7, 140.0], [0.0, 0.0, 0.0, 1.0]);
        assert!(matches!(world.judge_move(id, [53.5, 1.7, 40.0], &serde_json::json!({})).0, Verdict::Correct { .. }));
        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
