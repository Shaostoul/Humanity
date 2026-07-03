//! Inventory grid with item slots, equipment section, weight tracking,
//! item detail panel, and quick actions.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GardenArea, GuiAsteroid, GuiDrone, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

const COLS: usize = 8;

// Equipment slot definitions are loaded from `data/inventory/equipment_slots.json`
// into `GuiState.equipment_slots` at startup (see `lib.rs`). The equipped Vec is
// populated lazily on the first draw where slots are available.

/// Page-local state for the inventory.
struct InventoryPageState {
    /// Equipped items (slot_name -> item name or empty).
    equipped: Vec<(String, Option<String>)>,
    /// Current carry weight.
    carry_weight: f32,
    /// Max carry weight.
    max_carry_weight: f32,
    /// Whether we have initialized sample data.
    initialized: bool,
}

impl Default for InventoryPageState {
    fn default() -> Self {
        Self {
            // Populated from gui_state.equipment_slots on first draw.
            equipped: Vec::new(),
            carry_weight: 0.0,
            max_carry_weight: 50.0,
            initialized: false,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut InventoryPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<InventoryPageState> = RefCell::new(InventoryPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Get a color based on item category.
fn category_color(category: &str) -> Color32 {
    match category {
        "clothing" => Color32::from_rgb(100, 140, 200),
        "tool" => Color32::from_rgb(180, 150, 80),
        "weapon" => Color32::from_rgb(200, 80, 80),
        "furniture" => Color32::from_rgb(140, 120, 100),
        "food" => Color32::from_rgb(80, 180, 80),
        "material" => Color32::from_rgb(160, 160, 140),
        "machine" => Color32::from_rgb(140, 140, 180),
        "vehicle" => Color32::from_rgb(180, 100, 180),
        _ => Color32::from_rgb(120, 120, 120),
    }
}

/// Short, capitalized ore name for display: "iron_ore_0" -> "Iron".
fn ore_short(item_id: &str) -> String {
    let base = item_id.strip_suffix("_0").unwrap_or(item_id);
    let base = base.strip_suffix("_ore").unwrap_or(base);
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => base.to_string(),
    }
}

/// A segmented capacity bar for the drone manifest: a track sized to `cap`, with one
/// coloured segment per ore in the draft (width ∝ its allocated units). Lets the
/// player see the allocation fill the hold as they build it.
fn manifest_bar(ui: &mut egui::Ui, theme: &Theme, draft: &[(String, u32)], cap: u32) {
    let w = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(2), theme.border());
    let palette = [
        theme.accent(),
        theme.warning(),
        theme.danger(),
        theme.text_secondary(),
    ];
    let capf = cap.max(1) as f32;
    let mut x = rect.left();
    for (i, (_ore, units)) in draft.iter().enumerate() {
        let seg_w = (*units as f32 / capf) * w;
        let seg = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(seg_w, 12.0));
        painter.rect_filled(seg, egui::Rounding::same(2), palette[i % palette.len()]);
        x += seg_w;
    }
}

/// One vital as a TILE: a small card with the vital's name, its value, and a
/// chunky rounded colour-by-level bar. A grid of these reads at a glance and uses
/// the page width, replacing the old thin name/value/bar text rows.
fn vital_tile(ui: &mut egui::Ui, theme: &Theme, name: &str, value: &str, frac: f32, color: Color32) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius_lg as u8))
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(Vec2::new(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(name).size(theme.font_size_small).color(theme.text_muted()));
            ui.label(
                RichText::new(value)
                    .size(theme.font_size_heading)
                    .strong()
                    .color(color),
            );
            ui.add_space(theme.spacing_xs);
            let w = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 8.0), egui::Sense::hover());
            let r = Rounding::same(4);
            ui.painter().rect_filled(rect, r, theme.bg_secondary());
            let fill_w = w * frac.clamp(0.0, 1.0);
            if fill_w > 0.0 {
                let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, 8.0));
                ui.painter().rect_filled(fill, r, color);
            }
        });
}

// GardenArea + the loader moved to gui/mod.rs (loaded via the resolved data_dir in
// lib.rs and stored on GuiState.garden_areas), so the overview works regardless of the
// process CWD -- the page reads state.garden_areas (review fix, v0.504).

/// One grow area as a TILE: a colour swatch + name, the count (×N), and the per-unit
/// food output. A grid of these is the at-a-glance "whole garden" overview. The whole
/// tile is clickable (returns true) to open its per-medium edit modal.
fn garden_area_tile(ui: &mut egui::Ui, theme: &Theme, a: &GardenArea) -> bool {
    let inner = Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius_lg as u8))
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(Vec2::new(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, widgets::swatch_color(&a.machine_id));
                ui.label(
                    RichText::new(&a.label)
                        .strong()
                        .color(theme.text_primary())
                        .size(theme.font_size_small),
                );
            });
            ui.label(
                RichText::new(format!("×{}", a.count))
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.accent()),
            );
            if !a.food.is_empty() {
                ui.label(
                    RichText::new(&a.food)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
            ui.label(RichText::new("Edit").size(theme.font_size_small).color(theme.accent()));
        });
    let resp = inner.response.interact(egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// In-memory edit config for one grow area (until garden persistence lands). The modal
/// reads + writes this, keyed by machine id. Slider/toggle values are keyed by the
/// control's `key` from the grow-media registry, so a new control needs no field here.
#[derive(Clone, Default)]
struct GardenEditConfig {
    values: std::collections::HashMap<String, f32>,
    toggles: std::collections::HashMap<String, bool>,
    crop: String,
}

#[derive(Default)]
struct GardenEditState {
    /// machine id of the grow area whose edit modal is open.
    open: Option<String>,
    configs: std::collections::HashMap<String, GardenEditConfig>,
}

fn with_garden_edit<R>(f: impl FnOnce(&mut GardenEditState) -> R) -> R {
    thread_local! {
        static S: RefCell<GardenEditState> = RefCell::new(GardenEditState::default());
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

/// Publish the garden edit modal's water + nutrient sliders into `state.garden_irrigation`
/// and `state.garden_nutrient` so the FarmingSystem can act on them (lib.rs bridges both
/// fields into the DataStore each frame). The garden areas are keyed by machine TYPE
/// ("aeroponic_tower"), but crops carry a `tower_id` ("nutrition"); for a medium that
/// grows towers (`show_slots`), its one slider value applies to every tower variety, so
/// we fan each value out across all `tower_configs` ids. Non-tower media (soil bed,
/// field) have no `tower_id` crops yet, so they don't contribute. Recomputed each frame.
fn snapshot_garden_sim(state: &mut GuiState) {
    let mut irr: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut nut: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    with_garden_edit(|s| {
        for (machine_id, cfg) in &s.configs {
            let grows_towers = state
                .grow_media
                .iter()
                .find(|m| m.matches(machine_id))
                .map_or(false, |m| m.show_slots);
            if !grows_towers {
                continue;
            }
            let water = cfg.values.get("water").copied();
            let nutrient = cfg.values.get("nutrient").copied();
            for tc in &state.tower_configs {
                if let Some(w) = water {
                    irr.insert(tc.id.clone(), w);
                }
                if let Some(n) = nutrient {
                    nut.insert(tc.id.clone(), n);
                }
            }
        }
    });
    state.garden_irrigation = irr;
    state.garden_nutrient = nut;
}

/// Test hook: open the garden edit modal for a machine id, so the snapshot harness
/// can render the modal (which is otherwise opened by a click).
#[cfg(test)]
pub(crate) fn test_open_garden_edit(machine_id: &str) {
    with_garden_edit(|s| s.open = Some(machine_id.to_string()));
}

/// Test hook: close the modal, so a non-modal snapshot isn't polluted by a prior
/// modal snapshot's leftover thread_local open state (tests share a thread).
#[cfg(test)]
pub(crate) fn test_close_garden_edit() {
    with_garden_edit(|s| s.open = None);
}

/// The per-medium edit modal for the open grow area. Each medium gets controls
/// tailored to how it grows. Called at ctx level (after the inventory panel closes).
fn garden_edit_modal(ctx: &egui::Context, theme: &Theme, state: &GuiState) {
    let Some(machine_id) = with_garden_edit(|s| s.open.clone()) else {
        return;
    };
    let Some(area) = state.garden_areas.iter().find(|a| a.machine_id == machine_id).cloned() else {
        with_garden_edit(|s| s.open = None);
        return;
    };
    let medium = state.grow_media.iter().find(|m| m.matches(&machine_id)).cloned();
    // A real modal: egui::Modal dims the inventory behind it and closes on a
    // backdrop click or Escape, so it reads as a focused popup, not a floating window.
    let modal = egui::Modal::new(egui::Id::new(("garden_edit", &machine_id)))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.set_min_width(460.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 6.0, widgets::swatch_color(&machine_id));
                ui.label(RichText::new(&area.label).size(theme.font_size_heading).strong().color(theme.text_primary()));
                ui.label(RichText::new(format!("×{}", area.count)).color(theme.accent()).strong());
            });
            if !area.food.is_empty() {
                ui.label(RichText::new(format!("Output per unit: {}", area.food)).color(theme.text_secondary()).size(theme.font_size_small));
            }
            if area.size != (0.0, 0.0, 0.0) {
                ui.label(
                    RichText::new(format!("Footprint: {:.1} x {:.1} x {:.1} m", area.size.0, area.size.1, area.size.2))
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            }
            ui.separator();
            garden_medium_editor(ui, theme, &machine_id, medium.as_ref(), state);
            ui.add_space(theme.spacing_sm);
            if widgets::primary_button(ui, theme, "Done") {
                with_garden_edit(|s| s.open = None);
            }
        });
    if modal.should_close() {
        with_garden_edit(|s| s.open = None);
    }
}

/// Medium-specific controls for the edit modal, rendered from the grow-media registry
/// (data/garden/grow_media.ron) — so adding a plot-type is a data edit, not code.
fn garden_medium_editor(
    ui: &mut egui::Ui,
    theme: &Theme,
    machine_id: &str,
    medium: Option<&crate::gui::GrowMedium>,
    state: &GuiState,
) {
    let Some(medium) = medium else {
        ui.label(
            RichText::new("No medium-specific controls for this area yet.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        return;
    };
    let mut cfg = with_garden_edit(|s| s.configs.entry(machine_id.to_string()).or_default().clone());
    ui.label(RichText::new(&medium.label).strong().color(theme.text_secondary()));
    if !medium.note.is_empty() {
        ui.label(RichText::new(&medium.note).size(theme.font_size_small).color(theme.text_muted()));
    }
    ui.add_space(theme.spacing_xs);
    // An aeroponic tower also lists its planted slots (data flag: show_slots).
    if medium.show_slots {
        let cfg_id = machine_id.strip_prefix("aeroponic_tower_").unwrap_or("");
        if let Some(tc) = state.tower_configs.iter().find(|t| t.id == cfg_id) {
            ui.label(RichText::new(format!("{} slots planted as:", tc.slots)).color(theme.text_primary()).size(theme.font_size_small));
            egui::ScrollArea::vertical().id_salt("ga_slots").max_height(150.0).show(ui, |ui| {
                for p in &tc.plantings {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}x", p.slots)).color(theme.accent()).size(theme.font_size_small));
                        ui.label(RichText::new(&p.plant).color(theme.text_primary()).size(theme.font_size_small));
                        if !p.role.is_empty() {
                            ui.label(RichText::new(&p.role).color(theme.text_muted()).size(theme.font_size_small));
                        }
                    });
                }
            });
            ui.add_space(theme.spacing_xs);
        }
    }
    // The data-driven controls (sliders/crop field/toggles, keyed by the registry).
    for control in &medium.controls {
        match control {
            crate::gui::GrowControl::Slider { key, label } => {
                let v = cfg.values.entry(key.clone()).or_insert(0.5);
                widgets::labeled_slider(ui, theme, label, v, 0.0..=1.0);
            }
            crate::gui::GrowControl::Crop { label, hint } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).color(theme.text_secondary()).size(theme.font_size_small));
                    ui.add(egui::TextEdit::singleline(&mut cfg.crop).desired_width(220.0).hint_text(hint.as_str()));
                });
            }
            crate::gui::GrowControl::Toggle { key, label } => {
                let t = cfg.toggles.entry(key.clone()).or_insert(true);
                widgets::toggle(ui, theme, label, t);
            }
        }
    }
    with_garden_edit(|s| {
        s.configs.insert(machine_id.to_string(), cfg);
    });
}

/// Parse item data from embedded CSV to get details for a given item_id.
fn lookup_item_details(item_id: &str) -> Option<ItemDetails> {
    let csv = crate::embedded_data::ITEMS_CSV;
    for line in csv.lines() {
        if line.starts_with('#') || line.starts_with("id,") || line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() >= 9 && fields[0] == item_id {
            return Some(ItemDetails {
                name: fields[1].to_string(),
                category: fields[2].to_string(),
                subcategory: fields[3].to_string(),
                base_material: fields[4].to_string(),
                weight_kg: fields[5].parse().unwrap_or(0.0),
                stack_size: fields[6].parse().unwrap_or(1),
                durability: fields[7].parse().unwrap_or(0),
                description: fields[8].to_string(),
            });
        }
    }
    None
}

#[derive(Debug, Clone)]
struct ItemDetails {
    name: String,
    category: String,
    subcategory: String,
    base_material: String,
    weight_kg: f32,
    stack_size: u32,
    durability: u32,
    description: String,
}

/// Colour for a place/entity node by its `kind` — drives the tree swatches so the
/// structure is scannable ("what is where"). All theme tokens, no literals.
fn kind_color(theme: &Theme, kind: &str) -> egui::Color32 {
    match kind {
        "person" => theme.success(),
        "vehicle" => theme.warning(),
        "building" | "property" => theme.accent(),
        "planet" | "region" | "locale" => theme.info(),
        "item" => theme.text_muted(),
        // rooms, floors, packs, duffels, bags, pouches, generic containers
        _ => theme.text_secondary(),
    }
}

/// Fixed tile width for the nested-container inventory so a row of tiles stays even.
const ITEM_TILE_W: f32 = 96.0;

/// Selected placed-pool item (by index into `state.placed_items`) for the inspect +
/// "Move to" card — the spatial-inventory counterpart to `state.selected_slot` (which
/// tracks the live backpack selection).
fn with_placed_sel<R>(f: impl FnOnce(&mut Option<usize>) -> R) -> R {
    thread_local! {
        static S: RefCell<Option<usize>> = RefCell::new(None);
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

/// One clickable item tile: a stable-colored swatch (the item's initial as a glyph),
/// the truncated name, and a quantity badge. `selected` draws the accent outline.
/// Returns true when clicked.
fn item_tile(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    item_id: &str,
    qty: u32,
    selected: bool,
) -> bool {
    let glyph = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    let stroke = if selected {
        Stroke::new(2.0, theme.accent())
    } else {
        Stroke::new(1.0, theme.border())
    };
    let inner = Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius_lg as u8))
        .stroke(stroke)
        .inner_margin(Vec2::new(6.0, 6.0))
        .show(ui, |ui| {
            ui.set_width(ITEM_TILE_W - 12.0);
            ui.vertical_centered(|ui| {
                widgets::placeholder_tile(ui, theme, widgets::swatch_color(item_id), 48.0, &glyph);
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(name).size(theme.font_size_small).color(theme.text_primary()),
                    )
                    .truncate(),
                );
                if qty > 1 {
                    ui.label(
                        RichText::new(format!("x{qty}"))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
            });
        });
    let resp = inner.response.interact(egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Clicks the nested-container renderer recorded this frame: a live backpack tile (by
/// slot) or a placed-pool item tile (by index). Applied to selection after the render.
#[derive(Default)]
struct PlacesOut {
    clicked_slot: Option<usize>,
    clicked_placed: Option<usize>,
}

/// Recursively render one container (a [`crate::gui::Place`]) as a card: a clickable
/// header (kind dot + label/location, toggles open), a wrap-grid of its item TILES,
/// then its child containers nested inside — the spatial inventory (person -> shirt ->
/// pocket -> wallet, operator 2026-06-22). Live backpack items come from `inv` at the
/// `kind:"backpack"` node; every other container shows the `placed` pool entries tagged
/// with its path. Clicks land in `out`; open state persists in egui memory by `path`.
#[allow(clippy::too_many_arguments)]
fn draw_container(
    ui: &mut egui::Ui,
    theme: &Theme,
    place: &crate::gui::Place,
    inv: &[Option<crate::gui::GuiItemSlot>],
    placed: &[crate::gui::PlacedItem],
    path: &str,
    sel_slot: Option<usize>,
    sel_placed: Option<usize>,
    out: &mut PlacesOut,
) {
    let depth = path.matches('/').count();
    let dot = kind_color(theme, &place.kind);
    let mut title = place.label.clone();
    let mut loc = place.location.clone().unwrap_or_default();
    if let Some([lat, lon]) = place.coordinate {
        let coord = format!("{lat:.3}, {lon:.3}");
        loc = if loc.is_empty() { coord } else { format!("{loc} · {coord}") };
    }
    if !loc.is_empty() {
        title = format!("{title}  ({loc})");
    }
    // Counts for the header, so a COLLAPSED container still tells you what is inside.
    // Non-backpack item counts come from the placed pool (tagged by this path).
    let is_backpack = place.kind == "backpack";
    let item_count = if is_backpack {
        inv.iter().filter(|s| s.is_some()).count()
    } else {
        placed.iter().filter(|pi| pi.container == path).count()
    };
    let sub_count = place.children.iter().filter(|c| c.kind != "item").count();
    let mut hint_parts: Vec<String> = Vec::new();
    if item_count > 0 {
        hint_parts.push(format!("{item_count} item{}", if item_count == 1 { "" } else { "s" }));
    }
    if sub_count > 0 {
        hint_parts.push(format!("{sub_count} container{}", if sub_count == 1 { "" } else { "s" }));
    }
    let hint = hint_parts.join(", ");

    let open_id = egui::Id::new(("place_open", path));
    let mut open = ui.data_mut(|d| d.get_temp::<bool>(open_id).unwrap_or(depth < 2));

    widgets::card(ui, theme, |ui| {
        // Header: open triangle (a SHAPE, never a tofu glyph) + kind dot + label +
        // contents count. The WHOLE row toggles open (the trailing allocate claims the
        // remaining width so the click target is the full row, not just the label).
        let header = ui.horizontal(|ui| {
            let (tri, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            if open {
                widgets::icons::paint_triangle_down(ui.painter(), tri, theme.text_muted());
            } else {
                widgets::icons::paint_triangle_right(ui.painter(), tri, theme.text_muted());
            }
            let (dotr, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            ui.painter().circle_filled(dotr.center(), 5.0, dot);
            ui.label(
                RichText::new(&title).strong().color(theme.text_primary()).size(theme.font_size_body),
            );
            if !hint.is_empty() {
                ui.label(
                    RichText::new(format!("· {hint}")).size(theme.font_size_small).color(theme.text_muted()),
                );
            }
            ui.allocate_space(egui::vec2(ui.available_width().max(0.0), 1.0));
        });
        // Re-interact the header row with a STABLE per-path Id so the click is reliably
        // attributed to this header across frames (an auto-generated Id is sequence-
        // dependent; a stable one also lets the headless interaction harness drive it).
        let row = ui.interact(
            header.response.rect,
            egui::Id::new(("place_header", path)),
            egui::Sense::click(),
        );
        #[cfg(test)]
        RECORDED_HEADER_RECTS.with(|r| {
            r.borrow_mut().insert(path.to_string(), row.rect);
        });
        if row.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if row.clicked() {
            open = !open;
            #[cfg(test)]
            RECORDED_HEADER_CLICKS.with(|r| {
                r.borrow_mut().insert(path.to_string());
            });
        }
        if open {
            // Direct contents render as item TILES: the live backpack stack (at the
            // kind:"backpack" node) OR the placed-pool items tagged with this path.
            let here: Vec<usize> = if is_backpack {
                Vec::new()
            } else {
                placed
                    .iter()
                    .enumerate()
                    .filter(|(_, pi)| pi.container == path)
                    .map(|(i, _)| i)
                    .collect()
            };
            let has_tiles = (is_backpack && inv.iter().any(|s| s.is_some())) || !here.is_empty();
            if has_tiles {
                ui.add_space(theme.spacing_xs);
                ui.horizontal_wrapped(|ui| {
                    if is_backpack {
                        for (i, slot) in inv.iter().enumerate() {
                            if let Some(it) = slot {
                                if item_tile(ui, theme, &it.name, &it.item_id, it.quantity, sel_slot == Some(i)) {
                                    out.clicked_slot = Some(i);
                                }
                            }
                        }
                    } else {
                        for &idx in &here {
                            let pi = &placed[idx];
                            let sel = sel_placed == Some(idx);
                            if item_tile(ui, theme, &pi.name, &pi.key, pi.qty, sel) {
                                out.clicked_placed = Some(idx);
                            }
                        }
                    }
                });
            }
            // Sub-containers (everything that is NOT a leaf item) nest as their own cards.
            for (i, child) in place.children.iter().enumerate() {
                if child.kind == "item" {
                    continue;
                }
                ui.add_space(theme.spacing_xs);
                draw_container(ui, theme, child, inv, placed, &format!("{path}/{i}"), sel_slot, sel_placed, out);
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

/// Quick-action outputs from the inline item card. The card only borrows an item
/// SNAPSHOT (so it can't touch GuiState mid-render), so it records what the player
/// asked for here and the caller applies it to GuiState after the tree renders.
#[derive(Default)]
struct ItemCardActions {
    eat: Option<String>,
    drink: Option<String>,
    plant: Option<String>,
    /// Vehicle KIT item id to deploy into the world (economy Phase 2 Stage 1).
    deploy: Option<String>,
    equip: bool,
    drop: bool,
}

/// The EXPAND-IN-PLACE card for one inventory item (operator 2026-06-08: "click an
/// item row to expand in place — picture/3d + full details over rows — instead of a
/// popup/top detail"). Rendered under the selected row by the places/backpack tree's
/// inline renderer: a colored placeholder image tile (the universal widget, swatch by
/// item id) + category badge + a details grid + description + quick actions. Records
/// the chosen action into `acts`; the caller bridges it into GuiState.
fn draw_item_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    item: &crate::gui::GuiItemSlot,
    acts: Option<&mut ItemCardActions>,
) {
    let details = lookup_item_details(&item.item_id);
    ui.add_space(theme.spacing_xs);
    ui.horizontal_top(|ui| {
        // Colored placeholder image / 3D-model stand-in (stable colour per item id).
        let glyph = item
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        widgets::placeholder_tile(ui, theme, widgets::swatch_color(&item.item_id), 64.0, &glyph);
        ui.add_space(theme.spacing_sm);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&item.name)
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.accent()),
            );
            if let Some(d) = &details {
                // Category badge.
                egui::Frame::none()
                    .fill(category_color(&d.category))
                    .rounding(Rounding::same(3))
                    .inner_margin(Vec2::new(6.0, 2.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&d.category)
                                .size(theme.font_size_small)
                                .color(Color32::WHITE),
                        );
                    });
            }
        });
    });
    ui.add_space(theme.spacing_xs);
    // Details grid.
    widgets::card(ui, theme, |ui| {
        widgets::detail_row(ui, theme, "ID", &item.item_id);
        widgets::detail_row(ui, theme, "Quantity", &item.quantity.to_string());
        if let Some(d) = &details {
            widgets::detail_row(ui, theme, "Category", &d.category);
            widgets::detail_row(ui, theme, "Subcategory", &d.subcategory);
            widgets::detail_row(ui, theme, "Weight", &format!("{:.2} kg", d.weight_kg));
            widgets::detail_row(ui, theme, "Stack Size", &d.stack_size.to_string());
            widgets::detail_row(ui, theme, "Material", &d.base_material);
            if d.durability > 0 {
                widgets::detail_row(ui, theme, "Durability", &d.durability.to_string());
            }
        }
    });
    if let Some(d) = &details {
        if !d.description.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(&d.description)
                    .color(theme.text_secondary())
                    .size(theme.font_size_small),
            );
        }
    }
    // Quick actions — Eat (food) / Drink (liquid) / Plant (seed) / Use, then Equip +
    // Drop. Only shown for items in the LIVE backpack (acts = Some); a seeded item in
    // some other container is inspect-only (acts = None) until item transfer lands.
    let Some(acts) = acts else {
        return;
    };
    ui.add_space(theme.spacing_sm);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let is_drink = details
            .as_ref()
            .map(|d| d.subcategory == "drink" || d.subcategory == "liquid")
            .unwrap_or(false)
            || item.item_id.starts_with("water_");
        let is_food = details.as_ref().map(|d| d.category == "food").unwrap_or(false) && !is_drink;
        let is_seed = details.as_ref().map(|d| d.subcategory == "seed").unwrap_or(false)
            || item.item_id.starts_with("seed_");
        // A vehicle KIT (category vehicle / subcategory kit) deploys into the world
        // as a real Vehicle entity. The button is the trigger; VehicleSystem is the
        // authority (registry lookup + survival consume happen there).
        let is_vehicle_kit = details
            .as_ref()
            .map(|d| d.category == "vehicle" && d.subcategory == "kit")
            .unwrap_or(false);
        if is_vehicle_kit {
            if widgets::compact_button(ui, theme, "Deploy", widgets::ButtonVariant::Primary) {
                acts.deploy = Some(item.item_id.clone());
            }
        } else if is_drink {
            if widgets::compact_button(ui, theme, "Drink", widgets::ButtonVariant::Primary) {
                acts.drink = Some(item.item_id.clone());
            }
        } else if is_food {
            if widgets::compact_button(ui, theme, "Eat", widgets::ButtonVariant::Primary) {
                acts.eat = Some(item.item_id.clone());
            }
        } else if is_seed {
            if widgets::compact_button(ui, theme, "Plant", widgets::ButtonVariant::Primary) {
                acts.plant = Some(item.item_id.clone());
            }
        } else {
            let _ = widgets::compact_button(ui, theme, "Use", widgets::ButtonVariant::Secondary);
        }
        if widgets::compact_button(ui, theme, "Equip", widgets::ButtonVariant::Secondary) {
            acts.equip = true;
        }
        if widgets::compact_button(ui, theme, "Drop", widgets::ButtonVariant::Danger) {
            acts.drop = true;
        }
    });
}

/// One asteroid as a clickable CARD: swatch by class + name, class + distance, the ore
/// summary, and a "Mine" hint. Returns true when clicked (to open its mining modal).
fn asteroid_card(ui: &mut egui::Ui, theme: &Theme, ast: &GuiAsteroid) -> bool {
    let summary: Vec<String> = ast
        .ores
        .iter()
        .filter(|(_, q)| *q >= 1.0)
        .map(|(id, q)| format!("{} {:.0}", ore_short(id), q))
        .collect();
    let inner = Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius_lg as u8))
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(Vec2::new(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, widgets::swatch_color(&ast.classification));
                ui.label(RichText::new(&ast.name).strong().color(theme.text_primary()).size(theme.font_size_small));
            });
            ui.label(
                RichText::new(format!("Class {} · {:.0} km", ast.classification, ast.distance))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.label(
                RichText::new(if summary.is_empty() { "depleted".to_string() } else { summary.join(", ") })
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.label(RichText::new("Mine").size(theme.font_size_small).color(theme.accent()));
        });
    let resp = inner.response.interact(egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Which asteroid's mining modal is open + the per-asteroid hold draft (until launch).
#[derive(Default)]
struct MiningEditState {
    open: Option<String>,
    draft: std::collections::HashMap<String, Vec<(String, u32)>>,
}

fn with_mining_edit<R>(f: impl FnOnce(&mut MiningEditState) -> R) -> R {
    thread_local! {
        static S: RefCell<MiningEditState> = RefCell::new(MiningEditState::default());
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

#[cfg(test)]
pub(crate) fn test_open_mining_edit(asteroid_id: &str) {
    with_mining_edit(|m| m.open = Some(asteroid_id.to_string()));
}

#[cfg(test)]
pub(crate) fn test_close_mining_edit() {
    with_mining_edit(|m| m.open = None);
}

/// Test hook: select a placed-pool item by index, so the snapshot harness can render
/// the inspect + "Move to" transfer card (otherwise opened by a click).
#[cfg(test)]
pub(crate) fn test_select_placed(idx: usize) {
    with_placed_sel(|s| *s = Some(idx));
}

/// Test hook: clear the placed selection so a later snapshot on the shared thread is
/// not polluted by a leftover selection.
#[cfg(test)]
pub(crate) fn test_clear_placed() {
    with_placed_sel(|s| *s = None);
}

#[cfg(test)]
thread_local! {
    /// Container-header rects recorded during a draw, keyed by container path, so the
    /// headless INTERACTION harness can locate a header to click (egui has no
    /// query-widget-by-content API; we record the rect at layout time).
    static RECORDED_HEADER_RECTS: RefCell<std::collections::HashMap<String, egui::Rect>> =
        RefCell::new(std::collections::HashMap::new());
    /// Paths whose header registered a click on the most recent draw (debug/assert hook).
    static RECORDED_HEADER_CLICKS: RefCell<std::collections::HashSet<String>> =
        RefCell::new(std::collections::HashSet::new());
}

/// Test hook: did the container header at `path` register a click on the last draw?
#[cfg(test)]
pub(crate) fn test_header_was_clicked(path: &str) -> bool {
    RECORDED_HEADER_CLICKS.with(|r| r.borrow().contains(path))
}

/// Test hook: the recorded clickable rect of the container header at `path` (the
/// inventory renderer's path scheme, e.g. "1" for the second top-level place). `None`
/// if that container was not laid out this frame (collapsed ancestor / scrolled off).
#[cfg(test)]
pub(crate) fn test_recorded_header_rect(path: &str) -> Option<egui::Rect> {
    RECORDED_HEADER_RECTS.with(|r| r.borrow().get(path).copied())
}

/// Test hook: clear recorded header rects before a fresh interaction run.
#[cfg(test)]
pub(crate) fn test_clear_recorded_rects() {
    RECORDED_HEADER_RECTS.with(|r| r.borrow_mut().clear());
}

/// Test hook: render just the mining map (the snapshot harness can't reliably reach
/// it deep in the inventory due to shared egui collapse state).
#[cfg(test)]
pub(crate) fn draw_mining_map_for_test(
    ui: &mut egui::Ui,
    theme: &Theme,
    asteroids: &[GuiAsteroid],
    drones: &[GuiDrone],
) {
    draw_mining_map(ui, theme, asteroids, drones);
}

/// Set an ore's allocation in a manifest draft (insert/update/remove-at-zero).
fn set_draft_units(draft: &mut Vec<(String, u32)>, ore: &str, units: u32) {
    if units == 0 {
        draft.retain(|(o, _)| o != ore);
    } else if let Some(slot) = draft.iter_mut().find(|(o, _)| o == ore) {
        slot.1 = units;
    } else {
        draft.push((ore.to_string(), units));
    }
}

/// The per-asteroid mining modal: allocate the drone hold across THIS asteroid's ores
/// (bounded by each ore's stock + the hold capacity), then launch the drone to mine it.
fn mining_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(ast_id) = with_mining_edit(|m| m.open.clone()) else {
        return;
    };
    let Some(ast) = state.asteroids.iter().find(|a| a.id == ast_id).cloned() else {
        with_mining_edit(|m| m.open = None);
        return;
    };
    let cap = crate::systems::mining::DRONE_CAPACITY;
    let drone_active = state.drone_active;
    let mut draft = with_mining_edit(|m| m.draft.entry(ast_id.clone()).or_default().clone());
    let mut launch = false;
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new(("mining_edit", &ast_id)))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.set_min_width(440.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 6.0, widgets::swatch_color(&ast.classification));
                ui.label(RichText::new(&ast.name).size(theme.font_size_heading).strong().color(theme.text_primary()));
            });
            ui.label(
                RichText::new(format!("Class {} · {:.0} km away (a farther rock is a longer trip)", ast.classification, ast.distance))
                    .color(theme.text_secondary())
                    .size(theme.font_size_small),
            );
            let total: u32 = draft.iter().map(|(_, u)| u).sum();
            ui.label(
                RichText::new(format!("Drone hold: {total}/{cap} units. The drone mines ONLY this asteroid; the haul is capped by what it holds."))
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
            manifest_bar(ui, theme, &draft, cap);
            ui.separator();
            for (ore, avail) in ast.ores.iter().filter(|(_, q)| *q >= 1.0) {
                let cur = draft.iter().find(|(o, _)| o == ore).map(|(_, u)| *u).unwrap_or(0);
                let ore_cap = (*avail as u32).min(cap);
                ui.horizontal(|ui| {
                    widgets::row_cell(ui, 150.0, |ui| {
                        ui.label(RichText::new(ore_short(ore)).size(theme.font_size_small).color(theme.text_primary()));
                    });
                    ui.label(RichText::new(format!("{:.0} left", avail)).size(theme.font_size_small).color(theme.text_muted()));
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if widgets::stepper_button(ui, theme, "-", cur > 0, false) {
                        set_draft_units(&mut draft, ore, cur.saturating_sub(1));
                    }
                    ui.label(RichText::new(format!("{cur}")).color(theme.text_primary()));
                    let can_inc = total < cap && cur < ore_cap;
                    if widgets::stepper_button(ui, theme, "+", can_inc, true) {
                        set_draft_units(&mut draft, ore, cur + 1);
                    }
                });
            }
            ui.add_space(theme.spacing_sm);
            // Standing order (economy automation, v0.663): with this checked, the
            // drone re-launches the same trip automatically after every delivery
            // until the asteroid runs out or the box is unchecked.
            ui.checkbox(
                &mut state.auto_mine_enabled,
                RichText::new("Keep mining (auto-relaunch this trip after each delivery)")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                let any = draft.iter().any(|(_, u)| *u > 0);
                ui.add_enabled_ui(any && !drone_active, |ui| {
                    if widgets::primary_button(ui, theme, "Launch drone") {
                        launch = true;
                    }
                });
                if widgets::secondary_button(ui, theme, "Cancel") {
                    cancel = true;
                }
                if drone_active {
                    ui.label(RichText::new("A drone is already out.").size(theme.font_size_small).color(theme.text_muted()));
                }
            });
        });
    if launch {
        let manifest: Vec<(String, u32)> = draft.into_iter().filter(|(_, u)| *u > 0).collect();
        if !manifest.is_empty() {
            state.pending_drone_manifest = Some((ast_id.clone(), manifest));
        }
        with_mining_edit(|m| {
            m.open = None;
            m.draft.remove(&ast_id);
        });
    } else if cancel || modal.should_close() {
        // Cancel / backdrop / Escape: drop the in-progress draft so a reopen starts
        // clean + stock-consistent (no stale, now-impossible allocation).
        with_mining_edit(|m| {
            m.open = None;
            m.draft.remove(&ast_id);
        });
    } else {
        // Still open: persist the in-progress allocation between frames.
        with_mining_edit(|m| {
            m.draft.insert(ast_id.clone(), draft.clone());
        });
    }
}

/// A small top-down MINING MAP: home at the centre, each asteroid a dot at its (x, z)
/// position (labelled with name + distance), and the active drone a dot travelling
/// along the line to its target — so you can watch the drone go off to mine and come
/// back. All colours are theme tokens / data-seeded swatches.
fn draw_mining_map(ui: &mut egui::Ui, theme: &Theme, asteroids: &[GuiAsteroid], drones: &[GuiDrone]) {
    let h = 200.0;
    let w = ui.available_width().min(560.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(6), theme.bg_card());
    painter.rect_stroke(rect, Rounding::same(6), Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
    let center = rect.center();
    let max_d = asteroids.iter().map(|a| a.distance).fold(1.0f32, f32::max);
    let margin = 48.0;
    let scale = ((rect.width().min(rect.height()) / 2.0 - margin) / max_d).max(0.01);
    let proj = |p: [f32; 3]| egui::pos2(center.x + p[0] * scale, center.y + p[2] * scale);
    let font = egui::FontId::proportional(theme.font_size_small);
    // Active routes first (under the dots): home -> the drone's target asteroid, drawn
    // in accent so the journey reads at a glance (the operator wanted to SEE the drone
    // going off to mine).
    for d in drones {
        if let Some(ta) = asteroids.iter().find(|a| a.id == d.target) {
            painter.line_segment([center, proj(ta.position)], Stroke::new(1.5, theme.accent()));
        }
    }
    // Asteroids (labels to the right of their dot).
    for a in asteroids {
        let sp = proj(a.position);
        painter.circle_filled(sp, 4.0, widgets::swatch_color(&a.classification));
        painter.text(sp + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, format!("{} · {:.0}km", a.name, a.distance), font.clone(), theme.text_secondary());
    }
    // Home at the centre. Label sits to the LEFT so it never collides with an outbound
    // drone (which heads right toward the asteroids).
    painter.circle_filled(center, 5.0, theme.accent());
    painter.text(center + Vec2::new(-8.0, 0.0), egui::Align2::RIGHT_CENTER, "Home", font.clone(), theme.text_muted());
    // The drone(s), mid-journey. Skip drawing when parked at home (the Home dot already
    // marks that spot); otherwise label ABOVE the dot with its phase + cargo so its
    // status reads without overlapping the home or asteroid labels.
    for d in drones {
        let dp = proj(d.pos);
        if dp.distance(center) < 4.0 {
            continue;
        }
        painter.circle_filled(dp, 4.0, theme.warning());
        let label = if d.cargo_total > 0 {
            format!("drone · {} · {} ore", d.phase.to_lowercase(), d.cargo_total)
        } else {
            format!("drone · {}", d.phase.to_lowercase())
        };
        painter.text(dp + Vec2::new(0.0, -8.0), egui::Align2::CENTER_BOTTOM, label, font.clone(), theme.warning());
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Calculate carry weight from inventory items + populate equipped slots from
    // the loaded `data/inventory/equipment_slots.json` (lazily — guards against
    // the GUI rendering before lib.rs has wired the loaded data into GuiState).
    with_state(|ps| {
        if ps.equipped.is_empty() && !state.equipment_slots.is_empty() {
            ps.equipped = state.equipment_slots.iter()
                .map(|(id, _)| (id.clone(), None))
                .collect();
        }
        if !ps.initialized {
            let mut weight = 0.0f32;
            for slot in &state.inventory_items {
                if let Some(item) = slot {
                    if let Some(details) = lookup_item_details(&item.item_id) {
                        weight += details.weight_kg * item.quantity as f32;
                    }
                }
            }
            ps.carry_weight = weight;
            ps.initialized = true;
        }
    });

    // The selected item's inline expand-in-place card records its quick action here
    // (Eat/Drink/Plant/Equip/Drop); applied to GuiState after the panel closes (the
    // card only borrows an inventory snapshot, never GuiState mid-render).
    let mut item_acts = ItemCardActions::default();
    // Crop actions come from the Garden section in the central panel; applied after.
    let mut action_water_crop: Option<u64> = None;
    let mut action_harvest_crop: Option<u64> = None;
    let mut action_dev_grow = false;
    // "Dev: stock seeds" — the seed item ids of the starter set to grant.
    let mut action_stock_seeds: Option<Vec<String>> = None;
    // Plant a whole tower (v0.386): (tower id, plant ids) to spawn as crops, set by
    // the Garden "Plant a tower" buttons, applied to GuiState after the panel.
    let mut action_plant_tower: Option<(String, Vec<String>)> = None;
    let mut action_rest = false;
    let mut action_compost = false;
    let mut action_fertilize_crop: Option<u64> = None;
    // ── ONE PANEL (operator 2026-06-08: back to a single panel; the 3-panel split
    //    read as stair-stepped). Everything in one CentralPanel + a vertical scroll,
    //    widgets aligned, the status bars capped at theme.status_bar_width. The
    //    detail block shows only when something is selected. ──
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            ui.label(RichText::new("Inventory").size(theme.font_size_title).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);

            // Creative / survival mode (default Creative during early dev): in
            // Creative, planting + crafting don't need or consume resources, so the
            // seed/material economy can be built out before it actually bites.
            widgets::toggle(ui, theme, "Creative mode", &mut state.creative_mode);
            ui.label(
                RichText::new(if state.creative_mode {
                    "Creative: plant + craft freely, nothing consumed."
                } else {
                    "Survival: planting + crafting consume seeds + materials."
                })
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            // Collapse/Expand/Start-collapsed controls (operator 2026-06-08): ONE
            // control set driving every collapsible SECTION below + their nested
            // trees. Rendered up top so `tree_force` is set before any section reads
            // it (the buttons mutate it THIS frame; sections below pick it up).
            let mut tree_force: Option<bool> = None;
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "Collapse all") {
                    tree_force = Some(false);
                }
                if widgets::secondary_button(ui, theme, "Expand all") {
                    tree_force = Some(true);
                }
                // "Start collapsed" as a proper bordered button — the whole area is
                // clickable; .active() renders the ON state like a pressed button.
                if widgets::Button::new("Start collapsed")
                    .active(state.trees_start_collapsed)
                    .show(ui, theme)
                {
                    state.trees_start_collapsed = !state.trees_start_collapsed;
                    tree_force = Some(!state.trees_start_collapsed);
                }
            });
            let tree_default_open = !state.trees_start_collapsed;
            ui.add_space(theme.spacing_sm);

            // ── Status (collapsible) — the live player vitals. Body renders only
            //    when the section is open; closes just before the Equipment divider. ──
            if widgets::section_disclosure(ui, theme, ("inv_sec", "status"), "Status", tree_force) {

            // Live player vitals as a GRID OF TILES (name + value + a chunky
            // colour-by-level bar) instead of thin text rows — using the width and
            // reading at a glance. Weight is always shown; the survival vitals when
            // the ECS has synced them (satiation_max > 0).
            let (carry_weight, max_weight) = with_state(|ps| (ps.carry_weight, ps.max_carry_weight));
            let weight_frac =
                if max_weight > 0.0 { (carry_weight / max_weight).clamp(0.0, 1.0) } else { 0.0 };
            let weight_color = if weight_frac > 0.9 {
                theme.danger()
            } else if weight_frac > 0.7 {
                theme.warning()
            } else {
                theme.accent()
            };
            let color_for = |frac: f32| {
                if frac < 0.25 {
                    theme.danger()
                } else if frac < 0.5 {
                    theme.warning()
                } else {
                    theme.accent()
                }
            };

            // (label, value, fraction 0..1, colour) for each tile.
            let mut tiles: Vec<(&str, String, f32, Color32)> = Vec::new();
            tiles.push((
                "Weight",
                format!("{:.1} / {:.1} kg", carry_weight, max_weight),
                weight_frac,
                weight_color,
            ));
            let effects = state.vitals.effects.clone();
            let has_vitals = state.vitals.satiation_max > 0.0;
            if has_vitals {
                let v = &state.vitals;
                let sat_frac = (v.satiation / v.satiation_max).clamp(0.0, 1.0);
                let hyd_frac = (v.hydration / v.hydration_max.max(1.0)).clamp(0.0, 1.0);
                let energy_frac = (v.energy / v.energy_max.max(1.0)).clamp(0.0, 1.0);
                let oxy_frac = (v.oxygen / v.oxygen_max.max(1.0)).clamp(0.0, 1.0);
                let waste_frac = (v.waste / v.waste_max.max(1.0)).clamp(0.0, 1.0);
                // High waste is BAD — inverted colour vs the other vitals.
                let waste_col = if waste_frac > 0.75 {
                    theme.danger()
                } else if waste_frac > 0.5 {
                    theme.warning()
                } else {
                    theme.text_secondary()
                };
                tiles.push(("Satiation", format!("{:.0} / {:.0}", v.satiation, v.satiation_max), sat_frac, color_for(sat_frac)));
                tiles.push(("Hydration", format!("{:.0} / {:.0}", v.hydration, v.hydration_max), hyd_frac, color_for(hyd_frac)));
                tiles.push(("Energy", format!("{:.0} / {:.0}", v.energy, v.energy_max), energy_frac, color_for(energy_frac)));
                tiles.push(("Oxygen", format!("{:.0} / {:.0}", v.oxygen, v.oxygen_max), oxy_frac, color_for(oxy_frac)));
                tiles.push(("Waste", format!("{:.0} / {:.0}", v.waste, v.waste_max), waste_frac, waste_col));
            }

            ui.add_space(theme.spacing_xs);
            // Three tiles across so the vitals use the width instead of stacking
            // into one thin left-hugging column. Tiles are read-only, so they go
            // straight into the columns.
            let cols_n = 3usize;
            ui.columns(cols_n, |cols| {
                for (i, (name, value, frac, color)) in tiles.iter().enumerate() {
                    let c = &mut cols[i % cols_n];
                    vital_tile(c, theme, name, value, *frac, *color);
                    c.add_space(theme.spacing_sm);
                }
            });

            if has_vitals {
                // Body temperature + seal status as a readout line under the grid.
                let temp = state.vitals.body_temp_c;
                let temp_col = if temp < 35.0 || temp > 39.0 {
                    theme.danger()
                } else if temp < 36.0 || temp > 38.0 {
                    theme.warning()
                } else {
                    theme.accent()
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Body temp").color(theme.text_secondary()).size(theme.font_size_small));
                    ui.label(
                        RichText::new(format!("{:.1}°C", temp))
                            .color(temp_col)
                            .size(theme.font_size_small)
                            .strong(),
                    );
                    ui.add_space(theme.spacing_md);
                    if state.vitals.sealed {
                        ui.label(RichText::new("Sealed").size(theme.font_size_small).color(theme.accent()));
                    } else {
                        ui.label(RichText::new("EXPOSED, no air!").size(theme.font_size_small).color(theme.danger()));
                    }
                });
            }

            // Survival actions.
            ui.add_space(theme.spacing_sm);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "Rest") {
                    action_rest = true;
                }
                if widgets::secondary_button(ui, theme, "Compost") {
                    action_compost = true;
                }
            });
            if !effects.is_empty() {
                ui.add_space(theme.spacing_xs);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Effects:").color(theme.text_secondary()));
                    for (name, remaining) in &effects {
                        let label = if *remaining >= 60.0 {
                            format!("{} ({:.0}m)", name, remaining / 60.0)
                        } else {
                            format!("{} ({:.0}s)", name, remaining)
                        };
                        egui::Frame::none()
                            .fill(theme.bg_secondary())
                            .rounding(Rounding::same(3))
                            .inner_margin(Vec2::new(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(label)
                                        .size(theme.font_size_small)
                                        .color(theme.text_primary()),
                                );
                            });
                    }
                });
            }

            } // ── end Status ──

            widgets::rgb_section_divider(ui, theme);

                // ── Equipment (collapsible) — closes before the You & places divider ──
                if widgets::section_disclosure(ui, theme, ("inv_sec", "equipment"), "Equipment", tree_force) {
                ui.add_space(theme.spacing_xs);

                ui.horizontal_wrapped(|ui| {
                    with_state(|ps| {
                        for (slot_id, equipped_item) in &ps.equipped {
                            let label = state.equipment_slots.iter()
                                .find(|(id, _)| id == slot_id)
                                .map(|(_, name)| name.as_str())
                                .unwrap_or(slot_id.as_str());

                            let slot_size = 64.0;
                            ui.vertical(|ui| {
                                let (rect, _response) = ui.allocate_exact_size(
                                    Vec2::splat(slot_size),
                                    egui::Sense::click(),
                                );

                                let fill = theme.bg_secondary();
                                let stroke = Stroke::new(1.0, theme.border());
                                ui.painter().rect_filled(rect, Rounding::same(4), fill);
                                ui.painter().rect_stroke(rect, Rounding::same(4), stroke, egui::StrokeKind::Outside);

                                if let Some(item_name) = equipped_item {
                                    let icon = item_name.chars().next().unwrap_or('?').to_string();
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &icon,
                                        egui::FontId::proportional(18.0),
                                        theme.text_primary(),
                                    );
                                } else {
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "-",
                                        egui::FontId::proportional(14.0),
                                        theme.text_muted(),
                                    );
                                }

                                ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_muted()));
                            });
                        }
                    });
                });

                } // ── end Equipment ──

                widgets::rgb_section_divider(ui, theme);

                // Your entity / container tree — top-level entities (You, your
                // home, a vehicle, …), each a container with its own contents,
                // colour-coded by kind so "what is where" reads at a glance. The
                // spine is data-driven (data/places/seed.json → state.places); the
                // live backpack contents inject at the node marked kind:"backpack".
                // No place data → flat backpack fallback.
                // ── You & places (collapsible) — closes before the Garden divider ──
                let header = if state.places.is_empty() { "Backpack" } else { "You & your places" };
                if widgets::section_disclosure(ui, theme, ("inv_sec", "places"), header, tree_force) {
                ui.add_space(theme.spacing_xs);

                // Nested-container inventory (operator 2026-06-22): each place renders
                // as a card with item TILES + its child containers nested inside, so
                // "what is where" reads spatially (person -> shirt -> pocket -> wallet)
                // and multiple inventories are visible at once. Live backpack items
                // inject at the kind:"backpack" node; seeded items live in any
                // container's `items`. Clicking a tile selects it; its card shows below.
                let mut places_out = PlacesOut::default();
                let placed_sel = with_placed_sel(|s| *s);
                if !state.places.is_empty() {
                    let places = state.places.clone();
                    for (i, place) in places.iter().enumerate() {
                        draw_container(
                            ui,
                            theme,
                            place,
                            &state.inventory_items,
                            &state.placed_items,
                            &i.to_string(),
                            state.selected_slot,
                            placed_sel,
                            &mut places_out,
                        );
                        ui.add_space(theme.spacing_xs);
                    }
                } else if state.inventory_items.iter().all(|s| s.is_none()) {
                    ui.label(
                        RichText::new("Empty, mine, craft, or dev-stock to fill it.")
                            .color(theme.text_muted()),
                    );
                } else {
                    // No place spine: a single flat backpack grid of tiles.
                    ui.horizontal_wrapped(|ui| {
                        for (i, slot) in state.inventory_items.iter().enumerate() {
                            if let Some(it) = slot {
                                if item_tile(ui, theme, &it.name, &it.item_id, it.quantity, state.selected_slot == Some(i)) {
                                    places_out.clicked_slot = Some(i);
                                }
                            }
                        }
                    });
                }

                // Apply tile clicks to selection (toggle); live + placed are exclusive,
                // and either clears the garden selection.
                if let Some(i) = places_out.clicked_slot {
                    state.selected_slot = if state.selected_slot == Some(i) { None } else { Some(i) };
                    state.garden_selection = None;
                    with_placed_sel(|s| *s = None);
                }
                if let Some(idx) = places_out.clicked_placed {
                    with_placed_sel(|s| *s = if *s == Some(idx) { None } else { Some(idx) });
                    state.selected_slot = None;
                    state.garden_selection = None;
                }

                // The selected item's card, shown once below the section. A live
                // backpack item gets the full action card; a placed item gets an
                // inspect card + a "Move to" menu (the organize-layer transfer).
                if let Some(i) = state.selected_slot {
                    if let Some(Some(it)) = state.inventory_items.get(i) {
                        let it = it.clone();
                        let containers = crate::gui::collect_containers(&state.places);
                        let mut stash_to: Option<String> = None;
                        ui.add_space(theme.spacing_xs);
                        widgets::card(ui, theme, |ui| {
                            draw_item_card(ui, theme, &it, Some(&mut item_acts));
                            // Stash the whole stack OUT of the live backpack into a
                            // container (the backpack <-> container half of transfer).
                            if !containers.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Stash to")
                                            .size(theme.font_size_small)
                                            .color(theme.text_secondary()),
                                    );
                                    egui::ComboBox::from_id_salt("backpack_stash_to")
                                        .selected_text("a container")
                                        .show_ui(ui, |ui| {
                                            for (p, label) in &containers {
                                                if ui.selectable_label(false, label.as_str()).clicked() {
                                                    stash_to = Some(p.clone());
                                                }
                                            }
                                        });
                                });
                            }
                        });
                        if let Some(target) = stash_to {
                            state.pending_inventory_transfers.push((it.item_id.clone(), it.quantity, false));
                            state.placed_items.push(crate::gui::PlacedItem {
                                key: it.item_id.clone(),
                                name: it.name.clone(),
                                qty: it.quantity,
                                container: target,
                            });
                            state.selected_slot = None;
                        }
                    }
                } else if let Some(idx) = with_placed_sel(|s| *s) {
                    if let Some(pi) = state.placed_items.get(idx).cloned() {
                        let synth = crate::gui::GuiItemSlot {
                            name: pi.name.clone(),
                            item_id: pi.key.clone(),
                            quantity: pi.qty,
                        };
                        let containers = crate::gui::collect_containers(&state.places);
                        let mut move_to: Option<String> = None;
                        let mut take_to_backpack = false;
                        ui.add_space(theme.spacing_xs);
                        widgets::card(ui, theme, |ui| {
                            draw_item_card(ui, theme, &synth, None);
                            ui.add_space(theme.spacing_xs);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Move to")
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                                let cur = containers
                                    .iter()
                                    .find(|(p, _)| *p == pi.container)
                                    .map(|(_, l)| l.clone())
                                    .unwrap_or_else(|| "(here)".to_string());
                                egui::ComboBox::from_id_salt("placed_move_to")
                                    .selected_text(cur)
                                    .show_ui(ui, |ui| {
                                        for (p, label) in &containers {
                                            if *p != pi.container
                                                && ui.selectable_label(false, label.as_str()).clicked()
                                            {
                                                move_to = Some(p.clone());
                                            }
                                        }
                                    });
                                // Pull this item INTO the live backpack.
                                if widgets::secondary_button(ui, theme, "Take to backpack") {
                                    take_to_backpack = true;
                                }
                            });
                        });
                        if take_to_backpack {
                            state.pending_inventory_transfers.push((pi.key.clone(), pi.qty, true));
                            state.placed_items.remove(idx);
                            with_placed_sel(|s| *s = None);
                        } else if let Some(target) = move_to {
                            if let Some(p) = state.placed_items.get_mut(idx) {
                                p.container = target;
                            }
                        }
                    } else {
                        with_placed_sel(|s| *s = None);
                    }
                }

                } // ── end You & places ──

                // ── GARDEN (collapsible) — numbered SLOT rows per tower; click a
                //    slot to expand its multi-row plant card. The Dev seed/grow
                //    buttons sit in the body. Closes before the Mining divider. ──
                widgets::rgb_section_divider(ui, theme);
                if widgets::section_disclosure(ui, theme, ("inv_sec", "garden"), "Garden", tree_force) {
                ui.horizontal_wrapped(|ui| {
                    // Dev: grant the starter seed set (one of each tower variety), so
                    // survival-mode planting is testable now. Creative ignores seeds.
                    if theme.cheats_enabled && widgets::secondary_button(ui, theme, "Dev: stock seeds") {
                        let mut seeds: Vec<String> = Vec::new();
                        for t in &state.tower_configs {
                            for p in &t.plantings {
                                let sid = format!("seed_{}_0", p.plant);
                                if !seeds.contains(&sid) {
                                    seeds.push(sid);
                                }
                            }
                        }
                        if !seeds.is_empty() {
                            action_stock_seeds = Some(seeds);
                        }
                    }
                    if theme.cheats_enabled && !state.crops.is_empty() && widgets::secondary_button(ui, theme, "Dev: grow all") {
                        action_dev_grow = true;
                    }
                });
                ui.add_space(theme.spacing_xs);
                // Grow-area overview: EVERY growing machine in the home's garden
                // (towers, beds, racks, tanks, fields) and how many of each, as tiles,
                // so the section shows the whole garden at a glance, not just the two
                // plantable tower designs. Data-driven from data/machines/home.ron.
                let areas = state.garden_areas.clone();
                if !areas.is_empty() {
                    let total: u32 = areas.iter().map(|a| a.count).sum();
                    ui.label(
                        RichText::new(format!("{} grow areas, {} kinds", total, areas.len()))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.add_space(theme.spacing_xs);
                    let gcols = 4usize;
                    ui.columns(gcols, |cols| {
                        for (i, a) in areas.iter().enumerate() {
                            let c = &mut cols[i % gcols];
                            if garden_area_tile(c, theme, a) {
                                let mid = a.machine_id.clone();
                                with_garden_edit(|s| s.open = Some(mid));
                            }
                            c.add_space(theme.spacing_sm);
                        }
                    });
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new("Plantable tower designs")
                            .strong()
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_xs);
                }
                // Garden as an aligned TABLE grouped by tower (operator 2026-06-08:
                // rows/columns spreadsheet design, not a tree+detail). One collapsible
                // group per tower (a Plant button in its body), each a compact egui::Grid
                // with fixed columns; the growth bar is capped at theme.status_bar_width.
                // Seed-planted crops fall under "Other crops".
                if state.tower_configs.is_empty() && state.crops.is_empty() {
                    ui.label(
                        RichText::new("No garden plots yet. Add an aeroponic tower design at Home.")
                            .color(theme.text_muted()),
                    );
                } else {
                    // Group order: one per configured tower, then "Other crops" if any
                    // seed-planted (tower-less) crops exist.
                    let mut groups: Vec<(Option<String>, String)> = state
                        .tower_configs
                        .iter()
                        .map(|t| (Some(t.id.clone()), t.name.clone()))
                        .collect();
                    if state.crops.iter().any(|c| c.tower_id.is_none()) {
                        groups.push((None, "Other crops".to_string()));
                    }
                    for (gi, (tid, title)) in groups.iter().enumerate() {
                        let crops: Vec<&crate::gui::GuiCrop> =
                            state.crops.iter().filter(|c| &c.tower_id == tid).collect();
                        let ready = crops.iter().filter(|c| c.mature).count();
                        // The tower's CONFIG (None for the "Other crops" group, which
                        // holds loose seed-planted crops with no fixed-slot design).
                        let cfg = tid
                            .as_ref()
                            .and_then(|id| state.tower_configs.iter().find(|t| &t.id == id));
                        // Per-slot planned spec (plant id, role, note), flattened from the
                        // plantings EXACTLY as the farming handler assigns tower_slot, so
                        // slot index lines up with crop.tower_slot. Empty for "Other crops".
                        let slot_specs: Vec<(String, String, String)> = cfg
                            .map(|t| {
                                t.plantings
                                    .iter()
                                    .flat_map(|p| {
                                        std::iter::repeat((
                                            p.plant.clone(),
                                            p.role.clone(),
                                            p.note.clone(),
                                        ))
                                        .take(p.slots.max(1) as usize)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        // Slot rows to render: the design's planned slots for a real tower,
                        // else one row per loose crop (the "Other crops" fallback).
                        let slot_count = if cfg.is_some() { slot_specs.len() } else { crops.len() };
                        let count_txt = if slot_count == 0 {
                            "no slots".to_string()
                        } else if crops.is_empty() {
                            format!("{} slots · not planted", slot_count)
                        } else {
                            format!("{}/{} ready · {} slots", ready, crops.len(), slot_count)
                        };
                        // Make/model/version subtitle for the tower title row (operator
                        // 2026-06-08: "aeroponic tower make model version"). Data-driven.
                        let subtitle = cfg
                            .map(|t| {
                                let mut parts: Vec<String> = Vec::new();
                                if !t.make.is_empty() {
                                    parts.push(t.make.clone());
                                }
                                if !t.model.is_empty() {
                                    parts.push(t.model.clone());
                                }
                                if !t.version.is_empty() {
                                    parts.push(format!("v{}", t.version));
                                }
                                parts.join(" ")
                            })
                            .unwrap_or_default();
                        // Resolve the tower's plant-id list up front (so the Plant button
                        // doesn't borrow state inside the body closure).
                        let plant_ids: Option<(String, Vec<String>)> = cfg.map(|t| {
                            let ids: Vec<String> = t
                                .plantings
                                .iter()
                                .flat_map(|p| {
                                    std::iter::repeat(p.plant.clone()).take(p.slots.max(1) as usize)
                                })
                                .collect();
                            (t.id.clone(), ids)
                        });
                        let planted_label = if crops.is_empty() { "Plant this tower" } else { "Plant again" };
                        widgets::expandable_row(
                            ui,
                            ("garden_grp", gi),
                            tree_default_open,
                            tree_force,
                            |ui| {
                                // Tower title row: name + make/model/version + ready
                                // count + the Plant button (widgets in the title row).
                                ui.label(RichText::new(title).strong().color(theme.accent()));
                                if !subtitle.is_empty() {
                                    ui.label(RichText::new(format!("· {subtitle}")).size(theme.font_size_small).color(theme.text_muted()));
                                }
                                ui.label(RichText::new(format!("· {count_txt}")).size(theme.font_size_small).color(theme.text_secondary()));
                                if let Some((tid2, ids)) = &plant_ids {
                                    if widgets::compact_button(ui, theme, planted_label, widgets::ButtonVariant::Secondary) {
                                        action_plant_tower = Some((tid2.clone(), ids.clone()));
                                    }
                                }
                            },
                            |ui| {
                                if slot_count == 0 {
                                    ui.label(
                                        RichText::new("This tower has no plantings yet.")
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                    return;
                                }
                                // Prettify a plant id ("cherry_tomato" -> "Cherry Tomato")
                                // for slots not yet grown (no GuiCrop to read a name from).
                                let prettify = |id: &str| -> String {
                                    id.split(['_', '-', ' '])
                                        .filter(|w| !w.is_empty())
                                        .map(|w| {
                                            let mut ch = w.chars();
                                            match ch.next() {
                                                Some(f) => {
                                                    f.to_uppercase().collect::<String>() + ch.as_str()
                                                }
                                                None => String::new(),
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                };
                                // One SLOT row per slot (operator 2026-06-08: "rows of like
                                // slot 1, 2, 3 ... click on the specific plant can expand
                                // that single row to a multirow card"). Each slot is itself
                                // an expandable_row whose body is the plant CARD.
                                for s in 0..slot_count {
                                    let spec = slot_specs.get(s);
                                    // Live crop in this slot: real tower -> by tower_slot;
                                    // loose-crops fallback -> positional.
                                    let crop: Option<&crate::gui::GuiCrop> = if cfg.is_some() {
                                        crops.iter().copied().find(|c| c.tower_slot == Some(s as u32))
                                    } else {
                                        crops.get(s).copied()
                                    };
                                    let plant_id = spec
                                        .map(|sp| sp.0.clone())
                                        .or_else(|| crop.map(|c| c.name.clone()))
                                        .unwrap_or_default();
                                    let role = spec.map(|sp| sp.1.clone()).unwrap_or_default();
                                    let note = spec.map(|sp| sp.2.clone()).unwrap_or_default();
                                    let name = crop.map(|c| c.name.clone()).unwrap_or_else(|| {
                                        if plant_id.is_empty() {
                                            format!("Slot {}", s + 1)
                                        } else {
                                            prettify(&plant_id)
                                        }
                                    });
                                    let name_body = name.clone();
                                    let swatch = widgets::swatch_color(if plant_id.is_empty() {
                                        &name
                                    } else {
                                        &plant_id
                                    });
                                    let glyph = name
                                        .chars()
                                        .next()
                                        .map(|c| c.to_uppercase().to_string())
                                        .unwrap_or_default();
                                    widgets::expandable_row(
                                        ui,
                                        ("garden_slot", gi, s),
                                        false,
                                        tree_force,
                                        |ui| {
                                            widgets::row_cell(ui, theme.cell_narrow_width, |ui| {
                                                ui.label(
                                                    RichText::new(format!("Slot {}", s + 1))
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            });
                                            widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                                let col = match crop {
                                                    Some(c) if c.dead => theme.danger(),
                                                    Some(c) if c.mature => theme.accent(),
                                                    Some(_) => theme.text_primary(),
                                                    None => theme.text_muted(),
                                                };
                                                ui.label(RichText::new(&name).color(col));
                                            });
                                            widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                                let (txt, col) = match crop {
                                                    Some(c) if c.dead => {
                                                        ("dead".to_string(), theme.danger())
                                                    }
                                                    Some(c) if c.mature => {
                                                        ("ready".to_string(), theme.accent())
                                                    }
                                                    Some(c) => {
                                                        (c.stage.clone(), theme.text_secondary())
                                                    }
                                                    None => {
                                                        ("planned".to_string(), theme.text_muted())
                                                    }
                                                };
                                                ui.label(
                                                    RichText::new(txt)
                                                        .size(theme.font_size_small)
                                                        .color(col),
                                                );
                                            });
                                            if let Some(c) = crop {
                                                ui.add(
                                                    egui::ProgressBar::new(c.progress.clamp(0.0, 1.0))
                                                        .fill(theme.accent())
                                                        .desired_width(theme.status_bar_width)
                                                        .desired_height(theme.status_bar_height),
                                                );
                                            }
                                        },
                                        |ui| {
                                            // THE CARD — colored placeholder tile + name +
                                            // role + description (the planting note) + the
                                            // live details/actions when a crop occupies it.
                                            ui.add_space(theme.spacing_xs);
                                            ui.horizontal_top(|ui| {
                                                widgets::placeholder_tile(
                                                    ui, theme, swatch, 72.0, &glyph,
                                                );
                                                ui.add_space(theme.spacing_sm);
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        RichText::new(&name_body)
                                                            .size(theme.font_size_heading)
                                                            .strong()
                                                            .color(theme.accent()),
                                                    );
                                                    if !role.is_empty() {
                                                        ui.label(
                                                            RichText::new(&role)
                                                                .size(theme.font_size_small)
                                                                .italics()
                                                                .color(theme.text_muted()),
                                                        );
                                                    }
                                                    if !note.is_empty() {
                                                        ui.label(
                                                            RichText::new(&note)
                                                                .color(theme.text_secondary()),
                                                        );
                                                    }
                                                    ui.add_space(theme.spacing_xs);
                                                    match crop {
                                                        Some(c) => {
                                                            egui::Grid::new(("slot_card", gi, s))
                                                                .spacing([10.0, 2.0])
                                                                .show(ui, |ui| {
                                                                    let mut stat =
                                                                        |ui: &mut egui::Ui,
                                                                         k: &str,
                                                                         v: String| {
                                                                            ui.label(
                                                                                RichText::new(k)
                                                                                    .size(theme.font_size_small)
                                                                                    .color(theme.text_muted()),
                                                                            );
                                                                            ui.label(
                                                                                RichText::new(v)
                                                                                    .size(theme.font_size_small)
                                                                                    .color(theme.text_secondary()),
                                                                            );
                                                                            ui.end_row();
                                                                        };
                                                                    let stage = if c.dead {
                                                                        "dead".to_string()
                                                                    } else if c.mature {
                                                                        "ready".to_string()
                                                                    } else {
                                                                        c.stage.clone()
                                                                    };
                                                                    stat(ui, "Stage", stage);
                                                                    stat(ui, "Growth", format!("{:.0}%", c.progress * 100.0));
                                                                    stat(ui, "N·P·K", format!("{:.2} · {:.2} · {:.2}", c.n, c.p, c.k));
                                                                    stat(ui, "Water/day", format!("{:.1} L", c.water_per_day));
                                                                    stat(ui, "Temp window", format!("{:.0}-{:.0} °C", c.temp_min, c.temp_max));
                                                                    stat(ui, "Reservoir", format!("{:.0}%", c.water * 100.0));
                                                                    stat(ui, "Health", format!("{:.0}%", c.health));
                                                                });
                                                            ui.add_space(theme.spacing_xs);
                                                            ui.horizontal(|ui| {
                                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                                if c.mature && widgets::compact_button(ui, theme, "Harvest", widgets::ButtonVariant::Primary) {
                                                                    action_harvest_crop = Some(c.entity_bits);
                                                                }
                                                                if !c.dead && widgets::compact_button(ui, theme, "Water", widgets::ButtonVariant::Secondary) {
                                                                    action_water_crop = Some(c.entity_bits);
                                                                }
                                                                if !c.dead && widgets::compact_button(ui, theme, "Fertilize", widgets::ButtonVariant::Secondary) {
                                                                    action_fertilize_crop = Some(c.entity_bits);
                                                                }
                                                            });
                                                        }
                                                        None => {
                                                            ui.label(
                                                                RichText::new("Not yet planted. Use the Plant button above to fill empty slots.")
                                                                    .size(theme.font_size_small)
                                                                    .color(theme.text_muted()),
                                                            );
                                                        }
                                                    }
                                                });
                                            });
                                            ui.add_space(theme.spacing_xs);
                                        },
                                    );
                                }
                            },
                        );
                    }
                }
                } // ── end Garden ──

                widgets::rgb_section_divider(ui, theme);
                // ── Mining (collapsible) — commission drones to fetch ore from
                //    finite asteroids. Closes before the panel's scroll area. ──
                if widgets::section_disclosure(ui, theme, ("inv_sec", "mining"), "Mining", tree_force) {
                ui.add_space(theme.spacing_xs);
                if state.asteroids.is_empty() {
                    ui.label(RichText::new("No asteroids in range.").color(theme.text_muted()));
                } else {
                    let drone_active = state.drone_active;
                    ui.label(
                        RichText::new(if drone_active {
                            "Your drone is out on a run, one asteroid at a time."
                        } else {
                            "Pick an asteroid to mine. A drone mines ONE asteroid per run; the haul is capped by what that rock holds."
                        })
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                    );
                    ui.add_space(theme.spacing_xs);
                    // Asteroid CARDS: class swatch + name + distance + ore summary; click
                    // one to open its per-asteroid mining modal (matching the garden tiles).
                    let acols = 3usize;
                    let asts = state.asteroids.clone();
                    ui.columns(acols, |cols| {
                        for (i, ast) in asts.iter().enumerate() {
                            let c = &mut cols[i % acols];
                            if asteroid_card(c, theme, ast) && !drone_active {
                                with_mining_edit(|m| m.open = Some(ast.id.clone()));
                            }
                            c.add_space(theme.spacing_sm);
                        }
                    });
                    ui.add_space(theme.spacing_sm);
                    draw_mining_map(ui, theme, &asts, &state.drones);
                }
                ui.add_space(theme.spacing_xs);
                if !state.drones.is_empty() {
                    // The active drone (one per player) as an aligned EXPANDABLE row
                    // (v0.414): Stage | status | progress bar in the title row;
                    // expand for the manifest it's fetching + cargo on board.
                    for (di, drone) in state.drones.iter().enumerate() {
                        let (stage, desc) = match drone.phase.as_str() {
                            "Outbound" => ("1/3", "outbound"),
                            "Mining" => ("2/3", "mining"),
                            "Returning" => ("3/3", "returning"),
                            _ => ("done", "delivering"),
                        };
                        let target_name = state
                            .asteroids
                            .iter()
                            .find(|a| a.id == drone.target)
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| drone.target.clone());
                        widgets::expandable_row(
                            ui,
                            ("mining_drone", di),
                            false,
                            tree_force,
                            |ui| {
                                ui.label(RichText::new(format!("Drone → {target_name}")).size(theme.font_size_small).strong().color(theme.text_primary()));
                                ui.label(RichText::new(format!("· {desc} (stage {stage}) · {:.0} km", drone.distance)).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.add(egui::ProgressBar::new(drone.phase_progress.clamp(0.0, 1.0)).fill(theme.accent()).desired_width(theme.status_bar_width).desired_height(theme.status_bar_height));
                            },
                            |ui| {
                                let fetching: Vec<String> = drone
                                    .manifest
                                    .iter()
                                    .map(|(o, u)| format!("{}x {}", u, ore_short(o)))
                                    .collect();
                                ui.horizontal(|ui| {
                                    widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                        ui.label(RichText::new("Fetching").size(theme.font_size_small).color(theme.text_muted()));
                                    });
                                    ui.label(RichText::new(fetching.join(", ")).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                                ui.horizontal(|ui| {
                                    widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                        ui.label(RichText::new("Cargo").size(theme.font_size_small).color(theme.text_muted()));
                                    });
                                    ui.label(RichText::new(format!("{} units", drone.cargo_total)).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                            },
                        );
                    }
                }
                } // ── end Mining ──
        }); // close the single-panel ScrollArea
        }); // close the single CentralPanel

    // Apply the inline item card's quick action (set under the selected item row,
    // inside the panel) to GuiState now that the panel — and the snapshot-borrowing
    // card closure — are done. Drop clears the slot; Equip fills the first free
    // equipment slot; Eat/Drink/Plant bridge into the Food/Farming systems.
    if item_acts.drop {
        if let Some(idx) = state.selected_slot {
            if idx < state.inventory_items.len() {
                state.inventory_items[idx] = None;
                state.selected_slot = None;
                with_state(|ps| ps.initialized = false); // recalc weight
            }
        }
    }
    if item_acts.equip {
        if let Some(idx) = state.selected_slot {
            if let Some(Some(item)) = state.inventory_items.get(idx) {
                let item_name = item.name.clone();
                with_state(|ps| {
                    for slot in &mut ps.equipped {
                        if slot.1.is_none() {
                            slot.1 = Some(item_name.clone());
                            break;
                        }
                    }
                });
            }
        }
    }
    if let Some(item_id) = item_acts.eat {
        state.pending_consume_item = Some(item_id);
    }
    if let Some(item_id) = item_acts.drink {
        state.pending_drink_item = Some(item_id);
    }
    if let Some(seed_id) = item_acts.plant {
        state.pending_plant_seed = Some(seed_id);
    }
    if let Some(kit_id) = item_acts.deploy {
        state.pending_deploy_kit = Some(kit_id);
    }

    // Apply the Garden actions (set inside the central panel) to GuiState; the main
    // loop bridges these into FarmingSystem's command channels before the next tick.
    if let Some(ids) = action_plant_tower {
        state.pending_plant_tower = Some(ids);
    }
    if let Some(seeds) = action_stock_seeds {
        state.pending_stock_seeds = Some(seeds);
    }
    if let Some(bits) = action_water_crop {
        state.pending_water_crop = Some(bits);
    }
    if let Some(bits) = action_harvest_crop {
        state.pending_harvest_crop = Some(bits);
    }
    if action_dev_grow {
        state.dev_grow_crops = true;
    }
    // (The drone manifest is now built + launched in the per-asteroid mining modal,
    // which sets pending_drone_manifest = (asteroid id, manifest) directly.)
    // Bridge the Rest button to FoodSystem (refills energy).
    if action_rest {
        state.pending_rest = true;
    }
    // Bridge Compost (FoodSystem) + Fertilize (FarmingSystem).
    if action_compost {
        state.pending_compost = true;
    }
    if let Some(bits) = action_fertilize_crop {
        state.pending_fertilize_crop = Some(bits);
    }

    // Per-medium grow-area edit modal — shown at ctx level (a floating window) when
    // a Garden tile was clicked. Rendered after the panel so it overlays everything.
    garden_edit_modal(ctx, theme, state);
    // Publish this frame's water + nutrient sliders to the sim (after the modal edits).
    snapshot_garden_sim(state);
    // Per-asteroid mining modal (clicked a Mining card) — also at ctx level.
    mining_modal(ctx, theme, state);
}

// (garden_tree_nodes + crop_leaf removed in v0.402 — the garden is an aligned
// TABLE now, not a tree, so the tree-node builders are no longer needed.)

// detail_row moved to crate::gui::widgets::detail_row
