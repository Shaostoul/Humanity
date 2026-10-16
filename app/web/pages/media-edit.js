// ── media-edit.js ─────────────────────────────────────────────────────────
// An item's Edit form on the website, and the licence picker that Add to Media
// shares with it (Media round 5, lane 2A, 2026-10-12; docs/design/
// media-round-5-plan.md 3.4, 3.9, 3.10, with the orchestrator's decision 1 in
// section 9 and the operator's answers in section 10). The desktop app's
// Edit window is the original (src/gui/pages/media/edit.rs); this form copies
// its fields, rules and words:
//
// - the title, the description, the category (its description under it), the
//   content labels;
// - who can see it, under Add to Media's rules and words (a file already public
//   where it was shared stays public; a members-only Media page makes
//   everything members only);
// - the licence, with the server's own line for it and a link to its deed, and
//   "Offer this for the game" only with a licence the server marks `game`
//   (CC0 and CC BY 4.0). The facts are stated plainly before Save: a Creative
//   Commons licence cannot be taken back for copies already taken
//   (`cc_warning`), and on a members-only item any Creative Commons licence
//   lets every member who receives it republish it publicly
//   (`cc_members_warning`). An admin editing someone else's item sees the
//   licence and the tick but cannot change them: "Only Ann can change these."
//   (a licence is the rights holder's grant);
// - the hosting paragraph ("Your uploads stay yours."), as the server words it;
// - the files: Remove on each added file and the cover (the uploader and
//   admins), and, for the uploader, "Add a file" and "Cover picture". A picked
//   file is the same resumable upload as Add to Media's, listed under Uploads
//   in progress, and added to this item when it is up instead of published
//   (media-app.js `startAttach`, media-upload.js `attachUpload`).
//
// Save sends the edit (POST /api/media/edit, every field) signed over
// `media_edit:<id>:<BLAKE3 hex of the edit member's JSON text exactly as sent>`:
// the text is made ONCE (`editText`), those exact bytes are hashed, and those
// exact bytes are put in the body (`editBody`), never serialised again. The
// visibility goes by its own signed route when it changed. Every word about a
// licence comes from the server's answer (data/media/licences.json on the
// relay), never from a copy here; everything the server sends goes in with
// textContent.
//
// A classic script in the browser (loaded before media-item.js and
// media-app.js; it needs /shared/blake3.js), a CommonJS module under Node,
// where scripts/tests/media-web-item.test.js runs it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // BLAKE3 in plain JavaScript (/shared/blake3.js): the edit's purpose names the hash of its text.
  const B3 = typeof module === 'object' && module && module.exports ? require('../shared/blake3.js') : root.HosBlake3;

  const WORDS = Object.freeze({
    save: 'Save',
    saving: 'Saving...',
    cancel: 'Cancel',
    addFile: 'Add a file',
    coverPicture: 'Cover picture',
    replaceCover: 'Replace the cover picture',
    remove: 'Remove',
    removeTip: 'Take it off this item. It is deleted from the server.',
    uploadsHint: 'Each goes up under Uploads in progress on the Media page and is added here when it is up.',
    labelsLegend: 'Content labels: a labelled item stays hidden until someone clicks it.',
    noHash: 'This page could not load its hashing code, so the edit was not sent. Reload the page and try again.',
  });
  // The picture types a cover can be (the relay: "A cover must be a picture: PNG, JPEG, GIF or WebP.").
  const COVER_ACCEPT = 'image/png,image/jpeg,image/gif,image/webp';

  /** "Only Ann can change these." (decision 1: only the rights holder changes a licence). */
  function ownerOnlyWords(name) {
    return 'Only ' + name + ' can change these.';
  }
  /** What a file's Remove asks first. */
  function removeFileWords(name) {
    return 'Remove ' + name + ' from this item? It is deleted from the server.';
  }
  /** The line about the item's own first file, which is never removed here. */
  function firstFileWords(name) {
    return name + ': the item itself. To take it away, remove the item.';
  }

  /**
   * The edit's text, made once: every field, keys in name order (as the relay's own fixture writes
   * them). Whatever is hashed is exactly this text, and it is sent as it is.
   */
  function editText(fields) {
    const f = fields || {};
    return JSON.stringify({
      category_id: Number(f.category_id) || 0,
      description: String(f.description == null ? '' : f.description),
      labels: (Array.isArray(f.labels) ? f.labels : []).map(String),
      licence: String(f.licence || ''),
      project_use: !!f.project_use,
      title: String(f.title == null ? '' : f.title),
    });
  }

  /** `media_edit:<id>:<BLAKE3 hex of the edit text's UTF-8 bytes>`. */
  function editPurpose(id, text) {
    if (!B3) throw new Error(WORDS.noHash);
    return 'media_edit:' + id + ':' + B3.toHex(B3.hash(new TextEncoder().encode(text)));
  }

  /**
   * The request's body, as text: the signature, the item, its file (W5), then the edit text exactly
   * as it was hashed. Built by joining text, so the edit's bytes are never written a second time.
   */
  function editBody(auth, id, filename, text) {
    const head = JSON.stringify({ key: auth.key, timestamp: auth.timestamp, sig: auth.sig, id, filename });
    return head.slice(0, -1) + ',"edit":' + text + '}';
  }

  /** The licence the server offers with id `id`, or null. */
  function offeredLicence(licences, id) {
    const offered = licences && Array.isArray(licences.offered) ? licences.offered : [];
    return offered.find((l) => l && l.id === id) || null;
  }

  /**
   * The licence picker, shared by Add to Media and Edit. `o`: `licences` (the server's answer),
   * `choice` ({licence, project_use}, changed in place as the person picks), `members()` (whether
   * the item is or will be members only), `was` (the licence the item has now, so leaving a Creative
   * Commons licence states the fact too), `ownerOnly` (the uploader's name when an admin edits
   * someone else's item: shown, never changed) and `id` (the element's id). Returns {el, refresh}.
   */
  function licenceChoice(h, o) {
    const L = o.licences || {};
    const choice = o.choice;
    if (!choice.licence) choice.licence = L.default || 'all_rights_reserved';
    const offered = Array.isArray(L.offered) ? L.offered.filter((l) => l && typeof l.id === 'string') : [];
    // A server that sends no licences (one from before round 5): nothing to pick, and the relay's default goes.
    if (!offered.length && !o.ownerOnly) return { el: null, refresh: () => {} };
    // A licence the item carries that this server no longer offers is named by its id.
    const listed = offeredLicence(L, choice.licence) ? offered : offered.concat([{ id: choice.licence, name: choice.licence, line: '' }]);
    const nameOf = (id) => { const l = listed.find((x) => x.id === id); return (l && l.name) || id; };
    const more = h('div', { className: 'media-licence-more' });
    let control;
    if (o.ownerOnly) {
      control = h('span', { className: 'media-licence-name', dataset: { licenceName: '' }, text: nameOf(choice.licence) });
    } else {
      control = h('select', { className: 'media-select', dataset: { licence: '' }, attrs: { 'aria-label': 'Licence' } },
        listed.map((l) => h('option', {
          value: l.id, text: l.name || l.id, selected: l.id === choice.licence,
          title: (l.official_name ? l.official_name + '. ' : '') + (l.line || ''),
        })));
      control.addEventListener('change', () => { choice.licence = control.value; draw(); });
    }
    const el = h('div', { className: 'media-field media-licence', id: o.id || undefined }, [
      h('label', { className: 'media-licence-row' }, ['Licence ', control]),
      more,
    ]);
    function draw() {
      more.replaceChildren();
      const l = offeredLicence(L, choice.licence);
      if (l && l.line) more.appendChild(h('p', { className: 'media-hint', dataset: { licenceLine: '' }, text: l.line }));
      // Only a deed on https:// is linked; its words are the licence's name, as text.
      if (l && typeof l.url === 'string' && /^https:\/\//.test(l.url)) {
        more.appendChild(h('a', { href: l.url, target: '_blank', rel: 'noopener noreferrer', className: 'media-licence-deed', text: 'Read ' + (l.name || l.id) + ' in full' }));
      }
      // The game tick: only with a licence the server marks `game`; any other turns it off.
      const game = !!(l && l.game);
      if (!game) choice.project_use = false;
      const offer = (L.game_offer && typeof L.game_offer === 'object') ? L.game_offer : {};
      if (game && offer.label) {
        const tick = h('input', { type: 'checkbox', dataset: { gameOffer: '' }, checked: !!choice.project_use, disabled: !!o.ownerOnly });
        tick.addEventListener('change', () => { if (!o.ownerOnly) choice.project_use = !!tick.checked; });
        more.appendChild(h('label', { className: 'media-check', title: offer.line || '' }, [tick, ' ' + offer.label]));
      }
      if (o.ownerOnly) {
        more.appendChild(h('p', { className: 'media-hint', dataset: { ownerOnly: '' }, text: ownerOnlyWords(o.ownerOnly) }));
        return;
      }
      // The facts, stated plainly before Save (the operator's answer of 2026-10-12).
      const cc = !!(l && l.cc);
      const was = o.was ? offeredLicence(L, o.was) : null;
      const leaving = !!(was && was.cc && o.was !== choice.licence);
      if ((cc || leaving) && L.cc_warning) more.appendChild(h('p', { className: 'media-warn media-cc-warning', dataset: { ccWarning: '' }, text: L.cc_warning }));
      if (cc && typeof o.members === 'function' && o.members() && L.cc_members_warning) {
        more.appendChild(h('p', { className: 'media-warn media-cc-warning', dataset: { ccMembersWarning: '' }, text: L.cc_members_warning }));
      }
    }
    draw();
    return { el, refresh: draw };
  }

  /** The hosting paragraph ("Your uploads stay yours."), in the server's own words; null when it sent none. */
  function hostingBlock(h, licences) {
    const ho = licences && licences.hosting && typeof licences.hosting === 'object' ? licences.hosting : {};
    if (!ho.heading && !ho.text) return null;
    return h('div', { className: 'media-hosting', dataset: { hosting: '' } }, [
      ho.heading ? h('p', { className: 'media-hosting-heading', text: ho.heading }) : null,
      ho.text ? h('p', { className: 'media-hint', text: ho.text }) : null,
    ]);
  }

  /**
   * The Edit form for `item`, drawn into `box` (inside the item's page). `c` is the page's context
   * (media-item.js `createItemView`'s), `view` the item's page: `own(item)`, `takeItem(item)`,
   * `say(words)` (its status line), `closeEdit()`, `fileName(item, f)`, `filesOf(item)`, `current(id)`
   * and `send(path, purpose, item, body)` (a signed write that throws the relay's own words).
   * Returns {draw, drawFiles}: the form drawn once, and its files part drawn again as files are
   * added or taken off (the words being edited are the person's and stay as they are).
   */
  function createEditForm(c, view, item, box) {
    const h = c.h;
    const W = c.W;
    const own = view.own(item);
    const general = c.st.categories[0] ? c.st.categories[0].id : 1;
    const f = {
      was: item,
      title: item.title || '',
      description: item.description || '',
      category: item.category && item.category.id ? item.category.id : general,
      labels: new Set(Array.isArray(item.labels) ? item.labels : []),
      choice: { licence: item.licence || '', project_use: !!item.project_use },
      members: item.visibility === 'members',
      problem: '',
      saving: false,
    };
    const els = {};

    function draw() {
      const L = c.st.licences || {};
      const title = h('input', { type: 'text', id: 'media-edit-title', className: 'media-input', maxLength: 120, value: f.title });
      title.addEventListener('input', () => { f.title = title.value; });
      const category = h('select', { id: 'media-edit-category', className: 'media-select' },
        c.st.categories.map((cat) => h('option', { value: String(cat.id), text: cat.name, selected: String(cat.id) === String(f.category) })));
      const about = h('p', { id: 'media-edit-category-desc', className: 'media-hint' });
      const drawAbout = () => {
        const cat = c.st.categories.find((x) => String(x.id) === String(f.category));
        about.textContent = cat && cat.description ? cat.description : '';
        about.hidden = !about.textContent;
      };
      category.addEventListener('change', () => { f.category = Number(category.value); drawAbout(); });
      drawAbout();
      const description = h('textarea', { id: 'media-edit-description', className: 'media-input', maxLength: 2000, rows: 4, value: f.description });
      description.addEventListener('input', () => { f.description = description.value; });
      const kids = [
        h('h3', { text: 'Edit ' + c.lib.itemName(item, c.st.kinds) }),
        h('label', { className: 'media-field' }, ['Title', title]),
        c.st.categories.length ? h('label', { className: 'media-field' }, ['Category', category]) : null,
        about,
        h('label', { className: 'media-field' }, ['Description', description]),
      ];
      if (c.st.labels.length) {
        const ticks = h('fieldset', { className: 'media-field media-labels', id: 'media-edit-labels' }, [h('legend', { text: WORDS.labelsLegend })]);
        for (const l of c.st.labels) {
          const tick = h('input', { type: 'checkbox', value: l.id, dataset: { label: l.id }, checked: f.labels.has(l.id) });
          tick.addEventListener('change', () => { if (tick.checked) f.labels.add(l.id); else f.labels.delete(l.id); });
          ticks.appendChild(h('label', { className: 'media-check', title: l.desc || '' }, [tick, ' ' + l.label]));
        }
        kids.push(ticks);
      }
      // Who can see it: Add to Media's rules and words.
      let picker = null;
      if (item.keeps_file) {
        kids.push(h('p', { className: 'media-hint', text: W.alreadyPublic }));
      } else if (c.membersOnlyServer()) {
        kids.push(h('p', { className: 'media-hint', text: W.serverMembersOnly }));
      } else {
        const group = 'media-edit-visibility-' + item.id;
        const pub = h('input', { type: 'radio', name: group, value: 'public', id: 'media-edit-vis-public', checked: !f.members });
        const mem = h('input', { type: 'radio', name: group, value: 'members', id: 'media-edit-vis-members', checked: f.members });
        pub.addEventListener('change', () => { if (pub.checked) { f.members = false; if (picker) picker.refresh(); } });
        mem.addEventListener('change', () => { if (mem.checked) { f.members = true; if (picker) picker.refresh(); } });
        kids.push(h('fieldset', { className: 'media-field', id: 'media-edit-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      picker = licenceChoice(h, {
        licences: L, choice: f.choice, id: 'media-edit-licence', was: item.licence || '',
        members: () => c.membersOnlyServer() || f.members,
        ownerOnly: own ? null : (item.uploader_name || 'its uploader'),
      });
      kids.push(picker.el, hostingBlock(h, L));
      els.files = h('div', { id: 'media-edit-files', className: 'media-item-files' });
      kids.push(els.files);
      els.problem = h('p', { id: 'media-edit-problem', className: 'media-warn' });
      els.save = h('button', { type: 'button', id: 'media-edit-save', className: 'media-btn media-btn-primary', text: WORDS.save, on: { click: save } });
      kids.push(els.problem, h('div', { className: 'media-form-buttons' }, [
        els.save,
        h('button', { type: 'button', id: 'media-edit-cancel', className: 'media-btn', text: WORDS.cancel, title: 'Close this without changing anything.', on: { click: () => view.closeEdit() } }),
      ]));
      box.replaceChildren(h('div', { id: 'media-edit', className: 'media-panel media-edit' }, kids));
      drawFiles();
      drawProblem();
    }

    function drawProblem() {
      if (!els.problem) return;
      els.problem.textContent = f.problem;
      els.save.disabled = f.saving;
      els.save.textContent = f.saving ? WORDS.saving : WORDS.save;
    }

    /** The files part: the item's own first file (never removed here), each added file and the cover with Remove, the uploads on their way to it, and the uploader's Add a file and Cover picture. */
    function drawFiles() {
      if (!els.files) return;
      const now = view.current(item.id) || item;
      const files = view.filesOf(now);
      const kids = [h('h4', { text: 'Files' })];
      if (files[0]) kids.push(h('p', { className: 'media-hint', text: firstFileWords(view.fileName(now, files[0])) }));
      const rows = files.slice(1).map((x) => [x, 'file']).concat(now.cover ? [[now.cover, 'cover picture']] : []);
      for (const [x, what] of rows) {
        kids.push(h('div', { className: 'media-item-file-row', dataset: { editFile: x.file } }, [
          h('span', { className: 'media-item-file-name', text: view.fileName(now, x) + ', ' + what + ', ' + c.lib.sizeWords(x.size_bytes) }),
          h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { removeFile: x.file }, text: WORDS.remove, title: WORDS.removeTip, on: { click: () => removeFile(now, x) } }),
        ]));
      }
      for (const u of c.uploadsFor(item.id)) kids.push(h('p', { className: 'media-hint', dataset: { attachRow: '' }, text: u.heading + ': ' + u.words }));
      if (own) {
        const pick = (as) => {
          // Several files at once (2026-10-16); a cover is one picture.
          const input = h('input', { type: 'file', hidden: true, multiple: as === 'cover' ? undefined : true, dataset: { attachInput: as }, accept: as === 'cover' ? COVER_ACCEPT : undefined });
          input.addEventListener('change', async () => {
            const files = Array.from(input.files || []);
            try { input.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
            if (!files.length) return;
            // All of them against the share together first, so nothing starts that cannot finish.
            const together = c.together ? await c.together(files) : '';
            if (together) {
              f.problem = together;
            } else {
              const problems = [];
              for (const file of files) {
                const problem = await c.attach(file, now, as);
                if (problem) problems.push(files.length > 1 ? file.name + ': ' + problem : problem);
              }
              f.problem = problems.join(' ');
            }
            drawProblem();
            drawFiles();
          });
          return input;
        };
        const fileInput = pick('file');
        const coverInput = pick('cover');
        kids.push(h('div', { className: 'media-form-buttons' }, [
          h('button', { type: 'button', id: 'media-edit-add-file', className: 'media-btn', text: WORDS.addFile, title: 'Upload another file to go with this item.', on: { click: () => fileInput.click() } }),
          h('button', { type: 'button', id: 'media-edit-cover', className: 'media-btn', text: now.cover ? WORDS.replaceCover : WORDS.coverPicture, title: 'A picture shown on its card and page: PNG, JPEG, GIF or WebP.', on: { click: () => coverInput.click() } }),
          fileInput, coverInput,
        ]));
        kids.push(h('p', { className: 'media-hint', text: WORDS.uploadsHint }));
      }
      els.files.replaceChildren(...kids);
    }

    /** Take file `x` off the item (POST /api/media/files/remove, by its number and its own stored name), once confirmed. */
    async function removeFile(now, x) {
      if (!(await c.env.confirm(removeFileWords(view.fileName(now, x))))) return;
      try {
        const a = await view.send('/api/media/files/remove', 'media_file_remove:' + now.id + ':' + x.file, now, { file: x.file, file_name: x.filename });
        if (a && a.item) view.takeItem(a.item);
        view.say('Removed from the item.');
      } catch (e) {
        view.say(e && e.message ? e.message : String(e));
      }
    }

    /** Send the edit (and the visibility when it changed); the form closes once both are in. */
    async function save() {
      if (f.saving) return;
      const filename = c.lib.itemFileName(item);
      if (!filename) { f.problem = W.noFileName; drawProblem(); return; }
      // Someone who is not the uploader (an admin) sends the licence and the tick as they were.
      const L = c.st.licences || {};
      const choice = own ? f.choice : { licence: item.licence || L.default || 'all_rights_reserved', project_use: !!item.project_use };
      const labels = Array.from(f.labels).sort();
      let text;
      let purpose;
      try {
        text = editText({ title: f.title, description: f.description, category_id: f.category, labels, licence: choice.licence, project_use: choice.project_use });
        purpose = editPurpose(item.id, text);
      } catch (e) {
        f.problem = e.message;
        drawProblem();
        return;
      }
      f.saving = true;
      f.problem = '';
      drawProblem();
      try {
        const auth = await c.env.sign(purpose);
        if (!auth) throw new Error(W.cannotSign);
        const a = await c.lib.answerOf(await c.env.fetch('/api/media/edit', c.lib.jsonPost(editBody(auth, item.id, filename, text))));
        if (a && a.item) view.takeItem(a.item);
        // Who can see it goes by its own signed route, only when it changed and may change.
        const to = f.members ? 'members' : 'public';
        if (!item.keeps_file && !c.membersOnlyServer() && to !== (item.visibility === 'members' ? 'members' : 'public')) {
          const v = await view.send('/api/media/visibility', 'media_visibility:' + item.id + ':' + to, item, { visibility: to });
          if (v && v.item) view.takeItem(v.item);
        }
      } catch (e) {
        f.saving = false;
        f.problem = e && e.message ? e.message : String(e);
        drawProblem();
        return;
      }
      f.saving = false;
      view.closeEdit();
      view.say('Saved.');
    }

    draw();
    return { draw, drawFiles, state: f };
  }

  const api = { WORDS, COVER_ACCEPT, ownerOnlyWords, removeFileWords, firstFileWords, editText, editPurpose, editBody, offeredLicence, licenceChoice, hostingBlock, createEditForm };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaEdit = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
