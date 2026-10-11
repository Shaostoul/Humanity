// Profile changes made in this browser and not yet on the server (2026-10-11, BUG-195 and its
// review). A classic script in the browser (window.HosProfileChanges) and a CommonJS module under
// Node (the tests).
//
// The server saves a profile update WHOLE: a field left out is saved empty. And the person may
// edit their profile on several devices (the desktop app, another browser). So this browser never
// pushes its stored profile as it stands. It remembers WHICH fields were changed here
// ("bio", "socials", "privacy:directory", ...), sends those on top of the server's own copy of the
// profile (heard on connect), and forgets a change only once the server's copy carries it. Until
// 2026-10-11 every connect pushed the stored profile, undoing changes made elsewhere; the first fix
// counted a change as sent the moment it left, so a save the server turned down (an http:// link,
// a handle it refuses) was lost when the server's older copy came back.
//
// Used by: web/pages/profile.html (a save records what it changed), web/chat/chat-privacy.js (a
// privacy level records the directory choice), web/chat/chat-profile.js (the merge, the push and
// the confirmation).

(function (root) {
  'use strict';

  /** Where the list of changed fields is kept (localStorage). */
  const KEY = 'humanity_profile_changes';
  /** The text fields of the network profile, as the stored profile names them. */
  const TEXT_FIELDS = ['bio', 'avatar_url', 'banner_url', 'pronouns', 'location', 'website'];

  const str = (v) => (v == null ? '' : String(v)).trim();
  const obj = (v) => (v && typeof v === 'object' && !Array.isArray(v) ? v : {});
  /** Two link sets are the same when they hold the same keys and values, in any order. */
  function sameSocials(a, b) {
    const x = obj(a);
    const y = obj(b);
    const keys = new Set(Object.keys(x).concat(Object.keys(y)));
    for (const k of keys) if (str(x[k]) !== str(y[k])) return false;
    return true;
  }

  /** The fields that differ between two stored profiles: text fields by name, the links as
   *  "socials", and each privacy key as "privacy:<key>". */
  function changedFields(before, after) {
    const b = obj(before);
    const a = obj(after);
    const out = [];
    for (const k of TEXT_FIELDS) if (str(b[k]) !== str(a[k])) out.push(k);
    if (!sameSocials(b.socials, a.socials)) out.push('socials');
    const bp = obj(b.privacy);
    const ap = obj(a.privacy);
    for (const k of new Set(Object.keys(bp).concat(Object.keys(ap)))) {
      if (str(bp[k]) !== str(ap[k])) out.push('privacy:' + k);
    }
    return out;
  }

  function read(storage) {
    try {
      const v = JSON.parse((storage && storage.getItem(KEY)) || '[]');
      return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [];
    } catch (e) { return []; }
  }

  function write(storage, fields) {
    try {
      const list = Array.from(new Set(fields));
      if (list.length) storage.setItem(KEY, JSON.stringify(list));
      else storage.removeItem(KEY);
    } catch (e) { /* this browser keeps nothing */ }
  }

  /** Record that `fields` were changed here (added to those still waiting). */
  function add(storage, fields) {
    write(storage, read(storage).concat(fields || []));
  }

  /** A bio as the server keeps it: everything from a "<" to the next ">" taken out, and every ">"
   *  (src/relay/handlers/msg_handlers.rs, the profile save's clean-up). Without this a bio holding
   *  "<3" or ">" never matched the server's copy and stayed waiting for good, going back over
   *  bios saved on other devices (the web review of 2026-10-11). */
  function tidyBio(text) {
    let out = '';
    let inTag = false;
    for (const ch of String(text == null ? '' : text)) {
      if (ch === '<') inTag = true;
      else if (ch === '>') inTag = false;
      else if (!inTag) out += ch;
    }
    return out;
  }

  /** The value of one field id in a stored profile, as the server would keep it. */
  function valueOf(p, field) {
    if (field.startsWith('privacy:')) return str(obj(obj(p).privacy)[field.slice(8)]);
    if (field === 'socials') return obj(obj(p).socials);
    if (field === 'bio') return str(tidyBio(obj(p).bio));
    return str(obj(p)[field]);
  }

  function sameValue(field, x, y) {
    return field === 'socials' ? sameSocials(x, y) : x === y;
  }

  /** The stored profile to keep: the server's copy (`server`, already in the stored shape), with
   *  each field changed here taken from `local`, and every other key of `local` (a draft note,
   *  anything that is not part of the network profile) kept. */
  function merge(server, local, fields) {
    const l = obj(local);
    const s = obj(server);
    const out = Object.assign({}, l, s, { socials: Object.assign({}, obj(s.socials)), privacy: Object.assign({}, obj(s.privacy)) });
    for (const f of fields || []) {
      if (f.startsWith('privacy:')) {
        const k = f.slice(8);
        const v = obj(l.privacy)[k];
        if (v === undefined || v === '') delete out.privacy[k];
        else out.privacy[k] = v;
      } else if (f === 'socials') {
        out.socials = Object.assign({}, obj(l.socials));
      } else {
        out[f] = l[f] == null ? '' : l[f];
      }
    }
    return out;
  }

  /** The changes still waiting once the server's copy has come: those it does not carry yet. */
  function stillWaiting(server, local, fields) {
    return (fields || []).filter((f) => !sameValue(f, valueOf(server, f), valueOf(local, f)));
  }

  const api = { KEY, TEXT_FIELDS, changedFields, read, write, add, merge, stillWaiting, tidyBio };
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.HosProfileChanges = api;
})(typeof window !== 'undefined' ? window : null);
