//! THE SCENE FORMAT A/B SWITCH (HDR scene target, increment 3, 2026-09-27;
//! docs/design/hdr-scene-target.md). A DEV switch: the rig's showcase key
//! `scene_format` ("display" or "hdr"). Delete it with `present_direct` at
//! increment 6 at the latest.
//!
//! WHY IT EXISTS. Increment 3 changes the scene target from the display's
//! 8-bit format to `Rgba16Float`, and the claim to prove is "only the
//! intermediate quantisation disappears": at most about two codes anywhere
//! except star cores. Two boots of one exe do not park on identical frames at
//! most vantages (the terrain and the sky move between consecutive parked
//! captures), so a paired-boot diff drowns a one-code effect. This switch
//! rebuilds the scene target and EVERY scene pipeline in the other format in
//! place, so `scripts/probe-hot-ab.js` in `same_park` mode can take
//! display, hdr, display, hdr at ONE park and separate the switch's own
//! pixels from the scene's motion (the method of increments 1 and 2).
//!
//! WHAT IT REBUILDS: exactly the builder list `tests/scene_format_lint.rs`
//! keeps, less the star sky, which lives on the engine state and is rebuilt
//! by the caller (engine/ipc.rs, through `world_load::build_star_sky`). Every
//! pass here owns its layouts and makes its bind groups per frame, except the
//! particle frame group, which is remade below against the new layout, and
//! the megashader PSOs, which are swapped the way the shader hot reload
//! swaps them (same layouts, every live bind group intact).
//!
//! It costs the megashader compile (a few seconds, like a hot reload) and
//! blocks that frame; the rig waits for it.

use super::{bloom, cloud_composite, godrays, line, particles, scene_target, shader_loader, ssao, Renderer};

impl Renderer {
    /// Rebuild the scene target and every renderer-owned scene pipeline in
    /// the display format (`hdr == false`, increments 1 and 2) or in the
    /// increment-3 format. Returns whether anything changed; the caller must
    /// then rebuild the star sky, which is built for the old format.
    pub fn switch_scene_format(&mut self, hdr: bool) -> bool {
        let display = self.config.format; // display-format: the A/B switch's display arm draws the scene in it
        let scene_format = if hdr { scene_target::scene_format_for(display) } else { display };
        if scene_format == self.scene.format() {
            return false;
        }
        let t0 = std::time::Instant::now();
        let (w, h) = self.scene.size();
        // The clamp flag follows the formats (off when they are equal, so the
        // display arm's present stays the bit-exact passthrough).
        self.present.set_scene_format(&self.queue, scene_format);
        self.scene = scene_target::SceneTarget::new(&self.device, &self.present, w, h, scene_format);
        // A parked view scratch is in the old format; the next view makes one.
        self.view_scene.spare = None;
        if self.bloom.is_some() {
            self.bloom = Some(bloom::BloomPass::new(&self.device, w, h, scene_format));
        }
        self.godrays = godrays::GodrayPass::new(&self.device, scene_format);
        self.ssao = ssao::SsaoPass::new(&self.device, scene_format);
        self.cloud_composite = cloud_composite::CloudCompositePass::new(&self.device, scene_format);
        let (alpha, additive, frame_bgl) =
            particles::build_particle_pipelines(&self.device, scene_format, &self.pipeline.camera_bind_group_layout);
        self.particle_pipeline_alpha = alpha;
        self.particle_pipeline_additive = additive;
        // The new pipelines carry their own frame layout, so the frame bind
        // group is remade against it (same buffer, same single entry).
        self.particle_frame_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Frame BG (scene format A/B)"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.particle_frame_buffer.as_entire_binding() }],
        });
        self.line_pipeline = line::build_line_pipeline(&self.device, scene_format, &self.pipeline.camera_bind_group_layout);
        // The megashader family, from the source this boot builds from (the
        // disk parts under HUMANITY_SHADERS_FROM_DISK, as the rig runs).
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr megashader (scene format A/B)"),
            source: wgpu::ShaderSource::Wgsl(shader_loader::boot_pbr_source()),
        });
        let batch_module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr megashader (terrain-batch, scene format A/B)"),
            source: wgpu::ShaderSource::Wgsl(shader_loader::boot_pbr_batch_source().into()),
        });
        let rebuilt = self.pipeline.recreate_pipelines(&self.device, scene_format, &module, &batch_module);
        log::info!(
            "[SceneFormat] A/B: scene target and pipelines rebuilt in {:?} ({} PSOs from the megashader) in {:.1} s",
            scene_format,
            rebuilt.total(),
            t0.elapsed().as_secs_f32()
        );
        true
    }
}
