use super::Storage;
use super::MarketplaceListing;
use rusqlite::params;

fn map_listing_row(row: &rusqlite::Row) -> rusqlite::Result<MarketplaceListing> {
    Ok(MarketplaceListing {
        id: row.get(0)?,
        seller_key: row.get(1)?,
        seller_name: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        category: row.get(5)?,
        condition: row.get(6)?,
        price: row.get(7)?,
        payment_methods: row.get(8)?,
        location: row.get(9)?,
        images: row.get(10)?,
        status: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

impl Storage {
    // ── Marketplace methods ──

    /// Create a marketplace listing.
    pub fn create_listing(
        &self,
        id: &str,
        seller_key: &str,
        seller_name: &str,
        title: &str,
        description: &str,
        category: &str,
        condition: &str,
        price: &str,
        payment_methods: &str,
        location: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO marketplace_listings (id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active', datetime('now'))",
            params![id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location],
        )?;
        Ok(())
    }

    /// Update a marketplace listing. Returns true if updated.
    pub fn update_listing(
        &self,
        id: &str,
        seller_key: &str,
        title: &str,
        description: &str,
        category: &str,
        condition: &str,
        price: &str,
        payment_methods: &str,
        location: &str,
        status: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = if is_admin {
            conn.execute(
                "UPDATE marketplace_listings SET title=?1, description=?2, category=?3, condition=?4, price=?5, payment_methods=?6, location=?7, status=COALESCE(?8, status), updated_at=datetime('now') WHERE id=?9",
                params![title, description, category, condition, price, payment_methods, location, status, id],
            )?
        } else {
            conn.execute(
                "UPDATE marketplace_listings SET title=?1, description=?2, category=?3, condition=?4, price=?5, payment_methods=?6, location=?7, status=COALESCE(?8, status), updated_at=datetime('now') WHERE id=?9 AND seller_key=?10",
                params![title, description, category, condition, price, payment_methods, location, status, id, seller_key],
            )?
        };
        Ok(rows > 0)
    }

    /// Delete a marketplace listing. Returns true if deleted.
    pub fn delete_listing(&self, id: &str, seller_key: &str, is_admin: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = if is_admin {
            conn.execute("DELETE FROM marketplace_listings WHERE id=?1", params![id])?
        } else {
            conn.execute("DELETE FROM marketplace_listings WHERE id=?1 AND seller_key=?2", params![id, seller_key])?
        };
        Ok(rows > 0)
    }

    /// Get all marketplace listings, optionally filtered.
    pub fn get_listings(&self, category: Option<&str>, status: Option<&str>, limit: usize) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let query = format!(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings
             WHERE 1=1 {} {}
             ORDER BY created_at DESC
             LIMIT ?1",
            if category.is_some() { "AND category = ?2" } else { "" },
            if status.is_some() { if category.is_some() { "AND status = ?3" } else { "AND status = ?2" } } else { "" },
        );
        let mut stmt = conn.prepare(&query)?;
        let listings = if let Some(cat) = category {
            if let Some(st) = status {
                stmt.query_map(params![limit, cat, st], map_listing_row)?
            } else {
                stmt.query_map(params![limit, cat], map_listing_row)?
            }
        } else if let Some(st) = status {
            stmt.query_map(params![limit, st], map_listing_row)?
        } else {
            stmt.query_map(params![limit], map_listing_row)?
        };
        Ok(listings.filter_map(|r| r.ok()).collect())
    }

    /// Get a single listing by ID.
    pub fn get_listing_by_id(&self, id: &str) -> Result<Option<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings WHERE id=?1",
            params![id],
            map_listing_row,
        ) {
            Ok(l) => Ok(Some(l)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get listings for a specific seller.
    pub fn get_user_listings(&self, seller_key: &str) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings WHERE seller_key=?1 ORDER BY created_at DESC"
        )?;
        let listings = stmt.query_map(params![seller_key], map_listing_row)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(listings)
    }

    // ── Friend Code System ──

    /// Characters for friend codes (no 0/O/1/I/l confusion).
    const FRIEND_CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    /// Create a friend code for a user. Returns the code string.
    /// Rate limited to max 5 active codes per user.
    pub fn create_friend_code(&self, public_key: &str, expires_at: u64, max_uses: i32) -> Result<String, String> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis();

        // Clean up expired codes first.
        let _ = conn.execute("DELETE FROM friend_codes WHERE expires_at < ?1", params![now as i64]);

        // Check rate limit: max 5 active codes per user.
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_codes WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        ).unwrap_or(0);
        if count >= 5 {
            return Err("You already have 5 active friend codes. Wait for them to expire.".to_string());
        }

        // Generate 8-char code from safe alphabet.
        let mut code = String::with_capacity(8);
        let chars = Self::FRIEND_CODE_CHARS;
        use rand::Rng;
        let mut rng = rand::rng();
        for _ in 0..8 {
            let idx = rng.random_range(0..chars.len());
            code.push(chars[idx] as char);
        }

        conn.execute(
            "INSERT INTO friend_codes (code, public_key, created_at, expires_at, uses_remaining) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![code, public_key, now as i64, expires_at as i64, max_uses],
        ).map_err(|e| format!("DB error: {e}"))?;

        Ok(code)
    }

    /// Redeem a friend code. Returns Ok(Some((owner_public_key, owner_name))) on success.
    pub fn redeem_friend_code(&self, code: &str) -> Result<Option<(String, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis() as i64;

        // Look up the code (case-insensitive).
        let result = conn.query_row(
            "SELECT public_key, uses_remaining FROM friend_codes WHERE code = ?1 COLLATE NOCASE AND expires_at > ?2",
            params![code, now],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        );

        match result {
            Ok((owner_key, uses)) => {
                if uses <= 1 {
                    conn.execute("DELETE FROM friend_codes WHERE code = ?1 COLLATE NOCASE", params![code])?;
                } else {
                    conn.execute(
                        "UPDATE friend_codes SET uses_remaining = uses_remaining - 1 WHERE code = ?1 COLLATE NOCASE",
                        params![code],
                    )?;
                }

                // Look up owner's name.
                let owner_name: Option<String> = conn.query_row(
                    "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                    params![owner_key],
                    |row| row.get(0),
                ).ok();

                Ok(Some((owner_key, owner_name)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Clean up expired friend codes.
    pub fn cleanup_expired_friend_codes(&self) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis() as i64;
        let rows = conn.execute("DELETE FROM friend_codes WHERE expires_at < ?1", params![now])?;
        Ok(rows)
    }
}
