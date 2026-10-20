//! The Files page's places (2026-10-19), at the top of its left pane: where the files shown come
//! from. One Files page holds what were three (the Media page, the developer's Files page with the
//! server's Shared Files, and the Your folders tab); the operator, 2026-10-19: "What's the real
//! difference between these two? Why would we not want to merge them?"
//!
//! ON THE SERVER: "Shared on <server>" (what people chose to show: the list, the featured shelf and
//! the filters, then an item to fit and its details), "Shared files" (the server's public file
//! library: shared_files.rs) and, for an admin, "Admin" (who sees the page, its space, its
//! categories and what to prune: admin.rs).
//!
//! ON THIS COMPUTER: "HumanityOS folder" (the app's own folder, data/, assets/ and mods/ in it, to
//! read and edit: app_folder.rs) and "Your folders" (folders added to look through and pick files
//! from for Add to Files: your_folders.rs). Neither needs a server.

use super::{app_folder, shared_files, your_folders, Place};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use crate::gui::GuiState;

/// The admins' place.
pub const ADMIN: &str = "Admin";
/// The two groups' headings.
pub const ON_SERVER: &str = "On the server";
pub const ON_COMPUTER: &str = "On this computer";

/// A place's name in the list: "Shared on united-humanity.us" names the server.
pub(crate) fn label(state: &GuiState, place: Place) -> String {
    match place {
        Place::Shared => {
            let host = super::host_here(state);
            if host.is_empty() {
                "Shared on a server".to_string()
            } else {
                format!("Shared on {host}")
            }
        }
        Place::SharedFiles => shared_files::PLACE.to_string(),
        Place::Admin => ADMIN.to_string(),
        Place::AppFolder => app_folder::PLACE.to_string(),
        Place::YourFolders => your_folders::TAB.to_string(),
    }
}

/// Each place's id in the list.
fn id_of(place: Place) -> &'static str {
    match place {
        Place::Shared => "shared",
        Place::SharedFiles => "shared_files",
        Place::Admin => "admin",
        Place::AppFolder => "app_folder",
        Place::YourFolders => "your_folders",
    }
}

/// The places listed, in order: Admin for an admin of the server only.
pub(crate) fn listed(admin: bool) -> Vec<Place> {
    let mut v = vec![Place::Shared, Place::SharedFiles];
    if admin {
        v.push(Place::Admin);
    }
    v.extend([Place::AppFolder, Place::YourFolders]);
    v
}

/// The place shown for `place`: Admin is an admin's alone, and anyone else on it sees what is
/// shared on the server (an admin who lost the role, or a place left set by Server Settings' "Open
/// them").
pub(crate) fn shown_place(place: Place, admin: bool) -> Place {
    if place == Place::Admin && !admin {
        Place::Shared
    } else {
        place
    }
}

/// Whether this person is an admin of the server this page is for.
pub(crate) fn admin_here(state: &GuiState) -> bool {
    super::is_admin(state) && !super::server_here(state).is_empty()
}

/// The places, at the top of the left pane; a press shows that place.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let admin = admin_here(state);
    let current = shown_place(state.media.place, admin);
    let (server, computer) = (theme.nav_legacy_red(), theme.nav_tools());
    let items: Vec<SectionNavItem> = listed(admin)
        .into_iter()
        .map(|p| {
            let on_server = matches!(p, Place::Shared | Place::SharedFiles | Place::Admin);
            let item = SectionNavItem::new(id_of(p), label(state, p), if on_server { server } else { computer });
            match p {
                Place::Shared => item.group(ON_SERVER),
                Place::AppFolder => item.group(ON_COMPUTER),
                _ => item,
            }
        })
        .collect();
    if let Some(id) = widgets::section_nav(ui, theme, None, &items, id_of(current)) {
        if let Some(p) = listed(admin).into_iter().find(|p| id_of(*p) == id) {
            state.media.place = p;
        }
    }
    ui.add_space(theme.spacing_xs);
    ui.separator();
    ui.add_space(theme.spacing_xs);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE PLACES (2026-10-19): two on the server and two on this computer for everyone, Admin for
    /// an admin only, and Admin left set for someone who is not one shows what is shared.
    #[test]
    fn the_places_are_listed_and_admin_is_an_admins_alone() {
        assert_eq!(listed(false), vec![Place::Shared, Place::SharedFiles, Place::AppFolder, Place::YourFolders]);
        assert_eq!(listed(true), vec![Place::Shared, Place::SharedFiles, Place::Admin, Place::AppFolder, Place::YourFolders]);
        assert_eq!(shown_place(Place::Admin, false), Place::Shared);
        assert_eq!(shown_place(Place::Admin, true), Place::Admin);
        assert_eq!(shown_place(Place::AppFolder, false), Place::AppFolder);
        let ids: std::collections::HashSet<&str> = listed(true).into_iter().map(id_of).collect();
        assert_eq!(ids.len(), 5, "each place its own id");
    }
}
