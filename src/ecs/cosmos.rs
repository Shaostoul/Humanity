//! Cosmos position model — Phase 2 of the cosmos architecture
//! (v0.202.0). Introduces the `PositionInUniverse` component, the
//! `ContainerRef` enum that addresses where an entity is in the
//! universe, and the `VesselRegistry` resource that holds the
//! container graph.
//!
//! See `docs/design/cosmos-architecture.md` for the full design.
//! Key properties:
//! - Position is hierarchical: every entity is in some container, and
//!   its `local_pos` is meters/light-years (depending on container)
//!   from that container's frame origin.
//! - Position composition walks the container chain to compute a
//!   world (galactic) position when needed for rendering.
//! - All `local_pos` values stay small in their parent's frame
//!   (sub-mm precision in systems, sub-meter in deep space) thanks to
//!   the hierarchy keeping individual values bounded.
//! - Procedural rogue body positions are computed on-demand from
//!   `(galaxy_seed, query_position)` rather than stored — see
//!   `docs/design/cosmos-architecture.md` §10.
//!
//! Phase 2 scope: data types + container graph + resolver. Existing
//! `Transform` component continues to drive rendering for now; a
//! later phase will bridge `PositionInUniverse` → `Transform` via
//! the floating origin so the renderer reads from the new model.

use glam::{DVec3, DQuat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Identity types ───────────────────────────────────────────────────────

/// String id of a vessel. Stable across sessions. Examples:
/// `"mothership-pioneer"`, `"alice-home-001"`, `"ford-f150-abc"`.
/// VesselIds are assigned by content (RON layout files) or generated
/// when a player creates a custom vessel; either way they're persistent.
pub type VesselId = String;

/// String id of a pocket dimension. Examples: `"tutorial-cave"`,
/// `"boss-arena-42"`, `"vendor-instance-7"`. Pockets are isolated
/// coordinate spaces disconnected from the normal galaxy.
pub type PocketId = String;

// ── ContainerRef ─────────────────────────────────────────────────────────

/// Where in the universe an entity is. Each variant defines what its
/// `PositionInUniverse::local_pos` means in context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContainerRef {
    /// Inside or attached to a vessel. local_pos is meters from the
    /// vessel's local origin (defined by the vessel's RON layout file).
    /// "Vessel" generalizes to anything mobile-and-inhabitable:
    /// spaceships, cars, trucks, tanks, fighter jets, walking mechs,
    /// space stations, and even buildings (treated as stationary
    /// vessels). Layout determines size and rooms.
    Vessel(VesselId),

    /// On the surface of a celestial body. local_pos is meters in
    /// east/north/up from the body's surface origin (effectively a
    /// lat/lon/altitude → ECEF-style mapping). For a planet, lat/lon
    /// can be computed from local_pos if needed.
    Body { system_id: String, body_id: String },

    /// Free-floating in a star system. local_pos is meters from the
    /// system's barycenter (which for Sol is essentially the Sun's
    /// center, slightly offset by Jupiter's mass).
    Space { system_id: String },

    /// Free-floating in interstellar space. galaxy_pos_ly is a
    /// continuous 3D position in light-years from the chosen galactic
    /// origin (currently Sol at J2000.0 — see cosmos-architecture.md).
    /// f64 at 100 kly distance gives ~1 mm precision, ample for any
    /// ship-scale navigation. No chunks at the data-model level.
    Deep { galaxy_pos_ly: DVec3 },

    /// Pocket dimension — isolated coordinate space disconnected from
    /// the normal galaxy (tutorial spaces, instanced quest areas,
    /// tech demos). Travel into/out is a portal event, not a
    /// continuous transit.
    Pocket(PocketId),
}

impl ContainerRef {
    /// Human-readable label for debug + logging. Cheap, doesn't hit
    /// any storage.
    pub fn debug_label(&self) -> String {
        match self {
            ContainerRef::Vessel(id) => format!("Vessel({id})"),
            ContainerRef::Body { system_id, body_id } => format!("Body({system_id}/{body_id})"),
            ContainerRef::Space { system_id } => format!("Space({system_id})"),
            ContainerRef::Deep { galaxy_pos_ly } => format!(
                "Deep({:.3}, {:.3}, {:.3} ly)",
                galaxy_pos_ly.x, galaxy_pos_ly.y, galaxy_pos_ly.z
            ),
            ContainerRef::Pocket(id) => format!("Pocket({id})"),
        }
    }
}

// ── PositionInUniverse component ─────────────────────────────────────────

/// The canonical "where is this entity" component. Replaces the
/// implicit world-position model for entities that participate in
/// universe-scale positioning (players, ships, NPCs). Existing
/// `Transform` component continues to be used for in-game render
/// positions; a later phase will bridge from `PositionInUniverse`
/// via floating origin.
///
/// Units depend on `container`:
/// - Vessel: local_pos in meters from vessel origin
/// - Body: local_pos in meters east/north/up from surface origin
/// - Space: local_pos in meters from system barycenter
/// - Deep: local_pos is unused; the Deep variant carries its own
///   galactic position in `galaxy_pos_ly`. Keep local_pos at zero or
///   use it for sub-position offsets if useful (e.g. inside a tight
///   nebula structure).
/// - Pocket: local_pos is whatever the pocket's coordinate space
///   defines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInUniverse {
    pub container: ContainerRef,
    /// Position within the container's frame. Meters for most variants
    /// (see container-specific notes above). Use f64 (DVec3) so the
    /// hierarchy preserves precision — at AU scale (1.5e13 m), f64
    /// still gives sub-mm precision.
    pub local_pos: DVec3,
    /// Facing direction within the container's frame.
    pub local_rot: DQuat,
}

impl Default for PositionInUniverse {
    fn default() -> Self {
        // Default: standing on Earth at (0,0,0) surface coordinate.
        // Real placement happens via player setup flow.
        Self {
            container: ContainerRef::Body {
                system_id: "sol".to_string(),
                body_id: "earth".to_string(),
            },
            local_pos: DVec3::ZERO,
            local_rot: DQuat::IDENTITY,
        }
    }
}

// ── Vessel registry ──────────────────────────────────────────────────────

/// Persistent metadata for a single vessel. The vessel's POSITION
/// (a `PositionInUniverse`) lives separately so vessels can move via
/// the same machinery players use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VesselMeta {
    pub id: VesselId,
    /// Path to the layout RON file (relative to data dir).
    /// Example: `"data/vessels/mothership-pioneer.ron"`.
    pub layout_file: String,
    /// Public key of the owning identity. Empty = unowned / shared
    /// infrastructure (e.g. the mothership might be community-owned).
    pub owner_key: String,
    /// Display name. May differ from id (id is stable, name is editable).
    pub name: String,
}

/// In-memory registry of all vessels the relay/client knows about.
/// Vessels' POSITIONS live alongside in `vessel_positions: HashMap<VesselId, PositionInUniverse>`
/// so movement is uniform with player movement.
///
/// Thread-safety: this is a per-client cache. Network sync happens
/// via WS messages (`vessel_position_update`, `vessel_state`) — see
/// future Phase 5 (ship movement + sync).
#[derive(Debug, Clone, Default)]
pub struct VesselRegistry {
    pub meta: HashMap<VesselId, VesselMeta>,
    pub positions: HashMap<VesselId, PositionInUniverse>,
}

impl VesselRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, meta: VesselMeta, position: PositionInUniverse) {
        self.positions.insert(meta.id.clone(), position);
        self.meta.insert(meta.id.clone(), meta);
    }

    pub fn get(&self, id: &VesselId) -> Option<&VesselMeta> {
        self.meta.get(id)
    }

    pub fn get_position(&self, id: &VesselId) -> Option<&PositionInUniverse> {
        self.positions.get(id)
    }

    pub fn set_position(&mut self, id: &VesselId, position: PositionInUniverse) {
        self.positions.insert(id.clone(), position);
    }

    /// Iterate over all known vessels (id + meta + position).
    pub fn iter(&self) -> impl Iterator<Item = (&VesselId, &VesselMeta, Option<&PositionInUniverse>)> {
        self.meta.iter().map(move |(id, meta)| (id, meta, self.positions.get(id)))
    }
}

// ── World-position resolver ──────────────────────────────────────────────

/// Compute the absolute galactic position (in light-years) of an
/// entity given its `PositionInUniverse`. Walks the container chain
/// up to the galactic frame.
///
/// Returns the position in light-years, in the galactic frame
/// (currently: Sol at J2000.0 = origin).
///
/// Phase 2 implementation is approximate:
/// - Vessel: recurses into the vessel's own position
/// - Body: looks up the body's orbital position via stub fn (full
///   orbital mechanics lands in a later phase)
/// - Space: uses the system's `galaxy_position_ly` from index.json
/// - Deep: the variant carries its galactic position directly
/// - Pocket: returns NaN — pocket dimensions aren't in the galaxy
///
/// This will be tightened in subsequent phases as orbital + system
/// data is wired up.
pub fn world_position_ly(
    pos: &PositionInUniverse,
    vessels: &VesselRegistry,
    systems: &SystemPositions,
    sim_time_ms: u64,
) -> DVec3 {
    match &pos.container {
        ContainerRef::Deep { galaxy_pos_ly } => *galaxy_pos_ly,
        ContainerRef::Space { system_id } => {
            let sys_pos = systems.get(system_id).copied().unwrap_or(DVec3::ZERO);
            // local_pos is meters from system barycenter; convert to ly.
            sys_pos + meters_to_ly(pos.local_pos)
        }
        ContainerRef::Body { system_id, body_id } => {
            let sys_pos = systems.get(system_id).copied().unwrap_or(DVec3::ZERO);
            let body_pos_meters_in_system = body_position_in_system_meters(system_id, body_id, sim_time_ms);
            // local_pos is meters on the body's surface; combined with
            // body's position in system + system's galactic position.
            // Approximation: ignore surface-radius offset for now (sub-pixel
            // at galactic scale).
            sys_pos + meters_to_ly(body_pos_meters_in_system + pos.local_pos)
        }
        ContainerRef::Vessel(id) => {
            // Recursive: a vessel has its own PositionInUniverse.
            // Inherit the vessel's world position + the entity's local offset.
            if let Some(vp) = vessels.get_position(id) {
                world_position_ly(vp, vessels, systems, sim_time_ms) + meters_to_ly(pos.local_pos)
            } else {
                // Unknown vessel — treat as if the entity is at the
                // galactic origin. Defensive; a missing vessel ID is a
                // bug worth surfacing in logs but shouldn't crash.
                log::warn!("world_position_ly: unknown vessel id '{}'", id);
                meters_to_ly(pos.local_pos)
            }
        }
        ContainerRef::Pocket(_) => {
            // Pocket dimensions aren't in the galaxy. Returning NaN signals
            // "this entity has no galactic position" so callers (renderer,
            // map UI) can handle it explicitly. NaN-handling is opt-in:
            // calling code that doesn't expect Pocket containers should
            // assert before this call, not silently propagate NaN.
            DVec3::new(f64::NAN, f64::NAN, f64::NAN)
        }
    }
}

/// Meters per light-year (IAU definition: 9460730472580800 exactly).
/// Single canonical constant; the inverse is derived to ensure
/// roundtrip conversions cancel cleanly (modulo f64 multiplication
/// epsilon, ~2e-16 relative).
pub const METERS_PER_LY: f64 = 9_460_730_472_580_800.0;

/// Convert a meters DVec3 to a light-years DVec3.
#[inline]
pub fn meters_to_ly(meters: DVec3) -> DVec3 {
    meters / METERS_PER_LY
}

/// Convert a light-years DVec3 to a meters DVec3.
#[inline]
pub fn ly_to_meters(ly: DVec3) -> DVec3 {
    ly * METERS_PER_LY
}

// ── System + body position lookups (stubs for Phase 2) ───────────────────

/// In-memory cache of `system_id → galactic position (ly)`.
/// Populated at startup from `data/star_systems/index.json`.
#[derive(Debug, Clone, Default)]
pub struct SystemPositions {
    by_id: HashMap<String, DVec3>,
}

impl SystemPositions {
    pub fn new() -> Self { Self::default() }

    pub fn insert(&mut self, system_id: String, position_ly: DVec3) {
        self.by_id.insert(system_id, position_ly);
    }

    pub fn get(&self, system_id: &str) -> Option<&DVec3> {
        self.by_id.get(system_id)
    }

    pub fn len(&self) -> usize { self.by_id.len() }
    pub fn is_empty(&self) -> bool { self.by_id.is_empty() }
}

/// Stub: compute a body's position in its parent system's frame
/// (meters from system barycenter) at a given sim time. Full
/// implementation lands in a later phase — for now returns ZERO,
/// which means "body is at system center" (good enough for
/// system-scale rendering until orbital mechanics ships).
///
/// The eventual implementation reads orbital elements from the
/// system's data file and applies Kepler's equations:
/// - semi-major axis, eccentricity, inclination, longitude of
///   ascending node, argument of periapsis, mean anomaly at epoch
/// - solve for position at sim_time relative to the body's parent
/// - recurse if the body orbits another body (e.g. moons)
pub fn body_position_in_system_meters(
    _system_id: &str,
    _body_id: &str,
    _sim_time_ms: u64,
) -> DVec3 {
    // Phase 2: stub. Returns zero so initial rendering shows bodies at
    // system center — visually wrong but doesn't crash. Real orbital
    // math arrives in a later phase along with the data-load wiring.
    DVec3::ZERO
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meters_ly_roundtrip() {
        // Test at multiple scales — sub-AU, 1 ly, 100 kly (galactic radius).
        for &m_x in &[1.0, 1.5e11, METERS_PER_LY, 100_000.0 * METERS_PER_LY] {
            let m = DVec3::new(m_x, 0.0, 0.0);
            let back = ly_to_meters(meters_to_ly(m));
            // f64 multiplication is precise to ~2e-16 relative. Allow 1e-12
            // relative drift as the assertion bound.
            let drift = (back.x - m.x).abs();
            let tolerable = m_x.abs() * 1e-12;
            assert!(drift <= tolerable, "drift {} > tolerable {} at m={}", drift, tolerable, m_x);
        }
    }

    #[test]
    fn one_ly_converts_to_exactly_one_ly() {
        let m = DVec3::new(METERS_PER_LY, 0.0, 0.0);
        let ly = meters_to_ly(m);
        // m / m → 1 exactly under IEEE 754 since the denominator is the
        // canonical constant.
        assert_eq!(ly.x, 1.0);
    }

    #[test]
    fn deep_container_world_pos_is_galactic_pos() {
        let pos = PositionInUniverse {
            container: ContainerRef::Deep { galaxy_pos_ly: DVec3::new(1.5, -2.0, 0.7) },
            local_pos: DVec3::ZERO,
            local_rot: DQuat::IDENTITY,
        };
        let v = VesselRegistry::new();
        let s = SystemPositions::new();
        let wp = world_position_ly(&pos, &v, &s, 0);
        assert_eq!(wp, DVec3::new(1.5, -2.0, 0.7));
    }

    #[test]
    fn space_container_uses_system_position() {
        let mut systems = SystemPositions::new();
        systems.insert("sol".to_string(), DVec3::ZERO);
        systems.insert("alpha_centauri".to_string(), DVec3::new(-1.348, -3.972, -1.535));

        let pos = PositionInUniverse {
            container: ContainerRef::Space { system_id: "alpha_centauri".to_string() },
            local_pos: DVec3::ZERO,
            local_rot: DQuat::IDENTITY,
        };
        let v = VesselRegistry::new();
        let wp = world_position_ly(&pos, &v, &systems, 0);
        assert_eq!(wp, DVec3::new(-1.348, -3.972, -1.535));
    }

    #[test]
    fn vessel_container_inherits_vessel_world_pos() {
        // Vessel "ship-1" is in Sol space at the system center.
        // Player is inside ship-1 at local_pos (10, 0, 0) meters.
        // Player's world pos should be (0, 0, 0) ly + 10m offset = essentially (0,0,0) ly
        // (10m is sub-precision at galactic scale).
        let mut systems = SystemPositions::new();
        systems.insert("sol".to_string(), DVec3::ZERO);

        let mut vessels = VesselRegistry::new();
        vessels.register(
            VesselMeta {
                id: "ship-1".into(),
                layout_file: "data/vessels/test.ron".into(),
                owner_key: String::new(),
                name: "Test Ship".into(),
            },
            PositionInUniverse {
                container: ContainerRef::Space { system_id: "sol".into() },
                local_pos: DVec3::ZERO,
                local_rot: DQuat::IDENTITY,
            },
        );

        let player = PositionInUniverse {
            container: ContainerRef::Vessel("ship-1".into()),
            local_pos: DVec3::new(10.0, 0.0, 0.0),
            local_rot: DQuat::IDENTITY,
        };
        let wp = world_position_ly(&player, &vessels, &systems, 0);
        // 10 m in ly is ~1.057e-15 ly. Effectively zero at galactic scale.
        assert!(wp.x.abs() < 1e-13, "expected ~0 ly, got {}", wp.x);
    }

    #[test]
    fn unknown_vessel_logs_and_returns_local_offset() {
        let pos = PositionInUniverse {
            container: ContainerRef::Vessel("ghost-vessel".into()),
            local_pos: DVec3::new(5.0, 0.0, 0.0),
            local_rot: DQuat::IDENTITY,
        };
        let v = VesselRegistry::new();
        let s = SystemPositions::new();
        // Should not panic, just warn + return local offset converted to ly.
        let wp = world_position_ly(&pos, &v, &s, 0);
        assert!(wp.x > 0.0 && wp.x < 1e-14);
    }

    #[test]
    fn pocket_returns_nan() {
        let pos = PositionInUniverse {
            container: ContainerRef::Pocket("tutorial".into()),
            local_pos: DVec3::ZERO,
            local_rot: DQuat::IDENTITY,
        };
        let wp = world_position_ly(&pos, &VesselRegistry::new(), &SystemPositions::new(), 0);
        assert!(wp.x.is_nan());
    }
}
