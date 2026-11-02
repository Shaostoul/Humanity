//! Game events, folded (rung 3, 2026-11-01; docs/design/notifications.md section 11): what the
//! simulation raises through `systems::push_game_event` becomes at most one notice of each kind of
//! event per its interval, however many times it was raised.
//!
//! The first event of a kind opens a gather of [`GATHER_SECS`]; at its end ONE notice carries how
//! many there were and the first place ("12 crops are ripe, the first in Tower 1"). After it,
//! nothing more of that kind for the event's `every_secs` (data); what arrives meanwhile is counted
//! into the next notice, which goes out once the interval is over. So a farm of 2,000 crops ripening
//! in one tick is one notice, and a crop ripening every minute is one notice every ten. At most
//! [`KINDS_KEPT`] kinds wait at once; past that, a new kind's events are dropped (the file has five).
//!
//! Pure: the caller gives the time. Nothing here is saved or networked: an event is this device's.

use std::collections::BTreeMap;

/// How long the first event of a kind waits for the rest of its batch, in seconds.
pub const GATHER_SECS: f64 = 2.0;

/// How many kinds of event may wait at once.
pub const KINDS_KEPT: usize = 16;

/// One kind's state.
#[derive(Debug, Clone, Default, PartialEq)]
struct Waiting {
    /// Events since the last notice.
    count: u32,
    /// Where the first of them happened.
    first_where: String,
    /// When the first of them arrived.
    first_at: f64,
    /// When this kind's last notice went out.
    last_sent: Option<f64>,
}

/// One notice's worth of events.
#[derive(Debug, Clone, PartialEq)]
pub struct Folded {
    pub event: String,
    /// How many events it stands for (at least 1).
    pub count: u32,
    /// Where the first happened (a grow area's name; empty when the event has no place).
    pub first_where: String,
}

/// The events waiting to become notices.
#[derive(Debug, Default)]
pub struct Folder {
    kinds: BTreeMap<String, Waiting>,
}

impl Folder {
    /// An event `event` happened at `place`, at `now` (seconds).
    pub fn offer(&mut self, event: &str, place: &str, now: f64) {
        if !self.kinds.contains_key(event) && self.kinds.len() >= KINDS_KEPT {
            return;
        }
        let w = self.kinds.entry(event.to_string()).or_default();
        if w.count == 0 {
            w.first_where = place.to_string();
            w.first_at = now;
        }
        w.count = w.count.saturating_add(1);
    }

    /// Anything waiting (one map check a frame when nothing is).
    pub fn is_idle(&self) -> bool {
        self.kinds.values().all(|w| w.count == 0)
    }

    /// The notices due at `now`: each kind whose gather is over and whose interval
    /// (`every_secs(event)`) since its last notice has passed. Their counts start again.
    pub fn due(&mut self, now: f64, every_secs: impl Fn(&str) -> f64) -> Vec<Folded> {
        let mut out = Vec::new();
        for (event, w) in self.kinds.iter_mut() {
            if w.count == 0 || now - w.first_at < GATHER_SECS {
                continue;
            }
            if w.last_sent.is_some_and(|t| now - t < every_secs(event)) {
                continue;
            }
            out.push(Folded { event: event.clone(), count: w.count, first_where: std::mem::take(&mut w.first_where) });
            w.count = 0;
            w.last_sent = Some(now);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every(_: &str) -> f64 {
        600.0
    }

    /// A farm of 2,000 crops ripening in one tick is ONE notice, after the gather, with the count
    /// and the first place; more in the next ten minutes wait and come as one notice once they are
    /// over; another kind keeps its own time. Seen red 2026-11-01 with `due` ignoring the interval:
    /// "the next ten minutes" got a second notice at once.
    #[test]
    fn a_farm_of_2000_ripe_crops_is_one_notice() {
        let mut f = Folder::default();
        assert!(f.is_idle());
        for i in 0..2000 {
            f.offer("crop_ripe", if i == 0 { "Tower 1" } else { "Tower 2" }, 10.0 + i as f64 * 0.0005);
        }
        assert!(f.due(11.0, every).is_empty(), "still gathering");
        let n = f.due(12.5, every);
        assert_eq!(n, vec![Folded { event: "crop_ripe".into(), count: 2000, first_where: "Tower 1".into() }]);
        assert!(f.is_idle());
        f.offer("crop_ripe", "Bed 3", 100.0);
        f.offer("crop_ripe", "Bed 4", 300.0);
        assert!(f.due(400.0, every).is_empty(), "the next ten minutes");
        f.offer("short_of_air", "", 400.0);
        assert_eq!(f.due(403.0, every).len(), 1, "another kind keeps its own time");
        let late = f.due(613.0, every);
        assert_eq!(late, vec![Folded { event: "crop_ripe".into(), count: 2, first_where: "Bed 3".into() }], "folded into one");
    }

    /// At most `KINDS_KEPT` kinds wait at once: a flood of made-up event ids cannot grow the map.
    #[test]
    fn the_kinds_waiting_are_capped() {
        let mut f = Folder::default();
        for i in 0..100 {
            f.offer(&format!("e{i}"), "", 0.0);
        }
        assert_eq!(f.due(5.0, every).len(), KINDS_KEPT);
    }
}
