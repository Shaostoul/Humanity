//! GROUP 3 BIND GROUPS: the entry lists that hand the shader its albedo and
//! the engine-global texture set, and the ONE place a binding may be added.
//!
//! Extracted VERBATIM from `renderer/materials.rs` (2026-09-19) under the
//! file-size ratchet, which materials.rs had outgrown at 602 lines against a
//! 500 budget.
//!
//! WHY THIS IS THE CLUSTER. materials.rs does two separable things: it
//! REGISTERS and REWRITES material slots (add_material*, update_material*,
//! bark_material - short functions that push or patch a `Material`), and it
//! BUILDS the group-3 bind groups those slots carry. The second is where all
//! the length is: three functions, each an entry list of sixteen bindings,
//! and every caller reaches them through the same two names. Nothing else in
//! materials.rs builds a bind group, and nothing here registers a material.
//!
//! THE v0.1029-v0.1038 RULE LIVES HERE NOW, and it is why this file is worth
//! having its own name. `build_material_texture_bind_group` is one of the
//! three `texture_bind_group_layout` creation sites, and the one built
//! LAZILY when a textured material first loads - which is why a missing
//! binding there survived ten menu-only boot verifies and then panicked on
//! every world entry. If you touch that layout, EVERY site must carry EVERY
//! binding. The `build_albedo_group_from_view` / `_b14` pair is shaped to
//! make that structural rather than remembered: one entry list, written once
//! in the `_b14` variant, with the plain variant a one-line wrapper that
//! passes the tree atlas.
//!
//! NO RE-EXPORT SHIM IS NEEDED: these are inherent methods on `Renderer`,
//! resolved by receiver type rather than module path, so the sibling
//! `billboard_bake` and every pass keep calling them untouched. `pub(super)`
//! means the same thing here as it did in materials.rs - both are children of
//! `renderer` - so no visibility widened.

use super::{AlbedoBindGroup, Renderer};
use wgpu::util::DeviceExt;

impl Renderer {
    /// The general form (v0.1089): any number of MIP LEVELS, biggest first,
    /// and an explicit sampler. Single-level sRGB albedo images (v0.811,
    /// per-pixel planet imagery) now go through `create_albedo_texture` so
    /// the texture is retained on the Material; baked bark passes a full
    /// chain and the tiling sampler through here. The Srgb format makes
    /// sampling return LINEAR values automatically -- the whole material
    /// pipeline is linear; the sRGB encode happens once, on store to the
    /// sRGB render target.
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
}
