//! Headless UI snapshots (v0.495). Renders native egui pages to PNG images
//! offscreen, WITHOUT opening a window, so UI changes can be reviewed (and
//! regression-checked) from an image rather than only by a human at the app.
//!
//! It drives the app's OWN egui + egui-wgpu + wgpu against an offscreen texture,
//! so there is no extra dependency. The PNGs land in `tests/snapshots/`.
//! Generate / refresh them with `just snapshots`, then open the PNGs to review
//! the UI. (These currently GENERATE the images; pixel-diff regression checking
//! is a later add.)
//!
//! Note: this needs a GPU adapter (the dev machine has one). On a headless CI box
//! without a GPU the render is skipped with a printed note rather than failing.
#![cfg(all(test, feature = "native"))]

use crate::gui::theme::{load_theme, Theme};
use crate::gui::GuiState;

/// Render one settings-style page into an offscreen `w`x`h` surface and write
/// `tests/snapshots/<name>.png`.
fn render_page_png(name: &str, w: u32, h: u32, frame: impl Fn(&egui::Context, &Theme, &mut GuiState)) {
    pollster::block_on(async move {
        // ── wgpu device (offscreen) ──
        let instance = wgpu::Instance::default();
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        {
            Some(a) => a,
            None => {
                eprintln!("ui_snapshots: no GPU adapter; skipping {name}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("request_device");
        let format = wgpu::TextureFormat::Rgba8Unorm;

        // ── egui frame ──
        let ctx = egui::Context::default();
        let theme = load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        let ppp = 1.0_f32;
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w as f32, h as f32),
            )),
            ..Default::default()
        };
        let full_output = ctx.run(raw_input, |ctx| {
            frame(ctx, &theme, &mut state);
        });
        let clipped = ctx.tessellate(full_output.shapes, ppp);

        // ── egui-wgpu renderer ──
        let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        for (id, delta) in &full_output.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: ppp,
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.update_buffers(&device, &queue, &mut encoder, &clipped, &screen);

        // ── offscreen target ──
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui_snapshot"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = theme.bg_primary();
        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui_snapshot_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: srgb_to_lin(bg.r()),
                                g: srgb_to_lin(bg.g()),
                                b: srgb_to_lin(bg.b()),
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            renderer.render(&mut rpass, &clipped, &screen);
        }

        // ── copy texture -> buffer (rows padded to 256) ──
        let bpr = ((w * 4 + 255) / 256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_snapshot_readback"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);

        // ── map + unpad + save ──
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bpr) as usize;
            pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        drop(data);
        buffer.unmap();

        std::fs::create_dir_all("tests/snapshots").ok();
        let img = image::RgbaImage::from_raw(w, h, pixels).expect("image from pixels");
        let path = format!("tests/snapshots/{name}.png");
        img.save(&path).expect("save png");
        println!("ui_snapshots: wrote {path}");
    });
}

/// egui clear colors are sRGB bytes; the Rgba8Unorm target wants linear floats.
fn srgb_to_lin(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// A settings sub-panel (which expects a `&mut Ui`) wrapped into a full
/// ctx-level frame the renderer can drive.
fn settings_panel(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut GuiState,
    draw: impl Fn(&mut egui::Ui, &Theme, &mut GuiState),
) {
    theme.apply_to_egui(ctx);
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            draw(ui, theme, state);
        });
    });
}

#[test]
fn snapshot_audio_settings() {
    render_page_png("audio_settings", 960, 1100, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_audio_content);
    });
}

#[test]
fn snapshot_graphics_settings() {
    render_page_png("graphics_settings", 960, 900, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_graphics_content);
    });
}

#[test]
fn snapshot_controls_settings() {
    render_page_png("controls_settings", 960, 900, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_controls_content);
    });
}

#[test]
fn snapshot_laws_page() {
    render_page_png("laws_page", 1000, 1300, |ctx, theme, state| {
        crate::gui::pages::laws::draw(ctx, theme, state);
    });
}
