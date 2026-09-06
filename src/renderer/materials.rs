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
//! IMPORTANT (v0.1029-v0.1038 incident): `build_material_texture_bind_group`
//! below is one of the three `texture_bind_group_layout` creation sites, and
//! the one that is built LAZILY when a textured material first loads - which
//! is why a missing binding there survived ten menu-only boot verifies and
//! panicked on every world entry. If you touch that layout, every site must
//! carry every binding.

use super::pipeline::MaterialUniforms;
use super::{billboard_bake, tree_mesh, AlbedoBindGroup, Material, Renderer};
use wgpu::util::DeviceExt;

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
        });
        idx
    }

    /// Build a group-3 bind group for an sRGB RGBA8 image (v0.811, per-pixel
    /// planet imagery). The Srgb format makes sampling return LINEAR values
    /// automatically -- the whole material pipeline is linear; the sRGB
    /// encode happens once, on store to the sRGB render target. The bind
    /// group keeps the texture + view alive internally.
    fn build_albedo_bind_group(&self, rgba: &[u8], width: u32, height: u32) -> AlbedoBindGroup {
        self.build_material_texture_bind_group(&[rgba], width, height, &self.albedo_sampler)
    }

    /// The general form (v0.1089): any number of MIP LEVELS, biggest first,
    /// and an explicit sampler. `build_albedo_bind_group` above is this with
    /// one level and the shared clamp-V sampler; baked bark passes a full
    /// chain and the tiling sampler.
    ///
    /// Nothing here changes the bind group LAYOUT - the entry list below is
    /// still every binding 0..15, which is the invariant the v0.1029-v0.1038
    /// incident was about. Level count and sampler are texture/bind-group
    /// state, not layout state.
    ///
    /// Returns BOTH group-3 flavours (v0.1108): the colour-pass group and its
    /// shadow-safe twin. The entry list is written ONCE, in a closure whose
    /// only parameter is the binding-6 texture view, so "identical except at
    /// binding 6" is a property of the code's shape rather than something a
    /// test has to re-check - and so a future binding added to the layout
    /// cannot be added to one group and missed on the other (that is the
    /// v0.1029 incident class, one level down).
    // `pub(super)` restores EXACTLY the visibility this had in mod.rs: private
    // there meant "renderer and all its descendants", which is how the sibling
    // `billboard_bake::cluster_sprite_material` calls it. Private HERE would
    // mean "materials and its descendants" only, so the sibling would break.
    pub(super) fn build_material_texture_bind_group(
        &self,
        levels: &[&[u8]],
        width: u32,
        height: u32,
        sampler: &wgpu::Sampler,
    ) -> AlbedoBindGroup {
        assert!(!levels.is_empty(), "a material texture needs at least one level");
        assert_eq!(
            levels[0].len(),
            width as usize * height as usize * 4,
            "albedo texture byte count must be width*height*4"
        );
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Material Albedo Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, bytes) in levels.iter().enumerate() {
            let lw = (width >> level).max(1);
            let lh = (height >> level).max(1);
            debug_assert_eq!(bytes.len(), lw as usize * lh as usize * 4, "mip {level} size");
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * lw),
                    rows_per_image: Some(lh),
                },
                wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.build_albedo_group_from_view(&view, sampler)
    }

    /// Build the two group-3 bind groups (colour + shadow) around an
    /// EXISTING texture view in the albedo slot. Split out of
    /// `build_material_texture_bind_group` for the temporal cloud map
    /// (phase 4), whose render target rides the albedo slot so the
    /// composite needs NO layout change. ONE entry list, two bind groups:
    /// `depth6` is the ONLY difference - the real sun map for colour
    /// passes, the 1x1 dummy for the shadow pass (which writes the real
    /// one and so may not sample it).
    pub(super) fn build_albedo_group_from_view(
        &self,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> AlbedoBindGroup {
        // The one-line wrapper (far rung, A8): the entry list is written
        // ONCE, in the b14 variant below, with the tree atlas at binding 14.
        self.build_albedo_group_from_view_b14(view, sampler, &self.tree_atlas_view)
    }

    /// The same two group-3 bind groups with binding 14 OVERRIDDEN: `b14`
    /// takes the slot the tree atlas normally rides (`tree_atlas_tex`,
    /// read only by the vegetation branches). The cloud profile atlas
    /// (perf increment 4, the far rung) rides here for every cloud-side
    /// group - the march, the shell, the Low sheet, the bake's calibration
    /// source and the mip passes - so the profile needed NO bind-group-
    /// layout change (16 entries at every site, the v0.1029 rule). Every
    /// other caller goes through `build_albedo_group_from_view`, which
    /// passes the tree atlas, so this is still the ONE entry list.
    pub(super) fn build_albedo_group_from_view_b14(
        &self,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        b14: &wgpu::TextureView,
    ) -> AlbedoBindGroup {
        let build = |depth6: &wgpu::TextureView, label: &str| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.pipeline.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    // Shared cloud-noise volumes (clouds increment 3): every
                    // group-3 bind group carries the same engine-global views.
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.cloud_shape_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.cloud_detail_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.cloud_tile_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.weather_map_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(depth6),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&self.shadow_comparison_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: self.shadow_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::TextureView(&self.ground_textures.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::Sampler(&self.ground_textures.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::TextureView(&self.atmo_trans_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12,
                        resource: wgpu::BindingResource::TextureView(&self.atmo_ms_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 13,
                        resource: wgpu::BindingResource::TextureView(&self.sky_view.target_view),
                    },
                    // Binding 14: the tree atlas for every ordinary
                    // material, the cloud profile atlas (or one of its
                    // mips) for the cloud-side groups (see the b14 note).
                    wgpu::BindGroupEntry {
                        binding: 14,
                        resource: wgpu::BindingResource::TextureView(b14),
                    },
                    // v0.1039 CRASH FIX: binding 15 (FFT ocean tile) was added
                    // to the LAYOUT in v0.1029 but this per-material creation
                    // site was missed - the other two sites were updated, and
                    // menu-only boot-verifies never create a textured material,
                    // so every world entry on v0.1029-v0.1038 panicked with
                    // "15 bindings vs 16 in layout" (operator: "insta crashes
                    // when I press esc"). Every texture_bind_group_layout
                    // create_bind_group site MUST carry every binding.
                    wgpu::BindGroupEntry {
                        binding: 15,
                        resource: wgpu::BindingResource::TextureView(&self.water_fft_view),
                    },
                ],
            })
        };
        AlbedoBindGroup {
            colour: build(&self.shadow_map_view, "Material Albedo Bind Group"),
            shadow: build(&self.dummy_depth_view, "Material Albedo BG (shadow pass)"),
        }
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
        let albedo_bind_group = self.build_albedo_bind_group(rgba, width, height);
        let idx = self.add_material_full(base_color, metallic, roughness, material_type, emissive);
        self.materials[idx].albedo_bind_group = Some(albedo_bind_group);
        // VRAM inventory (resource budgets increment 1). Keyed by material
        // index so a later in-place albedo swap REPLACES this figure instead of
        // adding to it.
        super::frame_costs::set_vram_keyed(
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
    pub fn bark_material(&mut self, def: &tree_mesh::TreeDef) -> usize {
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
        super::frame_costs::set_vram_keyed(
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
        let bg = self.build_albedo_bind_group(rgba, width, height);
        self.materials[idx].albedo_bind_group = Some(bg);
        // The old texture is freed with its bind group, so the inventory
        // REPLACES this material's contribution rather than accumulating.
        super::frame_costs::set_vram_keyed(
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