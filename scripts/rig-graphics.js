#!/usr/bin/env node
// Graphics provenance for the probe rig: record WHICH settings a sweep ran at,
// and optionally mirror the operator's live graphics settings into the rig.
//
// WHY THIS EXISTS (2026-08-02, the failure this closes):
//   An SSAO A/B ran on the rig and came back "nothing here, +-3% noise" on a
//   bug that is plainly visible at the operator's settings (6.6-9.5%
//   separation). The rig differed from the operator on every axis that
//   mattered - ssao_strength 0.55 vs 0.9594, veg_density 0.6 vs 0.104,
//   fog_density 1 vs 4, render_distance 500 vs 2000, tree_model_distance
//   120 vs 300, godray_intensity 0.55 vs 0.979 - and NOTHING in the sweep
//   output said so. The verdict looked like a verdict about the game; it was
//   a verdict about the rig's config.json.
//
// So: every manifest now carries the settings the run actually used, every
// default run prints how far those are from the operator, and
// --operator-config mirrors the operator's visual settings into the rig.
//
// CLASSIFICATION (the only hand-maintained part; default is VISUAL on purpose)
//   private     identity / vault / network / personal. Never recorded into a
//               manifest, never copied into a rig.
//   not_visual  audio, input, chat + nav chrome, onboarding, voice. Not
//               recorded (noise), not copied.
//   record_only recorded because it changes what the numbers MEAN, but never
//               copied, each with a stated reason (frame caps, window mode,
//               gameplay content).
//   visual      everything else -> recorded AND copied by --operator-config.
//
// The default being "visual" is deliberate: a NEW graphics setting added to
// src/config.rs shows up in the manifest and in the mirror automatically
// instead of being silently missed, which is the exact drift that caused this.
// A new NON-visual setting gets classified wrong, so mirroring prints every
// unknown key it copies and tells you to classify it here.

const fs = require("fs");
const path = require("path");
const os = require("os");

// The six settings the 2026-08-02 miss turned on. If a snapshot cannot find
// these in the config it is not a usable provenance record, and the sweep says
// so out loud instead of writing a manifest that looks complete.
const REQUIRED_KEYS = [
  "ssao_strength",
  // v0.1106 split the one vegetation slider into three.
  "tree_density",
  "grass_density",
  "fog_density",
  "render_distance",
  "tree_model_distance",
  "godray_intensity",
];

// Never recorded, never copied. Anything secret-shaped is refused by pattern
// too, so a config key added later cannot leak just because nobody updated
// this list.
const PRIVATE_KEYS = new Set([
  "last_world",
  "server_url",
  "user_name",
  "public_key_hex",
  "private_key_hex",
  "encrypted_private_key",
  "key_salt",
  "key_iterations",
  "auto_unlock_mode",
  "pin_encrypted_seed",
  "pin_salt",
  "donate_solana_address",
  "donate_btc_address",
  "donate_addresses",
  "default_character",
  "profile_visible",
  "online_status_visible",
]);
const PRIVATE_PATTERN =
  /(^|_)(key|keys|secret|token|pass|passphrase|password|seed|salt|private|encrypted|signature|auth|address|wallet|email|url|host|hostname|server)s?(_|$)/i;

// Real settings, but not visual: they describe the operator, not the picture.
const NOT_VISUAL = new Set([
  "context_real",
  "completed_onboarding",
  "concept_tour_seen",
  "onboarding_quest_progress",
  "mouse_sensitivity",
  "invert_y",
  "master_volume",
  "music_volume",
  "sfx_volume",
  "nav_display_mode",
  "nav_two_tier",
  "nav_top_category",
  "default_page",
  "timestamp_format",
  "voice_input_device",
  "voice_output_device",
  "voice_gain",
  "voice_filter_mode",
  "voice_transmit_mode",
  "voice_ptt_key",
  "voice_vad_threshold",
  "chat_connection_collapsed",
  "chat_dm_collapsed",
  "chat_groups_collapsed",
  "chat_servers_collapsed",
  "chat_connected_server_collapsed",
  "chat_friends_collapsed",
  "chat_members_collapsed",
  "chat_studio_collapsed",
  "chat_left_panel_locked",
  "chat_right_panel_locked",
  "chat_left_panel_width",
  "chat_right_panel_width",
  "hud_chat_feed_visible",
  "ingame_chat_panel_height",
]);

// Recorded (they change what a reading means) but NEVER mirrored, with why.
const RECORD_ONLY = {
  window_mode:
    "window presentation: the capture-width gate is judged against the rig window, mirroring would change capture size",
  fullscreen: "window presentation, same reason as window_mode",
  vsync: "frame pacing: mirroring the operator's cap would corrupt the rig's fps readings",
  fps_foreground: "frame cap: would corrupt the rig's fps readings",
  fps_foreground_unlimited: "frame cap: would corrupt the rig's fps readings",
  fps_background: "frame cap: would corrupt the rig's fps readings",
  fps_background_sync: "frame cap: would corrupt the rig's fps readings",
  font_size: "egui text size: shifts every HUD overlay in a capture, not a 3D setting",
  dark_mode: "egui theme: HUD only, not a 3D setting",
  play_mode: "dev/creative gates, gameplay not graphics",
  home_variant: "which homestead design loads, gameplay content not graphics",
  hostile_wildlife: "spawns predators, gameplay content not graphics",
  vitals_drain: "survival decay rate, gameplay not graphics",
};

function classifyKey(key) {
  if (PRIVATE_KEYS.has(key) || PRIVATE_PATTERN.test(key)) return "private";
  if (Object.prototype.hasOwnProperty.call(RECORD_ONLY, key)) return "record_only";
  if (NOT_VISUAL.has(key)) return "not_visual";
  return "visual";
}

// True for values worth recording/copying: scalars only. Objects and arrays
// (onboarding_quest_progress, donate_addresses) are never provenance.
function isScalar(v) {
  return (
    typeof v === "boolean" ||
    (typeof v === "number" && Number.isFinite(v)) ||
    (typeof v === "string" && v.length <= 64)
  );
}

function readConfig(file) {
  try {
    const raw = fs.readFileSync(file, "utf8");
    return { ok: true, cfg: JSON.parse(raw), path: file, mtime: fs.statSync(file).mtime.toISOString() };
  } catch (e) {
    return { ok: false, cfg: null, path: file, error: e.code === "ENOENT" ? "no such file" : String(e.message || e) };
  }
}

// Where the RIG's config.json lives. Mirrors AppConfig::config_path()'s order
// for a rig: HUMANITY_DATA_DIR wins over the portable marker the rig writes.
function rigConfigPath(rigDir) {
  const override = (process.env.HUMANITY_DATA_DIR || "").trim();
  if (override) return path.join(override, "config.json");
  return path.join(rigDir, "config.json");
}

// Where the OPERATOR's live config.json lives. Same resolution as
// AppConfig::config_path() minus the portable branch (their exe runs from a
// dir with data/ beside it, which is LegacyBesideExe, and that branch falls
// through to the OS dir). HUMANITY_DATA_DIR is deliberately NOT honoured here:
// when a sweep shell sets it, it is pointing at a rig, not at the operator.
function operatorConfigPath() {
  if (process.platform === "win32") {
    const appdata = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(appdata, "HumanityOS", "config.json");
  }
  return path.join(os.homedir(), ".config", "HumanityOS", "config.json");
}

// The provenance record: every visual + record_only key, in config order, plus
// the list of REQUIRED_KEYS the config did not have.
function snapshotGraphics(cfg) {
  const values = {};
  if (cfg) {
    for (const [k, v] of Object.entries(cfg)) {
      const cls = classifyKey(k);
      if (cls !== "visual" && cls !== "record_only") continue;
      if (!isScalar(v)) continue;
      values[k] = v;
    }
  }
  const missing = REQUIRED_KEYS.filter((k) => !(k in values));
  return { values, missing };
}

// What a default (non-mirrored) run differs from the operator by. Read-only:
// nothing is written, nothing is changed. Returns null when the operator
// config cannot be read, with the reason, so a default sweep never fails just
// because the operator has not launched the app on this machine.
function diffAgainstOperator(rigCfg, opRead) {
  if (!opRead.ok) return { ok: false, reason: opRead.error, path: opRead.path, diffs: {} };
  const diffs = {};
  for (const [k, opVal] of Object.entries(opRead.cfg)) {
    const cls = classifyKey(k);
    if (cls !== "visual" && cls !== "record_only") continue;
    if (!isScalar(opVal)) continue;
    const rigVal = rigCfg ? rigCfg[k] : undefined;
    if (rigVal === opVal) continue;
    diffs[k] = { rig: rigVal === undefined ? null : rigVal, operator: opVal, mirrorable: cls === "visual" };
  }
  return { ok: true, path: opRead.path, mtime: opRead.mtime, diffs };
}

// Copy the operator's VISUAL settings into the rig config object. Pure: returns
// the next config plus a full account of what happened. Never touches private
// or record_only keys.
function mirrorOperatorGraphics(rigCfg, opCfg) {
  const next = { ...(rigCfg || {}) };
  const copied = {};
  const skipped = {};
  const unknown = [];
  for (const [k, opVal] of Object.entries(opCfg)) {
    const cls = classifyKey(k);
    if (cls === "private") continue;
    if (cls === "not_visual") continue;
    if (cls === "record_only") {
      if (rigCfg && rigCfg[k] !== opVal) skipped[k] = RECORD_ONLY[k];
      continue;
    }
    if (!isScalar(opVal)) {
      skipped[k] = `not a scalar (${Array.isArray(opVal) ? "array" : typeof opVal}), never provenance`;
      continue;
    }
    const rigVal = rigCfg ? rigCfg[k] : undefined;
    if (rigVal !== undefined && typeof rigVal !== typeof opVal) {
      skipped[k] = `type mismatch: rig has ${typeof rigVal}, operator has ${typeof opVal}`;
      continue;
    }
    // A key the classifier only calls visual because nobody has classified it:
    // copy it (the default is deliberate) but say so, loudly, every run.
    if (!KNOWN_VISUAL.has(k)) unknown.push(k);
    if (rigVal === opVal) continue;
    next[k] = opVal;
    copied[k] = { rig_was: rigVal === undefined ? null : rigVal, operator: opVal };
  }
  return { next, copied, skipped, unknown };
}

// Every key classified visual as of 2026-08-02, purely so mirroring can tell
// "known visual setting" from "new key that defaulted to visual". Adding a key
// here is NOT required for it to be recorded or mirrored - it only silences
// the "new key" notice, so forgetting to update it is loud, never silent.
const KNOWN_VISUAL = new Set([
  "fov",
  "render_distance",
  "water_detail_depth",
  "lights_tiled",
  "planet_detail",
  "sky_orbit_mode",
  "sky_constellations",
  "sky_milkyway_glow",
  "sky_milkyway_intensity",
  "sky_glow_tier",
  "sky_star_halos",
  "star_catalog_tier",
  "planet_lod_px",
  "planet_max_subdiv",
  "terrain_split_px",
  "terrain_patch_budget",
  "terrain_detail_distance",
  "tree_model_distance",
  "tree_density",
  "grass_density",
  "grass_detail",
  "veg_tree_card_m",
  "sun_shadows",
  "godray_intensity",
  "aerial_strength",
  "ssao_strength",
  "terrain_builds_per_frame",
  "planet_chunked",
  "terrain_lod_fade",
  "planet_atmo_scatter",
  "planet_clouds",
  "live_weather",
  "track_station",
  "planet_surface_detail",
  "water_fft",
  "water_clarity",
  "precip_density",
  "fog_density",
  "gpu_particles",
  "far_tree_sheet",
  "cloud_quality",
]);

// Pretty one-value formatter for the banners.
function fmt(v) {
  if (v === null || v === undefined) return "(absent)";
  if (typeof v === "number") return String(Math.round(v * 10000) / 10000);
  return String(v);
}

module.exports = {
  REQUIRED_KEYS,
  RECORD_ONLY,
  KNOWN_VISUAL,
  classifyKey,
  isScalar,
  readConfig,
  rigConfigPath,
  operatorConfigPath,
  snapshotGraphics,
  diffAgainstOperator,
  mirrorOperatorGraphics,
  fmt,
};
