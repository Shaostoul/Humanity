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
  // Room light shadows (2026-11-10, renderer/local_shadows.rs): "0" off, a level id pins it, "auto"
  // hands it back to Settings > Graphics.
  local_shadows: "auto",
  fp_shadow_twin: "1",
  // The shadow atlas's instanced caster path (2026-11-11): "0" draws every caster the classic way.
  shadow_fast: "1",
  // The GPU-driven shadow casters (2026-11-13, renderer/shadow_gpu.rs): "0" sends every plain caster's
  // draw from the processor, as before (the red twin).
  shadow_gpu: "1",
  // Shadows rung 1b (2026-11-12): "0" draws every caster inside a shadow face whether or not its
  // shadow can reach the view (no shadow-caster cull), and atlas_code "1" keeps the atlas lookups
  // compiled into the shading at Off: the two red twins.
  shadow_cull: "1",
  atlas_code: "0",
  // Temporal anti-aliasing (2026-11-13, renderer/taa.rs): {"taa":"off"} is its red twin (the
  // picture must equal the pre-TAA one), "auto" hands it back to Settings > Graphics; taa_view "0"
  // shows the picture again after a Dev view (1 motion, 2 history weight, 3 responsive mask).
  taa: "auto",
  taa_view: "0",
  fov: "auto",
  // Settings > Gameplay > Pipe markings: "auto" hands the mode back to the setting.
  pipe_marking: "auto",
  // The hull, as the H key toggles it (2026-10-05, the ship-first-street vantage hides it to see
  // the homes along First Street): "1" shows it again. Not a setting, so no "auto".
  hull: "1",
  // The HUD (2026-10-21): a sky vantage hides it ({"hud":"0"}) so the crosshair dot in the middle
  // of the screen does not sit on the body it aims at. "1" shows it again.
  hud: "1",
  // The fruiting tents' doors (2026-11-13, engine/fruiting_tent.rs): "up" or "down" holds every
  // door; "auto" hands each back to its rack's tending.
  tent_door: "auto",
  // Settings > Graphics > Bounced light detail (2026-10-20): the pin sets the setting itself, so
  // the release is the default level. A change relays the probes out, so allow the light a few
  // seconds to settle after it.
  bounced_light_detail: "high",
  // Floors between storeys (increment 7 lane L, 2026-10-21): "0" off, "sun" or "lamps" runs one
  // half alone; "1" is both, as released.
  slab_occlusion: "1",
  // The Sun (2026-10-21, renderer/sun_disc.rs): {"sun_glare":"0"} takes its glare out of the
  // present pass, {"sun_aperture_mm":"8"} sets the glare's aperture, {"godrays":"0"} stops the
  // god-ray pass. "1" and "auto" are the defaults.
  sun_glare: "1",
  sun_aperture_mm: "auto",
  godrays: "auto",
  // The characters (2026-11-08, engine/animation.rs): the dev crowd ({"crowd":"0"} clears it and
  // every `figure_path` stand-in), the box-figure A/B and the third-person camera.
  crowd: "0",
  figures: "bodies",
  camera_mode: "first",
  // The player's look and walking pace (2026-11-09): the body a vantage shows in third person, the
  // look's height, and Settings' walking pace; each released to the default a fresh character has.
  body: "default",
  look_height: "1",
  walk_pace: "simplified",
  // The body sliders (2026-11-09): the look's shape ({"shape":"default"}: every slider in its
  // middle, the body as made), the skeleton overlay, and whether people with no look of their own
  // (the crew, the dev crowd) take varied shapes. A `lineup` lives in the crowd, cleared above.
  shape: "default",
  skeletons: "0",
  varied_shapes: "1",
  // Feet on the world (2026-11-09, character animation rung 2a): the Dev page's switch (the A/B
  // against rung 1's pose as the clips made it) and Settings' animation mode, each released to the
  // default. A `crowd_crouch` lives in the crowd, cleared above.
  feet: "1",
  animation_mode: "realistic",
  // The look (2026-11-09, docs/design/character-animation.md section 16): each near person's own
  // mesh, garments hiding the body under them, soles measured with the shoes on, the garments'
  // textures (each a same-boot A/B), and the player's detail sliders and what they wear ("-": none;
  // the look_wear default puts back the wearer kind's defaults).
  own_mesh: "1",
  covers: "1",
  soles: "1",
  garment_textures: "1",
  look_detail: "-",
  look_wear: "hair=,brows=,lashes=,wear:chest=,wear:feet=,wear:head=",
  // Interactions (2026-11-10, character animation rung 2b, engine/interact.rs): the player's grant
  // ({"act":"none"} lets go at once), motion warping, the hands on their contacts and the hidden
  // first-person head and the seated feet planted (each a red twin's switch), and Settings'
  // first-person body and Reduce motion.
  // A `crowd_act` stand-in lives in the crowd, cleared above.
  act: "none",
  warp: "1",
  hand_ik: "1",
  fp_head: "1",
  seat_plant: "0",
  fp_body: "1",
  reduce_motion: "0",
  // How deep, and how near the camera, the terrain refines inside an installed map pack (2026-11-09,
  // whole-Earth maps, the rung between 2 and 3): a vantage pins {"pack_depth":"13"} to see a pack at
  // the planet's depth, or {"pack_within_m":"100"} to bound its reach.
  pack_depth: "auto",
  pack_within_m: "auto",
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
