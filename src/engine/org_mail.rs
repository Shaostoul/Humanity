//! Organisations, rung 2 (docs/design/organisations.md, 2026-11-15): the desktop's half of
//! delivery, which touches the sockets. The rules (the note, who gets what, what is taken) are
//! src/gui/org_mail.rs; the logs and the deliveries waiting are kept by src/gui/orgs_store.rs
//! inside the Organisations page (`GuiState::orgs_page`).
//!
//! - The page queues a delivery when this device signs a change (`OrgsPage::sign`), saved at once.
//! - [`pump`], every frame (frame_ws_poll.rs, beside the passes' pacer): while connected and once
//!   this connection's mailbox was read, each waiting delivery whose recipient has a checked key
//!   card here goes out as sealed notes, within the server's `dm_put` budget (the same pacer the
//!   friendship passes use, net/put_pacer.rs). One with no card yet asks for it
//!   (engine/key_cards.rs `want`) and waits; nothing is ever sealed to a key a server merely said.
//! - [`on_answer`]: the relay's `dm_put_ok` / `dm_put_refused` for a note. Refused for the
//!   member's "who can reach me" (`reach`) is shown on the page with the copy fallback; refused for
//!   the burst (`rate`) goes back in the queue.
//! - [`screens_dm`]: THE arrival rule, on the server I am on (engine/dm.rs `ingest_dm`) and on a
//!   parked one (engine/bg_connections.rs `bg_screen`): an organisation note is never stored or
//!   shown as a message. It is judged (gui/org_mail.rs `judge`), merged, and the fold checks every
//!   entry; the inviter's app forwards an acceptance of its invitation and sends the new member
//!   the whole log.

use std::time::Instant;

use humanity_protocol::object::Object;
use humanity_protocol::org;

use crate::gui::org_mail::{self as mail, OutItem, Take};
use crate::gui::GuiState;
use crate::net::dm_pq::DmInner;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Make the page's store the unlocked identity's. False while no identity is unlocked.
pub(crate) fn ensure(gs: &mut GuiState) -> bool {
    let me = gs.profile_public_key.clone();
    let seed = gs.private_key_bytes.clone();
    gs.orgs_page.ensure_store_with(&me, seed.as_deref())
}

fn connected(gs: &GuiState) -> bool {
    gs.ws_client.as_ref().is_some_and(|c| c.is_connected())
}

/// The `dm_put` frames of one waiting delivery (built, not sent, so a test can open them): to my
/// own mailbox for my other devices, or sealed to the recipient's checked key card with a `ref`
/// for the relay's answer. None when it cannot be sealed yet (locked, or no card for them here).
pub(crate) fn puts_for(gs: &GuiState, item: &OutItem, at: u64) -> Option<Vec<(serde_json::Value, Option<String>)>> {
    let objects = gs.orgs_page.pick(&item.org, &item.ids);
    if objects.is_empty() {
        return Some(Vec::new()); // forgotten here since it was queued: nothing to send
    }
    let mut out = Vec::new();
    for text in mail::note_texts(&item.org, &objects) {
        if item.to == gs.profile_public_key {
            let (put, _id) = crate::engine::block::note_put_text(gs, &text, at)?;
            out.push((put, None));
        } else {
            let (mut theirs, _ours) = crate::engine::dm::control_puts_at(gs, &item.to, &text, None, at)?;
            let reference = crate::net::put_answers::new_ref()?;
            theirs["ref"] = serde_json::Value::String(reference.clone());
            out.push((theirs, Some(reference)));
        }
    }
    Some(out)
}

/// Send what waits, within the server's budget (every frame; cheap when nothing waits).
pub(crate) fn pump(gs: &mut GuiState, now: Instant) {
    // The store is opened here too (once per identity), so what waited from an earlier run goes out
    // without the page ever being opened.
    if !connected(gs) || !crate::engine::dm::mailbox_read(gs) || !ensure(gs) || gs.orgs_page.outbox_len() == 0 {
        return;
    }
    let me = gs.profile_public_key.clone();
    let mut i = 0;
    let mut changed = false;
    while let Some(item) = gs.orgs_page.outbox_item(i) {
        if item.to != me && !gs.peer_kyber_keys.contains_key(&item.to) {
            crate::engine::key_cards::want(gs, &item.to, false);
            i += 1;
            continue;
        }
        if !gs.pass_pacer.has(1, now) {
            break;
        }
        let Some(puts) = puts_for(gs, &item, now_ms()) else {
            i += 1;
            continue;
        };
        gs.orgs_page.take_out(i);
        changed = true;
        for (put, reference) in puts {
            gs.pass_pacer.spend(1, now);
            if item.to == me {
                crate::engine::block::to_my_mailboxes(gs, &put.to_string());
            } else if let Some(c) = gs.ws_client.as_ref() {
                c.send(&put.to_string());
                if let Some(r) = reference {
                    gs.orgs_page.mail.sent.push((r, item.clone()));
                }
            }
        }
        let keep = gs.orgs_page.mail.sent.len().saturating_sub(256);
        gs.orgs_page.mail.sent.drain(..keep);
    }
    if changed {
        gs.orgs_page.flush_store();
    }
}

/// The relay's answer to a `dm_put` (engine/put_answer.rs `on_frame`, first). True when it answered
/// an organisation note.
pub(crate) fn on_answer(gs: &mut GuiState, answer: &crate::net::put_answers::Answer) -> bool {
    use crate::net::put_answers::Answer;
    let Some(pos) = gs.orgs_page.mail.sent.iter().position(|(r, _)| r == answer.reference()) else { return false };
    let (_, item) = gs.orgs_page.mail.sent.remove(pos);
    let (org_id, to) = (item.org.clone(), item.to.clone());
    match answer {
        Answer::Taken(_) => {
            gs.orgs_page.mail.unreachable.remove(&(org_id, to));
        }
        Answer::Refused(_, reason) if reason == "reach" => {
            log::info!("[orgs] a member's message settings kept a change out; the page offers the text to copy");
            gs.orgs_page.mail.unreachable.insert((org_id, to));
        }
        Answer::Refused(_, reason) if reason == "rate" => {
            // Over the server's burst: it waits in the queue and goes again at the server's pace.
            log::info!("[orgs] the server's burst refused a change; it waits to go again");
            gs.orgs_page.queue(vec![item]);
        }
        Answer::Refused(_, reason) => log::warn!("[orgs] a change was refused by the server ({reason})"),
    }
    true
}

/// An opened, signature-checked DM: true when it is an organisation note (taken or not, never
/// stored, listed or shown as a message).
pub(crate) fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !mail::is_note(&inner.text) {
        return false;
    }
    if inner.to != gs.profile_public_key {
        return true; // not to me: there is no self-copy of a note to someone else
    }
    let Some((org_id, objects)) = mail::parse_note(&inner.text) else {
        log::info!("[orgs] an organisation note did not read; dropped");
        return true;
    };
    if !ensure(gs) {
        return true;
    }
    take(gs, &org_id, &inner.from, objects);
    true
}

/// Judge and take one note's objects (and any parked note it lets in).
fn take(gs: &mut GuiState, org_id: &str, from_hex: &str, objects: Vec<Object>) {
    let me = hex::decode(&gs.profile_public_key).unwrap_or_default();
    let from = hex::decode(from_hex).unwrap_or_default();
    let now = now_ms();
    let mine = gs.orgs_page.objects(org_id).to_vec();
    match mail::judge(org_id, &mine, &objects, &me, &from, now) {
        Take::Merge => {
            let before = org::fold(org_id, &mine, now).ok();
            let was_known = !mine.is_empty();
            if gs.orgs_page.add(org_id, objects) == 0 {
                return;
            }
            let after = org::fold(org_id, gs.orgs_page.objects(org_id), now).ok();
            if let (Some(b), Some(a)) = (before.as_ref(), after.as_ref()) {
                forward_joins(gs, org_id, b, a, &me);
            }
            if !was_known {
                if let Ok(st) = org::fold_invitation(org_id, gs.orgs_page.objects(org_id), now) {
                    gs.orgs_page.note_invitation(org_id, &st, &me, now);
                }
            }
            // Notes waiting for the log that proves their sender: judged again now.
            let parked: Vec<(String, String, Vec<Object>)> = {
                let p = &mut gs.orgs_page.mail.parked;
                let (these, rest): (Vec<_>, Vec<_>) = std::mem::take(p).into_iter().partition(|(o, _, _)| o == org_id);
                *p = rest;
                these
            };
            for (o, f, objs) in parked {
                take(gs, &o, &f, objs);
            }
        }
        Take::Park => {
            let p = &mut gs.orgs_page.mail.parked;
            if p.len() >= mail::PARKED_MAX {
                p.remove(0);
            }
            p.push((org_id.to_string(), from_hex.to_string(), objects));
        }
        Take::Drop(why) => log::info!("[orgs] an organisation note was not taken ({why})"),
    }
}

/// The inviter's part: each member who joined through my invitation in this merge gets the whole
/// log, and every other member gets their acceptance.
fn forward_joins(gs: &mut GuiState, org_id: &str, before: &org::OrgState, after: &org::OrgState, me: &[u8]) {
    let joined = mail::joined_by_my_invitation(before, after, me);
    if joined.is_empty() {
        return;
    }
    let all: Vec<String> = gs.orgs_page.objects(org_id).iter().filter_map(|o| o.object_id().ok().map(|h| h.to_hex())).collect();
    let mut items = Vec::new();
    for (key, accept_id) in joined {
        items.extend(mail::chunked(org_id, &hex::encode(&key), &all));
        for m in after.members.values().filter(|m| m.key != key && m.key.as_slice() != me) {
            items.extend(mail::chunked(org_id, &hex::encode(&m.key), std::slice::from_ref(&accept_id)));
        }
    }
    gs.orgs_page.queue(items);
}

#[cfg(test)]
#[path = "org_mail_tests.rs"]
mod tests;
