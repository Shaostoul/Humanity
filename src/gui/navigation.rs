//! The top menu as data (docs/design/navigation.md, section 13; rung 1, 2026-11-03).
//!
//! `data/gui/navigation.json` holds the design's eight categories and every page placed in them,
//! what a person can reach that the design's rows leave out (`outside`, each with its reason), and
//! what the single top row shows until rung 2 draws the two rows (`today_row`). Both apps draw
//! from it: the desktop's row through [`menu`] (pages/escape_menu.rs), the website's through
//! scripts/gen-navigation.js, which writes the website's part into web/shared/shell.js. The file's
//! own `_comment` says what each field means.
//!
//! READ once, when the menu is first drawn, and again when the file is saved ([`reload`], from
//! engine/hot_reload_gates.rs): never each frame, and nothing else is kept. A file this version
//! cannot read is refused WHOLE, naming the entry and what is wrong with it ([`parse`]): a word
//! key that is not in data/i18n/en.json, a page that is not a page, an id or a page in two places,
//! a field it does not know. At start the copy built into the exe is then used (with the log line
//! every built-in copy writes); on a save while the app runs, the menu keeps what it had. The
//! tests (navigation_tests.rs) hold both copies to the rules, so the built-in one always reads.
//!
//! COST: [`menu`] is a read lock and a reference count each frame; the row's ids were turned into
//! pages when the file was read.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use egui::Color32;
use serde::Deserialize;

use crate::gui::theme::Theme;
use crate::gui::GuiPage;

/// The file, under the data folder.
pub const FILE: &str = "gui/navigation.json";

/// The world's character picker, as an entry's `section` on the page `none` (the world).
pub const PICKER: &str = "picker";

/// One of the design's categories: a page of its own, with its other pages (`items`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    /// Its words, a key in data/i18n/en.json.
    pub label: String,
    /// Its hint, a key in data/i18n/en.json.
    pub hint: String,
    /// A name in web/shared/icons.js (`humanity`: the brand H).
    pub icon: String,
    /// A theme token by name ([`colour_of`]).
    pub colour: String,
    /// The desktop page (a `dispatch::page_id`), or none yet.
    #[serde(default)]
    pub page: Option<String>,
    /// The part of that page it opens.
    #[serde(default)]
    pub section: Option<String>,
    /// The website address, or none.
    #[serde(default)]
    pub web: Option<String>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default, rename = "_note")]
    pub note: Option<String>,
}

/// A page in a category's row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub hint: String,
    pub icon: String,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub web: Option<String>,
    /// No website page, by nature (the website will show it dimmed, opening /download).
    #[serde(default)]
    pub desktop_only: bool,
    /// No desktop page, by nature.
    #[serde(default)]
    pub web_only: bool,
    /// A page the design adds and has not built yet.
    #[serde(default)]
    pub new: bool,
    #[serde(default, rename = "_note")]
    pub note: Option<String>,
}

/// Something a person can reach that the design's rows leave out, and why.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outside {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub web: Option<String>,
    pub why: String,
}

/// A group of today's desktop row: its colour (a theme token) and its entries' ids.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopGroup {
    pub colour: String,
    pub items: Vec<String>,
}

/// A group of today's website row: its CSS class and its entries' ids.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebGroup {
    pub class: String,
    pub items: Vec<String>,
}

/// What the single top row shows until rung 2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TodayRow {
    pub desktop: Vec<DesktopGroup>,
    pub web: Vec<WebGroup>,
    /// An entry's id to the older key the website lights its tab by, where the two differ.
    #[serde(default)]
    pub web_active: BTreeMap<String, String>,
}

/// `data/gui/navigation.json`, as written.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    #[serde(default, rename = "_comment")]
    pub comment: serde_json::Value,
    pub categories: Vec<Category>,
    #[serde(default)]
    pub outside: Vec<Outside>,
    pub today_row: TodayRow,
}

/// Where an entry sits in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Category,
    Item,
    Outside,
}

/// Any entry of the file, by reference: what the rules (and the tests) ask of every one.
#[derive(Debug, Clone, Copy)]
pub struct Place<'a> {
    pub kind: Kind,
    pub id: &'a str,
    pub label: Option<&'a str>,
    pub hint: Option<&'a str>,
    pub page: Option<&'a str>,
    pub section: Option<&'a str>,
    pub web: Option<&'a str>,
    pub desktop_only: bool,
    pub web_only: bool,
    pub new: bool,
}

impl File {
    /// Every entry: each category, then its items, then what is outside the rows.
    pub fn places(&self) -> Vec<Place<'_>> {
        let mut out = Vec::new();
        for c in &self.categories {
            out.push(Place {
                kind: Kind::Category,
                id: &c.id,
                label: Some(&c.label),
                hint: Some(&c.hint),
                page: c.page.as_deref(),
                section: c.section.as_deref(),
                web: c.web.as_deref(),
                desktop_only: false,
                web_only: false,
                new: false,
            });
            for i in &c.items {
                out.push(Place {
                    kind: Kind::Item,
                    id: &i.id,
                    label: Some(&i.label),
                    hint: Some(&i.hint),
                    page: i.page.as_deref(),
                    section: i.section.as_deref(),
                    web: i.web.as_deref(),
                    desktop_only: i.desktop_only,
                    web_only: i.web_only,
                    new: i.new,
                });
            }
        }
        for o in &self.outside {
            out.push(Place {
                kind: Kind::Outside,
                id: &o.id,
                label: o.label.as_deref(),
                hint: o.hint.as_deref(),
                page: o.page.as_deref(),
                section: o.section.as_deref(),
                web: o.web.as_deref(),
                desktop_only: false,
                web_only: false,
                new: false,
            });
        }
        out
    }
}

/// One button of today's desktop row, its id already turned into a page.
#[derive(Debug, Clone, PartialEq)]
pub struct RowButton {
    pub id: String,
    /// Its words' key in data/i18n/en.json.
    pub label: String,
    pub page: GuiPage,
    /// The character picker rather than the world (Characters).
    pub picker: bool,
}

/// A group of today's desktop row: a dot is drawn between two groups.
#[derive(Debug, Clone, PartialEq)]
pub struct RowGroup {
    /// A theme token by name ([`colour_of`]).
    pub colour: String,
    pub buttons: Vec<RowButton>,
}

/// The menu, read: the file, and the desktop's row resolved from it.
#[derive(Debug, Clone, Default)]
pub struct Menu {
    pub file: File,
    pub desktop_row: Vec<RowGroup>,
}

/// The theme tokens a menu entry may name (data/gui/theme.ron). A name not here is refused when
/// the file is read, so a typo is an error rather than a grey button.
pub fn colour_of(name: &str) -> Option<fn(&Theme) -> Color32> {
    Some(match name {
        "nav_reality" => Theme::nav_reality,
        "nav_sim" => Theme::nav_sim,
        "nav_tools" => Theme::nav_tools,
        "nav_settings" => Theme::nav_settings,
        "nav_dev" => Theme::nav_dev,
        "nav_legacy_red" => Theme::nav_legacy_red,
        "nav_legacy_green" => Theme::nav_legacy_green,
        "nav_legacy_blue" => Theme::nav_legacy_blue,
        "section_cyan" => Theme::section_cyan,
        "section_magenta" => Theme::section_magenta,
        _ => return None,
    })
}

/// The file's text, read and checked; refused whole (naming the entry and what is wrong) when any
/// of it breaks a rule, so the menu is never half a file.
pub fn parse(text: &str) -> Result<Menu, String> {
    let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
    check(&file)?;
    let desktop_row = desktop_row(&file)?;
    Ok(Menu { file, desktop_row })
}

/// An id: lower-case letters, digits and `_`.
fn is_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The rules every entry, and the row, keeps.
fn check(file: &File) -> Result<(), String> {
    let places = file.places();
    let mut ids: HashSet<&str> = HashSet::new();
    // Each desktop place (a page, or a part of one) and each website address in one entry only.
    let mut desktop_places: HashSet<(&str, &str)> = HashSet::new();
    let mut web_places: HashSet<&str> = HashSet::new();
    for p in &places {
        let who = format!("the entry `{}`", p.id);
        if !is_id(p.id) {
            return Err(format!("{who}: an id is lower-case letters, digits and _"));
        }
        if !ids.insert(p.id) {
            return Err(format!("{who} is in the file twice"));
        }
        if p.label.is_some() != p.hint.is_some() {
            return Err(format!("{who}: an entry with words has a hint too, and one without has neither"));
        }
        for (what, key) in [("label", p.label), ("hint", p.hint)] {
            if let Some(key) = key {
                if !humanity_core::i18n::has(key) {
                    return Err(format!("{who}: its {what} `{key}` is not a key in data/i18n/en.json"));
                }
            }
        }
        match p.page {
            Some(page) => {
                if crate::gui::dispatch::page_from_id(page).is_none() {
                    return Err(format!("{who}: `{page}` is not a desktop page (src/gui/dispatch.rs page_id)"));
                }
                if !desktop_places.insert((page, p.section.unwrap_or(""))) {
                    let part = p.section.map(|s| format!(", part `{s}`,")).unwrap_or_default();
                    return Err(format!("{who}: the desktop page `{page}`{part} is another entry's already"));
                }
            }
            None if p.section.is_some() => return Err(format!("{who}: a section is a part of a page, and it has no page")),
            None => {}
        }
        if let Some(web) = p.web {
            if !web.starts_with('/') {
                return Err(format!("{who}: the website address `{web}` does not start with /"));
            }
            if !web_places.insert(web) {
                return Err(format!("{who}: the website address `{web}` is another entry's already"));
            }
        }
        if p.desktop_only && p.web_only {
            return Err(format!("{who} cannot be on the desktop only and on the website only"));
        }
        if p.desktop_only && p.web.is_some() {
            return Err(format!("{who} is on the desktop only, so it has no website address"));
        }
        if p.web_only && p.page.is_some() {
            return Err(format!("{who} is on the website only, so it has no desktop page"));
        }
        if p.page.is_none() && p.web.is_none() && !p.new {
            return Err(format!("{who} opens nothing on either app (a page not built yet says `new`)"));
        }
    }
    for o in &file.outside {
        if o.why.trim().is_empty() {
            return Err(format!("the entry `{}` is outside the rows and does not say why", o.id));
        }
    }
    for c in &file.categories {
        if colour_of(&c.colour).is_none() {
            return Err(format!("the category `{}`: `{}` is not a theme token the menu knows", c.id, c.colour));
        }
    }
    let by_id: BTreeMap<&str, &Place> = places.iter().map(|p| (p.id, p)).collect();
    let mut shown: HashSet<&str> = HashSet::new();
    for g in &file.today_row.desktop {
        if colour_of(&g.colour).is_none() {
            return Err(format!("today's desktop row: `{}` is not a theme token the menu knows", g.colour));
        }
        for id in &g.items {
            let Some(p) = by_id.get(id.as_str()) else {
                return Err(format!("today's desktop row names `{id}`, which is no entry"));
            };
            if p.page.is_none() || p.label.is_none() {
                return Err(format!("today's desktop row names `{id}`, which has no desktop page or no words"));
            }
            if p.section.is_some_and(|s| !(p.page == Some("none") && s == PICKER)) {
                return Err(format!("today's desktop row names `{id}`, a part of a page: the single row opens whole pages"));
            }
            if !shown.insert(id) {
                return Err(format!("today's desktop row shows `{id}` twice"));
            }
        }
    }
    let mut shown_web: HashSet<&str> = HashSet::new();
    for g in &file.today_row.web {
        if g.class.trim().is_empty() {
            return Err("today's website row has a group with no class".to_string());
        }
        for id in &g.items {
            let Some(p) = by_id.get(id.as_str()) else {
                return Err(format!("today's website row names `{id}`, which is no entry"));
            };
            if p.web.is_none() || p.label.is_none() {
                return Err(format!("today's website row names `{id}`, which has no website address or no words"));
            }
            if !shown_web.insert(id) {
                return Err(format!("today's website row shows `{id}` twice"));
            }
        }
    }
    for (id, key) in &file.today_row.web_active {
        if !shown_web.contains(id.as_str()) || key.trim().is_empty() {
            return Err(format!("today's website row: web_active names `{id}`, which is not in that row (or has no key)"));
        }
    }
    Ok(())
}

/// Today's desktop row with its ids turned into pages (the file is checked first).
fn desktop_row(file: &File) -> Result<Vec<RowGroup>, String> {
    let places = file.places();
    let mut out = Vec::new();
    for g in &file.today_row.desktop {
        let mut buttons = Vec::new();
        for id in &g.items {
            let p = places.iter().find(|p| p.id == id.as_str()).ok_or_else(|| format!("today's desktop row names `{id}`, which is no entry"))?;
            let page = p.page.and_then(crate::gui::dispatch::page_from_id).ok_or_else(|| format!("`{id}` has no desktop page"))?;
            buttons.push(RowButton {
                id: id.clone(),
                label: p.label.unwrap_or_default().to_string(),
                page,
                picker: page == GuiPage::None && p.section == Some(PICKER),
            });
        }
        out.push(RowGroup { colour: g.colour.clone(), buttons });
    }
    Ok(out)
}

/// The menu read, shared by every frame until [`reload`] replaces it.
static MENU: RwLock<Option<Arc<Menu>>> = RwLock::new(None);

/// The menu: read from the data folder the first time it is asked for, then the same until the
/// file is saved again ([`reload`]).
pub fn menu() -> Arc<Menu> {
    if let Some(m) = MENU.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return m.clone();
    }
    let mut slot = MENU.write().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = slot.as_ref() {
        return m.clone();
    }
    let m = Arc::new(load(&humanity_core::storage::data_dir()));
    *slot = Some(m.clone());
    m
}

/// The menu from the data folder `data_dir`: its copy when this version can read all of it,
/// otherwise the copy built into the exe (the log says why). Empty only when neither reads, which
/// the tests rule out for the exe's copy.
pub fn load(data_dir: &Path) -> Menu {
    humanity_core::embedded_data::load_text_or_embedded(data_dir, FILE, parse).unwrap_or_else(|e| {
        log::error!("{e}: the top menu has no buttons this run");
        Menu::default()
    })
}

/// Read the file again, after it was saved while the app runs (engine/hot_reload_gates.rs). A copy
/// this version cannot read is refused, loudly in the log, and the menu keeps what it had.
pub fn reload(data_dir: &Path) {
    let path = data_dir.join(FILE);
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| parse(&text)) {
        Ok(m) => {
            *MENU.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(m));
            log::info!("data/{FILE} was saved: the top menu was read again");
        }
        Err(e) => log::error!("data/{FILE} was saved but cannot be read ({e}): the top menu keeps what it had"),
    }
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
