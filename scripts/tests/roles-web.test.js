// Who counts as an admin or a moderator on the website (BUG-332's sweep, 2026-11-20), run under
// Node.
//
//   node --test scripts/tests/roles-web.test.js       (in `just rig-tests`)
//
// What it proves:
//  1. web/shared/roles.js, the one helper: the owner counts as an admin and as staff, a moderator
//     (either spelling) as staff and not an admin, anyone else as neither; the owner's role reads
//     as the Admin row of the role list (roleDefId), as the relay's role_def does.
//  2. No page script compares a person's role to "admin", "mod", "moderator" or "owner" on its
//     own: each gate asks the helper (the copies of the rule hid the owner's controls while the
//     relay allowed them). A role's own NAME is not a gate: the palette's `requires_role` is data.
//  3. Every script that asks it names the helper, and every page that runs one loads
//     /shared/roles.js before it.
//
// Seen red 2026-11-20 before the sweep: test 1 failed loading web/shared/roles.js (it did not
// exist); test 2 listed the gates that read a role alone: chat/app.js (the row's pin and delete
// buttons), chat/chat-messages.js (the pins panel's unpin), chat/chat-reports.js (the Reports
// page), chat/chat-ui.js (the line limit, the member menu, the channel cog and Create Channel, Add
// Server, the command palette), chat/chat-voice-modal.js, chat/chat-game-admin.js,
// chat/chat-held.js, pages/market-app.js (listing for sale, deleting another's review and
// listing), pages/files-app.js; test 3 found no page loading /shared/roles.js.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");

/** The page scripts with a role gate, each with the name it reaches the helper by. */
const GATED = [
  ["chat/app.js", "HosRoles."],
  ["chat/chat-ui.js", "HosRoles."],
  ["chat/chat-messages.js", "HosRoles."],
  ["chat/chat-reports.js", "HosRoles."],
  ["chat/chat-voice-modal.js", "HosRoles."],
  ["chat/chat-game-admin.js", "HosRoles."],
  ["chat/chat-held.js", "HosRoles."],
  ["pages/market-app.js", "HosRoles."],
  ["pages/files-app.js", "ROLES."],
  ["pages/shared-files-upload.js", "HosRoles."],
  ["shared/relay-session.js", "ROLES."],
];

/** Which page loads which of them. */
const PAGES = [
  ["chat/index.html", ["chat/app.js", "chat/chat-ui.js", "chat/chat-messages.js", "chat/chat-reports.js", "chat/chat-voice-modal.js", "chat/chat-game-admin.js", "chat/chat-held.js"]],
  ["pages/market.html", ["pages/market-app.js"]],
  ["pages/files.html", ["shared/relay-session.js", "pages/shared-files-upload.js", "pages/files-app.js"]],
  ["pages/orgs.html", ["shared/relay-session.js"]],
  ["pages/settings.html", ["shared/relay-session.js"]],
];

const read = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8").replace(/\r\n/g, "\n");

/** `src` without its comments (line and block comments, its lines kept where they were). */
function code(src) {
  return src
    .replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, ""))
    .split("\n")
    .map((l) => l.replace(/(^|[^:'"`\\])\/\/.*$/, "$1"))
    .join("\n");
}

test("1. the owner is an admin and staff, a moderator staff, anyone else neither", () => {
  const R = require(path.join(WEB, "shared", "roles.js"));
  for (const r of ["admin", "owner", " Owner ", "ADMIN"]) {
    assert.ok(R.isAdminRole(r), `an admin: ${JSON.stringify(r)}`);
    assert.ok(R.isStaffRole(r), `staff: ${JSON.stringify(r)}`);
  }
  for (const r of ["mod", "moderator"]) {
    assert.ok(!R.isAdminRole(r), `not an admin: ${r}`);
    assert.ok(R.isStaffRole(r), `staff: ${r}`);
  }
  for (const r of ["", null, undefined, "member", "verified", "donor", "muted", "shipwright", "administrator", "owners"]) {
    assert.ok(!R.isAdminRole(r) && !R.isStaffRole(r), `neither: ${JSON.stringify(r)}`);
  }
  assert.strictEqual(R.roleDefId("owner"), "admin", "the owner reads as the Admin row");
  assert.strictEqual(R.roleDefId("verified"), "verified");
  assert.strictEqual(R.roleDefId(""), "");
});

test("2. no page script reads a person's role against a role name on its own", () => {
  // `<expression> === 'admin'` (or !==, ==, the other names, either side): a gate when the
  // expression is a role, a name when it is something else (a tab, an erase kind, data).
  const cmp = /([\w$.\[\]'"]+)\s*[!=]==?\s*(['"])(admin|mod|moderator|owner)\2|(['"])(admin|mod|moderator|owner)\4\s*[!=]==?\s*([\w$.]+)/g;
  const found = new Set();
  for (const [rel] of GATED) {
    const lines = code(read(rel)).split("\n");
    lines.forEach((line, i) => {
      for (const m of line.matchAll(cmp)) {
        const lhs = m[1] || m[6] || "";
        const isRole = /role/i.test(lhs) || lhs === "r";
        const isData = /requires_role/.test(lhs);
        if (isRole && !isData) found.add(`${rel}:${i + 1}: ${line.trim()}`);
      }
    });
  }
  assert.deepStrictEqual([...found], [], "these gates read a role on their own instead of asking web/shared/roles.js");
});

test("3. every gated script names the helper, and every page loads it first", () => {
  for (const [rel, via] of GATED) {
    assert.ok(code(read(rel)).includes(via), `${rel} asks the helper (${via})`);
  }
  for (const [page, scripts] of PAGES) {
    const html = read(page);
    const at = (rel) => html.search(new RegExp(`<script[^>]*src="/${rel.replace(/[.]/g, "[.]")}[?"]`));
    const roles = at("shared/roles.js");
    assert.ok(roles >= 0, `${page} loads /shared/roles.js`);
    for (const s of scripts) {
      const where = at(s);
      assert.ok(where >= 0, `${page} loads /${s}`);
      assert.ok(roles < where, `${page} loads /shared/roles.js before /${s}`);
    }
  }
});
