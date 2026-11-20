// Emoji pictures from our own copies (2026-11-20), run under Node:
//
//   node --test scripts/tests/emoji-web.test.js      (in `just rig-tests`)
//
// The operator chose OpenMoji (2026-11-20: "of that list the OpenMoji sounds better") to replace the
// chat's Twemoji, whose pictures every visitor's browser fetched from cdn.jsdelivr.net.
//
// What it proves:
//  1. The pack: assets/emoji/index.json is what scripts/gen-emoji-index.js writes from the folders
//     (fresh); every picture is named by code points and holds shapes only (no script, no event
//     attribute, no link out); every set has its LICENSE.md and a row in data/credits.ron.
//  2. web/shared/emoji.js finds the right picture for a set of tricky sequences: a lone emoji, a
//     text-by-default heart with and without U+FE0F, joined sequences typed with and without
//     U+FE0F (OpenMoji keeps it in sequence names and drops it from single ones), skin tones, a
//     couple with two tones, a flag, a subdivision flag, keycaps, text that must stay text (digits,
//     #, a bare (c), U+FE0E), and the fallbacks when no picture has the whole sequence. The cases
//     are scripts/tests/fixtures/emoji-v1.json, which the desktop's matcher is to answer too.
//  3. In a page: parse() turns the emoji in text nodes into <img class="emoji"> whose alt is the
//     exact text, skips textarea, script and style, changes nothing on a second parse, waits for
//     the index when given an element before it arrives, and every address it asks for is on this
//     server (/assets/emoji/...): no request to any other site.
//  4. The website: no page or script writes an image or script address on another site
//     (src="https://..."), nothing calls Twemoji any more, and the chat loads /shared/emoji.js.
//
// HOS_WEB_DIR points it at another copy of web/ (used to see each test red).
//
// Seen red 2026-11-20 (the broken copies of emoji.js through HOS_WEB_DIR, made by a scratch script):
//  - Test 4 on web/ as it was before this change: 'web/chat/app.js writes an address on another
//    site: src="https://cdn.jsdelivr.net/npm/@twemoji/svg@latest/1fab6.svg', 'web/chat/app.js still
//    calls Twemoji' (and the same for chat-dms, chat-messages, chat-profile, chat-ui,
//    chat-voice-rooms, chat-warnings, view/pillPopup), "web/chat/twemoji.min.js is Twemoji's
//    library".
//  - Test 2, first draft of emoji.js itself: 'the heart on fire typed without U+FE0F' got
//    [txt:"❤<200D>", img:1F525] (a text-by-default first element was not taken as the start of a
//    joined sequence); FIRST gained the minimally-qualified alternative.
//  - Test 2 with keyOf keeping U+FE0F: 'a red heart with U+FE0F' got the text. With filesFrom keeping
//    FE0F in its keys: 'the heart on fire, fully qualified' got [img:2764, txt:"<200D>", img:1F525].
//    With no fallback: 'man and T-rex, no picture of the pair' got [txt:"👨<200D>🦖"].
//  - Test 3 with SKIP emptied: 'each picture's alt is the exact text it stands for' got five
//    pictures (the textarea's, the script's and the style's too). With no pending queue: 'an element
//    given before the index arrives is drawn when it arrives', 0 pictures.
//  - Test 1 with one name (1F600) taken out of index.json: 'index.json is stale: run node
//    scripts/gen-emoji-index.js'. And before the credit row: 'the set openmoji names the credit
//    "openmoji", which is not a row of data/credits.ron'.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const pack = require("../lib/emoji-pack");

function loadModule() {
  // Fresh each time, from WEB (a HOS_WEB_DIR copy when seeing a test red).
  const file = path.join(WEB, "shared", "emoji.js");
  const mod = { exports: {} };
  vm.runInNewContext(fs.readFileSync(file, "utf8"), { module: mod, console }, { filename: file });
  return mod.exports;
}

const INDEX = JSON.parse(fs.readFileSync(path.join(ROOT, pack.INDEX_FILE), "utf8"));

const cp = (...hex) => String.fromCodePoint(...hex.map((h) => parseInt(h, 16)));

/** The fixture's notation: {HEX HEX} stands for those code points, everything else is itself. */
const expand = (s) => s.replace(/\{([0-9A-F ]+)\}/g, (_, hex) => cp(...hex.trim().split(/\s+/)));

/** tokens() in the fixture's shape: ["img", picture name] or ["text", the text]. Array.from: the
 *  module runs in its own vm context, whose arrays deepStrictEqual tells apart. */
function describe(toks) {
  return Array.from(toks, (t) => (t.url ? ["img", path.basename(t.url, ".svg")] : ["text", t.text]));
}

// The cases both apps must answer the same: scripts/tests/fixtures/emoji-v1.json.
const CASES = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", "emoji-v1.json"), "utf8")).cases.map((c) => [
  c.what,
  expand(c.text),
  c.want.map(([kind, v]) => [kind, kind === "text" ? expand(v) : v]),
]);

test("1. the pack: a fresh index, safe pictures named by code points, a licence and a credit for each set", () => {
  const have = fs.readFileSync(path.join(ROOT, pack.INDEX_FILE), "utf8").replace(/\r\n/g, "\n");
  assert.strictEqual(have, pack.indexText(ROOT), "index.json is stale: run node scripts/gen-emoji-index.js");
  const credits = fs.readFileSync(path.join(ROOT, "data", "credits.ron"), "utf8");
  let pictures = 0;
  for (const set of pack.readSets(ROOT)) {
    const dir = path.join(ROOT, pack.EMOJI_DIR, set.dir);
    assert.ok(fs.existsSync(path.join(dir, "LICENSE.md")), `${set.dir}/LICENSE.md: every set says whose pictures they are`);
    assert.ok(credits.includes(`id: "${set.credit}"`), `the set ${set.id} names the credit "${set.credit}", which is not a row of data/credits.ron`);
    for (const name of pack.setNames(ROOT, set)) {
      const problems = pack.checkSvg(fs.readFileSync(path.join(dir, name + ".svg"), "utf8"));
      assert.deepStrictEqual(problems, [], `${set.dir}/${name}.svg`);
      pictures++;
    }
  }
  assert.ok(pictures > 4000, `the pack holds ${pictures} pictures (OpenMoji 17.0.0 has 4,495)`);
  // The checker refuses what it must (a gate that cannot fail proves nothing).
  assert.ok(pack.checkSvg('<svg><script>alert(1)</script></svg>').length);
  assert.ok(pack.checkSvg('<svg><path onload="x()" d=""/></svg>').length);
  assert.ok(pack.checkSvg('<svg><image href="https://example.com/a.png"/></svg>').length);
  assert.ok(pack.checkSvg('<svg><rect fill="url(https://example.com/x)"/></svg>').length);
  assert.deepStrictEqual(pack.checkSvg('<svg viewBox="0 0 72 72"><g clip-path="url(#a)"><path d="M0 0"/></g></svg>'), []);
});

test("2. the right picture for each tricky sequence, and the fallbacks", () => {
  const emoji = loadModule();
  const files = emoji.filesFrom(INDEX);
  for (const [what, text, want] of CASES) {
    assert.deepStrictEqual(describe(emoji.tokens(text, files)), want, what);
  }
  // Every picture it can name exists on disk, under /assets/emoji/ (this server).
  for (const url of files.values()) {
    assert.ok(url.startsWith("/assets/emoji/"), url);
    assert.ok(fs.existsSync(path.join(ROOT, url.slice(1))), `${url} is named but not there`);
  }
  // A set listed first draws its pictures in place of a later set's (the volunteers' set, later).
  const mine = emoji.filesFrom({ sets: [{ dir: "hos", names: "1F600" }, ...INDEX.sets] });
  assert.deepStrictEqual(describe(emoji.tokens(cp("1F600") + cp("1F44D"), mine)), [["img", "1F600"], ["img", "1F44D"]]);
  assert.strictEqual(mine.get("1F600"), "/assets/emoji/hos/1F600.svg");
  assert.strictEqual(mine.get("1F44D"), "/assets/emoji/openmoji/1F44D.svg");
});

// ── A stand-in page, just what emoji.js touches ──────────────────────────────────────────────
class Node {
  constructor(nodeType, nodeName) {
    this.nodeType = nodeType;
    this.nodeName = nodeName;
    this.childNodes = [];
    this.parentNode = null;
    this.attrs = {};
  }
  appendChild(child) {
    if (child.nodeType === 11) {
      for (const c of child.childNodes.splice(0)) this.appendChild(c);
      return child;
    }
    if (child.parentNode) child.parentNode.childNodes.splice(child.parentNode.childNodes.indexOf(child), 1);
    child.parentNode = this;
    this.childNodes.push(child);
    return child;
  }
  replaceChild(next, old) {
    const at = this.childNodes.indexOf(old);
    const list = next.nodeType === 11 ? next.childNodes.splice(0) : [next];
    for (const c of list) c.parentNode = this;
    this.childNodes.splice(at, 1, ...list);
    old.parentNode = null;
    return old;
  }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  get textContent() {
    return this.nodeType === 3 ? this.nodeValue : this.childNodes.map((c) => c.textContent).join("");
  }
}
function makePage() {
  const requests = [];
  let release;
  const indexArrives = new Promise((r) => { release = r; });
  const document = {
    createElement: (tag) => new Node(1, tag.toUpperCase()),
    createTextNode: (text) => Object.assign(new Node(3, "#text"), { nodeValue: text }),
    createDocumentFragment: () => new Node(11, "#document-fragment"),
  };
  const fetch = async (url) => {
    requests.push(url);
    await indexArrives;
    if (url !== "/assets/emoji/index.json") return { ok: false, status: 404 };
    return { ok: true, status: 200, json: async () => INDEX };
  };
  const window = {};
  const ctx = vm.createContext({ window, document, fetch, console, module: undefined });
  const file = path.join(WEB, "shared", "emoji.js");
  vm.runInContext(fs.readFileSync(file, "utf8"), ctx, { filename: file });
  const el = (tag, ...kids) => {
    const e = document.createElement(tag);
    for (const k of kids) e.appendChild(typeof k === "string" ? document.createTextNode(k) : k);
    return e;
  };
  return { emoji: window.hosEmoji, document, el, requests, release };
}
const imgs = (node, out = []) => {
  for (const c of node.childNodes) {
    if (c.nodeName === "IMG") out.push(c);
    imgs(c, out);
  }
  return out;
};

test("3. in a page: pictures in text, skipped elements left alone, idempotent, the index awaited, only this server asked", async () => {
  const page = makePage();
  const { el, emoji } = page;
  // An element given before the index arrives is drawn when it arrives.
  const early = el("div", "early " + cp("1F600"));
  emoji.parse(early);
  assert.strictEqual(imgs(early).length, 0, "nothing can be drawn before the index arrives");
  page.release();
  assert.strictEqual(await emoji.ready, true, "the index loads");
  assert.strictEqual(imgs(early).length, 1, "an element given before the index arrives is drawn when it arrives");

  const msg = el("div",
    el("span", "Nice " + cp("1F44D", "1F3FD") + "! " + cp("2764", "FE0F", "200D", "1F525")),
    el("textarea", "draft " + cp("1F600")),
    el("script", "var s = '" + cp("1F600") + "';"),
    el("style", "/* " + cp("1F600") + " */"),
    el("b", "plain text, 10 # items"));
  emoji.parse(msg);
  const pics = imgs(msg);
  assert.deepStrictEqual(pics.map((i) => i.getAttribute("alt")), [cp("1F44D", "1F3FD"), cp("2764", "FE0F", "200D", "1F525")], "each picture's alt is the exact text it stands for");
  assert.deepStrictEqual(pics.map((i) => i.getAttribute("src")), ["/assets/emoji/openmoji/1F44D-1F3FD.svg", "/assets/emoji/openmoji/2764-FE0F-200D-1F525.svg"]);
  for (const i of pics) assert.strictEqual(i.className, "emoji", "the chat's img.emoji style sizes it to the text");
  assert.strictEqual(msg.childNodes[0].textContent, "Nice ! ", "the words around them stay");
  assert.strictEqual(imgs(msg.childNodes[1]).length, 0, "the textarea's text stays text");
  assert.strictEqual(imgs(msg.childNodes[2]).length + imgs(msg.childNodes[3]).length, 0, "script and style text stays text");
  emoji.parse(msg);
  assert.strictEqual(imgs(msg).length, 2, "a second parse changes nothing");

  // A picture that fails to load goes back to its text.
  pics[0].onerror.call(pics[0]);
  assert.strictEqual(imgs(msg).length, 1);
  assert.ok(msg.childNodes[0].textContent.includes(cp("1F44D", "1F3FD")), "the failed picture's text is back");

  // Every address asked for is on this server.
  const asked = [...page.requests, ...imgs(early).map((i) => i.getAttribute("src")), ...imgs(msg).map((i) => i.getAttribute("src"))];
  for (const a of asked) assert.ok(a.startsWith("/assets/emoji/"), `asked another place: ${a}`);
  assert.deepStrictEqual(page.requests, ["/assets/emoji/index.json"], "one request for the index, none per guess");
});

test("4. the website: no address on another site for an image or script, no Twemoji, the chat loads emoji.js", () => {
  const offenders = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) { walk(full); continue; }
      if (!/\.(js|html|css)$/.test(e.name)) continue;
      const rel = path.relative(WEB, full).split(path.sep).join("/");
      if (/twemoji/i.test(e.name)) offenders.push(`web/${rel} is Twemoji's library`);
      const text = fs.readFileSync(full, "utf8");
      for (const m of text.matchAll(/src=\\?["']https?:\/\/[^"'\\]*/g)) offenders.push(`web/${rel} writes an address on another site: ${m[0]}`);
      if (/\btwemoji\s*\.\s*(parse|replace|test|base|convert)|window\.twemoji/.test(text)) offenders.push(`web/${rel} still calls Twemoji`);
    }
  };
  walk(WEB);
  assert.deepStrictEqual(offenders, []);
  const chat = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  assert.match(chat, /<script src="\/shared\/emoji\.js(\?v=\d+)?"><\/script>/, "the chat loads /shared/emoji.js");
});
