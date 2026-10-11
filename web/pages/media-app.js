// ── media-app.js ──────────────────────────────────────────────────────────
// The Media page on the website (2026-10-11; docs/design/media-showcase.md,
// "Increment 2, exact"). The desktop app's Media page is the original
// (src/gui/pages/media.rs); this page mirrors its layout and words.
//
// What it shows: a strip of featured items, filters (category, kind, person,
// My picks, a search over titles), a grid of tiles newest first with More, and
// a viewer. What it does: Pick and Unpick, Remove (with the words naming what
// happens), Feature and Unfeature for admins, Add to Media (the file picker,
// then a short form), Put on Media for a Shared Files row (public only), and
// Suggest a category. The relay half is src/relay/media.rs.
//
// RULES THIS FILE KEEPS
// - Every element is built with createElement and textContent: nothing the
//   server sends is ever read as HTML.
// - A picture or video loads only from this server's own addresses: its
//   /uploads/ folder and the members' file route /api/media/file/{id}
//   (`ownFilePath`). Anything else is not loaded and says so.
// - A members-only item's file is fetched through /api/media/file/{id} with
//   this page's session upload token (`?token=`), which the relay gives only
//   to a socket it has signed in (/shared/relay-session.js).
// - A labelled item (spoiler, flashing lights, graphic) shows its label
//   instead of its picture until clicked: its picture is not even requested.
// - Every write is signed (Dilithium over "purpose\ntimestamp", getPqSignedAuth
//   in /shared/pq-relay-auth.js) with a purpose that names its target:
//   media_publish:<file name>, media_remove:<item id>, media_pick:<item id>,
//   media_feature:<item id> (the relay's F5). The members' list read signs
//   plain media_list; a category suggestion plain media_suggest.
// - The kinds and labels come from the server's answers: the label list is
//   the list answer's `labels`, and the kind filter offers the kinds the
//   server's items carry. KIND_WORDS below only says each kind in words.
//
// A classic script in the browser, a CommonJS module under Node, where
// scripts/tests/media-web.test.js runs the page against a stand-in document
// that has only what this file built in it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MIB = 1024 * 1024;
  const PAGE_SIZE = 48;
  // The most own items read to add up how much of the share is used (100 a page).
  const MY_USE_PAGES = 50;
  // The most pages of the person's picks read to know which tiles they picked.
  const MY_PICKS_PAGES = 10;

  // ── Words (the design's own, shared with the desktop page) ──
  const MEDIA_WORDS = Object.freeze({
    lede: 'Pictures, videos, 3D models and files people on this server chose to share.',
    membersLine: "Sign in to this server to see its members' media.",
    membersOnlyServer: "This server's Media page is for its members.",
    alreadyPublic: 'This file is already public where it was shared.',
    serverMembersOnly: "This server's Media page is for its members, so this goes on it as members only.",
    removeKeeps: 'Remove from Media. The file stays where it came from.',
    removeDeletes: 'Remove from Media and delete the file.',
    removeEither: 'Remove from Media. A file shared in a chat or in Shared Files stays there; a file uploaded on this page is deleted.',
    removedKept: 'Removed from Media. The file stays where it came from.',
    removedDeleted: 'Removed from Media, and the file was deleted.',
    removed: 'Removed from Media.',
    empty: 'Nothing is on the Media page yet.',
    emptyFiltered: 'Nothing here matches these filters.',
    notThisServer: 'Not shown: this file is not on this server.',
    waitingToken: 'This members-only file shows once this page is signed in to the server.',
    noIdentity: 'Not signed in on this page. Open Chat in this browser first (it keeps your identity here), then press Try again. A passphrase-protected identity cannot be read by this page yet.',
    signingIn: 'Signing in to this server...',
    cannotSign: 'This page could not sign the request with your identity. Open Chat in this browser first, then try again.',
    notSignedInUpload: 'This page is not signed in to this server yet, so nothing was uploaded. Try again in a moment.',
    picksPublicOnly: 'Only public items show under My picks on the website for now.',
    published: 'It is on the Media page now.',
    suggestSent: 'Suggestion sent. You are told here, and in Chat, when an admin decides.',
    clickToShow: 'Click to show',
    loading: 'Loading...',
  });

  // Each kind in words: the relay's kind ids (storage/media.rs `kind_of`) as one and as many.
  const KIND_WORDS = Object.freeze({
    picture: Object.freeze({ one: 'Picture', many: 'Pictures' }),
    video: Object.freeze({ one: 'Video', many: 'Videos' }),
    model: Object.freeze({ one: '3D model', many: 'Models' }),
    file: Object.freeze({ one: 'File', many: 'Files' }),
  });

  /** A kind in words; a kind this page has no words for is shown as the server named it. */
  function kindWords(kind) {
    return KIND_WORDS[kind] || { one: String(kind || ''), many: String(kind || '') };
  }

  /** The kinds in the order the filter offers them: the ones with words first, in that order. */
  function sortKinds(kinds) {
    const known = Object.keys(KIND_WORDS);
    const rank = (k) => (known.indexOf(k) === -1 ? known.length : known.indexOf(k));
    return Array.from(new Set(kinds)).sort((a, b) => rank(a) - rank(b) || String(a).localeCompare(String(b)));
  }

  function oneDecimal(x) {
    return x.toFixed(1).replace(/\.0$/, '');
  }

  /** Bytes as people read them: "412 MB", "12.4 GB", "50 GB", "900 KB". */
  function sizeWords(bytes) {
    const b = Math.max(0, Number(bytes) || 0);
    if (b >= 1024 * MIB) return oneDecimal(b / (1024 * MIB)) + ' GB';
    if (b >= MIB) return (b / MIB >= 100 ? String(Math.round(b / MIB)) : oneDecimal(b / MIB)) + ' MB';
    if (b >= 1024) return Math.round(b / 1024) + ' KB';
    return b + (b === 1 ? ' byte' : ' bytes');
  }

  /** "You have 412 MB of your 512 MB left". */
  function shareLeftWords(usedBytes, shareBytes) {
    const left = Math.max(0, shareBytes - usedBytes);
    return 'You have ' + sizeWords(left) + ' of your ' + sizeWords(shareBytes) + ' left';
  }

  /** The stop when a file is bigger than what is left of the person's share, else null. */
  function shareStop(file, usedBytes, shareBytes) {
    if (!file || !Number.isFinite(shareBytes) || shareBytes <= 0 || !Number.isFinite(usedBytes)) return null;
    const left = Math.max(0, shareBytes - usedBytes);
    if (file.size <= left) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', more than the ' + sizeWords(left) +
      ' you have left of your ' + sizeWords(shareBytes) + ' share, so it was not uploaded. Remove one of your items, or ask an admin to raise each person\'s share.';
  }

  /** The stop when a file is over the person's upload limit on this server, else null. */
  function limitStop(file, limitBytes) {
    if (!file || !Number.isFinite(limitBytes) || limitBytes <= 0 || file.size <= limitBytes) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', over your ' + sizeWords(limitBytes) +
      ' limit for one file on this server, so it was not uploaded. An admin can raise it in Server Settings > Roles.';
  }

  /** The admins' meter: "12.4 GB of 50 GB used". */
  function meterWords(usedBytes, budgetBytes) {
    return sizeWords(usedBytes) + ' of ' + sizeWords(budgetBytes) + ' used';
  }

  /**
   * The confirmation for Remove, naming what happens to the file. Only a file
   * uploaded for Media can be members only (the relay's `was_public`), so a
   * members-only item's file is deleted. A public item's answer does not say
   * where its file came from, so its words cover both.
   */
  function removeWords(item) {
    if (item && item.visibility === 'members') return MEDIA_WORDS.removeDeletes;
    return MEDIA_WORDS.removeEither;
  }

  /** What the relay's answer to a removal says happened. */
  function removedWords(answer) {
    if (answer && answer.file_kept === true) return MEDIA_WORDS.removedKept;
    if (answer && answer.file_removed === true) return MEDIA_WORDS.removedDeleted;
    return MEDIA_WORDS.removed;
  }

  // ── Addresses ──

  /**
   * The path of a file on this server's own addresses, or null: /uploads/<name>
   * or /api/media/file/<id>, on `origin`. Nothing from anywhere else is ever
   * loaded on this page.
   */
  function ownFilePath(url, origin) {
    if (typeof url !== 'string' || !url) return null;
    let u;
    try { u = new URL(url, origin); } catch (e) { return null; }
    if (u.origin !== new URL(origin).origin) return null;
    if (/^\/uploads\/[^/]+$/.test(u.pathname)) return u.pathname;
    if (/^\/api\/media\/file\/\d+$/.test(u.pathname)) return u.pathname;
    return null;
  }

  /**
   * Where the page fetches an item's file: `{src}`, or `{why}` when it cannot.
   * A members-only item's file needs the session's upload token.
   */
  function fileAddress(item, token, origin) {
    const path = ownFilePath(item && item.url, origin);
    if (!path) return { why: MEDIA_WORDS.notThisServer };
    if (path.startsWith('/api/media/file/')) {
      if (!token) return { why: MEDIA_WORDS.waitingToken };
      return { src: path + '?token=' + encodeURIComponent(token) };
    }
    return { src: path };
  }

  /** The stored file name from an upload's address ("/uploads/x.png" is "x.png"). */
  function storedName(url) {
    const s = String(url || '');
    return s.startsWith('/uploads/') ? s.slice('/uploads/'.length) : s;
  }

  /** What an item is called: its title, else its file's name, else its kind. */
  function itemName(item) {
    if (item && typeof item.title === 'string' && item.title.trim()) return item.title.trim();
    const url = String((item && item.url) || '');
    if (url.startsWith('/uploads/')) return storedName(url);
    return kindWords(item && item.kind).one || 'This item';
  }

  /** An item's labels as the server describes them, in its order; an unknown id as itself. */
  function labelsOf(item, labels) {
    const ids = Array.isArray(item && item.labels) ? item.labels : [];
    const known = Array.isArray(labels) ? labels : [];
    return ids.map((id) => known.find((l) => l && l.id === id) || { id, label: id, desc: '' });
  }

  // ── Requests ──

  /** The list's filters as the relay takes them, without the empty ones. */
  function listParams(filters, before, limit) {
    const f = filters || {};
    const p = {};
    if (f.category) p.category = Number(f.category);
    if (f.kind) p.kind = f.kind;
    if (f.by) p.by = f.by;
    if (f.q && f.q.trim()) p.q = f.q.trim();
    if (before != null) p.before = Number(before);
    p.limit = limit || PAGE_SIZE;
    return p;
  }

  function query(params) {
    const q = new URLSearchParams();
    for (const k of Object.keys(params)) q.set(k, String(params[k]));
    return q.toString();
  }

  /** The unsigned public list. */
  function publicListUrl(params) {
    return '/api/media?' + query(params);
  }

  /** One person's picks (unsigned: a signed GET is a URL a proxy refuses). */
  function picksUrl(key, params) {
    return '/api/media/picks/' + encodeURIComponent(key) + '?' + query(params);
  }

  const purposes = Object.freeze({
    list: () => 'media_list',
    publish: (filename) => 'media_publish:' + filename,
    remove: (target) => 'media_remove:' + target,
    pick: (id) => 'media_pick:' + id,
    feature: (id) => 'media_feature:' + id,
    suggest: () => 'media_suggest',
    suggestions: () => 'media_suggestions',
    decide: (id, accept) => 'media_suggestions:' + id + ':' + (accept ? 'accept' : 'decline'),
    categories: (hashHex) => 'media_categories:' + hashHex,
    usage: () => 'media_usage',
  });

  function jsonPost(body) {
    return { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: typeof body === 'string' ? body : JSON.stringify(body) };
  }

  /** The relay's words for a refusal, or plain words when something in front of it answered. */
  async function refusalWords(res) {
    let text = '';
    try { text = String(await res.text()).trim(); } catch (e) { text = ''; }
    if (!text || text.startsWith('<')) return 'The server answered ' + res.status + '.';
    return text;
  }

  /** A JSON answer, or throws Error(the relay's words). */
  async function answerOf(res) {
    if (!res || !res.ok) throw new Error(res ? await refusalWords(res) : 'The server could not be reached.');
    return res.json();
  }

  // ── DOM ──

  function makeH(doc) {
    return function h(tag, props, kids) {
      const el = doc.createElement(tag);
      const p = props || {};
      for (const k of Object.keys(p)) {
        const v = p[k];
        if (v === undefined || v === null) continue;
        if (k === 'text') el.textContent = String(v);
        else if (k === 'on') { for (const ev of Object.keys(v)) el.addEventListener(ev, v[ev]); }
        else if (k === 'dataset') { for (const d of Object.keys(v)) el.dataset[d] = String(v[d]); }
        else if (k === 'attrs') { for (const a of Object.keys(v)) el.setAttribute(a, String(v[a])); }
        else el[k] = v;
      }
      for (const c of [].concat(kids == null ? [] : kids)) {
        if (c == null || c === false) continue;
        el.appendChild(typeof c === 'string' ? doc.createTextNode(c) : c);
      }
      return el;
    };
  }

  function isAdminRole(role) {
    return role === 'admin' || role === 'owner';
  }

  /**
   * The page. `env`:
   *   document, root (the element the page is built in), fetch, origin,
   *   myKey (the person's Dilithium3 key hex, '' without an identity here),
   *   sign(purpose) -> Promise<{key, timestamp, sig}|null>,
   *   session (a /shared/relay-session.js session, or null),
   *   confirm(words) -> Promise<boolean>, FormData,
   *   put ({filename, name} to open Put on Media for a Shared Files upload, or null),
   *   mountAdmin(container) (the admins' Media section, /pages/media-admin.js), optional.
   */
  function createMediaPage(env) {
    const doc = env.document;
    const h = makeH(doc);
    const myKey = env.myKey || '';
    const session = env.session || null;

    const st = {
      items: [],
      nextBefore: null,
      answered: false,          // a non-picks list answer has come back
      membersView: false,
      membersOnlyForYou: false,
      mediaVisibility: 'public',
      categories: [],
      labels: [],
      kinds: [],
      people: new Map(),        // uploader key -> name, from the answers
      filters: { category: '', kind: '', by: '', q: '', picks: false },
      revealed: new Set(),      // labelled items the person clicked to show
      picked: new Set(),        // items the person picked (public ones: the picks route is unsigned)
      loading: false,
      listError: '',
      notice: '',
      noticeWarn: false,
      viewer: null,
      form: null,
      suggest: null,
      adminMounted: false,
      filtersKey: '',
      lastStatus: '',
    };
    let loadSeq = 0;

    // ── The page's fixed parts ──
    const els = {};
    const fileInput = h('input', { id: 'media-file-input', type: 'file', hidden: true, className: 'media-file-input' });
    fileInput.addEventListener('change', () => {
      const file = fileInput.files && fileInput.files[0];
      if (file) openAddForm(file);
      try { fileInput.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
    });
    function build() {
      doc.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && st.viewer) { st.viewer = null; renderViewer(); }
      });
      els.status = h('div', { id: 'media-status', className: 'media-status' });
      els.notice = h('div', { id: 'media-notice', className: 'media-notice' });
      els.members = h('div', { id: 'media-members', className: 'media-members' });
      els.actions = h('div', { id: 'media-actions', className: 'media-actions' });
      els.form = h('div', { id: 'media-form', className: 'media-form-wrap' });
      els.suggest = h('div', { id: 'media-suggest', className: 'media-form-wrap' });
      els.featured = h('section', { id: 'media-featured', className: 'media-featured' });
      els.filters = h('div', { id: 'media-filters', className: 'media-filters' });
      els.picksNote = h('div', { id: 'media-picks-note', className: 'media-picks-note' });
      els.grid = h('div', { id: 'media-grid', className: 'media-grid' });
      els.more = h('div', { id: 'media-more-wrap', className: 'media-more-wrap' });
      els.viewer = h('div', { id: 'media-viewer', className: 'media-viewer-wrap' });
      els.admin = h('div', { id: 'media-admin-wrap', className: 'media-admin-wrap' });
      env.root.replaceChildren(
        h('div', { className: 'media-head' }, [
          h('h1', { text: 'Media' }),
          h('p', { className: 'media-lede' }, [
            MEDIA_WORDS.lede + ' Files people share for download are in ',
            h('a', { href: '/shared-files', text: 'Shared Files' }),
            '.',
          ]),
        ]),
        els.status, els.notice, els.members, els.actions, els.form, els.suggest,
        els.featured, els.filters, els.picksNote, els.grid, els.more, els.viewer, els.admin, fileInput
      );
    }

    // ── Who the person is here ──
    function sessionState() {
      return session ? session.state : null;
    }
    function token() {
      const s = sessionState();
      return s && s.status === 'bound' ? s.uploadToken : '';
    }
    function isAdmin() {
      const s = sessionState();
      return !!(s && s.status === 'bound' && isAdminRole(s.role));
    }
    function settings() {
      const s = sessionState();
      return (s && s.settings) || null;
    }
    function membersOnlyServer() {
      const s = settings();
      if (s && typeof s.media_visibility === 'string') return s.media_visibility !== 'public';
      return st.mediaVisibility !== 'public';
    }
    /** Signed in to this server: the socket is bound, so uploads and the settings work. */
    function bound() {
      const s = sessionState();
      return !!(s && s.status === 'bound');
    }

    // ── Status and notices ──
    function say(text, warn) {
      st.notice = text || '';
      st.noticeWarn = !!warn;
      renderNotice();
    }
    function renderNotice() {
      els.notice.replaceChildren();
      els.notice.className = 'media-notice' + (st.noticeWarn ? ' media-warn' : '');
      if (st.notice) els.notice.appendChild(h('p', { className: 'media-notice-text', text: st.notice }));
    }
    function renderStatus() {
      els.status.replaceChildren();
      let words = '';
      let retry = null;
      const s = sessionState();
      if (!myKey) {
        words = MEDIA_WORDS.noIdentity;
        retry = env.retryIdentity || null;
      } else if (s) {
        if (s.status === 'connecting' || s.status === 'idle') words = MEDIA_WORDS.signingIn;
        else if (s.status === 'refused') { words = 'The server did not accept this page\'s sign-in: ' + s.refusal; retry = () => session.retry(); }
        else if (s.status === 'closed') words = 'Lost the connection to the server. Trying again in ' + (s.retryInSeconds || 5) + ' seconds.';
        else if (s.status === 'no_identity') { words = MEDIA_WORDS.noIdentity; retry = env.retryIdentity || null; }
      }
      if (!words) return;
      const p = h('p', { className: 'media-status-text', text: words });
      if (retry) {
        p.appendChild(doc.createTextNode(' '));
        p.appendChild(h('button', { type: 'button', className: 'media-btn media-btn-small', id: 'media-retry', text: 'Try again', on: { click: retry } }));
      }
      els.status.appendChild(p);
    }
    function renderMembers() {
      els.members.replaceChildren();
      if (!st.answered || st.membersView) return;
      els.members.appendChild(h('p', { id: 'media-members-line', className: 'media-members-line', text: MEDIA_WORDS.membersLine }));
    }

    // ── Actions bar ──
    function renderActions() {
      els.actions.replaceChildren();
      if (!myKey) return;
      if (bound() && st.membersView) {
        els.actions.appendChild(h('button', {
          type: 'button', id: 'media-add-btn', className: 'media-btn media-btn-primary', text: 'Add to Media',
          on: { click: () => fileInput.click() },
        }));
      }
      if (st.membersView) {
        els.actions.appendChild(h('button', {
          type: 'button', id: 'media-suggest-btn', className: 'media-btn', text: 'Suggest a category',
          on: { click: openSuggest },
        }));
      }
    }

    // ── Filters ──
    function renderFilters() {
      const peopleList = Array.from(st.people.entries()).sort((a, b) => String(a[1] || '').localeCompare(String(b[1] || '')));
      const key = JSON.stringify([st.categories.map((c) => [c.id, c.name]), st.kinds, peopleList, !!myKey, st.filters]);
      if (key === st.filtersKey) return;
      st.filtersKey = key;
      const keepQ = els.search ? els.search.value : st.filters.q;
      const option = (value, text, chosen) => h('option', { value: String(value), text, selected: String(chosen) === String(value) });
      const category = h('select', { id: 'media-filter-category', className: 'media-select', attrs: { 'aria-label': 'Category' } },
        [option('', 'All categories', st.filters.category)].concat(st.categories.map((c) => option(c.id, c.name, st.filters.category))));
      category.addEventListener('change', () => setFilter('category', category.value));
      const kind = h('select', { id: 'media-filter-kind', className: 'media-select', attrs: { 'aria-label': 'Kind' } },
        [option('', 'All kinds', st.filters.kind)].concat(st.kinds.map((k) => option(k, kindWords(k).many, st.filters.kind))));
      kind.addEventListener('change', () => setFilter('kind', kind.value));
      const person = h('select', { id: 'media-filter-person', className: 'media-select', attrs: { 'aria-label': 'Person' } },
        [option('', 'Everyone', st.filters.by)].concat(peopleList.map(([k, name]) => option(k, name || 'Someone without a name', st.filters.by))));
      person.addEventListener('change', () => setFilter('by', person.value));
      const kids = [category, kind, person];
      if (myKey) {
        const picks = h('input', { type: 'checkbox', id: 'media-filter-picks', checked: !!st.filters.picks });
        picks.addEventListener('change', () => setFilter('picks', !!picks.checked));
        kids.push(h('label', { className: 'media-check' }, [picks, ' My picks']));
      }
      els.search = h('input', { type: 'search', id: 'media-search', className: 'media-search', value: keepQ || '', placeholder: 'Search titles', attrs: { 'aria-label': 'Search titles' } });
      els.search.addEventListener('keydown', (e) => { if (e.key === 'Enter') setFilter('q', els.search.value); });
      kids.push(els.search);
      kids.push(h('button', { type: 'button', id: 'media-search-btn', className: 'media-btn', text: 'Search', on: { click: () => setFilter('q', els.search.value) } }));
      els.filters.replaceChildren(...kids);
    }
    function setFilter(name, value) {
      st.filters[name] = value;
      load(true);
    }
    function renderPicksNote() {
      els.picksNote.replaceChildren();
      if (st.filters.picks && st.membersView) els.picksNote.appendChild(h('p', { className: 'media-picks-note-text', text: MEDIA_WORDS.picksPublicOnly }));
    }

    // ── Tiles ──
    function tileActions(item) {
      const row = h('div', { className: 'media-tile-actions' });
      const mine = !!myKey && item.uploader_key === myKey;
      if (myKey && st.membersView && !mine) {
        const on = st.picked.has(item.id);
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: on ? 'unpick' : 'pick' },
          text: on ? 'Unpick' : 'Pick', on: { click: () => pick(item, !on) },
        }));
      }
      if (mine || isAdmin()) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { action: 'remove' },
          text: 'Remove', on: { click: () => remove(item) },
        }));
      }
      if (isAdmin()) {
        row.appendChild(h('button', {
          type: 'button', className: 'media-btn media-btn-small', dataset: { action: item.featured ? 'unfeature' : 'feature' },
          text: item.featured ? 'Unfeature' : 'Feature', on: { click: () => feature(item, !item.featured) },
        }));
      }
      return row.childNodes.length ? row : null;
    }

    function thumb(item) {
      const labels = labelsOf(item, st.labels);
      if (labels.length && !st.revealed.has(item.id)) {
        // The label instead of the picture until clicked: nothing is fetched before then.
        return h('button', {
          type: 'button', className: 'media-thumb media-veil', dataset: { veil: item.id },
          on: { click: () => { st.revealed.add(item.id); renderGrid(); renderFeatured(); } },
        }, [
          h('span', { className: 'media-veil-labels', text: labels.map((l) => l.label).join(', ') }),
          h('span', { className: 'media-veil-show', text: MEDIA_WORDS.clickToShow }),
        ]);
      }
      const open = () => openViewer(item);
      if (item.kind === 'picture') {
        const where = fileAddress(item, token(), env.origin);
        if (!where.src) return h('div', { className: 'media-thumb media-thumb-missing', text: where.why });
        return h('button', { type: 'button', className: 'media-thumb', on: { click: open } }, [
          h('img', { className: 'media-img', src: where.src, alt: itemName(item), loading: 'lazy' }),
        ]);
      }
      const mark = item.kind === 'video' ? 'media-mark-video' : item.kind === 'model' ? 'media-mark-model' : 'media-mark-file';
      return h('button', { type: 'button', className: 'media-thumb media-mark ' + mark, on: { click: open } }, [
        h('span', { className: 'media-mark-word', text: kindWords(item.kind).one }),
        h('span', { className: 'media-mark-size', text: sizeWords(item.size_bytes) }),
      ]);
    }

    function tile(item) {
      const badges = h('div', { className: 'media-badges' });
      for (const l of labelsOf(item, st.labels)) {
        badges.appendChild(h('span', { className: 'media-badge media-badge-label', text: l.label, title: l.desc || l.label }));
      }
      if (item.visibility === 'members') badges.appendChild(h('span', { className: 'media-badge', text: 'Members only' }));
      const picks = Number(item.picks) || 0;
      return h('div', { className: 'media-tile', dataset: { id: item.id } }, [
        thumb(item),
        badges.childNodes.length ? badges : null,
        h('div', { className: 'media-tile-title', text: itemName(item) }),
        h('div', { className: 'media-tile-meta', text: 'by ' + (item.uploader_name || 'someone') + ' · ' + picks + (picks === 1 ? ' pick' : ' picks') }),
        tileActions(item),
      ]);
    }

    function renderFeatured() {
      els.featured.replaceChildren();
      const featured = st.items.filter((i) => i.featured);
      if (!featured.length) return;
      els.featured.appendChild(h('h2', { text: 'Featured' }));
      els.featured.appendChild(h('div', { id: 'media-featured-row', className: 'media-featured-row' }, featured.map(tile)));
    }

    function renderGrid() {
      els.grid.replaceChildren();
      if (st.listError) {
        els.grid.appendChild(h('p', { className: 'media-empty media-warn', text: st.listError }));
      } else if (!st.items.length) {
        let words = MEDIA_WORDS.loading;
        if (!st.loading) {
          const filtered = !!(st.filters.category || st.filters.kind || st.filters.by || st.filters.q || st.filters.picks);
          words = st.membersOnlyForYou && !st.filters.picks ? MEDIA_WORDS.membersOnlyServer : filtered ? MEDIA_WORDS.emptyFiltered : MEDIA_WORDS.empty;
        }
        els.grid.appendChild(h('p', { className: 'media-empty', text: words }));
      } else {
        for (const item of st.items) els.grid.appendChild(tile(item));
      }
      els.more.replaceChildren();
      if (st.nextBefore != null && !st.listError) {
        els.more.appendChild(h('button', {
          type: 'button', id: 'media-more', className: 'media-btn', text: st.loading ? MEDIA_WORDS.loading : 'More',
          disabled: st.loading, on: { click: () => load(false) },
        }));
      }
    }

    // ── The viewer ──
    function openViewer(item) {
      st.viewer = item;
      renderViewer();
    }
    function renderViewer() {
      els.viewer.replaceChildren();
      const item = st.viewer;
      if (!item) return;
      const where = fileAddress(item, token(), env.origin);
      let body;
      if (!where.src) body = h('p', { className: 'media-warn', text: where.why });
      else if (item.kind === 'picture') body = h('img', { className: 'media-viewer-img', src: where.src, alt: itemName(item) });
      else if (item.kind === 'video') body = h('video', { className: 'media-viewer-video', src: where.src, controls: true, preload: 'metadata' });
      else {
        body = h('div', { className: 'media-viewer-file' }, [
          h('div', { className: 'media-viewer-name', text: itemName(item) }),
          h('div', { className: 'media-viewer-size', text: sizeWords(item.size_bytes) }),
          h('a', { className: 'media-btn media-btn-primary', href: where.src, text: 'Download', attrs: { download: '' } }),
        ]);
      }
      const labels = labelsOf(item, st.labels).map((l) => l.label);
      const close = () => { st.viewer = null; renderViewer(); };
      const overlay = h('div', { id: 'media-viewer-open', className: 'media-viewer', attrs: { role: 'dialog', 'aria-label': itemName(item) } }, [
        h('div', { className: 'media-viewer-box' }, [
          h('button', { type: 'button', id: 'media-viewer-close', className: 'media-btn media-viewer-close', text: 'Close', on: { click: close } }),
          body,
          h('h2', { className: 'media-viewer-title', text: itemName(item) }),
          item.description ? h('p', { className: 'media-viewer-desc', text: item.description }) : null,
          h('p', { className: 'media-viewer-meta', text: [
            'by ' + (item.uploader_name || 'someone'),
            item.category && item.category.name ? item.category.name : '',
            kindWords(item.kind).one,
            sizeWords(item.size_bytes),
            labels.length ? labels.join(', ') : '',
          ].filter(Boolean).join(' · ') }),
        ]),
      ]);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
      els.viewer.appendChild(overlay);
    }

    // ── Loading ──
    function takeAnswer(a, fromPicks) {
      const items = Array.isArray(a && a.items) ? a.items : [];
      st.items = st.items.concat(items);
      st.nextBefore = a && a.next_before != null ? a.next_before : null;
      if (!fromPicks) {
        st.answered = true;
        st.membersView = !!(a && a.members_view);
        st.membersOnlyForYou = !!(a && a.members_only_for_you);
        if (a && typeof a.media_visibility === 'string') st.mediaVisibility = a.media_visibility;
        if (Array.isArray(a && a.categories) && (a.categories.length || !st.membersView)) st.categories = a.categories;
        if (Array.isArray(a && a.labels) && (a.labels.length || !st.membersView)) st.labels = a.labels;
      }
      const kinds = st.kinds.slice();
      for (const i of items) {
        if (typeof i.kind === 'string' && i.kind) kinds.push(i.kind);
        if (typeof i.uploader_key === 'string' && i.uploader_key && !st.people.has(i.uploader_key)) st.people.set(i.uploader_key, i.uploader_name || '');
      }
      st.kinds = sortKinds(kinds);
    }

    /** The members' list, signed, when the person has an identity here; else the public one. */
    async function fetchList(params) {
      if (myKey) {
        const auth = await env.sign(purposes.list());
        if (auth) return env.fetch('/api/media/list', jsonPost(Object.assign({}, params, auth)));
      }
      return env.fetch(publicListUrl(params));
    }

    async function load(reset) {
      const seq = ++loadSeq;
      if (reset) {
        st.items = [];
        st.nextBefore = null;
      }
      st.loading = true;
      st.listError = '';
      renderGrid();
      const params = listParams(st.filters, reset ? null : st.nextBefore);
      const fromPicks = !!(st.filters.picks && myKey);
      try {
        const res = fromPicks ? await env.fetch(picksUrl(myKey, params)) : await fetchList(params);
        const a = await answerOf(res);
        if (seq !== loadSeq) return;
        takeAnswer(a, fromPicks);
      } catch (e) {
        if (seq !== loadSeq) return;
        st.listError = 'The Media list could not be loaded: ' + (e && e.message ? e.message : String(e));
      }
      st.loading = false;
      renderAll();
    }

    /** Which items the person picked: their picks list, public items only (it is read unsigned). */
    async function loadMyPicks() {
      if (!myKey) return;
      const picked = new Set();
      let before = null;
      for (let page = 0; page < MY_PICKS_PAGES; page++) {
        let a;
        try { a = await answerOf(await env.fetch(picksUrl(myKey, listParams({}, before, 100)))); } catch (e) { return; }
        for (const i of a.items || []) picked.add(i.id);
        if (a.next_before == null) break;
        before = a.next_before;
      }
      st.picked = picked;
      renderGrid();
      renderFeatured();
    }

    /** How much of the Media space the person's own items use (their published items). */
    async function myUse() {
      let used = 0;
      let before = null;
      for (let page = 0; page < MY_USE_PAGES; page++) {
        const a = await answerOf(await fetchList(listParams({ by: myKey }, before, 100)));
        for (const i of a.items || []) used += Math.max(0, Number(i.size_bytes) || 0);
        if (a.next_before == null) return used;
        before = a.next_before;
      }
      return used;
    }

    // ── Pick, remove, feature ──
    async function signed(purpose) {
      const auth = await env.sign(purpose);
      if (!auth) say(MEDIA_WORDS.cannotSign, true);
      return auth;
    }

    async function pick(item, on) {
      const auth = await signed(purposes.pick(item.id));
      if (!auth) return;
      try {
        const a = await answerOf(await env.fetch(on ? '/api/media/pick' : '/api/media/unpick', jsonPost(Object.assign({ id: item.id }, auth))));
        item.picks = Number(a.picks) || 0;
        if (on) st.picked.add(item.id); else st.picked.delete(item.id);
        say(on ? 'Picked. It is on your Picks now.' : 'Unpicked.');
      } catch (e) {
        say(e.message, true);
      }
      renderGrid();
      renderFeatured();
    }

    async function remove(item) {
      const ok = await env.confirm('"' + itemName(item) + '"\n\n' + removeWords(item));
      if (!ok) return;
      const auth = await signed(purposes.remove(item.id));
      if (!auth) return;
      try {
        const a = await answerOf(await env.fetch('/api/media/remove', jsonPost(Object.assign({ id: item.id }, auth))));
        st.items = st.items.filter((i) => i.id !== item.id);
        if (st.viewer && st.viewer.id === item.id) st.viewer = null;
        say(removedWords(a));
      } catch (e) {
        say(e.message, true);
      }
      renderAll();
    }

    async function feature(item, on) {
      const auth = await signed(purposes.feature(item.id));
      if (!auth) return;
      try {
        await answerOf(await env.fetch('/api/media/feature', jsonPost(Object.assign({ id: item.id, featured: on }, auth))));
        item.featured = on;
        say(on ? 'Featured: it leads the Media page now.' : 'Taken off the featured row.');
      } catch (e) {
        say(e.message, true);
      }
      renderGrid();
      renderFeatured();
    }

    // ── Add to Media and Put on Media ──
    async function openAddForm(file) {
      const s = settings();
      const knowShare = !!(s && Number.isFinite(s.media_per_person_mb) && s.media_per_person_mb > 0);
      // Until the share is worked out nothing can be uploaded, so an upload over it never starts.
      st.form = { mode: 'add', file, name: file.name, stop: limitStop(file, sessionState() ? sessionState().uploadLimitBytes : 0), shareLine: '', checking: knowShare, busy: false, error: '', uploadedName: '' };
      renderForm();
      if (knowShare) {
        const share = s.media_per_person_mb * MIB;
        let used = null;
        try { used = await myUse(); } catch (e) { used = null; }
        if (!st.form || st.form.file !== file) return;
        st.form.checking = false;
        if (used != null) {
          st.form.shareLine = shareLeftWords(used, share);
          st.form.stop = st.form.stop || shareStop(file, used, share);
        }
      }
      renderForm();
    }

    function openPutForm(put) {
      st.form = { mode: 'put', filename: storedName(put.filename), name: put.name || storedName(put.filename), stop: null, shareLine: '', busy: false, error: '', uploadedName: '' };
      renderForm();
    }

    function closeForm() {
      st.form = null;
      renderForm();
    }

    /** What the form is drawn from, so it is drawn again when that changes. */
    function formKey() {
      return JSON.stringify([st.categories.map((c) => [c.id, c.name]), st.labels.map((l) => l.id), membersOnlyServer()]);
    }

    function renderForm() {
      els.form.replaceChildren();
      const f = st.form;
      if (!f) return;
      f.key = formKey();
      const put = f.mode === 'put';
      const kids = [h('h2', { text: put ? 'Put on Media' : 'Add to Media' })];
      kids.push(h('p', { className: 'media-form-file', text: put ? f.name : f.name + ' (' + sizeWords(f.file.size) + ')' }));
      if (f.shareLine) kids.push(h('p', { id: 'media-share-line', className: 'media-share-line', text: f.shareLine }));
      if (f.stop) {
        kids.push(h('p', { id: 'media-form-stop', className: 'media-warn', text: f.stop }));
        kids.push(h('div', { className: 'media-form-buttons' }, [
          h('button', { type: 'button', className: 'media-btn', id: 'media-form-cancel', text: 'Close', on: { click: closeForm } }),
        ]));
        els.form.appendChild(h('div', { className: 'media-panel media-form' }, kids));
        return;
      }
      const title = h('input', { type: 'text', id: 'media-form-title', className: 'media-input', maxLength: 120, value: f.title || '' });
      const category = h('select', { id: 'media-form-category', className: 'media-select' },
        st.categories.map((c) => h('option', { value: String(c.id), text: c.name, selected: String(f.category || '') === String(c.id) })));
      const description = h('textarea', { id: 'media-form-description', className: 'media-input', maxLength: 2000, rows: 3, value: f.description || '' });
      // What is typed is kept as it is typed, so drawing the form again loses nothing.
      title.addEventListener('input', () => { f.title = title.value; });
      category.addEventListener('change', () => { f.category = category.value; });
      description.addEventListener('input', () => { f.description = description.value; });
      kids.push(h('label', { className: 'media-field' }, ['Title (optional)', title]));
      if (st.categories.length) kids.push(h('label', { className: 'media-field' }, ['Category', category]));
      kids.push(h('label', { className: 'media-field' }, ['Description (optional)', description]));
      const ticks = [];
      if (st.labels.length) {
        const box = h('fieldset', { className: 'media-field media-labels', id: 'media-form-labels' }, [h('legend', { text: 'Labels (the picture stays hidden until someone clicks it)' })]);
        for (const l of st.labels) {
          const tick = h('input', { type: 'checkbox', value: l.id, dataset: { label: l.id }, checked: !!(f.labels && f.labels.includes(l.id)) });
          tick.addEventListener('change', () => { f.labels = ticks.filter((t) => t.checked).map((t) => t.value); });
          ticks.push(tick);
          box.appendChild(h('label', { className: 'media-check', title: l.desc || '' }, [tick, ' ' + l.label]));
          if (l.desc) box.appendChild(h('div', { className: 'media-hint', text: l.desc }));
        }
        kids.push(box);
      }
      // Who sees it: a file already public goes on Media as public only (the relay's
      // `was_public`), and a members-only server takes members only.
      let visibility = null;
      if (put) {
        kids.push(h('p', { id: 'media-form-public-only', className: 'media-hint', text: MEDIA_WORDS.alreadyPublic }));
      } else if (membersOnlyServer()) {
        kids.push(h('p', { id: 'media-form-members-only', className: 'media-hint', text: MEDIA_WORDS.serverMembersOnly }));
      } else {
        const pub = h('input', { type: 'radio', name: 'media-form-visibility', value: 'public', id: 'media-form-vis-public', checked: f.visibility !== 'members' });
        const mem = h('input', { type: 'radio', name: 'media-form-visibility', value: 'members', id: 'media-form-vis-members', checked: f.visibility === 'members' });
        visibility = { pub, mem };
        pub.addEventListener('change', () => { if (pub.checked) f.visibility = 'public'; });
        mem.addEventListener('change', () => { if (mem.checked) f.visibility = 'members'; });
        kids.push(h('fieldset', { className: 'media-field', id: 'media-form-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      if (f.error) kids.push(h('p', { id: 'media-form-error', className: 'media-warn', text: f.error }));
      const go = h('button', {
        type: 'button', id: 'media-form-go', className: 'media-btn media-btn-primary', disabled: f.busy || !!f.checking,
        text: f.checking ? 'Working out your share...' : f.busy ? 'Working...' : put || f.uploadedName ? 'Put on Media' : 'Upload and add',
      });
      go.addEventListener('click', () => {
        // Keep what was typed for a second try.
        f.title = title.value;
        f.category = category.value;
        f.description = description.value;
        f.labels = ticks.filter((t) => t.checked).map((t) => t.value);
        f.visibility = put ? 'public' : membersOnlyServer() ? 'members' : visibility && visibility.mem.checked ? 'members' : 'public';
        submitForm();
      });
      kids.push(h('div', { className: 'media-form-buttons' }, [
        go,
        h('button', { type: 'button', className: 'media-btn', id: 'media-form-cancel', text: 'Cancel', on: { click: closeForm } }),
      ]));
      els.form.appendChild(h('div', { className: 'media-panel media-form' }, kids));
    }

    async function submitForm() {
      const f = st.form;
      if (!f || f.busy || f.stop || f.checking) return;
      f.busy = true;
      f.error = '';
      renderForm();
      try {
        let filename = f.mode === 'put' ? f.filename : f.uploadedName;
        if (!filename) {
          const t = token();
          if (!t) throw new Error(MEDIA_WORDS.notSignedInUpload);
          const form = new env.FormData();
          form.append('file', f.file);
          const up = await answerOf(await env.fetch('/api/upload?token=' + encodeURIComponent(t) + '&media=1', { method: 'POST', body: form }));
          filename = storedName(up.filename || up.url);
          f.uploadedName = filename;
        }
        const auth = await env.sign(purposes.publish(filename));
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        const body = Object.assign({
          filename,
          title: (f.title || '').trim(),
          description: (f.description || '').trim(),
          labels: f.labels || [],
          visibility: f.visibility || 'public',
        }, f.category ? { category_id: Number(f.category) } : {}, auth);
        await answerOf(await env.fetch('/api/media', jsonPost(body)));
        st.form = null;
        renderForm();
        say(MEDIA_WORDS.published);
        load(true);
        return;
      } catch (e) {
        f.error = (e && e.message ? e.message : String(e)) + (f.uploadedName && f.mode === 'add'
          ? ' The file is uploaded and waits 24 hours for you to try again; after that it is deleted.'
          : '');
      }
      f.busy = false;
      renderForm();
    }

    // ── Suggest a category ──
    function openSuggest() {
      st.suggest = { busy: false, error: '', mine: null };
      renderSuggest();
      loadMySuggestions();
    }
    async function loadMySuggestions() {
      const auth = await env.sign(purposes.suggestions());
      if (!auth || !st.suggest) return;
      try {
        const a = await answerOf(await env.fetch('/api/media/suggestions', jsonPost(auth)));
        if (!st.suggest) return;
        st.suggest.mine = (a.suggestions || []).filter((s) => s.suggester_key === myKey);
      } catch (e) {
        if (st.suggest) st.suggest.mine = [];
      }
      renderSuggest();
    }
    function renderSuggest() {
      els.suggest.replaceChildren();
      const s = st.suggest;
      if (!s) return;
      const name = h('input', { type: 'text', id: 'media-suggest-name', className: 'media-input', maxLength: 60, value: s.name || '' });
      const note = h('textarea', { id: 'media-suggest-note', className: 'media-input', maxLength: 500, rows: 2, value: s.note || '' });
      const go = h('button', { type: 'button', id: 'media-suggest-go', className: 'media-btn media-btn-primary', text: s.busy ? 'Sending...' : 'Send suggestion', disabled: s.busy });
      go.addEventListener('click', () => { s.name = name.value; s.note = note.value; sendSuggestion(); });
      const kids = [
        h('h2', { text: 'Suggest a category' }),
        h('p', { className: 'media-hint', text: "This server's admins choose its categories. Suggest one, and they accept or decline it." }),
        h('label', { className: 'media-field' }, ['Name', name]),
        h('label', { className: 'media-field' }, ['Note (optional)', note]),
      ];
      if (s.error) kids.push(h('p', { className: 'media-warn', id: 'media-suggest-error', text: s.error }));
      kids.push(h('div', { className: 'media-form-buttons' }, [
        go,
        h('button', { type: 'button', className: 'media-btn', text: 'Close', on: { click: () => { st.suggest = null; renderSuggest(); } } }),
      ]));
      if (s.mine && s.mine.length) {
        const words = { open: 'waiting for an admin', accepted: 'accepted', declined: 'declined' };
        kids.push(h('h3', { text: 'Your suggestions' }));
        kids.push(h('ul', { className: 'media-list', id: 'media-suggest-mine' },
          s.mine.map((m) => h('li', { text: m.name + ': ' + (words[m.state] || m.state) }))));
      }
      els.suggest.appendChild(h('div', { className: 'media-panel media-form' }, kids));
    }
    async function sendSuggestion() {
      const s = st.suggest;
      if (!s || s.busy) return;
      s.busy = true;
      s.error = '';
      renderSuggest();
      try {
        const auth = await env.sign(purposes.suggest());
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        await answerOf(await env.fetch('/api/media/suggest', jsonPost(Object.assign({ name: (s.name || '').trim(), note: (s.note || '').trim() }, auth))));
        st.suggest = null;
        renderSuggest();
        say(MEDIA_WORDS.suggestSent);
        return;
      } catch (e) {
        s.error = e.message;
      }
      s.busy = false;
      renderSuggest();
    }

    // ── Admins ──
    function renderAdmin() {
      if (st.adminMounted || !isAdmin() || typeof env.mountAdmin !== 'function') return;
      st.adminMounted = true;
      const body = h('div', { id: 'media-admin-body', className: 'media-admin-body' });
      els.admin.appendChild(h('details', { className: 'media-panel media-admin', id: 'media-admin' }, [
        h('summary', { text: 'Media settings, categories and space (admins)' }),
        body,
      ]));
      env.mountAdmin(body);
    }

    function renderAll() {
      renderStatus();
      renderNotice();
      renderMembers();
      renderActions();
      renderFilters();
      renderPicksNote();
      renderFeatured();
      renderGrid();
      renderViewer();
      if (st.form && !st.form.busy && st.form.key !== formKey()) renderForm();
      renderAdmin();
    }

    // ── The session's news ──
    function onSession(ev) {
      if (ev.type === 'state') {
        const was = st.lastStatus;
        st.lastStatus = session.state.status;
        // Signed in now: signing in makes the person a member of this server, so a list
        // read before that (or still on its way) may have missed the members' items.
        if (session.state.status === 'bound' && was !== 'bound' && (!st.answered || !st.membersView)) load(true);
        else renderAll();
        return;
      }
      const m = ev.msg;
      // The answer to a category suggestion comes as a notice (the relay's words).
      if (m && m.type === 'system' && typeof m.message === 'string' && m.message.startsWith('Your category suggestion')) {
        say(m.message);
        if (st.suggest) loadMySuggestions();
      }
    }

    async function start() {
      build();
      if (session) {
        session.subscribe(onSession);
        if (myKey) session.start();
      }
      renderAll();
      loadMyPicks();
      await load(true);
      // Put on Media (a Shared Files row's link) once the categories and labels are known.
      if (env.put && myKey) openPutForm(env.put);
    }

    return { start, state: st, load, openAddForm, openPutForm, removeItem: remove, pickItem: pick, featureItem: feature };
  }

  // ── In the browser ──
  function bootInBrowser() {
    const doc = root.document;
    const rootEl = doc && doc.getElementById('media-root');
    if (!rootEl) return;
    const params = new URLSearchParams(root.location.search || '');
    const put = params.get('put') ? { filename: params.get('put'), name: params.get('name') || '' } : null;
    const go = async () => {
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
      const sign = (purpose) => (typeof root.getPqSignedAuth === 'function' ? root.getPqSignedAuth(purpose) : Promise.resolve(null));
      const confirm = (words) => (typeof root.holdConfirm === 'function'
        ? root.holdConfirm(words, { seconds: 2, confirmLabel: 'Hold to remove' })
        : Promise.resolve(root.confirm(words)));
      let page = null;
      page = createMediaPage({
        document: doc,
        root: rootEl,
        fetch: (u, i) => root.fetch(u, i),
        origin: root.location.origin,
        myKey: identity ? identity.dilithiumPublicHex : '',
        sign,
        session,
        confirm,
        FormData: root.FormData,
        put,
        retryIdentity: () => root.location.reload(),
        // The admins' section (/pages/media-admin.js, loaded after this file).
        mountAdmin: (container) => {
          if (typeof root.mountMediaAdmin !== 'function') return;
          root.mountMediaAdmin(container, {
            document: doc, fetch: (u, i) => root.fetch(u, i), sign, confirm, session,
            blake3: (bytes) => root.pqBlake3(bytes),
            onChange: () => { if (page) page.load(true); },
          });
        },
      });
      page.start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    MEDIA_WORDS, KIND_WORDS, PAGE_SIZE, kindWords, sortKinds, sizeWords, shareLeftWords, shareStop, limitStop,
    meterWords, removeWords, removedWords, ownFilePath, fileAddress, storedName, itemName, labelsOf,
    listParams, publicListUrl, picksUrl, purposes, jsonPost, refusalWords, answerOf, makeH, isAdminRole,
    createMediaPage,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    Object.assign(root, { MediaPage: api });
    bootInBrowser();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
