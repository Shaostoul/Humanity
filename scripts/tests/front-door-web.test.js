// The website's front door (web/pages/index.html + index-app.js, docs/design/front-door.md, adopted
// 2026-11-02) and the Devlog (web/pages/devlog.html + devlog-app.js), run under Node.
//
// The pages run as in the browser, on the stand-in document of files-web-harness.js (it holds only
// what the page built: a test that finds a card proves the page made it, and setting innerHTML
// throws, so nothing the server says can become markup). The network is a stand-in that answers
// /data/... from this repository's data/ folder and /api/front-door and /api/releases from the
// answers below, shaped like src/relay/front_door.rs's; it records every address asked.
//
//   node --test scripts/tests/front-door-web.test.js       (in `just rig-tests`)
//
// HOS_WEB_DIR points it at another copy of web/ (used to see each test red; see the end).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const ROOT = H.ROOT;
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const FD = require(path.join(WEB, "shared", "front-door.js"));
const PAGE = require(path.join(WEB, "pages", "index-app.js"));
const DEVLOG = require(path.join(WEB, "pages", "devlog-app.js"));

const data = (rel) => JSON.parse(fs.readFileSync(path.join(ROOT, "data", rel), "utf8"));
const EN = data("i18n/en.json");
const CFG = data("front_door/config.json");
const INDEX_HTML = fs.readFileSync(path.join(WEB, "pages", "index.html"), "utf8");

/** en.json's words by key, {{placeholders}} filled, [[term]] marks shown as their words. */
function t(key, vars) {
  let v = key.split(".").reduce((o, k) => (o && typeof o === "object" ? o[k] : undefined), EN);
  if (typeof v !== "string") return key;
  v = v.replace(/\[\[([^\]|]+)(?:\|[^\]]+)?\]\]/g, "$1");
  return v.replace(/\{\{(\w+)\}\}/g, (_m, k) => (vars && vars[k] != null ? String(vars[k]) : ""));
}
const hint = (el, key) => { el.setAttribute("data-tip-desc", t(key)); el.setAttribute("data-hint-key", key); };

const PREFIX = "https://github.com/Shaostoul/Humanity/releases/tag/";
/** Nine releases, newest first, the newest and the fifth signed, the way the server answers. */
function updates(n) {
  return Array.from({ length: n || 9 }, (_v, i) => {
    const version = "0.15" + (47 - i) + ".0";
    return { version, title: "release " + (47 - i) + " headline", url: PREFIX + "v" + version, at: 1_792_800_000_000 - i * 3_600_000, signed: i === 0 || i === 4 };
  });
}

/** The network: /data/ from the repo, the release feed from `answers`. */
function network(answers) {
  const asked = [];
  return {
    asked,
    fetch(url) {
      asked.push(String(url));
      const u = String(url);
      const path0 = u.split("?")[0];
      if (path0 in answers) {
        const a = answers[path0];
        const body = typeof a === "function" ? a(u) : a;
        if (body === 500) return Promise.resolve({ ok: false, status: 500, json: () => Promise.reject(new Error("500")) });
        return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(JSON.parse(JSON.stringify(body))) });
      }
      const p = path.join(ROOT, "data", path0.replace(/^\/data\//, ""));
      if (!path0.startsWith("/data/") || !fs.existsSync(p)) return Promise.resolve({ ok: false, status: 404, json: () => Promise.reject(new Error("404")) });
      const text = fs.readFileSync(p, "utf8");
      return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(JSON.parse(text)) });
    },
  };
}

/** The ids index.html gives the script, made in the stand-in document (and checked to be in the page). */
const IDS = ["fd-welcome", "fd-welcome-name", "fd-welcome-join", "fd-welcome-error", "fd-q", "fd-results", "fd-start", "fd-menu", "fd-cards", "fd-cards-note", "start", "fd-back", "fd-back-words", "fd-change", "fd-goals", "fd-answer"];
const TAGS = { "fd-welcome-name": "input", "fd-q": "input", "fd-welcome-join": "button", "fd-change": "button", "fd-start": "a" };
const HIDDEN = ["fd-welcome", "fd-results", "fd-cards-note", "fd-back", "fd-answer"];

async function boot(o) {
  o = o || {};
  const doc = H.fakeDocument();
  for (const id of IDS) {
    const e = doc.createElement(TAGS[id] || "div");
    e.setAttribute("id", id);
    if (HIDDEN.includes(id)) e.hidden = true;
    doc.body.appendChild(e);
  }
  const net = network(Object.assign({ "/api/front-door": { updates: updates() } }, o.answers || {}));
  const storage = o.storage || H.fakeStorage();
  const replaced = [];
  let clock = 1_792_800_000_000;
  let width = o.width == null ? 1068 : o.width;
  const page = PAGE.createFrontDoor({
    document: doc, fetch: net.fetch, storage, t, hint, icon: () => "", fd: FD,
    location: Object.assign({ search: o.search || "", hash: "", replace: (u) => replaced.push(u) }, o.location || {}),
    history: null, now: () => clock, widthOf: () => width, scrollTo: () => {},
  });
  await page.start();
  await H.settle();
  return { doc, net, page, storage, replaced, setWidth: (w) => { width = w; }, advance: (ms) => { clock += ms; } };
}

const all = (root) => { const out = []; const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } }; walk(root); return out; };
const byClass = (root, c) => all(root).filter((e) => e.className.split(/\s+/).includes(c));
const cards = (doc) => byClass(doc.getElementById("fd-cards"), "update-card");

test("1. the page's ids are index.html's, and it loads the shared card and its own script after the words", () => {
  for (const id of IDS) assert.ok(INDEX_HTML.includes('id="' + id + '"'), "index.html has #" + id);
  const at = (s) => INDEX_HTML.indexOf(s);
  assert.ok(at('src="/shared/i18n.js"') < at('src="/shared/front-door.js"') && at('src="/shared/front-door.js"') < at('src="/pages/index-app.js"'), "words, then the rules, then the page");
  assert.ok(at('href="/shared/update-card.css"') > 0, "the card's styles");
  assert.ok(!/chat-slice|\/api\/messages/.test(INDEX_HTML + fs.readFileSync(path.join(WEB, "pages", "index-app.js"), "utf8")), "no chat on the front page");
});

test("2. the update cards come from this server's /api/front-door, four across in the standard column", async () => {
  const { doc, net } = await boot();
  assert.ok(net.asked.includes("/api/front-door"), "asks the front door's feed");
  const got = cards(doc);
  assert.strictEqual(got.length, 4, "1,068 px of row holds four 220 px cards");
  assert.strictEqual(got[0].textContent.includes("v0.1547.0"), true);
  assert.ok(got[0].textContent.includes("release 47 headline"));
  const link = all(got[1]).find((e) => e.tagName === "A");
  assert.strictEqual(link.getAttribute("href"), PREFIX + "v0.1546.0", "Release notes is the release's own page");
  assert.strictEqual(link.getAttribute("data-hint-key"), "front_door.update_notes_hint");
  // Only this server: no address but our own paths is ever asked.
  for (const u of net.asked) assert.ok(u.startsWith("/"), "asked only this server: " + u);
  assert.ok(!net.asked.some((u) => u.includes("search-index.json")), "never the Library's full-text index");
  assert.ok(!net.asked.includes("/data/how_to/index.json"), "the search's files wait for the first key");
});

test("3. a signed release's card is dressed its own way, with the word, and an unsigned one is not", async () => {
  const { doc } = await boot({ width: 2528 });
  const got = cards(doc);
  assert.strictEqual(got.length, 9, "a 2,528 px row would hold nine (the most)");
  const signedOnes = got.filter((c) => c.className.includes("update-card-signed"));
  assert.deepStrictEqual(signedOnes.map((c) => all(c)[0].textContent.slice(0, 9)), ["v0.1547.0", "v0.1543.0"]);
  for (const c of signedOnes) {
    const mark = byClass(c, "update-card-signed-mark")[0];
    assert.ok(mark, "a Signed mark");
    assert.strictEqual(mark.textContent, "Signed", "the word, not only a colour");
    assert.strictEqual(mark.getAttribute("tabindex"), "0", "the keyboard reaches it");
    assert.strictEqual(mark.getAttribute("data-hint-key"), "front_door.signed_hint");
    assert.ok(mark.getAttribute("aria-label").includes("The desktop app offers only signed versions"));
  }
  for (const c of got.filter((x) => !signedOnes.includes(x))) {
    assert.strictEqual(byClass(c, "update-card-signed-mark").length, 0, "an unsigned card has no mark");
  }
  const css = fs.readFileSync(path.join(WEB, "shared", "update-card.css"), "utf8");
  assert.match(css, /\.update-card\.update-card-signed\s*\{[^}]*border-top-color:\s*var\(--badge-verified\)/, "the verified band");
  assert.doesNotMatch(css.replace(/\/\*[\s\S]*?\*\//g, ""), /#[0-9a-f]{3,8}\b|rgba?\(/i, "theme tokens only");
});

test("4. below two cards side by side the row becomes a swipe row of nine; the rule follows the width", async () => {
  const rule = (w) => { const l = FD.cardLayout(w, CFG); return l.swipe ? "swipe" : l.n; };
  // The design's table (section 3), the standard column at each window, 16 px gutters.
  assert.deepStrictEqual([1068, 992, 736, 568, 468, 452, 398, 358, 328].map(rule), [4, 4, 3, 2, 2, 2, "swipe", "swipe", "swipe"]);
  assert.strictEqual(rule(2076), 9, "nine need 2,076 px");
  assert.strictEqual(rule(2075), 8);
  assert.strictEqual(FD.cardLayout(380, CFG).w, 280, "a swipe card is 280 px");
  assert.strictEqual(FD.cardLayout(300, CFG).w, 246, "or 82% of a narrow row");

  const phone = await boot({ width: 380 });
  const row = phone.doc.getElementById("fd-cards");
  assert.ok(row.className.split(/\s+/).includes("fd-swipe"), "a phone's row swipes");
  assert.strictEqual(cards(phone.doc).length, 9, "with all nine");
  assert.strictEqual(row.style["--fd-swipe-w"], "280px");
  phone.setWidth(800);
  phone.page.drawCards(false);
  assert.ok(!row.className.split(/\s+/).includes("fd-swipe"), "turned to a tablet, a grid again");
  assert.strictEqual(cards(phone.doc).length, 3);
  assert.strictEqual(row.style["--fd-n"], "3");
});

test("5. the cards carry only the release's own page, and the feed being down is said, not hidden", async () => {
  const odd = updates(4);
  odd[1].url = "https://evil.example/v0.1546.0";
  odd[2].url = PREFIX + "v0.1544.0/../x";
  const { doc } = await boot({ answers: { "/api/front-door": { updates: odd } } });
  const links = cards(doc).map((c) => all(c).filter((e) => e.tagName === "A").map((a) => a.getAttribute("href")));
  assert.deepStrictEqual(links, [[PREFIX + "v0.1547.0"], [], [], [PREFIX + "v0.1544.0"]], "a card links nowhere but its own release page");

  const down = await boot({ answers: { "/api/front-door": 500 } });
  assert.strictEqual(cards(down.doc).length, 0);
  const note = down.doc.getElementById("fd-cards-note");
  assert.strictEqual(note.hidden, false);
  assert.strictEqual(note.textContent, EN.front_door.updates_offline);
});

test("6. the tab coming back asks again only after ten minutes, never on a timer", async () => {
  const { net, page, advance } = await boot();
  const count = () => net.asked.filter((u) => u === "/api/front-door").length;
  assert.strictEqual(count(), 1);
  advance(CFG.refresh_after_ms - 1);
  assert.strictEqual(await page.visible(), false);
  advance(2);
  assert.strictEqual(await page.visible(), true);
  assert.strictEqual(count(), 2);
});

test("7. the search reads plain words: the design's table (section 7.4)", async () => {
  const { doc, page, net } = await boot();
  const table = [
    ["my jar lid is bulging", "Test the seals on home-canned jars"],
    ["diarrhea", "Mix oral rehydration solution from a packet"],
    ["throwing up", "Mix oral rehydration solution from a packet"],
    ["my water is cloudy", "Boil water to make it safe to drink"],
  ];
  for (const [q, first] of table) {
    await page.typeQuery(q);
    const box = doc.getElementById("fd-results");
    assert.strictEqual(box.hidden, false, q);
    const a = all(box).filter((e) => e.tagName === "A");
    assert.ok(a.length > 1, q + ": results and the whole-Library link");
    assert.ok(a[0].textContent.startsWith(first), q + ": the first answer is " + first + ", not " + a[0].textContent);
  }
  await page.typeQuery("power cut");
  assert.ok(doc.getElementById("fd-results").textContent.includes("Planned, not written yet"), "power cut: a planned guide, said so");
  assert.ok(doc.getElementById("fd-results").textContent.toLowerCase().includes("power cut"));
  await page.typeQuery("grow tomatoes");
  const tomato = all(doc.getElementById("fd-results")).filter((e) => e.tagName === "A")[0];
  assert.strictEqual(tomato.getAttribute("href"), "/library#your-first-tomato", "grow tomatoes: the Library's Your First Tomato");

  const more = byClass(doc.getElementById("fd-results"), "fd-r-more")[0];
  assert.strictEqual(more.getAttribute("href"), "/library?q=grow%20tomatoes", "the whole Library gets the words (?q=)");
  for (const f of ["/data/how_to/index.json", "/data/how_to/planned.json", "/data/library/index.json", "/data/front_door/search_words.json"]) {
    assert.strictEqual(net.asked.filter((u) => u === f).length, 1, "read once, on the first key: " + f);
  }
  await page.typeQuery("xyzzy");
  assert.ok(doc.getElementById("fd-results").textContent.includes('Nothing yet for "xyzzy"'));
});

test("8. Start here: each answer opens its first thing, and its steps share the Onboarding page's ticks", async () => {
  const goals = data("front_door/goals.json").goals;
  const chains = Object.fromEntries(data("onboarding/quests.json").chains.map((c) => [c.id, c]));
  const storage = H.fakeStorage();
  storage.setItem("humanity_quest_progress", JSON.stringify({ "water:understand-consumption": true }));
  const { doc, page } = await boot({ storage });
  const buttons = byClass(doc.getElementById("fd-goals"), "fd-goal");
  assert.deepStrictEqual(buttons.map((b) => b.dataset.goal), goals.map((g) => g.id), "one button per goal, in the file's order");

  buttons.find((b) => b.dataset.goal === "water").click();
  const answer = doc.getElementById("fd-answer");
  assert.strictEqual(answer.hidden, false);
  assert.ok(answer.textContent.includes(EN.front_door.goal_water_answer));
  const open = byClass(answer, "fd-open")[0];
  assert.strictEqual(open.getAttribute("href"), "/how-to#boil_water_safe");
  assert.ok(answer.textContent.includes("Step 2 of 4"), "a tick on the Onboarding page is a tick here: " + answer.textContent);
  assert.ok(answer.textContent.includes(chains.water.steps[1].title), "and the next step");
  assert.strictEqual(storage.getItem("hos_front_door_goal"), "water", "remembered on this device");
  assert.strictEqual(buttons.find((b) => b.dataset.goal === "water").getAttribute("aria-pressed"), "true");

  page.pick("care", true);
  const first = all(answer).find((e) => e.tagName === "P");
  assert.strictEqual(first.textContent, EN.front_door.goal_emergency, "someone in danger: call first, before anything else");
  assert.ok(answer.className.includes("fd-one"), "no chain beside it");

  const again = await boot({ storage });
  assert.strictEqual(again.doc.getElementById("fd-back").hidden, false);
  assert.strictEqual(again.doc.getElementById("fd-back-words").textContent, "Welcome back. You picked Someone is sick or hurt.");
  again.doc.getElementById("fd-change").click();
  assert.strictEqual(again.doc.getElementById("fd-answer").hidden, true);
  assert.strictEqual(storage.getItem("hos_front_door_goal"), null);

  const refused = await boot({ storage: H.fakeStorage(true) });
  assert.strictEqual(byClass(refused.doc.getElementById("fd-goals"), "fd-goal").length, goals.length, "a private window still works");
});

test("9. every button, goal and word the data names exists", () => {
  const icons = fs.readFileSync(path.join(WEB, "shared", "icons.js"), "utf8");
  const pages = new Set(fs.readdirSync(path.join(WEB, "pages")).filter((f) => f.endsWith(".html")).map((f) => "/" + f.replace(/\.html$/, "")));
  const exists = (href) => pages.has(href.split(/[?#]/)[0]) || href.startsWith("/?welcome=1");
  const key = (k) => typeof k.split(".").reduce((o, x) => (o ? o[x] : undefined), EN) === "string";
  for (const d of data("front_door/destinations.json").destinations) {
    assert.ok(exists(d.href), d.id + ": " + d.href + " is a page");
    assert.ok(new RegExp("\\b" + d.icon + ":").test(icons), d.id + ": icon " + d.icon);
    for (const k of [d.name, d.line, d.hint]) assert.ok(key(k), k);
  }
  const how = new Set(data("how_to/index.json").how_tos.map((h) => h.id));
  const slugs = new Set();
  (function walk(c) { (c || []).forEach((x) => { (x.docs || []).forEach((d) => slugs.add(FD.slugOf(d.file))); walk(x.children); }); })(data("library/index.json").categories);
  const chains = new Set(data("onboarding/quests.json").chains.map((c) => c.id));
  for (const g of data("front_door/goals.json").goals) {
    const f = g.first;
    if (f.kind === "how_to") assert.ok(how.has(f.id), g.id + ": how-to " + f.id);
    else if (f.kind === "library") assert.ok(slugs.has(f.id), g.id + ": Library " + f.id);
    else assert.ok(exists(f.id), g.id + ": page " + f.id);
    if (g.chain) assert.ok(chains.has(g.chain), g.id + ": chain " + g.chain);
    for (const k of [g.name, g.line, g.answer, f.label]) assert.ok(key(k), k);
  }
});

test("10. the welcome step keeps only the name, and only ever hands over to a page of this site", async () => {
  const shown = await boot({ search: "?welcome=1&return=/chat" });
  assert.strictEqual(shown.doc.getElementById("fd-welcome").hidden, false);
  const input = shown.doc.getElementById("fd-welcome-name");
  input.value = "no spaces allowed";
  shown.doc.getElementById("fd-welcome-join").click();
  assert.strictEqual(shown.doc.getElementById("fd-welcome-error").textContent, EN.front_door.welcome_error_name);
  assert.deepStrictEqual(shown.replaced, []);
  input.value = "River_7";
  shown.doc.getElementById("fd-welcome-join").click();
  assert.strictEqual(shown.storage.getItem("humanity_name"), "River_7");
  assert.deepStrictEqual(shown.replaced, ["/chat#joined"]);
  assert.strictEqual(shown.storage.m.size, 1, "the name and nothing else: the chat makes the key, and shows the recovery phrase");

  for (const ret of ["javascript:alert(document.domain)", "https://evil.example/", "//evil.example/x", "/chat\" onmouseover=\"x"]) {
    const b = await boot({ search: "?welcome=1&return=" + encodeURIComponent(ret) });
    b.doc.getElementById("fd-welcome-name").value = "River";
    b.doc.getElementById("fd-welcome-join").click();
    assert.deepStrictEqual(b.replaced, ["/chat#joined"], "return=" + ret + " is not followed");
  }
  const plain = await boot();
  assert.strictEqual(plain.doc.getElementById("fd-welcome").hidden, true, "no ?welcome=1, no welcome step");
});

// ── The Devlog ──

test("11. the Devlog reads this server's /api/releases, a page at a time, and never GitHub's API", async () => {
  const src = fs.readFileSync(path.join(WEB, "pages", "devlog-app.js"), "utf8") + fs.readFileSync(path.join(WEB, "pages", "devlog.html"), "utf8");
  assert.ok(!/api\.github\.com/.test(src), "the Devlog names api.github.com");
  const pages = {
    first: { releases: updates(3), next: 41 },
    second: { releases: updates(5).slice(3), next: null },
  };
  const doc = H.fakeDocument();
  for (const [id, tag] of [["devlog-list", "div"], ["devlog-more", "button"], ["devlog-status", "p"]]) {
    const e = doc.createElement(tag);
    e.setAttribute("id", id);
    doc.body.appendChild(e);
  }
  const net = network({ "/api/releases": (u) => (u.includes("before=41") ? pages.second : pages.first) });
  const page = DEVLOG.createDevlog({ document: doc, fetch: net.fetch, t, hint, icon: () => "", fd: FD, now: () => 1_792_800_000_000 });
  await page.start();
  await H.settle();
  const list = doc.getElementById("devlog-list");
  assert.strictEqual(byClass(list, "update-card").length, 3);
  assert.strictEqual(byClass(list, "update-card-signed").length, 1, "the signed release is dressed its own way");
  assert.strictEqual(byClass(list, "update-card-signed-mark")[0].textContent, "Signed");
  const more = doc.getElementById("devlog-more");
  assert.strictEqual(more.hidden, false, "older releases remain");
  more.click();
  await H.settle();
  assert.strictEqual(byClass(list, "update-card").length, 5);
  assert.strictEqual(byClass(list, "update-card-signed").length, 2);
  assert.strictEqual(more.hidden, true, "the end");
  assert.deepStrictEqual(net.asked, ["/api/releases?limit=25", "/api/releases?limit=25&before=41"]);
  assert.strictEqual(doc.getElementById("devlog-status").hidden, true);

  const down = H.fakeDocument();
  for (const [id, tag] of [["devlog-list", "div"], ["devlog-more", "button"], ["devlog-status", "p"]]) {
    const e = down.createElement(tag);
    e.setAttribute("id", id);
    down.body.appendChild(e);
  }
  const d = DEVLOG.createDevlog({ document: down, fetch: network({ "/api/releases": 500 }).fetch, t, hint, icon: () => "", fd: FD });
  await d.start();
  await H.settle();
  assert.strictEqual(down.getElementById("devlog-status").textContent, EN.devlog.offline);
});

// Red first, 2026-11-02, each break made in a copy of web/ (HOS_WEB_DIR) by
// scratchpad's fdbuild-webred.js, the rest passing:
//  - front-door.js updateCard ignoring u.signed (signed = false): 3 and 11 failed ("[] / ['v0.1547.0',
//    'v0.1543.0']").
//  - front-door.js cardLayout never swiping (fit >= 0): 4 failed ("[4, 4, 3, 2, 2, 2, 1, 1, 1]").
//  - front-door.js isReleasePage answering true: 5 failed (the evil link was on the card).
//  - front-door.js groupsOf without the same-words rows: 7 failed ("diarrhea: results").
//  - index-app.js progress() answering {}: 8 failed ("Step 1 of 4").
//  - index-app.js following ?return= as it was before (no same-site check): 10 failed
//    ("return=javascript:alert(document.domain) is not followed").
//  - devlog-app.js asking https://api.github.com/repos/Shaostoul/Humanity/releases again: 11 failed.

test("12. the search's suggestions float over the page and close on a press outside it", () => {
  // The operator, 2026-11-04: "it moves everything else on the page down". Red before the fix:
  // .fd-results had no position, so it sat in the page's flow under the box.
  const html = fs.readFileSync(path.join(ROOT, "web", "pages", "index.html"), "utf8");
  const rule = (html.match(/\.fd-results\s*\{[^}]*\}/) || [""])[0];
  assert.match(rule, /position:\s*absolute/, "the suggestions are out of the page's flow");
  assert.match(rule, /z-index:\s*var\(--z-page-float\)/, "on the layer scale, over the page");
  assert.match(html, /\.fd-search\s*\{[^}]*position:\s*relative/, "placed under the search box");
  const js = fs.readFileSync(path.join(ROOT, "web", "pages", "index-app.js"), "utf8");
  assert.ok(js.includes("addEventListener('pointerdown'") && js.includes(".fd-search"), "a press outside the search closes them");
});

test("13. Start here opens its question right under the button, not at the page's foot", () => {
  // The operator, 2026-11-04: "The new start here button just takes me back to the same page." The
  // section sat after the updates, 3,700 px down, and the button's jump read as nothing happening.
  // Red before the move: the start section came after the updates' section.
  const html = fs.readFileSync(path.join(ROOT, "web", "pages", "index.html"), "utf8");
  const header = html.indexOf("</header>");
  const startAt = html.indexOf('<section id="start"');
  const menu = html.indexOf('id="fd-menu"');
  const updates = html.indexOf('aria-labelledby="fd-updates-h"');
  assert.ok(header > 0 && startAt > header && startAt < menu && startAt < updates, "the question sits right after the header, above the cards and the updates");
  assert.match(html.slice(startAt, startAt + 120), /\shidden>/, "hidden until the button opens it");
  assert.match(html, /id="fd-start"[^>]*aria-controls="start"[^>]*aria-expanded="false"/, "the button says what it opens");
  const js = fs.readFileSync(path.join(ROOT, "web", "pages", "index-app.js"), "utf8");
  assert.ok(/function openStart\(\)[\s\S]{0,200}sec\.hidden = false/.test(js), "the button shows it");
  assert.ok(/stored\(GOAL_KEY\);\s*if \(saved\) \{ openStart\(\);/.test(js), "a choice made before shows it at once");
});
