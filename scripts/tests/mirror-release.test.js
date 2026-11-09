// The release mirror's puller (scripts/vps/humanity-mirror-release, BUG-288, 2026-11-09), run for
// real under Python against a fake GitHub on 127.0.0.1, and the manifest script it ends with
// (scripts/vps/regen-releases-manifest), run for real on the scratch folder.
//
// Run: node --test scripts/tests/mirror-release.test.js (in `just rig-tests`). Needs Python 3.9+
// (`python3` or `python`; HOS_PYTHON names another). About 15 s.
// Live check against GitHub (not in rig-tests, it needs the network): HOS_MIRROR_LIVE=1 runs the
// dry run on the real v0.1552.0 release, which fetches the API listing and the 305,320-byte data
// manifest only, into a temp folder.
//
// The release workflow used to PUSH 4.4 GB to the server with appleboy/scp-action, and for
// v0.1552.0 that step hung for 1 h 42 min, twice; the server's own long downloads from GitHub's CDN
// slowed to 168 KB/s on their first connection while a fresh one got 5.3 MB/s. The server now PULLS
// each file itself, dropping a crawling connection and resuming on a fresh one, checks every file
// against GitHub's SHA-256, and publishes the folder with one rename only when all match. The fake
// GitHub here answers the two API calls the puller makes (the release by tag, then its assets, ten
// to a page so the paging is used) and serves each file through a redirect, as GitHub does, with
// Range support; a test can make a file stall, crawl, arrive corrupt, close early, or come from a
// server that ignores Range. The puller runs from this checkout with HUMANITY_RELEASES_DIR,
// HUMANITY_MIRROR_API and HUMANITY_REGEN_CMD pointed into a scratch world; the regen command is
// the real regen-releases-manifest.
//
// Each case was seen red once against a deliberately broken copy (HOS_MIRROR_SCRIPT, or
// HOS_REGEN_SCRIPT for the manifest script, pointed at the broken copy; Windows, 2026-11-09); its
// comment says which break and what the red run showed. The file-mode check inside `if (!WINDOWS)`
// has not run yet (Windows has no Unix modes; CI does not run rig-tests).

const test = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const ROOT = path.join(__dirname, "..", "..");
const PULLER = process.env.HOS_MIRROR_SCRIPT || path.join(ROOT, "scripts", "vps", "humanity-mirror-release");
const REGEN = process.env.HOS_REGEN_SCRIPT || path.join(ROOT, "scripts", "vps", "regen-releases-manifest");
const WINDOWS = process.platform === "win32";
const TAG = "v0.9.0";

// ── Python ──────────────────────────────────────────────────────────────────────────────────────

function findPython() {
  const names = process.env.HOS_PYTHON ? [process.env.HOS_PYTHON] : WINDOWS ? ["python", "python3"] : ["python3", "python"];
  for (const name of names) {
    const r = spawnSync(name, ["-c", "import sys; print(sys.executable); print(sys.version_info >= (3, 9))"], { encoding: "utf8" });
    if (r.status === 0) {
      const [exe, ok] = r.stdout.trim().split(/\r?\n/);
      if (ok === "True") return exe;
    }
  }
  throw new Error("these tests need Python 3.9 or newer (python3 or python on PATH, or HOS_PYTHON)");
}
const PYTHON = findPython();
// The regen command as the puller splits it (shlex): each part single-quoted, which keeps a Windows
// path's backslashes as they are.
const REGEN_CMD = `'${PYTHON}' '${REGEN}'`;

// ── A fake GitHub ───────────────────────────────────────────────────────────────────────────────

const sha256 = (buf) => crypto.createHash("sha256").update(buf).digest("hex");
// Deterministic content per name, so a run is reproducible.
function content(name, size) {
  const out = Buffer.alloc(size);
  let block = crypto.createHash("sha256").update(name).digest();
  for (let off = 0; off < size; off += block.length) {
    block.copy(out, off, 0, Math.min(block.length, size - off));
    block = crypto.createHash("sha256").update(block).digest();
  }
  return out;
}

// The 14 files the mirror holds (four platforms' program and two archives, the data archive and
// manifest), and three it does not: the headless relay and its .sha256, and a file added later.
function releaseFiles(tag) {
  const mirrored = [`data-manifest-${tag}.json`, `HumanityOS-data-${tag}.tar.gz`];
  for (const label of ["windows-x64", "linux-x64", "macos-arm64", "macos-x64"]) {
    mirrored.push(label.startsWith("windows") ? `HumanityOS-${label}.exe` : `HumanityOS-${label}`);
    mirrored.push(`HumanityOS-${label}.tar.gz`, `HumanityOS-${label}.zip`);
  }
  const others = ["HumanityOS-relay-linux-x64", "HumanityOS-relay-linux-x64.sha256", "release-manifest.json"];
  const files = new Map();
  [...mirrored, ...others].forEach((name, i) => {
    const size = name.startsWith("data-manifest") ? 3000 : 20000 + i * 7919;
    files.set(name, { name, data: content(name, size) });
  });
  return { mirrored: mirrored.slice().sort(), files };
}

class FakeGitHub {
  constructor() {
    this.releases = new Map(); // tag -> { id, files: Map, assetOverrides }
    this.behaviour = {}; // name -> how the CDN serves it
    this.conns = {}; // name -> CDN connections so far
    this.requests = []; // { path, range }
    this.rateLimited = false;
    this.cdnDelayMs = 0; // every CDN answer waits this long, so overlapping fetches can be seen
    this.inflight = 0; // CDN answers being served right now
    this.maxInflight = 0; // the most at once
  }

  addRelease(tag, { omit = [], overrides = {} } = {}) {
    const { mirrored, files } = releaseFiles(tag);
    for (const name of omit) files.delete(name);
    this.releases.set(tag, { id: 1000 + this.releases.size, files, overrides });
    return { mirrored: mirrored.filter((n) => !omit.includes(n)), files };
  }

  asset(tag, file, overrides) {
    return {
      name: file.name,
      size: file.data.length,
      state: "uploaded",
      digest: `sha256:${sha256(file.data)}`,
      browser_download_url: `${this.base}/download/${tag}/${encodeURIComponent(file.name)}`,
      ...(overrides[file.name] || {}),
    };
  }

  start() {
    this.server = http.createServer((req, res) => this.handle(req, res));
    return new Promise((resolve) =>
      this.server.listen(0, "127.0.0.1", () => {
        this.base = `http://127.0.0.1:${this.server.address().port}`;
        resolve();
      }),
    );
  }

  stop() {
    this.server.closeAllConnections();
    return new Promise((resolve) => this.server.close(resolve));
  }

  cdnRequests() {
    return this.requests.filter((r) => r.path.startsWith("/cdn/"));
  }

  handle(req, res) {
    const url = new URL(req.url, this.base);
    this.requests.push({ path: url.pathname, range: req.headers.range || null });
    const json = (status, body, headers = {}) => {
      res.writeHead(status, { "Content-Type": "application/json", ...headers });
      res.end(JSON.stringify(body));
    };
    let m;
    if ((m = /^\/api\/releases\/tags\/([^/]+)$/.exec(url.pathname))) {
      if (this.rateLimited) {
        const reset = Math.floor(Date.now() / 1000) + 1800;
        return json(403, { message: "API rate limit exceeded" }, {
          "X-RateLimit-Limit": "60", "X-RateLimit-Remaining": "0", "X-RateLimit-Reset": String(reset),
        });
      }
      const rel = this.releases.get(decodeURIComponent(m[1]));
      if (!rel) return json(404, { message: "Not Found" });
      return json(200, { id: rel.id, tag_name: m[1], assets_url: `${this.base}/api/releases/${rel.id}/assets` });
    }
    if ((m = /^\/api\/releases\/(\d+)\/assets$/.exec(url.pathname))) {
      const [tag, rel] = [...this.releases].find(([, r]) => String(r.id) === m[1]) || [];
      if (!rel) return json(404, { message: "Not Found" });
      const all = [...rel.files.values()].map((f) => this.asset(tag, f, rel.overrides));
      const page = Number(url.searchParams.get("page") || 1);
      const per = 10; // fewer than GitHub's 100, so a 17-file release takes two pages
      const headers = {};
      if (page * per < all.length) headers.Link = `<${this.base}${url.pathname}?per_page=100&page=${page + 1}>; rel="next"`;
      return json(200, all.slice((page - 1) * per, page * per), headers);
    }
    if ((m = /^\/download\/([^/]+)\/([^/]+)$/.exec(url.pathname))) {
      // GitHub answers a release download with a redirect to its CDN.
      res.writeHead(302, { Location: `${this.base}/cdn/${m[1]}/${m[2]}?sig=fresh` });
      return res.end();
    }
    if ((m = /^\/cdn\/([^/]+)\/([^/]+)$/.exec(url.pathname))) {
      const rel = this.releases.get(decodeURIComponent(m[1]));
      const file = rel && rel.files.get(decodeURIComponent(m[2]));
      if (!file) return json(404, { message: "Not Found" });
      this.inflight += 1;
      this.maxInflight = Math.max(this.maxInflight, this.inflight);
      res.on("close", () => (this.inflight -= 1));
      if (this.cdnDelayMs) return setTimeout(() => this.serve(req, res, file), this.cdnDelayMs);
      return this.serve(req, res, file);
    }
    json(404, { message: "Not Found" });
  }

  serve(req, res, file) {
    const name = file.name;
    const conn = (this.conns[name] = (this.conns[name] || 0) + 1);
    const b = this.behaviour[name] || {};
    let body = file.data;
    if (b.corrupt === "always" || (b.corrupt === "once" && conn === 1)) {
      body = Buffer.from(body);
      body[body.length >> 1] ^= 0xff; // same size, wrong bytes
    }
    let start = 0;
    const range = /^bytes=(\d+)-$/.exec(req.headers.range || "");
    if (range && !b.ignoreRange) {
      start = Number(range[1]);
      if (start >= body.length) {
        res.writeHead(416, { "Content-Range": `bytes */${body.length}` });
        return res.end();
      }
      res.writeHead(206, { "Content-Length": body.length - start, "Content-Range": `bytes ${start}-${body.length - 1}/${body.length}` });
    } else {
      res.writeHead(200, { "Content-Length": body.length });
    }
    const rest = body.subarray(start);
    if (b.stallAfter !== undefined && conn <= (b.stallConns || 1)) {
      res.write(rest.subarray(0, b.stallAfter)); // then silence, the connection held open
      return;
    }
    if (b.trickle && conn <= (b.trickleConns || 1)) {
      // A crawl, not a silence: 64 bytes every 100 ms, until the client hangs up.
      let off = 0;
      const timer = setInterval(() => {
        if (off >= rest.length) {
          clearInterval(timer);
          return res.end();
        }
        res.write(rest.subarray(off, off + (b.trickleBytes || 64)));
        off += b.trickleBytes || 64;
      }, 100);
      res.on("close", () => clearInterval(timer));
      return;
    }
    if (b.closeAfter !== undefined && conn === 1) {
      res.write(rest.subarray(0, b.closeAfter), () => req.socket.destroy());
      return;
    }
    res.end(rest);
  }
}

// ── A scratch world and a run ───────────────────────────────────────────────────────────────────

const scratch = [];
const servers = [];
test.after(async () => {
  for (const s of servers) await s.stop();
  for (const d of scratch) fs.rmSync(d, { recursive: true, force: true });
});

async function world(opts) {
  const gh = new FakeGitHub();
  await gh.start();
  servers.push(gh);
  const release = gh.addRelease(TAG, opts);
  const releases = fs.mkdtempSync(path.join(os.tmpdir(), "hos-mirror-"));
  scratch.push(releases);
  return { gh, release, releases, published: path.join(releases, TAG), staging: path.join(releases, `.${TAG}.partial`) };
}

// Fast settings for loopback: a connection that crawls under 20 KB/s for 1 s, or is silent for 1 s,
// is dropped; no pauses between tries.
const FAST = ["--speed-time", "1", "--speed-limit", "20000", "--retry-wait", "0", "--tries", "5"];

// The tag is v0.9.0 unless the arguments start with one.
function runPuller(w, args, { env = {}, timeout = 30000 } = {}) {
  if (!args.length || args[0].startsWith("-")) args = [TAG, ...args];
  return new Promise((resolve) => {
    const child = spawn(PYTHON, [PULLER, ...args], {
      env: {
        ...process.env,
        HUMANITY_RELEASES_DIR: w.releases,
        HUMANITY_MIRROR_API: `${w.gh.base}/api`,
        HUMANITY_REGEN_CMD: REGEN_CMD,
        ...env,
      },
    });
    let out = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (out += d));
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeout);
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code: timedOut ? "timed out" : code, out, pid: child.pid });
    });
    w.child = child;
  });
}

const listDir = (d) => (fs.existsSync(d) ? fs.readdirSync(d).sort() : []);
const manifestOf = (w) => JSON.parse(fs.readFileSync(path.join(w.releases, "manifest.json"), "utf8"));
function assertPublished(w) {
  assert.deepStrictEqual(listDir(w.published), w.release.mirrored, "the published folder holds exactly the mirror's 14 files");
  for (const name of w.release.mirrored) {
    assert.ok(fs.readFileSync(path.join(w.published, name)).equals(w.release.files.get(name).data), `${name} is the release's file, byte for byte`);
  }
  assert.ok(!fs.existsSync(w.staging), "the staging folder was renamed into place");
}

// ── The cases ───────────────────────────────────────────────────────────────────────────────────

// Seen red with the relay's exclusion taken out of the platform pattern (`(?!relay-|data-)` became
// `(?!data-)` and `[a-z0-9_]+` became `[a-z0-9_-]+`): the relay's program was taken as a platform
// of its own, and the run refused the release, "not ready: the relay-linux-x64 tar.gz is missing"
// (exit 4, and so did 12 other cases). Seen red with the paging taken out (`url =
// next_link(headers)` became `url = None`): only the first ten assets were seen, "not ready: the
// macos-arm64 zip is missing" (exit 4).
test("a release becomes exactly the mirror's 14 files, every byte checked, and the manifest lists it", async () => {
  const w = await world();
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  for (const skipped of ["HumanityOS-relay-linux-x64", "HumanityOS-relay-linux-x64.sha256", "release-manifest.json"]) {
    assert.match(r.out, new RegExp(`skipping ${skipped.replace(/[.]/g, "\\.")} \\(not part of the mirror\\)`));
  }
  assert.match(r.out, /\[mirror v0\.9\.0\] RESULT: ok: published 14 files/);
  const listed = manifestOf(w).releases;
  assert.deepStrictEqual(listed.map((x) => x.tag), [TAG], "the real regen-releases-manifest ran and lists the release");
  assert.deepStrictEqual(listed[0].assets.map((a) => a.name).sort(), w.release.mirrored);
  // Every download went through GitHub's redirect and was asked for once.
  assert.strictEqual(w.gh.cdnRequests().length, 14, "one connection per file on a healthy path");
});

// SEVERAL FILES AT ONCE (2026-11-09): v0.1553.0's 5.3 GB fetched one file at a time, each
// connection at about 1 MB/s from the server to GitHub's CDN, ran out of the workflow's 50
// minutes. With --parallel 4 the CDN sees four answers in flight together; with --parallel 1 only
// ever one, and the release is the same 14 verified files either way.
// Seen red 2026-11-09 with the pool's max_workers pinned to 1: "four files are fetched at once: 1".
test("--parallel fetches several files at once, and --parallel 1 one at a time", async () => {
  const many = await world();
  many.gh.cdnDelayMs = 300;
  const r = await runPuller(many, [...FAST, "--parallel", "4"]);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(many);
  assert.strictEqual(many.gh.maxInflight, 4, `four files are fetched at once: ${many.gh.maxInflight}`);

  const one = await world();
  one.gh.cdnDelayMs = 50;
  const r1 = await runPuller(one, [...FAST, "--parallel", "1"]);
  assert.strictEqual(r1.code, 0, r1.out);
  assertPublished(one);
  assert.strictEqual(one.gh.maxInflight, 1, "one file at a time");

  const bad = await world();
  const r0 = await runPuller(bad, [...FAST, "--parallel", "0"]);
  assert.strictEqual(r0.code, 2, r0.out);
  assert.match(r0.out, /--parallel must be from 1 to 16/);
});

// Seen red with the socket timeout taken out (`timeout=args.speed_time` became `timeout=None` in
// one_connection): the run hung on the silent connection until this test's 30 s limit ("timed
// out"). Seen red with the Range header taken out (`headers["Range"] = ...` became a no-op):
// "resumed where it stopped" failed, the second connection asked for no range.
test("a connection that goes silent is dropped and the file resumed where it stopped, through the redirect", async () => {
  const w = await world();
  const victim = "HumanityOS-linux-x64.tar.gz";
  const half = w.release.files.get(victim).data.length >> 1;
  w.gh.behaviour[victim] = { stallAfter: half };
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  const asks = w.gh.cdnRequests().filter((q) => q.path.endsWith(victim));
  assert.strictEqual(asks.length, 2, r.out);
  assert.strictEqual(asks[1].range, `bytes=${half}-`, "resumed where it stopped");
  assert.match(r.out, /HumanityOS-linux-x64\.tar\.gz: dropped a crawling connection \(nothing arrived for 1 s\)/);
});

// BUG-288's own symptom: not silence, a crawl. Seen red with the speed check taken out (`if rate <
// args.speed_limit:` became `if False:`): the run sat on the 640 B/s connection until this test's
// 30 s limit ("timed out"), as the scp step sat for 1 h 42 min.
test("a connection that crawls under the speed floor is dropped and resumed on a fresh one", async () => {
  const w = await world();
  const victim = "HumanityOS-windows-x64.zip";
  w.gh.behaviour[victim] = { trickle: true };
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  const asks = w.gh.cdnRequests().filter((q) => q.path.endsWith(victim));
  assert.strictEqual(asks.length, 2, r.out);
  assert.match(asks[1].range || "", /^bytes=\d+-$/, "the fresh connection resumed");
  assert.match(r.out, /HumanityOS-windows-x64\.zip: dropped a crawling connection \(\d+ B\/s over the last 1 s, under the 20\.0 KB\/s floor\)/);
});

// Seen red with the 200 branch's reset taken out (`have, mode = 0, "wb"` became `mode = "ab"`):
// the whole file was appended to the half already there, and only the size check rescued it with a
// third connection ("not appended: 3 connections"; before the count was asserted this case passed
// on that broken copy). Also red with the socket timeout taken out ("timed out").
test("a server that ignores the resume request gets the file from the start, not appended", async () => {
  const w = await world();
  const victim = "HumanityOS-macos-arm64";
  w.gh.behaviour[victim] = { stallAfter: 5000, ignoreRange: true };
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  assert.match(r.out, /HumanityOS-macos-arm64: the server ignored the request to resume; starting it again/);
  // Two connections: the stalled one, then the whole file from the start. A third would mean the
  // whole file was appended to the half and only the size check saved it.
  const asks = w.gh.cdnRequests().filter((q) => q.path.endsWith(`/${victim}`));
  assert.strictEqual(asks.length, 2, `the restarted file was written from the start, not appended: ${asks.length} connections`);
});

// Seen red with the Range header taken out (`headers["Range"] = ...` became `pass`): "two
// connections, the second a resume" failed, the second asked for no range and refetched the file
// whole.
test("a connection cut off early is resumed", async () => {
  const w = await world();
  const victim = "HumanityOS-data-v0.9.0.tar.gz";
  w.gh.behaviour[victim] = { closeAfter: 7000 };
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  const asks = w.gh.cdnRequests().filter((q) => q.path.endsWith(victim));
  assert.deepStrictEqual(asks.map((q) => q.range), [null, "bytes=7000-"], "two connections, the second a resume");
});

// Seen red with the SHA-256 comparison taken out of check_file (`if got != asset["sha256"]:`
// became `if False:`): the corrupt file was published and the run said "RESULT: ok: published 14
// files" (exit 0).
test("a file that never matches its SHA-256 is never published: exit 1, the staging kept, no manifest", async () => {
  const w = await world();
  const victim = "HumanityOS-linux-x64";
  w.gh.behaviour[victim] = { corrupt: "always" };
  const r = await runPuller(w, [...FAST.slice(0, -2), "--tries", "3"]);
  assert.strictEqual(r.code, 1, r.out);
  assert.ok(!fs.existsSync(w.published), "nothing published");
  assert.ok(fs.existsSync(w.staging), "the staging folder is kept for a rerun");
  assert.ok(!fs.existsSync(path.join(w.releases, "manifest.json")), "the manifest was not rebuilt");
  assert.strictEqual(w.gh.cdnRequests().filter((q) => q.path.endsWith(`/${victim}`)).length, 3, "three connections, as --tries said");
  assert.match(r.out, /RESULT: failed \(exit 1\): could not fetch and verify HumanityOS-linux-x64 in 3 connections \(last: it did not match the release: SHA-256/);
});

// Seen red with run() made to throw away what was staged (the "already fetched and verified" check
// replaced by `if os.path.exists(dest): os.remove(dest)`): "the rerun fetches only what was not
// verified" failed, HumanityOS-data-v0.9.0.tar.gz was fetched again. Also red with the SHA-256
// check taken out (the first run published, exit 0).
test("a rerun after a failure fetches only what was not yet verified, then publishes", async () => {
  const w = await world();
  const victim = "HumanityOS-macos-x64.tar.gz";
  w.gh.behaviour[victim] = { corrupt: "always" };
  const first = await runPuller(w, [...FAST.slice(0, -2), "--tries", "1"]);
  assert.strictEqual(first.code, 1, first.out);
  const verifiedFirst = listDir(w.staging).filter((n) => n !== victim);
  assert.ok(verifiedFirst.length > 0, "some files were verified before the failure");
  delete w.gh.behaviour[victim];
  const before = w.gh.cdnRequests().length;
  const second = await runPuller(w, FAST);
  assert.strictEqual(second.code, 0, second.out);
  assertPublished(w);
  const fetchedAgain = w.gh.cdnRequests().slice(before).map((q) => decodeURIComponent(q.path.split("/").pop()));
  for (const name of verifiedFirst) assert.ok(!fetchedAgain.includes(name), `the rerun fetches only what was not verified: ${name} was fetched again`);
  assert.strictEqual(fetchedAgain.length, 14 - verifiedFirst.length);
});

// Seen red with both published-folder checks taken out (before and after the lock, each `if
// os.path.isdir(published):` became `if False:`): the second run fetched all 14 files again and
// then could not rename onto the existing folder ("RESULT: failed (exit 1): a file operation
// failed ([WinError 183] ...").
test("a rerun on a published release verifies it, fetches nothing and rebuilds the manifest", async () => {
  const w = await world();
  assert.strictEqual((await runPuller(w, FAST)).code, 0);
  fs.rmSync(path.join(w.releases, "manifest.json"));
  const before = w.gh.cdnRequests().length;
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 0, r.out);
  assert.strictEqual(w.gh.cdnRequests().length, before, "nothing fetched");
  assert.match(r.out, /RESULT: ok: already published and verified \(14 files/);
  assert.deepStrictEqual(manifestOf(w).releases.map((x) => x.tag), [TAG], "the manifest was rebuilt");
});

// Seen red with the published folder's verification skipped (in verify_published, the filtered
// `bad = [...]` became `bad = []`): the run said "RESULT: ok: already published and verified" over
// the altered and the missing file (exit 0). Also red with both published-folder checks taken out
// (exit 1, after fetching everything again beside the folder).
test("a published folder that does not match is named file by file and left exactly as it is", async () => {
  const w = await world();
  fs.mkdirSync(w.published);
  for (const name of w.release.mirrored) fs.writeFileSync(path.join(w.published, name), w.release.files.get(name).data);
  fs.writeFileSync(path.join(w.published, "HumanityOS-linux-x64.zip"), "not the release's zip");
  fs.rmSync(path.join(w.published, "HumanityOS-windows-x64.exe"));
  const snapshot = () => listDir(w.published).map((n) => `${n}:${sha256(fs.readFileSync(path.join(w.published, n)))}`);
  const before = snapshot();
  const r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 7, r.out);
  assert.deepStrictEqual(snapshot(), before, "the published folder is untouched");
  assert.match(r.out, /published HumanityOS-linux-x64\.zip: 21 bytes, the release's is \d+/);
  assert.match(r.out, /published HumanityOS-windows-x64\.exe: missing/);
  assert.strictEqual(w.gh.cdnRequests().length, 0, "nothing fetched");
});

// Seen red with the per-platform check taken out of select_assets (`for label, kinds in
// sorted(platforms.items()):` became `for label, kinds in []:`): the release without the macOS x64
// zip was published as 13 files (exit 0). Seen red with the upload-state check taken out (`if
// asset.get("state") != "uploaded":` became `if False:`): the file still uploading was not named.
test("a release missing a file, or one not uploaded or without a digest, is not ready: exit 4, nothing fetched", async () => {
  const missing = await world({ omit: ["HumanityOS-macos-x64.zip"] });
  let r = await runPuller(missing, FAST);
  assert.strictEqual(r.code, 4, r.out);
  assert.match(r.out, /not ready: the macos-x64 zip is missing/);
  assert.strictEqual(missing.gh.cdnRequests().length, 0);
  assert.ok(!fs.existsSync(missing.published) && !fs.existsSync(missing.staging));

  const unchecked = await world({
    overrides: { "HumanityOS-linux-x64.zip": { digest: null }, "HumanityOS-data-v0.9.0.tar.gz": { state: "starter" } },
  });
  r = await runPuller(unchecked, FAST);
  assert.strictEqual(r.code, 4, r.out);
  assert.match(r.out, /not ready: HumanityOS-linux-x64\.zip has no SHA-256 digest on GitHub/);
  assert.match(r.out, /not ready: HumanityOS-data-v0\.9\.0\.tar\.gz is not uploaded yet \(state 'starter'\)/);
  assert.strictEqual(unchecked.gh.cdnRequests().length, 0);
});

// Seen red with the 404 mapping taken out (`not_found_is=...` became `not_found_is=None`): an unknown
// tag ended "GitHub's API answered .../releases/tags/v9.9.9 with HTTP 404" (exit 6). Seen red with
// the rate-limit reading taken out (`if headers.get("X-RateLimit-Remaining") == "0":` became `if
// False:`): exit 6, "GitHub's API refused ...: HTTP 403", no reset time.
test("no such release is exit 3; GitHub's rate limit is exit 5 with the time it resets", async () => {
  const w = await world();
  let r = await runPuller(w, ["v9.9.9", ...FAST]);
  assert.strictEqual(r.code, 3, r.out);
  assert.match(r.out, /RESULT: failed \(exit 3\): GitHub has no release tagged v9\.9\.9/);
  w.gh.rateLimited = true;
  r = await runPuller(w, FAST);
  assert.strictEqual(r.code, 5, r.out);
  assert.match(r.out, /rate limit for this server's address is used up \(60 requests an hour without a token\); it resets at \d\d:\d\d:\d\d UTC \(in about 30 min\)/);
});

// Seen red with the tag check taken out of run() (`if not TAG_RE.match(tag) or ".." in tag:` became
// `if False:`): "../etc" went on to ask GitHub's API (exit 3, "no release tagged ../etc").
test("a tag that is not a tag is refused before anything is asked or written: exit 2", async () => {
  const w = await world();
  for (const bad of ["../etc", "v0.1/../../x", "latest", ".v0.9.0"]) {
    const r = await runPuller(w, [bad, ...FAST]);
    assert.strictEqual(r.code, 2, `${bad}: ${r.out}`);
    assert.match(r.out, /is not a release tag/);
  }
  assert.strictEqual(w.gh.requests.length, 0, "nothing asked");
  assert.deepStrictEqual(listDir(w.releases), [], "nothing written");
});

// Seen red with the dry run's branch skipped (`if args.dry_run:` became `if False:`): the "dry run"
// went on past the data manifest to fetch HumanityOS-data-v0.9.0.tar.gz and the rest. Seen red with
// its clean-up skipped (`if made_staging:` became `if False:`): ".v0.9.0.partial" was left behind;
// and with every staging folder counted as its own (`made_staging = True`): the real run's
// part-fetched folder was deleted (the listing came back empty).
test("the dry run fetches and verifies the data manifest only, publishes nothing, leaves nothing", async () => {
  const w = await world();
  const r = await runPuller(w, [TAG, "--dry-run", ...FAST]);
  assert.strictEqual(r.code, 0, r.out);
  assert.deepStrictEqual(w.gh.cdnRequests().map((q) => q.path), [`/cdn/${TAG}/data-manifest-${TAG}.json`]);
  assert.match(r.out, new RegExp(`data-manifest-v0\\.9\\.0\\.json: verified \\(3000 bytes, sha256 matches\\)`));
  assert.deepStrictEqual(listDir(w.releases), [".mirror-release.lock"], "the staging folder it made is gone again");
  assert.match(r.out, /RESULT: ok: dry run: 14 files .* would be mirrored; nothing published, manifest not rebuilt/);
  // The plan names every file with its size and digest.
  for (const name of w.release.mirrored) assert.ok(r.out.includes(`  ${name}  `), `the plan names ${name}`);
  // A real run's part-fetched staging folder is kept by a dry run, and gains the verified manifest.
  fs.mkdirSync(w.staging);
  fs.writeFileSync(path.join(w.staging, "HumanityOS-linux-x64"), "part of a real run");
  const again = await runPuller(w, [TAG, "--dry-run", ...FAST]);
  assert.strictEqual(again.code, 0, again.out);
  assert.deepStrictEqual(listDir(w.staging), ["HumanityOS-linux-x64", `data-manifest-${TAG}.json`].sort());
});

// Seen red with the lock made a no-op (`_try_lock` returning True at once): the second run did not
// refuse; it fetched into the same staging folder beside the first and ended on the crawling file
// (exit 1, "could not fetch and verify HumanityOS-windows-x64.tar.gz in 5 connections"), not 8.
// Seen red with the check after the lock taken out (`if os.path.isdir(published):` inside the lock
// became `if False:`): the third run, having waited, fetched everything again and could not rename
// onto the first's folder (exit 1, "[WinError 183] ...").
test("a second run while one holds the lock waits, and with --lock-wait 0 refuses: exit 8", async () => {
  const w = await world();
  const slow = "HumanityOS-windows-x64.tar.gz";
  // The first run's slowest file arrives at 640 B/s with the speed floor off: about 5 s holding the lock.
  w.gh.behaviour[slow] = { trickle: true, trickleConns: 99, trickleBytes: 64 };
  w.release.files.get(slow).data = w.release.files.get(slow).data.subarray(0, 3200);
  const first = runPuller(w, [...FAST, "--speed-limit", "0", "--speed-time", "10"]);
  const firstChild = w.child;
  // Wait until the first run is fetching (it holds the lock from before its first fetch).
  for (let i = 0; i < 100 && !w.gh.cdnRequests().length; i++) await new Promise((res) => setTimeout(res, 100));
  assert.ok(w.gh.cdnRequests().length, "the first run started fetching");
  const second = await runPuller(w, [...FAST, "--lock-wait", "0"]);
  assert.strictEqual(second.code, 8, second.out);
  assert.match(second.out, new RegExp(`another run holds the lock \\(pid ${firstChild.pid} mirroring v0\\.9\\.0 since `));
  // A third, willing to wait: it waits for the first, then finds the folder the first published,
  // verifies it and fetches nothing.
  const third = runPuller(w, [...FAST, "--lock-wait", "30"]);
  const r = await first;
  assert.strictEqual(r.code, 0, r.out);
  assertPublished(w);
  const t = await third;
  assert.strictEqual(t.code, 0, t.out);
  assert.match(t.out, new RegExp(`waiting for another run to finish \\(pid ${firstChild.pid} mirroring v0\\.9\\.0`));
  assert.match(t.out, /exists \(another run published it while this one waited\); verifying it/);
  assert.match(t.out, /RESULT: ok: already published and verified/);
  // The first fetched each file once on one connection (its speed floor is off); the second and
  // the third fetched nothing.
  assert.strictEqual(w.gh.cdnRequests().length, 14, "only the first run fetched");
});

// Seen red with the deadline taken out (Budget.out_of_time returning False, and the check in
// one_connection's read loop made `if False:`): the crawl ran to the end of the file and the run
// published (exit 0 after 5 s) instead of stopping at --max-minutes.
test("--max-minutes stops a run that is taking too long: exit 1, resumable", async () => {
  const w = await world();
  const slow = "data-manifest-v0.9.0.json";
  w.gh.behaviour[slow] = { trickle: true, trickleConns: 99 };
  const r = await runPuller(w, [...FAST, "--speed-limit", "0", "--speed-time", "10", "--max-minutes", "0.02"]);
  assert.strictEqual(r.code, 1, r.out);
  assert.match(r.out, /RESULT: failed \(exit 1\): ran out of time \(--max-minutes 0\.02\) fetching data-manifest-v0\.9\.0\.json; the staging folder is kept/);
  assert.ok(!fs.existsSync(w.published));
});

// Seen red with the regen command's exit code ignored (`if r.returncode != 0:` became `if False:`):
// "RESULT: ok: ... manifest rebuilt" (exit 0) when it had not been.
test("a manifest command that fails is exit 9, the files published", async () => {
  const w = await world();
  const r = await runPuller(w, FAST, { env: { HUMANITY_REGEN_CMD: `'${PYTHON}' -c "import sys; sys.exit(3)"` } });
  assert.strictEqual(r.code, 9, r.out);
  assertPublished(w);
  assert.match(r.out, /RESULT: failed \(exit 9\): the files are published but .* failed \(exit 3\)/);
});

// regen-releases-manifest skips a staging folder: before this, a run that did not finish would have
// put its half-fetched ".<tag>.partial" folder in the public manifest as a release (and the disk
// guard's `v*` and nginx's listing already skip dot names, which is why the staging folder has one).
// Seen red with the filter taken out of is_release_dir (`return os.path.isdir(...)` alone, the
// manifest script as it was): the manifest listed "v0.9.2.partial" and ".v0.9.1.partial" beside
// v0.9.0, the dot one sorted as the newest release.
test("the manifest lists published releases only, never a staging folder", () => {
  const releases = fs.mkdtempSync(path.join(os.tmpdir(), "hos-regen-"));
  scratch.push(releases);
  for (const dir of ["v0.9.0", ".v0.9.1.partial", "v0.9.2.partial"]) {
    fs.mkdirSync(path.join(releases, dir));
    fs.writeFileSync(path.join(releases, dir, "HumanityOS-linux-x64"), "x");
  }
  fs.writeFileSync(path.join(releases, ".mirror-release.lock"), "pid 1\n");
  const r = spawnSync(PYTHON, [REGEN], { encoding: "utf8", env: { ...process.env, HUMANITY_RELEASES_DIR: releases } });
  assert.strictEqual(r.status, 0, `${r.stdout}${r.stderr}`);
  const listed = JSON.parse(fs.readFileSync(path.join(releases, "manifest.json"), "utf8")).releases.map((x) => x.tag);
  assert.deepStrictEqual(listed, ["v0.9.0"]);
});

// The published files must be readable by nginx whatever umask the ssh session had (a 077 umask
// would have made every download a 403). Not yet run: Windows has no Unix file modes.
test("published files are 644 and the folder 755, even under umask 077", { skip: WINDOWS && "Unix file modes only" }, async () => {
  const w = await world();
  const r = await new Promise((resolve) => {
    const child = spawn("sh", ["-c", 'umask 077; exec "$0" "$@"', PYTHON, PULLER, TAG, ...FAST], {
      env: { ...process.env, HUMANITY_RELEASES_DIR: w.releases, HUMANITY_MIRROR_API: `${w.gh.base}/api`, HUMANITY_REGEN_CMD: REGEN_CMD },
    });
    let out = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (out += d));
    child.on("close", (code) => resolve({ code, out }));
  });
  assert.strictEqual(r.code, 0, r.out);
  assert.strictEqual(fs.statSync(w.published).mode & 0o777, 0o755);
  for (const name of w.release.mirrored) assert.strictEqual(fs.statSync(path.join(w.published, name)).mode & 0o777, 0o644, name);
});

// The real thing, by hand: HOS_MIRROR_LIVE=1. Fetches GitHub's listing of v0.1552.0 and its
// 305,320-byte data manifest into a temp folder, never the archives.
test("live: the dry run against the real v0.1552.0 release", { skip: !process.env.HOS_MIRROR_LIVE && "set HOS_MIRROR_LIVE=1 (needs the network)" }, async () => {
  const releases = fs.mkdtempSync(path.join(os.tmpdir(), "hos-mirror-live-"));
  scratch.push(releases);
  const r = await new Promise((resolve) => {
    const env = { ...process.env, HUMANITY_RELEASES_DIR: releases };
    delete env.HUMANITY_MIRROR_API;
    const child = spawn(PYTHON, [PULLER, "v0.1552.0", "--dry-run"], { env });
    let out = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (out += d));
    child.on("close", (code) => resolve({ code, out }));
  });
  console.log(r.out);
  assert.strictEqual(r.code, 0, r.out);
  assert.match(r.out, /14 files to mirror/);
  assert.match(r.out, /skipping HumanityOS-relay-linux-x64 \(not part of the mirror\)/);
  assert.match(r.out, /skipping HumanityOS-relay-linux-x64\.sha256 \(not part of the mirror\)/);
  assert.match(r.out, /data-manifest-v0\.1552\.0\.json: verified \(305320 bytes, sha256 matches\)/);
  assert.match(r.out, /RESULT: ok: dry run: 14 files \(4\.5 GB\) would be mirrored/);
  assert.deepStrictEqual(listDir(releases), [".mirror-release.lock"], "nothing left behind");
});
