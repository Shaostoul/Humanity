//! Mutes: everything, a server, one of a server's rooms, or a conversation with a person, silenced
//! for a while or until the person unmutes it (docs/design/notifications.md, section 5). Kept on
//! this device.
//!
//! A mute is a time: the Unix second it ends, or 0 for "until I unmute". An ended mute is the same
//! as none, and `prune` drops it so the saved list stays short. A server is its normalised address
//! (`chat::norm_server_url`), a room is `<server>|<room id>`, so the same room id on two servers is
//! two rooms. Muting a server mutes its rooms' notices, never a direct message: a DM is a
//! conversation with a person. A conversation (rung 2) is its Chat id, `dm:<their key>`, on every
//! server, because it is the same person wherever their message comes through; it stops their
//! direct messages and their contact requests, never a mention of you in a room.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The mutes in force.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mutes {
    /// Everything (the whole app's notices, DMs included) muted until this second, 0 for until
    /// unmuted, `None` when not muted. Settings' Mute everything, and the tray's Mute (rung 2).
    pub everything: Option<i64>,
    /// Server address -> until.
    pub servers: BTreeMap<String, i64>,
    /// `<server>|<room id>` -> until.
    pub rooms: BTreeMap<String, i64>,
    /// A conversation with a person, `dm:<their key>` -> until.
    pub people: BTreeMap<String, i64>,
}

/// Is a mute that ends at `until` (0: never) still on at `now`?
pub fn active(until: i64, now: i64) -> bool {
    until == 0 || until > now
}

/// When a mute begun at `now` for `seconds` ends (0 seconds: until unmuted).
pub fn ends(now: i64, seconds: u64) -> i64 {
    if seconds == 0 {
        0
    } else {
        now.saturating_add(seconds as i64)
    }
}

impl Mutes {
    /// The key of `room` on `server`.
    pub fn room_key(server: &str, room: &str) -> String {
        format!("{server}|{room}")
    }

    /// Until when everything is muted, if it is at `now`.
    pub fn everything_muted(&self, now: i64) -> Option<i64> {
        self.everything.filter(|&u| active(u, now))
    }

    /// Until when `server` is muted, if it is at `now`.
    pub fn server_muted(&self, server: &str, now: i64) -> Option<i64> {
        self.servers.get(server).copied().filter(|&u| active(u, now))
    }

    /// Until when `room` on `server` is muted, if it is at `now`.
    pub fn room_muted(&self, server: &str, room: &str, now: i64) -> Option<i64> {
        self.rooms.get(&Self::room_key(server, room)).copied().filter(|&u| active(u, now))
    }

    pub fn mute_everything(&mut self, now: i64, seconds: u64) {
        self.everything = Some(ends(now, seconds));
    }

    pub fn unmute_everything(&mut self) {
        self.everything = None;
    }

    pub fn mute_server(&mut self, server: &str, now: i64, seconds: u64) {
        self.servers.insert(server.to_string(), ends(now, seconds));
    }

    pub fn unmute_server(&mut self, server: &str) {
        self.servers.remove(server);
    }

    pub fn mute_room(&mut self, server: &str, room: &str, now: i64, seconds: u64) {
        self.rooms.insert(Self::room_key(server, room), ends(now, seconds));
    }

    pub fn unmute_room(&mut self, server: &str, room: &str) {
        self.rooms.remove(&Self::room_key(server, room));
    }

    /// Until when the conversation `conversation` (`dm:<key>`) is muted, if it is at `now`.
    pub fn person_muted(&self, conversation: &str, now: i64) -> Option<i64> {
        self.people.get(conversation).copied().filter(|&u| active(u, now))
    }

    pub fn mute_person(&mut self, conversation: &str, now: i64, seconds: u64) {
        self.people.insert(conversation.to_string(), ends(now, seconds));
    }

    pub fn unmute_person(&mut self, conversation: &str) {
        self.people.remove(conversation);
    }

    /// Drop every mute that has ended by `now`. True when anything was dropped (save then).
    pub fn prune(&mut self, now: i64) -> bool {
        let count = |m: &Mutes| (m.everything.is_some(), m.servers.len(), m.rooms.len(), m.people.len());
        let before = count(self);
        if self.everything.is_some_and(|u| !active(u, now)) {
            self.everything = None;
        }
        self.servers.retain(|_, u| active(*u, now));
        self.rooms.retain(|_, u| active(*u, now));
        self.people.retain(|_, u| active(*u, now));
        before != count(self)
    }

    /// Nothing muted at all (ended mutes count as none).
    pub fn is_empty_at(&self, now: i64) -> bool {
        self.everything_muted(now).is_none()
            && !self.servers.values().any(|&u| active(u, now))
            && !self.rooms.values().any(|&u| active(u, now))
            && !self.people.values().any(|&u| active(u, now))
    }
}
