// A rig sandbox's game as a BRAND-NEW IDENTITY on every run (2026-11-15, BUG-316).
//
// A rig's sandbox (portable.txt beside its copy of the exe) keeps the game's identity and its
// settings between runs: the autopilot's seed (debug/autopilot_seed.bin, src/engine/ipc.rs, kept
// on purpose so a long-lived relay's cap on new identities is not spent) and config.json (the
// chosen privacy level, the key, the first-run flags). So after its first run a rig never again
// takes the path every new person takes. verify-copresence passed 22/22 on such a sandbox
// (.probe-rig/copresence-old-20261115) while a fresh one failed 20/22: the first-run "Choose your
// privacy" window could not be closed and covered the walker, and the rig, having answered it
// once long ago, "simply found nothing to click".
//
// `forgetIdentity` removes both, so the next boot is a first launch: no config.json (spawnGame's
// pin, lib/rig-gameplay.js, writes the rig's own settings again) and a new seed. A rig whose relay
// is a throwaway one per run (lib/throwaway-relay.js) spends nothing by it. Call it before the
// boot, after any game left running from the sandbox was stopped (it could write config.json
// again on its way out).
//
// Std-only (node built-ins), like the rest of scripts/lib.

const fs = require("fs");
const path = require("path");

/** The files that make a sandbox's game a returning identity, for the sandbox at `rig`. */
function identityFiles(rig) {
  return [path.join(rig, "config.json"), path.join(rig, "debug", "autopilot_seed.bin")];
}

/** Forget the sandbox's identity. Returns { gone: [basename...], error: null } or, when a file
 *  is there and cannot be removed, { gone, error: "<path>: <why>" } (the run must then refuse:
 *  it would not be a first launch). */
function forgetIdentity(rig) {
  const gone = [];
  for (const f of identityFiles(rig)) {
    try {
      fs.unlinkSync(f);
      gone.push(path.basename(f));
    } catch (e) {
      if (e.code !== "ENOENT") return { gone, error: `${f}: ${e.message}` };
    }
  }
  return { gone, error: null };
}

module.exports = { identityFiles, forgetIdentity };
