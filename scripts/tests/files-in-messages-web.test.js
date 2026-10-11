// Files in messages, the rules (2026-10-11, docs/design/media-showcase.md, "Files on the sender's
// device and in messages, exact", part 3; the operator's answer 6): web/shared/files-in-messages.js.
// The chat's use of them (the cards, Accept, the Safety section, the attach button, the profile
// field, the sender's line) is in private-files-web.test.js.
//
// Run: node --test scripts/tests/files-in-messages-web.test.js
//
// What it proves:
//  1. The labels and choices are the spec's, and the defaults are its defaults (Friends, Friends,
//     the attach button on); anything kept that is not one of the choices reads as the default,
//     and storage that refuses changes nothing.
//  2. "Accept files from": Nobody refuses everyone but me (a friend too: "send but not receive"),
//     Friends lets friends through, Anyone I talk to everyone; mine always open.
//  3. "Show pictures without asking from": Friends shows a friend's at once, Nobody nobody's, and
//     the protected setup is always Nobody; mine always show.
//  4. The words: the card's ("Ann sent a file. You accept files only from friends. Accept this
//     one?"), the sender's line ("Ann accepts files only from friends."), shown only when it
//     applies, and the profile's value is one of the three words.
//
// HOS_WEB_DIR points the test at another copy of web/. Red first: see the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const F = require(path.join(WEB, "shared", "files-in-messages.js"));

const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "media-showcase.md"), "utf8");
const SECTION = SPEC.slice(SPEC.indexOf("## Files on the sender's device and in messages, exact")).replace(/\s+/g, " ");

function storage(init) {
  const m = new Map(Object.entries(init || {}));
  return { m, getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}
const refusing = { getItem() { throw new Error("denied"); }, setItem() { throw new Error("denied"); } };
const ME = { isMe: true, isFriend: true };
const FRIEND = { isMe: false, isFriend: true };
const STRANGER = { isMe: false, isFriend: false };

test("the labels, choices and defaults are the spec's; anything else kept reads as the default", () => {
  assert.ok(SECTION.includes(`"${F.WORDS.group}"`), "the group's name");
  const acceptLine = `**${F.WORDS.accept}:** ${F.ACCEPT_CHOICES.map((c) => F.WORDS.acceptChoices[c]).join(" / ")}. Default Friends.`;
  assert.ok(SECTION.includes(acceptLine), "Accept files from, its three choices in order, default Friends: " + acceptLine);
  assert.ok(SECTION.includes(`**${F.WORDS.pictures}:** ${F.PICTURE_CHOICES.map((c) => F.WORDS.pictureChoices[c]).join(" / ")}`), "Show pictures without asking from, Friends / Nobody");
  assert.ok(SECTION.includes(`**${F.WORDS.attach}:** on or off`), "the attach button, on or off");
  assert.deepEqual(F.DEFAULTS, { accept: "friends", pictures: "friends", attach: true }, "the defaults: Friends, Friends, the button on");
  assert.deepEqual(F.readSettings(storage()), F.DEFAULTS, "nothing kept: the defaults");
  assert.deepEqual(F.readSettings(storage({ [F.FILES_SETTINGS_KEY]: '{"accept":"everyone","pictures":7,"attach":"no"}' })), F.DEFAULTS, "nonsense: the defaults");
  assert.deepEqual(F.readSettings(storage({ [F.FILES_SETTINGS_KEY]: "not json" })), F.DEFAULTS);
  assert.deepEqual(F.readSettings(refusing), F.DEFAULTS, "storage that refuses: the defaults");
  assert.equal(F.writeSettings(refusing, { accept: "nobody" }), null, "and keeps nothing, without throwing");
  const st = storage();
  assert.equal(F.settingsChosen(st), false);
  assert.deepEqual(F.writeSettings(st, { accept: "anyone", pictures: "nobody", attach: false }), { accept: "anyone", pictures: "nobody", attach: false });
  assert.deepEqual(F.readSettings(st), { accept: "anyone", pictures: "nobody", attach: false }, "kept and read back");
  assert.equal(F.settingsChosen(st), true);
});

test("Accept files from: Nobody refuses a friend too, Friends lets friends through, Anyone everyone; mine always open", () => {
  const table = {
    nobody: [true, false, false],
    friends: [true, true, false],
    anyone: [true, true, true],
  };
  for (const [accept, [me, friend, stranger]] of Object.entries(table)) {
    const s = { accept };
    assert.equal(F.acceptsFrom(s, ME), me, `${accept}: mine`);
    assert.equal(F.acceptsFrom(s, FRIEND), friend, `${accept}: a friend's`);
    assert.equal(F.acceptsFrom(s, STRANGER), stranger, `${accept}: anyone else's`);
  }
  assert.equal(F.acceptLooser("friends", "anyone"), true, "Friends to Anyone is more open");
  assert.equal(F.acceptLooser("nobody", "friends"), true);
  assert.equal(F.acceptLooser("anyone", "nobody"), false, "closing is never more open");
});

test("Show pictures without asking: Friends shows a friend's at once, Nobody nobody's, the protected setup always Nobody", () => {
  assert.equal(F.picturesWithoutAsking({ pictures: "friends" }, FRIEND, false), true);
  assert.equal(F.picturesWithoutAsking({ pictures: "friends" }, STRANGER, false), false);
  assert.equal(F.picturesWithoutAsking({ pictures: "nobody" }, FRIEND, false), false, "Nobody: a friend's waits too");
  assert.equal(F.picturesWithoutAsking({ pictures: "friends" }, FRIEND, true), false, "the protected setup: always Nobody");
  for (const prot of [false, true]) for (const pictures of ["friends", "nobody"]) {
    assert.equal(F.picturesWithoutAsking({ pictures }, ME, prot), true, "mine always show");
  }
});

test("the card's words, the sender's line when it applies, and the profile's value", () => {
  const card = F.askWords("Ann", { accept: "friends" });
  assert.equal(card, "Ann sent a file. You accept files only from friends. Accept this one?");
  assert.ok(SECTION.includes(`"${card}"`), "the spec's own card");
  assert.equal(F.askWords("Ann", { accept: "nobody" }), "Ann sent a file. You do not accept files in messages. Accept this one?");

  const line = F.senderLine("Ann", "friends", false);
  assert.ok(line.startsWith("Ann accepts files only from friends."), "the spec's sender line first");
  assert.ok(SECTION.includes('"Ann accepts files only from friends."'));
  assert.equal(F.senderLine("Ann", "friends", true), null, "not when I am their friend");
  assert.equal(F.senderLine("Ann", "anyone", false), null, "not when they accept anyone's");
  assert.ok(F.senderLine("Ann", "nobody", true).startsWith("Ann does not accept files in messages."), "Nobody, even for a friend");
  assert.equal(F.senderLine("Ann", undefined, false), null, "not when their choice is not known");
  assert.equal(F.senderLine("Ann", "<b>", false), null, "nor for a word that is not one of the three");

  for (const accept of F.ACCEPT_CHOICES) assert.equal(F.profileValue({ accept }), accept);
  assert.equal(F.profileValue({}), "friends", "the default, published as such");
  assert.deepEqual(F.ACCEPT_CHOICES, ["nobody", "friends", "anyone"], "the profile's three words (the relay accepts only these)");
  assert.ok(SECTION.includes('`accepts_files` ("nobody", "friends" or "anyone"'), "the spec's three words");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  F1 files-in-messages.js, acceptsFrom letting a friend through under Nobody: test 2 "nobody: a
//     friend's".
//  F2 files-in-messages.js, picturesWithoutAsking ignoring the protected setup: test 3 "the
//     protected setup: always Nobody".
//  F3 files-in-messages.js, the default accept 'anyone': test 1 "the defaults: Friends, Friends,
//     the button on", and test 4 "the default, published as such".
//  F4 files-in-messages.js, senderLine said to a friend under Friends: test 4 "not when I am their
//     friend".
