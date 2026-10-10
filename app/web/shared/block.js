// ── block.js ──────────────────────────────────────────────────────────────
// Block (step C, 2026-10-09, docs/design/blocking-and-safe-mode.md 10d): the
// words and the note format, shared by the web chat client (web/chat/
// chat-privacy.js keeps the list and acts on it) and by Node, where
// scripts/tests/block-web.test.js holds them to the spec and to the desktop
// app's constants.
//
// A block is kept on your own devices, never on the server. Each device of
// the same identity learns of a block or an unblock made on another through a
// sealed note to yourself only: an ordinary signed, sealed v2 DM whose inner
// payload is from you to you and whose text is
//   [[hum:block:v1]]<their identity key>     or
//   [[hum:unblock:v1]]<their identity key>
// deposited in your own mailbox (the same self-copy path follows use). The
// blocked person is never sent anything. A note addressed to anyone but
// yourself is ignored. The two markers must match the desktop app's
// (src/net/dm_pq.rs, beside CTL_FOLLOW) exactly.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before chat-privacy.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The note markers. Must match native src/net/dm_pq.rs.
  const CTL_BLOCK = '[[hum:block:v1]]';
  const CTL_UNBLOCK = '[[hum:unblock:v1]]';

  // An identity key as the relay and both clients write it: lowercase hex.
  // (A Dilithium3 public key is 3904 hex characters; the floor keeps out
  // anything that is plainly not a key, the ceiling anything absurd.)
  const BLOCK_KEY_RE = /^[0-9a-f]{16,8192}$/;

  // The one line a block shows (10d), and its unblock twin.
  const BLOCKED_LINE = 'Blocked. You will not see anything from them. They are not told.';
  const UNBLOCKED_LINE = 'Unblocked. They can reach you again as your safety settings allow. To be friends again, follow them or send a contact request.';

  /** The key in the form the list keeps (lowercase hex), or null when it is not one. */
  function blockKeyNorm(key) {
    if (typeof key !== 'string') return null;
    const k = key.trim().toLowerCase();
    return BLOCK_KEY_RE.test(k) ? k : null;
  }

  /** The text of a note: action 'block' or 'unblock' and the key; null when either is not one. */
  function blockNoteText(action, key) {
    const k = blockKeyNorm(key);
    if (!k) return null;
    if (action === 'block') return CTL_BLOCK + k;
    if (action === 'unblock') return CTL_UNBLOCK + k;
    return null;
  }

  /** Does this DM text start with either note marker (whatever follows)? Such text is never shown. */
  function isBlockNoteText(text) {
    return typeof text === 'string' && (text.startsWith(CTL_BLOCK) || text.startsWith(CTL_UNBLOCK));
  }

  /** Read a note's text: {action, key}, or null when it is not exactly a marker and a key. */
  function blockNoteParse(text) {
    if (typeof text !== 'string') return null;
    let action = null;
    let rest = null;
    if (text.startsWith(CTL_BLOCK)) { action = 'block'; rest = text.slice(CTL_BLOCK.length); }
    else if (text.startsWith(CTL_UNBLOCK)) { action = 'unblock'; rest = text.slice(CTL_UNBLOCK.length); }
    if (!action || !BLOCK_KEY_RE.test(rest)) return null;
    return { action, key: rest };
  }

  /**
   * The note an opened, signature-checked DM carries, when it is one I sent to
   * myself: from me, to me, naming someone other than me. Anything else (a
   * note from someone else, or my own message to someone else that happens to
   * start with a marker) is null, and so ignored.
   */
  function blockNoteFromSelf(inner, me) {
    const mine = blockKeyNorm(me);
    if (!mine || !inner) return null;
    if (blockKeyNorm(inner.from) !== mine || blockKeyNorm(inner.to) !== mine) return null;
    const note = blockNoteParse(inner.text);
    if (!note || note.key === mine) return null;
    return note;
  }

  const api = {
    CTL_BLOCK, CTL_UNBLOCK, BLOCK_KEY_RE, BLOCKED_LINE, UNBLOCKED_LINE,
    blockKeyNorm, blockNoteText, isBlockNoteText, blockNoteParse, blockNoteFromSelf,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
