// The showcase pins a rig must release between vantages (2026-10-04).
//
// Showcase pins are STICKY: the engine keeps a pin (a frozen wind, an fov, the
// pipe marking mode) until it is set to "auto" (or its default), so a vantage
// that does not pin one would inherit the previous vantage's. probe-sweep.js and
// probe-hot-ab.js send these defaults before every vantage, overridden by the
// keys the vantage pins itself. One list, so the two rigs cannot drift apart:
// they had (probe-hot-ab released neither room_gi nor fov), and neither released
// the pipe marking pin when it was added.

"use strict";

// Every sticky pin and the value that releases it.
const STICKY_PIN_RESETS = Object.freeze({
  wind: "auto",
  anim_clock: "auto",
  aurora: "1",
  room_gi: "1",
  present_dither: "1",
  sun_shadows: "auto",
  near_levels: "auto",
  fov: "auto",
  // Settings > Gameplay > Pipe markings: "auto" hands the mode back to the setting.
  pipe_marking: "auto",
  // The hull, as the H key toggles it (2026-10-05, the ship-first-street vantage hides it to see
  // the homes along First Street): "1" shows it again. Not a setting, so no "auto".
  hull: "1",
});

// Diagnostic channels, also sticky, reset at the start of every vantage.
const DIAG_RESETS = Object.freeze({
  map_diag: "0",
  cloud_top_bound: "0",
  cloud_uniform_step: "0",
  cloud_step_m: "0",
});

// Does this vantage's showcase block hold a sticky pin at a non-default value,
// so the NEXT vantage (if it has no showcase block) must be sent the resets?
function holdsStickyPin(showcase) {
  if (!showcase) return false;
  return Object.keys(STICKY_PIN_RESETS).some((k) => k in showcase && String(showcase[k]) !== STICKY_PIN_RESETS[k]);
}

module.exports = { STICKY_PIN_RESETS, DIAG_RESETS, holdsStickyPin };
