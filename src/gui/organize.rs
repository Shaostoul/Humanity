//! The organize layer's small pure helpers (2026-09-25), kept out of
//! loaders.rs (which is at its file-size budget): putting items back into
//! storage when a backpack cannot take them, and the Garden panel's groups
//! for crops planted into a specific machine.

use super::{GardenArea, PlacedItem};

/// Garden panel groups for crops planted into a specific MACHINE (2026-09-25).
/// The showcase garden tags each crop with the physical tower or bed it sits
/// in ("ntower_3", "grain_field_1"), which the renderer needs, but the panel
/// only made groups for tower designs and bed TYPES, so the whole showcase
/// garden was invisible there. Returns one (grow-area id, title) per such
/// machine not already covered by `known`, in natural order (2 before 10),
/// titled "<type label> <n>" from the machine's type.
pub fn machine_crop_groups(
    crop_areas: &[&str],
    known: &[String],
    type_of: &std::collections::HashMap<String, String>,
    areas: &[GardenArea],
) -> Vec<(String, String)> {
    let mut ids: Vec<&str> = crop_areas.iter().copied().filter(|t| !known.iter().any(|k| k == t)).collect();
    ids.sort_by_key(|id| split_numbered(id));
    ids.dedup();
    ids.into_iter()
        .map(|id| {
            let (_, n) = split_numbered(id);
            let label = type_of
                .get(id)
                .and_then(|ty| areas.iter().find(|a| &a.machine_id == ty))
                .map(|a| a.label.clone());
            let title = match (label, n) {
                (Some(l), Some(n)) => format!("{l} {}", n + 1),
                (Some(l), None) => l,
                (None, _) => id.to_string(),
            };
            (id.to_string(), title)
        })
        .collect()
}

/// "ntower_12" -> ("ntower", Some(12)); "grain_field" -> ("grain_field", None).
fn split_numbered(id: &str) -> (String, Option<u32>) {
    match id.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            (head.to_string(), tail.parse().ok())
        }
        _ => (id.to_string(), None),
    }
}

/// Put `qty` of `key` back into the storage pool after a backpack could not
/// take it (2026-09-25), merging into the stack it came from when that stack
/// is still there. `origin` is the item as it was before it was taken; with
/// none (it should not happen) it goes to "Home". Returns the notice to show.
pub fn return_to_storage(
    pool: &mut Vec<PlacedItem>,
    key: &str,
    qty: u32,
    origin: Option<&PlacedItem>,
) -> String {
    let (name, container, wear, quality) = match origin {
        Some(o) => (o.name.clone(), o.container.clone(), o.wear, o.quality),
        None => (key.to_string(), "Home".to_string(), 0, 0),
    };
    // Merge only with an entry worn and graded the same.
    if let Some(p) = pool
        .iter_mut()
        .find(|p| p.key == key && p.container == container && p.wear == wear && p.quality == quality)
    {
        p.qty += qty;
    } else {
        pool.push(PlacedItem { key: key.to_string(), name: name.clone(), qty, container: container.clone(), wear, quality });
    }
    format!("Backpack full: {qty} x {name} stayed in {container}")
}

#[cfg(all(test, feature = "native"))]
mod return_to_storage_tests {
    use super::*;

    fn item(key: &str, qty: u32, container: &str) -> PlacedItem {
        PlacedItem { key: key.into(), name: "Rope".into(), qty, container: container.into(), wear: 0, quality: 0 }
    }

    /// Showcase crops carry machine instance ids; the panel gets one group per
    /// machine it does not already cover, titled by type, in natural order.
    #[test]
    fn machine_groups_cover_crops_planted_into_specific_machines() {
        let area = |id: &str, label: &str| GardenArea {
            label: label.into(),
            machine_id: id.into(),
            count: 1,
            food: String::new(),
            size: (1.0, 1.0, 1.0),
        };
        let areas = vec![area("aeroponic_tower_nutrition", "Nutrition Tower"), area("grain_field", "Grain Field")];
        let type_of: std::collections::HashMap<String, String> = [
            ("ntower_10", "aeroponic_tower_nutrition"),
            ("ntower_2", "aeroponic_tower_nutrition"),
            ("grain_field_1", "grain_field"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let crops = ["ntower_10", "ntower_2", "ntower_2", "grain_field_1", "nutrition", "mystery_7"];
        let known = vec!["nutrition".to_string()];
        let groups = machine_crop_groups(&crops, &known, &type_of, &areas);
        let got: Vec<(&str, &str)> = groups.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        assert_eq!(
            got,
            vec![
                ("grain_field_1", "Grain Field 2"),
                ("mystery_7", "mystery_7"),
                ("ntower_2", "Nutrition Tower 3"),
                ("ntower_10", "Nutrition Tower 11"),
            ]
        );
    }

    /// Overflow goes back into the stack it came from when that stack is
    /// still there, otherwise back to its container as a new stack, and the
    /// notice names what stayed where.
    #[test]
    fn overflow_goes_back_to_where_it_came_from() {
        let origin = item("rope_0", 10, "Home/Garage");
        let mut pool = vec![item("rope_0", 2, "Home/Garage"), item("rope_0", 1, "Home/Shed")];
        let msg = return_to_storage(&mut pool, "rope_0", 4, Some(&origin));
        assert_eq!(pool[0].qty, 6, "merged into the Garage stack");
        assert_eq!(pool[1].qty, 1, "the Shed stack is untouched");
        assert_eq!(msg, "Backpack full: 4 x Rope stayed in Home/Garage");

        let mut empty = Vec::new();
        return_to_storage(&mut empty, "rope_0", 3, Some(&origin));
        assert_eq!(empty.len(), 1);
        assert_eq!((empty[0].qty, empty[0].container.as_str()), (3, "Home/Garage"));
    }
}
