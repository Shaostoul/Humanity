# Node-based conduits -- Stage 1 (implementation-ready, grounded 2026-06-27)

> The operator wants pipes/conduits as a NODE GRAPH (main/sub/subsub lines; edit nodes, software
> auto-routes the mesh). This is the additive Stage-1 plan that reuses the existing route_conduit.

## Summary
GROUND NODE-BASED CONDUITS, Stage 1: introduce a per-home conduit NODE GRAPH (junction nodes + edges whose endpoints are either a machine id or a node id) saved on MachineHome, with a draggable ground orb per node that re-routes the pipe live. The current delete-only point-to-point `connections: Vec<MachineConnection>` keep working UNCHANGED; the new graph is ADDITIVE. Each graph edge is routed with the EXISTING `conduits::route_conduit` exactly as a machine-to-machine connection is today, by resolving every endpoint (machine OR node) to a Vec3 anchor and routing leg-by-leg. The smallest shippable increment: a node has a position + a kind; an edge connects two endpoints; the editor lets you place a node (click floor with a held "conduit node" tool), move it (drag its orb, mirroring the corner-node drag), and branch from a machine/node to it. Stage 2+ layers main/sub/subsub hierarchy + auto-routing on top of this same graph.

## Data model
Add to src/machines.rs (pure serde, compiles under every feature flag, infinite-of-X: pure data on MachineHome, no hardcoded arrays).

```rust
/// A conduit junction node: a draggable point in the home where conduit edges meet/branch.
/// Position is ABSOLUTE world (x,y,z) in a box home (box min corner at world origin), matching
/// MachineInstance.offset semantics in box_mode. (Stage 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConduitNode {
    pub id: String,                 // unique within the home, e.g. "node_0"
    pub pos: (f32, f32, f32),       // absolute world metres; y is the run height
    /// Tier in the eventual hierarchy: 0 = main, 1 = sub, 2 = subsub. Stage 1 ignores it for
    /// routing (always 0); Stage 2 uses it. Defaulted so older RON parses.
    #[serde(default)]
    pub tier: u8,
    /// Utility kind hint for color when an edge does not override ("water"|"power"|"gas"|...).
    #[serde(default)]
    pub kind: String,
}

/// One endpoint of a conduit edge: either a placed machine id OR a conduit node id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConduitEnd {
    Machine(String),  // -> a MachineInstance.id (incl. array-expanded)
    Node(String),     // -> a ConduitNode.id
}

/// A routed conduit edge between two endpoints. Replaces a straight machine->machine link with a
/// graph edge that can pass through junction nodes. (Stage 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConduitEdge {
    pub from: ConduitEnd,
    pub to: ConduitEnd,
    pub kind: String,  // "water"|"power"|"greywater"|"gas" -> ConduitKind::for_resource (reused)
}
```

On `MachineHome`, two new optional vecs (so every existing home.ron parses byte-unchanged, and the legacy `connections` field is untouched):

```rust
    #[serde(default)]
    pub conduit_nodes: Vec<ConduitNode>,
    #[serde(default)]
    pub conduit_edges: Vec<ConduitEdge>,
```

This is the EXACT graph the operator asked for: nodes = junction positions the user edits; edges = node-or-machine endpoints. `tier` is the seed for Stage-2 main/sub/subsub. No code-side arrays; everything lives in data/machines/home.ron.

## Files
- C:\Humanity\src\machines.rs
- C:\Humanity\src\lib.rs
- C:\Humanity\src\gui\pages\construction.rs
- C:\Humanity\src\gui\mod.rs
- C:\Humanity\data\machines\home.ron

## Stage 1 steps
1. DATA MODEL (src/machines.rs): add the ConduitNode / ConduitEnd / ConduitEdge structs above, and the two `#[serde(default)]` Vec fields (conduit_nodes, conduit_edges) on MachineHome. Update the four MachineHome literals inside machines.rs's own `#[cfg(test)]` mod (they construct MachineHome { ... } positionally with `loops: Vec::new()`) to add `conduit_nodes: Vec::new(), conduit_edges: Vec::new()` so they keep compiling. The serde defaults mean home.ron, the round-trip tests, and save() are unaffected.

2. GRAPH MUTATORS (src/machines.rs, mirror the existing connection helpers so the editor + AI share validated wiring): `unique_node_id(&self, base:&str)->String` (like unique_instance_id but over conduit_nodes ids); `add_conduit_node(&mut self, pos:(f32,f32,f32), kind:&str)->String` (mint id, push, return id); `move_conduit_node(&mut self, id:&str, pos:(f32,f32,f32))->bool`; `remove_conduit_node(&mut self, id:&str)` (drop the node AND prune every edge whose from/to is Node(id), exactly like remove_instance prunes connections); `add_conduit_edge(&mut self, from:ConduitEnd, to:ConduitEnd, kind:&str)->bool` (refuse self-edge, refuse endpoint that does not resolve to a live machine id or existing node id, refuse exact duplicate); `remove_conduit_edge(&mut self, idx:usize)->bool`. Also extend remove_instance to additionally prune conduit_edges referencing Machine(id) so deleting a machine never leaves a dangling edge (one added line + the existing connections retain).

3. ENDPOINT RESOLVER (src/machines.rs): `pub fn conduit_anchor(&self, end:&ConduitEnd, placements:&[PlacedMachine], box_dims:(f32,f32,f32)) -> Option<(f32,f32,f32)>` returning, for Machine(id) the SAME low pipe anchor lib.rs uses today `(p.pos.0, p.floor_y+0.35, p.pos.2)`, and for Node(id) the node's clamped `pos` (clamp x into 0.3..width-0.3, z into 0.3..depth-0.3, y into 0.1..height to match machine clamping). This keeps node anchors in the identical coordinate model as machine anchors so route_conduit treats them uniformly.

4. ROUTING REUSE (src/lib.rs::rebuild_connection_objects, NO new routing math): after the existing `for c in &connections` loop that already calls `route_conduit(a, b, kind, service_y, shell_mat, &walls)`, add a parallel `for e in &conduit_edges` loop. Resolve `a = home.conduit_anchor(&e.from, &placements, box_dims)` and `b = home.conduit_anchor(&e.to, &placements, box_dims)`; `continue` if either is None (dangling). Then call the SAME `conduits::route_conduit(a.into(), b.into(), ConduitKind::for_resource(&e.kind), service_y, shell_mat, &walls)` and feed its `route.points`/`route.fittings` into the EXACT SAME cylinder + fitting emission block already there (factor that emission body into a small local closure `emit_route(state, &route, kind)` so both loops share it verbatim -- up/across/down legs, ceiling hangers, passthrough gaskets, copper-vs-flexible material). Clone conduit_edges alongside connections where `connections` is cloned at the top of the fn. Result: a node-graph edge renders as a real routed pipe with zero new mesh code.

5. NODE STATE (src/gui/mod.rs GuiState): add `pub construction_node_selected: Option<String>` (the conduit node selected for the right panel; mutually exclusive with construction_machine_selected/_wall_selected/_light_selected -- clear the others when set, like the existing selects). Add `construction_machines_dirty` is ALREADY the rebuild trigger for machine/connection edits and ALSO drives rebuild_connection_objects via the v0.525 machine-only path -- confirm conduit-node edits set `construction_machines_dirty = true` so the existing choke point at lib.rs:5232 rebuilds machines AND conduits live (rebuild_machine_objects is called there; ensure rebuild_connection_objects is invoked too -- it already is at the end of rebuild_machine_objects, line 941). So setting construction_machines_dirty is sufficient; no new dirty flag needed.

6. EDITOR LIST (src/gui/pages/construction.rs::draw_machines_and_connections): under the existing 'Utility lines' CollapsingHeader, add a sibling 'Conduit nodes (N)' CollapsingHeader listing each conduit_node as a selectable row + an 'x' remove button (calls remove_conduit_node, set construction_machines_dirty). Add a 'Branch' control row: a from-combo (machines + nodes), a to-combo (machines + nodes), a kind-combo (water/power/greywater/gas, reuse the same list as connections), and a 'Branch' button calling add_conduit_edge. List existing conduit_edges grouped by kind with 'x' removers, mirroring the connections list exactly. This gives a fully usable node graph from the panel even before the viewport interaction lands.

7. VIEWPORT PLACE-NODE (src/lib.rs + construction.rs palette): add a 'Conduit node' entry to the palette as a special place tool. Simplest Stage-1 wiring: reuse construction_place_type with a sentinel value (e.g. set a new `state.gui_state.construction_place_conduit_node: bool` when its palette button is clicked) and in try_place_held_machine's sibling, on a floor click call `home.add_conduit_node((hx, service_y_or_floor+0.35, hz), default_kind)` then set construction_machines_dirty. Place at the low pipe height (floor+0.35) so it visually sits on the run. Keep the tool held so several can be dropped, matching machine placement.

8. VIEWPORT DRAG-NODE (src/lib.rs, mirror try_grab_node/apply_node_drag for conduit nodes): add `construction_conduit_node_grab: Option<String>` to EngineState (next to construction_node_grab around lib.rs:3337). In the build-mode click handler, after try_grab_node/try_pick_machine miss, call a new `try_grab_conduit_node(state)` that ray-tests each conduit_node pos (pick radius ~0.5 like the 0.7 corner radius) and stores the grabbed node id. Add `apply_conduit_node_drag(state)` mirroring apply_node_drag: tap-vs-drag threshold, `cursor_floor_hit`, optional grid snap, then `home.move_conduit_node(id, (hx, node.pos.1, hz))` and set construction_machines_dirty. On release with no drag, set construction_node_selected for the panel. Include construction_conduit_node_grab in the same release-clear sites as construction_node_grab (lines ~4456, 5088, 5107) and the compute_construction_hover early-out guard (line 1670).

9. NODE ORB RENDER (src/lib.rs ~line 5735, mirror the corner-orb loop): after the corner orbs, iterate conduit_nodes and push an overlay RenderObject sphere at each node.pos using a DISTINCT material (e.g. cyan emissive, add `construction_conduit_node_mat`) so nodes read differently from yellow corner orbs and red wall orbs; use the existing hot/hover material logic keyed on construction_conduit_node_grab / a HoverGizmo::ConduitNode(id) variant (add it to the HoverGizmo enum + compute_construction_hover). Reuse the existing construction_node_mesh sphere. Orb radius 0.05 like the others.

10. TESTS (src/machines.rs #[cfg(test)]): (a) conduit_node + edge round-trip through save()/load() (mirror placed_lights/locks round-trip tests); (b) add_conduit_edge validates (self-edge, dangling endpoint, duplicate all refused); (c) remove_conduit_node prunes its edges; (d) remove_instance prunes Machine-referencing conduit_edges; (e) conduit_anchor resolves a Machine end to floor+0.35 and a Node end to its clamped pos. Run via `cargo test --features native --lib` (the bin-link PDB workaround in CLAUDE.md). Verify RELAY build green: `cargo check --features relay --no-default-features` (machines.rs is pure data so it stays relay-safe; the lib.rs render additions are already inside the native-gated load_world/render path -- confirm no new ungated module).

11. DATA SEED (data/machines/home.ron): leave conduit_nodes/conduit_edges absent (serde default empty) so the shipped home is byte-unchanged and the existing connections render exactly as before -- prove the additive guarantee. Optionally add ONE example node + edge in a comment or a tiny demo to exercise the path in-world.

## Later stages
- STAGE 2 -- HIERARCHY: use ConduitNode.tier (0 main / 1 sub / 2 subsub). A main line runs the home spine at service height; sub lines branch off main nodes; subsub lines reach individual fixtures. Editor: placing a node picks its tier (or infers it from what you branched off). Routing prefers running along the parent tier's line before dropping to the child, so the auto-routed mesh reflects the trunk-and-branch topology instead of independent per-edge Manhattan runs.
- STAGE 3 -- AUTO-ROUTING / IMPLICIT EDGES: the user edits NODES and the software auto-derives edges by connecting each machine to its nearest node of the matching utility kind, and each node to its parent-tier node, building a minimum-spanning trunk per kind. The explicit conduit_edges become an override/manual layer on top of the auto graph (like the existing connections are manual). route_conduit is extended (or wrapped) to route a whole multi-node polyline in one call rather than per-edge, sharing service-height legs between branches so parallel runs bundle along the ceiling.
- STAGE 4 -- MIGRATE LEGACY CONNECTIONS: optionally lift each existing MachineConnection into a 2-endpoint ConduitEdge (Machine->Machine) so there is ONE rendering path; keep the old `connections` field as a deprecated alias that loads into conduit_edges, then retire it. Until then both render side by side with zero risk.
- STAGE 5 -- KIND-AWARE NODE TYPING + VALIDATION: a node carries which utility kinds pass through it; the buildability_report gains conduit checks (every fixture reaches a source through the graph; copper-only for potable enforced at the graph level). Console verbs (exec_construction_command) gain add_node/move_node/branch/rm_node to match the existing add_wall/add_light text commands, and the introspection snapshot (to_introspection_json) gains a conduit-graph block so an AI can see + edit the node graph.

## Risks
- Positional struct literals: machines.rs's own test module builds `MachineHome { catalog, instances, arrays, connections, loops }` positionally in ~10 tests -- adding fields there is a compile break unless every literal gets `conduit_nodes: Vec::new(), conduit_edges: Vec::new()`. (The serde defaults cover all RON/runtime construction; only these in-file test literals need editing.) Grep `MachineHome {` in machines.rs before building.
- Anchor coordinate model: machine anchors are box-mode ABSOLUTE world (offset.0/offset.2 clamped), and rebuild_connection_objects early-returns if room_bounds/placements are empty. ConduitNode.pos MUST use the same absolute model + the same clamps, or a node's pipe will not line up with its machine's pipe. The resolver must apply the identical 0.3..(dim-0.3) clamp placements() uses.
- Rebuild trigger coupling: conduit edits piggyback on construction_machines_dirty, which at lib.rs:5232 calls rebuild_machine_objects (which ends by calling rebuild_connection_objects). Confirm that path runs in box mode and is not gated behind a machine-count-change branch that could skip a node-only edit -- if so, set the flag in a way that forces the rebuild_connection_objects call (it is unconditional at the end of rebuild_machine_objects:941, so this should hold; verify).
- Relay build: the routing additions live in lib.rs's render/load_world path which is already native-gated; machines.rs is pure serde and relay-safe. Do NOT add any renderer/glam import to machines.rs (conduit_anchor returns plain tuples, no Vec3) so it stays compilable under `--features relay --no-default-features`. Run that check before pushing (the v0.416 ungated-save_load lesson).
- Gizmo pick precedence: adding a third draggable orb kind (conduit nodes) alongside corner orbs and machine picks means the build-mode click order (try_grab_node -> try_pick_machine -> try_grab_conduit_node -> room grab) must be deliberate, and compute_construction_hover must pick the nearest across all kinds. Keep conduit-node pick radius modest (~0.5) so it does not steal clicks from nearby corner orbs.
- egui borrow pattern: in draw_machines_and_connections the code snapshots conns/machines into owned Vecs BEFORE mutating state.home_machines (to avoid an aliasing borrow across the closure). The new conduit_nodes/edges UI must follow the same snapshot-then-mutate pattern (collect ids/labels first, apply add/remove after) or it will not compile.
- Cargo.lock / version SOP: this is a Rust change -> minor bump (0.X.0), `just verify`, archive a versioned exe after release (build-game), and sign the release. Tests must run via `cargo test --features native --lib` due to the Windows PDB LNK1318 limit on bin-linking integration tests.
