// ── files-shared.js ───────────────────────────────────────────────────────
// The Files page's "Shared files" place on the website (2026-10-19): the
// server's public file library (GET /api/uploads), which was the Shared Files
// page (/shared-files) until the Files page took it in as a place. The desktop
// app's twin is src/gui/pages/files/shared_files.rs.
//
// On the left, under the places: the heading and how sharing works, Upload a
// file (/pages/shared-files-upload.js, every rule of the page it came from: the
// person's limit, the cleaning on this device, the step when there is something
// to say, and files dropped on the page while this place is shown), a search
// over names and who shared them, and a row per file (its type's badge, its
// name, who shared it, its size and day). A press on a row shows the file in the
// middle (its badge and name large, Download) and on the right what it is, with
// Put on the Files page on a file of your own: the Shared on <server> list's
// form for it, public only (it is public here already).
//
// Everything is built with createElement and textContent. A classic script in
// the browser (FilesShared on window), a CommonJS module under Node, where
// scripts/tests/files-web-places.test.js drives it against stand-ins.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const NODE = typeof module === 'object' && module && module.exports;
  const SFU = NODE ? require('./shared-files-upload.js') : root.SharedFilesUpload;

  const WORDS = Object.freeze({
    heading: 'Shared files on the server',
    how: 'Files here are public: anyone can download them. Press Upload a file, or drop files on this page. In the chat, 3D and model files you attach (.blend, .stl, .obj, .gltf, .glb) are listed here too; photos and other chat attachments stay private to the people you send them to. A file of yours here can also go on the Shared on list: press Put on the Files page.',
    search: 'Search files by name (phone case, bushing, bracket...)',
    loading: 'Loading the library...',
    none: 'Nothing shared yet. Be the first: attach a 3D or model file in the chat and it will appear here.',
    noMatch: 'No files match that search.',
    unreachable: 'Could not reach the file library. If you are hosting your own server, make sure it is running; otherwise try again in a moment.',
    choose: 'Choose a file on the left to see it here.',
    download: 'Download',
    put: 'Put on the Files page',
    putTip: 'Show this file in the Shared on list too, with a title and a category.',
    yours: 'Shared by you',
  });

  /** "by Ann · 2.4 MB · 19/10/2026": who shared it, its size and day. */
  function subLine(f, sizeWords) {
    const by = f.uploader_name ? 'by ' + f.uploader_name : '';
    const size = f.size_bytes > 0 && typeof sizeWords === 'function' ? sizeWords(f.size_bytes) : '';
    let day = '';
    if (f.uploaded_at) { try { day = new Date(f.uploaded_at).toLocaleDateString(); } catch (e) { day = ''; } }
    return [by, size, day].filter(Boolean).join(' · ');
  }
  /** The files whose name or sharer has `q` in it (any case). */
  function matching(files, q) {
    const s = String(q || '').trim().toLowerCase();
    if (!s) return files.slice();
    return files.filter((f) => String(f.name || '').toLowerCase().includes(s) || String(f.uploader_name || '').toLowerCase().includes(s));
  }
  /** The stored name of an /uploads/ address (what Put on the Files page names), or ''. */
  function storedOf(url) {
    return typeof url === 'string' && url.startsWith('/uploads/') ? url.slice('/uploads/'.length) : '';
  }

  /**
   * env: { h, document, fetch, myKey(), onPut({filename, name}), badge(name) (an element), sizeWords(bytes),
   *        typesKnown(), loadTypes() (the list of kinds, files-edit.js), session, storage, FormData, clean }
   */
  function createSharedPlace(env) {
    const h = env.h;
    const st = { files: [], q: '', selected: null, loaded: false, failed: false };
    const els = {};
    els.uploadRoot = h('div', { id: 'sf-upload', className: 'sf-upload' });
    els.search = h('input', { type: 'search', id: 'sf-q', className: 'files-search', placeholder: WORDS.search, attrs: { 'aria-label': WORDS.search, autocomplete: 'off' } });
    els.search.addEventListener('input', () => { st.q = els.search.value || ''; renderList(); });
    els.list = h('div', { id: 'sf-list', className: 'sf-list' });
    const left = h('div', { id: 'files-shared-list', className: 'files-shared-list' }, [
      h('h2', { className: 'files-place-heading', text: WORDS.heading }),
      h('p', { className: 'files-hint', text: WORDS.how }),
      els.uploadRoot, els.search, els.list,
    ]);
    const middle = h('div', { id: 'files-shared-file', className: 'files-shared-file' });
    const right = h('div', { id: 'files-shared-details', className: 'files-shared-details' });

    // Upload a file: the Shared Files page's own module, its overlay left to the Files page (whose
    // drop goes here while this place is shown: dropDecision and takeFiles below).
    const upload = SFU && typeof SFU.createSharedFilesUpload === 'function'
      ? SFU.createSharedFilesUpload({
        document: env.document, root: els.uploadRoot, window: null, session: env.session || null, storage: env.storage || null,
        fetch: env.fetch, FormData: env.FormData, clean: env.clean === undefined ? false : env.clean, onUploaded: () => load(),
      })
      : null;

    function chosen() {
      return st.selected ? st.files.find((f) => f.url === st.selected) || null : null;
    }
    function badge(name) {
      return typeof env.badge === 'function' ? env.badge(name) : null;
    }

    function renderList() {
      if (!st.loaded) {
        els.list.replaceChildren(h('div', { className: 'sf-empty', text: st.failed ? WORDS.unreachable : WORDS.loading }));
        return;
      }
      const shown = matching(st.files, st.q);
      if (!shown.length) {
        els.list.replaceChildren(h('div', { className: 'sf-empty', text: st.files.length ? WORDS.noMatch : WORDS.none }));
        return;
      }
      els.list.replaceChildren(...shown.map((f) => h('button', {
        type: 'button', className: 'sf-row' + (f.url === st.selected ? ' sf-row-active' : ''),
        dataset: { url: f.url },
        attrs: f.url === st.selected ? { 'aria-current': 'true' } : {},
        on: { click: () => { st.selected = f.url; renderList(); renderChosen(); } },
      }, [
        h('span', { className: 'sf-name' }, [badge(f.name), h('span', { className: 'sf-name-text', text: f.name || '' })]),
        h('span', { className: 'sf-sub', text: subLine(f, env.sizeWords) }),
      ])));
    }

    function renderChosen() {
      const f = chosen();
      if (!f) {
        middle.replaceChildren(h('p', { className: 'files-hint files-pane-empty', text: WORDS.choose }));
        right.replaceChildren(h('p', { className: 'files-hint files-pane-empty', text: WORDS.how }));
        return;
      }
      middle.replaceChildren(h('div', { className: 'sf-file' }, [
        h('p', { className: 'sf-file-name' }, [badge(f.name), h('span', { text: f.name || '' })]),
        h('a', { className: 'files-btn files-btn-primary sf-dl', href: f.url, text: WORDS.download, attrs: { download: '' } }),
      ]));
      const mine = typeof env.myKey === 'function' && env.myKey() && f.uploader_key === env.myKey();
      const stored = storedOf(f.url);
      right.replaceChildren(
        h('h2', { className: 'files-place-heading', text: f.name || '' }),
        h('p', { className: 'sf-sub', text: subLine(f, env.sizeWords) }),
        mine ? h('p', { className: 'files-hint', text: WORDS.yours }) : null,
        h('p', { className: 'files-hint', text: WORDS.how }),
        h('p', { className: 'sf-actions' }, [
          h('a', { className: 'files-btn sf-dl', href: f.url, text: WORDS.download, attrs: { download: '' } }),
          mine && stored && typeof env.onPut === 'function' ? h('button', {
            type: 'button', id: 'sf-put', className: 'files-btn sf-put', text: WORDS.put, title: WORDS.putTip,
            on: { click: () => env.onPut({ filename: stored, name: f.name || '' }) },
          }) : null,
        ]),
      );
    }

    async function load() {
      let answer = null;
      try {
        const res = await env.fetch('/api/uploads?limit=500');
        if (!res || !res.ok) throw new Error('HTTP ' + (res ? res.status : 0));
        answer = await res.json();
        st.failed = false;
      } catch (e) {
        st.failed = true;
      }
      if (answer) st.files = Array.isArray(answer.files) ? answer.files : Array.isArray(answer) ? answer : [];
      st.loaded = !st.failed;
      if (st.selected && !chosen()) st.selected = null;
      renderList();
      renderChosen();
    }

    function start() {
      renderList();
      renderChosen();
      if (upload) upload.start();
      // Each badge is a plain grey "File" until the list of kinds is in; drawn again when it comes.
      if (typeof env.typesKnown === 'function' && !env.typesKnown() && typeof env.loadTypes === 'function') {
        Promise.resolve(env.loadTypes()).then((known) => { if (known) { renderList(); renderChosen(); } }, () => {});
      }
      return load();
    }

    return {
      left, middle, right, state: st, start, load, renderList, renderChosen,
      dropDecision: () => (upload ? upload.dropDecision() : { take: false, reason: '' }),
      takeFiles: (files) => { if (upload) upload.takeFiles(files); },
    };
  }

  const api = { WORDS, subLine, matching, storedOf, createSharedPlace };
  if (NODE) module.exports = api;
  else root.FilesShared = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
