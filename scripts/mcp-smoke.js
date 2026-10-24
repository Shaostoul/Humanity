#!/usr/bin/env node
// The dev MCP end to end, as a client sees it (2026-10-22): start the bridge
// (scripts/mcp-server.js) on stdio exactly as Claude Code does, then call game_status,
// game_screenshot and game_player_state through it, and print what came back: the picture's
// path on disk, its inline size, and where the player is. docs/ai/mcp.md.
//
//   node scripts/mcp-smoke.js            a game must already be up (game_boot, or a rig with HUMANITY_DEV_MCP=1)
//   node scripts/mcp-smoke.js --boot     boot one first through game_boot (the rigs' launcher; takes minutes)
//   node scripts/mcp-smoke.js --close    close the game at the end (game_close)
//   just mcp-smoke --boot --close        the whole round trip
//
// Exit 0: every call answered and the picture is on disk. 1: refused before anything was
// judged (no game and no --boot, or the boot was refused). 2: a call failed.

"use strict";

const path = require("path");
const fs = require("fs");
const { spawn } = require("child_process");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const BOOT = args.includes("--boot");
const CLOSE = args.includes("--close");

function bridge() {
  const child = spawn(process.execPath, [path.join(REPO, "scripts", "mcp-server.js")], { cwd: REPO, stdio: ["pipe", "pipe", "inherit"] });
  let buf = "";
  const waiting = new Map();
  child.stdout.on("data", (d) => {
    buf += d.toString();
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const m = JSON.parse(buf.slice(0, i));
      buf = buf.slice(i + 1);
      if (m.method === "notifications/progress") console.log(`  ... ${m.params.message}`);
      else if (waiting.has(m.id)) {
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
  return { child, request };
}

const text = (r) => ((r.result && r.result.content) || []).filter((c) => c.type === "text").map((c) => c.text).join("\n");

async function main() {
  const b = bridge();
  const call = async (name, a = {}) => {
    const r = await b.request("tools/call", { name, arguments: a, _meta: { progressToken: name } });
    if (r.error) throw new Error(`${name}: protocol error ${r.error.code} ${r.error.message}`);
    return r;
  };
  let code = 0;
  try {
    const init = await b.request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "mcp-smoke", version: "1" } });
    console.log(`bridge: ${init.result.serverInfo.name} v${init.result.serverInfo.version}, protocol ${init.result.protocolVersion}`);
    const list = await b.request("tools/list", {});
    console.log(`tools: ${list.result.tools.length} (${list.result.tools.map((t) => t.name).join(", ")})`);
    let st = await call("game_status");
    console.log(`game_status: ${text(st)}`);
    if (!(st.result.structuredContent.endpoint && st.result.structuredContent.endpoint.ok)) {
      if (!BOOT) {
        console.log("REFUSED: no game is up; run with --boot to start one through game_boot.");
        return 1;
      }
      console.log("game_boot (the rigs' launcher; waits its turn, then boots in the background) ...");
      const boot = await call("game_boot", {});
      console.log(text(boot).split("\n").slice(0, 3).join("\n"));
      if (boot.result.isError) return 1;
      st = await call("game_status");
    }
    const shot = await call("game_screenshot", {});
    if (shot.result.isError) {
      console.log(`game_screenshot FAILED: ${text(shot)}`);
      code = 2;
    } else {
      const img = shot.result.content.find((c) => c.type === "image");
      const p = shot.result.structuredContent.path;
      console.log(`game_screenshot: ${text(shot)}`);
      console.log(`  inline image: ${img ? `${img.mimeType}, ${Math.round((img.data.length * 3) / 4 / 1024)} KB` : "none"}`);
      console.log(`  on disk: ${p} (${fs.existsSync(p) ? `${Math.round(fs.statSync(p).size / 1024)} KB` : "MISSING"})`);
      if (!fs.existsSync(p)) code = 2;
    }
    const ps = await call("game_player_state", {});
    if (ps.result.isError) {
      console.log(`game_player_state FAILED: ${text(ps)}`);
      code = 2;
    } else {
      const s = ps.result.structuredContent;
      console.log(
        `game_player_state: world_loaded ${s.world_loaded}, aboard ${s.aboard_ship}, camera ${JSON.stringify(s.camera.pos)}, ` +
          `room ${JSON.stringify(s.storey.room)}, health ${s.health}, carrying ${s.carrying.items} item(s) of ${s.carrying.kinds} kind(s)`
      );
    }
    if (CLOSE) console.log(`game_close: ${text(await call("game_close"))}`);
  } catch (e) {
    console.log(`FAILED: ${e.message}`);
    code = 2;
  } finally {
    b.child.stdin.end();
  }
  return code;
}

main().then((c) => {
  console.log(c === 0 ? "OK: the dev MCP answered end to end" : c === 1 ? "REFUSED" : "FAILED");
  process.exitCode = c;
});
