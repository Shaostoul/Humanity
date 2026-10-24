// The dev MCP bridge (scripts/mcp-server.js) over real stdio, against a double of the game's
// endpoint. Pure node, no game, no GPU, about a second: in `just rig-tests`.
//
// What it proves: the bridge answers both eras of the protocol (a legacy initialize handshake
// and a modern server/discover with a refused version); it lists its own tools and every game
// tool of src/engine/mcp/tools.json; a game tool is forwarded to the endpoint the game wrote,
// with its token and the 2026-07-28 headers, and the picture comes back as image content; with
// no game up (or only a dead one's file) a game tool says how to start one; the pin reset sends
// exactly the rigs' own list; game_boot runs the rigs' launcher with only the two switches added.
//
// RED FIRST (2026-10-22): run against a bridge whose callGame skipped the Authorization header,
// whose bootCommand left out --keep-open, and whose makeHost listed only its own tools:
//   ✖ a game tool is forwarded with the token and the headers, and the picture comes back
//       AssertionError: the fake game saw Authorization undefined
//   ✖ lists its own tools and every game tool of tools.json
//       missing game_screenshot, game_player_state, ...
//   ✖ game_boot runs probe-sweep, keeps the game open, and adds only the two switches
//       args lack --keep-open
// (quoted in docs/ai/mcp.md with the Rust side's red run.)

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const http = require("node:http");
const { spawn } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const BRIDGE = path.join(REPO, "scripts", "mcp-server.js");
const GAME_TOOLS = JSON.parse(fs.readFileSync(path.join(REPO, "src", "engine", "mcp", "tools.json"), "utf8")).tools;
const { STICKY_PIN_RESETS, DIAG_RESETS } = require("../lib/showcase-pins.js");

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "hos-mcp-bridge-test-"));
process.on("exit", () => fs.rmSync(tmp, { recursive: true, force: true }));

/** A bridge on real stdio, its endpoint file and rig pointed into the temp folder. */
function startBridge(endpointFile) {
  const child = spawn(process.execPath, [BRIDGE], {
    cwd: REPO,
    env: { ...process.env, HUMANITY_MCP_ENDPOINT: endpointFile, HUMANITY_MCP_RIG: path.join(tmp, "rig"), HUMANITY_MCP_IDLE_MIN: "0" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let buf = "";
  const waiting = new Map();
  child.stdout.on("data", (d) => {
    buf += d.toString();
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      const m = JSON.parse(line); // every stdout line must be one JSON-RPC message
      if (m.id !== undefined && waiting.has(m.id)) {
        waiting.get(m.id)(m);
        waiting.delete(m.id);
      }
    }
  });
  let next = 1;
  const request = (method, params) =>
    new Promise((resolve) => {
      const id = next++;
      waiting.set(id, resolve);
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  const notify = (method, params) => child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
  const stop = () => child.stdin.end();
  return { request, notify, stop, child };
}

const modern = (extra = {}) => ({ ...extra, _meta: { "io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {} } });

/** A double of the game's endpoint: answers tools/call as the game would and records what it saw. */
function fakeGame(answer) {
  const seen = [];
  const server = http.createServer((req, res) => {
    let body = "";
    req.on("data", (d) => (body += d));
    req.on("end", () => {
      const msg = JSON.parse(body);
      seen.push({ headers: req.headers, msg });
      const result = answer(msg.params.name, msg.params.arguments);
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { ...result, resultType: "complete" } }));
    });
  });
  return new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      const file = path.join(tmp, `endpoint-${port}.json`);
      fs.writeFileSync(
        file,
        JSON.stringify({ ok: true, url: `http://127.0.0.1:${port}/mcp`, port, token: "t0k3n", pid: process.pid, version: "0.0.test", game_dir: tmp })
      );
      resolve({ server, seen, file });
    })
  );
}

test("answers a legacy initialize handshake and a modern discovery, and refuses an unknown modern version", async () => {
  const b = startBridge(path.join(tmp, "none.json"));
  try {
    const init = await b.request("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "t", version: "0" } });
    assert.strictEqual(init.result.protocolVersion, "2025-06-18");
    assert.ok(init.result.capabilities.tools);
    assert.strictEqual(init.result.serverInfo.name, "humanityos-dev");
    b.notify("notifications/initialized", {});
    const d = await b.request("server/discover", modern());
    assert.ok(d.result.supportedVersions.includes("2026-07-28") && d.result.supportedVersions.includes("2025-11-25"));
    assert.strictEqual(d.result.resultType, "complete");
    const bad = await b.request("tools/list", { _meta: { "io.modelcontextprotocol/protocolVersion": "2099-01-01" } });
    assert.strictEqual(bad.error.code, -32022);
    assert.strictEqual(bad.error.data.requested, "2099-01-01");
    const unknown = await b.request("tools/call", { name: "game_fly_to_mars", arguments: {} });
    assert.strictEqual(unknown.error.code, -32602);
  } finally {
    b.stop();
  }
});

test("lists its own tools and every game tool of tools.json", async () => {
  const b = startBridge(path.join(tmp, "none.json"));
  try {
    const l = await b.request("tools/list", modern());
    const names = l.result.tools.map((t) => t.name);
    const missing = GAME_TOOLS.map((t) => t.name).filter((n) => !names.includes(n));
    assert.deepStrictEqual(missing, [], `missing ${missing.join(", ")}`);
    for (const own of ["game_boot", "game_status", "game_close", "game_showcase_reset", "game_goto_vantage", "dev_rig_vantages", "dev_rig_list", "dev_rig_run", "dev_lib_test", "dev_lints", "dev_journal", "dev_priorities"]) {
      assert.ok(names.includes(own), `missing ${own}`);
    }
    assert.strictEqual(new Set(names).size, names.length, "a tool is listed twice");
    for (const t of l.result.tools) assert.strictEqual(t.inputSchema.type, "object", t.name);
    assert.strictEqual(l.result.ttlMs, 0);
  } finally {
    b.stop();
  }
});

test("a game tool is forwarded with the token and the headers, and the picture comes back", async () => {
  const g = await fakeGame((name) => ({
    content: [
      { type: "text", text: `Screenshot saved: C:\\rig\\debug\\screenshot_3.png` },
      { type: "image", data: Buffer.from("not really a jpeg").toString("base64"), mimeType: "image/jpeg" },
    ],
    isError: false,
    structuredContent: { path: "C:\\rig\\debug\\screenshot_3.png", called: name },
  }));
  const b = startBridge(g.file);
  try {
    const r = await b.request("tools/call", { name: "game_screenshot", arguments: { max_inline_px: 800 } });
    assert.strictEqual(r.result.isError, false, JSON.stringify(r));
    const img = r.result.content.find((c) => c.type === "image");
    assert.ok(img && img.mimeType === "image/jpeg", "the image block came through");
    assert.strictEqual(g.seen.length, 1);
    const h = g.seen[0].headers;
    assert.strictEqual(h.authorization, "Bearer t0k3n", `the fake game saw Authorization ${h.authorization}`);
    assert.strictEqual(h["mcp-method"], "tools/call");
    assert.strictEqual(h["mcp-name"], "game_screenshot");
    assert.strictEqual(h["mcp-protocol-version"], "2026-07-28");
    assert.strictEqual(g.seen[0].msg.params._meta["io.modelcontextprotocol/protocolVersion"], "2026-07-28");
    assert.deepStrictEqual(g.seen[0].msg.params.arguments, { max_inline_px: 800 });
    assert.strictEqual(r.result.resultType, undefined, "a legacy client gets a legacy-shaped result");
  } finally {
    b.stop();
    g.server.close();
  }
});

test("game_showcase_reset sends exactly the rigs' sticky pin resets", async () => {
  const g = await fakeGame(() => ({ content: [{ type: "text", text: "Taken." }], isError: false }));
  const b = startBridge(g.file);
  try {
    const r = await b.request("tools/call", { name: "game_showcase_reset", arguments: {} });
    assert.strictEqual(r.result.isError, false);
    assert.strictEqual(g.seen[0].msg.params.name, "game_showcase");
    assert.deepStrictEqual(g.seen[0].msg.params.arguments, { verbs: { ...DIAG_RESETS, ...STICKY_PIN_RESETS } });
  } finally {
    b.stop();
    g.server.close();
  }
});

test("with no game up, or only a dead game's file, a game tool says how to start one", async () => {
  const dead = path.join(tmp, "dead.json");
  fs.writeFileSync(dead, JSON.stringify({ ok: true, url: "http://127.0.0.1:9/mcp", token: "x", pid: 2147483000 }));
  for (const file of [path.join(tmp, "none.json"), dead]) {
    const b = startBridge(file);
    try {
      const r = await b.request("tools/call", { name: "game_player_state", arguments: {} });
      assert.strictEqual(r.result.isError, true);
      assert.match(r.result.content[0].text, /game_boot/);
      const s = await b.request("tools/call", { name: "game_status", arguments: {} });
      assert.strictEqual(s.result.structuredContent.endpoint, null);
    } finally {
      b.stop();
    }
  }
});

test("a game that refused its endpoint is reported with its reason", async () => {
  const refused = path.join(tmp, "refused.json");
  fs.writeFileSync(refused, JSON.stringify({ ok: false, refused: "this game is not a portable sandbox", pid: process.pid }));
  const b = startBridge(refused);
  try {
    const r = await b.request("tools/call", { name: "game_screenshot", arguments: {} });
    assert.strictEqual(r.result.isError, true);
    assert.match(r.result.content[0].text, /not a portable sandbox/);
  } finally {
    b.stop();
  }
});

test("game_boot runs probe-sweep, keeps the game open, and adds only the two switches", () => {
  const { bootCommand } = require("../mcp-server.js");
  const env = { PATH: "x", SOME: "y" };
  const c = bootCommand({ vantage: "home-clock-noon", waitMin: 7, rig: "R", env });
  assert.strictEqual(c.cmd, process.execPath);
  assert.ok(c.args[0].endsWith(path.join("scripts", "probe-sweep.js")));
  const flag = (f) => c.args[c.args.indexOf(f) + 1];
  assert.ok(c.args.includes("--keep-open"), "args lack --keep-open");
  assert.ok(c.args.includes("--game-only"));
  assert.strictEqual(flag("--only"), "home-clock-noon");
  assert.strictEqual(flag("--rig"), "R");
  assert.strictEqual(flag("--wait-min"), "7");
  // Only the endpoint switch and the frame-cost drop are added; everything else is the caller's,
  // unchanged (so no focus or other launch switch can ride in through here).
  const added = Object.keys(c.env).filter((k) => !(k in env)).sort();
  assert.deepStrictEqual(added, ["HUMANITY_DEV_MCP", "HUMANITY_FRAME_COSTS"]);
  for (const k of Object.keys(env)) assert.strictEqual(c.env[k], env[k]);
});

test("the dev tools read the tree: vantages, rigs, journal and priorities", async () => {
  const b = startBridge(path.join(tmp, "none.json"));
  try {
    const v = await b.request("tools/call", { name: "dev_rig_vantages", arguments: { filter: "home-overview" } });
    assert.ok(v.result.structuredContent.ids.includes("home-overview-noon"), JSON.stringify(v.result.structuredContent));
    const rigs = await b.request("tools/call", { name: "dev_rig_list", arguments: {} });
    const names = rigs.result.structuredContent.rigs.map((r) => r.name);
    for (const r of ["probe-sweep", "verify-runtime", "verify-storeys", "verify-starts"]) assert.ok(names.includes(r), `rig ${r} missing from ${names}`);
    assert.deepStrictEqual(rigs.result.structuredContent.rigs.find((r) => r.name === "verify-starts").flags, ["--starts"]);
    const j = await b.request("tools/call", { name: "dev_journal", arguments: { entries: 2 } });
    assert.strictEqual(j.result.structuredContent.recent_decisions.length, 2);
    const p = await b.request("tools/call", { name: "dev_priorities", arguments: { lines: 10 } });
    assert.match(p.result.content[0].text, /^## TIER 0/);
    // A rig is refused while a game is up: one GPU.
    const g = await fakeGame(() => ({ content: [], isError: false }));
    const b2 = startBridge(g.file);
    try {
      const r = await b2.request("tools/call", { name: "dev_rig_run", arguments: { rig: "verify-runtime" } });
      assert.strictEqual(r.result.isError, true);
      assert.match(r.result.content[0].text, /game_close/);
    } finally {
      b2.stop();
      g.server.close();
    }
  } finally {
    b.stop();
  }
});
