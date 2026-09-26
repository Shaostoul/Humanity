//! The VALUE TYPES the GUI passes around: one struct or enum per thing a page
//! shows, and the small conversions that build them.
//!
//! These are not `GuiState` and they are not the pages. They are the vocabulary
//! in between: an item slot, a task, a listing, a review, a trade, a crop, a
//! quest, a chat message, a channel, a server connection, a studio scene, a
//! construction room, a toast. Something upstream (the ECS bridge, a relay
//! reply, a data file) produces one, `GuiState` holds it, and a page draws it.
//!
//! They are one cluster because they share that role, and because the only code
//! that comes with them is the code that role needs: the `from_relay_json`
//! mappers that turn one JSON object into one of these, and the label/cycle
//! helpers on the small mode enums. Anything that reaches further than that
//! stayed in `gui/mod.rs`.
//!
//! Extracted VERBATIM from `src/gui/mod.rs` (file-size ratchet), which stood at
//! 7,915 lines against a 7,050 budget. Second of two clusters out in that pass.
//! Every item in here was already `pub`, so the visibility delta is ZERO: not
//! one signature or attribute changed, and `pub use state_types::*` in the
//! parent keeps every `crate::gui::ChatMessage` spelling in the crate resolving
//! exactly as before.
//!
//! WHAT DELIBERATELY STAYED in `gui/mod.rs`, and why in each case:
//!
//!   * `GuiPage`, `BOOT_PAGE_OPTIONS`, `page_to_config_str` and
//!     `config_str_to_page`. `tests/page_registry_lint.rs` reads the `GuiPage`
//!     enum out of `src/gui/mod.rs` by hand to check every variant is in
//!     docs/PAGES.md, so that enum's home is load-bearing, and its three
//!     config-string helpers belong beside it.
//!   * `PassphraseMode`, `ServerInfo`, `LauncherWhere`, `LauncherHome`,
//!     `ToolEntry` and `DonateAddress`: app-shell types rather than page
//!     vocabulary, and they sit with the module-level helpers that use them.
//!   * `GuiState` itself, its `impl`, and `SettingsState`. `src/engine/input.rs`
//!     also scans `src/gui/mod.rs` for every `cloud_dev_*` field on `GuiState`,
//!     which is a second reason that struct does not move casually.
//!
//! ONE CHARACTER-LEVEL EXCEPTION to "verbatim", and it is a comment: the
//! fallback guild colour in `GuiGuild::from_relay_json` had its
//! `// theme-exempt:` note on the line ABOVE the literal. `theme_token_lint`
//! matches per line, and `src/gui/mod.rs` was on its legacy allowlist while
//! this new file is not, so the note is now on the literal's own line where it
//! actually exempts something. That is the same note, moved one line, and it is
//! the last colour literal either file had.

use super::*;

/// Item slot data bridged from ECS Inventory for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiItemSlot {
    /// Item ID from items.csv.
    pub item_id: String,
    /// Human-readable item name (looked up from ItemRegistry).
    pub name: String,
    /// Quantity in this stack.
    pub quantity: u32,
    /// Uses worn off the top item (tools, 2026-09-26; 0 = unworn).
    pub wear: u32,
    /// Grade of a crafted durable good (0 = ungraded; crafting::quality).
    pub quality: u8,
}

/// Game time snapshot bridged from TimeSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiGameTime {
    /// The GLOBAL game clock: lon-0 mean solar time by construction
    /// (dev_travel::planet_spin_from_time ties the planet's spin to it).
    pub hour: f32,
    pub day_count: u32,
    pub season: String,
    pub is_daytime: bool,
    /// LOCAL mean solar time at the frame-locked surface site (global
    /// hour + lon/15), None when not standing on a planet. This is what
    /// the HUD clock shows: before it existed the HUD printed the global
    /// hour as if it were local, which at Silverdale (lon -122.7) put
    /// "20:04" beside a noon sun - the clock ran exactly lon/15 = 8.2 h
    /// ahead of the sky (the sun-clock incident, 2026-08-18).
    pub local_hour: Option<f32>,
}

/// Weather snapshot bridged from WeatherSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiWeather {
    pub condition: String,
    /// Weather intensity 0..1 (v0.1059). Needed by the aerial-haze fog ramp,
    /// which is the first thing in the engine to actually CONSUME the weather's
    /// severity rather than just its name.
    pub intensity: f32,
    pub temperature: f32,
    pub wind_speed: f32,
    /// Active extreme-weather event display name ("" = none, v0.1035):
    /// the HUD shows this instead of the plain condition while it runs.
    pub event: String,
    /// Hazard proximity warning ("" = none, v0.1038): set while the
    /// player is within 3x a Vortex event's hazard radius.
    pub warning: String,
}

/// Task priority levels for the task board.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Task status for kanban columns.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
}

/// A task for the GUI task board.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiTask {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub assignee: String,
    pub labels: Vec<String>,
}

/// A marketplace listing, mirroring the relay's ListingData shape (v0.752,
/// ladder rung 5: the native Market page finally speaks to the connected
/// relay instead of holding a page-local mock list).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiListing {
    /// Relay listing id (client-generated at create, e.g. "n18c2f-0001").
    pub id: String,
    pub title: String,
    pub description: String,
    /// Free-text price, matching web ("5 SOL", "20 CR negotiable", "").
    pub price: String,
    pub seller_key: String,
    pub seller_name: String,
    pub category: String,
    pub condition: String,
    pub payment_methods: String,
    pub location: String,
    pub status: String,
    pub created_at: String,
}

#[cfg(feature = "native")]
impl GuiListing {
    /// Map one relay `ListingData` JSON object (from listing_list /
    /// listing_new / listing_updated frames) into the GUI shape. Absent or
    /// null optional fields become empty strings.
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        };
        Self {
            id: s("id"),
            title: s("title"),
            description: s("description"),
            price: s("price"),
            seller_key: s("seller_key"),
            seller_name: s("seller_name"),
            category: s("category"),
            condition: s("condition"),
            payment_methods: s("payment_methods"),
            location: s("location"),
            status: s("status"),
            created_at: s("created_at"),
        }
    }
}

/// One review on a listing, mirroring the relay's ReviewData (v0.755).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiReview {
    pub id: i64,
    pub reviewer_key: String,
    pub reviewer_name: String,
    pub rating: i32,
    pub comment: String,
    pub created_at: String,
}

#[cfg(feature = "native")]
impl GuiReview {
    /// Map one ReviewData JSON object (REST reviews list, review_created
    /// frame). Null names become empty strings.
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        Self {
            id: v.get("id").and_then(|x| x.as_i64()).unwrap_or(0),
            reviewer_key: v.get("reviewer_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            reviewer_name: v.get("reviewer_name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            rating: v.get("rating").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            comment: v.get("comment").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            created_at: v.get("created_at").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        }
    }
}

// (GuiListingMsg removed 2026-08-23: listing buyer-seller threads were
// plaintext server-side; marketplace contact rides sealed-sender DMs now.)

/// The relay's aggregated civilization stats (GET /api/civilization, v0.757).
/// Flattened from the nested population/infrastructure/economy/resources/
/// social/activity JSON the relay serializes.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiCivStats {
    pub total_members: u32,
    pub online_now: u32,
    pub new_this_week: u32,
    pub channels: u32,
    pub voice_channels: u32,
    pub projects: u32,
    pub total_messages: u32,
    pub messages_today: u32,
    pub active_listings: u32,
    pub total_trades: u32,
    pub total_reviews: u32,
    pub total_tasks: u32,
    pub tasks_completed: u32,
    pub tasks_in_progress: u32,
    pub tasks_open: u32,
    pub total_follows: u32,
    pub total_dms: u32,
    pub most_active_channel: String,
    pub peak_online: u32,
}

#[cfg(feature = "native")]
impl GuiCivStats {
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        let n = |path: &[&str]| -> u32 {
            let mut cur = v;
            for k in path {
                match cur.get(k) {
                    Some(x) => cur = x,
                    None => return 0,
                }
            }
            cur.as_u64().unwrap_or(0) as u32
        };
        Self {
            total_members: n(&["population", "total_members"]),
            online_now: n(&["population", "online_now"]),
            new_this_week: n(&["population", "new_this_week"]),
            channels: n(&["infrastructure", "channels"]),
            voice_channels: n(&["infrastructure", "voice_channels"]),
            projects: n(&["infrastructure", "projects"]),
            total_messages: n(&["infrastructure", "total_messages"]),
            messages_today: n(&["infrastructure", "messages_today"]),
            active_listings: n(&["economy", "active_listings"]),
            total_trades: n(&["economy", "total_trades"]),
            total_reviews: n(&["economy", "total_reviews"]),
            total_tasks: n(&["resources", "total_tasks"]),
            tasks_completed: n(&["resources", "tasks_completed"]),
            tasks_in_progress: n(&["resources", "tasks_in_progress"]),
            tasks_open: n(&["resources", "tasks_open"]),
            total_follows: n(&["social", "total_follows"]),
            total_dms: n(&["social", "total_dms"]),
            most_active_channel: v
                .get("activity")
                .and_then(|a| a.get("most_active_channel"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            peak_online: n(&["activity", "peak_online"]),
        }
    }
}

/// One item on a side of a P2P trade, mirroring the relay's TradeItem (v0.756).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiTradeItem {
    pub item_type: String,
    pub name: String,
    pub quantity: u32,
    pub description: String,
}

/// One P2P trade, mirroring the relay's TradeDataPayload (v0.756). Delivered
/// through targeted `__trade_data__:` / `__trade_list__:` private wrappers.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiTrade {
    pub id: String,
    pub initiator_key: String,
    pub recipient_key: String,
    /// pending | active | completed | cancelled | rejected (relay strings).
    pub status: String,
    pub initiator_items: Vec<GuiTradeItem>,
    pub recipient_items: Vec<GuiTradeItem>,
    pub initiator_confirmed: bool,
    pub recipient_confirmed: bool,
    pub created_at: i64,
    pub message: String,
}

#[cfg(feature = "native")]
impl GuiTrade {
    /// Map one TradeDataPayload JSON object (the `trade` field of a
    /// `__trade_data__:` wrapper, or a `trades` element of `__trade_list__:`).
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        let items = |k: &str| -> Vec<GuiTradeItem> {
            v.get(k)
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .map(|i| GuiTradeItem {
                            item_type: i.get("item_type").and_then(|x| x.as_str()).unwrap_or("item").to_string(),
                            name: i.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            quantity: i.get("quantity").and_then(|x| x.as_u64()).unwrap_or(1) as u32,
                            description: i.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            id: v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            initiator_key: v.get("initiator_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            recipient_key: v.get("recipient_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            status: v.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            initiator_items: items("initiator_items"),
            recipient_items: items("recipient_items"),
            initiator_confirmed: v.get("initiator_confirmed").and_then(|x| x.as_bool()).unwrap_or(false),
            recipient_confirmed: v.get("recipient_confirmed").and_then(|x| x.as_bool()).unwrap_or(false),
            created_at: v.get("created_at").and_then(|x| x.as_i64()).unwrap_or(0),
            message: v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        }
    }
}

#[cfg(all(test, feature = "native"))]
mod listing_mapping_tests {
    use super::GuiListing;

    /// The exact frame shape the relay's listing_list / listing_new carry
    /// (ListingData serialized by src/relay/relay.rs) maps losslessly, and
    /// null optionals (a listing with no description/price) become empty
    /// strings instead of panicking.
    #[test]
    fn relay_listing_json_maps_and_tolerates_nulls() {
        let full: serde_json::Value = serde_json::from_str(
            r#"{
                "id": "n18c2f-001", "seller_key": "abc123def456", "seller_name": "Ada",
                "title": "Heirloom seeds", "description": "Greens + herbs",
                "category": "Seeds", "condition": "new", "price": "8 SOL",
                "payment_methods": "SOL, barter", "location": "Sol station",
                "status": "active", "created_at": "2026-07-07 12:00:00",
                "updated_at": null, "images": null
            }"#,
        )
        .unwrap();
        let l = GuiListing::from_relay_json(&full);
        assert_eq!(l.id, "n18c2f-001");
        assert_eq!(l.title, "Heirloom seeds");
        assert_eq!(l.price, "8 SOL");
        assert_eq!(l.seller_name, "Ada");
        assert_eq!(l.status, "active");

        let sparse: serde_json::Value = serde_json::from_str(
            r#"{"id": "x", "seller_key": "k", "seller_name": null, "title": "Bare",
                "description": null, "category": "Other", "condition": null,
                "price": null, "payment_methods": null, "location": null,
                "status": "active", "created_at": null}"#,
        )
        .unwrap();
        let l = GuiListing::from_relay_json(&sparse);
        assert_eq!(l.title, "Bare");
        assert_eq!(l.description, "");
        assert_eq!(l.price, "");
        assert_eq!(l.seller_name, "");
    }

    /// WIRE-CONTRACT pin: serialize the relay's REAL ListingList frame (the
    /// exact bytes the server sends) and run it through the same JSON path
    /// the native WS dispatch uses. If either side renames a serde field,
    /// this breaks here instead of as a silently-empty Market page.
    #[test]
    fn relay_listing_list_frame_round_trips_to_gui() {
        let frame = crate::relay::relay::RelayMessage::ListingList {
            target: Some("somekey".into()),
            listings: vec![crate::relay::relay::ListingData {
                id: "wire-1".into(),
                seller_key: "sellerkey".into(),
                seller_name: Some("Ada".into()),
                title: "Wire test".into(),
                description: Some("desc".into()),
                category: "Tools".into(),
                condition: None,
                price: Some("3 SOL".into()),
                payment_methods: None,
                location: None,
                images: None,
                status: "active".into(),
                created_at: Some("2026-07-07".into()),
                updated_at: None,
            }],
        };
        let wire = serde_json::to_string(&frame).unwrap();
        let val: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(
            val.get("type").and_then(|t| t.as_str()),
            Some("listing_list"),
            "frame type tag"
        );
        let arr = val.get("listings").and_then(|v| v.as_array()).unwrap();
        let l = GuiListing::from_relay_json(&arr[0]);
        assert_eq!(l.id, "wire-1");
        assert_eq!(l.title, "Wire test");
        assert_eq!(l.price, "3 SOL");
        assert_eq!(l.seller_name, "Ada");
        assert_eq!(l.condition, "");
    }

    /// Wire pin for the v0.755 review frame: the relay's REAL ReviewCreated
    /// frame maps through the native dispatch path (a serde rename would
    /// silently empty the reviews card). The listing-thread half of this
    /// test died with the plaintext thread protocol (2026-08-23).
    #[test]
    fn relay_review_frame_round_trips_to_gui() {
        use super::GuiReview;

        let review = crate::relay::relay::RelayMessage::ReviewCreated {
            review: crate::relay::relay::ReviewData {
                id: 3,
                listing_id: "wire-1".into(),
                reviewer_key: "buyerkey".into(),
                reviewer_name: None,
                rating: 4,
                comment: "Solid tower.".into(),
                created_at: "2026-07-08".into(),
            },
        };
        let val: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&review).unwrap()).unwrap();
        assert_eq!(val.get("type").and_then(|t| t.as_str()), Some("review_created"));
        let r = GuiReview::from_relay_json(val.get("review").unwrap());
        assert_eq!(r.id, 3);
        assert_eq!(r.rating, 4);
        assert_eq!(r.comment, "Solid tower.");
        assert_eq!(r.reviewer_name, "", "null name maps to empty");
        assert_eq!(
            val.get("review").and_then(|v| v.get("listing_id")).and_then(|v| v.as_str()),
            Some("wire-1"),
            "the dispatch arm filters on review.listing_id"
        );
    }

    /// Wire pin for the v0.756 trade flow: the relay's REAL private-wrapped
    /// `__trade_data__:` delivery (a serialized TradeData frame behind the
    /// prefix) maps through the native routing path exactly as lib.rs's
    /// private-message arm slices it.
    #[test]
    fn relay_trade_wrapper_round_trips_to_gui() {
        use super::GuiTrade;

        let frame = crate::relay::relay::RelayMessage::TradeData {
            trade: crate::relay::relay::TradeDataPayload {
                id: "trade-9".into(),
                initiator_key: "alicekey".into(),
                recipient_key: "bobkey".into(),
                status: "active".into(),
                initiator_items: vec![crate::relay::relay::TradeItem {
                    item_type: "item".into(),
                    name: "Iron Ingot".into(),
                    quantity: 10,
                    description: String::new(),
                    reference_id: None,
                }],
                recipient_items: vec![],
                initiator_confirmed: true,
                recipient_confirmed: false,
                created_at: 1_720_000_000_000,
                completed_at: None,
                message: Some("swap for wheat?".into()),
            },
        };
        // The relay wraps the serialized frame behind the private prefix.
        let wire = format!("__trade_data__:{}", serde_json::to_string(&frame).unwrap());

        // ... and the native private-message arm slices + parses it.
        let payload = wire.strip_prefix("__trade_data__:").unwrap();
        let val: serde_json::Value = serde_json::from_str(payload).unwrap();
        let t = GuiTrade::from_relay_json(val.get("trade").unwrap());
        assert_eq!(t.id, "trade-9");
        assert_eq!(t.status, "active");
        assert_eq!(t.initiator_items.len(), 1);
        assert_eq!(t.initiator_items[0].name, "Iron Ingot");
        assert_eq!(t.initiator_items[0].quantity, 10);
        assert!(t.initiator_confirmed);
        assert!(!t.recipient_confirmed);
        assert_eq!(t.message, "swap for wheat?");
    }

    /// Wire pin for the v0.757 guild list: the exact object shape
    /// GET /api/guilds serializes (src/relay/api.rs get_guilds) maps into
    /// the GUI row, hex colours parse, junk colours fall back.
    #[test]
    fn relay_guild_json_maps_with_hex_colors() {
        use super::{parse_hex_color, GuiGuild};

        let v: serde_json::Value = serde_json::from_str(
            r##"{"id": "g-1", "name": "Builders", "description": "We build.",
                "owner_key": "ownerkey", "icon": "", "color": "#2e86c1",
                "created_at": "2026-07-08", "member_count": 4}"##,
        )
        .unwrap();
        let g = GuiGuild::from_relay_json(&v);
        assert_eq!(g.id, "g-1");
        assert_eq!(g.name, "Builders");
        assert_eq!(g.owner_key, "ownerkey");
        assert_eq!(g.member_count, 4);
        assert_eq!(g.color, egui::Color32::from_rgb(0x2e, 0x86, 0xc1)); // theme-exempt: test fixture.
        assert!(!g.is_member, "membership is merged separately");

        assert_eq!(parse_hex_color("nonsense"), None);
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(
            parse_hex_color("#ff0080"),
            Some(egui::Color32::from_rgb(255, 0, 128)) // theme-exempt: test fixture.
        );
    }

    /// Wire pin for the v0.757 civilization stats: the REAL nested shape the
    /// relay serializes (CivilizationStats in storage/civilization.rs)
    /// flattens into the dashboard row; missing sections read as zero.
    #[test]
    fn relay_civilization_stats_flatten_to_gui() {
        use super::GuiCivStats;

        let v: serde_json::Value = serde_json::from_str(
            r#"{
                "population": {"total_members": 12, "online_now": 3, "new_this_week": 2, "roles": {"member": 11, "admin": 1}},
                "infrastructure": {"channels": 6, "voice_channels": 2, "projects": 4, "total_messages": 2375, "messages_today": 15},
                "economy": {"active_listings": 5, "total_trades": 1, "total_reviews": 2},
                "resources": {"total_tasks": 30, "tasks_completed": 21, "tasks_in_progress": 4, "tasks_open": 5},
                "social": {"total_follows": 8, "total_dms": 9},
                "activity": {"most_active_channel": "general", "messages_today": 15, "peak_online": 4}
            }"#,
        )
        .unwrap();
        let s = GuiCivStats::from_relay_json(&v);
        assert_eq!(s.total_members, 12);
        assert_eq!(s.online_now, 3);
        assert_eq!(s.new_this_week, 2);
        assert_eq!(s.total_messages, 2375);
        assert_eq!(s.active_listings, 5);
        assert_eq!(s.tasks_completed, 21);
        assert_eq!(s.total_follows, 8);
        assert_eq!(s.most_active_channel, "general");
        assert_eq!(s.peak_online, 4);

        // A partial payload (older server) degrades to zeros, not a panic.
        let sparse: serde_json::Value =
            serde_json::from_str(r#"{"population": {"total_members": 1}}"#).unwrap();
        let s = GuiCivStats::from_relay_json(&sparse);
        assert_eq!(s.total_members, 1);
        assert_eq!(s.channels, 0);
        assert_eq!(s.most_active_channel, "");
    }
}

/// One castable (or honestly-locked) ability row for the Profile page's
/// Abilities panel (v0.753, ladder rung 8). Bridged from the AbilityRegistry
/// + PlayerSkills each frame while the page is open.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiAbility {
    pub id: String,
    pub name: String,
    pub school: String,
    pub flavor: String,
    /// Energy the cast costs (mana + stamina columns combined).
    pub cost: f32,
    pub cooldown_s: f32,
    /// Seconds until castable again; 0 = ready.
    pub cooldown_remaining: f32,
    pub heals: f32,
    /// True when the v1 pipeline can cast it right now (self-scoped effect
    /// + skill gate met). Cooldown is shown separately.
    pub castable_now: bool,
    /// Honest reason a row is locked ("" when castable).
    pub locked_reason: String,
    pub description: String,
}

/// Planet data for the map viewer.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiPlanet {
    pub name: String,
    pub planet_type: String,
    pub radius_km: f64,
    pub gravity: f64,
    pub atmosphere: String,
    pub moons: u32,
    pub orbit_radius_au: f64,
}

/// A calendar event.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiCalendarEvent {
    pub title: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub time: String,
    pub color: egui::Color32,
}

/// A note entry.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiNote {
    pub id: u64,
    pub title: String,
    pub content: String,
    /// Unix timestamp of last modification.
    pub modified: u64,
}

/// Wallet network selector.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletNetwork {
    Mainnet,
    Devnet,
    Testnet,
}

#[cfg(feature = "native")]
impl WalletNetwork {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mainnet => "Mainnet",
            Self::Devnet => "Devnet",
            Self::Testnet => "Testnet",
        }
    }
}

/// A wallet transaction entry.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct WalletTransaction {
    pub signature: String,
    pub direction: String,
    pub amount: f64,
    pub counterparty: String,
    pub timestamp: String,
}

/// A crafting recipe for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiRecipe {
    pub id: String,
    pub name: String,
    pub category: String,
    pub inputs: Vec<(String, u32)>,
    pub outputs: Vec<(String, u32)>,
    pub craft_time_sec: f32,
    pub station_required: String,
    /// Canonical skill id required to craft (None = none), for the #8b tech-unlock
    /// gate display. The player's level comes from `GuiState::skills`.
    pub skill_required: Option<String>,
    /// Minimum level of `skill_required` (0 = none).
    pub skill_level: u32,
    pub description: String,
    /// Hand tools the craft needs in the backpack (data/crafting/tools.ron).
    pub tools: Vec<String>,
    /// Makes a durable good, so a hand craft grades it (2026-09-26).
    pub graded: bool,
}

/// One vendor-tradeable good for GUI display (v0.747, ladder rung 3).
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiTradeGood {
    pub id: String,
    pub name: String,
    pub category: String,
    /// What the PLAYER PAYS to buy one (vendor sell price, 1.25x base).
    pub buy_price: i64,
    /// What the PLAYER RECEIVES selling one (vendor buy price, 0.5x base).
    pub sell_price: i64,
}

/// A buildable structure blueprint for GUI display (v0.746, ladder rung 2):
/// the Crafting page's Structures section renders these; Build sends the id
/// through pending_build -> the "build_request" channel -> ConstructionSystem.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiBlueprint {
    pub id: String,
    pub name: String,
    pub category: String,
    pub materials: Vec<(String, u32)>,
    pub build_time: f32,
    /// Capability the finished structure provides (e.g. "smelting"), or empty.
    pub provides: String,
    /// Machine types it serves as once built (e.g. "smelter"), for the
    /// station gate and the "or build a Furnace" hint.
    pub stations: Vec<String>,
}

/// Player survival vitals for GUI display (synced from the ECS each frame).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiVitals {
    pub satiation: f32,
    pub hydration: f32,
    pub energy: f32,
    pub oxygen: f32,
    pub body_temp_c: f32,
    pub waste: f32,
    pub satiation_max: f32,
    pub hydration_max: f32,
    pub energy_max: f32,
    pub oxygen_max: f32,
    pub waste_max: f32,
    /// True if the player is in a sealed/oxygenated space (else exposed/vacuum).
    pub sealed: bool,
    /// Active status effects: (display name, seconds remaining).
    pub effects: Vec<(String, f32)>,
}

/// The pests in one grow area, for the Garden panel (2026-09-26,
/// systems::farming::pests): each pest with its level and its controls in
/// the IPM order (gentlest first), and the releases still working there.
#[derive(Debug, Clone, Default)]
pub struct GuiAreaPests {
    /// The grow area's id (a crop's `tower_id`; "" for hand-planted crops).
    pub area: String,
    /// (pest name, level 0..1, controls as (id, name, note)).
    pub pests: Vec<(String, f32, Vec<(String, String, String)>)>,
    /// (release name, garden days left).
    pub releases: Vec<(String, f32)>,
}

/// The Garden panel's pests and soil pH, and what the player just chose.
#[derive(Debug, Clone, Default)]
pub struct GardenPests {
    pub areas: Vec<GuiAreaPests>,
    /// (area, control id), carried to the farming system next frame.
    pub pending: Option<(String, String)>,
    /// Soil pH (farming::soil_ph): Settings "Soil pH: Off" (saved as
    /// AppConfig::soil_ph), the amendments as (id, label), and the one the
    /// player chose as (area, amendment id), carried next frame.
    pub soil_ph_off: bool,
    pub ph_amendments: Vec<(String, String)>,
    pub ph_pending: Option<(String, String)>,
}

/// A growing crop for GUI display (synced from the ECS each frame).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiCrop {
    /// hecs entity bits — used to target water/harvest commands.
    pub entity_bits: u64,
    pub name: String,
    pub stage: String,
    /// Growth progress 0..1 (by stage index).
    pub progress: f32,
    pub water: f32,
    pub health: f32,
    /// Average health over the growing season, 0..1 (2026-09-26): what sets
    /// the yield (farming::season_health). Current health recovers fast.
    pub season_health: f32,
    pub mature: bool,
    pub dead: bool,
    /// The tower this crop belongs to (its config id), if planted via a tower.
    pub tower_id: Option<String>,
    /// Which slot of the tower this crop occupies (0-based), for the slot view.
    pub tower_slot: Option<u32>,
    /// Plants in this crop's unit: 1 in a tower cup, as many as fit at the
    /// crop's spacing in a bed plot (farming::units, 2026-09-26).
    pub plants: u32,
    /// Plant-def reference data (from plants.csv) for the crop card: grams of
    /// N, P2O5 and K2O each kg of harvest carries out of the soil (the crop's
    /// cited removal columns, else its index read against the anchor;
    /// farming::soil::removal_per_kg), the UNIT's daily water need (L, per
    /// plant x plants), and the tolerated temperature window (Celsius). 0 when
    /// the species is unknown.
    pub n: f32,
    pub p: f32,
    pub k: f32,
    pub water_per_day: f32,
    pub temp_min: f32,
    pub temp_max: f32,
    /// What is lighting it now (farming::lighting::light_word).
    pub light: String,
    /// Grams of N, P2O5 and K2O the crop's unit holds, and what its season
    /// needs (2026-09-26, farming::soil).
    pub soil: [f32; 3],
    pub need: [f32; 3],
    /// The scarcest nutrient, when the crop is short of one.
    pub short_of: Option<String>,
    /// Its unit's pH (None: pH off or not modelled), its plants.csv window,
    /// the health cap pH sets, and whether a tower holds it (farming::soil_ph).
    pub ph: Option<f32>,
    pub ph_window: [f32; 2],
    pub ph_cap: f32,
    pub ph_held: bool,
}

/// An asteroid (with remaining ore) for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiAsteroid {
    /// Stable id used to target this asteroid for a mining run.
    pub id: String,
    pub name: String,
    pub classification: String,
    /// Remaining ore by item id.
    pub ores: Vec<(String, f32)>,
    /// World position (km) + straight-line distance from home, for the map + UI.
    pub position: [f32; 3],
    pub distance: f32,
}

/// One world vehicle for the Inventory page's Vehicles section (Stage 3, v0.680):
/// what it is, where it stands, and whether it is currently driving itself.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiVehicle {
    /// Entity bits — the Summon action's handle back into the ECS.
    pub bits: u64,
    pub name: String,
    /// Straight-line distance from the player (camera), meters.
    pub distance: f32,
    /// True while a VehicleRoute is attached (driving itself somewhere).
    pub in_transit: bool,
}

/// An active mining drone for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiDrone {
    /// The fetch order — `(ore_id, units)` — shown as the drone's manifest.
    pub manifest: Vec<(String, u32)>,
    pub phase: String,
    /// Total units currently in the hold.
    pub cargo_total: u32,
    /// Progress 0..1 through the current mission phase (for the panel's bar).
    pub phase_progress: f32,
    /// Target asteroid id, distance, and the drone's current world position (for the
    /// map dot + "mining X, N km away" readout).
    pub target: String,
    pub distance: f32,
    pub pos: [f32; 3],
}

/// A player skill (live level + XP) for GUI display, synced from the ECS
/// PlayerSkills component each frame. `xp_needed` is the XP to reach the next
/// level (per the skill's curve); the bar fills `xp / xp_needed`.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiSkill {
    pub id: String,
    pub name: String,
    pub category: String,
    pub level: u32,
    pub xp: u32,
    pub xp_needed: u32,
    /// The skill's top level (for the quality a level gives, 2026-09-26).
    pub max_level: u32,
}

/// A player quest for GUI display, synced from the ECS QuestTracker each frame.
/// Active quests carry their current step (index/total + description); completed
/// quests have `completed = true` and `step_total = 0`.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiQuest {
    pub name: String,
    pub step_index: usize,
    pub step_total: usize,
    pub step_desc: String,
    pub completed: bool,
}

/// A quest the player COULD accept (v0.747.x, ladder rung 4): prerequisite
/// satisfied, not active, not completed. The Quests page's Available section.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiAvailableQuest {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// A guild for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiGuild {
    /// Relay guild id (uuid).
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_key: String,
    /// Parsed from the relay's hex colour string (user-chosen guild colour).
    pub color: egui::Color32,
    pub member_count: i64,
    /// True when MY key appears in this guild's membership (merged from the
    /// `?user=` fetch).
    pub is_member: bool,
    pub created_at: String,
}

#[cfg(feature = "native")]
impl GuiGuild {
    /// Map one guild object from GET /api/guilds (v0.757).
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Self {
            id: s("id"),
            name: s("name"),
            description: s("description"),
            owner_key: s("owner_key"),
            color: parse_hex_color(v.get("color").and_then(|x| x.as_str()).unwrap_or(""))
                .unwrap_or(egui::Color32::from_rgb(68, 136, 255)), // theme-exempt: the fallback guild colour is user data, not a UI token.
            member_count: v.get("member_count").and_then(|x| x.as_i64()).unwrap_or(0),
            is_member: false,
            created_at: s("created_at"),
        }
    }
}

/// Parse a `#rrggbb` hex colour (the relay's guild colour format).
#[cfg(feature = "native")]
pub fn parse_hex_color(s: &str) -> Option<egui::Color32> {
    let hex = s.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b)) // theme-exempt: user-chosen guild colour from data.
}

/// A chat message received from or sent to the relay server.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct ChatMessage {
    pub sender_name: String,
    pub sender_key: String,
    pub content: String,
    /// Display-formatted timestamp string (e.g. "12:34:56").
    pub timestamp: String,
    /// Original numeric timestamp (ms since epoch). Used for reaction targeting
    /// (Reaction message references messages by sender_key + timestamp_ms).
    pub timestamp_ms: u64,
    pub channel: String,
    /// Reactions: emoji → list of sender public keys who reacted.
    /// Count = `reactions[emoji].len()`. Stored as Vec rather than count so
    /// we can prevent duplicate reactions per user and toggle.
    pub reactions: std::collections::HashMap<String, Vec<String>>,
    /// If this message is a reply, the parent message context.
    pub reply_to: Option<ReplyContext>,
    /// Normalized URL of the connection of OURS that delivered this message.
    /// Empty = legacy/local (scratchpad, pre-provenance rows). This is MY
    /// server, not where the author lives: a federated line delivered by
    /// united-humanity.us has server = united-humanity.us.
    pub server: String,
    /// Federation origin server id (the origin relay's public key hex) when
    /// this line was bridged from another server; empty for a message native
    /// to `server`. Dedup key across carriers is (origin_server, sender_key,
    /// timestamp_ms) per docs/design/federation-ux.md.
    pub origin_server: String,
}

/// Cached parent-message context for a thread reply.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ReplyContext {
    pub sender_key: String,
    pub sender_name: String,
    /// Preview snippet of the parent message (truncated to ~100 chars by render).
    pub preview: String,
    pub timestamp_ms: u64,
}

/// One row in the search results list.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatSearchResult {
    pub channel: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

/// One pinned message in a channel. Mirrors the relay's PinData type
/// so the WS handler can decode without a separate adapter.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatPin {
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

/// A user visible in the chat user list.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatUser {
    pub name: String,
    pub public_key: String,
    pub role: String,
    pub status: String,
}

/// A channel in the channel list.
// NOTE: PendingUnencryptedDm (the v0.199 "send unencrypted anyway"
// confirmation modal) was DELETED in the sealed-sender cutover
// (2026-08-23). The v2 DM protocol has no plaintext field at all — a DM
// that can't be sealed simply can't be sent, and the composer shows why.
// This is strictly stronger than the modal: there is no user-consented
// downgrade path left to attack.

#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct ChatChannel {
    /// Guaranteed local-only room (v0.1132): refuses federation while the
    /// server's guarantee is on; the editor locks its Federated toggle.
    pub local_only: bool,
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// Whether voice is currently active/joined for this channel.
    pub voice_joined: bool,
    /// Whether voice is enabled for this channel (shows mic icon).
    pub voice_enabled: bool,
    /// Whether the channel is read-only for non-admins. Settable from the
    /// Server Settings → Channels page and the chat channel-edit modal;
    /// persisted by the relay's admin-gated `channel_update` handler.
    pub read_only: bool,
    /// Whether the channel federates to peer servers. Settable from
    /// Server Settings → Channels; persisted via `channel_update`.
    pub federated: bool,
    /// Live voice roster for this channel: (public_key, display_name), populated
    /// from the relay's voice_channel_list broadcast (v0.481). Empty when no one
    /// is connected to voice here. Not persisted; refreshed on every broadcast.
    pub voice_participants: Vec<(String, String)>,
    /// A message arrived here while another channel was open — drives the
    /// sidebar unread dot, same pattern as ChatDm/ChatGroup. (v0.718)
    pub unread: bool,
}

/// In-flight edit state for one row of the Server Settings → Channels
/// spreadsheet (v0.188.0). Cloned from the live channel into the draft
/// when the row is opened, written back via slash command on Save.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct ChannelDraft {
    pub name: String,
    pub description: String,
    pub read_only: bool,
    pub federated: bool,
    pub voice_enabled: bool,
}

/// View mode of the in-world chat panel (unified-chat increment 1c). The
/// Enter-opened panel over the 3D world can show public channels (the v0.772
/// behavior), DM conversations, group chats, or a tiny options slice --
/// selected by the compact tab row at the top of the panel. Session-only on
/// purpose (a GuiState field, not AppConfig): the panel reopens in the last
/// mode used this session, and a fresh launch always starts on the public
/// Channels view so private text is never the first thing on screen.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngameChatMode {
    /// Public server channels + the channel switcher (today's behavior).
    Channels,
    /// DM conversations -- same store + send path as the Chat page.
    Dms,
    /// Group chats (P2P signed-object groups + legacy relay groups).
    Groups,
    /// Tiny settings slice: feed visibility, panel height, open full Chat page.
    Options,
}

#[cfg(feature = "native")]
impl IngameChatMode {
    /// Tab order as rendered left-to-right in the panel's mode row.
    pub const ALL: [IngameChatMode; 4] = [
        IngameChatMode::Channels,
        IngameChatMode::Dms,
        IngameChatMode::Groups,
        IngameChatMode::Options,
    ];

    /// Compact tab label. Options is "..." on purpose: it reads as "more"
    /// and keeps the row narrow (the panel is only ~470 px wide).
    pub fn label(self) -> &'static str {
        match self {
            IngameChatMode::Channels => "Chat",
            IngameChatMode::Dms => "DMs",
            IngameChatMode::Groups => "Groups",
            IngameChatMode::Options => "...",
        }
    }

    /// The next mode in tab order, wrapping at the end. Pure cycling helper
    /// so a future hotkey (and the unit tests) walk the modes through ONE
    /// definition of the order instead of duplicating `ALL`.
    pub fn next(self) -> IngameChatMode {
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// A DM conversation entry for the left panel.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatDm {
    pub user_name: String,
    pub user_key: String,
    pub last_message: String,
    pub timestamp: String,
    pub unread: bool,
}

// (ChatGroup removed 2026-08-23 with the legacy plaintext group system;
// P2P E2EE groups render from p2p_groups: Vec<P2pGroupInfo>.)

/// A server entry for the left panel (each server has text + voice channels).
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatServer {
    pub name: String,
    pub channels: Vec<ChatChannel>,
    pub voice_channels: Vec<String>,
    /// Stable id (typically `srv_<url>`) for nav highlighting + dedupe.
    /// (v0.187.0)
    pub id: String,
    /// Relay URL for this server (e.g. https://example.com). When the
    /// user clicks this server's row in the chat sidebar, the client
    /// reconnects to this URL. (v0.187.0)
    pub url: String,
    /// True iff the websocket to this server is currently open.
    /// (v0.187.0)
    pub connected: bool,
}

/// A parked (background) relay connection: its live socket plus every piece
/// of per-server chat state, preserved intact while another server is active.
///
/// Multi-connection model (docs/design/federation-ux.md, build order 2): the
/// legacy `ws_*` / `chat_*` fields on GuiState always describe the ACTIVE
/// connection -- the whole message router and every UI site keep reading and
/// writing them untouched. Switching servers PARKS the active state here
/// (socket included, still connected) and unparks the target, so a switch is
/// a pointer swap instead of a teardown: nothing disconnects, nothing is
/// cleared, and switching back restores messages, rosters, and even the room
/// you had open. While parked, the socket's reader thread keeps the link
/// alive (transport-level ping/pong) and incoming messages queue in the
/// WsClient channel; the FULL router drains that backlog on unpark, so a
/// parked server catches up through the same code path as live traffic.
/// (A per-frame background pump for unread badges + parked reconnect is the
/// next stage, alongside connect-to-all.)
#[cfg(feature = "native")]
#[derive(Default)]
pub struct ServerConnection {
    /// Normalized URL (pages::chat::norm_server_url) -- the identity key.
    pub url: String,
    /// The URL exactly as the user saved it (what server_url is set to on unpark).
    pub display_url: String,
    pub ws: Option<crate::net::ws_client::WsClient>,
    pub status: String,
    pub identified: bool,
    pub manually_disconnected: bool,
    pub reconnect_timer: f32,
    pub reconnect_delay: f32,
    pub reconnect_attempts: u32,
    pub rate_limited: bool,
    pub msgs_in: u64,
    pub history_fetched: bool,
    /// Federated channel ids still awaiting a REST history fetch (armed once
    /// when the channel list first arrives; drained one channel at a time by
    /// the background pump so Commons rooms have depth from EVERY carrier,
    /// not just servers the user has visited).
    pub history_queue: Vec<String>,
    /// In-flight background history fetch: (channel, body-or-error).
    pub history_rx: Option<std::sync::mpsc::Receiver<(String, Result<String, String>)>>,
    pub messages: Vec<ChatMessage>,
    pub channels: Vec<ChatChannel>,
    /// The room the user had open on this server; restored on switch-back.
    pub active_channel: String,
    pub users: Vec<ChatUser>,
    pub dms: Vec<ChatDm>,
    pub pins: std::collections::HashMap<String, Vec<ChatPin>>,
    pub friends: Vec<ChatUser>,
    pub following_keys: std::collections::HashSet<String>,
    pub followers: std::collections::HashSet<String>,
    pub sent_timestamps: Vec<u64>,
}

/// One snapshot of the connected server's health, from its public /health +
/// /api/stats endpoints (Server Settings → System health, v0.720). Read-only
/// in-app ops: replaces SSHing the VPS just to ask "is it up, what's running".
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct SystemHealth {
    /// "ok" from /health, or whatever the server reported.
    pub status: String,
    /// Deployed build: "<git short sha>-<epoch>" from /api/stats `version`.
    pub version: String,
    /// Relay process uptime in seconds (from /health).
    pub uptime_seconds: u64,
    pub total_messages: u64,
    pub connected_peers: u64,
}

/// Rich admin health snapshot from the SIGNED `GET /api/admin/stats` — the data
/// an operator currently SSHes for (disk, watchdog state, backup age). Only
/// available when the operator holds the admin role on the target relay; the
/// Relay Control Center's Health tab fetches it (v0.846). All fields are
/// best-effort: the relay returns null for any probe that fails, so an
/// `Option::None` here just means "the relay couldn't measure it".
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct RelayAdminStats {
    pub user_count: u64,
    pub online_count: u64,
    pub total_messages: u64,
    pub message_count_24h: u64,
    pub db_size_bytes: u64,
    pub upload_size_bytes: u64,
    pub uptime_seconds: u64,
    pub version: String,
    /// Watchdog last-known state: "up" / "suspect" / "healing" / "down-critical"
    /// / "unknown". Read by the relay from /run/humanity-relay-watchdog.state.
    pub watchdog_state: String,
    /// Disk usage of the relay's filesystem, when the relay could probe it.
    pub disk_used_pct: Option<u32>,
    pub disk_total_bytes: Option<u64>,
    pub disk_avail_bytes: Option<u64>,
    /// Newest DB backup age in seconds + total backup count, when present.
    pub backup_age_secs: Option<u64>,
    pub backup_count: Option<u64>,
}

/// One federated-server row from GET /api/federation/servers (Server
/// Settings → Federation panel, v0.722 — federation-activation Phase 1 UI).
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct FederationServerRow {
    pub server_id: String,
    pub name: String,
    pub url: String,
    /// 0 = untrusted .. 3 = fully trusted (relay's trust tiers).
    pub trust_tier: i32,
    pub status: String,
    pub accord_compliant: bool,
}

/// Studio source type variants.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub enum StudioSourceType {
    Camera(u32),
    Screen(u32),
    Microphone(u32),
    ChatOverlay,
    Image(String),
    Text(String),
    Timer,
}

/// A source in the broadcasting studio.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct StudioSource {
    pub name: String,
    pub source_type: StudioSourceType,
    pub visible: bool,
    /// Normalized position (0.0..1.0) within the preview area.
    pub position: (f32, f32),
    /// Normalized size (0.0..1.0) within the preview area.
    pub size: (f32, f32),
    pub opacity: f32,
    pub z_order: u32,
}

/// A scene preset storing which sources are active.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct StudioScene {
    pub name: String,
    pub is_default: bool,
    /// Per-source visibility override (indexed same as StudioState.sources).
    pub source_visibility: Vec<bool>,
}

/// Which pane the Studio center canvas shows when the window is too narrow for
/// the side-by-side Program/Preview split.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioPane {
    Program,
    Preview,
}

/// All state for the broadcasting studio page.
///
/// Program/Preview split (OBS-style, v0.664): `program_scene_index` is the scene
/// that is LIVE (what WOULD be broadcast once real transport exists) and
/// `program_sources` is a frozen copy of the source arrangement taken at the last
/// Cut. `preview_scene_index` + the working `sources` list are the STAGED side:
/// clicking a scene loads it into preview, and all source editing (position, size,
/// visibility, add/remove) operates on preview, so the live layout stays put until
/// `cut_to_program()` deliberately pushes preview to program.
#[cfg(feature = "native")]
pub struct StudioState {
    pub scenes: Vec<StudioScene>,
    /// Scene that is live in PROGRAM (what would be broadcast right now).
    pub program_scene_index: usize,
    /// Scene staged in PREVIEW (what you are editing; viewers would not see it).
    pub preview_scene_index: usize,
    /// Frozen snapshot of `sources` taken at the last cut; the Program pane renders
    /// this so preview edits cannot disturb the live layout.
    pub program_sources: Vec<StudioSource>,
    /// The PREVIEW working source set (edited by the Sources panel + properties).
    pub sources: Vec<StudioSource>,
    /// Which pane the single canvas shows in the narrow-window fallback layout.
    pub focused_pane: StudioPane,
    pub selected_source_index: Option<usize>,
    pub is_live: bool,
    pub is_paused: bool,
    pub is_afk: bool,
    pub afk_start_time: f64,
    pub live_start_time: f64,
    pub stream_platform: String,
    pub stream_key: String,
    pub stream_server_url: String,
    pub stream_resolution: String,
    pub stream_bitrate: u32,
    pub stream_fps: u32,
    pub chat_overlay_channel: String,
    pub chat_overlay_font_size: f32,
    pub chat_overlay_position: String,
    pub chat_overlay_opacity: f32,
    pub chat_overlay_max_messages: u32,
    pub chat_overlay_bg_opacity: f32,

    // ── Real broadcast (v0.853). The fields above drive the local rehearsal; these
    // drive an actual stream leaving the machine. The GUI cannot own the publisher
    // itself (it lives next to the renderer, on the engine side), so the page sets a
    // REQUEST here and the engine loop acts on it, then mirrors the live counters
    // back for the page to draw. See `docs/design/streaming.md`.
    /// Some(true) = start broadcasting, Some(false) = stop. Taken by the engine loop.
    pub broadcast_request: Option<bool>,
    /// True once the relay has accepted the stream. Distinct from `is_live`, which is
    /// only the rehearsal flag: this one means bytes are actually going out.
    pub broadcast_live: bool,
    /// Mirrored from the publisher each frame so the page draws honest numbers.
    pub broadcast_viewers: u32,
    pub broadcast_kbps: u32,
    pub broadcast_frames: u64,
    pub broadcast_dropped: u64,
    /// Empty unless the publisher failed. Shown verbatim: a stream that silently
    /// stops must never look like a stream that is running.
    pub broadcast_error: String,
    /// Public watch URL, filled in once the relay accepts the stream.
    pub broadcast_url: String,
}

#[cfg(feature = "native")]
impl Default for StudioState {
    fn default() -> Self {
        // Scenes and sources are populated at startup from data/studio/{scenes,sources}.json
        // by `apply_studio_presets` in lib.rs. Default starts empty — if the data files are
        // missing, the studio page renders a blank scene list rather than crashing.
        Self {
            scenes: Vec::new(),
            program_scene_index: 0,
            preview_scene_index: 0,
            program_sources: Vec::new(),
            sources: Vec::new(),
            focused_pane: StudioPane::Preview,
            selected_source_index: None,
            is_live: false,
            is_paused: false,
            is_afk: false,
            afk_start_time: 0.0,
            live_start_time: 0.0,
            stream_platform: "HumanityOS Server".into(),
            stream_key: String::new(),
            stream_server_url: "wss://united-humanity.us/ws".into(),
            stream_resolution: "1920x1080".into(),
            stream_bitrate: 3500,
            stream_fps: 30,
            chat_overlay_channel: "general".into(),
            chat_overlay_font_size: 14.0,
            chat_overlay_position: "Top-Right".into(),
            chat_overlay_opacity: 0.8,
            chat_overlay_max_messages: 15,
            chat_overlay_bg_opacity: 0.3,
            broadcast_request: None,
            broadcast_live: false,
            broadcast_viewers: 0,
            broadcast_kbps: 0,
            broadcast_frames: 0,
            broadcast_dropped: 0,
            broadcast_error: String::new(),
            broadcast_url: String::new(),
        }
    }
}

#[cfg(feature = "native")]
impl StudioState {
    /// Stage a scene into PREVIEW: remember the index and apply that scene's
    /// per-source visibility to the working (preview) source set. PROGRAM is
    /// deliberately untouched -- that is the whole point of the split: you can
    /// click through scenes and rearrange sources without changing what is live.
    pub fn select_preview_scene(&mut self, idx: usize) {
        if idx >= self.scenes.len() {
            return;
        }
        self.preview_scene_index = idx;
        let vis = self.scenes[idx].source_visibility.clone();
        for (j, src) in self.sources.iter_mut().enumerate() {
            if let Some(&v) = vis.get(j) {
                src.visible = v;
            }
        }
    }

    /// Cut transition: push the staged PREVIEW to PROGRAM. The program scene index
    /// takes the preview index and the program pane gets a frozen copy of the
    /// current working sources, so later preview edits leave program alone.
    pub fn cut_to_program(&mut self) {
        self.program_scene_index = self.preview_scene_index;
        self.program_sources = self.sources.clone();
    }

    /// Add a new custom (deletable) scene snapshotting the current preview source
    /// visibility. Returns the new scene's index.
    pub fn add_custom_scene(&mut self) -> usize {
        let idx = self.scenes.len();
        let vis = self.sources.iter().map(|s| s.visible).collect();
        self.scenes.push(StudioScene {
            name: format!("Custom {}", idx + 1),
            is_default: false,
            source_visibility: vis,
        });
        idx
    }

    /// Delete a non-default scene, keeping BOTH the program and preview indices
    /// pointing at the same scenes they pointed at before (shift down when a scene
    /// above them is removed; fall back to scene 0 if the deleted scene itself was
    /// program or preview). `program_sources` is a frozen copy, so the program
    /// pane keeps rendering the last-cut layout either way.
    pub fn delete_scene(&mut self, idx: usize) {
        if idx >= self.scenes.len() || self.scenes[idx].is_default {
            return;
        }
        self.scenes.remove(idx);
        for index in [&mut self.preview_scene_index, &mut self.program_scene_index] {
            if *index == idx {
                *index = 0;
            } else if *index > idx {
                *index -= 1;
            }
        }
    }
}

#[cfg(all(test, feature = "native"))]
mod studio_state_tests {
    use super::*;

    fn src(name: &str, visible: bool) -> StudioSource {
        StudioSource {
            name: name.into(),
            source_type: StudioSourceType::Text(name.into()),
            visible,
            position: (0.1, 0.1),
            size: (0.3, 0.3),
            opacity: 1.0,
            z_order: 0,
        }
    }

    fn scene(name: &str, is_default: bool, vis: &[bool]) -> StudioScene {
        StudioScene {
            name: name.into(),
            is_default,
            source_visibility: vis.to_vec(),
        }
    }

    /// Two scenes, two sources, program cut on scene 0.
    fn demo() -> StudioState {
        let mut s = StudioState::default();
        s.sources = vec![src("Camera", true), src("Chat", false)];
        s.scenes = vec![
            scene("Main", true, &[true, false]),
            scene("BRB", true, &[false, true]),
            scene("Custom", false, &[true, true]),
        ];
        s.cut_to_program();
        s
    }

    #[test]
    fn select_into_preview_leaves_program_alone() {
        let mut s = demo();
        s.select_preview_scene(1);
        assert_eq!(s.preview_scene_index, 1, "clicked scene becomes preview");
        assert_eq!(s.program_scene_index, 0, "program scene must NOT follow a preview click");
        // Preview working set took scene 1's visibility ...
        assert!(!s.sources[0].visible);
        assert!(s.sources[1].visible);
        // ... while the frozen program snapshot kept the cut-time arrangement.
        assert!(s.program_sources[0].visible);
        assert!(!s.program_sources[1].visible);
    }

    #[test]
    fn select_out_of_range_is_a_noop() {
        let mut s = demo();
        s.select_preview_scene(99);
        assert_eq!(s.preview_scene_index, 0);
    }

    #[test]
    fn cut_copies_preview_to_program() {
        let mut s = demo();
        s.select_preview_scene(1);
        s.cut_to_program();
        assert_eq!(s.program_scene_index, 1);
        assert_eq!(s.program_sources.len(), s.sources.len());
        assert!(!s.program_sources[0].visible);
        assert!(s.program_sources[1].visible);
    }

    #[test]
    fn source_edits_target_preview_not_program() {
        let mut s = demo();
        // Rearranging / retitling / hiding sources = the properties-panel edits.
        s.sources[0].position = (0.7, 0.7);
        s.sources[0].visible = false;
        s.sources.push(src("New overlay", true));
        assert_eq!(
            s.program_sources[0].position,
            (0.1, 0.1),
            "moving a preview source must not move it in program"
        );
        assert!(s.program_sources[0].visible, "hiding in preview must not hide in program");
        assert_eq!(s.program_sources.len(), 2, "adding a preview source must not appear in program");
    }

    #[test]
    fn delete_scene_shifts_both_indices() {
        let mut s = demo();
        s.select_preview_scene(2);
        s.cut_to_program(); // program = preview = 2 ("Custom")
        // Removing a DEFAULT scene is refused.
        s.delete_scene(0);
        assert_eq!(s.scenes.len(), 3);
        // Make a second custom scene above nothing, then delete index 2 while both
        // indices sit at 2: both fall back to 0.
        s.delete_scene(2);
        assert_eq!(s.scenes.len(), 2);
        assert_eq!(s.preview_scene_index, 0);
        assert_eq!(s.program_scene_index, 0);
    }

    #[test]
    fn delete_scene_below_indices_shifts_them_down() {
        let mut s = demo();
        // Insert a custom scene at the front so there is a deletable scene BELOW the others.
        s.scenes.insert(0, scene("Front custom", false, &[true, true]));
        s.preview_scene_index = 2;
        s.program_scene_index = 3;
        s.delete_scene(0);
        assert_eq!(s.preview_scene_index, 1, "preview index shifts down past a removed scene");
        assert_eq!(s.program_scene_index, 2, "program index shifts down past a removed scene");
    }

    #[test]
    fn add_custom_scene_snapshots_preview_visibility() {
        let mut s = demo();
        s.sources[0].visible = false;
        s.sources[1].visible = true;
        let idx = s.add_custom_scene();
        assert_eq!(idx, 3);
        assert!(!s.scenes[idx].is_default, "user-added scenes must be deletable");
        assert_eq!(s.scenes[idx].source_visibility, vec![false, true]);
    }
}

/// A machine's floating world-space label (built in load_world, drawn by the in-game
/// HUD with distance-based level-of-detail: dot, then name, then a stat card).
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct MachineLabel {
    /// World anchor, set just above the machine so the label floats over it.
    pub pos: glam::Vec3,
    pub name: String,
    pub stats: Vec<crate::machines::MachineStat>,
    /// The room this machine sits in (for room-based occlusion: a label only shows by
    /// default when you are in its room; hold Tab to see across rooms).
    pub room: String,
    /// The machine's instance id (home.ron `id`) — links this label to its ECS
    /// entity so the per-frame refresh can patch LIVE stats (cistern fill,
    /// battery charge) over the static RON placeholders. (v0.724)
    pub machine_id: String,
}

// NOTE (v0.725): the pinned machine card's auto-recipe selector state lives on
// GuiState (machine_card_recipe / _options / _pending) — published per frame
// by lib.rs from the selected machine's AutoRefine + the recipe registry, and
// applied back to the ECS entity when the player picks a different recipe.

/// A crew NPC's floating nameplate: name + live chore label ("Vex -- Taking reactor
/// readings"). Rebuilt EVERY frame in lib.rs from the relay-driven `RemoteNpc`
/// components (crew walk, so a cached position would lag), drawn by the in-game HUD
/// through the same world_to_screen + text_shadowed path as machine labels. (v0.667)
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct CrewLabel {
    /// World anchor, just above the NPC's head.
    pub pos: glam::Vec3,
    pub name: String,
    /// Human-readable current chore label from data/npc/chores.ron.
    pub activity: String,
    /// True while the NPC dwells at its chore site (vs walking to it).
    pub working: bool,
}

/// An axis-aligned room volume, used by the HUD to tell which room the camera is in
/// (for label occlusion). Populated by load_world from the homestead room info.
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct RoomBounds {
    pub id: String,
    pub min: glam::Vec3,
    pub max: glam::Vec3,
    /// Function joined from data/rooms.ron at load (v0.439): the room finally knows what it
    /// is FOR. Display name, purpose text, the in-room action labels, and access class.
    pub display_name: String,
    pub purpose: String,
    /// Human LABELS for the in-room actions ("Change Outfit"), for showing to a person.
    pub actions: Vec<String>,
    /// The PAGE each of those actions opens ("wardrobe", "appearance", "inventory"), which
    /// is what code matches on. A label is written to be read and can be reworded; the page
    /// is the action's identity. See `RoomTypeRegistry::action_pages` for why this is split.
    pub action_pages: Vec<String>,
    pub access: String,
}

/// Kind of a placed opening in the editor mirror (v0.469). Mirrors a subset of
/// `fibonacci::OpeningKind` (Hatch is engine-only for now -- the editor offers the three the
/// operator named: door, window, airlock).
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorOpeningKind {
    Door,
    Window,
    Airlock,
}

#[cfg(feature = "native")]
impl EditorOpeningKind {
    pub const ALL: [EditorOpeningKind; 3] =
        [EditorOpeningKind::Door, EditorOpeningKind::Window, EditorOpeningKind::Airlock];
    pub fn label(self) -> &'static str {
        match self {
            EditorOpeningKind::Door => "Door",
            EditorOpeningKind::Window => "Window",
            EditorOpeningKind::Airlock => "Airlock",
        }
    }
    /// Doors + airlocks sit on the floor (no vertical move, no resize); windows float + resize.
    pub fn floor_pinned(self) -> bool {
        matches!(self, EditorOpeningKind::Door | EditorOpeningKind::Airlock)
    }
    /// Sensible default (width, height) in metres when a new opening of this kind is added.
    pub fn default_size(self) -> (f32, f32) {
        match self {
            EditorOpeningKind::Door => (0.95, 2.1),
            EditorOpeningKind::Window => (1.4, 1.3),
            EditorOpeningKind::Airlock => (1.2, 2.1),
        }
    }
}

/// One placed opening in the editor mirror (v0.469): an additive door/window/airlock on a
/// still-solid wall. `wall` is the build-loop index (0=N,1=S,2=W,3=E); `u` is the centre along
/// that wall (metres from its start corner); `v` is the centre height up the wall (metres;
/// pinned to h/2 for floor kinds); `w`/`h` are the size.
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq)]
pub struct EditorOpening {
    pub kind: EditorOpeningKind,
    pub wall: usize,
    pub u: f32,
    pub v: f32,
    pub w: f32,
    pub h: f32,
}

/// One editable room row in the construction editor (v0.459). The engine fills this from the
/// live layout when the editor opens and reads it back on `construction_dirty`. `position` is
/// Some(x,y,z) once the room is explicitly placed (which kills the Fibonacci spiral override
/// for that room); None means "let the auto-layout compute it".
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct ConstructionRoom {
    pub id: String,
    pub walls: [crate::ship::fibonacci::WallKind; 4], // N, S, W, E
    /// Per-wall opening slide offset (metres along the wall, signed; 0 = centred). (v0.468)
    pub wall_offsets: [f32; 4],
    /// Placed openings (doors/windows/airlocks) on this room's walls (v0.469).
    pub openings: Vec<EditorOpening>,
    /// Vertical storey this room sits on (v0.471). 0 = ground floor; world Y = level * story_height.
    pub level: i32,
    pub position: Option<[f32; 3]>,
    pub dimensions: [f32; 3], // (width_x, height_y, depth_z) metres
    pub material_type: u32,
    pub color: [f32; 4],
}

#[cfg(feature = "native")]
impl ConstructionRoom {
    /// Length (metres) of wall `wi` (0=N,1=S along X; 2=W,3=E along Z).
    pub fn wall_len(&self, wi: usize) -> f32 {
        if wi < 2 { self.dimensions[0] } else { self.dimensions[2] }
    }
}

/// A transient confirmation toast (v0.861). Reusable feedback so an action like
/// "Save theme" is never silent -- the universal answer to the operator's complaint
/// that save buttons "don't change when clicked". Any code path can push one; they
/// stack bottom-center and fade out. Replaces the one-off per-button "Saved" notes.
#[cfg(feature = "native")]
pub struct Toast {
    pub text: String,
    /// egui time (seconds) when it was created; stamped on push.
    pub created: f64,
    pub kind: ToastKind,
    /// Seconds on screen, fade included. Short for an action confirmation
    /// ("Saved"), long for a notice the player has to read (2026-09-25: the
    /// "while you were away" line was queued during boot and its 2.6 s ran
    /// out before the menu had even settled).
    pub life: f64,
}

/// How long a confirmation toast stays up, fade included.
#[cfg(feature = "native")]
pub const TOAST_LIFE: f64 = 2.6;
/// How long a notice stays up: long enough to read a sentence after the app
/// has finished coming up around it.
#[cfg(feature = "native")]
pub const NOTICE_LIFE: f64 = 12.0;

#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Error,
}

/// How the top-nav buttons present themselves (v0.859), mirroring the web header's
/// icon/text toggle. `Both` is the default (icon + label); `IconOnly` is the compact
/// mode the operator wanted; `TextOnly` drops the glyphs.
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum NavDisplayMode {
    #[default]
    Both,
    IconOnly,
    TextOnly,
}

#[cfg(feature = "native")]
impl NavDisplayMode {
    /// Cycle Both -> IconOnly -> TextOnly -> Both.
    pub fn next(self) -> Self {
        match self {
            NavDisplayMode::Both => NavDisplayMode::IconOnly,
            NavDisplayMode::IconOnly => NavDisplayMode::TextOnly,
            NavDisplayMode::TextOnly => NavDisplayMode::Both,
        }
    }
    /// The cycle button's label glyphs (ASCII, so never tofu).
    pub fn glyph(self) -> &'static str {
        match self {
            NavDisplayMode::Both => "Aa",
            NavDisplayMode::IconOnly => "A",
            NavDisplayMode::TextOnly => "aa",
        }
    }
    pub fn show_icon(self) -> bool {
        !matches!(self, NavDisplayMode::TextOnly)
    }
    pub fn show_label(self) -> bool {
        !matches!(self, NavDisplayMode::IconOnly)
    }
}

/// How the per-setting DESCRIPTIONS (the muted help line under each control on
/// the Settings page) are shown (v0.1116, operator 2026-08-12). The descriptions
/// used to read as evenly "stepped": each sat a full item-spacing below its
/// control and the same distance above the next control, so a control and its
/// own description did not visually group. `widgets::setting_hint` uses this to
/// choose the layout.
///
/// EXTENSIBLE by design (operator: "make sure we potentially add others in case
/// we need to"). Serialization is by serde variant NAME (a stable short string:
/// "Full" / "Hover" / "Off"), never a bare discriminant, so reordering the
/// variants here does not change what is stored on disk. Adding a new mode is
/// safe: `setting_hint`'s match falls through to the `Full` layout for any
/// variant it does not explicitly handle, so a newer variant never blanks the
/// help text before it is wired up.
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum HintDisplay {
    /// Descriptions always visible, tightened to the control above them, with a
    /// larger gap below so each control + description reads as one unit. Default.
    #[default]
    Full,
    /// Descriptions not shown inline; a small muted "(?)" marker sits under the
    /// control and reveals the full text as a tooltip on hover.
    Hover,
    /// Descriptions hidden entirely; only the between-unit gap remains, so the
    /// controls keep the same airy rhythm without any help text.
    Off,
}

#[cfg(feature = "native")]
impl HintDisplay {
    /// The three modes, in picker order.
    pub const ALL: [HintDisplay; 3] = [HintDisplay::Full, HintDisplay::Hover, HintDisplay::Off];
    /// Short label for the Settings picker button.
    pub fn label(self) -> &'static str {
        match self {
            HintDisplay::Full => "Full",
            HintDisplay::Hover => "Hover",
            HintDisplay::Off => "Off",
        }
    }
}

