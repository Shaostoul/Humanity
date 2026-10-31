//! The 3D home's meshes, built and uploaded: homestead rooms, hull wrap, machines and their
//! pipes, the garden's plants, door and window panels, and room lights.
//! The plants are built on a worker and uploaded within a frame budget. Shared by the first world
//! load and the build editor's live rebuild (`rebuild_homestead`, `rebuild_machine_objects`),
//! which reuses the GPU mesh and material slots in place.
use glam::{Quat, Vec3};
use crate::engine::color::hsv_to_rgb;
use crate::engine::home_spawn::spawn_home_machine_entity;
use crate::engine::state::{EngineState, GrowSpot};
use crate::renderer::mesh::Mesh;
use crate::renderer::RenderObject;
use crate::ship::ship_structure::{zone_body, ShipStructure};

/// The glass roof's (metallic, roughness, emissive): the roof, and a glass floor slab drawn like
/// it (ship homes increment 7). Its colour and shader type are `home_structure::GLASS_ROOF_RGBA`
/// and `GLASS_ROOF_TYPE`.
const GLASS_ROOF_LOOK: (f32, f32, f32) = (0.0, 0.05, 0.0);

/// The opaque ceiling's (rgba, metallic, roughness, shader type): the roof when it is not glass,
/// and the same look a solid slab's underside is drawn in (`home_structure::OPAQUE_CEILING_RGBA`,
/// uploaded through the floors with the same metallic and roughness).
const OPAQUE_CEILING_LOOK: ([f32; 4], f32, f32, f32) =
    (crate::ship::home_structure::OPAQUE_CEILING_RGBA, 0.0, 0.8, crate::ship::home_structure::OPAQUE_CEILING_TYPE as f32);

/// A mesh slot and its material slot, as the renderer numbers them.
pub(crate) type Slot = (usize, usize);

/// How a shell family's mesh is drawn: (rgba, metallic, roughness, shader type, emissive).
type Look = ([f32; 4], f32, f32, f32, f32);

/// What the home's drawing keeps between rebuilds (`EngineState::home_draw`; ship homes increment 7
/// review, 2026-10-21).
#[derive(Default)]
pub(crate) struct HomeDraw {
    /// Mesh and material slots the home's shell let go of (a family that needs fewer meshes after a
    /// rebuild: the editor's cut-away taking the upper floor's slabs out, a room removed), each
    /// mesh swapped for a stand-in so its buffers are freed. The next family that needs more takes
    /// one of these before the renderer is asked for a new slot (`assign_slots`), so the slots the
    /// shell holds never pass the most it has needed at once. The renderer only appends: before
    /// this, every toggle of the cut-away added the upper floor's slots again.
    spare: Vec<Slot>,
    /// The mesh and material slot of each machine body, in placement order, from the last world
    /// entry or machine rebuild (`machine_body_slots`). A machine body's material is its own
    /// (a model's shared textured material is used instead, but the slot keeps its own), so a slot
    /// can go to the spares and come back as anything. Before the 2026-10-21 follow-ups the world
    /// entry asked the renderer for a fresh slot per machine every time, orphaning the last entry's.
    machine_slots: Vec<Slot>,
    /// The cut-aways as last drawn, empty when the whole ship is drawn (the build editor's one, or
    /// one per home of the level the Maps page shows cut open, Maps increment 3): set by every
    /// upload of the home's shell (`apply_homestead_meshes` draws the whole ship, which a world load
    /// does; `rebuild_homestead` then records the cuts it drew). It is THE record of what is drawn,
    /// so `editor::follow_storey_view` redraws when it is not what is wanted
    /// (`view_stale`, `editor::wanted_cuts`). The machines, their screens, the door panels, the
    /// elevator cars, the plants and the grow tents come from the whole ship, so they are left out
    /// where it says (`Cuts::hides`).
    pub(crate) cut: crate::engine::editor::Cuts,
    /// Which door panels stand at an elevator's landing (`footing::lift_landing_open` answers for
    /// them), parallel to `EngineState::door_panels`. Worked out with the doors
    /// (`rebuild_door_panels`), so each frame asks the elevators about these doors only.
    landing_doors: Vec<bool>,
    /// Counts every upload of the home's shell and every rebuild of its machines (Maps increment
    /// 3): the plan tiles (engine/map_tiles.rs) look for what an edit touched only when it moves.
    pub(crate) edits: u64,
}

impl HomeDraw {
    /// The home's shell was just uploaded whole (`apply_homestead_meshes`: every world load, and
    /// every rebuild before it says which cut it drew).
    pub(crate) fn drew_whole_ship(&mut self) {
        self.cut = Default::default();
    }

    /// The home's shell was just drawn as `cut` (`rebuild_homestead`, after its upload): the
    /// editor's cut-away, a Maps level's, or none for the whole ship.
    pub(crate) fn drew_cut(&mut self, cut: impl Into<crate::engine::editor::Cuts>) {
        self.cut = cut.into();
    }

    /// True when the home drawn is not the view `want` (the cut-away the editor should show, None
    /// for the whole ship), so it must be drawn again (`editor::follow_storey_view`). A world load
    /// draws the whole ship (`apply_homestead_meshes`), so with the editor open on a storey with
    /// another over it the cut-away is stale after it, and is drawn again on the next frame.
    pub(crate) fn view_stale(&self, want: impl Into<crate::engine::editor::Cuts>) -> bool {
        self.cut != want.into()
    }
}

/// THE SLOTS FOR THE HOME'S MACHINE BODIES: `need` of them, for a world entry or a machine
/// rebuild (2026-10-21, the increment 7 follow-ups). The machines' slots from last time are kept
/// first, what is needed beyond them comes from the home's spares (`HomeDraw::spare`, shared with
/// the shell), and only then from `fresh`; what is no longer needed goes to the spares
/// (`assign_slots`, one family). Returns the slots, in placement order, and those let go that
/// nothing took (to park their meshes). The world entry used to ask the renderer for a new slot
/// per machine on every entry, orphaning the old ones: the renderer only appends, so its lists
/// grew by the household's machine count at every entry.
pub(crate) fn machine_body_slots(draw: &mut HomeDraw, need: usize, fresh: impl FnMut() -> Slot) -> (Vec<Slot>, Vec<Slot>) {
    let prior = std::mem::take(&mut draw.machine_slots);
    let (mut got, idle) = assign_slots(&mut draw.spare, vec![(prior, need)], fresh);
    let slots = got.pop().unwrap_or_default();
    draw.machine_slots = slots.clone();
    (slots, idle)
}

/// `machine_body_slots` against the renderer: the slots for `need` machine bodies, a fresh one
/// holding a stand-in until it is filled, and every slot let go parked (its mesh freed).
pub(crate) fn take_machine_slots(state: &mut EngineState, need: usize) -> Vec<Slot> {
    let renderer = &mut state.renderer;
    let (slots, idle) = machine_body_slots(&mut state.home_draw, need, || new_slot(renderer));
    for s in idle {
        park_slot(&mut state.renderer, s);
    }
    slots
}

/// THE SLOTS FOR ONE REBUILD of a set of mesh families (the home's shell: floors, walls, trim,
/// ...). `families`: each family's slots from the last rebuild and how many meshes it needs now. A
/// family keeps its own slots first; what it needs beyond them comes from `spare`, and only then
/// from `fresh` (a new slot from the renderer); what it holds beyond its need goes to `spare`.
/// Every family gives back before any takes, so one shrinking while another grows reuses. Returns
/// each family's slots, in order, and the slots let go that nothing took (to park their meshes).
pub(crate) fn assign_slots(
    spare: &mut Vec<Slot>,
    families: Vec<(Vec<Slot>, usize)>,
    mut fresh: impl FnMut() -> Slot,
) -> (Vec<Vec<Slot>>, Vec<Slot>) {
    let mut freed = Vec::new();
    let mut held: Vec<(Vec<Slot>, usize)> = families
        .into_iter()
        .map(|(mut slots, need)| {
            if slots.len() > need {
                freed.extend(slots.drain(need..));
            }
            (slots, need)
        })
        .collect();
    spare.extend(freed.iter().copied());
    for (slots, need) in held.iter_mut() {
        while slots.len() < *need {
            slots.push(spare.pop().unwrap_or_else(&mut fresh));
        }
    }
    freed.retain(|s| spare.contains(s));
    (held.into_iter().map(|(s, _)| s).collect(), freed)
}

/// How many meshes each of the home's shell families needs for `m`, in the order
/// `apply_homestead_meshes` keeps their slots: the opaque floors; the combined walls; the wall
/// materials with the glass floor slabs after them; the trim; the windows; the mirrors; the glass
/// roofs' ceilings; the opaque roofs' ceilings. A combined family needs one mesh, or none when empty.
pub(crate) fn shell_family_needs(m: &crate::ship::fibonacci::HomesteadMeshes) -> [usize; 8] {
    let one = |b: &(Vec<crate::vertex::Vertex>, Vec<u32>)| usize::from(!b.0.is_empty());
    let glass = m.floors.iter().filter(|f| !is_opaque_floor(f.2)).count();
    [
        m.floors.len() - glass,
        one(&m.walls),
        m.material_walls.len() + glass,
        one(&m.trim),
        one(&m.windows),
        one(&m.mirrors),
        one(&m.ceilings),
        one(&m.ceilings_opaque),
    ]
}

/// True for a floor drawn in the opaque pass with the others; a GLASS floor slab (ship homes
/// increment 7, see-through: its colour's alpha under 1) goes with the glass walls into the
/// transparent list, in the glass roof's material.
fn is_opaque_floor(rgba: [f32; 4]) -> bool {
    rgba[3] >= 0.999
}

/// A one-triangle stand-in mesh, every corner at the origin so nothing is drawn: what a slot holds
/// while nothing uses it, so the mesh it held is freed at once.
fn stand_in_mesh(device: &wgpu::Device) -> Mesh {
    let v = crate::vertex::Vertex { position: [0.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0; 2] };
    Mesh::from_vertices(device, &[v; 3], &[0, 1, 2])
}

/// A new mesh and material slot from the renderer, holding a stand-in until it is filled.
fn new_slot(r: &mut crate::renderer::Renderer) -> Slot {
    let mesh = stand_in_mesh(&r.device);
    (r.add_mesh(mesh), r.add_material_full([1.0; 4], 0.0, 1.0, 0.0, 0.0))
}

/// Put a slot nothing uses to rest: its mesh becomes a stand-in (the buffers it held are freed).
fn park_slot(r: &mut crate::renderer::Renderer, (mi, _): Slot) {
    let mesh = stand_in_mesh(&r.device);
    r.replace_mesh(mi, mesh);
}

/// Fill `slot` with a mesh of `verts`/`indices` drawn in `look`.
fn fill_slot(r: &mut crate::renderer::Renderer, (mi, ma): Slot, verts: &[crate::vertex::Vertex], indices: &[u32], look: Look) {
    let mesh = Mesh::from_vertices(&r.device, verts, indices);
    r.replace_mesh(mi, mesh);
    let (rgba, metallic, roughness, ty, emissive) = look;
    r.update_material_full(ma, rgba, metallic, roughness, ty, emissive);
}

/// A combined family's one slot (`slots` holds it, or nothing when the family is empty), filled.
fn fill_one(r: &mut crate::renderer::Renderer, slots: &[Slot], mesh: &(Vec<crate::vertex::Vertex>, Vec<u32>), look: Look) -> Option<Slot> {
    let slot = *slots.first()?;
    fill_slot(r, slot, &mesh.0, &mesh.1, look);
    Some(slot)
}

/// Upload a freshly generated set of homestead meshes into the renderer + state slots
/// (v0.455). Shared by the initial world load AND the construction editor's live rebuild.
/// The renderer only appends, so every slot is reused: a family keeps its own slots (v0.531), and
/// since the increment 7 review (2026-10-21) the slots a family no longer needs go to the home's
/// spares (`HomeDraw::spare`) for the next family that needs more, their meshes freed, where they
/// used to be orphaned. A per-frame rebuild during a room drag, or the editor's cut-away shown and
/// hidden again and again, never grows the renderer's lists (`assign_slots`).
pub(crate) fn apply_homestead_meshes(state: &mut EngineState, homestead: crate::ship::fibonacci::HomesteadMeshes) {
    // These meshes are the whole ship unless `rebuild_homestead` says otherwise after this. A world
    // load ends here, so the editor's cut-away, if it is open on a storey with another over it, is
    // drawn again on the next frame (`HomeDraw::view_stale`).
    state.home_draw.drew_whole_ship();
    state.home_draw.edits += 1;
    let needs = shell_family_needs(&homestead);
    let held = |s: Option<Slot>| s.into_iter().collect::<Vec<_>>();
    let prior: [Vec<Slot>; 8] = [
        std::mem::take(&mut state.homestead_floors),
        held(state.homestead_walls),
        state.homestead_material_walls.drain(..).map(|(mi, ma, _)| (mi, ma)).collect(),
        held(state.homestead_trim),
        held(state.homestead_windows),
        held(state.homestead_mirrors),
        held(state.homestead_ceiling),
        held(state.homestead_ceiling_opaque),
    ];
    let renderer = &mut state.renderer;
    let (slots, idle) = assign_slots(&mut state.home_draw.spare, prior.into_iter().zip(needs).collect(), || new_slot(renderer));
    let r = &mut state.renderer;
    for s in idle {
        park_slot(r, s);
    }
    let [floor_s, wall_s, mat_wall_s, trim_s, window_s, mirror_s, ceiling_s, ceiling_opaque_s]: [Vec<Slot>; 8] =
        slots.try_into().expect("eight shell families");
    // Floors (one mesh + material per room). A glass floor slab is not one of them: the render
    // loop draws `homestead_floors` in the opaque pass, so it goes with the glass walls below.
    let (opaque_floors, glass_floors): (Vec<_>, Vec<_>) = homestead.floors.into_iter().partition(|f| is_opaque_floor(f.2));
    for (&slot, (verts, indices, color, material_type)) in floor_s.iter().zip(&opaque_floors) {
        fill_slot(r, slot, verts, indices, (*color, 0.0, 0.8, *material_type as f32, 0.0));
    }
    state.homestead_floors = floor_s;
    // The combined walls (the legacy fibonacci ship path).
    state.homestead_walls = fill_one(r, &wall_s, &homestead.walls, ([0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0, 0.0));
    // Per-material home walls (v0.552): one mesh+material per picked wall material so each wall
    // renders in its own color; the `is_transparent` flag routes glass (alpha < 1) to the
    // transparent pass at render time. Glass: low roughness; emissive zeroed (v0.780: self-emission
    // made glass GLOW at night). Then the glass floor slabs, in the glass roof's metallic,
    // roughness and emissive (the colour, a face's share of the roof's opacity, comes with the mesh:
    // `home_structure::glass_slab_face_alpha`).
    let (gmet, grough, gemis) = GLASS_ROOF_LOOK;
    let mut mat_walls = Vec::with_capacity(mat_wall_s.len());
    let mut mat_slots = mat_wall_s.iter().copied();
    for (verts, indices, color) in &homestead.material_walls {
        let Some(slot) = mat_slots.next() else { break };
        let transparent = color[3] < 0.999;
        let (met, rough, mtype) = if transparent { (0.0, 0.1, 1.0) } else { (0.1, 0.7, 0.0) };
        fill_slot(r, slot, verts, indices, (*color, met, rough, mtype, 0.0));
        mat_walls.push((slot.0, slot.1, transparent));
    }
    for (verts, indices, color, material_type) in &glass_floors {
        let Some(slot) = mat_slots.next() else { break };
        fill_slot(r, slot, verts, indices, (*color, gmet, grough, *material_type as f32, gemis));
        mat_walls.push((slot.0, slot.1, true));
    }
    state.homestead_material_walls = mat_walls;
    state.homestead_trim = fill_one(r, &trim_s, &homestead.trim, ([0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0, 0.0));
    // Tinted glass (alpha 0.45), transparent pass. NO emissive (v0.780): the old 0.12
    // self-emission bypassed lighting, so windows GLOWED in the dark (operator field report: "the
    // glass texture is emissive"). Glass now only shows light that actually falls on/through it.
    state.homestead_windows = fill_one(r, &window_s, &homestead.windows, ([0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.0));
    state.homestead_mirrors = fill_one(r, &mirror_s, &homestead.mirrors, ([0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 0.0));
    // v0.539: a glass roof renders the ceiling TRANSPARENT (you see the stars through the
    // sealed clear roof); otherwise it is the opaque grey ceiling. v0.754 (multi-zone ships):
    // roofs are PER ZONE -- `ShipStructure::generate_meshes` routes glass-roof zones' ceilings
    // into `ceilings` (this transparent slot) and opaque-roof zones' into `ceilings_opaque`
    // (its own slot below, show_roof-gated like the old single opaque roof). The fibonacci
    // fallback still fills this slot with `roof_glass = false` (opaque), unchanged.
    let roof_glass = state.gui_state.ship_structure.as_ref().map_or(false, |s| s.any_glass_roof());
    state.homestead_ceiling_glass = roof_glass;
    let (ocol, omet, orough, otype) = OPAQUE_CEILING_LOOK;
    let opaque_look = (ocol, omet, orough, otype, 0.0);
    let ceiling_look = if roof_glass {
        // Glass roof emissive zeroed (v0.780), the same night-glow fix as the windows.
        (crate::ship::home_structure::GLASS_ROOF_RGBA, gmet, grough, crate::ship::home_structure::GLASS_ROOF_TYPE as f32, gemis)
    } else {
        opaque_look
    };
    let r = &mut state.renderer;
    state.homestead_ceiling = fill_one(r, &ceiling_s, &homestead.ceilings, ceiling_look);
    // OPAQUE-roof zones' ceilings (v0.754): their own slot with the same opaque material the
    // single opaque roof used, rendered only when show_roof is on (see the render loop).
    state.homestead_ceiling_opaque = fill_one(r, &ceiling_opaque_s, &homestead.ceilings_opaque, opaque_look);
}

/// Regenerate + upload the HULL WRAP (ship-superstructure increment D): the generated
/// exterior shell around the zone cluster, lofted from data/blueprints/hull_profile.ron
/// (embedded fallback) with cutouts over glass roofs + glass corridor lids. Called wherever
/// the ship structure rebuilds (load_world + rebuild_homestead), so adding/moving a zone
/// grows the hull next frame. Reuses the prior mesh/material slot in place (the v0.531
/// no-leak discipline: a per-frame editor drag must not orphan GPU buffers), and an emptied
/// hull gives its slot to the home's spares (`assign_slots`). Purely visual: no collision is
/// registered for the hull.
pub(crate) fn rebuild_hull(state: &mut EngineState) {
    // Lazy profile load (disk first, embedded fallback). Cached until the
    // hot-reload poll sees hull_profile.ron change, which clears the cache
    // and calls back in here (v0.770) - silhouette tuning without relaunch.
    if state.hull_profile.is_none() {
        state.hull_profile = crate::ship::hull::HullProfile::load(&state.data_dir);
    }
    let meshes = match (state.gui_state.ship_structure.as_ref(), state.hull_profile.as_ref()) {
        (Some(ship), Some(profile)) => crate::ship::hull::generate_hull(ship, profile),
        // No ship (legacy fibonacci layout) or no parseable profile: no hull.
        _ => crate::ship::hull::HullMeshes::default(),
    };
    let prior: Vec<Slot> = state.homestead_hull.into_iter().collect();
    let need = usize::from(!meshes.plating.0.is_empty());
    let renderer = &mut state.renderer;
    let (slots, idle) = assign_slots(&mut state.home_draw.spare, vec![(prior, need)], || new_slot(renderer));
    for s in idle {
        park_slot(&mut state.renderer, s);
    }
    // Hull plating colors from the same wall-material palette as zone shells, so the
    // profile's `material` id means the same thing everywhere.
    let mat_id = state.hull_profile.as_ref().map_or(1, |p| p.material);
    let color = crate::ship::home_structure::HomeStructure::material_color(mat_id);
    state.homestead_hull = fill_one(&mut state.renderer, &slots[0], &meshes.plating, (color, 0.35, 0.55, mat_id as f32, 0.0));
}

/// Regenerate the homestead meshes from the live layout (the construction editor's apply).
/// Also refreshes room lights + the sealed-volume bounds, since a height/wall edit changes
/// them. (v0.455)
pub(crate) fn rebuild_homestead(state: &mut EngineState) {
    // Normalize every corner onto the corner grid (v0.574) so co-located corners are byte-identical
    // -- this self-heals any older home whose snapped corners had sub-tolerance residue (which read
    // as two overlapping orbs that dragged apart). Idempotent: an on-grid corner is unchanged.
    // Runs across EVERY zone (v0.754): all zones render + collide, not just the edited one.
    if let Some(ship) = state.gui_state.ship_structure.as_mut() {
        for zone in ship.zones.iter_mut() {
            for wall in zone.body.walls.iter_mut() {
                wall.a = crate::ship::home_structure::quantize_corner(wall.a);
                wall.b = crate::ship::home_structure::quantize_corner(wall.b);
            }
        }
    }
    // Dev tool (v0.576): write a machine-readable snapshot of the live home so an AI can READ what
    // the operator is building (the act surface -- a text-command console -- is the next stage).
    // Still the ACTIVE ZONE's body (the zone being edited -- what the operator is building right
    // now); a ship-level introspection surface is a follow-up.
    if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        let json = hs.to_introspection_json();
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write("debug/home_snapshot.json", json);
    }
    // v0.534/v0.754: regenerate the WHOLE SHIP (every zone, offset by its origin) when present,
    // else the legacy AABB-room layout.
    let homestead = if let Some(ship) = state.gui_state.ship_structure.as_ref() {
        // The B editor's storey cut-away (ship homes increment 7, engine/editor_storeys.rs
        // `editor_cut_away`): while it works on a storey with others above, the zone it edits is
        // drawn with those storeys taken out, so the storey can be seen and edited from above. The
        // rooms stay the whole ship's (the lights, room GI, the HUD, footing), so choosing a storey
        // changes nothing but the picture. Two mesh builds while a cut is shown, editor only.
        // Maps increment 3: or the level the Maps page shows cut open (`editor::wanted_cut_meshes`).
        match crate::engine::editor::wanted_cut_meshes(state) {
            Some(mut m) => {
                m.room_info = ship.generate_meshes().room_info;
                m
            }
            None => ship.generate_meshes(),
        }
    } else if let Some(layout) = state.homestead_layout.clone() {
        crate::ship::fibonacci::generate_from_layout(&layout)
    } else {
        return;
    };
    let room_info = homestead.room_info.clone();
    // Rebuild the wall collision segments from the live ship (v0.556; per-zone offsets v0.754)
    // so editing a wall updates what the player walks into. Empty for the legacy AABB layout.
    state.wall_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_wall_segments(ship),
        None => Vec::new(),
    };
    state.sight_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_sight_segments(ship),
        None => Vec::new(),
    };
    apply_homestead_meshes(state, homestead);
    // What else the cut-away leaves out, kept with the meshes it drew (`apply_homestead_meshes`
    // clears it, so a load that draws the whole ship drops it): the machines, their screens, the
    // door panels and the elevator cars, drawn from the whole ship, follow it by where they stand
    // (`HomeDraw::cut`; increment 7 review, 2026-10-21). It is also the record the editor's view
    // follows (`editor::follow_storey_view`).
    let drawn = crate::engine::editor::wanted_cuts(state);
    state.home_draw.drew_cut(drawn);
    // Room GI follows the rooms (engine/room_gi.rs; a no-op when they did not change).
    crate::engine::room_gi::rooms_changed(state, &room_info);
    // The hull wrap follows the structure (increment D): a zone add/move/resize or a roof
    // material change regrows the exterior shell in the same rebuild.
    rebuild_hull(state);
    // Refresh lights + sealed bounds from the new room_info (height edits move them).
    let auto_lights = auto_room_lights(state.gui_state.ship_structure.as_ref(), room_info.iter().map(|r| (r.center, r.dimensions)));
    // v0.571: placed lights (across ALL zones, v0.754) override the auto synthesis (empty -> auto).
    state.room_lights = home_lights(state.gui_state.ship_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);
    state.homestead_bounds = room_info.iter().fold(None, |acc, r| {
        let rmin = r.center - r.dimensions * 0.5;
        let rmax = r.center + r.dimensions * 0.5;
        Some(match acc { None => (rmin, rmax), Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)) })
    });
    // Whose air each place breathes and where aboard ends follow the ship (increment 4).
    crate::engine::survival_env::refresh_ship_spaces(state);
    // Refresh the HUD room volumes (the "you are in <room>" detection + occlusion) so a
    // moved/resized/added/removed room is tracked live, not just on restart. (v0.459)
    // (Machine placement + pipes + hologram/spawn still resolve at load_world; they refresh
    // on the next relaunch -- a follow-up will make them live too.)
    let room_types = crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
    // The function join (rooms.ron) and the boxes the grow rooms' air is published from
    // (RoomTypeRegistry::room_bounds, shared with world entry and the life-support tests).
    state.gui_state.room_bounds = room_types.room_bounds(&room_info);
    // Room geometry changed, so the machines in those rooms must follow (a moved/resized room
    // carries its machines). Refresh the machine meshes from the new room bounds. (v0.525)
    rebuild_machine_objects(state);
    // Door/window panels follow the structure too (a wall edit can add/move/remove openings).
    rebuild_door_panels(state);
    // When the plot the home stands on changed (a relay's welcome, the Dev Plots panel, an
    // undo of either), what the home holds goes with it and the save's frame follows
    // (engine/home_plot.rs, ship homes 1b; a no-op otherwise).
    crate::engine::home_plot::follow_home_box(state);
    log::info!("Homestead rebuilt: {} rooms", room_info.len());
}

/// The crude one-per-room fill light (`home_lights`' `auto`): a warm point near each room's
/// ceiling, sized by the room. Each room is its (centre, dimensions). The ceiling is the room's
/// own (ship homes increment 7, `ceiling_over`): under a floor slab the light hangs under the
/// slab's underside, not inside the slab at the room box's top.
pub(crate) fn auto_room_lights(
    ship: Option<&ShipStructure>,
    rooms: impl Iterator<Item = (Vec3, Vec3)>,
) -> Vec<crate::renderer::light::RoomLight> {
    rooms
        .map(|(center, dims)| {
            let top = center.y + dims.y * 0.5;
            let ceiling = ship.map_or(top, |s| ceiling_over(s, center, top));
            let light_pos = Vec3::new(center.x, ceiling - 0.1, center.z);
            let room_size = dims.x.max(dims.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            crate::renderer::light::RoomLight::point(light_pos, [1.0, 0.95, 0.85], intensity, room_size * 1.5)
        })
        .collect()
}

/// The plots this game may walk into changed (ship homes increment 5b, `ShipStructure::open`, set
/// by engine/household.rs `apply_open`): redo what reads them, and nothing else.
/// - The walking colliders (`wall_collision::ship_wall_segments`): an open plot's corridor and
///   home walls stop this player, and its street or Commons wall lets them through.
/// - The door placements (`door_panels::ship_panel_placements`): an open plot's corridor doors
///   open for this player. The list keeps its length and order whatever is open, so each door is
///   swapped in place and keeps its open fraction, its manual-open flag and its live lock states.
///   (`rebuild_door_panels` resets the last two, which is right after a structure edit and wrong
///   here: this runs while walking, as the player steps onto or off a plot.)
/// - The room lights (`home_lights`): the lit open plot's design lights, on nobody's meter. The
///   auto fill is redone from the rooms the HUD already holds (`room_bounds`, the same rooms the
///   last rebuild made).
///
/// The meshes, the hull and the sight lines are unchanged: what a neighbour draws, and what the
/// eye meets there, is the same open or not, and the neighbours' view is not made again
/// (`neighbours::NeighbourKey` does not hold the open set). Call it once after setting
/// `ship.open`, only when it changed.
pub(crate) fn open_plots_changed(state: &mut EngineState) {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else {
        return;
    };
    state.wall_colliders = crate::ship::wall_collision::ship_wall_segments(ship);
    let placements = crate::ship::door_panels::ship_panel_placements(ship);
    let auto = auto_room_lights(Some(ship), state.gui_state.room_bounds.iter().map(|r| ((r.min + r.max) * 0.5, r.max - r.min)));
    state.room_lights = home_lights(Some(ship), auto, state.gui_state.gi_enabled);
    if !swap_door_placements(&mut state.door_panels, placements) {
        // A different door count means the structure changed under us: the full rebuild.
        rebuild_door_panels(state);
    } else {
        note_landing_doors(state);
    }
}

/// Put `placements` in place of the doors' placements, each door keeping its open fraction, when
/// the count is the same (true); else change nothing (false). `open_plots_changed`.
fn swap_door_placements(
    doors: &mut [(crate::ship::door_panels::PanelPlacement, f32)],
    placements: Vec<crate::ship::door_panels::PanelPlacement>,
) -> bool {
    if placements.len() != doors.len() {
        return false;
    }
    for (d, p) in doors.iter_mut().zip(placements) {
        d.0 = p;
    }
    true
}

/// Build a machine's primitive mesh from its shape + size. Shared by load_world (initial spawn)
/// and rebuild_machine_objects (the editor's live refresh) so both draw a machine identically.
pub(crate) fn machine_mesh(device: &wgpu::Device, shape: &str, size: (f32, f32, f32)) -> Mesh {
    let (sx, sy, sz) = size;
    match shape {
        // Capped so tanks/cisterns have a visible top + bottom (v0.623: "the tops are missing").
        "cylinder" => Mesh::cylinder_capped(device, sx.max(0.02), sy.max(0.05), 16),
        "sphere" => Mesh::sphere(device, sx.max(0.02), 10, 12),
        "pyramid" => Mesh::pyramid(device, sx.max(0.05), sy.max(0.05)),
        _ => Mesh::box_xyz(device, sx.max(0.02), sy.max(0.02), sz.max(0.02)),
    }
}

/// A placed machine's body: open shelving when its grow medium stacks its
/// plots as shelves (a mushroom rack, `stacked` in data/garden/grow_media.ron,
/// 2026-09-27), so the shelves and what grows on them show instead of hiding
/// inside a solid box; otherwise [`machine_mesh`]. `machine_type` is the
/// catalog key (`mushroom_rack`). Both machine paths draw through this: the
/// world load and the editor's rebuild.
pub(crate) fn machine_body_mesh(
    device: &wgpu::Device,
    grow_media: &[crate::systems::grow_machines::GrowMedium],
    machine_type: &str,
    shape: &str,
    size: (f32, f32, f32),
) -> Mesh {
    // The first medium that matches, the one the farm divides it by.
    match grow_media.iter().find(|m| m.matches(machine_type)).filter(|m| m.stacked) {
        Some(m) => {
            let (v, ix) = crate::engine::plant_layout::shelf_rack_geometry(size, m.plots.max(1));
            Mesh::from_vertices(device, &v, &ix)
        }
        None => machine_mesh(device, shape, size),
    }
}

/// Resolve the home's drone hangar (the "drone_hangar" catalog machine, e.g. `drone_hangar_1`
/// in data/machines/home.ron) to its world DECK position + yaw, so the mining-drone visual
/// (v0.639) can sit exactly where the hangar pad is drawn. Reuses the same tested
/// `MachineHome::placements` the machine meshes themselves are built from, so the drone never
/// drifts from the pad under it even as the home layout is edited live.
///
/// Design choice (v1, documented per the operator's ask): today's home.ron places exactly ONE
/// "drone_hangar" instance, and `DroneSystem` allows only ONE drone in flight at a time (see
/// `systems::mining::DroneSystem::tick`'s "one drone per player" gate -- a second commission
/// while one is flying is refused). So "the hangar" and "the drone" are both singular right
/// now -- this picks the FIRST drone_hangar instance found, and the caller treats hangar
/// occupancy as a simple binary (any drone in flight => pad empty; no drone => docked). That is
/// not a simplifying guess, it is EXACT given today's one-drone rule. If a future change adds
/// multiple hangars or multiple simultaneous drones, this needs real per-drone-per-hangar
/// assignment (e.g. tagging `Drone` with the hangar id it launched from).
pub(crate) fn hangar_placement(state: &EngineState) -> Option<(Vec3, f32)> {
    use std::collections::HashMap;
    let home = state.gui_state.home_machines.as_ref()?;
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    if rooms.is_empty() {
        return None;
    }
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    // Match by CATALOG TYPE ("drone_hangar"), not by id or label text, so a renamed label or a
    // re-numbered instance id still resolves correctly.
    let hangar_id = home.all_instances().into_iter().find(|i| i.machine == "drone_hangar")?.id;
    home.placements(&rooms, zone_rects.as_deref())
        .into_iter()
        .find(|p| p.id == hangar_id)
        .map(|p| (Vec3::new(p.pos.0, p.pos.1, p.pos.2), p.rotation))
}

/// The current machine placements, resolved the way `rebuild_machine_objects`
/// resolves them (room bounds -> RoomGeom, the ship's zone rects, the live
/// `home_machines`). `None` before the room geometry exists. Used by the
/// world load to build the in-world screens once the machines are placed,
/// without duplicating the resolve at that call site.
pub(crate) fn current_placements(state: &EngineState) -> Option<Vec<crate::machines::PlacedMachine>> {
    use std::collections::HashMap;
    let home = state.gui_state.home_machines.as_ref()?;
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    if rooms.is_empty() {
        return None;
    }
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    Some(home.placements(&rooms, zone_rects.as_deref()))
}

/// How far over a machine's top (or under its bottom) its port gizmos stand, metres.
const GIZMO_RISE_M: f32 = 0.3;
/// How far a port gizmo is kept inside the inner face of its home's shell (metres): the length
/// of the arrows drawn round it (lib.rs, 0.28 m), so they stay inside too.
const GIZMO_WALL_CLEAR_M: f32 = 0.3;
/// How far a port gizmo is kept under the ceiling over its machine and over its floor (metres).
const GIZMO_CEILING_CLEAR_M: f32 = 0.1;

/// The space a machine's port gizmos keep inside (`port_gizmo_pos`), world metres (increment 7
/// follow-ups, 2026-10-21): inside the shell of the zone its foot stands in, less
/// `GIZMO_WALL_CLEAR_M`, and from just over the floor of its storey to just under the ceiling over
/// it (`lowest_ceiling_above`: the slab over it, else the roof), `GIZMO_CEILING_CLEAR_M` from each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GizmoRoom {
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
}

/// The ceiling over world point `p` in a body whose floor slabs are `slabs`, `height` tall,
/// standing at `origin` (world metres): the underside of the lowest slab over `p` whose underside
/// is above it, else the roof. What a hanger reaches (`RunSite::hanger_top`) and what a port gizmo
/// keeps under (`gizmo_room`). Increment 7 follow-ups, 2026-10-21: in the double-height greenhouse
/// there is no slab, so the ceiling is the roof at 6 m, where the service level of its storey
/// (`service_levels`, one per storey of the whole home) says 2.75.
pub(crate) fn lowest_ceiling_above(slabs: &[crate::ship::home_structure::FloorSlab], height: f32, origin: Vec3, p: Vec3) -> f32 {
    let (x, y, z) = (p.x - origin.x, p.y - origin.y, p.z - origin.z);
    origin.y
        + slabs
            .iter()
            .filter(|s| s.contains(x, z) && s.underside() > y + crate::ship::home_structure::STOREY_EPS)
            .map(|s| s.underside())
            .fold(height, f32::min)
}

/// The machine's foot: where it stands (a sphere's placement is its centre, a radius over it).
fn machine_foot(p: &crate::machines::PlacedMachine) -> Vec3 {
    let foot = if p.shape == "sphere" { p.pos.1 - p.size.0 } else { p.pos.1 };
    Vec3::new(p.pos.0, foot, p.pos.2)
}

/// The `GizmoRoom` of machine `p` aboard `ship`: None when its foot is in no zone (the gizmos
/// then ring it as they always did).
pub(crate) fn gizmo_room(ship: Option<&ShipStructure>, p: &crate::machines::PlacedMachine) -> Option<GizmoRoom> {
    let foot = machine_foot(p);
    let (o, body) = zone_holding(ship?, foot)?;
    let inset = body.shell_resolved_thickness() * 0.5 + GIZMO_WALL_CLEAR_M;
    let span = |lo: f32, len: f32| {
        let (a, b) = (lo + inset, lo + len - inset);
        if a <= b { (a, b) } else { (lo + len * 0.5, lo + len * 0.5) }
    };
    let ((x0, x1), (z0, z1)) = (span(o.x, body.width), span(o.z, body.depth));
    let floor = storey_floor_at(ship, foot, foot.y) + GIZMO_CEILING_CLEAR_M;
    let ceiling = (lowest_ceiling_above(&body.floors, body.height, o, foot) - GIZMO_CEILING_CLEAR_M).max(floor);
    Some(GizmoRoom { min: Vec3::new(x0, floor, z0), max: Vec3::new(x1, ceiling, z1) })
}

/// World position of a machine's port gizmo (v0.625, the viewport drag-to-connect handles). Ports
/// carry no authored anchor (they default to 0,0,0), so spread a machine's N ports in a ring just
/// ABOVE the body, so each reads as its own grab-able handle that never overlaps the machine mesh.
/// Kept inside `room` (`gizmo_room`; increment 7 follow-ups, 2026-10-21): a ring that would reach
/// past the home's shell is pulled inside it (the study's uplink, router and server and the shower
/// room's WC and basin, against the east wall, had gizmos at x 55.02 to 55.15 on a 55 m house, so
/// their risers ran outside the shell), and a unit hung under a ceiling with no room over it for
/// the ring (the plant room's and the court's air handlers top out 1 cm under the slab, the
/// mushroom room's fan stood 2.91 m up under 2.75) has its ring under it instead, where it was
/// drawn on the floor above.
/// A run ends at its machine's gizmo (`conduit_runs`), so the runs follow.
pub(crate) fn port_gizmo_pos(p: &crate::machines::PlacedMachine, i: usize, n: usize, room: Option<GizmoRoom>) -> Vec3 {
    let mut at = if n <= 1 {
        Vec3::new(p.pos.0, p.top_y + GIZMO_RISE_M, p.pos.2)
    } else {
        let r = (p.size.0.max(p.size.2)) * 0.5 + GIZMO_RISE_M;
        let a = (i as f32 / n as f32) * std::f32::consts::TAU;
        Vec3::new(p.pos.0 + r * a.cos(), p.top_y + GIZMO_RISE_M, p.pos.2 + r * a.sin())
    };
    let Some(room) = room else { return at };
    if at.y > room.max.y {
        let under = machine_foot(p).y - GIZMO_RISE_M;
        at.y = if under >= room.min.y { under } else { room.max.y };
    }
    at.clamp(room.min, room.max)
}

/// Keep the live machine ECS in sync with the editor placements (v0.730).
/// Spawns entities for newly-placed machines (with their power/water/
/// AutoRefine/Container roles), despawns removed ones, and updates the
/// Transform of survivors so behaviors anchored to the machine's pose
/// (the assembler's factory pad) follow a move. Idempotent + cheap.
pub(crate) fn sync_machine_entities(state: &mut EngineState, placements: &[crate::machines::PlacedMachine]) {
    use crate::ecs::components::{MachineInstanceId, Transform};
    let Some(home) = state.gui_state.home_machines.as_ref() else { return };
    let all = home.all_instances();
    let power_islands = home.electrical_islands(&all);
    let water_islands = home.water_islands(&all);
    let inst_by_id: std::collections::HashMap<&str, &crate::machines::MachineInstance> =
        all.iter().map(|i| (i.id.as_str(), i)).collect();
    let placed_ids: std::collections::HashSet<&str> =
        placements.iter().map(|p| p.id.as_str()).collect();
    // Snapshot the existing machine entities (id -> entity) in one scope so
    // the query borrow ends before we mutate the world.
    let existing: std::collections::HashMap<String, hecs::Entity> = state
        .game_world
        .world
        .query::<&MachineInstanceId>()
        .iter()
        .map(|(e, mid)| (mid.0.clone(), e))
        .collect();
    // Despawn entities whose machine no longer exists in the editor state.
    for (id, e) in &existing {
        if !placed_ids.contains(id.as_str()) {
            let _ = state.game_world.world.despawn(*e);
            log::info!("[Machines] despawned removed machine entity {id}");
        }
    }
    let containers = state
        .data_store
        .get::<crate::systems::inventory::containers::ContainerRegistry>("container_registry");
    for p in placements {
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        if let Some(&e) = existing.get(&p.id) {
            // Keep the pose current (moves are count-unchanged edits).
            if let Ok(mut t) = state.game_world.world.get::<&mut Transform>(e) {
                t.position = pos;
                t.rotation = Quat::from_rotation_y(p.rotation.to_radians());
            }
        } else if let Some(inst) = inst_by_id.get(p.id.as_str()) {
            if let Some(def) = home.catalog.get(&inst.machine) {
                spawn_home_machine_entity(
                    &mut state.game_world.world,
                    inst,
                    def,
                    &power_islands,
                    &water_islands,
                    Some(pos),
                    containers,
                );
                log::info!("[Machines] spawned live entity for placed machine {}", p.id);
            }
        }
    }
}

pub(crate) fn rebuild_machine_objects(state: &mut EngineState) {
    use std::collections::HashMap;
    state.home_draw.edits += 1; // the plan tiles look for what moved (engine/map_tiles.rs)
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    // Guard: if there is no room geometry yet (room_bounds not populated), do NOT wipe the
    // machines load_world already placed -- otherwise an edit before bounds are ready blanks
    // the whole home. (v0.528)
    if rooms.is_empty() {
        return;
    }
    // v0.538: a box home positions machines by ABSOLUTE world coords (not room-center-relative)
    // so they survive flood-fill room-id churn. v0.754: clamped per machine into ITS ship
    // zone's footprint at that zone's origin.
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    let placements = match &state.gui_state.home_machines {
        Some(h) => h.placements(&rooms, zone_rects.as_deref()),
        None => return,
    };
    // Port pick volumes (v0.625): every machine's derived ports -> a grab-able world gizmo, so the
    // viewport can DRAG a port onto another machine to wire them. Keyed by id (placements may skip a
    // machine whose catalog/room is missing), so zip-by-index is unsafe -- look the type up by id.
    state.port_pick = port_picks(state.gui_state.home_machines.as_ref().unwrap(), &placements, state.gui_state.ship_structure.as_ref());
    // ── Live ECS sync (v0.730, operator field report): the editor previously
    // rebuilt VISUALS only, so a machine placed in the construction editor had
    // NO entity until the next Enter World — no AutoRefine (a placed vehicle
    // assembler never assembled and showed no recipe selector), no Container
    // (a placed silo's card stayed on the static RON stats), no power/water
    // roles. Diff the placements against the live entities by machine id:
    // spawn missing, despawn removed, keep Transforms current so a moved
    // assembler's factory pad follows. Runs on every editor commit (and per
    // frame during drags) — one small map + query, cheap at homestead scale.
    // NOTE: electrical/plumbing ISLANDS for pre-existing entities are not
    // recomputed here (a new connection re-islands on the next world entry,
    // same as before).
    sync_machine_entities(state, &placements);

    // Fast path: the machine COUNT is unchanged (an offset drag / room move, not add/remove).
    // Reuse the existing meshes + materials and only update positions, so a per-frame drag does
    // NOT leak a fresh mesh per machine every frame (the v0.527 regression). placements() is
    // deterministically ordered (instances then array cells), so index i is the same machine.
    if placements.len() == state.machine_objects.len()
        && placements.len() == state.gui_state.machine_labels.len()
        && placements.len() == state.machine_pick.len()
    {
        for (i, p) in placements.iter().enumerate() {
            state.machine_objects[i].2 = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
            state.machine_objects[i].3 = p.rotation; // keep the yaw in sync on a position-only update
            state.gui_state.machine_labels[i].pos = Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2);
            state.gui_state.machine_labels[i].foot_y = p.pos.1;
            // Keep the pick volume in sync (v0.553) -- else a move WITHOUT a count change (a room
            // drag, a clamp-on-resize) leaves the click ray-test + the highlight ring at the OLD
            // position. Same math as the slow-path build below.
            let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
            let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
            state.machine_pick[i] = (
                p.id.clone(),
                Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
                half_h.max(half_w) + 0.35,
            );
        }
        // Grow machines follow a move too (2026-09-27, critic review: a moved
        // rack's tent, plants and published plot stayed where it had stood
        // until the next world entry). Only when one actually moved, and the
        // plant pass then rebuilds just the moved machines, because its
        // per-machine signature covers where each stands.
        let mut grow_moved = false;
        for p in &placements {
            if let Some(g) = state.grow_positions.iter_mut().find(|g| g.id == p.id) {
                let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
                if g.pos != pos || g.yaw != p.rotation || g.top_y != p.top_y || g.size != p.size {
                    g.pos = pos;
                    g.yaw = p.rotation;
                    g.top_y = p.top_y;
                    g.size = p.size;
                    grow_moved = true;
                }
            }
        }
        if grow_moved {
            publish_grow_plots(state);
            state.plant_mesh_sig = 0;
        }
        // In-world screens follow their bodies on a move: geometry only, no
        // surface or renderer slot is touched (a drag must not recreate the
        // page's context and lose its scroll state).
        state.screens.update_poses(&placements);
        rebuild_connection_objects(state);
        return;
    }
    // Count changed (add / remove) or first build. Reuse the machines' mesh/material SLOTS
    // (replace in place) instead of clear()+re-add, so a single add/remove doesn't orphan the
    // whole ~100-mesh home (v0.531 -- the renderer free path). Since the 2026-10-21 follow-ups
    // a shrink gives its tail to the home's spares (`machine_body_slots`), where it was orphaned.
    let slots = take_machine_slots(state, placements.len());
    state.machine_objects.clear();
    state.gui_state.machine_labels.clear();
    state.machine_pick.clear();
    // Catalog type by instance id (placements carry no catalog key): the
    // body mesh and the grow anchors below both need it.
    let type_by_id: HashMap<String, String> = state
        .gui_state
        .home_machines
        .as_ref()
        .map(|home| home.all_instances().into_iter().map(|i| (i.id, i.machine)).collect())
        .unwrap_or_default();
    let mut objs = Vec::with_capacity(placements.len());
    for (i, p) in placements.iter().enumerate() {
        // GLB model when the def declares one (v0.734): parsed fresh PER
        // INSTANCE (no shared cache), so the replace/reuse slot logic
        // below stays safe — each machine owns its mesh slot. Primitive
        // fallback on any load error, so a bad file never blanks it.
        // v0.993 (textured machine models): the TEXTURED parse runs first;
        // a model that carries an albedo texture (the Kenney furniture
        // palettes) gets a type-19 textured material from the per-model
        // cache instead of the def's flat color.
        let mut model_tex: Option<(Vec<u8>, u32, u32)> = None;
        let mut model_path: Option<String> = None;
        let mesh = p
            .model
            .as_deref()
            .and_then(|m| {
                state
                    .asset_manager
                    // Fitted to the def SIZE height (v0.1324): the pack is authored at
                    // about half real scale, so an unfitted model put a 0.46 m chair
                    // next to a player whose eyes are at 1.7 m.
                    .parse_gltf_mesh_textured_fit_height(&state.renderer.device, m, p.size.1)
                    .map_err(|e| {
                        log::warn!("machine {} model '{m}' failed: {e}; primitive fallback", p.id)
                    })
                    .ok()
                    .map(|(mesh, tex)| {
                        model_tex = tex;
                        model_path = Some(m.to_string());
                        mesh
                    })
            })
            .unwrap_or_else(|| {
                let ty = type_by_id.get(&p.id).map_or("", String::as_str);
                machine_body_mesh(&state.renderer.device, &state.gui_state.grow_media, ty, &p.shape, p.size)
            });
        // Shared textured material per model path, created once. NEVER
        // updated in place: instances of the same model share it.
        let textured_mat: Option<usize> = match (&model_path, model_tex) {
            (Some(mp), Some((rgba, w, h))) => {
                Some(match state.machine_model_materials.get(mp) {
                    Some(&ma) => ma,
                    None => {
                        let ma = state.renderer.add_textured_material(
                            [1.0, 1.0, 1.0, 1.0],
                            0.0,
                            0.85,
                            19.0,
                            0.0,
                            &rgba,
                            w,
                            h,
                        );
                        state.machine_model_materials.insert(mp.clone(), ma);
                        ma
                    }
                })
            }
            _ => None,
        };
        let color = [p.color.0, p.color.1, p.color.2, 1.0];
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        // The machine's own slot (`machine_body_slots`): its mesh replaced in place, and its own
        // material painted unless the model's shared textured material draws it. A slot's own
        // material is never a shared one, so painting it can never repaint a sibling.
        let (mi, own_ma) = slots[i];
        state.renderer.replace_mesh(mi, mesh);
        let use_ma = match textured_mat {
            Some(tm) => tm,
            None => {
                state.renderer.update_material_typed(own_ma, color, 0.1, 0.7, 0.0);
                own_ma
            }
        };
        objs.push((mi, use_ma, pos, p.rotation));
        // What E does at it (the home's bed sleeps you, first-hour audit F5).
        let on_use = type_by_id
            .get(&p.id)
            .and_then(|ty| state.gui_state.home_machines.as_ref().and_then(|h| h.catalog.get(ty)))
            .and_then(crate::engine::built_uses::machine_use);
        state.gui_state.machine_labels.push(crate::gui::MachineLabel {
            pos: Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2),
            foot_y: p.pos.1,
            name: p.label.clone(),
            stats: p.stats.clone(),
            room: p.room.clone(),
            machine_id: p.id.clone(),
            on_use,
        });
        // Pick volume for viewport selection: a sphere covering the machine body. Center at its
        // mid-height; radius the larger of half-height / half-width plus a click margin.
        let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
        let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
        state.machine_pick.push((
            p.id.clone(),
            Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
            half_h.max(half_w) + 0.35,
        ));
    }
    state.machine_objects = objs;
    // Record grow anchors for the procedural plant pass (v0.862/0.863):
    // EVERY placed machine, with its catalog type, footprint and top
    // height, so the plant pass can dress towers (helix) and beds/fields
    // (grid across the footprint at top_y). Type looked up by instance id
    // (placements carry no catalog key).
    state.grow_positions.clear();
    for p in &placements {
        if let Some(ty) = type_by_id.get(&p.id) {
            state.grow_positions.push(GrowSpot {
                ty: ty.clone(),
                id: p.id.clone(),
                pos: Vec3::new(p.pos.0, p.pos.1, p.pos.2),
                yaw: p.rotation,
                top_y: p.top_y,
                size: p.size,
            });
        }
    }
    publish_grow_plots(state);
    // In-world screens (rung 2): a surface + display quad per placed machine
    // with a `screen` def. Reuses surfaces by instance id (a count change
    // that keeps a screen keeps its page state), drops the ones whose
    // machine is gone. Same prior-slot reuse discipline as the bodies above.
    crate::engine::screens::sync_screens(state, &placements);
    state.plant_mesh_sig = 0; // machine layout may have moved: replant visuals
    rebuild_connection_objects(state);
}

/// Publish the home's grow machines for the farming system (2026-09-26),
/// from the grow anchors just recorded, in placement order:
/// - "grow_plots" (`Vec<lighting::GrowPlot>`): where each stands, its
///   footprint and a tower's cups, for the grow lights (farming::lighting);
/// - "grow_instances" (type -> [(instance id, plots)]): the machines a bed
///   Plant button for a TYPE sows, one crop per plot;
/// - "grow_plot_area_m2" (instance id and type id -> m2 of one plot): the
///   floor one crop in that machine has.
/// A grow machine is one a grow medium (data/garden/grow_media.ron) matches;
/// a medium's `plots` divides a bed, tray, rack or field into plots.
pub(crate) fn publish_grow_plots(state: &mut EngineState) {
    use crate::systems::farming::lighting::GrowPlot;
    use std::collections::HashMap;
    let tower_cfgs = crate::gui::load_tower_configs(&crate::data_dir());
    let mut plots: Vec<GrowPlot> = Vec::new();
    let mut instances: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    let mut plot_area: HashMap<String, f32> = HashMap::new();
    // And the enclosures the garden draws around them (2026-09-27).
    let mut tents: Vec<(Vec3, f32, (f32, f32, f32))> = Vec::new();
    for g in &state.grow_positions {
        let Some(medium) = state.gui_state.grow_media.iter().find(|m| m.matches(&g.ty)) else { continue };
        if let Some(e) = &medium.enclosure {
            tents.push((g.pos, g.yaw, e.size));
        }
        let footprint = (g.size.0 * g.size.2).max(0.0);
        let tower_cfg = g.ty.strip_prefix("aeroponic_tower_").and_then(|k| tower_cfgs.iter().find(|t| t.id == k));
        plots.push(GrowPlot {
            id: g.id.clone(),
            aliases: tower_cfg.map(|t| vec![t.id.clone()]).unwrap_or_default(),
            pos: g.pos.to_array(),
            footprint_m2: footprint,
            cups: tower_cfg.map_or(0, |t| t.slots),
            outdoors: crate::systems::farming::is_field_area(&g.ty) || crate::systems::farming::is_field_area(&g.id),
            enclosure: medium.enclosure.clone(),
        });
        if tower_cfg.is_none() {
            let n = medium.plots.max(1);
            let per_plot = if medium.stacked { footprint } else { footprint / n as f32 };
            instances.entry(g.ty.clone()).or_default().push((g.id.clone(), n));
            plot_area.insert(g.id.clone(), per_plot);
            plot_area.entry(g.ty.clone()).or_insert(per_plot);
        }
    }
    state.data_store.insert("grow_plots", plots);
    state.data_store.insert("grow_instances", instances);
    state.data_store.insert("grow_plot_area_m2", plot_area);
    state.garden_draw.tents = tents;
}

/// Procedural plants (v0.862): merged world-space plant meshes built from the
/// live CropInstances, each drawn as one RenderObject.
///
/// - Towers: one mesh per planted tower, a plant per net cup up the helix.
///   Crops bound to a tower CONFIG id ("nutrition", the GUI Plant button)
///   dress the first placed column of that type; a tower INSTANCE id dresses
///   that column.
/// - Beds, trays, racks and fields (2026-09-26): each crop is one plot of its
///   machine and draws the plants that plot really holds, the count the farm
///   harvests, feeds and waters (`farming::units::plants_in_plot`), laid out
///   at the crop's spacing by `engine::plant_layout`. Above the per-plot cap
///   in data/plants_visual.ron (`plot_visual_cap`) a plot draws that many
///   clumps instead, each as wide as the plants it stands for.
/// - A species with converted stage models (assets/models/plants/<set>_1..4)
///   bakes one merged mesh per machine and model, so a plot of 128 wheat
///   clumps is one draw, not 128. Procedural plants share one mesh per machine.
///
/// Cheap change-signature gate: rebuilds only when growth actually moves, and
/// then only the machines whose crops changed (2026-09-26).
///
/// OFF THE FRAME (2026-09-27). The frame only works out which machines need
/// new plants ([`plan_plant_groups`]); a worker thread builds their geometry
/// (`engine::plant_pass`, the same code that used to run here) and the frame
/// uploads what is finished a few milliseconds at a time
/// ([`drain_plant_work`]). A machine keeps its old plants until its new ones
/// are up, so nothing blinks. Measured on the photograph-home rig before the
/// move (run.log's "rebuild breakdown" and "frame times" lines): entering
/// the world built the garden in ONE frame of 1,804 ms (1,314 ms of it
/// loading the stage models, 306 ms geometry, 184 ms upload, 3.21 million
/// vertices); a whole-garden change took a 313 ms pass inside a 466 ms frame
/// against 51 ms typical; and a crop changing stage every few seconds cost
/// 36 to 166 ms frames on its own. After, same rig and garden, two runs:
/// the world entry build took at most 5.8 and 7.4 ms of any one frame (397
/// and 443 ms on the worker), a whole-garden change at most 4.6 ms of a
/// frame, and in the quieter run every rebuild's worst frame was 3.3 to
/// 6.7 ms. In the other, with other compiles sharing the machine, 6 of 26
/// rebuilds reached 8 to 30 ms, each in one step that usually costs a few
/// milliseconds (a 56,000-vertex upload, a plan); run.log's "slow
/// plant-pass frame" lines name the step whenever that happens.
pub(crate) fn rebuild_plant_meshes(state: &mut EngineState) {
    use std::hash::{Hash, Hasher};
    // The plants as they are now (2026-10-31, docs/design/farm-clock.md rung 2): the farm
    // advances each grow area on its own turn, so while aboard (on a planet or in space the home
    // is out of sight) the grow areas IN SIGHT are kept within a second of now, and a stage or a
    // wilt shows when it happens: one coming into sight is brought up this frame, before the
    // signature below reads it (`farming::settle_areas`). The rest follow the Garden updates
    // setting (until the same day every area was kept so whenever aboard, which made every
    // setting behave like every second there). Inside a home-storage window of its own
    // (stock_piles::settle_farm_areas, the seam review's A).
    if state.aboard_station {
        let frustum = crate::terrain::planet_chunks::FrustumPlanes::from_view_proj(&state.camera.view_projection_matrix().as_dmat4());
        let in_sight = areas_in_sight(&state.grow_positions, &frustum);
        crate::engine::stock_piles::settle_farm_areas(
            &mut state.game_world.world,
            &mut state.data_store,
            &mut state.gui_state.placed_items,
            &mut state.gui_state.home_stock,
            &in_sight,
            crate::systems::farming::clock::LOOK_FRESH_S,
        );
    }
    // Signature over everything that changes a plant's look.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (_e, c) in state
        .game_world
        .world
        .query::<&crate::ecs::components::CropInstance>()
        .iter()
    {
        c.crop_def_id.hash(&mut h);
        c.growth_stage.hash(&mut h);
        c.tower_id.hash(&mut h);
        c.tower_slot.hash(&mut h);
        ((c.health / 10.0) as u32).hash(&mut h);
    }
    state.grow_positions.len().hash(&mut h);
    let sig = h.finish().max(1);
    let frame_ms = hitch_probe_frame();
    let changed = sig != state.plant_mesh_sig;
    if !changed && state.garden_draw.plants.idle() {
        hitch_probe_pass(frame_ms, 0.0, false, 0, 0);
        return;
    }
    // The pass's main-thread milliseconds show on the Performance page
    // (cpu.plant_rebuild, in the vegetation slice); the worker's do not
    // cost the frame and are summarised in run.log by `note_plant_rebuild`.
    let _cost = crate::frame_costs::stage("cpu.plant_rebuild");
    let t0 = std::time::Instant::now();
    let mut pass = std::mem::take(&mut state.garden_draw.plants);
    if changed {
        // A zeroed signature (world load, a plants_visual.ron hot reload) is
        // a forced rebuild: re-read the data files whatever their stamps.
        let forced = state.plant_mesh_sig == 0;
        state.plant_mesh_sig = sig;
        plan_plant_groups(state, &mut pass, forced);
    }
    let plan_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let (files_ms, split) = if changed { (pass.inputs_ms, pass.plan_split) } else { (0.0, [0.0; 3]) };
    let drained = drain_plant_work(state, &mut pass, t0);
    let (groups, verts) = (drained.groups, drained.verts);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    if ms > SLOW_PLANT_FRAME_MS {
        note_slow_plant_frame(ms, plan_ms, files_ms, split, &drained);
    }
    let busy = !pass.idle() || groups > 0;
    if pass.batch.active {
        pass.batch.main_ms += ms;
        pass.batch.worst_ms = pass.batch.worst_ms.max(ms);
        pass.batch.frames += 1;
    }
    if pass.batch.active && pass.idle() {
        finish_plant_batch(state, &mut pass);
    }
    state.garden_draw.plants = pass;
    hitch_probe_pass(frame_ms, ms as f32, busy, groups, verts);
}

/// How far a grow area's plants may reach beyond its machine's box (a tower's helix, a tall crop),
/// and so how early before its machine enters the view the area is brought up to now, metres.
const SIGHT_MARGIN_M: f32 = 3.0;

/// The grow areas whose plants are in sight (2026-10-31, docs/design/farm-clock.md, the readers):
/// each grow machine whose box, as a sphere `SIGHT_MARGIN_M` larger, is not wholly outside one of
/// the camera's frustum planes (the plant meshes are world-space geometry drawn at the origin, so
/// nothing else culls them on the CPU, and walls between rooms are not counted: a plant behind one
/// counts as in sight). By the machine's instance id, which its crops carry, and, for the first
/// placed column of each tower design, the design's id too: crops planted by the design (the
/// Garden page's Plant button) dress that column (`plan_plant_groups`).
pub(crate) fn areas_in_sight(spots: &[GrowSpot], frustum: &crate::terrain::planet_chunks::FrustumPlanes) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen_type: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for g in spots {
        let first_of_type = seen_type.insert(g.ty.as_str());
        let (w, h, d) = g.size;
        let tall = (g.top_y - g.pos.y).max(h).max(0.0);
        let center = g.pos + Vec3::Y * (tall * 0.5);
        let radius = 0.5 * (w * w + tall * tall + d * d).sqrt() + SIGHT_MARGIN_M;
        if !frustum.sphere_visible(center.as_dvec3(), f64::from(radius)) {
            continue;
        }
        out.push(g.id.clone());
        if first_of_type {
            if let Some(design) = g.ty.strip_prefix("aeroponic_tower_") {
                out.push(design.to_string());
            }
        }
    }
    out
}

/// Work out which machines need new plants and hand them to a worker
/// (2026-09-27; the per-machine signature is the 2026-09-26 one, unchanged).
/// A machine whose crops, place and recipes are what its drawn plants were
/// built from keeps them; so does one whose rebuild is already finished and
/// waiting to upload. A job still building is replaced: whatever it had not
/// finished is in the new one.
fn plan_plant_groups(state: &mut EngineState, pass: &mut PlantPass, forced: bool) {
    use crate::engine::plant_layout::plot_rects;
    use crate::engine::plant_pass::{CropDraw, GroupJob, GroupLayout, Helix, Job, WorkerMsg};
    use crate::systems::farming::units;
    use std::collections::{HashMap, HashSet};
    use std::hash::{Hash, Hasher};

    // Per species its stage names; per (species, machine) the plants one
    // plot of it holds, counted exactly as the farm counts them (the plot
    // areas `publish_grow_plots` put in the DataStore).
    let t0 = std::time::Instant::now();
    let (stage_map, plot_plant_count) = {
        let plant_reg = state
            .data_store
            .get::<crate::systems::farming::PlantRegistry>("plant_registry");
        let plot_areas = state.data_store.get::<HashMap<String, f32>>(units::PLOT_AREA_KEY);
        let mut stages: HashMap<String, Vec<String>> = HashMap::new();
        let mut counts: HashMap<(String, String), u32> = HashMap::new();
        for (_e, c) in state
            .game_world
            .world
            .query::<&crate::ecs::components::CropInstance>()
            .iter()
        {
            let def = plant_reg.and_then(|r| r.get(&c.crop_def_id));
            if !stages.contains_key(&c.crop_def_id) {
                let names: Vec<String> = def
                    .map(|d| d.stages().iter().map(|s| s.to_string()).collect())
                    .unwrap_or_else(|| {
                        crate::ecs::components::DEFAULT_GROWTH_STAGES
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    });
                stages.insert(c.crop_def_id.clone(), names);
            }
            if let Some(tid) = &c.tower_id {
                counts.entry((c.crop_def_id.clone(), tid.clone())).or_insert_with(|| {
                    def.map_or(1, |d| units::plants_in_plot(d, units::plot_area(plot_areas, Some(tid))))
                });
            }
        }
        (stages, counts)
    };
    // The visual recipes (data/plants_visual.ron, the committed file the
    // only source, no embedded fallback yet) and the tower configs, re-read
    // only when a file changed on disk, so live editing still works while a
    // crop changing stage no longer opens and parses two files on the frame.
    let inputs = pass.inputs(forced);
    let visuals = inputs.visuals.clone();
    let vis_hash = inputs.vis_hash;
    let tower_cfgs = inputs.towers.clone();
    let default_stages: Vec<String> =
        crate::ecs::components::DEFAULT_GROWTH_STAGES.iter().map(|s| s.to_string()).collect();

    // Group crops by tower config id / grow machine instance id.
    let mut by_tower: HashMap<String, Vec<(String, String, u32, f32)>> = HashMap::new();
    for (_e, c) in state
        .game_world
        .world
        .query::<&crate::ecs::components::CropInstance>()
        .iter()
    {
        let (Some(tid), Some(slot)) = (c.tower_id.clone(), c.tower_slot) else { continue };
        by_tower.entry(tid).or_default().push((c.crop_def_id.clone(), c.growth_stage.clone(), slot, c.health));
    }
    for crops in by_tower.values_mut() {
        crops.sort_by(|x, y| x.2.cmp(&y.2).then_with(|| x.0.cmp(&y.0)));
    }
    if pass.material.is_none() {
        // Type 20 (v0.1063): albedo comes from the packed per-face UV
        // colors, same as type 12, but plants get their OWN type so they
        // (a) stop inheriting type 12's planet terminator gate, which read
        // base_color as a planet centre and switched direct sun off across
        // half of every garden, and (b) have somewhere to grow close-range
        // leaf/fruit detail. See assets/shaders/pbr/90-fragment-main.wgsl.
        pass.material = Some(state.renderer.add_material_typed([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 20.0));
    }
    if pass.rigid_material.is_none() {
        // The same type-20 look, flagged RIGID (2026-09-27): a mushroom
        // rack's blocks, beds and fruit stand in the still air of their
        // tent and must not sway with the wind the way leaves do.
        pass.rigid_material = Some(state.renderer.add_material_full(
            [1.0, 1.0, 1.0, 1.0],
            0.0,
            0.9,
            20.0,
            crate::terrain::plant_mesh::PLANT_RIGID_FLAG,
        ));
    }
    // Everything the worker has already sent is in hand before deciding
    // what to build again.
    pass.pull();

    let t_snapshot = std::time::Instant::now();
    let mut jobs: Vec<GroupJob> = Vec::new();
    let mut live: HashSet<String> = HashSet::new();
    for (cfg_id, crops) in &by_tower {
        // Resolve the crop group's world layout. Three key shapes:
        // - legacy tower CONFIG id ("nutrition", from the GUI Plant button):
        //   dress the first placed column of that type;
        // - tower INSTANCE id ("ntower_5", from the showcase auto-seed):
        //   dress that exact column;
        // - bed/field/rack INSTANCE id: its plots across the machine.
        let (helix_cfg, base, grid_spot) = if let Some(cfg) = tower_cfgs.iter().find(|t| t.id == *cfg_id) {
            let cat_key = format!("aeroponic_tower_{cfg_id}");
            match state.grow_positions.iter().find(|g| g.ty == cat_key) {
                Some(g) => (Some(cfg), g.pos, None),
                None => continue,
            }
        } else if let Some(g) = state.grow_positions.iter().find(|g| g.id == *cfg_id).cloned() {
            if let Some(cfg_key) = g.ty.strip_prefix("aeroponic_tower_") {
                match tower_cfgs.iter().find(|t| t.id == cfg_key) {
                    Some(cfg) => (Some(cfg), g.pos, None),
                    None => continue,
                }
            } else {
                let pos = g.pos;
                (None, pos, Some(g))
            }
        } else {
            continue;
        };
        let grid_spot = grid_spot.as_ref();
        // The machine's signature covers its crops, where it stands, the
        // visual recipes and the plants each plot holds.
        let group_sig = {
            let mut gh = std::collections::hash_map::DefaultHasher::new();
            vis_hash.hash(&mut gh);
            base.to_array().map(f32::to_bits).hash(&mut gh);
            if let Some(g) = grid_spot {
                g.ty.hash(&mut gh);
                [g.size.0, g.size.1, g.size.2, g.yaw].map(f32::to_bits).hash(&mut gh);
            }
            if let Some(cfg) = helix_cfg {
                cfg.slots.hash(&mut gh);
                [cfg.height_m, cfg.helix_turns, cfg.diameter_m].map(f32::to_bits).hash(&mut gh);
            }
            for (def_id, stage, slot, health) in crops {
                def_id.hash(&mut gh);
                stage.hash(&mut gh);
                slot.hash(&mut gh);
                ((health / 10.0) as u32).hash(&mut gh);
                plot_plant_count.get(&(def_id.clone(), cfg_id.clone())).hash(&mut gh);
            }
            gh.finish()
        };
        live.insert(cfg_id.clone());
        pass.wanted.insert(cfg_id.clone(), group_sig);
        // Where its machine stands, which the build editor's cut-away hides its plants by
        // (`plant_draws`). A move changes the signature too, so its plants are rebuilt anyway; a
        // changed foot recollects the draw list either way.
        if pass.feet.insert(cfg_id.clone(), base) != Some(base) {
            pass.objects_dirty = true;
        }
        if pass.groups.get(cfg_id).is_some_and(|g| g.sig == group_sig) || pass.queued(cfg_id, group_sig) {
            continue;
        }
        let layout = match (helix_cfg, grid_spot) {
            (Some(cfg), _) => GroupLayout::Tower {
                helix: Helix {
                    slots: cfg.slots.max(1),
                    turns: cfg.helix_turns,
                    height_m: cfg.height_m,
                    radius_m: cfg.diameter_m * 0.5,
                },
                base,
            },
            (None, Some(g)) => {
                // A bed/tray/field's plots (data/garden/grow_media.ron): the
                // medium's count, shelves when stacked. The farm gives every
                // crop in the machine one plot's floor, and a crop is sown
                // into plot `slot`.
                let (n, stacked) = state
                    .gui_state
                    .grow_media
                    .iter()
                    .find(|m| m.matches(&g.ty))
                    .map_or((1, false), |m| (m.plots.max(1), m.stacked));
                GroupLayout::Plots { pos: g.pos, yaw_deg: g.yaw, rects: plot_rects(g.size, n, stacked) }
            }
            (None, None) => continue,
        };
        let crops = crops
            .iter()
            .map(|(def_id, stage, slot, health)| {
                // Stage index -> growth t (same bucketing the GUI shows).
                let stages: &Vec<String> = stage_map.get(def_id).unwrap_or(&default_stages);
                let dead = stage.as_str() == crate::ecs::components::STAGE_DEAD;
                let t = if dead {
                    0.6
                } else {
                    stages
                        .iter()
                        .position(|s| *s == stage.as_str())
                        .map(|i| (i as f32 + 1.0) / stages.len().max(1) as f32)
                        .unwrap_or(0.1)
                };
                let wilt = if dead { 1.0 } else { (1.0 - health / 100.0).clamp(0.0, 1.0) };
                let holds = plot_plant_count.get(&(def_id.clone(), cfg_id.clone())).copied().unwrap_or(1);
                CropDraw { def_id: def_id.clone(), slot: *slot, t, wilt, dead, holds }
            })
            .collect();
        jobs.push(GroupJob { key: cfg_id.clone(), sig: group_sig, layout, crops });
    }
    let t_groups = std::time::Instant::now();
    // Where the plan's time went, for the slow-frame line: the crop
    // snapshot (with the data files), the per-machine signatures and jobs,
    // and the rest (parking slots, starting the worker).
    let ms = |a: std::time::Instant, b: std::time::Instant| (b - a).as_secs_f64() * 1000.0;
    pass.plan_split = [ms(t0, t_snapshot), ms(t_snapshot, t_groups), 0.0];
    // Machines with no crops left give their slots to the spare list.
    let gone: Vec<String> = pass.groups.keys().filter(|k| !live.contains(k.as_str())).cloned().collect();
    for key in gone {
        if let Some(g) = pass.groups.remove(&key) {
            for (mi, _) in g.slots {
                release_plant_slot(state, pass, mi);
            }
            pass.objects_dirty = true;
        }
    }
    pass.wanted.retain(|k, _| live.contains(k.as_str()));
    pass.feet.retain(|k, _| live.contains(k.as_str()));
    if let Some((_, cancel)) = pass.in_flight.take() {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if jobs.is_empty() {
        pass.plan_split[2] = ms(t_groups, std::time::Instant::now());
        return;
    }
    // The worker starts from every stage model already loaded or already
    // on its way in, and every one known not to exist.
    let mut models: HashMap<String, std::sync::Arc<crate::assets::GltfCpuMesh>> =
        pass.models.iter().map(|(k, (m, _))| (k.clone(), m.clone())).collect();
    let mut missing = state.hero_plant_missing.clone();
    for msg in &pass.inbox {
        match msg {
            WorkerMsg::Model { name, cpu, .. } => {
                models.insert(name.clone(), cpu.clone());
            }
            WorkerMsg::Missing(name) => {
                missing.insert(name.clone());
            }
            _ => {}
        }
    }
    if !pass.batch.active {
        pass.batch = PlantBatch { active: true, ..PlantBatch::default() };
    }
    pass.generation += 1;
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let job = Job {
        generation: pass.generation,
        groups: jobs,
        visuals,
        models,
        missing,
        data_dir: state.asset_manager.data_path(""),
        cancel: cancel.clone(),
        checksum: std::env::var("HUMANITY_PLANT_CHECKSUM").is_ok_and(|v| v == "1"),
    };
    crate::engine::plant_pass::spawn(job, pass.sender());
    pass.plan_split[2] = ms(t_groups, std::time::Instant::now());
    pass.in_flight = Some((pass.generation, cancel));
}

/// A frame's share of uploading the worker's plants (2026-09-27): about
/// this many milliseconds of the frame, counting the plan before it, and
/// never less than one message so the garden always finishes.
const PLANT_UPLOAD_BUDGET_MS: f64 = 3.0;

/// Take what the worker has finished, in order, until this frame's budget
/// is spent: a newly loaded stage model gets its material, a model found
/// missing is remembered, a machine whose plants are still wanted is
/// uploaded into its own mesh slots (then spare ones), and the job's end
/// clears the pass to go idle. Returns the machines uploaded and their
/// vertices.
fn drain_plant_work(state: &mut EngineState, pass: &mut PlantPass, t0: std::time::Instant) -> DrainStats {
    use crate::engine::plant_pass::WorkerMsg;
    pass.pull();
    let mut out = DrainStats::default();
    let mut spent = false;
    while !pass.inbox.is_empty() {
        if spent && t0.elapsed().as_secs_f64() * 1000.0 >= PLANT_UPLOAD_BUDGET_MS {
            break;
        }
        let Some(msg) = pass.inbox.pop_front() else { break };
        let m0 = std::time::Instant::now();
        let what = match msg {
            WorkerMsg::Model { name, cpu, texture } => {
                let size = texture.as_ref().map_or((0, 0), |t| (t.1, t.2));
                if !pass.models.contains_key(&name) {
                    let material = stage_model_material(state, texture);
                    pass.models.insert(name.clone(), (cpu, material));
                    spent = true;
                }
                format!("stage model {name} ({} x {} texture)", size.0, size.1)
            }
            WorkerMsg::Missing(name) => {
                let what = format!("missing model {name}");
                state.hero_plant_missing.insert(name);
                what
            }
            WorkerMsg::Group(g) => {
                let wanted = pass.wanted.get(&g.key) == Some(&g.sig);
                let drawn = pass.groups.get(&g.key).is_some_and(|d| d.sig == g.sig);
                let (key, n) = (g.key.clone(), g.vertex_count());
                if wanted && !drawn {
                    out.verts += upload_plant_group(state, pass, g);
                    out.groups += 1;
                    spent = true;
                    format!("upload of {key} ({n} vertices)")
                } else {
                    format!("stale {key} dropped ({n} vertices)")
                }
            }
            WorkerMsg::Done { generation, worker_ms } => {
                pass.batch.worker_ms += worker_ms;
                if pass.in_flight.as_ref().is_some_and(|(g, _)| *g == generation) {
                    pass.in_flight = None;
                }
                "end of a job".to_string()
            }
        };
        out.taken += 1;
        let ms = m0.elapsed().as_secs_f64() * 1000.0;
        if ms > out.slowest_ms {
            out.slowest_ms = ms;
            out.slowest = what;
        }
    }
    let r0 = std::time::Instant::now();
    if pass.objects_dirty {
        // Each mesh with the foot of the machine it grows on (`PlantPass::feet`).
        let feet = &pass.feet;
        state.plant_objects = pass
            .groups
            .iter()
            .flat_map(|(key, g)| {
                let foot = feet.get(key).copied().unwrap_or(Vec3::ZERO);
                g.slots.iter().map(move |&(mesh, material)| (mesh, material, foot))
            })
            .collect();
        pass.objects_dirty = false;
    }
    out.collect_ms = r0.elapsed().as_secs_f64() * 1000.0;
    pass.batch.machines += out.groups;
    pass.batch.verts += out.verts;
    out
}

/// One frame's [`drain_plant_work`]: machines uploaded and their vertices,
/// and for the slow-frame line the messages taken and the slowest of them.
#[derive(Default)]
struct DrainStats {
    groups: usize,
    verts: usize,
    taken: usize,
    slowest: String,
    slowest_ms: f64,
    collect_ms: f64,
}

/// A plant-pass frame this slow (main thread, ms) is written to run.log
/// with where its time went, so a hitch the pass still causes can be traced
/// rather than guessed at (2026-09-27).
const SLOW_PLANT_FRAME_MS: f64 = 8.0;

fn note_slow_plant_frame(ms: f64, plan_ms: f64, files_ms: f64, split: [f64; 3], d: &DrainStats) {
    static LINES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    // The first 40 of a session: enough to see a pattern, never a flood.
    if LINES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 40 {
        return;
    }
    log::info!(
        "[Plants] slow plant-pass frame: {ms:.2} ms = plan {plan_ms:.2} ms (crop snapshot {:.2}, {files_ms:.2} of \
         it on the data files; signatures and jobs {:.2}; the rest {:.2}) + {} message(s) taken, the slowest the \
         {} in {:.2} ms, then {:.2} ms collecting the draw list",
        split[0],
        split[1],
        split[2],
        d.taken,
        if d.slowest.is_empty() { "none" } else { d.slowest.as_str() },
        d.slowest_ms,
        d.collect_ms
    );
}

/// Put one machine's finished plants on the GPU (2026-09-27, the upload the
/// 2026-09-26 rebuild did inline): its meshes go into the machine's own
/// slots first, then spare ones, adding a slot only past both, and a slot
/// it no longer needs is parked. Returns its vertices.
fn upload_plant_group(
    state: &mut EngineState,
    pass: &mut PlantPass,
    g: crate::engine::plant_pass::BuiltGroup,
) -> usize {
    let Some(plant_material) = pass.material else { return 0 };
    // Each finished mesh with its material: the shared procedural plant
    // material, or a stage model's textured one.
    let mut built: Vec<(&crate::terrain::plant_mesh::PlantMeshBuilder, usize)> = Vec::new();
    if !g.procedural.vertices.is_empty() {
        built.push((&g.procedural, plant_material));
    }
    if !g.rigid.vertices.is_empty() {
        built.push((&g.rigid, pass.rigid_material.unwrap_or(plant_material)));
    }
    for (name, m) in &g.models {
        match pass.models.get(name) {
            Some(&(_, material)) => built.push((m, material)),
            None => log::warn!("[Plants] stage model {name} arrived without its material; its plants are skipped"),
        }
    }
    let mut old_slots = pass.groups.remove(&g.key).map(|p| p.slots).unwrap_or_default();
    let mut slots_now = Vec::with_capacity(built.len());
    let (mut verts, mut bytes) = (0usize, 0u64);
    for (mesh_b, material) in built {
        verts += mesh_b.vertices.len();
        bytes += (std::mem::size_of_val(mesh_b.vertices.as_slice()) + std::mem::size_of_val(mesh_b.indices.as_slice()))
            as u64;
        let mesh = Mesh::from_vertices(&state.renderer.device, &mesh_b.vertices, &mesh_b.indices);
        let mi = match old_slots.pop().map(|(mi, _)| mi).or_else(|| pass.spare.pop()) {
            Some(mi) => {
                state.renderer.replace_mesh(mi, mesh);
                mi
            }
            None => state.renderer.add_mesh(mesh),
        };
        slots_now.push((mi, material));
    }
    for (mi, _) in old_slots {
        release_plant_slot(state, pass, mi);
    }
    pass.groups.insert(
        g.key.clone(),
        PlantGroupMeshes { sig: g.sig, slots: slots_now, drawn: g.drawn, meant: g.meant, verts, bytes, checksum: g.checksum },
    );
    pass.objects_dirty = true;
    verts
}

/// The rebuild that just went idle, into run.log (`note_plant_rebuild`).
fn finish_plant_batch(state: &EngineState, pass: &mut PlantPass) {
    let b = std::mem::take(&mut pass.batch);
    let checksum = pass
        .groups
        .values()
        .map(|g| g.checksum)
        .try_fold(0u64, |acc, c| c.map(|c| acc.wrapping_add(c)));
    note_plant_rebuild(PlantRebuild {
        ms: b.main_ms,
        worst_ms: b.worst_ms,
        frames: b.frames,
        worker_ms: b.worker_ms,
        machines: pass.groups.len(),
        machines_rebuilt: b.machines,
        draws: state.plant_objects.len(),
        spare: pass.spare.len(),
        drawn: pass.groups.values().map(|g| g.drawn).sum(),
        meant: pass.groups.values().map(|g| g.meant).sum(),
        verts_built: b.verts,
        verts_held: pass.groups.values().map(|g| g.verts).sum(),
        bytes_held: pass.groups.values().map(|g| g.bytes).sum(),
        checksum: checksum.filter(|_| !pass.groups.is_empty()),
    });
}

/// Park a plant mesh slot the pass no longer draws (2026-09-26): its
/// buffers are swapped for a one-triangle stand-in, so the geometry is freed
/// at once, and the slot waits in `spare` for the next mesh that needs one.
/// Without the swap a slot let go kept its last vertices until it was reused
/// (one rice tray plot at its last stage is about 4 MB).
fn release_plant_slot(state: &mut EngineState, pass: &mut PlantPass, mi: usize) {
    let stub = stand_in_mesh(&state.renderer.device);
    state.renderer.replace_mesh(mi, stub);
    pass.spare.push(mi);
}

/// The garden's drawing state between frames (2026-09-27): the plant pass
/// and the grow enclosures. On `EngineState::garden_draw`.
#[derive(Default)]
pub(crate) struct GardenDraw {
    /// The plant pass ([`rebuild_plant_meshes`]).
    pub(crate) plants: PlantPass,
    /// Every grow enclosure to draw, from [`publish_grow_plots`]: the
    /// machine's floor position, its yaw in degrees and the enclosure's size.
    tents: Vec<(Vec3, f32, (f32, f32, f32))>,
    /// A tent mesh per enclosure size (one size today), and their material.
    tent_meshes: Vec<((f32, f32, f32), usize)>,
    tent_material: Option<usize>,
}

/// What the plant pass keeps between frames (2026-09-26 as a cache in the
/// DataStore; on EngineState since it got a worker thread, 2026-09-27).
#[derive(Default)]
pub(crate) struct PlantPass {
    /// The type-20 material every procedural plant mesh draws with.
    material: Option<usize>,
    /// The same material flagged rigid (`plant_mesh::PLANT_RIGID_FLAG`), for
    /// geometry that must not sway: mushroom blocks, beds and their fruit.
    rigid_material: Option<usize>,
    /// Mesh slots no machine needs now, reused before adding more.
    spare: Vec<usize>,
    /// Stage models by name ("wheat_3"): the geometry baked per plant (the
    /// worker reads it) and the model's textured material.
    models: std::collections::HashMap<String, (std::sync::Arc<crate::assets::GltfCpuMesh>, usize)>,
    /// Each machine's drawn meshes, by crop group key.
    groups: std::collections::HashMap<String, PlantGroupMeshes>,
    /// The signature each machine's plants should be built from (the last
    /// plan); a finished build with any other is stale and dropped.
    wanted: std::collections::HashMap<String, u64>,
    /// Where each machine with plants stands (its placement, world metres),
    /// by crop group key, from the last plan: the build editor's cut-away
    /// hides a machine's plants by it, as it hides the machine
    /// (`plant_draws`; increment 7 follow-ups, 2026-10-21).
    feet: std::collections::HashMap<String, Vec3>,
    /// The job building now: its generation and its cancel flag.
    in_flight: Option<(u64, std::sync::Arc<std::sync::atomic::AtomicBool>)>,
    generation: u64,
    /// Worker to frame. One channel for the session, so a cancelled job's
    /// last messages still arrive (its stage models are kept).
    chan: Option<(
        std::sync::mpsc::Sender<crate::engine::plant_pass::WorkerMsg>,
        std::sync::mpsc::Receiver<crate::engine::plant_pass::WorkerMsg>,
    )>,
    /// Received, not yet taken (the upload budget ran out).
    inbox: std::collections::VecDeque<crate::engine::plant_pass::WorkerMsg>,
    /// `EngineState::plant_objects` needs recollecting.
    objects_dirty: bool,
    /// Totals of the rebuild in progress, for run.log.
    batch: PlantBatch,
    /// The data files a plan reads, parsed, and how long the last plan
    /// spent re-reading them (0 when they were unchanged).
    inputs: Option<PlanInputs>,
    inputs_ms: f64,
    /// The last plan's milliseconds: crop snapshot, signatures and jobs,
    /// the rest.
    plan_split: [f64; 3],
}

/// The data files a plan reads (2026-09-27): the visual recipes and the
/// tower configs, parsed, with the stamps of the files they came from.
struct PlanInputs {
    stamps: [FileStamp; 2],
    visuals: std::sync::Arc<crate::terrain::plant_mesh::PlantVisualRegistry>,
    /// Hash of the recipes' text: part of every machine's signature, so an
    /// edit to the file replants the whole garden.
    vis_hash: u64,
    towers: Vec<crate::gui::TowerConfig>,
}

/// A file's modified time and length: a changed file changes it, and
/// reading it is a stat, not an open (opening a file is what a virus
/// scanner watches, and what cost the plan milliseconds).
type FileStamp = Option<(std::time::SystemTime, u64)>;

fn file_stamp(path: &std::path::Path) -> FileStamp {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn read_plan_inputs(dir: &std::path::Path, stamps: [FileStamp; 2]) -> PlanInputs {
    use std::hash::{Hash, Hasher};
    let vis_text = std::fs::read_to_string(dir.join("plants_visual.ron")).unwrap_or_default();
    let visuals = crate::terrain::plant_mesh::PlantVisualRegistry::from_ron(&vis_text).unwrap_or_default();
    let mut vh = std::collections::hash_map::DefaultHasher::new();
    vis_text.hash(&mut vh);
    PlanInputs {
        stamps,
        visuals: std::sync::Arc::new(visuals),
        vis_hash: vh.finish(),
        towers: crate::gui::load_tower_configs(dir),
    }
}

impl PlantPass {
    /// The plan's data files, re-read when `forced` or when either file's
    /// stamp moved since they were last read.
    fn inputs(&mut self, forced: bool) -> &PlanInputs {
        // Timed from the stamps on: `inputs_ms` is the plan's file work.
        let t0 = std::time::Instant::now();
        let dir = crate::data_dir();
        let stamps = [file_stamp(&dir.join("plants_visual.ron")), file_stamp(&dir.join("towers/aeroponic_configs.ron"))];
        if forced || self.inputs.as_ref().map_or(true, |i| i.stamps != stamps) {
            self.inputs = Some(read_plan_inputs(&dir, stamps));
        }
        self.inputs_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.inputs.get_or_insert_with(|| read_plan_inputs(&dir, stamps))
    }

    /// Nothing building and nothing waiting to upload.
    fn idle(&self) -> bool {
        self.in_flight.is_none() && self.inbox.is_empty()
    }

    /// A sender for a new job's worker.
    fn sender(&mut self) -> std::sync::mpsc::Sender<crate::engine::plant_pass::WorkerMsg> {
        self.chan.get_or_insert_with(std::sync::mpsc::channel).0.clone()
    }

    /// Move everything the worker has sent into the inbox.
    fn pull(&mut self) {
        if let Some((_, rx)) = &self.chan {
            while let Ok(msg) = rx.try_recv() {
                self.inbox.push_back(msg);
            }
        }
    }

    /// Is `key`'s plants at `sig` already built and waiting to upload?
    fn queued(&self, key: &str, sig: u64) -> bool {
        self.inbox.iter().any(|m| {
            matches!(m, crate::engine::plant_pass::WorkerMsg::Group(g) if g.key == key && g.sig == sig)
        })
    }
}

/// One machine's plant meshes and what they were built from.
struct PlantGroupMeshes {
    /// Signature of everything that shaped them; equal means reuse as is.
    sig: u64,
    /// (mesh, material) per draw.
    slots: Vec<(usize, usize)>,
    /// Plants and clumps drawn, and the plants the crops really hold.
    drawn: usize,
    meant: u64,
    /// Vertices in these meshes, and their vertex and index bytes.
    verts: usize,
    bytes: u64,
    /// `plant_pass::geometry_checksum` of them, when asked for.
    checksum: Option<u64>,
}

/// Totals of one rebuild, from the plan that starts it until the pass is
/// idle again.
#[derive(Default)]
struct PlantBatch {
    active: bool,
    /// Main-thread milliseconds in all, the worst frame's, and the frames.
    main_ms: f64,
    worst_ms: f64,
    frames: u32,
    /// Worker milliseconds (every job the rebuild started).
    worker_ms: f64,
    /// Machines uploaded and their vertices.
    machines: usize,
    verts: usize,
}

/// Running totals behind the plant-rebuild line in run.log.
struct PlantRebuildLog {
    since: std::time::Instant,
    rebuilds: u32,
    sum_ms: f64,
    max_ms: f64,
    /// Whether a rebuild that built geometry has been logged yet.
    built_logged: bool,
}

/// What one plant rebuild did, for [`note_plant_rebuild`].
struct PlantRebuild {
    /// Main-thread milliseconds in all, in the worst frame, and the frames
    /// it was spread over.
    ms: f64,
    worst_ms: f64,
    frames: u32,
    /// Milliseconds the worker spent building (off the frame).
    worker_ms: f64,
    /// Planted machines (towers, beds, fields), and how many were rebuilt.
    machines: usize,
    machines_rebuilt: usize,
    /// Plant draw calls, the plants and clumps in them, and the plants the
    /// crops really hold (more than `drawn` where a plot is over the cap).
    draws: usize,
    /// Mesh slots parked for reuse (their geometry already freed).
    spare: usize,
    drawn: usize,
    meant: u64,
    /// Vertices generated this rebuild (the rebuilt machines only), and in
    /// all the plant meshes drawn after it.
    verts_built: usize,
    verts_held: usize,
    /// GPU bytes of the plant meshes drawn (vertex + index buffers).
    bytes_held: u64,
    /// Sum of every drawn machine's geometry checksum, with
    /// `HUMANITY_PLANT_CHECKSUM=1`: equal before and after a change to the
    /// pass means it draws exactly the same garden.
    checksum: Option<u64>,
}

/// Summarise the garden plant rebuilds in run.log (2026-09-26): the first
/// rebuild and the first that builds any geometry (the full build after the
/// garden is sown, the expensive one) at once, then one line per ten seconds
/// at most. A fresh world's clock runs fast, so some crop changes stage every
/// few seconds and a line per rebuild would bury the log.
fn note_plant_rebuild(r: PlantRebuild) {
    static LOG: std::sync::Mutex<Option<PlantRebuildLog>> = std::sync::Mutex::new(None);
    let Ok(mut guard) = LOG.lock() else { return };
    let first = guard.is_none();
    let s = guard.get_or_insert_with(|| PlantRebuildLog {
        since: std::time::Instant::now(),
        rebuilds: 0,
        sum_ms: 0.0,
        max_ms: 0.0,
        built_logged: false,
    });
    s.rebuilds += 1;
    s.sum_ms += r.ms;
    s.max_ms = s.max_ms.max(r.worst_ms);
    let secs = s.since.elapsed().as_secs_f64();
    if first || secs >= 10.0 || (!s.built_logged && r.verts_built > 0) {
        let checksum = r.checksum.map_or(String::new(), |c| format!(", geometry checksum {c:016x}"));
        log::info!(
            "[Plants] {} mesh rebuild(s) in {:.1} s: last {:.2} ms on the frame over {} frames (worst frame {:.2} ms) \
             + {:.1} ms on the worker ({} of {} machines, {} vertices), worst frame of all {:.2} ms; now {} draws \
             ({} spare slots), {} plants drawn for {} grown, {} vertices, {:.1} MB{checksum}",
            s.rebuilds,
            secs,
            r.ms,
            r.frames,
            r.worst_ms,
            r.worker_ms,
            r.machines_rebuilt,
            r.machines,
            r.verts_built,
            s.max_ms,
            r.draws,
            r.spare,
            r.drawn,
            r.meant,
            r.verts_held,
            r.bytes_held as f64 / 1_048_576.0
        );
        *s = PlantRebuildLog {
            since: std::time::Instant::now(),
            rebuilds: 0,
            sum_ms: 0.0,
            max_ms: 0.0,
            built_logged: s.built_logged || r.verts_built > 0,
        };
    }
}

/// Frame times around a garden rebuild (2026-09-27). The plant pass runs
/// once a frame (`hot_reload_gates::poll_and_apply`), so the time between two of
/// its calls is one whole frame. The probe keeps the last 30 of them and,
/// from the frame the pass starts building geometry until five frames after
/// it has none left to do, the worst and the mean frame, and the plant
/// pass's own main-thread milliseconds. A rebuild big enough to matter (a
/// quarter of a million vertices, or any frame over twice the typical one)
/// is written to run.log as one line, so a hitch is a reading, not a guess.
struct HitchProbe {
    last: Option<std::time::Instant>,
    recent: std::collections::VecDeque<f32>,
    window: Option<HitchWindow>,
}

/// One rebuild's frames, for [`HitchProbe`].
#[derive(Default)]
struct HitchWindow {
    /// Median frame of the 30 before the rebuild began, ms.
    typical_ms: f32,
    frames: u32,
    worst_frame_ms: f32,
    sum_frame_ms: f32,
    /// The plant pass's main-thread time: its worst single frame and total.
    worst_pass_ms: f32,
    sum_pass_ms: f32,
    /// Frames since the pass last had work.
    quiet: u32,
    machines: usize,
    verts: usize,
}

static HITCH_PROBE: std::sync::Mutex<HitchProbe> =
    std::sync::Mutex::new(HitchProbe { last: None, recent: std::collections::VecDeque::new(), window: None });

/// Stamp the start of this frame's plant pass; the milliseconds since the
/// last stamp (one whole frame) or 0 on the first call.
fn hitch_probe_frame() -> f32 {
    let Ok(mut p) = HITCH_PROBE.lock() else { return 0.0 };
    let now = std::time::Instant::now();
    let ms = p.last.map_or(0.0, |t| (now - t).as_secs_f32() * 1000.0);
    p.last = Some(now);
    if ms > 0.0 {
        p.recent.push_back(ms);
        while p.recent.len() > 30 {
            p.recent.pop_front();
        }
    }
    ms
}

/// Record this frame's plant pass: `frame_ms` from [`hitch_probe_frame`],
/// the pass's own main-thread `pass_ms`, whether it had work, and what it
/// built. Opens a window on the first busy frame and closes it (logging it
/// when it matters) five quiet frames later.
fn hitch_probe_pass(frame_ms: f32, pass_ms: f32, busy: bool, machines: usize, verts: usize) {
    let Ok(mut p) = HITCH_PROBE.lock() else { return };
    let p = &mut *p;
    if let Some(w) = p.window.as_mut() {
        // This frame's interval is the frame that followed the previous
        // pass, so it belongs to the window already open.
        w.frames += 1;
        w.worst_frame_ms = w.worst_frame_ms.max(frame_ms);
        w.sum_frame_ms += frame_ms;
    }
    if busy && p.window.is_none() {
        let mut sorted: Vec<f32> = p.recent.iter().copied().collect();
        // The interval that ends at this frame came BEFORE the work.
        sorted.sort_by(f32::total_cmp);
        let typical_ms = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        p.window = Some(HitchWindow { typical_ms, ..HitchWindow::default() });
    }
    let Some(w) = p.window.as_mut() else { return };
    w.worst_pass_ms = w.worst_pass_ms.max(pass_ms);
    w.sum_pass_ms += pass_ms;
    w.machines += machines;
    w.verts += verts;
    if busy {
        w.quiet = 0;
        return;
    }
    w.quiet += 1;
    if w.quiet < 5 {
        return;
    }
    let w = p.window.take().unwrap_or_default();
    if w.verts >= 250_000 || (w.typical_ms > 0.0 && w.worst_frame_ms > 2.0 * w.typical_ms) {
        log::info!(
            "[Plants] frame times around a garden rebuild: typical {:.1} ms before; over the {} frames \
             it took, worst {:.1} ms, mean {:.1} ms. The plant pass on the main thread: worst {:.2} ms \
             in one frame, {:.1} ms in all ({} machines, {} vertices)",
            w.typical_ms,
            w.frames,
            w.worst_frame_ms,
            w.sum_frame_ms / w.frames.max(1) as f32,
            w.worst_pass_ms,
            w.sum_pass_ms,
            w.machines,
            w.verts
        );
    }
}

/// The material a hero crop stage model draws with (v0.992): its textured
/// type-19 material, or a plain green one when the model carries no texture.
/// `CROP_LEAF_LIGHT` in params.w (2026-09-29) turns on the type-19 foliage
/// transmission, the sunlight a leaf lets through when the sun is behind it
/// (shadow-gated, the same term the near trees' leaves use), and, being
/// negative, leaves the wind off: crops indoors stand in still air. It does
/// not light leaf backs: opaque draws cull back faces, so a leaf's back is
/// never drawn. (The black tower sprouts this was first blamed on were the
/// palettes' colour space, fixed in the models on 2026-10-03.)
/// Made once per model, when the worker first sends it (2026-09-27; the
/// model's geometry used to load right here, on the frame).
const CROP_LEAF_LIGHT: f32 = -2.0;

fn stage_model_material(state: &mut EngineState, texture: Option<(Vec<u8>, u32, u32)>) -> usize {
    match texture {
        Some((rgba, w, h)) => state.renderer.add_textured_material([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 19.0, CROP_LEAF_LIGHT, &rgba, w, h),
        None => state.renderer.add_material_full([0.35, 0.5, 0.3, 1.0], 0.0, 0.9, 0.0, 0.0),
    }
}

/// The clear sheeting of a grow enclosure (2026-09-27): a mushroom rack's
/// fruiting tent is clear PVC over wire shelving. Nearly colourless at
/// `TENT_ALPHA` opacity so the shelves inside stay readable, smooth, and
/// drawn as material type 4 (glass), whose Fresnel term brightens the sheet
/// where it is seen edge-on: the way a tent's sides and seams catch the
/// light while its face looks through.
const TENT_COLOR: [f32; 3] = [0.92, 0.95, 0.97];
const TENT_ALPHA: f32 = 0.18;
const TENT_ROUGHNESS: f32 = 0.12;

/// The plant meshes to draw: each of `plants` (mesh, material, the foot of the machine it grows on,
/// world metres; `EngineState::plant_objects`) but those the build editor's cut-away leaves out,
/// by that foot, as it leaves out the machine (`CutAway::hides`, `push_home_machines`). A plant
/// mesh's geometry is in world coordinates, so each is drawn at the origin. Increment 7 follow-ups
/// (2026-10-21): the plants and the grow tents ignored the cut-away, so a grow machine upstairs
/// would have left its plants in the air over the cut. Nothing grows upstairs in the homestead
/// today; a player can put a bed anywhere.
pub(crate) fn plant_draws(plants: &[(usize, usize, Vec3)], cut: impl Into<crate::engine::editor::Cuts>) -> Vec<RenderObject> {
    let cut = cut.into();
    plants
        .iter()
        .filter(|(_, _, foot)| !cut.hides(*foot))
        .map(|&(mesh, material, _)| RenderObject {
            fade: 0.0,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh,
            material,
        })
        .collect()
}

/// The grow tents to draw: each of `tents` (its machine's floor position, its yaw, its size;
/// `GardenDraw::tents`) the build editor's cut-away does not leave out (`plant_draws`).
pub(crate) fn tents_shown(
    tents: &[(Vec3, f32, (f32, f32, f32))],
    cut: impl Into<crate::engine::editor::Cuts>,
) -> Vec<(Vec3, f32, (f32, f32, f32))> {
    let cut = cut.into();
    tents.iter().filter(|(pos, _, _)| !cut.hides(*pos)).copied().collect()
}

/// Push this frame's garden plants (v0.862: one merged world-space mesh per planted machine and
/// stage model) and the photoscanned decoration plants (v0.909) into `out` (the render loop,
/// lib.rs), but those the build editor's cut-away leaves out (`HomeDraw::cut`): a plant by the
/// foot of its machine (`plant_draws`), a decoration by where it stands.
pub(crate) fn push_home_plants(state: &EngineState, out: &mut Vec<RenderObject>) {
    let cut = &state.home_draw.cut;
    out.extend(plant_draws(&state.plant_objects, cut.clone()));
    for &(mesh, material, pos, yaw, scl, _anchor) in &state.decoration_objects {
        if !cut.hides(pos) {
            out.push(RenderObject {
                fade: 0.0,
                position: pos,
                rotation: Quat::from_rotation_y(yaw.to_radians()),
                scale: Vec3::splat(scl),
                mesh,
                material,
            });
        }
    }
}

/// Draw every grow enclosure (a fruiting tent around each mushroom rack,
/// 2026-09-27) into the transparent list: one tent mesh per enclosure size
/// from `plant_layout::tent_geometry`, placed on its machine with the
/// machine's yaw. The enclosures come from the grow media
/// (`enclosure` in data/garden/grow_media.ron), recorded by
/// [`publish_grow_plots`]; the frame draws them with the machines, so they
/// hide with them, and the build editor's cut-away leaves them out with
/// their machines (`tents_shown`, 2026-10-21).
pub(crate) fn push_grow_enclosures(state: &mut EngineState, transparent: &mut Vec<RenderObject>) {
    if state.garden_draw.tents.is_empty() {
        return;
    }
    let material = match state.garden_draw.tent_material {
        Some(m) => m,
        None => {
            let [r, g, b] = TENT_COLOR;
            let m = state.renderer.add_material_full([r, g, b, TENT_ALPHA], 0.0, TENT_ROUGHNESS, 4.0, 0.0);
            state.garden_draw.tent_material = Some(m);
            m
        }
    };
    // Those the build editor's cut-away leaves out are not drawn (`tents_shown`).
    for (pos, yaw, size) in tents_shown(&state.garden_draw.tents, state.home_draw.cut.clone()) {
        let mesh = match state.garden_draw.tent_meshes.iter().find(|(s, _)| *s == size) {
            Some(&(_, m)) => m,
            None => {
                let (v, ix) = crate::engine::plant_layout::tent_geometry(size);
                let m = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &v, &ix));
                state.garden_draw.tent_meshes.push((size, m));
                m
            }
        };
        transparent.push(RenderObject {
            fade: 0.0,
            position: pos,
            rotation: Quat::from_rotation_y(yaw.to_radians()),
            scale: Vec3::ONE,
            mesh,
            material,
        });
    }
}

/// Each storey's SERVICE LEVEL in a body standing at `origin` (world metres), bottom up: (its floor,
/// the ceiling its pipes hang from: the underside of the floor slabs over it, or the roof for the
/// top storey). Ship homes increment 7: pipes run under their own storey's ceiling, never through
/// the floor above. A body with no slab has the one level it always had, floor to roof.
pub(crate) fn service_levels(body: &crate::ship::home_structure::HomeStructure, origin: Vec3) -> Vec<(f32, f32)> {
    let storeys = body.storeys();
    storeys
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let lid = storeys
                .get(k + 1)
                .map_or(0.0, |above| body.floors.iter().filter(|f| f.is_floor_of(above)).map(|f| f.thickness.max(0.0)).fold(0.0, f32::max));
            (origin.y + s.floor, origin.y + s.top - lid)
        })
        .collect()
}

/// Where a run between ends standing on floors `foot_a` and `foot_b` (world metres: a machine's
/// storey floor, `storey_floor_at`) hangs: (its service height, the ceiling its hangers reach), on
/// the LOWER of the two ends' levels (`service_levels`). A run crosses under the ceiling of the
/// storey it starts in, and a run to a machine upstairs rises through the floor only under that
/// machine, as a real riser does (`route_run`). Until the increment 7 review (2026-10-21) it took
/// the higher level, so every run from a machine downstairs to one upstairs rose at the downstairs
/// end, through the upper floor, and crossed the upper storey under its glass roof, through rooms
/// like the bedroom that have nothing to do with it. The service height is 0.3 m under the
/// ceiling, never under 0.6 m over the floor.
pub(crate) fn run_level(levels: &[(f32, f32)], foot_a: f32, foot_b: f32) -> (f32, f32) {
    let on = |y: f32| levels.iter().rposition(|(floor, _)| *floor <= y + crate::ship::home_structure::STOREY_EPS).unwrap_or(0);
    let (floor, ceiling) = levels.get(on(foot_a).min(on(foot_b))).copied().unwrap_or((0.0, 3.0));
    ((ceiling - 0.3).max(floor + 0.6), ceiling)
}

/// One run to route (`conduit_runs`): its two ends (world metres: a machine's port of the run's
/// utility, else its low anchor, or a conduit node), what it carries, the ids it joins ("node:<id>"
/// for a node; the flow markers match them), and the floor each end stands on (a machine's storey
/// floor, `storey_floor_at`; a node's own height), which picks the run's level (`run_level`).
#[derive(Debug, Clone)]
pub(crate) struct ConduitRun {
    pub(crate) a: Vec3,
    pub(crate) b: Vec3,
    pub(crate) kind: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) foot_a: f32,
    pub(crate) foot_b: f32,
}

/// Every run of the home's machine layout `home`, its machines placed at `placements` with their
/// port gizmos `port_pick` (`port_picks`): the point-to-point connections, then the conduit node
/// graph's edges (v0.581), one routing list for both. A wire ends at its machine's PORT of its
/// utility (v0.627, the gizmo above the machine), else at the machine's low anchor 0.35 m over the
/// floor of the storey it stands on; a node (it carries no zone) is clamped into the ship's box.
pub(crate) fn conduit_runs(
    ship: Option<&ShipStructure>,
    home: &crate::machines::MachineHome,
    placements: &[crate::machines::PlacedMachine],
    port_pick: &[(String, usize, crate::utilities::Port, Vec3)],
) -> Vec<ConduitRun> {
    use std::collections::HashMap;
    let node_bounds = match ship {
        Some(ship) => {
            let (mn, mx) = ship.world_bounds();
            ((mn.x, mn.y, mn.z), (mx.x, mx.y, mx.z))
        }
        None => ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
    };
    // The floor of the storey each machine stands on (ship homes increment 7): its placement's
    // floor is its zone's, which upstairs is the storey below. A sphere's position is its centre,
    // so its foot is a radius under it.
    let storey_floor: HashMap<&str, f32> = placements
        .iter()
        .map(|p| {
            let foot = if p.shape == "sphere" { p.pos.1 - p.size.0 } else { p.pos.1 };
            (p.id.as_str(), storey_floor_at(ship, Vec3::new(p.pos.0, foot, p.pos.2), p.floor_y))
        })
        .collect();
    let anchors: HashMap<&str, Vec3> =
        placements.iter().map(|p| (p.id.as_str(), Vec3::new(p.pos.0, storey_floor[p.id.as_str()] + 0.35, p.pos.2))).collect();
    let port_pos = |id: &str, kind: &str| -> Option<Vec3> {
        port_pick.iter().find(|(mid, _, port, _)| mid == id && port.utility.id() == kind).map(|(_, _, _, wp)| *wp)
    };
    let foot = |id: &str, at: Vec3| storey_floor.get(id).copied().unwrap_or(at.y);
    let mut runs: Vec<ConduitRun> = home
        .connections
        .iter()
        .filter_map(|c| {
            let a = port_pos(&c.from, &c.kind).or_else(|| anchors.get(c.from.as_str()).copied())?;
            let b = port_pos(&c.to, &c.kind).or_else(|| anchors.get(c.to.as_str()).copied())?;
            let (foot_a, foot_b) = (foot(&c.from, a), foot(&c.to, b));
            Some(ConduitRun { a, b, kind: c.kind.clone(), from: c.from.clone(), to: c.to.clone(), foot_a, foot_b })
        })
        .collect();
    let placement_tuples: Vec<(String, (f32, f32, f32), f32)> =
        placements.iter().map(|p| (p.id.clone(), p.pos, storey_floor[p.id.as_str()])).collect();
    let end_id = |e: &crate::machines::ConduitEnd| match e {
        crate::machines::ConduitEnd::Machine(id) => id.clone(),
        crate::machines::ConduitEnd::Node(id) => format!("node:{id}"),
    };
    for e in &home.conduit_edges {
        if let (Some(a), Some(b)) =
            (home.conduit_anchor(&e.from, &placement_tuples, node_bounds), home.conduit_anchor(&e.to, &placement_tuples, node_bounds))
        {
            let (a, b) = (Vec3::new(a.0, a.1, a.2), Vec3::new(b.0, b.1, b.2));
            let (from, to) = (end_id(&e.from), end_id(&e.to));
            let (foot_a, foot_b) = (foot(&from, a), foot(&to, b));
            runs.push(ConduitRun { a, b, kind: e.kind.clone(), from, to, foot_a, foot_b });
        }
    }
    runs
}

/// What the runs are routed against (`route_run`): the home zone's storey levels
/// (`service_levels`; one level shared by every zone, as since v0.754), where it stands, its floor
/// slabs (a riser's sleeves) and shell material, and EVERY zone's walls moved to the world, heights
/// included, so a run gaskets only through the walls it passes at its own height. Without a ship,
/// the one level a legacy home had.
pub(crate) struct RunSite {
    levels: Vec<(f32, f32)>,
    origin: Vec3,
    /// The home zone's box: (width, height, depth), metres.
    size: (f32, f32, f32),
    slabs: Vec<crate::ship::home_structure::FloorSlab>,
    shell_mat: u32,
    walls: Vec<crate::ship::home_structure::InteriorWall>,
}

impl RunSite {
    /// The ceiling a run's hanger at `at` reaches up to (world metres): the ceiling really over it
    /// in the home (`lowest_ceiling_above`: the slab over it, else the roof), or `level_ceiling`,
    /// the ceiling of the run's level (`run_level`), outside the home's box. Increment 7 follow-ups,
    /// 2026-10-21: the levels are per storey of the whole home, so in the double-height greenhouse,
    /// which has no slab, every hanger on a ground run stopped at the 2.75 m of the slab elsewhere,
    /// in mid-air; it reaches the greenhouse's roof at 6 m now.
    pub(crate) fn hanger_top(&self, at: Vec3, level_ceiling: f32) -> f32 {
        let (w, h, d) = self.size;
        let o = self.origin;
        let in_home = at.x >= o.x && at.x <= o.x + w && at.z >= o.z && at.z <= o.z + d && at.y >= o.y && at.y <= o.y + h;
        if !in_home {
            return level_ceiling;
        }
        lowest_ceiling_above(&self.slabs, h, o, at)
    }

    /// The ceiling a run's END at `p` is kept under (`route_run`; the increment 7 follow-ups,
    /// 2026-10-22): the ceiling really over it from the floor it stands on, `foot` (world metres:
    /// the underside of the first slab over that floor there, else the roof), as a hanger reaches
    /// (`hanger_top`); outside the home's box, its level's ceiling, `level_ceiling`. Measured from
    /// the floor, not from the end, so an end that came to be over a slab (a port gizmo over a unit
    /// hung under it) is still held under that slab.
    pub(crate) fn end_ceiling(&self, p: Vec3, foot: f32, level_ceiling: f32) -> f32 {
        self.hanger_top(Vec3::new(p.x, foot, p.z), level_ceiling)
    }
}

/// The `RunSite` of `ship`.
pub(crate) fn run_site(ship: Option<&ShipStructure>) -> RunSite {
    let Some(ship) = ship else {
        return RunSite {
            levels: vec![(0.0, 3.0)],
            origin: Vec3::ZERO,
            size: (0.0, 0.0, 0.0),
            slabs: Vec::new(),
            shell_mat: 1,
            walls: Vec::new(),
        };
    };
    let home = &ship.zones[ship.home_zone_index()];
    let mut walls: Vec<crate::ship::home_structure::InteriorWall> = Vec::new();
    for z in &ship.zones {
        let (ox, oy, oz) = z.origin;
        walls.extend(z.body.walls.iter().cloned().map(|mut w| {
            w.a = (w.a.0 + ox, w.a.1 + oz);
            w.b = (w.b.0 + ox, w.b.1 + oz);
            w.base += oy;
            w
        }));
    }
    let o = home.origin_vec();
    RunSite {
        levels: service_levels(&home.body, o),
        origin: o,
        size: (home.body.width, home.body.height, home.body.depth),
        slabs: home.body.floors.clone(),
        shell_mat: home.body.shell_material,
        walls,
    }
}

/// How far under its storey's ceiling a run's end is kept (metres; `route_run`).
const END_UNDER_CEILING_M: f32 = 0.05;

/// THE ROUTE OF ONE RUN (`run`, at `site`): (its path and fittings, its service height, the
/// ceiling its hangers reach). It hangs on the lower end's level (`run_level`): up from each end to
/// the service height, across in Manhattan legs (`conduits::route_conduit`: never a diagonal
/// through a room), down to the other end. Ship homes increment 7 review, 2026-10-21:
/// - an end ABOVE the service height, a machine upstairs, is reached by a riser straight up from
///   under it, through the floor (sleeved, `conduits::add_slab_passthroughs`), so no run between
///   storeys crosses the storey above;
/// - an end is kept under the ceiling of the storey its machine stands on (`END_UNDER_CEILING_M`):
///   a port gizmo stood 0.3 m over its machine (`port_gizmo_pos`), which over a unit hung under
///   the ceiling (the air handlers, 2.5 m up) was in or over the floor above, and a run to it used
///   to rise through that floor and cross the upper storey. Since the 2026-10-21 follow-ups the
///   gizmos keep under the ceiling over their machine themselves (`gizmo_room`); this still holds
///   every end, a conduit node's too, under the ceiling over it.
/// - that ceiling is the one REALLY over the end from its floor (`RunSite::end_ceiling`: the
///   slab's underside, else the roof; the increment 7 follow-ups, 2026-10-22), not its storey's
///   service level: in the double-height greenhouse, open to its 6 m roof, the runs to the air
///   handlers g1 and g2 (hung 2.7 m up) ended at the level's 2.70 m, at the bottom of the unit,
///   short of its port; they rise to the port now, by a riser from the service height.
pub(crate) fn route_run(site: &RunSite, run: &ConduitRun) -> (crate::ship::conduits::ConduitRoute, f32, f32) {
    use crate::ship::conduits::{add_slab_passthroughs, route_conduit, ConduitKind};
    let kind = ConduitKind::for_resource(&run.kind);
    let (service_y, ceiling) = run_level(&site.levels, run.foot_a, run.foot_b);
    // Each end, kept under the ceiling over it from its floor.
    let keep = |p: Vec3, foot: f32| {
        let top = site.end_ceiling(p, foot, run_level(&site.levels, foot, foot).1) - END_UNDER_CEILING_M;
        if p.y > top { Vec3::new(p.x, top.max(foot), p.z) } else { p }
    };
    let (a, b) = (keep(run.a, run.foot_a), keep(run.b, run.foot_b));
    // The point under an end above the service height, where its riser leaves the run.
    let under = |p: Vec3| if p.y > service_y { Vec3::new(p.x, service_y, p.z) } else { p };
    let (ua, ub) = (under(a), under(b));
    let mut route = route_conduit(ua, ub, kind, service_y, site.shell_mat, &site.walls);
    if ua != a {
        route.points.insert(0, a);
    }
    if ub != b {
        route.points.push(b);
    }
    add_slab_passthroughs(&mut route, &site.slabs, site.origin);
    (route, service_y, ceiling)
}

/// The parts of a run's path `points` the build editor's cut-away draws (`cut`, ship homes
/// increment 7 review): what stands over the cut, in the cut zone (`CutAway::hides`), is left out
/// with the machines up there, and a leg passing out of that (a riser through the cut) is cut where
/// it does. Each part is a path of its own; with no cut, the whole path.
pub(crate) fn parts_under_cut(points: &[Vec3], cut: impl Into<crate::engine::editor::Cuts>) -> Vec<Vec<Vec3>> {
    let c = cut.into();
    if c.is_empty() {
        return vec![points.to_vec()];
    }
    // Where along p->q the drawn and the hidden meet (p and q on either side): halved to a
    // millimetre on a metre-long leg, which is all the eye can see.
    let edge = |p: Vec3, q: Vec3| {
        let (mut shown, mut hidden) = if c.hides(p) { (q, p) } else { (p, q) };
        for _ in 0..24 {
            let m = (shown + hidden) * 0.5;
            if c.hides(m) {
                hidden = m;
            } else {
                shown = m;
            }
        }
        shown
    };
    let mut parts = Vec::new();
    let mut part: Vec<Vec3> = Vec::new();
    for w in points.windows(2) {
        let (p, q) = (w[0], w[1]);
        match (c.hides(p), c.hides(q)) {
            (false, false) => {
                if part.is_empty() {
                    part.push(p);
                }
                part.push(q);
            }
            (false, true) => {
                if part.is_empty() {
                    part.push(p);
                }
                part.push(edge(p, q));
                parts.push(std::mem::take(&mut part));
            }
            (true, false) => part = vec![edge(p, q), q],
            (true, true) => {}
        }
    }
    if part.len() >= 2 {
        parts.push(part);
    }
    parts.retain(|p| p.len() >= 2);
    parts
}

/// Every machine's port gizmos (v0.625): (machine id, port index, the port, its world position,
/// `port_gizmo_pos`, kept inside the machine's home and storey aboard `ship`, `gizmo_room`), for
/// the viewport's drag-to-connect and the runs' ends (`conduit_runs`). Keyed by id (placements
/// may skip a machine whose catalog entry is missing), the type looked up by id.
pub(crate) fn port_picks(
    home: &crate::machines::MachineHome,
    placements: &[crate::machines::PlacedMachine],
    ship: Option<&ShipStructure>,
) -> Vec<(String, usize, crate::utilities::Port, Vec3)> {
    let type_by_id: std::collections::HashMap<String, String> = home.all_instances().into_iter().map(|i| (i.id, i.machine)).collect();
    let mut pp = Vec::new();
    for p in placements {
        let Some(ty) = type_by_id.get(&p.id) else { continue };
        let Some(def) = home.catalog.get(ty) else { continue };
        let ports = def.derive_ports();
        let n = ports.len();
        let room = gizmo_room(ship, p);
        for (i, port) in ports.into_iter().enumerate() {
            pp.push((p.id.clone(), i, port, port_gizmo_pos(p, i, n, room)));
        }
    }
    pp
}

/// The zone of `ship` whose box holds `p` (world metres): its origin and its body. A home put away
/// holds nothing aboard.
fn zone_holding(ship: &ShipStructure, p: Vec3) -> Option<(Vec3, &crate::ship::home_structure::HomeStructure)> {
    ship.zones.iter().enumerate().filter(|(zi, _)| !ship.is_away_home(*zi)).find_map(|(_, z)| {
        let (o, b) = (z.origin_vec(), &z.body);
        let inside = p.x >= o.x
            && p.x <= o.x + b.width
            && p.z >= o.z
            && p.z <= o.z + b.depth
            && p.y >= o.y - crate::ship::home_structure::STOREY_EPS
            && p.y <= o.y + b.height.max(1.0);
        inside.then_some((o, b))
    })
}

/// The floor of the storey `p` stands on (world metres, ship homes increment 7): in the zone whose
/// box holds it, the highest of that zone's storey floors at or under it. `fallback` (the
/// placement's own floor, its zone's) outside every zone. A machine's pipes leave from here.
pub(crate) fn storey_floor_at(ship: Option<&ShipStructure>, p: Vec3, fallback: f32) -> f32 {
    let Some((o, body)) = ship.and_then(|s| zone_holding(s, p)) else {
        return fallback;
    };
    let storeys = body.storeys();
    o.y + storeys[crate::ship::home_structure::storey_index(&storeys, p.y - o.y)].floor
}

/// The ceiling over a room centred at `center` whose box tops out at `top` (world metres, ship homes
/// increment 7): the underside of a floor slab of its zone over the centre whose top is that height
/// (a room under a floor ends at the floor's top, `detect_rooms`, but its ceiling is the floor's
/// underside), else `top`.
pub(crate) fn ceiling_over(ship: &ShipStructure, center: Vec3, top: f32) -> f32 {
    let Some((o, body)) = zone_holding(ship, center) else {
        return top;
    };
    body.floors
        .iter()
        .filter(|s| (o.y + s.top - top).abs() < crate::ship::home_structure::STOREY_EPS && s.contains(center.x - o.x, center.z - o.z))
        .map(|s| o.y + s.underside())
        .fold(top, f32::min)
}

/// Rebuild the home connection cylinders from the live machine layout (gui_state.home_machines
/// + room_bounds): one routed run per connection, between the two machines' pipe anchors. Uses
/// a cached unit cylinder + a material cached per material, so a per-frame rebuild never leaks.
/// Replaces the old static routed pipes -- connections now follow rooms. (v0.530)
///
/// Since 2026-10-04 a run's BODY is drawn in its real material (copper, rubber hose, a cord's
/// jacket; data/piping/pipe_materials.ron) and what it carries is said by MARKER BANDS from the
/// ship's marking scheme (ISO 14726; data/piping/marking_schemes.ron), at each end, past each
/// bend and every 6 m (engine::pipe_markers). The utility colour no longer paints the run.
pub(crate) fn rebuild_connection_objects(state: &mut EngineState) {
    use std::collections::HashMap;
    state.connection_objects.clear();
    state.connection_flow_paths.clear();
    // Record the marking mode this build uses FIRST, before any early return, so a home with no
    // pipes is not rebuilt every frame by `pipe_markers::rebuild_if_mode_changed`.
    let marking_mode = crate::engine::pipe_markers::wanted_mode(state);
    state.pipe_markers.built_mode = Some(marking_mode);
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    if rooms.is_empty() {
        return;
    }
    // v0.538: box-mode absolute positioning when a ship home is active (mirrors
    // rebuild_machine_objects so the conduit anchors match the machine meshes). v0.754:
    // per-zone footprints; conduit NODES clamp into the whole ship's AABB (they carry no zone).
    // Every run of the layout, and what each CARRIES, for its marker bands (2026-10-04 review):
    // what the machine it leaves puts out (water leaving the purifier is potable water, leaving an
    // air handler condensate), else its connection kind, whose marker names only the group.
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    let (runs, contents): (Vec<ConduitRun>, Vec<String>) = match &state.gui_state.home_machines {
        Some(h) => {
            let placements = h.placements(&rooms, zone_rects.as_deref());
            let runs = conduit_runs(state.gui_state.ship_structure.as_ref(), h, &placements, &state.port_pick);
            let types = h.instance_types();
            let contents = runs.iter().map(|r| h.line_content(&types, &r.from, &r.kind).to_string()).collect();
            (runs, contents)
        }
        None => return,
    };

    // Rainbow emissive materials for the selected line's flow markers (v0.623), created once.
    // Moderate emissive (1.4) + some roughness so the little beads still READ AS SPHERES (a
    // gradient across the curve) instead of flat-bright discs -- the v0.622 markers were emissive
    // 3.0, which washed out the shading and looked inside-out ("inverted normals", operator).
    // They also WERE inside out, then and until 2026-10-03: `Mesh::sphere` was wound so the
    // opaque pipeline drew each bead's far inside (BUG-128). The emissive change stays (it is a
    // brightness choice, not a flip); the winding is fixed in the mesh itself.
    if state.flow_rgb_mats.is_empty() {
        for k in 0..16u32 {
            let h = k as f32 / 16.0;
            let (r, g, b) = hsv_to_rgb(h, 0.85, 1.0);
            let m = state.renderer.add_material_full([r, g, b, 1.0], 0.0, 0.55, 0.0, 1.4);
            state.flow_rgb_mats.push(m);
        }
    }
    if runs.is_empty() {
        return;
    }
    // Cached unit cylinder mesh (+Y, base at origin, radius 0.05, height 1) -- reused for every
    // conduit segment + fitting, scaled/rotated, so a rebuild never leaks.
    let cyl = match state.connection_cyl {
        Some(m) => m,
        None => {
            let m = state
                .renderer
                .add_mesh(Mesh::cylinder(&state.renderer.device, 0.05, 1.0, 8));
            state.connection_cyl = Some(m);
            m
        }
    };
    // Home geometry for routing (v0.536): run conduits UP to a service height near the ceiling
    // and ACROSS in Manhattan legs (never a straight diagonal through the room -- the operator's
    // "the straight lines that pass through everything is wrong"), placing material-aware
    // passthroughs where a run crosses an interior wall. v0.754 (multi-zone): the service
    // height + shell material come from the HOME zone (one shared service level), and the
    // wall-passthrough list is EVERY zone's walls shifted to WORLD coords so a run through any
    // zone gaskets where it crosses that zone's walls (`run_site`).
    // Ship homes increment 7: the service level is per STOREY of the home zone (`service_levels`,
    // `run_level`): a run hangs under the ceiling of the lower storey its ends stand on, the
    // underside of the floor above, and reaches a machine upstairs by a riser through that floor
    // under it (sleeved; `route_run`). With no slab there is the one level there always was.
    let site = run_site(state.gui_state.ship_structure.as_ref());
    // The build editor's cut-away (`HomeDraw::cut`, set before this rebuild): the parts of a run
    // over its cut are left out with the machines up there (`parts_under_cut`).
    let cut = state.home_draw.cut.clone();
    const CYL_R: f32 = 0.05; // the unit cylinder's modeled radius
    // Dedup the support fittings ACROSS all connections (v0.626): many pipes share the same service-
    // height run, so without this their ceiling hangers + wall gaskets stack at the SAME spot --
    // invisible overlap that still costs polygons (the operator's "brackets overlap, more polys than
    // we should"). Key by rounded position so one bracket serves all pipes passing that point.
    let mut placed_fittings: HashMap<(i32, i32, i32), ()> = HashMap::new();
    // The marker bands of every run, gathered per colour and uploaded once after the loop.
    let mut bands = crate::engine::pipe_markers::BandBatch::default();
    for (run, content) in runs.iter().zip(&contents) {
        let (from_id, to_id, kind_str) = (&run.from, &run.to, &run.kind);
        let kind = crate::ship::conduits::ConduitKind::for_resource(kind_str);
        let (route, service_y, ceiling) = route_run(&site, run);
        let parts = parts_under_cut(&route.points, cut.clone());
        // Stash the routed path + the from/to ids, so the SELECTED machine's connections animate
        // their flow markers (v0.623) while every other pipe stays a quiet static line.
        for part in parts.iter().filter(|p| p.len() >= 2) {
            if state.connection_flow_sphere.is_none() {
                let s = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 0.10, 10, 10));
                state.connection_flow_sphere = Some(s);
            }
            state.connection_flow_paths.push((part.clone(), from_id.clone(), to_id.clone()));
        }

        // Pipe BODY: its real material (2026-10-04, data/piping/pipe_materials.ron): copper tube,
        // rubber hose, a cord's or a data cable's jacket. No marking standard colours the wall
        // material (findings F28); what the run carries is said by its marker bands below. Until
        // then (v0.623) the whole run was painted, faintly glowing, in its utility colour.
        // `body_look` is the one place that choice is made, so its test checks what is drawn.
        let body = crate::ship::pipe_materials::pipe_materials().body_look(kind_str);
        let pipe_mat = match state.connection_mats.get(&body.key) {
            Some(&m) => m,
            None => {
                let m = state.renderer.add_material_full(body.linear, body.metallic, body.roughness, 0.0, 0.0);
                state.connection_mats.insert(body.key.clone(), m);
                m
            }
        };
        // The run's marker bands: generated from what it carries, never typed by anyone.
        for part in &parts {
            bands.add_run(part, content, kind.radius(), marking_mode);
        }
        let rscale = kind.radius() / CYL_R;
        // The routed pipe: one cylinder per leg (up, across, across, down).
        for seg in parts.iter().flat_map(|part| part.windows(2)) {
            let (p, q) = (seg[0], seg[1]);
            let diff = q - p;
            let len = diff.length();
            if len < 1e-4 {
                continue;
            }
            let rot = Quat::from_rotation_arc(Vec3::Y, diff / len);
            state
                .connection_objects
                .push((cyl, pipe_mat, p, rot, Vec3::new(rscale, len, rscale)));
        }
        // Procedural support structures: a ceiling hanger at each service-height bracket + a
        // material-aware gasket collar at each wall passthrough. The fitting colour comes from the
        // material it attaches to, so a steel vs wood wall reads differently.
        for f in route.fittings.iter().filter(|f| !cut.hides(f.at)) {
            // Skip a fitting whose spot is already bracketed by another pipe's run (v0.626 dedup).
            let pos_key = ((f.at.x * 5.0) as i32, (f.at.y * 5.0) as i32, (f.at.z * 5.0) as i32);
            if placed_fittings.insert(pos_key, ()).is_some() {
                continue;
            }
            let fkey = format!("fitting:{}", f.material);
            let fmat = match state.connection_mats.get(&fkey) {
                Some(&m) => m,
                None => {
                    let col = match f.material {
                        1 => [0.58, 0.60, 0.65, 1.0], // steel
                        2 => [0.64, 0.64, 0.62, 1.0], // concrete
                        3 => [0.52, 0.37, 0.22, 1.0], // wood
                        _ => [0.50, 0.52, 0.56, 1.0],
                    };
                    let m = state.renderer.add_material_typed(col, 0.6, 0.4, f.material as f32);
                    state.connection_mats.insert(fkey.clone(), m);
                    m
                }
            };
            match f.kind {
                crate::ship::conduits::FittingKind::Bracket => {
                    // Ceiling hanger (a thin post up to the ceiling) for the horizontal service
                    // runs, which hang at the service height; the vertical drops and risers are
                    // held at their ends, so skip them (a riser to a machine upstairs climbs past
                    // the service height, and a bracket on it is no hanger's). Up to the ceiling
                    // really over it (`RunSite::hanger_top`): the greenhouse's roof at 6 m.
                    if (f.at.y - service_y).abs() <= 0.1 {
                        let drop = (site.hanger_top(f.at, ceiling) - f.at.y).max(0.05);
                        state.connection_objects.push((
                            cyl,
                            fmat,
                            f.at,
                            Quat::IDENTITY,
                            Vec3::new(0.5, drop, 0.5),
                        ));
                    }
                }
                crate::ship::conduits::FittingKind::Passthrough => {
                    // A short gasket collar straddling the wall at the crossing.
                    state.connection_objects.push((
                        cyl,
                        fmat,
                        f.at - Vec3::new(0.0, 0.12, 0.0),
                        Quat::IDENTITY,
                        Vec3::new(2.4, 0.24, 2.4),
                    ));
                }
                crate::ship::conduits::FittingKind::Elbow => {}
            }
        }
    }
    // One merged mesh per band colour, each in its own slot (replaced in place next time).
    bands.flush(state);
}

/// Recompute the door/window panel placements from the live HomeStructure (v0.537). Called after
/// a structure rebuild + on load. Preserves the per-panel open fraction when the panel COUNT is
/// unchanged (so editing a far wall does not slam every door shut); otherwise resets to closed.
/// Corridor-mouth door pairs (v0.795) are part of the same list -- `ship_panel_placements`
/// appends them -- so a corridor add/move/remove in the editor re-derives its doors right here,
/// and their open fractions live in the same `door_panels` Vec as every other door's.
pub(crate) fn rebuild_door_panels(state: &mut EngineState) {
    // v0.754: every zone's doors, at world positions (per-zone origin offsets).
    let placements = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::door_panels::ship_panel_placements(ship),
        None => Vec::new(),
    };
    if placements.len() == state.door_panels.len() {
        for (i, p) in placements.into_iter().enumerate() {
            state.door_panels[i].0 = p;
        }
    } else {
        state.door_panels = placements.into_iter().map(|p| (p, 0.0)).collect();
    }
    // Reset every manual door to CLOSED on a structural rebuild (v0.567). This runs only on a
    // structure edit / world load (build mode, orbit cam), never while walking, so we deliberately
    // do NOT trust positional parallelism across an edit -- an open-flag must never land on the
    // wrong door just because the opening count happened to stay equal.
    state.door_manual_open = vec![false; state.door_panels.len()];
    // Reset live lock state to each door's AUTHORED states on a rebuild (v0.570), parallel to
    // door_panels. Same reasoning as the manual-open reset above.
    state.door_locks = state
        .door_panels
        .iter()
        .map(|(p, _)| p.locks.iter().map(|l| l.state).collect())
        .collect();
    note_landing_doors(state);
}

/// Mark which doors stand at an elevator's landing (`HomeDraw::landing_doors`), from the doors and
/// elevators the ship has now: `footing::lift_landing_open` answers Some for those whatever the car
/// does. Called wherever the doors are placed, so the frame asks the elevators about these only.
fn note_landing_doors(state: &mut EngineState) {
    let no_cars = crate::engine::footing::ElevatorCars::new();
    state.home_draw.landing_doors = match state.gui_state.ship_structure.as_ref() {
        Some(ship) => state
            .door_panels
            .iter()
            .map(|(p, _)| crate::engine::footing::lift_landing_open(ship, &no_cars, p.center).is_some())
            .collect(),
        None => Vec::new(),
    };
}

/// Where door `p` is heading this frame, 0 shut to 1 open (`open` is where it is now). A window,
/// a door styled "fixed" or a locked door stays shut. An elevator's LANDING door whose car is not
/// at its landing (`landing` Some(false), `footing::lift_landing_open`; ship homes increment 7
/// review) stays shut, so it never opens onto an empty shaft: the shaft is gated whatever the door
/// does (`footing::elevator_gates`), and before this the upper lobby's door (U11) slid open at a
/// player's approach with the car at the bottom. Otherwise a manual door opens while toggled at its
/// panel (`manual_open`), and an auto door by its nearest actor's distance `dist`
/// (`door_anim::auto_open_target`). A door at no landing (`landing` None) or at one with the car
/// there opens as it always has.
pub(crate) fn door_target(
    p: &crate::ship::door_panels::PanelPlacement,
    open: f32,
    locked_now: bool,
    manual_open: bool,
    landing: Option<bool>,
    dist: f32,
) -> f32 {
    let operable = !p.is_window && crate::systems::door_anim::is_operable(&p.style);
    if !operable || locked_now || landing == Some(false) {
        0.0
    } else if !p.auto_open {
        // A MANUAL door (v0.564) opens only when toggled at its control panel (v0.567).
        if manual_open { 1.0 } else { 0.0 }
    } else {
        // Proximity + hysteresis (v0.540, pure + tested in door_anim since v0.795).
        crate::systems::door_anim::auto_open_target(dist, p.open_dist, open)
    }
}

/// THE ELEVATOR CARS' running clearance (metres a side, ship homes increment 7 review): each car is
/// drawn this much inside its footprint, which is what the landing gates close round and what a
/// rider stands on (`footing::elevator_gates`, `Footing`). It keeps the car's ends off the shaft
/// frame's posts, whose outer faces are the footprint's edge, so the two never share a face and
/// flicker. 2 cm, about the gap a real car keeps from a landing's sill. The car used to be drawn at
/// 0.85 of its footprint, a 13 cm gap a side round a floor that is solid to its edge.
pub(crate) const CAR_CLEARANCE_M: f32 = 0.02;
/// The elevator car's floor slab's thickness (metres); its top is the floor a rider stands on.
const CAR_THICK_M: f32 = 0.15;

/// Every elevator car to draw (v0.590; moved here from lib.rs in the increment 7 review): (where,
/// its turn, its scale for the unit box), one per elevator of the ship (`ship::lifts::lifts`, the same
/// list the cars ride on and the gates stand round), its floor's top at the car's height (`cars`),
/// the slab its footprint less `CAR_CLEARANCE_M` a side. A car the editor's cut-away hides (`cut`:
/// at or over the cut, standing on the floor above) is left out.
pub(crate) fn elevator_car_poses(
    ship: &ShipStructure,
    cars: &crate::engine::footing::ElevatorCars,
    cut: impl Into<crate::engine::editor::Cuts>,
) -> Vec<(Vec3, Quat, Vec3)> {
    let cut = cut.into();
    crate::ship::lifts::lifts(ship)
        .into_iter()
        .filter_map(|lift| {
            let top = lift.base.y + cars.get(&lift.key).map_or(0.0, |e| e.0);
            if cut.hides(Vec3::new(lift.base.x, top, lift.base.z)) {
                return None;
            }
            let (w, _, d) = lift.ty.size;
            // box_xyz is y-bottom-origin + XZ-centred: the slab's TOP sits at the car's floor.
            let pos = Vec3::new(lift.base.x, top - CAR_THICK_M, lift.base.z);
            let scale = Vec3::new(w - 2.0 * CAR_CLEARANCE_M, CAR_THICK_M, d - 2.0 * CAR_CLEARANCE_M);
            Some((pos, Quat::from_rotation_y(lift.yaw), scale))
        })
        .collect()
}

/// Push this frame's home machines and their screens into `out` (the render loop, lib.rs), but
/// those the build editor's cut-away leaves out (`HomeDraw::cut`, ship homes increment 7 review):
/// each by where it stands, its placement (a sphere's centre), which its screen's quad shares
/// (`screens::Screens::push_render_objects`). Before this the upper floor's furniture and screens
/// drew in the air over the cut.
pub(crate) fn push_home_machines(state: &EngineState, out: &mut Vec<RenderObject>) {
    let cut = &state.home_draw.cut;
    let shown = |at: Vec3| !cut.hides(at);
    for &(mesh, material, pos, yaw) in &state.machine_objects {
        if shown(pos) {
            out.push(RenderObject {
                fade: 0.0,
                position: pos,
                rotation: Quat::from_rotation_y(yaw.to_radians()), // v0.633 machine yaw
                scale: Vec3::ONE,
                mesh,
                material,
            });
        }
    }
    // In-world screens (rung 2): the display quad of every placed machine with a `screen` def,
    // drawn with the body's own position + yaw. Hidden with the machines, since each sits on one.
    let first = out.len();
    state.screens.push_render_objects(out);
    if !cut.is_empty() {
        let screens: Vec<RenderObject> = out.drain(first..).filter(|o| shown(o.position)).collect();
        out.extend(screens);
    }
}

/// Is door `panel` currently locked, using its LIVE lock states when present (v0.570)? A door with
/// locks is locked iff any live lock is not open; an empty lock list falls back to the legacy
/// `panel.locked` bool, so v0.567 doors are unchanged. `live` is `door_locks[i]`.
pub(crate) fn door_locked_now(
    panel: &crate::ship::door_panels::PanelPlacement,
    live: Option<&Vec<crate::ship::lock_types::LockState>>,
) -> bool {
    if panel.locks.is_empty() {
        return panel.locked;
    }
    match live {
        Some(states) if states.len() == panel.locks.len() => states.iter().any(|s| !s.is_open()),
        _ => panel.locks.iter().any(|l| !l.state.is_open()), // fall back to authored
    }
}

/// The room point-lights to upload (v0.571, refined v0.572). A home's PLACED lights (resolved from
/// light_types.ron + per-instance overrides) take over once ANY are placed; otherwise the crude
/// `auto` one-per-room fill is used, and ONLY when GI is on. Rationale (operator v0.572 feedback):
/// the auto fill is a single bright point light at room centre that reads as an ugly "sun spotlight"
/// pool -- so once the operator places their own lights, we drop it entirely (their lights ARE the
/// room lighting; the directional SUN, gated separately by GI, still provides the even base when GI
/// is on). With NO placed lights the old behaviour is unchanged (auto fill when GI on, dark when off).
///
/// OPEN PLOTS (ship homes increment 5b): when `ship.open.lit` names an open plot drawn as a
/// neighbour's, that plot's design lights are added at it (`open_plot_lights`), whatever else is
/// lit. They light the neighbour's plot only, so they never decide whether this ship's own rooms
/// get the auto fill; and they are no zone's lights, so no meter counts them
/// (`ShipStructure::lighting_watts`, the house lighting consumer in lib.rs).
pub(crate) fn home_lights(
    ship: Option<&ShipStructure>,
    auto: Vec<crate::renderer::light::RoomLight>,
    gi_on: bool,
) -> Vec<crate::renderer::light::RoomLight> {
    use crate::renderer::light::RoomLight;
    // v0.754: EVERY zone's placed lights, each offset by its zone's world origin; none of a home
    // put away (ship homes increment 2, `ShipStructure::put_home_away`): it lights nothing aboard.
    let placed: Vec<RoomLight> = ship
        .map(|s| {
            s.zones
                .iter()
                .enumerate()
                .filter(|(zi, _)| !s.is_away_home(*zi))
                .flat_map(|(_, z)| placed_room_lights(&z.body.lights, z.origin_vec()))
                .collect()
        })
        .unwrap_or_default();
    // Any placed lights -> the home is manually lit, no auto centre-spot. Else auto fill if GI on.
    let mut lights = if !placed.is_empty() {
        placed
    } else if gi_on {
        auto
    } else {
        Vec::new()
    };
    if let Some(s) = ship {
        lights.extend(open_plot_lights(s));
    }
    lights
}

/// The lit open plot's own design lights, at its plot (ship homes increment 5b, `home_lights`):
/// none unless `ship.open.lit` names a plot that is in `ship.open.plots` and drawn as a
/// neighbour's (`NeighbourView::design_lights_of`, the shipped design of its kind, about 35
/// lights for the homestead). The neighbours' view is asked for only when a plot is lit.
pub(crate) fn open_plot_lights(ship: &ShipStructure) -> Vec<crate::renderer::light::RoomLight> {
    let Some(lit) = ship.open.lit.as_deref().filter(|p| ship.open.plots.contains(*p)) else {
        return Vec::new();
    };
    let view = crate::ship::neighbours::neighbour_view(ship);
    match view.design_lights_of(lit) {
        Some((lights, origin)) => placed_room_lights(lights, origin),
        None => Vec::new(),
    }
}

/// Every switched-on light of `lights` (placed in a body's own metres) as the lights the renderer
/// draws, offset by `o`, the body's origin: a point, a spot, or a strip's line segments, each
/// resolved through light_types.ron with its per-instance overrides. A light of an unknown type
/// is left out. `home_lights` for each zone, `open_plot_lights` for an open plot's design.
fn placed_room_lights(lights: &[crate::ship::home_structure::PlacedLight], o: Vec3) -> Vec<crate::renderer::light::RoomLight> {
    use crate::renderer::light::{LightKind, RoomLight};
    lights.iter().filter(|l| l.on).filter_map(move |l| {
        let t = crate::renderer::light::light_type(&l.type_id)?;
        let c = l.color.unwrap_or(t.color);
        let pos = Vec3::new(l.pos.0, l.pos.1, l.pos.2) + o;
        let color = [c.0, c.1, c.2];
        let intensity = l.intensity.unwrap_or(t.intensity);
        let range = l.range.unwrap_or(t.range);
        Some(match t.kind {
            LightKind::Spot => {
                let dir = Vec3::new(l.dir.0, l.dir.1, l.dir.2);
                vec![RoomLight::spot(pos, color, intensity, range, dir, t.cone_inner_deg, t.cone_outer_deg)]
            }
            // A strip is a LINE LIGHT (v0.786, operator: "make the
            // full length of the bar emit light onto surfaces"):
            // one segment light per sampled leg, each lit from its
            // closest point in the shader. Intensity is split
            // across segments by length so the strip's total
            // output matches its dial (energy conservation).
            // Emission follows the SAME subdivided curve as the
            // rendered tube (v0.792) -- it used to follow the
            // straight CONTROL polyline, so a curved strip's
            // rounded sections glowed without lighting anything
            // (operator screenshot). strip_emission_segments caps
            // the per-strip segment count so a 100-subdivision
            // strip can't flood the (uncapped, v0.782) light
            // buffer with useless segments.
            LightKind::Bar => {
                let (pts, sub): (Vec<Vec3>, u32) = if l.path.is_empty() {
                    // Pathless: the classic straight bar of the
                    // type's length along the horizontal dir. A
                    // straight segment has nothing to subdivide.
                    let flat = Vec3::new(l.dir.0, 0.0, l.dir.2);
                    let axis = if flat.length_squared() > 1e-4 {
                        flat.normalize()
                    } else {
                        Vec3::X
                    };
                    let half = t.length_m.max(0.3) * 0.5;
                    (vec![pos - axis * half, pos + axis * half], 0)
                } else {
                    (
                        std::iter::once(pos)
                            .chain(l.path.iter().map(|p| Vec3::new(p.0, p.1, p.2) + o))
                            .collect(),
                        l.subdivision,
                    )
                };
                crate::renderer::light::strip_emission_segments(&pts, sub)
                    .into_iter()
                    .map(|(a, b, share)| {
                        RoomLight::line(a, b, color, intensity * share, range)
                    })
                    .collect()
            }
            _ => vec![RoomLight::point(pos, color, intensity, range)],
        })
    })
    .flatten()
    .collect()
}

/// Per-frame: animate + emit the door/window panels (v0.537). A door eases open as an actor
/// approaches (v0.795: the nearest of local player / remote players / creatures -- see the
/// actor gather below) by its data-driven style via systems::door_anim; a window is a fixed
/// glass pane. Corridor-mouth door pairs (v0.795) are ordinary entries in the same list, so
/// they animate + collide with zero extra machinery here. Reuses one cached unit-box mesh + a
/// slab + a glass material (scaled/rotated/animated per frame), so nothing leaks. Doors go to
/// the opaque pass, glass to the transparent pass.
pub(crate) fn render_door_panels(
    state: &mut EngineState,
    opaque: &mut Vec<RenderObject>,
    transparent: &mut Vec<RenderObject>,
    ring_lines: &mut Vec<crate::renderer::line::LineVertex>,
    dt: f32,
) {
    if state.door_panels.is_empty() {
        return;
    }
    let mesh = match state.door_panel_mesh {
        Some(m) => m,
        None => {
            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            state.door_panel_mesh = Some(m);
            m
        }
    };
    let slab_mat = match state.door_slab_mat {
        Some(m) => m,
        None => {
            // theme-exempt: world-object material, not a themed UI surface.
            let m = state.renderer.add_material_typed([0.36, 0.38, 0.43, 1.0], 0.3, 0.5, 1.0);
            state.door_slab_mat = Some(m);
            m
        }
    };
    let glass_mat = match state.door_glass_mat {
        Some(m) => m,
        None => {
            // theme-exempt: tinted glass, transparent pass.
            let m = state.renderer.add_material_full([0.55, 0.78, 0.92, 0.34], 0.0, 0.08, 1.0, 0.10);
            state.door_glass_mat = Some(m);
            m
        }
    };
    // Energy + nanowall door materials (v0.554), all rendered in the transparent pass: an ENERGY
    // door is a glowing FIELD -- green while operable, red while LOCKED; a NANOWALL is a metallic
    // semi-transparent surface you see through as it dissolves open.
    let energy_open_mat = match state.door_energy_open_mat {
        Some(m) => m,
        None => {
            // theme-exempt: glowing green energy field.
            let m = state.renderer.add_material_full([0.20, 1.0, 0.40, 0.42], 0.0, 0.3, 1.0, 1.4);
            state.door_energy_open_mat = Some(m);
            m
        }
    };
    let energy_locked_mat = match state.door_energy_locked_mat {
        Some(m) => m,
        None => {
            // theme-exempt: glowing red energy field (locked).
            let m = state.renderer.add_material_full([1.0, 0.18, 0.20, 0.50], 0.0, 0.3, 1.0, 1.4);
            state.door_energy_locked_mat = Some(m);
            m
        }
    };
    let nanowall_mat = match state.door_nanowall_mat {
        Some(m) => m,
        None => {
            // theme-exempt: metallic gray nanowall, semi-transparent.
            let m = state.renderer.add_material_full([0.62, 0.64, 0.70, 0.60], 0.85, 0.15, 1.0, 0.15);
            state.door_nanowall_mat = Some(m);
            m
        }
    };
    // Nanowall shimmer (v0.554): drift the metallic gray + emissive over time so the surface reads
    // as a live, shifting "water" field rather than a static slab. One shared-material write/frame.
    state.door_anim_time += dt.max(0.0);
    let shimmer = 0.5 + 0.5 * (state.door_anim_time * 1.6).sin();
    let g = 0.58 + 0.10 * shimmer;
    state.renderer.update_material_full(nanowall_mat, [g * 0.94, g, g * 1.06, 0.60], 0.85, 0.10 + 0.08 * shimmer, 1.0, 0.08 + 0.16 * shimmer);
    // Auto-doors open for ANY nearby actor, not just the local player (v0.795, with the
    // corridor doors): a REMOTE player must be able to walk through a corridor mouth on your
    // screen (you SEE the door part for them, and its collider clears for you both), and a
    // wandering animal should not phase through a shut door's visual. Gathered once per frame:
    // the camera (the local player) + every RemotePlayer + every Creature transform. The
    // per-door check below takes the NEAREST actor, so one loiterer holds the door open.
    // v0.1005 (operator: "I was hearing doors open as my character moved
    // around the home despite my camera being in some far off distance"):
    // while the player is AWAY (surface-flying a planet, FTL), the local
    // camera coordinates are frozen at their last home-frame values - a
    // stale point usually INSIDE the house - so the "camera" kept counting
    // as a door actor and kept passing the earshot gate below. The camera
    // is only a door actor, and door sounds only reach the ears, while
    // actually aboard (the same gate the wall-collision system uses).
    // Kept apart by kind (ship homes increment 2 review, finding 8): a neighbour's corridor door
    // opens for the other players only (`door_panels::door_actor_distance`).
    // Each actor by its FEET (ship homes increment 7: a door opens only for a body on its storey,
    // `door_panels::door_actor_distance`): the camera and the other players are eyes (the camera's
    // own eye height; another player's position is the eye their game sends), an animal's position
    // is where it stands.
    let camera_actor = state.aboard_station.then_some(state.camera.position - Vec3::Y * state.controller.eye_height());
    let remote_eye = crate::ecs::components::EYE_HEIGHT_M as f32;
    let others: Vec<Vec3> = state
        .game_world
        .world
        .query::<(&crate::ecs::components::Transform, &crate::net::sync::RemotePlayer)>()
        .iter()
        .map(|(_e, (t, _))| t.position - Vec3::Y * remote_eye)
        .collect();
    let animals: Vec<Vec3> = state
        .game_world
        .world
        .query::<(&crate::ecs::components::Transform, &crate::ecs::components::Creature)>()
        .iter()
        .map(|(_e, (t, _))| t.position)
        .collect();
    // v0.547: per-door open distance. The interaction ring shows it in build mode / dev overlay.
    // The ring is a constant-width LINE circle now (v0.568), so there is no polygon-ring mesh.
    let show_widgets = state.gui_state.construction_active || state.gui_state.construction_dev_overlay;
    // Snapshot the per-door manual-open flags (v0.567) so the loop can read them while it holds a
    // &mut on door_panels (a disjoint-field borrow the checker won't always see through).
    let manual = state.door_manual_open.clone();
    // Local SFX buffer: the panel loop holds &mut door_panels, so edge sounds
    // collect here and land on state.pending_sfx after the borrow ends.
    let mut sfx: Vec<(&'static str, &'static str)> = Vec::new();
    let state_cam = state.camera.position;
    let locks_live = state.door_locks.clone();
    // The elevators' landing doors (ship homes increment 7 review): one at a landing opens only
    // while the car is there (`door_target`). Only the doors marked when they were placed ask.
    let ship = state.gui_state.ship_structure.as_ref();
    let cars = &state.elevator_state;
    let landing_doors = &state.home_draw.landing_doors;
    // The editor's cut-away leaves out the doors over its cut (they still open and close: only
    // their drawing goes), with their rings, panels and locks.
    let cut = state.home_draw.cut.clone();
    for (di, (p, open)) in state.door_panels.iter_mut().enumerate() {
        // An operable DOOR opens on approach; a window or a "fixed"-styled opening stays shut
        // (v0.538: consult door_anim::is_operable so a door explicitly styled "fixed" does not
        // chase an open target it can never animate to).
        let operable = !p.is_window && crate::systems::door_anim::is_operable(&p.style);
        // Is the door LOCKED right now (v0.570)? Live lock states if present, else the legacy bool.
        let locked_now = door_locked_now(p, locks_live.get(di));
        let drawn = !cut.hides(p.center);
        // Interaction-distance ring on the floor at the door (v0.547), drawn as a LINE circle
        // (v0.565, operator's idea -- like the orbit paths) so its width is CONSTANT regardless of
        // radius, instead of a polygon strip that thickened as open_dist grew.
        if drawn && show_widgets && operable && p.auto_open {
            const RING_COL: [f32; 4] = [0.35, 0.85, 1.0, 0.9]; // cyan
            // On the door's own floor (ship homes increment 7: an upstairs door's ring upstairs).
            crate::renderer::line::push_circle(
                ring_lines, [p.center.x, p.center.y + 0.04, p.center.z], p.open_dist, RING_COL, 72,
            );
        }
        // Wall-mounted CONTROL PANEL beside a manual/controlled door (v0.567): a glowing tech panel
        // the player walks up to and presses E. Green while openable, red while LOCKED. Routed to the
        // transparent pass since it glows. Drawn before the door's hidden-check so it always shows.
        // Only on a MANUAL door -- an auto door opens by itself, so its panel would be a dead control.
        if drawn && p.control_panel && !p.auto_open {
            let cp = p.control_panel_pos;
            let mat = if locked_now { energy_locked_mat } else { energy_open_mat };
            transparent.push(RenderObject { fade: 0.0,
                position: Vec3::new(cp.x, cp.y - 0.14, cp.z),
                rotation: p.rotation,
                scale: Vec3::new(0.18, 0.28, 0.06),
                mesh,
                material: mat,
            });
        }
        // Lock indicators (v0.570): a small box per lock on the door face -- RED locked, GREEN
        // unlocked, GREY broken. Shows whether (and how) a door is secured even without a panel.
        // Doors only -- a window is a fixed pane (locks on a hand-authored window are inert).
        if drawn && !p.is_window {
            for (li, lock) in p.locks.iter().enumerate() {
                let st = locks_live.get(di).and_then(|v| v.get(li)).copied().unwrap_or(lock.state);
                let lm = match st {
                    crate::ship::lock_types::LockState::Locked => energy_locked_mat,
                    crate::ship::lock_types::LockState::Unlocked => energy_open_mat,
                    crate::ship::lock_types::LockState::Broken => slab_mat,
                };
                transparent.push(RenderObject { fade: 0.0,
                    position: Vec3::new(lock.pos.x, lock.pos.y - 0.05, lock.pos.z),
                    rotation: p.rotation,
                    scale: Vec3::new(0.1, 0.1, 0.05),
                    mesh,
                    material: lm,
                });
            }
        }
        // Nearest actor's HORIZONTAL distance -- eye/body height must not count, or a tall
        // camera would never trigger a short door.
        let dist = crate::ship::door_panels::door_actor_distance(p, camera_actor, &others, &animals);
        let landing = match ship {
            Some(ship) if landing_doors.get(di).copied().unwrap_or(false) => {
                crate::engine::footing::lift_landing_open(ship, cars, p.center)
            }
            _ => None,
        };
        let target = door_target(p, *open, locked_now, manual.get(di).copied().unwrap_or(false), landing, dist);
        // Frame-rate-independent exponential ease toward the target: smooth open/close, ~0.4 s
        // to settle, no snapping (v0.540, pure + tested in door_anim since v0.795).
        let open_before = *open;
        *open = crate::systems::door_anim::ease_open(*open, target, dt);
        // Door SFX (v0.983): fire on the swing's START edges - rising off
        // fully-closed plays the open sound, dropping off fully-open plays
        // the close. Mid-swing reversals stay quiet (no re-trigger spam when
        // a player hovers at an auto-door's radius), and doors beyond
        // earshot stay silent (a REMOTE actor can open doors far from you;
        // play_sound is non-spatial, so gate by distance until the spatial
        // path is wired).
        let within_earshot = state.aboard_station
            && (p.center - state_cam).length_squared() < 25.0 * 25.0;
        if within_earshot {
            if open_before <= 0.02 && *open > 0.02 {
                sfx.push(("sfx.door_open", "audio/sfx/door_open.ogg"));
            } else if open_before >= 0.98 && *open < 0.98 {
                sfx.push(("sfx.door_close", "audio/sfx/door_close.ogg"));
            }
        }
        let m = crate::systems::door_anim::panel_motion(&p.style, *open, p.size.x, p.size.y);
        if m.hidden || !drawn {
            continue;
        }
        let hinge_rot = Quat::from_rotation_y(m.hinge);
        let world_off = p.rotation * Vec3::new(m.offset.0, m.offset.1, m.offset.2);
        let c = p.center + world_off;
        let pos = p.hinge + hinge_rot * (c - p.hinge);
        let rot = hinge_rot * p.rotation;
        let scale = Vec3::new(p.size.x * m.scale.0, p.size.y * m.scale.1, p.size.z * m.scale.2);
        // Pick the panel material by style + lock state, and route glowing / glassy panels through
        // the transparent pass so they blend (v0.554).
        let (material, is_transparent) = if p.is_window {
            (glass_mat, true)
        } else if p.style == "energy" {
            // v0.570: lock-list aware (was `p.locked`), so an energy door driven by a lock list
            // glows red while actually impassable instead of a misleading green.
            (if locked_now { energy_locked_mat } else { energy_open_mat }, true)
        } else if p.style == "nanowall" {
            (nanowall_mat, true)
        } else {
            (slab_mat, false)
        };
        let obj = RenderObject { fade: 0.0, position: pos, rotation: rot, scale, mesh, material };
        if is_transparent {
            transparent.push(obj);
        } else {
            opaque.push(obj);
        }
    }
    state.pending_sfx.extend(sfx);
}

#[cfg(test)]
#[path = "home_meshes_tests.rs"]
mod tests;
