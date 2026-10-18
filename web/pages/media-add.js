// ── media-add.js ──────────────────────────────────────────────────────────
// Add to Media on the website (moved out of web/pages/media-app.js 2026-10-17, verbatim, when the
// page passed its line budget in tests/file_size_ratchet.rs): the form for a picked file (Add to
// Media), a Shared Files row (Put on Media) and an upload that waits (Publish an upload), in three
// tinted parts (Files, About it, Who can see it); each chosen file's row with its type's badge; what
// is hidden in a picture and the switch; the person's share; the licence; the uploads in progress
// with their bars, Pause, Resume and Cancel; publishing; a file picked in an item's Edit, added to
// it instead; and "Also post in chat" (2026-10-17, the desktop's media/add.rs): the tick, the
// channel (the one open in the chat first, then favourites, then the rest), Add to favorites, and
// the post once published, signed as the chat signs, the chat's recovery-phrase guard first. Also
// the page's side of the upload window (/shared/upload-window.js): files handed over by the page
// it is over, held until this page may upload, and what it tells that page.
//
// The Media page (media-app.js) makes it with createAddForm, handing it its state, its parts and
// the functions it shares (signing in, the share, the status line); the form is the page's own,
// in its own file. A classic script in the browser (MediaAdd on window), loaded before
// media-app.js; a CommonJS module under Node, where media-app.js requires it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The recovery-phrase guard's words (/shared/warnings.js), said when "Also post in chat" is held
  // back by it, as the chat says them for anything it holds back.
  const WARN = typeof module === 'object' && module && module.exports ? require('../shared/warnings.js') : root;
  const PHRASE_HELD = (WARN && WARN.PHRASE_GUARD_SENTENCE) || 'it holds your recovery phrase.';

  // ── "Also post in chat" (the operator, 2026-10-17; the desktop's media/add.rs draw_post_row,
  // post_choices, post_text and post_published, in its words) ──
  // "If we're also in a chat channel ... upon clicking upload it also posts the image with the
  // title and description to the general chat. However we should add an ability to choose the
  // channel that gets posted to. Maybe like current first and then favorites." And: "Maybe have a
  // checkbox to the left of the channel info for toggling the channel posting on/off. Gray out the
  // channel controls when they're disabled."
  const POST_WORDS = Object.freeze({
    tick: 'Also post in chat',
    tickTip: 'When it is up, a card for it is posted in this channel: its title, then its picture, description and every file when opened.',
    none: 'Also post in chat: this server has no channel to post in.',
    addFavorite: 'Add to favorites',
    removeFavorite: 'Remove from favorites',
    favoriteTip: 'Favorite channels are listed first, after the one open in the chat.',
    openInChat: ' (open in the chat)',
    notConnected: 'the chat is not connected to its server.',
    notAChannel: 'it is not one of this server\'s channels.',
    cannotSign: 'this page could not sign it with your identity.',
    cannotCheck: 'this page could not check it for your recovery phrase, so it was kept back.',
    noAnswer: 'the server did not answer.',
  });
  /** The star before a favourite channel (U+2B50, no variation selector: the desktop's). */
  const STAR = '⭐';
  // The person's favourite channels, on this device only (localStorage), each "<server>\n<channel id>".
  const POST_FAVORITES_KEY = 'hos_post_favorites';
  // The channel the chat in this browser showed last (web/chat/app.js keeps it under this name).
  const CHAT_CHANNEL_KEY = 'humanity_channel';
  // After a line from the server that may be a post's refusal, how long its echo is still waited for:
  // a line about something else can arrive meanwhile.
  const POST_GRACE_MS = 1500;

  /** How a favourite channel is kept: its server and its id (the desktop's post_key). */
  function postFavoriteKey(server, channel) {
    return String(server || '') + '\n' + String(channel || '');
  }
  /** The favourites kept in this browser (a Set of postFavoriteKey); empty when storage is refused. */
  function postFavorites(storage) {
    try {
      const list = JSON.parse((storage && storage.getItem(POST_FAVORITES_KEY)) || '[]');
      return new Set(Array.isArray(list) ? list.filter((k) => typeof k === 'string') : []);
    } catch (e) {
      return new Set();
    }
  }
  /** Add a favourite, or take it away; the new set (unchanged when storage is refused). */
  function togglePostFavorite(storage, key) {
    const set = postFavorites(storage);
    if (set.has(key)) set.delete(key); else set.add(key);
    try { if (storage) storage.setItem(POST_FAVORITES_KEY, JSON.stringify(Array.from(set))); } catch (e) { /* kept for this page only */ }
    return set;
  }
  /**
   * The channels an upload can be posted in, as [{id, words}] (the desktop's post_choices): the one
   * open in the chat first ("#general (open in the chat)"), then the person's favourites ("⭐
   * #printing"), then the rest ("#random"); a read-only channel only for an admin. `opts`: open (the
   * chat's channel id), admin, isFavorite(id).
   */
  function postChoices(channels, opts) {
    const o = opts || {};
    const fav = typeof o.isFavorite === 'function' ? o.isFavorite : () => false;
    const usable = (Array.isArray(channels) ? channels : []).filter((c) => c && typeof c.id === 'string' && c.id && (o.admin || !c.read_only));
    const name = (c) => '#' + (c.name || c.id);
    const out = [];
    const open = usable.find((c) => c.id === o.open);
    if (open) out.push({ id: open.id, words: name(open) + POST_WORDS.openInChat });
    for (const c of usable) if (c !== open && fav(c.id)) out.push({ id: c.id, words: STAR + ' ' + name(c) });
    for (const c of usable) if (c !== open && !fav(c.id)) out.push({ id: c.id, words: name(c) });
    return out;
  }
  /**
   * What "Also post in chat" posts for a published item (the desktop's post_text, 2026-10-17): its
   * own address on this server's Media page, `{origin}/media/{id}`, public and members-only alike.
   * Both apps show it as the item's card (web/chat/chat-media-card.js): its title, how many files
   * and how much, opening out to its picture, description and every file, and its full window on a
   * click. The operator: "I'd rather people know exactly what they're downloading instead of
   * guessing." A members-only item's page opens for members only.
   */
  function postText(item, origin) {
    return String(origin || '').replace(/\/+$/, '') + '/media/' + (item ? item.id : '');
  }
  /** "Posted in #general." and "Not posted in #general: why" (the desktop's words). */
  const postedIn = (name) => 'Posted in ' + name + '.';
  const notPostedIn = (name, why) => 'Not posted in ' + name + ': ' + why;

  /**
   * The form, made by the Media page (media-app.js createMediaPage) with `c`: its state `st`, its
   * parts `els`, `h` and `doc`, its `env`, `session`, `myKey`, `upWin` (the upload window's
   * connector, or null), `itemWin` (the item window's, or null: told only whether an upload is
   * going, so it stays while an Edit's file goes up), `cleaner`, `later`, `cancelLater`, `now`, `Abort`, `itemView`; the page's
   * own functions it shares (canUpload, bound, token, sessionState, knownCannotUpload, isAdmin,
   * membersOnlyServer, loadMine, load, say, renderWaiting, sayCancelled); and `lib`, the page's
   * words and helpers (MEDIA_WORDS and the rest, named below).
   */
  function createAddForm(c) {
    const { st, els, h, doc, env, session, myKey, upWin, itemWin, cleaner, later, cancelLater, now, Abort, itemView } = c;
    const { canUpload, bound, token, sessionState, knownCannotUpload, isAdmin, membersOnlyServer, loadMine, load, say, renderWaiting, sayCancelled } = c;
    const { MEDIA_WORDS, MORE_LEAD, ANSWER_WAIT_MS, DEFAULT_LICENCE, num, sizeWords, shareLeftWords, limitStop, shareStop, budgetStop,
      togetherStop, resetsWords, storedName, itemFileName, itemName, purposes, jsonPost, answerOf, findUpload, createUploader,
      fingerprint, U, ED } = c.lib;
    let tails = 0;              // published items whose post in chat and other files are still being started
    const postWaiters = new Set(); // what each of them waits for (the session's frames and state)
    let uploadSeq = 0;

    // ── Add to Media, Put on Media, and publishing an upload that waits ──
    //
    // `st.form` is the one form not yet uploading. Pressing "Upload and add" moves that form,
    // fields and all, into the uploads list (`st.uploads`, drawn in #media-uploads): there it
    // shows its own bar, words, Pause, Resume and Cancel, and publishes ITSELF once its file is
    // up, with what was typed in it (round 3, review 2). So the form area is free again at once,
    // and opening another form never leaves an upload without the form that publishes it.

    /** Phases in which an upload is still under way (not ended, not being cancelled). */
    function uploadLive(f) {
      const p = f && f.up ? f.up.state.phase : '';
      return !!p && p !== 'failed' && p !== 'cancelling' && p !== 'cancelled' && p !== 'done';
    }
    const sameFile = (a, b) => !!(a && b && a.name === b.name && a.size === b.size && a.lastModified === b.lastModified);

    /**
     * What goes up for an Add to Media form, as its switch stands (part 1): {file, keepMetadata,
     * refused}. The Media page is public, so a file that could not be checked is never added unless
     * the person keeps its details by choice; `refused` holds the words saying so.
     */
    function sendingOf(f) {
      if (!f || !f.clean || !cleaner) return { file: f ? f.file : null, keepMetadata: false, refused: null };
      const d = cleaner.decide(f.clean, { where: 'public', remove: f.remove !== false, protectedOn: !!f.protectedOn });
      if (d.action === 'send') return { file: d.file, keepMetadata: d.keepMetadata, refused: null };
      return { file: null, keepMetadata: false, refused: d.words };
    }

    /** Does the form show what the file holds and the switch: a picture or a video, or a file that could not be checked. */
    function detailsShown(f) {
      const c = f && f.clean;
      return !!(c && cleaner && f.mode === 'add' && (c.unchecked || c.summary || ['jpeg', 'png', 'webp', 'mp4', 'heif'].includes(c.kind)));
    }

    /** The form's part on what the file holds: its hidden details, the switch, and why it cannot go. */
    function detailsBox(f) {
      const W = cleaner.WORDS;
      const c = f.clean;
      const kids = [];
      if (c.summary) {
        kids.push(h('p', { id: 'media-form-hidden', className: 'media-hint', text: c.summary }));
        kids.push(h('ul', { className: 'media-hidden-details' }, (c.details || []).map((d) => h('li', { text: d.words }))));
      }
      f.detailsSwitch = null;
      if (f.protectedOn) {
        kids.push(h('p', { id: 'media-form-protected', className: 'media-hint', text: W.protectedLine }));
      } else {
        const box = h('input', { type: 'checkbox', id: 'media-form-remove-details', checked: f.remove !== false, disabled: !!f.up });
        box.addEventListener('change', () => {
          if (f.up) return;
          f.remove = !!box.checked;
          drawDetailsWords(f);
          renderFormActions(f);
        });
        f.detailsSwitch = box;
        kids.push(h('label', { className: 'media-check' }, [box, ' ' + W.switchLabel]));
      }
      f.offEl = h('p', { id: 'media-form-details-off', className: 'media-hint' });
      f.refusedEl = h('p', { id: 'media-form-unchecked', className: 'media-warn' });
      kids.push(f.offEl, f.refusedEl);
      drawDetailsWords(f);
      return h('div', { id: 'media-form-details', className: 'media-field media-details' }, kids);
    }

    function drawDetailsWords(f) {
      if (!f.offEl || !cleaner) return;
      f.offEl.textContent = !f.protectedOn && f.remove === false ? cleaner.WORDS.switchOffLine : '';
      f.refusedEl.textContent = sendingOf(f).refused || '';
    }

    /**
     * Add to Media for `file` (and `more` with it). `postTo`: a channel to tick "Also post in chat"
     * for (files dropped on that channel of the chat, handed to the upload window); '' for none.
     */
    async function openAddForm(file, more, postTo) {
      const f = { mode: 'add', file, more: more || [], name: file.name, stop: null, shareLine: '', checking: true, busy: false, error: '', uploadedName: '', up: null, fp: '' };
      f.postOn = !!postTo;
      f.postTo = postTo || '';
      st.form = f;
      renderForm();
      if (!canUpload()) {
        f.checking = false;
        f.stop = bound() ? MEDIA_WORDS.cannotUpload : MEDIA_WORDS.notSignedInUpload;
        renderForm();
        return;
      }
      // The same file already going up on this page: never two uploads of it at once.
      if (st.uploads.some((u) => sameFile(u.file, file) && uploadLive(u))) {
        f.checking = false;
        f.stop = '"' + file.name + '"' + MEDIA_WORDS.alreadyUploading;
        renderForm();
        return;
      }
      // Cleaned on this device first (part 1): what it holds, for the form's words and switch.
      if (cleaner === false) {
        f.checking = false;
        f.stop = MEDIA_WORDS.checkerMissing;
        renderForm();
        return;
      }
      if (cleaner) {
        f.protectedOn = cleaner.protectedSetupOn(env.storage);
        f.remove = cleaner.removeDetailsChosen(env.storage);
        f.clean = await cleaner.cleanFile(file);
        if (st.form !== f) return;
      }
      // Its fingerprint (the first and last megabyte), which a resume is matched by: of the file that
      // goes up (the cleaned one, unless the details are kept).
      f.fpFile = sendingOf(f).file || file;
      try { f.fp = fingerprint ? await fingerprint(f.fpFile, env.digest) : ''; } catch (e) { f.fp = ''; }
      if (st.form !== f) return;
      // Until the relay has said what is left, nothing can be uploaded, so an upload over it never starts.
      const mine = await loadMine();
      if (st.form !== f) return;
      f.checking = false;
      if (mine) {
        // An upload of this same file already part way up is counted by the relay as in progress
        // already; its own bytes are not "more" on top of that.
        const known = f.fp ? findUpload(env.storage, myKey, f.fpFile, f.fp, now()) : null;
        const inProgress = Math.max(0, num(mine.in_progress_bytes) - (known ? file.size : 0));
        const used = num(mine.used_bytes) + inProgress;
        const share = num(mine.share_bytes);
        if (share) f.shareLine = shareLeftWords(used, share);
        const budget = Number.isFinite(Number(mine.budget_left_bytes)) ? Number(mine.budget_left_bytes) : NaN;
        f.stop = limitStop(file, num(mine.file_limit_bytes)) || shareStop(file, used, share) ||
          (Number.isFinite(budget) ? budgetStop(file, budget) : null) ||
          f.more.map((m) => limitStop(m, num(mine.file_limit_bytes))).find(Boolean) ||
          togetherStop([file].concat(f.more), used, share, budget);
      }
      renderForm();
    }

    function openPutForm(put) {
      st.form = { mode: 'put', filename: storedName(put.filename), name: put.name || storedName(put.filename), stop: null, shareLine: '', busy: false, error: '', uploadedName: '' };
      renderForm();
    }

    function openWaitingForm(w) {
      st.form = { mode: 'waiting', filename: w.filename, name: w.original_name || w.filename, size: w.size_bytes, stop: null, shareLine: '', busy: false, error: '', uploadedName: w.filename };
      renderForm();
    }

    /** Close the form not yet uploading (an upload in the list goes by its own Cancel or Close). */
    function closeForm() {
      st.form = null;
      renderForm();
      reportBusy();
    }
    /** Cancel (or Close) on the form: it closes, and so does the upload window with nothing left in it. */
    function cancelForm() {
      closeForm();
      if (upWin && !st.uploads.length && !st.pending) upWin.send('close');
    }

    // ── "Also post in chat" (2026-10-17) ──

    /** Is this a form that may also post in chat: a file picked here for Add to Media, not yet going. */
    function postable(f) {
      return !!f && f.mode === 'add' && !f.attach && !f.up && !f.uploadedName;
    }
    /** The channels "Also post in chat" offers now (postChoices with this page's facts). */
    function postChoicesNow() {
      const favs = postFavorites(env.storage);
      let open = '';
      try { open = (env.storage && env.storage.getItem(CHAT_CHANNEL_KEY)) || ''; } catch (e) { open = ''; }
      return postChoices(st.channels, { open, admin: isAdmin(), isFavorite: (id) => favs.has(postFavoriteKey(env.origin, id)) });
    }
    /**
     * The post row, at the foot of "Who can see it" (the desktop's draw_post_row): the tick on the
     * left, then the channel and Add to favorites, both greyed out (disabled) while the tick is off.
     * Drawn again only when what it shows changes, so an open dropdown is never pulled away.
     */
    function renderPostRow(f) {
      const box = f && f.postEl;
      if (!box || !postable(f)) return;
      // Nothing until the server has listed its channels (it does so as the page signs in).
      const choices = st.channelsKnown ? postChoicesNow() : null;
      if (choices && choices.length && !choices.some((c) => c.id === f.postTo)) {
        // A channel asked for that this list does not hold (gone, read-only for this person, or not a
        // channel at all) is never swapped for another one silently: the tick goes off.
        if (f.postTo) f.postOn = false;
        f.postTo = choices[0].id;
      }
      const favKey = postFavoriteKey(env.origin, f.postTo);
      const fav = postFavorites(env.storage).has(favKey);
      const sig = JSON.stringify([choices, !!f.postOn, f.postTo, fav]);
      if (sig === f.postSig) return;
      f.postSig = sig;
      box.replaceChildren();
      f.postEls = null;
      if (!choices) return;
      if (!choices.length) {
        box.appendChild(h('p', { id: 'media-form-post-none', className: 'media-hint', text: POST_WORDS.none }));
        return;
      }
      const on = !!f.postOn;
      const tick = h('input', { type: 'checkbox', id: 'media-form-post-on', checked: on });
      tick.addEventListener('change', () => { f.postOn = !!tick.checked; renderPostRow(f); });
      const select = h('select', { id: 'media-form-post-to', className: 'media-select', disabled: !on, attrs: { 'aria-label': 'Channel' } },
        choices.map((c) => h('option', { value: c.id, text: c.words, selected: c.id === f.postTo })));
      select.addEventListener('change', () => { f.postTo = select.value; renderPostRow(f); });
      const star = h('button', {
        type: 'button', id: 'media-form-post-favorite', className: 'media-btn media-btn-small', disabled: !on,
        title: POST_WORDS.favoriteTip, text: fav ? POST_WORDS.removeFavorite : POST_WORDS.addFavorite,
      });
      star.addEventListener('click', () => {
        if (!f.postOn) return;
        togglePostFavorite(env.storage, postFavoriteKey(env.origin, f.postTo));
        renderPostRow(f);
      });
      f.postEls = { tick, select, star };
      box.appendChild(h('div', { className: 'media-post-row' + (on ? '' : ' media-post-off') }, [
        h('label', { className: 'media-check', title: POST_WORDS.tickTip }, [tick, ' ' + POST_WORDS.tick]),
        select,
        star,
      ]));
    }
    /** The channel this form posts in once it is published, or '' (none, or not one of this server's). */
    function postChannelOf(f) {
      if (!f || !f.postOn || !f.postTo) return '';
      return st.channels.some((c) => c.id === f.postTo) ? f.postTo : '';
    }

    /** The session's channel list (sent as the page signs in, and again when it changes). */
    function takeChannels(list) {
      st.channels = list.filter((c) => c && typeof c.id === 'string' && c.id)
        .map((c) => ({ id: c.id, name: String(c.name || c.id), read_only: c.read_only === true }));
      st.channelsKnown = true;
      if (st.form) renderPostRow(st.form);
    }

    /** The next timestamp a post is signed with: never one this page used before. */
    let lastPostTs = 0;
    function nextPostTimestamp() {
      const t = Math.floor(now());
      lastPostTs = t > lastPostTs ? t : lastPostTs + 1;
      return lastPostTs;
    }

    /**
     * A just-published item's "Also post in chat" (the desktop's post_published): its own address on
     * this page (postText, which the chat shows as its card) posted in `channel` as a chat message, signed with the person's
     * identity over "content\ntimestamp" exactly as the chat signs one (crypto.js
     * pqSignChatMessage), on this page's own signed-in connection; the recovery-phrase guard first,
     * as for anything the chat sends (chat-warnings.js). Only ever one of this server's channels
     * (its channel_list): never a direct message or a group. Resolves to the words for the page's
     * line: "Posted in #general." once the server has it (its echo of the message came back), else
     * "Not posted in #general: " and why; '' when the publish's answer names no item.
     */
    async function postPublished(answer, channel) {
      const item = answer && answer.item;
      if (!item || typeof item !== 'object') return '';
      const known = st.channels.find((c) => c.id === channel);
      const name = '#' + (known ? known.name : channel);
      const not = (why) => notPostedIn(name, why);
      if (!known) return not(POST_WORDS.notAChannel);
      if (!session || !bound()) return not(POST_WORDS.notConnected);
      const content = postText(item, env.origin);
      let held = null;
      try { held = typeof env.phraseIn === 'function' ? await env.phraseIn([content]) : null; } catch (e) { held = null; }
      if (held === true) return not(PHRASE_HELD);
      if (held !== false) return not(POST_WORDS.cannotCheck);
      const timestamp = nextPostTimestamp();
      let sig = null;
      try { sig = typeof env.signBytes === 'function' ? await env.signBytes(new TextEncoder().encode(content + '\n' + timestamp)) : null; } catch (e) { sig = null; }
      if (!sig || !sig.length) return not(POST_WORDS.cannotSign);
      let hex = '';
      for (let i = 0; i < sig.length; i++) hex += sig[i].toString(16).padStart(2, '0');
      let fromName = null;
      try { fromName = ((env.storage && env.storage.getItem('humanity_name')) || '').trim() || null; } catch (e) { fromName = null; }
      const frame = { type: 'chat', from: myKey, from_name: fromName, content, timestamp, channel, pq_signature: hex };
      return new Promise((resolve) => {
        let done = false;
        let refusal = '';
        let grace = null;
        let timer = null;
        const w = {};
        const finish = (words) => {
          if (done) return;
          done = true;
          postWaiters.delete(w);
          if (timer != null) cancelLater(timer);
          if (grace != null) cancelLater(grace);
          resolve(words);
        };
        // The relay echoes a message to its sender's connections once it has stored it; it says
        // why it refused one as a line to its sender (a read-only channel, a mute, too long).
        w.frame = (m) => {
          if (m.type === 'chat' && m.from === myKey && Number(m.timestamp) === timestamp && m.channel === channel) {
            finish(postedIn(name));
          } else if (m.type === 'system' && typeof m.message === 'string' && m.message && !m.message.startsWith('__')) {
            // Lines about something else can come meanwhile: its echo is waited for a moment more.
            refusal = m.message;
            if (grace == null) grace = later(() => finish(not(refusal)), POST_GRACE_MS);
          }
        };
        w.state = () => { if (!bound()) finish(not(POST_WORDS.notConnected)); };
        postWaiters.add(w);
        timer = later(() => finish(not(refusal || POST_WORDS.noAnswer)), ANSWER_WAIT_MS);
        if (!session.send(frame)) finish(not(POST_WORDS.notConnected));
      });
    }

    // ── The upload window (2026-10-17, /shared/upload-window.js) ──

    /**
     * Files the page this window is over handed to it (dropped on a channel of the chat: `postTo`
     * is that channel). They open Add to Media as chosen files do, "Also post in chat" ticked for
     * the channel, once the page knows whether this person may upload here (it signs in first).
     */
    function takeFiles(files, postTo) {
      const list = Array.prototype.slice.call(files || []).filter(Boolean);
      if (!list.length) return;
      st.pending = { files: list, postTo: typeof postTo === 'string' ? postTo : '' };
      // Files dropped on this window itself go the same way, posting in the same channel (the page
      // it is over is still on that channel): media-app.js dropDecision.
      st.windowPostTo = st.pending.postTo;
      if (st.viewer) itemView.close();
      renderForm();
      takePending();
      reportBusy();
    }
    /** Does the page know yet what Add to Media would say for this person? */
    function readyForFiles() {
      if (!myKey || !session) return true;
      const s = sessionState();
      if (!s || s.status === 'refused' || s.status === 'no_identity') return true;
      if (s.status !== 'bound') return false;
      return st.answered && (canUpload() || knownCannotUpload());
    }
    function takePending() {
      const p = st.pending;
      if (!p || !els.form || !readyForFiles()) return;
      st.pending = null;
      openAddForm(p.files[0], p.files.slice(1), p.postTo);
    }
    /** Tell the page this window is over whether something is going in it (it keeps it then). */
    let busyTold = null;
    function reportBusy() {
      // The item window too: an Edit's file uploading keeps it (hidden) until it is up.
      const win = upWin || itemWin;
      if (!win) return;
      const busy = !!st.pending || !!(st.form && st.form.busy) || tails > 0 || st.uploads.some((u) => u.busy || uploadLive(u));
      if (busy === busyTold) return;
      busyTold = busy;
      win.send('busy', { busy });
    }

    /** What the form is drawn from, so it is drawn again when that changes. */
    function formKey() {
      return JSON.stringify([st.categories.map((c) => [c.id, c.name]), st.labels.map((l) => l.id), membersOnlyServer()]);
    }

    /**
     * A chosen file's line in the form: its type's badge (media-edit.js fileBadge, 2026-10-17), then
     * "name (size)" (the name alone when its size is not known).
     */
    function fileLine(name, size, cls) {
      return h('div', { className: cls }, [
        ED.fileBadge(h, doc, name),
        h('span', { className: 'media-form-file-name', text: size != null ? name + ' (' + sizeWords(size) + ')' : name }),
      ]);
    }

    // The list of kinds (/shared/file-risk.js), asked for once, the first time a form shows a file
    // before it is in; each form's badges are drawn again when it comes (a plain grey "File" until then).
    let typesAsked = false;
    function askFileTypes() {
      if (typesAsked || ED.fileTypesKnown()) return;
      typesAsked = true;
      ED.loadFileTypes(env.fetch).then((known) => {
        if (!known) return;
        ED.fillFileBadges(h, doc, els.form);
        ED.fillFileBadges(h, doc, els.uploads);
      });
    }

    let formSeq = 0;
    function renderForm() {
      if (!els.form) return; // not built yet
      els.form.replaceChildren();
      const f = st.form;
      if (!f) {
        // Files handed to the upload window while this page signs in: named, with why they wait.
        if (st.pending) {
          els.form.appendChild(h('div', { id: 'media-form-pending', className: 'media-panel media-form' }, [h('h2', { text: 'Add to Media' })]
            .concat(st.pending.files.map((x) => fileLine(x.name, x.size, 'media-form-file')))
            .concat([h('p', { className: 'media-hint', text: MEDIA_WORDS.signingIn })])));
        }
        return;
      }
      f.key = formKey();
      f.formId = f.formId || ++formSeq;
      const put = f.mode === 'put';
      const kids = [h('h2', { text: put ? 'Put on Media' : f.mode === 'waiting' ? 'Publish an upload' : 'Add to Media' })];
      // Three parts in the chat sidebar's colours, darker (2026-10-17, media-edit.js tintSection; the
      // desktop's Add to Media): its files first (blue: each chosen file with its type's badge and
      // size, what is hidden in it and the switch, the share left), then what it is (red), then
      // who can see it (green). The buttons stay outside them, under them.
      const size = f.file ? f.file.size : f.size;
      const fileParts = [fileLine(f.name, size, 'media-form-file')];
      if (f.more && f.more.length) {
        fileParts.push(h('div', { id: 'media-form-more', className: 'media-form-more' },
          [h('p', { className: 'media-hint', text: MORE_LEAD })].concat(f.more.map((m) => fileLine(m.name, m.size, 'media-form-file media-form-more-file')))));
      }
      // What the file holds and the switch (part 1), before anything else is asked.
      if (!f.stop && detailsShown(f)) fileParts.push(detailsBox(f));
      if (f.shareLine) fileParts.push(h('p', { id: 'media-share-line', className: 'media-share-line', text: f.shareLine }));
      kids.push(ED.tintSection(h, 'blue', ED.WORDS.files, fileParts));
      askFileTypes();
      f.actionsEl = h('div', { id: 'media-form-actions', className: 'media-form-actions' });
      if (f.stop) {
        kids.push(h('p', { id: 'media-form-stop', className: 'media-warn', text: f.stop }));
        kids.push(f.actionsEl);
        f.panel = h('div', { className: 'media-panel media-form' }, kids);
        els.form.appendChild(f.panel);
        renderFormActions(f);
        return;
      }
      const aboutParts = [];
      const whoParts = [];
      const title = h('input', { type: 'text', id: 'media-form-title', className: 'media-input', maxLength: 120, value: f.title || '' });
      const category = h('select', { id: 'media-form-category', className: 'media-select' },
        st.categories.map((c) => h('option', { value: String(c.id), text: c.name, selected: String(f.category || '') === String(c.id) })));
      const description = h('textarea', { id: 'media-form-description', className: 'media-input', maxLength: 2000, rows: 3, value: f.description || '' });
      // What is typed is kept as it is typed, so drawing the form again loses nothing, and an
      // upload publishes with what its form says when its file is up.
      title.addEventListener('input', () => { f.title = title.value; });
      category.addEventListener('change', () => { f.category = category.value; });
      description.addEventListener('input', () => { f.description = description.value; });
      aboutParts.push(h('label', { className: 'media-field' }, ['Title (optional)', title]));
      if (st.categories.length) aboutParts.push(h('label', { className: 'media-field' }, ['Category', category]));
      aboutParts.push(h('label', { className: 'media-field' }, ['Description (optional)', description]));
      f.ticks = [];
      if (st.labels.length) {
        const box = h('fieldset', { className: 'media-field media-labels', id: 'media-form-labels' }, [h('legend', { text: 'Labels (the picture stays hidden until someone clicks it)' })]);
        for (const l of st.labels) {
          const tick = h('input', { type: 'checkbox', value: l.id, dataset: { label: l.id }, checked: !!(f.labels && f.labels.includes(l.id)) });
          tick.addEventListener('change', () => { f.labels = f.ticks.filter((t) => t.checked).map((t) => t.value); });
          f.ticks.push(tick);
          box.appendChild(h('label', { className: 'media-check', title: l.desc || '' }, [tick, ' ' + l.label]));
          if (l.desc) box.appendChild(h('div', { className: 'media-hint', text: l.desc }));
        }
        aboutParts.push(box);
      }
      // Who sees it: a file already public goes on Media as public only (the relay's
      // `was_public`), and a members-only server takes members only. Each form's choice is its
      // own group (its own name), so another form open beside an upload never changes it.
      f.visibilityEls = null;
      if (put) {
        whoParts.push(h('p', { id: 'media-form-public-only', className: 'media-hint', text: MEDIA_WORDS.alreadyPublic }));
      } else if (membersOnlyServer()) {
        whoParts.push(h('p', { id: 'media-form-members-only', className: 'media-hint', text: MEDIA_WORDS.serverMembersOnly }));
      } else {
        const group = 'media-form-visibility-' + f.formId;
        const pub = h('input', { type: 'radio', name: group, value: 'public', id: 'media-form-vis-public', checked: f.visibility !== 'members' });
        const mem = h('input', { type: 'radio', name: group, value: 'members', id: 'media-form-vis-members', checked: f.visibility === 'members' });
        f.visibilityEls = { pub, mem };
        const picked = () => { f.visibility = mem.checked ? 'members' : 'public'; if (f.licencePicker) f.licencePicker.refresh(); };
        pub.addEventListener('change', picked);
        mem.addEventListener('change', picked);
        whoParts.push(h('fieldset', { className: 'media-field', id: 'media-form-visibility' }, [
          h('legend', { text: 'Who sees it' }),
          h('label', { className: 'media-check' }, [pub, ' Public']),
          h('label', { className: 'media-check' }, [mem, ' Members only']),
        ]));
      }
      // The licence, "All rights reserved" preselected, and the hosting paragraph beside it (round 5,
      // as the desktop's Add to Media has them): the server's own words (media-edit.js).
      f.choice = f.choice || { licence: '', project_use: false };
      f.licencePicker = ED.licenceChoice(h, { licences: st.licences, choice: f.choice, id: 'media-form-licence', members: () => membersOnlyServer() || f.visibility === 'members' });
      whoParts.push(f.licencePicker.el, ED.hostingBlock(h, st.licences));
      // "Also post in chat" (2026-10-17), at the foot of this part, for a file picked here only.
      f.postEl = null;
      f.postEls = null;
      f.postSig = null;
      if (postable(f)) {
        f.postEl = h('div', { id: 'media-form-post', className: 'media-field media-post' });
        whoParts.push(f.postEl);
      }
      kids.push(ED.tintSection(h, 'red', ED.WORDS.aboutIt, aboutParts), ED.tintSection(h, 'green', ED.WORDS.whoCanSeeIt, whoParts));
      f.inputs = { title, category, description };
      kids.push(f.actionsEl);
      f.panel = h('div', { className: 'media-panel media-form' }, kids);
      els.form.appendChild(f.panel);
      renderPostRow(f);
      renderFormActions(f);
    }

    /** Give every id inside a moved form a suffix of its own, so the next form's ids stay unique. */
    function suffixIds(el, suffix) {
      for (const c of Array.from(el.children || [])) {
        if (c.id) c.id = c.id + suffix;
        suffixIds(c, suffix);
      }
    }

    /** The upload's words for where it is. */
    function uploadWords(u) {
      const s = u.state;
      const sent = sizeWords(s.offset) + ' of ' + sizeWords(s.size) + ' sent';
      const seconds = (n) => (n || 1) + (n === 1 ? ' second.' : ' seconds.');
      switch (s.phase) {
        case 'idle':
        case 'starting': return MEDIA_WORDS.uploadStarting;
        case 'sending': return (s.resumed ? MEDIA_WORDS.uploadResumed + ' ' : '') + sent + '.';
        case 'finishing': return MEDIA_WORDS.uploadFinishing;
        case 'pausing': return sent + '. ' + MEDIA_WORDS.uploadPausing;
        case 'paused': return sent + '. ' + MEDIA_WORDS.uploadPaused;
        case 'reconnecting': return sent + '. The connection dropped. Trying again in ' + seconds(s.retryIn);
        case 'server_wait': return sent + '. ' + (s.error ? s.error + ' ' : '') + 'Trying again in ' + seconds(s.retryIn);
        case 'waiting_connection': return sent + '. ' + MEDIA_WORDS.uploadWaitingConnection;
        case 'allowance': return [s.error, resetsWords(s.resetsAt), MEDIA_WORDS.uploadAllowanceThen].filter(Boolean).join(' ');
        case 'server_error': return s.error + MEDIA_WORDS.uploadServerErrorTail;
        case 'no_room': return s.error;
        case 'failed': return s.error;
        case 'cancelling': return MEDIA_WORDS.uploadCancelling;
        case 'cancelled': return MEDIA_WORDS.uploadCancelling;
        case 'done': return MEDIA_WORDS.uploadPublishing;
        default: return sent + '.';
      }
    }

    /**
     * The form's progress, words and buttons: drawn again on every change without touching the
     * fields. A form in the uploads list names its controls by `data-upload-*` (there may be
     * several); the form not yet uploading keeps its ids.
     */
    function renderFormActions(f) {
      reportBusy();
      const box = f.actionsEl;
      if (!box) return;
      box.replaceChildren();
      const listed = !!f.n;
      const button = (action, text, onClick, cls) => h('button', {
        type: 'button', id: listed ? undefined : 'media-form-' + action, dataset: listed ? { uploadAction: action } : undefined,
        className: 'media-btn' + (cls ? ' ' + cls : ''), text, on: { click: onClick },
      });
      if (f.stop) {
        box.appendChild(h('div', { className: 'media-form-buttons' }, [button('cancel', 'Close', cancelForm)]));
        return;
      }
      if (f.error) box.appendChild(h('p', { id: listed ? undefined : 'media-form-error', dataset: listed ? { uploadError: f.n } : undefined, className: 'media-warn', text: f.error }));
      const u = f.up;
      if (u) {
        const s = u.state;
        const pct = s.size > 0 ? Math.min(100, Math.round((s.offset / s.size) * 1000) / 10) : 0;
        const fill = h('div', { className: 'media-meter-fill' });
        fill.style.width = pct + '%';
        box.appendChild(h('div', { className: 'media-meter', dataset: { uploadBar: f.n }, attrs: { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': pct, 'aria-label': 'Upload of ' + f.name } }, [fill]));
        const warn = s.phase === 'allowance' || s.phase === 'no_room' || s.phase === 'failed' || s.phase === 'server_error';
        box.appendChild(h('p', { dataset: { uploadWords: f.n }, className: warn ? 'media-warn' : 'media-hint', text: uploadWords(u) }));
        const buttons = [];
        const p = s.phase;
        if (p === 'starting' || p === 'sending' || p === 'finishing' || p === 'reconnecting' || p === 'server_wait' || p === 'waiting_connection' || p === 'allowance') {
          // Pause stops it in every phase: the request on its way and every wait.
          buttons.push(button('pause', 'Pause', () => u.pause()));
        } else if (p === 'paused') {
          buttons.push(button('resume', 'Resume', () => u.resume(), 'media-btn-primary'));
        } else if (p === 'no_room' || p === 'server_error') {
          buttons.push(button('retry', 'Retry', () => u.resume(), 'media-btn-primary'));
        }
        if (p === 'failed') {
          // Close ends it on the server too: nothing is left holding part of the person's share.
          buttons.push(button('close', 'Close', () => { u.cancel(); dropUpload(f); }));
        } else if (p !== 'done' && p !== 'cancelling' && p !== 'cancelled') {
          buttons.push(button('cancel', 'Cancel', () => u.cancel()));
        }
        box.appendChild(h('div', { className: 'media-form-buttons' }, buttons));
        return;
      }
      const put = f.mode === 'put';
      const goOff = f.busy || !!f.checking || !!(f.mode === 'add' && !f.uploadedName && sendingOf(f).refused);
      // The Upload button wears the nav bar's current-page border, its RGB cycle (the operator,
      // 2026-10-17: "Upload button with the RGB border effect"; /shared/media.css .media-btn-rgb), when
      // it would upload a file picked here: the desktop's add.rs rgb_border, ready and picked.
      const rgb = !goOff && f.mode === 'add' && !f.attach && !f.uploadedName;
      const go = h('button', {
        type: 'button', id: listed ? undefined : 'media-form-go', dataset: listed ? { uploadAction: 'go' } : undefined,
        className: 'media-btn media-btn-primary' + (rgb ? ' media-btn-rgb' : ''), disabled: goOff,
        text: f.checking ? 'Working out your share...' : f.busy ? 'Working...' : f.attach ? 'Add it to the item' : put || f.uploadedName ? 'Put on Media' : 'Upload and add to Media',
      });
      go.addEventListener('click', () => {
        // Keep what was typed for a second try.
        if (f.inputs) {
          f.title = f.inputs.title.value;
          f.category = f.inputs.category.value;
          f.description = f.inputs.description.value;
        }
        if (f.ticks) f.labels = f.ticks.filter((t) => t.checked).map((t) => t.value);
        f.visibility = put ? 'public' : membersOnlyServer() ? 'members' : f.visibilityEls && f.visibilityEls.mem.checked ? 'members' : 'public';
        submitForm(f);
      });
      // A form in the list whose publish failed: Close leaves its upload waiting (listed below with
      // Publish and Remove); the form not yet uploading just closes.
      const close = listed ? button('close', 'Close', () => dropUpload(f)) : button('cancel', 'Cancel', cancelForm);
      box.appendChild(h('div', { className: 'media-form-buttons' }, [go, close]));
    }

    function submitForm(f) {
      if (!f || f.busy || f.stop || f.checking || f.up) return;
      f.error = '';
      if (f.mode === 'add' && !f.uploadedName) { startUpload(f); return; }
      if (f.attach) { attachUp(f); return; }
      publish(f);
    }

    /**
     * A file picked in an item's Edit form ("Add a file", or "Cover picture" as `as` "cover"):
     * checked and cleaned like Add to Media's, then the same resumable upload, listed under Uploads
     * in progress, and added to the item when it is up instead of published (`attachUp`). Resolves
     * to '' once it started, else the words saying why not (shown in the Edit form).
     */
    async function startAttach(file, item, as) {
      const filename = itemFileName(item);
      if (!filename) return MEDIA_WORDS.noFileName;
      if (!canUpload()) return bound() ? MEDIA_WORDS.cannotUpload : MEDIA_WORDS.notSignedInUpload;
      if (st.uploads.some((u) => sameFile(u.file, file) && uploadLive(u))) return '"' + file.name + '"' + MEDIA_WORDS.alreadyUploading;
      if (cleaner === false) return MEDIA_WORDS.checkerMissing;
      const title = itemName(item, st.kinds);
      const f = { mode: 'add', file, name: file.name, attach: { id: item.id, filename, as: as === 'cover' ? 'cover' : 'file', title }, busy: false, error: '', uploadedName: '', up: null, fp: '' };
      f.heading = file.name + ' (' + sizeWords(file.size) + '), to add to ' + title;
      if (cleaner) {
        f.protectedOn = cleaner.protectedSetupOn(env.storage);
        f.remove = cleaner.removeDetailsChosen(env.storage);
        f.clean = await cleaner.cleanFile(file);
      }
      const sending = sendingOf(f);
      if (!sending.file) return sending.refused || '';
      f.fpFile = sending.file;
      try { f.fp = fingerprint ? await fingerprint(f.fpFile, env.digest) : ''; } catch (e) { f.fp = ''; }
      // The person's share and the role's largest Media file, as Add to Media checks them.
      const mine = await loadMine();
      if (mine) {
        const stop = limitStop(file, num(mine.file_limit_bytes)) || shareStop(file, num(mine.used_bytes) + num(mine.in_progress_bytes), num(mine.share_bytes));
        if (stop) return stop;
      }
      f.actionsEl = h('div', { className: 'media-form-actions' });
      f.panel = h('div', { className: 'media-panel media-form' }, [h('h2', { text: f.heading }), f.actionsEl]);
      startUpload(f);
      return '';
    }

    /** An item just published with more files chosen beside its own: each starts its own upload to
     *  the new item, as Edit's Add a file does, and the line says so (uploading.rs attach_more). */
    async function attachMore(answer, more) {
      const item = answer && answer.item;
      // Said after what the line says already (published, and posted in chat), as the desktop's does.
      const before = st.notice || MEDIA_WORDS.published;
      if (!item || !itemFileName(item)) {
        say(before + ' Its other files were not added: the server\'s answer did not say which item it made. Add them from the item\'s Edit window.');
        return;
      }
      let started = 0;
      const problems = [];
      for (const file of more) {
        const stop = await startAttach(file, item, 'file');
        if (stop) problems.push(file.name + ': ' + stop); else started++;
      }
      let line = before;
      if (started) line += ' ' + (started === 1 ? 'Its other file is' : 'Its other ' + started + ' files are') + ' uploading under Uploads in progress, and added to it when up.';
      if (problems.length) line += ' ' + problems.join(' ');
      say(line);
    }

    /** An attaching upload is up: add it to its item (media-upload.js attachUpload), never publish it. */
    async function attachUp(f) {
      if (f.busy) return;
      f.busy = true;
      renderFormActions(f);
      try {
        const a = await U.attachUpload({ fetch: env.fetch, sign: env.sign, to: f.attach, add: f.uploadedName, cannotSign: MEDIA_WORDS.cannotSign });
        if (st.uploads.includes(f)) dropUpload(f);
        if (a && a.item) itemView.takeItem(a.item);
        say('Added to "' + f.attach.title + '".');
        itemView.uploadsChanged();
        loadMine();
        return;
      } catch (e) {
        f.error = (e && e.message ? e.message : String(e)) + MEDIA_WORDS.uploadWaits;
        f.up = null;
        loadMine();
      }
      f.busy = false;
      if (st.uploads.includes(f)) renderFormActions(f);
      itemView.uploadsChanged();
    }

    /** Take a form out of the uploads list (its upload ended, or it was published or closed). */
    function dropUpload(f) {
      st.uploads = st.uploads.filter((u) => u !== f);
      if (f.panel) f.panel.remove();
      renderWaiting();
      reportBusy();
    }

    /** Send the file in pieces (src/relay/resumable.rs), then publish it with its own form. */
    function startUpload(f) {
      if (!token()) {
        f.error = MEDIA_WORDS.notSignedInUpload;
        renderFormActions(f);
        return;
      }
      // The cleaned file, or the file as picked when its details are kept by choice (keep_metadata).
      const sending = sendingOf(f);
      if (!sending.file) {
        f.error = sending.refused || '';
        renderFormActions(f);
        return;
      }
      // The switch as it was sent is the last choice on this device.
      if (cleaner && f.clean && !f.protectedOn) cleaner.rememberRemoveDetails(env.storage, f.remove !== false);
      if (f.detailsSwitch) f.detailsSwitch.disabled = true;
      // "Also post in chat" as it was sent: the channel it posts in once published ('' for none),
      // and its controls no longer change anything.
      f.postIn = postChannelOf(f);
      if (f.postEls) for (const c of [f.postEls.tick, f.postEls.select, f.postEls.star]) c.disabled = true;
      // The form moves into the uploads list; the form area is free for another at once.
      f.n = ++uploadSeq;
      if (st.form === f) st.form = null;
      st.uploads.push(f);
      if (f.panel) {
        suffixIds(f.panel, '-' + f.n);
        f.panel.dataset.upload = String(f.n);
        els.uploads.appendChild(f.panel);
      }
      renderForm();
      f.up = createUploader({
        fetch: env.fetch, token, file: sending.file, owner: myKey, storage: env.storage,
        fp: sending.file === f.fpFile ? f.fp : '', keepMetadata: sending.keepMetadata,
        sleep: env.sleep, now, digest: env.digest, AbortController: Abort,
        onChange: (s) => {
          if (s.phase === 'cancelled') {
            // Deleted on the server now (Cancel, or Close on a failed one), a file its finish had
            // stored with it (round 4's cancel answer): the page says what the server answered,
            // and the share is asked again.
            if (st.uploads.includes(f)) dropUpload(f);
            sayCancelled(s.cancelAnswer, s.error);
            loadMine();
            return;
          }
          if (st.uploads.includes(f)) renderFormActions(f);
          if (f.attach) itemView.uploadsChanged();
        },
        onDone: (answer) => {
          f.uploadedName = storedName((answer && (answer.filename || answer.url)) || '');
          loadMine();
          // Round 5: a file for an item's Edit form is added to that item, never published.
          if (f.attach) attachUp(f); else publish(f);
        },
      });
      renderFormActions(f);
      f.up.start();
    }

    async function publish(f) {
      if (f.busy) return;
      f.busy = true;
      renderFormActions(f);
      try {
        const filename = f.mode === 'put' ? f.filename : f.uploadedName;
        // The licence and the game tick (round 5): what the form's picker holds.
        const licence = (f.choice && f.choice.licence) || DEFAULT_LICENCE;
        const forGame = !!(f.choice && f.choice.project_use);
        const auth = await env.sign(purposes.publish(filename, licence, forGame));
        if (!auth) throw new Error(MEDIA_WORDS.cannotSign);
        const body = Object.assign({
          filename,
          title: (f.title || '').trim(),
          description: (f.description || '').trim(),
          labels: f.labels || [],
          visibility: f.visibility || 'public',
          licence,
          project_use: forGame,
        }, f.category ? { category_id: Number(f.category) } : {}, auth);
        const answer = await answerOf(await env.fetch('/api/media', jsonPost(body)));
        // What follows a publish (its post in chat, its other files) counts as going until it has
        // started, so an upload window closed meanwhile is kept for it (reportBusy).
        tails++;
        try {
          if (st.form === f) closeForm();
          if (st.uploads.includes(f)) dropUpload(f);
          say(MEDIA_WORDS.published);
          // "Also post in chat" (2026-10-17): its own address on this page (its card), in the channel.
          if (f.postIn) {
            const words = await postPublished(answer, f.postIn);
            if (words) say((st.notice ? st.notice + ' ' : '') + words);
          }
          if (f.more && f.more.length) await attachMore(answer, f.more);
        } finally {
          tails--;
          reportBusy();
        }
        load(true);
        loadMine();
        return;
      } catch (e) {
        f.error = (e && e.message ? e.message : String(e)) + (f.uploadedName && f.mode !== 'put' ? MEDIA_WORDS.uploadWaits : '');
        f.up = null;
        loadMine();
      }
      f.busy = false;
      if (st.form === f || st.uploads.includes(f)) renderFormActions(f);
    }

    /** A frame or a change from the session: "Also post in chat" waiting for its answer hears it first. */
    function heard(ev) {
      for (const w of Array.from(postWaiters)) {
        if (ev.type === 'state') w.state();
        else if (ev.msg && typeof ev.msg === 'object') w.frame(ev.msg);
      }
    }

    return {
      openAddForm, openPutForm, openWaitingForm, closeForm, formKey, renderForm, renderPostRow, takePending, reportBusy,
      takeChannels, takeFiles, uploadWords, startAttach, heard,
    };
  }

  const api = {
    POST_WORDS, STAR, POST_FAVORITES_KEY, CHAT_CHANNEL_KEY, POST_GRACE_MS, postFavoriteKey, postFavorites, togglePostFavorite,
    postChoices, postText, postedIn, notPostedIn, createAddForm,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaAdd = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
