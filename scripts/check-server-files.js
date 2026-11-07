#!/usr/bin/env node
/**
 * check-server-files.js: does each server run the repo's copy of every file it runs from outside
 * its checkout? (BUG-252, 2026-11-06; `just brief`'s SCRIPTS row, `just server-files`.)
 *
 * Why: public.guide restarted every 30 minutes for two days because its /usr/local/bin backup
 * script was a stale copy installed by hand, and nothing anywhere said so: the checkout was
 * current, the relay was current, only the copy outside the checkout was months old. The list of
 * such files is scripts/server-files.txt (the installer reads it too); the servers to check are its
 * `server` lines.
 *
 * READ ONLY. One ssh call per server (BatchMode, so it never asks for a password, and a short
 * connect timeout), which prints the commit its checkout is on and the md5 of each installed copy
 * and of the checkout's own copy. Each installed copy is compared with the repo's copy at that
 * commit (read from the local repo with git cat-file, as committed, so a Windows checkout's line
 * endings do not matter), or with the server checkout's copy when that commit is not here.
 *
 * Usage: node scripts/check-server-files.js   one line per server; exit 1 if any file differs
 *                                             or a server could not be read.
 */
"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { spawn, execFileSync } = require("child_process");
const { parseManifest, filesFor, remoteScript, parseRemote, judgeServer } = require("./lib/server-files.js");

const ROOT = path.join(__dirname, "..");

function sshRead(alias, script) {
  return new Promise((resolve) => {
    const p = spawn("ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", alias, script], { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    const timer = setTimeout(() => p.kill(), 30000);
    p.stdout.on("data", (d) => (out += d));
    p.stderr.on("data", (d) => (err += d));
    p.on("error", (e) => {
      clearTimeout(timer);
      resolve({ ok: false, out, err: String(e.message) });
    });
    p.on("close", (code) => {
      clearTimeout(timer);
      resolve({ ok: code === 0 && /^HEAD /m.test(out), out, err });
    });
  });
}

// The repo's md5 of `src` at `commit`, as committed; null when the commit is not in this repo.
function repoMd5At(commit) {
  return (src) => {
    if (!/^[0-9a-f]{40}$/.test(commit || "")) return null;
    try {
      const blob = execFileSync("git", ["cat-file", "blob", `${commit}:${src}`], { cwd: ROOT, stdio: ["ignore", "pipe", "ignore"] });
      return crypto.createHash("md5").update(blob).digest("hex");
    } catch {
      return null;
    }
  };
}

async function main() {
  const { files, servers } = parseManifest(fs.readFileSync(path.join(ROOT, "scripts", "server-files.txt"), "utf8"));
  const results = await Promise.all(
    servers.map(async (s) => {
      const r = await sshRead(s.alias, remoteScript(filesFor(files, s.role)));
      if (!r.ok) {
        const why = (r.err.trim().split("\n").pop() || "no answer").slice(0, 120);
        return { ok: false, line: `UNREACHABLE: ${s.name}: could not read its installed files over ssh ${s.alias} (${why})` };
      }
      const remote = parseRemote(r.out);
      return judgeServer(s, files, remote, repoMd5At(remote.head));
    }),
  );
  for (const r of results) console.log(r.line);
  process.exit(results.every((r) => r.ok) ? 0 : 1);
}

if (require.main === module) main();
