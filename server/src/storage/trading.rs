//! Peer-to-peer trading with escrow + order-book market trading.
//! Items are "locked" in DB while a trade is active — they aren't removed
//! from inventory until both parties confirm and the trade completes.
//!
//! The order book allows players to post sell orders at a fixed price
//! and other players to buy (with partial fills supported).

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

// ── Order Book types ──

/// A sell order on the order book.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TradeOrder {
    pub id: i64,
    pub seller_key: String,
    pub item_type: String,
    pub item_id: String,
    pub quantity: i64,
    pub remaining_qty: i64,
    pub price_per_unit: f64,
    pub currency: String,
    pub status: String,
    pub created_at: i64,
    pub filled_at: Option<i64>,
}

/// A completed trade from the history ledger.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TradeHistoryRecord {
    pub id: i64,
    pub order_id: i64,
    pub buyer_key: String,
    pub seller_key: String,
    pub item_type: String,
    pub item_id: String,
    pub quantity: i64,
    pub price_per_unit: f64,
    pub total_price: f64,
    pub timestamp: i64,
}

fn map_order_row(row: &rusqlite::Row) -> rusqlite::Result<TradeOrder> {
    Ok(TradeOrder {
        id: row.get(0)?,
        seller_key: row.get(1)?,
        item_type: row.get(2)?,
        item_id: row.get(3)?,
        quantity: row.get(4)?,
        remaining_qty: row.get(5)?,
        price_per_unit: row.get(6)?,
        currency: row.get(7)?,
        status: row.get(8)?,
        created_at: row.get(9)?,
        filled_at: row.get(10)?,
    })
}

fn map_history_row(row: &rusqlite::Row) -> rusqlite::Result<TradeHistoryRecord> {
    Ok(TradeHistoryRecord {
        id: row.get(0)?,
        order_id: row.get(1)?,
        buyer_key: row.get(2)?,
        seller_key: row.get(3)?,
        item_type: row.get(4)?,
        item_id: row.get(5)?,
        quantity: row.get(6)?,
        price_per_unit: row.get(7)?,
        total_price: row.get(8)?,
        timestamp: row.get(9)?,
    })
}

impl Storage {
    // ── Order Book methods ──

    /// Post a new sell order to the order book.
    pub fn create_trade_order(
        &self,
        seller_key: &str,
        item_type: &str,
        item_id: &str,
        quantity: i64,
        price_per_unit: f64,
        currency: &str,
    ) -> Result<i64, rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO trade_orders (seller_key, item_type, item_id, quantity, remaining_qty, price_per_unit, currency, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, 'open', ?7)",
                params![seller_key, item_type, item_id, quantity, price_per_unit, currency, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Cancel an open order. Only the seller can cancel their own order.
    pub fn cancel_trade_order(
        &self,
        order_id: i64,
        seller_key: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let updated = conn.execute(
                "UPDATE trade_orders SET status = 'cancelled'
                 WHERE id = ?1 AND seller_key = ?2 AND status = 'open'",
                params![order_id, seller_key],
            )?;
            Ok(updated > 0)
        })
    }

    /// Fill (buy from) an order. Supports partial fills.
    /// Returns the trade history record ID on success, or an error message.
    pub fn fill_trade_order(
        &self,
        order_id: i64,
        buyer_key: &str,
        quantity: i64,
    ) -> Result<i64, String> {
        let now = super::now_millis() as i64;
        self.with_conn_mut(|conn| {
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            // Fetch the order.
            let order: TradeOrder = tx.query_row(
                "SELECT id, seller_key, item_type, item_id, quantity, remaining_qty,
                        price_per_unit, currency, status, created_at, filled_at
                 FROM trade_orders WHERE id = ?1 AND status = 'open'",
                params![order_id],
                map_order_row,
            ).map_err(|_| "Order not found or not open.".to_string())?;

            // Can't buy from yourself.
            if order.seller_key == buyer_key {
                return Err("Cannot fill your own order.".to_string());
            }

            if quantity <= 0 {
                return Err("Quantity must be positive.".to_string());
            }

            if quantity > order.remaining_qty {
                return Err(format!(
                    "Requested {} but only {} remaining.",
                    quantity, order.remaining_qty
                ));
            }

            let total_price = quantity as f64 * order.price_per_unit;
            let new_remaining = order.remaining_qty - quantity;

            // Update order remaining quantity (and status if fully filled).
            if new_remaining == 0 {
                tx.execute(
                    "UPDATE trade_orders SET remaining_qty = 0, status = 'filled', filled_at = ?1
                     WHERE id = ?2",
                    params![now, order_id],
                ).map_err(|e| e.to_string())?;
            } else {
                tx.execute(
                    "UPDATE trade_orders SET remaining_qty = ?1 WHERE id = ?2",
                    params![new_remaining, order_id],
                ).map_err(|e| e.to_string())?;
            }

            // Record the trade in history.
            tx.execute(
                "INSERT INTO trade_history (order_id, buyer_key, seller_key, item_type, item_id, quantity, price_per_unit, total_price, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    order_id, buyer_key, order.seller_key,
                    order.item_type, order.item_id,
                    quantity, order.price_per_unit, total_price, now
                ],
            ).map_err(|e| e.to_string())?;

            let history_id = tx.last_insert_rowid();
            tx.commit().map_err(|e| e.to_string())?;
            Ok(history_id)
        })
    }

    /// Get open sell orders for an item type, sorted by price (lowest first).
    pub fn get_open_orders(
        &self,
        item_type: &str,
    ) -> Result<Vec<TradeOrder>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, seller_key, item_type, item_id, quantity, remaining_qty,
                        price_per_unit, currency, status, created_at, filled_at
                 FROM trade_orders
                 WHERE item_type = ?1 AND status = 'open'
                 ORDER BY price_per_unit ASC, created_at ASC
                 LIMIT 200"
            )?;
            let rows = stmt.query_map(params![item_type], map_order_row)?;
            rows.collect()
        })
    }

    /// Get trade history for a user (as buyer or seller), most recent first.
    pub fn get_trade_history(
        &self,
        user_key: &str,
        limit: usize,
    ) -> Result<Vec<TradeHistoryRecord>, rusqlite::Error> {
        let limit = limit.min(200) as i64;
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, order_id, buyer_key, seller_key, item_type, item_id,
                        quantity, price_per_unit, total_price, timestamp
                 FROM trade_history
                 WHERE buyer_key = ?1 OR seller_key = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![user_key, limit], map_history_row)?;
            rows.collect()
        })
    }

    /// Get the latest trade price for an item type.
    pub fn get_market_price(
        &self,
        item_type: &str,
    ) -> Result<Option<f64>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT price_per_unit FROM trade_history
                 WHERE item_type = ?1
                 ORDER BY timestamp DESC
                 LIMIT 1",
                params![item_type],
                |row| row.get(0),
            ).optional()
        })
    }
}
