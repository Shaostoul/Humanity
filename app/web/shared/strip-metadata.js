// ── strip-metadata.js ─────────────────────────────────────────────────────
// Removes hidden location and camera information from a picture or a video
// IN THE BROWSER, before the file is uploaded or encrypted (2026-10-11,
// docs/design/files-showcase.md, "The operator's answers (2026-10-11)",
// answer 4). The sender's own app strips, so a photo sent in a DM or a group,
// which the server only ever receives as a sealed blob it can never clean, no
// longer carries where it was taken. The relay keeps stripping public uploads
// as a backstop.
//
// THE SPECIFICATION IS THE RELAY'S RUST, and this is a port of it with the
// same rules and the same results, byte for byte:
//   src/relay/core/strip_metadata.rs        JPEG, PNG and WebP pictures
//   src/relay/core/strip_video_metadata.rs  MP4, MOV and 3GP videos
// scripts/tests/strip-metadata.test.js builds the same test files as the Rust
// tests, and where rustc is installed it strips thousands of inputs with the
// Rust itself and compares every byte and every report.
//
// What each kind of file gets, in plain words:
//   JPEG  whole segments removed: APP1 (Exif, where the GPS position lives,
//         and XMP), APP13 (Photoshop and IPTC captions and place names) and
//         COM (comments). Colour segments (JFIF, ICC, Adobe) are kept.
//   PNG   whole chunks removed: tEXt, zTXt, iTXt (text notes), eXIf (Exif)
//         and tIME (when it was last changed). Bytes after IEND go too.
//   WebP  the EXIF and XMP chunks removed, their flags cleared in the VP8X
//         header and the RIFF size rewritten.
//   GIF   passed through untouched, as the relay does: GIF has no standard
//         place for location or camera details.
//   MP4, MOV, 3GP  nothing is removed, because the movie's index records the
//         byte position of every chunk of frames: each metadata box is
//         NEUTRALISED where it stands instead (its contents overwritten with
//         zeros, then renamed `free`), so the file keeps its exact size. The
//         boxes: `udta` and `meta` inside `moov` and inside each `trak`, every
//         XMP `uuid` box at those levels and the top level, and a top-level
//         `meta` only in a file that also has a `moov`. Only box headers are
//         read; `mdat` (the frames) is never looked at.
//   HEIC, AVIF  NOT cleaned yet (the relay refuses them for the same reason):
//         their location sits in a top-level `meta` box that also holds the
//         picture itself. The report says so.
//   WebM and anything else  left as they are, as the relay leaves them.
//
// No re-encoding anywhere: pictures and sound are untouched.
//
// Broken files. A picture whose structure cannot be walked is given back
// exactly as it came (fail open, as the Rust does), and the report says so
// (`unfinished`, `untouchedReason`). A video whose boxes stop making sense is
// cleaned as far as the walk got, and `stoppedEarly` says where it stopped.
//
// THE API (window.HosStrip in the browser, module.exports under Node):
//
//   HosStrip.stripFile(bytes, nameOrType) -> {
//     bytes,           // Uint8Array to send. When nothing changed it is the
//                      // very array passed in (never written to); when
//                      // something changed it is a new array. A picture may
//                      // come back shorter; a video always the same length.
//     kind,            // what the BYTES are: 'jpeg' 'png' 'webp' 'gif' 'mp4'
//                      // (MOV and 3GP too) 'heif' 'webm' 'other'
//     claimed,         // what the name or type said, same words, or null
//     changed,         // true when `bytes` differs from the input
//     removed,         // [{path, offset, size}]: each segment, chunk or box
//                      // taken out (pictures) or neutralised (videos), with
//                      // its byte position and size in the ORIGINAL file
//     bytesZeroed,     // videos: how many bytes were overwritten with zeros
//     foundMoov,       // videos: a movie index was found and walked
//     stoppedEarly,    // videos: one sentence per place the walk had to stop
//                      // (the relay's own sentences, word for word)
//     unfinished,      // true when this module tried to clean the file and
//                      // could not finish: where it was taken may still be
//                      // in it (the "Send anyway?" question, answer 5)
//     untouchedReason, // a plain sentence when the file, or part of it, was
//                      // left as it was (null when it was read through)
//   }
//
//   HosStrip.describe(bytes, nameOrType, {coordinates: false}) -> {
//     kind, claimed,   // as in stripFile
//     hasLocation,     // it says where it was taken (a position or a place name)
//     hasCamera,       // it says which camera (make, model, serial or lens)
//     details,         // [{key, words}], plain words for the attach step,
//                      // only for what stripFile would remove
//     summary,         // "This photo says where it was taken and which
//                      // camera took it." or null when nothing was found
//     complete,        // false when the file could not be read through
//     coordinates,     // {latitude, longitude} ONLY when asked for with
//     place,           // {coordinates: true}; never in the words
//   }
//   Read only: describe never changes anything.
//
// `nameOrType` is a file name ("IMG_0001.JPG"), a type ("image/jpeg"), or a
// File-like object with `name` and `type`. The kind is decided by the bytes,
// which is what matters for cleaning; the claimed kind is reported beside it.
//
// Everything works on the bytes in memory. A video is walked by its box
// headers and copied once, only when something in it needs neutralising.
//
// A classic script in the browser (HosStrip lands on window), a CommonJS
// module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // ════════════════════════════════════════════════════════════════════════
  // Reading bytes
  // ════════════════════════════════════════════════════════════════════════

  /** The input as a Uint8Array, without copying. */
  function asBytes(input) {
    const tag = Object.prototype.toString.call(input);
    if (tag === '[object Uint8Array]') return input; // a Node Buffer says this too
    if (tag === '[object ArrayBuffer]' || tag === '[object SharedArrayBuffer]') return new Uint8Array(input);
    if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    throw new TypeError('HosStrip: the file must be a Uint8Array or an ArrayBuffer');
  }

  function be16(b, i) { return (b[i] << 8) | b[i + 1]; }
  function be32(b, i) { return b[i] * 0x1000000 + ((b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]); }
  function le32(b, i) { return b[i + 3] * 0x1000000 + ((b[i + 2] << 16) | (b[i + 1] << 8) | b[i]); }

  /** True when the four bytes at `i` spell `s` (four ASCII letters). */
  function is4(b, i, s) {
    return i >= 0 && i + 4 <= b.length &&
      b[i] === s.charCodeAt(0) && b[i + 1] === s.charCodeAt(1) &&
      b[i + 2] === s.charCodeAt(2) && b[i + 3] === s.charCodeAt(3);
  }

  /** Four bytes as a string, one character per byte (0xA9 is the copyright sign). */
  function name4(b, i) { return String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3]); }

  function startsWithAscii(b, i, s) {
    if (i < 0 || i + s.length > b.length) return false;
    for (let k = 0; k < s.length; k++) if (b[i + k] !== s.charCodeAt(k)) return false;
    return true;
  }

  function sameBytes(a, b) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
  }

  /** A new array made of `prefix` then the ranges [start, end) of `src`, in order. */
  function assemble(src, prefix, ranges) {
    let n = prefix ? prefix.length : 0;
    for (const r of ranges) n += r[1] - r[0];
    const out = new Uint8Array(n);
    let at = 0;
    if (prefix) { out.set(prefix, 0); at = prefix.length; }
    for (const r of ranges) { out.set(src.subarray(r[0], r[1]), at); at += r[1] - r[0]; }
    return out;
  }

  // ════════════════════════════════════════════════════════════════════════
  // What kind of file
  // ════════════════════════════════════════════════════════════════════════

  const TYPE_KIND = {
    'image/jpeg': 'jpeg', 'image/jpg': 'jpeg', 'image/pjpeg': 'jpeg',
    'image/png': 'png', 'image/apng': 'png',
    'image/webp': 'webp', 'image/gif': 'gif',
    'video/mp4': 'mp4', 'video/quicktime': 'mp4', 'video/3gpp': 'mp4', 'video/3gpp2': 'mp4',
    'video/x-m4v': 'mp4', 'audio/mp4': 'mp4', 'audio/x-m4a': 'mp4', 'audio/m4a': 'mp4',
    'image/heic': 'heif', 'image/heif': 'heif', 'image/heic-sequence': 'heif',
    'image/heif-sequence': 'heif', 'image/avif': 'heif',
    'video/webm': 'webm', 'audio/webm': 'webm', 'video/x-matroska': 'webm',
  };
  const EXT_KIND = {
    jpg: 'jpeg', jpeg: 'jpeg', jpe: 'jpeg', jfif: 'jpeg', png: 'png', apng: 'png',
    webp: 'webp', gif: 'gif',
    mp4: 'mp4', m4v: 'mp4', m4a: 'mp4', mov: 'mp4', qt: 'mp4', '3gp': 'mp4', '3g2': 'mp4',
    heic: 'heif', heif: 'heif', hif: 'heif', avif: 'heif',
    webm: 'webm', mkv: 'webm',
  };

  /** What a file name, a type, or a File-like {name, type} says the file is, or null. */
  function claimedKind(nameOrType) {
    if (!nameOrType) return null;
    if (typeof nameOrType === 'object') return claimedKind(nameOrType.type) || claimedKind(nameOrType.name);
    const s = String(nameOrType).trim().toLowerCase();
    const t = s.split(';')[0].trim();
    if (TYPE_KIND[t]) return TYPE_KIND[t];
    const dot = s.lastIndexOf('.');
    return EXT_KIND[dot >= 0 ? s.slice(dot + 1) : s] || null;
  }

  /**
   * The relay's `is_mp4_family`: the first box is an `ftyp` box with a usable
   * size (1 for a 64-bit size, or 8 and more). Eight bytes are enough.
   */
  function isMp4Family(b) {
    b = asBytes(b);
    if (b.length < 8 || !is4(b, 4, 'ftyp')) return false;
    const size = be32(b, 0);
    return size === 1 || size >= 8;
  }

  /**
   * What the bytes are. The picture signatures are the relay's own
   * (`picture_is_what_it_says` in src/relay/upload_rules.rs); the video ones
   * are the video strip's (`sniff`).
   */
  function sniffKind(b) {
    b = asBytes(b);
    const n = b.length;
    if (n >= 3 && b[0] === 0xFF && b[1] === 0xD8 && b[2] === 0xFF) return 'jpeg';
    if (n >= 4 && b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4E && b[3] === 0x47) return 'png';
    if (n >= 12 && is4(b, 0, 'RIFF') && is4(b, 8, 'WEBP')) return 'webp';
    if (n >= 6 && (startsWithAscii(b, 0, 'GIF87a') || startsWithAscii(b, 0, 'GIF89a'))) return 'gif';
    if (n >= 4 && b[0] === 0x1A && b[1] === 0x45 && b[2] === 0xDF && b[3] === 0xA3) return 'webm';
    if (isMp4Family(b)) return 'mp4';
    return 'other';
  }

  // The brands an `ftyp` box gives a HEIF or AVIF still picture.
  const HEIF_BRANDS = new Set(['heic', 'heix', 'heim', 'heis', 'hevc', 'hevx', 'hevm', 'hevs',
    'mif1', 'mif2', 'msf1', 'avif', 'avis', 'avci']);
  // The brands of an MP4-family file that is sound only (for the words).
  const AUDIO_BRANDS = new Set(['M4A ', 'M4B ', 'M4P ', 'F4A ', 'F4B ']);

  /** The `ftyp` box's major and compatible brands (the box was checked by isMp4Family). */
  function ftypBrands(b) {
    const size = be32(b, 0);
    if (size < 16 || size > b.length) return b.length >= 12 ? [name4(b, 8)] : [];
    const out = [name4(b, 8)];
    for (let i = 16; i + 4 <= size && out.length < 64; i += 4) out.push(name4(b, i));
    return out;
  }

  // ════════════════════════════════════════════════════════════════════════
  // The sentences
  // ════════════════════════════════════════════════════════════════════════

  const SAY = {
    pictureUnreadable: 'This picture could not be read all the way through, so it was left exactly as it was: where it was taken may still be in it.',
    videoUnreadable: 'This video could not be read all the way through, so where it was filmed may still be in it.',
    noMovie: 'No movie index was found in this file (it may have been cut off), so where it was filmed could not be checked.',
    heif: 'HEIC and AVIF photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send that.',
    gif: 'GIF pictures have no standard place for location or camera details, so it is sent as it is (the server does the same).',
    webm: 'WebM videos are not cleaned yet (the server does not clean them either), so it is sent as it is.',
    other: 'This is not a picture or video this app can clean, so it is sent as it is.',
    notWhatItSays: (claimed) => `This file is named as ${claimed === 'heif' ? 'a HEIC' : claimed === 'mp4' ? 'a video' : 'a ' + claimed.toUpperCase()} file but its first bytes are not one, so it could not be checked: where it was taken may still be in it.`,
  };

  // ════════════════════════════════════════════════════════════════════════
  // Pictures: strip_metadata.rs, line for line
  // ════════════════════════════════════════════════════════════════════════
  //
  // Each walker returns {ok, keep, removed}: `keep` the byte ranges that make
  // the result, `removed` what was left out. `ok` false is the Rust's `None`
  // (parse trouble), and the caller then gives back the original bytes.

  /** A plain name for a JPEG segment that is dropped. */
  function jpegLabel(d, i, marker) {
    if (marker === 0xE1) {
      if (startsWithAscii(d, i + 4, 'Exif\0\0')) return 'APP1 Exif';
      if (startsWithAscii(d, i + 4, 'http://ns.adobe.com/xap/1.0/\0')) return 'APP1 XMP';
      if (startsWithAscii(d, i + 4, 'http://ns.adobe.com/xmp/extension/\0')) return 'APP1 XMP extension';
      return 'APP1';
    }
    return marker === 0xED ? 'APP13' : 'COM';
  }

  /** strip_jpeg: the marker walk. */
  function walkJpeg(d) {
    const n = d.length;
    const w = { ok: false, keep: [], removed: [] };
    if (n < 4 || d[0] !== 0xFF || d[1] !== 0xD8) return w;
    w.keep.push([0, 2]); // SOI
    let i = 2;
    while (i + 4 <= n) {
      if (d[i] !== 0xFF) return w; // lost sync
      const marker = d[i + 1];
      // Standalone markers without a length (RSTn, TEM): tolerated.
      if ((marker >= 0xD0 && marker <= 0xD7) || marker === 0x01) {
        w.keep.push([i, i + 2]);
        i += 2;
        continue;
      }
      // Start of scan: the rest is copied verbatim, never walked.
      if (marker === 0xDA) {
        w.keep.push([i, n]);
        w.ok = true;
        return w;
      }
      // End of image before any scan: anything after it is dropped (the loop
      // condition means at least two bytes follow it here).
      if (marker === 0xD9) {
        w.keep.push([i, i + 2]);
        w.removed.push({ path: 'bytes after the end marker', offset: i + 2, size: n - i - 2 });
        w.ok = true;
        return w;
      }
      const len = be16(d, i + 2);
      if (len < 2 || i + 2 + len > n) return w;
      // APP1 (Exif + XMP: the GPS lives here), APP13 (Photoshop IRB / IPTC),
      // COM (comments).
      if (marker === 0xE1 || marker === 0xED || marker === 0xFE) {
        w.removed.push({ path: jpegLabel(d, i, marker), offset: i, size: 2 + len });
      } else {
        w.keep.push([i, i + 2 + len]);
      }
      i += 2 + len;
    }
    return w;
  }

  const PNG_SIG = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
  const PNG_DROP = new Set(['tEXt', 'zTXt', 'iTXt', 'eXIf', 'tIME']);

  /** strip_png: the chunk walk. */
  function walkPng(d) {
    const n = d.length;
    const w = { ok: false, keep: [], removed: [] };
    if (n < 8) return w;
    for (let k = 0; k < 8; k++) if (d[k] !== PNG_SIG[k]) return w;
    w.keep.push([0, 8]);
    let i = 8;
    while (i + 12 <= n) {
      const total = 12 + be32(d, i); // length + name + data + CRC
      if (i + total > n) return w;
      const name = name4(d, i + 4);
      if (PNG_DROP.has(name)) w.removed.push({ path: name, offset: i, size: total });
      else w.keep.push([i, i + total]);
      if (name === 'IEND') {
        if (i + total < n) w.removed.push({ path: 'bytes after IEND', offset: i + total, size: n - i - total });
        w.ok = true;
        return w;
      }
      i += total;
    }
    return w;
  }

  /**
   * strip_webp: the RIFF walk. Drops EXIF and XMP chunks, clears their flag
   * bits in VP8X (`flagAt`: positions in the rebuilt body), and the caller
   * writes a fresh RIFF size. The RIFF size in the input is never read.
   */
  function walkWebp(d) {
    const n = d.length;
    const w = { ok: false, keep: [], removed: [], flagAt: [], bodyLen: 0 };
    if (n < 12 || !is4(d, 0, 'RIFF') || !is4(d, 8, 'WEBP')) return w;
    let i = 12;
    while (i + 8 <= n) {
      const len = le32(d, i + 4);
      const padded = len + (len & 1); // chunks are 2-byte aligned
      if (i + 8 + padded > n) {
        // Tolerate a final unpadded chunk at the end of the file.
        if (i + 8 + len !== n) return w;
      }
      const end = Math.min(i + 8 + padded, n);
      const exif = is4(d, i, 'EXIF');
      if (exif || is4(d, i, 'XMP ')) {
        w.removed.push({ path: exif ? 'EXIF' : 'XMP', offset: i, size: end - i });
      } else {
        const start = w.bodyLen;
        w.keep.push([i, end]);
        w.bodyLen += end - i;
        if (is4(d, i, 'VP8X') && w.bodyLen > start + 8) w.flagAt.push(start + 8);
      }
      i += 8 + padded;
      if (i >= n) break;
    }
    // Fewer than eight bytes after the last chunk: the Rust leaves them out.
    if (i < n) w.removed.push({ path: 'bytes after the last chunk', offset: i, size: n - i });
    w.ok = true;
    return w;
  }

  /** The cleaned picture from a walk, or null when the walk had trouble. */
  function rebuildPicture(kind, d, w) {
    if (!w.ok) return null;
    if (kind !== 'webp') return assemble(d, null, w.keep);
    const size = (w.bodyLen + 4) >>> 0;
    const head = new Uint8Array([0x52, 0x49, 0x46, 0x46, size & 0xFF, (size >>> 8) & 0xFF,
      (size >>> 16) & 0xFF, (size >>> 24) & 0xFF, 0x57, 0x45, 0x42, 0x50]);
    const out = assemble(d, head, w.keep);
    // Clear the EXIF (bit 3) and XMP (bit 2) flags so readers do not look for
    // chunks that are no longer there.
    for (const at of w.flagAt) out[12 + at] &= ~0x0C;
    return out;
  }

  const PICTURE_WALK = { jpeg: walkJpeg, png: walkPng, webp: walkWebp };

  // ════════════════════════════════════════════════════════════════════════
  // Videos: strip_video_metadata.rs, line for line
  // ════════════════════════════════════════════════════════════════════════

  /** The 16-byte id of the `uuid` box that carries XMP (Adobe's XMP spec). */
  const XMP_UUID = [0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC];
  /** The most box headers one call reads (the Rust's MAX_BOXES). */
  const MAX_BOXES = 1000000;

  const TOP = { prefix: '', name: 'the top level of the file' };
  const MOOV = { prefix: 'moov/', name: 'moov' };
  const TRAK = { prefix: 'moov/trak/', name: 'a moov/trak' };

  /** The Rust's show_name: printable ASCII as it is, anything else as '?'. */
  function showName(b, i) {
    let s = '';
    for (let k = 0; k < 4; k++) {
      const c = b[i + k];
      s += (c >= 0x21 && c <= 0x7E) || c === 0x20 ? String.fromCharCode(c) : '?';
    }
    return s;
  }

  /** A 64-bit size as decimal text, exact at any size. */
  function u64Text(hi, lo) {
    if (hi < 0x200000) return String(hi * 4294967296 + lo);
    return String(BigInt(hi) * BigInt(4294967296) + BigInt(lo));
  }

  /**
   * The Rust's `header`: read the box at `pos` on a level ending at `end`.
   * Returns null for "no room" (fewer than 8 bytes left), a sentence for a
   * size that makes no sense, or the box.
   */
  function boxHeader(b, pos, end, level) {
    const room = end - pos;
    if (room < 8) return null;
    const size32 = be32(b, pos);
    const bad = (what) => `in ${level.name}, the box '${showName(b, pos + 4)}' at byte ${pos} ${what}`;
    let headerLen;
    let size;
    if (size32 === 0) {
      // Size 0: the box runs to the end of whatever holds it.
      headerLen = 8;
      size = room;
    } else if (size32 === 1) {
      // Size 1: the real size follows as a 64-bit number.
      if (room < 16) return bad('says a 64-bit size follows, but the data ends first');
      const hi = be32(b, pos + 8);
      const lo = be32(b, pos + 12);
      if (hi === 0 && lo < 16) return bad(`claims a 64-bit size of ${lo}, smaller than its own header`);
      // Exact below 2^53; anything above is far past `room` anyway.
      const large = hi * 4294967296 + lo;
      if (large > room) return bad(`claims ${u64Text(hi, lo)} bytes, but only ${room} are left there`);
      headerLen = 16;
      size = large;
    } else if (size32 < 8) {
      return bad(`claims a size of ${size32}, smaller than its own header`);
    } else {
      headerLen = 8;
      size = size32;
    }
    if (size > room) return bad(`claims ${size} bytes, but only ${room} are left there`);
    return { name: name4(b, pos + 4), start: pos, bodyStart: pos + headerLen, end: pos + size };
  }

  function isXmpUuid(b, h) {
    if (h.end - h.bodyStart < 16) return false;
    for (let k = 0; k < 16; k++) if (b[h.bodyStart + k] !== XMP_UUID[k]) return false;
    return true;
  }

  function plan(path, h) {
    return { path, offset: h.start, bodyStart: h.bodyStart, end: h.end };
  }

  /** The Rust's Walk::level: the boxes between `start` and `end` at one level. */
  function walkLevel(b, st, start, end, level) {
    let pos = start;
    while (pos < end) {
      if (st.boxesRead >= MAX_BOXES) {
        st.stoppedEarly.push(`stopped after reading ${MAX_BOXES} box headers, more than any real video has`);
        return;
      }
      st.boxesRead++;
      const h = boxHeader(b, pos, end, level);
      if (h === null) return; // too little room left to hold a box
      if (typeof h === 'string') {
        // Nobody can know where the next box at this level starts, so this
        // level ends here. The level above carries on.
        st.stoppedEarly.push(h);
        return;
      }
      const nm = h.name;
      if (level === TOP && nm === 'moov') {
        st.foundMoov = true;
        walkLevel(b, st, h.bodyStart, h.end, MOOV);
      } else if (level === MOOV && nm === 'trak') {
        walkLevel(b, st, h.bodyStart, h.end, TRAK);
      } else if (level === TOP && nm === 'meta') {
        st.topMeta.push(plan('meta', h)); // only if the file has a movie
      } else if ((level === MOOV || level === TRAK) && (nm === 'udta' || nm === 'meta')) {
        st.plans.push(plan(level.prefix + nm, h));
      } else if (nm === 'uuid') {
        // Only the XMP uuid: any other (360-degree video, a maker's data) may
        // be needed and is left alone.
        if (isXmpUuid(b, h)) st.plans.push(plan(level.prefix + 'uuid(xmp)', h));
      }
      // Everything else (mdat, mvhd, tkhd, mdia, edts, free...) is stepped
      // over without reading a byte of it.
      pos = h.end;
    }
  }

  /** Step 1 of strip_stream: walk the headers and list what to neutralise. */
  function walkMp4(b) {
    const st = { boxesRead: 0, foundMoov: false, plans: [], topMeta: [], stoppedEarly: [] };
    walkLevel(b, st, 0, b.length, TOP);
    let plans = st.foundMoov ? st.plans.concat(st.topMeta) : st.plans;
    plans.sort((x, y) => x.offset - y.offset);
    // A second lock on the same door: never write outside the data.
    const len = b.length;
    plans = plans.filter((p) => p.end <= len && p.offset + 8 <= p.bodyStart && p.bodyStart <= p.end);
    return { foundMoov: st.foundMoov, plans, stoppedEarly: st.stoppedEarly };
  }

  /** Steps 2 and 3, on a copy: zero every planned box's contents, then rename it `free`. */
  function neutralise(b, plans) {
    const out = new Uint8Array(b.length);
    out.set(b); // never Buffer.slice, which would share the caller's memory
    let zeroed = 0;
    for (const p of plans) {
      out.fill(0, p.bodyStart, p.end);
      zeroed += p.end - p.bodyStart;
    }
    for (const p of plans) {
      out[p.offset + 4] = 0x66; // f
      out[p.offset + 5] = 0x72; // r
      out[p.offset + 6] = 0x65; // e
      out[p.offset + 7] = 0x65; // e
    }
    return { out, zeroed };
  }

  // ════════════════════════════════════════════════════════════════════════
  // stripFile
  // ════════════════════════════════════════════════════════════════════════

  function stripFile(input, nameOrType) {
    const bytes = asBytes(input);
    const kind = sniffKind(bytes);
    const r = {
      bytes, kind, claimed: claimedKind(nameOrType), changed: false, removed: [],
      bytesZeroed: 0, foundMoov: false, stoppedEarly: [], unfinished: false, untouchedReason: null,
    };

    if (PICTURE_WALK[kind]) {
      const w = PICTURE_WALK[kind](bytes);
      const out = rebuildPicture(kind, bytes, w);
      if (!out) {
        // Fail open, as the Rust does: the original bytes, unchanged.
        r.unfinished = true;
        r.untouchedReason = SAY.pictureUnreadable;
        return r;
      }
      r.removed = w.removed;
      if (!sameBytes(out, bytes)) {
        r.bytes = out;
        r.changed = true;
      }
      return r;
    }

    if (kind === 'mp4') {
      const w = walkMp4(bytes);
      r.foundMoov = w.foundMoov;
      r.stoppedEarly = w.stoppedEarly;
      if (w.plans.length) {
        const n = neutralise(bytes, w.plans);
        r.bytes = n.out;
        r.changed = true;
        r.bytesZeroed = n.zeroed;
        r.removed = w.plans.map((p) => ({ path: p.path, offset: p.offset, size: p.end - p.offset }));
      }
      if (!w.foundMoov) {
        const heif = ftypBrands(bytes).some((x) => HEIF_BRANDS.has(x));
        if (heif) r.kind = 'heif';
        r.unfinished = true;
        r.untouchedReason = heif ? SAY.heif : SAY.noMovie;
      } else if (w.stoppedEarly.length) {
        r.unfinished = true;
        r.untouchedReason = SAY.videoUnreadable;
      }
      return r;
    }

    if (kind === 'gif') r.untouchedReason = SAY.gif;
    else if (kind === 'webm') r.untouchedReason = SAY.webm;
    else if (r.claimed && r.claimed !== 'gif' && r.claimed !== 'webm') {
      // Named as something this module cleans, but the bytes are not that.
      r.unfinished = true;
      r.untouchedReason = SAY.notWhatItSays(r.claimed);
    } else {
      r.untouchedReason = SAY.other;
    }
    return r;
  }

  // ════════════════════════════════════════════════════════════════════════
  // describe: what the hidden information says, in plain words (read only)
  // ════════════════════════════════════════════════════════════════════════
  //
  // It looks only inside what stripFile would remove, so the switch "Remove
  // location and camera details" removes exactly what this lists.

  /** The text of some bytes, control characters removed, at most `max` characters. */
  function textOf(b, s, e, max) {
    s = Math.max(0, s);
    e = Math.min(b.length, e);
    if (e <= s) return '';
    const view = b.subarray(s, Math.min(e, s + (max || 200) * 4));
    let t;
    if (typeof TextDecoder === 'function') t = new TextDecoder('utf-8').decode(view);
    else { t = ''; for (let k = 0; k < view.length; k++) t += String.fromCharCode(view[k]); }
    return t.replace(/[\u0000-\u001F\u007F]+/g, ' ').trim().slice(0, max || 200);
  }

  function hasContent(b, s, e) {
    for (let i = Math.max(0, s); i < Math.min(b.length, e); i++) if (b[i] !== 0) return true;
    return false;
  }

  function newFound() {
    return { keys: new Set(), hits: 0, make: '', model: '', coords: null, place: null };
  }
  function note(f, key) { f.keys.add(key); f.hits++; }
  function setCoords(f, c) { if (c && !f.coords) f.coords = c; }
  function setPlace(f, t) { if (t && !f.place) f.place = t; }
  function setMake(f, t) { note(f, 'camera'); if (t && !f.make) f.make = t; }
  function setModel(f, t) { note(f, 'camera'); if (t && !f.model) f.model = t; }

  /** Look inside one removed block; anything with content that says nothing known is "other". */
  function inspect(f, b, s, e, look) {
    const before = f.hits;
    look();
    if (f.hits === before && hasContent(b, s, e)) note(f, 'other');
  }

  function validCoords(lat, lon) {
    if (!isFinite(lat) || !isFinite(lon) || Math.abs(lat) > 90 || Math.abs(lon) > 180) return null;
    return { latitude: lat, longitude: lon };
  }

  /** ISO 6709 text ("+37.3349-122.0090+030.000/"), as Apple and Android write it. */
  function parseIso6709(t) {
    const m = /([+-])(\d+)(\.\d+)?([+-])(\d+)(\.\d+)?/.exec(t || '');
    if (!m) return null;
    const part = (sign, int, frac, deg) => {
      frac = frac || '';
      let v;
      if (int.length <= deg) v = parseFloat(int + frac);
      else if (int.length === deg + 2) v = parseInt(int.slice(0, deg), 10) + parseFloat(int.slice(deg) + frac) / 60;
      else if (int.length === deg + 4) {
        v = parseInt(int.slice(0, deg), 10) + parseInt(int.slice(deg, deg + 2), 10) / 60 + parseFloat(int.slice(deg + 2) + frac) / 3600;
      } else return NaN;
      return sign === '-' ? -v : v;
    };
    return validCoords(part(m[1], m[2], m[3], 2), part(m[4], m[5], m[6], 3));
  }

  /** An XMP GPS value: "37,20.094N", "37,20,5.6N", or a plain decimal. */
  function parseXmpGps(t) {
    if (!t) return NaN;
    const m = /^\s*(\d+(?:\.\d+)?),(\d+(?:\.\d+)?)(?:,(\d+(?:\.\d+)?))?\s*([NSEW])/i.exec(t);
    if (m) {
      const v = parseFloat(m[1]) + parseFloat(m[2]) / 60 + (m[3] ? parseFloat(m[3]) / 3600 : 0);
      return /[SW]/i.test(m[4]) ? -v : v;
    }
    return parseFloat(t);
  }

  // ---------- Exif (a TIFF structure) ----------

  const TIFF_UNIT = [0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8, 4];

  /** Read an Exif block (TIFF header at `start`) for what it says. */
  function readTiff(b, start, end, f) {
    end = Math.min(end, b.length);
    if (end - start < 8) return;
    let little;
    if (b[start] === 0x49 && b[start + 1] === 0x49) little = true;
    else if (b[start] === 0x4D && b[start + 1] === 0x4D) little = false;
    else return;
    const u16 = (o) => (little ? b[o] | (b[o + 1] << 8) : be16(b, o));
    const u32 = (o) => (little ? le32(b, o) : be32(b, o));
    if (u16(start + 2) !== 42) return;
    const seen = new Set();
    const gps = { latRef: '', lat: null, lonRef: '', lon: null, other: false };

    // ASCII values end at their first zero byte.
    const ascii = (at, n) => {
      let stop = at;
      while (stop < at + n && b[stop] !== 0) stop++;
      return textOf(b, at, stop, 120);
    };
    const rationals = (at, n) => {
      const out = [];
      for (let k = 0; k < n; k++) {
        const den = u32(at + 8 * k + 4);
        out.push(den ? u32(at + 8 * k) / den : NaN);
      }
      return out;
    };

    const ifd = (rel, which) => {
      if (seen.has(rel) || seen.size >= 8) return;
      seen.add(rel);
      const at = start + rel;
      if (rel < 8 || at + 2 > end) return;
      const count = u16(at);
      for (let k = 0; k < count && k < 1024; k++) {
        const e = at + 2 + 12 * k;
        if (e + 12 > end) return;
        const tag = u16(e);
        const type = u16(e + 2);
        const n = u32(e + 4);
        const bytes = (TIFF_UNIT[type] || 0) * n;
        const valAt = bytes <= 4 ? e + 8 : start + u32(e + 8);
        const ok = bytes > 0 && valAt >= start && valAt + bytes <= end;
        if (which === 'ifd0') {
          if (tag === 0x8769) ifd(u32(e + 8), 'exif');
          else if (tag === 0x8825) ifd(u32(e + 8), 'gps');
          else if (tag === 0x010F) setMake(f, ok && type === 2 ? ascii(valAt, n) : '');
          else if (tag === 0x0110) setModel(f, ok && type === 2 ? ascii(valAt, n) : '');
          else if (tag === 0x0131) note(f, 'software');
          else if (tag === 0x0132) note(f, 'time');
          else if (tag === 0x013B || tag === 0x8298 || tag === 0x9C9D) note(f, 'person');
          else if (tag === 0x010E || (tag >= 0x9C9B && tag <= 0x9C9F)) note(f, 'comment');
        } else if (which === 'exif') {
          if (tag === 0x9003 || tag === 0x9004) note(f, 'time');
          else if (tag === 0xA430) note(f, 'person');
          else if (tag === 0xA431 || tag === 0xA435) note(f, 'serial');
          else if (tag >= 0xA432 && tag <= 0xA434) note(f, 'lens');
          else if (tag === 0x9286 && ok && hasText(b, valAt + 8, valAt + bytes)) note(f, 'comment');
          else if (tag === 0x927C || tag === 0xA420) note(f, 'other'); // maker notes, unique id
        } else if (which === 'gps') {
          if (tag === 1 && ok) gps.latRef = ascii(valAt, n);
          else if (tag === 3 && ok) gps.lonRef = ascii(valAt, n);
          else if (tag === 2 || tag === 4) {
            note(f, 'location');
            if (ok && type === 5 && n >= 3) {
              const r = rationals(valAt, 3);
              const v = r[0] + r[1] / 60 + r[2] / 3600;
              if (tag === 2) gps.lat = v; else gps.lon = v;
            }
          } else gps.other = true;
        }
      }
    };
    ifd(u32(start + 4), 'ifd0');
    if (gps.lat !== null && gps.lon !== null) {
      setCoords(f, validCoords(/S/i.test(gps.latRef) ? -gps.lat : gps.lat, /W/i.test(gps.lonRef) ? -gps.lon : gps.lon));
    } else if (gps.other && !f.keys.has('location')) note(f, 'other');
  }

  function hasText(b, s, e) {
    for (let i = Math.max(0, s); i < Math.min(b.length, e); i++) if (b[i] > 0x20) return true;
    return false;
  }

  /** An Exif block that may begin with "Exif\0\0" (PNG eXIf and WebP EXIF sometimes do). */
  function readExifBlock(b, s, e, f) {
    readTiff(b, startsWithAscii(b, s, 'Exif\0\0') ? s + 6 : s, e, f);
  }

  // ---------- XMP (text) ----------

  function xmpValue(text, tag) {
    const t = tag.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const m = new RegExp(t + '\\s*=\\s*"([^"]*)"|<' + t + '(?:\\s[^>]*)?>([^<]*)</' + t + '>').exec(text);
    return m ? (m[1] !== undefined ? m[1] : m[2]).trim() : '';
  }

  const XMP_RULES = [
    ['location', /GPS(?:Latitude|Longitude)\b/i],
    ['place-name', /photoshop:(?:City|State|Country)\b|Iptc4xmpCore:(?:Location|CountryCode)\b|Iptc4xmpExt:(?:LocationShown|LocationCreated|City|CountryName|ProvinceState|Sublocation)\b/],
    ['serial', /aux:SerialNumber|exifEX:BodySerialNumber|LensSerialNumber/],
    ['lens', /aux:Lens\b|aux:LensInfo|aux:LensID|exifEX:LensModel|exifEX:LensMake/],
    ['software', /xmp:CreatorTool|tiff:Software/],
    ['time', /xmp:CreateDate|exif:DateTimeOriginal|photoshop:DateCreated|xmp:ModifyDate/],
    ['person', /dc:creator|xmpRights:Owner|aux:OwnerName|exifEX:CameraOwnerName|tiff:Artist|dc:rights/],
    ['comment', /dc:description|dc:title|exif:UserComment|photoshop:Headline/],
  ];

  function scanXmp(b, s, e, f) {
    const text = textOf(b, s, Math.min(e, s + 1048576), 1048576);
    for (const [key, re] of XMP_RULES) if (re.test(text)) note(f, key);
    const make = xmpValue(text, 'tiff:Make');
    const model = xmpValue(text, 'tiff:Model');
    if (make) setMake(f, make);
    if (model) setModel(f, model);
    if (f.keys.has('location')) {
      const lat = xmpValue(text, 'exif:GPSLatitude') || xmpValue(text, 'drone-dji:GpsLatitude');
      const lon = xmpValue(text, 'exif:GPSLongitude') || xmpValue(text, 'drone-dji:GpsLongitude');
      setCoords(f, validCoords(parseXmpGps(lat), parseXmpGps(lon)));
    }
    setPlace(f, xmpValue(text, 'photoshop:City') || xmpValue(text, 'Iptc4xmpCore:Location'));
  }

  // ---------- IPTC (in a JPEG's APP13) ----------

  function scanIptc(b, s, e, f) {
    let i = s;
    while (i + 5 <= e) {
      if (b[i] === 0x1C && b[i + 1] === 0x02) {
        const ds = b[i + 2];
        const len = be16(b, i + 3);
        if (!(len & 0x8000) && i + 5 + len <= e) {
          if ([90, 92, 95, 100, 101].includes(ds)) {
            note(f, 'place-name');
            if (ds === 90) setPlace(f, textOf(b, i + 5, i + 5 + len, 120));
          } else if ([80, 116, 118].includes(ds)) note(f, 'person');
          else if ([5, 105, 120].includes(ds)) note(f, 'comment');
          else if (ds === 55 || ds === 60) note(f, 'time');
          else if (ds === 65) note(f, 'software');
          else if (ds === 25) note(f, 'text');
          i += 5 + len;
          continue;
        }
      }
      i++;
    }
  }

  // ---------- PNG text chunks ----------

  /** The keyword of a tEXt, zTXt or iTXt chunk (Latin-1, up to 79 letters). */
  function pngKeyword(b, s, e) {
    let k = '';
    for (let i = s; i < Math.min(e, s + 80); i++) {
      if (b[i] === 0) return { word: k, next: i + 1 };
      k += String.fromCharCode(b[i]);
    }
    return { word: k, next: -1 };
  }

  /** ImageMagick's "Raw profile type exif" text ("exif", the length, then hex), from s to e. */
  function rawProfileExif(b, s, e, f) {
    let t = '';
    for (let i = s; i < Math.min(e, s + 4194304); i++) t += String.fromCharCode(b[i]);
    const tokens = t.trim().split(/\s+/);
    if (tokens.length < 3 || !/^\d+$/.test(tokens[1])) return false;
    const hex = tokens.slice(2).join('');
    if (!/^[0-9a-fA-F]*$/.test(hex) || hex.length < 16) return false;
    const bytes = new Uint8Array(Math.min(hex.length >> 1, 1048576));
    for (let k = 0; k < bytes.length; k++) bytes[k] = parseInt(hex.substr(2 * k, 2), 16);
    readExifBlock(bytes, 0, bytes.length, f);
    return true;
  }

  function pngTextChunk(b, name, s, e, f) {
    const kw = pngKeyword(b, s, e);
    const word = kw.word.toLowerCase();
    // Where the text itself starts, when it is not compressed.
    let textAt = -1;
    if (name === 'tEXt') textAt = kw.next;
    else if (name === 'iTXt' && kw.next >= 0 && kw.next + 2 <= e && b[kw.next] === 0) {
      let p = kw.next + 2;
      for (let skip = 0; skip < 2 && p >= 0; skip++) { // language tag, then translated keyword
        while (p < e && b[p] !== 0) p++;
        p = p < e ? p + 1 : -1;
      }
      textAt = p;
    }
    if (word === 'xml:com.adobe.xmp') {
      if (textAt >= 0) scanXmp(b, textAt, e, f); else note(f, 'unread');
    } else if (word === 'raw profile type exif' || word === 'raw profile type app1') {
      if (!(textAt >= 0 && rawProfileExif(b, textAt, e, f))) {
        // The text is usually compressed: its contents cannot be read here.
        note(f, 'unread');
      }
    } else if (word === 'author' || word === 'copyright' || word === 'artist') note(f, 'person');
    else if (word === 'title' || word === 'description' || word === 'comment') note(f, 'comment');
    else if (word === 'creation time') note(f, 'time');
    else if (word === 'software') note(f, 'software');
    else if (word === 'source') setModel(f, textAt >= 0 ? textOf(b, textAt, e, 80) : '');
    else note(f, 'text');
  }

  // ---------- MP4 metadata boxes ----------

  /** Walk the child boxes between s and e; false when a size made no sense. */
  function eachChild(b, s, e, budget, fn) {
    let pos = s;
    while (pos + 8 <= e) {
      if (--budget.left < 0) return false;
      let size = be32(b, pos);
      let hl = 8;
      if (size === 0) size = e - pos;
      else if (size === 1) {
        if (pos + 16 > e || be32(b, pos + 8) !== 0) return false;
        size = be32(b, pos + 12);
        hl = 16;
        if (size < 16) return false;
      } else if (size < 8) return false;
      if (pos + size > e) return false;
      fn(name4(b, pos + 4), pos + hl, pos + size, pos);
      pos += size;
    }
    return true;
  }

  /** The text of a metadata item: a `data` box (ilst style) or QuickTime's length-prefixed text. */
  function atomText(b, s, e) {
    if (e - s >= 16 && is4(b, s + 4, 'data')) return textOf(b, s + 16, Math.min(e, s + be32(b, s)), 200);
    if (e - s >= 4) return textOf(b, s + 4, Math.min(e, s + 4 + be16(b, s)), 200);
    return '';
  }

  // What each item name in `udta` or an `ilst` says. (0xA9 is the copyright sign.)
  const ATOM_KEY = {
    '©xyz': 'location', '©mak': 'make', '©mod': 'model', manu: 'make', modl: 'model',
    '©swr': 'software', '©too': 'software', '©enc': 'software', FIRM: 'software',
    '©day': 'time',
    '©ART': 'person', '©aut': 'person', '©wrt': 'person', '©prf': 'person',
    '©cpy': 'person', auth: 'person', cprt: 'person', perf: 'person',
    '©nam': 'comment', '©cmt': 'comment', '©des': 'comment', '©inf': 'comment',
    desc: 'comment', dscp: 'comment', titl: 'comment',
    kywd: 'text', '©key': 'text',
    CAME: 'serial', LENS: 'lens',
  };

  function applyItem(f, key, value) {
    if (key === 'location') { note(f, 'location'); setCoords(f, parseIso6709(value)); }
    else if (key === 'make') setMake(f, value);
    else if (key === 'model') setModel(f, value);
    else note(f, key);
  }

  /** One of Apple's QuickTime metadata keys (com.apple.quicktime.*). */
  function appleKey(f, key, value) {
    const k = key.replace(/^com\.apple\.quicktime\./, '');
    if (k === 'location.ISO6709') { note(f, 'location'); setCoords(f, parseIso6709(value)); }
    else if (k === 'location.name') { note(f, 'place-name'); setPlace(f, value); }
    else if (k === 'make' || key === 'com.android.manufacturer') setMake(f, value);
    else if (k === 'model' || key === 'com.android.model') setModel(f, value);
    else if (k === 'software' || key === 'com.android.version') note(f, 'software');
    else if (k === 'creationdate') note(f, 'time');
    else if (['author', 'artist', 'copyright', 'publisher'].includes(k)) note(f, 'person');
    else if (['comment', 'description', 'title', 'displayname', 'information'].includes(k)) note(f, 'comment');
    else if (k === 'keywords') note(f, 'text');
    else if (/^camera\.(?:lens|identifier)/.test(k)) note(f, 'lens');
    else note(f, 'other');
  }

  /** A `meta` box: QuickTime's (children right away) or ISO's (4 bytes of version and flags first). */
  function metaBox(b, s, e, f, budget) {
    let c = s;
    if (e - s >= 12 && !is4(b, s + 4, 'hdlr') && (is4(b, s + 8, 'hdlr') || be32(b, s) === 0)) c = s + 4;
    const keys = [];
    let ilst = null;
    eachChild(b, c, e, budget, (nm, cs, ce) => {
      if (nm === 'keys') {
        let p = cs + 8; // version and flags, then the count
        const count = ce - cs >= 8 ? be32(b, cs + 4) : 0;
        for (let k = 0; k < count && k < 4096 && p + 8 <= ce; k++) {
          const size = be32(b, p);
          if (size < 8 || p + size > ce) break;
          keys.push(textOf(b, p + 8, p + size, 200));
          p += size;
        }
      } else if (nm === 'ilst') ilst = [cs, ce];
      else if (nm === 'XMP_') scanXmp(b, cs, ce, f);
    });
    if (!ilst) return;
    eachChild(b, ilst[0], ilst[1], budget, (nm, cs, ce, at) => {
      const index = be32(b, at + 4);
      const value = atomText(b, cs, ce);
      if (keys.length && index >= 1 && index <= keys.length) appleKey(f, keys[index - 1], value);
      else if (ATOM_KEY[nm]) applyItem(f, ATOM_KEY[nm], value);
      else if (hasContent(b, cs, ce)) note(f, 'other');
    });
  }

  /** 3GPP's `loci`: a named place with its coordinates (16.16 fixed point, longitude first). */
  function lociBox(b, s, e, f) {
    note(f, 'location');
    note(f, 'place-name');
    let p = s + 6; // version and flags, language
    if (p >= e) return;
    let name = '';
    if (b[p] === 0xFE && b[p + 1] === 0xFF) {
      let q = p + 2;
      while (q + 1 < e && (b[q] | b[q + 1])) { name += String.fromCharCode((b[q] << 8) | b[q + 1]); q += 2; }
      p = q + 2;
    } else {
      let q = p;
      while (q < e && b[q] !== 0) q++;
      name = textOf(b, p, q, 120);
      p = q + 1;
    }
    setPlace(f, name.trim());
    p += 1; // role
    if (p + 8 > e) return;
    const fixed = (o) => { const v = be32(b, o); return (v >= 0x80000000 ? v - 0x100000000 : v) / 65536; };
    setCoords(f, validCoords(fixed(p + 4), fixed(p)));
  }

  function udtaBox(b, s, e, f, budget) {
    eachChild(b, s, e, budget, (nm, cs, ce) => {
      if (nm === 'meta') metaBox(b, cs, ce, f, budget);
      else if (nm === 'XMP_') scanXmp(b, cs, ce, f);
      else if (nm === 'loci') lociBox(b, cs, ce, f);
      else if (ATOM_KEY[nm]) applyItem(f, ATOM_KEY[nm], atomText(b, cs, ce));
      else if (hasContent(b, cs, ce)) note(f, 'other');
    });
  }

  // ---------- describe ----------

  const ORDER = ['location', 'place-name', 'camera', 'serial', 'lens', 'time', 'changed-time',
    'person', 'software', 'comment', 'text', 'unread', 'other'];

  /** The camera's name from its make and model ("Apple iPhone 15 Pro"; "Canon EOS R5", not "Canon Canon EOS R5"). */
  function cameraName(f) {
    const make = f.make.trim();
    const model = f.model.trim();
    if (!make) return model;
    if (!model) return make;
    return model.toLowerCase().startsWith(make.toLowerCase()) ? model : `${make} ${model}`;
  }

  function wordsFor(key, medium, f) {
    const taken = medium === 'photo' ? 'taken' : medium === 'video' ? 'filmed' : 'recorded';
    const took = medium === 'photo' ? 'took' : medium === 'video' ? 'filmed' : 'recorded';
    const device = medium === 'recording' ? 'device' : 'camera or phone';
    switch (key) {
      case 'location': return `Where it was ${taken}`;
      case 'place-name': return 'The name of a place (an address, a city or a country)';
      case 'camera': {
        const name = cameraName(f);
        return `Which ${device} ${took} it` + (name ? ` (${name})` : '');
      }
      case 'serial': return "The camera's own serial number";
      case 'lens': return 'Which lens was used';
      case 'time': return `When it was ${taken}`;
      case 'changed-time': return 'When it was last changed';
      case 'person': return "A person's name (who made it or who owns the camera)";
      case 'software': return 'Which program or app made or edited it';
      case 'comment': return 'A title, caption or comment';
      case 'text': return 'Text notes stored inside the file';
      case 'unread': return `Packed details this app could not open (they may say where it was ${taken})`;
      default: return 'Other hidden details';
    }
  }

  function summaryFor(medium, hasLocation, hasCamera, anything) {
    const noun = medium === 'photo' ? 'This photo' : medium === 'video' ? 'This video' : 'This recording';
    const taken = medium === 'photo' ? 'taken' : medium === 'video' ? 'filmed' : 'recorded';
    const which = medium === 'photo' ? 'which camera took it' : medium === 'video' ? 'which camera filmed it' : 'which device recorded it';
    if (hasLocation && hasCamera) return `${noun} says where it was ${taken} and ${which}.`;
    if (hasLocation) return `${noun} says where it was ${taken}.`;
    if (hasCamera) return `${noun} says ${which}.`;
    if (anything) return `${noun} carries other hidden details.`;
    return null;
  }

  function describe(input, nameOrType, options) {
    const b = asBytes(input);
    let kind = sniffKind(b);
    const f = newFound();
    const budget = { left: 100000 };
    let complete = false;
    let medium = 'photo';

    if (kind === 'jpeg') {
      const w = walkJpeg(b);
      complete = w.ok;
      for (const r of w.removed) {
        const s = r.offset + 4;
        const e = r.offset + r.size;
        if (r.path === 'APP1 Exif') inspect(f, b, s, e, () => readTiff(b, s + 6, e, f));
        else if (r.path === 'APP1 XMP' || r.path === 'APP1 XMP extension') inspect(f, b, s, e, () => scanXmp(b, s, e, f));
        else if (r.path === 'APP13') inspect(f, b, s, e, () => scanIptc(b, s, e, f));
        else if (r.path === 'COM') { if (hasText(b, s, e)) note(f, 'comment'); }
        else inspect(f, b, r.offset, e, () => {});
      }
    } else if (kind === 'png') {
      const w = walkPng(b);
      complete = w.ok;
      for (const r of w.removed) {
        const s = r.offset + 8;
        const e = r.offset + r.size - 4; // the chunk's data, without its CRC
        if (r.path === 'eXIf') inspect(f, b, s, e, () => readExifBlock(b, s, e, f));
        else if (r.path === 'tIME') note(f, 'changed-time');
        else if (r.path === 'tEXt' || r.path === 'zTXt' || r.path === 'iTXt') pngTextChunk(b, r.path, s, e, f);
        else inspect(f, b, r.offset, r.offset + r.size, () => {});
      }
    } else if (kind === 'webp') {
      const w = walkWebp(b);
      complete = w.ok;
      for (const r of w.removed) {
        const s = r.offset + 8;
        const e = r.offset + r.size;
        if (r.path === 'EXIF') inspect(f, b, s, e, () => readExifBlock(b, s, e, f));
        else if (r.path === 'XMP') inspect(f, b, s, e, () => scanXmp(b, s, e, f));
        else inspect(f, b, r.offset, e, () => {});
      }
    } else if (kind === 'mp4') {
      const w = walkMp4(b);
      complete = w.foundMoov && !w.stoppedEarly.length;
      const brands = ftypBrands(b);
      if (!w.foundMoov && brands.some((x) => HEIF_BRANDS.has(x))) kind = 'heif';
      else medium = AUDIO_BRANDS.has(brands[0]) ? 'recording' : 'video';
      for (const p of w.plans) {
        const leaf = p.path.slice(p.path.lastIndexOf('/') + 1);
        if (leaf === 'uuid(xmp)') inspect(f, b, p.bodyStart + 16, p.end, () => scanXmp(b, p.bodyStart + 16, p.end, f));
        else if (leaf === 'meta') inspect(f, b, p.bodyStart, p.end, () => metaBox(b, p.bodyStart, p.end, f, budget));
        else inspect(f, b, p.bodyStart, p.end, () => udtaBox(b, p.bodyStart, p.end, f, budget));
      }
    } else if (kind === 'gif') {
      complete = true; // nothing standard to look for, and nothing is removed
    }

    const details = ORDER.filter((k) => f.keys.has(k)).map((key) => ({ key, words: wordsFor(key, medium, f) }));
    const hasLocation = f.keys.has('location') || f.keys.has('place-name');
    const hasCamera = f.keys.has('camera') || f.keys.has('serial') || f.keys.has('lens');
    const out = {
      kind, claimed: claimedKind(nameOrType), hasLocation, hasCamera, details,
      summary: summaryFor(medium, hasLocation, hasCamera, details.length > 0),
      complete,
    };
    if (options && options.coordinates) {
      out.coordinates = f.coords;
      out.place = f.place;
    }
    return out;
  }

  const api = {
    stripFile, describe, claimedKind, sniffKind, isMp4Family,
    XMP_UUID, MAX_BOXES, SAY,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosStrip = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
