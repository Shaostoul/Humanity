// The respawn rig's judge (scripts/lib/respawn-judge.js, `just verify-respawn`;
// docs/design/game-setting-and-intro.md rung 2): a run that went right passes, and each way it can
// go wrong fails its own check, judged against the tree's own data.
//
// Run: node --test scripts/tests/respawn-judge.test.js
//
// Seen red 2026-11-21: the "woke at the front door" case below is what the game did before
// respawners (Respawn stood the player at the Entry, (53.5, 1.7, 40.5), on the ground floor), and
// it fails home_upstairs, home_in_bedroom, home_hud_names_bedroom and home_at_the_pod.
const { test } = require("node:test");
const assert = require("node:assert");
const path = require("node:path");
const RJ = require(path.join(__dirname, "..", "lib", "respawn-judge.js"));

const DATA = RJ.treeData(path.join(__dirname, "..", ".."));
const WAKE_HOME = [37.5, 4.7, 31.468];
const WAKE_GS = [183.6, 1.7, 133];

function run(over = {}) {
  const v = [
    { id: "respawn-pod-bedroom", screenshot: "respawn-pod-bedroom.png" },
    { id: "respawn-death-screen", screenshot: "respawn-death-screen.png", probe: { wake: { listed: ["home", "general-services-1"] } } },
    { id: "respawn-wake-home", screenshot: "respawn-wake-home.png", probe: { camera_end: { pos: WAKE_HOME, yaw: 0, pitch: 0 }, wake: { last: { place: "home" }, storey: { floor: 3.0, room: { name: "Bedroom" } } } } },
    { id: "respawn-wake-general-services", screenshot: "respawn-wake-general-services.png", probe: { camera_end: WAKE_GS, wake: { last: { place: "general-services-1" }, storey: { floor: 0.0, room: { name: "General Services" } } } } },
    { id: "respawn-solo-general-services-pod", screenshot: "respawn-solo-general-services-pod.png", probe: { camera_end: { pos: [189, 1.7, 133], yaw: -1.5708, pitch: -0.12 }, household: "solo", wake: { listed: ["home", "general-services-1"], not_drawn: ["general-services-1"] } } },
    { id: "respawn-family-again", screenshot: "respawn-family-again.png", probe: { camera_end: [53.5, 1.7, 40.5], household: "family" } },
  ];
  for (const [id, f] of Object.entries(over)) {
    const i = v.findIndex((x) => x.id === id);
    v[i] = f(v[i]);
  }
  return { vantages: v };
}

const failed = (r) => r.checks.filter((c) => !c.ok).map((c) => c.id);

test("the tree has what the rig judges", () => {
  assert.deepEqual(DATA.bedroom.floor, 3);
  assert.ok(DATA.services.max[0] > DATA.services.min[0]);
  assert.deepEqual(DATA.pod.slice(0, 2), [37.5, 3]);
});

test("a run that went right passes", () => {
  const r = RJ.judge(run(), DATA);
  assert.ok(r.pass, JSON.stringify(failed(r)));
});

test("woke at the front door, as before respawners: the home checks fail", () => {
  const r = RJ.judge(
    run({ "respawn-wake-home": (x) => ({ ...x, probe: { camera_end: [53.5, 1.7, 40.5], wake: { last: null, storey: { floor: 0, room: { name: "Entry" } } } } }) }),
    DATA,
  );
  assert.deepEqual(failed(r), ["home_upstairs", "home_in_bedroom", "home_hud_names_bedroom", "home_at_the_pod"]);
});

test("woke on the ground floor under the bedroom: the floor check fails", () => {
  const r = RJ.judge(run({ "respawn-wake-home": (x) => ({ ...x, probe: { ...x.probe, wake: { ...x.probe.wake, storey: { floor: 0, room: { name: "Bedroom" } } } } }) }), DATA);
  assert.deepEqual(failed(r), ["home_upstairs"]);
});

test("woke in the Commons instead of General Services", () => {
  const r = RJ.judge(run({ "respawn-wake-general-services": (x) => ({ ...x, probe: { camera_end: [87.5, 1.7, 67.5], wake: { last: { place: "home" }, storey: { floor: 0 } } } }) }), DATA);
  assert.deepEqual(failed(r), ["services_inside", "services_last_place", "services_hud_names_the_place"]);
});

// Seen red 2026-11-21 on the arrival rungs' own real run (20261121-182216, re-judged with
// --dry-verdict): "FAIL services_hud_names_the_place: the HUD names "general-services-1:room_3";
// the hall is "General Services" (en.json ship_zones.general_services_1)".
test("the HUD names General Services' hall by its id: the name check fails", () => {
  assert.equal(DATA.servicesName, "General Services", "the key's words");
  const r = RJ.judge(run({ "respawn-wake-general-services": (x) => ({ ...x, probe: { ...x.probe, wake: { ...x.probe.wake, storey: { floor: 0, room: { name: "general-services-1:room_3" } } } } }) }), DATA);
  assert.deepEqual(failed(r), ["services_hud_names_the_place"]);
});

test("a death screen with one button and no list", () => {
  const r = RJ.judge(run({ "respawn-death-screen": (x) => ({ ...x, probe: { wake: { listed: [] } } }) }), DATA);
  assert.deepEqual(failed(r), ["death_screen_lists_both"]);
});

// The household verb never ran (a game from before 2026-11-21 ignores the key): the solo vantage
// is the family's, and nothing is put back. Seen red that way: the solo checks fail.
test("the solo vantage still on the family household: the solo check fails", () => {
  const r = RJ.judge(run({ "respawn-solo-general-services-pod": (x) => ({ ...x, probe: { ...x.probe, household: "family" } }) }), DATA);
  assert.deepEqual(failed(r), ["solo_household"]);
  const s = RJ.judge(run({ "respawn-family-again": (x) => ({ ...x, probe: { ...x.probe, household: "solo" } }) }), DATA);
  assert.deepEqual(failed(s), ["family_household_again"]);
});

// The death screen as the arrival rungs left it: General Services listed for the one-person
// homestead with no word that its pod is not drawn there. Seen red that way.
test("the solo death screen says nothing about the pod it does not draw: the check fails", () => {
  const r = RJ.judge(run({ "respawn-solo-general-services-pod": (x) => ({ ...x, probe: { ...x.probe, wake: { listed: ["home", "general-services-1"], not_drawn: [] } } }) }), DATA);
  assert.deepEqual(failed(r), ["solo_says_why_the_pod_is_not_drawn"]);
  const old = RJ.judge(run({ "respawn-solo-general-services-pod": (x) => ({ ...x, probe: { ...x.probe, wake: { listed: ["home", "general-services-1"] } } }) }), DATA);
  assert.deepEqual(failed(old), ["solo_says_why_the_pod_is_not_drawn"], "a game from before `not_drawn`");
});

test("a vantage that took no picture", () => {
  const r = RJ.judge(run({ "respawn-pod-bedroom": (x) => ({ id: x.id, error: "camera: timeout" }) }), DATA);
  assert.deepEqual(failed(r), ["respawn-pod-bedroom_captured"]);
});
