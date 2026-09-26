//! Typed volumetric containers with content/material compatibility.
//!
//! # What this module is for
//!
//! In HumanityOS a container is *dedicated* to certain contents. A cryogenic
//! pressure tank can safely hold liquid nitrogen OR liquid CO2 (both cryogenic
//! liquids), but pouring the WRONG class of material into a container can BREAK
//! it — the canonical example being cement (a dry solid) poured into a
//! dairy/liquid tank.
//!
//! This module models that with three pieces:
//!   1. [`Container`] — a runtime, typed container instance (an ECS component).
//!   2. [`ContainerType`] / [`ContentClass`] — data loaded from
//!      `data/containers/types.csv` and `data/containers/content_classes.ron`.
//!   3. [`ContainerRegistry`] — the loaded rule table + the compatibility check.
//!
//! # Infinite-of-X
//!
//! Every container archetype and every content class is a DATA FILE, never a
//! hardcoded Rust list (see `docs/design/infinite-of-X.md`). To add a vessel,
//! append a row to `types.csv`. To add a content class, append an entry to
//! `content_classes.ron`. This module only holds the *schema* + *logic*.
//!
//! # Build note
//!
//! This module compiles in BOTH the `native` and `relay` builds (the
//! `systems` module is not feature-gated). The CSV/RON *parsing* helpers here
//! use the crate's pure `assets::loader` parsers (also not feature-gated), so
//! the registry can be built from in-memory bytes in either build and in tests.
//! The file-reading wiring (AssetManager -> DataStore) lives in `src/lib.rs`
//! behind `#[cfg(feature = "native")]`, mirroring how `item_registry` loads.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

// ===========================================================================
// Data-file schemas
// ===========================================================================

/// One container archetype, deserialized from a row of `data/containers/types.csv`.
///
/// `accepted_content_classes` arrives as a pipe-separated string (e.g.
/// `"liquid|water"`) and is split into a `Vec<String>` by [`ContainerType::from_row`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerTypeRow {
    pub id: String,
    pub name: String,
    pub base_material: String,
    pub capacity_liters: f32,
    pub pressure_rating_atm: f32,
    pub max_temp_c: f32,
    pub min_temp_c: f32,
    /// Pipe-separated whitelist of content-class ids this vessel accepts.
    pub accepted_content_classes: String,
    pub description: String,
    /// Contents touch the material itself (2026-09-26). See types.csv.
    #[serde(default = "default_direct_contact")]
    pub direct_contact: bool,
}

fn default_direct_contact() -> bool {
    true
}

/// A container archetype with the accepted-class whitelist already parsed.
///
/// This is what the registry stores and the runtime queries. It is built from
/// a [`ContainerTypeRow`] by splitting the pipe-separated whitelist.
#[derive(Debug, Clone)]
pub struct ContainerType {
    pub id: String,
    pub name: String,
    pub base_material: String,
    pub capacity_liters: f32,
    pub pressure_rating_atm: f32,
    pub max_temp_c: f32,
    pub min_temp_c: f32,
    /// The content classes this container can safely hold.
    pub accepted_content_classes: Vec<String>,
    pub description: String,
    /// Contents touch the material itself, so its food-grade and reactivity
    /// rules apply (false for cabinets and freezers of packaged goods).
    pub direct_contact: bool,
}

/// A contact material, one row of `data/containers/materials.csv`
/// (2026-09-26, from the dated container research).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub food_grade: bool,
    pub absorbent: bool,
    pub memory_fills: u32,
    /// Pipe-separated content traits it must not hold (acidic, salty,
    /// fatty, dairy, water).
    #[serde(default)]
    pub refuses: String,
    #[serde(default)]
    pub note: String,
}

impl Material {
    pub fn refuses_trait(&self, t: &str) -> bool {
        self.refuses.split('|').any(|r| r.trim() == t)
    }
}

/// `data/containers/content_traits.ron`.
#[derive(Debug, Clone, Default, Deserialize)]
struct ContentTraitsFile {
    #[serde(default)]
    profiles: HashMap<String, Vec<String>>,
    #[serde(default)]
    classes: HashMap<String, Vec<String>>,
}

/// The item -> food profile list in `data/food/item_profiles.ron`.
#[derive(Debug, Clone, Default, Deserialize)]
struct ItemProfilesFile {
    #[serde(default)]
    items: Vec<(String, String)>,
}

/// How a trait reads in a refusal message.
fn trait_words(t: &str) -> &str {
    match t {
        "acidic" => "acidic",
        "salty" => "salty or brined",
        "fatty" => "oily or fatty",
        "dairy" => "milk or dairy",
        "water" => "drinking water",
        other => other,
    }
}

impl ContainerType {
    /// Build a parsed [`ContainerType`] from a raw CSV row.
    fn from_row(row: ContainerTypeRow) -> Self {
        // Split "liquid|water" -> ["liquid", "water"], trimming + dropping empties.
        let accepted = row
            .accepted_content_classes
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            id: row.id,
            name: row.name,
            base_material: row.base_material,
            capacity_liters: row.capacity_liters,
            pressure_rating_atm: row.pressure_rating_atm,
            max_temp_c: row.max_temp_c,
            min_temp_c: row.min_temp_c,
            accepted_content_classes: accepted,
            description: row.description,
            direct_contact: row.direct_contact,
        }
    }

    /// Whether this container's whitelist includes the given content class.
    pub fn accepts_class(&self, content_class: &str) -> bool {
        self.accepted_content_classes
            .iter()
            .any(|c| c == content_class)
    }
}

/// One content class, deserialized from `data/containers/content_classes.ron`.
///
/// This is the per-class requirement record (the rule table). It documents what
/// a content class needs (pressure / insulation / sealed / food-safe + temp
/// bounds) and, via `accepted_by`, which container content-class tags are valid
/// homes for it. The runtime gate keys off the container whitelist
/// ([`ContainerType::accepts_class`]); these fields drive richer diagnostics
/// and the registry self-check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentClass {
    pub id: String,
    pub name: String,
    pub needs_pressure: bool,
    pub needs_insulation: bool,
    pub needs_sealed: bool,
    pub food_safe: bool,
    pub min_temp_c: f32,
    pub max_temp_c: f32,
    /// Container content-class tags that may hold this content (mirror of the
    /// per-container whitelists in types.csv).
    pub accepted_by: Vec<String>,
    pub description: String,
    /// A container that ever holds this class may never hold food or drinking
    /// water again (2026-09-26; FDA Food Code 7-203.11: a container that held
    /// "poisonous or toxic materials", which include petroleum products and
    /// cleaners, may not be used for food). See docs/design/containers.md.
    #[serde(default)]
    pub leaves_toxic_history: bool,
    /// The item washing this class's residue out takes besides water
    /// (2026-09-26): soap for food and dairy, whose cleaning is a hot wash
    /// with detergent (the Pasteurized Milk Ordinance's rinse, alkaline wash,
    /// acid rinse and sanitize, docs/design/containers.md). One use of it is
    /// worn per clean. None = water alone does.
    #[serde(default)]
    pub cleaning_agent: Option<String>,
}

/// Clean the emptied container on entity `e` (the machine card's Clean,
/// 2026-09-26): water from the home tanks (the caller checks there is some)
/// and, when the residue's content class names a cleaning agent, one use of
/// that agent from the backpack. Ok = (litres of water, the agent used, if
/// any); Err = why it was not cleaned, for the player.
pub fn clean_container(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    e: hecs::Entity,
) -> Result<(f32, Option<String>), String> {
    use crate::systems::inventory::{Inventory, ItemRegistry};
    let items = data.get::<ItemRegistry>("item_registry");
    let name_of = |id: &str| items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.to_string());
    let last = {
        let c = world.get::<&Container>(e).map_err(|_| "That is not a container.".to_string())?;
        if !c.needs_cleaning() {
            return Err("There is nothing to clean out.".to_string());
        }
        c.last_content.clone().unwrap_or_default()
    };
    let agent = data.get::<ContainerRegistry>("container_registry").and_then(|reg| {
        let class = items.map(|r| r.class_for(&last).to_string()).unwrap_or_default();
        reg.content_classes.get(&class).and_then(|c| c.cleaning_agent.clone())
    });
    let player = world
        .query::<(&Inventory, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(p, _)| p);
    if let Some(a) = &agent {
        let has = player.and_then(|p| world.get::<&Inventory>(p).ok().map(|i| i.count_item(a) > 0)).unwrap_or(false);
        if !has {
            return Err(format!(
                "Washing out {} residue takes a {} as well as water: the hot wash needs soap.",
                name_of(&last),
                name_of(a)
            ));
        }
    }
    let litres = world
        .get::<&mut Container>(e)
        .ok()
        .and_then(|mut c| c.clean())
        .ok_or_else(|| "There is nothing to clean out.".to_string())?;
    if let Some(m) = data.get::<std::sync::Mutex<f32>>("hand_water_draw_l") {
        if let Ok(mut v) = m.lock() {
            *v += litres;
        }
    }
    if let (Some(a), Some(p)) = (&agent, player) {
        let base = items.map(|r| r.durability_for(a)).unwrap_or(0);
        let levels = data.get::<crate::systems::crafting::quality::QualityLevels>("quality_levels");
        if let Ok(mut inv) = world.get::<&mut Inventory>(p) {
            inv.wear_item(a, |q| levels.map_or(base, |l| l.durability(base, q)));
        }
    }
    Ok((litres, agent.map(|a| name_of(&a))))
}

// ===========================================================================
// Runtime container instance (ECS component)
// ===========================================================================

/// A typed container instance placed in the world or carried.
///
/// Attach this as an ECS component to any entity that is a typed container
/// (a fuel drum, a cryo tank, a dairy tank). It tracks what it currently holds
/// and how damaged it is. Storing an incompatible content class drives
/// `damage_ratio` toward 0.0; at 0.0 the container is broken and its contents
/// spill (are cleared).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    /// Which archetype this is — an id in `data/containers/types.csv`.
    pub container_type_id: String,
    /// Maximum volume in liters (copied from the type so it survives without
    /// the registry; the registry is the source of truth on (re)load).
    pub capacity_liters: f32,
    /// Volume currently used, in liters.
    pub used_liters: f32,
    /// The item currently stored (None = empty). A typed container holds ONE
    /// content item at a time (mixing is not allowed).
    pub current_content_item: Option<String>,
    /// Quantity of the current content item.
    pub current_qty: u32,
    /// Structural integrity: 1.0 = intact, 0.0 = broken. Reduced when an
    /// incompatible content class is forced in.
    pub damage_ratio: f32,
    /// What it held last (2026-09-26). Survives emptying: residue stays until
    /// the container is CLEANED, and until then it takes only more of the
    /// same content.
    #[serde(default)]
    pub last_content: Option<String>,
    /// The first toxic content it ever held (fuel, solvent, cleaner...). Never
    /// cleared: such a container may not hold food or drinking water again.
    #[serde(default)]
    pub toxic_from: Option<String>,
}

impl Container {
    /// Create a new, empty, intact container of the given type.
    pub fn new(container_type_id: impl Into<String>, capacity_liters: f32) -> Self {
        Self {
            container_type_id: container_type_id.into(),
            capacity_liters,
            used_liters: 0.0,
            current_content_item: None,
            current_qty: 0,
            damage_ratio: 1.0,
            last_content: None,
            toxic_from: None,
        }
    }

    /// Empty but carrying the residue of something it must be cleaned of
    /// before it can hold anything else.
    pub fn needs_cleaning(&self) -> bool {
        self.is_empty() && self.last_content.is_some()
    }

    /// Clean an EMPTY container: its last content is washed out, so it can
    /// take something else. A toxic history stays. Returns the litres of water
    /// the wash used (2% of capacity, at least 1 L, at most 50 L), or None
    /// when there is nothing to clean or it still holds something.
    pub fn clean(&mut self) -> Option<f32> {
        if !self.needs_cleaning() {
            return None;
        }
        self.last_content = None;
        Some((self.capacity_liters * 0.02).clamp(1.0, 50.0))
    }

    /// Build a container instance directly from a registry archetype.
    pub fn from_type(t: &ContainerType) -> Self {
        Self::new(t.id.clone(), t.capacity_liters)
    }

    /// Remaining free volume in liters (0 if broken).
    pub fn remaining_liters(&self) -> f32 {
        if self.is_broken() {
            return 0.0;
        }
        (self.capacity_liters - self.used_liters).max(0.0)
    }

    /// Whether the container has been destroyed by incompatible content.
    pub fn is_broken(&self) -> bool {
        self.damage_ratio <= 0.0
    }

    /// Whether the container currently holds anything.
    pub fn is_empty(&self) -> bool {
        self.current_content_item.is_none() || self.current_qty == 0
    }

    /// Apply one increment of incompatibility damage. Clamps at 0.0.
    /// Returns true if this hit BROKE the container (crossed to <= 0.0).
    pub fn apply_incompatible_damage(&mut self, amount: f32) -> bool {
        if self.is_broken() {
            return false; // already broken
        }
        self.damage_ratio = (self.damage_ratio - amount).max(0.0);
        if self.is_broken() {
            // Contents spill when the vessel ruptures.
            self.spill();
            true
        } else {
            false
        }
    }

    /// Empty the container (contents are lost — spilled on break, or poured out).
    pub fn spill(&mut self) {
        self.current_content_item = None;
        self.current_qty = 0;
        self.used_liters = 0.0;
    }
}

// ===========================================================================
// Compatibility result
// ===========================================================================

/// The outcome of checking whether an item may be stored in a container.
#[derive(Debug, Clone, PartialEq)]
pub enum Compatibility {
    /// The content class is on the container's whitelist — safe to store.
    Accepted,
    /// The content class is NOT accepted. Storing it will damage the container.
    /// Carries a human-readable reason for tooltips / logs.
    Incompatible { reason: String },
}

impl Compatibility {
    pub fn is_accepted(&self) -> bool {
        matches!(self, Compatibility::Accepted)
    }
}

/// The result of a `try_store` attempt against a typed container.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreOutcome {
    /// Stored successfully; returns the quantity actually stored.
    Stored { quantity: u32 },
    /// Incompatible content — the container took damage. `broke` is true if
    /// this attempt destroyed the container.
    Damaged { reason: String, broke: bool },
    /// Compatible but no room (full or broken).
    NoRoom,
    /// Compatible but a DIFFERENT content item is already inside (no mixing).
    WrongContent { current: String },
    /// Empty, but it still carries the residue of `last`: clean it first.
    NeedsCleaning { last: String },
    /// Its material is not fit for this content (not food-grade, or it
    /// reacts with it). Refused without damage.
    Unfit { reason: String },
    /// It once held a toxic content, so it may never hold food or drinking
    /// water again (it can still hold other non-food contents).
    Contaminated { toxic_from: String },
}

// ===========================================================================
// Registry — loaded rule table + the compatibility check
// ===========================================================================

/// How much integrity an incompatible store attempt removes (per attempt).
/// 0.1 -> ten attempts break a fresh container. Tunable; lives here as the one
/// behavioral constant (the *data* is what classes/containers exist).
pub const INCOMPATIBLE_DAMAGE_PER_ATTEMPT: f32 = 0.1;

/// Registry of container archetypes + content classes, loaded from data files.
///
/// Cached in the [`DataStore`] under the key `"container_registry"` (native
/// build). Game systems read it to run the compatibility check.
#[derive(Debug, Clone, Default)]
pub struct ContainerRegistry {
    /// Container archetypes keyed by id (from `types.csv`).
    pub types: HashMap<String, ContainerType>,
    /// Content classes keyed by id (from `content_classes.ron`).
    pub content_classes: HashMap<String, ContentClass>,
    /// Contact materials keyed by id (from `materials.csv`, 2026-09-26).
    pub materials: HashMap<String, Material>,
    /// Food profile -> traits, and content class -> traits
    /// (`content_traits.ron`).
    pub traits_by_profile: HashMap<String, Vec<String>>,
    pub traits_by_class: HashMap<String, Vec<String>>,
    /// Edible item -> food profile (`food/item_profiles.ron`).
    pub profile_of: HashMap<String, String>,
}

impl ContainerRegistry {
    /// Build a registry from raw file bytes.
    ///
    /// `types_csv` = bytes of `data/containers/types.csv`.
    /// `classes_ron` = bytes of `data/containers/content_classes.ron`.
    ///
    /// Uses the crate's pure parsers, so this works in every build and in
    /// tests without touching the filesystem. Malformed rows are skipped with
    /// a warning by the underlying CSV parser (graceful degradation).
    pub fn from_bytes(types_csv: &[u8], classes_ron: &[u8]) -> Result<Self, String> {
        let rows: Vec<ContainerTypeRow> = crate::assets::loader::parse_csv(types_csv)?;
        let classes: Vec<ContentClass> = crate::assets::loader::parse_ron(classes_ron)?;

        let mut types = HashMap::new();
        for row in rows {
            let t = ContainerType::from_row(row);
            types.insert(t.id.clone(), t);
        }

        let mut content_classes = HashMap::new();
        for c in classes {
            content_classes.insert(c.id.clone(), c);
        }

        Ok(Self {
            types,
            content_classes,
            // The contact rules arrive through `with_contact_rules`.
            ..Default::default()
        })
    }

    /// Look up a container archetype by id.
    pub fn container_type(&self, id: &str) -> Option<&ContainerType> {
        self.types.get(id)
    }

    /// Look up a content class by id.
    pub fn content_class(&self, id: &str) -> Option<&ContentClass> {
        self.content_classes.get(id)
    }

    /// THE compatibility check: may a given content class go into a given
    /// container type?
    ///
    /// The authoritative gate is the container's `accepted_content_classes`
    /// whitelist. If the class is on the whitelist -> [`Compatibility::Accepted`].
    /// Otherwise -> [`Compatibility::Incompatible`] with a reason that explains
    /// *why* (using the content class's physical requirements when known).
    ///
    /// This is data-driven end to end: it consults only the loaded archetype +
    /// class records. No hardcoded container or class names.
    pub fn check(&self, container_type_id: &str, content_class: &str) -> Compatibility {
        let ctype = match self.types.get(container_type_id) {
            Some(t) => t,
            None => {
                return Compatibility::Incompatible {
                    reason: format!("Unknown container type '{container_type_id}'"),
                };
            }
        };

        // The whitelist is authoritative.
        if ctype.accepts_class(content_class) {
            return Compatibility::Accepted;
        }

        // Not accepted — build a helpful reason. If we know the class's needs,
        // explain the likely mismatch (pressure / insulation), which is what
        // makes "cement into a dairy tank" and "LN2 into a plain tank" read
        // naturally to the player.
        let reason = if let Some(cc) = self.content_classes.get(content_class) {
            if cc.needs_insulation && ctype.min_temp_c > cc.min_temp_c {
                format!(
                    "{} needs an insulated vessel (down to {:.0} C); {} only rated to {:.0} C",
                    cc.name, cc.min_temp_c, ctype.name, ctype.min_temp_c
                )
            } else if cc.needs_pressure && ctype.pressure_rating_atm <= 1.0 {
                format!(
                    "{} must be held under pressure; {} is not a pressure vessel",
                    cc.name, ctype.name
                )
            } else {
                format!(
                    "{} cannot safely hold {} ({})",
                    ctype.name, cc.name, cc.description
                )
            }
        } else {
            format!(
                "{} does not accept content class '{}'",
                ctype.name, content_class
            )
        };

        Compatibility::Incompatible { reason }
    }

    /// Attempt to store `qty` units of an item (with the given content class
    /// and per-unit volume in liters) into a container instance.
    ///
    /// Behavior:
    ///   * Broken or full container -> [`StoreOutcome::NoRoom`].
    ///   * Incompatible class -> damage the container (and possibly break it),
    ///     returning [`StoreOutcome::Damaged`]. NOTHING is stored.
    ///   * Compatible but a different item already inside -> [`StoreOutcome::WrongContent`].
    ///   * Compatible + room -> store as much as fits, [`StoreOutcome::Stored`].
    ///
    /// This is the single funnel the inventory/storage flow should call so the
    /// "wrong material breaks the container" rule is enforced in one place.
    /// Add the contact rules (2026-09-26, docs/design/containers.md): the
    /// materials table, content traits, and the item -> food profile map the
    /// traits are keyed through. Without them no material rule applies.
    pub fn with_contact_rules(
        mut self,
        materials_csv: &[u8],
        traits_ron: &[u8],
        item_profiles_ron: &[u8],
    ) -> Result<Self, String> {
        let mats: Vec<Material> = crate::assets::loader::parse_csv(materials_csv)?;
        self.materials = mats.into_iter().map(|m| (m.id.clone(), m)).collect();
        let traits: ContentTraitsFile = crate::assets::loader::parse_ron(traits_ron)?;
        self.traits_by_profile = traits.profiles;
        self.traits_by_class = traits.classes;
        let profiles: ItemProfilesFile = crate::assets::loader::parse_ron(item_profiles_ron)?;
        self.profile_of = profiles.items.into_iter().collect();
        Ok(self)
    }

    /// The traits a content carries: its food profile's, plus its class's.
    pub fn traits_for(&self, item_id: &str, content_class: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .profile_of
            .get(item_id)
            .and_then(|p| self.traits_by_profile.get(p))
            .cloned()
            .unwrap_or_default();
        for t in self.traits_by_class.get(content_class).into_iter().flatten() {
            if !out.contains(t) {
                out.push(t.clone());
            }
        }
        out
    }

    /// Is this vessel's material fit for this content? Only a direct-contact
    /// vessel is judged: food and drinking water need a food-grade surface,
    /// and a material refuses the traits it reacts with (copper and acidic
    /// foods, galvanised steel and drinking water, and so on).
    pub fn material_fit(&self, ctype: &ContainerType, item_id: &str, content_class: &str) -> Result<(), String> {
        if !ctype.direct_contact {
            return Ok(());
        }
        let Some(mat) = self.materials.get(&ctype.base_material) else {
            return Ok(());
        };
        if self.content_classes.get(content_class).map_or(false, |c| c.food_safe) && !mat.food_grade {
            return Err(format!("{} is not food-grade: no food or drinking water in it", mat.name));
        }
        for t in self.traits_for(item_id, content_class) {
            if mat.refuses_trait(&t) {
                return Err(format!("{} must not hold {} contents", mat.name, trait_words(&t)));
            }
        }
        Ok(())
    }

    /// Would `try_store` take this item now, and if not, why? The class
    /// whitelist plus the history rules (mixing, residue, toxic history), for
    /// UIs that list what a container will accept without trying.
    pub fn would_accept(&self, container: &Container, item_id: &str, content_class: &str) -> Result<(), String> {
        if let Compatibility::Incompatible { reason } = self.check(&container.container_type_id, content_class) {
            return Err(reason);
        }
        if let Some(ctype) = self.types.get(&container.container_type_id) {
            self.material_fit(ctype, item_id, content_class)?;
        }
        if self.content_classes.get(content_class).map_or(false, |c| c.food_safe) {
            if let Some(t) = &container.toxic_from {
                return Err(format!("held {t}: never food or drinking water again"));
            }
        }
        match (&container.current_content_item, &container.last_content) {
            (Some(cur), _) if !container.is_empty() && cur != item_id => Err(format!("already holds {cur}")),
            (_, Some(last)) if container.is_empty() && last != item_id => Err(format!("still has {last} residue: clean it first")),
            _ => Ok(()),
        }
    }

    pub fn try_store(
        &self,
        container: &mut Container,
        item_id: &str,
        content_class: &str,
        unit_volume_liters: f32,
        qty: u32,
    ) -> StoreOutcome {
        if container.is_broken() {
            return StoreOutcome::NoRoom;
        }

        // 1. Compatibility gate — wrong class damages the vessel.
        if let Compatibility::Incompatible { reason } =
            self.check(&container.container_type_id, content_class)
        {
            let broke = container.apply_incompatible_damage(INCOMPATIBLE_DAMAGE_PER_ATTEMPT);
            return StoreOutcome::Damaged { reason, broke };
        }

        // 1a. Material (2026-09-26): an unfit surface refuses, without damage.
        if let Some(ctype) = self.types.get(&container.container_type_id) {
            if let Err(reason) = self.material_fit(ctype, item_id, content_class) {
                return StoreOutcome::Unfit { reason };
            }
        }

        // 1b. History (2026-09-26, docs/design/containers.md). A container
        //     that ever held a toxic content never holds food or water again,
        //     and an emptied container takes nothing new until it is cleaned.
        let class = self.content_classes.get(content_class);
        if class.map_or(false, |c| c.food_safe) {
            if let Some(t) = &container.toxic_from {
                return StoreOutcome::Contaminated { toxic_from: t.clone() };
            }
        }
        if container.is_empty() {
            if let Some(last) = &container.last_content {
                if last != item_id {
                    return StoreOutcome::NeedsCleaning { last: last.clone() };
                }
            }
        }

        // 2. No mixing: a typed container holds one content item at a time.
        if let Some(current) = &container.current_content_item {
            if current != item_id {
                return StoreOutcome::WrongContent {
                    current: current.clone(),
                };
            }
        }

        // 3. Volume check — store as much as fits.
        let unit_vol = unit_volume_liters.max(0.0);
        let storable = if unit_vol <= 0.0 {
            // Zero-volume items are quantity-limited only by qty (treat as fitting).
            qty
        } else {
            let room = container.remaining_liters();
            let fits = (room / unit_vol).floor() as u32;
            fits.min(qty)
        };

        if storable == 0 {
            return StoreOutcome::NoRoom;
        }

        container.current_content_item = Some(item_id.to_string());
        container.last_content = Some(item_id.to_string());
        if container.toxic_from.is_none() && class.map_or(false, |c| c.leaves_toxic_history) {
            container.toxic_from = Some(item_id.to_string());
        }
        container.current_qty += storable;
        container.used_liters += unit_vol * storable as f32;
        StoreOutcome::Stored { quantity: storable }
    }
}

// ===========================================================================
// ContainerCompatibilitySystem — per-tick enforcement
// ===========================================================================

/// A queued request to store an item into a specific container entity.
///
/// Game code (UI drag-drop, NPC AI, automation) queues these; the system
/// validates them against the [`ContainerRegistry`] each tick.
#[derive(Debug, Clone)]
pub struct StoreRequest {
    pub container_entity: hecs::Entity,
    pub item_id: String,
    pub content_class: String,
    pub unit_volume_liters: f32,
    pub quantity: u32,
}

/// System that enforces container/content compatibility each tick.
///
/// It drains queued [`StoreRequest`]s, runs them through
/// [`ContainerRegistry::try_store`], and logs the outcome (stored / damaged /
/// broken). Registered in the `SystemRunner` alongside `InventorySystem`.
pub struct ContainerCompatibilitySystem {
    pending: Vec<StoreRequest>,
}

impl ContainerCompatibilitySystem {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Queue a store attempt for validation on the next tick.
    pub fn queue_store(&mut self, req: StoreRequest) {
        self.pending.push(req);
    }
}

impl Default for ContainerCompatibilitySystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for ContainerCompatibilitySystem {
    fn name(&self) -> &str {
        "ContainerCompatibilitySystem"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        if self.pending.is_empty() {
            return;
        }

        // The registry is data-driven and loaded into the DataStore at startup.
        // If it isn't loaded yet, keep the requests for a later tick.
        let registry = match data.get::<ContainerRegistry>("container_registry") {
            Some(r) => r,
            None => {
                log::debug!(
                    "ContainerRegistry not loaded; deferring {} store request(s)",
                    self.pending.len()
                );
                return;
            }
        };

        let requests: Vec<_> = self.pending.drain(..).collect();
        for req in requests {
            let mut container = match world.get::<&mut Container>(req.container_entity) {
                Ok(c) => c,
                Err(_) => {
                    log::warn!("StoreRequest on entity without a Container component");
                    continue;
                }
            };

            match registry.try_store(
                &mut container,
                &req.item_id,
                &req.content_class,
                req.unit_volume_liters,
                req.quantity,
            ) {
                StoreOutcome::Stored { quantity } => {
                    log::debug!(
                        "Stored {}x {} in {} ({:.0}/{:.0} L)",
                        quantity,
                        req.item_id,
                        container.container_type_id,
                        container.used_liters,
                        container.capacity_liters
                    );
                }
                StoreOutcome::Damaged { reason, broke } => {
                    if broke {
                        log::warn!(
                            "Container {} BROKE: {} (contents spilled)",
                            container.container_type_id,
                            reason
                        );
                    } else {
                        log::warn!(
                            "Incompatible content into {} (integrity {:.0}%): {}",
                            container.container_type_id,
                            container.damage_ratio * 100.0,
                            reason
                        );
                    }
                }
                StoreOutcome::NoRoom => {
                    log::debug!(
                        "No room in {} for {}",
                        container.container_type_id, req.item_id
                    );
                }
                StoreOutcome::WrongContent { current } => {
                    log::debug!(
                        "{} already holds {} — empty it before adding {}",
                        container.container_type_id,
                        current,
                        req.item_id
                    );
                }
                StoreOutcome::Unfit { reason } => {
                    log::debug!("{} refused {}: {}", container.container_type_id, req.item_id, reason);
                }
                StoreOutcome::NeedsCleaning { last } => {
                    log::debug!(
                        "{} still has {} residue: clean it before adding {}",
                        container.container_type_id,
                        last,
                        req.item_id
                    );
                }
                StoreOutcome::Contaminated { toxic_from } => {
                    log::debug!(
                        "{} once held {}: never food or water again (refused {})",
                        container.container_type_id,
                        toxic_from,
                        req.item_id
                    );
                }
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Load the registry from the actual repo data files so the tests also
    /// exercise the real CSV/RON content (test (d): the registry loads).
    fn load_registry() -> ContainerRegistry {
        // CARGO_MANIFEST_DIR points at the crate root; data/ lives beside it.
        let root = env!("CARGO_MANIFEST_DIR");
        let types =
            std::fs::read(format!("{root}/data/containers/types.csv")).expect("read types.csv");
        let classes = std::fs::read(format!("{root}/data/containers/content_classes.ron"))
            .expect("read content_classes.ron");
        let read = |rel: &str| std::fs::read(format!("{root}/data/{rel}")).expect(rel);
        ContainerRegistry::from_bytes(&types, &classes)
            .expect("build registry")
            .with_contact_rules(
                &read("containers/materials.csv"),
                &read("containers/content_traits.ron"),
                &read("food/item_profiles.ron"),
            )
            .expect("contact rules")
    }

    #[test]
    fn registry_loads_csv_and_ron() {
        // Test (d): the registry parses both data files and finds the
        // archetypes + classes the rest of the tests rely on.
        let reg = load_registry();
        assert!(
            reg.types.len() >= 10,
            "expected the full container spread, got {}",
            reg.types.len()
        );
        assert!(reg.content_classes.len() >= 8, "expected >=8 content classes");

        // Spot-check the operator's example vessels exist and parsed their
        // pipe-separated whitelists correctly.
        let cryo = reg.container_type("cryo_pressure_tank").expect("cryo tank");
        assert!(cryo.accepts_class("liquid_cryogenic"));
        assert!(cryo.pressure_rating_atm > 1.0, "cryo tank is a pressure vessel");

        let dairy = reg.container_type("dairy_food_tank").expect("dairy tank");
        assert!(dairy.accepts_class("liquid"));
        assert!(dairy.accepts_class("food"));
        assert!(!dairy.accepts_class("solid"), "dairy tank must reject solids");

        // Content classes parsed their requirement flags.
        let cryo_class = reg.content_class("liquid_cryogenic").expect("cryo class");
        assert!(cryo_class.needs_pressure && cryo_class.needs_insulation);
    }

    #[test]
    fn liquid_stores_fine_in_a_liquid_tank() {
        // Test (a): a compatible liquid stores cleanly, no damage.
        let reg = load_registry();
        let mut tank = Container::from_type(reg.container_type("hdpe_water_tank").unwrap());

        // water (content_class "water") into a water tank.
        let outcome = reg.try_store(&mut tank, "water_purified_0", "water", 1.0, 100);
        assert_eq!(outcome, StoreOutcome::Stored { quantity: 100 });
        assert_eq!(tank.damage_ratio, 1.0, "no damage from compatible content");
        assert_eq!(
            tank.current_content_item.as_deref(),
            Some("water_purified_0")
        );
        assert!((tank.used_liters - 100.0).abs() < 1e-3);
        assert!(!tank.is_broken());

        // A generic "liquid" also fits a wooden barrel (whitelist liquid|food).
        let mut barrel = Container::from_type(reg.container_type("wooden_barrel").unwrap());
        assert!(reg.check("wooden_barrel", "liquid").is_accepted());
        let r = reg.try_store(&mut barrel, "oil_lamp_fuel_0", "liquid", 2.0, 10);
        assert_eq!(r, StoreOutcome::Stored { quantity: 10 });
    }

    #[test]
    fn cement_into_a_dairy_tank_damages_then_breaks_it() {
        // Test (b): the operator's headline example. Cement is a dry solid;
        // a dairy/liquid tank only accepts liquid|food. Each attempt damages
        // the tank; enough attempts BREAK it and spill the contents.
        let reg = load_registry();

        // Sanity: cement (solid/dry_goods) is incompatible with the liquid tank.
        let compat = reg.check("dairy_food_tank", "dry_goods");
        assert!(
            matches!(compat, Compatibility::Incompatible { .. }),
            "cement (dry_goods) must be incompatible with a dairy/liquid tank"
        );

        let mut tank = Container::from_type(reg.container_type("dairy_food_tank").unwrap());

        // First incompatible store: damaged, not yet broken, nothing stored.
        let outcome = reg.try_store(&mut tank, "cement_0", "dry_goods", 1.6, 1);
        match outcome {
            StoreOutcome::Damaged { broke, .. } => {
                assert!(!broke, "one hit should not break it")
            }
            other => panic!("expected Damaged, got {other:?}"),
        }
        assert!(tank.damage_ratio < 1.0 && tank.damage_ratio > 0.0);
        assert!(tank.is_empty(), "incompatible content is never stored");

        // Keep pouring cement in. With 0.1 damage/attempt, the 10th attempt
        // (from a fresh 1.0) reaches 0.0 and breaks it. We already did one.
        let mut broke_at = None;
        for attempt in 2..=20 {
            let o = reg.try_store(&mut tank, "cement_0", "dry_goods", 1.6, 1);
            if let StoreOutcome::Damaged { broke: true, .. } = o {
                broke_at = Some(attempt);
                break;
            }
        }
        assert_eq!(broke_at, Some(10), "should break on the 10th attempt at 0.1/hit");
        assert!(tank.is_broken());
        assert!(tank.is_empty(), "a broken tank has spilled its contents");

        // A broken container takes no more and stores nothing.
        assert_eq!(
            reg.try_store(&mut tank, "milk_0", "food", 1.0, 1),
            StoreOutcome::NoRoom
        );
    }

    #[test]
    fn liquid_nitrogen_needs_the_cryogenic_tank() {
        // Test (c): cryogenic liquids require a vessel whose whitelist includes
        // `liquid_cryogenic`. A normal liquid tank is INCOMPATIBLE; the cryo
        // pressure tank ACCEPTS.
        let reg = load_registry();

        // A plain HDPE water tank must reject liquid nitrogen.
        let normal = reg.check("hdpe_water_tank", "liquid_cryogenic");
        assert!(
            matches!(normal, Compatibility::Incompatible { .. }),
            "a normal tank cannot hold liquid nitrogen"
        );
        // And storing it would damage that tank.
        let mut water_tank = Container::from_type(reg.container_type("hdpe_water_tank").unwrap());
        match reg.try_store(&mut water_tank, "liquid_nitrogen_0", "liquid_cryogenic", 1.0, 5) {
            StoreOutcome::Damaged { broke, .. } => assert!(!broke),
            other => panic!("expected Damaged storing LN2 in a water tank, got {other:?}"),
        }
        assert!(water_tank.damage_ratio < 1.0);

        // The cryogenic pressure tank ACCEPTS it and stores it cleanly.
        assert!(reg.check("cryo_pressure_tank", "liquid_cryogenic").is_accepted());
        let mut cryo = Container::from_type(reg.container_type("cryo_pressure_tank").unwrap());
        let outcome = reg.try_store(&mut cryo, "liquid_nitrogen_0", "liquid_cryogenic", 1.0, 50);
        assert_eq!(outcome, StoreOutcome::Stored { quantity: 50 });
        assert_eq!(cryo.damage_ratio, 1.0);

        // Liquid CO2 (also cryogenic) shares the same tank — the operator's
        // "N2 OR CO2 in the same pressure tank" point. Empty first (no mixing),
        // and since 2026-09-26 clean it out (a purge, in reality) before the
        // other gas goes in: an emptied tank still carries its last content.
        cryo.spill();
        assert!(matches!(
            reg.try_store(&mut cryo, "liquid_co2_0", "liquid_cryogenic", 1.0, 50),
            StoreOutcome::NeedsCleaning { .. }
        ));
        cryo.clean().expect("purge the empty tank");
        assert_eq!(
            reg.try_store(&mut cryo, "liquid_co2_0", "liquid_cryogenic", 1.0, 50),
            StoreOutcome::Stored { quantity: 50 }
        );
    }

    /// Containers remember what they held (2026-09-26; docs/design/containers.md).
    /// A tote that held fuel may never hold water or food again, even cleaned;
    /// it may still hold fuel.
    #[test]
    fn a_container_that_held_fuel_never_holds_water_again() {
        let reg = load_registry();
        let mut tote = Container::from_type(reg.container_type("ibc_tote").unwrap());
        assert_eq!(reg.try_store(&mut tote, "fuel_refined_0", "flammable", 1.0, 10), StoreOutcome::Stored { quantity: 10 });
        assert_eq!(tote.toxic_from.as_deref(), Some("fuel_refined_0"));
        tote.spill();
        assert!(tote.needs_cleaning(), "the residue stays after emptying");
        assert!(tote.clean().is_some());
        match reg.try_store(&mut tote, "water_purified_0", "water", 1.0, 1) {
            StoreOutcome::Contaminated { toxic_from } => assert_eq!(toxic_from, "fuel_refined_0"),
            other => panic!("expected Contaminated, got {other:?}"),
        }
        assert!(reg.would_accept(&tote, "water_purified_0", "water").is_err());
        assert_eq!(
            reg.try_store(&mut tote, "fuel_refined_0", "flammable", 1.0, 1),
            StoreOutcome::Stored { quantity: 1 },
            "it can still hold fuel"
        );
    }

    /// An emptied container carries the residue of what it held: the same
    /// content tops up, anything else is refused until it is cleaned, and
    /// cleaning costs water in proportion to its size.
    #[test]
    fn an_emptied_container_must_be_cleaned_before_holding_something_else() {
        let reg = load_registry();
        let mut tank = Container::from_type(reg.container_type("dairy_food_tank").unwrap());
        assert_eq!(reg.try_store(&mut tank, "milk_0", "food", 1.0, 5), StoreOutcome::Stored { quantity: 5 });
        tank.spill();
        match reg.try_store(&mut tank, "juice_0", "food", 1.0, 1) {
            StoreOutcome::NeedsCleaning { last } => assert_eq!(last, "milk_0"),
            other => panic!("expected NeedsCleaning, got {other:?}"),
        }
        assert_eq!(reg.try_store(&mut tank, "milk_0", "food", 1.0, 1), StoreOutcome::Stored { quantity: 1 }, "more milk is fine");
        assert_eq!(tank.clean(), None, "cannot clean while it holds something");
        tank.spill();
        let litres = tank.clean().expect("clean an empty tank");
        assert!((litres - 40.0).abs() < 1e-3, "2% of 2000 L: {litres}");
        assert!(!tank.needs_cleaning());
        assert_eq!(reg.try_store(&mut tank, "juice_0", "food", 1.0, 1), StoreOutcome::Stored { quantity: 1 });
        assert!(tank.toxic_from.is_none(), "food never leaves a toxic history");
    }

    /// Washing out a food residue takes soap as well as water (2026-09-26):
    /// refused without a bar, one use of the bar worn when cleaned; a vessel
    /// that held only water is rinsed with water alone.
    #[test]
    fn a_food_residue_needs_soap_and_water_does_not() {
        use crate::hot_reload::data_store::DataStore;
        use crate::systems::inventory::{Inventory, ItemRegistry};
        let reg = load_registry();
        let mut data = DataStore::new();
        let root = env!("CARGO_MANIFEST_DIR");
        data.insert("item_registry", ItemRegistry::from_csv(&std::fs::read(format!("{root}/data/items.csv")).unwrap()).unwrap());
        data.insert("container_registry", reg.clone());
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(8), crate::ecs::components::Controllable));
        let mut tank = Container::from_type(reg.container_type("dairy_food_tank").unwrap());
        reg.try_store(&mut tank, "milk_0", "food", 1.0, 2);
        tank.current_qty = 0;
        tank.current_content_item = None;
        let e = world.spawn((tank,));
        let err = clean_container(&mut world, &data, e).unwrap_err();
        assert!(err.contains("Soap"), "{err}");
        assert!(world.get::<&Container>(e).unwrap().needs_cleaning(), "not cleaned without soap");
        world.get::<&mut Inventory>(player).unwrap().add_item("soap_bar_0", 1, 20);
        let (litres, agent) = clean_container(&mut world, &data, e).unwrap();
        assert!(litres > 0.0);
        assert_eq!(agent.as_deref(), Some("Soap Bar"));
        assert!(!world.get::<&Container>(e).unwrap().needs_cleaning());
        let wear = world.get::<&Inventory>(player).unwrap().slots.iter().flatten().find(|s| s.item_id == "soap_bar_0").map(|s| s.wear);
        assert_eq!(wear, Some(1), "one wash worn off the bar");

        let mut jug = Container::from_type(reg.container_type("ibc_tote").unwrap());
        reg.try_store(&mut jug, "water_purified_0", "water", 1.0, 1);
        jug.current_qty = 0;
        jug.current_content_item = None;
        let j = world.spawn((jug,));
        let (_, agent) = clean_container(&mut world, &data, j).unwrap();
        assert_eq!(agent, None, "water residue rinses out with water alone");
    }

    fn unfit(o: StoreOutcome) -> String {
        match o {
            StoreOutcome::Unfit { reason } => reason,
            other => panic!("expected Unfit, got {other:?}"),
        }
    }

    /// The material a vessel is made of decides what it may hold (2026-09-26,
    /// docs/design/containers.md; the rules come from the dated research).
    /// Copper refuses acidic foods, oils and milk but holds water; galvanised
    /// steel refuses drinking water and acidic foods but holds grain; glazed
    /// stoneware takes juice; 304 stainless takes milk but not a salty cheese.
    #[test]
    fn the_material_decides_what_a_vessel_may_hold() {
        let reg = load_registry();
        let fresh = |id: &str| Container::from_type(reg.container_type(id).unwrap());

        let mut pot = fresh("copper_pot");
        assert!(unfit(reg.try_store(&mut pot, "juice_0", "food", 1.0, 1)).contains("acidic"));
        assert!(unfit(reg.try_store(&mut pot, "milk_0", "food", 1.0, 1)).contains("milk"));
        assert!(unfit(reg.try_store(&mut pot, "cooking_oil_0", "food", 1.0, 1)).contains("oily"));
        assert_eq!(reg.try_store(&mut pot, "water_purified_0", "water", 1.0, 1), StoreOutcome::Stored { quantity: 1 });
        assert_eq!(pot.damage_ratio, 1.0, "a refusal does no damage");

        let mut bucket = fresh("galvanized_bucket");
        assert!(unfit(reg.try_store(&mut bucket, "water_purified_0", "water", 1.0, 1)).contains("drinking water"));
        assert!(unfit(reg.try_store(&mut bucket, "honey_0", "food", 1.0, 1)).contains("acidic"));
        assert_eq!(reg.try_store(&mut bucket, "grain_wheat_0", "dry_goods", 1.0, 3), StoreOutcome::Stored { quantity: 3 });

        let mut crock = fresh("fermenting_crock");
        assert_eq!(reg.try_store(&mut crock, "juice_0", "food", 1.0, 2), StoreOutcome::Stored { quantity: 2 });

        let mut tank = fresh("dairy_food_tank");
        assert_eq!(reg.try_store(&mut tank, "milk_0", "food", 1.0, 5), StoreOutcome::Stored { quantity: 5 });
        let mut tank2 = fresh("dairy_food_tank");
        assert!(unfit(reg.try_store(&mut tank2, "cheese_0", "food", 1.0, 1)).contains("salty"));

        // A freezer holds packaged food: its aluminium liner is not judged.
        let mut freezer = fresh("freezer_chest");
        assert_eq!(reg.try_store(&mut freezer, "juice_0", "food", 1.0, 1), StoreOutcome::Stored { quantity: 1 });

        // would_accept agrees with try_store, so a menu never offers a refusal.
        assert!(reg.would_accept(&fresh("copper_pot"), "juice_0", "food").is_err());
        assert!(reg.would_accept(&fresh("copper_pot"), "water_purified_0", "water").is_ok());
    }

    /// A food vessel made of a non-food-grade material refuses all food.
    #[test]
    fn a_non_food_grade_vessel_refuses_food() {
        let root = env!("CARGO_MANIFEST_DIR");
        let types = b"id,name,base_material,capacity_liters,pressure_rating_atm,max_temp_c,min_temp_c,accepted_content_classes,description,direct_contact\n\
            steel_bin,Steel Bin,carbon_steel,50.0,1.0,60.0,-20.0,food|dry_goods,test,true\n";
        let read = |rel: &str| std::fs::read(format!("{root}/data/{rel}")).expect(rel);
        let reg = ContainerRegistry::from_bytes(types, &read("containers/content_classes.ron"))
            .unwrap()
            .with_contact_rules(
                &read("containers/materials.csv"),
                &read("containers/content_traits.ron"),
                &read("food/item_profiles.ron"),
            )
            .unwrap();
        let mut bin = Container::from_type(reg.container_type("steel_bin").unwrap());
        assert!(unfit(reg.try_store(&mut bin, "milk_0", "food", 1.0, 1)).contains("not food-grade"));
    }

    /// Every container type names a real material, so no type silently
    /// escapes the material rules.
    #[test]
    fn every_container_type_names_a_real_material() {
        let reg = load_registry();
        for t in reg.types.values() {
            assert!(reg.materials.contains_key(&t.base_material), "{} names material {}", t.id, t.base_material);
        }
    }

    #[test]
    fn volume_capacity_is_respected_and_no_mixing() {
        // Extra guard: stores cap at remaining volume, and a second (different)
        // item is refused while one is already inside.
        let reg = load_registry();
        // glass food jar capacity is 2.0 L.
        let mut jar = Container::from_type(reg.container_type("glass_food_jar").unwrap());
        assert!((jar.capacity_liters - 2.0).abs() < 1e-3);

        // Try to store 5 units of a 1 L food item — only 2 fit.
        let outcome = reg.try_store(&mut jar, "honey_0", "food", 1.0, 5);
        assert_eq!(outcome, StoreOutcome::Stored { quantity: 2 });
        assert!(jar.remaining_liters() < 1e-3);

        // A different compatible item is refused (no mixing) until emptied.
        match reg.try_store(&mut jar, "jam_0", "food", 1.0, 1) {
            StoreOutcome::WrongContent { current } => assert_eq!(current, "honey_0"),
            // If it read NoRoom because the jar is full, that's also acceptable
            // — either way the second item was not mixed in.
            StoreOutcome::NoRoom => {}
            other => panic!("expected WrongContent/NoRoom, got {other:?}"),
        }
    }
}
