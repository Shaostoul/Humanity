#!/usr/bin/env node
/**
 * check-server-upkeep.js: is each server keeping up with its security updates, and does an open
 * call port there have a name to offer? (`just brief`'s SERVERS row, `just server-upkeep`.)
 *
 * Why: on 2026-11-20 public.guide was found with 55 security updates waiting for months (BUG-321:
 * its automatic updates matched no Debian archive) and with its call port open while its relay
 * told callers to use united-humanity.us's forwarder (BUG-322: no TURN_PUBLIC_HOST in its .env).
 * Nothing anywhere said either. The checks and their thresholds are scripts/lib/server-upkeep.js;
 * the servers are scripts/server-files.txt's `server` lines.
 *
 * READ ONLY. One ssh call per server (BatchMode, so it never asks for a password, a short connect
 * timeout, and killed after 40 s), which simulates an upgrade and greps the settings files for key
 * NAMES only, never printing a value.
 *
 * Usage: node scripts/check-server-upkeep.js   one line per server; exit 1 if any server has a
 *                                              problem or could not be read.
 */
"use strict";

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const { parseManifest } = require("./lib/server-files.js");
const { remoteScript, parseRemote, judgeServer } = require("./lib/server-upkeep.js");

const ROOT = path.join(__dirname, "..");

function sshRead(alias, script) {
  return new Promise((resolve) => {
    const p = spawn("ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", alias, script], { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    const timer = setTimeout(() => p.kill(), 40000);
    p.stdout.on("data", (d) => (out += d));
    p.stderr.on("data", (d) => (err += d));
    p.on("error", (e) => {
      clearTimeout(timer);
      resolve({ ok: false, out, err: String(e.message) });
    });
    p.on("close", (code) => {
      clearTimeout(timer);
      // grep's "nothing found" makes the last command's status 1, so the answer's own first line
      // is what says the script ran, not the exit code.
      resolve({ ok: /^UPKEEP 1$/m.test(out), out, err: err || (code === null ? "timed out" : `exit ${code}`) });
    });
  });
}

async function main() {
  const { servers } = parseManifest(fs.readFileSync(path.join(ROOT, "scripts", "server-files.txt"), "utf8"));
  const script = remoteScript();
  const results = await Promise.all(
    servers.map(async (s) => {
      const r = await sshRead(s.alias, script);
      if (!r.ok) {
        const why = (r.err.trim().split("\n").pop() || "no answer").slice(0, 120);
        return { ok: false, line: `could not check ${s.name} over ssh ${s.alias} (${why})` };
      }
      return judgeServer(s, parseRemote(r.out));
    }),
  );
  for (const r of results) console.log(r.line);
  process.exit(results.every((r) => r.ok) ? 0 : 1);
}

if (require.main === module) main();
