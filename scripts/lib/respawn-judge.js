// The respawn rig's judge (scripts/verify-respawn.js; docs/design/game-setting-and-intro.md rung 2,
// 2026-11-21): reads the probe-sweep manifest of tests/visual/respawn-vantages.json and judges
// where the game stood the player after each respawn, against what the TREE's data says, not
// against what the game says it chose:
//   - "home" wakes upstairs: the floor stood on is the homestead's upper floor (3.0 m, the
//     bedroom room's own floor in data/homes/shipped/homestead.ron), the HUD names the Bedroom,
//     and the camera stands within reach of the home pod's door;
//   - "general-services-1" wakes on the ship's deck (floor 0) inside General Services' box
//     (data/blueprints/ship_structure.ron), and the HUD names the hall by the place's words, the
//     key ship_zones.general_services_1 in data/i18n/en.json (2026-11-21: it said
//     "general-services-1:room_3", the hall's anonymous id);
//   - the death screen listed both places, home first (the usual place);
//   - with the one-person homestead's household (the showcase `household` verb, 2026-11-21),
//     whose world draws none of the ship's machines (data/machines/home_solo.ron says why), the
//     death screen in General Services' hall lists General Services and says its pod is not drawn
//     (the probe's wake.not_drawn, src/engine/wake.rs `entries`), and the family household is put
//     back after it.
// Pure: `judge(manifest, data)` returns { pass, checks }.

const fs = require("fs");
const path = require("path");

/** What the judge needs from the tree's data: the bedroom (home-local box and floor), General
 *  Services' ship box, the home pod's position and the home plot's origin is not needed: the
 *  probe's camera is in ship metres and the rig's home stands on p1 at the origin. */
function treeData(repo) {
  const home = fs.readFileSync(path.join(repo, "data/homes/shipped/homestead.ron"), "utf8");
  const ship = fs.readFileSync(path.join(repo, "data/blueprints/ship_structure.ron"), "utf8");
  const machines = fs.readFileSync(path.join(repo, "data/machines/home.ron"), "utf8");
  const words = JSON.parse(fs.readFileSync(path.join(repo, "data/i18n/en.json"), "utf8"));
  const num = (s) => s.split(",").map((v) => Number(v.trim()));
  const room = home.match(/id: "room-bedroom",\s*type_id: "[^"]*",\s*origin: \(([^)]*)\),\s*size: \(([^)]*)\)/);
  const gs = ship.match(/id: "general-services-1",[\s\S]*?origin: \(([^)]*)\),\s*body: \(\s*width: ([\d.]+),\s*depth: ([\d.]+),/);
  const pod = machines.match(/id: "respawn_pod_1",\s*machine: "respawn_pod",\s*room: "[^"]*",\s*offset: \(([^)]*)\)/);
  if (!room || !gs || !pod) throw new Error(`the tree lacks what the rig judges: bedroom ${!!room}, General Services ${!!gs}, home pod ${!!pod}`);
  const [ro, rs] = [num(room[1]), num(room[2])];
  const go = num(gs[1]);
  return {
    bedroom: { min: [ro[0], ro[1], ro[2]], max: [ro[0] + rs[0], ro[1] + rs[1], ro[2] + rs[2]], floor: ro[1] },
    services: { min: [go[0], go[1], go[2]], max: [go[0] + Number(gs[2]), go[1] + 3, go[2] + Number(gs[3])] },
    pod: num(pod[1]),
    // The words the HUD names General Services' hall with (room_types.rs `zone_words`).
    servicesName: (words.ship_zones || {}).general_services_1 || null,
  };
}

function judge(manifest, data) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const results = (manifest && (manifest.results || manifest.vantages || manifest)) || [];
  const byId = (id) => (Array.isArray(results) ? results.find((r) => r.id === id) : null);
  // The probe's camera_end is { pos, yaw, pitch } (ipc.rs recorder_camera); an array is taken as is.
  const posOf = (c) => (Array.isArray(c) ? c : c && Array.isArray(c.pos) ? c.pos : null);
  const fmt = (p) => (Array.isArray(p) ? `(${p.slice(0, 3).map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const inXZ = (p, box) => Array.isArray(p) && [0, 2].every((k) => p[k] >= box.min[k] - 0.05 && p[k] <= box.max[k] + 0.05);

  const shot = (id) => byId(id) && byId(id).screenshot;
  for (const id of ["respawn-pod-bedroom", "respawn-death-screen", "respawn-wake-home", "respawn-wake-general-services", "respawn-solo-general-services-pod"]) {
    add(`${id}_captured`, !!shot(id), shot(id) ? `${id}: ${shot(id)} (a person looks at it)` : `${id}: no picture was taken (${byId(id) ? byId(id).error || "no error recorded" : "the vantage did not run"})`);
  }

  // What the death screen itself listed (the probe's wake.listed, its GuiState list).
  const ids = byId("respawn-death-screen")?.probe?.wake?.listed || null;
  add(
    "death_screen_lists_both",
    Array.isArray(ids) && ids[0] === "home" && ids.includes("general-services-1"),
    `the death screen listed ${ids ? JSON.stringify(ids) : "nothing (no wake block in the probe)"}; home (the usual place) first and General Services are expected`,
  );

  const home = byId("respawn-wake-home")?.probe || {};
  const hw = home.wake || {};
  const hcam = posOf(home.camera_end);
  const floor = hw.storey ? hw.storey.floor : null;
  add(
    "home_upstairs",
    typeof floor === "number" && Math.abs(floor - data.bedroom.floor) < 0.05,
    `after waking at home the floor stood on is ${floor === null ? "unknown" : Number(floor).toFixed(2)} m; the bedroom's floor is ${data.bedroom.floor} m`,
  );
  add("home_in_bedroom", inXZ(hcam, data.bedroom), `the camera at ${fmt(hcam)} is ${inXZ(hcam, data.bedroom) ? "inside" : "NOT inside"} the bedroom (x ${data.bedroom.min[0]}..${data.bedroom.max[0]}, z ${data.bedroom.min[2]}..${data.bedroom.max[2]})`);
  const roomName = hw.storey && hw.storey.room ? hw.storey.room.name : null;
  add("home_hud_names_bedroom", roomName === "Bedroom", `the HUD names ${JSON.stringify(roomName)}`);
  const podGap = Array.isArray(hcam) ? Math.hypot(hcam[0] - data.pod[0], hcam[2] - data.pod[2]) : Infinity;
  add("home_at_the_pod", podGap < 1.5, `the camera stands ${podGap.toFixed(2)} m across the floor from the home pod at ${fmt(data.pod)} (a step outside its door is about 1 m)`);

  const gs = byId("respawn-wake-general-services")?.probe || {};
  const gw = gs.wake || {};
  const gcam = posOf(gs.camera_end);
  const gfloor = gw.storey ? gw.storey.floor : null;
  add("services_on_the_deck", typeof gfloor === "number" && Math.abs(gfloor) < 0.05, `after waking in General Services the floor stood on is ${gfloor === null ? "unknown" : Number(gfloor).toFixed(2)} m`);
  add("services_inside", inXZ(gcam, data.services), `the camera at ${fmt(gcam)} is ${inXZ(gcam, data.services) ? "inside" : "NOT inside"} General Services (x ${data.services.min[0]}..${data.services.max[0]}, z ${data.services.min[2]}..${data.services.max[2]})`);
  add("services_last_place", gw.last && gw.last.place === "general-services-1", `the game says it woke at ${JSON.stringify(gw.last ? gw.last.place : null)}`);
  const gName = gw.storey && gw.storey.room ? gw.storey.room.name : null;
  add(
    "services_hud_names_the_place",
    !!data.servicesName && gName === data.servicesName,
    `the HUD names ${JSON.stringify(gName)}; the hall is ${JSON.stringify(data.servicesName)} (en.json ship_zones.general_services_1)`,
  );

  // The one-person homestead's household, looking at General Services' pod (2026-11-21).
  const solo = byId("respawn-solo-general-services-pod")?.probe || {};
  const soloHousehold = solo.household ?? null;
  const scam = posOf(solo.camera_end);
  add("solo_household", soloHousehold === "solo", `the solo vantage's household is ${JSON.stringify(soloHousehold)}; "solo" (home_solo.ron) is expected`);
  add("solo_in_general_services", inXZ(scam, data.services), `the solo vantage's camera at ${fmt(scam)} is ${inXZ(scam, data.services) ? "inside" : "NOT inside"} General Services`);
  const sw = solo.wake || {};
  const soloListed = Array.isArray(sw.listed) ? sw.listed : null;
  const soloNotDrawn = Array.isArray(sw.not_drawn) ? sw.not_drawn : null;
  add(
    "solo_says_why_the_pod_is_not_drawn",
    !!soloListed && soloListed.includes("general-services-1") && !!soloNotDrawn && soloNotDrawn.includes("general-services-1") && !soloNotDrawn.includes("home"),
    `the solo household's death screen listed ${JSON.stringify(soloListed)}, saying the pod is not drawn under ${JSON.stringify(soloNotDrawn)}; General Services listed and said so, the home's own pod not, are expected`,
  );
  const back = byId("respawn-family-again")?.probe || {};
  const backHousehold = back.household ?? null;
  add("family_household_again", backHousehold === "family", `after the solo vantage the household is ${JSON.stringify(backHousehold)}; "family" is expected, so the rig's save keeps its own`);

  return { pass: checks.every((c) => c.ok), checks };
}

module.exports = { judge, treeData };
