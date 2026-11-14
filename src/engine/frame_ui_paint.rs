//! Painting the GUI (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the platform output handled, the shapes tessellated,
//! the textures updated and the GUI drawn over the frame.

use crate::engine::frame_prelude::*;

/// Paints the egui pass's output onto `view`.
pub(crate) fn run(state: &mut EngineState, full_output: egui::FullOutput, view: &wgpu::TextureView) {
    humanity_core::frame_costs::span("cpu.span.ui_paint");
    state.egui_state.handle_platform_output(&state.window, full_output.platform_output);

    // Main-UI dev IPC "find" (2026-09-18): answer a pending
    // text lookup from THIS frame's shapes, before tessellation
    // consumes them. A no-op unless a find is pending.
    crate::engine::ipc::ui_request_scan_shapes(state, &full_output.shapes);

    // Tessellate and render egui
    let paint_jobs = state.egui_ctx.tessellate(
        full_output.shapes,
        full_output.pixels_per_point,
    );

    // Handle egui texture updates
    for (id, image_delta) in &full_output.textures_delta.set {
        state.egui_renderer.update_texture(
            &state.renderer.device,
            &state.renderer.queue,
            *id,
            image_delta,
        );
    }

    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [
            state.renderer.surface_size().0,
            state.renderer.surface_size().1,
        ],
        pixels_per_point: full_output.pixels_per_point,
    };

    // Render egui overlay on top of the 3D scene.
    // Use two encoders: one for buffer updates, one for rendering.
    // This avoids lifetime issues with wgpu 24's render pass borrows.
    {
        let mut encoder = state.renderer.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("egui Buffer Update"),
            },
        );

        state.egui_renderer.update_buffers(
            &state.renderer.device,
            &state.renderer.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        state.renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    {
        let mut encoder = state.renderer.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("egui Render"),
            },
        );

        // SAFETY: The render pass is created, used, and dropped
        // within this block before encoder.finish(). The 'static
        // lifetime on egui_wgpu::Renderer::render() is overly
        // conservative. The render pass does not actually outlive
        // the encoder since we drop it before calling finish().
        let render_pass = unsafe {
            std::mem::transmute::<
                wgpu::RenderPass<'_>,
                wgpu::RenderPass<'static>,
            >(encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: Some("egui Render Pass"),
                    timestamp_writes: state.renderer.pass_timer("gpu.ui"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    ..Default::default()
                },
            ))
        };
        let mut render_pass = render_pass;

        state.egui_renderer.render(
            &mut render_pass,
            &paint_jobs,
            &screen_descriptor,
        );
        drop(render_pass);

        state.renderer.queue.submit(std::iter::once(encoder.finish()));
    }
    // `cpu.frame_total` does NOT close here: the egui submit
    // is not the frame's last. The screenshot captures, the
    // weather upload and the live broadcast's capture copy
    // (`pump_live_broadcast`, its own submit) all follow, so
    // the total is recorded just before `present()` below.

    // Free egui textures that are no longer needed
    for id in &full_output.textures_delta.free {
        state.egui_renderer.free_texture(id);
    }

    // This frame's draw lists, borrowed for the (rare) hi-res
    // capture path -- zero cost when no capture is pending. (v0.810)
}
