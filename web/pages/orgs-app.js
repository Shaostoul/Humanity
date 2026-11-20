// Organisations on the website (rung 1, 2026-11-14, docs/design/organisations.md): the mirror of
// the desktop page src/gui/pages/orgs.rs. The rules are web/shared/org-log.js (the twin of
// crates/humanity-protocol/src/org.rs); every change is signed with the person's own Dilithium3 key
// (window.pqSignMessage, as the Governance page signs votes). The logs are kept in this browser
// (localStorage `hos_orgs_v1:<did>`), never on a server. Rung 2 (2026-11-15): every change goes to
// each member as a sealed DM and my mailbox's organisation notes are taken in (the Delivery section
// below; the rules are web/shared/org-mail.js); copying text stays as the fallback.
(function () {
  'use strict';
  const STORE_PREFIX = 'hos_orgs_v1:';
  const DAY = 86400000;
  const $ = (id) => document.getElementById(id);
  const t = (k, v) => window.i18n.t(k, v);
  const tn = (k, n, v) => window.i18n.tn(k, n, v);

  let mods = null;
  async function deps() {
    if (mods) return mods;
    const [org, om, noble] = await Promise.all([import('/shared/org-log.js'), import('/shared/org-mail.js'), import('/shared/vendor/noble-pq.bundle.js')]);
    const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
    const pqVerify = async (pub, msg, sig) => { try { return !!noble.ml_dsa65.verify(sig, msg, pub); } catch (_e) { return false; } };
    mods = { org, om, blake3, pqVerify };
    return mods;
  }

  function hexToBytes(hex) {
    const a = new Uint8Array(hex.length / 2);
    for (let i = 0; i < a.length; i++) a[i] = parseInt(hex.substr(i * 2, 2), 16);
    return a;
  }

  let meP = null;
  async function me() {
    if (!meP) {
      meP = (async () => {
        if (typeof window.getPqIdentity !== 'function') return null;
        const id = await window.getPqIdentity();
        if (!id) return null;
        const { org, blake3 } = await deps();
        const pub = hexToBytes(id.dilithiumPublicHex);
        return { id, pub, hex: id.dilithiumPublicHex, did: org.didFor(pub, blake3), sign: (bytes) => window.pqSignMessage(id.dilithiumSecret, bytes) };
      })().catch(() => null);
      meP.then((r) => { if (!r) meP = null; });
    }
    return meP;
  }

  // ── The logs kept in this browser, encrypted: AES-256-GCM under window.getPqOrgsKey (HKDF of
  // the seed, info hum/orgs-web/v1), stored as {v: 1, iv, ct}. A value that does not open is never
  // written over: this session then saves nothing and the page says so. ──
  let unreadable = null;
  let saving = Promise.resolve();
  function b64(u8) { let s = ''; for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]); return btoa(s); }
  function unb64(s) { const b = atob(s); const u = new Uint8Array(b.length); for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i); return u; }
  async function storeKey() { return typeof window.getPqOrgsKey === 'function' ? window.getPqOrgsKey() : null; }
  // What this page keeps, for the identity `keptFor`: {logs, outbox, pins: {pins, changes}, hw}.
  let kept = null;
  let keptFor = null;
  async function load(did) {
    if (kept && keptFor === did) return kept.logs;
    kept = { logs: {}, outbox: [], pins: { pins: {}, changes: {} }, hw: {} };
    keptFor = did;
    let raw = null;
    try { raw = localStorage.getItem(STORE_PREFIX + did); } catch (_e) { /* storage blocked: start empty */ }
    if (!raw) return kept.logs;
    try {
      const box = JSON.parse(raw);
      const key = await storeKey();
      const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(box.iv) }, key, unb64(box.ct));
      const v = JSON.parse(new TextDecoder().decode(plain)) || {};
      kept = { logs: v.logs || {}, outbox: v.outbox || [], pins: v.pins || { pins: {}, changes: {} }, hw: v.hw || {} };
      return kept.logs;
    } catch (e) {
      unreadable = String((e && e.message) || e);
      return kept.logs;
    }
  }
  function saveBody() { return keptFor ? save(keptFor, kept.logs) : Promise.resolve(); }
  function save(did, logs) {
    if (kept && keptFor === did) kept.logs = logs;
    if (unreadable) { say(t('orgs.store.unreadable', { why: unreadable })); return saving; }
    saving = saving.then(async () => {
      try {
        const key = await storeKey();
        const iv = crypto.getRandomValues(new Uint8Array(12));
        const ct = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, new TextEncoder().encode(JSON.stringify(kept || { logs }))));
        localStorage.setItem(STORE_PREFIX + did, JSON.stringify({ v: 1, iv: b64(iv), ct: b64(ct) }));
      } catch (e) { say(t('orgs.store.unreadable', { why: String((e && e.message) || e) })); }
    });
    return saving;
  }

  // ── Small DOM helpers: every control gets its words and its hint by key ──
  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }
  function btn(labelKey, hintKey, onClick, cls, labelText) {
    const b = el('button', cls || '', labelText != null ? labelText : t(labelKey));
    b.type = 'button';
    b.setAttribute('data-i18n-tip', hintKey);
    b.setAttribute('data-tip-desc', t(hintKey));
    b.addEventListener('click', onClick);
    return b;
  }
  function field(labelKey, hintKey, input) {
    const wrap = el('div');
    const id = 'og-f-' + labelKey.replace(/\W/g, '_');
    const l = el('label', '', t(labelKey));
    l.htmlFor = id;
    input.id = id;
    input.setAttribute('data-i18n-tip', hintKey);
    input.setAttribute('data-tip-desc', t(hintKey));
    wrap.append(l, input);
    return wrap;
  }
  function textInput(value, max) {
    const i = el('input');
    i.type = 'text';
    i.value = value || '';
    if (max) i.maxLength = max;
    return i;
  }
  function say(text) { $('og-msg').textContent = text || ''; }
  function when(ms) { return new Date(ms).toISOString().slice(0, 16).replace('T', ' '); }
  function shortDid(did) { return (did || '').replace('did:hum:', '').slice(0, 10); }

  const ui = { view: 'list', selected: null, tab: 'members', out: null, outNote: null, pending: null };

  function roleWord(st, id) {
    const { org } = mods;
    if (org.isBuiltIn(id)) return t('orgs.role.' + id);
    const r = org.roleOf(st, id);
    return r ? (r.name || r.id) : id;
  }
  function permWord(p) { return t('orgs.perm.' + p.replace(/\./g, '_')); }
  function shownWord(s) { return s.private ? t('orgs.private_person') : (s.name || shortDid(s.did)); }

  async function stateOf(logs, orgId) {
    const { org, blake3, pqVerify } = await deps();
    const r = await org.fold(orgId, logs[orgId] || [], Date.now(), { blake3, pqVerify });
    return r.state || null;
  }

  /** The fold an act is signed against: a member's, or for an acceptance the invitee's (rung 2). */
  async function viewFor(logs, orgId, act) {
    const { org, blake3, pqVerify } = await deps();
    const f = act.act === 'accept' ? org.foldInvitation : org.fold;
    return (await f(orgId, logs[orgId] || [], Date.now(), { blake3, pqVerify })).state || null;
  }

  async function signAct(m, logs, orgId, act) {
    const { org, om, blake3 } = await deps();
    const before = await viewFor(logs, orgId, act);
    const built = await org.buildEntry(orgId, act, { authorPublicKey: m.pub, sign: m.sign, blake3, prev: before ? before.heads : [] });
    logs[orgId] = logs[orgId] || [];
    org.merge(orgId, logs[orgId], [built.submission], blake3);
    const st = await viewFor(logs, orgId, act);
    const line = st && st.log.find((l) => l.entryId === built.objectId);
    const refused = line ? line.refused : 'malformed';
    if (!refused) {
      const inv = act.act === 'invite' ? st.invites.get(built.objectId) : null;
      queue(om.deliveries(st, m.pub, built.objectId, act, inv ? [orgId, ...inv.support, built.objectId] : null));
    }
    await save(m.did, logs);
    pump();
    return { built, refused };
  }

  function refusedWords(r) { return t('orgs.not_applied', { why: t('orgs.refused.' + r) }); }

  // ── Delivery (rung 2, docs/design/organisations.md section 3; 2026-11-15) ──
  // Every change this browser signs goes to each member as a sealed DM, and my mailbox's
  // organisation notes are taken in. The rules are /shared/org-mail.js (the twin of the desktop's
  // src/gui/org_mail.rs). The page signs in on its own socket (/shared/relay-session.js), reads its
  // own mailbox, and seals to a member only with the Kyber768 key on their own key card, checked here
  // (/shared/key-card.js) and pinned in this page's encrypted store; a key a server merely says is
  // never sealed to. What waits to go is kept in the same store, so a closed tab loses nothing.
  const mail = { session: null, bound: false, ref: null, after: 0, unreachable: new Set(), parked: [], sent: new Map(), asked: new Set(), pumping: false };
  let chain = Promise.resolve();
  function inOrder(fn) { chain = chain.then(fn).catch((e) => console.warn('[orgs] delivery:', e && e.message)); return chain; }
  const pause = (ms) => new Promise((r) => setTimeout(r, ms));
  async function dmKeys() { return typeof window.getPqDmKeys === 'function' ? window.getPqDmKeys() : null; }
  function newRef() { return 'orgs-' + [...crypto.getRandomValues(new Uint8Array(12))].map((b) => b.toString(16).padStart(2, '0')).join(''); }

  function queue(items) {
    for (const it of items) {
      if (!kept.outbox.some((x) => x.org === it.org && x.to === it.to && x.ids.join() === it.ids.join())) kept.outbox.push(it);
    }
  }

  async function startMail() {
    if (mail.session || typeof window.createRelaySession !== 'function' || typeof window.pqDmSeal !== 'function') return;
    const m = await me();
    if (!m) return;
    mail.session = window.createRelaySession({
      getIdentity: async () => m.id,
      signBytes: (secret, bytes) => window.pqSignMessage(secret, bytes),
      WebSocket: window.WebSocket, location: window.location, storage: window.localStorage,
    });
    mail.session.subscribe((ev, st) => {
      const bound = st.status === 'bound';
      if (bound && !mail.bound) inOrder(() => fetchMail((kept && kept.hw[location.host]) || 0));
      mail.bound = bound;
      if (ev.type !== 'frame') return;
      const msg = ev.msg;
      if (msg.type === 'dm_batch') inOrder(() => takeBatch(msg));
      else if (msg.type === 'dm_new') inOrder(() => takeLive(msg));
      else if (msg.type === 'key_cards') inOrder(() => takeCards(msg));
      else if (msg.type === 'dm_put_ok' || msg.type === 'dm_put_refused') answer(msg);
    });
    mail.session.start();
  }

  function fetchMail(after) {
    mail.ref = newRef();
    mail.after = Number(after) || 0;
    mail.session.send({ type: 'dm_fetch', after_id: mail.after, ref: mail.ref });
  }

  async function takeBatch(msg) {
    if (!mail.ref || msg.ref !== mail.ref) return; // another device's page of my mailbox
    let last = mail.after;
    for (const item of msg.messages || []) {
      if (item.id > last) last = item.id;
      if (item.content) await takeEnvelope(item.content);
    }
    if (last > (kept.hw[location.host] || 0)) kept.hw[location.host] = last;
    await saveBody();
    if (msg.done === false) { fetchMail(last); return; }
    mail.ref = null;
    render();
    pump();
  }

  async function takeLive(msg) {
    if (!msg.content) return;
    const took = await takeEnvelope(msg.content);
    if (!mail.ref && Number(msg.id) > (kept.hw[location.host] || 0)) kept.hw[location.host] = Number(msg.id);
    await saveBody();
    if (took) { render(); pump(); }
  }

  /** One envelope from my mailbox: an organisation note to me, its inner signature checked by its sender's key. */
  async function takeEnvelope(content) {
    const keys = await dmKeys();
    const m = await me();
    if (!keys || !m) return false;
    let inner;
    try {
      const env = JSON.parse(content);
      if (!env || env.v !== 2 || !env.ek_ct_b64) return false;
      const json = await window.pqDmOpen(keys.kyberSecret, env.ek_ct_b64, env.nonce_b64, env.ct_b64);
      inner = json ? JSON.parse(json) : null;
    } catch (_e) { return false; }
    if (!inner || inner.v !== 2 || !mods.om.isNote(inner.text) || inner.to !== m.hex || typeof inner.sig !== 'string') return false;
    const words = new TextEncoder().encode(`hum/dm/v2\n${inner.from}\n${inner.to}\n${Number(inner.ts) || 0}\n${inner.text}`);
    if (!(await window.pqVerifyMessage(hexToBytes(inner.from), words, unb64(inner.sig)))) return false;
    const note = mods.om.parseNote(inner.text);
    if (!note) return false;
    await take(m, note.org, hexToBytes(inner.from), note.objects);
    return true;
  }

  /** Judge and take one note's objects; the inviter forwards an acceptance of its own invitation. */
  async function take(m, orgId, from, objects) {
    const { org, om, blake3, pqVerify } = await deps();
    const d = { blake3, pqVerify };
    const now = Date.now();
    if (!kept || keptFor !== m.did) await load(m.did);
    const logs = kept.logs;
    const mine = logs[orgId] || [];
    const verdict = await om.judge(orgId, mine, objects, m.pub, from, now, d);
    if (verdict === 'park') {
      mail.parked.push({ orgId, from, objects });
      if (mail.parked.length > om.PARKED_MAX) mail.parked.shift();
      return;
    }
    if (verdict !== 'merge') return;
    const before = mine.length ? (await org.fold(orgId, mine, now, d)).state : null;
    logs[orgId] = mine;
    if (!org.merge(orgId, logs[orgId], objects, blake3)) return;
    const after = (await org.fold(orgId, logs[orgId], now, d)).state;
    if (before && after) {
      for (const j of om.joinedByMyInvitation(before, after, m.pub)) {
        queue(om.chunked(orgId, org.hex(j.key), logs[orgId].map((s) => org.objectIdOf(s, blake3))));
        for (const x of after.members.values()) {
          const k = org.hex(x.key);
          if (k !== org.hex(j.key) && k !== m.hex) queue(om.chunked(orgId, k, [j.accept]));
        }
      }
    }
    if (!before) {
      const lenient = (await org.foldInvitation(orgId, logs[orgId], now, d)).state;
      const inv = lenient && !org.memberOf(lenient, m.pub) && org.openInvites(lenient, now).find((i) => i.whoDid === m.did);
      if (inv) { ui.pending = { orgId, inviteId: inv.id }; say(t('orgs.mail.invited', { org: lenient.name })); }
    }
    await saveBody();
    const again = mail.parked.filter((p) => p.orgId === orgId);
    mail.parked = mail.parked.filter((p) => p.orgId !== orgId);
    for (const p of again) await take(m, p.orgId, p.from, p.objects);
  }

  async function takeCards(msg) {
    let dirty = false;
    for (const raw of Array.isArray(msg.cards) ? msg.cards : []) {
      const card = window.HosKeyCard ? window.HosKeyCard.parseCard(raw) : null;
      if (!card) continue;
      mail.asked.delete(card.key);
      if (!(await window.HosKeyCard.cardFor(card, card.key, (p, x, s) => window.pqVerifyMessage(p, x, s)))) continue;
      if (window.HosKeyCard.take(kept.pins, card).dirty) dirty = true;
    }
    if (dirty) { await saveBody(); pump(); }
  }

  function answer(msg) {
    const it = mail.sent.get(msg.ref);
    if (!it) return;
    mail.sent.delete(msg.ref);
    const k = it.org + '|' + it.to;
    if (msg.type === 'dm_put_ok') { mail.unreachable.delete(k); return; }
    if (msg.reason === 'reach') { mail.unreachable.add(k); render(); }
    else if (msg.reason === 'rate') { queue([it]); setTimeout(pump, 5000); }
  }

  /** One signed, padded, sealed DM carrying `text` to `to`, sealed to `kyberB64` (Chat's own shape). */
  async function sealedPut(m, to, kyberB64, text) {
    const ts = Date.now();
    const sig = await window.pqSignMessage(m.id.dilithiumSecret, new TextEncoder().encode(`hum/dm/v2\n${m.hex}\n${to}\n${ts}\n${text}`));
    if (!sig) return null;
    const inner = { v: 2, from: m.hex, to, ts, text, sig: b64(sig) };
    // Padded in UTF-8 bytes, as Chat and the desktop pad (BUG-328; scripts/tests/dm-padding-web.test.js).
    const bare = new TextEncoder().encode(JSON.stringify(inner)).length;
    const bucket = [256, 1024, 4096, 16384, 32768].find((x) => bare + 12 <= x) || (bare + 12);
    inner.pad = ' '.repeat(Math.max(0, bucket - bare - 12));
    const sealed = await window.pqDmSeal(kyberB64, JSON.stringify(inner));
    if (!sealed) return null;
    return { type: 'dm_put', to, content: JSON.stringify({ v: 2, ek_ct_b64: sealed.ek_ct_b64, nonce_b64: sealed.nonce_b64, ct_b64: sealed.ct_b64 }) };
  }

  /** Send what waits, at the server's pace (a burst of six, then one a second). */
  async function pump() {
    if (mail.pumping || !mail.session || !mail.bound || mail.ref || !kept || !kept.outbox.length) return;
    mail.pumping = true;
    try {
      const { org, om, blake3 } = await deps();
      const m = await me();
      const keys = await dmKeys();
      if (!m || !keys) return;
      const want = [];
      let burst = 5;
      for (let i = 0; i < kept.outbox.length;) {
        const it = kept.outbox[i];
        const sealTo = it.to === m.hex ? keys.kyberPublicB64 : (window.HosKeyCard ? window.HosKeyCard.sealingKey(kept.pins, it.to) : null);
        if (!sealTo) { if (!mail.asked.has(it.to) && !want.includes(it.to)) want.push(it.to); i++; continue; }
        kept.outbox.splice(i, 1);
        const subs = (kept.logs[it.org] || []).filter((s) => it.ids.includes(org.objectIdOf(s, blake3)));
        for (const text of om.noteTexts(it.org, subs)) {
          const put = await sealedPut(m, it.to, sealTo, text);
          if (!put) continue;
          if (it.to !== m.hex) { put.ref = newRef(); mail.sent.set(put.ref, it); }
          if (!mail.session.send(put)) { queue([it]); return; }
          if (burst > 0) burst--; else await pause(1100);
        }
      }
      for (const k of want) mail.asked.add(k);
      for (let i = 0; i < want.length; i += 16) mail.session.send({ type: 'key_card_request', keys: want.slice(i, i + 16) });
    } finally {
      await saveBody();
      mail.pumping = false;
    }
  }

  /** How delivery stands for one organisation, in words (the desktop's draw_delivery). */
  function deliveryLines(view, st) {
    view.append(el('p', 'muted small', t('orgs.mail.how')));
    const waiting = kept.outbox.filter((x) => x.org === st.orgId).length;
    if (waiting) view.append(el('p', 'small', tn('orgs.mail.waiting', waiting)));
    const names = [...mail.unreachable].filter((k) => k.startsWith(st.orgId + '|')).map((k) => {
      const key = hexToBytes(k.slice(st.orgId.length + 1));
      const mem = mods.org.memberOf(st, key);
      return (mem && mem.name) || shortDid(st.did(key));
    });
    if (names.length) view.append(el('p', 'small warn', t('orgs.mail.unreachable', { names: names.join(', ') })));
    if (st.held) view.append(el('p', 'muted small', tn('orgs.held', st.held)));
  }

  /** Handing the organisation on (the desktop's draw_handover). */
  function handover(view, st, m, act) {
    const { org } = mods;
    const now = Date.now();
    const open = org.openTransfer(st, now);
    const nameOf = (key) => { const mem = org.memberOf(st, key); return (mem && mem.name) || shortDid(st.did(key)); };
    if (open && org.hex(open.who) === m.hex) {
      const c = el('div', 'card');
      c.append(el('p', '', t('orgs.handover.offered_you', { who: nameOf(open.by), when: when(open.until) })));
      c.append(btn('orgs.handover.take', 'orgs.handover.take_hint', async () => { await act({ act: 'accept_transfer', transfer: open.id }); render(); }, 'primary'));
      view.append(c);
      return;
    }
    const mine = org.memberOf(st, m.pub);
    if (!mine || mine.role !== org.OWNER) return;
    view.append(el('h2', '', t('orgs.handover.title')), el('p', 'muted', t('orgs.handover.intro')));
    if (open) {
      view.append(el('p', 'muted', t('orgs.handover.offered', { who: nameOf(open.who), when: when(open.until) })));
      view.append(btn('orgs.handover.withdraw', 'orgs.handover.withdraw_hint', async () => { await act({ act: 'withdraw', invite: open.id }); render(); }));
      return;
    }
    const pick = el('select');
    pick.append(new Option(t('orgs.handover.pick'), ''));
    for (const x of st.members.values()) if (org.hex(x.key) !== m.hex) pick.append(new Option(nameOf(x.key), org.hex(x.key)));
    view.append(field('orgs.handover.member', 'orgs.handover.member_hint', pick));
    view.append(btn('orgs.handover.offer', 'orgs.handover.offer_hint', async () => {
      if (!pick.value) return;
      await act({ act: 'transfer', who: hexToBytes(pick.value), until: now + 7 * DAY });
      render();
    }, 'primary'));
  }

  // ── Views ──
  async function render() {
    const view = $('og-view');
    view.textContent = '';
    const m = await me();
    if (!m) { view.append(el('p', 'muted', t('orgs.web_identity'))); return; }
    await deps();
    const logs = await load(m.did);
    if (unreadable) view.append(el('p', 'muted', t('orgs.store.unreadable', { why: unreadable })));
    if (ui.view === 'found') return renderFound(view, m, logs);
    if (ui.view === 'open') return renderOpen(view, m, logs);
    if (ui.view === 'org' && ui.selected) return renderOrg(view, m, logs);
    if (ui.pending) await renderPending(view, m, logs);
    outCard(view);
    const ids = Object.keys(logs);
    if (!ids.length) { view.append(el('p', 'muted', t('orgs.none_yet'))); return; }
    for (const id of ids) {
      const st = await stateOf(logs, id);
      if (!st) continue;
      const mine = mods.org.memberOf(st, m.pub);
      const role = mine ? t('orgs.my_role', { role: roleWord(st, mine.role) }) : t('orgs.not_member');
      const b = btn(null, 'orgs.list_hint', () => { ui.view = 'org'; ui.selected = id; ui.tab = 'members'; ui.out = null; render(); }, '', st.name + '  (' + tn('orgs.members_count', st.members.size) + ', ' + role + ')');
      const row = el('div', 'row');
      row.append(b);
      view.append(row);
    }
  }

  function backButton() {
    return btn('orgs.back', 'orgs.back_hint', () => { ui.view = 'list'; ui.out = null; ui.pending = null; render(); });
  }

  function outCard(view) {
    if (!ui.out) return;
    const c = el('div', 'card');
    if (ui.outNote) c.append(el('p', '', t(ui.outNote)));
    c.append(btn('orgs.copy', 'orgs.copy_hint', async () => {
      try { await navigator.clipboard.writeText(ui.out); say(t('orgs.copied')); } catch (_e) { say(t('orgs.copy_hint')); }
    }, 'primary'));
    view.append(c);
  }

  function directoryRadios(current, onPick) {
    const wrap = el('div', 'card');
    wrap.append(el('div', 'name', t('orgs.directory')));
    for (const d of ['members', 'organisers', 'public']) {
      const b = btn('orgs.dir.' + d, 'orgs.dir.' + d + '_hint', () => { onPick(d); [...wrap.querySelectorAll('button')].forEach((x) => x.classList.toggle('active', x === b)); }, d === current ? 'active' : '');
      wrap.append(b);
    }
    return wrap;
  }

  function renderFound(view, m, logs) {
    view.append(backButton(), el('h2', '', t('orgs.found_title')));
    const name = textInput('', 80);
    name.placeholder = t('orgs.name_placeholder');
    const about = el('textarea');
    about.maxLength = 500;
    let directory = 'members';
    view.append(field('orgs.name', 'orgs.name_hint', name), field('orgs.about', 'orgs.about_hint', about), directoryRadios(directory, (d) => { directory = d; }));
    view.append(btn('orgs.found_submit', 'orgs.found_submit_hint', async () => {
      if (!name.value.trim()) return;
      try {
        const { org, blake3 } = await deps();
        const book = org.roleBookFrom(await (await fetch('/data/orgs/roles.json')).json());
        const built = await org.buildFounding({ name: name.value.trim(), about: about.value.trim(), directory, roles: book.roles }, { authorPublicKey: m.pub, sign: m.sign, blake3 });
        logs[built.objectId] = [built.submission];
        queue(mods.om.chunked(built.objectId, m.hex, [built.objectId])); // my other devices
        await save(m.did, logs);
        pump();
        ui.view = 'org'; ui.selected = built.objectId; ui.tab = 'members';
        say(t('orgs.signed'));
        render();
      } catch (e) { say(t('orgs.sign_failed', { why: String(e && e.message) })); }
    }, 'primary'));
  }

  function renderOpen(view, m, logs) {
    view.append(backButton(), el('h2', '', t('orgs.open_title')));
    const ta = el('textarea');
    view.append(field('orgs.paste', 'orgs.paste_hint', ta));
    view.append(btn('orgs.open_submit', 'orgs.open_submit_hint', async () => {
      const { org, blake3 } = await deps();
      const parsed = org.parseBundle(ta.value);
      if (parsed.error) { say(t('orgs.bad.' + parsed.error)); return; }
      logs[parsed.org] = logs[parsed.org] || [];
      const added = org.merge(parsed.org, logs[parsed.org], parsed.objects, blake3);
      if (!logs[parsed.org].length) delete logs[parsed.org];
      await save(m.did, logs);
      const st = await viewFor(logs, parsed.org, { act: 'accept' }); // as the invitee sees it
      if (!st) { say(t('orgs.bad.no_founding')); return; }
      say(added ? tn('orgs.opened', added) : t('orgs.opened_nothing'));
      const inv = org.openInvites(st, Date.now()).find((i) => i.whoDid === m.did && !org.memberOf(st, m.pub));
      ui.pending = inv ? { orgId: parsed.org, inviteId: inv.id } : null;
      render();
    }, 'primary'));
    if (ui.pending) renderPending(view, m, logs);
    outCard(view);
  }

  async function renderPending(view, m, logs) {
    const { orgId, inviteId } = ui.pending;
    const st = await viewFor(logs, orgId, { act: 'accept' }); // as the invitee sees it
    const inv = st && st.invites.get(inviteId);
    if (!inv) return;
    const by = mods.org.memberOf(st, inv.by);
    const inviter = (by && by.name) || shortDid(st.did(inv.by));
    const c = el('div', 'card');
    c.append(el('p', '', t('orgs.invited', { inviter, org: st.name, role: roleWord(st, inv.role) })));
    c.append(el('p', 'muted small', t('orgs.invited_until', { when: when(inv.until) })));
    const row = el('div', 'row');
    row.append(btn('orgs.accept', 'orgs.accept_hint', async () => {
      const r = await signAct(m, logs, orgId, { act: 'accept', invite: inviteId });
      if (r.refused) { say(refusedWords(r.refused)); return; }
      ui.out = mods.org.bundleText(orgId, [r.built.submission]);
      ui.outNote = 'orgs.send_back';
      ui.pending = null;
      say(t('orgs.signed'));
      render();
    }, 'primary'), btn('orgs.not_now', 'orgs.not_now_hint', () => { ui.pending = null; render(); }));
    c.append(row);
    view.append(c);
  }

  async function renderOrg(view, m, logs) {
    const orgId = ui.selected;
    const { org } = mods;
    const st = await stateOf(logs, orgId);
    view.append(backButton());
    if (!st) { view.append(el('p', 'muted', t('orgs.bad.no_founding'))); return; }
    view.append(el('h2', '', st.name));
    if (st.about) view.append(el('p', 'muted', st.about));
    const mine = org.memberOf(st, m.pub);
    const head = el('div', 'row');
    head.append(el('span', 'muted', mine ? t('orgs.my_role', { role: roleWord(st, mine.role) }) : t('orgs.not_member')));
    if (mine) head.append(btn('orgs.share_latest', 'orgs.share_latest_hint', () => { ui.out = org.bundleText(orgId, logs[orgId]); ui.outNote = 'orgs.share_latest_note'; render(); }));
    view.append(head);
    if (st.waiting) view.append(el('p', 'muted small', tn('orgs.waiting', st.waiting)));
    deliveryLines(view, st);
    outCard(view);
    const tabs = el('div', 'row');
    tabs.setAttribute('role', 'group');
    tabs.setAttribute('aria-label', t('orgs.tab_label'));
    for (const tab of ['members', 'invite', 'roles', 'log', 'settings', 'me']) {
      tabs.append(btn('orgs.tab.' + tab, 'orgs.tab.' + tab + '_hint', () => { ui.tab = tab; render(); }, ui.tab === tab ? 'active' : ''));
    }
    view.append(tabs);
    const act = async (a) => {
      const r = await signAct(m, logs, orgId, a);
      if (r.refused) { say(refusedWords(r.refused)); render(); return null; }
      say(t('orgs.signed'));
      return r;
    };
    const body = el('div');
    view.append(body);
    const rank = org.rankOf(st, m.pub) ?? 0;
    if (ui.tab === 'members') {
      const d = org.directoryFor(st, m.pub);
      if (d.refused) { body.append(el('p', 'muted', t('orgs.refused_read.' + d.refused))); return; }
      for (const s of d.named) {
        const mem = st.members.get(s.did);
        const c = el('div', 'card');
        const line = el('div', 'row');
        line.append(el('span', 'name', (s.name || t('orgs.no_name')) + (s.did === m.did ? '  (' + t('orgs.you') + ')' : '')), el('span', 'muted', roleWord(st, s.role)), el('span', 'muted small', shortDid(s.did)));
        c.append(line);
        const theirs = org.roleOf(st, mem.role);
        if (s.did !== m.did && mem.role !== org.OWNER && theirs && theirs.rank < rank) {
          const acts = el('div', 'row');
          if (org.can(st, m.pub, org.PERM.ASSIGN)) {
            for (const r of st.roles.filter((r) => r.rank < rank && r.id !== mem.role)) {
              acts.append(btn(null, 'orgs.change_role_hint', async () => { await act({ act: 'assign', who: mem.key, role: r.id }); render(); }, '', roleWord(st, r.id)));
            }
          }
          if (org.can(st, m.pub, org.PERM.REMOVE)) acts.append(btn('orgs.remove', 'orgs.remove_hint', async () => { await act({ act: 'remove', who: mem.key }); render(); }, 'danger'));
          c.append(acts);
        }
        body.append(c);
      }
      if (d.private) body.append(el('p', 'muted', tn('orgs.private_count', d.private)));
      body.append(el('p', 'muted small', t('orgs.privacy_note')));
    } else if (ui.tab === 'invite') {
      if (!org.can(st, m.pub, org.PERM.INVITE)) { body.append(el('p', 'muted', t('orgs.invite_needs'))); return; }
      const key = textInput('');
      let role = 'member';
      const days = el('input');
      days.type = 'number'; days.min = '1'; days.max = '60'; days.value = '7';
      const roles = el('div', 'row');
      for (const r of st.roles.filter((r) => r.rank < rank)) {
        const b = btn(null, 'orgs.invite_role_hint', () => { role = r.id; [...roles.children].forEach((x) => x.classList.toggle('active', x === b)); }, r.id === role ? 'active' : '', roleWord(st, r.id));
        roles.append(b);
      }
      body.append(field('orgs.invite_key', 'orgs.invite_key_hint', key), el('label', '', t('orgs.invite_role')), roles, field('orgs.invite_days', 'orgs.invite_days_hint', days));
      body.append(btn('orgs.invite_submit', 'orgs.invite_submit_hint', async () => {
        const hex = key.value.trim();
        if (!/^[0-9a-fA-F]{3904}$/.test(hex)) { say(t('orgs.invite_key_bad')); return; }
        const n = Math.max(1, Math.min(60, parseInt(days.value, 10) || 7));
        const r = await act({ act: 'invite', who: hexToBytes(hex), role, until: Date.now() + n * DAY });
        if (!r) return;
        const st2 = await stateOf(logs, orgId);
        ui.out = org.inviteBundle(st2, logs[orgId], r.built.objectId, mods.blake3);
        ui.outNote = 'orgs.invite_send';
        render();
      }, 'primary'));
      body.append(el('h2', '', t('orgs.open_invites')));
      const open = org.openInvites(st, Date.now());
      if (!open.length) body.append(el('p', 'muted', t('orgs.no_open_invites')));
      for (const inv of open) {
        const row = el('div', 'row');
        row.append(el('span', 'muted', shortDid(inv.whoDid) + '  ' + roleWord(st, inv.role) + '  ' + when(inv.until)));
        row.append(btn('orgs.copy', 'orgs.copy_hint', async () => { try { await navigator.clipboard.writeText(org.inviteBundle(st, logs[orgId], inv.id, mods.blake3)); say(t('orgs.copied')); } catch (_e) { /* the browser refused the clipboard */ } }));
        row.append(btn('orgs.withdraw', 'orgs.withdraw_hint', async () => { await act({ act: 'withdraw', invite: inv.id }); render(); }, 'danger'));
        body.append(row);
      }
    } else if (ui.tab === 'roles') {
      body.append(el('p', 'muted', t('orgs.roles_intro')));
      for (const r of st.roles.slice().sort((a, b) => b.rank - a.rank)) {
        const c = el('div', 'card');
        const line = el('div', 'row');
        line.append(el('span', 'name', roleWord(st, r.id)), el('span', 'muted small', t('orgs.rank', { n: r.rank })));
        if (!org.isBuiltIn(r.id) && r.rank < rank && org.can(st, m.pub, org.PERM.DEFINE)) line.append(btn('orgs.role_drop', 'orgs.role_drop_hint', async () => { await act({ act: 'role_drop', role: r.id }); render(); }, 'danger'));
        c.append(line);
        if (!r.perms.length) c.append(el('div', 'muted small', t('orgs.no_perms')));
        for (const p of r.perms) c.append(el('div', 'small', permWord(p)));
        body.append(c);
      }
      body.append(el('h2', '', t('orgs.new_role')));
      if (!org.can(st, m.pub, org.PERM.DEFINE)) { body.append(el('p', 'muted', t('orgs.role_needs'))); return; }
      const name = textInput('', 40);
      const rk = el('input');
      rk.type = 'number'; rk.min = '0'; rk.max = String(Math.max(0, rank - 1)); rk.value = String(Math.min(20, rank - 1));
      const myPerms = org.roleOf(st, mine.role).perms;
      const picked = new Set();
      const perms = el('div', 'row');
      for (const p of myPerms) {
        const b = btn(null, 'orgs.role_perm_hint', () => { if (picked.has(p)) picked.delete(p); else picked.add(p); b.classList.toggle('active', picked.has(p)); }, '', permWord(p));
        perms.append(b);
      }
      body.append(field('orgs.role_name', 'orgs.role_name_hint', name), field('orgs.role_rank', 'orgs.role_rank_hint', rk), perms);
      body.append(btn('orgs.role_submit', 'orgs.role_submit_hint', async () => {
        const id = name.value.trim().toLowerCase().replace(/[^a-z0-9]/g, '_').slice(0, 32);
        if (!id.replace(/_/g, '') || org.isBuiltIn(id)) return;
        const r = Math.max(0, Math.min(rank - 1, parseInt(rk.value, 10) || 0));
        await act({ act: 'role_set', role: { id, name: name.value.trim(), rank: r, perms: myPerms.filter((p) => picked.has(p)) } });
        render();
      }, 'primary'));
    } else if (ui.tab === 'log') {
      const a = org.auditLogFor(st, m.pub);
      if (a.refused) { body.append(el('p', 'muted', t('orgs.refused_read.log_not_a_member'))); return; }
      body.append(el('p', 'muted', t('orgs.log_intro')));
      for (const { line, actor, subject } of a.lines.slice(0, 500)) {
        let what;
        if (line.entryId === st.orgId) what = t('orgs.act.founding');
        else if (!line.act) what = t('orgs.act.unknown');
        else {
          const ac = line.act;
          const role = ac.act === 'role_set' ? (ac.role.name || ac.role.id) : (ac.role ? roleWord(st, ac.role) : '');
          what = t('orgs.act.' + ac.act, { subject: subject ? shownWord(subject) : '', role });
        }
        let text = when(line.at) + '  ' + shownWord(actor) + ': ' + what;
        if (line.refused) text += '  ' + refusedWords(line.refused);
        body.append(el('div', 'log-line' + (line.refused ? ' refused' : ''), text));
      }
    } else if (ui.tab === 'settings') {
      if (!org.can(st, m.pub, org.PERM.SETTINGS)) { body.append(el('p', 'muted', t('orgs.settings_needs'))); return; }
      const name = textInput(st.name, 80);
      const about = el('textarea');
      about.value = st.about; about.maxLength = 500;
      let directory = st.directory;
      body.append(field('orgs.name', 'orgs.name_hint', name), field('orgs.about', 'orgs.about_hint', about), directoryRadios(directory, (d) => { directory = d; }));
      body.append(btn('orgs.settings_submit', 'orgs.settings_submit_hint', async () => {
        const a = { act: 'settings', name: name.value.trim() !== st.name ? name.value.trim() : null, about: about.value !== st.about ? about.value : null, directory: directory !== st.directory ? directory : null };
        if (a.name == null && a.about == null && a.directory == null) return;
        await act(a);
        render();
      }, 'primary'));
    } else if (ui.tab === 'me') {
      if (!mine) {
        body.append(el('p', 'muted', t('orgs.not_member')));
      } else {
        const name = textInput(mine.name || '', 60);
        let level = mine.listing;
        const levels = el('div', 'card');
        levels.append(el('div', 'name', t('orgs.me_level')));
        for (const l of ['members', 'organisers']) {
          const b = btn('orgs.listing.' + l, 'orgs.listing.' + l + '_hint', () => { level = l; [...levels.querySelectorAll('button')].forEach((x) => x.classList.toggle('active', x === b)); }, l === level ? 'active' : '');
          levels.append(b);
        }
        body.append(field('orgs.me_name', 'orgs.me_name_hint', name), levels);
        body.append(btn('orgs.me_submit', 'orgs.me_submit_hint', async () => { await act({ act: 'listing', name: name.value.trim(), level }); render(); }, 'primary'));
        if (mine.role === org.OWNER) body.append(el('p', 'muted small', t('orgs.leave_owner')));
        else body.append(btn('orgs.leave', 'orgs.leave_hint', async () => { await act({ act: 'leave' }); render(); }, 'danger'));
      }
      handover(body, st, m, act);
      body.append(btn('orgs.forget', 'orgs.forget_hint', async () => { delete logs[orgId]; kept.outbox = kept.outbox.filter((x) => x.org !== orgId); await save(m.did, logs); ui.view = 'list'; ui.selected = null; render(); }, 'danger'));
    }
  }

  function start() {
    $('og-found').addEventListener('click', () => { ui.view = 'found'; ui.out = null; say(''); render(); });
    $('og-open').addEventListener('click', () => { ui.view = 'open'; ui.out = null; say(''); render(); });
    $('og-mykey').addEventListener('click', async () => {
      const m = await me();
      if (!m) { say(t('orgs.web_identity')); return; }
      try { await navigator.clipboard.writeText(m.hex); say(t('orgs.copied')); } catch (_e) { say(m.hex); }
    });
    render();
    startMail();
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start); else start();
})();
