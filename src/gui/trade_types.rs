//! The P2P trade value types (moved out of `state_types.rs` 2026-09-29, when
//! an offer line gained the item id a finished trade moves, and that file was
//! at its size budget). `pub use trade_types::*` in the parent keeps the
//! `crate::gui::GuiTrade` spelling.

/// One item on a side of a P2P trade, mirroring the relay's TradeItem (v0.756).
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default)]
pub struct GuiTradeItem {
    pub item_type: String,
    pub name: String,
    pub quantity: u32,
    pub description: String,
    /// The item's id in items.csv (2026-09-29): what a finished trade takes
    /// out of one backpack and puts in the other. None for a typed-in line
    /// (the web page's offers are text), which a trade cannot move.
    pub reference_id: Option<String>,
    /// Wear and grade of the offered stack, so a worn tool arrives worn and
    /// trading is not a free repair.
    pub wear: u32,
    pub quality: u8,
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
        let s = |i: &serde_json::Value, f: &str| i.get(f).and_then(|x| x.as_str()).map(str::to_string);
        let n = |i: &serde_json::Value, f: &str, d: u64| i.get(f).and_then(|x| x.as_u64()).unwrap_or(d);
        let items = |k: &str| -> Vec<GuiTradeItem> {
            v.get(k)
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .map(|i| GuiTradeItem {
                            item_type: s(i, "item_type").unwrap_or_else(|| "item".to_string()),
                            name: s(i, "name").unwrap_or_default(),
                            quantity: n(i, "quantity", 1) as u32,
                            description: s(i, "description").unwrap_or_default(),
                            reference_id: s(i, "reference_id").filter(|r| !r.is_empty()),
                            wear: n(i, "wear", 0) as u32,
                            quality: n(i, "quality", 0).min(255) as u8,
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
