// Organisations, rung 2 (docs/design/organisations.md, section 3; 2026-11-15): an organisation's
// changes reach every member as sealed DMs. The website's twin of src/gui/org_mail.rs: the note
// text, who gets what, and what is taken. No socket here; web/pages/orgs-app.js sends and reads.
//
// THE NOTE. `[[hum:org:v1]]` and then the carried text (`bundleText`: the organisation's id and
// whole signed objects), the text of an ordinary signed sealed-sender v2 DM sealed only to the key
// on the member's own checked key card. Pinned to native CTL_ORG (src/net/dm_pq.rs) by
// scripts/tests/org-mail-web.test.js, which also takes the very notes in
// scripts/tests/fixtures/org-note-v1.json through these rules, as org_mail_tests.rs does on the
// desktop: the same bytes, the same state.
//
// WHO GETS WHAT (`recipients`, `deliveries`): my entry goes to every member of the organisation as
// it is once the entry applies, except me, plus the person a removal names, plus my own mailbox; an
// invitation also goes, as the invitation text, to the person invited. The inviter's app forwards an
// acceptance of its own invitation and sends the new member the whole log (`joinedByMyInvitation`).
//
// WHAT IS TAKEN (`judge`): my own key's notes; a member's (or an open invitee's or handover
// recipient's) note to an organisation I am in; an invitation for me from its inviter. A note from
// someone not yet known as a member is parked for the log that proves them; anything else is not
// merged. Whatever is taken, the fold checks every signature, rank and order.

import { bundleText, parseBundle, foldInvitation, memberOf, openInvites, openTransfer, hex } from './org-log.js';

export const CTL_ORG = '[[hum:org:v1]]';
export const NOTE_TEXT_MAX = 19000;
export const PARKED_MAX = 64;

const sameBytes = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

export function isNote(text) { return typeof text === 'string' && text.startsWith(CTL_ORG); }

function noteOf(orgId, subs) { return CTL_ORG + bundleText(orgId, subs); }

/** The notes carrying `subs` of `orgId`: as many whole objects per note as fit (one always goes). */
export function noteTexts(orgId, subs) {
  const notes = [];
  let batch = [];
  for (const s of subs) {
    batch.push(s);
    if (batch.length > 1 && noteOf(orgId, batch).length > NOTE_TEXT_MAX) {
      const last = batch.pop();
      notes.push(noteOf(orgId, batch));
      batch = [last];
    }
  }
  if (batch.length) notes.push(noteOf(orgId, batch));
  return notes;
}

/** `{org, objects}` of a note, or null. */
export function parseNote(text) {
  if (!isNote(text)) return null;
  const b = parseBundle(text.slice(CTL_ORG.length));
  return b.error ? null : b;
}

/** Who an entry I signed goes to besides my own mailbox: keys (Uint8Array), sorted by hex, once each. */
export function recipients(after, me, act) {
  const out = new Map();
  for (const m of after.members.values()) if (!sameBytes(m.key, me)) out.set(hex(m.key), m.key);
  if (act.act === 'remove' && !sameBytes(act.who, me)) out.set(hex(act.who), act.who);
  return [...out.keys()].sort().map((k) => out.get(k));
}

/** Deliveries of `ids` of `orgId` to `to` (hex), two objects to a delivery. */
export function chunked(orgId, to, ids) {
  const out = [];
  for (let i = 0; i < ids.length; i += 2) out.push({ org: orgId, to, ids: ids.slice(i, i + 2) });
  return out;
}

/** What to queue when this browser signed `entryId`, applied in `after` (see the file's WHO GETS WHAT). */
export function deliveries(after, me, entryId, act, invitation) {
  const out = chunked(after.orgId, hex(me), [entryId]);
  for (const k of recipients(after, me, act)) out.push(...chunked(after.orgId, hex(k), [entryId]));
  if (act.act === 'invite' && invitation) out.push(...chunked(after.orgId, hex(act.who), invitation));
  return out;
}

async function invitesMe(orgId, subs, me, from, nowMs, deps) {
  const r = await foldInvitation(orgId, subs, nowMs, deps);
  if (!r.state || memberOf(r.state, me)) return false;
  return openInvites(r.state, nowMs).some((i) => sameBytes(i.who, me) && sameBytes(i.by, from));
}

/** `'merge'`, `'park'`, or `'drop:<why>'` for a note from `from` carrying `incoming`, this browser holding `mine`. */
export async function judge(orgId, mine, incoming, me, from, nowMs, deps) {
  if (sameBytes(from, me)) return 'merge';
  const both = [...mine, ...incoming];
  const view = mine.length ? (await foldInvitation(orgId, mine, nowMs, deps)).state : null;
  if (!view) return (await invitesMe(orgId, incoming, me, from, nowMs, deps)) ? 'merge' : 'drop:not_invited';
  const inIt = !!memberOf(view, me) || openInvites(view, nowMs).some((i) => sameBytes(i.who, me));
  if (!inIt) return (await invitesMe(orgId, both, me, from, nowMs, deps)) ? 'merge' : 'drop:not_mine';
  const known = (st) => !!memberOf(st, from) || openInvites(st, nowMs).some((i) => sameBytes(i.who, from)) || !!(openTransfer(st, nowMs) && sameBytes(openTransfer(st, nowMs).who, from));
  if (known(view)) return 'merge';
  const wider = (await foldInvitation(orgId, both, nowMs, deps)).state;
  return wider && known(wider) ? 'merge' : 'park';
}

/** Members whose joining applied in `after` but not `before`, through an invitation `me` made: [{key, accept}]. */
export function joinedByMyInvitation(before, after, me) {
  const out = [];
  for (const m of after.members.values()) {
    if (before.members.has(m.did)) continue;
    const line = after.log.find((l) => !l.refused && sameBytes(l.actor, m.key) && l.act && l.act.act === 'accept'
      && after.invites.has(l.act.invite) && sameBytes(after.invites.get(l.act.invite).by, me));
    if (line) out.push({ key: m.key, accept: line.entryId });
  }
  return out;
}
