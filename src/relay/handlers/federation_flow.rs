//! How fast each linked server may send, and what happens when it sends faster (2026-10-22,
//! docs/design/federation-activation.md Phase 2).
//!
//! Before this, a linked server was limited on two kinds of frame only (chat at 10 a second, and
//! signed objects at 50), and only by DROPPING what came over the limit, so a burst lost
//! messages; profile copies had no limit at all (the one clear flood a trusted but compromised
//! peer had, flagged in docs/INCIDENT-PLAYBOOK.md). Now every kind a peer sends has an allowance
//! ([`Kind`], [`FlowLimits`]), and going over it costs time, never messages:
//!
//! 1. SLOWED. A frame over its kind's allowance waits for its turn (the read pump sleeps, so the
//!    peer's socket backs up and its sending slows: TCP does the rest). Nothing is dropped.
//! 2. PAUSED. A peer that has spent more than [`FlowLimits::pause_after`] of the last minute being
//!    slowed is not having a burst, it is flooding. It is paused: for a while (5 s, then 5, 10,
//!    15, 25, 40, 65, 105 s by the Fibonacci delays the chat limiter uses, each pause in a row
//!    longer) what it sends is read and set aside unprocessed.
//! 3. RESUMED. When the pause ends this server asks the peer for everything after the last frame
//!    it applied (handlers/federation_log.rs): every numbered frame set aside comes again, once.
//!    So a pause loses nothing and repeats nothing. Ten minutes without a pause and the next one
//!    starts from 5 s again.
//!
//! What this server ASKED for (the frames answering its own request to catch up) is paced by its
//! asking, one batch at a time, so it is never counted as a flood. Everything here is per peer,
//! in memory, and shown to the admins (`snapshot`, in the admin API's federation rows).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::relay::relay::FIB_DELAYS;

/// The kinds of frame a linked server sends, each with its own allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A chat line in a shared room.
    Chat,
    /// An edit of one.
    Edit,
    /// A delete of one, or an erase of an account's.
    Delete,
    /// A ban or unban.
    Moderation,
    /// A copy of someone's profile.
    Profile,
    /// A signed object (votes, vouches, group messages, ...).
    Object,
    /// A request to send again everything after a number (costs the sender a database read).
    Resume,
    /// Bookkeeping: a skipped number, a gap, the end of a batch, a welcome.
    Control,
}

impl Kind {
    pub const ALL: [Kind; 8] = [Kind::Chat, Kind::Edit, Kind::Delete, Kind::Moderation, Kind::Profile, Kind::Object, Kind::Resume, Kind::Control];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Chat => "chat",
            Kind::Edit => "edit",
            Kind::Delete => "delete",
            Kind::Moderation => "moderation",
            Kind::Profile => "profile",
            Kind::Object => "object",
            Kind::Resume => "resume",
            Kind::Control => "control",
        }
    }

    fn index(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// One kind's allowance: a burst at once, then a steady rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindLimit {
    pub per_sec: f64,
    pub burst: f64,
}

/// The allowances and the pause rules (`RelayState::fed`; tests set small ones).
#[derive(Debug, Clone, PartialEq)]
pub struct FlowLimits {
    pub kinds: [KindLimit; 8],
    /// Slowed this long within [`FlowLimits::window`] and the peer is paused.
    pub pause_after: Duration,
    pub window: Duration,
    /// The first pause; each pause in a row is this times the next Fibonacci delay.
    pub pause_unit: Duration,
    /// This long without a pause and the next one starts from the first again.
    pub calm_after: Duration,
}

impl Default for FlowLimits {
    /// Chat at 20 a second (the old cap was 10, and dropped the rest), 100 at once. Profile copies
    /// at 500 at once and 100 a second, so the catch-up a new link is sent (up to 1,000 copies,
    /// federation.rs `send_own_copies`) waits about 5 seconds in all, well short of a pause.
    fn default() -> Self {
        let l = |per_sec: f64, burst: f64| KindLimit { per_sec, burst };
        FlowLimits {
            kinds: [l(20.0, 100.0), l(10.0, 50.0), l(20.0, 100.0), l(2.0, 20.0), l(100.0, 500.0), l(50.0, 200.0), l(2.0, 10.0), l(200.0, 1_000.0)],
            pause_after: Duration::from_secs(15),
            window: Duration::from_secs(60),
            pause_unit: Duration::from_secs(5),
            calm_after: Duration::from_secs(600),
        }
    }
}

impl FlowLimits {
    pub fn of(&self, kind: Kind) -> KindLimit {
        self.kinds[kind.index()]
    }
}

/// What to do with one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Admit {
    /// Apply it now.
    Go,
    /// Wait this long, then apply it.
    Slow(Duration),
    /// Set it aside: the peer is paused.
    Paused,
}

/// One linked server's flow, as this server receives it.
#[derive(Debug, Clone)]
pub struct PeerFlow {
    tokens: [f64; 8],
    refilled: Option<Instant>,
    /// The waits taken in the last window, to tell a burst from a flood.
    slowed: VecDeque<(Instant, Duration)>,
    paused_until: Option<Instant>,
    /// Set when a pause ends, until the caller has asked the peer to send again.
    resume_due: bool,
    /// Steps along the Fibonacci delays the next pause takes.
    strikes: usize,
    last_pause: Option<Instant>,
    /// Frames this server asked for and has not received yet (they are not a flood).
    pub(crate) asked: u32,
    // Counts since this server started, for the admins.
    pub pauses: u64,
    pub set_aside: u64,
    pub applied: [u64; 8],
    pub slowed_total: Duration,
    /// Numbered frames the peer no longer had when asked (older than its outbox keeps).
    pub missed: u64,
}

impl Default for PeerFlow {
    fn default() -> Self {
        PeerFlow {
            tokens: [f64::NAN; 8],
            refilled: None,
            slowed: VecDeque::new(),
            paused_until: None,
            resume_due: false,
            strikes: 0,
            last_pause: None,
            asked: 0,
            pauses: 0,
            set_aside: 0,
            applied: [0; 8],
            slowed_total: Duration::ZERO,
            missed: 0,
        }
    }
}

impl PeerFlow {
    /// Whether the peer is paused at `now`. A pause that has ended is cleared here, and leaves
    /// `resume_due` set for the caller.
    pub fn paused(&mut self, now: Instant) -> bool {
        match self.paused_until {
            Some(until) if until > now => true,
            Some(_) => {
                self.paused_until = None;
                self.resume_due = true;
                false
            }
            None => false,
        }
    }

    /// When the pause ends, if the peer is paused.
    pub fn paused_until(&self) -> Option<Instant> {
        self.paused_until
    }

    /// Whether a pause has ended since the last call: the caller asks the peer to send again.
    pub fn take_resume_due(&mut self, now: Instant) -> bool {
        self.paused(now);
        std::mem::take(&mut self.resume_due)
    }

    /// Admit one frame of `kind` at `now`. `asked`: it answers this server's own request to catch
    /// up (paced by the asking, so it neither waits nor counts toward a pause).
    pub fn admit(&mut self, kind: Kind, asked: bool, limits: &FlowLimits, now: Instant) -> Admit {
        if self.paused(now) {
            self.set_aside += 1;
            return Admit::Paused;
        }
        if self.last_pause.is_some_and(|at| now.saturating_duration_since(at) >= limits.calm_after) {
            self.strikes = 0;
        }
        // Refill every kind's bucket for the time since the last frame.
        let since = self.refilled.map_or(Duration::ZERO, |at| now.saturating_duration_since(at)).as_secs_f64();
        self.refilled = Some(now);
        for k in Kind::ALL {
            let l = limits.of(k);
            let t = &mut self.tokens[k.index()];
            *t = if t.is_nan() { l.burst } else { (*t + since * l.per_sec).min(l.burst) };
        }
        self.applied[kind.index()] += 1;
        if asked {
            return Admit::Go;
        }
        let l = limits.of(kind);
        let t = &mut self.tokens[kind.index()];
        *t -= 1.0;
        if *t >= 0.0 {
            return Admit::Go;
        }
        // Over the allowance: wait for its turn, unless the peer has been slowed long enough to
        // be flooding.
        let wait = Duration::from_secs_f64(-*t / l.per_sec.max(0.001));
        while self.slowed.front().is_some_and(|(at, _)| now.saturating_duration_since(*at) > limits.window) {
            self.slowed.pop_front();
        }
        let slowed: Duration = self.slowed.iter().map(|(_, d)| *d).sum::<Duration>() + wait;
        if slowed > limits.pause_after {
            let pause = limits.pause_unit * FIB_DELAYS[self.strikes] as u32;
            self.strikes = (self.strikes + 1).min(FIB_DELAYS.len() - 1);
            self.paused_until = Some(now + pause);
            self.last_pause = Some(now);
            self.pauses += 1;
            self.slowed.clear();
            self.applied[kind.index()] -= 1;
            self.set_aside += 1;
            // A fresh allowance after the pause: the frames it set aside come again when asked.
            for k in Kind::ALL {
                self.tokens[k.index()] = limits.of(k).burst;
            }
            return Admit::Paused;
        }
        self.slowed.push_back((now, wait));
        self.slowed_total += wait;
        Admit::Slow(wait)
    }

    /// Undo the count of one frame `admit` counted as applied that was not (a duplicate, or one
    /// out of order that will come again).
    pub fn not_applied(&mut self, kind: Kind) {
        let n = &mut self.applied[kind.index()];
        *n = n.saturating_sub(1);
    }

    /// Pause the peer now for `how_long` (admin tests and a future admin action).
    pub fn pause_for(&mut self, now: Instant, how_long: Duration) {
        self.paused_until = Some(now + how_long);
        self.last_pause = Some(now);
        self.pauses += 1;
    }

    /// What the admin page shows for this peer.
    pub fn snapshot(&self, limits: &FlowLimits, now: Instant) -> serde_json::Value {
        let paused_for = self.paused_until.filter(|u| *u > now).map(|u| u.saturating_duration_since(now));
        let slowed_recently: Duration = self
            .slowed
            .iter()
            .filter(|(at, _)| now.saturating_duration_since(*at) <= limits.window)
            .map(|(_, d)| *d)
            .sum();
        let state = if paused_for.is_some() {
            "paused"
        } else if self.slowed.back().is_some_and(|(at, _)| now.saturating_duration_since(*at) < Duration::from_secs(10)) {
            "slowed"
        } else {
            "ok"
        };
        let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        let per_kind = |f: &dyn Fn(Kind) -> serde_json::Value| -> serde_json::Value {
            Kind::ALL.iter().map(|k| (k.key().to_string(), f(*k))).collect::<serde_json::Map<_, _>>().into()
        };
        serde_json::json!({
            "state": state,
            "paused_until": paused_for.map(|d| unix_ms + d.as_millis() as u64),
            "pauses": self.pauses,
            "strikes": self.strikes,
            "set_aside": self.set_aside,
            "missed": self.missed,
            "slowed_ms_last_minute": slowed_recently.as_millis() as u64,
            "slowed_ms_total": self.slowed_total.as_millis() as u64,
            "received": per_kind(&|k| serde_json::json!(self.applied[k.index()])),
            "limits": per_kind(&|k| serde_json::json!({ "per_sec": limits.of(k).per_sec, "burst": limits.of(k).burst })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> FlowLimits {
        FlowLimits {
            kinds: [KindLimit { per_sec: 5.0, burst: 5.0 }; 8],
            pause_after: Duration::from_secs(2),
            window: Duration::from_secs(60),
            pause_unit: Duration::from_secs(1),
            calm_after: Duration::from_secs(600),
        }
    }

    fn at(t0: Instant, secs: f64) -> Instant {
        t0 + Duration::from_secs_f64(secs)
    }

    /// A PEER THAT FLOODS IS SLOWED, THEN PAUSED, THEN ASKED TO SEND AGAIN: with 5 a second and 5
    /// at once, the first five chat lines go, the next ones wait a fifth of a second each, and
    /// once two seconds of the last minute were spent waiting the peer is paused for 1 s; what it
    /// sends meanwhile is set aside; when the pause ends, `take_resume_due` says so once. The next
    /// pause in a row is longer (1, 1, 2, 3, 5 s); ten calm minutes and it is 1 s again.
    ///
    /// Seen red 2026-10-22 with the pause branch taken out of `admit` (a flood only ever slowed):
    /// "a flood is paused: [Slow(200ms), Slow(400ms), ... Slow(4s)] / left: Some(Slow(4s)) /
    /// right: Some(Paused)".
    #[test]
    fn a_peer_that_floods_is_slowed_then_paused_then_asked_again() {
        let limits = small();
        let mut f = PeerFlow::default();
        let t0 = Instant::now();
        let first: Vec<Admit> = (0..5).map(|_| f.admit(Kind::Chat, false, &limits, t0)).collect();
        assert!(first.iter().all(|a| *a == Admit::Go), "the burst goes: {first:?}");
        let mut seen = Vec::new();
        for _ in 0..20 {
            let a = f.admit(Kind::Chat, false, &limits, t0);
            seen.push(a);
            if a == Admit::Paused {
                break;
            }
        }
        assert_eq!(seen.last(), Some(&Admit::Paused), "a flood is paused: {seen:?}");
        assert!(matches!(seen[0], Admit::Slow(d) if d == Duration::from_millis(200)), "slowed first: {seen:?}");
        assert_eq!(f.admit(Kind::Profile, false, &limits, at(t0, 0.5)), Admit::Paused, "every kind is set aside while paused");
        assert!(!f.take_resume_due(at(t0, 0.9)), "not yet");
        assert!(f.take_resume_due(at(t0, 1.1)), "the pause ended: ask the peer to send again");
        assert!(!f.take_resume_due(at(t0, 1.2)), "said once");
        assert_eq!(f.admit(Kind::Chat, false, &limits, at(t0, 1.2)), Admit::Go, "a fresh allowance after the pause");

        // Pauses in a row grow along the Fibonacci delays.
        let mut lengths = Vec::new();
        let mut now = at(t0, 1.2);
        for _ in 0..4 {
            while f.admit(Kind::Chat, false, &limits, now) != Admit::Paused {}
            let until = f.paused_until().unwrap();
            lengths.push(until.saturating_duration_since(now).as_secs());
            now = until + Duration::from_millis(1);
        }
        assert_eq!(lengths, vec![1, 2, 3, 5]);
        let calm = now + Duration::from_secs(601);
        while f.admit(Kind::Chat, false, &limits, calm) != Admit::Paused {}
        assert_eq!(f.paused_until().unwrap().saturating_duration_since(calm).as_secs(), 1, "ten calm minutes start again from the first");
        let snap = f.snapshot(&limits, calm);
        assert_eq!(snap["state"], "paused");
        assert!(snap["pauses"].as_u64().unwrap() >= 6);
    }

    /// WHAT THIS SERVER ASKED FOR IS NOT A FLOOD: 500 frames answering its own request go through
    /// without waiting and without a pause.
    #[test]
    fn what_this_server_asked_for_is_not_a_flood() {
        let limits = small();
        let mut f = PeerFlow::default();
        let t0 = Instant::now();
        for i in 0..500 {
            assert_eq!(f.admit(Kind::Chat, true, &limits, t0), Admit::Go, "frame {i}");
        }
        assert_eq!(f.snapshot(&limits, t0)["received"]["chat"], 500);
    }

    /// A BURST UNDER THE PAUSE LINE ONLY WAITS: a peer sending chat at twice the default rate for
    /// ten seconds (400 lines), to a receiver that waits when told, is slowed once its burst is
    /// spent, never paused, and every line goes.
    #[test]
    fn a_burst_under_the_pause_line_only_waits() {
        let limits = FlowLimits::default();
        let mut f = PeerFlow::default();
        let t0 = Instant::now();
        let (mut now, mut slowed) = (t0, 0);
        for i in 0..400 {
            now = now.max(at(t0, i as f64 / 40.0));
            match f.admit(Kind::Chat, false, &limits, now) {
                Admit::Go => {}
                Admit::Slow(d) => {
                    slowed += 1;
                    now += d;
                }
                Admit::Paused => panic!("paused at line {i}"),
            }
        }
        assert!(slowed > 100, "past the burst it waits: {slowed}");
        assert_eq!(f.snapshot(&limits, now)["received"]["chat"], 400);
    }
}
