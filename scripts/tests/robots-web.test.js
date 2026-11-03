// The website's robots.txt and sitemap (2026-11-02, docs/design/front-door.md 4.5): search engines
// are kept off the server's API, the uploads and a person's own pages, and pointed at the public
// pages. The server's half, X-Robots-Tag: noindex on every /api/ answer, is held by the relay's
// tests (src/relay/front_door_tests.rs, every_api_answer_is_kept_out_of_search_indexes).
//
//   node --test scripts/tests/robots-web.test.js       (in `just rig-tests`)
//
// The rules are read the way Google and Bing read them (RFC 9309): the group for "*", the longest
// matching Allow or Disallow wins, Allow on a tie. HOS_WEB_DIR points it at another copy of web/.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const http = require("node:http");
const { spawn } = require("node:child_process");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const SITEMAP = require(path.join(ROOT, "scripts", "gen-sitemap.js"));
const PAGES = JSON.parse(fs.readFileSync(SITEMAP.DATA, "utf8"));
const ROBOTS = fs.readFileSync(path.join(WEB, "robots.txt"), "utf8");

/** The rules of the group for every crawler ("*"). */
function starRules(text) {
  const rules = [];
  let agents = [];
  let inStar = false;
  let lastWasAgent = false;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.replace(/#.*$/, "").trim();
    if (!line) continue;
    const m = /^([A-Za-z-]+)\s*:\s*(.*)$/.exec(line);
    if (!m) continue;
    const field = m[1].toLowerCase();
    if (field === "user-agent") {
      if (!lastWasAgent) agents = [];
      agents.push(m[2].trim());
      inStar = agents.includes("*");
      lastWasAgent = true;
      continue;
    }
    lastWasAgent = false;
    if (inStar && (field === "allow" || field === "disallow")) rules.push({ allow: field === "allow", path: m[2].trim() });
  }
  return rules;
}

/** Whether a crawler may fetch `url` under `rules` (longest match wins, Allow on a tie). */
function allowed(rules, url) {
  let best = null;
  for (const r of rules) {
    if (!r.path || !url.startsWith(r.path)) continue;
    if (!best || r.path.length > best.path.length || (r.path.length === best.path.length && r.allow)) best = r;
  }
  return !best || best.allow;
}

const RULES = starRules(ROBOTS);

test("1. robots.txt keeps every crawler off the API, the uploads and a person's own pages", () => {
  const off = [
    "/api/messages?channel=global", "/api/front-door", "/api/profile/abc", "/api/files/file/3",
    "/uploads/a.png", "/ws", "/releases/HumanityOS-windows-x64.exe",
    "/chat", "/chat/index.html", "/settings", "/settings.html", "/identity", "/recovery", "/profile?key=ab",
    "/notes", "/wallet", "/admin", "/ops", "/dev", "/dev.html", "/bugs", "/widgets",
  ];
  for (const u of off) assert.strictEqual(allowed(RULES, u), false, u + " must be off limits");
  const on = ["/", "/library", "/library.html", "/how-to", "/devlog", "/devlog.html", "/wallet-guide", "/roadmap", "/files/12", "/data/library/index.json", "/shared/theme.css", "/sitemap.xml"];
  for (const u of on) assert.strictEqual(allowed(RULES, u), true, u + " must be open");
  assert.match(ROBOTS, /^Sitemap: https:\/\/united-humanity\.us\/sitemap\.xml$/m, "the Sitemap line");
  assert.ok(RULES.length > 5, "the * group was read");
});

test("2. the sitemap is what data/site/public_pages.json says, every page in it exists and may be fetched", () => {
  const have = fs.readFileSync(path.join(WEB, "sitemap.xml"), "utf8").replace(/\r\n/g, "\n");
  assert.strictEqual(have, SITEMAP.build(PAGES), "web/sitemap.xml is out of date: run node scripts/gen-sitemap.js");
  for (const want of ["/", "/library", "/how-to", "/roadmap", "/devlog"]) {
    assert.ok(PAGES.pages.some((p) => p.path === want), "the brief's pages are listed: " + want);
  }
  for (const p of PAGES.pages) {
    const file = p.path === "/" ? "index" : p.path.slice(1);
    assert.ok(fs.existsSync(path.join(WEB, "pages", file + ".html")), p.path + " is a page");
    assert.strictEqual(allowed(RULES, p.path), true, p.path + " is listed but robots.txt keeps crawlers off it");
  }
  assert.strictEqual(new Set(PAGES.pages.map((p) => p.path)).size, PAGES.pages.length, "no page twice");
});

test("2b. AI training crawlers get the public pages under the same rules as everyone (operator, 2026-11-02)", () => {
  // "Allow them.": no crawler is singled out, so GPTBot, ClaudeBot and the rest read the * group,
  // which still keeps them off /api/ (people's posts) and /uploads/.
  const agents = ROBOTS.split(/\r?\n/).map((l) => l.replace(/#.*$/, "").trim()).filter((l) => /^user-agent\s*:/i.test(l));
  assert.deepStrictEqual(agents.map((l) => l.split(":")[1].trim()), ["*"], "one group, for every crawler");
  assert.strictEqual(allowed(RULES, "/library"), true);
  assert.strictEqual(allowed(RULES, "/api/messages?channel=global"), false);
});

test("3. the deploy puts both files at the web root", () => {
  const sync = fs.readFileSync(path.join(ROOT, "scripts", "sync-web-root.sh"), "utf8");
  assert.match(sync, /for f in robots\.txt sitemap\.xml; do\s*\n\s*\[ -f "\$SRC\/web\/\$f" \] && cp "\$SRC\/web\/\$f" "\$DST\/\$f"/, "sync-web-root.sh copies them to the root");
});

test("4. served as nginx serves them, at /robots.txt and /sitemap.xml, as text", async () => {
  const port = 20000 + Math.floor(Math.random() * 20000);
  const child = spawn(process.execPath, [path.join(ROOT, "scripts", "preview-server.js"), String(port)], { stdio: ["ignore", "pipe", "pipe"] });
  try {
    await new Promise((resolve, reject) => {
      const to = setTimeout(() => reject(new Error("the preview server did not start")), 8000);
      child.stdout.on("data", (d) => { if (String(d).includes("preview server on")) { clearTimeout(to); resolve(); } });
      child.on("exit", (c) => { clearTimeout(to); reject(new Error("the preview server exited " + c)); });
    });
    const get = (p) => new Promise((resolve, reject) => {
      http.get({ host: "127.0.0.1", port, path: p }, (res) => {
        let body = "";
        res.on("data", (c) => { body += c; });
        res.on("end", () => resolve({ status: res.statusCode, type: res.headers["content-type"], body }));
      }).on("error", reject);
    });
    const robots = await get("/robots.txt");
    assert.strictEqual(robots.status, 200);
    assert.match(robots.type, /^text\/plain/);
    assert.strictEqual(robots.body.replace(/\r\n/g, "\n"), fs.readFileSync(path.join(ROOT, "web", "robots.txt"), "utf8").replace(/\r\n/g, "\n"));
    const map = await get("/sitemap.xml");
    assert.strictEqual(map.status, 200);
    assert.match(map.type, /xml/);
    assert.match(map.body, /<loc>https:\/\/united-humanity\.us\/library<\/loc>/);
  } finally {
    child.kill();
  }
});

// Red first, 2026-11-02, each in a copy of web/ (HOS_WEB_DIR):
//  - robots.txt without "Allow: /devlog": 1 failed ("/devlog must be open": "Disallow: /dev" took
//    it), and 2 with it (the Devlog is in the sitemap).
//  - robots.txt without "Disallow: /api/": 1 failed ("/api/messages?channel=global must be off limits").
