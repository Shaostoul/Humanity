// The upkeep of each server, read over ssh and changing nothing (`just brief`'s SERVERS row,
// `just server-upkeep`; scripts/check-server-upkeep.js runs it, scripts/tests/server-upkeep.test.js
// proves each warning can fire). Two warnings, from two of 2026-11-20's findings on public.guide:
//
// - Security updates piling up (BUG-321): public.guide sat on 55 waiting security updates for
//   months because its automatic-update settings matched no Debian archive, and nothing said so.
//   More than SECURITY_LIMIT waiting is a warning; so are package lists not refreshed for
//   STALE_LIST_DAYS, since a server that stopped refreshing them would count nothing waiting.
// - A call port open with no public name (BUG-322): a relay whose .env names neither
//   TURN_PUBLIC_HOST nor TURN_SERVER_HOST, bound to every interface, offers no call forwarder
//   (before the fix it sent its callers to united-humanity.us's), so an open call port there
//   carries nothing.
//
// Secrets: the .env holds keys, so the server only ever answers whether a setting is there
// (grep -q, then a fixed word); the one value printed, TURN_PORT, is matched as digits only.
"use strict";

const { SERVER_REPO } = require("./server-files.js");

// More waiting security updates than this is a warning (BUG-321: public.guide had 55).
const SECURITY_LIMIT = 5;
// apt's daily job refreshes the package lists once a day; this many days without it is a warning.
const STALE_LIST_DAYS = 3;
// The call port when the .env does not set TURN_PORT (src/relay/call_forwarder.rs DEFAULT_PORT).
const DEFAULT_CALL_PORT = 3478;
// The settings that name the server to its callers (Settings::from_vars reads them in this order).
const NAME_KEYS = ["TURN_PUBLIC_HOST", "TURN_SERVER_HOST"];

// The shell a read-only ssh call runs on a server. Nothing is written: apt-get only simulates
// (-s, with locking off so a running automatic update neither blocks it nor is blocked), and the
// settings files are only grepped.
function remoteScript() {
  const env = `${SERVER_REPO}/.env`;
  const nodeEnv = `${SERVER_REPO}/node.env`;
  const named = NAME_KEYS.map(
    (k) => `if grep -qE '^${k}=["'"'"']?[^"'"'"'[:space:]]' ${env} 2>/dev/null; then echo "NAMED ${k}"; fi`,
  );
  return [
    `echo "UPKEEP 1"`,
    `apt-get -s -o Debug::NoLocking=1 dist-upgrade 2>/dev/null | grep '^Inst '`,
    `echo "LISTS $(stat -c %Y /var/lib/apt/periodic/update-stamp 2>/dev/null)"`,
    `echo "NOW $(date +%s)"`,
    `if [ -f ${env} ]; then echo "ENV present"; else echo "ENV missing"; fi`,
    ...named,
    `if grep -qE '^ALLOWED_ORIGINS=["'"'"']?[^"'"'"'[:space:]]' ${env} 2>/dev/null; then echo "ORIGINS set"; fi`,
    `grep -oE '^TURN_PORT=[0-9]+' ${env} 2>/dev/null`,
    `grep -oE '^HUMANITY_OPEN_CALL_PORT=[01]' ${nodeEnv} 2>/dev/null`,
    `if r=$(nft list ruleset 2>/dev/null); then echo "NFT read"; printf '%s\\n' "$r" | grep -oE 'udp dport [0-9]+ accept'; else echo "NFT unreadable"; fi`,
  ].join("; ");
}

// What remoteScript printed: the upgradable packages (total and from a security archive), the
// package lists' age, and the call settings.
function parseRemote(text) {
  const out = {
    read: false,
    upgradable: 0,
    security: [],
    listsAt: null,
    now: null,
    env: null,
    named: [],
    origins: false,
    turnPort: null,
    openFlag: null,
    nft: null,
    udpOpen: [],
  };
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.trim();
    let m;
    if (line === "UPKEEP 1") out.read = true;
    else if ((m = line.match(/^Inst\s+(\S+)\s.*?\(([^)]*)\)/))) {
      // `Inst nginx [1.22.1-9+deb12u10] (1.22.1-9+deb12u11 Debian-Security:12/oldstable-security [amd64]) []`:
      // the archive is inside the brackets after the old version.
      out.upgradable++;
      if (/security/i.test(m[2])) out.security.push(m[1]);
    } else if ((m = line.match(/^LISTS\s*(\d*)$/))) out.listsAt = m[1] ? Number(m[1]) : null;
    else if ((m = line.match(/^NOW\s*(\d*)$/))) out.now = m[1] ? Number(m[1]) : null;
    else if ((m = line.match(/^ENV (present|missing)$/))) out.env = m[1] === "present";
    else if ((m = line.match(/^NAMED ([A-Z_]+)$/))) out.named.push(m[1]);
    else if (line === "ORIGINS set") out.origins = true;
    else if ((m = line.match(/^TURN_PORT=(\d+)$/))) out.turnPort = Number(m[1]);
    else if ((m = line.match(/^HUMANITY_OPEN_CALL_PORT=([01])$/))) out.openFlag = m[1] === "1";
    else if ((m = line.match(/^NFT (read|unreadable)$/))) out.nft = m[1] === "read";
    else if ((m = line.match(/^udp dport (\d+) accept$/))) out.udpOpen.push(Number(m[1]));
  }
  return out;
}

// One server's line: { ok, problems, line }. `server` is a scripts/server-files.txt server line
// ({ alias, name }).
function judgeServer(server, r) {
  if (!r.read) {
    return { ok: false, problems: ["not read"], line: `could not check ${server.name}: its answer over ssh ${server.alias} was not the upkeep script's` };
  }
  const problems = [];
  const facts = [];

  const sec = r.security.length;
  const others = r.upgradable - sec;
  if (sec > SECURITY_LIMIT) {
    const some = r.security.slice(0, 4).join(", ") + (sec > 4 ? ", ..." : "");
    problems.push(
      `${sec} security updates waiting (${some}), more than ${SECURITY_LIMIT}: are its automatic updates installing them? (BUG-321: ssh ${server.alias} "tail -20 /var/log/unattended-upgrades/unattended-upgrades.log")`,
    );
  } else {
    facts.push(`${sec} security update${sec === 1 ? "" : "s"} waiting` + (others ? ` (${others} other${others === 1 ? "" : "s"})` : ""));
  }
  if (r.listsAt && r.now) {
    const days = Math.floor((r.now - r.listsAt) / 86400);
    if (days >= STALE_LIST_DAYS) {
      problems.push(`its package lists were last refreshed ${days} days ago, so updates waiting may be undercounted (ssh ${server.alias} "systemctl status apt-daily.timer")`);
    }
  } else {
    facts.push("package lists' refresh time unknown");
  }

  const port = r.turnPort || DEFAULT_CALL_PORT;
  // The firewall is the truth when it could be read; the provision script's switch otherwise.
  const open = r.nft ? r.udpOpen.includes(port) : r.openFlag === true;
  const how = r.nft ? "" : " (by node.env; the firewall could not be read)";
  const name = NAME_KEYS.find((k) => r.named.includes(k));
  if (open && !name) {
    const where = r.env === false ? `there is no ${SERVER_REPO}/.env` : `${SERVER_REPO}/.env has no TURN_PUBLIC_HOST`;
    problems.push(
      `its call port (UDP ${port}) is open${how} but ${where}, so its relay offers no call forwarder (BUG-322): add TURN_PUBLIC_HOST=${server.name} there and restart the relay`,
    );
  } else if (open) {
    facts.push(`call port UDP ${port} open${how}, ${name} set`);
  } else {
    facts.push(`call port UDP ${port} closed${how}`);
  }
  // Not a problem either way (BUG-329: unset, the relay lets in only its own name and its own
  // https pages), but worth seeing: a server that relies on that fallback says so.
  if (r.env !== false) {
    facts.push(r.origins ? "ALLOWED_ORIGINS set" : "no ALLOWED_ORIGINS (browsers: its own pages only)");
  }

  if (problems.length) {
    return { ok: false, problems, line: `PROBLEM: ${server.name}: ${problems.join("; ")}` + (facts.length ? ` [${facts.join("; ")}]` : "") };
  }
  return { ok: true, problems, line: `${server.name}: ${facts.join("; ")}` };
}

module.exports = { remoteScript, parseRemote, judgeServer, SECURITY_LIMIT, STALE_LIST_DAYS, DEFAULT_CALL_PORT, NAME_KEYS };
