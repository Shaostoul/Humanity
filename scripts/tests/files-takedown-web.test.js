// "Report a copyright problem" and the public takedown log on the website (2026-10-13):
// web/pages/files-takedown.js, its button on an item's page (web/pages/files-item.js) and at the
// Media page's foot (web/pages/files-app.js), the admins' view on the Admin tab, and the log on
// web/pages/rules.html. The relay half is src/relay/files_takedown.rs.
//
// Run: node --test scripts/tests/files-takedown-web.test.js
//
// Against the stand-ins in scripts/tests/files-web-harness.js (a document that holds only what the
// page built, where reading or setting innerHTML throws; a network that records every request;
// signing that returns "sig:" plus the words signed). HOS_WEB_DIR points the test at another copy
// of web/ (used to see each test red).
//
// What it proves:
//  1. The form posts exactly {item, name, contact, statement, good_faith: true} to
//     /api/files/takedown, unsigned and with no token, from an item's page with nobody signed in,
//     and says what the server answered; the page's foot opens it asking for the item, and an
//     address pasted there is read as its number.
//  2. The box must be ticked: Send stays off, and nothing is sent, until it is; a refusal from the
//     server is shown in its own words.
//  3. The log shows the item, the dates, the outcome and the note, and nothing personal, even if a
//     server sent more.
//  4. The admins' view lists the open reports (signed media_takedowns) and decides one (signed
//     media_takedown:<id>:<removed|kept>, with the note), on the Media page's Admin tab.
//  5. The words beside the box and the limits are the relay's own.
//
// Seen red 2026-10-13: each with one break made in a copy of web/ under HOS_WEB_DIR, then green
// again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { ROOT, WEB, ANN, byId, text, everyElement, respond, at, path0, fakeDocument, fakeFetch, fakeSigner, fakeSession, settle, item, listRoutes, listAnswer, openPage } = H;
const TD = require(path.join(WEB, "pages", "files-takedown.js"));

const STATEMENT = "I painted this in 2019; it is on my own site with the date.";

/** The form in a document of its own: {doc, box, form, net}. `routes` answer its requests. */
function formAlone(routes) {
  const doc = fakeDocument();
  const box = doc.createElement("div");
  box.id = "takedown-box";
  doc.body.appendChild(box);
  const net = fakeFetch(routes || [[at("/api/files/takedown", "POST"), () => respond(200, { ok: true, id: 4, words: "Received. An admin will look at it." })]]);
  const form = TD.createTakedownForm({ document: doc, fetch: net.fetch, container: box });
  return { doc, box, form, net };
}

/** Type `value` into the element with id `id`, as a person does. */
function type(doc, id, value) {
  const el = byId(doc, id);
  assert.ok(el, "the form has #" + id);
  el.value = value;
  el.dispatch("input");
}

function tick(doc, on) {
  const box = byId(doc, "files-takedown-good-faith");
  box.checked = on;
  box.dispatch("change");
}

const takedowns = (net) => net.calls.filter((c) => path0(c.url) === "/api/files/takedown");

// 1. Seen red 2026-10-13 with `takedownBody` leaving out good_faith: "Expected values to be
//    strictly deep-equal: + actual - expected ... - good_faith: true".
test("the form posts the right body, signed out, from an item's page and from the foot", async () => {
  const dusk = item({ id: 12, title: "Dusk", uploader_key: ANN });
  const routes = listRoutes(listAnswer([dusk]), [
    [at("/api/files/takedown", "POST"), () => respond(200, { ok: true, id: 4, words: "Received. An admin will look at it. The outcome will appear in the public takedown log, without your name or contact." })],
  ]);
  // Nobody signed in: no identity, no session.
  const p = await openPage({ routes, session: null, myKey: "" });
  p.page.openViewer(dusk);
  await settle();
  const button = byId(p.doc, "files-item-takedown");
  assert.ok(button, "the item's page has Report a copyright problem, signed out");
  assert.strictEqual(text(button), "Report a copyright problem");
  button.click();
  assert.match(text(byId(p.doc, "files-takedown-item")), /Item 12, "Dusk"/);
  assert.match(text(byId(p.doc, "files-takedown-next")), /public takedown log, without your name or contact/, "it says what happens next");
  type(p.doc, "files-takedown-name", "  Rosa Painter ");
  type(p.doc, "files-takedown-contact", "rosa@example.org");
  type(p.doc, "files-takedown-statement", STATEMENT);
  tick(p.doc, true);
  byId(p.doc, "files-takedown-send").click();
  await settle();

  const sent = takedowns(p.net);
  assert.strictEqual(sent.length, 1, "one report sent");
  assert.strictEqual(sent[0].init.method, "POST");
  assert.deepStrictEqual(sent[0].body, { item: 12, name: "Rosa Painter", contact: "rosa@example.org", statement: STATEMENT, good_faith: true });
  assert.strictEqual(sent[0].headers["X-Upload-Token"], undefined, "no token rides with it");
  assert.deepStrictEqual(p.signer.signed, [], "nothing was signed");
  assert.match(text(byId(p.doc, "files-takedown-done")), /^Received\. An admin will look at it\./, "the server's own words");

  // The page's foot: the same form, asking for the item; a pasted address is read as its number.
  byId(p.doc, "files-takedown-ok").click();
  assert.strictEqual(byId(p.doc, "files-takedown-open"), null, "closed");
  const foot = byId(p.doc, "files-foot-takedown");
  assert.ok(foot, "the Files page's foot has the button, signed out");
  foot.click();
  type(p.doc, "files-takedown-item-input", "https://united-humanity.us/files/7?x=1");
  type(p.doc, "files-takedown-statement", STATEMENT);
  tick(p.doc, true);
  byId(p.doc, "files-takedown-send").click();
  await settle();
  assert.strictEqual(takedowns(p.net)[1].body.item, 7);
  assert.strictEqual(takedowns(p.net)[1].body.name, "", "the name is optional");
  assert.ok(everyElement(p.doc).some((e) => e.tagName === "A" && e.href === "/rules#takedown-log"), "the public log is linked");

  // What a person may type for the item.
  for (const [typed, n] of [["12", 12], ["#12", 12], ["/files/12", 12], ["united-humanity.us/files/12#top", 12], ["twelve", null], ["0", null], ["/files/12/extra/13x", null]]) {
    assert.strictEqual(TD.itemNumberFrom(typed), n, typed);
  }
});

// 2. Seen red 2026-10-13 twice: with Send never turned off, "Send is off until the box is ticked /
//    false !== true"; and with only `formProblem`'s box check taken out, "the box is not ticked, so
//    nothing is sent / 1 !== 0".
test("the box must be ticked", async () => {
  const refused = "3 copyright reports have come from your connection today, the most one connection may send in a day, so this one was not sent. Try again after midnight UTC.";
  let refuse = false;
  const f = formAlone([[at("/api/files/takedown", "POST"), () => (refuse ? respond(429, refused) : respond(200, { ok: true, id: 1, words: "Received." }))]]);
  f.form.open({ id: 3, title: "Dawn" });
  type(f.doc, "files-takedown-statement", STATEMENT);
  const send = () => byId(f.doc, "files-takedown-send");
  assert.strictEqual(send().disabled, true, "Send is off until the box is ticked");
  // Even a Send that a browser let through sends nothing: the form asks for the box.
  send().disabled = false;
  send().click();
  await settle();
  assert.strictEqual(takedowns(f.net).length, 0, "the box is not ticked, so nothing is sent");
  assert.strictEqual(text(byId(f.doc, "files-takedown-error")), TD.WORDS.needBox);
  tick(f.doc, true);
  assert.strictEqual(send().disabled, false, "ticked, Send is on");
  tick(f.doc, false);
  assert.strictEqual(send().disabled, true, "unticked again, off again");

  // No statement: asked for, nothing sent.
  tick(f.doc, true);
  type(f.doc, "files-takedown-statement", "   ");
  send().click();
  await settle();
  assert.strictEqual(takedowns(f.net).length, 0);
  assert.strictEqual(text(byId(f.doc, "files-takedown-error")), TD.WORDS.needStatement);

  // The server's refusal, in its own words, and the form stays for another try.
  refuse = true;
  type(f.doc, "files-takedown-statement", STATEMENT);
  tick(f.doc, true);
  send().click();
  await settle();
  assert.strictEqual(takedowns(f.net).length, 1);
  assert.strictEqual(text(byId(f.doc, "files-takedown-error")), refused);
  assert.strictEqual(byId(f.doc, "files-takedown-done"), null);
});

// 3. Seen red 2026-10-13 with the log's row showing `r.name` and `r.contact` beside the item: "the
//    log shows what a person wrote: Rosa Painter".
test("the log renders without personal fields", async () => {
  const answer = {
    takedowns: [
      // A server that sent more than it should: none of it may reach the page.
      { id: 9, item_id: 12, item_title: "Dusk", created_at: Date.UTC(2026, 9, 11), decided_at: Date.UTC(2026, 9, 13), outcome: "removed", note: "Not theirs to share.",
        name: "Rosa Painter", contact: "rosa@example.org", statement: STATEMENT, decided_by: "operator_key" },
      { id: 8, item_id: 5, item_title: "", created_at: Date.UTC(2026, 9, 10), decided_at: Date.UTC(2026, 9, 12), outcome: "kept", note: "" },
    ],
  };
  const doc = fakeDocument();
  const box = doc.createElement("div");
  box.id = "takedown-log";
  doc.body.appendChild(box);
  const net = fakeFetch([[at("/api/files/takedowns", "GET"), () => respond(200, answer)]]);
  await TD.loadTakedownLog(box, { document: doc, fetch: net.fetch });
  assert.deepStrictEqual(net.calls.map((c) => path0(c.url)), ["/api/files/takedowns"]);
  const all = text(box);
  for (const personal of ["Rosa Painter", "rosa@example.org", "I painted this", "operator_key"]) {
    assert.ok(!all.includes(personal), "the log shows what a person wrote: " + personal);
  }
  const rows = box.querySelectorAll(".takedown-log-row");
  assert.strictEqual(rows.length, 2);
  assert.match(text(rows[0]), /^Removed Item 12, "Dusk" · Reported 2026-10-11, decided 2026-10-13 · #9Not theirs to share\.$/);
  assert.match(text(rows[1]), /^Kept Item 5 · Reported 2026-10-10, decided 2026-10-12 · #8$/, "a log row with no title names the item by number");

  // Empty, and a server that cannot be reached.
  TD.renderTakedownLog(box, { takedowns: [] }, doc);
  assert.strictEqual(text(box), TD.WORDS.logEmpty);
  const down = fakeFetch([[at("/api/files/takedowns"), () => respond(503, "<html>nginx</html>")]]);
  await TD.loadTakedownLog(box, { document: doc, fetch: down.fetch });
  assert.strictEqual(text(box), TD.WORDS.logFailed + "The server answered 503.");

  // The rules page holds the log's place and loads this file, and no longer sends people to GitHub
  // to report a copyright problem.
  const rules = fs.readFileSync(path.join(WEB, "pages", "rules.html"), "utf8");
  assert.ok(rules.includes('id="takedown-log"') && rules.includes('src="/pages/files-takedown.js'), "rules.html shows the log");
  const section = rules.slice(rules.indexOf('id="takedowns"'), rules.indexOf("<h2>Appealing a decision</h2>"));
  assert.ok(section.includes("Report a copyright problem") && !section.includes("github.com"), "the takedowns section points to the form");
});

// 4. Seen red 2026-10-13 with Remove item signing `media_takedown:<id>:remove`: "Expected values to
//    be strictly deep-equal: + sig: 'sig:media_takedown:3:remove', - sig: 'sig:media_takedown:3:removed'".
test("the admins' view lists the open reports and decides them, on the Admin tab", async () => {
  const open = [
    { id: 3, item_id: 12, item_title: "Dusk", item_public: true, item_on_page: true, created_at: Date.UTC(2026, 9, 12), name: "Rosa", contact: "rosa@example.org", statement: STATEMENT, state: "open" },
    { id: 4, item_id: 13, item_title: "", item_public: false, item_on_page: false, created_at: Date.UTC(2026, 9, 12), name: "", contact: "", statement: "Mine.", state: "open" },
  ];
  const routes = [
    [at("/api/files/takedowns/open", "POST"), () => respond(200, { requests: open })],
    [at("/api/files/takedown/decide", "POST"), (c) => respond(200, { ok: true, id: c.body.id, outcome: c.body.state, item_removed: c.body.state === "removed", item_already_gone: false })],
  ];
  let view = null;
  const signer = fakeSigner("a1".repeat(32));
  const p = await openPage({
    routes: listRoutes(listAnswer([]), routes), sign: signer.sign,
    session: fakeSession({ role: "admin" }), hash: "#admin",
    mountAdmin: () => {},
    mountTakedowns: (c, ctx) => {
      view = TD.mountTakedownAdmin(c, { document: ctx.doc, fetch: ctx.net.fetch, sign: signer.sign, confirm: async () => true });
    },
  });
  await view.ready;
  await settle();
  const panel = byId(p.doc, "files-admin-takedowns");
  assert.ok(panel && !byId(p.doc, "files-admin-wrap").hidden, "the Admin tab holds the copyright reports");
  const rows = panel.querySelectorAll(".files-takedown-row");
  assert.strictEqual(rows.length, 2);
  assert.match(text(rows[0]), /Report #3 about Item 12, "Dusk"/);
  assert.match(text(rows[0]), /Name: Rosa/);
  assert.match(text(rows[0]), /Contact: rosa@example\.org/);
  assert.match(text(rows[1]), /no longer on the Files page/);
  assert.ok(signer.signed.includes("media_takedowns"), "the open list is a signed read");

  const note = rows[0].querySelector("textarea");
  note.value = "Not theirs to share.";
  note.dispatch("input");
  rows[0].querySelector('[data-decide="removed"]').click();
  await settle();
  const decides = p.net.calls.filter((c) => path0(c.url) === "/api/files/takedown/decide");
  assert.strictEqual(decides.length, 1);
  assert.deepStrictEqual({ id: decides[0].body.id, state: decides[0].body.state, note: decides[0].body.note, sig: decides[0].body.sig },
    { id: 3, state: "removed", note: "Not theirs to share.", sig: "sig:media_takedown:3:removed" });
  panel.querySelectorAll(".files-takedown-row")[1].querySelector('[data-decide="kept"]').click();
  await settle();
  const second = p.net.calls.filter((c) => path0(c.url) === "/api/files/takedown/decide")[1];
  assert.deepStrictEqual([second.body.id, second.body.state, second.body.sig], [4, "kept", "sig:media_takedown:4:kept"]);
});

// 5. Seen red 2026-10-13 with the box's words changed to "...is not allowed by the law...": "the
//    box's words are the relay's GOOD_FAITH_LINE".
test("the words beside the box and the limits are the relay's own", () => {
  const rs = fs.readFileSync(path.join(ROOT, "src", "relay", "files_takedown.rs"), "utf8");
  const line = /pub const GOOD_FAITH_LINE: &str =\s*"([^"]+)";/.exec(rs);
  assert.ok(line, "media_takedown.rs has GOOD_FAITH_LINE");
  assert.strictEqual(TD.WORDS.goodFaith, line[1], "the box's words are the relay's GOOD_FAITH_LINE");
  for (const name of ["NAME_MAX", "CONTACT_MAX", "STATEMENT_MAX", "NOTE_MAX"]) {
    const m = new RegExp("pub const " + name + ": usize = (\\d+);").exec(rs);
    assert.ok(m, name);
    assert.strictEqual(TD[name], Number(m[1]), name);
  }
});
