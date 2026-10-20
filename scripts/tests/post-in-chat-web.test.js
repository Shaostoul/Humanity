// "Also post in chat" and the upload window on the website (2026-10-17): the web half of what the
// desktop app does since the operator redesigned a drop that day (src/gui/drop_files.rs,
// src/gui/pages/files/add.rs: draw_post_row, post_choices, post_text, post_published, rgb_border),
// in its words. The operator: "user drag and drops file onto HumanityOS. First window popups with
// all the controls of what to do with the file. Upload button with the RGB border effect. If we're
// also in a chat channel ... upon clicking upload it also posts the image with the title and
// description to the general chat. However we should add an ability to choose the channel that
// gets posted to. Maybe like current first and then favorites." And: "Maybe have a checkbox to the
// left of the channel info for toggling the channel posting on/off. Gray out the channel controls
// when they're disabled."
//
// The chat page's half (a file dropped on a channel opens this window, posting ticked for it; a
// file dropped in a DM never does) is in scripts/tests/private-files-web.test.js, its "drop:" tests,
// which have the chat's harness; the Media page's overlay words are in drop-files-web.test.js.
//
// Run: node --test scripts/tests/post-in-chat-web.test.js   (in `just rig-tests`)
//
// What runs: the Media page (web/pages/files-app.js, with Add to Media in web/pages/files-add.js) on the stand-ins of
// scripts/tests/files-web-harness.js (a document holding only what the page built, a network that
// records every request, a session whose frames a test sends, timers that never run by themselves),
// the upload window's two halves (web/shared/upload-window.js) between stand-in windows, and the
// recovery-phrase guard (web/shared/pq-relay-auth.js pqRecoveryPhraseIn) in a node:vm context with
// the real word list (web/chat/bip39-english.js) and matcher (web/shared/warnings.js).
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  A. Add to Media ends with "Also post in chat", in its green "Who can see it" part: the tick on
//     the left, then the channel and Add to favorites, both greyed (disabled) while the tick is
//     off; the channel open in the chat first ("#general (open in the chat)"), then favourites
//     ("⭐ #printing"), then the rest; a read-only channel only for an admin; favourites kept in
//     this browser, by server and channel; with no channel, one line saying so; a form for a file
//     already on the server has none.
//  B. Once the item is published, its own address on this server's Media page ({origin}/files/{id},
//     which the chat shows as the item's card, web/chat/chat-files-card.js) is posted in the
//     channel ticked, as a chat message signed over "content\ntimestamp" the way the chat signs one,
//     and the line says "Posted in #general." once the server's echo came back. A members-only item
//     posts the same: its page (since 2026-10-17, the desktop's add.rs post_text).
//  C. Where it is not posted, the line says why and nothing is sent: the connection gone, the
//     server's refusal (its words), no answer, the recovery-phrase guard (its own sentence), the
//     guard unable to run, the tick off; and never into a direct message, even when one is asked for.
//  D. The Upload button wears the nav bar's RGB border (its `channeling` animation, still under
//     reduced motion) when it would upload a file picked here; its words are the desktop's.
//  E. The upload window (/files?window=upload): Add to Media and its uploads alone; files handed
//     over wait until the page knows the person may upload, then open the same form as the Media
//     page's own, posting ticked; it tells the page it is over what it says, whether something is
//     going, and when it is cancelled; files dropped on the window itself post in the same channel.
//  F. The two halves of /shared/upload-window.js: files wait until the frame says it is there;
//     only the frame's own messages, from this origin, count; Close hides a window with something
//     going (its lines go to the chat) and removes it once nothing is.
//  G. The guard: pqRecoveryPhraseIn finds four words of the recovery phrase in a row (BIP39's own
//     test phrases), and says it cannot check without the word list or a seed.
//
// Red first: each test names the break it was seen failing on.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./files-web-harness.js");
const { ROOT, WEB, ME, ANN, MIB, ORIGIN, ROLES, byId, text, respond, at, fakeSession, fakeStorage, fakeDocument, settle, listAnswer, listRoutes, openPage, resumableRelay, aFile } = H;
const M = H.media;
const PW = M.POST_WORDS;
const UW = require(path.join(WEB, "shared", "upload-window.js"));
const WARN = require(path.join(WEB, "shared", "warnings.js"));
// The server's licences (the relay's answer, as round 5's shared fixture has it).
const LICENCES = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "files-round5-v1.json"), "utf8")).licences;

// This server's channels, as its channel_list names them (src/relay/relay.rs ChannelInfo).
const CHANNELS = [
  { id: "general", name: "general", read_only: false },
  { id: "announcements", name: "announcements", read_only: true },
  { id: "printing", name: "printing", read_only: false },
  { id: "random", name: "random", read_only: false },
];
/** The server lists its channels on the page's connection, as it does when the page signs in. */
const listChannels = (session, channels) => session.emit({ type: "frame", msg: { type: "channel_list", channels: channels || CHANNELS } });
/** localStorage holding the chat's open channel (web/chat/app.js keeps it as humanity_channel). */
function storageWith(open) {
  const s = fakeStorage();
  if (open) s.setItem("humanity_channel", open);
  return s;
}
async function pick(doc, files) {
  const input = byId(doc, "files-file-input");
  input.files = files;
  input.dispatch("change");
  await settle();
}
/** The post row and its parts, read off the page as a person sees them. */
function postRow(doc) {
  const box = byId(doc, "files-form-post");
  const row = box && box.querySelector(".files-post-row");
  if (!row) return null;
  const tick = byId(doc, "files-form-post-on");
  const select = byId(doc, "files-form-post-to");
  const star = byId(doc, "files-form-post-favorite");
  return {
    row, tick, select, star,
    order: row.children.map((c) => c.tagName),
    options: select.querySelectorAll("option").map(text),
    chosen: select.value,
    greyed: select.disabled === true && star.disabled === true && row.className.split(/\s+/).includes("files-post-off"),
  };
}
const tickOn = async (doc, on) => {
  const t = byId(doc, "files-form-post-on");
  t.checked = on;
  t.dispatch("change");
  await settle();
};
const notice = (doc) => text(byId(doc, "files-notice").querySelector(".files-notice-text"));
/** The chat messages the page sent on its connection. */
const chats = (session) => session.sent.filter((m) => m && m.type === "chat");

// ── A. The post row ──────────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/pages/files-add.js:
// with renderPostRow drawing the select first and the tick after it: "the tick on the left, then the
// channel, then Add to favorites" (actual [ 'SELECT', 'LABEL', 'BUTTON' ]); with the select's
// `disabled: !on` left out: "greyed while the tick is off" (actual false); with postChoices listing the
// favourites after the rest: "a favourite: after the open one, before the rest" (actual [ '#random
// (open in the chat)', '#general', '⭐ #printing' ]); with togglePostFavorite never writing to storage
// (where the favourites are read back from): the same assertion (actual [ '#random (open in the
// chat)', '#general', '#printing' ]).
test("A. Also post in chat: the tick on the left, the channel and Add to favorites greyed while it is off; the open channel first, then favourites, then the rest", async () => {
  const storage = storageWith("random");
  const p = await openPage({ storage, routes: listRoutes(listAnswer([])) });
  listChannels(p.session);
  await pick(p.doc, [aFile("dusk.mp4", 3 * MIB)]);
  let r = postRow(p.doc);
  assert.ok(r, "Add to Files has the post row");
  // In the green "Who can see it" part, at its foot.
  const green = byId(p.doc, "files-form").querySelectorAll(".tint-section").find((s) => s.className.split(/\s+/).includes("tint-green"));
  assert.ok(green && green.children[green.children.length - 1] === byId(p.doc, "files-form-post"), "at the foot of Who can see it");
  assert.deepEqual(r.order, ["LABEL", "SELECT", "BUTTON"], "the tick on the left, then the channel, then Add to favorites");
  assert.equal(r.row.children[0].children[0], r.tick, "the tick is in the label");
  assert.equal(text(r.row.children[0]).trim(), "Also post in chat", "in the desktop's words");
  assert.equal(r.row.children[0].title, "When it is up, a card for it is posted in this channel: its title, then its picture, description and every file when opened.", "its tooltip");
  assert.equal(r.tick.checked, false, "off unless asked for");
  assert.ok(r.greyed, "greyed while the tick is off");
  assert.deepEqual(r.options, ["#random (open in the chat)", "#general", "#printing"], "the open channel first, then the rest; a read-only channel is not offered");
  assert.equal(text(r.star), "Add to favorites");
  assert.equal(r.star.title, "Favorite channels are listed first, after the one open in the chat.");
  r.star.click();
  assert.equal(storage.getItem(M.POST_FAVORITES_KEY), null, "a greyed Add to favorites does nothing");

  await tickOn(p.doc, true);
  r = postRow(p.doc);
  assert.ok(r.tick.checked && !r.select.disabled && !r.star.disabled && !r.row.className.includes("files-post-off"), "ticked: the channel and the star can be used");
  r.select.value = "printing";
  r.select.dispatch("change");
  postRow(p.doc).star.click();
  await settle();
  r = postRow(p.doc);
  assert.deepEqual(r.options, ["#random (open in the chat)", "⭐ #printing", "#general"], "a favourite: after the open one, before the rest");
  assert.equal(r.chosen, "printing", "still the one chosen");
  assert.equal(text(r.star), "Remove from favorites");
  assert.deepEqual(JSON.parse(storage.getItem(M.POST_FAVORITES_KEY)), [ORIGIN + "\nprinting"], "kept by server and channel, in this browser");

  const again = await openPage({ storage, routes: listRoutes(listAnswer([])) });
  listChannels(again.session);
  await pick(again.doc, [aFile("dawn.mp4", MIB)]);
  assert.deepEqual(postRow(again.doc).options, ["#random (open in the chat)", "⭐ #printing", "#general"], "kept on this device: another page lists it the same");

  // A read-only channel only for an admin (the relay lets its staff post there).
  const admin = await openPage({ session: fakeSession({ role: "admin" }), storage: storageWith("general"), routes: listRoutes(listAnswer([])) });
  listChannels(admin.session);
  await pick(admin.doc, [aFile("dusk.mp4", MIB)]);
  assert.deepEqual(postRow(admin.doc).options, ["#general (open in the chat)", "#announcements", "#printing", "#random"], "an admin: the read-only channel too");

  // No channel: one line saying so.
  const none = await openPage({ routes: listRoutes(listAnswer([])) });
  listChannels(none.session, []);
  await pick(none.doc, [aFile("dusk.mp4", MIB)]);
  assert.equal(postRow(none.doc), null);
  assert.equal(text(byId(none.doc, "files-form-post-none")), "Also post in chat: this server has no channel to post in.");

  // A file already on the server (Put on Media) is not posted from here.
  const put = await openPage({ routes: listRoutes(listAnswer([])) });
  listChannels(put.session);
  put.page.openPutForm({ filename: "1700_part.stl", name: "bushing.stl" });
  assert.ok(byId(put.doc, "files-form-public-only"), "test setup: Put on the Files page is open");
  assert.equal(byId(put.doc, "files-form-post"), null, "no post row for a file already on the server");
});

// ── B and C. The post once published ─────────────────────────────────────

/**
 * The Media page with a file picked, its title and description typed, ready to upload: publishing
 * answers the item the relay would make of it (`visibility` as sent, its id `7`). `opts.publish(c)`
 * runs as the publish is answered (to drop the connection then, say).
 */
async function readyToUpload(opts) {
  const o = opts || {};
  const relay = resumableRelay();
  const published = [];
  const p = await openPage(Object.assign({
    storage: storageWith("general"),
    phraseIn: o.phraseIn === undefined ? async () => false : o.phraseIn,
    routes: listRoutes(listAnswer([]), relay.routes.concat([
      [at("/api/files", "POST"), (c) => {
        published.push(c.body);
        if (o.publish) o.publish(c);
        return respond(200, { ok: true, item: H.item({ id: 7, url: "/uploads/" + c.body.filename, title: c.body.title, description: c.body.description, visibility: c.body.visibility, uploader_key: ME }) });
      }],
    ])),
  }, o.page || {}));
  listChannels(p.session);
  await pick(p.doc, [aFile("dusk.mp4", 3 * MIB)]);
  byId(p.doc, "files-form-title").value = o.title === undefined ? "Dusk" : o.title;
  byId(p.doc, "files-form-description").value = o.description === undefined ? "At the beach, after the rain." : o.description;
  if (o.members) byId(p.doc, "files-form-vis-members").checked = true;
  return Object.assign(p, { relay, published });
}
async function upload(p) {
  byId(p.doc, "files-form-go").click();
  for (let i = 0; i < 20 && !p.published.length; i++) await settle();
  await settle();
}
/** The server's echo of a chat message the page sent: it stored it. */
const echo = (p, m) => p.session.emit({ type: "frame", msg: { type: "chat", from: ME, from_name: "Me", content: m.content, timestamp: m.timestamp, channel: m.channel, message_id: 41 } });
const runTimer = (p, ms) => { for (const t of p.timers.filter((x) => x.ms === ms)) t.fn(); };

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/pages/files-add.js:
// with postText back to the title, the description and the file, each on its own line: "its own
// address on the Media page, nothing else" (actual 'Dusk\nAt the beach, after the rain.\n
// /uploads/1760_dusk.mp4'); with postText giving a public item its file's address: the same assertion
// (actual '/uploads/1760_dusk.mp4'). Seen red earlier the same day (before the post became the item's
// address): with the signature made over the content alone: "signed over content\ntimestamp, as the
// chat signs"; with postPublished answering "Posted" as soon as it was sent (no echo awaited): "not
// before the server has it".
test("B. once published, its own address on the Files page is posted in the channel ticked, signed as the chat signs, and the line says so once the server has it", async () => {
  const p = await readyToUpload();
  await tickOn(p.doc, true);
  assert.equal(postRow(p.doc).chosen, "general", "test setup: #general, the channel open in the chat");
  await upload(p);
  assert.equal(p.published.length, 1, "published");
  const sent = chats(p.session);
  assert.equal(sent.length, 1, "one chat message");
  const m = sent[0];
  assert.equal(m.channel, "general", "in the channel ticked");
  // The item's own page on this server, which both apps draw as its card: never its title,
  // description or file's address (the desktop's add.rs post_text, 2026-10-17).
  assert.equal(m.content, ORIGIN + "/files/7", "its own address on the Files page, nothing else");
  assert.equal(m.content, M.postText({ title: "Dusk", description: "At the beach, after the rain.", url: "/uploads/1760_dusk.mp4", visibility: "public", id: 7 }, ORIGIN + "/"));
  assert.equal(m.from, ME);
  assert.ok(Number.isInteger(m.timestamp) && m.timestamp > 0);
  assert.equal(Buffer.from(m.pq_signature, "hex").toString("utf8"), "sig:" + m.content + "\n" + m.timestamp, "signed over content\\ntimestamp, as the chat signs");
  assert.equal(notice(p.doc), M.FILES_WORDS.published, "not before the server has it");
  echo(p, m);
  await settle();
  assert.equal(notice(p.doc), M.FILES_WORDS.published + " Posted in #general.", "Posted in #general.");

  // A members-only item: the same, its page, which only members can open, never its file.
  const mem = await readyToUpload({ members: true, title: "", description: "" });
  await tickOn(mem.doc, true);
  await upload(mem);
  assert.equal(mem.published[0].visibility, "members", "test setup: published as members only");
  const mm = chats(mem.session)[0];
  assert.equal(mm.content, ORIGIN + "/files/7", "a members-only item: its page, never its file");
  echo(mem, mm);
  await settle();
  assert.equal(notice(mem.doc), M.FILES_WORDS.published + " Posted in #general.");
});

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/pages/files-add.js:
// with postPublished's bound() check taken out: "the connection gone: nothing sent" (actual 1); with
// system lines never heard while waiting: "the server's refusal, in its words" (actual 'It is on the
// Media page now.'); with a held phrase (`held === true`) let through: "the guard's own sentence:
// nothing sent" (actual 1); with renderPostRow swapping an unknown channel for the first one and
// keeping the tick: "a direct message asked for: the tick goes off" (actual true).
test("C. where it is not posted the line says why and nothing goes: the connection, the server's refusal, no answer, the recovery phrase, the tick off, a direct message", async () => {
  const published = M.FILES_WORDS.published;

  // The connection gone by the time it is published.
  const gone = await readyToUpload({ publish: function () { gone.session.state.status = "closed"; gone.session.state.uploadToken = ""; } });
  await tickOn(gone.doc, true);
  await upload(gone);
  assert.equal(chats(gone.session).length, 0, "the connection gone: nothing sent");
  assert.equal(notice(gone.doc), published + " Not posted in #general: the chat is not connected to its server.");

  // The server refuses it: its words. A line about something else meanwhile does not count.
  const ro = await readyToUpload();
  await tickOn(ro.doc, true);
  await upload(ro);
  assert.equal(chats(ro.session).length, 1);
  ro.session.emit({ type: "frame", msg: { type: "system", message: "__task_comment__:{}" } });
  ro.session.emit({ type: "frame", msg: { type: "system", message: "This channel is read-only." } });
  await settle();
  assert.equal(notice(ro.doc), published, "a moment is given for its echo");
  runTimer(ro, M.POST_GRACE_MS);
  await settle();
  assert.equal(notice(ro.doc), published + " Not posted in #general: This channel is read-only.", "the server's refusal, in its words");
  // A line about something else, then its echo: posted.
  const other = await readyToUpload();
  await tickOn(other.doc, true);
  await upload(other);
  other.session.emit({ type: "frame", msg: { type: "system", message: "Channel #news created." } });
  echo(other, chats(other.session)[0]);
  await settle();
  runTimer(other, M.POST_GRACE_MS);
  assert.equal(notice(other.doc), published + " Posted in #general.", "another line meanwhile: posted once its echo came");

  // No answer at all.
  const silent = await readyToUpload();
  await tickOn(silent.doc, true);
  await upload(silent);
  runTimer(silent, 15000);
  await settle();
  assert.equal(notice(silent.doc), published + " Not posted in #general: the server did not answer.");

  // The recovery-phrase guard: the chat's own sentence, and nothing signed or sent.
  const held = await readyToUpload({ phraseIn: async () => true });
  await tickOn(held.doc, true);
  await upload(held);
  assert.equal(chats(held.session).length, 0, "the guard's own sentence: nothing sent");
  assert.equal(notice(held.doc), published + " Not posted in #general: " + WARN.PHRASE_GUARD_SENTENCE, "the guard's own sentence");
  // The guard cannot run: kept back, never sent unchecked.
  const unchecked = await readyToUpload({ phraseIn: null });
  await tickOn(unchecked.doc, true);
  await upload(unchecked);
  assert.equal(chats(unchecked.session).length, 0, "unchecked: nothing sent");
  assert.equal(notice(unchecked.doc), published + " Not posted in #general: this page could not check it for your recovery phrase, so it was kept back.");

  // The tick off: published, and nothing posted.
  const off = await readyToUpload();
  await upload(off);
  assert.equal(off.published.length, 1);
  assert.equal(chats(off.session).length, 0, "the tick off: nothing posted");
  assert.equal(notice(off.doc), published);

  // A direct message asked for (as no page should ever ask): never offered, the tick goes off, nothing
  // posted anywhere; only this server's channels are ever posted in.
  const dm = await readyToUpload({ page: { uploadWindow: { send() {} } } });
  dm.page.takeFiles([aFile("dusk.mp4", 3 * MIB)], "dm:" + ANN);
  await settle();
  const r = postRow(dm.doc);
  assert.equal(r.tick.checked, false, "a direct message asked for: the tick goes off");
  assert.ok(r.options.every((o) => /^(⭐ )?#/.test(o)), "only channels are offered: " + r.options);
  await upload(dm);
  assert.equal(dm.session.sent.filter((x) => x && (x.type === "chat" || x.type === "dm_put")).length, 0, "nothing posted, in a channel or a direct message");
});

// ── D. The Upload button ─────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with renderFormActions (files-add.js)
// never adding files-btn-rgb: "ready, for a file picked here: the RGB border" (actual false); with the
// class added however the button stood: "not while it cannot be pressed" (actual false); with
// files.css's rule naming another animation (`animation: files-rgb 3s ...`): "the nav bar's own
// animation" (actual 'files-rgb 3s linear infinite'); with the [data-reduced-motion] ring taken out: "still, a
// ring in the accent colour, as the desktop holds one" (actual false).
test("D. the Upload button wears the nav bar's RGB border when it would upload a file picked here, and stops moving under reduced motion", async () => {
  const p = await openPage({ routes: listRoutes(listAnswer([])) });
  const input = byId(p.doc, "files-file-input");
  input.files = [aFile("dusk.mp4", 3 * MIB)];
  input.dispatch("change");
  // While the share is being worked out the button cannot be pressed.
  const checking = byId(p.doc, "files-form-go");
  assert.ok(checking.disabled && !checking.className.split(/\s+/).includes("files-btn-rgb"), "not while it cannot be pressed");
  await settle();
  const go = byId(p.doc, "files-form-go");
  assert.equal(text(go), "Upload and add to Files", "the desktop's words");
  assert.ok(!go.disabled && go.className.split(/\s+/).includes("files-btn-rgb"), "ready, for a file picked here: the RGB border");
  const put = await openPage({ routes: listRoutes(listAnswer([])) });
  put.page.openPutForm({ filename: "1700_part.stl", name: "bushing.stl" });
  assert.equal(text(byId(put.doc, "files-form-go")), "Put on the Files page");
  assert.ok(!byId(put.doc, "files-form-go").className.includes("files-btn-rgb"), "Put on the Files page uploads nothing: no border");

  const css = fs.readFileSync(path.join(WEB, "shared", "files.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const shell = fs.readFileSync(path.join(WEB, "shared", "shell.js"), "utf8");
  const theme = fs.readFileSync(path.join(WEB, "shared", "theme.css"), "utf8");
  const navAnim = /\.hub-nav \.tab\.active \{[^}]*animation:\s*([^;]+);/.exec(shell);
  assert.ok(navAnim, "test setup: the nav's current-page tab is animated");
  const btnAnim = /\.files-btn-rgb \{[^}]*animation:\s*([^;]+);/.exec(css);
  assert.ok(btnAnim, "files.css animates .files-btn-rgb");
  assert.equal(btnAnim[1].trim(), navAnim[1].trim(), "the nav bar's own animation");
  assert.ok(/@keyframes channeling\b/.test(shell), "whose keyframes the shell gives every page");
  assert.ok(/@media \(prefers-reduced-motion: reduce\) \{\s*\.files-btn-rgb \{\s*animation: none;/.test(css), "the system's reduced motion stops it");
  assert.ok(/\[data-reduced-motion\] \* \{ animation: none !important;/.test(theme), "and so does the site's own setting, as for the nav");
  assert.ok(/@media \(prefers-reduced-motion: reduce\) \{\s*\.files-btn-rgb \{[^}]*outline: 2px solid var\(--accent\)/.test(css) &&
    /\[data-reduced-motion\] \.files-btn-rgb \{[^}]*outline: 2px solid var\(--accent\)/.test(css), "still, a ring in the accent colour, as the desktop holds one");
});

// ── E. The upload window ─────────────────────────────────────────────────

/** A connector to the page the window is over that keeps what it was told. */
function windowConnector() {
  const told = [];
  return { told, send: (event, data) => told.push(Object.assign({ event }, data || {})), last: (event) => told.filter((t) => t.event === event).pop() };
}
/** Element ids under `el`, in order: what a form shows, to hold two forms to each other. */
function idsIn(el) {
  const out = [];
  const walk = (n) => { for (const c of n.children) { if (c.id) out.push(c.id); walk(c); } };
  walk(el);
  return out;
}

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with files-app.js build() drawing the
// whole page in the window: "Add to Media and its uploads alone, under the sign-in line and the
// notices" (actual [ '', 'files-status', 'files-notice', 'files-tabs-wrap', 'files-main',
// 'files-admin-wrap', 'files-viewer', 'files-report', 'files-takedown', 'files-file-input' ]); in
// files-add.js, with takePending opening the form at once, before the page signed in: "they wait,
// named, while the page signs in" (actual null); with openAddForm ignoring postTo: "posting ticked,
// for #general" (actual false); with cancelForm not telling the page it is over: "Cancel closes the
// window" (actual false); in files-app.js, with dropDecision saying Drop to upload to Media over the
// window whatever channel it was opened for: "over the window: the chat's channel's words" (actual
// 'Drop to upload to Media'); with dropped() not posting a drop on the window in that channel: "and
// the drop opens Add to Media posting in #general" (actual false).
test("E. the upload window shows Add to Files alone, takes the files handed over once it may, posting ticked, and tells the page it is over what goes on", async () => {
  const relay = resumableRelay();
  const win = windowConnector();
  const session = fakeSession({ status: "connecting", role: "", roles: null, uploadToken: "" });
  const dropWin = fakeWindow(ORIGIN); // where files dragged over the window itself are heard
  const p = await openPage({
    session, storage: storageWith("general"), uploadWindow: win, phraseIn: async () => false, window: dropWin,
    routes: listRoutes(listAnswer([], { licences: LICENCES }), relay.routes.concat([
      [at("/api/files", "POST"), (c) => respond(200, { ok: true, item: H.item({ id: 7, url: "/uploads/" + c.body.filename, title: c.body.title, visibility: c.body.visibility }) })],
      // Its other file, added to the item once up (Edit's Add a file).
      [at("/api/files/item-files/add", "POST"), () => respond(200, { ok: true, item: H.item({ id: 7 }) })],
    ])),
  });
  assert.deepEqual(p.root.children.map((c) => c.id), ["files-status", "files-notice", "files-main", "files-file-input"], "Add to Files and its uploads alone, under the sign-in line and the notices");
  assert.deepEqual(idsIn(byId(p.doc, "files-main")), ["files-form", "files-uploads"]);
  for (const id of ["files-title", "files-tabs-wrap", "files-grid", "files-featured", "files-actions", "files-foot"]) assert.equal(byId(p.doc, id), null, id + " is not in the window");
  const lists = p.net.calls.filter((c) => c.url === "/api/files/list");
  assert.ok(lists.length >= 1 && lists.every((c) => c.body.limit === 1 && !c.body.featured), "the list read for its categories and licences only: one item, no shelf");

  const dusk = aFile("dusk.mp4", 3 * MIB);
  const dawn = aFile("dawn.mp4", 2 * MIB);
  p.page.takeFiles([dusk, dawn], "general");
  await settle();
  const waiting = byId(p.doc, "files-form-pending");
  assert.ok(waiting && !p.page.state.form, "they wait, named, while the page signs in");
  assert.deepEqual(waiting.querySelectorAll(".files-form-file-name").map(text), ["dusk.mp4 (" + M.sizeWords(3 * MIB) + ")", "dawn.mp4 (" + M.sizeWords(2 * MIB) + ")"]);
  assert.equal(win.last("busy").busy, true, "something is going: the window is kept");

  Object.assign(session.state, { status: "bound", role: "verified", roles: ROLES, uploadToken: "tok-1" });
  session.emit({ type: "state" });
  listChannels(session);
  await settle();
  const f = p.page.state.form;
  assert.ok(f && f.file === dusk, "signed in: Add to Files opens with the first file as the item");
  assert.deepEqual(f.more, [dawn], "the rest to be added once it is up");
  const r = postRow(p.doc);
  assert.ok(r && r.tick.checked && r.chosen === "general", "posting ticked, for #general");
  assert.equal(r.options[0], "#general (open in the chat)");
  assert.equal(win.last("busy").busy, false, "nothing going until Upload");

  // The same form as the Media page's own, every check in it (the share, the licence, Who can see it).
  const plain = await openPage({ storage: storageWith("general"), routes: listRoutes(listAnswer([], { licences: LICENCES })) });
  listChannels(plain.session);
  await pick(plain.doc, [dusk, dawn]);
  assert.deepEqual(idsIn(byId(p.doc, "files-form")), idsIn(byId(plain.doc, "files-form")), "the same form, part for part");
  assert.ok(byId(p.doc, "files-share-line") && byId(p.doc, "files-form-licence") && byId(p.doc, "files-form-vis-members"), "with the share left, the licence and who can see it");

  // Upload: it says what it says to the page it is over too, and is going until published and posted.
  byId(p.doc, "files-form-go").click();
  await settle();
  const sent = chats(session);
  assert.equal(sent.length, 1, "posted in #general once published");
  assert.equal(win.last("busy").busy, true, "going until the post is answered");
  echo({ session }, sent[0]);
  await settle();
  // Its line, as the desktop's says it: published, posted, then its other file going up to the item.
  const lines = win.told.filter((t) => t.event === "status").map((t) => t.text);
  assert.ok(lines.includes(M.FILES_WORDS.published + " Posted in #general. Its other file is uploading under Uploads in progress, and added to it when up."), "its line, for the page it is over: " + JSON.stringify(lines));
  assert.equal(lines[lines.length - 1], "Added to \"1760_dusk.mp4\".", "and the next, when its other file is in");

  // Files dropped on the window itself post in the same channel, as a drop on the chat under it does.
  const drag = (files) => ({ dataTransfer: { types: ["Files"], files: files || [], dropEffect: "none" }, preventDefault() {} });
  dropWin.fire("dragenter", drag());
  const overlay = p.doc.body.querySelector(".hos-drop-overlay");
  assert.equal(text(overlay.children[0]), "Drop to upload to Files and post in #general", "over the window: the chat's channel's words");
  const again = aFile("again.mp4", MIB);
  dropWin.fire("drop", drag([again]));
  await settle();
  assert.ok(p.page.state.form && p.page.state.form.file === again && postRow(p.doc).tick.checked && postRow(p.doc).chosen === "general", "and the drop opens Add to Files posting in #general");

  // Cancel on a form with nothing going closes the window.
  p.page.takeFiles([aFile("other.mp4", MIB)], "");
  await settle();
  byId(p.doc, "files-form-cancel").click();
  assert.ok(win.told.some((t) => t.event === "close"), "Cancel closes the window");
});

// ── F. The two halves of the upload window ───────────────────────────────

/** A window that keeps its listeners. */
function fakeWindow(origin, parent) {
  const listeners = {};
  const w = {
    location: { origin },
    posted: [],
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    postMessage(data, to) { w.posted.push({ data, to }); },
    fire(type, ev) { for (const f of (listeners[type] || []).slice()) f(ev); },
    setTimeout() { return 0; }, // the overlay's timer for a drag that left: never runs here
    clearTimeout() {},
  };
  w.parent = parent || w;
  return w;
}
/** A document that keeps its keydown listeners (the stand-in's ignores them). */
function docWithKeys() {
  const doc = fakeDocument();
  const keys = [];
  doc.addEventListener = (t, f) => { if (t === "keydown") keys.push(f); };
  doc.key = (key) => { for (const f of keys) f({ key }); };
  return doc;
}

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/shared/upload-window.js:
// with flush() not waiting for 'ready': "nothing goes before the page in it is there"; with onMessage
// not checking the message's source: "another window's message is not the frame's, nor one from
// another site"; with close() always removing the window: "closed with an upload going: hidden,
// kept"; with connectFrame taking files from any window: "only its host's".
test("F. the two halves: files wait until the frame is there; only its own messages count; Close keeps a window with something going, hidden, and its lines go to the chat", () => {
  const doc = docWithKeys();
  const host = fakeWindow(ORIGIN);
  const said = [];
  const w = UW.createHost({ window: host, document: doc, onStatus: (t) => said.push(t) });
  const a = { name: "dusk.mp4" };
  const b = { name: "dawn.mp4" };
  assert.ok(w.show([a, b], "general"));
  const el = w.element();
  assert.ok(el && doc.body.children.includes(el) && el.hidden === false, "the window is shown");
  assert.equal(el.className, "hos-upload-window");
  assert.equal(el.getAttribute("role"), "dialog");
  const frame = w.frame();
  assert.equal(frame.tagName, "IFRAME");
  assert.equal(frame.src, "/files?window=upload", "the Files page itself, in its upload-window form");
  assert.equal(UW.isWindowAddress("?window=upload"), true);
  assert.equal(UW.isWindowAddress("?put=1"), false);
  const inside = fakeWindow(ORIGIN, host);
  frame.contentWindow = inside;
  w.show([], ""); // shown again, nothing more dropped: the files still wait
  assert.equal(inside.posted.length, 0, "nothing goes before the page in it is there");
  assert.equal(w.pending().length, 1);

  const stranger = fakeWindow(ORIGIN);
  host.fire("message", { source: stranger, origin: ORIGIN, data: { kind: UW.KIND, event: "ready" } });
  host.fire("message", { source: inside, origin: "https://elsewhere.example", data: { kind: UW.KIND, event: "ready" } });
  assert.equal(inside.posted.length, 0, "another window's message is not the frame's, nor one from another site");
  host.fire("message", { source: inside, origin: ORIGIN, data: { kind: UW.KIND, event: "ready" } });
  assert.deepEqual(inside.posted, [{ data: { kind: UW.KIND, event: "files", files: [a, b], postTo: "general" }, to: ORIGIN }], "then the files, and the channel to post in");
  w.show([b], "");
  assert.deepEqual(inside.posted[1].data.files, [b], "the next drop goes at once");

  // Something going: Close hides it, kept; its lines go to the chat; once nothing is going it goes.
  host.fire("message", { source: inside, origin: ORIGIN, data: { kind: UW.KIND, event: "status", text: "Uploading." } });
  assert.deepEqual(said, [], "while it is open its lines are its own");
  host.fire("message", { source: inside, origin: ORIGIN, data: { kind: UW.KIND, event: "busy", busy: true } });
  doc.key("Escape");
  assert.ok(w.element() === el && el.hidden === true && doc.body.children.includes(el), "closed with an upload going: hidden, kept");
  assert.equal(w.isOpen(), false);
  host.fire("message", { source: inside, origin: ORIGIN, data: { kind: UW.KIND, event: "status", text: "It is on the Files page now. Posted in #general." } });
  assert.deepEqual(said, ["It is on the Files page now. Posted in #general."], "closed: its lines go to the chat");
  host.fire("message", { source: inside, origin: ORIGIN, data: { kind: UW.KIND, event: "busy", busy: false } });
  assert.equal(w.element(), null, "nothing going: removed");
  assert.ok(!doc.body.children.includes(el));

  // Nothing going: Close (its button, or the page's own close) removes it at once.
  w.show([a], "");
  const second = w.element();
  w.frame().contentWindow = inside;
  second.querySelector(".hos-upload-window-close").click();
  assert.equal(w.element(), null, "Close with nothing going: gone");

  // The window's half: only its host's messages, from this site, hand files over.
  const frameWin = fakeWindow(ORIGIN, host);
  const took = [];
  const c = UW.connectFrame({ window: frameWin, onFiles: (files, postTo) => took.push([files, postTo]) });
  c.ready();
  assert.deepEqual(host.posted.pop(), { data: { kind: UW.KIND, event: "ready" }, to: ORIGIN }, "it says it is there");
  frameWin.fire("message", { source: host, origin: ORIGIN, data: { kind: UW.KIND, event: "files", files: [a], postTo: "general" } });
  frameWin.fire("message", { source: stranger, origin: ORIGIN, data: { kind: UW.KIND, event: "files", files: [b], postTo: "general" } });
  frameWin.fire("message", { source: host, origin: "https://elsewhere.example", data: { kind: UW.KIND, event: "files", files: [b], postTo: "" } });
  assert.deepEqual(took, [[[a], "general"]], "only its host's");
  assert.equal(UW.connectFrame({ window: fakeWindow(ORIGIN) }), null, "a page not in a frame is no upload window");
});

// ── G. The recovery-phrase guard ─────────────────────────────────────────

/** pq-relay-auth.js in a context of its own, its seed in this browser's storage (as Chat keeps it). */
function guardWith(seed, opts) {
  const o = opts || {};
  const storage = fakeStorage();
  if (seed) {
    const pkcs8 = Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), Buffer.from(seed)]);
    storage.setItem("humanity_key_backup", JSON.stringify({ privateKeyPkcs8: pkcs8.toString("base64") }));
  }
  const ctx = { localStorage: storage, crypto: globalThis.crypto, TextEncoder, atob, console, Uint8Array, JSON };
  ctx.window = ctx;
  vm.createContext(ctx);
  if (!o.noWords) vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "bip39-english.js"), "utf8"), ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "warnings.js"), "utf8"), ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "pq-relay-auth.js"), "utf8"), ctx);
  return (texts) => ctx.pqRecoveryPhraseIn(texts);
}

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/shared/pq-relay-auth.js:
// with _phraseWords leaving out the checksum's bits (the last word then wrong): "its last word, from
// the checksum" (actual false); with pqRecoveryPhraseIn answering false when the word list is
// missing: "without the word list it cannot check" (actual false).
test("G. the guard finds four words of the recovery phrase in a row, as the chat's does, and says when it cannot check", async () => {
  // BIP39's own test phrases (trezor python-mnemonic vectors.json): 32 bytes of 0x00 and of 0x7f.
  const zero = guardWith(new Uint8Array(32));
  assert.equal(await zero(["Look: abandon abandon abandon abandon, and more"]), true, "four of BIP39's 0x00 phrase in a row");
  assert.equal(await zero(["abandon abandon abandon only three"]), false, "three are not enough");
  assert.equal(await zero(["abandon abandon abandon art"]), true, "its last word, from the checksum");
  const sevens = guardWith(new Uint8Array(32).fill(0x7f));
  assert.equal(await sevens(["the end: wave sausage worth title"]), true, "the last four words of BIP39's 0x7f phrase");
  assert.equal(await sevens(["Dusk\nAt the beach, after the rain.\n/uploads/1760_dusk.mp4"]), false, "an ordinary post");
  assert.equal(await sevens(["1. legal 2. winner 3. thank 4. year"]), true, "numbered as a backup screen shows it (warnings.js)");
  assert.equal(await guardWith(new Uint8Array(32), { noWords: true })(["abandon abandon abandon abandon"]), null, "without the word list it cannot check");
  assert.equal(await guardWith(null)(["abandon abandon abandon abandon"]), null, "without a seed in this browser it cannot check");
  assert.equal(await sevens([]), false, "nothing to check");
});

// ── The pages load it ────────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with files.html's upload-window.js line
// taken out: "pages/files.html loads the guard's parts and /shared/upload-window.js before
// /pages/files-app.js: 3033,3336,3387,3435,-1,6400"; with a hand-written colour in
// upload-window.css (`color: #000`): "theme variables only".
test("the pages load the upload window and the guard, and its stylesheet uses the theme only", () => {
  const page = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
  const order = (html, list) => list.map((s) => html.indexOf('src="' + s));
  const media = page("pages/files.html");
  const m = order(media, ["/chat/pq.js", "/chat/bip39-english.js", "/shared/warnings.js", "/shared/pq-relay-auth.js", "/shared/upload-window.js", "/pages/files-app.js"]);
  assert.ok(m.every((i) => i > 0), "pages/files.html loads the guard's parts and /shared/upload-window.js before /pages/files-app.js: " + m);
  assert.deepEqual(m.slice().sort((x, y) => x - y), m, "in order");
  assert.ok(/data-files-window/.test(media) && /window\.self !== window\.top/.test(media), "the menu is left out only in a frame");
  // In a frame, everything but the page and what its own parts put on the body is left out (which
  // parts: files-card-web.test.js, "8. the framed windows show the page's own dialogs").
  const hide = /\[data-files-window\] body > ((?::not\([^)]*\))+) \{ display: none !important; \}/.exec(media);
  const kept = hide ? hide[1].match(/:not\(([^)]*)\)/g).map((s) => s.slice(5, -1)) : [];
  assert.ok(kept.includes("#files-root") && kept.includes(".hos-drop-overlay"), "everything but the page and the drop overlay is left out: " + kept);
  const chat = page("chat/index.html");
  assert.ok(/<link rel="stylesheet" href="\/shared\/upload-window\.css\?v=\d+">/.test(chat), "the chat links /shared/upload-window.css");
  const c = order(chat, ["/shared/drop-overlay.js", "/shared/upload-window.js", "/chat/chat-messages.js"]);
  assert.ok(c.every((i) => i > 0) && c[0] < c[1] && c[1] < c[2], "the chat loads it before chat-messages.js: " + c);
  const css = page("shared/upload-window.css").replace(/\/\*[\s\S]*?\*\//g, "");
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i.test(css), "theme variables only");
  assert.ok(/\.hos-upload-window \{[^}]*background: var\(--bg-modal\)/.test(css), "dimmed as the dialogs are");
  assert.ok(/padding: [^;]*16px/.test(css), "a 16px gutter at a phone's width");
});
