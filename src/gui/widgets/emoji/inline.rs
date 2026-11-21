//! Pictures in a line of text (2026-11-20, docs/design/emoji.md rung 3).
//!
//! egui's text layout has no inline images, so each emoji that has a picture is laid out as a
//! TRANSPARENT placeholder exactly as wide as the picture's room in the line (the same characters,
//! drawn invisible, at a font size that makes their run that wide, and no taller than the text
//! around it), and the picture is painted over the placeholder's rectangle read back from the
//! laid-out galley. The galley's text is unchanged, so wrapping treats the emoji as the character it
//! is, a click maps to the same character index, and copying gives the real emoji.
//!
//! Where pictures come from is one small trait, [`EmojiPictures`]: "the picture of this file at this
//! pixel size, if there is one". Rung 2 (`pack.rs`, resvg drawing the pack's SVGs) supplies it and
//! installs it on every egui context (`pack::install_pack`, through `gui::install_fonts`); when
//! nothing is [`install`]ed, or when it has no picture, an emoji is drawn as before, as the system
//! emoji font's glyph. Used by the chat's message bodies (`widgets::row::message_row`), reaction
//! pills (`pages::chat` `paint_timestamp_pill`) and the reaction buttons (`button.rs`).

use std::ops::Range;
use std::sync::Arc;

use egui::text::{Fonts, Galley, LayoutJob, LayoutSection};
use egui::{Color32, Rect, Stroke, TextureId};

use super::{pieces, EmojiIndex, Piece};

/// The picture's side and the room around it, in ems of the text's font size: the website's
/// `img.emoji` (web/chat/base.css: 1.2em square, margin 0.1em on the left and 0.05em on the right).
const SIDE_EM: f32 = 1.2;
const LEFT_EM: f32 = 0.1;
const RIGHT_EM: f32 = 0.05;

/// Where emoji pictures come from. Rung 2 implements it (`pack::PackPictures`: resvg drawing each
/// picture at the asked pixel size into an egui texture).
pub trait EmojiPictures: Send + Sync {
    /// The texture of the picture `file` (its name without `.svg`) in the set folder `set`, drawn
    /// `px` pixels square, or None when there is none (the emoji then stays the font's glyph).
    fn picture(&self, set: &str, file: &str, px: u32) -> Option<TextureId>;
}

/// The pack's list and its pictures, as the text that shows emoji sees them.
pub struct EmojiSource {
    pub index: EmojiIndex,
    pub pictures: Box<dyn EmojiPictures>,
    /// When the list came from the shipped pack, the pack version and time it was read (it is read
    /// again when the pack changes, or on a schedule with the reuse switch off: `pack::list_due`);
    /// None for a source made in code (a test's stand-in), which stays as it was installed.
    pub pack: Option<super::pack::PackStamp>,
}

fn source_id() -> egui::Id {
    egui::Id::new("hos_emoji_source")
}

/// Hand the app's emoji pictures to every text that shows emoji (None takes them away). Rung 2
/// calls it once its pictures can be drawn; tests call it with a stand-in.
pub fn install(ctx: &egui::Context, source: Option<Arc<EmojiSource>>) {
    ctx.data_mut(|d| match source {
        Some(s) => d.insert_temp(source_id(), s),
        None => d.remove::<Arc<EmojiSource>>(source_id()),
    });
}

/// The installed emoji pictures, if any; the pack's are read again first when they are due (the pack
/// changed, or the reuse switch's schedule: `pack::list_due`). Never call it inside `ctx.fonts`.
pub fn source(ctx: &egui::Context) -> Option<Arc<EmojiSource>> {
    let src = ctx.data(|d| d.get_temp::<Arc<EmojiSource>>(source_id()))?;
    match src.pack {
        Some(stamp) if super::pack::due(&stamp) => super::pack::reload(ctx),
        _ => Some(src),
    }
}

/// Do the installed pictures come from the shipped pack (OpenMoji's, which ask for their credit
/// where they are picked: `button::credit`)?
pub fn pack_installed(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Arc<EmojiSource>>(source_id())).is_some_and(|s| s.pack.is_some())
}

/// One picture to paint over its placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// The section of the laid-out job that is the placeholder.
    pub section: usize,
    /// The byte range of the emoji in the job's text (what copying gives).
    pub range: Range<usize>,
    pub texture: TextureId,
    /// The picture's side, in points.
    pub side: f32,
}

/// `job` with a transparent placeholder in place of each emoji the installed pictures can draw, and
/// the pictures to paint once it is laid out. A job with no such emoji comes back unchanged.
pub fn with_pictures(fonts: &Fonts, job: LayoutJob, source: &EmojiSource, pixels_per_point: f32) -> (LayoutJob, Vec<Placed>) {
    // The emoji that have a picture, with the section each starts in.
    let mut cuts: Vec<(Range<usize>, usize, TextureId, f32)> = Vec::new();
    for piece in pieces(&job.text, &source.index) {
        let Piece::Picture { range, set, file } = piece else { continue };
        let Some(sec) = job.sections.iter().position(|s| s.byte_range.contains(&range.start)) else { continue };
        let side = SIDE_EM * job.sections[sec].format.font_id.size;
        let px = (side * pixels_per_point).round().max(1.0) as u32;
        if let Some(texture) = source.pictures.picture(&set, &file, px) {
            cuts.push((range, sec, texture, side));
        }
    }
    if cuts.is_empty() {
        return (job, Vec::new());
    }

    let mut sections: Vec<LayoutSection> = Vec::with_capacity(job.sections.len() + 2 * cuts.len());
    let mut placed = Vec::with_capacity(cuts.len());
    let mut next = cuts.iter().peekable();
    let mut pos = 0; // every byte before this is in `sections` already
    for sec in &job.sections {
        let mut s = sec.byte_range.start.max(pos);
        while s < sec.byte_range.end {
            let leading_space = if s == sec.byte_range.start { sec.leading_space } else { 0.0 };
            match next.peek().copied() {
                Some((range, _, texture, side)) if range.start < sec.byte_range.end => {
                    if range.start > s {
                        sections.push(LayoutSection { leading_space, byte_range: s..range.start, format: sec.format.clone() });
                    }
                    let leading_space = if range.start == sec.byte_range.start { sec.leading_space } else { 0.0 };
                    placed.push(Placed { section: sections.len(), range: range.clone(), texture: *texture, side: *side });
                    sections.push(LayoutSection { leading_space, byte_range: range.clone(), format: placeholder(fonts, &job.text[range.clone()], &sec.format) });
                    s = range.end;
                    pos = range.end;
                    next.next();
                }
                _ => {
                    sections.push(LayoutSection { leading_space, byte_range: s..sec.byte_range.end, format: sec.format.clone() });
                    s = sec.byte_range.end;
                }
            }
        }
        pos = pos.max(sec.byte_range.end);
    }
    (LayoutJob { sections, ..job }, placed)
}

/// The format of an emoji's placeholder: its characters drawn invisible, at the font size that makes
/// their run the picture's room wide, and no taller than the text around it.
fn placeholder(fonts: &Fonts, text: &str, base: &egui::TextFormat) -> egui::TextFormat {
    let em = base.font_id.size;
    let room = (SIDE_EM + LEFT_EM + RIGHT_EM) * em;
    let natural: f32 = text.chars().map(|c| fonts.glyph_width(&base.font_id, c)).sum();
    let mut f = base.clone();
    if natural > 0.0 {
        // Glyph advances grow with the font size; a quarter point keeps the number of font sizes
        // egui makes small (the run is measured back from the galley, so it need not be exact).
        f.font_id.size = (em * room / natural * 4.0).round().max(1.0) / 4.0;
    }
    f.line_height = Some(base.line_height.unwrap_or_else(|| fonts.row_height(&base.font_id)));
    f.color = Color32::TRANSPARENT;
    f.extra_letter_spacing = 0.0;
    f.italics = false;
    f.underline = Stroke::NONE;
    f.strikethrough = Stroke::NONE;
    f
}

/// Where each picture goes, relative to the galley's origin: on its placeholder's run (in the row
/// that holds the run's first glyph), placed in that room as the website places it, centred on the
/// row's height.
pub fn picture_rects(galley: &Galley, placed: &[Placed]) -> Vec<(Rect, TextureId)> {
    let mut out = Vec::with_capacity(placed.len());
    for p in placed {
        let row = galley.rows.iter().find(|r| r.glyphs.iter().any(|g| g.section_index as usize == p.section));
        let Some(row) = row else { continue };
        let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
        for g in row.glyphs.iter().filter(|g| g.section_index as usize == p.section) {
            left = left.min(g.pos.x);
            right = right.max(g.pos.x + g.advance_width);
        }
        let x = left + (right - left - p.side) * (LEFT_EM / (LEFT_EM + RIGHT_EM));
        let y = row.rect.center().y - p.side / 2.0;
        out.push((Rect::from_min_size(egui::pos2(x, y), egui::vec2(p.side, p.side)), p.texture));
    }
    out
}

/// Paint the pictures of a galley drawn at `origin`.
pub fn paint(painter: &egui::Painter, origin: egui::Pos2, galley: &Galley, placed: &[Placed]) {
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    for (rect, texture) in picture_rects(galley, placed) {
        painter.image(texture, rect.translate(origin.to_vec2()), uv, Color32::WHITE);
    }
}

/// Lay out `job` with the installed pictures in its emoji (see the top of this file); with none
/// installed, exactly as `painter.layout_job(job)` would. Paint the galley, then [`paint`] the pictures.
pub fn layout(ctx: &egui::Context, job: LayoutJob) -> (Arc<Galley>, Vec<Placed>) {
    match source(ctx) {
        None => (ctx.fonts(|f| f.layout_job(job)), Vec::new()),
        Some(src) => {
            let ppp = ctx.pixels_per_point();
            ctx.fonts(|f| {
                let (job, placed) = with_pictures(f, job, &src, ppp);
                (f.layout_job(job), placed)
            })
        }
    }
}

/// [`layout`] for one line of text in one font and colour (as `fonts.layout_no_wrap`).
pub fn layout_singleline(ctx: &egui::Context, text: String, font_id: egui::FontId, color: Color32) -> (Arc<Galley>, Vec<Placed>) {
    layout(ctx, LayoutJob::simple_singleline(text, font_id, color))
}
