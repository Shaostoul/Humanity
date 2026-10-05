// The settings a rig pins in its sandbox's config.json before it boots the game (2026-10-04):
// the GAMEPLAY it was built on, and NO SERVER OFF THIS COMPUTER.
//
// GAMEPLAY. Fresh installs start in Normal mode with progress kept between launches (the
// operator's decision of 2026-10-04: src/config.rs, PlayMode's default and
// fresh_world_each_launch's default). A rig's sandbox is a fresh install: portable.txt keeps its
// config.json beside its copy of the exe, and the first run has none at all. Every rig was built
// on the defaults before that change, and still needs them:
//   play_mode "Dev"                  the probe vantages' garden is planted only while free
//                                    resources are on (src/engine/ipc.rs showcase_gate: "a rig is
//                                    a Dev-mode sandbox"), and the Dev tools, the Dev page's
//                                    travel and the build editor's whole-ship scope are Dev's.
//   fresh_world_each_launch true     every run starts from the tree's default home, not from what
//                                    an earlier run left in the rig's save. A rig that is about
//                                    kept progress can ask for false; none does today.
// The play mode is not a rig's to choose: a rig is a Dev sandbox.
//
// NO LIVE SERVER (found 2026-10-05, .probe-rig/screens/logs/run.log). The game remembers every
// server it ever connected to in `saved_servers` and dials each one in the background at every
// boot (src/engine/bg_connections.rs). Four rig sandboxes (the probe sweep's, verify-screens',
// photograph-home's and make-clips') had united-humanity.us there from an older run, so a
// verify-screens run "dialing saved server https://united-humanity.us" connected to the LIVE
// server with a rig identity, received the live #general history and its 56 users, and tried to
// join the live shared world (refused only because the ship differed). So the pin also:
//   server_url        the rig's own throwaway relay when it has one (`server_url` in the
//                     overrides; it must be on this computer), else RIG_NO_SERVER, a loopback
//                     port nothing listens on. Never absent: a config without one gets the
//                     game's built-in default, which IS the live server (GuiState::default,
//                     src/config.rs). An empty one is no server since BUG-160 (2026-10-05),
//                     but a rig names its own port rather than lean on that.
//   saved_servers     emptied. A rig's server is the one its autopilot request names; the
//                     background pump has nothing else to dial.
//   home_plots, account_erased_on, collapsed_servers, last_world
//                     every entry naming a server off this computer is dropped (the plot
//                     remembered for a relay on this computer stays: verify-copresence --plots
//                     proves it survives a relaunch).
// And spawnGame refuses to boot a rig whose config still names a server off this computer after
// the pin (a config field the pin does not know yet): `publicServers` finds any URL whose host is
// not loopback, and any mention of the live domain, anywhere in the file.
//
// ONE PLACE: spawnGame (lib/game-launch.js), the one way every rig starts the game
// (scripts/tests/rig-boot.test.js holds them to it), calls pinSandboxGameplay right before it
// spawns, so no rig can boot without the pin, and a rig written later gets it for nothing.
// scripts/tests/rig-gameplay.test.js runs it.
//
// WHICH CONFIG (src/config.rs AppConfig::config_path, src/storage.rs portable mode):
// HUMANITY_DATA_DIR/config.json when that is set; else config.json beside the exe when
// portable.txt is there; else the player's own (%APPDATA%\HumanityOS\config.json), which a rig
// never writes: sandboxConfigPath returns null and nothing is pinned. The operator's own config
// holds "Dev" explicitly, so his `just launch` and `just launch-bg` are untouched by the change.
//
// Std-only (node built-ins), like the rest of scripts/lib.

const fs = require("fs");
const path = require("path");

/** What every rig runs with unless it says otherwise (only fresh_world_each_launch may differ). */
const RIG_GAMEPLAY = Object.freeze({ play_mode: "Dev", fresh_world_each_launch: true });

/** The server a rig with no relay of its own is pointed at: a loopback port nothing listens on
 *  (9, "discard"), so a connect is refused at once on this computer and nothing leaves it. */
const RIG_NO_SERVER = "http://127.0.0.1:9";

/** The live service's domain: a mention of it anywhere in a rig's config is a public server. */
const LIVE_DOMAIN = /united-humanity/i;

/** A URL with a scheme anywhere in a string, also inside an id ("srv_https://..."): group 1 is
 *  the host. */
const URL_HOST = /[a-z][a-z0-9+.-]*:\/\/(\[[^\]]*\]|[^\/:?#\s"'\]]+)/gi;

/** True for a host on this computer: localhost, 127.x.x.x, ::1. */
function isLoopbackHost(host) {
  const h = String(host || "").trim().replace(/^\[|\]$/g, "").toLowerCase();
  return h === "localhost" || h === "::1" || /^127(\.\d{1,3}){3}$/.test(h);
}

/** The servers a string names: the host of every URL in it, and the live domain when it is
 *  named bare ("united-humanity.us"). */
function serversIn(s) {
  const out = [];
  for (const m of String(s).matchAll(URL_HOST)) out.push(m[1]);
  if (LIVE_DOMAIN.test(String(s)) && !out.some((h) => LIVE_DOMAIN.test(h))) out.push("united-humanity.us");
  return out;
}

/** True when `s` names a server off this computer. */
function namesPublicServer(s) {
  return serversIn(s).some((h) => !isLoopbackHost(h));
}

/** Every place `cfg` names a server off this computer: [{ path, value }], over every string and
 *  every object key, however deep. Empty for a config that keeps a rig on this computer. */
function publicServers(cfg) {
  const out = [];
  const walk = (v, at) => {
    if (typeof v === "string") {
      if (namesPublicServer(v)) out.push({ path: at || "(root)", value: v });
    } else if (Array.isArray(v)) {
      v.forEach((x, i) => walk(x, `${at}[${i}]`));
    } else if (v && typeof v === "object") {
      for (const [k, x] of Object.entries(v)) {
        if (namesPublicServer(k)) out.push({ path: `${at}{key}`, value: k });
        walk(x, at ? `${at}.${k}` : k);
      }
    }
  };
  walk(cfg, "");
  return out;
}

/** The values to pin: the Dev mode always; the default home every launch unless the rig asks
 *  otherwise with a boolean `fresh_world_each_launch`; the rig's own relay as its server when it
 *  names one (`server_url`, which must be on this computer), else RIG_NO_SERVER. Throws for a
 *  relay off this computer. */
function rigGameplay(overrides = {}) {
  const o = overrides || {};
  let server = RIG_NO_SERVER;
  if (o.server_url !== undefined && o.server_url !== null && o.server_url !== "") {
    if (namesPublicServer(o.server_url) || !serversIn(o.server_url).length) {
      throw new Error(`a rig's server must be its throwaway relay on this computer (scripts/lib/throwaway-relay.js), not ${JSON.stringify(o.server_url)}`);
    }
    server = String(o.server_url);
  }
  return {
    play_mode: RIG_GAMEPLAY.play_mode,
    fresh_world_each_launch:
      typeof o.fresh_world_each_launch === "boolean" ? o.fresh_world_each_launch : RIG_GAMEPLAY.fresh_world_each_launch,
    server_url: server,
  };
}

/** `cfg` (a parsed config.json, or nothing) with the rig's settings pinned. Pure: returns
 *  { next, changed }, where changed maps each key it set to { was, now } (absent = undefined). */
function pinGameplay(cfg, overrides = {}) {
  const next = { ...(cfg && typeof cfg === "object" && !Array.isArray(cfg) ? cfg : {}) };
  const changed = {};
  const set = (k, v) => {
    if (JSON.stringify(next[k]) !== JSON.stringify(v)) changed[k] = { was: next[k], now: v };
    next[k] = v;
  };
  for (const [k, v] of Object.entries(rigGameplay(overrides))) set(k, v);
  // Nothing to dial in the background: the rig's server is its autopilot's (or its relay).
  set("saved_servers", []);
  // Memories of servers off this computer go; a loopback relay's stay.
  for (const k of ["home_plots", "account_erased_on"]) {
    const m = next[k];
    if (m && typeof m === "object" && !Array.isArray(m)) {
      const kept = Object.fromEntries(Object.entries(m).filter(([key, v]) => !namesPublicServer(key) && !publicServers(v).length));
      if (Object.keys(kept).length !== Object.keys(m).length) set(k, kept);
    }
  }
  if (Array.isArray(next.collapsed_servers)) {
    const kept = next.collapsed_servers.filter((s) => !namesPublicServer(s));
    if (kept.length !== next.collapsed_servers.length) set("collapsed_servers", kept);
  }
  if (typeof next.last_world === "string" && namesPublicServer(next.last_world)) set("last_world", "");
  return { next, changed };
}

/** Why `cfg` would not boot a rig's game as a Dev sandbox from the default home, kept on this
 *  computer (empty = fine). A missing key is the fresh-install default, which is Normal with
 *  progress kept, and for server_url the game's built-in server, the live one. An empty
 *  server_url is no server (BUG-160), but it is not the rig's pin either, so it is listed too. */
function gameplayProblems(cfg, overrides = {}) {
  const want = rigGameplay(overrides);
  const c = cfg && typeof cfg === "object" ? cfg : {};
  const out = [];
  for (const k of ["play_mode", "fresh_world_each_launch"]) {
    if (c[k] !== want[k]) out.push(`${k} is ${c[k] === undefined ? "absent (the fresh-install default)" : JSON.stringify(c[k])}, not ${JSON.stringify(want[k])}`);
  }
  if (c.server_url == null) out.push("server_url is absent, which the game reads as its built-in server, the live one");
  else if (!c.server_url) out.push("server_url is empty (no server), not the rig's relay or its dead loopback port");
  if (Array.isArray(c.saved_servers) && c.saved_servers.length) out.push(`saved_servers holds ${c.saved_servers.length}, which the game dials in the background at boot`);
  for (const p of publicServers(c)) out.push(`${p.path} names a server off this computer: ${p.value}`);
  return out;
}

/** The config.json the game started as `exe` with `env` reads, when that is a sandbox's:
 *  HUMANITY_DATA_DIR's, else the one beside a portable exe. null for the player's own. */
function sandboxConfigPath(exe, env = process.env) {
  const override = String((env && env.HUMANITY_DATA_DIR) || "").trim();
  if (override) return path.join(override, "config.json");
  const exeDir = path.dirname(path.resolve(exe));
  if (fs.existsSync(path.join(exeDir, "portable.txt"))) return path.join(exeDir, "config.json");
  return null;
}

/** Pin the rig's settings into the sandbox config of the game about to start as `exe` with `env`.
 *  Returns { path, gameplay, changed, public_servers }, or null when `exe` is not in a sandbox
 *  (nothing written). `public_servers` is what still names a server off this computer after the
 *  pin (a field the pin does not know): spawnGame refuses to boot then. A config that does not
 *  parse would boot on the fresh-install defaults, so the pin starts over from nothing and the
 *  unreadable file is kept beside it as config.json.unreadable-<time>. */
function pinSandboxGameplay(exe, env = process.env, overrides = {}, log = console.log, rigName = "rig") {
  const file = sandboxConfigPath(exe, env);
  if (!file) return null;
  let cfg = {};
  if (fs.existsSync(file)) {
    const raw = fs.readFileSync(file, "utf8");
    try {
      cfg = JSON.parse(raw);
    } catch (e) {
      const kept = `${file}.unreadable-${Date.now()}`;
      fs.copyFileSync(file, kept);
      log(`[${rigName}] the sandbox's config.json did not parse (${e.message}); kept as ${path.basename(kept)}, starting over from the pin`);
      cfg = {};
    }
  }
  const { next, changed } = pinGameplay(cfg, overrides);
  if (Object.keys(changed).length || !fs.existsSync(file)) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, JSON.stringify(next, null, 2));
  }
  const gameplay = rigGameplay(overrides);
  const brief = (v) => {
    const s = v === undefined ? "(absent)" : JSON.stringify(v);
    return s.length > 60 ? `${s.slice(0, 57)}...` : s;
  };
  const what = Object.entries(changed)
    .map(([k, c]) => `${k} ${brief(c.was)} -> ${brief(c.now)}`)
    .join(", ");
  log(
    `[${rigName}] rig settings pinned in ${file}: play_mode ${gameplay.play_mode}, fresh_world_each_launch ${gameplay.fresh_world_each_launch}, ` +
      `server ${gameplay.server_url}, no saved servers` +
      (what ? ` (changed: ${what})` : " (already so)"),
  );
  return { path: file, gameplay, changed, public_servers: publicServers(next) };
}

module.exports = {
  RIG_GAMEPLAY,
  RIG_NO_SERVER,
  isLoopbackHost,
  namesPublicServer,
  publicServers,
  rigGameplay,
  pinGameplay,
  gameplayProblems,
  sandboxConfigPath,
  pinSandboxGameplay,
};
