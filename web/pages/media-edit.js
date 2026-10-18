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
// - the files, each with its type's badge before its name (2026-10-17,
//   `fileBadge`, which media-item.js and media-app.js draw with too): Remove
//   on each added file and the cover (the uploader and
//   admins), and, for the uploader, "Add a file" and "Cover picture". A picked
//   file is the same resumable upload as Add to Media's, listed under Uploads
//   in progress, and added to this item when it is up instead of published
//   (media-app.js `startAttach`, media-upload.js `attachUpload`). Files
//   dragged from the computer onto the page while the form is open go the same
//   way (`addFiles`, 2026-10-17); under an admin's Edit of someone else's item,
//   "Only Ann can add files to this item." stands where Add a file would be.
//   Every row, the item's own file included, has "Rename" (2026-10-17, the
//   operator: "Can we allow renaming of files in the edit window?"): the row
//   becomes a box holding the name without its ending, the ending after it,
//   fixed, then "Save name" and "Cancel" (Enter saves too), one row at a time
//   (POST /api/media/files/rename, signed over `media_file_rename:<id>:<file>:
//   <BLAKE3 hex of the name exactly as sent>`; the relay keeps the ending). For
//   the uploader, a picture already on the item has "Use as cover" (POST
//   /api/media/files/cover, `media_file_cover:<id>:<file>`; the operator: "I
//   uploaded a png with the blend and STL files but, I can't seem to assign it
//   as the preview photo"), never on the item's own file or the cover there now.
//   The desktop's media/edit.rs draw_files is the original, words and all.
//
// The form is in three parts in the chat sidebar's colours, darker (2026-10-17,
// `tintSection`, /shared/tint-section.css; the desktop's Edit window): "About
// it" (red: the title, the category and its description, the description, the
// content labels), "Who can see it" (green: who sees it, the licence and its
// words, the hosting paragraph) and "Files" (blue). Save and Cancel stay under
// them. Add to Media (media-app.js) uses the same three, its files first.
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
// media-app.js; it needs /shared/blake3.js and /shared/file-risk.js), a CommonJS module under Node,
// where scripts/tests/media-web-item.test.js runs it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // BLAKE3 in plain JavaScript (/shared/blake3.js): the edit's purpose names the hash of its text.
  const B3 = typeof module === 'object' && module && module.exports ? require('../shared/blake3.js') : root.HosBlake3;
  // What kind of file a file is (/shared/file-risk.js, 2026-10-17): under Node its module; in the
  // browser the page's HosFileRisk, looked up when a badge is drawn.
  const FR_MODULE = typeof module === 'object' && module && module.exports ? require('../shared/file-risk.js') : null;
  const fileRisk = () => FR_MODULE || root.HosFileRisk || null;

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
    // The three parts of Edit and of Add to Media (the desktop's media/edit.rs ABOUT_IT,
    // WHO_CAN_SEE_IT and FILES), each in one of the chat sidebar's colours, darker.
    aboutIt: 'About it',
    whoCanSeeIt: 'Who can see it',
    files: 'Files',
    // A file row's buttons and what the page says once they are answered (2026-10-17; the
    // desktop's media/edit.rs RENAME, RENAME_TIP, SAVE_NAME, USE_AS_COVER, USE_AS_COVER_TIP and
    // media/item_view.rs FILE_RENAMED, COVER_CHOSEN).
    rename: 'Rename',
    renameTip: 'Change the name people see and download it as. Its ending stays.',
    saveName: 'Save name',
    useAsCover: 'Use as cover',
    useAsCoverTip: "Show this picture on the item's card and page. The cover there now becomes one of its files.",
    renamed: 'Renamed.',
    coverChosen: 'That picture is the cover now.',
    noHashName: 'This page could not load its hashing code, so the name was not sent. Reload the page and try again.',
  });

  // ── A file's type, always, just before its name (2026-10-17) ──
  // The operator: "highlight the file type so people ALWAYS know what file type they're looking
  // at ... protect people inexperienced with technology and possibly illiterate people." The badge
  // is HosFileRisk.badgeHtml's shape (/shared/file-risk.js; styled by /shared/file-badge.css; the
  // desktop app's gui/widgets/file_badge.rs), built here with createElement and textContent, as
  // everything on the Media page is: a drawing, the type's word and the file's ending, coloured by
  // how careful to be. Before the list of kinds is in it is a plain grey "File"; fillFileBadges
  // draws each one again once it is.

  const SVG_NS = 'http://www.w3.org/2000/svg';

  /**
   * A badge's drawing as SVG elements (createElementNS), from HosFileRisk.iconSvg: its own markup of
   * numbers and fixed words, read tag by tag, never handed to an HTML parser. Null without SVG.
   */
  function badgeIcon(doc, FR, icon) {
    if (!doc || typeof doc.createElementNS !== 'function' || !FR || typeof FR.iconSvg !== 'function') return null;
    let svg = null;
    for (const m of String(FR.iconSvg(icon, 16)).matchAll(/<(svg|rect|line|circle|polygon|polyline)\b([^>]*?)\/?>/g)) {
      const el = doc.createElementNS(SVG_NS, m[1]);
      for (const a of m[2].matchAll(/([\w:-]+)="([^"]*)"/g)) el.setAttribute(a[1], a[2]);
      if (svg) svg.appendChild(el);
      else if (m[1] === 'svg') svg = el;
      else return null;
    }
    return svg;
  }

  /**
   * The badge of a file named `name` (plain text; `head` its first bytes, or null): a span of class
   * `file-badge level-<view|care|danger|unknown>` (and `disguised` for a file named like one kind
   * over the bytes of another), carrying `data-file-badge` (its name) and `data-file-type`, holding
   * its drawing, the type's word (`file-badge-label`) and the file's ending (`file-badge-ending`).
   */
  function fileBadge(h, doc, name, head) {
    const FR = fileRisk();
    const shown = String(name == null ? '' : name);
    const t = FR && typeof FR.fileType === 'function'
      ? FR.fileType(shown, head || null)
      : { id: 'unknown', label: 'File', icon: 'unknown', level: 'unknown', ending: '', disguised: false };
    const level = ['view', 'care', 'danger', 'unknown'].includes(t.level) ? t.level : 'unknown';
    return h('span', {
      className: 'file-badge level-' + level + (t.disguised ? ' disguised' : ''),
      dataset: { fileBadge: shown, fileType: t.id },
      title: t.disguised ? t.label + ', pretending to be something else' : t.label,
    }, [
      badgeIcon(doc, FR, t.icon),
      h('span', { className: 'file-badge-label', text: t.label }),
      t.ending ? h('span', { className: 'file-badge-ending', text: t.ending }) : null,
    ]);
  }

  /** Draw every badge under `root` again, from the list of kinds (a badge drawn before it came in). */
  function fillFileBadges(h, doc, root) {
    if (!root || typeof root.querySelectorAll !== 'function') return;
    for (const old of Array.from(root.querySelectorAll('[data-file-badge]'))) {
      const parent = old.parentNode;
      if (!parent) continue;
      parent.insertBefore(fileBadge(h, doc, old.dataset.fileBadge), old);
      old.remove();
    }
  }

  /** Is the list of kinds in (so a badge drawn now is its type, not the grey "File")? */
  function fileTypesKnown() {
    const FR = fileRisk();
    return !!(FR && typeof FR.ready === 'function' && FR.ready());
  }

  /** Ask for the list of kinds (`fetchFn`: the page's fetch); resolves true once it is in. */
  function loadFileTypes(fetchFn) {
    const FR = fileRisk();
    if (!FR || typeof FR.load !== 'function') return Promise.resolve(false);
    return Promise.resolve(FR.load(fetchFn)).then(() => fileTypesKnown(), () => false);
  }

  /**
   * One part of a window in one of the chat sidebar's colours, darker (/shared/tint-section.css;
   * the desktop's gui/widgets/tint_section.rs): `tint` red, green or blue, its `title` on top, then
   * `kids`.
   */
  function tintSection(h, tint, title, kids) {
    return h('div', { className: 'tint-section tint-' + tint, dataset: { tint } },
      [h('div', { className: 'tint-section-title', text: title })].concat(kids || []));
  }
  // The picture types a cover can be (the relay: "A cover must be a picture: PNG, JPEG, GIF or WebP.").
  const COVER_ACCEPT = 'image/png,image/jpeg,image/gif,image/webp';

  /** "Only Ann can change these." (decision 1: only the rights holder changes a licence). */
  function ownerOnlyWords(name) {
    return 'Only ' + name + ' can change these.';
  }
  /** Under an admin's Edit of someone else's item, where Add a file would be (2026-10-17; the desktop's media/edit.rs words). */
  function addFilesOwnerOnlyWords(name) {
    return 'Only ' + name + ' can add files to this item.';
  }
  /** What a file's Remove asks first. */
  function removeFileWords(name) {
    return 'Remove ' + name + ' from this item? It is deleted from the server.';
  }
  /** The line about the item's own first file, which is never removed here. */
  function firstFileWords(name) {
    return name + ': the item itself. To take it away, remove the item.';
  }

  // ── Renaming a file (2026-10-17) ──
  /** ASCII letters to lower case and nothing else (Rust's to_ascii_lowercase): the length never changes. */
  const asciiLower = (s) => String(s).replace(/[A-Z]/g, (ch) => ch.toLowerCase());
  /**
   * A stored file's ending, as the relay keeps it on a rename (storage/media_files.rs
   * `shown_name_for`; the desktop's media/edit.rs `stored_ending`): "tar.gz" for a tarball, else its
   * last, lower case; '' when it has none.
   */
  function storedEnding(stored) {
    const lower = asciiLower(stored == null ? '' : stored);
    if (lower.endsWith('.tar.gz')) return 'tar.gz';
    const dot = lower.lastIndexOf('.');
    return dot >= 0 ? lower.slice(dot + 1) : '';
  }
  /** `name` without its `ending`, for the rename box (the ending is shown after it, fixed; the desktop's `stem_of`). */
  function stemOf(name, ending) {
    const n = String(name == null ? '' : name);
    return ending && asciiLower(n).endsWith('.' + ending) ? n.slice(0, n.length - ending.length - 1) : n;
  }
  /** What Save name sends: the box's text, trimmed, with "." and the ending put back. */
  function nameWithEnding(typed, ending) {
    const t = String(typed == null ? '' : typed).trim();
    return ending ? t + '.' + ending : t;
  }
  /** `media_file_rename:<id>:<file>:<BLAKE3 hex of the name's UTF-8 bytes, exactly as sent>` (relay media_edit.rs). */
  function renamePurpose(id, file, name) {
    if (!B3) throw new Error(WORDS.noHashName);
    return 'media_file_rename:' + id + ':' + file + ':' + B3.toHex(B3.hash(new TextEncoder().encode(name)));
  }
  /** `media_file_cover:<id>:<file>` (relay media_edit.rs). */
  function coverPurpose(id, file) {
    return 'media_file_cover:' + id + ':' + file;
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
   * `say(words)` (its status line), `closeEdit()`, `fileName(item, f)`, `shownName(item, f)` (the name a
   * file is shown by: plain, so it cannot hide its ending), `filesOf(item)`, `current(id)`
   * and `send(path, purpose, item, body)` (a signed write that throws the relay's own words).
   * Returns {draw, drawFiles}: the form drawn once, and its files part drawn again as files are
   * added or taken off (the words being edited are the person's and stay as they are).
   */
  function createEditForm(c, view, item, box) {
    const h = c.h;
    const W = c.W;
    const own = view.own(item);
    // The uploader's name, for the words saying only they can change the licence or add files.
    const ownerName = item.uploader_name || 'its uploader';
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
      // The row being renamed: {file, typed (the box's text), fresh (just opened: focus it)}, or null.
      renaming: null,
      // A rename or a Use as cover on its way: their buttons wait for its answer.
      fileBusy: false,
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
      // Three parts in the chat sidebar's colours, darker (2026-10-17, tintSection; the desktop's
      // Edit window): what it is (red), who can see it (green), its files (blue). Save and Cancel
      // stay outside them, under them.
      const aboutParts = [
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
        aboutParts.push(ticks);
      }
      // Who can see it: Add to Media's rules and words.
      const who = [];
      let picker = null;
      if (item.keeps_file) {
        who.push(h('p', { className: 'media-hint', text: W.alreadyPublic }));
      } else if (c.membersOnlyServer()) {
        who.push(h('p', { className: 'media-hint', text: W.serverMembersOnly }));
      } else {
        const group = 'media-edit-visibility-' + item.id;
        const pub = h('input', { type: 'radio', name: group, value: 'public', id: 'media-edit-vis-public', checked: !f.members });
        const mem = h('input', { type: 'radio', name: group, value: 'members', id: 'media-edit-vis-members', checked: f.members });
        pub.addEventListener('change', () => { if (pub.checked) { f.members = false; if (picker) picker.refresh(); } });
        mem.addEventListener('change', () => { if (mem.checked) { f.members = true; if (picker) picker.refresh(); } });
        who.push(h('fieldset', { className: 'media-field', id: 'media-edit-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      picker = licenceChoice(h, {
        licences: L, choice: f.choice, id: 'media-edit-licence', was: item.licence || '',
        members: () => c.membersOnlyServer() || f.members,
        ownerOnly: own ? null : ownerName,
      });
      who.push(picker.el, hostingBlock(h, L));
      els.files = h('div', { id: 'media-edit-files', className: 'media-item-files' });
      const kids = [
        h('h3', { text: 'Edit ' + c.lib.itemName(item, c.st.kinds) }),
        tintSection(h, 'red', WORDS.aboutIt, aboutParts),
        tintSection(h, 'green', WORDS.whoCanSeeIt, who),
        tintSection(h, 'blue', WORDS.files, [els.files]),
      ];
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

    /** File `x`'s own stored name, bare (the item's own file: the item's). */
    function storedOf(now, x) {
      const s = String((x && x.filename) || (x && x.file === 0 ? c.lib.itemFileName(now) : '') || '').trim();
      return s.replace(/^\/uploads\//, '');
    }

    /**
     * File `x`'s row while it is being renamed: its badge, the box holding its name without its
     * ending, the ending after it (fixed), Save name and Cancel. Enter in the box saves too.
     */
    function renameRow(now, x, name, first) {
      const doc = c.env.document;
      const ending = storedEnding(storedOf(now, x));
      const box = h('input', {
        type: 'text', className: 'media-input', maxLength: 120, value: f.renaming.typed,
        dataset: { renameInput: x.file }, attrs: { 'aria-label': 'New name for ' + name },
      });
      box.addEventListener('input', () => { if (f.renaming) f.renaming.typed = box.value; });
      box.addEventListener('keydown', (e) => {
        if (!e || e.key !== 'Enter') return;
        if (typeof e.preventDefault === 'function') e.preventDefault();
        saveName(now, x);
      });
      const row = h('div', { className: 'media-item-file-row' + (first ? ' media-edit-first-file' : ''), dataset: first ? { renaming: x.file } : { editFile: x.file, renaming: x.file } }, [
        fileBadge(h, doc, name),
        box,
        ending ? h('span', { className: 'media-hint', dataset: { renameEnding: '' }, text: '.' + ending }) : null,
        h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-primary', dataset: { saveName: x.file }, text: WORDS.saveName, disabled: f.fileBusy, on: { click: () => saveName(now, x) } }),
        h('button', { type: 'button', className: 'media-btn media-btn-small', dataset: { cancelRename: x.file }, text: WORDS.cancel, on: { click: () => { f.renaming = null; drawFiles(); } } }),
      ]);
      return { row, box };
    }

    /**
     * The files part (titled "Files" by its section): the item's own first file (never removed
     * here), each added file and the cover with Remove, the uploads on their way to it, and the
     * uploader's Add a file and Cover picture. Each file's type first (its badge), so its row says
     * only "name, size" ("name, cover picture, size" for the cover), as the desktop's does. Every
     * row has Rename; a picture that is not the cover has Use as cover, for the uploader.
     */
    function drawFiles() {
      if (!els.files) return;
      const now = view.current(item.id) || item;
      const files = view.filesOf(now);
      const doc = c.env.document;
      const kids = [];
      // The rename box drawn again (an upload moving on draws this part again) keeps its focus and
      // where the cursor was in it.
      const was = els.renameBox && doc && doc.activeElement === els.renameBox
        ? [els.renameBox.selectionStart, els.renameBox.selectionEnd] : null;
      els.renameBox = null;
      const renameButton = (x) => h('button', {
        type: 'button', className: 'media-btn media-btn-small', dataset: { renameFile: x.file }, text: WORDS.rename, title: WORDS.renameTip,
        on: { click: () => { f.renaming = { file: x.file, typed: stemOf(view.shownName(now, x), storedEnding(storedOf(now, x))), fresh: true }; drawFiles(); } },
      });
      const rows = (files[0] ? [[files[0], false]] : []).concat(files.slice(1).map((x) => [x, false]), now.cover ? [[now.cover, true]] : []);
      for (const [x, cover] of rows) {
        // Shown by its plain name (2026-10-17): a direction mark cannot hide its real ending.
        const name = view.shownName(now, x);
        const first = x.file === 0;
        if (f.renaming && f.renaming.file === x.file) {
          const r = renameRow(now, x, name, first);
          els.renameBox = r.box;
          kids.push(r.row);
          continue;
        }
        if (first) {
          kids.push(h('div', { className: 'media-item-file-row media-edit-first-file' }, [
            fileBadge(h, doc, name),
            h('span', { className: 'media-hint', text: firstFileWords(name) }),
            renameButton(x),
          ]));
          continue;
        }
        kids.push(h('div', { className: 'media-item-file-row', dataset: { editFile: x.file } }, [
          fileBadge(h, doc, name),
          h('span', { className: 'media-item-file-name', text: name + (cover ? ', cover picture, ' : ', ') + c.lib.sizeWords(x.size_bytes) }),
          renameButton(x),
          // A picture already on the item can be its cover: the uploader's, as adding a cover is.
          own && !cover && x.kind === 'picture'
            ? h('button', { type: 'button', className: 'media-btn media-btn-small', dataset: { useAsCover: x.file }, text: WORDS.useAsCover, title: WORDS.useAsCoverTip, disabled: f.fileBusy, on: { click: () => useAsCover(now, x) } })
            : null,
          h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', dataset: { removeFile: x.file }, text: WORDS.remove, title: WORDS.removeTip, on: { click: () => removeFile(now, x) } }),
        ]));
      }
      for (const u of c.uploadsFor(item.id)) kids.push(h('p', { className: 'media-hint', dataset: { attachRow: '' }, text: u.heading + ': ' + u.words }));
      if (!own) {
        // Only the uploader adds files (an admin edits the words, never the files' owner's set).
        kids.push(h('p', { className: 'media-hint', id: 'media-edit-files-owner-only', text: addFilesOwnerOnlyWords(ownerName) }));
      }
      if (own) {
        const pick = (as) => {
          // Several files at once (2026-10-16); a cover is one picture.
          const input = h('input', { type: 'file', hidden: true, multiple: as === 'cover' ? undefined : true, dataset: { attachInput: as }, accept: as === 'cover' ? COVER_ACCEPT : undefined });
          input.addEventListener('change', () => {
            const files = Array.from(input.files || []);
            try { input.value = ''; } catch (e) { /* some browsers refuse; harmless */ }
            addFiles(files, as);
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
      const box = els.renameBox;
      if (box && (was || (f.renaming && f.renaming.fresh))) {
        try {
          box.focus();
          if (was && typeof box.setSelectionRange === 'function') box.setSelectionRange(was[0], was[1]);
          else if (!was && typeof box.select === 'function') box.select();
        } catch (e) { /* the box is still there to click */ }
        f.renaming.fresh = false;
      }
    }

    /**
     * Save name: file `x` named the box's text with its ending put back (POST
     * /api/media/files/rename, signed over the BLAKE3 of the name exactly as sent). Answered, the
     * item is the answer's and the page says "Renamed."; refused, the relay's words stand in the
     * problem line and the box stays open with what was typed.
     */
    async function saveName(now, x) {
      if (!f.renaming || f.renaming.file !== x.file || f.fileBusy) return;
      const name = nameWithEnding(f.renaming.typed, storedEnding(storedOf(now, x)));
      let purpose;
      try {
        purpose = renamePurpose(now.id, x.file, name);
      } catch (e) {
        f.problem = e.message;
        drawProblem();
        return;
      }
      f.fileBusy = true;
      drawFiles();
      let a;
      try {
        a = await view.send('/api/media/files/rename', purpose, now, { file: x.file, file_name: storedOf(now, x), name });
      } catch (e) {
        f.fileBusy = false;
        f.problem = e && e.message ? e.message : String(e);
        drawProblem();
        drawFiles();
        return;
      }
      f.fileBusy = false;
      f.renaming = null;
      f.problem = '';
      drawProblem();
      if (a && a.item) view.takeItem(a.item); else drawFiles();
      view.say(WORDS.renamed);
    }

    /**
     * Use as cover: picture file `x`, already on the item, made its cover (POST
     * /api/media/files/cover); the cover there now becomes one of its files. Answered, the page says
     * "That picture is the cover now."; refused, the relay's words stand in the problem line.
     */
    async function useAsCover(now, x) {
      if (!own || f.fileBusy) return;
      f.fileBusy = true;
      drawFiles();
      let a;
      try {
        a = await view.send('/api/media/files/cover', coverPurpose(now.id, x.file), now, { file: x.file, file_name: storedOf(now, x) });
      } catch (e) {
        f.fileBusy = false;
        f.problem = e && e.message ? e.message : String(e);
        drawProblem();
        drawFiles();
        return;
      }
      f.fileBusy = false;
      f.problem = '';
      drawProblem();
      if (a && a.item) view.takeItem(a.item); else drawFiles();
      view.say(WORDS.coverChosen);
    }

    /**
     * Add files to this item, as its Add a file (`as` 'file') or Cover picture ('cover') does: the
     * chosen ones, and files dropped on the page while this form is open (2026-10-17,
     * media-app.js dropped). All of them against the share together first, so nothing starts that
     * cannot finish; then each the same resumable upload as Add to Media's, attached when it is up.
     */
    async function addFiles(files, as) {
      if (!own || !files || !files.length) return;
      const now = view.current(item.id) || item;
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
    }

    /** Take file `x` off the item (POST /api/media/files/remove, by its number and its own stored name), once confirmed. */
    async function removeFile(now, x) {
      if (!(await c.env.confirm(removeFileWords(view.shownName(now, x))))) return;
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
    // addFiles: what files dropped on the page do while this form is open (null when only the
    // uploader could add them, with addFilesWhyNot saying so).
    return { draw, drawFiles, state: f, addFiles: own ? addFiles : null, addFilesWhyNot: own ? '' : addFilesOwnerOnlyWords(ownerName) };
  }

  const api = {
    WORDS, COVER_ACCEPT, ownerOnlyWords, addFilesOwnerOnlyWords, removeFileWords, firstFileWords, editText, editPurpose, editBody, offeredLicence, licenceChoice, hostingBlock, createEditForm,
    fileBadge, fillFileBadges, fileTypesKnown, loadFileTypes, tintSection,
    storedEnding, stemOf, nameWithEnding, renamePurpose, coverPurpose,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaEdit = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
