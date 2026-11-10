//! Material registry: everything that creates, textures or rewrites the
//! `Material` slots the render passes bind at group 2 (uniforms) and group 3
//! (albedo + the engine-global texture set).
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1093) under the file-size
//! ratchet, which mod.rs had outgrown at 4,090 lines against a 4,000 budget.
//! The material system is the cluster that comes out cleanest: registration
//! (`add_material*`), the textured variants (`add_textured_material`,
//! `bark_material`), in-place updates (`update_material_*`,
//! `set_material_albedo_texture`) and the two bind-group builders all of them
//! funnel through, with no other mod.rs code calling in.
//!
//! NO RE-EXPORT SHIM IS NEEDED. These are inherent methods on `Renderer`,
//! resolved by receiver type rather than by module path, so every call site in
//! the crate keeps working untouched - the same arrangement `billboard_bake.rs`
//! already uses for `cluster_sprite_material`. A child module can also see its
//! parent's private items, so the private `Renderer` fields (`pipeline`,
//! `materials`, `albedo_sampler`, `bark_sampler`, `bark_materials`, the shared
//! cloud/shadow/atmosphere views) stay private and are reached from here.
//!
//! `Material` itself deliberately STAYS in `mod.rs`: it is a field of the
//! `Renderer` struct and its private `bind_group` / `albedo_bind_group` are
//! read by ~20 render-pass sites that are not moving. Hoisting it here would
//! mean widening those two fields to `pub(super)` for the parent to keep
//! reading them - a privacy loosening bought for no cohesion gain.
//!
//! WHAT IS NO LONGER HERE (2026-09-19, the file-size ratchet at 602 of 500):
//! the GROUP 3 BIND GROUP builders - `build_material_texture_bind_group`,
//! `build_albedo_group_from_view` and its `_b14` variant - moved to
//! `renderer/material_bind_groups.rs`. This file registers and rewrites
//! material SLOTS; that one writes the sixteen-binding entry lists those slots
//! carry, which is where all the length was. The v0.1029-v0.1038 rule (touch
//! the layout and EVERY creation site must carry EVERY binding) travelled with
//! them and is restated at the top of that file.

use super::pipeline::MaterialUniforms;
use super::{billboard_bake, tree_mesh, Material, Renderer};
use wgpu::util::DeviceExt;

/// Material type 24: an in-world SCREEN (rung 1 of the in-world screens
/// ladder, docs/design/in-world-screens.md). The albedo texture is a
/// `ScreenSurface`'s render target bound by view; the shader shows it as an
/// EMITTER at `params.w` brightness (a display is a light source, not a
/// reflector: sun and shadow map do not darken it), with aerial haze and
/// underwater extinction still applied so it stays part of the scene. No
/// alpha cutout, no `fs_shadow` band: a screen is an opaque shadow caster.
///
/// The material type space, as `90-fragment-main.wgsl` dispatches it
/// (`material.params.z`, banded at +-0.5): 0 panel grid, 1 brushed metal,
/// 2 concrete, 3 wood, 4..11 the procedural surfaces of the `< N.5` chain,
/// 12 planet surface / terrain patch, 13 atmosphere shell, 14 scattering
/// atmosphere, 15 cloud deck, 16 ocean shell, 17 and 18 gas giant bands,
/// 19 textured mesh (photoscans, furniture), 20 near-tree foliage mesh,
/// 21 baked cluster card, 22 baked bark, 23 grass strand, 24 screen (this),
/// 25 character (below).
pub const MATERIAL_TYPE_SCREEN: f32 = 24.0;

/// Material type 25: a CHARACTER's skin, hair or clothes (character animation
/// rung 1, 2026-11-08; docs/design/character-animation.md 2.4). The surface
/// class's plain PBR of its base colour and roughness: no branch in
/// `90-fragment-main.wgsl` claims the type, so it reaches the entry's "keeps
/// the plain PBR look of its base_color" fall-through. Type 0 would lay the
/// walls' world-space panel grid over a person, type 1 brushed-metal
/// scratches. No wind (the vertex shader's wind is types 19 to 23), no cutout
/// in the shadow pass: a body is an opaque caster.
pub const MATERIAL_TYPE_CHARACTER: f32 = 25.0;

/// Material type 26: CARD HAIR, brows and lashes (2026-11-09, character animation's "the look";
/// docs/design/character-animation.md section 16). The technique, for an engine with no MSAA and no
/// temporal anti-aliasing (so no alpha-to-coverage and no hashed alpha): Scheuermann's two passes
/// (2004), an opaque pass that keeps only the cards' solid core (alpha at or over the cut, writing
/// depth, casting a cutout shadow) and a blended pass in the transparent list for the soft fringe
/// under it; and Karis's hair shading (2016, Marschner's reflection, transmission and second
/// reflection lobes along the strand, the second tinted by the hair), the strand direction taken
/// from the texture's V direction on screen. Its texture is a character pack
/// (`engine::look_textures`): colour (and alpha) left, normal map right. `params2`: x 1 (packed),
/// y 1 when the right half is a real normal map, z the alpha cut (positive: the opaque core pass;
/// negative: the blended fringe pass, which draws only the alpha under it), w the colour half's
/// mean luminance (the hair colour is laid over the strands' light and dark by it).
pub const MATERIAL_TYPE_HAIR: f32 = 26.0;

/// The alpha the opaque hair pass keeps at or over (Scheuermann's core; the fringe pass blends the
/// rest). High, so the solid core is only the cards' dense middle and the edges stay soft.
pub const HAIR_CUTOFF: f32 = 0.8;

impl Renderer {
    /// Register a material and return its handle (index).
    /// Uses material_type = 0.0 (default panel grid).
    pub fn add_material(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
    ) -> usize {
        self.add_material_typed(base_color, metallic, roughness, 0.0)
    }

    /// Register a material with an explicit material_type and return its handle (index).
    /// material_type: 0 = default panel grid, 1 = brushed metal, 2 = concrete, 3 = wood.
    /// emissive: 0.0 = no glow, 1.0+ = self-illuminating (sun, lava, neon lights).
    pub fn add_material_typed(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
    ) -> usize {
        self.add_material_full(base_color, metallic, roughness, material_type, 0.0)
    }

    /// Register a material with all parameters including emissive.
    pub fn add_material_full(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
    ) -> usize {
        let uniforms = MaterialUniforms {
            base_color,
            params: [metallic, roughness, material_type, emissive],
            params2: [0.0; 4],
        };
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material Uniform Buffer"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.pipeline.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let idx = self.materials.len();
        self.materials.push(Material {
            base_color,
            metallic,
            roughness,
            emissive,
            material_type,
            params2: [0.0; 4],
            buffer,
            bind_group,
            albedo_bind_group: None,
            albedo_texture: None,
        });
        idx
    }

    /// Create a single-level sRGB RGBA8 texture and upload `rgba` into it
    /// (row-major, row 0 = top). The RETAINED form of the texture the
    /// bind-group builders make internally: `add_textured_material` keeps
    /// the returned texture on the `Material` so later pixel updates can
    /// write into it in place.
    fn create_albedo_texture(&self, rgba: &[u8], width: u32, height: u32) -> wgpu::Texture {
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "albedo texture byte count must be width*height*4"
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Material Albedo Texture (retained)"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.write_albedo_pixels(&texture, rgba, width, height);
        texture
    }

    /// Upload `rgba` into level 0 of `texture` (which must be `width` x
    /// `height`). One `queue.write_texture`, no allocation.
    fn write_albedo_pixels(&self, texture: &wgpu::Texture, rgba: &[u8], width: u32, height: u32) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    /// Register a material whose albedo slot is bound to an EXISTING texture
    /// view owned by someone else (in-world screens, rung 1: the view is a
    /// `ScreenSurface`'s render target, so the scene samples the page's
    /// pixels directly with no readback and no copy). Everything else about
    /// the material is `add_material_full`; the bind group keeps the view
    /// alive, and `albedo_texture` stays `None` because the texture is not
    /// this material's to update.
    ///
    /// Nothing here touches the bind group LAYOUT: the groups come from
    /// `build_albedo_group_from_view`, the same entry list every other
    /// textured material uses (the v0.1029 rule).
    pub fn add_material_with_albedo_view(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
        view: &wgpu::TextureView,
    ) -> usize {
        let bg = self.build_albedo_group_from_view(view, &self.albedo_sampler);
        let idx = self.add_material_full(base_color, metallic, roughness, material_type, emissive);
        self.materials[idx].albedo_bind_group = Some(bg);
        idx
    }

    /// Rebind the albedo slot of material `idx` to another existing view (a
    /// screen surface recreated at a new size). Drops any retained texture:
    /// the material no longer owns its albedo. No-op if idx is out of range.
    pub fn set_material_albedo_view(&mut self, idx: usize, view: &wgpu::TextureView) {
        if idx >= self.materials.len() {
            return;
        }
        let bg = self.build_albedo_group_from_view(view, &self.albedo_sampler);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.materials[idx].albedo_texture = None;
        humanity_core::frame_costs::set_vram_keyed("vram.textures", idx as u64, 0);
    }

    /// Write new pixels into material `idx`'s RETAINED albedo texture. When
    /// the size matches the texture it already owns this is one
    /// `queue.write_texture` and the bind group is untouched (the shape a
    /// per-frame video feed needs); when the size differs, or the material
    /// owns no texture yet, a new texture is allocated and rebound. No-op if
    /// idx is out of range.
    pub fn update_material_albedo_pixels(&mut self, idx: usize, rgba: &[u8], width: u32, height: u32) {
        if idx >= self.materials.len() {
            return;
        }
        let same_size = matches!(
            self.materials[idx].albedo_texture,
            Some((_, w, h)) if w == width && h == height
        );
        if same_size {
            let (tex, _, _) = self.materials[idx].albedo_texture.as_ref().expect("checked above");
            self.write_albedo_pixels(tex, rgba, width, height);
            return;
        }
        let texture = self.create_albedo_texture(rgba, width, height);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.build_albedo_group_from_view(&view, &self.albedo_sampler);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.materials[idx].albedo_texture = Some((texture, width, height));
        humanity_core::frame_costs::set_vram_keyed(
            "vram.textures",
            idx as u64,
            (width as u64) * (height as u64) * 4,
        );
    }

    /// Register a material that carries a real albedo texture at group 3
    /// (v0.811: per-pixel planet imagery; sRGB RGBA8 bytes, row-major,
    /// row 0 = top). Draws using it bind the texture instead of the white
    /// fallback; everything else about the material behaves like
    /// `add_material_full`.
    pub fn add_textured_material(
        &mut self,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> usize {
        // Retained form (rung 1): the texture stays on the Material so
        // `update_material_albedo_pixels` can write into it in place later.
        let texture = self.create_albedo_texture(rgba, width, height);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let albedo_bind_group = self.build_albedo_group_from_view(&view, &self.albedo_sampler);
        let idx = self.add_material_full(base_color, metallic, roughness, material_type, emissive);
        self.materials[idx].albedo_bind_group = Some(albedo_bind_group);
        self.materials[idx].albedo_texture = Some((texture, width, height));
        // VRAM inventory (resource budgets increment 1). Keyed by material
        // index so a later in-place albedo swap REPLACES this figure instead of
        // adding to it.
        humanity_core::frame_costs::set_vram_keyed(
            "vram.textures",
            idx as u64,
            (width as u64) * (height as u64) * 4,
        );
        idx
    }

    /// The BAKED BARK material for one tree species (v0.1089), material type
    /// 22, created on first use and shared by every variant of that species.
    ///
    /// This is the whole wiring surface of the bark work: a caller that has a
    /// wood mesh asks for the species' material and draws it. The bake, the
    /// mip chain, the tiling sampler and the once-per-session memo all live
    /// here, where they can be reasoned about, rather than at a call site
    /// inside a per-frame block (the BUG-059 shape).
    ///
    /// `base_color` is white: the per-species colour is IN the texture, so the
    /// shader's `albedo * texture` is one multiply and not a squared
    /// trunk_color. Emissive is 0 - type 22 does not repurpose params.w (its
    /// wind class is implied by the type in the vertex shader), so the normal
    /// emissive meaning of that slot is left alone.
    pub fn bark_material(&mut self, def: &humanity_terrain::tree_registry::TreeDef) -> usize {
        if let Some(&idx) = self.bark_materials.get(&def.id) {
            return idx;
        }
        let t0 = std::time::Instant::now();
        let px = tree_mesh::BARK_PX;
        let base = tree_mesh::bake_bark_rgba(def);
        let levels = billboard_bake::build_opaque_mip_chain(&base, px);
        let refs: Vec<&[u8]> = levels.iter().map(|l| l.as_slice()).collect();
        let bg = self.build_material_texture_bind_group(&refs, px, px, &self.bark_sampler);
        // Roughness 0.85 is the BASE; the type-22 branch varies it per texel
        // from the baked height (crevices rougher, ridges smoother).
        let idx = self.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.85, 22.0, 0.0);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.bark_materials.insert(def.id.clone(), idx);
        // VRAM inventory: the whole mip chain, not just the base level.
        humanity_core::frame_costs::set_vram_keyed(
            "vram.textures",
            idx as u64,
            levels.iter().map(|l| l.len() as u64).sum(),
        );
        log::info!(
            "[Bark] {} baked {px}x{px} + {} mips, tile {:.2} m, in {:.0} ms",
            def.id,
            levels.len() - 1,
            tree_mesh::bark_tile_m(def),
            t0.elapsed().as_secs_f32() * 1000.0
        );
        idx
    }

    /// A CHARACTER material with a packed texture (2026-11-09, the look): type 25 (cloth, shoes,
    /// hats: colour times the texture, the normal map from its right half) or 26 (hair cards, see
    /// `MATERIAL_TYPE_HAIR`), with its whole mip chain (`levels`, biggest first, `width` by `height`
    /// at level 0) and its `params2` (the type's notes). The texture is the material's own; a garment's
    /// materials are made once a session and shared by everyone wearing it in that colour.
    pub fn add_character_material(&mut self, base_color: [f32; 4], roughness: f32, material_type: f32, params2: [f32; 4], levels: &[Vec<u8>], width: u32, height: u32) -> usize {
        let refs: Vec<&[u8]> = levels.iter().map(|l| l.as_slice()).collect();
        let bg = self.build_material_texture_bind_group(&refs, width, height, &self.albedo_sampler);
        let idx = self.add_material_full(base_color, 0.0, roughness, material_type, 0.0);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.update_material_params2(idx, params2);
        humanity_core::frame_costs::set_vram_keyed("vram.textures", idx as u64, levels.iter().map(|l| l.len() as u64).sum());
        idx
    }

    /// Another material over the SAME texture as material `src` (shared, not copied): a garment's
    /// pack worn in another colour, or hair's fringe pass beside its core. None when `src` has no
    /// texture of its own.
    pub fn add_material_like(&mut self, src: usize, base_color: [f32; 4], roughness: f32, material_type: f32, params2: [f32; 4]) -> Option<usize> {
        let groups = self.materials.get(src)?.albedo_bind_group.as_ref().map(|g| super::AlbedoBindGroup { colour: g.colour.clone(), shadow: g.shadow.clone() })?;
        let idx = self.add_material_full(base_color, 0.0, roughness, material_type, 0.0);
        self.materials[idx].albedo_bind_group = Some(groups);
        self.update_material_params2(idx, params2);
        Some(idx)
    }

    /// Let go of material `idx`'s texture (2026-11-10, the look: a garment nobody near has worn for a
    /// while): its bind group is dropped, so the texture is freed once no other material shares it,
    /// and its memory figure goes to 0. The material itself stays, its index valid, until
    /// `set_character_texture` gives it a texture again. No-op if idx is out of range.
    pub fn drop_material_texture(&mut self, idx: usize) {
        if let Some(mat) = self.materials.get_mut(idx) {
            mat.albedo_bind_group = None;
            mat.albedo_texture = None;
            humanity_core::frame_costs::set_vram_keyed("vram.textures", idx as u64, 0);
        }
    }

    /// Give material `idx` a packed character texture again (its whole mip chain, as
    /// `add_character_material`), in place: a released garment worn again keeps its material
    /// indices. No-op if idx is out of range.
    pub fn set_character_texture(&mut self, idx: usize, params2: [f32; 4], levels: &[Vec<u8>], width: u32, height: u32) {
        if idx >= self.materials.len() {
            return;
        }
        let refs: Vec<&[u8]> = levels.iter().map(|l| l.as_slice()).collect();
        let bg = self.build_material_texture_bind_group(&refs, width, height, &self.albedo_sampler);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.update_material_params2(idx, params2);
        humanity_core::frame_costs::set_vram_keyed("vram.textures", idx as u64, levels.iter().map(|l| l.len() as u64).sum());
    }

    /// Point material `dst` at the texture material `src` holds (shared, as `add_material_like`):
    /// a released garment's other colours and its fringe pass, once its texture is read again.
    pub fn share_material_texture(&mut self, dst: usize, src: usize) {
        let groups = self.materials.get(src).and_then(|m| m.albedo_bind_group.as_ref()).map(|g| super::AlbedoBindGroup { colour: g.colour.clone(), shadow: g.shadow.clone() });
        if let (Some(groups), Some(mat)) = (groups, self.materials.get_mut(dst)) {
            mat.albedo_bind_group = Some(groups);
        }
    }

    /// Replace the albedo texture of an existing material IN PLACE (v0.811):
    /// hot-reloading a planet's RON re-bakes its imagery, and swapping the
    /// texture on the existing material index keeps VRAM bounded (the old
    /// texture is freed when its bind group drops) and every RenderObject's
    /// material index stable. No-op if idx is out of range.
    pub fn set_material_albedo_texture(
        &mut self,
        idx: usize,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) {
        if idx >= self.materials.len() {
            return;
        }
        let texture = self.create_albedo_texture(rgba, width, height);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.build_albedo_group_from_view(&view, &self.albedo_sampler);
        self.materials[idx].albedo_bind_group = Some(bg);
        self.materials[idx].albedo_texture = Some((texture, width, height));
        // The old texture is freed with its bind group, so the inventory
        // REPLACES this material's contribution rather than accumulating.
        humanity_core::frame_costs::set_vram_keyed(
            "vram.textures",
            idx as u64,
            (width as u64) * (height as u64) * 4,
        );
    }

    /// Update the material at `idx` in place by rewriting its existing uniform buffer (reuses the
    /// buffer + bind group, zero allocation). No-op if idx is out of range. (v0.531)
    pub fn update_material_full(
        &mut self,
        idx: usize,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
        emissive: f32,
    ) {
        if let Some(mat) = self.materials.get_mut(idx) {
            let uniforms = MaterialUniforms {
                base_color,
                params: [metallic, roughness, material_type, emissive],
                // Preserve the stored per-material data vector: this method
                // rewrites the whole buffer, and zeroing params2 here would
                // wipe the cloud slab bounds on every frame update.
                params2: mat.params2,
            };
            // The CPU-side copy tracks the uniform (v0.1108). A slot reused
            // for a different type - the machine rebuild does exactly this -
            // must not keep the old type's shadow-PSO choice.
            mat.material_type = material_type;
            self.queue
                .write_buffer(&mat.buffer, 0, bytemuck::bytes_of(&uniforms));
        }
    }

    /// Set the material's second data vector (`material.params2` in the
    /// shader) in place: stores the CPU copy (so `update_material_full`
    /// preserves it) and writes just that 16-byte slice of the uniform
    /// buffer. Today's only consumer is the type-15 cloud shell:
    /// [slab base ratio, slab top ratio, planet radius km, 0].
    pub fn update_material_params2(&mut self, idx: usize, params2: [f32; 4]) {
        if let Some(mat) = self.materials.get_mut(idx) {
            if mat.params2 == params2 {
                return;
            }
            mat.params2 = params2;
            // params2 sits after base_color (16 B) + params (16 B).
            self.queue
                .write_buffer(&mat.buffer, 32, bytemuck::bytes_of(&params2));
        }
    }

    /// Update the material at `idx` in place (typed convenience; emissive 0). (v0.531)
    pub fn update_material_typed(
        &mut self,
        idx: usize,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        material_type: f32,
    ) {
        self.update_material_full(idx, base_color, metallic, roughness, material_type, 0.0);
    }

}