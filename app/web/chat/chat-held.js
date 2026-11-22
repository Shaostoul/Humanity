// chat-held.js: hide and keep in the website's chat (2026-11-20, docs/design/hide-and-keep.md rung 5),
// the browser twin of src/gui/pages/server_settings/held.rs and src/engine/held.rs.
//
// Two things, both for this server's staff, both over Chat's signed-in socket:
//   1. On an open report about a post, "Hide and keep" beside the other decisions (the Reports view,
//      chat-reports.js, calls heldDecorateReports after drawing). It sends the decision with the
//      decider's own signed mod_action_v1 (/shared/pq-object.js). When the server cannot keep a
//      sealed copy (no owner, or no checked key card for the owner) the button is not offered: the
//      relay's own sentence says why, then what works instead ("Delete the post").
//   2. Held copies (admins; /held, or the button the Reports view gains): how many items are held,
//      each one's dates and sizes and the signed action log. Never contents, never package names,
//      and no button that shows an item, for anyone. The OWNER alone gets Legal hold, Destroy now
//      (asks first, says it cannot be undone), Export (this browser opens the sealed parts with the
//      owner's own Kyber768 key straight into one .zip file the owner saves, never drawn) and
//      Restore (opens each file here and uploads it again; the relay checks every byte against what
//      it held, then puts the post back and deletes the held copy). Every request to a held part
//      route is signed as me (`callerHeaders`, the owner review's finding 1): a grant alone fetches
//      nothing.
//   3. The server owner's part of Held copies (Claim ownership while there is no owner, the
//      successor, the owner log, and the owner's Hand over and Successor controls) is drawn by
//      chat-owner.js, loaded after this file (window.ownerHeldNodes, window.ownerRefusalNodes).
//
// Built as DOM: createElement, textContent and addEventListener only. No innerHTML and no data-*
// handlers, so it does not depend on /shared/safe-markup.js. Words by key (data/i18n/en.json).
// Depends on chat globals: ws, myKey, myDilithiumSecret, myKyberSecret, window.hosServerDid,
// window.pqSignMessage, handleMessage, addSystemMessage, and chat-reports.js's reportsMyRole,
// reportsView and requestReports.

(function () {
  'use strict';

  const DAY_MS = 86400000;
  const HOLD_MAX_DAYS = 366;
  const ANSWER_WAIT_MS = 10000;

  const held = {
    open: false,
    data: null,           // the last held_items frame
    keep: { can: true, why: '' }, // from the last reports frame
    status: '',
    busy: false,
    reasons: {},          // item id -> the reason being typed
    days: {},             // item id -> legal hold days being typed
    confirm: null,        // { id, kind: 'destroy' | 'restore' } while a question is open
    waiters: [],          // resolvers waiting for the next held_items frame
  };

  let _mods = null;
  async function mods() {
    if (_mods) return _mods;
    const [obj, noble, seal] = await Promise.all([
      import('/shared/pq-object.js'),
      import('/shared/vendor/noble-pq.bundle.js'),
      import('/shared/held-seal.js'),
    ]);
    _mods = { obj, noble, seal };
    return _mods;
  }

  function t(key, vars) { return (typeof i18n !== 'undefined' && i18n && typeof i18n.t === 'function') ? i18n.t(key, vars) : key; }
  function hint(node, key) { if (typeof i18n !== 'undefined' && i18n && typeof i18n.hint === 'function') i18n.hint(node, key); }
  function say(text) { if (typeof addSystemMessage === 'function') addSystemMessage(text); }

  function make(tag, cls, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }
  function button(textKey, hintKey, onClick, cls) {
    const b = make('button', 'vr-btn' + (cls ? ' ' + cls : ''), t(textKey));
    b.type = 'button';
    hint(b, hintKey);
    b.addEventListener('click', onClick);
    return b;
  }

  function myRole() { return typeof reportsMyRole === 'function' ? reportsMyRole() : (window.myPeerRole || ''); }
  function amAdmin() { return HosRoles.isAdminRole(myRole()); }
  function socketOpen() { return typeof ws !== 'undefined' && ws && ws.readyState === 1; }
  function hexToBytes(hex) {
    const a = new Uint8Array(hex.length / 2);
    for (let i = 0; i < a.length; i++) a[i] = parseInt(hex.substr(i * 2, 2), 16);
    return a;
  }
  function when(ms) {
    if (!ms) return '';
    const d = new Date(ms);
    try { return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
    catch (_) { return d.toISOString().slice(0, 10); }
  }
  function size(bytes) {
    const n = Number(bytes) || 0;
    if (n < 1024) return n + ' B';
    if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
    if (n < 1073741824) return (n / 1048576).toFixed(1) + ' MB';
    return (n / 1073741824).toFixed(2) + ' GB';
  }

  /** Sign a held action as this person, for this server: { objectId, submission }. */
  async function signedAction(action, kind, target, reason, until) {
    const did = window.hosServerDid;
    if (!did) throw new Error(t('held.no_server_did'));
    if (typeof myKey !== 'string' || !myKey || typeof myDilithiumSecret === 'undefined' || !myDilithiumSecret || typeof window.pqSignMessage !== 'function') {
      throw new Error(t('held.need_key'));
    }
    const { obj, noble } = await mods();
    const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
    return obj.buildModActionV1({
      spaceId: did, action, target: String(target), targetKind: kind, reason, rule: '', expiresAt: until || 0, role: '',
      authorPublicKey: hexToBytes(myKey), sign: async (b) => window.pqSignMessage(myDilithiumSecret, b), blake3, createdAt: Date.now(),
    });
  }

  // ── 1. Hide and keep on a report ──

  /** Decide report `item` (the Reports view's model item) with hide and keep, signed. */
  async function hideAndKeep(item) {
    if (!socketOpen()) { say(t('held.not_connected')); return false; }
    const note = (typeof reportsView !== 'undefined' && reportsView.notes) ? String(reportsView.notes[String(item.id)] || '').trim() : '';
    const reason = note || t('held.decide_reason', { id: item.id, reason: item.reasonLabel || item.reason || '' });
    let signed;
    try { signed = await signedAction('hide_and_keep', 'report', item.id, reason, 0); }
    catch (e) { say(e.message); return false; }
    ws.send(JSON.stringify({ type: 'report_decide', id: item.id, decision: 'hide_and_keep', note, action: signed.submission }));
    if (typeof reportsView !== 'undefined' && reportsView.notes) delete reportsView.notes[String(item.id)];
    if (typeof requestReports === 'function' && typeof reportsView !== 'undefined') requestReports(reportsView.tab);
    return true;
  }

  /** The Reports view was drawn: add Hide and keep (or why not) to each open report about a post,
   *  and, for admins, a Held copies button. */
  function decorateReports(card, model) {
    if (!card || !model || typeof card.querySelectorAll !== 'function') return;
    if (amAdmin()) {
      const open = button('held.open', 'held.open_hint', () => openHeldView());
      open.style.fontSize = '0.75rem';
      const row = make('div', 'held-open-row');
      row.style.margin = 'var(--space-sm) 0';
      row.appendChild(open);
      const first = card.querySelector('.report-item');
      if (first && first.parentNode) first.parentNode.insertBefore(row, first);
      else card.appendChild(row);
    }
    if (model.tab !== 'open') return;
    for (const node of card.querySelectorAll('.report-item')) {
      const item = model.items.find((x) => String(x.id) === String(node.dataset && node.dataset.reportId));
      if (!item || item.context !== 'post') continue;
      const box = make('div', 'held-decide');
      box.style.marginTop = 'var(--space-xs)';
      if (held.keep.can) {
        const b = button('held.decide_label', 'held.decide_hint', () => { b.disabled = true; hideAndKeep(item); });
        b.style.fontSize = '0.75rem';
        b.style.color = 'var(--danger)';
        box.appendChild(b);
      } else {
        box.appendChild(refusalNode());
      }
      node.appendChild(box);
    }
  }

  /** Why the server cannot keep a copy: the relay's sentence as it came, what works instead, and
   *  what to do today when there is no owner. */
  function refusalNode() {
    const box = make('div', 'held-refusal');
    box.style.borderLeft = '3px solid var(--warning)';
    box.style.paddingLeft = 'var(--space-sm)';
    box.style.color = 'var(--text-muted)';
    box.style.fontSize = 'var(--text-sm)';
    box.appendChild(make('div', 'held-refusal-title', t('held.cannot_keep_title')));
    if (held.keep.why) box.appendChild(make('div', 'held-refusal-why', held.keep.why));
    box.appendChild(make('div', 'held-refusal-instead', t('held.instead')));
    // What to do today (chat-owner.js): claim the server when it has no owner, or wait for the
    // owner's key card.
    if (typeof window.ownerRefusalNodes === 'function') for (const n of window.ownerRefusalNodes()) box.appendChild(n);
    return box;
  }

  // ── 2. Held copies ──

  function openHeldView() {
    if (!amAdmin()) { say(t('held.admins_only')); return false; }
    held.open = true;
    render();
    if (!requestList()) held.status = t('held.not_connected');
    render();
    return true;
  }
  function closeHeldView() { held.open = false; render(); }
  function requestList() {
    if (!socketOpen()) return false;
    ws.send(JSON.stringify({ type: 'held_list' }));
    return true;
  }
  function nextList() {
    return new Promise((resolve) => {
      const timer = setTimeout(() => { held.waiters = held.waiters.filter((w) => w !== done); resolve(false); }, ANSWER_WAIT_MS);
      const done = () => { clearTimeout(timer); resolve(true); };
      held.waiters.push(done);
    });
  }
  function setStatus(text) { held.status = text; render(); }

  function reasonOf(item) {
    const r = String(held.reasons[item.id] || '').trim();
    if (!r) throw new Error(t('held.reason_needed'));
    return r;
  }

  /** Send `action` on `item`, signed; resolves once the relay answered with the list (or not). */
  async function act(item, action, until) {
    const signed = await signedAction(action, 'held', item.id, reasonOf(item), until);
    if (!socketOpen()) throw new Error(t('held.not_connected'));
    const answered = nextList();
    ws.send(JSON.stringify({ type: 'held_act', action: signed.submission }));
    await answered;
    return signed;
  }

  async function legalHold(item, days) {
    if (held.busy) return;
    const n = Math.floor(Number(days));
    if (!(n >= 0 && n <= HOLD_MAX_DAYS)) { setStatus(t('held.hold_days_range')); return; }
    held.busy = true;
    try {
      await act(item, 'held_legal_hold', n === 0 ? 0 : Date.now() + n * DAY_MS);
      setStatus(t(n === 0 ? 'held.hold_lift_sent' : 'held.hold_sent', { id: item.id }));
    } catch (e) { setStatus(e.message); }
    held.busy = false;
    render();
  }

  async function destroy(item) {
    held.confirm = null;
    if (held.busy) return;
    held.busy = true;
    try { await act(item, 'held_destroy', 0); setStatus(t('held.destroy_sent', { id: item.id })); }
    catch (e) { setStatus(e.message); }
    held.busy = false;
    render();
  }

  // ── Who asks (2026-11-20, the owner review's finding 1) ──
  //
  // A grant alone fetches nothing: the relay serves a held part only to the key the grant belongs
  // to, so every request to a held part route carries my key, the time and my Dilithium3 signature
  // over the signed request `hum/request/v2\n{this page's server}\n{purpose}\n{time}` (pq.js
  // pqRequestPreimage and pqAudience; the purpose /shared/held-seal.js callerPurpose; the desktop's
  // net/held.rs `Caller`; merged 2026-11-22: until then `purpose\ntime`, a chat line's shape).
  // chat-owner.js makes no held requests since owner changes became desktop-only (2026-11-22).

  let lastSignedAt = 0;
  /** Unix ms, strictly later than the last request's, so a retried request is never a replay. */
  function nextMs() {
    lastSignedAt = Math.max(Date.now(), lastSignedAt + 1);
    return lastSignedAt;
  }

  /** The three headers proving I asked for `purpose`, signed now. */
  async function callerHeaders(purpose) {
    if (typeof myKey !== 'string' || !myKey || typeof myDilithiumSecret === 'undefined' || !myDilithiumSecret || typeof window.pqSignMessage !== 'function'
      || typeof window.pqRequestPreimage !== 'function' || typeof window.pqAudience !== 'function') {
      throw new Error(t('held.need_key'));
    }
    const audience = window.pqAudience();
    if (!audience) throw new Error(t('held.need_key'));
    const { seal } = await mods();
    const ts = nextMs();
    const sig = await window.pqSignMessage(myDilithiumSecret, new TextEncoder().encode(window.pqRequestPreimage(audience, purpose, ts)));
    if (!sig) throw new Error(t('held.need_key'));
    let hex = '';
    for (let i = 0; i < sig.length; i++) hex += sig[i].toString(16).padStart(2, '0');
    return { [seal.CALLER_KEY]: myKey, [seal.CALLER_TIME]: String(ts), [seal.CALLER_SIG]: hex };
  }

  /** A part of `item` under the grant `grant`, still sealed, asked for as me. */
  async function fetchPart(item, part, grant) {
    const { seal } = await mods();
    const headers = await callerHeaders(seal.callerPurpose('part', grant, item.id, part, 0));
    const r = await fetch('/api/held/' + item.id + '/' + part + '?grant=' + grant, { cache: 'no-store', headers });
    if (!r.ok) throw new Error(t('held.not_granted'));
    return new Uint8Array(await r.arrayBuffer());
  }

  /** The most one restore piece carries: the relay's RESTORE_PIECE_MAX, under nginx's 1 MB limit
   *  on /api/. */
  const PIECE_MAX = 768 * 1024;

  /** Send file part `part` of `item` back on a restore, opened here: in pieces of at most
   *  PIECE_MAX, each signed for its own offset, from wherever the relay says it has got to (a 409
   *  names the byte to send from). */
  async function putPart(item, part, grant, restoreKey, bytes) {
    const { seal } = await mods();
    let offset = 0;
    const tries = Math.ceil(bytes.length / PIECE_MAX) * 4 + 8;
    for (let n = 0; n < tries; n++) {
      const piece = bytes.subarray(offset, Math.min(bytes.length, offset + PIECE_MAX));
      const headers = Object.assign({ 'x-held-restore-key': restoreKey }, await callerHeaders(seal.callerPurpose('restore', grant, item.id, part, offset)));
      const r = await fetch('/api/held/' + item.id + '/' + part + '?grant=' + grant + '&offset=' + offset, { method: 'PUT', headers, body: piece });
      const answer = await r.json().catch(() => ({}));
      if (r.status === 409 && Number.isSafeInteger(answer.offset) && answer.offset >= 0 && answer.offset < bytes.length) {
        offset = answer.offset;
        continue;
      }
      if (!r.ok) throw new Error(answer.error || t('held.not_granted'));
      if (answer.done === true) return;
      if (!Number.isSafeInteger(answer.offset) || answer.offset <= offset || answer.offset > bytes.length) throw new Error(t('held.not_granted'));
      offset = answer.offset;
    }
    throw new Error(t('held.not_granted'));
  }

  function kyberSecret() {
    if (typeof myKyberSecret === 'undefined' || !myKyberSecret) throw new Error(t('held.need_key'));
    return myKyberSecret;
  }

  /** Ask where the export goes: the browser's own save chooser when it has one. Called straight
   *  from the click, before any waiting, since a browser lets a page ask only then. */
  async function chooseExportFile(item) {
    const name = 'held-item-' + item.id + '.zip';
    if (typeof window.showSaveFilePicker === 'function') {
      return window.showSaveFilePicker({ suggestedName: name, types: [{ description: t('held.export_file_kind'), accept: { 'application/zip': ['.zip'] } }] });
    }
    return { fallbackName: name };
  }

  async function writeExport(target, bytes) {
    if (target && typeof target.createWritable === 'function') {
      const w = await target.createWritable();
      await w.write(bytes);
      await w.close();
      return target.name || '';
    }
    // No save chooser in this browser: the browser's own download, never drawn on the page.
    const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
    const a = document.createElement('a'); // hint-exempt: never shown, it only starts the browser's own download
    a.href = url;
    a.download = target.fallbackName;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    return target.fallbackName;
  }

  /** Throws the words unless the opened `manifest` is the item the owner chose (its package and
   *  report as the list showed them; /shared/held-seal.js manifestIs, the third review's W7: a relay
   *  could hand over another item's copy under the one clicked; and its post's author and time, the
   *  fourth review's finding 8: one report can hold several posts). Nothing is written or sent then. */
  function chosen(seal, manifest, item) {
    if (seal.manifestIs(manifest, item)) return;
    if (manifest && Number(manifest.report_id) === Number(item.report_id) && String(manifest.package) === String(item.package)) {
      throw new Error(t('held.not_the_post', { id: item.id, report: item.report_id }));
    }
    throw new Error(t('held.not_the_item', { id: item.id, report: item.report_id, got: manifest && manifest.report_id }));
  }

  async function exportItem(item) {
    if (held.busy) return;
    let target;
    try { reasonOf(item); setStatus(t('held.export_web_ask', { id: item.id })); target = await chooseExportFile(item); }
    catch (e) { setStatus(e && e.name === 'AbortError' ? t('held.export_cancelled') : e.message); return; }
    held.busy = true;
    try {
      setStatus(t('held.exporting', { id: item.id }));
      const signed = await act(item, 'held_export', 0);
      const { noble, seal } = await mods();
      const secret = kyberSecret();
      const manifest = await seal.openManifest(noble, secret, item.package, await fetchPart(item, 0, signed.objectId));
      chosen(seal, manifest, item);
      const files = new Map();
      for (const f of manifest.files || []) {
        for (const p of [f.part, f.poster_part]) {
          if (p == null) continue;
          files.set(p, await seal.openPart(noble, secret, seal.partContext(item.package, p), await fetchPart(item, p, signed.objectId)));
        }
      }
      const zip = seal.zipStored(seal.exportEntries(manifest, files, Date.now()));
      const where = await writeExport(target, zip);
      setStatus(t('held.export_done', { id: item.id, path: where }));
    } catch (e) {
      setStatus(t('held.export_failed', { why: e.message }));
    }
    held.busy = false;
    render();
  }

  async function restore(item) {
    held.confirm = null;
    if (held.busy) return;
    held.busy = true;
    try {
      setStatus(t('held.restoring', { id: item.id }));
      const signed = await act(item, 'held_restore', 0);
      const { noble, seal } = await mods();
      const secret = kyberSecret();
      const manifest = await seal.openManifest(noble, secret, item.package, await fetchPart(item, 0, signed.objectId));
      chosen(seal, manifest, item);
      const restoreKey = String(manifest.restore_key || '');
      for (const f of manifest.files || []) {
        for (const p of [f.part, f.poster_part]) {
          if (p == null) continue;
          const plain = await seal.openPart(noble, secret, seal.partContext(item.package, p), await fetchPart(item, p, signed.objectId));
          await putPart(item, p, signed.objectId, restoreKey, plain);
          plain.fill(0);
        }
      }
      const headers = Object.assign({ 'x-held-restore-key': restoreKey }, await callerHeaders(seal.callerPurpose('finish', signed.objectId, item.id, 0, 0)));
      const r = await fetch('/api/held/' + item.id + '/restore?grant=' + signed.objectId, { method: 'POST', headers });
      const answer = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(answer.error || t('held.not_granted'));
      setStatus(answer.words || t('held.restore_done', { id: item.id }));
      requestList();
    } catch (e) {
      setStatus(t('held.restore_failed', { why: e.message }));
    }
    held.busy = false;
    render();
  }

  function itemNode(item, owner) {
    const box = make('div', 'held-item');
    box.style.border = '1px solid var(--border)';
    box.style.borderRadius = 'var(--radius-sm)';
    box.style.padding = 'var(--space-md)';
    box.style.marginBottom = 'var(--space-md)';
    const line = (text, cls) => { const d = make('div', cls || '', text); d.style.color = 'var(--text-muted)'; d.style.fontSize = 'var(--text-sm)'; box.appendChild(d); return d; };
    const head = make('div', 'held-item-title', t('held.item_line', { id: item.id, report: item.report_id }));
    head.style.fontWeight = '600';
    box.appendChild(head);
    // Which post it is: one report can hold several (the fourth review's finding 8).
    line(t('held.post_line', { when: when(item.stamp), name: item.author_name || '' }));
    line(t('held.held_line', { when: when(item.held_at), name: item.held_by_name || '' }));
    line(t('held.expires_line', { when: when(item.deleted_at || item.delete_after) }));
    line(item.legal_hold_until ? t('held.hold_line', { when: when(item.legal_hold_until) }) : t('held.no_hold'));
    line(t('held.size_line', { files: item.file_count, size: size(item.sealed_bytes), exports: item.exported_count }));
    if (!item.sealed_to_owner) line(t('held.not_owner_sealed'), 'held-earlier-owner');
    if (item.sealed_to_successor) line(t('chat_owner.item_sealed_to_successor'), 'held-successor-sealed');
    if (!owner) return box;

    const reason = make('input', 'held-reason');
    reason.type = 'text';
    reason.maxLength = 500;
    reason.placeholder = t('held.reason');
    reason.setAttribute('aria-label', t('held.reason'));
    hint(reason, 'held.reason_hint');
    reason.value = held.reasons[item.id] || '';
    reason.addEventListener('input', () => { held.reasons[item.id] = reason.value; });
    reason.style.width = '100%';
    reason.style.boxSizing = 'border-box';
    reason.style.margin = 'var(--space-sm) 0';
    box.appendChild(reason);

    const row = make('div', 'held-actions');
    row.style.display = 'flex';
    row.style.flexWrap = 'wrap';
    row.style.gap = 'var(--space-xs)';
    row.style.alignItems = 'center';
    const days = make('input', 'held-days');
    days.type = 'number';
    days.min = '0';
    days.max = String(HOLD_MAX_DAYS);
    days.value = String(held.days[item.id] != null ? held.days[item.id] : 90);
    days.setAttribute('aria-label', t('held.hold_days'));
    hint(days, 'held.hold_days_hint');
    days.style.width = '5em';
    days.addEventListener('input', () => { held.days[item.id] = days.value; });
    row.appendChild(make('span', 'held-days-label', t('held.hold_days')));
    row.appendChild(days);
    row.appendChild(button('held.hold_set', 'held.hold_set_hint', () => legalHold(item, days.value)));
    if (item.legal_hold_until) row.appendChild(button('held.hold_lift', 'held.hold_lift_hint', () => legalHold(item, 0)));
    if (item.sealed_to_owner) {
      row.appendChild(button('held.export', 'held.export_hint', () => exportItem(item)));
      row.appendChild(button('held.restore', 'held.restore_hint', () => { held.confirm = { id: item.id, kind: 'restore' }; render(); }));
    }
    const d = button('held.destroy', 'held.destroy_hint', () => { held.confirm = { id: item.id, kind: 'destroy' }; render(); });
    d.style.color = 'var(--danger)';
    row.appendChild(d);
    for (const b of row.querySelectorAll ? row.querySelectorAll('button') : []) b.disabled = held.busy;
    box.appendChild(row);

    if (held.confirm && held.confirm.id === item.id) {
      const q = make('div', 'held-confirm');
      q.setAttribute('role', 'alertdialog');
      q.style.marginTop = 'var(--space-sm)';
      q.style.borderLeft = '3px solid var(--danger)';
      q.style.paddingLeft = 'var(--space-sm)';
      const destroying = held.confirm.kind === 'destroy';
      q.appendChild(make('div', 'held-confirm-words', t(destroying ? 'held.destroy_confirm' : 'held.restore_confirm', { id: item.id })));
      const yes = button(destroying ? 'held.destroy_yes' : 'held.restore_yes', destroying ? 'held.destroy_yes_hint' : 'held.restore_yes_hint', () => {
        try { reasonOf(item); } catch (e) { held.confirm = null; setStatus(e.message); return; }
        if (destroying) destroy(item); else restore(item);
      });
      if (destroying) yes.style.color = 'var(--danger)';
      q.appendChild(yes);
      q.appendChild(button('held.cancel', 'held.cancel_hint', () => { held.confirm = null; render(); }));
      box.appendChild(q);
    }
    return box;
  }

  function viewNodes() {
    const nodes = [];
    const top = make('div', 'held-top');
    top.style.display = 'flex';
    top.style.justifyContent = 'space-between';
    top.style.alignItems = 'center';
    top.appendChild(make('h2', 'held-title', t('held.title')));
    top.appendChild(button('held.close', 'held.close_hint', () => closeHeldView()));
    nodes.push(top);
    const intro = make('p', 'held-intro', t('held.intro'));
    intro.style.color = 'var(--text-muted)';
    intro.style.fontSize = 'var(--text-sm)';
    nodes.push(intro);
    const bar = make('div', 'held-bar');
    bar.style.display = 'flex';
    bar.style.gap = 'var(--space-sm)';
    bar.style.alignItems = 'center';
    bar.appendChild(button('held.refresh', 'held.refresh_hint', () => { if (!requestList()) setStatus(t('held.not_connected')); }));
    if (held.status) bar.appendChild(make('span', 'held-status', held.status));
    nodes.push(bar);
    const d = held.data;
    if (!d) { nodes.push(make('p', 'held-asking', t('held.asking'))); return nodes; }
    if (!d.can_keep) {
      held.keep = { can: false, why: d.cannot_keep_why || '' };
      nodes.push(refusalNode());
    }
    const items = Array.isArray(d.items) ? d.items : [];
    const c = d.counts || {};
    const counted = (typeof i18n !== 'undefined' && i18n && typeof i18n.tn === 'function') ? i18n.tn('held.count', items.length) : String(items.length);
    nodes.push(make('p', 'held-count', counted));
    nodes.push(make('p', 'held-counts', t('held.counts_line', { held: c.held || 0, exported: c.exported || 0, holds: c.legal_holds || 0, destroyed: c.destroyed || 0, restored: c.restored || 0 })));
    // Hides whose copy is not sealed yet, said in words (the fourth review's finding 5a).
    if (c.waiting > 0) nodes.push(make('p', 'held-waiting', t('held.waiting_line', { n: c.waiting })));
    if (c.unidentified > 0) nodes.push(make('p', 'held-unidentified', t('held.unidentified_line', { n: c.unidentified })));
    if (!d.i_am_owner) nodes.push(make('p', 'held-owner-only', t('held.owner_only')));
    if (!items.length) nodes.push(make('p', 'held-none', t('held.none')));
    for (const item of items) nodes.push(itemNode(item, !!d.i_am_owner));
    nodes.push(make('h3', 'held-log-title', t('held.log_title')));
    const actions = Array.isArray(d.actions) ? d.actions : [];
    if (!actions.length) nodes.push(make('p', 'held-log-empty', t('held.log_empty')));
    for (const a of actions) {
      const name = t('held.action_' + a.action);
      const row = make('div', 'held-log-line', t('held.log_line', { when: when(a.signed_at), name: a.signer_name || '', action: name, target: a.target, reason: a.reason || '' }));
      row.style.fontSize = 'var(--text-sm)';
      nodes.push(row);
    }
    // The server owner (chat-owner.js): the successor and the owner log for every admin, Claim
    // ownership while there is no owner, and the owner's Hand over and Successor controls.
    if (typeof window.ownerHeldNodes === 'function') for (const n of window.ownerHeldNodes(d)) nodes.push(n);
    return nodes;
  }

  function render() {
    if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
    let overlay = document.getElementById('held-overlay');
    if (!held.open) {
      if (overlay && overlay.classList) overlay.classList.remove('open');
      return;
    }
    let card = document.getElementById('held-card');
    if (!overlay) {
      overlay = make('div', 'profile-modal-overlay');
      overlay.id = 'held-overlay';
      overlay.style.zIndex = 'var(--z-shade)';
      overlay.addEventListener('click', (e) => { if (e && e.target === overlay) closeHeldView(); });
      card = make('div', 'profile-modal');
      card.id = 'held-card';
      card.setAttribute('role', 'dialog');
      card.setAttribute('aria-label', t('held.title'));
      card.addEventListener('click', (e) => { if (e && e.stopPropagation) e.stopPropagation(); });
      overlay.appendChild(card);
      if (document.body) document.body.appendChild(overlay);
    }
    if (overlay.classList) overlay.classList.add('open');
    if (!card) return;
    while (card.firstChild) card.removeChild(card.firstChild);
    for (const n of viewNodes()) card.appendChild(n);
  }

  // The relay's frames for this file: the held items list, and whether keeping works (on the
  // Reports list). Everything else goes on down the chain.
  const _orig = handleMessage;
  handleMessage = function (msg) {
    if (msg && msg.type === 'held_items') {
      held.data = msg;
      held.keep = { can: !!msg.can_keep, why: msg.cannot_keep_why || '' };
      const waiting = held.waiters;
      held.waiters = [];
      for (const w of waiting) w();
      render();
      return;
    }
    if (msg && msg.type === 'reports' && typeof msg.can_keep === 'boolean') {
      held.keep = { can: msg.can_keep, why: msg.cannot_keep_why || '' };
    }
    return _orig(msg);
  };

  window.openHeldView = openHeldView;
  window.closeHeldView = closeHeldView;
  window.heldDecorateReports = decorateReports;
  window.heldHideAndKeep = hideAndKeep;
  // chat-owner.js draws its part of the view and asks for a fresh drawing when its state changes.
  window.heldRender = render;
  // chat-owner.js signs its requests to the held part routes the same way (finding 1).
  window.heldCallerHeaders = callerHeaders;
  // For scripts/tests/held-chat-web.test.js: the state, read only by the test.
  window.__heldState = held;
})();
