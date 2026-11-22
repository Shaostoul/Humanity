//! Which rooms the Map page's ship view names (maps increment 3; split out of ship_view.rs on
//! 2026-11-21, which is at its size budget). The web plan mirrors these rules
//! (web/shared/ship-plan.js `roomIsNamed`, `zoneFloor`).

use humanity_ship::room_types::RoomBounds;

/// Whether a room is NAMED on the map: the data gives it a name, and it is a room rather than a
/// corridor tube. A room with no name would show its id, which on a map is noise; a corridor
/// (`corridor_0`, which the HUD names "Corridor between ..." since 2026-11-21) is drawn as the
/// tube it is, filled and outlined, not named. A zone's own floor (`commons:room_1`) reads as its
/// zone since 2026-11-21 ("The Commons") and is named, once per level (`zone_floor`).
pub fn room_is_named(r: &RoomBounds) -> bool {
    r.display_name != r.id && !r.id.starts_with("corridor_")
}

/// Whether a room is a shared zone's own floor, the part of it none of its own rooms names
/// (`<zone>:room_N`, or `<zone>:home` for a zone with no inner walls): it reads as the zone, and
/// a zone whose walls cut its floor into pockets is still one place, so the map names the biggest
/// pocket and leaves the others (the Commons has ten).
pub fn zone_floor(r: &RoomBounds) -> bool {
    r.id.split_once(':').is_some_and(|(_, rest)| rest.starts_with("room_") || rest == "home")
}
