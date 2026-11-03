// Chat access levels on the website (2026-11-02, docs/design/chat-access.md), run under Node.
//
//   node --test scripts/tests/chat-access-web.test.js       (in `just rig-tests`)
//
// What it proves:
//  1. web/shared/chat-access.js gives every answer of the shared fixture
//     (scripts/tests/fixtures/chat-access-v1.json), as the Rust rules do (chat_access.rs).
//  2. The read-only view for a visitor without a name (web/chat/read.html + read.js) lists the
//     channels the server says anyone may read, draws their posts as plain words with names, and
//     has no box to type in; its one button opens the welcome step; /chat sends a browser with no
//     key there; search engines are asked not to keep it.
//  3. The chat's line above the box says who can read the channel, and what I may post, by the
//     channel's levels and my rank; a direct message has none.
//  4. A poster below verified has their links, pictures and files drawn as plain words.
//  5. The admin page's Channels panel draws each channel's two pickers and Save runs the
//     registry's channels.access with what was picked.
//
// The page scripts run as in the browser, in one shared global scope (node:vm), on the stand-in
// document of files-web-harness.js where they build elements, and a stub elsewhere.
// HOS_WEB_DIR points it at another copy of web/ (used to see each test red).
//
// Seen red 2026-11-02, one break at a time in a copy of web/: the twin counting only https:// links
// (test 1, "links in see https://a.example and HTTP://b.example"); the twin letting a verified member
// call @everyone (test 1, the mass_mention row); read.js writing a post as markup (test 2, "the page
// set innerHTML"); /chat sending a keyless visitor to the welcome step again (test 2); the line above
// the box without its second part (test 3, "Anyone, then the one-link rule") and without its badge
// (test 3, "the level badge first"); formatBody linking an unverified poster's address (test 4, "no
// link, picture or card"); the admin panel sending a fixed read level (test 5, "Save runs
// channels.access with what was picked").

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./files-web-harness.js");
const ROOT = H.ROOT;
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const A = require(path.join(WEB, "shared", "chat-access.js"));
const FX = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "chat-access-v1.json"), "utf8"));
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8"));

/** en.json's words by key, [[term]] marks shown as their words. */
function t(key) {
  const v = key.split(".").reduce((o, k) => (o && typeof o === "object" ? o[k] : undefined), EN);
  return typeof v === "string" ? v.replace(/\[\[([^\]|]+)(?:\|[^\]]+)?\]\]/g, "$1") : key;
}

function anything() {
  const fn = function () {};
  return new Proxy(fn, {
    get(_t, prop) {
      if (prop === Symbol.toPrimitive) return () => "";
      if (prop === Symbol.iterator) return function* () {};
      if (prop === "then") return undefined;
      if (prop === "length") return 0;
      return anything();
    },
    set: () => true,
    has: () => true,
    apply: () => anything(),
    construct: () => anything(),
  });
}

const all = (root) => { const out = []; const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } }; walk(root); return out; };

test("1. the website's rules give every answer of the shared fixture", () => {
  const rank = (w) => A.RANKS[w];
  for (const [role, trust, want] of FX.roles) assert.strictEqual(A.rankForMember(role, trust), rank(want), "role " + JSON.stringify(role));
  for (const [read, who, want] of FX.reads) assert.strictEqual(A.mayRead(read, rank(who)), want, "read " + read + " by " + who);
  for (const [read, post, want] of FX.normalized) assert.strictEqual(A.normalizedPost(read, post), want, "normalized " + read + "/" + post);
  for (const [content, n] of FX.links) assert.strictEqual(A.linksIn(content), n, "links in " + content);
  for (const [content, yes] of FX.attachments) assert.strictEqual(A.carriesAttachment(content), yes, "attachment in " + content);
  for (const [content, yes] of FX.mass_mentions) assert.strictEqual(A.massMention(content), yes, "a call on everyone in " + content);
  for (const p of FX.posts) assert.strictEqual(A.postRefusal(p.read, p.post, rank(p.rank), p.content), p.refusal, "post " + JSON.stringify(p));
  assert.strictEqual(A.readPurpose(FX.purpose[0], FX.purpose[1]), FX.purpose[2]);
  assert.strictEqual(A.readLevel("public"), "members", "a word that names no level is the safe default");
});

// ── 2. The read-only view ──

const READ_HTML = fs.readFileSync(path.join(WEB, "chat", "read.html"), "utf8");
const IDS = ["read-tabs", "read-refresh", "read-list", "read-status", "read-join"];

async function bootRead(answers) {
  const doc = H.fakeDocument();
  for (const id of IDS) {
    const e = doc.createElement(id === "read-refresh" ? "button" : id === "read-join" ? "a" : "div");
    e.setAttribute("id", id);
    if (id === "read-refresh") doc.getElementById("read-tabs").appendChild(e);
    else doc.body.appendChild(e);
  }
  const asked = [];
  const fetch = (url) => {
    asked.push(String(url));
    const p = String(url).split("?")[0];
    const body = answers[p];
    const a = typeof body === "function" ? body(String(url)) : body;
    if (a === undefined || a === 401) return Promise.resolve({ ok: false, status: a || 404, json: () => Promise.resolve({ error: "no" }) });
    return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(JSON.parse(JSON.stringify(a))) });
  };
  const ctx = { document: doc, fetch, console, Date, URLSearchParams, encodeURIComponent };
  ctx.window = ctx;
  ctx.i18n = { t, hint: (el, key) => { el.setAttribute("data-i18n-tip", key); el.setAttribute("data-tip-desc", t(key)); return el; } };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "read.js"), "utf8"), ctx, { filename: "read.js" });
  await H.settle();
  return { doc, asked, ctx };
}

test("2. a visitor without a name reads the channels anyone may read, as plain words, with no box to type in", async () => {
  const posts = [
    { type: "chat", from: "", from_name: "ann", content: "see https://a.example and /uploads/1_cat.png", timestamp: 1_792_800_000_000, channel: "general" },
    { type: "chat", from: "", from_name: "bob", content: "<b>not markup</b>", timestamp: 1_792_800_060_000, channel: "general" },
  ];
  const { doc, asked } = await bootRead({
    "/api/channels": { channels: [{ id: "general", name: "general" }, { id: "announcements", name: "announcements" }] },
    "/api/messages": (u) => (u.includes("channel=general") ? { messages: posts, cursor: 2 } : { messages: [], cursor: 0 }),
  });
  const tabs = all(doc.getElementById("read-tabs")).filter((e) => e.className === "read-tab");
  assert.deepStrictEqual(tabs.map((b) => b.textContent), ["#general", "#announcements"], "the channels the server lists");
  assert.strictEqual(tabs[0].getAttribute("aria-selected"), "true", "the first is open");
  assert.strictEqual(tabs[0].getAttribute("data-i18n-tip"), "chat_read.channel_hint", "each with its hint");
  assert.ok(asked.some((u) => u.startsWith("/api/messages?channel=general")), "it reads #general without a key");
  for (const u of asked) assert.ok(u.startsWith("/api/"), "only this server: " + u);
  const rows = all(doc.getElementById("read-list")).filter((e) => e.className === "read-line");
  assert.strictEqual(rows.length, 2);
  assert.ok(rows[0].textContent.startsWith("ann"), "the poster's name");
  assert.ok(rows[0].textContent.includes("see https://a.example and /uploads/1_cat.png"), "the words as written");
  assert.ok(rows[1].textContent.includes("<b>not markup</b>"), "words from the server stay words");
  const made = all(doc.body).map((e) => e.tagName);
  for (const tag of ["A", "IMG", "TEXTAREA", "INPUT", "VIDEO", "AUDIO"]) {
    assert.ok(!made.includes(tag) || (tag === "A" && made.filter((x) => x === "A").length === 1), "no " + tag + " made from a post (the one A is the join button)");
  }
  // The page itself: no box to type in, the one button, and no search engine keeping it.
  assert.ok(!/<textarea|<input/i.test(READ_HTML), "read.html has no box to type in");
  assert.ok(READ_HTML.includes('href="/?welcome=1&amp;return=/chat"') && READ_HTML.includes('data-i18n="chat_read.join"'), "Choose a name to join in opens the welcome step and comes back to the chat");
  assert.ok(/<meta name="robots" content="noindex/.test(READ_HTML), "search engines are asked not to keep people's words");
  // /chat sends a browser with no key here.
  const index = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  assert.ok(index.includes("location.replace('/chat/read')"), "/chat sends a visitor with no key to the read-only view");
});

test("2b. an empty channel and a server with none say so", async () => {
  const empty = await bootRead({ "/api/channels": { channels: [{ id: "general", name: "general" }] }, "/api/messages": { messages: [], cursor: 0 } });
  assert.strictEqual(empty.doc.getElementById("read-status").textContent, t("chat_read.empty"));
  const none = await bootRead({ "/api/channels": { channels: [] } });
  assert.strictEqual(none.doc.getElementById("read-status").textContent, t("chat_read.no_channels"));
  assert.ok(!none.asked.some((u) => u.startsWith("/api/messages")), "nothing is read when nothing may be");
});

// ── 3 and 4. The chat page ──

function loadChat() {
  const doc = H.fakeDocument();
  const audience = doc.createElement("div");
  audience.setAttribute("id", "chat-audience");
  doc.body.appendChild(audience);
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === "createElement") return (tag) => doc.createElement(tag);
      if (p === "createTextNode") return (s) => doc.createTextNode(s);
      if (p === "getElementById") return (id) => (id === "chat-audience" ? audience : anything());
      return anything();
    },
  });
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document, navigator: anything(),
    location: { hash: "", host: "u.example", origin: "https://u.example", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: anything(), sessionStorage: anything(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {}, removeEventListener: () => {}, matchMedia: () => anything(),
    requestAnimationFrame: () => 0, Notification: anything(), crypto: anything(),
    TextEncoder, TextDecoder, URLSearchParams, URL, CustomEvent: function () {},
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  run("shared/i18n-en.js");
  run("shared/i18n.js");
  run("shared/events.js");
  run("shared/chat-access.js");
  run("chat/app.js");
  ctx.hosIcon = () => "";
  // app.js escapes through an element's innerHTML; the stand-in elements refuse it, so a plain escaper stands in.
  ctx.esc = (str) => String(str).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const set = (js) => vm.runInContext(js, ctx);
  return { ctx, audience, set };
}

test("3. the line above the box says who can read the channel and what I may post", () => {
  const { audience, set } = loadChat();
  set("myKey = 'me'; activeDmPartner = null; activeChannel = 'general'; peerData = { me: { public_key: 'me', role: '' } };");
  set("channelList = [{ id: 'general', name: 'general', read_level: 'anyone', post_level: 'members' }, { id: 'lounge', name: 'lounge', read_level: 'members', post_level: 'members' }, { id: 'announcements', name: 'announcements', read_level: 'anyone', post_level: 'mods' }];");
  set("renderAudienceLine()");
  assert.strictEqual(audience.hidden, false);
  assert.strictEqual(audience.textContent.trim(), t("chat.access_read_anyone") + " " + t("chat.access_one_link"), "Anyone, then the one-link rule for a member below verified");
  const badge = all(audience).find((e) => e.className.includes("ch-level"));
  assert.ok(badge && badge.getAttribute("data-level") === "anyone", "the level badge first (6.1)");
  assert.strictEqual(badge.getAttribute("data-tip-desc"), "Who can read: Anyone. Who can post: Members.", "its hint says who can read and post");
  assert.strictEqual(badge.getAttribute("role"), "img");
  const term = all(audience).find((e) => e.getAttribute("data-term"));
  assert.ok(term && term.getAttribute("data-term") === "verified_member", "its word verified opens the Verified glossary card");

  set("activeChannel = 'lounge'; peerData.me.role = 'verified'; renderAudienceLine()");
  assert.strictEqual(audience.textContent.trim(), t("chat.access_read_members"), "a Members channel, for a verified member: no more");

  set("activeChannel = 'announcements'; peerData.me.role = ''; renderAudienceLine()");
  assert.strictEqual(all(audience).find((e) => e.className.includes("ch-level")).getAttribute("data-tip-desc"), "Who can read: Anyone. Who can post: Moderators and admins.");
  assert.strictEqual(audience.textContent.trim(), t("chat.access_read_anyone") + " " + t("chat.access_post_mods"), "why I cannot post here");

  // The channel list draws the same badge (chat-ui.js, from app.js chatLevelBadgeHtml).
  const listBadge = set("chatLevelBadgeHtml(channelList[2])");
  assert.ok(listBadge.includes('data-level="anyone"') && listBadge.includes('data-tip-desc="Who can read: Anyone. Who can post: Moderators and admins."'), "the channel list's badge: " + listBadge);
  assert.ok(fs.readFileSync(path.join(WEB, "chat", "chat-ui.js"), "utf8").includes("chatLevelBadgeHtml(ch) +"), "chat-ui.js puts it beside each channel's name");
  set("activeDmPartner = 'someone'; renderAudienceLine()");
  assert.strictEqual(audience.hidden, true, "a direct message is no channel");

  const index = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  assert.ok(index.indexOf('id="chat-audience"') > 0 && index.indexOf('id="chat-audience"') < index.indexOf('id="input-area"'), "just above the box you type in");
  assert.ok(index.indexOf('src="/shared/chat-access.js') < index.indexOf('src="/chat/app.js'), "the rules load before app.js");
});

test("4. a poster below verified has links, pictures and files drawn as plain words", () => {
  const { ctx, set } = loadChat();
  set("myKey = 'me'; activeDmPartner = null; activeChannel = 'general'; peerData = { ann: { role: '' }, vic: { role: 'verified' } };");
  const body = (from) => vm.runInContext("formatBody", ctx)("see https://a.example and https://u.example/uploads/1_cat.png", from);
  const plain = body("ann");
  assert.ok(!plain.includes("<a ") && !plain.includes("img-placeholder") && !plain.includes("file-card"), "no link, picture or card: " + plain);
  assert.ok(plain.includes("https://a.example"), "the address is there to read");
  const verified = body("vic");
  assert.ok(verified.includes("<a ") && verified.includes("img-placeholder"), "a verified poster's link and picture: " + verified);
  set("activeDmPartner = 'ann'");
  assert.ok(body("ann").includes("<a "), "in a direct message links are drawn as before");
});

// ── 5. The admin page's Channels panel ──

test("5. the admin page's Channels panel sets who can read and who can post through channels.access", async () => {
  const doc = H.fakeDocument();
  const list = doc.createElement("div");
  list.setAttribute("id", "channels-list");
  const statusEl = doc.createElement("div");
  statusEl.setAttribute("id", "channels-status");
  doc.body.append(list, statusEl);
  const ran = [];
  const view = { channels: [{ id: "lounge", name: "lounge", read_level: "members", post_level: "members" }], read_levels: A.READ_LEVELS, post_levels: A.POST_LEVELS };
  const ctx = { document: doc, console };
  ctx.window = ctx;
  ctx.i18n = { t, hint: (el, key) => { el.setAttribute("data-i18n-tip", key); el.setAttribute("data-tip-desc", t(key)); return el; } };
  ctx.HosAdminActions = {
    adminRun: async (action, inputs) => {
      ran.push([action, inputs]);
      const v = JSON.parse(JSON.stringify(view));
      v.channels[0].read_level = inputs.read;
      v.channels[0].post_level = inputs.post;
      return { message: "done", data: v };
    },
  };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "pages", "admin-console.js"), "utf8"), ctx, { filename: "admin-console.js" });
  ctx.hosAdminChannels.renderChannels(view);
  const row = all(list).find((e) => e.className.includes("channel-access-row"));
  assert.ok(row && row.textContent.includes("#lounge"), "the channel's row");
  const selects = all(row).filter((e) => e.tagName === "SELECT");
  assert.strictEqual(selects.length, 2, "Who can read and Who can post");
  assert.deepStrictEqual(selects.map((s) => s.value), ["members", "members"], "each shows the channel's level");
  assert.deepStrictEqual(all(selects[1]).map((o) => o.textContent), [t("chat.level_members"), t("chat.level_verified"), t("chat.level_mods")]);
  assert.strictEqual(selects[0].getAttribute("data-i18n-tip"), "admin.channels_read_hint", "with its hint");
  selects[0].value = "anyone";
  selects[1].value = "mods";
  const save = all(row).find((e) => e.tagName === "BUTTON");
  assert.strictEqual(save.getAttribute("data-i18n-tip"), "admin.channels_save_hint");
  save.click();
  await H.settle();
  // Compared as JSON: the inputs object was made in the page's own realm.
  assert.strictEqual(JSON.stringify(ran), JSON.stringify([["channels.access", { channel: "lounge", read: "anyone", post: "mods" }]]), "Save runs channels.access with what was picked");
  const after = all(list).filter((e) => e.tagName === "SELECT").map((s) => s.value);
  assert.deepStrictEqual(after, ["anyone", "mods"], "and the panel draws the server's answer");
  const adminHtml = fs.readFileSync(path.join(WEB, "pages", "admin.html"), "utf8");
  assert.ok(adminHtml.includes('id="channels-panel"') && adminHtml.includes('id="channels-list"'), "admin.html has the panel");
  assert.ok(adminHtml.indexOf('src="/shared/i18n.js"') < adminHtml.indexOf("/shared/shell.js"), "the words load before the shell");
});
