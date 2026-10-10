// ── admin-erase.js ────────────────────────────────────────────────────────
// An admin erases another person's data (2026-10-10,
// docs/design/blocking-and-safe-mode.md 10i): the rules and the words, shared
// by the web chat client (web/chat/chat-ui.js draws the action and its
// confirm, web/chat/app.js reads `account_erased`) and by Node, where
// scripts/tests/admin-erase-web.test.js holds them to the spec.
//
// Who may: an admin or the owner of this server. Never a moderator (an erase
// cannot be undone, and the admins answer for the server), never on their own
// row (the ordinary Erase account is for that), and never on an admin's or the
// owner's row (demote them first, the rule the self-erase has). The relay
// checks all of this again and refuses with a notice; these rules only decide
// what the menu offers.
//
// The frame, exactly 10i's:
//   {"type":"admin_erase","target":"<their public key>","confirm_name":"<their name, typed>"}
// sent only when the typed name, trimmed, is exactly their registered name.
// The relay answers the admin with
//   {"type":"admin_erase_done","name":"<the name typed>","receipt":[["<table>",<rows>],...],"partial":<bool>}
// and tells the erased person's own clients `account_erased` with
// `by_admin: true`.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before app.js and chat-ui.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The roles that may erase another person's data, and whose own rows are
  // never offered it.
  const ADMIN_ERASE_ROLES = ['admin', 'owner'];

  // The action's name in the member menu and the voice modal, and the
  // confirm's button.
  const ADMIN_ERASE_LABEL = 'Erase their data';
  const ADMIN_ERASE_BUTTON = 'Erase';
  const ADMIN_ERASE_TYPE_LABEL = 'Type their name exactly to confirm';

  // What the erased person's client says instead of the self-erase words (10i).
  const ERASED_BY_ADMIN_WORDS = 'A server admin erased your data from this server. Local data on your own devices is untouched.';
  // The login screen's note after it: the same words, then what pressing Enter
  // does there, which is what the self-erase note says too (BUG-135).
  const ERASED_BY_ADMIN_NOTE = ERASED_BY_ADMIN_WORDS + ' Pressing Enter signs you up again as a new account on this server.';

  function adminEraseRoleNorm(role) {
    return String(role == null ? '' : role).trim().toLowerCase();
  }

  /** Is this role an admin's or the owner's? */
  function adminEraseIsAdminRole(role) {
    return ADMIN_ERASE_ROLES.includes(adminEraseRoleNorm(role));
  }

  /**
   * Whether to offer "Erase their data" on someone's row:
   * { myRole, myKey, targetKey, targetRole }. Only to an admin or the owner,
   * never on their own row, a bot's, an admin's or the owner's.
   */
  function adminEraseOffered(opts) {
    const o = opts || {};
    if (!adminEraseIsAdminRole(o.myRole)) return false;
    const target = typeof o.targetKey === 'string' ? o.targetKey.trim() : '';
    if (!target || target.startsWith('bot_')) return false;
    const mine = typeof o.myKey === 'string' ? o.myKey.trim() : '';
    if (mine && target.toLowerCase() === mine.toLowerCase()) return false;
    if (adminEraseIsAdminRole(o.targetRole)) return false;
    return true;
  }

  /**
   * The confirm's sentence for this person, 10i's words exactly. Corrected
   * 2026-10-10 after the batch review: the erase keeps the reports, bans and
   * mutes about them (as the self-erase does), so it does not delete
   * "everything" without saying so.
   */
  function adminEraseConfirmText(name) {
    return 'This deletes everything this server stores about ' + String(name == null ? '' : name)
      + ': their messages, profile, uploads, membership and settings. Reports, bans and mutes about'
      + ' them are kept, as when someone erases their own account. It cannot be undone. It does'
      + ' not touch anything on their own devices, and it does not stop them joining again (ban them'
      + ' too for that).';
  }

  /** Does the typed name, trimmed, match their registered name exactly (letter case too)? */
  function adminEraseNameMatches(typed, name) {
    if (typeof typed !== 'string' || typeof name !== 'string' || name === '') return false;
    return typed.trim() === name;
  }

  /**
   * The frame to send, or null when the typed name does not match or there is
   * no key: { type: 'admin_erase', target, confirm_name }, nothing else.
   */
  function adminEraseFrame(target, typed, name) {
    if (typeof target !== 'string' || target.trim() === '') return null;
    if (!adminEraseNameMatches(typed, name)) return null;
    return { type: 'admin_erase', target: target.trim(), confirm_name: typed.trim() };
  }

  /** The receipt's rows that count: [label, n] with n above zero. */
  function adminEraseReceiptRows(receipt) {
    if (!Array.isArray(receipt)) return [];
    return receipt.filter((r) => Array.isArray(r) && typeof r[0] === 'string' && Number(r[1]) > 0)
      .map((r) => [r[0], Number(r[1])]);
  }

  /** Did part of the erase fail? `partial`, or a `<table>_FAILED` row. */
  function adminErasePartial(msg) {
    const m = msg || {};
    return m.partial === true || adminEraseReceiptRows(m.receipt).some(([label]) => label.endsWith('_FAILED'));
  }

  /**
   * What the admin is shown for `admin_erase_done`: whose data, the per-table
   * counts (as the self-erase reports them), and, when part of it failed, that
   * it did not finish and to erase again.
   */
  function adminEraseReceiptText(msg) {
    const m = msg || {};
    const name = typeof m.name === 'string' && m.name.trim() ? m.name.trim() : 'this person';
    const rows = adminEraseReceiptRows(m.receipt);
    const summary = rows.length ? rows.map(([label, n]) => label + ': ' + n).join(', ') : 'nothing was stored';
    if (adminErasePartial(m)) {
      return 'The erase of ' + name + "'s data did not finish: part of it failed (" + summary + '). '
        + 'Use ' + ADMIN_ERASE_LABEL + ' again to finish it.';
    }
    return 'Erased ' + name + "'s data from this server (" + summary + '). '
      + 'Anything on their own devices is untouched.';
  }

  /**
   * Which login note an `account_erased` frame leaves: 'unfinished' when part
   * of the erase failed (then Enter does not sign up afresh, so that note wins),
   * 'admin' when a server admin erased the account, else 'erased'.
   */
  function erasedKindOf(msg) {
    const m = msg || {};
    if (m.partial === true) return 'unfinished';
    if (m.by_admin === true) return 'admin';
    return 'erased';
  }

  const api = {
    ADMIN_ERASE_ROLES, ADMIN_ERASE_LABEL, ADMIN_ERASE_BUTTON, ADMIN_ERASE_TYPE_LABEL,
    ERASED_BY_ADMIN_WORDS, ERASED_BY_ADMIN_NOTE,
    adminEraseIsAdminRole, adminEraseOffered, adminEraseConfirmText, adminEraseNameMatches,
    adminEraseFrame, adminEraseReceiptRows, adminErasePartial, adminEraseReceiptText, erasedKindOf,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
