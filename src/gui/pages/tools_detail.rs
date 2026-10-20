// Child of pages/tools.rs (#[path]): an entry's details, at the right of the shelf while one is
// chosen, in the details panel the Games page uses (widgets/shelf.rs). Drawn from the catalog and
// from what the page's worker found, so nothing here reads a file or the registry.
//
// Top to bottom: its banner (a panel in its category's colour with its name, and an installed
// program's own icon), its category and Close; whether it is installed and from where; Launch,
// Show in folder, Website, Download page and Install (each only where it applies); its
// description; its facts (licence, platforms, download size, the file types its category opens,
// its version, folder and program when installed, its winget package); and where it comes from.

use egui::{RichText, Sense, Vec2};

use super::{Act, Installing, ToolsState};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cover_tile, icons, shelf};
use crate::gui::GuiState;
use crate::software::install::{self, Package};
use crate::software::{Installed, ToolEntry, Via};

/// The details' width when they open (the person can drag their edge).
pub(super) const DETAILS_WIDTH: f32 = 440.0;
/// The banner's shape: the Games page's.
const HERO_ASPECT: f32 = 3.1;
/// The section headings.
pub(super) const ABOUT: &str = "About";
/// Said under Install about what it will be.
pub(super) const INSTALL_TIP: &str = "Install it with winget, Windows' own package manager. Its licence is shown first, for you to agree to.";
/// Said at the end of a program's details.
pub(super) const COMES_FROM: &str = "It comes from its own project: Download page and Website open the project's own pages in your browser.";

/// The details, when an entry is chosen. Returns what was clicked.
pub(super) fn panel(ctx: &egui::Context, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let page = &state.tools;
    let Some(i) = page.selected else { return acts };
    let Some(entry) = state.tools_catalog.get(i) else { return acts };
    shelf::details_panel(ctx, theme, "tools_details", DETAILS_WIDTH, |ui| {
        body(ui, theme, page, i, entry, &mut acts);
    });
    acts
}

fn body(ui: &mut egui::Ui, theme: &Theme, page: &ToolsState, i: usize, e: &ToolEntry, acts: &mut Vec<Act>) {
    let width = ui.available_width();
    let installed = page.installed(i);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, (width / HERO_ASPECT).round().max(80.0)), Sense::hover());
    cover_tile::paint_cover(ui.painter(), theme, rect, None, page.icon_of(i), &e.name, &e.category_id, theme.font_size_title, theme.border_radius_lg);
    ui.add_space(theme.spacing_sm);

    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_small), Sense::hover());
        ui.painter().circle_filled(dot.center(), dot.width() * 0.35, super::library::category_color(theme, e));
        ui.label(RichText::new(&e.category).size(theme.font_size_small).color(theme.text_secondary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::ghost(super::CLOSE).size(widgets::ButtonSize::Small).tooltip("Close its details.").show(ui, theme) {
                acts.push(Act::CloseDetails);
            }
        });
    });
    if let Some(found) = installed {
        ui.horizontal(|ui| {
            let (mark, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_body), Sense::hover());
            icons::paint_check(ui.painter(), mark, theme.success());
            ui.label(RichText::new(installed_words(found)).size(theme.font_size_small).color(theme.success()));
        });
        if found.launch.is_none() {
            ui.label(RichText::new(super::NO_PROGRAM).size(theme.font_size_small).color(theme.text_muted()));
        }
    } else if e.is_software() && !e.web_only() {
        // Never "not installed": Windows has no sure way to find every program.
        ui.label(RichText::new(super::NOT_FOUND).size(theme.font_size_small).color(theme.text_muted())).on_hover_text("Windows has no sure way to find every program, so one installed somewhere unusual may not be found.");
    }
    ui.add_space(theme.spacing_xs);
    actions(ui, theme, page, i, e, installed, acts);
    install_step(ui, theme, page, i, e, installed, acts);
    ui.add_space(theme.spacing_sm);
    ui.add(egui::Label::new(RichText::new(&e.description).size(theme.font_size_body).color(theme.text_primary())).wrap());
    about(ui, theme, i, e, installed);
    if e.is_software() && !e.web_only() {
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(COMES_FROM).size(theme.font_size_small).color(theme.text_muted()));
    }
    ui.add_space(theme.spacing_lg);
}

/// "Installed, version 3.2.4.0", or through Steam.
fn installed_words(found: &Installed) -> String {
    match (&found.version, found.via) {
        (Some(v), _) => format!("Installed, version {v}"),
        (None, Via::Steam) => "Installed through Steam".to_string(),
        (None, _) => "Installed".to_string(),
    }
}

/// Launch and Show in folder when installed; Website; Download page (a program to download whose
/// project has one); Install (a program not installed with a winget package, on Windows).
fn actions(ui: &mut egui::Ui, theme: &Theme, page: &ToolsState, i: usize, e: &ToolEntry, installed: Option<&Installed>, acts: &mut Vec<Act>) {
    // Install: on Windows, for a program not found that winget has and that is not website only,
    // while no install is under way (the consent step, below, takes its place while it is).
    let can_install = cfg!(windows) && installed.is_none() && e.installable() && page.install.is_none();
    ui.horizontal_wrapped(|ui| {
        if let Some(found) = installed {
            if found.launch.is_some() {
                let tip = if found.via == Via::Steam { "Asks Steam to start it." } else { "Starts its program." };
                if widgets::Button::primary(super::LAUNCH).size(widgets::ButtonSize::Large).tooltip(tip).show(ui, theme) {
                    acts.push(Act::Launch(i));
                }
            }
            if let Some(show) = found.show() {
                if widgets::Button::ghost(super::SHOW_IN_FOLDER).tooltip("Show it in the computer's file manager.").show(ui, theme) {
                    acts.push(Act::Open(crate::games::Launch::ShowInFolder(show)));
                }
            }
        }
        if can_install && widgets::Button::primary(super::INSTALL).tooltip(INSTALL_TIP).show(ui, theme) {
            acts.push(Act::Install(i));
        }
        let website_label = if e.is_software() { super::WEBSITE } else { super::OPEN_WEBSITE };
        if widgets::Button::secondary(website_label).tooltip("Opens its website in your browser.").show(ui, theme) {
            acts.push(Act::Web(e.url.clone()));
        }
        let has_download = e.is_software() && !e.web_only() && e.download_page() != e.url;
        if has_download && widgets::Button::secondary(super::DOWNLOAD_PAGE).tooltip("Opens the project's own download page in your browser.").show(ui, theme) {
            acts.push(Act::Web(e.download_page().to_string()));
        }
    });
    // Why there is no Install, said where the person looks for it (CLAUDE.md, "Saying what we will
    // not build, and why": the catalog names whose rule it is).
    if installed.is_none() {
        if let Some(why) = &e.website_only {
            ui.add(egui::Label::new(RichText::new(format!("No Install here. {why}")).size(theme.font_size_small).color(theme.text_secondary())).wrap());
        } else if cfg!(windows) && e.is_software() && !e.web_only() && e.winget.is_none() {
            ui.add(egui::Label::new(RichText::new(NOT_IN_WINGET).size(theme.font_size_small).color(theme.text_muted())).wrap());
        }
    }
    if let Some(busy) = &page.install {
        if busy.entry() != i && installed.is_none() && e.installable() {
            ui.label(RichText::new("One install at a time: another is under way.").size(theme.font_size_small).color(theme.text_muted()));
        }
    }
}

/// Said for a program winget does not have (no reason of law or rule: nobody packaged it there).
pub(super) const NOT_IN_WINGET: &str = "Not in winget, Windows' own package manager, so there is no Install here: Download opens its maker's own page.";
/// Said in the consent step.
pub(super) const PERMISSION_LINE: &str = "Windows may ask your permission for the installer to make changes to this computer. If you say no, nothing is installed.";
pub(super) const USAGE_DATA_LINE: &str = "winget is Microsoft's tool, and it sends Microsoft data about its use unless that is turned off on this computer.";
pub(super) const OWN_INSTALLER_LINE: &str = "Its maker's own installer opens too, with its own licence page, as its maker intends: follow it to finish.";
pub(super) const NO_AGREEMENTS: &str = "The package lists no agreements of its own in winget.";

/// The install under way for entry `i`, in its details: winget being asked about the package, the
/// consent step, winget installing it, or how the last install ended.
fn install_step(ui: &mut egui::Ui, theme: &Theme, page: &ToolsState, i: usize, e: &ToolEntry, installed: Option<&Installed>, acts: &mut Vec<Act>) {
    match &page.install {
        Some(Installing::Reading { entry, .. }) if *entry == i => {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(theme.font_size_body));
                ui.label(RichText::new("Asking winget about its package...").size(theme.font_size_small).color(theme.text_secondary()));
            });
        }
        Some(Installing::Asking { entry, package }) if *entry == i => consent(ui, theme, i, e, package, acts),
        Some(Installing::Running { entry, line, .. }) if *entry == i => {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(theme.font_size_body));
                ui.label(RichText::new(format!("Installing {}...", e.name)).size(theme.font_size_body).color(theme.text_primary()));
            });
            if !line.is_empty() {
                ui.add(egui::Label::new(RichText::new(line).size(theme.font_size_small).color(theme.text_muted())).truncate());
            }
        }
        _ => {
            if let Some((entry, words, problem)) = &page.install_note {
                if *entry == i && (installed.is_none() || !*problem) {
                    let color = if *problem { theme.warning() } else { theme.text_secondary() };
                    ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(color)).wrap());
                }
            }
        }
    }
}

/// The consent step: what is installed and from whom, its licence and its agreements, Microsoft's
/// two lines as winget prints them, Windows' permission prompt, winget's usage data, and what
/// pressing "I agree, install" means. Nothing has run but `winget show`.
fn consent(ui: &mut egui::Ui, theme: &Theme, i: usize, e: &ToolEntry, package: &Package, acts: &mut Vec<Act>) {
    egui::Frame::none().fill(theme.bg_secondary()).stroke(egui::Stroke::new(1.0, theme.accent())).rounding(egui::Rounding::same(theme.border_radius_lg as u8)).inner_margin(theme.spacing_md).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(format!("Install {}", e.name)).size(theme.font_size_heading).color(theme.text_primary()));
        let mut rows: Vec<(&str, String)> = Vec::new();
        let what = match &package.version {
            Some(v) => format!("{} {v}, from winget's community source", package.id),
            None => format!("{}, from winget's community source", package.id),
        };
        rows.push(("Package", what));
        if let Some(p) = &package.publisher {
            rows.push(("Publisher", p.clone()));
        }
        let licence = package.license.clone().unwrap_or_else(|| e.license.clone());
        rows.push(("Licence", if licence.is_empty() { "Not stated".to_string() } else { licence }));
        shelf::facts(ui, theme, ("tools_consent", i), &rows);
        let licence_url = e.license_url.clone().or_else(|| package.license_url.clone());
        if let Some(url) = licence_url {
            if widgets::Button::ghost("Read the licence").size(widgets::ButtonSize::Small).tooltip("Opens the licence's own page in your browser.").show(ui, theme) {
                acts.push(Act::Web(url));
            }
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Its agreements").size(theme.font_size_small).color(theme.text_muted()));
        if package.agreements.is_empty() {
            ui.label(RichText::new(NO_AGREEMENTS).size(theme.font_size_small).color(theme.text_secondary()));
        } else {
            for line in &package.agreements {
                ui.add(egui::Label::new(RichText::new(line).size(theme.font_size_small).color(theme.text_primary())).wrap());
            }
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("winget says:").size(theme.font_size_small).color(theme.text_muted()));
        ui.add(egui::Label::new(RichText::new(format!("\"{}\"", install::DISCLAIMER_OWNER)).size(theme.font_size_small).color(theme.text_primary())).wrap());
        ui.add(egui::Label::new(RichText::new(format!("\"{}\"", install::DISCLAIMER_MICROSOFT)).size(theme.font_size_small).color(theme.text_primary())).wrap());
        ui.add_space(theme.spacing_xs);
        for line in [PERMISSION_LINE, USAGE_DATA_LINE] {
            ui.add(egui::Label::new(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary())).wrap());
        }
        if e.install_interactive {
            ui.add(egui::Label::new(RichText::new(OWN_INSTALLER_LINE).size(theme.font_size_small).color(theme.text_secondary())).wrap());
        }
        ui.add_space(theme.spacing_xs);
        ui.add(egui::Label::new(RichText::new(agree_words(&e.name)).size(theme.font_size_small).strong().color(theme.text_primary())).wrap());
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary(super::AGREE_INSTALL).tooltip("Agree to its licence and agreements, and install it with winget.").show(ui, theme) {
                acts.push(Act::Agree(i));
            }
            if widgets::Button::secondary(super::CANCEL).tooltip("Nothing is installed.").show(ui, theme) {
                acts.push(Act::CancelInstall);
            }
        });
    });
}

/// What pressing "I agree, install" means, said before it is pressed (the Ninth Circuit's test,
/// the finding's 2.4: a click is agreement only when the person is told it is).
pub(super) fn agree_words(name: &str) -> String {
    format!("Pressing \"{}\" means you agree to {name}'s licence and to the agreements above, and HumanityOS tells winget that you agreed.", super::AGREE_INSTALL)
}

/// Its facts, each only where the catalog or what was found has it.
fn about(ui: &mut egui::Ui, theme: &Theme, i: usize, e: &ToolEntry, installed: Option<&Installed>) {
    let mut rows: Vec<(&str, String)> = Vec::new();
    if !e.license.is_empty() {
        rows.push(("Licence", e.license.clone()));
    }
    if !e.platforms.is_empty() {
        rows.push(("Runs on", super::platforms_words(e)));
    }
    if !e.size.is_empty() && !e.web_only() {
        rows.push(("Download size", e.size.clone()));
    }
    if e.is_software() && !e.extensions.is_empty() {
        rows.push(("Opens", e.extensions.join(", ")));
    }
    if let Some(found) = installed {
        if let Some(v) = &found.version {
            rows.push(("Version", v.clone()));
        }
        if let Some(folder) = &found.folder {
            rows.push(("Installed in", folder.display().to_string()));
        }
        if let Some(program) = &found.program {
            rows.push(("Program", program.display().to_string()));
        }
        rows.push(("Found in", found.via.words().to_string()));
    }
    if let Some(id) = &e.winget {
        rows.push(("winget package", id.clone()));
    }
    rows.push(("Website", e.host()));
    shelf::section_heading(ui, theme, ABOUT, None);
    shelf::facts(ui, theme, ("tools_about", i), &rows);
}
