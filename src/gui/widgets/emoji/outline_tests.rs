// The outline on a dark theme (2026-11-20, docs/design/emoji.md "The outline"): measured on the
// drawn pixels of the pack's own pictures. Each test was seen failing once on purpose; the break
// and what it said are recorded at the test.

use std::sync::Arc;

use egui::ColorImage;

use super::outline::{self, Outline, Tokens};
use super::pack::{rasterise, PackPictures};

fn pack_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("assets").join("emoji")
}

fn svg(file: &str) -> Vec<u8> {
    std::fs::read(pack_dir().join("openmoji").join(format!("{file}.svg"))).expect("the pack's picture")
}

/// The shipped theme's outline (data/gui/theme.ron), the one the desktop draws with.
fn shipped_tokens() -> Tokens {
    crate::gui::theme::load_theme().emoji_outline_tokens()
}

/// The pixels outside the picture's own silhouette (fully transparent without the outline) that
/// the outline covers, and their mean colour un-premultiplied, from the same picture drawn
/// without and with it.
fn ring(plain: &ColorImage, outlined: &ColorImage) -> (usize, [f32; 3], f32) {
    let (mut n, mut sum, mut alpha) = (0usize, [0f32; 3], 0f32);
    for (p, o) in plain.pixels.iter().zip(&outlined.pixels) {
        if p.a() == 0 && o.a() > 0 {
            n += 1;
            let a = o.a() as f32;
            sum[0] += o.r() as f32 * 255.0 / a;
            sum[1] += o.g() as f32 * 255.0 / a;
            sum[2] += o.b() as f32 * 255.0 / a;
            alpha += a / 255.0;
        }
    }
    let k = n.max(1) as f32;
    (n, [sum[0] / k, sum[1] / k, sum[2] / k], alpha)
}

/// The pixels where the picture itself is fully opaque.
fn opaque(image: &ColorImage) -> Vec<usize> {
    image.pixels.iter().enumerate().filter(|(_, p)| p.a() == 255).map(|(i, _)| i).collect()
}

/// A DARK-OUTLINED PICTURE GETS THE OUTLINE'S COLOUR ROUND IT ON THE DARK THEME, AND NONE WHEN THE
/// TOKEN IS OFF: the seedling and the grinning face (OpenMoji's black lines, the two the chat
/// snapshot lost on its dark ground) drawn at the chat's size with the shipped theme's outline have
/// a ring of pixels outside their silhouette in the outline's colour, the picture's own opaque
/// pixels are unchanged, and with the width at 0 or a fully transparent colour the drawn picture is
/// exactly the plain one.
///
/// Seen red 2026-11-20 with `outline::apply` taken out of `rasterise`: "seedling: the outline
/// covers a ring round the picture (0 pixels)".
#[test]
fn a_dark_outlined_picture_gets_the_outline_colour_round_it_and_none_when_off() {
    let tokens = shipped_tokens();
    assert!(tokens.width_pt > 0.0 && tokens.rgba[3] > 0, "the shipped (dark) theme has the outline on: {tokens:?}");
    for (file, what) in [("1F331", "seedling"), ("1F600", "grinning face")] {
        let bytes = svg(file);
        let px = 24;
        let plain = rasterise(&bytes, px, Outline::NONE).expect("draws");
        let outlined = rasterise(&bytes, px, tokens.at(1.0)).expect("draws");
        let (n, colour, _) = ring(&plain, &outlined);
        assert!(n >= 40, "{what}: the outline covers a ring round the picture ({n} pixels)");
        let want = [tokens.rgba[0] as f32, tokens.rgba[1] as f32, tokens.rgba[2] as f32];
        for c in 0..3 {
            assert!((colour[c] - want[c]).abs() <= 6.0, "{what}: the ring is the outline's colour: {colour:?}, the token's {want:?}");
        }
        for i in opaque(&plain) {
            assert_eq!(plain.pixels[i], outlined.pixels[i], "{what}: the picture's own opaque pixels are unchanged (pixel {i})");
        }
        // Off: the width at 0, or a colour with no opacity, draws exactly the plain picture.
        let off_width = Tokens { width_pt: 0.0, ..tokens };
        let off_colour = Tokens { rgba: [tokens.rgba[0], tokens.rgba[1], tokens.rgba[2], 0], ..tokens };
        for off in [off_width, off_colour] {
            assert_eq!(off.at(1.0), Outline::NONE, "{what}: {off:?} is no outline");
            assert!(rasterise(&bytes, px, off.at(1.0)).expect("draws").pixels == plain.pixels, "{what}: with the token off the picture is exactly the plain one");
        }
    }
}

/// Draw `file` as the website does: the picture in the filter emoji.js builds (feMorphology dilate
/// on the alpha by `radius`, feFlood in the colour, feComposite in, feMerge under the picture,
/// sRGB), here drawn by resvg, which follows the SVG filter specification as the browsers do.
fn as_the_website_draws(file: &str, px: u32, rgba: [u8; 4], radius: f32) -> ColorImage {
    let inner = String::from_utf8(svg(file)).expect("text");
    let inner = match inner.find("<svg") {
        Some(at) => inner[at..].to_string(),
        None => inner,
    };
    let page = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{px}\" height=\"{px}\" viewBox=\"0 0 {px} {px}\">\
         <defs><filter id=\"o\" x=\"-25%\" y=\"-25%\" width=\"150%\" height=\"150%\" color-interpolation-filters=\"sRGB\">\
         <feMorphology in=\"SourceAlpha\" operator=\"dilate\" radius=\"{radius}\" result=\"grown\"/>\
         <feFlood flood-color=\"rgb({}, {}, {})\" flood-opacity=\"{}\" result=\"ink\"/>\
         <feComposite in=\"ink\" in2=\"grown\" operator=\"in\" result=\"halo\"/>\
         <feMerge><feMergeNode in=\"halo\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter></defs>\
         <g filter=\"url(#o)\">{inner}</g></svg>",
        rgba[0],
        rgba[1],
        rgba[2],
        rgba[3] as f32 / 255.0
    );
    rasterise(page.as_bytes(), px, Outline::NONE).expect("the website's filter draws")
}

/// THE DESKTOP'S OUTLINE LOOKS LIKE THE WEBSITE'S: the same pictures, size and token drawn both
/// ways (the desktop's grown disc, and the website's SVG filter drawn by resvg) cover about the same
/// ring with the same colour. The browser's feMorphology grows by a square of whole pixels and the
/// desktop by a disc with a soft edge, so the two are compared by how much outline each lays round
/// the picture (the summed opacity of the ring), not pixel for pixel. Prints both numbers.
///
/// Seen red 2026-11-20 on the first draft of the desktop's disc, which covered a neighbour at
/// distance d by `r + 1 - d` (a diagonal neighbour at r = 1 got 0.59 of its colour, where the exact
/// disc gives about pi/4 and the browser's square all of it): "seedling at 24 px, width 1 px: the
/// desktop's ring holds 0.74 of the website's" (75 ring pixels both ways, opacity 15.2 against
/// 20.6). `outline::disc` now measures the exact share.
#[test]
fn the_desktop_outline_matches_the_websites_filter() {
    let tokens = shipped_tokens();
    for (file, what) in [("1F331", "seedling"), ("1F600", "grinning face"), ("2764", "red heart")] {
        for (px, width) in [(24u32, 1.0f32), (48, 2.0)] {
            let bytes = svg(file);
            let plain = rasterise(&bytes, px, Outline::NONE).expect("draws");
            let desktop = rasterise(&bytes, px, Outline::new(tokens.rgba, width)).expect("draws");
            let web = as_the_website_draws(file, px, tokens.rgba, width);
            let (dn, dc, da) = ring(&plain, &desktop);
            let (wn, wc, wa) = ring(&plain, &web);
            let share = da / wa.max(1e-6);
            eprintln!("{what} at {px} px, width {width} px: desktop ring {dn} pixels, opacity {da:.1}, colour {dc:.0?}; website ring {wn} pixels, opacity {wa:.1}, colour {wc:.0?}; desktop/website {share:.2}");
            assert!(wa > 5.0, "{what}: the website's filter drew an outline (resvg ran it): {wa}");
            let reach = dn as f32 / wn.max(1) as f32;
            assert!((0.9..=1.1).contains(&reach), "{what} at {px} px, width {width} px: the outline reaches the same pixels ({dn} against {wn})");
            // The browser's square fills a diagonal neighbour whole where the disc fills its share
            // (pi/4 at a 1 px width), so the desktop's ring is a little fainter at the corners:
            // 0.79 to 0.86 of the website's when this was written.
            assert!((0.75..=1.25).contains(&share), "{what} at {px} px, width {width} px: the desktop's ring holds {share:.2} of the website's");
            for c in 0..3 {
                assert!((dc[c] - wc[c]).abs() <= 8.0, "{what}: the same colour both ways: desktop {dc:?}, website {wc:?}");
            }
        }
    }
}

/// A THEME CHANGE DRAWS THE PICTURES AGAIN: on a context with the pack's pictures, a line with an
/// emoji draws its picture once and keeps it; after the theme's outline colour changes (Settings,
/// then `Theme::apply_to_egui`) the same line draws it again, into a new texture, with the new
/// outline; back to the first theme, the first picture is still kept.
///
/// Seen red 2026-11-20 with the kept picture's key leaving the outline out (`(set, file, px)` as
/// before): "a theme change draws the picture again" (left: 1, right: 2).
#[test]
fn a_theme_change_draws_the_pictures_again() {
    /// The pack's pictures, shared with the test so it can count what they drew.
    struct Shared(Arc<PackPictures>);
    impl super::EmojiPictures for Shared {
        fn picture(&self, set: &str, file: &str, px: u32, outline: Outline) -> Option<egui::TextureId> {
            super::EmojiPictures::picture(&*self.0, set, file, px, outline)
        }
    }
    let ctx = egui::Context::default();
    let counter = Arc::new(PackPictures::new(pack_dir(), ctx.tex_manager()));
    let index = super::EmojiIndex::load(&pack_dir().join("index.json")).expect("the pack's list");
    super::install(&ctx, Some(Arc::new(super::EmojiSource { index, pictures: Box::new(Shared(counter.clone())), pack: None })));
    let mut theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    // One frame laying out a line with the seedling (egui's fonts exist only inside a frame).
    let line = |ctx: &egui::Context| {
        let mut texture = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let (_, placed) = super::layout_singleline(ctx, "\u{1F331}".to_string(), egui::FontId::proportional(16.0), egui::Color32::WHITE);
            texture = placed.first().map(|p| p.texture);
        });
        texture.expect("the seedling is a picture")
    };
    let first = line(&ctx);
    assert_eq!(counter.drawn(), 1, "drawn once");
    assert_eq!(line(&ctx), first, "kept");
    assert_eq!(counter.drawn(), 1, "not drawn again for the same theme");
    theme.emoji_outline = (0.6, 0.9, 1.0, 0.7);
    theme.apply_to_egui(&ctx);
    assert_eq!(outline::tokens(&ctx), theme.emoji_outline_tokens(), "the theme handed the context its outline");
    let second = line(&ctx);
    assert_eq!(counter.drawn(), 2, "a theme change draws the picture again");
    assert_ne!(second, first, "into a new texture");
    theme.emoji_outline = crate::gui::theme::load_theme().emoji_outline;
    theme.apply_to_egui(&ctx);
    assert_eq!(line(&ctx), first, "back to the first theme: its picture is still kept");
    assert_eq!(counter.drawn(), 2);
}

/// Writes the comparison pictures (plain, desktop outline, website filter, each on the theme's
/// black, scaled up 8 times) to `HUMANITY_EMOJI_OUTLINE_PNGS`, for looking at by eye. A tool, not a
/// gate.
#[test]
#[ignore = "writes pictures for a person to look at"]
fn write_the_outline_comparison_pictures() {
    let Some(dir) = std::env::var_os("HUMANITY_EMOJI_OUTLINE_PNGS").map(std::path::PathBuf::from) else { return };
    let tokens = shipped_tokens();
    let ground = crate::gui::theme::load_theme().bg_primary();
    for file in ["1F331", "1F600", "1F44D", "2764", "1F525"] {
        let px = 24u32;
        let bytes = svg(file);
        let pics = [
            rasterise(&bytes, px, Outline::NONE).expect("draws"),
            rasterise(&bytes, px, tokens.at(1.0)).expect("draws"),
            as_the_website_draws(file, px, tokens.rgba, tokens.width_pt),
        ];
        let scale = 8u32;
        let mut out = image::RgbaImage::new(px * scale * 3 + 2 * scale, px * scale);
        for (n, pic) in pics.iter().enumerate() {
            for y in 0..px * scale {
                for x in 0..px * scale {
                    let p = pic.pixels[((y / scale) * px + x / scale) as usize];
                    let k = 255 - p.a() as u32;
                    let over = |c: u8, g: u8| (c as u32 + g as u32 * k / 255).min(255) as u8;
                    let at = n as u32 * (px * scale + scale) + x;
                    out.put_pixel(at, y, image::Rgba([over(p.r(), ground.r()), over(p.g(), ground.g()), over(p.b(), ground.b()), 255]));
                }
            }
        }
        out.save(dir.join(format!("halo-{file}.png"))).expect("save");
    }
}

/// The outline's width and colour from the theme's floats, quantised for a picture's key.
#[test]
fn tokens_become_an_outline_in_pixels() {
    let t = Tokens::from_theme((1.0, 0.5, 0.0, 0.5), 1.0);
    assert_eq!(t.rgba, [255, 128, 0, 128]);
    assert_eq!(t.at(1.5).width_px(), 1.5, "points times the screen's scale");
    assert_eq!(Tokens::from_theme((1.0, 1.0, 1.0, 1.0), -2.0).at(1.0), Outline::NONE, "a negative width is none");
    assert_eq!(Tokens::from_theme((1.0, 1.0, 1.0, 1.0), f32::NAN).at(1.0), Outline::NONE, "a width that is not a number is none");
    assert!(Outline::new([255; 4], 100.0).width_px() <= outline::MAX_WIDTH_PX, "the width is bounded");
}
