// ── profile-copy.js ───────────────────────────────────────────────────────
// The copy of my profile that other servers keep (BUG-191, 2026-10-12,
// docs/design/bug-191-profile-copies-plan.md sections 3.1 to 3.4 and 3.11).
// The website's twin of the relay's src/relay/core/profile_copy.rs, held to it
// byte for byte by scripts/tests/profile-copy-web.test.js (which reads the
// Rust constants and the pinned words out of that file) and by the shared
// fixture scripts/tests/fixtures/profile-copy-v1.json (made by the Rust
// builder, read by both apps' tests).
//
// How it is used: when the server I am signed in to sends me my own complete
// copy of my profile with `sign_now: true`, the chat (web/chat/chat-profile.js
// signProfileCopy) builds the PUBLIC copy of exactly what the server stored,
// signs these words with my Dilithium3 key, and sends one
// {"type":"profile_sign","at":...,"signature":"..."}. The server checks the
// signature over the copy it rebuilds itself, keeps it, and passes it on to
// the servers it is linked with.
//
// THE PRIVACY RULE. The copy is my profile as anyone may see it:
//   - name, bio, avatar, banner and links always;
//   - pronouns, location and website blank when I keep them for friends
//     (`"private"` under the field's name in my privacy map);
//   - every field blank (a "withdrawal") when I am unlisted
//     (`"directory": "unlisted"`).
// A field kept for friends never leaves the server, signed or not.
//
// The words signed, as UTF-8:
//   profile_v1\n{key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n
//   {pronouns}\n{location}\n{website}\n{at}
// `at` in decimal milliseconds. No field but the bio may hold a line break, so
// the words split one way only. Nothing is trimmed or re-serialized: the links
// are the JSON text the server stored, in its own key order.
//
// Limits are counted the way the relay counts them: the bio, pronouns and
// location in characters (Unicode code points, not JS .length, which counts a
// character outside the Basic Multilingual Plane twice); the links, avatar,
// banner and website in UTF-8 bytes; only "\n" is refused in a one-line field.
//
// A classic script in the browser (window.HosProfileCopy) and a CommonJS
// module under Node (the tests), the way profile-changes.js is. Load before
// chat-profile.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** The fields a person may keep for friends. Must match the Rust list. */
  const FIELDS_FOR_FRIENDS = ['pronouns', 'location', 'website'];
  /** The tag that opens the signed words. */
  const COPY_TAG = 'profile_v1';
  /** The longest bio, in characters. */
  const BIO_MAX_CHARS = 280;
  /** The longest name, in bytes (names are ASCII: letters, digits, `_` and `-`). */
  const NAME_MAX_BYTES = 24;
  /** The links (a JSON object), in bytes. */
  const LINKS_MAX_BYTES = 1024;
  /** An avatar or banner link, in bytes. */
  const PICTURE_LINK_MAX_BYTES = 512;
  /** The website link, in bytes. */
  const WEBSITE_MAX_BYTES = 256;
  /** Pronouns, in characters. */
  const PRONOUNS_MAX_CHARS = 64;
  /** Location, in characters. */
  const LOCATION_MAX_CHARS = 128;
  /** A person's key in a copy: a Dilithium3 public key (1,952 bytes) in lowercase hex. */
  const KEY_HEX_LEN = 1952 * 2;
  /** A copy's signature: a Dilithium3 signature (3,309 bytes) in lowercase hex. */
  const SIGNATURE_HEX_LEN = 3309 * 2;
  /** How far ahead of a server's clock a copy may be dated and still be kept: a day. */
  const MAX_AHEAD_MS = 24 * 60 * 60 * 1000;

  /** A field's text: a string as it is, anything else (an absent field) blank. */
  const text = (v) => (typeof v === 'string' ? v : '');
  const own = (m, k) => (Object.prototype.hasOwnProperty.call(m, k) ? m[k] : undefined);

  /** The privacy map as an object, or null when the text is not a JSON object. */
  function mapOf(privacyText) {
    if (typeof privacyText !== 'string') return null;
    try {
      const v = JSON.parse(privacyText);
      return v && typeof v === 'object' && !Array.isArray(v) ? v : null;
    } catch (e) {
      return null;
    }
  }

  /** Am I unlisted: the map's "directory" is the string "unlisted". */
  function unlisted(privacyText) {
    const m = mapOf(privacyText);
    return !!m && own(m, 'directory') === 'unlisted';
  }

  /**
   * Do I keep `field` for friends: the map's value for it is the string
   * "private". Any other value, a value that is not a string, or a map that is
   * not an object leaves it public.
   */
  function keptForFriends(privacyText, field) {
    const m = mapOf(privacyText);
    return !!m && own(m, field) === 'private';
  }

  /** The withdrawal: what an unlisted person's copy is. Every field empty, links "{}". */
  function withdrawal() {
    return { name: '', bio: '', avatar_url: '', banner_url: '', socials: '{}', pronouns: '', location: '', website: '' };
  }

  /** Is this copy a withdrawal (a real name is never empty). */
  function isWithdrawal(c) {
    return !!c && text(c.name) === '';
  }

  /**
   * The copy for other servers of my own copy `p` (a profile_data frame, or
   * anything with its field names), under my privacy map's text. `name` is the
   * own copy's name exactly. Unlisted gives the withdrawal; otherwise every
   * field as the server stored it, links blank read as "{}", and each of
   * FIELDS_FOR_FRIENDS blank when kept for friends. No trimming.
   */
  function publicCopy(name, p, privacyText) {
    if (unlisted(privacyText)) return withdrawal();
    const f = p && typeof p === 'object' ? p : {};
    const shown = (field) => (keptForFriends(privacyText, field) ? '' : text(f[field]));
    const socials = text(f.socials);
    return {
      name: text(name),
      bio: text(f.bio),
      avatar_url: text(f.avatar_url),
      banner_url: text(f.banner_url),
      socials: socials === '' ? '{}' : socials,
      pronouns: shown('pronouns'),
      location: shown('location'),
      website: shown('website'),
    };
  }

  /** The words a copy's signature covers (sign their UTF-8 bytes). Byte for byte the relay's `copy_text`. */
  function copyText(publicKey, c, at) {
    return `${COPY_TAG}\n${publicKey}\n${c.name}\n${c.bio}\n${c.avatar_url}\n${c.banner_url}\n${c.socials}\n`
      + `${c.pronouns}\n${c.location}\n${c.website}\n${String(at)}`;
  }

  /** A time in milliseconds as given, or 0 for none (absent, not a number, negative). */
  const ms = (v) => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? Math.floor(v) : 0);
  const plusOne = (v) => (v >= Number.MAX_SAFE_INTEGER ? Number.MAX_SAFE_INTEGER : v + 1);

  /**
   * The time to put on the next copy signed for a server: after the clock
   * (`nowMs`), after the copy that server holds (`signedAt`, from the own
   * copy; 0 for none) and after the last one this page signed for it
   * (`lastHere`; 0 for none). So a device whose clock runs fast no longer wins
   * every later edit.
   */
  function nextCopyTime(nowMs, signedAt, lastHere) {
    return Math.max(ms(nowMs), plusOne(ms(signedAt)), plusOne(ms(lastHere)));
  }

  // ── The limits (the relay's `within_limits`) ──

  /** Characters as the relay counts them: Unicode code points. */
  function chars(s) {
    return Array.from(s).length;
  }

  /** The length of `s` in UTF-8 bytes (a lone surrogate counts as the 3 bytes of its replacement). */
  function utf8Bytes(s) {
    let n = 0;
    for (const ch of s) {
      const cp = ch.codePointAt(0);
      n += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    }
    return n;
  }

  const isLowerHex = (s) => /^[0-9a-f]*$/.test(s);

  /** Is `key` a key as copies carry it: 3,904 lowercase hex characters. */
  function isKeyText(key) {
    return typeof key === 'string' && key.length === KEY_HEX_LEN && isLowerHex(key);
  }

  /** Is `sig` a copy's signature as it travels: 6,618 lowercase hex characters. */
  function isSignatureText(sig) {
    return typeof sig === 'string' && sig.length === SIGNATURE_HEX_LEN && isLowerHex(sig);
  }

  /** Is the text a JSON object. */
  function isJsonObject(s) {
    try {
      const v = JSON.parse(s);
      return !!v && typeof v === 'object' && !Array.isArray(v);
    } catch (e) {
      return false;
    }
  }

  /** The field half of `withinLimits`: the save's own limits, so a copy a save would refuse is never signed. */
  function fieldsWithinLimits(c) {
    if (!c || typeof c !== 'object') return false;
    const keys = ['name', 'bio', 'avatar_url', 'banner_url', 'socials', 'pronouns', 'location', 'website'];
    if (!keys.every((k) => typeof c[k] === 'string')) return false;
    if (isWithdrawal(c)) {
      const w = withdrawal();
      return keys.every((k) => c[k] === w[k]);
    }
    const nameOk = utf8Bytes(c.name) <= NAME_MAX_BYTES && /^[A-Za-z0-9_-]*$/.test(c.name);
    const bioOk = chars(c.bio) <= BIO_MAX_CHARS && !/[<>]/.test(c.bio);
    const linksOk = utf8Bytes(c.socials) <= LINKS_MAX_BYTES && isJsonObject(c.socials);
    const linkOk = (s, max) => utf8Bytes(s) <= max && (s === '' || s.startsWith('https://'));
    const picturesOk = linkOk(c.avatar_url, PICTURE_LINK_MAX_BYTES)
      && linkOk(c.banner_url, PICTURE_LINK_MAX_BYTES)
      && linkOk(c.website, WEBSITE_MAX_BYTES);
    const wordsOk = chars(c.pronouns) <= PRONOUNS_MAX_CHARS && chars(c.location) <= LOCATION_MAX_CHARS;
    const oneLine = [c.name, c.avatar_url, c.banner_url, c.socials, c.pronouns, c.location, c.website]
      .every((s) => !s.includes('\n'));
    return nameOk && bioOk && linksOk && picturesOk && wordsOk && oneLine;
  }

  /** Does a copy of `publicKey`'s profile keep the save's own limits (the relay's `within_limits`). */
  function withinLimits(publicKey, c) {
    return isKeyText(publicKey) && fieldsWithinLimits(c);
  }

  const api = {
    FIELDS_FOR_FRIENDS, COPY_TAG, BIO_MAX_CHARS, NAME_MAX_BYTES, LINKS_MAX_BYTES, PICTURE_LINK_MAX_BYTES,
    WEBSITE_MAX_BYTES, PRONOUNS_MAX_CHARS, LOCATION_MAX_CHARS, KEY_HEX_LEN, SIGNATURE_HEX_LEN, MAX_AHEAD_MS,
    unlisted, keptForFriends, withdrawal, isWithdrawal, publicCopy, copyText, nextCopyTime,
    chars, utf8Bytes, isKeyText, isSignatureText, fieldsWithinLimits, withinLimits,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  if (root) root.HosProfileCopy = api;
})(typeof window !== 'undefined' ? window : null);
