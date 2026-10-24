//! Key cards kept on this server (2026-10-22, docs/design/key-cards.md, `core::key_card`): one
//! row per person, the card their own app signed. Served to anyone who asks (`/api/key-card/{key}`,
//! the `key_card_request` frame, a group's roster) and passed on to linked servers like profile
//! copies. No home server: the signature is the authority, so a card that arrived from a linked
//! server is as good as one from a sign-in here.
//!
//! This layer does not check signatures (the relay does, off the async pool, before it stores):
//! it holds the keep rule (`core::key_card::keep_rule`), so no path can store a card no server
//! should keep. Listed by the account export and deleted by the account erase (storage/account.rs).

use rusqlite::{params, OptionalExtension};

use super::Storage;
use crate::relay::core::key_card::{keep_rule, KeyCard, Keep, CARD_VERSION};

/// The most cards a server sends a linked server when their link comes up (the catch-up): the
/// newest, as for profile copies (`signed_profiles::CATCH_UP_COPIES_MAX`).
pub const CATCH_UP_CARDS_MAX: usize = 1_000;

fn card_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<KeyCard> {
    Ok(KeyCard {
        v: CARD_VERSION,
        key: row.get(0)?,
        kyber: row.get(1)?,
        at: row.get::<_, i64>(2)? as u64,
        sig: row.get(3)?,
    })
}

impl Storage {
    /// Keep `card` (checked by the caller) by the keep rule: nothing held, or a different key that
    /// is newer than the one held, replaces the row; the same key changes nothing; an older card
    /// for a different key, or one dated more than a day ahead, is refused. Returns what happened.
    pub fn store_key_card(&self, card: &KeyCard) -> Result<Keep, rusqlite::Error> {
        let now_ms = super::now_millis();
        self.with_conn(|conn| {
            let held: Option<(String, i64)> = conn
                .query_row(
                    "SELECT kyber_public, signed_at FROM key_cards WHERE public_key = ?1",
                    params![card.key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let keep = keep_rule(held.as_ref().map(|(k, at)| (k.as_str(), *at as u64)), card, now_ms);
            if keep.stored() {
                conn.execute(
                    "INSERT INTO key_cards (public_key, kyber_public, signed_at, signature) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(public_key) DO UPDATE SET
                       kyber_public = excluded.kyber_public,
                       signed_at = excluded.signed_at,
                       signature = excluded.signature",
                    params![card.key, card.kyber, card.at as i64, card.sig],
                )?;
            }
            Ok(keep)
        })
    }

    /// The card held for `public_key`, if any.
    pub fn get_key_card(&self, public_key: &str) -> Result<Option<KeyCard>, rusqlite::Error> {
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT public_key, kyber_public, signed_at, signature FROM key_cards WHERE public_key = ?1",
                params![public_key],
                card_of,
            )
            .optional()
        })
    }

    /// The time on the card held for `public_key`, if any: what the member lists carry, so an app
    /// knows when a card is newer than the one it pinned without fetching every card.
    pub fn key_card_at(&self, public_key: &str) -> Option<u64> {
        self.with_read_conn(|conn| {
            conn.query_row("SELECT signed_at FROM key_cards WHERE public_key = ?1", params![public_key], |row| {
                row.get::<_, i64>(0)
            })
            .optional()
        })
        .ok()
        .flatten()
        .map(|at| at as u64)
    }

    /// Does this server hold a card for `public_key`: can an app seal anything to them from here.
    pub fn has_key_card(&self, public_key: &str) -> bool {
        self.key_card_at(public_key).is_some()
    }

    /// The cards this server sends a linked server when their link comes up: the cards of keys
    /// with an account HERE (`registered_names`), newest first, at most [`CATCH_UP_CARDS_MAX`]. A
    /// card this server only received from another server is that server's to send.
    pub fn key_cards_of_accounts_here(&self) -> Result<Vec<KeyCard>, rusqlite::Error> {
        // Read-only: SELECT + query_map. Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT kc.public_key, kc.kyber_public, kc.signed_at, kc.signature FROM key_cards kc
                 WHERE EXISTS (SELECT 1 FROM registered_names rn WHERE rn.public_key = kc.public_key)
                 ORDER BY kc.signed_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![CATCH_UP_CARDS_MAX as i64], card_of)?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::key_card::MAX_AHEAD_MS;

    fn card(key: &str, kyber: &str, at: u64) -> KeyCard {
        KeyCard { v: CARD_VERSION, key: key.into(), kyber: kyber.into(), at, sig: format!("sig-{kyber}-{at}") }
    }

    /// THE KEEP RULE, AS STORED: the first card is kept; the same key again changes nothing (the
    /// first time and signature stay); a different, newer key replaces it; an older different key
    /// and a card a day and a minute ahead are refused and change nothing.
    ///
    /// Seen red 2026-10-23 with `store_key_card` writing on `Keep::SameKey` too: "the same key
    /// again kept the first time" failed (left `now`, right `now - 5_000`).
    #[test]
    fn the_keep_rule_as_stored() {
        let db = Storage::open_temp("kc_keep");
        let now = super::super::now_millis();
        assert_eq!(db.store_key_card(&card("ann", "K1", now - 5_000)).unwrap(), Keep::New);
        assert_eq!(db.store_key_card(&card("ann", "K1", now)).unwrap(), Keep::SameKey);
        assert_eq!(db.key_card_at("ann"), Some(now - 5_000), "the same key again kept the first time");
        assert_eq!(db.store_key_card(&card("ann", "K2", now - 6_000)).unwrap(), Keep::Older);
        assert_eq!(db.store_key_card(&card("ann", "K2", now + MAX_AHEAD_MS + 60_000)).unwrap(), Keep::TooFarAhead);
        assert_eq!(db.get_key_card("ann").unwrap().unwrap().kyber, "K1", "nothing changed");
        assert_eq!(db.store_key_card(&card("ann", "K2", now - 1_000)).unwrap(), Keep::Replace);
        let held = db.get_key_card("ann").unwrap().unwrap();
        assert_eq!((held.kyber.as_str(), held.at, held.sig.as_str()), ("K2", now - 1_000, format!("sig-K2-{}", now - 1_000).as_str()));
        assert!(db.has_key_card("ann") && !db.has_key_card("bo"));
    }

    /// THE CATCH-UP sends the cards of this server's own accounts only, newest first.
    #[test]
    fn the_catch_up_holds_only_this_servers_accounts() {
        let db = Storage::open_temp("kc_catch_up");
        db.register_name("Ann", "ann").unwrap();
        db.register_name("Bo", "bo").unwrap();
        let now = super::super::now_millis();
        db.store_key_card(&card("ann", "K1", now - 3_000)).unwrap();
        db.store_key_card(&card("bo", "K2", now - 2_000)).unwrap();
        db.store_key_card(&card("far", "K3", now - 1_000)).unwrap();
        let keys: Vec<String> = db.key_cards_of_accounts_here().unwrap().into_iter().map(|c| c.key).collect();
        assert_eq!(keys, ["bo", "ann"]);
    }
}
