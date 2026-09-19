//! The SCENE passes: the draw loops that put the near world (the ship, the
//! home, props, vegetation, water, holograms) on a target, plus the two
//! screen-space passes that finish that image.
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1319) under the file-size
//! ratchet, which mod.rs had outgrown badly: 5,652 lines against a 3,883
//! budget. Second of four clusters out in that pass.
//!
//! WHY THIS CLUSTER. Read it top to bottom and it is one story: build the
//! per-object uniforms (`upload_object_uniforms`), then draw the opaque list
//! class by class (`draw_opaque_objects`), then the same onto a caller's
//! target (`render_scene_onto`), then the transparent list over it
//! (`render_transparent_onto`), then the overlay slot that ignores depth
//! (`render_overlay_onto`), then god rays and SSAO over the finished colour
//! (`render_godrays_onto`, `render_ssao_onto`). `render_scene` and `render`
//! are the whole-frame convenience wrappers around that sequence. Nothing
//! here configures a device, registers a material or knows what a planet is.
//!
//! THE CELESTIAL PASS IS NOT HERE, on purpose. It draws the far world at
//! planet scale with its own far plane, its own uniform packing and its own
//! shadow and cloud work, and it is long enough to be its own file:
//! `renderer/celestial.rs`. The one rule that spans the two files is the
//! ORDER the frame loop calls them in, which each function's own doc states.
//!
//! TWO CLASS RULES LIVE IN `draw_opaque_objects` and are the reason it is a
//! loop over classes rather than a loop over objects. Every megashader PSO
//! compiles ONE fragment class entry (see `pipeline.rs` `ShaderClass` and
//! `PSO_REGISTRY`), so a draw must pick its pipeline by the object's class or
//! it renders through an entry that has no branch for its material type -
//! visibly wrong, with no error in release. And a shell, cloud or water
//! material in an OPAQUE list is a caller bug, which the debug assert names
//! rather than letting it pass silently.
//!
//! NO RE-EXPORT SHIM IS NEEDED, for the same reason `materials.rs`,
//! `capture.rs` and `surface.rs` needed none: these are inherent methods on
//! `Renderer`, resolved by receiver type rather than by module path, so every
//! call site in the crate keeps working untouched.
//!
//! The glob import below is deliberate and is the arrangement
//! `tree_mesh`/`tree_species` already uses: a child module can see its
//! parent's private items AND its parent's private `use` bindings, so the
//! ~20 names this file needs (`Camera`, `RenderObject`, `MAX_OBJECTS`,
//! `ObjectUniforms`, `ShaderClass`, `frame_costs`, the glam types, ...)
//! arrive through one line instead of a list that would go stale on every
//! edit, and no `Renderer` field has to be widened to `pub(super)`.

use super::*;

impl Renderer {
    /// Batched object-uniform upload (v0.891): build every per-object uniform
    /// block in ONE staging vec and issue ONE queue.write_buffer, instead of a
    /// queue call per object. At 3000+ terrain patches the per-call overhead
    /// (per-call validation + copy scheduling) dominated CPU frame time.
    /// `pub(super)` only because the celestial pass (celestial.rs) and the
    /// instanced path (mod.rs) upload their own object blocks through it: it
    /// was private while all three lived in one file, and this is one of the
    /// two privacy widenings the v0.1319 move cost.
    pub(super) fn upload_object_uniforms<'a>(&self, objects: impl Iterator<Item = &'a RenderObject>) {
        const ALIGN: usize = 256;
        let mut staging: Vec<u8> = Vec::with_capacity(ALIGN * 1024);
        for (i, obj) in objects.enumerate() {
            if i >= MAX_OBJECTS {
                break;
            }
            let clean =
                Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position);
            // Normal matrix from the CLEAN transform - the fade smuggled into
            // the w row below would corrupt the inverse.
            let normal_matrix = clean.inverse().transpose();
            // LOD crossfade (v0.920) rides model[0].w; the vertex shader
            // rebuilds the homogeneous w after transforming, so this slot is
            // free per-object metadata (see RenderObject::fade).
            let mut model = clean;
            model.x_axis.w = obj.fade;
            let uniforms = ObjectUniforms {
                model: model.to_cols_array_2d(),
                normal_matrix: normal_matrix.to_cols_array_2d(),
            };
            // Pad the previous slot out to the 256-byte dynamic-offset
            // alignment, then append this 128-byte block.
            staging.resize(i * ALIGN, 0);
            staging.extend_from_slice(bytemuck::bytes_of(&uniforms));
        }
        if !staging.is_empty() {
            self.queue.write_buffer(&self.object_buffer, 0, &staging);
        }
    }

    /// Render a frame with the given camera and objects.
    pub fn render(&self, camera: &Camera, objects: &[RenderObject]) -> Result<(), wgpu::SurfaceError> {
        let (output, _view) = self.render_scene(camera, objects)?;
        output.present();
        Ok(())
    }

    /// Render the 3D scene and return the surface texture + view for further
    /// overlay rendering (e.g., egui). Caller must call `output.present()`
    /// after all overlay passes are complete.
    pub fn render_scene(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        // Update camera uniforms
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                timestamp_writes: self.pass_timer("gpu.scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            self.upload_object_uniforms(objects.iter());

            // The list may carry surfaces, vegetation and terrain (P3): one
            // pass per class present, each through its own class PSO.
            self.draw_opaque_objects(&mut render_pass, objects, &(0..0), "scene opaque pass");
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok((output, view))
    }

    /// Draw an OPAQUE object list through the class PSOs (increment P3 of
    /// the frame-cost arc): one walk of the list per material class it
    /// carries, in `pipeline::OPAQUE_CLASS_ORDER`, each walk on that class's
    /// own PSO (`Pipeline::opaque_for`). Shared by the live frame, the
    /// camera-screen re-render and the celestial pass's classic loop.
    ///
    /// Why walk per class rather than switch pipelines in list order: the
    /// lists interleave classes at fine grain (a procedural near tree is a
    /// bark part, vegetation, next to a photoscan's stem, a surface; a
    /// garden bed is a planter next to its plant), so switching on every
    /// class change would rebind a pipeline hundreds of times per frame.
    /// One walk per class binds each PSO once. The walks cost a cheap scan
    /// of the list per class (a material lookup and one float compare per
    /// object), microseconds against the draw encoding itself, and nothing
    /// in lib.rs has to sort by class. Within a class the list order is
    /// kept exactly (the lists are built roughly front to back), and across
    /// classes opaque draws with depth test and write resolve to the same
    /// image in any order, so the class order is a cost choice only (see
    /// `OPAQUE_CLASS_ORDER`).
    ///
    /// Object uniforms must already be staged (`upload_object_uniforms`) so
    /// the dynamic offset of object `i` is `256 * i`. Group 0 and the pass's
    /// own state are the caller's; this binds slot 1 (the classic dummy
    /// instance buffer) after every pipeline switch, the belt-and-braces the
    /// v0.1060 water switch uses.
    ///
    /// A shell, cloud or water material in an opaque list is a caller bug:
    /// debug builds stop here naming the site, release builds draw it
    /// through the surface PSO (`opaque_for` falls back), whose entry has
    /// no dispatch for the type, so it renders as the default look,
    /// visibly wrong and never silently absent.
    /// `pub(super)` only because the celestial pass (celestial.rs) draws its
    /// planet-scale opaque list through the same loop. Private before the
    /// v0.1319 split; see `upload_object_uniforms` above.
    pub(super) fn draw_opaque_objects(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        objects: &[RenderObject],
        colour_skip: &std::ops::Range<usize>,
        site: &str,
    ) {
        let uniform_align = 256_u64;
        // Which classes the list carries, one cheap scan: a class with no
        // object costs no pipeline bind and no walk. Indexed by the class's
        // position in `ShaderClass::ALL` (its discriminant).
        let mut present = [false; ShaderClass::ALL.len()];
        for obj in objects.iter().take(MAX_OBJECTS) {
            if let Some(material) = self.materials.get(obj.material) {
                present[pipeline::shader_class(material.material_type) as usize] = true;
            }
        }
        // The opaque classes first, in their order; then any class that
        // should never be here, so it is drawn (wrongly, visibly) rather
        // than dropped.
        let stray = ShaderClass::ALL.iter().copied().filter(|c| !c.draws_opaque());
        for class in pipeline::OPAQUE_CLASS_ORDER.into_iter().chain(stray) {
            if !present[class as usize] {
                continue;
            }
            debug_assert!(
                class.draws_opaque(),
                "{site}: a {class:?}-class material is in an OPAQUE draw list; shells, clouds \
                 and water must be pushed to the transparent list (lib.rs celestial_transparent)"
            );
            pass.set_pipeline(self.pipeline.opaque_for(class));
            pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            let mut bound_material = usize::MAX;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS {
                    break;
                }
                // Shadow-only objects (V1): drawn by the sun-shadow pass,
                // which walks the whole list, never by this colour pass.
                if colour_skip.contains(&i) {
                    continue;
                }
                let Some(mesh) = self.meshes.get(obj.mesh) else { continue };
                let Some(material) = self.materials.get(obj.material) else { continue };
                if pipeline::shader_class(material.material_type) != class {
                    continue;
                }
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }

    /// Render 3D objects onto an already-acquired surface texture.
    /// Uses LoadOp::Load to preserve existing content (e.g. stars rendered first).
    ///
    /// `who` says whose frame this pass is, which decides the cost keys it
    /// is published under: the live window (`gpu.scene`) or a camera
    /// screen's 10 Hz re-render (`gpu.screen_scene`). Same pass, same
    /// pipeline; only the bookkeeping differs, so the Performance page can
    /// show the camera wall as its own number.
    pub fn render_scene_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        view: &wgpu::TextureView,
        who: frame_costs::SceneView,
    ) {
        let (gpu_id, cpu_id) = who.scene_ids();
        let _cost = frame_costs::stage(cpu_id);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Overlay Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Overlay Pass"),
                timestamp_writes: self.pass_timer(gpu_id),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve star background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            self.upload_object_uniforms(objects.iter());

            // The list may carry surfaces, vegetation and terrain (P3): one
            // pass per class present, each through its own class PSO.
            self.draw_opaque_objects(&mut render_pass, objects, &(0..0), "scene opaque pass");
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render TRANSPARENT objects (glass windows, the portal) over the already-drawn scene,
    /// alpha-blended (v0.456). Call AFTER `render_scene_onto`: it preserves the colour
    /// (LoadOp::Load) and LOADS the scene depth (so glass behind a wall is occluded) but does
    /// not WRITE depth (so you see through it). A material's `base_color.a` is its opacity.
    /// `who` picks the cost keys the same way it does for `render_scene_onto`.
    pub fn render_transparent_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        view: &wgpu::TextureView,
        who: frame_costs::SceneView,
    ) {
        let (gpu_id, cpu_id) = who.transparent_ids();
        let _cost = frame_costs::stage(cpu_id);
        if objects.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transparent Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Transparent Pass"),
                timestamp_writes: self.pass_timer(gpu_id),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // blend over the scene
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // test against the opaque scene; no write
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());

            let mut bound_material = usize::MAX;
            // The pipeline is picked by the material's CLASS (P2,
            // pipeline.rs `shader_class`): glass, holograms and particles
            // ride the general transparent PSO, a shell rides the shell or
            // cloud one. Switched only when the class changes, so a list of
            // general objects binds one pipeline once, as before.
            let mut bound_class: Option<ShaderClass> = None;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let class = pipeline::shader_class(material.material_type);
                if bound_class != Some(class) {
                    bound_class = Some(class);
                    render_pass.set_pipeline(self.pipeline.transparent_for(class));
                    // Rebind slot 1 and force the material rebind after a
                    // pipeline switch, the same belt-and-braces the v0.1060
                    // water switch uses.
                    render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    bound_material = usize::MAX;
                }
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render editor GIZMOS on top of everything (v0.560): same as the transparent pass but with the
    /// depth-test-disabled `overlay_pipeline`, so corner orbs / the avatar / rings show THROUGH walls
    /// + floors. Call AFTER `render_transparent_onto`. Reuses the shared object buffer (the prior pass
    /// already drew), so the writes are safe.
    ///
    /// `who` names whose frame this is for the cost keys (`gpu.overlay` for the live frame,
    /// `gpu.screen_overlay` for a camera screen's re-render), the same split `render_scene_onto`
    /// makes; before it a camera wall's overlay draw was summed into the live frame's number.
    pub fn render_overlay_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        view: &wgpu::TextureView,
        who: frame_costs::SceneView,
    ) {
        let (gpu_id, cpu_id) = who.overlay_ids();
        let _cost = frame_costs::stage(cpu_id);
        if objects.is_empty() {
            return;
        }
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.lit_uniform(camera.uniforms())));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Overlay Encoder") });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Pass"),
                timestamp_writes: self.pass_timer(gpu_id),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    // CLEAR depth (reverse-Z far = 0.0) so gizmos ignore the world but still depth-sort
                    // among themselves; the colour is Loaded so they blend over the rendered scene.
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            // One batched object-uniform upload (v0.891).
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter());
            let mut bound_material = usize::MAX;
            // Pipeline by material class (P2), switched only on a class
            // change: the gizmo list is all general, so this binds the
            // general overlay PSO once. See `Pipeline::overlay_for`.
            let mut bound_class: Option<ShaderClass> = None;
            for (i, obj) in objects.iter().enumerate() {
                if i >= MAX_OBJECTS { break; }
                let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                let class = pipeline::shader_class(material.material_type);
                if bound_class != Some(class) {
                    bound_class = Some(class);
                    render_pass.set_pipeline(self.pipeline.overlay_for(class));
                    render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    bound_material = usize::MAX;
                }
                let dynamic_offset = (uniform_align as u32) * (i as u32);
                render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                // Material bind groups (2 + 3) skipped when unchanged
                // (v0.891): terrain patches share one material, so 3000+
                // redundant rebinds per frame collapse to one.
                if bound_material != obj.material {
                    bound_material = obj.material;
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    // Group 3 (v0.811): the material's albedo texture when it
                    // has one (textured planets), the 1x1 white fallback
                    // otherwise -- the shared pipeline layout requires
                    // SOMETHING bound here.
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render CELESTIAL bodies (planet + Sun + solar-system bodies) onto the frame with a
    /// HUGE far plane, so they are not clipped by the gameplay far (~500 m). Call BETWEEN the
    /// star pass and `render_scene_onto`: it preserves the stars (LoadOp::Load color) and
    /// clears its own depth so the bodies depth-sort among themselves; the interior scene then
    /// clears depth again and draws OVER the bodies' color where home geometry exists. (v0.450)
    /// Crepuscular god rays (v0.895): call BETWEEN the celestial pass and
    /// the scene pass, while the shared depth buffer still holds the
    /// terrain + bodies silhouettes (the scene pass clears it right after).
    /// `sun_dir` = world direction TOWARD the sun; the pass skips itself
    /// when the sun projects behind the camera or intensity is 0.
    pub fn render_godrays_onto(
        &self,
        camera: &Camera,
        sun_dir: Vec3,
        view: &wgpu::TextureView,
        weather_scale: f32,
    ) {
        let _cost = frame_costs::stage("cpu.godrays");
        // Settings slider at 0 = pass off entirely (v0.907).
        if self.godray_intensity <= 0.001 {
            return;
        }
        // The SAME projection the celestial pass rendered depth with
        // (reverse-Z, far plane at 1e13) — a mismatched matrix would park
        // the sun uv in the wrong place and bend every shaft.
        let proj = Mat4::perspective_rh(
            camera.fov_degrees.to_radians(),
            camera.aspect,
            1.0e13,
            1.0,
        );
        let view_proj = proj * camera.view_matrix();
        self.godrays.render(
            &self.device,
            &self.queue,
            &self.depth_view,
            view,
            view_proj,
            camera.effective_position(),
            sun_dir,
            camera.aspect,
            self.godray_intensity * weather_scale.clamp(0.0, 1.0),
            // The timers, not a claimed slot: the pass has early returns and
            // claims its own slot only once it knows it will draw (a slot
            // claimed here for a pass that skipped read as a frozen 35 ms).
            self.gpu_timers.as_deref(),
        );
    }

    /// Screen-space ambient occlusion (v0.901): call right after
    /// render_godrays_onto, same celestial slot (depth still holds terrain +
    /// vegetation). Multiplies contact shade into the color target.
    pub fn render_ssao_onto(&self, camera: &Camera, view: &wgpu::TextureView) {
        let _cost = frame_costs::stage("cpu.ssao");
        // Settings slider at 0 = pass off entirely (v0.907).
        if self.ssao_strength <= 0.001 {
            return;
        }
        // The SAME projection the celestial depth was rendered with; its
        // [2][2] / [3][2] elements linearize reverse-Z depth in the shader.
        let proj = Mat4::perspective_rh(
            camera.fov_degrees.to_radians(),
            camera.aspect,
            1.0e13,
            1.0,
        );
        let m = proj.to_cols_array_2d();
        // True focal length in pixels — the shader reconstructs view-space
        // positions from it, so the small-angle px-per-radian approximation
        // is no longer good enough (v0.1100 estimator rebuild, BUG-062).
        let focal_px = self.config.height as f32 * 0.5
            / (camera.fov_degrees.to_radians() * 0.5).tan().max(1.0e-4);
        self.ssao.render(
            &self.device,
            &self.queue,
            &self.depth_view,
            view,
            m[2][2],
            m[3][2],
            focal_px,
            // Contact-AO neighborhood. 0.4 m, not the old 1.6 m: contact
            // shading is a decimetre-scale effect; 1.6 m let every trunk
            // shade ground far behind it (the BUG-062 aura).
            0.4,
            self.ssao_strength,
            // The timers, not a claimed slot: the pass returns early at zero
            // strength and claims its own slot only when it will draw.
            self.gpu_timers.as_deref(),
        );
    }
}
