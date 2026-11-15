//! Organisations, rung 2 (docs/design/organisations.md, section 3; 2026-11-15): every change an
//! organisation's log takes reaches every other member, and my own other devices, as a sealed DM,
//! with nobody copying text. The rules here have no socket, so each has a unit test
//! (org_mail_tests.rs); the half that sends and receives is src/engine/org_mail.rs, and the
//! website's twin of these rules is web/shared/org-mail.js.
//!
//! THE NOTE. `[[hum:org:v1]]` and then the carried text (`org::bundle_text`: the organisation's
//! id and whole signed objects), the text of an ordinary signed sealed-sender v2 DM, sealed only to
//! the Kyber768 key on the member's own key card, checked and pinned by this app (engine/dm.rs
//! `control_puts_at`; docs/design/key-cards.md). A server sees padded sealed mail in a member's
//! mailbox: how many, the padded size, which day; never the organisation, the sender or a word of
//! it. It travels through the member's "who can reach me" like any message: when their setting
//! keeps the sender out, the server refuses it (`dm_put_refused`, reason `reach`) and the page says
//! so, with the text to copy as the fallback. A change bigger than one note is split, each note
//! carrying whole objects; the merge takes them in any order and a repeat changes nothing.
//!
//! WHO GETS WHAT ([`recipients`]). My own entry goes to every member of the organisation as it is
//! once the entry applies, except me (so a removed member stops receiving), plus the person a
//! removal names (so they learn they were removed: nothing after it), plus my own mailbox (my other
//! devices). An invitation also goes, as the invitation text (the chain that proves it, never the
//! member list), to the person invited. When the invitee accepts, their app sends the acceptance to
//! the members its invitation names; the inviter's app, seeing an acceptance of its own invitation
//! applied ([`joined_by_my_invitation`]), forwards it to every other member and sends the new member
//! the whole log.
//!
//! WHAT IS TAKEN ([`judge`]). Nothing is applied that does not verify: whatever is taken is merged
//! into the log and the fold (`humanity_protocol::org`) checks every signature, rank and order, and
//! holds an entry until what it follows arrives. Taking is the spam gate in front of that: a note
//! from my own key (another device of mine); from a member of an organisation I am in (or someone
//! with an open invitation or handover there, whose acceptance it carries); or an invitation for me
//! signed by its sender. Anything else is not merged, so someone who was removed, or a stranger,
//! cannot fill my log; a note from someone this device does not know as a member YET (the log that
//! proves them has not arrived) is parked in memory and judged again after every merge.

use std::collections::BTreeSet;

use humanity_protocol::object::Object;
use humanity_protocol::org::{self, Act, OrgState};

use crate::net::dm_pq::CTL_ORG;

/// The most text one note carries: with the inner payload's two keys and signature (about 12.3 KB)
/// a note then fits the largest padding bucket (32,768, `dm_pq::DM_PAD_BUCKETS`), as a task note
/// does (gui/mine_tasks.rs). An entry is about 7 KB of text, so two travel per note.
pub const NOTE_TEXT_MAX: usize = 19_000;

/// At most this many notes wait parked in memory for the log that proves their sender.
pub const PARKED_MAX: usize = 64;

/// Whether a DM's text is an organisation note.
pub fn is_note(text: &str) -> bool {
    text.starts_with(CTL_ORG)
}

fn note_of(org_id: &str, objects: &[Object]) -> String {
    format!("{CTL_ORG}{}", org::bundle_text(org_id, objects))
}

/// The notes carrying `objects` of organisation `org_id`: as many whole objects per note as fit
/// within [`NOTE_TEXT_MAX`] (one object always goes, even alone).
pub fn note_texts(org_id: &str, objects: &[Object]) -> Vec<String> {
    let mut notes = Vec::new();
    let mut batch: Vec<Object> = Vec::new();
    for o in objects {
        batch.push(o.clone());
        if batch.len() > 1 && note_of(org_id, &batch).len() > NOTE_TEXT_MAX {
            let last = batch.pop().expect("two or more");
            notes.push(note_of(org_id, &batch));
            batch = vec![last];
        }
    }
    if !batch.is_empty() {
        notes.push(note_of(org_id, &batch));
    }
    notes
}

/// The organisation id and objects a note carries, or `None` when it is not a readable note.
pub fn parse_note(text: &str) -> Option<(String, Vec<Object>)> {
    org::parse_bundle(text.strip_prefix(CTL_ORG)?).ok()
}

/// Who an entry I signed goes to, besides my own mailbox: every member of `after` (the
/// organisation with the entry applied) except me, plus the person a removal names. Sorted, once
/// each.
pub fn recipients(after: &OrgState, me: &[u8], act: &Act) -> Vec<Vec<u8>> {
    let mut out: BTreeSet<Vec<u8>> = after.members.values().map(|m| m.key.clone()).filter(|k| k.as_slice() != me).collect();
    if let Act::Remove { who } = act {
        if who.as_slice() != me {
            out.insert(who.clone());
        }
    }
    out.into_iter().collect()
}

/// Deliveries of the objects `ids` of `org_id` to `to`, two objects to a delivery (one note each:
/// an entry is about 7 KB of text), so the pump sends them one at a time at the server's pace.
pub fn chunked(org_id: &str, to: &str, ids: &[String]) -> Vec<OutItem> {
    ids.chunks(2).map(|c| OutItem { org: org_id.to_string(), to: to.to_string(), ids: c.to_vec() }).collect()
}

/// What to queue when this device signed entry `entry_id`, applied in `after`: the entry to my own
/// mailbox (my other devices) and to every [`recipients`], and for an invitation, the invitation
/// (`invitation`: the ids it carries, `OrgState::invite_bundle`'s) to the person invited.
pub fn deliveries(after: &OrgState, me: &[u8], entry_id: &str, act: &Act, invitation: Option<&[String]>) -> Vec<OutItem> {
    let ids = [entry_id.to_string()];
    let mut out = chunked(&after.org_id, &hex::encode(me), &ids);
    for k in recipients(after, me, act) {
        out.extend(chunked(&after.org_id, &hex::encode(&k), &ids));
    }
    if let (Act::Invite { who, .. }, Some(chain)) = (act, invitation) {
        out.extend(chunked(&after.org_id, &hex::encode(who), chain));
    }
    out
}

/// What to do with an organisation note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    /// Merge it into the log (the fold then checks every entry).
    Merge,
    /// Its sender is not (yet) known here as a member: keep it in memory and judge it again after
    /// the next merge.
    Park,
    /// Not merged: the word says why (logged, never shown).
    Drop(&'static str),
}

/// Whether `objects` (folded as an invitation is) hold an open invitation for `me` made by `from`.
fn invites_me(org_id: &str, objects: &[Object], me: &[u8], from: &[u8], now_ms: u64) -> bool {
    org::fold_invitation(org_id, objects, now_ms).is_ok_and(|st| st.member(me).is_none() && st.open_invites(now_ms).iter().any(|i| i.who == me && i.by == from))
}

/// Judge a note from `from` carrying `incoming` for organisation `org_id`, of which this device
/// holds `mine` (empty: none). See the module's WHAT IS TAKEN.
pub fn judge(org_id: &str, mine: &[Object], incoming: &[Object], me: &[u8], from: &[u8], now_ms: u64) -> Take {
    if from == me {
        return Take::Merge; // another device of mine: only my seed signs as me
    }
    let mut both = mine.to_vec();
    both.extend(incoming.iter().cloned());
    let Ok(view) = org::fold_invitation(org_id, mine, now_ms) else {
        // An organisation this device does not hold: only an invitation for me, from its inviter.
        return if invites_me(org_id, incoming, me, from, now_ms) { Take::Merge } else { Take::Drop("not_invited") };
    };
    let in_it = view.member(me).is_some() || view.open_invites(now_ms).iter().any(|i| i.who == me);
    if !in_it {
        // Removed, left, or never joined: only a new invitation for me.
        return if invites_me(org_id, &both, me, from, now_ms) { Take::Merge } else { Take::Drop("not_mine") };
    }
    let known = |st: &OrgState| {
        st.member(from).is_some() || st.open_invites(now_ms).iter().any(|i| i.who == from) || st.open_transfer(now_ms).is_some_and(|t| t.who == from)
    };
    if known(&view) || org::fold_invitation(org_id, &both, now_ms).is_ok_and(|st| known(&st)) {
        return Take::Merge;
    }
    Take::Park
}

/// The members whose joining applied in `after` but not in `before`, through an invitation `me`
/// made: (their key, the id of their acceptance). The inviter's app forwards each acceptance to the
/// other members and sends the new member the whole log.
pub fn joined_by_my_invitation(before: &OrgState, after: &OrgState, me: &[u8]) -> Vec<(Vec<u8>, String)> {
    let mut out = Vec::new();
    for m in after.members.values() {
        if before.members.contains_key(&m.did) {
            continue;
        }
        let accepted = after.log.iter().find(|l| {
            l.refused.is_none()
                && l.actor == m.key
                && matches!(&l.act, Some(Act::Accept { invite }) if after.invites.get(invite).is_some_and(|i| i.by == me))
        });
        if let Some(line) = accepted {
            out.push((m.key.clone(), line.entry_id.clone()));
        }
    }
    out
}

/// One delivery waiting to go: the objects `ids` of organisation `org`, to the person whose key is
/// `to` (lowercase hex; my own key: my other devices). Kept in the organisations file until sent
/// (src/gui/orgs_store.rs), so a crash or a closed app loses nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutItem {
    pub org: String,
    pub to: String,
    pub ids: Vec<String>,
}

impl OutItem {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "org": self.org, "to": self.to, "ids": self.ids })
    }
    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        let ids = v["ids"].as_array()?.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?;
        Some(Self { org: v["org"].as_str()?.to_string(), to: v["to"].as_str()?.to_string(), ids })
    }
}

/// What the page shows of delivery for one organisation, kept in memory by the engine.
#[derive(Debug, Default, Clone)]
pub struct MailState {
    /// Sends whose answer has not come: their `ref` and what they carried.
    pub sent: Vec<(String, OutItem)>,
    /// (org, member key hex) the server refused because of the member's "who can reach me".
    pub unreachable: BTreeSet<(String, String)>,
    /// Notes from people not yet known here as members: (org, from hex, objects).
    pub parked: Vec<(String, String, Vec<Object>)>,
}

#[cfg(test)]
#[path = "org_mail_tests.rs"]
mod tests;
