// The list of files a server runs from outside its checkout, scripts/server-files.txt (BUG-252,
// 2026-11-06), read for node: the tests and `just brief`'s check. scripts/install-server-files.sh
// reads the same file with bash; the format is described at the top of the file.
"use strict";

// { files: [{ role, mode, src, dest }], servers: [{ alias, role, name }] }
function parseManifest(text) {
  const files = [];
  const servers = [];
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const cols = line.split(/\s+/);
    if (cols[0] === "file" && cols.length >= 5) {
      files.push({ role: cols[1], mode: cols[2], src: cols[3], dest: cols[4] });
    } else if (cols[0] === "server" && cols.length >= 4) {
      servers.push({ alias: cols[1], role: cols[2], name: cols[3] });
    }
  }
  return { files, servers };
}

// The lines a server of this role installs.
function filesFor(files, role) {
  return files.filter((f) => f.role === "all" || f.role === role);
}

// The checkout on every server (the deploy's and the follower's REPO).
const SERVER_REPO = "/opt/Humanity";

// The shell a read-only ssh call runs on a server: the commit and tag its checkout is on, then
// the md5 of each installed copy and of the checkout's own copy (the fallback when that commit
// is not in the local repo). Nothing is written. git runs with safe.directory because the
// checkout belongs to the relay's user and ssh comes in as root ("dubious ownership" otherwise).
function remoteScript(files) {
  const g = `git -c safe.directory=${SERVER_REPO} -C ${SERVER_REPO}`;
  const paths = [];
  for (const f of files) paths.push(f.dest, `${SERVER_REPO}/${f.src}`);
  const quoted = paths.map((p) => `'${p.replace(/'/g, "")}'`).join(" ");
  return [
    `echo "HEAD $(${g} rev-parse HEAD 2>/dev/null)"`,
    `echo "TAG $(${g} describe --tags --exact-match 2>/dev/null)"`,
    `for p in ${quoted}; do if [ -f "$p" ]; then md5sum "$p"; else echo "MISSING $p"; fi; done`,
  ].join("; ");
}

// What remoteScript printed: { head, tag, md5: { path: md5 or null when missing } }.
function parseRemote(text) {
  const out = { head: "", tag: "", md5: {} };
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim();
    let m;
    if ((m = line.match(/^HEAD\s*([0-9a-f]*)$/))) out.head = m[1];
    else if ((m = line.match(/^TAG\s*(\S*)$/))) out.tag = m[1];
    else if ((m = line.match(/^MISSING (\S+)$/))) out.md5[m[1]] = null;
    else if ((m = line.match(/^([0-9a-f]{32})\s+\*?(\S+)$/))) out.md5[m[2]] = m[1];
  }
  return out;
}

// One server's verdict. `repoMd5(src)` gives the repo's md5 of a source at the commit the server's
// checkout is on, or null when that commit is not in the local repo; then the server checkout's
// own copy is the reference. Returns { ok, differ: [{ dest, installed, repo }], line }.
function judgeServer(server, files, remote, repoMd5) {
  const mine = filesFor(files, server.role);
  const at = remote.tag || (remote.head ? remote.head.slice(0, 8) : "its checkout");
  const differ = [];
  let fromCheckout = 0;
  for (const f of mine) {
    let repo = repoMd5(f.src);
    if (!repo) {
      repo = remote.md5[`${SERVER_REPO}/${f.src}`] || null;
      fromCheckout++;
    }
    const installed = remote.md5[f.dest];
    if (installed === undefined) {
      differ.push({ dest: f.dest, installed: "not read", repo });
    } else if (installed === null) {
      differ.push({ dest: f.dest, installed: "missing", repo });
    } else if (!repo || installed !== repo) {
      differ.push({ dest: f.dest, installed, repo });
    }
  }
  const against = fromCheckout === mine.length && mine.length ? `the copies in its checkout (${at})` : `${at}'s copies`;
  if (!differ.length) {
    return { ok: true, differ, line: `${server.name}: all ${mine.length} installed files match ${against}` };
  }
  const named = differ
    .map((d) => {
      const was = d.installed.length === 32 ? d.installed.slice(0, 8) : d.installed;
      const want = d.repo ? d.repo.slice(0, 8) : "not in the repo";
      return `${d.dest} (${was}; the repo's ${want})`;
    })
    .join(", ");
  const fix =
    server.role === "main"
      ? "the next deploy installs them"
      : "the follower installs them within the hour once its release carries scripts/install-server-files.sh";
  return {
    ok: false,
    differ,
    line: `STALE: ${server.name}: ${differ.length} of ${mine.length} installed files differ from ${against}: ${named}; ${fix}`,
  };
}

module.exports = { parseManifest, filesFor, remoteScript, parseRemote, judgeServer, SERVER_REPO };
