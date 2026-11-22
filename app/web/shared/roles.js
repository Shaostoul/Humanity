// ── roles.js ──────────────────────────────────────────────────────────────
// Who counts as an admin or a moderator on this server: the one place the
// website reads a person's role for a gate (BUG-332's sweep, 2026-11-20).
//
// The server's owner (role "owner") counts as an admin everywhere, and a
// moderator is spelled "mod" or "moderator" (the relay accepts both). Each
// page that shows or hides an admin's or a moderator's control asks these
// functions, never `role === 'admin'` on its own: that copy of the rule hid
// the owner's controls while the relay allowed them. The relay's twins are
// `is_admin_role` and `is_staff_role` (src/relay/handlers/msg_handlers.rs);
// the desktop app's are in src/net/roles.rs. The relay checks every action
// again; these only decide what a page offers.
//
// A role's own name (a badge's letter, a "requires_role" in data, the value a
// role is set to) is not a gate and stays a plain string.
//
// A classic script in the browser (`HosRoles` lands on window), a CommonJS
// module under Node (scripts/tests/roles-web.test.js). Load it before the
// scripts that use it: web/chat/index.html, web/pages/market.html and
// web/pages/files.html do.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** A role as the relay writes it: trimmed, lower case ('' for none). */
  function roleName(role) {
    return String(role == null ? '' : role).trim().toLowerCase();
  }

  /** An admin's or the owner's role. */
  function isAdminRole(role) {
    const r = roleName(role);
    return r === 'admin' || r === 'owner';
  }

  /** An admin's, the owner's or a moderator's role (both spellings). */
  function isStaffRole(role) {
    const r = roleName(role);
    return isAdminRole(r) || r === 'mod' || r === 'moderator';
  }

  /**
   * The id of the role definition (the server's role list) that `role` reads
   * as: the owner has no row of its own and reads as the Admin row, as the
   * relay's role_def does (src/relay/storage/roles.rs). Anyone else's is their
   * own role's.
   */
  function roleDefId(role) {
    const r = roleName(role);
    return r === 'owner' ? 'admin' : r;
  }

  const api = { roleName, isAdminRole, isStaffRole, roleDefId };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosRoles = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
