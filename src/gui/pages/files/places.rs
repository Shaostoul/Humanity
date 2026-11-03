//! The Files page's places (2026-10-19), at the top of its left pane: where the files shown come
//! from. One Files page holds what were three (the Media page, the developer's Files page with the
//! server's Shared Files, and the Your folders tab); the operator, 2026-10-19: "What's the real
//! difference between these two? Why would we not want to merge them?"
//!
//! IN THE LAUNCHER'S RED, GREEN AND BLUE (2026-11-03). The operator, on the navigation mock-up of
//! this page: "I feel like that first image should be ordered as your private local files files,
//! your publicly shared files, then files by friends/servers? Maintains the RGB styling. R for
//! private, B for servers." So the places come in three groups, top to bottom, each tinted with the
//! colour the character and server picker gives the same trust model (showroom.rs `section_card`:
//! your own homes RED, the open net GREEN, the closed net BLUE; docs/design/characters-and-servers.md,
//! "Launcher color language"), the same theme tokens (`danger`, `success`, `info`), so the
//! Settings theme moves both together:
//!
//! - RED, PRIVATE, ON THIS COMPUTER: "HumanityOS folder" (the app's own folder, data/, assets/ and
//!   mods/ in it, to read and edit; for a copy installed to the person's folder its data folder,
//!   never the person's own files: app_folder.rs `app_folder_for`) and "Your folders" (folders added
//!   to look through and pick files from for Add to Files: your_folders.rs). Neither needs a server.
//!   The person's own private files kept encrypted on a server (the personal backup of
//!   docs/design/two-realities.md) belong here too, when they are built: private is the trust
//!   model, not where the bytes sit, and a server that cannot read them is only holding them.
//!   Nothing of that is built, so nothing of it is listed.
//! - GREEN, WHAT YOU SHARE: "Yours on <server>" (the server's Files page, only what you put on it:
//!   the same list and panes as Shared on <server>, asked for with the person filter set to you,
//!   files.rs `list_filters`) and "Your shared files" (your own files in the server's Shared files:
//!   shared_files.rs, filtered to your key). Each is the green twin of a blue place, so neither
//!   needed a new server route.
//! - BLUE, WHAT EVERYONE SHARES: "Shared on <server>" (what people chose to show: the list, the
//!   featured shelf and the filters, then an item to fit and its details), "Shared files" (the
//!   server's public file library: shared_files.rs) and, for an admin, "Admin" (who sees the page,
//!   its space, its categories and what to prune: admin.rs).
//!
//! The page still opens on Shared on <server> (`Place::default`): what the server's people share
//! is what someone comes to a Files page for, the HumanityOS folder holds the files that make the
//! app work (a poor first sight for someone new), and someone who has shared nothing would open
//! on an empty place.

use super::Place;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use crate::gui::GuiState;
use humanity_core::i18n::{t, tf};

/// The places' three groups, top to bottom, in the launcher's red, green and blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    /// RED: on this computer, yours alone.
    Private,
    /// GREEN: what you put on the server for others to see.
    YouShare,
    /// BLUE: what everyone on the server shares, and the admins' place.
    EveryoneShares,
}

impl Group {
    /// The groups in the order they are listed (the tests hold `listed` to it).
    #[cfg(test)]
    pub const ALL: [Group; 3] = [Group::Private, Group::YouShare, Group::EveryoneShares];

    /// The group a place is in.
    pub fn of(place: Place) -> Group {
        match place {
            Place::AppFolder | Place::YourFolders => Group::Private,
            Place::Yours | Place::YourSharedFiles => Group::YouShare,
            Place::Shared | Place::SharedFiles | Place::Admin => Group::EveryoneShares,
        }
    }

    /// The group's heading over its places.
    pub fn heading(self) -> &'static str {
        match self {
            Group::Private => t("files.places.private"),
            Group::YouShare => t("files.places.you_share"),
            Group::EveryoneShares => t("files.places.everyone_shares"),
        }
    }

    /// The group's colour: the character and server picker's for the same trust model
    /// (showroom.rs `section_card`: Your Homes `danger`, Open Net `success`, Closed Net `info`).
    /// `the_groups_draw_in_the_launchers_colours` draws both and holds them equal.
    pub fn color(self, theme: &Theme) -> egui::Color32 {
        match self {
            Group::Private => theme.danger(),
            Group::YouShare => theme.success(),
            Group::EveryoneShares => theme.info(),
        }
    }
}

/// A place's name in the list: "Shared on united-humanity.us" names the server.
pub(crate) fn label(state: &GuiState, place: Place) -> String {
    let host = super::host_here(state);
    match place {
        Place::Shared if host.is_empty() => t("files.places.shared_on_a_server").to_string(),
        Place::Shared => tf("files.places.shared_on", &[("host", &host)]),
        Place::Yours if host.is_empty() => t("files.places.yours_on_a_server").to_string(),
        Place::Yours => tf("files.places.yours_on", &[("host", &host)]),
        Place::YourSharedFiles => t("files.places.your_shared_files").to_string(),
        Place::SharedFiles => t("files.places.shared_files").to_string(),
        Place::Admin => t("files.places.admin").to_string(),
        Place::AppFolder => t("files.places.app_folder").to_string(),
        Place::YourFolders => t("files.places.your_folders").to_string(),
    }
}

/// A place's plain-language hint, by its key: what it holds, shown when the pointer rests on it.
pub(crate) fn hint_key(place: Place) -> &'static str {
    match place {
        Place::AppFolder => "files.places.app_folder_hint",
        Place::YourFolders => "files.places.your_folders_hint",
        Place::Yours => "files.places.yours_hint",
        Place::YourSharedFiles => "files.places.your_shared_files_hint",
        Place::Shared => "files.places.shared_hint",
        Place::SharedFiles => "files.places.shared_files_hint",
        Place::Admin => "files.places.admin_hint",
    }
}

/// Each place's id in the list.
fn id_of(place: Place) -> &'static str {
    match place {
        Place::AppFolder => "app_folder",
        Place::YourFolders => "your_folders",
        Place::Yours => "yours",
        Place::YourSharedFiles => "your_shared_files",
        Place::Shared => "shared",
        Place::SharedFiles => "shared_files",
        Place::Admin => "admin",
    }
}

/// The places listed, in order, red then green then blue: Admin for an admin of the server only.
pub(crate) fn listed(admin: bool) -> Vec<Place> {
    let mut v = vec![Place::AppFolder, Place::YourFolders, Place::Yours, Place::YourSharedFiles, Place::Shared, Place::SharedFiles];
    if admin {
        v.push(Place::Admin);
    }
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

/// The places as the list draws them: each in its group's colour with its hint, the group's
/// heading over the first of its places.
pub(crate) fn items(state: &GuiState, theme: &Theme, admin: bool) -> Vec<SectionNavItem> {
    let mut last: Option<Group> = None;
    listed(admin)
        .into_iter()
        .map(|p| {
            let group = Group::of(p);
            let item = SectionNavItem::new(id_of(p), label(state, p), group.color(theme)).hint(hint_key(p));
            if last == Some(group) {
                item
            } else {
                last = Some(group);
                item.group(group.heading())
            }
        })
        .collect()
}

/// The places, at the top of the left pane; a press shows that place. `across`: the panes are
/// stacked (a tall window), so the left pane is short and the places go a line per group
/// (`widgets::section_nav_wrapped`) instead of a row each, leaving the pane room for its list.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, across: bool) {
    let admin = admin_here(state);
    let current = shown_place(state.files.place, admin);
    let items = items(state, theme, admin);
    let picked = if across { widgets::section_nav_wrapped(ui, theme, &items, id_of(current)) } else { widgets::section_nav(ui, theme, None, &items, id_of(current)) };
    if let Some(id) = picked {
        if let Some(p) = listed(admin).into_iter().find(|p| id_of(*p) == id) {
            state.files.place = p;
        }
    }
    ui.add_space(theme.spacing_xs);
    ui.separator();
    ui.add_space(theme.spacing_xs);
}

/// Whether `place` takes files dropped on the window itself (to Shared Files), rather than the
/// page opening Add to Files for them.
pub(crate) fn takes_drops(place: Place) -> bool {
    matches!(place, Place::SharedFiles | Place::YourSharedFiles)
}

/// Whether `place` is one of the two that show the server's Files page list (everyone's, or
/// yours alone).
pub(crate) fn shows_the_list(place: Place) -> bool {
    matches!(place, Place::Shared | Place::Yours)
}

#[cfg(test)]
#[path = "places_tests.rs"]
mod tests;
