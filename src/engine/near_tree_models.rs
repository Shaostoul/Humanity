//! Near-tree MODEL CACHE: the one-time build/parse of every tree mesh the
//! near field can draw, plus the sprite-atlas bake that gives the far cards
//! the same trees (extracted from lib.rs, v0.1108).
//!
//! WHY IT IS A MODULE and not still inline in the frame loop: it is ~420
//! lines with ONE job (populate `decoration_mesh_cache` + `bake_models`),
//! it reads nothing from the frame besides `state`, and the file-size
//! ratchet had named it as the extraction lib.rs owed - three increments in
//! a row paid the budget back by shortening comments instead.
//!
//! WHY `engine/` and not `terrain/` (which the ratchet note suggested): this
//! needs the wgpu renderer and the asset manager, both native-only, while
//! `terrain/` is ungated and compiles into the headless relay build. Putting
//! it there would have forced a cfg gate onto an otherwise clean module tree;
//! `engine/` is already the native-gated home for extracted loop code.
//!
//! WHY IT TAKES FOUR BORROWS and not `&mut EngineState`: the frame loop calls
//! this while holding a mutable borrow of `state.planet_chunk_states` (the
//! chunk state whose `last_drawn` set drives the whole near-tree block), so a
//! whole-state borrow is an E0499. Naming the four fields it actually touches
//! keeps the disjoint-field borrow the inline code had, and documents the
//! dependency surface besides.

use crate::assets::AssetManager;
use crate::renderer::Renderer;
use std::collections::HashMap;

/// Build/parse every near-tree model exactly once, then bake the sprite atlas.
///
/// Idempotent and cheap after the first call: every branch is guarded by a
/// `decoration_mesh_cache` lookup, so a steady-state frame walks the name list
/// and returns. Called from the near-tree draw block each frame precisely so a
/// species that streams in later still gets its mesh.
pub(crate) fn ensure_near_tree_models(
    renderer: &mut Renderer,
    asset_manager: &AssetManager,
    decoration_mesh_cache: &mut HashMap<String, (usize, usize)>,
    tree_atlas_attempted: &mut bool,
) {
    // Lazy-load all six conifer shapes (3 fir +
    // 3 pine variants) plus their _bark trunk
    // parts (v0.913 - the trunk carries its own
    // bark texture now, so it no longer
    // alpha-discards into invisibility).
    let mut tree_names: Vec<String> = Vec::new();
    for base in ["fir_sapling", "pine_sapling_small"] {
        for v in 1..=3 {
            for suffix in ["", "_bark"] {
                tree_names.push(format!("{base}_v{v}{suffix}"));
            }
        }
    }
    // v0.1066: PROCEDURAL species from
    // data/vegetation/trees.ron are built here
    // instead of loaded. One mesh per (species,
    // variant) - roughly 18 meshes total - then
    // instanced by transform, so a forest costs
    // meshes, not one per tree. These are the
    // only trees a RELEASE build has, because
    // the bundle does not ship assets/models/.
    // v0.1083: every CPU mesh built or parsed
    // in this block is ALSO handed to the card
    // bake below, so nothing is generated or
    // parsed twice. Before this, the bake
    // re-parsed the same 12 glTF files (each
    // pine bark paying a 2048->1024 texture
    // downscale, 220-275 ms apiece in the log)
    // - that redundant second parse, not the
    // GPU bake, was the dominant cost of the
    // world-entry freeze the bake was blamed
    // for.
    let mut bake_models: std::collections::HashMap<
        String,
        crate::renderer::billboard_bake::BakeCpuModel,
    > = std::collections::HashMap::new();
    let mut tree_parse_ms = 0.0f32;
    // Cluster sprites baked up front (v0.1088): the
    // card MATERIALS need the sprite textures at
    // mesh-build time. BUG-059 (v0.1088.3): this
    // whole block is PER-FRAME (the moved>12 m
    // hysteresis closes above it, not around it),
    // and the original unconditional call here ran
    // the ~90 ms blocking bake EVERY FRAME - 1671+
    // [Cluster] lines per session on three rigs,
    // costing the whole frame budget. Bake ONLY
    // when a clustered species still lacks its
    // card cache entry (i.e. once, at world entry,
    // and again only if new species stream in).
    let need_sprites = {
        use crate::renderer::tree_mesh;
        tree_mesh::registry().trees.iter().any(|t| {
            t.is_procedural()
                && t.clusters.is_some()
                && !decoration_mesh_cache.contains_key(
                    &format!("proc:{}_v0:card0", t.id),
                )
        })
    };
    let cluster_sprites = if need_sprites {
        renderer.bake_cluster_sprites(None)
    } else {
        Vec::new()
    };
    {
        use crate::renderer::tree_mesh;
        let reg = tree_mesh::registry();
        for t in reg.trees.iter().filter(|t| t.is_procedural()) {
            for v in 0..t.variants.max(1) {
                let key = format!("proc:{}_v{v}", t.id);
                if decoration_mesh_cache.contains_key(&key) {
                    continue;
                }
                // Built at the species' nominal
                // height; per-tree height rides
                // on the instance scale below.
                // Clustered species also return
                // their card layers (v0.1088) -
                // the crown mass lives on those.
                let tb = tree_mesh::build_tree_and_cards(
                    t,
                    t.height_m,
                    v.wrapping_mul(2_654_435_761),
                );
                for (ci, card) in tb.cards.iter().enumerate() {
                    let Some(spr) = cluster_sprites.iter().find(
                        |s| s.species == t.id && s.layer == card.layer,
                    ) else {
                        continue;
                    };
                    let cmesh = crate::renderer::mesh::Mesh::from_vertices(
                        &renderer.device,
                        &card.mesh.vertices,
                        &card.mesh.indices,
                    );
                    let cmi = renderer.add_mesh(cmesh);
                    // Type 21 = cluster card (sprite
                    // texture, AO-coded UVs, foliage
                    // transmission in the shader).
                    // Carries the sprite's FULL MIP
                    // CHAIN behind a trilinear clamped
                    // sampler (v0.1090): the chain was
                    // already baked and the old
                    // add_textured_material upload
                    // threw it away, so every cluster
                    // card crawled as it minified.
                    let cma =
                        renderer.cluster_sprite_material(spr);
                    decoration_mesh_cache.insert(
                        format!("proc:{}_v{v}:card{ci}", t.id),
                        (cmi, cma),
                    );
                }
                // WOOD (v0.1089): the bark is
                // its own mesh with real
                // cylindrical UVs on material
                // type 22, sampling the
                // species' baked bark texture
                // through the per-material
                // albedo slot - the same slot
                // the cards above use, so no
                // bind-group layout changes.
                // The material is memoized per
                // SPECIES inside the renderer.
                if !tb.wood.indices.is_empty() {
                    let wmesh = crate::renderer::mesh::Mesh::from_vertices(
                        &renderer.device,
                        &tb.wood.vertices,
                        &tb.wood.indices,
                    );
                    let wmi = renderer.add_mesh(wmesh);
                    let wma = renderer.bark_material(t);
                    decoration_mesh_cache.insert(
                        format!("proc:{}_v{v}:wood", t.id),
                        (wmi, wma),
                    );
                }
                let mesh = crate::renderer::mesh::Mesh::from_vertices(
                    &renderer.device,
                    &tb.mesh.vertices,
                    &tb.mesh.indices,
                );
                let mi = renderer.add_mesh(mesh);
                // Type 20 = packed per-face
                // colour + the close-range leaf
                // shading (venation, wax,
                // backlit transmission).
                let ma = renderer.add_material_typed(
                    [1.0, 1.0, 1.0, 1.0],
                    0.0,
                    0.9,
                    20.0,
                );
                decoration_mesh_cache
                    .insert(key, (mi, ma));
                // The card baker wants the same
                // geometry; hand it over rather
                // than regenerating it there.
                bake_models.insert(
                    crate::renderer::billboard_bake::proc_key(
                        &t.id, v,
                    ),
                    crate::renderer::billboard_bake::BakeCpuModel {
                        // The SINGLE-MESH form
                        // (foliage + the wood's
                        // packed-colour twin):
                        // the atlas bake shader
                        // knows only the packed
                        // decode, so it must not
                        // be handed the
                        // UV-carrying wood.
                        vertices: tb.bake.vertices,
                        indices: tb.bake.indices,
                        texture: None,
                    },
                );
            }
        }
    }
    for name in &tree_names {
        let name = name.as_str();
        if decoration_mesh_cache.contains_key(name) {
            continue;
        }
        let stem = name.strip_suffix("_bark").unwrap_or(name);
        let base =
            stem.rfind("_v").map(|i| &stem[..i]).unwrap_or(stem);
        let rel = format!(
            "assets/models/plants/{}/{}.gltf",
            base, name
        );
        let t_parse = std::time::Instant::now();
        let mut parsed = asset_manager
            .parse_gltf_mesh_with_texture(&rel);
        tree_parse_ms +=
            t_parse.elapsed().as_secs_f32() * 1000.0;
        // SCAN-STRETCH GUARD (v0.1101, BUG-063).
        // The draw site scales a photoscan by
        // species_height / model_height applied
        // uniformly to EVERY triangle, so a scan
        // of a sapling standing in for a mature
        // tree inflates its leaves to match. No
        // scale factor turns a sapling into a
        // mature tree - the architecture differs,
        // not just the size - so past a modest
        // stretch we GROW the species instead.
        // Rejection reuses the missing-asset path
        // (sentinel + procedural twin, BUG-058)
        // rather than adding a second fallback.
        if let Ok((cpu, _)) = &parsed {
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for v in &cpu.vertices {
                lo = lo.min(v.position[1]);
                hi = hi.max(v.position[1]);
            }
            let model_h = (hi - lo).max(1.0e-3);
            let target_h = crate::renderer::tree_mesh::registry()
                .trees
                .iter()
                .find(|t| t.model == base)
                .map(|t| t.height_m)
                .unwrap_or(model_h);
            let stretch = target_h / model_h;
            let max_stretch = crate::MAX_MODEL_STRETCH;
            if stretch > max_stretch {
                log::warn!(
                    "[NearTree] {name} REJECTED: the scan is \
                     {model_h:.2} m and the species is \
                     {target_h:.1} m, a {stretch:.1}x uniform \
                     stretch (limit {max_stretch:.1}x). \
                     Every leaf would be {stretch:.1}x too big. \
                     Growing this species procedurally instead."
                );
                parsed = Err(format!(
                    "scan stretch {stretch:.1}x exceeds \
                     {max_stretch:.1}x"
                ));
            }
        }
        match parsed {
            Ok((cpu, tex)) => {
                let mesh =
                    crate::renderer::mesh::Mesh::from_vertices(
                        &renderer.device,
                        &cpu.vertices,
                        &cpu.indices,
                    );
                let mi = renderer.add_mesh(mesh);
                if let Some((rgba, w, h)) = &tex {
                    let clear = rgba
                        .chunks_exact(4)
                        .filter(|p| p[3] < 128)
                        .count();
                    log::info!(
                        "[NearTree] {name}: texture {w}x{h}, {}% transparent texels",
                        clear * 100 / (rgba.len() / 4).max(1)
                    );
                }
                let ma = match &tex {
                    Some((rgba, w, h)) => {
                        renderer.add_textured_material(
                            [1.0, 1.0, 1.0, 1.0],
                            0.0,
                            0.9,
                            19.0,
                            // WIND CLASS, not
                            // emissive (v0.1089):
                            // params.w is free on
                            // type 19 (the shader
                            // zeroes emissive for
                            // this type) and the
                            // vertex stage reads
                            // it as the wind
                            // class. 1.0 = woody.
                            // It is set HERE and
                            // nowhere else on
                            // purpose: type 19
                            // also draws furniture
                            // and machine glTFs
                            // (home_meshes.rs) and
                            // world decorations
                            // (world_load.rs),
                            // which must stay
                            // rigid.
                            1.0,
                            rgba,
                            *w,
                            *h,
                        )
                    }
                    None => renderer.add_material_full(
                        [0.2, 0.35, 0.15, 1.0],
                        0.0,
                        0.9,
                        0.0,
                        0.0,
                    ),
                };
                decoration_mesh_cache
                    .insert(name.to_string(), (mi, ma));
                bake_models.insert(
                    rel,
                    crate::renderer::billboard_bake::BakeCpuModel {
                        vertices: cpu.vertices,
                        indices: cpu.indices,
                        texture: tex,
                    },
                );
            }
            Err(e) => {
                // Sentinel so a missing asset logs
                // once, not per frame. The sentinel
                // is ALSO the shipped-build fallback
                // marker: the draw site sees
                // mi == usize::MAX on the model key
                // and switches the species to its
                // procedural twin (use_proc).
                log::warn!(
                    "near-tree model {name} failed: {e}"
                );
                decoration_mesh_cache.insert(
                    name.to_string(),
                    (usize::MAX, usize::MAX),
                );
                // SHIPPED-BUILD FALLBACK (v0.1086,
                // BUG-058): release bundles carry no
                // assets/models/, so fir and pine
                // rendered NOTHING at any distance.
                // Build the species procedurally,
                // exactly like the is_procedural()
                // loop above - AT THE SPECIES
                // HEIGHT, so the draw site's
                // use_proc scale stays ~1.0. (Doing
                // this at model scale and keeping
                // the TREE_MODEL_H divisor would
                // draw a 381 m fir - the critic's
                // trap, journaled 2026-08-01.)
                if !name.ends_with("_bark") {
                    if let Some(vn) = stem
                        .rfind("_v")
                        .and_then(|i| stem[i + 2..].parse::<u32>().ok())
                    {
                        let va = vn.saturating_sub(1);
                        let reg = crate::renderer::tree_mesh::registry();
                        if let Some(t) =
                            reg.trees.iter().find(|t| t.model == base)
                        {
                            let key = format!("proc:{}_v{va}", t.id);
                            if !decoration_mesh_cache
                                .contains_key(&key)
                            {
                                let mut b = crate::renderer::plant_mesh::PlantMeshBuilder::new();
                                crate::renderer::tree_mesh::build_tree(
                                    &mut b,
                                    t,
                                    t.height_m,
                                    va.wrapping_mul(2_654_435_761),
                                );
                                let mesh = crate::renderer::mesh::Mesh::from_vertices(
                                    &renderer.device,
                                    &b.vertices,
                                    &b.indices,
                                );
                                let mi = renderer.add_mesh(mesh);
                                let ma = renderer.add_material_typed(
                                    [1.0, 1.0, 1.0, 1.0],
                                    0.0,
                                    0.9,
                                    20.0,
                                );
                                decoration_mesh_cache
                                    .insert(key, (mi, ma));
                                bake_models.insert(
                                    crate::renderer::billboard_bake::proc_key(&t.id, va),
                                    crate::renderer::billboard_bake::BakeCpuModel {
                                        vertices: b.vertices,
                                        indices: b.indices,
                                        texture: None,
                                    },
                                );
                                log::info!(
                                    "[NearTree] {}: procedural fallback built (shipped-build path)",
                                    t.id
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Sprite atlas bake (v0.961 increment 2;
    // registry-driven since v0.1083): once per
    // session, render EVERY (species, variant)
    // in data/vegetation/trees.ron side-on
    // into its atlas tile, so the terrain card
    // stage textures its quads with the SAME
    // trees the near field draws in 3D.
    // Procedural species build their mesh
    // inside the baker (no files); model
    // species come from the parse cache above,
    // and a missing model just leaves its tile
    // transparent instead of aborting the
    // whole atlas the way it used to.
    if !renderer.tree_atlas_ready && !*tree_atlas_attempted {
        *tree_atlas_attempted = true;
        renderer.bake_tree_atlas_from_registry(
            &bake_models,
            tree_parse_ms,
            None,
        );
    }
}
