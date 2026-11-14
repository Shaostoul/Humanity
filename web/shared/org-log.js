// org-log.js: organisations on the website (rung 1, 2026-11-14, docs/design/organisations.md).
//
// The twin of crates/humanity-protocol/src/org.rs: the founding (`org_v1`), the signed log of
// changes (`org_entry_v1`), the fold every app computes the same way, the directory and the audit
// log as one reader may see them, the invitation's proof chain and the carried text. Both fold the
// frozen log scripts/tests/fixtures/org-log-v1.json to the state recorded beside it
// (scripts/tests/org-log-web.test.js; org::tests on the desktop side).
//
// Pure: crypto is injected (`{ blake3, pqVerify, sign }`), as in pq-object.js, so Node tests run
// it with the vendored noble bundle and the page with the browser's.

import { cborText, cborBytes, cborMap, cborUint, cborArray, decodeCanonicalCbor, encodeObjectCanonical } from './canonical-cbor.js';
import { buildSignedObject, verifyObjectSubmission } from './pq-object.js';

export const FOUNDING_TYPE = 'org_v1';
export const ENTRY_TYPE = 'org_entry_v1';
export const BUNDLE_VERSION = 1;
export const MAX_ENTRIES = 10000;
export const FUTURE_SLACK_MS = 5 * 60 * 1000;
export const OWNER = 'owner';
export const ADMIN = 'admin';
export const MEMBER = 'member';
export const TOP_RANK = 100;
export const LIMITS = { name: 80, about: 500, roleName: 40, roleId: 32, listingName: 60, perms: 64, roles: 64 };

export const PERM = {
  INVITE: 'members.invite',
  REMOVE: 'members.remove',
  ASSIGN: 'roles.assign',
  DEFINE: 'roles.define',
  SETTINGS: 'org.settings',
  SEE_ALL: 'directory.see_all',
};
export const ENFORCED = Object.values(PERM);

const DIRECTORY = ['members', 'organisers', 'public'];
const LISTING = ['members', 'organisers'];

// ── Text checks (the same as org.rs) ──

function chars(s) { return [...s].length; }
function plain(s, max) {
  // Rust's char::is_control: Unicode general category Cc.
  return typeof s === 'string' && chars(s) <= max && !/\p{Cc}/u.test(s);
}
function roleIdOk(id) { return typeof id === 'string' && id.length >= 1 && id.length <= LIMITS.roleId && /^[a-z0-9_]+$/.test(id); }
function permIdOk(p) { return typeof p === 'string' && p.length >= 1 && p.length <= 48 && /^[a-z0-9_.]+$/.test(p); }
export function isBuiltIn(id) { return id === OWNER || id === ADMIN || id === MEMBER; }

export function roleProblem(r) {
  if (!r || !roleIdOk(r.id)) return 'role_id';
  if (!plain(r.name, LIMITS.roleName)) return 'role_name';
  if (!Number.isInteger(r.rank) || r.rank < 0 || r.rank > TOP_RANK) return 'role_rank';
  if (!Array.isArray(r.perms) || r.perms.length > LIMITS.perms || !r.perms.every(permIdOk)) return 'role_perms';
  if (new Set(r.perms).size !== r.perms.length) return 'role_perms';
  return null;
}

export function foundingProblem(f) {
  if (!plain(f.name, LIMITS.name) || !f.name.trim()) return 'name';
  if (!plain(f.about, LIMITS.about)) return 'about';
  if (!Array.isArray(f.roles) || !f.roles.length || f.roles.length > LIMITS.roles) return 'roles';
  const ids = new Set();
  for (const r of f.roles) {
    const p = roleProblem(r);
    if (p) return p;
    if (ids.has(r.id)) return 'roles';
    ids.add(r.id);
    if (r.id !== OWNER && r.rank >= TOP_RANK) return 'role_rank';
  }
  const owner = f.roles.find((r) => r.id === OWNER);
  if (!owner || owner.rank !== TOP_RANK) return 'owner';
  if (!ids.has(MEMBER) || !ids.has(ADMIN)) return 'roles';
  return null;
}

/** The starting roles and permission ids from data/orgs/roles.json's parsed JSON. */
export function roleBookFrom(json) {
  const permissions = (json.permissions || []).map((p) => p.id);
  const roles = (json.roles || []).map((r) => ({ id: r.id, name: '', rank: r.rank, perms: r.perms.slice() }));
  for (const r of roles) {
    const bad = r.perms.find((p) => !permissions.includes(p));
    if (bad) throw new Error(`roles.json: role ${r.id} names ${bad}, which is not in permissions`);
  }
  return { roles, permissions };
}

// ── Encoding ──

function roleCbor(r) {
  return cborMap([
    [cborText('id'), cborText(r.id)],
    [cborText('name'), cborText(r.name)],
    [cborText('perms'), cborArray(r.perms.map((p) => cborText(p)))],
    [cborText('rank'), cborUint(r.rank)],
  ]);
}

export function foundingPayload(f) {
  return cborMap([
    [cborText('about'), cborText(f.about)],
    [cborText('directory'), cborText(f.directory)],
    [cborText('name'), cborText(f.name)],
    [cborText('roles'), cborArray(f.roles.map(roleCbor))],
  ]);
}

/** An act as `{act: 'invite', who: Uint8Array, role, until}` and so on (the wire words). */
export function actPayload(a) {
  const e = [[cborText('act'), cborText(a.act)]];
  const t = (k, v) => e.push([cborText(k), cborText(v)]);
  switch (a.act) {
    case 'invite': e.push([cborText('who'), cborBytes(a.who)]); t('role', a.role); e.push([cborText('until'), cborUint(a.until)]); break;
    case 'withdraw': case 'accept': t('invite', a.invite); break;
    case 'remove': e.push([cborText('who'), cborBytes(a.who)]); break;
    case 'leave': break;
    case 'assign': e.push([cborText('who'), cborBytes(a.who)]); t('role', a.role); break;
    case 'role_set': e.push([cborText('role'), roleCbor(a.role)]); break;
    case 'role_drop': t('role', a.role); break;
    case 'listing': t('name', a.name); t('level', a.level); break;
    case 'settings':
      if (a.name != null) t('name', a.name);
      if (a.about != null) t('about', a.about);
      if (a.directory != null) t('directory', a.directory);
      break;
    default: throw new Error(`org-log: unknown act ${a.act}`);
  }
  return cborMap(e);
}

/** Sign a founding. `{ objectId (the organisation's id), submission }`. */
export async function buildFounding(f, { authorPublicKey, sign, blake3, createdAt }) {
  const p = foundingProblem(f);
  if (p) throw new Error(`founding: ${p}`);
  return buildSignedObject({ objectType: FOUNDING_TYPE, payload: foundingPayload(f), authorPublicKey, sign, blake3, createdAt: createdAt ?? Date.now() });
}

/** Sign one change to organisation `orgId`. */
export async function buildEntry(orgId, act, { authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({ objectType: ENTRY_TYPE, payload: actPayload(act), references: [orgId], authorPublicKey, sign, blake3, createdAt: createdAt ?? Date.now() });
}

// ── Decoding ──

function num(v) { return typeof v === 'bigint' ? Number(v) : v; }
function isUint(v) { return (typeof v === 'number' && Number.isInteger(v) && v >= 0) || (typeof v === 'bigint' && v >= 0n); }

function roleFrom(v) {
  if (!v || typeof v !== 'object' || !Array.isArray(v.perms) || typeof v.id !== 'string' || typeof v.name !== 'string' || !isUint(v.rank)) return null;
  if (!v.perms.every((p) => typeof p === 'string')) return null;
  const r = { id: v.id, name: v.name, rank: num(v.rank), perms: v.perms.slice() };
  return roleProblem(r) ? null : r;
}

function payloadOf(sub) {
  try {
    const v = decodeCanonicalCbor(b64d(sub.payload_b64));
    return v && typeof v === 'object' && !Array.isArray(v) && !(v instanceof Uint8Array) ? v : null;
  } catch (_e) { return null; }
}

export function parseFounding(sub) {
  if (sub.object_type !== FOUNDING_TYPE) return null;
  const m = payloadOf(sub);
  if (!m || typeof m.name !== 'string' || typeof m.about !== 'string' || !DIRECTORY.includes(m.directory) || !Array.isArray(m.roles)) return null;
  const roles = m.roles.map(roleFrom);
  if (roles.some((r) => !r)) return null;
  const f = { name: m.name, about: m.about, directory: m.directory, roles };
  return foundingProblem(f) ? null : f;
}

export function parseAct(sub) {
  const m = payloadOf(sub);
  if (!m || typeof m.act !== 'string') return null;
  const s = (k) => (typeof m[k] === 'string' ? m[k] : null);
  const b = (k) => (m[k] instanceof Uint8Array ? m[k] : null);
  switch (m.act) {
    case 'invite': return b('who') && s('role') != null && isUint(m.until) ? { act: 'invite', who: b('who'), role: s('role'), until: num(m.until) } : null;
    case 'withdraw': case 'accept': return s('invite') != null ? { act: m.act, invite: s('invite') } : null;
    case 'remove': return b('who') ? { act: 'remove', who: b('who') } : null;
    case 'leave': return { act: 'leave' };
    case 'assign': return b('who') && s('role') != null ? { act: 'assign', who: b('who'), role: s('role') } : null;
    case 'role_set': { const r = roleFrom(m.role); return r ? { act: 'role_set', role: r } : null; }
    case 'role_drop': return s('role') != null ? { act: 'role_drop', role: s('role') } : null;
    case 'listing':
      if (s('name') == null || !plain(m.name, LIMITS.listingName) || !LISTING.includes(m.level)) return null;
      return { act: 'listing', name: m.name, level: m.level };
    case 'settings': {
      const name = s('name'); const about = s('about');
      const directory = m.directory === undefined ? null : (DIRECTORY.includes(m.directory) ? m.directory : undefined);
      if (directory === undefined) return null;
      if (name != null && (!name.trim() || !plain(name, LIMITS.name))) return null;
      if (about != null && !plain(about, LIMITS.about)) return null;
      if (name == null && about == null && directory == null) return null;
      return { act: 'settings', name, about, directory };
    }
    default: return null;
  }
}

// ── Bytes ──

export function b64d(s) {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
export function hex(u8) { return [...u8].map((x) => x.toString(16).padStart(2, '0')).join(''); }
function sameBytes(a, b) { if (a.length !== b.length) return false; for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false; return true; }

const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function base58(bytes) {
  let n = 0n;
  for (const x of bytes) n = (n << 8n) | BigInt(x);
  let out = '';
  while (n > 0n) { out = B58[Number(n % 58n)] + out; n /= 58n; }
  for (const x of bytes) { if (x !== 0) break; out = '1' + out; }
  return out || '1';
}
/** `did:hum:<base58(BLAKE3(key)[..16])>`, as crates/humanity-protocol/src/did.rs. */
export function didFor(key, blake3) { return 'did:hum:' + base58(blake3(key).slice(0, 16)); }

/** The object id of a submission (BLAKE3 of its canonical bytes). */
export function objectIdOf(sub, blake3) {
  const f = {
    protocol_version: sub.protocol_version, object_type: sub.object_type,
    space_id: sub.space_id ?? null, channel_id: sub.channel_id ?? null,
    author_public_key: b64d(sub.author_public_key_b64), created_at: sub.created_at ?? null,
    references: sub.references || [], payload_schema_version: sub.payload_schema_version,
    payload_encoding: sub.payload_encoding, payload: b64d(sub.payload_b64), signature: b64d(sub.signature_b64),
  };
  return hex(blake3(encodeObjectCanonical(f)));
}

// ── The carried text ──

export function bundleText(orgId, subs) { return JSON.stringify({ hum_org: BUNDLE_VERSION, org: orgId, objects: subs }); }

/** `{org, objects}` from a carried text (surrounding words ignored), or `{error}`. */
export function parseBundle(text) {
  const a = text.indexOf('{'); const z = text.lastIndexOf('}');
  if (a < 0 || z < a) return { error: 'no_bundle' };
  let v;
  try { v = JSON.parse(text.slice(a, z + 1)); } catch (_e) { return { error: 'no_bundle' }; }
  if (!v || v.hum_org !== BUNDLE_VERSION) return { error: 'bundle_version' };
  if (typeof v.org !== 'string' || !/^[0-9a-f]{64}$/i.test(v.org) || !Array.isArray(v.objects)) return { error: 'no_bundle' };
  if (v.objects.length > MAX_ENTRIES + 1) return { error: 'too_many' };
  return { org: v.org, objects: v.objects };
}

/** Merge `incoming` into `log` (both arrays of submissions) by object id; how many were new. */
export function merge(orgId, log, incoming, blake3) {
  const have = new Set(log.map((s) => objectIdOf(s, blake3)));
  let added = 0;
  for (const s of incoming) {
    const belongs = s.object_type === FOUNDING_TYPE || (s.object_type === ENTRY_TYPE && (s.references || [])[0] === orgId);
    if (!belongs || log.length > MAX_ENTRIES) continue;
    let id;
    try { id = objectIdOf(s, blake3); } catch (_e) { continue; }
    if (s.object_type === FOUNDING_TYPE && id !== orgId) continue;
    if (!have.has(id)) { have.add(id); log.push(s); added++; }
  }
  return added;
}

// ── The fold ──

/**
 * Fold the submissions of organisation `orgId` into its state, as every app does. Returns
 * `{ state }` or `{ error: 'no_founding' }`. `deps` = `{ blake3, pqVerify }`.
 */
export async function fold(orgId, subs, nowMs, deps) {
  const { blake3 } = deps;
  let founding = null;
  const entries = new Map();
  let dropped = 0; let waiting = 0;
  for (const s of subs) {
    let id;
    try { id = objectIdOf(s, blake3); } catch (_e) { dropped++; continue; }
    if (s.object_type === FOUNDING_TYPE && id === orgId && !founding) {
      const v = await verifyObjectSubmission(s, deps);
      const f = v.ok ? parseFounding(s) : null;
      if (f && s.created_at != null) founding = { sub: s, f, key: b64d(s.author_public_key_b64) }; else dropped++;
    } else if (s.object_type === ENTRY_TYPE && (s.references || [])[0] === orgId) {
      if (s.created_at == null) { dropped++; continue; }
      const k = `${s.created_at}|${id}`;
      if (entries.has(k)) continue;
      const v = await verifyObjectSubmission(s, deps);
      if (!v.ok) { dropped++; continue; }
      if (s.created_at > nowMs + FUTURE_SLACK_MS) { waiting++; continue; }
      entries.set(k, { at: s.created_at, id, sub: s, key: b64d(s.author_public_key_b64) });
    } else if (s.object_type !== FOUNDING_TYPE) {
      dropped++;
    }
    if (entries.size > MAX_ENTRIES) break;
  }
  if (!founding) return { error: 'no_founding' };
  const did = (k) => didFor(k, blake3);
  const foundedAt = founding.sub.created_at;
  const st = {
    orgId, foundedAt, founder: founding.key, name: founding.f.name, about: founding.f.about,
    directory: founding.f.directory, roles: founding.f.roles.map((r) => ({ ...r, perms: r.perms.slice() })),
    members: new Map(), invites: new Map(), log: [], waiting, dropped, did,
  };
  st.members.set(did(founding.key), { key: founding.key, did: did(founding.key), role: OWNER, name: null, listing: 'members', joinedAt: foundedAt, standing: new Set() });
  st.log.push({ entryId: orgId, at: foundedAt, actor: founding.key, act: null, subject: null, refused: null });
  const leftAt = new Map();
  const roleSupport = new Map();
  // Rust orders by (created_at, object_id) through a BTreeMap: numbers, then hex strings.
  const ordered = [...entries.values()].sort((a, b) => (a.at - b.at) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  for (const e of ordered) {
    const act = parseAct(e.sub);
    let subject = null;
    if (act && (act.act === 'invite' || act.act === 'remove' || act.act === 'assign')) subject = act.who;
    else if (act && (act.act === 'accept' || act.act === 'leave' || act.act === 'listing')) subject = e.key;
    let refused = null;
    if (!act) refused = 'malformed';
    else if (e.at < foundedAt) refused = 'before_founding';
    else refused = apply(st, leftAt, roleSupport, e.id, e.at, e.key, act);
    st.log.push({ entryId: e.id, at: e.at, actor: e.key, act, subject, refused });
  }
  return { state: st };
}

export function roleOf(st, id) { return st.roles.find((r) => r.id === id) || null; }
export function memberOf(st, key) { return st.members.get(st.did(key)) || null; }
export function rankOf(st, key) { const m = memberOf(st, key); const r = m && roleOf(st, m.role); return r ? r.rank : null; }
export function can(st, key, p) { const m = memberOf(st, key); const r = m && roleOf(st, m.role); return !!(r && r.perms.includes(p)); }
export function openInvites(st, nowMs) { return [...st.invites.values()].filter((i) => !i.used && !i.withdrawn && i.until >= nowMs); }

function supportOf(st, key) { const m = memberOf(st, key); return new Set(m ? m.standing : []); }

function apply(st, leftAt, roleSupport, id, at, actor, a) {
  const actorDid = st.did(actor);
  const actorRank = rankOf(st, actor);
  if (a.act !== 'accept' && actorRank == null) return 'not_a_member';
  const ar = actorRank ?? 0;
  const need = (p) => (can(st, actor, p) ? null : 'lacks_permission');
  let r;
  switch (a.act) {
    case 'invite': {
      if ((r = need(PERM.INVITE))) return r;
      const role = roleOf(st, a.role);
      if (!role) return 'no_such_role';
      if (role.rank >= ar) return 'rank_too_low';
      if (memberOf(st, a.who)) return 'already_member';
      st.invites.set(id, { id, who: a.who, whoDid: st.did(a.who), role: a.role, until: a.until, by: actor, at, support: supportOf(st, actor), used: false, withdrawn: false });
      return null;
    }
    case 'withdraw': {
      const inv = st.invites.get(a.invite);
      if (!inv) return 'no_such_invite';
      if (!sameBytes(inv.by, actor) && (r = need(PERM.INVITE))) return r;
      if (inv.used) return 'invite_used';
      inv.withdrawn = true;
      return null;
    }
    case 'accept': {
      const inv = st.invites.get(a.invite);
      if (!inv) return 'no_such_invite';
      if (!sameBytes(inv.who, actor)) return 'not_the_invitee';
      if (inv.used) return 'invite_used';
      if (inv.withdrawn) return 'invite_withdrawn';
      if (at > inv.until) return 'invite_expired';
      if (memberOf(st, actor)) return 'already_member';
      if (leftAt.has(actorDid) && leftAt.get(actorDid) >= inv.at) return 'removed_since';
      const role = roleOf(st, inv.role) ? inv.role : MEMBER;
      const roleRank = roleOf(st, role)?.rank ?? 0;
      const byRank = rankOf(st, inv.by);
      if (!(byRank != null && can(st, inv.by, PERM.INVITE) && roleRank < byRank)) return 'inviter_cannot';
      const standing = new Set(inv.support);
      standing.add(inv.id); standing.add(id);
      for (const x of roleSupport.get(role) || []) standing.add(x);
      inv.used = true;
      st.members.set(actorDid, { key: actor, did: actorDid, role, name: null, listing: 'members', joinedAt: at, standing });
      return null;
    }
    case 'remove': {
      if ((r = need(PERM.REMOVE))) return r;
      const t = memberOf(st, a.who);
      if (!t) return 'not_a_member';
      if (t.role === OWNER) return 'owner_stays';
      if ((rankOf(st, a.who) ?? 0) >= ar) return 'rank_too_low';
      st.members.delete(t.did); leftAt.set(t.did, at);
      return null;
    }
    case 'leave': {
      const m = memberOf(st, actor);
      if (!m) return 'not_a_member';
      if (m.role === OWNER) return 'owner_stays';
      st.members.delete(actorDid); leftAt.set(actorDid, at);
      return null;
    }
    case 'assign': {
      if ((r = need(PERM.ASSIGN))) return r;
      const t = memberOf(st, a.who);
      if (!t) return 'not_a_member';
      const role = roleOf(st, a.role);
      if (!role) return 'no_such_role';
      if (t.role === OWNER) return 'owner_stays';
      if (role.rank >= ar || (rankOf(st, a.who) ?? 0) >= ar) return 'rank_too_low';
      const standing = new Set(t.standing);
      for (const x of supportOf(st, actor)) standing.add(x);
      standing.add(id);
      for (const x of roleSupport.get(a.role) || []) standing.add(x);
      t.role = a.role; t.standing = standing;
      return null;
    }
    case 'role_set': {
      if ((r = need(PERM.DEFINE))) return r;
      if (isBuiltIn(a.role.id)) return 'built_in_role';
      if (a.role.rank >= ar) return 'rank_too_low';
      const old = roleOf(st, a.role.id);
      if (old) { if (old.rank >= ar) return 'rank_too_low'; } else if (st.roles.length >= LIMITS.roles) return 'malformed';
      if (a.role.perms.some((p) => !can(st, actor, p))) return 'lacks_permission';
      const support = supportOf(st, actor); support.add(id);
      const had = roleSupport.get(a.role.id) || new Set();
      for (const x of support) had.add(x);
      roleSupport.set(a.role.id, had);
      const copy = { ...a.role, perms: a.role.perms.slice() };
      const i = st.roles.findIndex((x) => x.id === a.role.id);
      if (i >= 0) st.roles[i] = copy; else st.roles.push(copy);
      return null;
    }
    case 'role_drop': {
      if ((r = need(PERM.DEFINE))) return r;
      if (isBuiltIn(a.role)) return 'built_in_role';
      const role = roleOf(st, a.role);
      if (!role) return 'no_such_role';
      if (role.rank >= ar) return 'rank_too_low';
      st.roles = st.roles.filter((x) => x.id !== a.role);
      roleSupport.delete(a.role);
      for (const m of st.members.values()) if (m.role === a.role) m.role = MEMBER;
      return null;
    }
    case 'listing': {
      const m = st.members.get(actorDid);
      if (!m) return 'not_a_member';
      m.name = a.name.trim() ? a.name.trim() : null;
      m.listing = a.level;
      return null;
    }
    case 'settings': {
      if ((r = need(PERM.SETTINGS))) return r;
      if (a.name != null) st.name = a.name.trim();
      if (a.about != null) st.about = a.about;
      if (a.directory != null) st.directory = a.directory;
      return null;
    }
    default: return 'malformed';
  }
}

// ── What a reader may see ──

function names(st, viewer, m) { return m.listing === 'members' || sameBytes(m.key, viewer) || can(st, viewer, PERM.SEE_ALL); }

function shown(st, viewer, key) {
  const m = memberOf(st, key);
  if (m && !names(st, viewer, m)) return { private: true };
  if (m) return { did: m.did, name: m.name, role: m.role };
  return { did: st.did(key), name: null, role: null };
}

/** `{ named: [{did, name, role}], private }` or `{ refused: 'not_a_member' | 'organisers_only' }`. */
export function directoryFor(st, viewer) {
  const member = !!memberOf(st, viewer);
  if (st.directory === 'members' && !member) return { refused: 'not_a_member' };
  if (st.directory === 'organisers' && !member) return { refused: 'not_a_member' };
  if (st.directory === 'organisers' && !can(st, viewer, PERM.SEE_ALL)) return { refused: 'organisers_only' };
  const named = []; let priv = 0;
  const ordered = [...st.members.values()].sort((a, b) => (a.did < b.did ? -1 : a.did > b.did ? 1 : 0));
  for (const m of ordered) {
    if (names(st, viewer, m)) named.push({ did: m.did, name: m.name, role: m.role }); else priv++;
  }
  return { named, private: priv };
}

/** Newest first: `[{line, actor, subject}]`, or `{ refused: 'not_a_member' }`. */
export function auditLogFor(st, viewer) {
  if (!memberOf(st, viewer)) return { refused: 'not_a_member' };
  return { lines: st.log.slice().reverse().map((l) => ({ line: l, actor: shown(st, viewer, l.actor), subject: l.subject ? shown(st, viewer, l.subject) : null })) };
}

/** The text an invitation carries: the founding, the invite and its proof chain, nothing else. */
export function inviteBundle(st, subs, inviteId, blake3) {
  const inv = st.invites.get(inviteId);
  if (!inv) return null;
  const want = new Set(inv.support); want.add(inviteId); want.add(st.orgId);
  return bundleText(st.orgId, subs.filter((s) => want.has(objectIdOf(s, blake3))));
}
