//! The outline on a dark theme (2026-11-20, docs/design/emoji.md "The outline").
//!
//! OpenMoji draws every picture with black lines, and on a dark ground they vanish: in the chat's
//! GPU snapshot the faces lost their edge and the seedling showed as two green dashes. The operator
//! (2026-11-20) asked for "a faint light halo behind each emoji on dark themes, as a theme setting
//! so it can be adjusted or turned off". Two theme tokens hold it (`data/gui/theme.ron`
//! `emoji_outline`, its colour, and `emoji_outline_width`, in points, 0 = off), editable in Settings
//! and copied to the website's `theme.css` (`--emoji-outline`, `--emoji-outline-width`).
//!
//! Because the desktop rasterises each picture once (`pack.rs`), the outline is drawn INTO the
//! picture: the picture's alpha silhouette grown by the width, filled with the colour, laid beneath
//! the picture. It costs nothing per frame. The outline is part of a kept picture's key, so a theme
//! change draws the pictures again. The website does the same three steps with an SVG filter
//! (`web/shared/emoji.js`: feMorphology on the alpha, feFlood and feComposite, feMerge under the
//! picture). One difference, chosen: the browser's feMorphology grows by a square of whole pixels,
//! and this grows by a disc with an anti-aliased edge and fractional widths, which keeps the halo
//! round at large sizes; at chat sizes (a 1 pt width, 1 to 2 pixels) the two cover the same ring
//! (`outline_tests.rs` compares this with resvg drawing the website's own filter).
//!
//! The width is in points (CSS pixels on the website), times the screen's scale, so it is the same
//! on the screen whatever the emoji's size, as the website's is.

/// The theme's outline as the text that shows emoji reads it: its colour (sRGB, unmultiplied) and
/// its width in points. Put on each egui context by `Theme::apply_to_egui` ([`set_tokens`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub rgba: [u8; 4],
    pub width_pt: f32,
}

impl Tokens {
    pub const NONE: Tokens = Tokens { rgba: [0; 4], width_pt: 0.0 };

    /// From the theme's colour (0 to 1 floats, as theme.ron writes them) and width in points.
    pub fn from_theme(colour: (f32, f32, f32, f32), width_pt: f32) -> Tokens {
        let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Tokens { rgba: [b(colour.0), b(colour.1), b(colour.2), b(colour.3)], width_pt: if width_pt.is_finite() { width_pt.max(0.0) } else { 0.0 } }
    }

    /// The outline for a picture drawn at `pixels_per_point`.
    pub fn at(&self, pixels_per_point: f32) -> Outline {
        Outline::new(self.rgba, self.width_pt * pixels_per_point)
    }
}

/// Widths are kept in sixteenths of a pixel, so an outline can key a kept picture exactly.
const STEPS: f32 = 16.0;
/// The widest outline drawn, in pixels: far past any chat size (Settings offers 0 to 3 points), and
/// it bounds the work for one picture.
pub const MAX_WIDTH_PX: f32 = 12.0;

/// One picture's outline: its colour and its width in pixels of the drawn picture. Part of the key
/// of a kept picture (`pack.rs`), so a theme change draws the pictures again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Outline {
    pub rgba: [u8; 4],
    /// The width in sixteenths of a pixel; 0 is none.
    width_16: u16,
}

impl Outline {
    pub const NONE: Outline = Outline { rgba: [0; 4], width_16: 0 };

    pub fn new(rgba: [u8; 4], width_px: f32) -> Outline {
        let w = if width_px.is_finite() { width_px.clamp(0.0, MAX_WIDTH_PX) } else { 0.0 };
        let width_16 = (w * STEPS).round() as u16;
        if width_16 == 0 || rgba[3] == 0 {
            return Outline::NONE;
        }
        Outline { rgba, width_16 }
    }

    /// Is there nothing to draw (no width, or a fully transparent colour)?
    pub fn is_none(&self) -> bool {
        self.width_16 == 0 || self.rgba[3] == 0
    }

    pub fn width_px(&self) -> f32 {
        self.width_16 as f32 / STEPS
    }
}

/// Lay the outline beneath the picture in `pixels` (premultiplied RGBA, `w` by `h`, as tiny-skia
/// draws it and egui uploads it): the picture's alpha grown by the outline's width (a disc, its edge
/// anti-aliased), filled with the outline's colour, the picture drawn over it. Nothing changes for
/// [`Outline::NONE`].
pub fn apply(pixels: &mut [u8], w: usize, h: usize, outline: Outline) {
    if outline.is_none() || w == 0 || h == 0 || pixels.len() < w * h * 4 {
        return;
    }
    let disc = disc(outline.width_px());
    let alpha: Vec<f32> = pixels.chunks_exact(4).take(w * h).map(|p| p[3] as f32 / 255.0).collect();
    let [cr, cg, cb, ca] = outline.rgba.map(|c| c as f32 / 255.0);
    let (wi, hi) = (w as i32, h as i32);
    for y in 0..hi {
        for x in 0..wi {
            let i = (y * wi + x) as usize;
            let src_a = alpha[i];
            if src_a >= 1.0 {
                continue; // fully covered by the picture: the outline beneath cannot show
            }
            let mut grown = 0.0f32;
            for &(dx, dy, weight) in &disc {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= wi || ny >= hi {
                    continue;
                }
                let a = alpha[(ny * wi + nx) as usize] * weight;
                if a > grown {
                    grown = a;
                    if grown >= 1.0 {
                        break;
                    }
                }
            }
            if grown <= 0.0 {
                continue;
            }
            // The halo, premultiplied, under the picture (premultiplied "over").
            let halo_a = ca * grown;
            let under = 1.0 - src_a;
            let p = &mut pixels[i * 4..i * 4 + 4];
            let mix = |src: u8, halo: f32| (src as f32 + halo * 255.0 * under).round().clamp(0.0, 255.0) as u8;
            p[0] = mix(p[0], cr * halo_a);
            p[1] = mix(p[1], cg * halo_a);
            p[2] = mix(p[2], cb * halo_a);
            p[3] = mix(p[3], halo_a);
        }
    }
}

/// How much of the pixel `(dx, dy)` away a fully drawn pixel grown by `r` covers, for every
/// neighbour it reaches: the share of that pixel lying within `r` of the drawn pixel's square (the
/// exact disc-shaped growth, measured on an 8 by 8 grid of points in the pixel). Straight
/// neighbours within `r` are covered whole; a diagonal one at `r` = 1 by about pi/4.
fn disc(r: f32) -> Vec<(i32, i32, f32)> {
    const GRID: i32 = 8;
    let reach = (r + 1.0).ceil() as i32;
    let mut out = Vec::with_capacity(((2 * reach + 1) * (2 * reach + 1)) as usize);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let mut inside = 0;
            for sy in 0..GRID {
                for sx in 0..GRID {
                    // A point of the neighbour pixel, and its distance to the square [-0.5, 0.5]².
                    let px = dx as f32 - 0.5 + (sx as f32 + 0.5) / GRID as f32;
                    let py = dy as f32 - 0.5 + (sy as f32 + 0.5) / GRID as f32;
                    let ox = (px.abs() - 0.5).max(0.0);
                    let oy = (py.abs() - 0.5).max(0.0);
                    if ox * ox + oy * oy <= r * r {
                        inside += 1;
                    }
                }
            }
            if inside > 0 {
                out.push((dx, dy, inside as f32 / (GRID * GRID) as f32));
            }
        }
    }
    out
}

fn tokens_id() -> egui::Id {
    egui::Id::new("hos_emoji_outline")
}

/// Give `ctx` the theme's outline for every picture it draws from now on (`Theme::apply_to_egui`,
/// which runs at start-up, after an edit in Settings, and every frame for a wall screen).
pub fn set_tokens(ctx: &egui::Context, tokens: Tokens) {
    ctx.data_mut(|d| d.insert_temp(tokens_id(), tokens));
}

/// The outline `ctx` draws pictures with: none until a theme gave it one. Never call it inside
/// `ctx.fonts`.
pub fn tokens(ctx: &egui::Context) -> Tokens {
    ctx.data(|d| d.get_temp::<Tokens>(tokens_id())).unwrap_or(Tokens::NONE)
}
