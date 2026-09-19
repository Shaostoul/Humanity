//! POST-PASSES: thin lines and particle billboards drawn ON TOP of a frame
//! that has already been rendered.
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1319) under the file-size
//! ratchet, which mod.rs had outgrown badly: 5,652 lines against a 3,883
//! budget. Last of four clusters out in that pass.
//!
//! WHY THIS CLUSTER. Every function here has the same shape, and it is a
//! shape none of the main passes have: build a TRANSIENT per-frame vertex or
//! instance buffer, open a render pass that LOADS the existing colour and
//! depth, depth-TEST against the scene without writing depth, draw, submit.
//! Nothing here owns a persistent buffer except the GPU particle pool, and
//! nothing here is on the critical path of the picture: drop the whole file
//! and the world still renders, minus its orbit rings and its rain.
//!
//! WHICH FAR PLANE decides which line function to call, and it is the one
//! thing in here that is easy to get wrong. `draw_lines_onto` uses the
//! gameplay camera, so a line more than a few hundred metres out is clipped;
//! `draw_celestial_lines_onto` uses the celestial far plane, which is what an
//! AU-scale orbit ring needs, and runs BETWEEN the celestial pass and the
//! scene pass so a ring is occluded by a planet but drawn over by the walls
//! of a room. Each function's own doc states its call position.
//!
//! THE CPU AND GPU PARTICLE PATHS SHARE A PIPELINE, deliberately: the compute
//! shader writes the identical vertex layout the CPU path uploads, so the
//! draw does not know or care which produced the data. That is also why
//! `deactivate_gpu_particles` exists and must be called every frame the GPU
//! path is off: stopping the SIM does not stop the DRAW, and without zeroing
//! `live` the last frame's verts hang in the air as rain frozen mid-fall
//! (operator field report, 2026-07-31).
//!
//! NO RE-EXPORT SHIM IS NEEDED, for the same reason the other four extracted
//! files needed none: these are inherent methods on `Renderer`, resolved by
//! receiver type rather than by module path, so every call site in the crate
//! keeps working untouched. The glob import is the arrangement
//! `scene_draw.rs` and `celestial.rs` use: a child module sees its parent's
//! private items and private `use` bindings, so no `Renderer` field has to be
//! widened for the move.

use super::*;

impl Renderer {
    /// Draw world-space thin lines (orbit paths) onto an already-rendered
    /// frame. Call AFTER `render_scene_onto` so the depth buffer holds
    /// the planets — the reverse-Z depth-test (no depth-write) then
    /// occludes any segment passing behind a planet. Same camera as the
    /// scene (full view-proj + floating origin), so lines sit exactly on
    /// the bodies. Transient per-frame vertex buffer (a few thousand
    /// verts — trivial).
    ///
    /// `who`: `gpu.lines` for the live frame, `gpu.screen_lines` for a
    /// camera screen's re-render (see `render_overlay_onto`).
    pub fn draw_lines_onto(
        &self,
        camera: &Camera,
        verts: &[line::LineVertex],
        view: &wgpu::TextureView,
        who: frame_costs::SceneView,
    ) {
        let (gpu_id, cpu_id) = who.lines_ids();
        let _cost = frame_costs::stage(cpu_id);
        if verts.len() < 2 {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("World Line VB"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World Line Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World Line Pass"),
                timestamp_writes: self.pass_timer(gpu_id),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve stars + scene
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the planets
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.line_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.draw(0..verts.len() as u32, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Draw particle billboards onto an already-rendered frame (v0.966).
    /// Same post-pass shape as draw_lines_onto: transient instance buffer,
    /// reverse-Z depth TEST against the scene, no depth write. Billboard
    /// axes come from the camera basis, uploaded to the frame uniform.
    pub fn draw_particles_onto(
        &mut self,
        camera: &Camera,
        alpha: &[particles::ParticleVertexData],
        additive: &[particles::ParticleVertexData],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.particles");
        if alpha.is_empty() && additive.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let right = camera.right();
        let up = right.cross(camera.forward()).normalize_or_zero() * -1.0;
        // right.w carries the scene illumination scalar (v0.1154): particle
        // billboards have no lighting of their own, and an authored-tint
        // raindrop rendered at full brightness at midnight (the operator's
        // "rain glows at night" report). Daylight = 1, night = the floor.
        let frame: [f32; 8] =
            [right.x, right.y, right.z, self.scene_illum(), up.x, up.y, up.z, 0.0];
        self.queue
            .write_buffer(&self.particle_frame_buffer, 0, bytemuck::cast_slice(&frame));
        // Grow-to-high-water-mark, then refill. Reallocating only when the
        // count exceeds the previous peak means a steady downpour allocates
        // once and then never again.
        let mut ensure = |buf: &mut Option<wgpu::Buffer>,
                          cap: &mut usize,
                          data: &[particles::ParticleVertexData],
                          label: &str| {
            if data.is_empty() {
                return;
            }
            if buf.is_none() || *cap < data.len() {
                // Round up so a slowly-growing storm does not reallocate every
                // frame on the way up.
                let want = (data.len() * 3 / 2).max(4096);
                *buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: (want * std::mem::size_of::<particles::ParticleVertexData>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                *cap = want;
            }
            if let Some(b) = buf.as_ref() {
                self.queue.write_buffer(b, 0, bytemuck::cast_slice(data));
            }
        };
        let mut vb_a = self.particle_vb_alpha.take();
        let mut vb_b = self.particle_vb_additive.take();
        let mut cap_a = self.particle_vb_alpha_cap;
        let mut cap_b = self.particle_vb_additive_cap;
        ensure(&mut vb_a, &mut cap_a, alpha, "Particle VB (alpha)");
        ensure(&mut vb_b, &mut cap_b, additive, "Particle VB (additive)");
        let vb_alpha = (!alpha.is_empty()).then(|| vb_a.as_ref().expect("ensured")).cloned();
        let vb_add = (!additive.is_empty()).then(|| vb_b.as_ref().expect("ensured")).cloned();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Particle Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Pass"),
                timestamp_writes: self.pass_timer("gpu.particles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_bind_group(1, &self.particle_frame_bind_group, &[]);
            if let Some(vb) = &vb_alpha {
                rp.set_pipeline(&self.particle_pipeline_alpha);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.draw(0..4, 0..alpha.len() as u32);
            }
            if let Some(vb) = &vb_add {
                rp.set_pipeline(&self.particle_pipeline_additive);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.draw(0..4, 0..additive.len() as u32);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.particle_vb_alpha = vb_a;
        self.particle_vb_additive = vb_b;
        self.particle_vb_alpha_cap = cap_a;
        self.particle_vb_additive_cap = cap_b;
    }

    /// Stop drawing the GPU precipitation pool. Called every frame the GPU
    /// path is inactive (Clear condition, altitude gate closed, setting off):
    /// the sim dispatch stopping does NOT stop the draw, so without zeroing
    /// `live` the last-written verts render forever as rain frozen mid-air
    /// (operator field report 2026-07-31).
    pub fn deactivate_gpu_particles(&mut self) {
        if let Some(g) = self.gpu_particles.as_mut() {
            g.live = 0;
        }
    }

    /// Advance the GPU particle pool, creating it on first use (v0.1068).
    /// `live` is how many slots to simulate this frame; the pool itself only
    /// grows, so dialling density up and down costs nothing after the peak.
    pub fn simulate_gpu_particles(
        &mut self,
        params: particles_gpu::SimParams,
        live: u32,
        capacity_hint: u32,
    ) {
        // Named to pair with the compute pass's `gpu.particles_sim`, so the
        // no-timestamp fallback (`gpu.x` reads `cpu.x`) finds it.
        let _cost = frame_costs::stage("cpu.particles_sim");
        if self.gpu_particles.is_none()
            || self
                .gpu_particles
                .as_ref()
                .is_some_and(|g| g.capacity() < capacity_hint)
        {
            self.gpu_particles = Some(particles_gpu::GpuParticles::new(
                &self.device,
                capacity_hint.max(live),
            ));
        }
        // The timers are read from the FIELD, not through `compute_pass_timer`
        // (a `&self` method), because `gpu_particles` is borrowed mutably on
        // the same line: disjoint field borrows are fine, a whole-self borrow
        // beside a field borrow is not. The slot pair itself is claimed
        // INSIDE `simulate`, past its `live == 0` early return, so a frame
        // with no live particles never resolves a pair no pass wrote.
        let timers = self.gpu_timers.as_deref();
        if let Some(g) = self.gpu_particles.as_mut() {
            g.simulate(&self.device, &self.queue, params, live, timers);
        }
    }

    /// Draw the GPU-simulated pool. Deliberately the SAME pipeline and the same
    /// instanced quad the CPU path uses - the compute shader wrote the identical
    /// vertex layout, so nothing here knows or cares where the data came from.
    pub fn draw_gpu_particles_onto(&self, camera: &Camera, view: &wgpu::TextureView) {
        let _cost = frame_costs::stage("cpu.gpu_particles");
        let Some(g) = self.gpu_particles.as_ref() else {
            return;
        };
        if g.live == 0 {
            return;
        }
        // Same billboard basis the CPU path uses - see draw_particles_onto.
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );
        let right = camera.right();
        let up = right.cross(camera.forward()).normalize_or_zero() * -1.0;
        // right.w carries the scene illumination scalar (v0.1154): particle
        // billboards have no lighting of their own, and an authored-tint
        // raindrop rendered at full brightness at midnight (the operator's
        // "rain glows at night" report). Daylight = 1, night = the floor.
        let frame: [f32; 8] =
            [right.x, right.y, right.z, self.scene_illum(), up.x, up.y, up.z, 0.0];
        self.queue
            .write_buffer(&self.particle_frame_buffer, 0, bytemuck::cast_slice(&frame));
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU Particle Encoder"),
            });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GPU Particle Pass"),
                timestamp_writes: self.pass_timer("gpu.gpu_particles"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_bind_group(1, &self.particle_frame_bind_group, &[]);
            rp.set_pipeline(&self.particle_pipeline_alpha);
            rp.set_vertex_buffer(0, g.vertex_buf.slice(..));
            rp.draw(0..4, 0..g.live);
        }
        self.queue.submit(std::iter::once(enc.finish()));
    }

    /// Orbit paths drawn with the CELESTIAL far plane (v0.451) so the AU-scale rings
    /// are not clipped by the gameplay far (~500 m) the way `draw_lines_onto` clips them.
    /// Call BETWEEN `render_celestial_onto` and `render_scene_onto`: it loads the
    /// celestial depth (so a ring passing behind a planet is occluded by that body) and
    /// the interior scene then clears depth + draws OVER the rings where home geometry
    /// exists (walls occlude the sky-rings). Same transient-VB approach as `draw_lines_onto`.
    pub fn draw_celestial_lines_onto(
        &self,
        camera: &Camera,
        verts: &[line::LineVertex],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.celestial_lines");
        if verts.len() < 2 {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera.celestial_uniforms()),
        );
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Celestial Line VB"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Celestial Line Encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Line Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial_lines"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve stars + bodies
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the celestial bodies
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.line_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_vertex_buffer(0, vbuf.slice(..));
            rp.draw(0..verts.len() as u32, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
