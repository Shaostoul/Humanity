//! A Steam game's mods from the Steam Workshop (2026-10-19), as Steam keeps them on this computer:
//! read only, never written to.
//!
//! Each library's `steamapps\workshop\appworkshop_<appid>.acf` (KeyValues text) lists the items
//! the person subscribed to and Steam downloaded, under `AppWorkshop > WorkshopItemsInstalled >
//! <item id>`: each one's `size` (bytes) and `timeupdated` (Unix time). Their files are in
//! `steamapps\workshop\content\<appid>\<item id>\`. An item's title is not kept on this computer
//! (only on Steam's servers), and nothing here asks for it: items are named by their number, and
//! each one's own page opens in Steam on a click (links.rs).

use std::path::{Path, PathBuf};

use super::keyvalues::{self, Value};
use super::{list_dir, read_small};

/// One Workshop item.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Its number on the Steam Workshop.
    pub id: String,
    /// Bytes, as Steam recorded them.
    pub size: Option<u64>,
    /// When it was last updated (Unix seconds).
    pub updated: Option<u64>,
    /// Its folder, and whether that was there when read.
    pub folder: PathBuf,
    pub folder_here: bool,
}

/// A game's Workshop items: the most recently updated first.
#[derive(Debug, Clone, PartialEq)]
pub struct Workshop {
    pub items: Vec<Item>,
    /// Every item's size added up (those Steam recorded).
    pub total_size: u64,
    /// `workshop\content\<appid>` (Show in folder), and whether it was there.
    pub folder: PathBuf,
    pub folder_here: bool,
}

/// An item number, which goes into a link: digits only.
fn is_item_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit())
}

/// Game `appid`'s Workshop items in the library whose `steamapps` folder is `steamapps`: those its
/// record lists, and any folder in its content folder the record does not. `None` when it has none.
pub fn read(steamapps: &Path, appid: &str) -> Option<Workshop> {
    let workshop = steamapps.join("workshop");
    let folder = workshop.join("content").join(appid);
    let mut items: Vec<Item> = Vec::new();
    if let Some(kv) = read_small(&workshop.join(format!("appworkshop_{appid}.acf"))).and_then(|t| keyvalues::parse(&t).ok()) {
        let installed = keyvalues::find(&kv, "AppWorkshop").and_then(Value::block).and_then(|w| keyvalues::find(w, "WorkshopItemsInstalled")).and_then(Value::block).unwrap_or(&[]);
        for (id, entry) in installed {
            let Some(entry) = entry.block() else { continue };
            if !is_item_id(id) || items.iter().any(|i| &i.id == id) {
                continue;
            }
            let number = |key: &str| keyvalues::find_text(entry, key).and_then(|t| t.trim().parse::<u64>().ok());
            let item_folder = folder.join(id);
            items.push(Item { id: id.clone(), size: number("size"), updated: number("timeupdated").filter(|t| *t > 0), folder_here: item_folder.is_dir(), folder: item_folder });
        }
    }
    for entry in list_dir(&folder) {
        let id = entry.file_name().to_string_lossy().into_owned();
        if is_item_id(&id) && entry.file_type().is_ok_and(|t| t.is_dir()) && !items.iter().any(|i| i.id == id) {
            items.push(Item { id, size: None, updated: None, folder: entry.path(), folder_here: true });
        }
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    let total_size = items.iter().filter_map(|i| i.size).sum();
    Some(Workshop { items, total_size, folder_here: folder.is_dir(), folder })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures;

    /// THE WORKSHOP: Orbital Farmstead's record lists two items (sizes and times) and its content
    /// folder holds a third the record does not name; the newest first, the sizes added up, an
    /// item id that is not a number left out; a game with no Workshop has none.
    ///
    /// Seen red 2026-10-19 with the content folder not listed (only the record read): "the item
    /// only on the disk is listed too" (left: 2, right: 3).
    #[test]
    fn the_workshop() {
        let fx = fixtures::steam("workshop_read");
        let w = read(&fx.root.join("steamapps"), "1000010").expect("it has items");
        assert_eq!(w.items.len(), 3, "the item only on the disk is listed too");
        let ids: Vec<&str> = w.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["3000002", "3000001", "3000003"], "the most recently updated first, the unrecorded last");
        assert_eq!(w.total_size, 1_500_000_000 + 2_500_000);
        assert!(w.folder_here && w.items[0].folder_here);
        assert_eq!(w.items[2].size, None);
        assert!(read(&fx.root.join("steamapps"), "1000030").is_none(), "no Workshop, none");
    }
}
