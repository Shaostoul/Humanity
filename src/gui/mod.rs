//! egui GUI system for the native desktop client.
//!
//! Theme loaded from data/gui/theme.ron (hot-reloadable).
//! Widgets are reusable components. Pages compose widgets into screens.

#[cfg(feature = "native")]
pub mod theme;
#[cfg(feature = "native")]
pub mod widgets;
#[cfg(feature = "native")]
pub mod pages;
#[cfg(feature = "native")]
pub mod fonts;
#[cfg(feature = "native")]
pub mod glossary;

/// Location-aware rules ("Laws") data loader (v0.496). See pages/laws.rs.
pub mod laws;

// Headless UI snapshot tests (v0.495): render egui pages to PNGs for review +
// regression. Test-only; pulls in egui_kittest (a dev-dependency).
#[cfg(test)]
mod ui_snapshots;

/// Current engine version (read from Cargo.toml at compile time).
#[cfg(feature = "native")]
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A tool entry loaded from data/tools/catalog.json.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub platforms: Vec<String>,
    /// Optional download size hint (e.g. "~350MB").
    #[serde(default)]
    pub size: String,
    /// Category name, populated during loading from the parent category.
    #[serde(skip)]
    pub category: String,
}

/// A single donation address entry (for the dynamic addresses array).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct DonateAddress {
    /// Network display name, e.g. "Solana (SOL)", "Bitcoin (BTC)"
    pub network: String,
    /// Type: "address" or "url"
    pub addr_type: String,
    /// The address or URL value
    pub value: String,
    /// Human-readable label, e.g. "Send SOL or SPL tokens"
    pub label: String,
}

#[cfg(feature = "native")]
impl DonateAddress {
    /// Parse the server's `/api/server-info` `funding` object into address entries.
    /// Entries with an empty `value` are skipped (the shipped server-config.json
    /// leaves placeholder rows blank until the operator fills them in), so an
    /// unconfigured network never renders as a dead button on the Donate page.
    pub fn from_funding_json(funding: &serde_json::Value) -> Vec<Self> {
        let Some(addrs) = funding.get("addresses").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        addrs
            .iter()
            .filter_map(|a| {
                let value = a.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if value.is_empty() {
                    return None;
                }
                Some(Self {
                    network: a.get("network").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    addr_type: a.get("type").and_then(|v| v.as_str()).unwrap_or("address").to_string(),
                    value: value.to_string(),
                    label: a.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect()
    }
}

#[cfg(feature = "native")]
impl GuiState {
    /// Land a finished connect-time /api/server-info fetch into the Donate page's
    /// server-funding fields. `from_url` is the trimmed server URL the fetch was
    /// sent to; a result from a server we are no longer connected to is DISCARDED
    /// (donation info is money-routing data -- showing server A's addresses while
    /// connected to server B sends a donor's money to the wrong operator). A
    /// matching result always REPLACES both fields, so a server whose funding is
    /// disabled/absent clears any previously-shown list instead of inheriting it.
    pub fn apply_server_funding(&mut self, from_url: &str, funding: Option<&serde_json::Value>) {
        if from_url != self.server_url.trim_end_matches('/') {
            return;
        }
        self.donate_funding_server = from_url.to_string();
        self.donate_addresses_server = funding.map(DonateAddress::from_funding_json).unwrap_or_default();
        self.donate_funding_goal = funding.and_then(|f| {
            f.get("goal_usd").and_then(|v| v.as_f64()).filter(|g| *g > 0.0).map(|goal| {
                let label = f.get("goal_label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (goal, label)
            })
        });
    }
}

#[cfg(all(test, feature = "native"))]
mod donate_funding_tests {
    use super::{DonateAddress, GuiState};

    #[test]
    fn funding_result_from_the_connected_server_lands_and_absent_funding_clears() {
        let mut state = GuiState::default();
        state.server_url = "https://united-humanity.us/".to_string();
        let funding = serde_json::json!({
            "goal_usd": 100000.0, "goal_label": "Full-time development",
            "addresses": [{"network": "GitHub Sponsors", "type": "url",
                           "value": "https://github.com/sponsors/Shaostoul", "label": "x"}]
        });
        state.apply_server_funding("https://united-humanity.us", Some(&funding));
        assert_eq!(state.donate_addresses_server.len(), 1);
        assert_eq!(state.donate_funding_goal, Some((100000.0, "Full-time development".to_string())));

        // Reconnect-to-a-server-without-funding: the SAME url reporting no funding
        // must CLEAR the fields, not leave the old list showing (the adversarial
        // review's major finding: stale addresses = money to the wrong operator).
        state.apply_server_funding("https://united-humanity.us", None);
        assert!(state.donate_addresses_server.is_empty(), "absent funding clears the list");
        assert!(state.donate_funding_goal.is_none(), "absent funding clears the goal");
    }

    #[test]
    fn late_result_from_a_previous_server_is_discarded() {
        let mut state = GuiState::default();
        state.server_url = "https://server-b.example".to_string();
        let funding_a = serde_json::json!({
            "addresses": [{"network": "Bitcoin (BTC)", "type": "address", "value": "bc1qA", "label": ""}]
        });
        // A stale fetch tagged with server A's url lands while connected to B.
        state.apply_server_funding("https://server-a.example", Some(&funding_a));
        assert!(state.donate_addresses_server.is_empty(), "server A's addresses must not display on server B");
        assert!(state.donate_funding_server.is_empty());
    }

    #[test]
    fn parses_filled_entries_and_skips_blank_placeholders() {
        // Mirrors the real shipped server-config.json shape: GitHub Sponsors is
        // filled in, the crypto rows are blank placeholders awaiting the operator.
        let funding = serde_json::json!({
            "enabled": true,
            "goal_usd": 100000,
            "addresses": [
                {"network": "GitHub Sponsors", "type": "url",
                 "value": "https://github.com/sponsors/Shaostoul", "label": "Recurring or one-time"},
                {"network": "Solana (SOL)", "type": "address", "value": "", "label": "Send SOL"},
                {"network": "Bitcoin (BTC)", "type": "address", "value": "", "label": "Send BTC"},
            ]
        });
        let parsed = DonateAddress::from_funding_json(&funding);
        assert_eq!(parsed.len(), 1, "blank-value placeholder rows are dropped");
        assert_eq!(parsed[0].network, "GitHub Sponsors");
        assert_eq!(parsed[0].addr_type, "url");
        assert_eq!(parsed[0].value, "https://github.com/sponsors/Shaostoul");
        assert_eq!(parsed[0].label, "Recurring or one-time");
    }

    #[test]
    fn missing_or_malformed_addresses_yield_empty_not_panic() {
        assert!(DonateAddress::from_funding_json(&serde_json::json!({"enabled": true})).is_empty());
        assert!(DonateAddress::from_funding_json(&serde_json::json!({"addresses": "oops"})).is_empty());
        assert!(DonateAddress::from_funding_json(&serde_json::json!(null)).is_empty());
    }

    #[test]
    fn missing_type_defaults_to_address() {
        let funding = serde_json::json!({"addresses": [{"network": "X", "value": "abc123"}]});
        let parsed = DonateAddress::from_funding_json(&funding);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].addr_type, "address");
        assert_eq!(parsed[0].label, "");
    }
}

/// What the passphrase / PIN prompt is for.
///
/// The first three (SetNew, Unlock, Change) gate the BIP39-derived passphrase
/// vault that has existed since the early native client. The PIN variants
/// (v0.278.0 auto-unlock) gate the short PIN that wraps a keychain-stored
/// device key. PIN and passphrase coexist: setting a PIN doesn't remove the
/// passphrase vault — it's an alternate, faster unlock that lives on top.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseMode {
    /// Setting a new passphrase (first time or migration from plaintext).
    SetNew,
    /// Unlocking an existing encrypted key.
    Unlock,
    /// Changing the passphrase (requires old + new).
    Change,
    /// Setting a new PIN — requires the seed already in memory (called
    /// either right after a successful passphrase unlock OR on a freshly
    /// generated identity). Encrypts seed with PIN+device_key.
    PinSetup,
    /// Unlocking with the PIN. Device key already loaded from keychain.
    PinUnlock,
    /// Changing PIN — requires old PIN + new PIN. Re-encrypts the seed
    /// blob; the device_key in the keychain stays the same.
    PinChange,
}

/// Which page/overlay is currently active.
/// Server metadata fetched from a relay's GET /api/server-info, shown in the
/// launcher's server-detail pane (v0.478). A subset of the relay's
/// ServerInfoResponse; serde ignores fields we don't render.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub users_online: u64,
    #[serde(default)]
    pub member_count: u64,
    #[serde(default)]
    pub accord_compliant: bool,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub owner_key: String,
    /// Donation config, present only when the server has `funding.enabled: true`
    /// in its `server-config.json` (the relay omits the field entirely otherwise).
    /// Shape: `{"enabled":true,"goal_usd":...,"addresses":[{"network","type","value","label"}]}`.
    #[serde(default)]
    pub funding: Option<serde_json::Value>,
}

/// Which kind of thing the unified launcher (the showroom in character-select
/// mode) currently has selected in its left pane. Drives what the right pane
/// shows: a character editor (Home / OpenNet / ClosedNet) or server details.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherSel {
    /// A local, self-custodial home/character save (the wired path today).
    Home,
    /// A self-custodial character used on an open-net server (multiplayer).
    OpenNet,
    /// A server-held, anti-cheat character (multiplayer).
    ClosedNet,
    /// A server row: the right pane shows server details instead of a character.
    Server,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    /// In-game, no menu overlay (HUD still visible).
    None,
    /// Title screen: Play, Settings, Quit.
    MainMenu,
    // ── Tool pages ──
    Settings,
    Inventory,
    Tasks,
    Maps,
    Market,
    Profile,
    Civilization,
    Chat,
    Calculator,
    Notes,
    Calendar,
    Crafting,
    Wallet,
    Guilds,
    Trade,
    Files,
    BugReport,
    // v0.415.0: removed the Resources and Onboarding GuiPage variants. The
    // Resources directory was retired into the Library (v0.374-375) and the
    // standalone onboarding page into the Mission Dashboard + Quests page
    // (v0.373); first boot now lands on Humanity. Saved config strings
    // ("resources" / "onboarding") migrate in config_str_to_page below.
    Library,
    Donate,
    Tools,
    Studio,
    Quests,
    /// Server / group administration settings page. Opened from the cog
    /// menu on the server or group row in the chat sidebar.
    ServerSettings,
    // v0.479: GuiPage::GameAdmin removed. Game-world bans folded into a
    // subsection of Server Settings > ADMIN (game_admin::draw_section), so the
    // nav has one fewer button. The two ban systems stay structurally separate
    // (disjoint tables + a distinct subsection with the free-speech disclaimer).
    /// Identity hub: DID, Verifiable Credentials, trust score, AI status.
    /// Mirrors the web `/identity` page.
    Identity,
    /// Local + civilization-scope governance: proposals, votes, tally.
    /// Mirrors the web `/governance` page.
    Governance,
    /// Location-aware rules + rights, nested Humanity -> locality (v0.496).
    Laws,
    /// Social key recovery setup + active recovery requests.
    /// Mirrors the web `/recovery` page.
    Recovery,
    // v0.197.0: removed Agents and AiUsage GuiPage variants. Operator
    // 2026-05-08: "That AI Agents page also seems useless. As well as
    // the AI usage." Multi-AI orchestration is handled via
    // data/coordination/* + the relay agent_sessions table — the UI
    // pages weren't pulling their weight. The page modules + state
    // fields were removed in the same release.
    /// Cosmos page (v0.203.0, Phase 3). Three-mode astronomical map:
    /// System (Sol planets), Galactic (Sol-centered nearby stars in ly),
    /// Night Sky (Earth-centered celestial sphere with constellations).
    /// Lives under Sim category in the nav. See pages/cosmos.rs.
    Cosmos,
    /// QA testing tasks — operator-facing checklist of features to manually verify.
    /// Each task has Mark Passed / Report Issue buttons that post results to chat.
    Testing,
    /// Curated bookmarks page. First step toward the in-app browser — for
    /// now each card opens its URL in the OS default browser via egui's
    /// open_url. Data lives in `data/browser/bookmarks.json`.
    Browser,
    /// Real — the merged "your actual life" tab (v0.358): one page with a
    /// section_nav sidebar folding in Profile's sections + Inventory, Wallet,
    /// Tasks, Map, Market. Replaces six separate nav buttons. See pages/real.rs.
    Real,
    // v0.415.0: removed the Play GuiPage variant (the v0.360 tab that folded
    // Crafting + Studio). Both are top-level tabs now and nothing navigated to
    // it; the nav's Play button is GuiPage::None (FPS mode), unrelated.
    /// Platform — the software-itself tab (v0.360): section_nav folds Settings,
    /// Recovery, Tools, Bugs, Testing, Browser.
    Platform,
    /// Humanity — the collective / mission tab (v0.360): section_nav folds the
    /// Community/Mission Dashboard (Civilization) + Governance, Directory
    /// (Identity), Donate. What the H button opens, and the first-boot landing.
    Humanity,
    /// Home — your offline homestead (v0.379): the Fibonacci homestead Design
    /// browsed as rooms + bill-of-materials + power/water demand + a self-
    /// sufficiency summary. The "homes as save profiles" surface, offline-first
    /// (server/real homes come later). See pages/homes.rs + homes-as-profiles.md.
    Homes,
    // v0.699.0: removed the two-tier-nav-era category-browse subsystem --
    // the 5 Overview* category landing pages (OverviewReality/Sim/Tools/
    // Settings/Dev) and the 12 Settings* sub-page variants (SettingsAccount
    // .. SettingsUpdates). All 17 were dead since the v0.196 single-row nav
    // rewrite: nothing navigated to an Overview page, and the Settings*
    // variants were only reachable as cards on the (unreachable)
    // OverviewSettings. Settings content lives in `settings.rs` (its own
    // internal SettingsCategory router, still fully reachable via the
    // top-level Settings tab); the stranded working pages Calculator/Files
    // (now in Platform) and Trade/Guilds (now in Real) were rehomed in the
    // same release. See the 2026-07-04 page-access audit.
}

/// Pages that can be selected as the startup boot page.
#[cfg(feature = "native")]
pub const BOOT_PAGE_OPTIONS: &[(GuiPage, &str)] = &[
    (GuiPage::Humanity, "Humanity (Mission Dashboard)"),
    (GuiPage::Chat, "Chat"),
    (GuiPage::Tasks, "Tasks"),
    (GuiPage::Maps, "Maps"),
    (GuiPage::Notes, "Notes"),
    (GuiPage::Calendar, "Calendar"),
    (GuiPage::Cosmos, "Cosmos"),
    (GuiPage::Library, "Library"),
];

#[cfg(feature = "native")]
pub fn page_to_config_str(page: GuiPage) -> &'static str {
    match page {
        GuiPage::Humanity => "humanity",
        GuiPage::Chat => "chat",
        GuiPage::Tasks => "tasks",
        GuiPage::Maps => "maps",
        GuiPage::Notes => "notes",
        GuiPage::Calendar => "calendar",
        GuiPage::Cosmos => "cosmos",
        GuiPage::Library => "library",
        _ => "humanity",
    }
}

#[cfg(feature = "native")]
pub fn config_str_to_page(s: &str) -> GuiPage {
    match s {
        "humanity" => GuiPage::Humanity,
        "chat" => GuiPage::Chat,
        "tasks" => GuiPage::Tasks,
        "maps" => GuiPage::Maps,
        "notes" => GuiPage::Notes,
        "calendar" => GuiPage::Calendar,
        "cosmos" => GuiPage::Cosmos,
        "library" => GuiPage::Library,
        // Retired pages saved in old configs land on their successors:
        // the Resources directory lives in the Library; the onboarding
        // landing is the Mission Dashboard (also the unknown-id default).
        "resources" => GuiPage::Library,
        _ => GuiPage::Humanity,
    }
}

/// Item slot data bridged from ECS Inventory for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiItemSlot {
    /// Item ID from items.csv.
    pub item_id: String,
    /// Human-readable item name (looked up from ItemRegistry).
    pub name: String,
    /// Quantity in this stack.
    pub quantity: u32,
}

/// Game time snapshot bridged from TimeSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiGameTime {
    pub hour: f32,
    pub day_count: u32,
    pub season: String,
    pub is_daytime: bool,
}

/// Weather snapshot bridged from WeatherSystem for GUI display.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct GuiWeather {
    pub condition: String,
    pub temperature: f32,
    pub wind_speed: f32,
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

/// One buyer-seller message on a listing thread, mirroring the relay's
/// ListingMessageData (v0.755).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiListingMsg {
    pub sender_key: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: i64,
}

#[cfg(feature = "native")]
impl GuiListingMsg {
    pub fn from_relay_json(v: &serde_json::Value) -> Self {
        Self {
            sender_key: v.get("sender_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            sender_name: v.get("sender_name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            content: v.get("content").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            timestamp: v.get("timestamp").and_then(|x| x.as_i64()).unwrap_or(0),
        }
    }
}

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

    /// Same wire pin for the v0.755 trade-flow frames: the relay's REAL
    /// ListingMessages and ReviewCreated frames map through the native
    /// dispatch path (a serde rename would silently empty the thread or
    /// the reviews card).
    #[test]
    fn relay_thread_and_review_frames_round_trip_to_gui() {
        use super::{GuiListingMsg, GuiReview};

        let msgs = crate::relay::relay::RelayMessage::ListingMessages {
            listing_id: "wire-1".into(),
            messages: vec![crate::relay::relay::ListingMessageData {
                id: 7,
                listing_id: "wire-1".into(),
                sender_key: "buyerkey".into(),
                sender_name: Some("Ada".into()),
                content: "Is this still available?".into(),
                timestamp: 1_720_000_000_000,
            }],
            target: Some("buyerkey".into()),
        };
        let val: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&msgs).unwrap()).unwrap();
        assert_eq!(val.get("type").and_then(|t| t.as_str()), Some("listing_messages"));
        let arr = val.get("messages").and_then(|v| v.as_array()).unwrap();
        let m = GuiListingMsg::from_relay_json(&arr[0]);
        assert_eq!(m.sender_name, "Ada");
        assert_eq!(m.content, "Is this still available?");
        assert_eq!(m.timestamp, 1_720_000_000_000);

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
    pub mature: bool,
    pub dead: bool,
    /// The tower this crop belongs to (its config id), if planted via a tower.
    pub tower_id: Option<String>,
    /// Which slot of the tower this crop occupies (0-based), for the slot view.
    pub tower_slot: Option<u32>,
    /// Plant-def reference data (from plants.csv) shown as Garden-table columns:
    /// relative nutrient demand (N, P, K), daily water need (L), and the
    /// tolerated temperature window (Celsius). 0 when the species is unknown.
    pub n: f32,
    pub p: f32,
    pub k: f32,
    pub water_per_day: f32,
    pub temp_min: f32,
    pub temp_max: f32,
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
                // theme-exempt: the fallback guild colour is user data, not a UI token.
                .unwrap_or(egui::Color32::from_rgb(68, 136, 255)),
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
/// A DM that the user clicked Send on, but which we COULDN'T encrypt
/// (recipient's ECDH key not known, our key not set, or encryption
/// errored). Stored on GuiState so a confirmation modal can pop up
/// asking the user to either send it as plaintext anyway, or cancel.
///
/// Backstory: before v0.199 the code silently sent the DM as plaintext
/// with only a log message. Operator security audit (B3, 2026-04-30)
/// flagged this as a downgrade attack vector — an attacker who can
/// suppress ECDH key announcements could strip encryption from a DM
/// the user thinks is private. The confirmation modal forces explicit
/// user opt-in for any plaintext send.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct PendingUnencryptedDm {
    /// Recipient's public key (Ed25519 hex).
    pub partner_key: String,
    /// Recipient's display name (best-known label for the modal copy).
    pub partner_name: String,
    /// The plaintext message body the user typed.
    pub content: String,
    /// Original send timestamp (ms since epoch). Reused on confirm so
    /// the eventual sent message has the same `ts` the user clicked Send at.
    pub timestamp_ms: u64,
    /// Why we can't encrypt — one of:
    ///   "missing_peer_key"     — recipient hasn't broadcast their ECDH pub key yet
    ///   "no_own_ecdh"          — this client doesn't have its own ECDH key set
    ///   "encryption_failed: X" — encrypt_dm() errored with X
    pub reason: String,
}

#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatChannel {
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

/// A group chat entry for the left panel.
/// Each group acts like a mini-server with its own channels.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct ChatGroup {
    pub name: String,
    pub id: String,
    pub member_count: u32,
    /// Group channels (default: just #general)
    pub channels: Vec<ChatChannel>,
    /// Whether this group's channel list is collapsed in the sidebar
    pub collapsed: bool,
    /// This user's role in the group, as reported by the server's
    /// `group_list` message (`GroupData.role`, `"admin"` for the group's
    /// creator, `"member"` otherwise -- see `group_members.role` in
    /// `src/relay/storage/social.rs::create_group`). Defaults to `"member"`
    /// so an old/partial payload never silently grants admin.
    pub role: String,
    /// A message arrived in this group while its channel wasn't open —
    /// drives the sidebar unread dot, same pattern as ChatDm. (v0.717)
    pub unread: bool,
}

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
    pub actions: Vec<String>,
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

/// Tracks all GUI state for the native app.
#[cfg(feature = "native")]
pub struct GuiState {
    pub active_page: GuiPage,
    /// Last page visited before returning to game view. Escape reopens this page.
    pub last_page: GuiPage,
    /// Navigation back-stack. When a page opens a sub-page (e.g. clicking
    /// the cog on a server row opens ServerSettings from Chat), the source
    /// page is pushed here so Escape returns to it instead of jumping
    /// straight to FPS mode. Operator 2026-05-08: "if I'm in nested pages
    /// like that esc needs to reliably take people back to the previous
    /// menu/page". Use `push_nav_to` / `pop_nav_back` helpers below.
    /// Direct nav-bar clicks DO NOT push (they replace the current page,
    /// not nest under it).
    pub nav_back_stack: Vec<GuiPage>,
    pub show_chat: bool,
    /// In-world interactive chat panel open (v0.772): Enter opens it, which
    /// frees the cursor + disables look/move so you can read + type in the
    /// same relay chat as the Chat page/website without leaving the 3D world.
    pub chat_input_active: bool,
    /// Set true when chat_input_active just opened so the panel requests
    /// keyboard focus for its input exactly once (re-focusing every frame
    /// would steal focus from the channel buttons). (v0.772)
    pub chat_input_focus_pending: bool,
    /// Shared-world co-presence status (v0.774), mirrored from the ECS each
    /// frame by the multiplayer block in lib.rs so the paint-only HUD can show
    /// it. `copresence_active` = we've joined the relay's shared game world
    /// (in-world + connected). `copresence_names` = the OTHER players currently
    /// present (RemotePlayer entities). Makes the mission-critical co-presence
    /// visible: without this you can't tell you're in a shared world, or see
    /// when someone else joins.
    pub copresence_active: bool,
    pub copresence_names: Vec<String>,
    pub show_hud: bool,
    pub settings: SettingsState,
    pub chat_input: String,
    /// v0.282.0: peers currently typing in the active channel. Keyed by
    /// sender pubkey to avoid duplicate "X is typing…" rows when a peer
    /// emits multiple rate-limited typing events. Value = (display_name,
    /// when_received) — the renderer prunes entries older than 3 seconds
    /// to match the web client's auto-clear behavior.
    pub chat_typing_users: std::collections::HashMap<String, (String, std::time::Instant)>,
    /// v0.282.0: last time WE sent a typing indicator. Sites that want
    /// to emit typing should consult this for the 3-second rate limit
    /// rather than sending on every keystroke (matches the relay's
    /// `TYPING_RATE_LIMIT_SECS` so we never get silently dropped).
    pub chat_typing_last_sent: Option<std::time::Instant>,
    /// When the user clicks "Reply" on a message, this holds the parent context.
    /// Cleared on send or cancel. Drives the "Replying to ... [X]" banner above the input.
    pub chat_reply_to: Option<ReplyContext>,
    pub chat_messages: Vec<ChatMessage>,
    /// Whether the message search modal is open.
    pub chat_search_open: bool,
    /// Live search input text.
    pub chat_search_query: String,
    /// Most recent search results (cleared when the modal closes).
    pub chat_search_results: Vec<ChatSearchResult>,
    /// Pins per channel (id → list of pinned messages).
    /// Populated by `pins_sync` and `pin_added` server messages.
    pub chat_pins: std::collections::HashMap<String, Vec<ChatPin>>,
    /// Whether the pins modal is open.
    pub chat_pins_open: bool,
    /// `(timestamp_ms, draft_content)` for the message currently being edited.
    /// None = no edit in progress.
    pub chat_edit_target: Option<(u64, String)>,
    /// Timestamps of messages sent from THIS client (for dedup on echo).
    pub chat_sent_timestamps: Vec<u64>,
    pub chat_channels: Vec<ChatChannel>,
    pub chat_active_channel: String,
    pub chat_users: Vec<ChatUser>,
    pub chat_dms: Vec<ChatDm>,
    pub chat_groups: Vec<ChatGroup>,
    pub chat_servers: Vec<ChatServer>,
    pub chat_friends: Vec<ChatUser>,
    /// Keys of everyone I follow — RAW from the relay's follow_list, so it
    /// includes offline people that `chat_friends` (filtered against the
    /// online `chat_users`) misses. Drives the follow-direction badges. (v0.721)
    pub chat_following_keys: std::collections::HashSet<String>,
    /// Keys of everyone who follows ME (follow_list.followers + live
    /// follow_update). The relay always sent this; native dropped it until
    /// v0.721 — which is why the "Follows you" badges disappeared.
    pub chat_followers: std::collections::HashSet<String>,
    pub ws_client: Option<crate::net::ws_client::WsClient>,
    pub ws_status: String,
    /// Native WebRTC DataChannel P2P manager handle (increment 1). Lazily
    /// started after the WS connect/identify so we have our pubkey hex. `None`
    /// until then. Native-only — relay/wasm builds don't open peer channels.
    #[cfg(feature = "native")]
    pub webrtc: Option<crate::net::webrtc::WebrtcHandle>,
    /// DEV: the peer pubkey hex the chat page armed a P2P self-test for. When
    /// the channel to this peer opens, `lib.rs` auto-sends "native p2p test".
    /// Cleared is fine to leave; it just gates the one-shot test send. inc-1
    /// transport proof only — not user-facing UI.
    #[cfg(feature = "native")]
    pub webrtc_test_peer: Option<String>,
    /// Whether the user manually disconnected (suppresses auto-reconnect).
    pub ws_manually_disconnected: bool,
    /// Countdown to next reconnect attempt (seconds).
    pub ws_reconnect_timer: f32,
    /// Current reconnect delay with exponential backoff (seconds).
    pub ws_reconnect_delay: f32,
    /// Number of consecutive failed reconnect attempts.
    pub ws_reconnect_attempts: u32,
    /// True after the relay sent "Too many connection attempts" (v0.544). Holds the 65s back-off in
    /// place (the connection OPENS before the throttled identify, so the on-connect backoff reset
    /// would otherwise clobber it and loop). Cleared when the next retry actually fires.
    pub ws_rate_limited: bool,
    pub selected_slot: Option<usize>,
    /// Garden selection in the inventory left tree: "crop:<entity_bits>" or
    /// "tower:<id>". Drives the right detail panel for garden objects; mutually
    /// exclusive with selected_slot (selecting one clears the other).
    pub garden_selection: Option<String>,
    /// Start the inventory + garden trees COLLAPSED (operator 2026-06-08: "so when
    /// I first load the lists will start collapsed instead of expanded"). Default
    /// true; toggled by the "Start collapsed" checkbox. The Collapse/Expand-all
    /// buttons force every branch for the current frame.
    pub trees_start_collapsed: bool,
    pub fps: f32,
    pub updater: crate::updater::Updater,
    /// Set true when an update notification toast should show.
    pub update_toast_visible: bool,

    // ── Onboarding state ──

    /// Whether the user has completed first-run onboarding.
    pub onboarding_complete: bool,
    /// Current onboarding step (0 = welcome, 1 = server connect, 2 = identity, 3 = done).
    pub onboarding_step: u8,
    /// Server URL input field.
    pub server_url: String,
    /// Whether currently connected to a server.
    pub server_connected: bool,
    /// Onboarding step 1's "Connect" button (v0.643) used to just set
    /// `server_connected = true` unconditionally with no real check -- the
    /// full WS identify handshake genuinely can't happen yet at this step
    /// (the auto-connect gate in src/lib.rs requires `onboarding_complete`,
    /// and identity/pubkey isn't created until step 2), so this is instead a
    /// lightweight `/health` reachability probe, spawned on a background
    /// thread (mirrors src/updater.rs's `check_now` mpsc pattern) so the UI
    /// thread never blocks on the network. `None` = idle, `Some(rx)` =
    /// checking (poll it once per frame in `draw_step_server`).
    #[allow(clippy::type_complexity)]
    pub server_check_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Human-readable error from the last failed reachability check (empty
    /// if the last check succeeded or none has run yet).
    pub server_check_error: String,
    /// User display name input.
    pub user_name: String,
    // v0.197.0: removed `context_real`. Real/Sim toggle deleted —
    // pages commit to Real, game-mode equivalents live inside the
    // game loop (FPS) rather than as toggleable views.
    /// Whether the first-run concept tour (Onboarding page) has been
    /// completed. v0.198.0: new users land on Onboarding after identity
    /// setup so they understand what HumanityOS IS before being dropped
    /// into chat. Once they click "Open the Chat" from Onboarding
    /// (or skip it explicitly) this flips true and they go straight
    /// to Chat on subsequent launches. Existing users created before
    /// this field defaults to true via the AppConfig migration so they
    /// don't get force-routed into the tour they've never seen but
    /// don't need.
    pub concept_tour_seen: bool,
    /// Default page to load after onboarding (Chat by default).
    pub default_page: GuiPage,

    // ── Task board state ──
    pub tasks: Vec<GuiTask>,
    pub task_next_id: u32,
    pub task_search: String,
    pub task_filter_priority: Option<TaskPriority>,
    pub task_filter_assignee: String,
    pub task_show_new_form: bool,
    pub task_new_title: String,
    pub task_new_description: String,
    pub task_new_priority: TaskPriority,
    pub task_new_assignee: String,

    // ── Profile state ──
    pub profile_name: String,
    pub profile_bio: String,
    pub profile_public_key: String,
    pub profile_section: ProfileSection,
    // Body & Measurements
    pub profile_height: String,
    pub profile_weight: String,
    pub profile_eye_color: String,
    pub profile_blood_type: String,
    pub profile_hair_color: String,
    pub profile_hair_length: String,
    pub profile_hair_style: String,
    pub profile_hair_texture: String,
    pub profile_neck: String,
    pub profile_shoulders: String,
    pub profile_chest: String,
    pub profile_waist: String,
    pub profile_hips: String,
    pub profile_thighs: String,
    pub profile_inseam: String,
    pub profile_shoe_size: String,
    pub profile_shirt_size: String,
    pub profile_pants_size: String,
    // Identity
    pub profile_pronouns: String,
    pub profile_location: String,
    pub profile_website: String,
    // Private Notes
    pub profile_private_notes: String,
    // Network Profile
    pub profile_network_name: String,
    pub profile_network_bio: String,
    pub profile_network_avatar: String,
    /// Whether to appear in the server's public member directory (audit 2026-06-12
    /// opt-out). On = listed; off sends profile privacy `directory:"unlisted"`.
    /// Defaults to listed; native does not fetch server privacy, so it reflects the
    /// session's intent rather than the stored server state.
    pub profile_directory_listed: bool,
    /// Floating machine labels in the 3D home (v0.428), populated by load_world.
    pub machine_labels: Vec<MachineLabel>,
    /// Index into machine_labels of the machine the player is currently looking at
    /// within interact range (walk-up interaction, v0.431). Recomputed each frame.
    pub targeted_machine: Option<usize>,
    /// Crosshair prompt for the vehicle the player faces ("[E] drive Rover") or
    /// "[E] exit vehicle" while driving. Precomputed in lib.rs like the door prompt.
    pub vehicle_prompt: String,
    /// Crosshair prompt for the farm animal the player faces (v0.751):
    /// "[E] collect Egg (Chicken)" when ready, the regrow countdown otherwise.
    pub livestock_prompt: String,
    /// Transient collect feedback ("+1 Egg from Chicken" / "Your pack is full").
    pub livestock_notice: String,
    /// game_time when livestock_notice was set; lib.rs clears it after 3 s.
    pub livestock_notice_at: f64,
    /// Index into `door_panels` of the door CONTROL PANEL the player is looking at within
    /// arm's reach (v0.567). Drives the "[E] open/close door" prompt; E toggles the door.
    /// Recomputed each frame in first person. Mirrors `targeted_machine`.
    pub targeted_control_panel: Option<usize>,
    /// The crosshair prompt for the targeted control panel (v0.567), precomputed each frame
    /// (the HUD can't see the door's open/locked state, which lives in EngineState). Empty = none.
    pub control_panel_prompt: String,
    /// Index of the machine whose card is pinned open (toggled with E). Stays until E
    /// again or it is cleared. Survives walking away (it is the "opened station").
    pub selected_machine: Option<usize>,
    /// Pinned machine's current auto-recipe id (None when the selected machine
    /// has no AutoRefine). Published per frame by lib.rs. (v0.725)
    pub machine_card_recipe: Option<String>,
    /// Selectable same-station recipes for the pinned machine: (id, display
    /// name), sorted by name. Infinite-of-X: rows come from recipes.csv via
    /// the registry, never hardcoded. Empty when no selector applies. (v0.725)
    pub machine_card_recipe_options: Vec<(String, String)>,
    /// The player's pick from the card's recipe dropdown; lib.rs applies it to
    /// the machine entity's AutoRefine next frame and clears it. (v0.725)
    pub machine_card_recipe_pending: Option<String>,
    /// Pinned machine's container contents (item id, display name, qty) when
    /// it holds something — drives the card panel's "Take" action. (v0.731)
    pub machine_card_container: Option<(String, String, u32)>,
    /// Set by the card panel's Take button; lib.rs moves as much as fits
    /// (volume-gated) from the machine's Container into the backpack. (v0.731)
    pub machine_card_take_pending: bool,
    /// Pack items the pinned machine's container ACCEPTS (class-compatible +
    /// no-mixing respected): (item id, display name, qty carried). Drives the
    /// per-item "Store" buttons — the deposit path that completes the
    /// refinery -> pack -> genset-drum fuel handoff. (v0.733)
    pub machine_card_storable: Vec<(String, String, u32)>,
    /// Item id the player chose to Store; lib.rs moves as much as fits from
    /// the backpack into the machine's Container next frame. (v0.733)
    pub machine_card_store_pending: Option<String>,
    /// True when the pinned machine is a trading post (v0.747, ladder rung 3):
    /// the card shows a Trade button that opens the vendor modal.
    pub machine_card_vendor: bool,
    /// Vendor modal open (v0.747). Opened from the trading post's card.
    pub vendor_open: bool,
    /// The vendor's catalog: goods that exist in BOTH trade_goods.ron and
    /// items.csv, with the player-facing prices (pay / receive).
    pub vendor_goods: Vec<GuiTradeGood>,
    /// Last vendor transaction receipt or refusal.
    pub vendor_status: String,
    /// The player's live credit balance, bridged from the Wallet each frame.
    pub wallet_credits: i64,
    /// Buy/Sell clicked this frame: (item id, quantity). lib.rs settles them
    /// against the ECS inventory + Wallet via economy::vendor_buy/vendor_sell.
    pub pending_vendor_buy: Option<(String, u32)>,
    pub pending_vendor_sell: Option<(String, u32)>,
    /// Equip/unequip intents (v0.750, ladder rung 8): item id to wear / slot
    /// id to clear. The frame bridge moves items between the pack and the
    /// ECS Outfit (slot validated against equipment.csv).
    pub pending_equip: Option<String>,
    pub pending_unequip: Option<String>,
    /// Last equip action's receipt or refusal, shown in the Equipment section.
    pub equip_status: String,
    /// Room volumes (v0.429), for room-based label occlusion: which room is the camera in.
    pub room_bounds: Vec<RoomBounds>,
    /// Hold-Tab "reveal" peek (v0.429): triples the label distances and shows labels
    /// through walls across all owned/explored rooms. True only while Tab is held.
    pub reveal_held: bool,
    /// Distance (meters) at which a machine's DOT appears (the coarsest LOD).
    pub machine_label_dot_dist: f32,
    /// Distance (meters) at which a machine's NAME appears (closer than the dot).
    pub machine_label_name_dist: f32,
    /// Distance (meters) at which the full stat CARD appears (closest). Hold Tab x3 all.
    pub machine_label_card_dist: f32,
    /// Crew NPC nameplates (v0.667): name + live chore label floating over each
    /// relay-driven crew member. Rebuilt every frame from the RemoteNpc components
    /// (lib.rs, just before hud::draw), so it is always in sync with the amber figures.
    pub crew_labels: Vec<CrewLabel>,
    /// Transient confirmation shown after a "Save to server" click.
    pub profile_network_saved_note: String,
    // Interests
    pub profile_interests: Vec<String>,
    pub profile_interest_input: String,
    // Skills
    pub profile_skills: Vec<(String, f32)>,
    // Social Links
    pub profile_social_links: Vec<(String, String)>,
    pub profile_social_platform: String,
    pub profile_social_url: String,
    // Streaming
    pub profile_streaming_url: String,
    pub profile_streaming_live: bool,

    // ── Map state ──
    pub map_planets: Vec<GuiPlanet>,
    /// Seeded entities (You, your home, a vehicle, …) — each a container with its
    /// own contents + optional location. Loaded at startup; the inventory view
    /// renders them as top-level nodes, injecting live items at kind:"backpack".
    /// Empty until loaded. See [`Place`].
    pub places: Vec<Place>,
    /// The loaded Fibonacci homestead blueprint (the first offline "Design"),
    /// browsed on the Home page. None if the blueprint file is absent.
    pub homestead_design: Option<HomesteadDesign>,
    /// The homestead's self-sufficiency loops (energy/water/food/nutrients) from
    /// data/machines/home.ron, rendered as the closure summary on the Home page (v0.432).
    pub homestead_loops: Vec<crate::machines::HomeLoop>,
    /// The curated aeroponic tower configs (nutrition + apothecary), browsed on the
    /// Home page. Empty if data/towers/aeroponic_configs.ron is absent.
    pub tower_configs: Vec<TowerConfig>,
    /// Garden grow-areas (towers/beds/racks/tanks/fields) from home.ron, loaded via
    /// the resolved data_dir, for the Inventory Garden overview + per-medium edit modal.
    pub garden_areas: Vec<GardenArea>,
    /// Grow-media registry (data/garden/grow_media.ron) — the per-medium edit form is
    /// rendered from this, so adding a plot-type is a data edit (infinite-of-X).
    pub grow_media: Vec<GrowMedium>,
    /// Per-area irrigation targets the garden edit modal publishes to the sim, keyed by
    /// tower_id (e.g. "nutrition" -> 0.0..=1.0 water level). lib.rs bridges this into the
    /// DataStore's "garden_irrigation" channel each frame; FarmingSystem tops matching
    /// crops up to the target. Empty = no automated irrigation. Snapshotted by the
    /// inventory page each frame from the garden edit configs.
    pub garden_irrigation: std::collections::HashMap<String, f32>,
    /// Per-area nutrient strength the garden edit modal publishes (tower_id -> 0..1).
    /// Bridged to the DataStore's "garden_nutrient" channel; FarmingSystem scales
    /// matching crops' growth speed by it. Sibling of `garden_irrigation`.
    pub garden_nutrient: std::collections::HashMap<String, f32>,
    /// Organize-layer inventory pool: every seeded item tagged with its container path,
    /// so the nested-container inventory can move items between containers. Seeded from
    /// `places` at startup via `flatten_placed_items`. The live backpack is separate.
    pub placed_items: Vec<PlacedItem>,
    /// Pending backpack <-> container transfers (item_id, qty, is_add). The inventory
    /// page pushes these when an item moves into/out of the live backpack; lib.rs drains
    /// them into the InventorySystem channel each frame. is_add => add to the backpack.
    pub pending_inventory_transfers: Vec<(String, u32, bool)>,
    /// Per-tower shared-reservoir compatibility (parallel to `tower_configs`),
    /// computed once from the plant registry in the crop sync. The "make sure
    /// they grow together" check shown on the Home page.
    pub tower_compat: Vec<TowerCompat>,
    /// Creative mode (default ON during early dev): resource-consuming actions
    /// (planting seeds, fertilizing, crafting) skip the inventory requirement and
    /// consumption, so the seed/material economy can be built out before it bites.
    /// OFF = survival (consume normally). Bridged to the DataStore each frame so the
    /// farming + crafting systems read it. Not persisted yet: defaults Creative on
    /// every launch, which is exactly the early-dev default the operator wants.
    pub creative_mode: bool,
    /// Which section the merged Real tab shows — either a Profile section id
    /// ("body"/"identity"/"notes"/…) or a page id ("inventory"/"wallet"/
    /// "tasks"/"maps"/"market"). Drives `real::draw`'s delegate.
    pub active_real_section: String,
    /// Selected section for the folded Platform tab ("settings"/"recovery"/
    /// "tools"/"bugs"/"testing"/"browser").
    pub active_platform_section: String,
    /// Selected section for the folded Humanity tab ("civilization"/
    /// "governance"/"identity"/"onboarding"/"donate"/"resources").
    pub active_humanity_section: String,
    pub map_selected_planet: Option<usize>,
    pub map_zoom: f32,

    // ── Market state ──
    /// Abilities panel rows (v0.753), rebuilt while the Profile page is open.
    pub abilities: Vec<GuiAbility>,
    /// Cast click -> lib.rs bridges it into the ability_request channel.
    /// (ability id, optional target entity bits - the faced creature, v0.760).
    pub pending_cast: Option<(String, Option<u64>)>,
    /// One-line cast feedback from AbilitySystem ("First Aid restores 35 health").
    pub ability_status: String,
    /// game_time when ability_status was set; lib.rs fades it after a few
    /// seconds so the HUD line does not linger forever. (v0.754)
    pub ability_status_at: f64,
    /// F pressed: swing the held tool (or bare hands) at the faced creature.
    pub pending_swing: bool,
    /// game_time when the next swing is allowed (0.8s between swings). (v0.765)
    pub swing_ready_at: f64,

    pub listings: Vec<GuiListing>,
    /// Set once listing_browse has been sent this connection; cleared on
    /// disconnect so a reconnect re-syncs. (v0.752)
    pub listings_synced: bool,
    /// One-line market feedback ("Listing published", errors).
    pub listing_status: String,
    pub listing_search: String,
    pub listing_filter_category: String,
    /// Detail-view selection by LISTING ID (not index: live broadcasts
    /// reorder the vector under the open detail view).
    pub listing_selected: Option<String>,
    /// Reviews for the OPEN detail listing (v0.755), fetched over REST.
    pub listing_reviews: Vec<GuiReview>,
    /// Which listing the loaded reviews belong to ("" = none loaded).
    pub listing_reviews_for: String,
    /// In-flight REST fetch of the reviews list.
    pub listing_reviews_rx:
        Option<std::sync::mpsc::Receiver<Result<(Vec<GuiReview>, f32, i64), String>>>,
    pub listing_reviews_avg: f32,
    pub listing_reviews_count: i64,
    /// Review form drafts (detail view).
    pub review_rating_draft: i32,
    pub review_comment_draft: String,
    /// Buyer-seller thread for the OPEN detail listing (v0.755).
    pub listing_thread: Vec<GuiListingMsg>,
    pub listing_thread_for: String,
    pub listing_thread_open: bool,
    pub listing_msg_draft: String,

    /// P2P trades (v0.756), delivered via targeted private wrappers.
    pub trades: Vec<GuiTrade>,
    /// Set once trade_list_request has been sent this connection.
    pub trades_synced: bool,
    /// One-line trade feedback ("Trade completed", errors).
    pub trade_status: String,
    pub listing_show_new_form: bool,
    pub listing_new_title: String,
    pub listing_new_description: String,
    pub listing_new_price: String,
    pub listing_new_category: String,

    // ── Calculator state ──
    pub calc_display: String,
    pub calc_expression: String,
    pub calc_history: Vec<String>,

    // ── Calendar state ──
    pub cal_year: i32,
    pub cal_month: u32,
    pub cal_selected_day: u32,
    pub cal_events: Vec<GuiCalendarEvent>,
    pub cal_new_title: String,
    pub cal_new_time: String,
    pub cal_new_color: egui::Color32,

    // ── Notes state ──
    pub notes: Vec<GuiNote>,
    pub notes_selected: Option<u64>,
    pub notes_next_id: u64,

    // ── Civilization state (live from GET /api/civilization, v0.757) ──
    /// The relay's aggregated community stats; None until the fetch lands.
    pub civ_stats: Option<GuiCivStats>,
    pub civ_stats_loaded: bool,
    pub civ_stats_rx: Option<std::sync::mpsc::Receiver<Result<GuiCivStats, String>>>,
    pub civ_status: String,

    // ── Wallet state ──
    pub wallet_balance: f64,
    pub wallet_address: String,
    pub wallet_network: WalletNetwork,
    pub wallet_send_to: String,
    pub wallet_send_amount: String,
    pub wallet_transactions: Vec<WalletTransaction>,
    pub wallet_sol_price: f64,

    // ── Crafting state ──
    pub craft_recipes: Vec<GuiRecipe>,
    pub craft_selected: Option<usize>,
    pub craft_selected_category: Option<String>,
    pub craft_status: String,
    /// Buildable blueprints (data/blueprints/basic.ron), bridged from the
    /// blueprint_registry for the Crafting page's Structures section
    /// (v0.746, closure ladder rung 2).
    pub blueprints: Vec<GuiBlueprint>,
    /// Blueprint id the player clicked Build on this frame; lib.rs turns it
    /// into a "build_request" at a spot in front of the camera.
    pub pending_build: Option<String>,
    /// ConstructionSystem's honest status line ("need 4x wood_plank to build
    /// Wooden Wall", "Building...", "... complete"), synced each frame.
    pub build_status: String,
    /// Recipe id the player clicked "Craft" on this frame; the main loop bridges it
    /// to the ECS CraftingSystem. None = nothing pending.
    pub pending_craft_recipe: Option<String>,
    /// Dev/creative provisioning request: stock the player with one stack of every
    /// recipe input (raws + intermediates) so every recipe is craftable immediately.
    pub dev_stock_materials: bool,

    // ── Survival / nutrition state ──
    /// Item id the player clicked "Eat" on this frame; the main loop bridges it to
    /// FoodSystem's consume channel. None = nothing pending.
    pub pending_consume_item: Option<String>,
    /// Item id the player clicked "Drink" on this frame → FoodSystem (restores hydration).
    pub pending_drink_item: Option<String>,
    /// True for the frame the player clicked "Rest" → refills energy via FoodSystem.
    pub pending_rest: bool,
    /// True for the frame the player clicked "Compost" → waste→fertilizer via FoodSystem.
    pub pending_compost: bool,
    /// Crop entity bits the player clicked "Fertilize" on this frame → FarmingSystem.
    pub pending_fertilize_crop: Option<u64>,
    /// Player vitals (satiation/hydration + active status effects), synced from the
    /// ECS each frame for the HUD / inventory page to display.
    pub vitals: GuiVitals,

    /// Vehicle KIT item id the player clicked "Deploy" on this frame → bridged
    /// into the "deploy_kit_request" channel for VehicleSystem (economy Phase 2
    /// Stage 1: the kit unpacks into a real Vehicle entity in front of the player).
    pub pending_deploy_kit: Option<String>,

    // ── Gardening state ──
    /// Seed item id the player clicked "Plant" on this frame → FarmingSystem.
    pub pending_plant_seed: Option<String>,
    /// GUI -> ECS: plant a whole aeroponic tower. The Vec is the plant ids (one per
    /// slot) to spawn as CropInstances; drained into the "plant_tower_request"
    /// channel for FarmingSystem (v0.386). Dev-friendly: no seed consumption yet.
    pub pending_plant_tower: Option<(String, Vec<String>)>,
    /// GUI -> ECS: plant a bed/tray/field grow area (v0.738 grain loop).
    /// (machine_id, plant_id, unit count) — one CropInstance per unit, tagged
    /// with the machine id as its grow-area; drained into "plant_bed_request".
    pub pending_plant_bed: Option<(String, String, u32)>,
    /// Seed item ids to grant the player (the "Dev: stock seeds" starter set);
    /// drained into "stock_seeds_request" for FarmingSystem.
    pub pending_stock_seeds: Option<Vec<String>>,
    /// Crop entity bits the player clicked "Water" on this frame.
    pub pending_water_crop: Option<u64>,
    /// Crop entity bits the player clicked "Harvest" on this frame.
    pub pending_harvest_crop: Option<u64>,
    /// Bulk harvest (v0.739): every mature crop's bits from a Garden group's
    /// "Harvest N ready" button; drained into "harvest_many_request".
    pub pending_harvest_many: Vec<u64>,
    /// Dev: instantly mature all crops (testing affordance, like dev-stock).
    pub dev_grow_crops: bool,
    /// Growing crops, synced from the ECS each frame for the Garden panel.
    pub crops: Vec<GuiCrop>,

    // ── Mining / drones state ──
    /// Set the frame the player clicks "Launch drone" → bridged to DroneSystem's
    /// commission channel: `(target asteroid id, manifest)`. One asteroid per run.
    pub pending_drone_manifest: Option<(String, Vec<(String, u32)>)>,
    /// "Keep mining" (economy automation, v0.663): while true, a launched drone
    /// order becomes a STANDING order that auto-relaunches the same trip each
    /// time a haul is delivered, until the asteroid depletes or this goes false.
    pub auto_mine_enabled: bool,
    /// The most recently launched drone order, kept so RE-CHECKING "Keep mining"
    /// mid-flight (after having unchecked it) can re-arm the standing order --
    /// the checkbox otherwise only took effect at launch time (review fix).
    pub last_drone_order: Option<(String, Vec<(String, u32)>)>,
    /// Previous frame's `auto_mine_enabled`, for rising-edge detection in the
    /// lib.rs bridge.
    pub prev_auto_mine_enabled: bool,
    /// True while a drone is in flight (synced) — one drone per player, so the panel
    /// shows the active drone instead of the builder + disables Launch.
    pub drone_active: bool,
    /// Asteroids (name + remaining ore), synced from the ECS for the Mining panel.
    pub asteroids: Vec<GuiAsteroid>,
    /// Active mining drones (ore / phase / cargo), synced from the ECS.
    pub drones: Vec<GuiDrone>,
    /// World vehicles (Stage 3, v0.680), synced from the ECS each frame for the
    /// Inventory page's Vehicles section.
    pub vehicles: Vec<GuiVehicle>,
    /// GUI -> ECS: summon this vehicle (entity bits) to drive itself to the
    /// player; bridged into the "summon_vehicle" channel.
    pub pending_summon_vehicle: Option<u64>,
    /// Cosmos -> Inventory jump (unified map slice 2): an asteroid id whose
    /// mining modal should open when the Inventory page next draws.
    pub pending_open_mining_modal: Option<String>,
    /// GUI -> engine: chase-cam this vehicle (entity bits) while it self-drives;
    /// any WASD input or arrival breaks the follow (Stage 3, v0.690).
    pub pending_follow_vehicle: Option<u64>,
    /// Live production status lines (v0.681), one per auto machine, synced from
    /// CraftingSystem's "auto_craft_status" channel each frame -- e.g.
    /// "Assemble Rover — 42%" or "Smelt Iron — waiting for Iron Ore x2".
    pub factory_status: Vec<String>,

    // ── Skills / progression state ──
    /// Player skills (live level + XP), synced from the ECS PlayerSkills each
    /// frame for the profile Skills panel. Empty until the first XP is earned.
    pub skills: Vec<GuiSkill>,
    /// Dev: max all skills next frame (testing affordance under #8b skill-gating).
    pub pending_dev_max_skills: bool,
    /// Player quests (active + completed), synced from the ECS QuestTracker each
    /// frame for the profile Quests panel.
    pub quests: Vec<GuiQuest>,
    /// Acceptable-but-unaccepted quests (v0.747.x, rung 4). Before this, quests
    /// with no prerequisite were unreachable authored content.
    pub quests_available: Vec<GuiAvailableQuest>,
    /// Quest id the player clicked Accept on; the frame bridge applies it to
    /// the ECS QuestTracker.
    pub pending_accept_quest: Option<String>,

    // ── Guilds state (live from the relay's REST guild API, v0.757) ──
    pub guilds: Vec<GuiGuild>,
    /// Selection by GUILD ID (refetches reorder the vector).
    pub guild_selected: Option<String>,
    pub guild_search: String,
    pub guild_show_create: bool,
    pub guild_new_name: String,
    pub guild_new_desc: String,
    pub guild_new_color: egui::Color32,
    /// Set once the list fetch has run this session; cleared to refetch.
    pub guilds_loaded: bool,
    /// In-flight list fetch (all guilds + my memberships merged).
    pub guilds_rx: Option<std::sync::mpsc::Receiver<Result<Vec<GuiGuild>, String>>>,
    /// In-flight create/join/leave action; Ok(msg) triggers a refetch.
    pub guild_action_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    /// One-line guild feedback ("Joined", errors).
    pub guild_status: String,
    /// Member list for the OPEN guild detail: (display name, role).
    pub guild_members: Vec<(String, String)>,
    pub guild_members_for: String,
    pub guild_members_rx:
        Option<std::sync::mpsc::Receiver<Result<Vec<(String, String)>, String>>>,

    // ── Bridged game state (written by lib.rs each frame) ──

    /// Player health fraction (0.0 to 1.0). Updated from ECS Health component.
    pub player_health: f32,
    /// Player max health. Updated from ECS Health component.
    pub player_health_max: f32,
    /// Inventory items from the player entity's Inventory component.
    pub inventory_items: Vec<Option<GuiItemSlot>>,
    /// Total inventory slot count.
    pub inventory_max_slots: usize,
    /// Current game time snapshot.
    pub game_time: Option<GuiGameTime>,
    /// Current weather snapshot.
    pub weather: Option<GuiWeather>,
    /// Live home electrical readout (watts), mirrored from ElectricalSystem each frame.
    pub power_generation: f32,
    pub power_consumption: f32,
    pub power_balance: f32,
    /// Live battery state (v0.473): total charge, capacity (watt-hours), and hours of autonomy.
    pub power_battery_wh: f32,
    pub power_battery_capacity_wh: f32,
    pub power_autonomy_hours: f32,
    /// Live home WATER readout (v0.608), mirrored from PlumbingSystem each frame: production + demand
    /// (L/min), stored + capacity (litres), and days of autonomy at the current demand.
    pub water_production_lpm: f32,
    pub water_demand_lpm: f32,
    pub water_stored_l: f32,
    pub water_capacity_l: f32,
    pub water_days_autonomy: f32,
    /// Live home AIR readout (v0.617), mirrored from AtmosphereSystem each frame: O2/CO2 percent, total
    /// pressure (atm), temperature (C), and whether the mix is breathable.
    pub air_o2_pct: f32,
    pub air_co2_pct: f32,
    pub air_pressure_atm: f32,
    pub air_temp_c: f32,
    pub air_breathable: bool,
    /// Character-select showroom (v0.441): when active, the home is hidden and the avatar is
    /// previewed against a backdrop with an orbit camera + the customization panel.
    pub showroom_active: bool,
    /// Construction mode (v0.453): show the home's roof/ceiling. Default OFF so the sky
    /// (stars + the real solar system) stays visible from inside; toggle on for atmosphere
    /// tests or a sealed look.
    pub show_roof: bool,
    /// Hull wrap (ship-superstructure increment D): show the generated exterior hull around
    /// the whole zone cluster. Default ON so the ship reads as a vessel from outside; toggle
    /// off (H in-world, or the Settings checkbox) for unobstructed interior screenshots or a
    /// clear top-down build view. Glass roofs keep their starlight either way (the hull top is
    /// cut open above them).
    pub show_hull: bool,
    /// Construction EDITOR (v0.455): when active, a panel lets the player set each room's
    /// per-wall kind + the uniform height and rebuild the home live. Toggled with B in-world.
    pub construction_active: bool,
    /// The home's MACHINE layout (data/machines/home.ron), loaded at startup + EDITABLE in
    /// the construction editor (v0.519: machine placement -- the #1 home-design parity gap).
    /// The AI edits the same file by hand, so an AI-placed machine is player-editable and
    /// vice versa. Saved by `home_machines_save` alongside the room layout. See
    /// docs/design/home-design.md. None if home.ron is absent.
    pub home_machines: Option<crate::machines::MachineHome>,
    /// The editor's Add-Machine picker selection (a catalog type id). (v0.519)
    pub home_machine_add_type: String,
    /// The editor's Add-Connection pickers: from-machine id, to-machine id, and kind. (v0.523,
    /// Stage 2: players wire machines -- power/water/etc. -- the same connections the AI authors.)
    pub home_conn_from: String,
    pub home_conn_to: String,
    pub home_conn_kind: String,
    /// Conduit node-graph branch picker (v0.581): the from/to endpoints ("m:id" machine or "n:id" node)
    /// + the kind for the Branch button that adds a conduit edge.
    pub conduit_from: String,
    pub conduit_to: String,
    pub conduit_kind: String,
    /// Construction text-command console (v0.578): the input line + the last result/output. An
    /// AI-enumerable ACT surface -- the same struct mutations the gizmos make, driven by typed verbs.
    pub construction_console_input: String,
    pub construction_console_output: String,
    /// Set by the editor's Save to write home_machines back to home.ron (mirrors
    /// `construction_save` for rooms). The engine clears it after writing.
    pub home_machines_save: bool,
    /// Transient "Saved …" confirmation under the editor's Save button —
    /// set by the engine after the write lands so saving is never silent
    /// (v0.735, operator: "the save home button doesn't do anything").
    pub construction_save_note: String,
    /// Editable mirror of the layout's rooms (walls + position + size). The engine fills this
    /// when the editor opens and reads it back when `construction_dirty`. (v0.459)
    pub construction_rooms: Vec<ConstructionRoom>,
    /// Room-type ids from the registry, for the Add-Room picker (sorted, stable). (v0.459)
    pub construction_room_types: Vec<String>,
    /// Current Add-Room picker selection. (v0.459)
    pub construction_add_type: String,
    /// A room index the panel requested to delete; applied after the scroll loop. (v0.459)
    pub construction_remove: Option<usize>,
    /// Show the top-down floor-plan overlay (v0.464). Default OFF: construction uses the free
    /// orbit "astral" camera (drag/pan/dolly/fly); toggle on for the 2D plan when wanted.
    pub construction_plan_view: bool,
    /// Keymap reference (v0.465): true while F1 is held; shows the bindings for the current
    /// screen/mode. Loaded once from data/keymaps.ron.
    pub keymap_visible: bool,
    pub keymaps: Vec<crate::gui::pages::keymap::KeymapContext>,
    /// Diagnostics dev-HUD overlays (v0.482), each toggled by an F-key and shown
    /// stacked in the top-right corner. F2 = performance, F3 = network, F4 =
    /// system. Listed in the F1 keymap so they are discoverable.
    pub show_perf_overlay: bool,
    pub show_network_overlay: bool,
    pub show_system_overlay: bool,
    /// Recent frame times in milliseconds (ring buffer, newest last), for the
    /// performance overlay's frame-time sparkline. Capped at ~120 samples.
    pub frame_times: Vec<f32>,
    /// Count of WebSocket messages received this session (network overlay).
    pub ws_msgs_in: u64,
    /// Settings audio (v0.485). mic_test_active is the toggle: while true, lib.rs
    /// keeps a mic to Opus to speaker loopback running so you can confirm audio
    /// works. The device fields are the chosen input/output (empty = system
    /// default); the *_devices lists are cached (enumerating cpal every frame is
    /// slow), refreshed on demand. mic_meter is a decayed level for the meter.
    pub mic_test_active: bool,
    pub audio_input_device: String,
    pub audio_output_device: String,
    pub audio_input_devices: Vec<String>,
    pub audio_output_devices: Vec<String>,
    pub audio_devices_loaded: bool,
    pub mic_meter: f32,
    /// Previous mic_test_active, so lib.rs starts/stops the loopback only on the
    /// toggle EDGE (not every frame, which would spin-retry a failing start).
    pub mic_test_prev: bool,
    // ── v0.488 voice input prefs (persisted via AppConfig) ──────────────
    /// Mic input gain, 1.0 = 100%. Range 0.0..=2.0 (200%).
    pub voice_gain: f32,
    /// Noise filter applied to the mic before encode.
    pub voice_filter_mode: crate::config::VoiceFilterMode,
    /// When the mic is actually transmitted (open mic / PTT / VAD / push-to-mute).
    pub voice_transmit_mode: crate::config::VoiceTransmitMode,
    /// The push key (egui Key name) for PTT / push-to-mute.
    pub voice_ptt_key: String,
    /// Voice-activation RMS threshold (0.0..=1.0).
    pub voice_vad_threshold: f32,
    /// Runtime: is the push key currently held this frame? Set by lib.rs from
    /// the input state; gates transmit for PTT / push-to-mute. Not persisted.
    pub voice_ptt_held: bool,
    /// Runtime: the settings UI is waiting for the user to press a key to bind
    /// as the push key. Not persisted.
    pub voice_binding_key: bool,
    /// Live diagnostics sampled from EngineState each frame (only while the
    /// relevant overlay is open, so they cost nothing when hidden). entity_count
    /// = ECS entities, mem_mb = process RSS, uptime_secs = since launch.
    pub diag_entity_count: usize,
    pub diag_mem_mb: f32,
    pub diag_uptime_secs: u64,
    /// Index into construction_rooms of the room selected/grabbed in the 3D astral editor, for
    /// the highlight tint. None = no selection. (v0.466)
    pub construction_selected_room: Option<usize>,
    /// Editable uniform ceiling height (mirrors layout.default_wall_height).
    pub construction_height: f32,
    /// The active storey the editor is focused on (v0.471). The room tree filters to this level
    /// and new rooms are created on it; the level stepper per room moves a room between storeys.
    pub construction_level: i32,
    /// Set by the panel when a wall/position/size/add/remove changed -> the engine rebuilds.
    pub construction_dirty: bool,
    /// Set by the panel on a MACHINE edit (offset / add / remove / connect) -> the engine refreshes
    /// just the machine meshes live, no full room rebuild. (v0.525)
    pub construction_machines_dirty: bool,
    /// Footer placement-palette state (v0.527): the selected category tab, whether the grid is
    /// expanded (1 row -> multi-row).
    pub construction_palette_category: String,
    pub construction_palette_expanded: bool,
    /// The machine type currently "held" for placement (v0.529): the palette puts a type here, the
    /// editor renders it as a ghost following the cursor + drops it where you click the floor (click
    /// the same item again, or Escape/right-click, to cancel). None = not placing.
    pub construction_place_type: Option<String>,
    /// The STRUCTURAL piece type currently "held" for placement (v0.583): set by the "Structure"
    /// palette category, dropped where you click the floor. Mutually exclusive with
    /// construction_place_type (a machine) + wall_mode. None = not placing a structure.
    pub construction_structure_type: Option<String>,
    /// Place-a-CONDUIT-NODE mode (v0.629): when true, clicking the floor in the 3D view drops a pipe-graph
    /// junction node there (a "main line" point you then drag machine ports onto). Toggled from the Conduit
    /// nodes panel; right-click cancels. Mutually exclusive with the other place modes.
    pub construction_place_conduit_node: bool,
    /// Zone-type id selected in the "Add zone" picker (v0.631, superstructure M1).
    pub zone_add_type: String,
    /// Selected ZONE id (v0.634): picked in the 3D view -> its detail shows on the right + it highlights;
    /// draggable on the floor. Not serialized (a pure selection). None when no zone is selected.
    pub construction_zone_selected: Option<String>,
    /// Rail-graph add-edge picker endpoints (v0.635, superstructure M2).
    pub rail_edge_from: u32,
    pub rail_edge_to: u32,
    /// Index of the placed structure selected in the editor (its detail shows on the right). (v0.583)
    pub construction_structure_selected: Option<usize>,
    /// Camera FOCUS request (v0.593): set to a world (x,y,z) when a left-list row is double-clicked;
    /// the engine snaps the orbit camera to it next frame (so you can see what you clicked) + clears it.
    pub construction_focus_request: Option<(f32, f32, f32)>,
    /// Objects-browser filter text (v0.598): rows whose name contains it (case-insensitive) show;
    /// empty = all. With a non-empty filter, matching type-groups auto-expand.
    pub construction_object_filter: String,
    /// Multi-select set for the object browser (v0.612): Ctrl+click a row to add/remove it; the set is
    /// keyed by a stable "tag:id" string (e.g. "Machine:tower_0", "Wall:3"). Drives group delete +
    /// group nudge. Empty = normal single-selection.
    pub construction_multi: std::collections::HashSet<String>,
    /// LOCKED object-type tags (v0.614): a type in this set ("Wall"/"Struct"/"Machine"/"Light"/"Road"/
    /// "Pipe") can't be selected or grabbed in the viewport -- so you can lock your walls while arranging
    /// machines and never fat-finger them. Toggled per type-group in the object browser.
    pub construction_locked_types: std::collections::HashSet<String>,
    /// Per-type HIDE set (v0.636): object types ("Machine"/"Wall"/"Pipe"/"Zone"/...) whose meshes +
    /// gizmos are skipped in the 3D view (and can't be picked), to declutter a busy build. Mirrors the
    /// lock set; not serialized (a pure view toggle).
    pub construction_hidden_types: std::collections::HashSet<String>,
    /// Selected ROAD-graph node id (v0.597): its detail shows on the right; draggable in the viewport.
    pub construction_road_node_selected: Option<u32>,
    /// Selected CONDUIT-graph node id (v0.597): its detail shows on the right; draggable in the viewport.
    pub construction_conduit_node_selected: Option<String>,
    /// Selected machine-machine CONNECTION (v0.626): (from id, to id) of the pipe/wire picked in the 3D
    /// view, so a connection is a first-class clickable object (detail + Remove on the right panel) like
    /// walls/doors. None when no pipe is selected.
    pub construction_connection_selected: Option<(String, String)>,
    /// Yaw (degrees) applied to the next placed structure -- rotate the held piece with [ and ].
    pub construction_structure_yaw: f32,
    /// Height above the room floor (metres) the next placed structure drops at (v0.588): 0 = on the
    /// floor; set it to a staircase's top so a deck lands as an upper-level landing.
    pub construction_structure_place_y: f32,
    /// Road-graph editor form state (v0.586): the from/to nodes, class, and width for the next edge.
    pub construction_road_from: u32,
    pub construction_road_to: u32,
    pub construction_road_class: String,
    pub construction_road_width: f32,
    /// Dimension overlay toggle (v0.595): the floating measurement text (wall lengths, corner angles,
    /// feature gaps). Default on; turn off from the Options/Dev section to de-clutter the view.
    pub construction_dimension_overlay: bool,
    /// Master toggle for the build-mode HELPER overlays (v0.587): the non-interactive bounds gizmos on
    /// machines + structures, the road graph (node rings + edge lines), and conduit-node markers.
    /// Default on. The interactive editing handles (corner orbs, resize cubes) + the light gizmos (the
    /// diamond is clickable) are always shown -- this only quiets the passive helpers.
    pub construction_show_helpers: bool,
    /// The SHIP: many pressurized zones, each a fixed outer box + freely-designed interior walls
    /// (v0.754, ship-superstructure increment A; the single-home model was v0.534). The editor
    /// edits the zone selected by `construction_zone` (via `ship::ship_structure::zone_body[_mut]`,
    /// free functions on this field so sibling GuiState fields stay borrowable); the engine renders
    /// ALL zones (load_world + rebuild_homestead via `ShipStructure::generate_meshes`) instead of
    /// the old room-AABB layout when present. Loaded in load_world (with one-time adoption of a
    /// legacy home_structure.ron).
    pub ship_structure: Option<crate::ship::ship_structure::ShipStructure>,
    /// Index into `ship_structure.zones` of the zone the construction editor is EDITING (the zone
    /// selector at the top of the Home structure panel, v0.754). All viewport tools + panel edits
    /// operate on this zone; gizmos and ray picks convert between world and zone-local space
    /// through its origin.
    pub construction_zone: usize,
    /// Two-click delete confirm for the zone selector's "Delete zone" (v0.754): armed by the first
    /// click, executed by "Confirm delete", cleared on cancel / zone switch.
    pub construction_zone_delete_arm: bool,
    /// Corridor add-flow state (ship-superstructure increment B, the "Corridors" section under the
    /// zone selector): pending from/to zone indices + door indices (into `zone_door_refs` order),
    /// tube width, glass-top flag, and the last validation error ("" = none) shown under Create.
    pub construction_corridor_from_zone: usize,
    pub construction_corridor_from_door: usize,
    pub construction_corridor_to_zone: usize,
    pub construction_corridor_to_door: usize,
    pub construction_corridor_width: f32,
    pub construction_corridor_glass: bool,
    pub construction_corridor_error: String,
    /// Set by the panel on an interior-wall edit (add / remove / move corner / opening) -> the
    /// engine rebuilds the home mesh and writes ship_structure.ron on Save. (v0.534)
    pub construction_structure_dirty: bool,
    /// Wall-drawing mode (v0.534): true while the "Add wall" tool is active. Click the floor to
    /// drop corner nodes; the first click sets `construction_wall_start`, the second adds the wall
    /// segment and chains (start = the new corner). Escape / right-click exits.
    pub construction_wall_mode: bool,
    /// The pending first corner (x, z metres from the box min corner) while drawing a wall. (v0.534)
    pub construction_wall_start: Option<(f32, f32)>,
    /// The cursor's current floor position (box-local x, z) in build mode (v0.545), set by the engine
    /// each frame so the dimension overlay can show the live segment length + cursor readout.
    pub construction_cursor_world: Option<(f32, f32)>,
    /// Index of the interior wall currently selected in the editor (for remove / opening edits), or
    /// None. (v0.534)
    pub construction_wall_selected: Option<usize>,
    /// Id of the machine currently selected in the editor (clicked in the viewport or the list), or
    /// None. Mutually exclusive with construction_wall_selected -- the right panel shows whichever is
    /// set. (v0.553)
    pub construction_machine_selected: Option<String>,
    /// Index into home_structure.lights of the light selected in the editor (clicked its diamond gizmo),
    /// or None. The right panel shows its detail. Mutually exclusive with wall/machine selection. (v0.576)
    pub construction_light_selected: Option<usize>,
    /// Where the player avatar stands in BUILD mode (x, z in box coords), draggable by its pyramid
    /// gizmo. Leaving build mode drops you into first person right here. (v0.557)
    pub build_char_pos: Option<(f32, f32)>,
    /// Snap wall corners to a 0.25 m grid while drawing + dragging (v0.541). Endpoint snapping (to
    /// the box edges + other corners) is always on for airtight seals; this toggles the grid.
    pub construction_grid_snap: bool,
    /// Dev overlay (v0.547): when on, the build widgets (dimension overlay + door interaction rings)
    /// stay visible in NORMAL PLAY, not just in the construction editor. Toggled in the wall editor.
    pub construction_dev_overlay: bool,
    /// Global illumination master switch (v0.571): when FALSE, the sun + fill directional lights are
    /// zeroed so a room is lit ONLY by local placed lights -- the "turn off GI and still see" test.
    /// Default true. Toggled in the wall editor.
    pub gi_enabled: bool,
    /// Sun-position override for the construction editor (operator: the real
    /// astronomical sun direction is tied to Earth's slow orbital drift + a
    /// FIXED ship position that never rotates, so there is no way to get a
    /// better lighting angle while editing -- "the sun is low on the horizon
    /// and I can't roll the ship to change that"). When true (and only while
    /// `construction_active`), the celestial-pass sun direction/color use
    /// `construction_sun_override_hour` via `TimeSystem::sun_direction`/
    /// `sun_color` instead of the real astronomical vector, so a bad real-world
    /// sun angle never blocks seeing your own build. Does not affect normal
    /// gameplay lighting. Default false.
    pub construction_sun_override: bool,
    /// Hour of day (0.0..24.0) used when `construction_sun_override` is on.
    /// Default noon (12.0) -- the best overhead angle for construction work.
    pub construction_sun_override_hour: f32,
    /// Max construction-editor undo steps (v0.575), Blender-style configurable depth. Set in the wall
    /// editor. Default 64; clamped 1..=4096.
    pub construction_undo_depth: usize,
    /// Set by the panel's Save button -> the engine writes the layout back to the RON.
    pub construction_save: bool,
    /// Index into the backdrop list (the names mirror is `showroom_backdrop_names`).
    pub showroom_backdrop: usize,
    /// Backdrop display names, mirrored from the loaded registry for the panel.
    pub showroom_backdrop_names: Vec<String>,
    /// Set true by the "Enter your home" button; the main loop consumes it to leave the
    /// showroom (write appearance/outfit to the player, save, switch to first-person).
    pub showroom_confirm: bool,
    /// The avatar appearance being edited (the live preview source). Synced to the ECS
    /// player Appearance on confirm so it persists in the save.
    pub appearance: crate::ecs::components::Appearance,
    /// Set when the appearance edits change; the main loop rebuilds the avatar mesh.
    pub appearance_dirty: bool,
    /// The equipped cosmetic outfit being edited in the wardrobe (synced to the ECS player
    /// on confirm). slot id -> cosmetic id.
    pub outfit: crate::ecs::components::Outfit,
    /// Set when the outfit changes; the main loop rebuilds the avatar.
    pub outfit_dirty: bool,
    /// Which showroom panel is shown: 0 = character select (spawn), 1 = appearance editor
    /// (wetroom mirror), 2 = wardrobe (bedroom).
    pub showroom_mode: u8,
    /// Cosmetic catalog mirror for the wardrobe UI: (id, name, slot).
    pub cosmetics_list: Vec<(String, String, String)>,
    /// The GAME character's name being edited in the showroom (v0.448). DECOUPLED from the
    /// chat profile name (`profile_name` / `user_name`): a character is a local save, not
    /// your network identity. Synced to the player's ECS Name + saved on confirm.
    pub character_name: String,
    /// Whether settings were changed this frame (signals lib.rs to apply them).
    pub settings_dirty: bool,
    /// Request to quit the application.
    pub quit_requested: bool,
    /// Set to true when identity has been recovered from seed phrase and WS needs reconnect.
    pub identity_recovered: bool,
    /// The Ed25519 private key bytes (32 bytes) for signing, if available.
    pub private_key_bytes: Option<Vec<u8>>,
    /// Whether initial channel history has been fetched after connecting.
    pub history_fetched: bool,

    // ── Passphrase / key encryption state ──

    /// Whether a passphrase prompt is needed before the key is usable.
    pub passphrase_needed: bool,
    /// What mode the passphrase prompt is in.
    pub passphrase_mode: PassphraseMode,
    /// Input field for the passphrase.
    pub passphrase_input: String,
    /// Input field for confirming a new passphrase.
    pub passphrase_confirm: String,
    /// Input field for the old passphrase (change mode).
    pub passphrase_old_input: String,
    /// Status/error message for the passphrase prompt.
    pub passphrase_status: String,
    /// The encrypted private key (base64), persisted through save cycles.
    pub encrypted_private_key: String,
    /// The PBKDF2 salt (base64), persisted through save cycles.
    pub key_salt: String,
    /// PBKDF2 iteration count the current vault was encrypted with.
    /// Defaults to `PBKDF2_ITERATIONS_LEGACY` (100_000) for vaults written
    /// before v0.277.0; new encryptions set it to `PBKDF2_ITERATIONS_NEW`
    /// (600_000). The Unlock site re-encrypts to the new count on the
    /// next successful unlock — silent one-time migration per vault.
    pub key_iterations: u32,
    /// Per-section unlock state for `lockable_gate` (private sections like the
    /// Wallet). In memory only — never persisted, so a restart re-locks all.
    pub section_locks: std::collections::HashMap<String, crate::gui::widgets::LockState>,

    // ── v0.278.0 auto-unlock state ──
    /// User's chosen unlock mode: AlwaysPrompt / Keychain / KeychainPin.
    /// Default is AlwaysPrompt — opt-in is explicit.
    pub auto_unlock_mode: crate::auto_unlock::AutoUnlockMode,
    /// AES-GCM blob of the seed encrypted with `PIN ‖ device_key`. Empty
    /// when KeychainPin mode is not set up. Persisted in AppConfig.
    pub pin_encrypted_seed: String,
    /// PBKDF2 salt for the PIN-encrypted seed (base64). Empty when unset.
    pub pin_salt: String,
    /// "Remember on this device" checkbox state on the unlock modal —
    /// when true and the modal completes successfully, the seed is
    /// stashed in the OS keychain and `auto_unlock_mode` flips to
    /// `Keychain`. Resets to false after each modal close.
    pub remember_on_device: bool,
    /// True while the passphrase-unlock worker runs the 600k-iter PBKDF2 OFF
    /// the UI thread (v0.306.0). Drives the "Unlocking…" spinner + disables the
    /// button so the click can't re-fire. Previously the PBKDF2 ran inline on
    /// the click → ~200ms–1s UI freeze on every unlock.
    pub passphrase_unlocking: bool,
    /// Receiver for the background unlock result: `Ok((seed_bytes, optional
    /// (encrypted, salt, iters) when a legacy 100k vault was re-encrypted to
    /// 600k in the worker))`, or `Err(message)` for a wrong passphrase. Drained
    /// each frame by `draw_unlock`; the cheap post-steps (keychain stash,
    /// apply_pq_identity, save) then run on the main thread.
    #[cfg(feature = "native")]
    pub passphrase_unlock_rx: Option<
        std::sync::mpsc::Receiver<Result<(Vec<u8>, Option<(String, String, u32)>), String>>,
    >,
    /// True while the PIN-unlock worker runs `decrypt_seed_with_pin` (PBKDF2)
    /// off the UI thread (v0.307.0). Mirrors `passphrase_unlocking` for the PIN
    /// path (per-launch for KeychainPin users — the same freeze class).
    pub pin_unlocking: bool,
    /// Background PIN-unlock result: `Ok(seed_bytes)` or `Err(message)`.
    #[cfg(feature = "native")]
    pub pin_unlock_rx: Option<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>>,
    /// In-flight clipboard-image upload (v0.307.0): `(target_channel, rx)` where
    /// rx yields the uploaded image URL or an error. The network POST runs on a
    /// worker thread so a big paste doesn't freeze the UI; the drain sends the
    /// chat message with the returned URL on the main thread (needs ws_client +
    /// the signing key).
    #[cfg(feature = "native")]
    pub clipboard_upload: Option<(String, std::sync::mpsc::Receiver<Result<String, String>>)>,
    /// PIN entry buffer (active digit-only field on the PinSetup /
    /// PinUnlock / PinChange modal forms).
    pub pin_input: String,
    /// Confirm-PIN entry buffer for PinSetup / PinChange.
    pub pin_confirm: String,
    /// Current-PIN entry buffer for PinChange.
    pub pin_old_input: String,
    /// Status/error text displayed under the PIN entry fields.
    pub pin_status: String,
    /// Full-PQ: our Kyber768 (ML-KEM-768) public key, base64. Derived
    /// deterministically from the BIP39 seed on recovery/unlock and
    /// advertised at identify; the secret re-derives from the seed on
    /// demand and is never stored. Replaces the old ECDH keypair.
    pub kyber_public_b64: String,
    /// Map of peer Dilithium pubkey hex -> their Kyber768 public base64.
    /// Populated from peer_list, full_user_list, profile_data, peer_joined.
    pub peer_kyber_keys: std::collections::HashMap<String, String>,
    /// Map of voice-channel NAME -> its numeric relay id (as a string). Populated
    /// from voice_channel_list (the id is i64 on the wire). Needed because the
    /// relay tracks voice rooms by numeric id, but the chat UI keys channels by
    /// name; this lets the join/leave send the correct room_id. (Phase C, v0.491.)
    pub voice_channel_ids: std::collections::HashMap<String, String>,
    /// Count of inbound voice Opus frames received (Phase C diagnostics, v0.492).
    /// Proves audio is flowing over the WebRTC pipe before playback (Phase D).
    pub voice_rx_frames: u64,
    /// The numeric id of the voice room we are currently joined to, if any
    /// (Phase C, v0.492). Drives the roster-based WebRTC offer logic.
    pub voice_active_room: Option<String>,
    /// Whether we have already offered to the incumbents present in our first
    /// roster after joining. Per the web's "newcomer offers, incumbents wait"
    /// rule, we offer to the peers present at our join, and let later joiners
    /// offer to us. Reset on each join. (Phase C, v0.492.)
    pub voice_incumbents_captured: bool,
    /// Peers whose voice WebRTC transport is connected (Phase D, v0.494). We send
    /// our captured mic Opus to each of these. Populated on VoiceConnected,
    /// cleared on Closed / leave.
    pub voice_connected_peers: std::collections::HashSet<String>,
    /// Previous "joined to a voice room" state, so lib.rs starts/stops the live
    /// voice session only on the edge. (Phase D, v0.494.)
    pub voice_session_prev: bool,
    /// Incoming 1:1 voice call ringing: `(peer pubkey hex, display name)`.
    /// Set when a `voice_call ring` arrives; the chat page renders the
    /// Accept / Decline modal from this. (v0.703 — closes the parity bug
    /// where a web caller rang a native user forever.)
    pub call_incoming: Option<(String, String)>,
    /// Active 1:1 call: `(peer pubkey hex, display name)`. While set, the
    /// live voice session runs (same pump as voice rooms) and the chat page
    /// shows the in-call bar with Hang up.
    pub call_active: Option<(String, String)>,
    /// Outgoing 1:1 call we placed, waiting for the peer to accept:
    /// `(peer pubkey hex, display name)`. On their `accept` we create the
    /// WebRTC offer and move to `call_active`; on `reject` / timeout we
    /// clear it. (v0.705 — native-initiated calls.)
    pub call_outgoing: Option<(String, String)>,
    /// Ring-out deadline: if the peer hasn't accepted by this instant we
    /// auto-cancel (matches the web's 30 s setTimeout). Not serialized.
    pub call_outgoing_deadline: Option<std::time::Instant>,
    /// Mic muted during a 1:1 call: the voice pump skips sending our Opus
    /// while true (the peer still reaches us; we just stop transmitting).
    pub call_muted: bool,
    /// Open file-attach picker modal (v0.708, in-app file browser).
    /// Some = the chat attach picker is open.
    pub chat_attach_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,

    // ── Donation address config ──

    /// Admin-configurable Solana donation address (legacy).
    pub donate_solana_address: String,
    /// Admin-configurable Bitcoin donation address (legacy).
    pub donate_btc_address: String,
    /// Dynamic donation addresses configured LOCALLY in Settings (persisted to
    /// AppConfig -- this is a self-hosting operator's own list).
    pub donate_addresses: Vec<DonateAddress>,
    /// Donation addresses fetched from the CONNECTED server's GET /api/server-info
    /// `funding.addresses` (v0.659). Kept separate from `donate_addresses` on
    /// purpose: this list is never persisted (AppConfig ignores it), so a server's
    /// funding info can never clobber an operator's hand-configured local list,
    /// and it refreshes from the server on every connect. The Donate page prefers
    /// this list when non-empty. Previously only the web client fetched this;
    /// native's Donate page stayed empty for everyone except a self-hosting
    /// operator who had manually filled in Settings.
    pub donate_addresses_server: Vec<DonateAddress>,
    /// The connected server's funding goal, from `funding.goal_usd` +
    /// `funding.goal_label` (v0.659). Replaces the Donate page's old hardcoded
    /// fake "$350 / $1000" progress bar -- the card only renders when a real
    /// goal exists.
    pub donate_funding_goal: Option<(f64, String)>,
    /// Which server (trimmed URL) the two fields above came from. Donation info is
    /// MONEY-ROUTING data: server A's addresses must never be displayed while
    /// connected to server B (adversarial review, 2026-07-01), so the connect
    /// handler clears the fields whenever this doesn't match the new server, and
    /// `apply_server_funding` discards results that arrive tagged with a URL we
    /// are no longer connected to.
    pub donate_funding_server: String,
    /// In-flight GET {url}/api/server-info fetch spawned on connect (the
    /// `peer_list` handler in lib.rs), tagged with the trimmed server URL it was
    /// sent to (same style as `server_info_loader`); drained once per frame in
    /// the render loop via `apply_server_funding`. Replaced (old receiver
    /// dropped, its late send fails harmlessly) when a connect targets a
    /// different server, so a stalled fetch can't block future ones.
    pub donate_info_rx: Option<(String, std::sync::mpsc::Receiver<Result<ServerInfo, String>>)>,
    /// Temp fields for the "Add Address" form in settings.
    pub donate_new_network: String,
    /// Temp type for new address ("address" or "url").
    pub donate_new_type: String,
    /// Temp value for new address.
    pub donate_new_value: String,
    /// Temp label for new address.
    pub donate_new_label: String,

    // ── Chat user profile modal ──

    /// Whether the user profile modal is open.
    pub chat_user_modal_open: bool,
    /// Display name of the user shown in the modal.
    pub chat_user_modal_name: String,
    /// Public key of the user shown in the modal.
    pub chat_user_modal_key: String,

    // ── Channel create modal ──
    pub show_create_channel_modal: bool,
    pub new_channel_name: String,
    /// "+ Add Server" modal (v0.187.0). Lets the user paste a relay URL
    /// (e.g. https://other-server.example) and connect to it. Maintains
    /// the previous server list rather than swapping — multi-server
    /// support is the eventual goal.
    pub show_add_server_modal: bool,
    pub add_server_url_draft: String,
    pub add_server_name_draft: String,
    /// Active tab index on the Server Settings page (v0.188.0).
    /// 0 = Overview (USER/MOD/ADMIN tiered sections).
    /// 1 = Channels (spreadsheet editor).
    /// 2 = Members (list with role + actions).
    /// 3 = Reports (placeholder for v0.189 mod review surface).
    pub server_settings_tab: u8,
    /// Per-channel-row draft state for the Channels spreadsheet — keyed
    /// by channel id, value is the in-flight edit (name, desc, flags).
    /// Saved when the user clicks the row's Save button.
    pub server_settings_channel_drafts: std::collections::HashMap<String, ChannelDraft>,
    /// Pending "new channel" row at the bottom of the Channels grid.
    pub server_settings_new_channel: ChannelDraft,
    pub new_channel_description: String,

    // ── Create group modal (P2P signed-object groups, v0.295+) ──
    pub show_create_group_modal: bool,
    pub new_group_name: String,
    /// Create-group choice: share full message history with members who join
    /// later (signed into group_v1). false = private (default, forward secrecy).
    pub new_group_share_history: bool,
    /// Set after a successful create — the shareable invite ticket to copy.
    /// The modal flips into "share this ticket" mode while `Some`.
    pub create_group_ticket: Option<String>,
    /// Inline status/error for the create modal.
    pub create_group_status: String,

    // ── Join group modal ──
    pub show_join_group_modal: bool,
    pub join_group_invite_code: String,
    /// Inline status/error for the join modal.
    pub join_group_status: String,
    /// Set to the joined group's name after a successful join — the modal flips
    /// into a "✅ Joined …" confirmation view while `Some` so the user has
    /// visible feedback that it worked (instead of the modal silently closing).
    pub join_group_result: Option<String>,

    /// P2P (signed-object) groups the user is a member of — read-only cache of
    /// the relay's `/api/v2/groups` projection. Rendered in the left panel
    /// alongside legacy `chat_groups` during the migration. Refreshed after
    /// create/join and on explicit refresh.
    pub p2p_groups: Vec<crate::net::api_v2::P2pGroupInfo>,
    /// When the P2P-groups projection was last fetched — used to do a one-time
    /// fetch on first render so the list is populated without a manual action.
    pub p2p_groups_last_fetch: Option<std::time::Instant>,

    // ── Active P2P-group conversation (inline, channel-style) ──
    // A P2P group opens like a channel: clicking it sets
    // `chat_active_channel = "p2pgroup:<id>"` and its decrypted messages render
    // in the SAME center panel as channels/DMs (no modal). All network/crypto
    // for a group runs on a BACKGROUND THREAD (v0.303.0) so switching is
    // instant and the periodic refresh never freezes the UI — the worker sends
    // a `GroupLoad` back over a channel and the GUI applies it on the main
    // thread. These fields cache the applied result.
    /// Transient status line for the P2P-group invite action (e.g.
    /// "Invite copied"). Shown briefly in the group header / popup.
    pub p2p_group_invite_status: String,
    /// The group id currently open (matches the active `p2pgroup:<id>` channel).
    /// Empty when no P2P group is open.
    pub p2p_group_active_id: String,
    /// Current epoch number for the open group (used when sending).
    pub p2p_group_chat_epoch: u64,
    /// Decapsulated 32-byte AES key for the current epoch. None = we don't have
    /// a copy yet (no epoch issued, or it isn't sealed to us).
    pub p2p_group_chat_epoch_key: Option<Vec<u8>>,
    /// When we last KICKED OFF a background refresh of the open group — drives
    /// the periodic reload cadence.
    pub p2p_group_last_fetch: Option<std::time::Instant>,
    /// Roster index for the open group: author fingerprint → full pubkey hex
    /// (lets group messages reuse the standard identicon + name resolution).
    pub p2p_group_fp_to_key: std::collections::HashMap<String, String>,
    /// Roster index for the open group: author fingerprint → display name.
    pub p2p_group_fp_to_name: std::collections::HashMap<String, String>,
    /// In-flight background load for the open group: `(group_id, receiver)`.
    /// Drained each frame; applied if it still matches the active group.
    #[cfg(feature = "native")]
    pub p2p_group_loader:
        Option<(String, std::sync::mpsc::Receiver<crate::net::api_v2::GroupLoad>)>,
    /// In-flight background refresh of the whole P2P-group list (keeps the left
    /// rail + member counts fresh when membership changes on another client,
    /// and detects when the open group was disbanded/left elsewhere).
    #[cfg(feature = "native")]
    pub p2p_groups_list_loader:
        Option<std::sync::mpsc::Receiver<Vec<crate::net::api_v2::P2pGroupInfo>>>,
    /// True while a freshly-opened group is still loading (shows "Loading…"
    /// instead of the no-key/no-message hint for that brief window).
    pub p2p_group_loading: bool,
    /// inc-2: object_ids of group messages already handled over the WebRTC mesh
    /// (sent or received P2P), so a push + the 2s relay poll don't double-render
    /// the same message. Mirrors the web's `_p2pGroupSeenObjIds`. Cleared on
    /// group switch (in `spawn_group_load(fresh=true)`).
    #[cfg(feature = "native")]
    pub p2p_group_seen_obj_ids: std::collections::HashSet<String>,

    // ── Sidebar section settings popups (v0.195.0) ──
    // Rendered as floating Areas anchored below the section's cog
    // button. Using GuiState fields instead of egui's popup machinery
    // because the previous `popup_below_widget(... CloseOnClick ...)`
    // pattern self-closed on the trigger click — the popup flickered
    // on for one frame then disappeared (operator bug 2026-05-08).
    pub dm_settings_popup_open: bool,
    pub groups_settings_popup_open: bool,

    /// Notification preferences (v0.641, was unwired client-side despite the relay + web
    /// client already fully supporting it -- see `update_notification_prefs`/
    /// `get_notification_prefs`/`notification_prefs_data` in `src/relay/relay.rs`, mirrored by
    /// `web/pages/settings-app.js`). Defaults match the server's own column defaults
    /// (`notification_prefs` table) so a not-yet-fetched popup shows sensible values instead
    /// of a false "off." `notif_prefs_loaded` is false until the first real
    /// `notification_prefs_data` round-trips.
    pub notif_dm_enabled: bool,
    pub notif_mentions_enabled: bool,
    pub notif_tasks_enabled: bool,
    pub notif_dnd_start: Option<String>,
    pub notif_dnd_end: Option<String>,
    pub notif_prefs_loaded: bool,

    /// Pending unencrypted-DM confirmation (v0.199.0).
    ///
    /// When the user tries to send a DM but the recipient's ECDH key
    /// is missing or encryption fails, we DO NOT silently send plaintext
    /// (operator security audit B3 / 2026-04-30). Instead we stash the
    /// would-be message here and pop a modal asking the user to either
    /// confirm "Send unencrypted anyway" or cancel.
    pub dm_unencrypted_confirm: Option<PendingUnencryptedDm>,

    // ── Cosmos page state (v0.203.0, Phase 3) ──
    /// Which view the Cosmos page is currently rendering.
    pub cosmos_view: crate::gui::pages::cosmos::CosmosView,
    /// Pan offset (screen pixels). Updated by click-drag on the canvas.
    pub cosmos_pan: egui::Vec2,
    /// Zoom factor — 1.0 = default, > 1.0 = zoomed in, < 1.0 = zoomed out.
    /// Updated by mouse wheel scroll. Clamped in allocate_canvas.
    pub cosmos_zoom: f32,
    /// Currently selected body id in the System view (for the right-side
    /// details panel + map highlight). v0.203.2 — populated by clicking
    /// a body in the left-side browser sidebar OR clicking it on the map.
    pub cosmos_selected_body: Option<String>,
    /// Which planet groups are expanded in the body browser sidebar.
    /// Stored as ids of planets whose moon list is expanded. v0.203.2.
    pub cosmos_expanded_planets: std::collections::HashSet<String>,
    /// Pending focus request — when Some, the next System view render
    /// computes the pan needed to center this body on screen, then
    /// clears the request. Set by sidebar click or "Focus" button.
    /// v0.205.0 (operator pushback on zoom always centering on Sun).
    pub cosmos_focus_request: Option<String>,
    /// Body id the camera continuously follows across frames as it moves
    /// along its orbit, or `None` for no auto-follow. Unlike
    /// `cosmos_focus_request` (a one-shot snap-to), this re-centers every
    /// frame. Set by the body detail card's "Track"/"Stop Tracking" action;
    /// cleared automatically by any other Focus request (see
    /// `gui/pages/cosmos.rs`'s focus-consumption site).
    pub cosmos_tracked_body: Option<String>,
    /// 3D camera state for System view (Phase 4, v0.206.0).
    /// Yaw + pitch + distance + look-at target define the camera; mouse
    /// drag rotates, scroll zooms, sidebar click re-centers the target.
    pub cosmos_camera_3d: crate::gui::pages::cosmos::Cosmos3DCamera,

    /// Cosmos sim-time in seconds since the J2000.0 epoch
    /// (2000-01-01 12:00:00 UTC). Drives Kepler-evolved body positions.
    /// Initialized to "current real-world time" on first cosmos page
    /// open so the user immediately sees today's planetary configuration.
    /// v0.208.0.
    pub cosmos_sim_time_seconds: f64,
    /// Sim speed multiplier — 0 = paused, 1 = real-time (1 second sim =
    /// 1 second real), 86400 = 1 day per real second, etc. Negative
    /// values rewind. v0.208.0.
    pub cosmos_sim_speed: f64,
    /// Wall-clock Instant of the previous frame, used to compute dt for
    /// sim_time advancement. None on first frame. v0.208.0.
    #[allow(clippy::type_complexity)]
    pub cosmos_last_real_instant: Option<std::time::Instant>,
    /// Whether the cosmos sim_time has been initialized (sets to "now"
    /// on first cosmos page draw). v0.208.0.
    pub cosmos_sim_time_initialized: bool,
    /// Which body's pill is currently expanded into an info card on the
    /// 3D System view. Only one body can be expanded at a time — clicking
    /// a different body's pill swaps the expansion; clicking the same
    /// pill collapses it. Independent of `cosmos_selected_body` which
    /// drives the right-side details panel. v0.209.0.
    pub cosmos_expanded_body: Option<String>,
    /// Whether to render Lagrange-point overlay markers (L1-L5 for
    /// Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn pairs).
    /// Off by default to keep the wide view clean. Toggled from the
    /// cosmos canvas overlay button. v0.211.0.
    pub cosmos_show_lagrange: bool,
    /// Whether to render reference-orbit rings (LEO/MEO/GEO/etc) around
    /// supported planets when zoomed close enough. Off by default —
    /// rings only appear when the user explicitly enables AND the camera
    /// is close enough that they're not microscopic on screen. v0.212.0.
    pub cosmos_show_reference_orbits: bool,
    /// Cached forward Sky-Events scan (Phase 4d-quad, v0.248). The scan
    /// is O(days × bodies²) so it must NOT run every frame like the
    /// instant detector — it's recomputed lazily when sim_time drifts
    /// far from `cosmos_upcoming_scan_origin`, throttled by
    /// `cosmos_upcoming_last_scan`. Each entry: when (sim seconds since
    /// J2000), human label, severity (0 info / 1 notable / 2 major).
    pub cosmos_upcoming_events: Vec<crate::gui::pages::cosmos::UpcomingSkyEvent>,
    /// sim_time the cached forward scan was computed at. Recompute when
    /// the live sim_time moves more than ~12h away from this.
    pub cosmos_upcoming_scan_origin: f64,
    /// Wall-clock instant of the last forward scan — throttles recompute
    /// so fast-forward / scrubbing can't trigger a scan every frame.
    pub cosmos_upcoming_last_scan: Option<std::time::Instant>,

    /// Cached server-wide settings received from the relay (v0.200.0).
    /// Populated on `server_settings_state` WS message. None means we
    /// haven't received the state yet (during initial connect, before
    /// any modify happens). UI uses defaults until populated.
    pub server_settings: Option<crate::relay::storage::ServerSettings>,
    /// All role definitions, from the relay's `role_list` WS broadcast
    /// (sent on connect + after any role change). Drives the user-modal
    /// role dropdown + badge colors. Empty until the first broadcast.
    /// v0.241 (roles Phase R2).
    pub chat_roles: Vec<crate::relay::storage::RoleDef>,
    /// Server→Services snapshot from the relay's `service_state` reply
    /// (admin-only; sent after `service_control` start/stop/refresh).
    /// Each entry: soft gate + live daemon active/enabled. Empty until
    /// the admin opens the Services panel (which sends a refresh).
    /// v0.262.16.
    pub service_state: Vec<crate::relay::services::ServiceInfo>,
    /// Per-role working copies for the Server Settings → Roles editor,
    /// keyed by role id. Seeded from `chat_roles` on first edit of a
    /// row; lets the operator tweak label/color/caps before pressing
    /// Save (which sends role_upsert). v0.242 (Phase R3).
    pub roles_drafts: std::collections::HashMap<String, crate::relay::storage::RoleDef>,
    /// The "add a custom role" form draft. id starts empty (operator
    /// types one). v0.242 (Phase R3).
    pub new_role_draft: crate::relay::storage::RoleDef,
    /// Currently-banned users for the Server Settings → Banned users
    /// admin panel. Populated by the `banned_list` WS message (only
    /// admins receive it). Empty until the panel requests it. v0.245.
    pub chat_banned_users: Vec<crate::relay::storage::BannedUser>,
    /// True once a `banned_list_request` has been sent this session so
    /// the panel doesn't re-request every repaint. Reset on disconnect.
    pub chat_banned_requested: bool,

    // ── Game admin: game-world bans, STRUCTURALLY SEPARATE from chat bans
    //    (v0.474). Free speech is a right (chat is never affected); playing on
    //    the shared MMO world is a privilege. These read the relay's
    //    `game_banned_keys` table, never `banned_keys`. See pages/game_admin.rs
    //    + docs/design/characters-and-servers.md.
    /// Players banned from the 3D game world only. Populated by the
    /// `game_banned_list` reply (admins only). Empty until requested.
    pub game_bans: Vec<crate::relay::storage::GameBan>,
    /// True once a `game_banned_list_request` was sent this session so the
    /// page doesn't re-request every repaint. Reset on disconnect.
    pub game_bans_requested: bool,
    /// The target public key typed into the Game Admin ban form.
    pub game_admin_target_key: String,
    /// The reason typed into the Game Admin ban form.
    pub game_admin_ban_reason: String,
    /// Last status / error line shown on the Game Admin page.
    pub game_admin_status: String,

    // ── Character launcher (v0.474). Play opens this screen: pick a home /
    //    character, customize your look offline, set a default to skip the
    //    picker next time, then Enter World. See pages/launcher.rs.
    /// Cached local save list (filename stem, modified-unix-secs), refreshed
    /// when the launcher opens. Each save is a self-custodial home+character.
    pub launcher_saves: Vec<(String, u64)>,
    /// False until the launcher has loaded `launcher_saves` once this opening.
    /// Reset to false every time the launcher page is entered so the list is
    /// fresh (a new save made in-session shows up).
    pub launcher_saves_loaded: bool,
    /// The save stem currently highlighted in the launcher ("" = the active
    /// offline home / default character).
    pub launcher_selected: String,
    /// The default character's save stem ("" = no default, always show the
    /// launcher). Persisted to AppConfig.default_character. When non-empty,
    /// Play skips the launcher and enters the world with this character.
    pub launcher_default_character: String,
    /// A non-active save stem the launcher asked to load on Enter World; lib.rs
    /// applies it to the live player after the world loads, then clears this.
    pub launcher_pending_load: Option<String>,
    /// One-shot signal (v0.476): Play wants the unified character picker (the
    /// showroom in mode 0). Distinguishes "Play -> show the picker" from "Esc ->
    /// plain first-person". load_world only opens the showroom when this is set,
    /// then clears it -- so Esc to FPS never surfaces the old character-select.
    pub launcher_open_select: bool,
    /// Which left-pane category the unified launcher has selected, so the right
    /// pane knows whether to draw the character editor or server details.
    pub launcher_selected_kind: LauncherSel,
    /// The id/url of the selected server row (when launcher_selected_kind ==
    /// Server), so the detail pane knows which server to describe.
    pub launcher_selected_server: Option<String>,
    /// Set by the picker's "Back" button to cancel the showroom and return to
    /// the menu without entering the world (lib.rs handles it, same as Esc).
    pub showroom_cancel: bool,
    /// Cache of fetched server metadata (GET /api/server-info), keyed by server
    /// id, for the launcher's server-detail pane (v0.478). Avoids refetching.
    pub server_info_cache: std::collections::HashMap<String, ServerInfo>,
    /// In-flight server-info fetch: (server id, result channel). One at a time.
    pub server_info_loader: Option<(String, std::sync::mpsc::Receiver<Result<ServerInfo, String>>)>,
    /// Currently-muted users for the Server Settings → Muted users mod
    /// panel. Populated by the `muted_list` WS message (mods/admins
    /// only). v0.246.
    pub chat_muted_users: Vec<crate::relay::storage::MutedUser>,
    /// True once a `muted_list_request` has been sent this session.
    /// Reset on disconnect (mirrors chat_banned_requested).
    pub chat_muted_requested: bool,
    /// In-progress draft of server settings being edited in the admin
    /// UI. None = not editing. Cloned from `server_settings` when admin
    /// opens the editor. Save button sends a ServerSettingsUpdate WS
    /// message and clears the draft.
    pub server_settings_draft: Option<crate::relay::storage::ServerSettings>,

    // ── Channel edit modal ──
    pub show_channel_edit_modal: bool,
    pub edit_channel_id: String,
    pub edit_channel_name: String,
    pub edit_channel_description: String,
    /// Whether the delete confirmation is showing in the edit modal.
    pub edit_channel_confirm_delete: bool,
    /// Whether the slash commands help modal is visible.
    pub show_help_modal: bool,

    // ── Server settings page (mod / admin actions) ──
    /// Username target for kick/mute/ban/verify/promote actions.
    pub server_settings_target_user: String,
    /// Channel name input for create/delete/readonly actions.
    pub server_settings_channel_name: String,
    /// Last-generated invite code (shown after admin clicks "Generate invite").
    pub server_settings_invite_code: String,
    /// Draft input for "Redeem friend code" (Server Settings → User). (v0.722)
    pub redeem_code_draft: String,
    /// Draft input for "Revoke a device" key prefix (Server Settings → User). (v0.722)
    pub revoke_key_draft: String,
    /// Last action result message (success or error feedback).
    pub server_settings_status: String,
    /// Live server health snapshot (Server Settings → System health, v0.720).
    /// Read-only: /health + /api/stats of the connected server, fetched on a
    /// worker thread. In-app ops slice 1 native parity (docs/design/in-app-ops.md).
    pub system_health: Option<SystemHealth>,
    /// System-health fetch status line ("Loading…", error text, or empty).
    pub system_health_status: String,
    /// In-flight system-health fetch (worker thread → per-frame drain).
    pub system_health_rx: Option<std::sync::mpsc::Receiver<Result<SystemHealth, String>>>,
    /// Federated-server list (Server Settings → Federation, v0.722).
    pub federation_servers: Vec<FederationServerRow>,
    /// Federation panel status line ("Loading…", error text, or empty).
    pub federation_status: String,
    /// In-flight federation-list fetch (worker thread → per-frame drain).
    pub federation_rx: Option<std::sync::mpsc::Receiver<Result<Vec<FederationServerRow>, String>>>,
    /// Draft inputs for the Federation "Add server" row.
    pub federation_add_url_draft: String,
    pub federation_add_name_draft: String,
    /// Whether the danger-zone confirm-delete prompt is showing.
    pub server_settings_confirm_action: Option<String>,

    // ── Debug console state ──

    /// Whether the F12 debug console overlay is visible.
    pub debug_console_visible: bool,
    /// Ring buffer of timestamped debug log lines for the overlay.
    pub debug_log: Vec<String>,

    // ── Tools catalog (loaded from data/tools/catalog.json) ──
    pub tools_catalog: Vec<ToolEntry>,

    // ── Page taxonomies (Infinite-of-X migrations, v0.123.0) ──
    /// Equipment slots for the inventory page (`data/inventory/equipment_slots.json`).
    pub equipment_slots: Vec<(String, String)>,
    /// Bug-report severity labels (`data/bugs/taxonomy.json`).
    pub bug_severities: Vec<String>,
    /// Bug-report category labels (`data/bugs/taxonomy.json`).
    pub bug_categories: Vec<String>,
    /// Crafting category filters (`data/crafting/categories.json`).
    pub crafting_category_groups: Vec<CraftCategoryGroup>,
    /// Marketplace category filters (`data/market/categories.json`).
    pub market_categories: Vec<String>,
    /// In-app Library: sections of nested categories holding docs + external
    /// links (`data/library/` + the shared `data/resources/catalog.json`).
    pub library: Vec<LibrarySection>,
    /// Studio scene presets (`data/studio/scenes.json`).
    pub studio_scene_presets: Vec<StudioScenePreset>,
    /// Studio source presets (`data/studio/sources.json`).
    pub studio_source_presets: Vec<StudioSourcePreset>,
    /// Studio streaming pickers (`data/studio/streaming_config.json`).
    pub studio_streaming_config: StudioStreamingConfig,
    /// Donate page FAQ entries (`data/donate/faq.json`).
    pub donate_faq: Vec<DonateFaqEntry>,
    /// QA test tasks (`data/testing/qa_tasks.json`) shown on the Testing page.
    pub qa_test_tasks: Vec<QaTestTask>,
    /// Per-task local status: id → "passed" / "issue" / "" (untouched).
    pub qa_test_status: std::collections::HashMap<String, String>,
    /// Per-task draft note (when typing into Report Issue field).
    pub qa_test_note: std::collections::HashMap<String, String>,
    /// Filter chip on the Testing page: "all" / category id.
    pub qa_test_filter: String,
    /// Bookmark categories (`data/browser/bookmarks.json`) shown on the Browser page.
    pub browser_bookmarks: Vec<BrowserCategory>,
    /// Filter chip on the Browser page: "all" / category id.
    pub browser_filter: String,
    /// Preview opt-in: render the new two-tier nav (Reality / Sim / Tools /
    /// Settings + sub-pages) instead of the legacy single-row nav. Toggled
    /// from the [≡] / [▤] button in the nav itself. Not persisted yet so
    /// each launch starts on the legacy nav until operator picks a winner.
    pub nav_two_tier: bool,
    /// Active top-tier category id when nav_two_tier is on.
    /// One of: "reality", "sim", "tools", "settings".
    pub nav_top_category: String,
    /// True when the player is taking damage / under attack — flips the
    /// nav RGB separator from cyclic spectrum to a pulsing red so the
    /// player can tell mid-menu without sound. Set by combat / damage
    /// systems; cleared after a short cooldown.
    pub attack_pulse_active: bool,
    /// game_time when attack_pulse_active was last set; used to auto-clear
    /// after a few seconds of no new damage events.
    pub attack_pulse_last_hit_at: f64,
    /// Death & recovery (v0.745, loop-map rung 1): Some(cause) while the
    /// player is dead — the death screen overlay shows it ("starvation",
    /// "suffocation", an effect name...). Cleared by respawn.
    pub player_death_cause: Option<String>,
    /// The death screen's Respawn button; lib.rs performs the respawn
    /// (teleport to spawn, reset vitals, remove Dead) and clears it.
    pub pending_respawn: bool,
    // v0.197.0: ai_usage_filters removed (AI Usage page deleted).
    // v0.415.0: onboarding_concepts + onboarding_core_pages removed with the
    // standalone onboarding page (the web /onboarding page still reads the
    // JSON files; the quest chains below remain the native consumer).

    // ── Universal help modal (loaded from data/help/topics.json) ──
    /// Registry of help topics. Populated at startup from data/help/topics.json.
    pub help_registry: crate::gui::widgets::help_modal::HelpRegistry,
    /// ID of the currently-open help topic, if any. Setting this opens the help modal.
    pub active_help_topic: Option<String>,

    // ── Onboarding quest chains (loaded from data/onboarding/quests.json) ──
    /// Quest chains displayed on the Onboarding page.
    pub onboarding_quest_chains: Vec<crate::gui::pages::onboarding::QuestChain>,
    /// Map of "chain_id:step_id" -> done?. Persisted via AppConfig for local progress.
    pub onboarding_quest_progress: std::collections::HashMap<String, bool>,

    // ── Inline image cache (for chat attachments) ──
    /// Fetches, decodes, and caches images referenced in chat messages so
    /// they render inline instead of as raw /uploads/... text.
    pub image_cache: crate::gui::widgets::image_cache::ImageCache,
    /// URL of the image currently shown full-screen in the viewer modal.
    /// `None` means the modal is closed.
    pub image_viewer_url: Option<String>,

    // ── Studio state ──
    pub studio: StudioState,

    // ── Chat panel collapse state (persisted in config) ──
    pub chat_connection_collapsed: bool,
    pub chat_dm_collapsed: bool,
    pub chat_groups_collapsed: bool,
    pub chat_servers_collapsed: bool,
    pub chat_connected_server_collapsed: bool,
    pub chat_friends_collapsed: bool,
    pub chat_members_collapsed: bool,
    /// Collapse state of the Studio quick-access section in the chat right rail.
    pub chat_studio_collapsed: bool,
    /// Set by the winit layer (src/lib.rs window_event) when Ctrl+V is
    /// pressed on the Chat page. The chat page reads + clears this each
    /// frame and, if an image is on the clipboard, uploads it. Needed
    /// because egui-winit swallows Ctrl+V (translates to Event::Paste
    /// text-only, returns before emitting the V key event) so egui's
    /// input layer never sees Ctrl+V for an image clipboard. v0.234.
    pub pending_clipboard_paste: bool,
    /// Highlighted row in the @mention autocomplete popup. Up/Down arrows
    /// move it; Enter / click / hover select. Reset to 0 whenever the
    /// match set changes. v0.235.
    pub chat_mention_index: usize,
    /// Which message's reaction-popup is currently open (timestamp_ms key).
    /// Popups open only on Þ hover; the popup_hovered gate (sticky-on-popup)
    /// is only honored once a popup is actually open for that message.
    /// This prevents the reaction popup from opening when the user just
    /// hovers the message text right of the pill (operator feedback
    /// 2026-05-12 - "if I mouse over the text of a reply the reaction
    /// pill comes up even though I never clicked on the Þ"). v0.229.
    pub chat_open_popup_ts: Option<u64>,
    /// How many DM conversations to show (3, 5, 10, or 0 = all)
    pub chat_dm_display_limit: usize,

    // ── Chat panel resize/lock state ──
    pub chat_left_panel_locked: bool,
    pub chat_right_panel_locked: bool,
    pub chat_left_panel_width: f32,
    pub chat_right_panel_width: f32,

    // ── Identity / Governance / Recovery page state (v0.115.0) ──
    /// DID being looked up on the Identity page.
    pub identity_lookup_did: String,
    /// Set to true when the Identity page wants to issue a fresh fetch.
    pub identity_lookup_pending: bool,
    /// Active scope tab on the Governance page (0=All, 1=Local, 2=Civilization).
    pub governance_scope_tab: usize,
    // ── Governance live data (v0.660) ──
    /// Proposals joined from GET /api/v2/proposals + each object's payload +
    /// tally, loaded on a background thread (see pages/governance.rs).
    pub governance_proposals: Vec<crate::gui::pages::governance::ProposalView>,
    /// In-flight proposal fetch, tagged with the server URL it targets so a
    /// late result from a previous server is discarded (same staleness rule as
    /// `donate_info_rx`).
    pub governance_rx: Option<(String, std::sync::mpsc::Receiver<Result<Vec<crate::gui::pages::governance::ProposalView>, String>>)>,
    /// Which server URL `governance_proposals` BELONGS TO (the data-origin tag,
    /// same rule as `donate_funding_server`): when it doesn't match the current
    /// server, the page clears the list immediately -- another server's
    /// proposals must never render or take votes.
    pub governance_fetched_for: String,
    /// Set to request a refetch (Refresh button; after a vote/proposal lands so
    /// the tally updates). Cleared ONLY when a fetch spawns, never by a fetch
    /// completing -- so an invalidation raised while a fetch was already in
    /// flight still triggers its own refetch instead of being clobbered.
    pub governance_refresh: bool,
    /// Last proposal-list fetch error, shown on the page.
    pub governance_error: String,
    /// Votes cast THIS SESSION: proposal_id -> choice. Server-side votes are
    /// final (INSERT OR IGNORE on (proposal, voter DID)), so this only needs to
    /// stop double-submits and relabel the row; it intentionally doesn't try to
    /// reconstruct votes from earlier sessions.
    pub governance_my_votes: std::collections::HashMap<String, String>,
    /// In-flight vote submission; Ok carries (proposal_id, choice) back.
    pub governance_vote_rx: Option<std::sync::mpsc::Receiver<Result<(String, String), String>>>,
    /// Status line for the last vote/proposal submission.
    pub governance_vote_status: String,
    /// In-flight proposal submission.
    pub governance_propose_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Whether the new-proposal form is open.
    pub governance_show_propose: bool,
    /// New-proposal form fields.
    pub governance_new_title: String,
    pub governance_new_body: String,
    pub governance_new_type_idx: usize,
    pub governance_new_scope_idx: usize,
    pub governance_new_days: f32,
    // ── Laws page (v0.496) ──
    /// Selected jurisdiction id ("silverdale", "usa", ...). Empty => default to
    /// the most-local jurisdiction in the data on first draw.
    pub laws_location: String,
    /// Free-text search on the Laws page.
    pub laws_search: String,
    /// Kind filter tab (0=All, 1=HumanityOS base, 2=Real laws).
    pub laws_filter_tab: usize,
    /// Selected category chip on the Laws page ("" = all). Categories come from
    /// `data/laws/laws.json`'s own `categories` list (v0.661 -- loaded since
    /// v0.496 but never surfaced in the UI until now).
    pub laws_category: String,
    /// Active filter tab on the Governance page (0=Open, 1=All).
    pub governance_filter_tab: usize,
    /// DID being looked up on the Recovery page.
    pub recovery_lookup_did: String,
    /// Set to true when the Recovery page wants to fetch a setup.
    pub recovery_lookup_pending: bool,
    /// Guardian DID being looked up on the Recovery page.
    pub recovery_guardian_did: String,
    /// Set to true when the Recovery page wants to fetch held shares.
    pub recovery_guardian_pending: bool,

    // v0.197.0: AI Usage page form state removed (page deleted).
}

#[cfg(feature = "native")]
impl GuiState {
    /// Full-PQ: derive the Dilithium3 identity + Kyber768 DM key from
    /// the in-memory BIP39 seed (`private_key_bytes`) and force a clean
    /// reconnect so `identify` re-advertises `kyber_public`. Idempotent;
    /// a no-op if the seed isn't unlocked. MUST be called from every
    /// path that puts the seed into memory (passphrase unlock, seed
    /// recovery, legacy plaintext load) — otherwise the client has its
    /// persisted Dilithium identity but NO Kyber key, so it can neither
    /// send nor receive encrypted DMs (it never advertises a key and
    /// `try_encrypt_dm` fails `no_own_key`).
    pub fn apply_pq_identity(&mut self) {
        let seed = match self.private_key_bytes.as_ref() {
            Some(s) => s.clone(),
            None => return,
        };
        match crate::net::identity::derive_pq_identity(&seed) {
            Ok(pq) => {
                self.profile_public_key = pq.dilithium_hex;
                self.kyber_public_b64 = pq.kyber_public_b64;
                // Force a clean reconnect: drop the socket and clear the
                // reconnect guards so the auto-connect path re-runs and
                // sends `kyber_public` at identify. Without this the
                // relay never learns our Kyber key and no peer can seal
                // a DM to us.
                self.ws_client = None;
                self.ws_manually_disconnected = false;
                self.ws_reconnect_timer = 0.0;
                self.ws_reconnect_attempts = 0;
                // Governance vote tracking is PER IDENTITY (adversarial review
                // 2026-07-01): a different seed is a different voter DID, so the
                // previous identity's session votes must not label rows or
                // suppress vote buttons for this one.
                self.governance_my_votes.clear();
                self.governance_vote_rx = None;
                self.governance_vote_status.clear();
                let kp = &self.profile_public_key[..16.min(self.profile_public_key.len())];
                log::info!("PQ identity applied (Dilithium {kp}…); reconnecting to advertise Kyber");
            }
            Err(e) => log::error!("apply_pq_identity: PQ derivation failed: {e}"),
        }
    }

    /// Navigate to a sub-page, pushing the CURRENT page onto the back
    /// stack so Escape returns there. Use this for contextual openings
    /// (cog → ServerSettings, message → details modal, etc.). For
    /// peer-level navigation (clicking a top-tier nav button), set
    /// `active_page` directly and call `clear_nav_back` to drop the
    /// stack — those navigations don't nest.
    pub fn push_nav_to(&mut self, target: GuiPage) {
        // Avoid pushing duplicate top-of-stack — repeatedly opening the
        // same sub-page from the same parent shouldn't bury the parent
        // under N copies of itself.
        if self.nav_back_stack.last() != Some(&self.active_page) {
            self.nav_back_stack.push(self.active_page);
        }
        self.active_page = target;
    }

    /// Pop the back stack and switch to that page. Returns true if a
    /// page was popped (caller can decide what to do if false — e.g.
    /// fall through to "Esc closes menu" behavior at the root level).
    pub fn pop_nav_back(&mut self) -> bool {
        if let Some(prev) = self.nav_back_stack.pop() {
            self.active_page = prev;
            true
        } else {
            false
        }
    }

    /// Drop the back stack — used when navigating laterally (e.g.
    /// clicking a top-tier nav button) so the user doesn't end up with
    /// a stack of unrelated pages from earlier sessions.
    pub fn clear_nav_back(&mut self) {
        self.nav_back_stack.clear();
    }
}

#[cfg(feature = "native")]
impl Default for GuiState {
    fn default() -> Self {
        Self {
            active_page: GuiPage::MainMenu,
            last_page: GuiPage::Chat,
            nav_back_stack: Vec::new(),
            show_chat: false,
            chat_input_active: false,
            chat_input_focus_pending: false,
            copresence_active: false,
            copresence_names: Vec::new(),
            show_hud: true,
            settings: SettingsState::default(),
            chat_input: String::new(),
            chat_typing_users: std::collections::HashMap::new(),
            chat_typing_last_sent: None,
            chat_reply_to: None,
            chat_messages: Vec::new(),
            chat_search_open: false,
            chat_search_query: String::new(),
            chat_search_results: Vec::new(),
            chat_pins: std::collections::HashMap::new(),
            chat_pins_open: false,
            chat_edit_target: None,
            chat_sent_timestamps: Vec::new(),
            chat_channels: Vec::new(),
            chat_active_channel: "general".to_string(),
            chat_users: Vec::new(),
            chat_dms: Vec::new(),
            chat_groups: Vec::new(),
            chat_servers: Vec::new(),
            chat_friends: Vec::new(),
            chat_following_keys: std::collections::HashSet::new(),
            chat_followers: std::collections::HashSet::new(),
            ws_client: None,
            ws_status: "Not connected".to_string(),
            #[cfg(feature = "native")]
            webrtc: None,
            #[cfg(feature = "native")]
            webrtc_test_peer: None,
            ws_manually_disconnected: false,
            ws_reconnect_timer: 0.0,
            ws_reconnect_delay: 5.0,
            ws_reconnect_attempts: 0,
            ws_rate_limited: false,
            selected_slot: None,
            garden_selection: None,
            trees_start_collapsed: true,
            fps: 0.0,
            updater: crate::updater::Updater::new(VERSION),
            update_toast_visible: false,

            onboarding_complete: false,
            onboarding_step: 0,
            server_url: "https://united-humanity.us".to_string(),
            server_connected: false,
            server_check_rx: None,
            server_check_error: String::new(),
            user_name: "Player".to_string(),
            concept_tour_seen: false,
            default_page: GuiPage::Humanity,

            // Task board defaults
            tasks: Vec::new(),
            task_next_id: 1,
            task_search: String::new(),
            task_filter_priority: None,
            task_filter_assignee: String::new(),
            task_show_new_form: false,
            task_new_title: String::new(),
            task_new_description: String::new(),
            task_new_priority: TaskPriority::Medium,
            task_new_assignee: String::new(),

            // Profile defaults
            profile_name: String::new(),
            profile_bio: String::new(),
            profile_public_key: String::new(),
            profile_section: ProfileSection::Identity,
            profile_height: String::new(),
            profile_weight: String::new(),
            profile_eye_color: String::new(),
            profile_blood_type: String::new(),
            profile_hair_color: String::new(),
            profile_hair_length: String::new(),
            profile_hair_style: String::new(),
            profile_hair_texture: String::new(),
            profile_neck: String::new(),
            profile_shoulders: String::new(),
            profile_chest: String::new(),
            profile_waist: String::new(),
            profile_hips: String::new(),
            profile_thighs: String::new(),
            profile_inseam: String::new(),
            profile_shoe_size: String::new(),
            profile_shirt_size: String::new(),
            profile_pants_size: String::new(),
            profile_pronouns: String::new(),
            profile_location: String::new(),
            profile_website: String::new(),
            profile_private_notes: String::new(),
            profile_network_name: String::new(),
            profile_network_bio: String::new(),
            profile_network_avatar: String::new(),
            profile_directory_listed: true,
            machine_labels: Vec::new(),
            targeted_machine: None,
            vehicle_prompt: String::new(),
            livestock_prompt: String::new(),
            livestock_notice: String::new(),
            livestock_notice_at: 0.0,
            targeted_control_panel: None,
            control_panel_prompt: String::new(),
            selected_machine: None,
            machine_card_recipe: None,
            machine_card_recipe_options: Vec::new(),
            machine_card_recipe_pending: None,
            machine_card_container: None,
            machine_card_take_pending: false,
            machine_card_storable: Vec::new(),
            machine_card_store_pending: None,
            machine_card_vendor: false,
            vendor_open: false,
            vendor_goods: Vec::new(),
            vendor_status: String::new(),
            wallet_credits: 0,
            pending_vendor_buy: None,
            pending_vendor_sell: None,
            pending_equip: None,
            pending_unequip: None,
            equip_status: String::new(),
            room_bounds: Vec::new(),
            reveal_held: false,
            machine_label_dot_dist: 21.0,
            machine_label_name_dist: 13.0,
            machine_label_card_dist: 8.0,
            crew_labels: Vec::new(),
            profile_network_saved_note: String::new(),
            profile_interests: Vec::new(),
            profile_interest_input: String::new(),
            // Populated from data/skills/default_profile.json at startup
            // (see lib.rs `load_default_player_skills`). Empty at construction.
            profile_skills: Vec::new(),
            profile_social_links: Vec::new(),
            profile_social_platform: String::new(),
            profile_social_url: String::new(),
            profile_streaming_url: String::new(),
            profile_streaming_live: false,

            // Map defaults: populated from data/solar_system/bodies.json at
            // startup in lib.rs (see `load_planets`). Empty at construction.
            map_planets: Vec::new(),
            places: Vec::new(),
            homestead_design: None,
            homestead_loops: Vec::new(),
            tower_configs: Vec::new(),
            garden_areas: Vec::new(),
            grow_media: Vec::new(),
            garden_irrigation: std::collections::HashMap::new(),
            garden_nutrient: std::collections::HashMap::new(),
            placed_items: Vec::new(),
            pending_inventory_transfers: Vec::new(),
            tower_compat: Vec::new(),
            creative_mode: true,
            active_real_section: "inventory".to_string(),
            active_platform_section: "recovery".to_string(),
            active_humanity_section: "civilization".to_string(),
            map_selected_planet: None,
            map_zoom: 1.0,

            // Market defaults
            abilities: Vec::new(),
            pending_cast: None,
            ability_status: String::new(),
            ability_status_at: 0.0,
            pending_swing: false,
            swing_ready_at: 0.0,
            listings: Vec::new(),
            listings_synced: false,
            listing_status: String::new(),
            listing_search: String::new(),
            listing_filter_category: String::new(),
            listing_selected: None,
            listing_reviews: Vec::new(),
            listing_reviews_for: String::new(),
            listing_reviews_rx: None,
            listing_reviews_avg: 0.0,
            listing_reviews_count: 0,
            review_rating_draft: 5,
            review_comment_draft: String::new(),
            listing_thread: Vec::new(),
            listing_thread_for: String::new(),
            listing_thread_open: false,
            listing_msg_draft: String::new(),
            trades: Vec::new(),
            trades_synced: false,
            trade_status: String::new(),
            listing_show_new_form: false,
            listing_new_title: String::new(),
            listing_new_description: String::new(),
            listing_new_price: String::new(),
            listing_new_category: String::new(),

            // Calculator defaults
            calc_display: "0".to_string(),
            calc_expression: String::new(),
            calc_history: Vec::new(),

            // Calendar defaults
            cal_year: 2026,
            cal_month: 3,
            cal_selected_day: 1,
            cal_events: Vec::new(),
            cal_new_title: String::new(),
            cal_new_time: String::new(),
            cal_new_color: egui::Color32::from_rgb(237, 140, 36),

            // Notes defaults
            notes: Vec::new(),
            notes_selected: None,
            notes_next_id: 1,

            // Civilization defaults
            civ_stats: None,
            civ_stats_loaded: false,
            civ_stats_rx: None,
            civ_status: String::new(),

            // Wallet defaults
            wallet_balance: 0.0,
            wallet_address: String::new(),
            wallet_network: WalletNetwork::Devnet,
            wallet_send_to: String::new(),
            wallet_send_amount: String::new(),
            wallet_transactions: Vec::new(),
            wallet_sol_price: 0.0,

            // Crafting defaults
            craft_recipes: Vec::new(),
            craft_selected: None,
            craft_selected_category: None,
            pending_craft_recipe: None,
            dev_stock_materials: false,
            craft_status: String::new(),
            blueprints: Vec::new(),
            pending_build: None,
            build_status: String::new(),
            pending_consume_item: None,
            pending_drink_item: None,
            pending_rest: false,
            pending_compost: false,
            pending_fertilize_crop: None,
            vitals: GuiVitals::default(),
            pending_deploy_kit: None,
            pending_plant_seed: None,
            pending_plant_tower: None,
            pending_plant_bed: None,
            pending_stock_seeds: None,
            pending_water_crop: None,
            pending_harvest_crop: None,
            pending_harvest_many: Vec::new(),
            dev_grow_crops: false,
            crops: Vec::new(),
            pending_drone_manifest: None,
            auto_mine_enabled: false,
            last_drone_order: None,
            prev_auto_mine_enabled: false,
            drone_active: false,
            asteroids: Vec::new(),
            drones: Vec::new(),
            vehicles: Vec::new(),
            pending_summon_vehicle: None,
            pending_open_mining_modal: None,
            pending_follow_vehicle: None,
            factory_status: Vec::new(),
            skills: Vec::new(),
            pending_dev_max_skills: false,
            quests: Vec::new(),
            quests_available: Vec::new(),
            pending_accept_quest: None,

            // Guilds defaults
            guilds: Vec::new(),
            guild_selected: None,
            guilds_loaded: false,
            guilds_rx: None,
            guild_action_rx: None,
            guild_status: String::new(),
            guild_members: Vec::new(),
            guild_members_for: String::new(),
            guild_members_rx: None,
            guild_search: String::new(),
            guild_show_create: false,
            guild_new_name: String::new(),
            guild_new_desc: String::new(),
            guild_new_color: egui::Color32::from_rgb(46, 134, 193),

            player_health: 100.0,
            player_health_max: 100.0,
            inventory_items: Vec::new(),
            inventory_max_slots: 36,
            game_time: None,
            weather: None,
            power_generation: 0.0,
            power_battery_wh: 0.0,
            power_battery_capacity_wh: 0.0,
            power_autonomy_hours: 0.0,
            power_consumption: 0.0,
            power_balance: 0.0,
            water_production_lpm: 0.0,
            water_demand_lpm: 0.0,
            water_stored_l: 0.0,
            water_capacity_l: 0.0,
            water_days_autonomy: 0.0,
            air_o2_pct: 0.0,
            air_co2_pct: 0.0,
            air_pressure_atm: 0.0,
            air_temp_c: 0.0,
            air_breathable: false,
            showroom_active: false,
            show_roof: false,
            show_hull: true,
            construction_active: false,
            home_machines: None,
            home_machine_add_type: String::new(),
            home_conn_from: String::new(),
            home_conn_to: String::new(),
            home_conn_kind: "power".to_string(),
            conduit_from: String::new(),
            conduit_to: String::new(),
            conduit_kind: "water".to_string(),
            construction_console_input: String::new(),
            construction_console_output: String::new(),
            home_machines_save: false,
            construction_save_note: String::new(),
            construction_rooms: Vec::new(),
            construction_room_types: Vec::new(),
            construction_add_type: String::new(),
            construction_remove: None,
            construction_plan_view: false,
            keymap_visible: false,
            show_perf_overlay: false,
            show_network_overlay: false,
            show_system_overlay: false,
            frame_times: Vec::new(),
            ws_msgs_in: 0,
            mic_test_active: false,
            audio_input_device: String::new(),
            audio_output_device: String::new(),
            audio_input_devices: Vec::new(),
            audio_output_devices: Vec::new(),
            audio_devices_loaded: false,
            mic_meter: 0.0,
            mic_test_prev: false,
            voice_gain: 1.0,
            voice_filter_mode: crate::config::VoiceFilterMode::default(),
            voice_transmit_mode: crate::config::VoiceTransmitMode::default(),
            voice_ptt_key: "CapsLock".to_string(),
            voice_vad_threshold: 0.05,
            voice_ptt_held: false,
            voice_binding_key: false,
            diag_entity_count: 0,
            diag_mem_mb: 0.0,
            diag_uptime_secs: 0,
            keymaps: Vec::new(),
            construction_selected_room: None,
            construction_height: 3.0,
            construction_level: 0,
            construction_dirty: false,
            construction_machines_dirty: false,
            ship_structure: None,
            construction_zone: 0,
            construction_zone_delete_arm: false,
            construction_corridor_from_zone: 0,
            construction_corridor_from_door: 0,
            construction_corridor_to_zone: 0,
            construction_corridor_to_door: 0,
            construction_corridor_width: 3.0,
            construction_corridor_glass: false,
            construction_corridor_error: String::new(),
            construction_structure_dirty: false,
            construction_wall_mode: false,
            construction_wall_start: None,
            construction_cursor_world: None,
            construction_wall_selected: None,
            construction_machine_selected: None,
            construction_light_selected: None,
            build_char_pos: None,
            construction_grid_snap: true,
            construction_dev_overlay: false,
            gi_enabled: true,
            construction_sun_override: false,
            construction_sun_override_hour: 12.0,
            construction_undo_depth: 64,
            construction_palette_category: String::new(),
            construction_palette_expanded: false,
            construction_place_type: None,
            construction_structure_type: None,
            construction_place_conduit_node: false,
            zone_add_type: String::new(),
            construction_zone_selected: None,
            rail_edge_from: 0,
            rail_edge_to: 0,
            construction_structure_selected: None,
            construction_focus_request: None,
            construction_object_filter: String::new(),
            construction_multi: std::collections::HashSet::new(),
            construction_locked_types: std::collections::HashSet::new(),
            construction_hidden_types: std::collections::HashSet::new(),
            construction_road_node_selected: None,
            construction_conduit_node_selected: None,
            construction_connection_selected: None,
            construction_structure_yaw: 0.0,
            construction_structure_place_y: 0.0,
            construction_road_from: 0,
            construction_road_to: 0,
            construction_road_class: String::new(),
            construction_road_width: 4.0,
            construction_dimension_overlay: true,
            construction_show_helpers: true,
            construction_save: false,
            showroom_backdrop: 0,
            showroom_backdrop_names: Vec::new(),
            showroom_confirm: false,
            appearance: crate::ecs::components::Appearance::default(),
            appearance_dirty: false,
            outfit: crate::ecs::components::Outfit::default(),
            outfit_dirty: false,
            showroom_mode: 0,
            cosmetics_list: Vec::new(),
            character_name: "Wanderer".to_string(),
            settings_dirty: false,
            quit_requested: false,
            identity_recovered: false,
            private_key_bytes: None,
            history_fetched: false,
            passphrase_needed: false,
            passphrase_mode: PassphraseMode::Unlock,
            passphrase_input: String::new(),
            passphrase_confirm: String::new(),
            passphrase_old_input: String::new(),
            passphrase_status: String::new(),
            encrypted_private_key: String::new(),
            key_salt: String::new(),
            // Fresh GuiState has no vault yet; the new-encrypt path stamps
            // `PBKDF2_ITERATIONS_NEW` when the user first picks a passphrase.
            // A loaded legacy config overwrites this with its stored value
            // (defaults to 100_000 via serde for pre-v0.277.0 configs).
            key_iterations: crate::config::PBKDF2_ITERATIONS_NEW,
            section_locks: std::collections::HashMap::new(),
            // v0.278.0 auto-unlock — default is opt-out (always prompt).
            // A loaded config overwrites this with the user's stored choice.
            auto_unlock_mode: crate::auto_unlock::AutoUnlockMode::AlwaysPrompt,
            pin_encrypted_seed: String::new(),
            pin_salt: String::new(),
            remember_on_device: false,
            passphrase_unlocking: false,
            #[cfg(feature = "native")]
            passphrase_unlock_rx: None,
            pin_unlocking: false,
            #[cfg(feature = "native")]
            pin_unlock_rx: None,
            #[cfg(feature = "native")]
            clipboard_upload: None,
            pin_input: String::new(),
            pin_confirm: String::new(),
            pin_old_input: String::new(),
            pin_status: String::new(),
            kyber_public_b64: String::new(),
            peer_kyber_keys: std::collections::HashMap::new(),
            voice_channel_ids: std::collections::HashMap::new(),
            voice_rx_frames: 0,
            voice_active_room: None,
            voice_incumbents_captured: false,
            voice_connected_peers: std::collections::HashSet::new(),
            voice_session_prev: false,
            call_incoming: None,
            call_active: None,
            call_outgoing: None,
            call_outgoing_deadline: None,
            call_muted: false,
            chat_attach_picker: None,
            donate_solana_address: String::new(),
            donate_btc_address: String::new(),
            donate_addresses: Vec::new(),
            donate_addresses_server: Vec::new(),
            donate_funding_goal: None,
            donate_funding_server: String::new(),
            donate_info_rx: None,
            donate_new_network: String::new(),
            donate_new_type: "address".into(),
            donate_new_value: String::new(),
            donate_new_label: String::new(),
            chat_user_modal_open: false,
            chat_user_modal_name: String::new(),
            chat_user_modal_key: String::new(),
            show_create_channel_modal: false,
            show_add_server_modal: false,
            add_server_url_draft: String::new(),
            add_server_name_draft: String::new(),
            server_settings_tab: 0,
            server_settings_channel_drafts: std::collections::HashMap::new(),
            server_settings_new_channel: ChannelDraft::default(),
            new_channel_name: String::new(),
            new_channel_description: String::new(),
            show_create_group_modal: false,
            dm_settings_popup_open: false,
            groups_settings_popup_open: false,
            notif_dm_enabled: true,
            notif_mentions_enabled: true,
            notif_tasks_enabled: true,
            notif_dnd_start: None,
            notif_dnd_end: None,
            notif_prefs_loaded: false,
            dm_unencrypted_confirm: None,
            server_settings: None,
            chat_roles: Vec::new(),
            service_state: Vec::new(),
            roles_drafts: std::collections::HashMap::new(),
            new_role_draft: {
                // Blank template for a new custom role: empty id (operator
                // types one), sensible non-privileged defaults.
                let mut r = crate::relay::storage::RoleDef::default();
                r.id = String::new();
                r.label = String::new();
                r.color = "#7E57C2".to_string();
                r.trust_level = 1;
                r.built_in = false;
                r.can_stream = false;
                r.can_upload = true;
                r.can_voice = true;
                r.can_image_share = true; // a typical custom role (e.g. "family") shares media
                r.can_file_share = true;
                r.base_tier = "verified".to_string();
                r.sort_order = 50;
                r
            },
            chat_banned_users: Vec::new(),
            chat_banned_requested: false,
            // Game admin (game-world bans, separate from chat bans)
            game_bans: Vec::new(),
            game_bans_requested: false,
            game_admin_target_key: String::new(),
            game_admin_ban_reason: String::new(),
            game_admin_status: String::new(),
            // Character launcher
            launcher_saves: Vec::new(),
            launcher_saves_loaded: false,
            launcher_selected: String::new(),
            launcher_default_character: String::new(),
            launcher_pending_load: None,
            launcher_open_select: false,
            launcher_selected_kind: LauncherSel::Home,
            launcher_selected_server: None,
            showroom_cancel: false,
            server_info_cache: std::collections::HashMap::new(),
            server_info_loader: None,
            chat_muted_users: Vec::new(),
            chat_muted_requested: false,
            server_settings_draft: None,
            cosmos_view: crate::gui::pages::cosmos::CosmosView::System,
            cosmos_pan: egui::Vec2::ZERO,
            cosmos_zoom: 1.0,
            cosmos_selected_body: None,
            cosmos_expanded_planets: std::collections::HashSet::new(),
            cosmos_focus_request: None,
            cosmos_tracked_body: None,
            cosmos_camera_3d: crate::gui::pages::cosmos::Cosmos3DCamera::default(),
            cosmos_sim_time_seconds: 0.0,
            cosmos_sim_speed: 0.0, // Paused by default — operator scrubs / plays.
            cosmos_last_real_instant: None,
            cosmos_sim_time_initialized: false,
            cosmos_upcoming_events: Vec::new(),
            cosmos_upcoming_scan_origin: f64::NAN, // forces first scan
            cosmos_upcoming_last_scan: None,
            cosmos_expanded_body: None,
            cosmos_show_lagrange: false,
            cosmos_show_reference_orbits: false,
            new_group_name: String::new(),
            new_group_share_history: false,
            create_group_ticket: None,
            create_group_status: String::new(),
            show_join_group_modal: false,
            join_group_invite_code: String::new(),
            join_group_status: String::new(),
            join_group_result: None,
            p2p_groups: Vec::new(),
            p2p_groups_last_fetch: None,
            p2p_group_invite_status: String::new(),
            p2p_group_active_id: String::new(),
            p2p_group_chat_epoch: 0,
            p2p_group_chat_epoch_key: None,
            p2p_group_last_fetch: None,
            p2p_group_fp_to_key: std::collections::HashMap::new(),
            p2p_group_fp_to_name: std::collections::HashMap::new(),
            #[cfg(feature = "native")]
            p2p_group_loader: None,
            #[cfg(feature = "native")]
            p2p_groups_list_loader: None,
            p2p_group_loading: false,
            #[cfg(feature = "native")]
            p2p_group_seen_obj_ids: std::collections::HashSet::new(),
            show_channel_edit_modal: false,
            edit_channel_id: String::new(),
            edit_channel_name: String::new(),
            edit_channel_description: String::new(),
            edit_channel_confirm_delete: false,
            server_settings_target_user: String::new(),
            server_settings_channel_name: String::new(),
            server_settings_invite_code: String::new(),
            redeem_code_draft: String::new(),
            revoke_key_draft: String::new(),
            server_settings_status: String::new(),
            system_health: None,
            system_health_status: String::new(),
            system_health_rx: None,
            federation_servers: Vec::new(),
            federation_status: String::new(),
            federation_rx: None,
            federation_add_url_draft: String::new(),
            federation_add_name_draft: String::new(),
            server_settings_confirm_action: None,
            show_help_modal: false,
            debug_console_visible: false,
            debug_log: Vec::new(),
            tools_catalog: Vec::new(),
            equipment_slots: Vec::new(),
            bug_severities: Vec::new(),
            bug_categories: Vec::new(),
            crafting_category_groups: Vec::new(),
            market_categories: Vec::new(),
            library: Vec::new(),
            studio_scene_presets: Vec::new(),
            studio_source_presets: Vec::new(),
            studio_streaming_config: StudioStreamingConfig::default(),
            donate_faq: Vec::new(),
            qa_test_tasks: Vec::new(),
            qa_test_status: std::collections::HashMap::new(),
            qa_test_note: std::collections::HashMap::new(),
            qa_test_filter: "all".to_string(),
            browser_bookmarks: Vec::new(),
            browser_filter: "all".to_string(),
            // v0.174.0: default to two-tier nav for fresh installs. Existing
            // users with `nav_two_tier=false` saved in config keep their
            // legacy layout until they flip via [▤]; new sessions land on
            // the two-tier layout immediately.
            nav_two_tier: true,
            nav_top_category: "reality".to_string(),
            attack_pulse_active: false,
            attack_pulse_last_hit_at: 0.0,
            player_death_cause: None,
            pending_respawn: false,
            // v0.197.0: ai_usage_filters removed (page deleted).
            help_registry: crate::gui::widgets::help_modal::HelpRegistry::new(),
            active_help_topic: None,
            onboarding_quest_chains: Vec::new(),
            onboarding_quest_progress: std::collections::HashMap::new(),
            image_cache: crate::gui::widgets::image_cache::ImageCache::new(),
            image_viewer_url: None,
            studio: StudioState::default(),

            // Chat panel collapse state (all expanded by default except connection)
            chat_connection_collapsed: true,
            chat_dm_collapsed: false,
            chat_groups_collapsed: false,
            chat_servers_collapsed: false,
            chat_connected_server_collapsed: false,
            chat_friends_collapsed: false,
            chat_members_collapsed: false,
            chat_studio_collapsed: false,
            chat_open_popup_ts: None,
            pending_clipboard_paste: false,
            chat_mention_index: 0,
            chat_dm_display_limit: 5,

            // Chat panel resize/lock state
            chat_left_panel_locked: false,
            chat_right_panel_locked: false,
            chat_left_panel_width: 220.0,
            chat_right_panel_width: 220.0,

            // Identity / Governance / Recovery page state (v0.115.0)
            identity_lookup_did: String::new(),
            identity_lookup_pending: false,
            governance_scope_tab: 0,
            governance_proposals: Vec::new(),
            governance_rx: None,
            governance_fetched_for: String::new(),
            governance_refresh: false,
            governance_error: String::new(),
            governance_my_votes: std::collections::HashMap::new(),
            governance_vote_rx: None,
            governance_vote_status: String::new(),
            governance_propose_rx: None,
            governance_show_propose: false,
            governance_new_title: String::new(),
            governance_new_body: String::new(),
            governance_new_type_idx: 0,
            governance_new_scope_idx: 0,
            governance_new_days: 7.0,
            laws_location: String::new(),
            laws_search: String::new(),
            laws_filter_tab: 0,
            laws_category: String::new(),
            governance_filter_tab: 0,
            recovery_lookup_did: String::new(),
            recovery_lookup_pending: false,
            recovery_guardian_did: String::new(),
            recovery_guardian_pending: false,

            // v0.197.0: AI Usage page form state removed (page deleted).
        }
    }
}

/// Load the tools catalog from data/tools/catalog.json.
/// `data_dir` is the root data directory (e.g. from AssetManager).
/// Returns an empty Vec on any error (graceful degradation).
#[cfg(feature = "native")]
pub fn load_tools_catalog(data_dir: &std::path::Path) -> Vec<ToolEntry> {
    /// JSON shape for the catalog file (categories with nested tools).
    #[derive(serde::Deserialize)]
    struct Catalog {
        categories: Vec<CatalogCategory>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogCategory {
        name: String,
        tools: Vec<ToolEntry>,
        #[allow(dead_code)]
        #[serde(default)]
        id: String,
        #[allow(dead_code)]
        #[serde(default)]
        extensions: Vec<String>,
    }

    let path = data_dir.join("tools").join("catalog.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[tools] Failed to read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let catalog: Catalog = match serde_json::from_str(&bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[tools] Failed to parse catalog.json: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for cat in catalog.categories {
        for mut tool in cat.tools {
            tool.category = cat.name.clone();
            out.push(tool);
        }
    }
    out
}

/// Load solar system planets from `data/solar_system/bodies.json`.
/// Falls back to an empty Vec if the file is missing or malformed so the
/// game still boots (the map page will show a "no planets loaded" state).
#[cfg(feature = "native")]
pub fn load_planets(data_dir: &std::path::Path) -> Vec<GuiPlanet> {
    #[derive(serde::Deserialize)]
    struct Bodies {
        bodies: Vec<Body>,
    }
    #[derive(serde::Deserialize)]
    struct Body {
        #[serde(default)]
        name: String,
        #[serde(default, rename = "type")]
        type_: String,
        #[serde(default)]
        physical: Option<Physical>,
        #[serde(default)]
        orbit: Option<Orbit>,
        #[serde(default)]
        atmosphere: Option<Atmosphere>,
        #[serde(default)]
        moons: Vec<serde_json::Value>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Physical {
        #[serde(default)]
        radius_km: f64,
        #[serde(default)]
        surface_gravity_ms2: f64,
    }
    #[derive(serde::Deserialize, Default)]
    struct Orbit {
        #[serde(default)]
        semi_major_axis_au: f64,
    }
    #[derive(serde::Deserialize, Default)]
    struct Atmosphere {
        #[serde(default)]
        composition: std::collections::BTreeMap<String, f64>,
        #[serde(default)]
        surface_pressure_atm: Option<f64>,
    }

    let path = data_dir.join("solar_system").join("bodies.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[planets] Could not read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let parsed: Bodies = match serde_json::from_str(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[planets] Could not parse bodies.json: {}", e);
            return Vec::new();
        }
    };

    parsed
        .bodies
        .into_iter()
        .filter(|b| {
            // Show real planets and dwarf planets. Stars have no orbit_au,
            // moons have a parent != "sun". We display the simple solar
            // system view so we skip the Sun itself.
            matches!(
                b.type_.as_str(),
                "terrestrial" | "gas_giant" | "ice_giant" | "dwarf_planet"
            )
        })
        .map(|b| {
            let phys = b.physical.unwrap_or_default();
            let orbit = b.orbit.unwrap_or_default();
            let atm = b.atmosphere.unwrap_or_default();
            let atm_str = if atm.composition.is_empty() {
                if atm.surface_pressure_atm.map_or(true, |p| p < 0.001) {
                    "None".to_string()
                } else {
                    "Trace".to_string()
                }
            } else {
                // Join top 3 components by percentage, descending.
                let mut pairs: Vec<(String, f64)> =
                    atm.composition.into_iter().collect();
                pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                pairs
                    .into_iter()
                    .take(3)
                    .map(|(k, _)| k)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let planet_type = match b.type_.as_str() {
                "terrestrial" => "Rocky",
                "gas_giant" => "Gas Giant",
                "ice_giant" => "Ice Giant",
                "dwarf_planet" => "Dwarf",
                other => other,
            }
            .to_string();

            GuiPlanet {
                name: b.name,
                planet_type,
                radius_km: phys.radius_km,
                gravity: phys.surface_gravity_ms2,
                atmosphere: atm_str,
                moons: b.moons.len() as u32,
                orbit_radius_au: orbit.semi_major_axis_au,
            }
        })
        .collect()
}

// ─── Infinite-of-X data loaders (v0.123.0) ─────────────────────────────────
//
// One small JSON file per page taxonomy. All loaders share the same shape:
// graceful fallback to an empty Vec on missing/malformed input so the GUI still
// boots — pages render an empty filter row instead of crashing. The empty-vec
// path is also what the page sees during the brief window before lib.rs wires
// the loaders into GuiState at startup.

// v0.415.0: ResourceEntry / ResourceCategory / load_resource_categories removed
// with the Resources page (retired into the Library, which loads the shared
// data/resources/catalog.json itself).

/// A streaming-studio scene preset.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioScenePreset {
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub source_visibility: Vec<bool>,
}

/// A streaming-studio source preset. Kinds: `camera|screen|microphone|chat_overlay|image|text|timer`.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StudioSourcePreset {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub device: u32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub position: (f32, f32),
    #[serde(default)]
    pub size: (f32, f32),
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub z_order: u32,
}
#[cfg(feature = "native")]
fn one() -> f32 { 1.0 }

/// Convert a deserialised preset into the runtime [`StudioSource`].
/// Unknown `kind` values fall back to `Camera(0)` — the most benign default.
#[cfg(feature = "native")]
pub fn studio_source_from_preset(p: &StudioSourcePreset) -> StudioSource {
    let source_type = match p.kind.as_str() {
        "camera" => StudioSourceType::Camera(p.device),
        "screen" => StudioSourceType::Screen(p.device),
        "microphone" => StudioSourceType::Microphone(p.device),
        "chat_overlay" => StudioSourceType::ChatOverlay,
        "image" => StudioSourceType::Image(p.text.clone()),
        "text" => StudioSourceType::Text(p.text.clone()),
        "timer" => StudioSourceType::Timer,
        _ => StudioSourceType::Camera(p.device),
    };
    StudioSource {
        name: p.name.clone(),
        source_type,
        visible: p.visible,
        position: p.position,
        size: p.size,
        opacity: p.opacity,
        z_order: p.z_order,
    }
}

/// Convert a deserialised preset into the runtime [`StudioScene`].
#[cfg(feature = "native")]
pub fn studio_scene_from_preset(p: &StudioScenePreset) -> StudioScene {
    StudioScene {
        name: p.name.clone(),
        is_default: p.is_default,
        source_visibility: p.source_visibility.clone(),
    }
}

/// Read a JSON file under `data/` and deserialise into `T`. Logs and returns
/// `None` on any error so callers can fall back gracefully. Disk-first,
/// embedded fallback (v0.744) — zero-file installs keep their data-driven UI.
#[cfg(feature = "native")]
fn read_data_json<T: serde::de::DeserializeOwned>(
    data_dir: &std::path::Path,
    relative: &str,
) -> Option<T> {
    let path = data_dir.join(relative);
    let bytes = match crate::embedded_data::read_data_or_embedded(data_dir, relative) {
        Some(b) => b,
        None => {
            eprintln!("[data] failed to read {} (no embedded copy)", path.display());
            return None;
        }
    };
    match serde_json::from_str::<T>(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[data] failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

/// Load equipment slot definitions from `data/inventory/equipment_slots.json`.
#[cfg(feature = "native")]
pub fn load_equipment_slots(data_dir: &std::path::Path) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Slot { id: String, label: String }
    #[derive(serde::Deserialize)]
    struct File { slots: Vec<Slot> }
    read_data_json::<File>(data_dir, "inventory/equipment_slots.json")
        .map(|f| f.slots.into_iter().map(|s| (s.id, s.label)).collect())
        .unwrap_or_default()
}

/// A node in the uniform entity/place/container model — the operator's "mark
/// Earth as my container" idea, generalised: ONE recursive shape spans the whole
/// scale. Top-level entries are ENTITIES (you, your home, a vehicle); each is a
/// container holding rooms / sub-containers / items, any depth. A planet, a
/// building, a backpack, and a toothbrush are all just a `Place` with children —
/// the same nesting top to bottom.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Place {
    /// Stable id (optional — items can omit it). Used by future in-app editing
    /// and `location` references.
    #[serde(default)]
    pub id: String,
    pub label: String,
    /// person | vehicle | building | property | floor | room | container |
    /// backpack | pack | duffel | bag | pouch | planet | region | locale | item
    /// | … — free-form so the DATA leads, not code (drives the node colour too).
    #[serde(default)]
    pub kind: String,
    /// Soft location reference (a label or another node's id) — e.g. a vehicle
    /// "@ Home". Shown as detail, NOT a hard tree edge, so an entity can sit at
    /// the top level yet still say where it is without deep nesting.
    #[serde(default)]
    pub location: Option<String>,
    /// `[latitude, longitude]` for geographic nodes; the bridge to real-terrain
    /// world-gen — the point that says "render THIS hillside here".
    #[serde(default)]
    pub coordinate: Option<[f64; 2]>,
    /// Leaf items held DIRECTLY in this container, by item id (resolved against
    /// items.csv for the name/details). The nested-container inventory renders these
    /// as tiles; sub-containers go in `children`. A pocket might hold `["pen_0"]`
    /// plus a `keychain` child container. Empty for pure location/spine nodes (the
    /// live backpack injects its items at the node marked `kind: "backpack"`).
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub children: Vec<Place>,
}

/// Load the seeded entities from `data/places/seed.json` — top-level entries
/// (You, your home, a vehicle, …), each a container with its own contents and an
/// optional `location`. Empty vec if absent (callers fall back to a flat view).
pub fn load_places(data_dir: &std::path::Path) -> Vec<Place> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        entities: Vec<Place>,
    }
    read_data_json::<File>(data_dir, "places/seed.json")
        .map(|f| f.entities)
        .unwrap_or_default()
}

/// One item placed in a container, for the organize-layer inventory (operator
/// 2026-06-22: "one item pool; each item records WHICH container it's in", and
/// transfer = move it between containers). `container` is the container's PATH in the
/// places tree (e.g. "1/0/0"), so a transfer is just changing this string. Seeded from
/// the places spine at load; serializable so a save can persist transfers. The live
/// backpack is NOT in this pool (its items come from the ECS) until the ECS-boundary
/// transfer lands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacedItem {
    /// Item id (resolves against items.csv) OR a descriptive label for seed items.
    pub key: String,
    /// Display name (item name if `key` is an id; else the label).
    pub name: String,
    pub qty: u32,
    /// Container PATH in the places tree this item currently sits in.
    pub container: String,
}

/// Flatten the places spine into the organize-layer item pool: every leaf `kind:"item"`
/// child and id-based `items` entry becomes a [`PlacedItem`] tagged with its container's
/// PATH (the same scheme the inventory renderer walks). The live backpack is excluded.
pub fn flatten_placed_items(places: &[Place]) -> Vec<PlacedItem> {
    fn walk(place: &Place, path: &str, out: &mut Vec<PlacedItem>) {
        for (j, child) in place.children.iter().enumerate() {
            if child.kind == "item" {
                out.push(PlacedItem {
                    key: child.label.clone(),
                    name: child.label.clone(),
                    qty: 1,
                    container: path.to_string(),
                });
            } else {
                walk(child, &format!("{path}/{j}"), out);
            }
        }
        for id in &place.items {
            out.push(PlacedItem { key: id.clone(), name: id.clone(), qty: 1, container: path.to_string() });
        }
    }
    let mut out = Vec::new();
    for (i, p) in places.iter().enumerate() {
        walk(p, &i.to_string(), &mut out);
    }
    out
}

/// Collect (path, label) for every CONTAINER in the places spine (not leaf items, and
/// not the live-only backpack), for the "Move to..." transfer menu. Path matches the
/// inventory renderer's scheme.
pub fn collect_containers(places: &[Place]) -> Vec<(String, String)> {
    fn walk(place: &Place, path: &str, out: &mut Vec<(String, String)>) {
        if place.kind != "backpack" {
            out.push((path.to_string(), place.label.clone()));
        }
        for (j, child) in place.children.iter().enumerate() {
            if child.kind != "item" {
                walk(child, &format!("{path}/{j}"), out);
            }
        }
    }
    let mut out = Vec::new();
    for (i, p) in places.iter().enumerate() {
        walk(p, &i.to_string(), &mut out);
    }
    out
}

// ── Homestead Design (the "homes" feature, offline-first; v0.379) ──
// The Fibonacci homestead blueprint (data/blueprints/fibonacci_homestead.ron),
// surfaced read-only as a browsable Design: rooms carry their materials (the bill
// of materials / parts list), power, and water needs, so the Home page can total
// the demand + parts and show how self-sufficient the build is. More designs can
// drop in as data later. See pages/homes.rs + docs/design/homes-as-profiles.md.

/// A whole homestead blueprint (one "Design").
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HomesteadDesign {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub rooms: Vec<DesignRoom>,
    #[serde(default)]
    pub tiers: Vec<DesignTier>,
    #[serde(default)]
    pub build_order: Vec<String>,
    #[serde(default)]
    pub scaling_notes: String,
}

/// One room in a homestead Design, with its build requirements.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DesignRoom {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: Size3,
    #[serde(default)]
    pub fibonacci_index: u32,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub requirements: RoomRequirements,
    #[serde(default)]
    pub environment_notes: String,
}

/// Room dimensions in metres.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Size3 {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
}

/// What a room needs to build + run: a bill of materials, plus power + water draw.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RoomRequirements {
    /// (item_id, quantity) pairs — the proto bill-of-materials.
    #[serde(default)]
    pub materials: Vec<(String, u32)>,
    #[serde(default)]
    pub power_watts: u32,
    #[serde(default)]
    pub water_liters_per_day: u32,
}

/// A construction tier (core / residential / industrial / exterior).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DesignTier {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub wall_thickness_cm: u32,
    #[serde(default)]
    pub radiation_shielding: bool,
}

/// Load the Fibonacci homestead blueprint. None if absent/unparseable (the Home
/// page then shows an empty state). Reads RON directly, like src/ship/fibonacci.rs.
pub fn load_homestead_design(data_dir: &std::path::Path) -> Option<HomesteadDesign> {
    let path = data_dir.join("blueprints/fibonacci_homestead.ron");
    let text =
        crate::embedded_data::read_data_or_embedded(data_dir, "blueprints/fibonacci_homestead.ron")?;
    match ron::from_str::<HomesteadDesign>(&text) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("load_homestead_design: failed to parse {}: {e}", path.display());
            None
        }
    }
}

// ── Aeroponic tower configs (the homestead food loop; v0.382) ──
// Two curated 50-slot vertical aeroponic towers (nutrition + apothecary), loaded
// from data/towers/aeroponic_configs.ron. Each planting references an existing
// plant id in plants.csv. Browsed on the Home page; the 3D placeholder + planting
// integration come later. See docs/design/self-sufficiency.md.

/// One aeroponic tower configuration (a curated 50-slot plant set).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Make / model / version, shown in the tower's title row (operator 2026-06-08:
    /// "aeroponic tower make model version"). Data-driven so the community can brand
    /// their own designs; empty strings just hide that part of the title.
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub covers: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub gaps_note: String,
    #[serde(default)]
    pub disclaimer: String,
    #[serde(default)]
    pub slots: u32,
    /// 3D geometry for the placeholder (and the eventual real design; operator
    /// 2026-06-07: design + plant amount should be dynamic + scale infinitely). The
    /// column DIAMETER + HEIGHT in metres, and how many times the plant HELIX wraps
    /// the column: low helix_turns = coarse / spread out, high = fine / dense, like
    /// thread pitch on a bolt. A wide diameter + fine helix packs more plants.
    #[serde(default = "default_diameter_m")]
    pub diameter_m: f32,
    #[serde(default = "default_height_m")]
    pub height_m: f32,
    #[serde(default = "default_helix_turns")]
    pub helix_turns: f32,
    #[serde(default)]
    pub plantings: Vec<TowerPlanting>,
    /// Real-world parts to BUILD this tower (the game->real bridge / north star:
    /// every in-game system maps to a real buildable thing). Optional starting
    /// bill of materials; refine the parts, quantities, and sources for your build.
    #[serde(default)]
    pub parts: Vec<TowerPart>,
}

fn default_diameter_m() -> f32 {
    0.4
}
fn default_height_m() -> f32 {
    2.0
}
fn default_helix_turns() -> f32 {
    4.0
}

/// One plant assigned to N slots of a tower, with its role + a nutrition/medicinal note.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerPlanting {
    #[serde(default)]
    pub plant: String,
    #[serde(default)]
    pub slots: u32,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub note: String,
}

/// One real-world component needed to BUILD a tower (the game->real bridge).
/// `source` is how you would obtain it: "buy" / "3d_print" / "diy" / "trade" /
/// "scavenge". A starting list; refine for your build.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TowerPart {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub qty: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub note: String,
}

/// A garden grow AREA kind + how many the homestead has, its per-unit food output,
/// and footprint. Loaded from the garden room of `data/machines/home.ron` and shown
/// in the Inventory Garden overview + per-medium edit modal.
#[derive(Debug, Clone, Default)]
pub struct GardenArea {
    pub label: String,
    pub machine_id: String,
    pub count: u32,
    pub food: String,
    /// Footprint (w, h, d) in meters from the machine catalog.
    pub size: (f32, f32, f32),
}

/// Count every growing machine in the garden room of `data/machines/home.ron` (a
/// "grow" machine has a food stat but is not pure storage like the silo), grouped by
/// type, with its catalog label / food stat / footprint. Resolved via `data_dir` so it
/// works regardless of the process CWD. Empty if the file is absent.
pub fn load_garden_areas(data_dir: &std::path::Path) -> Vec<GardenArea> {
    let path = crate::machines::home_ron_path(data_dir);
    let Some(home) = crate::machines::MachineHome::load(&path) else {
        return Vec::new();
    };
    let is_grow = |machine: &str| {
        home.catalog.get(machine).map_or(false, |d| {
            d.stats.iter().any(|s| s.kind == "food") && !d.stats.iter().any(|s| s.kind == "storage")
        })
    };
    // v0.538: count EVERY grow machine, not just those in a literal "garden" room. The HomeStructure
    // home's rooms are flood-fill ids (home/room_1/...) that never equal "garden", so the old
    // room-name filter silently emptied the garden inventory. The is_grow catalog predicate is the
    // real signal; room membership is not.
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for inst in &home.instances {
        if is_grow(&inst.machine) {
            *counts.entry(inst.machine.clone()).or_insert(0) += 1;
        }
    }
    for arr in &home.arrays {
        if is_grow(&arr.machine) {
            *counts.entry(arr.machine.clone()).or_insert(0) += arr.rows * arr.cols;
        }
    }
    let mut out: Vec<GardenArea> = counts
        .into_iter()
        .map(|(machine, count)| {
            let def = home.catalog.get(&machine);
            let label = def.map(|d| d.label.clone()).unwrap_or_else(|| machine.clone());
            let food = def
                .and_then(|d| d.stats.iter().find(|s| s.kind == "food"))
                .map(|s| s.value.clone())
                .unwrap_or_default();
            let size = def.map(|d| d.size).unwrap_or((0.0, 0.0, 0.0));
            GardenArea { label, machine_id: machine, count, food, size }
        })
        .collect();
    // Most-numerous first, then by name, so the overview reads stably frame to frame.
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.label.cmp(&b.label)));
    out
}

/// One control in a grow medium's edit form (rendered top-to-bottom in the modal).
#[derive(Debug, Clone, serde::Deserialize)]
pub enum GrowControl {
    /// A 0..1 slider stored under `key` (water / nutrient / humidity / ...).
    Slider { key: String, label: String },
    /// A free-text field for the primary crop / species / fish.
    Crop { label: String, hint: String },
    /// A checkbox stored under `key`.
    Toggle { key: String, label: String },
}

/// A grow MEDIUM: a way crops are grown (aeroponic, soil bed, field, ...), matched to a
/// garden machine by id, with the controls its edit modal shows. Data-driven from
/// `data/garden/grow_media.ron` so plot-types are added without code (infinite-of-X).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GrowMedium {
    pub id: String,
    #[serde(default)]
    pub match_prefix: Option<String>,
    #[serde(default)]
    pub match_suffix: Option<String>,
    #[serde(default)]
    pub match_exact: Option<String>,
    pub label: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub show_slots: bool,
    /// Plant id (data/plants.csv) the bed/tray/field Plant button sows when the
    /// user hasn't typed a crop into the edit modal (v0.738 grain loop).
    #[serde(default)]
    pub default_crop: Option<String>,
    #[serde(default)]
    pub controls: Vec<GrowControl>,
}

impl GrowMedium {
    /// Does this medium apply to the given machine id? (exact, then prefix, then suffix.)
    pub fn matches(&self, machine_id: &str) -> bool {
        self.match_exact.as_deref() == Some(machine_id)
            || self.match_prefix.as_deref().is_some_and(|p| machine_id.starts_with(p))
            || self.match_suffix.as_deref().is_some_and(|s| machine_id.ends_with(s))
    }
}

/// Load the grow-media registry (data/garden/grow_media.ron). Empty on absence/parse error.
pub fn load_grow_media(data_dir: &std::path::Path) -> Vec<GrowMedium> {
    #[derive(serde::Deserialize)]
    struct File {
        media: Vec<GrowMedium>,
    }
    match crate::embedded_data::read_data_or_embedded(data_dir, "garden/grow_media.ron") {
        Some(t) => match ron::from_str::<File>(&t) {
            Ok(f) => f.media,
            Err(e) => {
                log::warn!("grow_media parse failed: {e}");
                Vec::new()
            }
        },
        None => Vec::new(),
    }
}

/// Load the aeroponic tower configs (data/towers/aeroponic_configs.ron). Empty on
/// absence/parse error.
pub fn load_tower_configs(data_dir: &std::path::Path) -> Vec<TowerConfig> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        towers: Vec<TowerConfig>,
    }
    let path = data_dir.join("towers/aeroponic_configs.ron");
    let text = match crate::embedded_data::read_data_or_embedded(data_dir, "towers/aeroponic_configs.ron")
    {
        Some(t) => t,
        None => return Vec::new(),
    };
    match ron::from_str::<File>(&text) {
        Ok(f) => f.towers,
        Err(e) => {
            eprintln!("load_tower_configs: failed to parse {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Whether the plants in one tower can share a single reservoir + air — the
/// operator's "make sure they grow together". Aeroponics shares one nutrient
/// reservoir and one air volume (NOT soil), so soil companion/adverse rules
/// relax and the real constraint becomes a COMMON pH / temperature / humidity
/// window every plant tolerates. Each axis here is the intersection of the
/// per-plant windows (from plants.csv): `Some((lo, hi))` means all plants
/// overlap and can share it; `None` means no shared window (a conflict), and
/// `conflicts` names the binding extremes to reconsider.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct TowerCompat {
    /// Distinct species considered (those found in the plant registry).
    pub species: usize,
    /// Shared reservoir pH window, °C temperature window, and 0..1 humidity
    /// window. `None` on an axis = the plants have no common window there.
    pub ph: Option<(f32, f32)>,
    pub temp: Option<(f32, f32)>,
    pub humidity: Option<(f32, f32)>,
    /// One note per conflicting axis, naming the two binding plants, e.g.
    /// "Temp: Rosemary (warm) vs Lettuce (cool) have no overlap".
    pub conflicts: Vec<String>,
    /// Total daily water draw across EVERY slot (L/day) — feeds the homestead's
    /// self-sufficiency water loop. 0 if no plant water data.
    pub water_per_day_total: f32,
    /// Harvest window: soonest and latest species maturity in days (0 if unknown).
    pub first_harvest_days: f32,
    pub full_harvest_days: f32,
}

/// Intersect one window axis across a tower's plants. Degenerate windows
/// (`hi <= lo`, i.e. an unset 0..0 column) are skipped so missing data can't
/// fake a conflict. Returns the shared window if all valid windows overlap,
/// else `None` plus a note naming the warmest-floor and coolest-ceiling plants.
#[cfg(feature = "native")]
fn intersect_axis(windows: &[(String, (f32, f32))], label: &str) -> (Option<(f32, f32)>, Option<String>) {
    let valid: Vec<&(String, (f32, f32))> =
        windows.iter().filter(|(_, (lo, hi))| hi > lo).collect();
    if valid.is_empty() {
        return (None, None);
    }
    let lo = valid.iter().map(|(_, (l, _))| *l).fold(f32::MIN, f32::max);
    let hi = valid.iter().map(|(_, (_, h))| *h).fold(f32::MAX, f32::min);
    if lo <= hi {
        return (Some((lo, hi)), None);
    }
    // Conflict: the plant with the highest floor vs the one with the lowest ceiling.
    let warm = valid
        .iter()
        .max_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    let cool = valid
        .iter()
        .min_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
    (
        None,
        Some(format!("{}: {} vs {} have no overlap", label, warm.0, cool.0)),
    )
}

/// Compute a tower's shared-reservoir compatibility from the plant registry.
/// Plants not found in the registry are skipped (so a partial registry still
/// gives a useful answer for the plants it knows).
#[cfg(feature = "native")]
pub fn compute_tower_compat(
    tower: &TowerConfig,
    reg: &crate::systems::farming::PlantRegistry,
) -> TowerCompat {
    // Distinct plant ids (a max-variety tower is mostly distinct already).
    let mut ids: Vec<String> = Vec::new();
    for p in &tower.plantings {
        if !ids.contains(&p.plant) {
            ids.push(p.plant.clone());
        }
    }
    // (name, ph window, temp window, humidity window) for each known species.
    let mut ph_w = Vec::new();
    let mut temp_w = Vec::new();
    let mut hum_w = Vec::new();
    let mut species = 0usize;
    for id in &ids {
        if let Some(d) = reg.get(id) {
            species += 1;
            ph_w.push((d.name.clone(), (d.ph_min, d.ph_max)));
            temp_w.push((d.name.clone(), (d.temp_min_c, d.temp_max_c)));
            hum_w.push((d.name.clone(), (d.humidity_min, d.humidity_max)));
        }
    }
    let (ph, ph_c) = intersect_axis(&ph_w, "pH");
    let (temp, temp_c) = intersect_axis(&temp_w, "Temp");
    let (humidity, hum_c) = intersect_axis(&hum_w, "Humidity");
    let conflicts: Vec<String> = [ph_c, temp_c, hum_c].into_iter().flatten().collect();
    // Total daily water draw across ALL slots (not distinct species), and the
    // harvest window across the distinct species.
    let mut water_per_day_total = 0.0f32;
    for p in &tower.plantings {
        if let Some(d) = reg.get(&p.plant) {
            water_per_day_total += d.water_per_day * p.slots.max(1) as f32;
        }
    }
    let growth: Vec<f32> = ids
        .iter()
        .filter_map(|id| reg.get(id))
        .map(|d| d.growth_days)
        .filter(|g| *g > 0.0)
        .collect();
    let (first_harvest_days, full_harvest_days) = if growth.is_empty() {
        (0.0, 0.0)
    } else {
        (
            growth.iter().cloned().fold(f32::MAX, f32::min),
            growth.iter().cloned().fold(0.0_f32, f32::max),
        )
    };
    TowerCompat {
        species,
        ph,
        temp,
        humidity,
        conflicts,
        water_per_day_total,
        first_harvest_days,
        full_harvest_days,
    }
}

#[cfg(all(test, feature = "native"))]
mod tower_compat_tests {
    use super::*;
    use crate::systems::farming::PlantRegistry;

    fn reg_from(csv: &[u8]) -> PlantRegistry {
        PlantRegistry::from_csv(csv).expect("parse")
    }

    fn tower_of(ids: &[&str]) -> TowerConfig {
        let mut t = TowerConfig {
            id: "t".into(),
            name: "T".into(),
            make: String::new(),
            model: String::new(),
            version: String::new(),
            purpose: String::new(),
            description: String::new(),
            covers: vec![],
            gaps: vec![],
            gaps_note: String::new(),
            disclaimer: String::new(),
            slots: 50,
            diameter_m: 0.4,
            height_m: 2.0,
            helix_turns: 4.0,
            plantings: vec![],
            parts: vec![],
        };
        for id in ids {
            t.plantings.push(TowerPlanting {
                plant: (*id).into(),
                slots: 1,
                role: String::new(),
                note: String::new(),
            });
        }
        t
    }

    #[test]
    fn compatible_plants_share_a_window() {
        // Two plants with overlapping pH/temp/humidity windows → one shared window,
        // no conflicts.
        let csv = b"id,name,growth_days,water_liters_per_day,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max\n\
                    lettuce,Lettuce,45,0.5,6.0,7.0,10,22,0.5,0.8\n\
                    spinach,Spinach,40,0.6,6.2,7.2,8,24,0.5,0.9\n";
        let c = compute_tower_compat(&tower_of(&["lettuce", "spinach"]), &reg_from(csv));
        assert_eq!(c.species, 2);
        assert_eq!(c.ph, Some((6.2, 7.0)));
        assert_eq!(c.temp, Some((10.0, 22.0)));
        assert!(c.conflicts.is_empty(), "no conflict expected, got {:?}", c.conflicts);
        // Water draw sums per slot; the harvest window spans soonest..latest.
        assert!((c.water_per_day_total - 1.1).abs() < 1e-6, "water {}", c.water_per_day_total);
        assert!((c.first_harvest_days - 40.0).abs() < 1e-6, "first {}", c.first_harvest_days);
        assert!((c.full_harvest_days - 45.0).abs() < 1e-6, "full {}", c.full_harvest_days);
    }

    #[test]
    fn non_overlapping_temp_is_flagged() {
        // A warm herb and a cool green that can't share an air temperature.
        let csv = b"id,name,ph_min,ph_max,temp_min_c,temp_max_c,humidity_min,humidity_max\n\
                    rosemary,Rosemary,6.0,7.0,20,30,0.3,0.6\n\
                    lettuce,Lettuce,6.0,7.0,8,18,0.5,0.8\n";
        let c = compute_tower_compat(&tower_of(&["rosemary", "lettuce"]), &reg_from(csv));
        assert!(c.temp.is_none(), "temp should conflict");
        assert_eq!(c.conflicts.len(), 1);
        assert!(c.conflicts[0].contains("Temp"), "note: {}", c.conflicts[0]);
        // pH still overlaps, so it is reported as a shared window.
        assert_eq!(c.ph, Some((6.0, 7.0)));
    }
}

/// What a Library entry points to: an embedded document (markdown body) or an
/// external website / tool (url + short description).
pub enum LibraryEntryKind {
    Doc(String),
    Link { url: String, desc: String },
}

/// One entry in the Library: a document to read or a website to open.
pub struct LibraryEntry {
    pub title: String,
    pub kind: LibraryEntryKind,
}

/// A named category of entries within a section.
pub struct LibraryCategory {
    pub name: String,
    pub entries: Vec<LibraryEntry>,
}

/// A top-level Library section (e.g. "HumanityOS", "Tools and Websites") that
/// groups nested categories.
pub struct LibrarySection {
    pub name: String,
    pub categories: Vec<LibraryCategory>,
}

/// Load the in-app Library into sections. "HumanityOS" is the Accord + companion
/// docs (`data/library/index.json` plus the markdown files it lists); "Tools and
/// Websites" is the curated external links shared with the Resources page
/// (`data/resources/catalog.json`), so the link data has a single source. Empty
/// vec on error, so the page falls back to a "nothing loaded" note.
#[cfg(feature = "native")]
pub fn load_library(data_dir: &std::path::Path) -> Vec<LibrarySection> {
    let mut sections = Vec::new();

    // HumanityOS: the Accord + its companion docs.
    {
        #[derive(serde::Deserialize)]
        struct DocEntry {
            title: String,
            file: String,
        }
        #[derive(serde::Deserialize)]
        struct DocCat {
            name: String,
            #[serde(default)]
            docs: Vec<DocEntry>,
        }
        #[derive(serde::Deserialize)]
        struct Manifest {
            #[serde(default)]
            categories: Vec<DocCat>,
        }
        if let Some(m) = read_data_json::<Manifest>(data_dir, "library/index.json") {
            let dir = data_dir.join("library");
            let cats: Vec<LibraryCategory> = m
                .categories
                .into_iter()
                .map(|c| LibraryCategory {
                    name: c.name,
                    entries: c
                        .docs
                        .into_iter()
                        .filter_map(|d| {
                            std::fs::read_to_string(dir.join(&d.file)).ok().map(|body| LibraryEntry {
                                title: d.title,
                                kind: LibraryEntryKind::Doc(body),
                            })
                        })
                        .collect(),
                })
                .filter(|c| !c.entries.is_empty())
                .collect();
            if !cats.is_empty() {
                sections.push(LibrarySection { name: "HumanityOS".to_string(), categories: cats });
            }
        }
    }

    // Tools and Websites: external links, shared with the Resources page catalog.
    {
        #[derive(serde::Deserialize)]
        struct Res {
            title: String,
            #[serde(default)]
            description: String,
            url: String,
        }
        #[derive(serde::Deserialize)]
        struct ResCat {
            name: String,
            #[serde(default)]
            real_resources: Vec<Res>,
        }
        #[derive(serde::Deserialize)]
        struct ResFile {
            #[serde(default)]
            categories: Vec<ResCat>,
        }
        if let Some(rf) = read_data_json::<ResFile>(data_dir, "resources/catalog.json") {
            let cats: Vec<LibraryCategory> = rf
                .categories
                .into_iter()
                .map(|c| LibraryCategory {
                    name: c.name,
                    entries: c
                        .real_resources
                        .into_iter()
                        .map(|r| LibraryEntry {
                            title: r.title,
                            kind: LibraryEntryKind::Link { url: r.url, desc: r.description },
                        })
                        .collect(),
                })
                .filter(|c| !c.entries.is_empty())
                .collect();
            if !cats.is_empty() {
                sections.push(LibrarySection { name: "Tools and Websites".to_string(), categories: cats });
            }
        }
    }

    sections
}

/// Load `(severities, categories)` for the bug reporter from `data/bugs/taxonomy.json`.
#[cfg(feature = "native")]
pub fn load_bug_taxonomy(data_dir: &std::path::Path) -> (Vec<String>, Vec<String>) {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)] severities: Vec<String>,
        #[serde(default)] categories: Vec<String>,
    }
    read_data_json::<File>(data_dir, "bugs/taxonomy.json")
        .map(|f| (f.severities, f.categories))
        .unwrap_or_default()
}

/// Load crafting recipes from `data/recipes.csv` into the Crafting page's recipe
/// browser (`GuiState.craft_recipes`).
#[cfg(feature = "native")]
pub fn load_crafting_recipes(data_dir: &std::path::Path) -> Vec<GuiRecipe> {
    // Mirrors the runtime RecipeRegistry load (Wiring-1) but builds the GUI-facing
    // GuiRecipe rows the Crafting page browses. Reuses the shared CSV loader (skips
    // # comments, row-resilient) + Recipe::parse_ingredients for the pipe-separated
    // item:qty inputs/outputs. Before this the page's craft_recipes Vec was never
    // populated, so the Crafting page always showed "No recipes match your filter"
    // even after the recipe registry loaded into the runtime.
    #[derive(serde::Deserialize)]
    struct Row {
        id: String,
        name: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        inputs: String,
        #[serde(default)]
        outputs: String,
        #[serde(default)]
        craft_time_sec: f32,
        #[serde(default)]
        station_required: String,
        #[serde(default)]
        skill_required: String,
        #[serde(default)]
        skill_level: u32,
        #[serde(default)]
        description: String,
    }
    let bytes = match std::fs::read(data_dir.join("recipes.csv")) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let rows: Vec<Row> = crate::assets::loader::parse_csv(&bytes).unwrap_or_default();
    rows.into_iter()
        .map(|r| GuiRecipe {
            id: r.id,
            name: r.name,
            category: r.category,
            inputs: crate::systems::crafting::Recipe::parse_ingredients(&r.inputs),
            outputs: crate::systems::crafting::Recipe::parse_ingredients(&r.outputs),
            craft_time_sec: r.craft_time_sec,
            station_required: r.station_required,
            skill_required: {
                let s = r.skill_required.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            },
            skill_level: r.skill_level,
            description: r.description,
        })
        .collect()
}

/// Load the hierarchical crafting category tree from `data/crafting/categories.json`
/// (top-level groups -> leaf categories). The Crafting page renders these as
/// collapsible groups; leaf categories are matched case-insensitively against
/// `recipe.category`. Fully data-driven (infinite-of-X) — add groups/categories
/// freely; for very large categories, split a recipe's category into finer values
/// and group them here.
#[cfg(feature = "native")]
pub fn load_crafting_category_groups(data_dir: &std::path::Path) -> Vec<CraftCategoryGroup> {
    #[derive(serde::Deserialize)]
    struct File { groups: Vec<CraftCategoryGroup> }
    read_data_json::<File>(data_dir, "crafting/categories.json")
        .map(|f| f.groups)
        .unwrap_or_default()
}

/// One group in the hierarchical crafting-category tree: a collapsible group name
/// plus its leaf categories.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CraftCategoryGroup {
    pub name: String,
    pub categories: Vec<String>,
}

#[cfg(all(test, feature = "native"))]
mod crafting_recipes_load_tests {
    use super::*;

    #[test]
    fn load_crafting_recipes_populates_from_real_data() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let recipes = load_crafting_recipes(&data_dir);
        assert!(
            recipes.len() > 50,
            "Crafting page should load the real recipes.csv (got {})",
            recipes.len()
        );
        let smelt = recipes
            .iter()
            .find(|r| r.id == "smelt_iron")
            .expect("smelt_iron present in the browser");
        assert!(!smelt.inputs.is_empty(), "smelt_iron has inputs");
        assert!(!smelt.outputs.is_empty(), "smelt_iron has outputs");
    }
}

/// Load marketplace category filters from `data/market/categories.json`.
#[cfg(feature = "native")]
pub fn load_market_categories(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<String> }
    read_data_json::<File>(data_dir, "market/categories.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Load streaming-studio scene presets from `data/studio/scenes.json`.
#[cfg(feature = "native")]
pub fn load_studio_scenes(data_dir: &std::path::Path) -> Vec<StudioScenePreset> {
    #[derive(serde::Deserialize)]
    struct File { scenes: Vec<StudioScenePreset> }
    read_data_json::<File>(data_dir, "studio/scenes.json")
        .map(|f| f.scenes)
        .unwrap_or_default()
}

/// Load streaming-studio source presets from `data/studio/sources.json`.
#[cfg(feature = "native")]
pub fn load_studio_sources(data_dir: &std::path::Path) -> Vec<StudioSourcePreset> {
    #[derive(serde::Deserialize)]
    struct File { sources: Vec<StudioSourcePreset> }
    read_data_json::<File>(data_dir, "studio/sources.json")
        .map(|f| f.sources)
        .unwrap_or_default()
}

/// Picker options for the Broadcasting Studio page (platforms, resolutions, etc.).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct StudioStreamingConfig {
    #[serde(default)] pub platforms: Vec<String>,
    #[serde(default)] pub resolutions: Vec<String>,
    #[serde(default)] pub fps: Vec<u32>,
    #[serde(default)] pub chat_positions: Vec<String>,
}

/// Load streaming pickers from `data/studio/streaming_config.json`.
#[cfg(feature = "native")]
pub fn load_studio_streaming_config(data_dir: &std::path::Path) -> StudioStreamingConfig {
    read_data_json::<StudioStreamingConfig>(data_dir, "studio/streaming_config.json")
        .unwrap_or_default()
}

/// One Q&A entry on the Donate page FAQ.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DonateFaqEntry {
    pub question: String,
    pub answer: String,
}

/// Load donate-page FAQ entries from `data/donate/faq.json`.
#[cfg(feature = "native")]
pub fn load_donate_faq(data_dir: &std::path::Path) -> Vec<DonateFaqEntry> {
    #[derive(serde::Deserialize)]
    struct File { entries: Vec<DonateFaqEntry> }
    read_data_json::<File>(data_dir, "donate/faq.json")
        .map(|f| f.entries)
        .unwrap_or_default()
}

/// One QA test task surfaced on the Testing page.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QaTestTask {
    pub id: String,
    #[serde(default)] pub version: String,
    pub feature: String,
    pub what_to_test: String,
    pub expected: String,
    #[serde(default)] pub category: String,
    #[serde(default)] pub note: Option<String>,
}

/// Load QA test tasks from `data/testing/qa_tasks.json`.
#[cfg(feature = "native")]
pub fn load_qa_test_tasks(data_dir: &std::path::Path) -> Vec<QaTestTask> {
    #[derive(serde::Deserialize)]
    struct File { tasks: Vec<QaTestTask> }
    read_data_json::<File>(data_dir, "testing/qa_tasks.json")
        .map(|f| f.tasks)
        .unwrap_or_default()
}

/// One bookmark on the Browser page (curated link to an external site).
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrowserBookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)] pub description: String,
    #[serde(default)] pub icon: String,
}

/// A category of bookmarks. Color is one of: accent, info, success, warning, danger.
#[cfg(feature = "native")]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BrowserCategory {
    pub id: String,
    pub name: String,
    #[serde(default = "default_browser_color")] pub color: String,
    pub bookmarks: Vec<BrowserBookmark>,
}

#[cfg(feature = "native")]
fn default_browser_color() -> String { "accent".to_string() }

/// Load browser bookmarks from `data/browser/bookmarks.json`.
#[cfg(feature = "native")]
pub fn load_browser_bookmarks(data_dir: &std::path::Path) -> Vec<BrowserCategory> {
    #[derive(serde::Deserialize)]
    struct File { categories: Vec<BrowserCategory> }
    read_data_json::<File>(data_dir, "browser/bookmarks.json")
        .map(|f| f.categories)
        .unwrap_or_default()
}

// v0.415.0: OnboardingConcept / OnboardingCorePage + their loaders removed with
// the standalone onboarding page. The JSON files stay (the web /onboarding page
// reads them); the native consumers are gone.

// v0.197.0: AiUsageFilters and load_ai_usage_filters removed (AI Usage
// page deleted along with its data/ai_usage/filters.json loader).

/// Load default task project names from `data/tasks/default_projects.json`.
/// These seed the task board for brand-new identities; existing users keep
/// their own list.
#[cfg(feature = "native")]
pub fn load_default_task_projects(data_dir: &std::path::Path) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { projects: Vec<String> }
    read_data_json::<File>(data_dir, "tasks/default_projects.json")
        .map(|f| f.projects)
        .unwrap_or_default()
}

/// Load the starting per-skill XP profile applied to brand-new identities,
/// from `data/skills/default_profile.json`. The skill catalog itself lives in
/// `data/skills/skills.csv`; this file is just the initial XP weights.
#[cfg(feature = "native")]
pub fn load_default_player_skills(data_dir: &std::path::Path) -> Vec<(String, f32)> {
    #[derive(serde::Deserialize)]
    struct Skill { name: String, xp: f32 }
    #[derive(serde::Deserialize)]
    struct File { skills: Vec<Skill> }
    read_data_json::<File>(data_dir, "skills/default_profile.json")
        .map(|f| f.skills.into_iter().map(|s| (s.name, s.xp)).collect())
        .unwrap_or_default()
}

#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Account,
    Appearance,
    Animations,
    Widgets,
    Notifications,
    Wallet,
    Audio,
    Graphics,
    Controls,
    Privacy,
    Data,
    Updates,
}

/// Profile page sidebar sections.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSection {
    // Private (red)
    BodyMeasurements,
    Identity,
    PrivateNotes,
    // Personal (orange)
    NetworkProfile,
    Interests,
    Skills,
    // v0.415.0: Quests section removed — live game quests render on the
    // top-level Quests page beside the learn-by-doing chains (the operator's
    // "one page, two kinds" model), not buried in Profile.
    // Public (green)
    SocialLinks,
    Streaming,
}

#[cfg(feature = "native")]
pub struct SettingsState {
    pub category: SettingsCategory,
    /// When set, the settings page scrolls to this section and clears the field.
    pub scroll_to_section: Option<SettingsCategory>,
    // Graphics
    pub fullscreen: bool,
    /// How the desktop window is presented (v0.454). Default = WindowedFullscreen.
    pub window_mode: crate::config::WindowMode,
    pub vsync: bool,
    pub fov: f32,
    pub render_distance: f32,
    /// Master toggle for procedural sky-planet surfaces (v0.763). Off falls
    /// back to smooth flat-colored spheres (the pre-v0.763 look).
    pub planet_detail: bool,
    /// Screen-size LOD base threshold in pixels: a sky body subdivides one
    /// icosphere level each time its projected diameter doubles past this.
    /// See terrain::planet::lod_level_for_pixels.
    pub planet_lod_px: f32,
    /// Max icosphere subdivision level for sky planets (0-7; level 6 is
    /// ~82k faces). Stored as f32 for the slider; rounded at use.
    pub planet_max_subdiv: f32,
    // Audio
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    // Controls
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    // Appearance
    pub dark_mode: bool,
    pub font_size: f32,
    // Notifications
    pub notify_dm: bool,
    pub notify_mentions: bool,
    pub notify_tasks: bool,
    pub dnd_start: String,
    pub dnd_end: String,
    // Gameplay
    /// Which home design loads (2026-07-01): `"home"` (default, family-scale) or
    /// `"home_solo"` (one-person self-sufficient design, see
    /// `docs/design/homestead-solo-design.md`). Applied via `crate::machines::
    /// home_ron_path` at world-load time -- changing this takes effect on next
    /// world load, not live mid-session (see the Settings UI's own note).
    pub home_variant: String,
    // Wallet
    pub wallet_network: WalletNetwork,
    pub custom_rpc_url: String,
    // Privacy
    pub profile_visible: bool,
    pub online_status_visible: bool,
    // Data
    pub seed_phrase_visible: bool,
    // Seed phrase recovery
    pub seed_phrase_input: String,
    pub seed_phrase_recovery_status: String,
    pub seed_phrase_show_recover: bool,
}

#[cfg(feature = "native")]
impl Default for SettingsState {
    fn default() -> Self {
        Self {
            category: SettingsCategory::Graphics,
            scroll_to_section: None,
            fullscreen: false,
            window_mode: crate::config::WindowMode::default(),
            vsync: true,
            fov: 90.0,
            render_distance: 500.0,
            planet_detail: true,
            planet_lod_px: 10.0,
            planet_max_subdiv: 6.0,
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            mouse_sensitivity: 0.25,
            invert_y: false,
            dark_mode: true,
            font_size: 14.0,
            notify_dm: true,
            notify_mentions: true,
            notify_tasks: true,
            dnd_start: String::new(),
            dnd_end: String::new(),
            home_variant: "home".to_string(),
            wallet_network: WalletNetwork::Devnet,
            custom_rpc_url: String::new(),
            profile_visible: true,
            online_status_visible: true,
            seed_phrase_visible: false,
            seed_phrase_input: String::new(),
            seed_phrase_recovery_status: String::new(),
            seed_phrase_show_recover: false,
        }
    }
}
