//! Push notification subscription storage.
//!
//! Stores WebPush subscriptions keyed by user public key.
//! Each device gets its own subscription (unique endpoint).

use rusqlite::params;
use super::{Storage, PushSubscriptionRecord};

impl Storage {
    /// Save or update a push subscription (UPSERT by endpoint).
    pub fn save_push_subscription(
        &self,
        public_key: &str,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            let now = super::now_millis() as i64;
            conn.execute(
                "INSERT INTO push_subscriptions (public_key, endpoint, p256dh, auth, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(endpoint) DO UPDATE SET
                    public_key = excluded.public_key,
                    p256dh = excluded.p256dh,
                    auth = excluded.auth,
                    created_at = excluded.created_at",
                params![public_key, endpoint, p256dh, auth, now],
            )?;
            Ok(())
        })
    }

    /// Remove a push subscription by endpoint.
    pub fn remove_push_subscription(&self, endpoint: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM push_subscriptions WHERE endpoint = ?1",
                params![endpoint],
            )?;
            Ok(())
        })
    }

    /// Get all push subscriptions for a user's public key.
    pub fn get_push_subscriptions(&self, public_key: &str) -> Vec<PushSubscriptionRecord> {
        self.with_conn(|conn| {
            let mut stmt = match conn.prepare(
                "SELECT public_key, endpoint, p256dh, auth FROM push_subscriptions WHERE public_key = ?1"
            ) {
                Ok(s) => s,
                Err(_) => return vec![],
            };
            stmt.query_map(params![public_key], |row| {
                Ok(PushSubscriptionRecord {
                    public_key: row.get(0)?,
                    endpoint: row.get(1)?,
                    p256dh: row.get(2)?,
                    auth: row.get(3)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
    }
}
