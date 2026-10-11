// ── upload-clean.js ───────────────────────────────────────────────────────
// Cleaning a file on the sender's own device before it leaves (2026-10-11,
// docs/design/media-showcase.md, "Files on the sender's device and in
// messages, exact", parts 1 and 2; the operator's answers 4 and 5).
//
// Every upload path on the website runs a picked, pasted or dropped file
// through cleanFile() before anything is uploaded or encrypted: the chat's
// attach button, paste and drop, in public rooms and in private
// conversations (chat-messages.js), and Add to Media (pages/media-app.js).
// The cleaning itself is /shared/strip-metadata.js (HosStrip, the port of the
// relay's own Rust strips, held to them byte for byte by its test); this file
// is what the pages need around it:
//
//   - reading the file: pictures whole; a video in the MP4 family whole up to
//     64 MB, and above that by its box headers only (the movie index and the
//     few boxes that can hold a location are read, the frames never are), so a
//     video of several GB is cleaned without holding it in memory. The bytes
//     that come out are the same either way (scripts/tests/upload-clean-web.test.js);
//   - the one switch, "Remove location and camera details", on by default and
//     remembered on this device as the last choice (localStorage, in
//     try/catch); with the protected setup on it is always on and not shown;
//   - what happens to a file that could not be checked (the strip stopped
//     early, a picture it could not walk, a file whose bytes are not what its
//     name says, or a HEIC or AVIF photo, which this port cannot clean yet):
//     on a public shelf it is not sent, with words; in a DM or a group the
//     person is asked "Send it anyway?", and on yes the file's marker says
//     `"unchecked": true` so the recipient sees one line; with the protected
//     setup on it is not sent at all;
//   - the words.
//
// When the person keeps the details (the switch off), nothing is cleaned and
// the upload says `keep_metadata=1`, so the relay skips its own strip too
// (B4). When they are removed, the relay strips a public upload again with
// the same rules, which changes nothing.
//
// A classic script in the browser (HosUploadClean lands on window; load it
// after /shared/strip-metadata.js), a CommonJS module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const S = (typeof module === 'object' && module && module.exports && typeof require === 'function')
    ? require('./strip-metadata.js')
    : root.HosStrip;

  const MIB = 1024 * 1024;
  // The switch's last choice on this device: '0' keeps the details, anything else removes them.
  const REMOVE_DETAILS_KEY = 'hos_remove_file_details';
  // Where the protected setup is kept (/shared/protected.js PROTECTED_STORAGE_KEY; a test holds
  // the two names together). Read here the way protected.js reads it, because the Media page does
  // not load protected.js.
  const PROTECTED_STORAGE_KEY = 'humanity_protected_setup';
  // A video in the MP4 family up to this is read whole; above it, by its box headers.
  const WHOLE_MAX = 64 * MIB;
  // The most of a picture read to clean it: the relay's own picture cap.
  const PICTURE_MAX = 64 * MIB;
  // A movie index, a top-level `meta` or a `uuid` box is read whole for the walk up to this.
  const BOX_READ_MAX = 256 * MIB;
  // The first bytes read to tell what a file is.
  const HEAD_BYTES = 4096;
  // The top-level boxes copied whole for the walk: everything the relay's walk reads at the top
  // level (`ftyp` for the brands, `moov`, `meta`, `uuid`). Every other box is stood in for by an
  // empty box of the same name, which the walk steps over exactly as it steps over the real one.
  const COPIED = new Set(['ftyp', 'moov', 'meta', 'uuid']);
  // The relay's limit on box headers read (strip-metadata.js MAX_BOXES).
  const MAX_TOP_BOXES = (S && S.MAX_BOXES) || 1000000;

  const SAY = (S && S.SAY) || {};

  const WORDS = Object.freeze({
    switchLabel: 'Remove location and camera details',
    switchOffLine: 'Everything hidden in it will be sent, where it was taken included.',
    protectedLine: 'The protected setup removes location and camera details from every file sent from this device.',
    // Answer 5's question, in a DM or a group.
    askPrivate: 'This file could not be read all the way through, so where it was taken may still be in it. Send it anyway?',
    askPrivateHeif: 'This app cannot clean HEIC and AVIF photos of where they were taken yet, so where it was taken may still be in it. Send it anyway?',
    // A public shelf (a room, Media) never takes one.
    publicRefused: 'A file shared in public must be checked first, so it was not sent.',
    publicKeepHint: 'To share it with everything in it, turn off "Remove location and camera details".',
    protectedRefused: 'With the protected setup on, only files this app could check are sent, so it was not sent.',
    // The line under a file whose marker says `"unchecked": true`, for whoever opens it.
    recipientLine: "The sender's app could not check this file for hidden location.",
    tooLarge: 'This picture is too large to check on this device, so where it was taken may still be in it.',
    boxTooLarge: 'Part of this video that can hold where it was filmed is too large to check on this device, so where it was filmed may still be in it.',
    unreadable: 'This file could not be read on this device, so it could not be checked.',
    checkerMissing: 'The file was not sent: this page could not load its file checker. Reload the page and try again.',
    send: 'Send',
    sendAnyway: 'Send anyway',
    cancel: 'Cancel',
  });

  // ── The switch and the protected setup ─────────────────────────────────

  /** Does this device remove the details (the switch's last choice, on by default)? */
  function removeDetailsChosen(storage) {
    try {
      return !(storage && storage.getItem(REMOVE_DETAILS_KEY) === '0');
    } catch (e) {
      return true;
    }
  }

  /** Remember the switch as the last choice on this device. */
  function rememberRemoveDetails(storage, on) {
    try {
      if (storage) storage.setItem(REMOVE_DETAILS_KEY, on ? '1' : '0');
      return true;
    } catch (e) {
      return false;
    }
  }

  /**
   * Is the protected setup on, on this device? The rule of /shared/protected.js
   * protectedStateRead: nothing kept, or a setup that says it is off, is off;
   * anything else kept counts as on (fail closed); storage that cannot be read
   * counts as nothing kept.
   */
  function protectedSetupOn(storage) {
    let raw = null;
    try { raw = storage ? storage.getItem(PROTECTED_STORAGE_KEY) : null; } catch (e) { return false; }
    if (raw == null || raw === '') return false;
    let v = null;
    try { v = JSON.parse(raw); } catch (e) { v = null; }
    return !(v && typeof v === 'object' && v.on === false);
  }

  // ── Reading the file ───────────────────────────────────────────────────

  /** Bytes a..b of a File or Blob. */
  async function readRange(file, a, b) {
    const part = file.slice(a, b);
    return new Uint8Array(await part.arrayBuffer());
  }

  function be32(b, i) { return b[i] * 0x1000000 + ((b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]); }
  function name4(b, i) { return String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3]); }
  function asciiBytes(s) { return Uint8Array.from(s, (c) => c.charCodeAt(0) & 0xff); }

  /**
   * A big video read by its top-level box headers, in the relay walk's own terms
   * (strip-metadata.js boxHeader): the stand-in it builds is a file the walk
   * reads exactly as it reads the whole one. Each top-level box the walk looks
   * into (`ftyp`, `moov`, `meta`, `uuid`) is copied whole; every other box (the
   * frames in `mdat`, `free`, `wide`) becomes an empty box of the same name. A
   * header whose size makes no sense ends the walk at the top level in the
   * whole file, so its first bytes (at most 16) end the stand-in too, where they
   * make no sense either; the bytes after them are never walked, as in the whole
   * file. Returns {bytes, segments, tail} or {tooLarge}.
   *   segments: [{orig, syn, len}] each box copied whole (orig and syn its start
   *             in the file and in the stand-in);
   *   tail:     where the walk stopped in the file (its end when it read through).
   */
  async function standIn(file) {
    const size = file.size;
    const parts = [];
    const segments = [];
    let synLen = 0;
    const push = (bytes) => { parts.push(bytes); synLen += bytes.length; };
    let pos = 0;
    let boxes = 0;
    while (pos < size) {
      if (boxes >= MAX_TOP_BOXES) break;
      boxes++;
      const room = size - pos;
      if (room < 8) {
        // Too little room for a box: the walk stops here without a word, so the stand-in carries
        // the same few bytes.
        push(await readRange(file, pos, size));
        pos = size;
        break;
      }
      const head = await readRange(file, pos, pos + Math.min(room, 16));
      const size32 = be32(head, 0);
      let boxSize;
      let bad = false;
      if (size32 === 0) {
        boxSize = room;
      } else if (size32 === 1) {
        if (room < 16) bad = true;
        else {
          const hi = be32(head, 8);
          const lo = be32(head, 12);
          const large = hi * 4294967296 + lo;
          if ((hi === 0 && lo < 16) || large > room) bad = true;
          boxSize = large;
        }
      } else if (size32 < 8) {
        bad = true;
      } else {
        boxSize = size32;
      }
      if (!bad && boxSize > room) bad = true;
      if (bad) {
        // The walk ends at this header in the whole file; these bytes make it end here too.
        push(head);
        break;
      }
      const nm = name4(head, 4);
      if (COPIED.has(nm)) {
        if (boxSize > BOX_READ_MAX) return { tooLarge: true };
        const whole = await readRange(file, pos, pos + boxSize);
        segments.push({ orig: pos, syn: synLen, len: boxSize });
        push(whole);
      } else {
        // An empty box of the same name: stepped over by the walk, as the real one is.
        const stand = new Uint8Array(8);
        stand[3] = 8;
        stand.set(asciiBytes(nm), 4);
        push(stand);
      }
      pos += boxSize;
    }
    const bytes = new Uint8Array(synLen);
    let at = 0;
    for (const p of parts) { bytes.set(p, at); at += p.length; }
    return { bytes, segments, tail: pos };
  }

  /**
   * The cleaned file from a stand-in's result: each box copied whole comes from the cleaned
   * stand-in (a video's strip never changes a length), everything else from the file itself.
   */
  function fromStandIn(file, sin, cleanedStandIn) {
    const parts = [];
    let pos = 0;
    for (const s of sin.segments) {
      if (s.orig > pos) parts.push(file.slice(pos, s.orig));
      parts.push(cleanedStandIn.subarray(s.syn, s.syn + s.len));
      pos = s.orig + s.len;
    }
    if (pos < file.size) parts.push(file.slice(pos, file.size));
    return parts;
  }

  /** A File holding `parts`, with the original's name, type and date. */
  function sameNamedFile(file, parts) {
    const FileCtor = root.File;
    const type = file.type || '';
    if (typeof FileCtor === 'function') {
      return new FileCtor(parts, file.name, { type, lastModified: file.lastModified });
    }
    const blob = new root.Blob(parts, { type });
    blob.name = file.name;
    blob.lastModified = file.lastModified;
    return blob;
  }

  /**
   * Clean a picked, pasted or dropped file. Never throws. Returns:
   *   original   the file as picked
   *   cleaned    what to send when the details are removed (the original itself when nothing
   *              changed, or for a file that could not be checked and is sent anyway)
   *   changed    `cleaned` differs from the original
   *   kind       what the bytes are (strip-metadata.js words: 'jpeg', 'png', 'webp', 'gif',
   *              'mp4', 'heif', 'webm', 'other')
   *   summary    the attach step's sentence ("This photo says where it was taken and which
   *              camera took it."), or null when nothing was found
   *   details    [{key, words}] the plain-word details behind it
   *   unchecked  it could not be checked: where it was taken may still be in it
   *   heif       a HEIC or AVIF photo (which this port cannot clean yet)
   *   reason     the plain sentence why it could not be checked, or null
   * `opts.wholeMax` overrides WHOLE_MAX (tests).
   */
  async function cleanFile(file, opts) {
    const o = opts || {};
    const wholeMax = Number.isFinite(o.wholeMax) ? o.wholeMax : WHOLE_MAX;
    const out = {
      original: file, cleaned: file, changed: false, kind: 'other', summary: null, details: [],
      unchecked: false, heif: false, reason: null,
    };
    if (!S) {
      out.unchecked = true;
      out.reason = WORDS.checkerMissing;
      return out;
    }
    try {
      const size = Number(file && file.size) || 0;
      const head = await readRange(file, 0, Math.min(size, HEAD_BYTES));
      const kind = S.sniffKind(head);
      const claimed = S.claimedKind(file);
      out.kind = kind;
      const take = (r, d) => {
        if (d) {
          out.summary = d.summary;
          out.details = d.details;
        }
        out.unchecked = !!r.unfinished;
        out.reason = r.unfinished ? r.untouchedReason : null;
        if (r.kind === 'heif') out.kind = 'heif';
      };

      if (kind === 'jpeg' || kind === 'png' || kind === 'webp') {
        if (size > PICTURE_MAX) {
          out.unchecked = true;
          out.reason = WORDS.tooLarge;
          return out;
        }
        const bytes = await readRange(file, 0, size);
        const r = S.stripFile(bytes, file);
        take(r, S.describe(bytes, file));
        if (r.changed) {
          out.cleaned = sameNamedFile(file, [r.bytes]);
          out.changed = true;
        }
        return out;
      }

      if (kind === 'mp4') {
        let r;
        let d;
        let parts = null;
        if (size <= wholeMax) {
          const bytes = await readRange(file, 0, size);
          r = S.stripFile(bytes, file);
          d = S.describe(bytes, file);
          if (r.changed) parts = [r.bytes];
        } else {
          const sin = await standIn(file);
          if (sin.tooLarge) {
            out.unchecked = true;
            out.reason = WORDS.boxTooLarge;
            return out;
          }
          r = S.stripFile(sin.bytes, file);
          d = S.describe(sin.bytes, file);
          if (r.changed) parts = fromStandIn(file, sin, r.bytes);
        }
        take(r, d);
        // A HEIC or AVIF photo (no movie in it), or a file NAMED as one in the same family: the
        // web cannot clean those yet (no HEIF strip on the web), so it counts as could not be
        // checked, and nothing of the video strip is applied to it.
        if (out.kind === 'heif' || claimed === 'heif') {
          out.kind = 'heif';
          out.heif = true;
          out.unchecked = true;
          out.reason = SAY.heif || WORDS.askPrivateHeif;
          return out;
        }
        if (parts) {
          out.cleaned = sameNamedFile(file, parts);
          out.changed = true;
        }
        return out;
      }

      // Anything else is left as it is, as the relay leaves it (GIF, WebM, a document); a file
      // NAMED as a picture or a video whose bytes are not one could not be checked.
      const r = S.stripFile(head, file);
      take(r, null);
      return out;
    } catch (e) {
      out.cleaned = file;
      out.changed = false;
      out.unchecked = true;
      out.reason = WORDS.unreadable;
      return out;
    }
  }

  // ── What to do with it ─────────────────────────────────────────────────

  /**
   * What a cleaned file's send looks like, for where it is going:
   *   ctx.where      'public' (a room, the Media page) or 'private' (a DM, a group, the scratch pad)
   *   ctx.remove     the switch (ignored with the protected setup on)
   *   ctx.protectedOn the protected setup is on
   * Returns:
   *   action        'send' (go), 'ask' (a private file that could not be checked: send only on
   *                 the person's yes) or 'refuse' (never sent)
   *   file          what to send
   *   keepMetadata  the upload says keep_metadata=1 (the person chose to keep the details)
   *   unchecked     the marker says "unchecked": true (only on an asked yes)
   *   words         the question or the refusal, or null
   *   canKeep       turning the switch off would let it go (a public refusal)
   */
  function decide(c, ctx) {
    const x = ctx || {};
    const protectedOn = !!x.protectedOn;
    const remove = protectedOn || x.remove !== false;
    if (!remove) return { action: 'send', file: c.original, keepMetadata: true, unchecked: false, words: null, canKeep: false };
    if (!c.unchecked) return { action: 'send', file: c.cleaned, keepMetadata: false, unchecked: false, words: null, canKeep: false };
    const why = c.reason || WORDS.unreadable;
    if (protectedOn) return { action: 'refuse', file: null, keepMetadata: false, unchecked: false, words: why + ' ' + WORDS.protectedRefused, canKeep: false };
    if (x.where !== 'private') {
      return {
        action: 'refuse', file: null, keepMetadata: false, unchecked: false, canKeep: true,
        words: (c.heif ? (SAY.heif || why) : why + ' ' + WORDS.publicRefused) + ' ' + WORDS.publicKeepHint,
      };
    }
    return {
      action: 'ask', file: c.cleaned, keepMetadata: false, unchecked: true, canKeep: false,
      words: c.heif ? WORDS.askPrivateHeif : WORDS.askPrivate,
    };
  }

  /**
   * Does the attach step need to be shown before this file goes? Only when there is something to
   * say or decide: hidden details found (the switch), a file that could not be checked (the
   * question or the refusal), or a line about the recipient (`extraLine`). A file with nothing
   * in it goes at once, as before.
   */
  function stepNeeded(c, ctx, extraLine) {
    const x = ctx || {};
    if (extraLine) return true;
    if (c.unchecked && (x.protectedOn || x.remove !== false)) return true;
    if (c.summary && !x.protectedOn) return true;
    return false;
  }

  const api = {
    REMOVE_DETAILS_KEY, PROTECTED_STORAGE_KEY, WHOLE_MAX, PICTURE_MAX, BOX_READ_MAX, WORDS,
    removeDetailsChosen, rememberRemoveDetails, protectedSetupOn, cleanFile, decide, stepNeeded,
    standIn,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosUploadClean = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
