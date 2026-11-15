// The home's furniture as the walk's footprints (scripts/lib/home-furniture.js): what the storeys
// rig keeps its route clear of (BUG-306).
//
// Run: node --test scripts/tests/home-furniture.test.js   (in `just rig-tests`)
//
// The reader on a small machine home written the way data/machines/home.ron is (comments with
// brackets in them, a URL inside a string, ports nested in a def, an array of beds), the footprint
// of each shape as `Footprint::of_placed` makes it (crates/humanity-ship/src/crowd_nav/mesh.rs),
// the knee-to-eye rule, and the exact clearance of a straight walk; then the real file.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const HF = require("../lib/home-furniture.js");

const SMALL = `// A test home (a comment with (brackets) and "quotes" in it)
(
    ship_machines: Some("ship.ron"),
    catalog: {
        "wardrobe": (
            shape: "box",
            size: (1.20, 2.00, 0.60),
            label: "Wardrobe",
            // its maker's page (https://example.org/wardrobe) and a size in a comment: size: (9, 9, 9)
            ports: [
                (
                    utility: Electricity,
                    label: "lamp (5 W, size: (1, 1, 1))",
                    anchor: (0.0, 0.0, 0.0),
                ),
            ],
        ),
        "tank": (
            shape: "cylinder",
            size: (0.40, 1.50, 0.40),
            label: "see https://example.org/tank",
        ),
        "ball": (
            shape: "sphere",
            size: (0.25, 0.25, 0.25),
        ),
        "rug": (
            shape: "box",
            size: (3.00, 0.02, 2.00),
            label: "Rug",
        ),
        "bed_row": (
            shape: "box",
            size: (1.0, 0.5, 2.0),
        ),
    },
    instances: [
        (
            id: "wardrobe_1",
            machine: "wardrobe",
            room: "room-a",
            offset: (10.0, 3.0, 20.0),
            rotation: 90.0,
            zone: "home",
        ),
        (
            id: "tank_1",
            machine: "tank",
            room: "room-a",
            offset: (12.0, 0.0, 20.0),
            rotation: 0.0,
            zone: "home",
        ),
        (
            id: "ball_1",
            machine: "ball",
            room: "room-a",
            offset: (14.0, 0.0, 20.0),
            rotation: 0.0,
            zone: "home",
        ),
        (
            id: "rug_1",
            machine: "rug",
            room: "room-a",
            offset: (10.0, 3.0, 24.0),
            rotation: 0.0,
            zone: "home",
        ),
        (
            id: "ghost_1",
            machine: "not_in_the_catalog",
            room: "room-a",
            offset: (0.0, 0.0, 0.0),
            rotation: 0.0,
            zone: "home",
        ),
    ],
    arrays: [
        (
            machine: "bed_row",
            room: "room-b",
            origin: (30.0, 0.0, 50.0),
            rows: 2,
            cols: 3,
            spacing: (2.5, 3.0),
            id_prefix: "bed",
            zone: "home",
        ),
    ],
    connections: [],
)
`;

const near = (a, b, what) => assert.ok(Math.abs(a - b) < 1e-9, `${what}: ${a} is not ${b}`);

test("the reader reads the catalog, the instances and the arrays, comments and strings aside", () => {
  const h = HF.readMachineHome(SMALL);
  assert.deepStrictEqual(Object.keys(h.catalog), ["wardrobe", "tank", "ball", "rug", "bed_row"]);
  assert.deepStrictEqual(h.catalog.wardrobe, { shape: "box", size: [1.2, 2, 0.6], label: "Wardrobe" }, "the def's own size, not a port's or a comment's");
  assert.strictEqual(h.catalog.tank.label, "see https://example.org/tank", "a URL inside a string is not a comment");
  assert.deepStrictEqual(
    h.placements.map((p) => p.id),
    ["wardrobe_1", "tank_1", "ball_1", "rug_1", "ghost_1", "bed_0", "bed_1", "bed_2", "bed_3", "bed_4", "bed_5"]
  );
  const w = h.placements[0];
  assert.deepStrictEqual([w.machine, w.offset, w.rotation, w.zone], ["wardrobe", [10, 3, 20], 90, "home"]);
  // MachineHome::all_instances: row r, column c at origin + (c * spacing.0, 0, r * spacing.1).
  assert.deepStrictEqual(h.placements.find((p) => p.id === "bed_4").offset, [32.5, 0, 53]);
});

test("each shape's footprint is the one Footprint::of_placed makes", () => {
  const fps = HF.footprints(HF.readMachineHome(SMALL));
  assert.ok(!fps.some((f) => f.id === "ghost_1"), "a machine the catalog does not have is left out, as placements leaves it");
  const by = (id) => fps.find((f) => f.id === id);
  // A box: half its width and depth, turned by its rotation, from its floor to its height.
  const w = by("wardrobe_1");
  assert.deepStrictEqual([w.centre, w.half, w.y0, w.y1], [[10, 20], [0.6, 0.3], 3, 5]);
  near(w.yaw, Math.PI / 2, "turned 90 degrees");
  // Turned 90 degrees, its 1.2 m width runs along z: 0.3 m each way in x, 0.6 m in z.
  near(HF.distance(w, 10.3, 20), 0, "on its east face");
  near(HF.distance(w, 10, 20.6), 0, "on its south face");
  near(HF.distance(w, 11, 20), 0.7, "0.7 m east of it");
  // A cylinder: size.0 is its radius, each way.
  assert.deepStrictEqual([by("tank_1").half, by("tank_1").y1], [[0.4, 0.4], 1.5]);
  // A sphere: its radius each way, standing on the floor (its centre a radius up).
  assert.deepStrictEqual([by("ball_1").half, by("ball_1").y0, by("ball_1").y1], [[0.25, 0.25], 0, 0.5]);
});

test("a piece stops a body only across its knee-to-eye band", () => {
  const fps = HF.footprints(HF.readMachineHome(SMALL));
  const by = (id) => fps.find((f) => f.id === id);
  const [EYE, KNEE] = [1.7, 0.4];
  assert.ok(HF.stopsBody(by("wardrobe_1"), 3, EYE, KNEE), "a wardrobe upstairs stops a body upstairs");
  assert.ok(!HF.stopsBody(by("wardrobe_1"), 0, EYE, KNEE), "and not one on the floor under it");
  assert.ok(!HF.stopsBody(by("rug_1"), 3, EYE, KNEE), "a rug under the knee is walked over");
  assert.ok(HF.stopsBody(by("ball_1"), 0, EYE, KNEE), "a 0.5 m ball reaches over the knee");
});

test("a straight walk's clearance is exact: through a piece 0, past its face the gap, past its corner the corner's", () => {
  const w = HF.footprints(HF.readMachineHome(SMALL)).find((f) => f.id === "wardrobe_1"); // x 9.7..10.3, z 19.4..20.6
  assert.strictEqual(HF.clearance(w, [8, 20], [12, 20]), 0, "straight through it");
  assert.strictEqual(HF.clearance(w, [8, 20], [10, 20]), 0, "into it and stopping inside");
  near(HF.clearance(w, [8, 19], [12, 19]), 0.4, "0.4 m north of its north face, all the way along");
  near(HF.clearance(w, [10.8, 15], [10.8, 25]), 0.5, "0.5 m east of its east face");
  near(HF.clearance(w, [8, 20], [9.2, 20]), 0.5, "stopping 0.5 m short of it");
  // Diagonally past its north-east corner (10.3, 19.4), x - z = -9.1: the line x - z = -8.6 runs
  // 0.5/sqrt 2 off it, and the walk passes the corner's foot on it, (10.55, 19.15).
  near(HF.clearance(w, [10.0, 18.6], [11.4, 20.0]), 0.5 / Math.SQRT2, "past its corner");
  near(HF.clearance(w, [10.8, 19.4], [11.8, 20.4]), 0.5, "a walk that starts past the corner: its start is nearest");
  near(HF.clearance(w, [10.8, 19.4]), 0.5, "a placement 0.5 m east of the corner");
});

test("the real homestead's furniture reads, the pieces the storeys route keeps clear of among it", () => {
  const h = HF.readMachineHome(fs.readFileSync(path.join(__dirname, "..", "..", "data", "machines", "home.ron"), "utf8"));
  const fps = HF.footprints(h);
  assert.ok(Object.keys(h.catalog).length > 100 && fps.length > 250, `${Object.keys(h.catalog).length} catalog entries, ${fps.length} pieces`);
  assert.strictEqual(fps.length, h.placements.length, "every placement's machine is in the catalog");
  const shelf = fps.find((f) => f.id === "shelf_6");
  assert.ok(shelf, "shelf_6, against the bedroom's east wall");
  // A 0.9 x 0.56 shelf turned 90 degrees at (41.652, 34.0): x 41.372 to 41.932, z 33.55 to 34.45,
  // flush against the bedroom's skirting, whose face stands 18 mm off the wall's at 41.95 (a
  // torus_145 skirting, data/blueprints/room_finishes.ron; 2026-11-15). Until 2026-11-14 it stood
  // at (41.75, 36.6), 0.08 m into the wall (crates/humanity-ship/src/furniture_fit_tests.rs); moved
  // flush to (41.652, 36.55), it still stood in the bedroom door's clear zone, so on 2026-11-15 it
  // moved 2.55 m south along the wall, between the standing mirror and the door to the dressing
  // room (crates/humanity-ship/src/doorway_tests.rs).
  near(HF.distance(shelf, 41.372, 34.0), 0, "its west face");
  near(HF.distance(shelf, 41.932, 34.0), 0, "its east face, on the skirting's face");
  near(HF.distance(shelf, 41.652, 34.45), 0, "its south face");
  const wardrobe = fps.find((f) => f.id === "wardrobe_3");
  assert.ok(wardrobe && HF.stopsBody(wardrobe, 3, 1.7, 0.4), "wardrobe_3 stands upstairs, in the dressing room against U7");
});
