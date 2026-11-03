// The privacy levels and the presence switch on the website (BUG-237, 2026-11-03). Until this
// fix the website showed the level (Private, Balanced, Open) once, at the first chat, and never
// again, and had no "Show others when I am online" switch at all: a person on the website who
// chose to be seen could not become harder to see. The desktop app's Settings > Privacy had both
// (src/gui/pages/privacy.rs). Protect the users (CLAUDE.md, 2026-10-09): the way back to Private
// is always one page away.
//
// Run: node --test scripts/tests/privacy-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).
//
// What it proves, with the real scripts run in node:vm over a stand-in page, socket and
// localStorage (the words from the real data/i18n/en.json, the levels from the real
// data/gui/privacy_tiers.json):
//  1. Settings > Privacy draws one card per level, in en.json's words, with the level last used
//     picked; a person whose last level was Spotlight (taken out, BUG-238) presses "Use this
//     privacy level" on Private and is Private: presence hidden and the directory left, kept in
//     this browser, and exactly one privacy_update {hide_presence: true} goes to this server on
//     the page's own signed-in connection, whose answer the page then shows.
//  2. "Show others when I am online" on its own: kept, and one privacy_update with just that
//     choice, both ways; the level is left as it was.
//  3. "List me in the public member directory" on its own: the profile this browser keeps says
//     unlisted, every other field kept, recorded as changed so the chat sends it on top of the
//     server's copy; no frame goes from this page.
//  4. No readable key in this browser: the choice is kept, nothing is sent, and the page says the
//     chat tells the server the next time it connects.
//  5. The chat tells its server the presence choice on every connect, with or without a level,
//     and at once when Settings changes it in another tab (the storage event); using a level in
//     the chat's chooser sets both switches as Settings does.
//  6. The words the page waits for are the relay's own answer to privacy_update, and the frame
//     both apps send is the one the relay reads.
//  7. The page has the section's controls, each with its words and hint by key, and loads its
//     scripts in order.
//  8. (BUG-239) Who sees what you keep for friends is said in one key, privacy.kept_for_friends,
//     in both apps (the website's Profile page under its locks and Settings > Privacy, the
//     desktop's Settings > Privacy), true to the relay (the owner's friendship pass decides), and
//     the Profile page no longer says follows decide it or that the whole profile is friends-only.
//
// Red first, 2026-11-03, each against web/ as it was before the fix (HOS_WEB_DIR at a copy of
// main's web/, where settings.html has no levels and settings-privacy.js does not exist) or a copy
// of the fixed web/ with one thing taken out:
//  1: main's web/: "cannot read web/shared/privacy-tiers.js"; with applyTier not setting the
//     directory: "the directory is left: unlisted".
//  2: setPresenceVisible sending the switch's own state as hide_presence: "one frame, hide".
//  3: setDirectory not recording the change: "recorded as changed for the chat".
//  4: tellServer without its no-identity line: "the page says why nothing went".
//  5: chat-privacy.js without its storage listener: "a change in Settings reaches the chat's
//     server at once"; main's reassert (from the level only): "the switch alone, with no level".
//  6: ANSWER_HIDDEN spelt "Presence is hidden:": "the relay's answer: Presence is hidden:" (and test 1's
//     "the server's answer is shown").
//  7: main's settings.html: "the level cards' place, a radio group".
//  8: main's profile.html: "the Profile page shows privacy.kept_for_friends"; the old lock note put
//     back beside the key: "the Profile page no longer says: people you follow who follow you back".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const read = (rel) => {
  const file = path.join(WEB, rel);
  if (!fs.existsSync(file)) throw new Error("cannot read web/" + rel);
  return fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
};
const TIERS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "privacy_tiers.json"), "utf8"));
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8"));
const en = (key) => key.split(".").reduce((o, k) => (o ? o[k] : undefined), EN);
const plain = (s) => String(s).replace(/\[\[([^\]|]+)(\|[^\]]+)?\]\]/g, "$1");
const fill = (key, vars) => plain(en(key)).replace(/\{\{(\w+)\}\}/g, (_, n) => String(vars[n]));

function fakeStorage(initial) {
  const m = new Map(Object.entries(initial || {}));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    _map: m,
  };
}

/** A stand-in element: enough of the DOM for the section (children, attributes, events). */
function fakeEl(tag, id) {
  const el = {
    tagName: String(tag).toUpperCase(), id: id || "", children: [], attrs: {}, listeners: {},
    className: "", type: "", name: "", value: "", checked: false, _text: "",
    appendChild(c) { this.children.push(c); c.parentNode = this; return c; },
    setAttribute(k, v) { this.attrs[k] = String(v); },
    getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; },
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); },
    fire(t) { for (const f of this.listeners[t] || []) f({ target: this }); },
  };
  Object.defineProperty(el, "textContent", {
    get() { return el.children.length ? el.children.map((c) => c.textContent).join("") : el._text; },
    set(v) { el.children = []; el._text = String(v); },
  });
  return el;
}

/** Every element under `el` (itself included) matching `pred`. */
function all(el, pred, out = []) {
  if (pred(el)) out.push(el);
  for (const c of el.children || []) all(c, pred, out);
  return out;
}

const IDS = ["sec-privacy", "privacy-tier-cards", "privacy-apply", "privacy-current", "privacy-presence", "privacy-directory", "privacy-status"];

function fakeWebSocketClass(sockets) {
  return class FakeWS {
    constructor(url) { this.url = url; this.readyState = 0; this.sent = []; this.listeners = {}; sockets.push(this); }
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
    fire(t, ev) { for (const f of this.listeners[t] || []) f(ev || {}); }
    send(s) { if (this.readyState !== 1) throw new Error("send on a socket that is not open"); this.sent.push(JSON.parse(s)); }
    close() { if (this.readyState === 3) return; this.readyState = 3; this.fire("close"); }
    open() { this.readyState = 1; this.fire("open"); }
    receive(o) { this.fire("message", { data: JSON.stringify(o) }); }
  };
}

async function settle() {
  for (let i = 0; i < 10; i++) await new Promise((r) => setImmediate(r));
}

const ME = "ab".repeat(32);

/** The website's Settings page with its Privacy section, as settings.html loads it. */
async function settingsPage(opts = {}) {
  const storage = fakeStorage(Object.assign({ humanity_name: "Ann" }, opts.stored || {}));
  const els = {};
  for (const id of IDS) els[id] = fakeEl(id === "privacy-apply" ? "button" : id.startsWith("privacy-p") || id === "privacy-directory" ? "input" : "div", id);
  const sockets = [];
  const storageListeners = [];
  const ctx = {
    console: { log() {}, info() {}, warn() {}, error() {} },
    document: {
      readyState: "complete",
      getElementById: (id) => els[id] || null,
      createElement: (tag) => fakeEl(tag),
      documentElement: fakeEl("html"),
      addEventListener() {},
    },
    localStorage: storage,
    fetch: async (url) => {
      if (url === "/data/gui/privacy_tiers.json") return { ok: true, json: async () => JSON.parse(JSON.stringify(opts.tiers || TIERS)) };
      throw new Error("no network in tests: " + url);
    },
    WebSocket: fakeWebSocketClass(sockets),
    location: { protocol: "https:", host: "united-humanity.us" },
    getPqIdentity: async () => (opts.noIdentity ? null : { dilithiumPublicHex: ME, dilithiumSecret: new Uint8Array(4) }),
    pqSignMessage: async () => new Uint8Array([1, 2, 3]),
    addEventListener: (t, f) => { if (t === "storage") storageListeners.push(f); },
    dispatchEvent() {},
    CustomEvent: function () {},
    setTimeout: () => 0,
    clearTimeout: () => {},
    TextEncoder,
    btoa: globalThis.btoa,
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  // In settings.html's order: the words (shell.js loads them first), then the section's scripts.
  for (const rel of ["shared/i18n-en.js", "shared/i18n.js", "shared/profile-changes.js", "shared/relay-session.js", "shared/privacy-tiers.js", "pages/settings-privacy.js"]) {
    vm.runInContext(read(rel), ctx, { filename: rel });
  }
  await settle();
  /** Sign the page's socket in as the relay does: challenge, then the member list. */
  async function signIn() {
    const ws = sockets[sockets.length - 1];
    assert.ok(ws, "the page opened its own connection");
    assert.equal(ws.url, "wss://united-humanity.us/ws");
    ws.open();
    ws.receive({ type: "identify_challenge", nonce: "n0nce" });
    await settle();
    ws.receive({ type: "peer_list", peers: [{ public_key: ME, role: "verified" }] });
    await settle();
    return ws;
  }
  const cards = () => all(els["privacy-tier-cards"], (e) => e.tagName === "LABEL");
  const radios = () => all(els["privacy-tier-cards"], (e) => e.tagName === "INPUT");
  const stored = () => JSON.parse(storage.getItem("humanity_profile") || "{}");
  const privacyFrames = (ws) => ws.sent.filter((m) => m.type === "privacy_update");
  return { ctx, els, storage, sockets, signIn, cards, radios, stored, privacyFrames, storageListeners };
}

// ── 1. Back to Private in one page ───────────────────────────────────────

test("1. Settings > Privacy draws the levels in en.json's words, and takes a person who chose Spotlight back to Private", async () => {
  const page = await settingsPage({
    stored: {
      humanity_privacy_tier: "spotlight", // the level taken out (BUG-238): its switches were Open's
      humanity_hide_presence: "0",
      humanity_profile: JSON.stringify({ bio: "Gardener", location: "Lisbon", privacy: { location: "private" } }),
    },
  });
  // One card per level in the file, in its order and en.json's words, and its hint by key.
  const cards = page.cards();
  assert.deepEqual(cards.map((c) => c.getAttribute("data-tier")), TIERS.tiers.map((t) => t.id), "one card per level");
  assert.deepEqual(
    cards.map((c) => c.textContent),
    TIERS.tiers.map((t) => plain(en(t.name)) + plain(en(t.tagline)) + plain(en(t.description))),
    "each card's words are en.json's, by the keys the file names",
  );
  for (const r of page.radios()) assert.equal(r.getAttribute("data-i18n-tip"), "privacy.tier_card_hint", "each card has its hint by key");
  // Spotlight is not a level any more: the default is picked, nothing is applied, no "last used".
  assert.deepEqual(page.radios().map((r) => r.checked), TIERS.tiers.map((t) => t.id === TIERS.default_tier), "the default is picked");
  assert.equal(page.els["privacy-current"].textContent, "", "no level last used that the file still has");
  assert.equal(page.els["privacy-presence"].checked, true, "seen online, as Spotlight left them");
  assert.equal(page.els["privacy-directory"].checked, true, "listed, as Spotlight left them");
  assert.equal(page.sockets.length, 0, "opening Settings signs nobody in");

  // One press: Private.
  page.els["privacy-apply"].fire("click");
  await settle();
  assert.equal(page.storage.getItem("humanity_privacy_tier"), "private", "kept as the level last used");
  assert.equal(page.storage.getItem("humanity_hide_presence"), "1", "presence hidden");
  const profile = page.stored();
  assert.equal(profile.privacy.directory, "unlisted", "the directory is left: unlisted");
  assert.deepEqual([profile.bio, profile.location, profile.privacy.location], ["Gardener", "Lisbon", "private"], "every other field of the profile kept");
  assert.deepEqual(JSON.parse(page.storage.getItem("humanity_profile_changes")), ["privacy:directory"], "recorded as changed for the chat");
  assert.equal(page.els["privacy-status"].textContent, fill("privacy.web_saved_sending", {}), "the page says it is telling the server");
  // This server is told, once signed in, in the frame the relay reads.
  const ws = await page.signIn();
  assert.deepEqual(page.privacyFrames(ws), [{ type: "privacy_update", hide_presence: true }], "exactly one privacy_update: hidden");
  ws.receive({ type: "private", message: "Presence hidden: you no longer appear online, no last-seen is stored, and no join/leave or typing signals are sent." });
  assert.equal(page.els["privacy-status"].textContent, fill("privacy.web_presence_hidden_done", {}), "the server's answer is shown");
  // The page shows what is now true.
  assert.equal(page.els["privacy-presence"].checked, false, "the presence switch shows hidden");
  assert.equal(page.els["privacy-directory"].checked, false, "the directory switch shows unlisted");
  assert.equal(page.els["privacy-current"].textContent, fill("privacy.current_tier", { tier: plain(en("privacy.tier_private_name")) }), "the level last used, in its words");
});

// ── 2. The presence switch on its own ────────────────────────────────────

test("2. Show others when I am online: kept, and one privacy_update with just that choice, both ways", async () => {
  const page = await settingsPage({ stored: { humanity_privacy_tier: "private", humanity_hide_presence: "1" } });
  assert.equal(page.els["privacy-presence"].checked, false, "hidden, as kept");
  page.els["privacy-presence"].checked = true;
  page.els["privacy-presence"].fire("change");
  await settle();
  const ws = await page.signIn();
  assert.deepEqual(page.privacyFrames(ws), [{ type: "privacy_update", hide_presence: false }], "one frame, shown");
  assert.equal(page.storage.getItem("humanity_hide_presence"), "0", "kept: shown");
  ws.receive({ type: "private", message: "Presence visible: you appear online to other members." });
  assert.equal(page.els["privacy-status"].textContent, fill("privacy.web_presence_shown_done", {}), "the server's answer");
  // And back, on the same connection.
  page.els["privacy-presence"].checked = false;
  page.els["privacy-presence"].fire("change");
  await settle();
  assert.equal(page.sockets.length, 1, "the same connection");
  assert.deepEqual(page.privacyFrames(ws), [{ type: "privacy_update", hide_presence: false }, { type: "privacy_update", hide_presence: true }], "one frame, hide");
  assert.equal(page.storage.getItem("humanity_privacy_tier"), "private", "the level last used is left as it was");
  assert.equal(page.storage.getItem("humanity_profile"), null, "the profile is not touched");
});

// ── 3. The directory switch on its own ───────────────────────────────────

test("3. List me in the public member directory: kept in the profile the chat sends, recorded as changed, no frame from here", async () => {
  const page = await settingsPage({ stored: { humanity_profile: JSON.stringify({ bio: "Gardener", socials: { github: "ann" } }) } });
  assert.equal(page.els["privacy-directory"].checked, true, "listed: no unlisted in the profile");
  assert.equal(page.els["privacy-presence"].checked, false, "presence never chosen shows hidden, as the server keeps a new account");
  page.els["privacy-directory"].checked = false;
  page.els["privacy-directory"].fire("change");
  await settle();
  assert.deepEqual(page.stored(), { bio: "Gardener", socials: { github: "ann" }, privacy: { directory: "unlisted" } }, "unlisted, every other field kept");
  assert.deepEqual(JSON.parse(page.storage.getItem("humanity_profile_changes")), ["privacy:directory"], "recorded as changed for the chat");
  assert.equal(page.els["privacy-status"].textContent, fill("privacy.web_directory_saved", {}), "the page says the chat sends it");
  assert.equal(page.sockets.length, 0, "no frame from this page: the directory goes with the profile, from the chat");
  // Back on: the key goes, the change is still recorded.
  page.els["privacy-directory"].checked = true;
  page.els["privacy-directory"].fire("change");
  assert.deepEqual(page.stored().privacy, {}, "listed again");
});

// ── 4. No readable key here ──────────────────────────────────────────────

test("4. With no readable key in this browser the choice is kept, nothing is sent, and the page says when it goes", async () => {
  const page = await settingsPage({ noIdentity: true, stored: { humanity_hide_presence: "0" } });
  page.els["privacy-presence"].checked = false;
  page.els["privacy-presence"].fire("change");
  await settle();
  assert.equal(page.storage.getItem("humanity_hide_presence"), "1", "kept: hidden");
  assert.equal(page.sockets.length, 0, "no connection without a key");
  assert.equal(page.els["privacy-status"].textContent, fill("privacy.web_no_identity", {}), "the page says why nothing went");
});

// ── 5. The chat ──────────────────────────────────────────────────────────

/** The chat's privacy script over a stand-in page and an open socket. */
function chatPage(stored) {
  const storage = fakeStorage(stored);
  const sent = [];
  const storageListeners = [];
  const lines = [];
  const ctx = {
    console: { log() {}, info() {}, warn() {}, error() {} },
    document: { getElementById: () => null, createElement: (t) => fakeEl(t), body: fakeEl("body"), documentElement: fakeEl("html"), addEventListener() {} },
    localStorage: storage,
    fetch: async (url) => {
      if (url === "/data/gui/privacy_tiers.json") return { ok: true, json: async () => JSON.parse(JSON.stringify(TIERS)) };
      throw new Error("no network in tests: " + url);
    },
    WebSocket: { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 },
    addEventListener: (t, f) => { if (t === "storage") storageListeners.push(f); },
    dispatchEvent() {},
    CustomEvent: function () {},
    setTimeout: () => 0,
    clearTimeout: () => {},
    esc: (s) => String(s),
    addSystemMessage: (m) => lines.push(String(m)),
    pushProfileToRelay: () => {},
    handleMessage: () => {},
  };
  ctx.window = ctx;
  ctx.ws = { readyState: 1, send: (s) => sent.push(JSON.parse(s)) };
  vm.createContext(ctx);
  for (const rel of ["shared/i18n-en.js", "shared/i18n.js", "shared/profile-changes.js", "shared/privacy-tiers.js", "chat/chat-privacy.js"]) {
    vm.runInContext(read(rel), ctx, { filename: rel });
  }
  return { ctx, storage, sent, storageListeners, lines };
}

test("5. The chat tells its server the presence choice on connect and at once when Settings changes it; its chooser sets both switches", async () => {
  // Never chosen: nothing is said (the server keeps a new account hidden).
  const fresh = chatPage({});
  await fresh.ctx.reassertPrivacyTier();
  assert.deepEqual(fresh.sent, [], "never chosen: nothing sent");
  // The switch alone, set in Settings, with no level ever picked.
  const page = chatPage({ humanity_hide_presence: "0" });
  await page.ctx.reassertPrivacyTier();
  assert.deepEqual(page.sent, [{ type: "privacy_update", hide_presence: false }], "the switch alone, with no level");
  // Settings changes it in another tab: the chat's server hears it at once.
  page.storage.setItem("humanity_hide_presence", "1");
  for (const f of page.storageListeners) f({ key: "humanity_hide_presence" });
  assert.deepEqual(page.sent.slice(1), [{ type: "privacy_update", hide_presence: true }], "a change in Settings reaches the chat's server at once");
  for (const f of page.storageListeners) f({ key: "humanity_unrelated" });
  assert.equal(page.sent.length, 2, "another key changing sends nothing");
  // The chooser's level: both switches, as Settings sets them, and its words by key.
  page.sent.length = 0;
  await page.ctx.applyPrivacyTier("balanced");
  assert.deepEqual(page.sent, [{ type: "privacy_update", hide_presence: false }], "Balanced: seen online");
  assert.equal(page.storage.getItem("humanity_hide_presence"), "0");
  assert.equal(JSON.parse(page.storage.getItem("humanity_profile")).privacy.directory, "unlisted", "Balanced: not in the directory");
  assert.equal(page.storage.getItem("humanity_privacy_tier"), "balanced");
  assert.deepEqual(page.lines, [fill("privacy.web_level_set", { level: plain(en("privacy.tier_balanced_name")) })], "the level named in its words");
  // app.js calls the chooser on every sign-in: with a level chosen it asks nothing, and each
  // connect tells the server again.
  page.sent.length = 0;
  await page.ctx.maybeShowPrivacyTierModal();
  await page.ctx.maybeShowPrivacyTierModal();
  assert.deepEqual(page.sent, [{ type: "privacy_update", hide_presence: false }, { type: "privacy_update", hide_presence: false }], "every connect tells the server");
});

// ── 6. The relay's words and frame ───────────────────────────────────────

test("6. The page waits for the relay's own answer, and both apps send the frame the relay reads", () => {
  const P = require(path.join(WEB, "shared", "privacy-tiers.js"));
  const relay = fs.readFileSync(path.join(ROOT, "src", "relay", "relay.rs"), "utf8");
  for (const start of [P.ANSWER_HIDDEN, P.ANSWER_SHOWN]) {
    assert.ok(relay.includes('"' + start), "the relay's answer: " + start);
  }
  assert.match(relay, /#\[serde\(rename = "privacy_update"\)\]\s*PrivacyUpdate \{\s*hide_presence: bool,/, "the relay reads privacy_update {hide_presence}");
  const desktop = fs.readFileSync(path.join(ROOT, "src", "gui", "pages", "privacy.rs"), "utf8");
  assert.ok(desktop.includes('"type": "privacy_update", "hide_presence"'), "the desktop sends the same frame");
  assert.deepEqual(P.presenceFrame(true), { type: "privacy_update", hide_presence: true });
  // A level file that cannot be read, or names a word en.json lacks, gives the one Private level.
  const has = (k) => typeof en(k) === "string";
  assert.ok(P.parseTiers(TIERS, has), "the shipped file is accepted");
  const bad = JSON.parse(JSON.stringify(TIERS));
  bad.tiers[1].name = "privacy.no_such_word";
  assert.equal(P.parseTiers(bad, has), null, "a missing word refuses the file");
  const fb = P.fallback();
  assert.deepEqual([fb.defaultTier, fb.tiers.length, fb.tiers[0].hide_presence, fb.tiers[0].directory_unlisted], ["private", 1, true, true], "fail-private");
  for (const t of TIERS.tiers) for (const f of ["name", "tagline", "description"]) assert.ok(has(t[f]), t.id + "." + f + " is a key in en.json");
});

// ── 7. The page ──────────────────────────────────────────────────────────

test("7. Settings > Privacy has the levels and both switches, each with its words and hint by key, and loads its scripts in order", () => {
  const html = read("pages/settings.html");
  const section = html.slice(html.indexOf('id="sec-privacy"'), html.indexOf("<!-- Chat -->"));
  assert.ok(section.length > 100, "the Privacy section");
  assert.match(section, /id="privacy-tier-cards"[^>]*role="radiogroup"/, "the level cards' place, a radio group");
  assert.match(section, /<button[^>]*id="privacy-apply"[^>]*data-i18n="privacy\.apply_tier"[^>]*data-i18n-tip="privacy\.apply_tier_hint"/, "the button, by key, with its hint");
  // A Settings row's description is its control's hint (shell.js copies it onto the row's
  // controls, as the desktop's Settings page shows it), and the switch is described by it for a
  // screen reader. (privacy.directory_does marks a glossary term, which a tooltip attribute would
  // drop: glossary-terms.test.js.)
  for (const [id, label, does] of [["privacy-presence", "presence_label", "presence_does"], ["privacy-directory", "directory_label", "directory_does"]]) {
    const row = section.slice(section.lastIndexOf('<div class="setting-row">', section.indexOf('id="' + id + '"')), section.indexOf('id="' + id + '"') + 400);
    assert.ok(row.includes('<strong data-i18n="privacy.' + label + '">'), id + ": the switch is named by key on its row");
    assert.ok(row.includes('<span id="' + id + '-does" data-i18n="privacy.' + does + '">'), id + ": its row says what it does, by key");
    assert.match(row, new RegExp('<input type="checkbox" id="' + id + '" data-i18n-aria-label="privacy\\.' + label + '"[^>]*aria-describedby="' + id + '-does"'), id + ": the switch, by key, described by its row");
  }
  assert.match(section, /id="privacy-status"[^>]*role="status"/, "the line that says what happened");
  assert.ok(!section.includes("privacy.web_switches_title"), "the old label-only switches are gone");
  const at = (s) => { const i = html.indexOf('src="' + s); assert.ok(i >= 0, "settings.html loads " + s); return i; };
  // The words come first (2026-11-03, seen on the live site): settings-privacy.js draws the level
  // cards with i18n.t as it mounts, and the copies shell.js adds to the page run only after the
  // page's own scripts, so without these two the cards showed their keys ("privacy.tier_private_
  // name"). The tests above hand the section its i18n directly, which is why they could not see it.
  // Red before the two tags were added to settings.html: "settings.html loads /shared/i18n-en.js".
  assert.ok(at("/shared/i18n-en.js") < at("/shared/i18n.js"), "English first, then i18n.js (web/shared/i18n.js says why)");
  assert.ok(at("/shared/i18n.js") < at("/shared/privacy-tiers.js"), "the words before the privacy scripts");
  const order = ["/chat/pq.js", "/shared/pq-relay-auth.js", "/shared/profile-changes.js", "/shared/relay-session.js", "/shared/privacy-tiers.js", "/pages/settings-privacy.js"];
  for (let i = 1; i < order.length; i++) assert.ok(at(order[i - 1]) < at(order[i]), order[i - 1] + " before " + order[i]);
  const chat = read("chat/index.html");
  assert.ok(chat.indexOf('src="/shared/privacy-tiers.js') > 0 && chat.indexOf('src="/shared/privacy-tiers.js') < chat.indexOf('src="/chat/chat-privacy.js'), "the chat loads privacy-tiers.js before chat-privacy.js");
});

// ── 8. Who sees what you keep for friends (BUG-239) ──────────────────────

test("8. Both apps say who sees what you keep for friends in one key, and the Profile page no longer says follows decide it", () => {
  // The words, true to the relay: a field kept for friends is shown to the holder of the owner's
  // friendship pass on that server (handlers/profile.rs handle_profile_request), nobody else.
  const handler = fs.readFileSync(path.join(ROOT, "src", "relay", "handlers", "profile.rs"), "utf8");
  assert.ok(handler.includes("friend_pass(state, &owner_key, my_key, friend_cert.as_deref())"), "the relay decides by the owner's friendship pass");
  const words = en("privacy.kept_for_friends");
  assert.ok(words.includes("[[friendship pass]]"), "the words name the friendship pass, as a glossary term");
  const glossary = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "glossary.json"), "utf8"));
  assert.ok(glossary.terms["friendship pass"], "the glossary explains it");
  // The website's Profile page: the locks' words by key, the profile's and the Interests tab's too.
  const profile = read("pages/profile.html");
  for (const key of ["privacy.kept_for_friends", "profile.lock_public_note", "profile.network_seen_by", "profile.interests_kept_here"]) {
    assert.ok(profile.includes('data-i18n="' + key + '"'), "the Profile page shows " + key);
  }
  const shown = profile.replace(/<!--[\s\S]*?-->/g, "");
  for (const old of ["people you follow who follow you back", "Visible to people you follow and who follow you back", "Shared with friends to help match"]) {
    assert.ok(!shown.includes(old), "the Profile page no longer says: " + old);
  }
  // Settings > Privacy in both apps shows the same key.
  const settings = read("pages/settings.html");
  assert.ok(settings.slice(settings.indexOf('id="sec-privacy"'), settings.indexOf("<!-- Chat -->")).includes('data-i18n="privacy.kept_for_friends"'), "the website's Settings > Privacy");
  const desktop = fs.readFileSync(path.join(ROOT, "src", "gui", "pages", "privacy.rs"), "utf8");
  assert.ok(desktop.includes('t_marked("privacy.kept_for_friends")'), "the desktop's Settings > Privacy");
  const desktopProfile = fs.readFileSync(path.join(ROOT, "src", "gui", "pages", "profile.rs"), "utf8");
  assert.ok(desktopProfile.includes('t("profile.network_seen_by")'), "the desktop's Network Profile says who sees it, in the same words");
});
