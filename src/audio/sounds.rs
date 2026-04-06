//! Sound effect path constants and surface types.

// UI sounds
pub const UI_CLICK: &str = "data/audio/ui/click.ogg";
pub const UI_HOVER: &str = "data/audio/ui/hover.ogg";
pub const UI_OPEN: &str = "data/audio/ui/open.ogg";
pub const UI_CLOSE: &str = "data/audio/ui/close.ogg";

// Footsteps
pub const FOOTSTEP_GRASS: &str = "data/audio/sfx/footstep_grass.ogg";
pub const FOOTSTEP_STONE: &str = "data/audio/sfx/footstep_stone.ogg";
pub const FOOTSTEP_METAL: &str = "data/audio/sfx/footstep_metal.ogg";
pub const FOOTSTEP_DIRT: &str = "data/audio/sfx/footstep_dirt.ogg";
pub const FOOTSTEP_WOOD: &str = "data/audio/sfx/footstep_wood.ogg";

// Actions
pub const MINING_HIT: &str = "data/audio/sfx/mining_hit.ogg";
pub const HARVEST: &str = "data/audio/sfx/harvest.ogg";
pub const CRAFT_COMPLETE: &str = "data/audio/sfx/craft_complete.ogg";
pub const BUILD_PLACE: &str = "data/audio/sfx/build_place.ogg";
pub const ITEM_PICKUP: &str = "data/audio/sfx/item_pickup.ogg";

// Ambient
pub const AMBIENT_FOREST: &str = "data/audio/ambient/forest.ogg";
pub const AMBIENT_SPACE: &str = "data/audio/ambient/space.ogg";
pub const AMBIENT_SHIP: &str = "data/audio/ambient/ship_interior.ogg";
pub const AMBIENT_RAIN: &str = "data/audio/ambient/rain.ogg";
pub const AMBIENT_WIND: &str = "data/audio/ambient/wind.ogg";
pub const AMBIENT_NIGHT: &str = "data/audio/ambient/night_crickets.ogg";

// Music
pub const MUSIC_MENU: &str = "data/audio/music/menu.ogg";
pub const MUSIC_EXPLORATION: &str = "data/audio/music/exploration.ogg";
pub const MUSIC_COMBAT: &str = "data/audio/music/combat.ogg";
pub const MUSIC_BUILDING: &str = "data/audio/music/building.ogg";
pub const MUSIC_PEACEFUL: &str = "data/audio/music/peaceful.ogg";

/// Surface type for selecting footstep sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceType {
    Grass,
    Stone,
    Metal,
    Dirt,
    Wood,
}

impl SurfaceType {
    pub fn footstep_sound(&self) -> &'static str {
        match self {
            SurfaceType::Grass => FOOTSTEP_GRASS,
            SurfaceType::Stone => FOOTSTEP_STONE,
            SurfaceType::Metal => FOOTSTEP_METAL,
            SurfaceType::Dirt => FOOTSTEP_DIRT,
            SurfaceType::Wood => FOOTSTEP_WOOD,
        }
    }
}
