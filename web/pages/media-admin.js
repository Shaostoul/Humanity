// ── media-admin.js ────────────────────────────────────────────────────────
// The admins' Media section on the website (2026-10-11; docs/design/
// media-showcase.md, "Increment 2, exact", "Admins"). The desktop app keeps it
// in Server Settings; the website has no Server Settings page, so it lives on
// the Admin dashboard (/admin, admin.html) and, for admins, on the Media page
// itself. One section, mounted in both places by `mountMediaAdmin`.
//
// It holds:
// - Who sees the Media page (Public / Members only), with the relay's own
//   sentence about what a member is under it (the settings' `media_members_note`);
//   the Media budget and each person's share in MB; "Public visitors get
//   previews only". These four are server settings: they are read from
//   `server_settings_state` and saved with `server_settings_update` over the
//   page's signed-in socket (/shared/relay-session.js), the frame the relay's
//   handlers/server_settings_update.rs takes from admins.
// - The categories: add, rename, reorder, remove (General stays). Saved whole
//   with POST /api/media/categories, signed over
//   media_categories:<BLAKE3 hex of the category list's JSON text as sent>, so
//   the request carries exactly that text (`categoriesBody`).
// - Suggestions waiting: Accept or Decline, the suggester named.
// - The meter ("12.4 GB of 50 GB used") with a bar, and the prune lists
//   (largest, oldest, least viewed, waiting to be published), each with Remove.
//
// Built with createElement and textContent only. A classic script in the
// browser (after /pages/media-app.js, whose helpers it uses), a CommonJS module
// under Node (scripts/tests/media-web.test.js).
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const M = typeof module === 'object' && module && module.exports ? require('./media-app.js') : root.MediaPage;

  // General: the relay refuses a category list without it ("General (category 1)
  // cannot be removed", storage/media.rs GENERAL_CATEGORY_ID); it can be renamed.
  const GENERAL_ID = 1;

  const ADMIN_WORDS = Object.freeze({
    noSession: 'This page is not signed in to this server yet, so these settings cannot be read or saved here.',
    waitingSettings: "Waiting for this server's settings...",
    saved: 'Saved.',
    notSaved: 'This page is not signed in to this server, so nothing was saved.',
    generalStays: 'General stays: it holds the items of removed categories.',
    noSuggestions: 'No suggestions are waiting.',
    removeWaiting: 'Remove this upload and delete the file.',
  });

  function hex(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return s;
  }

  /** The category list's JSON text, as it is sent and hashed: existing ones by id, new ones without. */
  function categoriesListText(draft) {
    return JSON.stringify(draft.map((c) => {
      const name = String(c.name || '').trim();
      const description = String(c.description || '').trim();
      return c.id != null ? { id: c.id, name, description } : { name, description };
    }));
  }

  /** The request body with the list's text in it unchanged (the relay hashes that member as sent). */
  function categoriesBody(auth, listText) {
    return '{"key":' + JSON.stringify(auth.key) + ',"timestamp":' + Number(auth.timestamp) +
      ',"sig":' + JSON.stringify(auth.sig) + ',"categories":' + listText + '}';
  }

  /** media_categories:<BLAKE3 hex of the list text>, or null when it cannot be hashed. */
  async function categoriesPurpose(listText, blake3) {
    if (typeof blake3 !== 'function') return null;
    const digest = await blake3(new TextEncoder().encode(listText));
    if (!digest || digest.length !== 32) return null;
    return M.purposes.categories(hex(digest));
  }

  /** "Deleted at <time> unless it is published." for an upload waiting to be published. */
  function expiresWords(ms) {
    const n = Number(ms);
    if (!Number.isFinite(n) || n <= 0) return '';
    let when = '';
    try { when = new Date(n).toLocaleString(); } catch (e) { when = ''; }
    return when ? 'Deleted ' + when + ' unless it is published.' : '';
  }

  /**
   * Mount the section in `container`. `env`: document, fetch, sign(purpose),
   * blake3(bytes), session (a relay-session), confirm(words), onChange() (the
   * Media page reloads its list after a change here; optional).
   */
  function mountMediaAdmin(container, env) {
    const h = M.makeH(env.document);
    const session = env.session || null;
    const st = {
      settingsDraft: null,
      settingsDirty: false,
      saving: false,
      settingsMsg: '',
      settingsWarn: false,
      categories: null,
      draft: null,
      catMsg: '',
      catWarn: false,
      catBusy: false,
      suggestions: null,
      sugMsg: '',
      usage: null,
      usageMsg: '',
    };
    const els = {
      settings: h('div', { id: 'media-admin-settings', className: 'media-admin-block' }),
      categories: h('div', { id: 'media-admin-categories', className: 'media-admin-block' }),
      suggestions: h('div', { id: 'media-admin-suggestions', className: 'media-admin-block' }),
      usage: h('div', { id: 'media-admin-usage', className: 'media-admin-block' }),
    };
    container.replaceChildren(els.settings, els.categories, els.suggestions, els.usage);

    function changed() {
      if (typeof env.onChange === 'function') env.onChange();
    }

    async function signedPost(url, purpose, body) {
      const auth = await env.sign(purpose);
      if (!auth) throw new Error(M.MEDIA_WORDS.cannotSign);
      return M.answerOf(await env.fetch(url, M.jsonPost(Object.assign({}, body || {}, auth))));
    }

    function msg(id, text, warn) {
      return text ? h('p', { id, className: 'media-hint' + (warn ? ' media-warn' : ''), text }) : null;
    }

    // ── The four settings ──
    function settingsNow() {
      return session && session.state ? session.state.settings : null;
    }
    function draftFrom(s) {
      return {
        media_visibility: s.media_visibility === 'members' ? 'members' : 'public',
        media_budget_mb: Number(s.media_budget_mb) || 1,
        media_per_person_mb: Number(s.media_per_person_mb) || 1,
        media_public_previews_only: s.media_public_previews_only !== false,
      };
    }
    function renderSettings() {
      els.settings.replaceChildren(h('h3', { text: 'Who sees the Media page' }));
      const s = settingsNow();
      if (!session || !session.state || session.state.status !== 'bound') {
        els.settings.appendChild(h('p', { className: 'media-hint', text: ADMIN_WORDS.noSession }));
        if (!s) return;
      }
      if (!s) {
        els.settings.appendChild(h('p', { className: 'media-hint', text: ADMIN_WORDS.waitingSettings }));
        return;
      }
      if (!st.settingsDirty || !st.settingsDraft) st.settingsDraft = draftFrom(s);
      const d = st.settingsDraft;
      const dirty = () => { st.settingsDirty = true; st.settingsMsg = ''; };
      const pub = h('input', { type: 'radio', name: 'media-admin-visibility', value: 'public', id: 'media-admin-vis-public', checked: d.media_visibility === 'public' });
      const mem = h('input', { type: 'radio', name: 'media-admin-visibility', value: 'members', id: 'media-admin-vis-members', checked: d.media_visibility === 'members' });
      pub.addEventListener('change', () => { if (pub.checked) { d.media_visibility = 'public'; dirty(); } });
      mem.addEventListener('change', () => { if (mem.checked) { d.media_visibility = 'members'; dirty(); } });
      const budget = h('input', { type: 'number', id: 'media-admin-budget', className: 'media-input media-input-number', min: 1, value: String(d.media_budget_mb) });
      budget.addEventListener('input', () => { d.media_budget_mb = Math.round(Number(budget.value)); dirty(); });
      const share = h('input', { type: 'number', id: 'media-admin-share', className: 'media-input media-input-number', min: 1, value: String(d.media_per_person_mb) });
      share.addEventListener('input', () => { d.media_per_person_mb = Math.round(Number(share.value)); dirty(); });
      const previews = h('input', { type: 'checkbox', id: 'media-admin-previews', checked: !!d.media_public_previews_only });
      previews.addEventListener('change', () => { d.media_public_previews_only = !!previews.checked; dirty(); });
      const save = h('button', { type: 'button', id: 'media-admin-save', className: 'media-btn media-btn-primary', text: st.saving ? 'Saving...' : 'Save', disabled: st.saving });
      save.addEventListener('click', saveSettings);
      // (A missing line is left out: append() would write a null as the word "null".)
      els.settings.append(...[
        h('div', { className: 'media-field' }, [
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]),
        typeof s.media_members_note === 'string' && s.media_members_note
          ? h('p', { id: 'media-admin-members-note', className: 'media-hint', text: s.media_members_note })
          : null,
        h('label', { className: 'media-field' }, ['Media budget (MB)', budget]),
        h('label', { className: 'media-field' }, ["Each person's share (MB)", share]),
        h('label', { className: 'media-check' }, [previews, ' Public visitors get previews only']),
        h('div', { className: 'media-form-buttons' }, [save]),
      ].filter(Boolean));
      const m = msg('media-admin-settings-msg', st.settingsMsg, st.settingsWarn);
      if (m) els.settings.appendChild(m);
    }
    function saveSettings() {
      const d = st.settingsDraft;
      if (!d) return;
      const frame = {
        type: 'server_settings_update',
        media_visibility: d.media_visibility,
        media_budget_mb: Math.max(1, Number(d.media_budget_mb) || 1),
        media_per_person_mb: Math.max(1, Number(d.media_per_person_mb) || 1),
        media_public_previews_only: !!d.media_public_previews_only,
      };
      if (!session || !session.send(frame)) {
        st.settingsMsg = ADMIN_WORDS.notSaved;
        st.settingsWarn = true;
      } else {
        st.saving = true;
        st.settingsMsg = '';
        st.settingsWarn = false;
      }
      renderSettings();
    }
    function onSession(ev) {
      if (ev.type === 'frame' && ev.msg && ev.msg.type === 'server_settings_state') {
        if (st.saving) {
          st.saving = false;
          st.settingsDirty = false;
          st.settingsMsg = ADMIN_WORDS.saved;
          st.settingsWarn = false;
          changed();
        }
        renderSettings();
      } else if (ev.type === 'frame' && ev.msg && ev.msg.type === 'system' && st.saving) {
        const text = String(ev.msg.message || '');
        if (/server settings/i.test(text) && !/updated by admin/i.test(text)) {
          st.saving = false;
          st.settingsMsg = text;
          st.settingsWarn = true;
          renderSettings();
        }
      } else if (ev.type === 'state') {
        renderSettings();
      }
    }

    // ── Categories ──
    async function loadCategories() {
      try {
        const a = await signedPost('/api/media/list', M.purposes.list(), { limit: 1 });
        st.categories = Array.isArray(a.categories) ? a.categories : [];
        st.draft = st.categories.map((c) => ({ id: c.id, name: c.name, description: c.description || '' }));
      } catch (e) {
        st.catMsg = 'The categories could not be loaded: ' + e.message;
        st.catWarn = true;
      }
      renderCategories();
    }
    function move(i, by) {
      const j = i + by;
      if (!st.draft || j < 0 || j >= st.draft.length) return;
      const t = st.draft[i];
      st.draft[i] = st.draft[j];
      st.draft[j] = t;
      st.catMsg = '';
      renderCategories();
    }
    function renderCategories() {
      els.categories.replaceChildren(h('h3', { text: 'Categories' }));
      if (!st.draft) {
        els.categories.appendChild(msg('media-admin-cat-msg', st.catMsg || M.MEDIA_WORDS.loading, st.catWarn));
        return;
      }
      const list = h('div', { className: 'media-admin-rows', id: 'media-admin-cat-rows' });
      st.draft.forEach((c, i) => {
        const name = h('input', { type: 'text', className: 'media-input', maxLength: 60, value: c.name, dataset: { catName: i }, attrs: { 'aria-label': 'Category name' } });
        name.addEventListener('input', () => { c.name = name.value; });
        const desc = h('input', { type: 'text', className: 'media-input', maxLength: 300, value: c.description || '', placeholder: 'Description (optional)', attrs: { 'aria-label': 'Category description' } });
        desc.addEventListener('input', () => { c.description = desc.value; });
        const general = c.id === GENERAL_ID;
        list.appendChild(h('div', { className: 'media-admin-row', dataset: { catRow: i } }, [
          name, desc,
          h('button', { type: 'button', className: 'media-btn media-btn-small', text: 'Up', disabled: i === 0, dataset: { catUp: i }, on: { click: () => move(i, -1) } }),
          h('button', { type: 'button', className: 'media-btn media-btn-small', text: 'Down', disabled: i === st.draft.length - 1, dataset: { catDown: i }, on: { click: () => move(i, 1) } }),
          general
            ? h('span', { className: 'media-hint', text: ADMIN_WORDS.generalStays })
            : h('button', {
              type: 'button', className: 'media-btn media-btn-small media-btn-danger', text: 'Remove', dataset: { catRemove: i },
              on: { click: () => { st.draft.splice(i, 1); st.catMsg = 'Its items move to General when you save.'; st.catWarn = false; renderCategories(); } },
            }),
        ]));
      });
      const newName = h('input', { type: 'text', id: 'media-admin-cat-new-name', className: 'media-input', maxLength: 60, placeholder: 'New category' });
      const newDesc = h('input', { type: 'text', id: 'media-admin-cat-new-desc', className: 'media-input', maxLength: 300, placeholder: 'Description (optional)' });
      const add = h('button', { type: 'button', id: 'media-admin-cat-add', className: 'media-btn', text: 'Add' });
      add.addEventListener('click', () => {
        const n = newName.value.trim();
        if (!n) return;
        st.draft.push({ name: n, description: newDesc.value.trim() });
        st.catMsg = '';
        renderCategories();
      });
      const save = h('button', { type: 'button', id: 'media-admin-cat-save', className: 'media-btn media-btn-primary', text: st.catBusy ? 'Saving...' : 'Save categories', disabled: st.catBusy });
      save.addEventListener('click', saveCategories);
      els.categories.append(list, h('div', { className: 'media-admin-row' }, [newName, newDesc, add]), h('div', { className: 'media-form-buttons' }, [save]));
      const m = msg('media-admin-cat-msg', st.catMsg, st.catWarn);
      if (m) els.categories.appendChild(m);
    }
    async function saveCategories() {
      if (!st.draft || st.catBusy) return;
      st.catBusy = true;
      renderCategories();
      try {
        const listText = categoriesListText(st.draft);
        const purpose = await categoriesPurpose(listText, env.blake3);
        if (!purpose) throw new Error('The category list could not be hashed for its signature. Reload the page and try again.');
        const auth = await env.sign(purpose);
        if (!auth) throw new Error(M.MEDIA_WORDS.cannotSign);
        const a = await M.answerOf(await env.fetch('/api/media/categories', M.jsonPost(categoriesBody(auth, listText))));
        st.categories = a.categories || [];
        st.draft = st.categories.map((c) => ({ id: c.id, name: c.name, description: c.description || '' }));
        st.catMsg = ADMIN_WORDS.saved;
        st.catWarn = false;
        changed();
      } catch (e) {
        st.catMsg = e.message;
        st.catWarn = true;
      }
      st.catBusy = false;
      renderCategories();
    }

    // ── Suggestions ──
    async function loadSuggestions() {
      try {
        const a = await signedPost('/api/media/suggestions', M.purposes.suggestions());
        st.suggestions = (a.suggestions || []).filter((s) => s.state === 'open');
        st.sugMsg = '';
      } catch (e) {
        st.suggestions = st.suggestions || [];
        st.sugMsg = 'The suggestions could not be loaded: ' + e.message;
      }
      renderSuggestions();
    }
    async function decide(s, accept) {
      try {
        await signedPost('/api/media/suggestions/decide', M.purposes.decide(s.id, accept), { id: s.id, accept });
        st.sugMsg = (accept ? 'Accepted: "' + s.name + '" is a category now.' : 'Declined.') + ' ' + (s.suggester_name || 'The suggester') + ' is told.';
        if (accept) { loadCategories(); changed(); }
      } catch (e) {
        st.sugMsg = e.message;
      }
      loadSuggestions();
    }
    function renderSuggestions() {
      els.suggestions.replaceChildren(h('h3', { text: 'Suggestions waiting' }));
      if (!st.suggestions) {
        els.suggestions.appendChild(h('p', { className: 'media-hint', text: M.MEDIA_WORDS.loading }));
        return;
      }
      if (!st.suggestions.length) els.suggestions.appendChild(h('p', { className: 'media-hint', id: 'media-admin-no-suggestions', text: ADMIN_WORDS.noSuggestions }));
      for (const s of st.suggestions) {
        els.suggestions.appendChild(h('div', { className: 'media-admin-row', dataset: { suggestion: s.id } }, [
          h('div', { className: 'media-admin-row-text' }, [
            h('strong', { text: s.name }),
            h('div', { className: 'media-hint', text: 'Suggested by ' + (s.suggester_name || 'someone without a name') }),
            s.note ? h('div', { className: 'media-hint', text: s.note }) : null,
          ]),
          h('button', { type: 'button', className: 'media-btn media-btn-small', text: 'Accept', dataset: { decide: 'accept' }, on: { click: () => decide(s, true) } }),
          h('button', { type: 'button', className: 'media-btn media-btn-small', text: 'Decline', dataset: { decide: 'decline' }, on: { click: () => decide(s, false) } }),
        ]));
      }
      const m = msg('media-admin-sug-msg', st.sugMsg, false);
      if (m) els.suggestions.appendChild(m);
    }

    // ── The meter and the prune lists ──
    async function loadUsage() {
      try {
        st.usage = await signedPost('/api/media/usage', M.purposes.usage());
        st.usageMsg = st.usageMsg || '';
      } catch (e) {
        st.usageMsg = 'How the space is used could not be loaded: ' + e.message;
      }
      renderUsage();
    }
    async function removeFromPrune(target) {
      const words = target.item ? M.removeWords(target.item) : ADMIN_WORDS.removeWaiting;
      const name = target.item ? M.itemName(target.item) : target.waiting.filename;
      if (!await env.confirm('"' + name + '"\n\n' + words)) return;
      try {
        const a = target.item
          ? await signedPost('/api/media/remove', M.purposes.remove(target.item.id), { id: target.item.id })
          : await signedPost('/api/media/remove', M.purposes.remove(target.waiting.filename), { filename: target.waiting.filename });
        st.usageMsg = target.item ? M.removedWords(a) : 'Removed, and the file was deleted.';
        changed();
      } catch (e) {
        st.usageMsg = e.message;
      }
      loadUsage();
    }
    function pruneList(title, id, rows) {
      return h('div', { className: 'media-admin-prune', id }, [
        h('h4', { text: title }),
        rows.length ? h('div', { className: 'media-admin-rows' }, rows) : h('p', { className: 'media-hint', text: 'Nothing here.' }),
      ]);
    }
    function renderUsage() {
      els.usage.replaceChildren(h('h3', { text: 'Space' }));
      const u = st.usage;
      if (!u) {
        els.usage.appendChild(msg('media-admin-usage-msg', st.usageMsg || M.MEDIA_WORDS.loading, !!st.usageMsg));
        return;
      }
      const used = Math.max(0, Number(u.used_bytes) || 0);
      const budget = Math.max(1, Number(u.budget_bytes) || 1);
      const pct = Math.min(100, Math.round((used / budget) * 1000) / 10);
      const fill = h('div', { className: 'media-meter-fill' });
      fill.style.width = pct + '%';
      els.usage.append(
        h('p', { id: 'media-admin-meter', className: 'media-meter-words', text: M.meterWords(used, budget) }),
        h('div', { className: 'media-meter', id: 'media-admin-meter-bar', attrs: { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': pct } }, [fill]),
      );
      const itemRow = (it) => h('div', { className: 'media-admin-row', dataset: { pruneItem: it.id } }, [
        h('div', { className: 'media-admin-row-text', text: M.itemName(it) + ' · ' + M.sizeWords(it.size_bytes) + ' · by ' + (it.uploader_name || 'someone') }),
        h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', text: 'Remove', on: { click: () => removeFromPrune({ item: it }) } }),
      ]);
      const waitingRow = (w) => h('div', { className: 'media-admin-row', dataset: { pruneWaiting: w.filename } }, [
        h('div', { className: 'media-admin-row-text', text: [w.filename, M.sizeWords(w.size_bytes), 'by ' + (w.uploader_name || 'someone'), expiresWords(w.expires_at)].filter(Boolean).join(' · ') }),
        h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', text: 'Remove', on: { click: () => removeFromPrune({ waiting: w }) } }),
      ]);
      els.usage.append(
        pruneList('Largest', 'media-admin-largest', (u.largest || []).map(itemRow)),
        pruneList('Oldest', 'media-admin-oldest', (u.oldest || []).map(itemRow)),
        pruneList('Least viewed', 'media-admin-least-viewed', (u.least_viewed || []).map(itemRow)),
        pruneList('Waiting to be published', 'media-admin-waiting', (u.waiting || []).map(waitingRow)),
        h('div', { className: 'media-form-buttons' }, [
          h('button', { type: 'button', className: 'media-btn', id: 'media-admin-refresh', text: 'Refresh', on: { click: () => { st.usageMsg = ''; loadUsage(); loadSuggestions(); } } }),
        ]),
      );
      const m = msg('media-admin-usage-msg', st.usageMsg, false);
      if (m) els.usage.appendChild(m);
    }

    if (session) session.subscribe(onSession);
    renderSettings();
    renderCategories();
    renderSuggestions();
    renderUsage();
    const ready = Promise.all([loadCategories(), loadSuggestions(), loadUsage()]);
    return { state: st, ready, reload: () => Promise.all([loadCategories(), loadSuggestions(), loadUsage()]) };
  }

  /** On the Admin dashboard: called by admin-app.js once the dashboard shows (the person is an admin). */
  let startedOnAdminPage = false;
  async function startMediaAdmin() {
    if (startedOnAdminPage) return;
    const doc = root.document;
    const container = doc && doc.getElementById('media-admin-root');
    if (!container) return;
    startedOnAdminPage = true;
    let identity = null;
    try { identity = typeof root.getPqIdentity === 'function' ? await root.getPqIdentity() : null; } catch (e) { identity = null; }
    const session = identity && typeof root.createRelaySession === 'function'
      ? root.createRelaySession({
        getIdentity: async () => identity,
        signBytes: (secret, bytes) => root.pqSignMessage(secret, bytes),
        WebSocket: root.WebSocket,
        location: root.location,
        storage: root.localStorage,
      })
      : null;
    if (session) session.start();
    mountMediaAdmin(container, {
      document: doc,
      fetch: (u, i) => root.fetch(u, i),
      sign: (purpose) => (typeof root.getPqSignedAuth === 'function' ? root.getPqSignedAuth(purpose) : Promise.resolve(null)),
      blake3: (bytes) => root.pqBlake3(bytes),
      confirm: (words) => (typeof root.holdConfirm === 'function'
        ? root.holdConfirm(words, { seconds: 2, confirmLabel: 'Hold to remove' })
        : Promise.resolve(root.confirm(words))),
      session,
    });
  }

  const api = { GENERAL_ID, ADMIN_WORDS, categoriesListText, categoriesBody, categoriesPurpose, expiresWords, mountMediaAdmin, startMediaAdmin };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, { mountMediaAdmin, startMediaAdmin, MediaAdmin: api });
})(typeof globalThis !== 'undefined' ? globalThis : this);
