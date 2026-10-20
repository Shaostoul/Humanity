//! HumanityOS's own tools, the Apps page's green part (2026-10-19; the operator: "Green could be
//! humanityOS tools"): the app's own pages a person uses (Maps, Tasks, Crafting, the Library...),
//! each a tile that opens its page here.
//!
//! The list is DATA, data/gui/humanityos_tools.json (Infinite-of-X: a new tool is a new entry
//! there, never code): each entry's name, its page (the id gui/dispatch.rs gives it) and one plain
//! line. Dev-only pages (Testing, Files, Dev) are left out of it. Loaded once at startup (lib.rs);
//! an entry whose page id is not a page shows no Open, and the test below keeps the shipped file
//! free of such entries.

use serde::Deserialize;

use crate::gui::GuiPage;

/// The data file, under the data folder.
pub const FILE: [&str; 2] = ["gui", "humanityos_tools.json"];

/// Open: go to its page.
pub const OPEN: &str = "Open";

/// One of HumanityOS's own tools.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OwnTool {
    pub name: String,
    /// Its page's id (gui/dispatch.rs `page_id`: "maps", "tasks"...).
    pub page: String,
    /// One plain line about it.
    pub line: String,
}

impl OwnTool {
    /// The page it opens, when its id is one.
    pub fn gui_page(&self) -> Option<GuiPage> {
        crate::gui::dispatch::page_from_id(&self.page)
    }
}

#[derive(Deserialize)]
struct ToolsFile {
    tools: Vec<OwnTool>,
}

/// The tools in a file's text, in its order.
pub fn parse(text: &str) -> Result<Vec<OwnTool>, String> {
    serde_json::from_str::<ToolsFile>(text).map(|f| f.tools).map_err(|e| e.to_string())
}

/// The tools from the data folder `data_dir`; none (and a line in the log) when the file is not
/// there or does not read, so a missing file never stops the app.
pub fn load(data_dir: &std::path::Path) -> Vec<OwnTool> {
    let path = FILE.iter().fold(data_dir.to_path_buf(), |p, part| p.join(part));
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
        Ok(tools) => tools,
        Err(e) => {
            eprintln!("[apps] HumanityOS's own tools were not read from {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// The shipped list (tests and snapshots).
#[cfg(test)]
pub(crate) fn shipped() -> Vec<OwnTool> {
    load(std::path::Path::new("data"))
}

/// The order they are shown in: those whose name or line holds every word of `search` (letters'
/// case ignored), by name.
pub fn order_of(tools: &[OwnTool], search: &str) -> Vec<usize> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<usize> = tools
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            let text = format!("{}\n{}", t.name, t.line).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    out.sort_by(|&a, &b| tools[a].name.to_lowercase().cmp(&tools[b].name.to_lowercase()).then(a.cmp(&b)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE SHIPPED LIST OPENS REAL PAGES, NONE OF THEM DEV-ONLY: every entry of
    /// data/gui/humanityos_tools.json names a page the app draws (not the in-game view, not the Apps
    /// page itself, not Testing, Files or the Platform tab's dev tools), has a name and a line, and
    /// is listed once; the ones the operator named are there.
    ///
    /// Seen red 2026-10-19 with "maps" written "map" in the file: "Maps opens a page" failed (no
    /// page has the id "map").
    #[test]
    fn the_shipped_list_opens_real_pages() {
        let tools = shipped();
        assert!(tools.len() >= 10, "the list is there: {}", tools.len());
        let mut seen = std::collections::HashSet::new();
        for t in &tools {
            let page = t.gui_page();
            assert!(page.is_some(), "{} opens a page (its id {:?})", t.name, t.page);
            let page = page.unwrap();
            assert!(!crate::gui::dispatch::ENGINE_BOUND_PAGES.contains(&page), "{} is a page, not the game", t.name);
            assert!(![GuiPage::Apps, GuiPage::Testing, GuiPage::Platform, GuiPage::Settings].contains(&page), "{} is not this page or a dev or settings page", t.name);
            assert!(!t.name.trim().is_empty() && !t.line.trim().is_empty(), "{} has a name and a line", t.page);
            assert!(seen.insert(t.page.clone()), "{} is listed once", t.page);
        }
        for name in ["Maps", "Tasks", "Crafting", "Library", "Studio", "Watch", "Files", "Inventory"] {
            assert!(tools.iter().any(|t| t.name == name), "{name} is listed");
        }
    }

    /// THE SEARCH AND THE ORDER: by name, and only those whose name or line holds every word typed.
    ///
    /// Seen red 2026-10-19 with the line left out of the search (`t.name` alone): "by its line"
    /// got [].
    #[test]
    fn the_search_and_the_order() {
        let tools = shipped();
        let names = |s: &str| order_of(&tools, s).into_iter().map(|i| tools[i].name.clone()).collect::<Vec<_>>();
        let all = names("");
        assert!(all.windows(2).all(|w| w[0].to_lowercase() <= w[1].to_lowercase()), "by name: {all:?}");
        assert_eq!(names("THIS COMPUTER"), vec!["Studio"], "by its line");
        assert_eq!(names("maps"), vec!["Maps"]);
    }
}
