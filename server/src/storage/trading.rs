//! Peer-to-peer trading with escrow.
//! Items are "locked" in DB while a trade is active — they aren't removed
//! from inventory until both parties confirm and the trade completes.

use super::Storage;
use rusqlite::{params, OptionalExtension};

/// A trade record from the database.
#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub id: String,
    pub initiator_key: String,
    pub recipient_key: String,
    pub status: String,
    pub initiator_items: String,
    pub recipient_items: String,
    pub initiator_confirmed: bool,
    pub recipient_confirmed: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub message: Option<String>,
}

fn map_trade_row(row: &rusqlite::Row) -> rusqlite::Result<TradeRecord> {
    Ok(TradeRecord {
        id: row.get(0)?,
        initiator_key: row.get(1)?,
        recipient_key: row.get(2)?,
        status: row.get(3)?,
        initiator_items: row.get(4)?,
        recipient_items: row.get(5)?,
        initiator_confirmed: row.get::<_, i32>(6)? != 0,
        recipient_confirmed: row.get::<_, i32>(7)? != 0,
        created_at: row.get(8)?,
        completed_at: row.get(9)?,
        message: row.get(10)?,
    })
}

impl Storage {
    // ── Trading methods ──

    /// Create a new trade between two parties.
    pub fn create_trade(
        &self,
        id: &str,
        initiator_key: &str,
        recipient_key: &str,
        message: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO trades (id, initiator_key, recipient_key, status, initiator_items, recipient_items, initiator_confirmed, recipient_confirmed, created_at, message)
                 VALUES (?1, ?2, ?3, 'pending', '[]', '[]', 0, 0, ?4, ?5)",
                params![id, initiator_key, recipient_key, now, message],
            )?;
            Ok(())
        })
    }

    /// Get a trade by ID.
    pub fn get_trade(&self, id: &str) -> Result<Option<TradeRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, initiator_key, recipient_key, status, initiator_items, recipient_items,
                        initiator_confirmed, recipient_confirmed, created_at, completed_at, message
                 FROM trades WHERE id = ?1",
                params![id],
                map_trade_row,
            ).optional()
        })
    }

    /// Update items for one side of a trade.
    /// `side` must be "initiator" or "recipient".
    /// Resets both confirmation flags when items change.
    pub fn update_trade_items(
        &self,
        trade_id: &str,
        side: &str,
        items_json: &str,
    ) -> Result<bool, rusqlite::Error> {
        let col = match side {
            "initiator" => "initiator_items",
            "recipient" => "recipient_items",
            _ => return Ok(false),
        };
        self.with_conn(|conn| {
            let updated = conn.execute(
                &format!(
                    "UPDATE trades SET {} = ?1, initiator_confirmed = 0, recipient_confirmed = 0
                     WHERE id = ?2 AND status = 'active'",
                    col
                ),
                params![items_json, trade_id],
            )?;
            Ok(updated > 0)
        })
    }

    /// Set a trade's status to 'active' (accepted) or 'cancelled' (rejected).
    pub fn respond_to_trade(
        &self,
        trade_id: &str,
        accepted: bool,
    ) -> Result<bool, rusqlite::Error> {
        let new_status = if accepted { "active" } else { "cancelled" };
        self.with_conn(|conn| {
            let updated = conn.execute(
                "UPDATE trades SET status = ?1 WHERE id = ?2 AND status = 'pending'",
                params![new_status, trade_id],
            )?;
            Ok(updated > 0)
        })
    }

    /// Confirm a trade for one side. Returns (updated, both_confirmed).
    pub fn confirm_trade(
        &self,
        trade_id: &str,
        user_key: &str,
    ) -> Result<(bool, bool), rusqlite::Error> {
        self.with_conn(|conn| {
            // Determine which side the user is.
            let trade: Option<TradeRecord> = conn.query_row(
                "SELECT id, initiator_key, recipient_key, status, initiator_items, recipient_items,
                        initiator_confirmed, recipient_confirmed, created_at, completed_at, message
                 FROM trades WHERE id = ?1 AND status = 'active'",
                params![trade_id],
                map_trade_row,
            ).optional()?;

            let trade = match trade {
                Some(t) => t,
                None => return Ok((false, false)),
            };

            let col = if user_key == trade.initiator_key {
                "initiator_confirmed"
            } else if user_key == trade.recipient_key {
                "recipient_confirmed"
            } else {
                return Ok((false, false));
            };

            conn.execute(
                &format!("UPDATE trades SET {} = 1 WHERE id = ?1", col),
                params![trade_id],
            )?;

            // Check if both are now confirmed.
            let (ic, rc): (i32, i32) = conn.query_row(
                "SELECT initiator_confirmed, recipient_confirmed FROM trades WHERE id = ?1",
                params![trade_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let both = ic != 0 && rc != 0;
            Ok((true, both))
        })
    }

    /// Complete a trade (both confirmed). Sets status to 'completed'.
    pub fn complete_trade(&self, trade_id: &str) -> Result<bool, rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            let updated = conn.execute(
                "UPDATE trades SET status = 'completed', completed_at = ?1
                 WHERE id = ?2 AND status = 'active'
                 AND initiator_confirmed = 1 AND recipient_confirmed = 1",
                params![now, trade_id],
            )?;
            Ok(updated > 0)
        })
    }

    /// Cancel a trade. Only allowed if not already completed.
    pub fn cancel_trade(&self, trade_id: &str, user_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let updated = conn.execute(
                "UPDATE trades SET status = 'cancelled'
                 WHERE id = ?1 AND status IN ('pending', 'active')
                 AND (initiator_key = ?2 OR recipient_key = ?2)",
                params![trade_id, user_key],
            )?;
            Ok(updated > 0)
        })
    }

    /// Get all trades for a user (as initiator or recipient).
    pub fn get_trades_for_user(&self, user_key: &str) -> Result<Vec<TradeRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, initiator_key, recipient_key, status, initiator_items, recipient_items,
                        initiator_confirmed, recipient_confirmed, created_at, completed_at, message
                 FROM trades
                 WHERE initiator_key = ?1 OR recipient_key = ?1
                 ORDER BY created_at DESC
                 LIMIT 50"
            )?;
            let rows = stmt.query_map(params![user_key], map_trade_row)?;
            rows.collect()
        })
    }
}
