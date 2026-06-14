//! Cosmetic outfit catalog (v0.442): craftable/tradeable clothing items that tint the
//! avatar's body slots. Pure data; loaded from `data/cosmetics/cosmetics.csv`. Slots are
//! the ids in data/inventory/equipment_slots.json (head/chest/legs/feet/hands/back).
//!
//! A cosmetic is structurally an ITEM (it has an id, flows through inventory/market), so
//! there is no store code path; the wardrobe is an inventory FILTER on slot-tagged items.
//! For now a cosmetic just carries a tint color (the blockman avatar is untextured); when a
//! skinned mesh lands, a `model` column gets added and the slot renders the gltf instead.

use std::path::Path;

/// One cosmetic clothing item.
#[derive(Debug, Clone)]
pub struct Cosmetic {
    pub id: String,
    pub name: String,
    /// Equipment slot id (head/chest/legs/feet/hands/back).
    pub slot: String,
    /// Linear RGB tint applied to the avatar's slot part.
    pub color: [f32; 3],
    pub description: String,
}

/// Load the cosmetic catalog from `data/cosmetics/cosmetics.csv`. Returns empty on a
/// missing/invalid file (the wardrobe just shows nothing).
pub fn load_cosmetics(data_dir: &Path) -> Vec<Cosmetic> {
    let path = data_dir.join("cosmetics").join("cosmetics.csv");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // header / blank
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 7 {
            continue;
        }
        let parse = |s: &str| s.trim().parse::<f32>().unwrap_or(0.5);
        out.push(Cosmetic {
            id: f[0].trim().to_string(),
            name: f[1].trim().to_string(),
            slot: f[2].trim().to_string(),
            color: [parse(f[3]), parse(f[4]), parse(f[5])],
            description: f[6].trim().to_string(),
        });
    }
    out
}

/// Resolved per-slot tint colors for the avatar, from an equipped outfit. `None` = use the
/// default body color for that slot.
#[derive(Debug, Clone, Default)]
pub struct OutfitColors {
    pub head: Option<[f32; 3]>,
    pub chest: Option<[f32; 3]>,
    pub legs: Option<[f32; 3]>,
    pub feet: Option<[f32; 3]>,
    pub hands: Option<[f32; 3]>,
    pub back: Option<[f32; 3]>,
}

/// Resolve the equipped outfit's cosmetic ids into per-slot colors via the catalog.
pub fn resolve_outfit_colors(
    outfit: &crate::ecs::components::Outfit,
    cosmetics: &[Cosmetic],
) -> OutfitColors {
    let color_of = |slot: &str| -> Option<[f32; 3]> {
        let id = outfit.equipped.get(slot)?;
        cosmetics.iter().find(|c| &c.id == id).map(|c| c.color)
    };
    OutfitColors {
        head: color_of("head"),
        chest: color_of("chest"),
        legs: color_of("legs"),
        feet: color_of("feet"),
        hands: color_of("hands"),
        back: color_of("back"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_resolves() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let cos = load_cosmetics(&dir);
        assert!(cos.len() >= 5, "cosmetics.csv parses, got {}", cos.len());
        assert!(cos.iter().any(|c| c.slot == "chest"), "has a chest cosmetic");
        let mut o = crate::ecs::components::Outfit::default();
        o.equipped.insert("chest".to_string(), "red_tee".to_string());
        let rc = resolve_outfit_colors(&o, &cos);
        assert!(rc.chest.is_some(), "red_tee resolves a chest color");
        assert!(rc.legs.is_none(), "no legs cosmetic equipped");
    }
}
