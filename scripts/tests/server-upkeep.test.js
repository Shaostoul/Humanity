// Each server's upkeep, as `just brief`'s SERVERS row reads it over ssh (scripts/lib/server-upkeep.js,
// scripts/check-server-upkeep.js; BUG-321 and BUG-322, 2026-11-20).
//
// Run: node --test scripts/tests/server-upkeep.test.js (in `just rig-tests`).
//
// public.guide sat on 55 waiting security updates for months, and later had its call port open
// while its relay sent callers to united-humanity.us's forwarder, and nothing said either. These
// cases prove each warning fires on the answer a server gives (captured from both servers on
// 2026-11-20, then altered to the states the two bugs left), stays quiet on a healthy answer, and
// that the script the servers run prints no setting's value.
//
// Seen red 2026-11-20 against deliberately broken copies of the library (HOS_SERVER_UPKEEP_LIB),
// one break each: with the comparison made `sec > 500`, "more than five waiting security updates
// is a warning" failed ("public.guide: 55 security updates waiting (3 others); ..." judged ok);
// with the open-port check taken out, "an open call port with no public name is a warning" and
// "the firewall unreadable, the node.env switch decides" failed ("... call port UDP 3478 open,
// undefined set" judged ok); with the name grep printing its match, "the script the server runs
// never prints a setting's value" failed ("the answer carried calls.secret-name"); and with the
// lists' age check taken out, "package lists not refreshed for days are a warning" failed. The
// ALLOWED_ORIGINS fact (BUG-329), the same way: with it always "set", "a healthy answer is one quiet
// line" failed on united-humanity.us's captured answer; with its grep printing its match, "the
// answer carried origins.secret-name".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

// HOS_SERVER_UPKEEP_LIB: a deliberately broken copy of the library, for the red runs.
const lib = require(process.env.HOS_SERVER_UPKEEP_LIB || "../lib/server-upkeep.js");
const { remoteScript, parseRemote, judgeServer, SECURITY_LIMIT } = lib;

const MAIN = { alias: "humanity-vps", name: "united-humanity.us" };
const FOLLOW = { alias: "humanity-vps2", name: "public.guide" };

// united-humanity.us's answer, verbatim, 2026-11-20 (Debian 12: two nginx security updates).
const MAIN_ANSWER = `UPKEEP 1
Inst nginx [1.22.1-9+deb12u10] (1.22.1-9+deb12u11 Debian-Security:12/oldstable-security [amd64]) []
Inst nginx-common [1.22.1-9+deb12u10] (1.22.1-9+deb12u11 Debian-Security:12/oldstable-security [all])
LISTS 1795133962
NOW 1795145751
ENV present
NAMED TURN_PUBLIC_HOST
TURN_PORT=3478
HUMANITY_OPEN_CALL_PORT=1
NFT read
udp dport 3478 accept
`;

// public.guide's answer, 2026-11-20 after both fixes (Debian 13: point-release updates only; the
// first three of its 42 Inst lines, verbatim).
const FOLLOW_STABLE = [
  "Inst libc6-dev [2.41-12+deb13u3] (2.41-12+deb13u4 Debian:13.7/stable [amd64]) []",
  "Inst libc-dev-bin [2.41-12+deb13u3] (2.41-12+deb13u4 Debian:13.7/stable [amd64]) []",
  "Inst libc6 [2.41-12+deb13u3] (2.41-12+deb13u4 Debian:13.7/stable [amd64])",
];
function followAnswer({ security = 0, named = true, origins = true, nft = true, open = true, port = null, listsAgo = 43583 } = {}) {
  const now = 1795145754;
  const lines = ["UPKEEP 1", ...FOLLOW_STABLE];
  // Security updates in the shape Debian 13's security archive gives them.
  for (let i = 0; i < security; i++) lines.push(`Inst libpkg${i} [1.0-1] (1.0-1+deb13u1 Debian-Security:13/stable-security [amd64])`);
  lines.push(listsAgo === null ? "LISTS " : `LISTS ${now - listsAgo}`, `NOW ${now}`, "ENV present");
  if (named) lines.push("NAMED TURN_PUBLIC_HOST");
  if (origins) lines.push("ORIGINS set");
  if (port) lines.push(`TURN_PORT=${port}`);
  lines.push(`HUMANITY_OPEN_CALL_PORT=${open ? 1 : 0}`);
  if (nft) {
    lines.push("NFT read");
    if (open) lines.push("udp dport 3478 accept");
  } else {
    lines.push("NFT unreadable");
  }
  return lines.join("\n") + "\n";
}
const judge = (server, text) => judgeServer(server, parseRemote(text));

test("a healthy answer is one quiet line", () => {
  const r = judge(MAIN, MAIN_ANSWER);
  assert.strictEqual(r.ok, true, r.line);
  assert.strictEqual(r.line, "united-humanity.us: 2 security updates waiting; call port UDP 3478 open, TURN_PUBLIC_HOST set; no ALLOWED_ORIGINS (browsers: its own pages only)");
  const f = judge(FOLLOW, followAnswer());
  assert.strictEqual(f.ok, true, f.line);
  assert.match(f.line, /^public\.guide: 0 security updates waiting \(3 others\); call port UDP 3478 open, TURN_PUBLIC_HOST set; ALLOWED_ORIGINS set$/);
  // Relying on the relay's own-pages fallback (BUG-329) is shown, never a problem.
  const bare = judge(FOLLOW, followAnswer({ origins: false }));
  assert.strictEqual(bare.ok, true, bare.line);
  assert.match(bare.line, /; no ALLOWED_ORIGINS \(browsers: its own pages only\)$/);
});

test("more than five waiting security updates is a warning (BUG-321)", () => {
  assert.strictEqual(SECURITY_LIMIT, 5);
  const r = judge(FOLLOW, followAnswer({ security: 55 }));
  assert.strictEqual(r.ok, false, r.line);
  assert.match(r.line, /^PROBLEM: public\.guide: 55 security updates waiting \(libpkg0, libpkg1, libpkg2, libpkg3, \.\.\.\), more than 5/);
  assert.match(r.line, /ssh humanity-vps2 /, "it says where to look");
  assert.strictEqual(judge(FOLLOW, followAnswer({ security: 5 })).ok, true, "five is not more than five");
  assert.strictEqual(judge(FOLLOW, followAnswer({ security: 6 })).ok, false, "six is");
});

test("package lists not refreshed for days are a warning, since nothing would then look waiting", () => {
  const r = judge(FOLLOW, followAnswer({ listsAgo: 10 * 86400 }));
  assert.strictEqual(r.ok, false, r.line);
  assert.match(r.line, /last refreshed 10 days ago/);
  assert.strictEqual(judge(FOLLOW, followAnswer({ listsAgo: 2 * 86400 })).ok, true, "two days is fine");
  assert.match(judge(FOLLOW, followAnswer({ listsAgo: null })).line, /refresh time unknown/);
});

test("an open call port with no public name is a warning (BUG-322)", () => {
  const r = judge(FOLLOW, followAnswer({ named: false }));
  assert.strictEqual(r.ok, false, r.line);
  assert.match(r.line, /call port \(UDP 3478\) is open but \/opt\/Humanity\/\.env has no TURN_PUBLIC_HOST/);
  assert.match(r.line, /add TURN_PUBLIC_HOST=public\.guide there/, "it names the setting with this server's own name");
  // TURN_SERVER_HOST names the server too (the relay falls back to it).
  const older = followAnswer({ named: false }).replace("ENV present", "ENV present\nNAMED TURN_SERVER_HOST");
  assert.strictEqual(judge(FOLLOW, older).ok, true, judge(FOLLOW, older).line);
  // No .env at all, same warning, said plainly.
  assert.match(judge(FOLLOW, followAnswer({ named: false }).replace("ENV present", "ENV missing")).line, /there is no \/opt\/Humanity\/\.env/);
});

test("a closed call port needs no name", () => {
  const r = judge(FOLLOW, followAnswer({ named: false, open: false }));
  assert.strictEqual(r.ok, true, r.line);
  assert.match(r.line, /call port UDP 3478 closed; ALLOWED_ORIGINS set$/);
  // The firewall opens 3478, but this relay listens on 3479: its port is closed.
  const moved = judge(FOLLOW, followAnswer({ named: false, port: 3479 }));
  assert.strictEqual(moved.ok, true, moved.line);
  assert.match(moved.line, /call port UDP 3479 closed; ALLOWED_ORIGINS set$/);
});

test("the firewall unreadable, the node.env switch decides", () => {
  const r = judge(FOLLOW, followAnswer({ named: false, nft: false }));
  assert.strictEqual(r.ok, false, r.line);
  assert.match(r.line, /is open \(by node\.env; the firewall could not be read\)/);
  assert.strictEqual(judge(FOLLOW, followAnswer({ named: false, nft: false, open: false })).ok, true);
});

test("an answer that is not the script's is 'could not check', never healthy", () => {
  for (const text of ["", "Permission denied (publickey).", "Inst x [1] (2 Debian-Security:13/stable-security [amd64])"]) {
    const r = judge(FOLLOW, text);
    assert.strictEqual(r.ok, false, text);
    assert.match(r.line, /^could not check public\.guide/);
  }
});

// Git for Windows' bash, not the WSL launcher in System32 that a bare "bash" can resolve to.
function bashPath() {
  if (process.platform !== "win32") return "bash";
  for (const p of ["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"]) {
    if (fs.existsSync(p)) return p;
  }
  return "bash";
}

test("the script the server runs never prints a setting's value, and its answer parses", () => {
  // The real script under bash, against a scratch checkout whose .env holds values that must not
  // come back, with stand-ins for apt-get and nft (the only commands that need a Debian server).
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum-upkeep-"));
  try {
    const repo = path.join(root, "Humanity").replace(/\\/g, "/");
    const stubs = path.join(root, "stubs");
    fs.mkdirSync(repo);
    fs.mkdirSync(stubs);
    fs.writeFileSync(
      path.join(repo, ".env"),
      ["TURN_PUBLIC_HOST=calls.secret-name.example", "TURN_SERVER_HOST=", "TURN_PORT=3478", "ALLOWED_ORIGINS=https://origins.secret-name.example", "ADMIN_TOKEN=do-not-print-this-7f3a", 'BACKUP_KEY="also-secret"', ""].join("\n"),
    );
    fs.writeFileSync(path.join(repo, "node.env"), "HUMANITY_OPEN_CALL_PORT=1\nNODE_SECRET=nope\n");
    const stub = (name, body) => fs.writeFileSync(path.join(stubs, name), `#!/bin/bash\n${body}\n`, { mode: 0o755 });
    stub("apt-get", 'echo "NOTE: This is only a simulation!"\necho "Inst nginx [1.22.1-9+deb12u10] (1.22.1-9+deb12u11 Debian-Security:12/oldstable-security [amd64]) []"\necho "Conf nginx (1.22.1-9+deb12u11 Debian-Security:12/oldstable-security [amd64])"');
    stub("nft", 'echo "table inet filter {"\necho "  chain input {"\necho "    udp dport 3478 accept"\necho "  }"\necho "}"');
    const script = remoteScript().split("/opt/Humanity").join(repo);
    const stubsPath = process.platform === "win32" ? stubs.replace(/\\/g, "/").replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`) : stubs;
    const r = spawnSync(bashPath(), ["-c", `PATH="${stubsPath}:$PATH"; ${script}`], { encoding: "utf8", timeout: 30000 });
    assert.strictEqual(r.error, undefined, String(r.error));
    for (const secret of ["calls.secret-name", "origins.secret-name", "do-not-print-this", "also-secret", "nope", "ADMIN_TOKEN", "NODE_SECRET"]) {
      assert.ok(!r.stdout.includes(secret), `the answer carried ${secret}:\n${r.stdout}`);
    }
    const p = parseRemote(r.stdout);
    assert.deepStrictEqual(
      { read: p.read, upgradable: p.upgradable, security: p.security, env: p.env, named: p.named, origins: p.origins, turnPort: p.turnPort, openFlag: p.openFlag, nft: p.nft, udpOpen: p.udpOpen },
      { read: true, upgradable: 1, security: ["nginx"], env: true, named: ["TURN_PUBLIC_HOST"], origins: true, turnPort: 3478, openFlag: true, nft: true, udpOpen: [3478] },
      `an empty TURN_SERVER_HOST names nothing; the answer was:\n${r.stdout}`,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
