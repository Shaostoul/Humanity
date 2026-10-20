// ── files-space.js ────────────────────────────────────────────────────────
// The Files page's words for sizes and space (2026-10-20, out of files-app.js, which keeps
// using them under the same names): a size as people read it, what is left of a person's
// share of the server's file space, why a file was not uploaded (over the role's largest file,
// the person's share or the whole space, alone or with the others chosen with it), the admins'
// meter, and when an upload allowance resets. The desktop app's words are the same (its
// src/gui/pages/files/api.rs); files-add.js is handed these by files-app.js.
//
// A classic script in the browser (FilesSpace on window), loaded before files-app.js; a
// CommonJS module under Node, where files-app.js requires it and the files-web tests run it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MIB = 1024 * 1024;
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

  /** The stop when a file is over the role's largest Media file on this server (0: none), else null. */
  function limitStop(file, limitBytes) {
    if (!file || !Number.isFinite(limitBytes) || limitBytes <= 0 || file.size <= limitBytes) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', over your role\'s ' + sizeWords(limitBytes) +
      ' limit for one file on this server\'s Files page, so it was not uploaded. An admin can raise it in Server Settings > Roles.';
  }

  /** Several files going up together (2026-10-16): the stop when all of them together are more
   *  than what is left of the person's share or of the server's Media space, else null. Each file
   *  on its own is checked as one is. The desktop app's words (api.rs room_problem_together). */
  function togetherStop(files, usedBytes, shareBytes, budgetLeft) {
    if (!files || files.length < 2) return null;
    const total = files.reduce((n, f) => n + ((f && f.size) || 0), 0);
    const n = files.length;
    if (Number.isFinite(shareBytes) && shareBytes > 0 && Number.isFinite(usedBytes)) {
      const left = Math.max(0, shareBytes - usedBytes);
      if (total > left) {
        return 'These ' + n + ' files come to ' + sizeWords(total) + ', more than the ' + sizeWords(left) +
          ' left of your share of this server\'s file space, so none was uploaded. Choose fewer, remove one of your items, or ask an admin to raise each person\'s share.';
      }
    }
    if (Number.isFinite(budgetLeft) && budgetLeft >= 0 && total > budgetLeft) {
      return 'These ' + n + ' files come to ' + sizeWords(total) + ', more than the ' + sizeWords(budgetLeft) +
        ' left in this server\'s whole file space, so none was uploaded. Choose fewer, or its admins can make more room.';
    }
    return null;
  }

  /** What the Add to Files form says above the further files chosen with the first (each on its own row, with its type's badge). */
  const MORE_LEAD = 'Also on this item, each added once the item is up:';
  /** The same in one line. */
  function moreWords(more) {
    return MORE_LEAD + ' ' + more.map((m) => m.name + ' (' + sizeWords(m.size) + ')').join(', ');
  }

  /** The stop when a file is bigger than what is left of the whole Media space, else null. */
  function budgetStop(file, leftBytes) {
    if (!file || !Number.isFinite(leftBytes) || leftBytes < 0 || file.size <= leftBytes) return null;
    return '"' + file.name + '" is ' + sizeWords(file.size) + ', more than the ' + sizeWords(leftBytes) +
      ' left in this server\'s file space, so it was not uploaded. An admin can make room, or make the file space larger.';
  }

  /** The admins' meter: "12.4 GB of 50 GB used". */
  function meterWords(usedBytes, budgetBytes) {
    return sizeWords(usedBytes) + ' of ' + sizeWords(budgetBytes) + ' used';
  }

  /** The meter's own line for what the server keeps (inside "used", in nobody's share). */
  function keptMeterWords(keptBytes) {
    return 'Kept by the server: ' + sizeWords(keptBytes) + ' of that, counted in the budget and in nobody\'s share';
  }

  /** When an upload allowance resets, in this device's own time, or ''. */
  function resetsWords(ms) {
    const n = Number(ms);
    if (!Number.isFinite(n) || n <= 0) return '';
    let when = '';
    try { when = new Date(n).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' }); } catch (e) { when = ''; }
    if (!when) { try { when = new Date(n).toISOString().slice(0, 16).replace('T', ' ') + ' UTC'; } catch (e) { when = ''; } }
    return when ? 'That is ' + when + ' here.' : '';
  }

  const api = { sizeWords, shareLeftWords, shareStop, limitStop, togetherStop, MORE_LEAD, moreWords, budgetStop, meterWords, keptMeterWords, resetsWords };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.FilesSpace = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
