use super::Storage;
use super::{MarketplaceListing, ListingImage};
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

    /// Create a marketplace listing and index it in FTS5.
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
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO marketplace_listings (id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active', datetime('now'))",
                params![id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location],
            )?;
            // Sync FTS5 index (best-effort — ignore if FTS5 unavailable).
            let _ = conn.execute(
                "INSERT INTO marketplace_fts (listing_id, title, description, category) VALUES (?1, ?2, ?3, ?4)",
                params![id, title, description, category],
            );
            Ok(())
        })
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
        self.with_conn(|conn| {
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
            if rows > 0 {
                // Sync FTS5 index: delete old entry, insert updated one.
                let _ = conn.execute(
                    "DELETE FROM marketplace_fts WHERE listing_id = ?1",
                    params![id],
                );
                let _ = conn.execute(
                    "INSERT INTO marketplace_fts (listing_id, title, description, category) VALUES (?1, ?2, ?3, ?4)",
                    params![id, title, description, category],
                );
            }
            Ok(rows > 0)
        })
    }

    /// Delete a marketplace listing. Returns true if deleted.
    pub fn delete_listing(&self, id: &str, seller_key: &str, is_admin: bool) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = if is_admin {
                conn.execute("DELETE FROM marketplace_listings WHERE id=?1", params![id])?
            } else {
                conn.execute("DELETE FROM marketplace_listings WHERE id=?1 AND seller_key=?2", params![id, seller_key])?
            };
            if rows > 0 {
                // Sync FTS5 index.
                let _ = conn.execute(
                    "DELETE FROM marketplace_fts WHERE listing_id = ?1",
                    params![id],
                );
                // Cascade deletes listing_images via FK, but SQLite requires PRAGMA foreign_keys=ON.
                // Explicitly delete to be safe.
                let _ = conn.execute(
                    "DELETE FROM listing_images WHERE listing_id = ?1",
                    params![id],
                );
            }
            Ok(rows > 0)
        })
    }

    /// Get all marketplace listings, optionally filtered.
    pub fn get_listings(&self, category: Option<&str>, status: Option<&str>, limit: usize) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Get a single listing by ID.
    pub fn get_listing_by_id(&self, id: &str) -> Result<Option<MarketplaceListing>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Get listings for a specific seller.
    pub fn get_user_listings(&self, seller_key: &str) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
                 FROM marketplace_listings WHERE seller_key=?1 ORDER BY created_at DESC"
            )?;
            let listings = stmt.query_map(params![seller_key], map_listing_row)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(listings)
        })
    }

    /// Full-text search marketplace listings using FTS5, with LIKE fallback.
    pub fn search_listings(&self, query: &str, limit: usize) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        self.with_conn(|conn| {
            let limit = limit.min(200);

            // Try FTS5 first.
            let fts_sql =
                "SELECT l.id, l.seller_key, l.seller_name, l.title, l.description, l.category, \
                        l.condition, l.price, l.payment_methods, l.location, l.images, l.status, \
                        l.created_at, l.updated_at \
                 FROM marketplace_fts f \
                 JOIN marketplace_listings l ON f.listing_id = l.id \
                 WHERE marketplace_fts MATCH ?1 \
                 ORDER BY rank \
                 LIMIT ?2";

            match conn.prepare(fts_sql).and_then(|mut s| {
                let results: Vec<MarketplaceListing> = s.query_map(params![query, limit], map_listing_row)?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(results)
            }) {
                Ok(v) if !v.is_empty() || query.contains('"') || query.contains('*') => Ok(v),
                _ => {
                    // Fallback: LIKE search across title, description, category.
                    let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                    let pattern = format!("%{}%", escaped);
                    let mut stmt = conn.prepare(
                        "SELECT id, seller_key, seller_name, title, description, category, \
                                condition, price, payment_methods, location, images, status, \
                                created_at, updated_at \
                         FROM marketplace_listings \
                         WHERE (title LIKE ?1 ESCAPE '\\' OR description LIKE ?1 ESCAPE '\\' OR category LIKE ?1 ESCAPE '\\') \
                         ORDER BY created_at DESC \
                         LIMIT ?2"
                    )?;
                    let listings = stmt.query_map(params![pattern, limit], map_listing_row)?
                        .filter_map(|r| r.ok())
                        .collect();
                    Ok(listings)
                }
            }
        })
    }

    // ── Listing Images ──

    /// Add an image to a listing. Max 5 images per listing enforced.
    pub fn add_listing_image(&self, listing_id: &str, url: &str, position: i32) -> Result<i64, String> {
        self.with_conn(|conn| {
            // Enforce max 5 images per listing.
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM listing_images WHERE listing_id = ?1",
                params![listing_id],
                |row| row.get(0),
            ).unwrap_or(0);
            if count >= 5 {
                return Err("Maximum 5 images per listing.".to_string());
            }

            conn.execute(
                "INSERT INTO listing_images (listing_id, url, position, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
                params![listing_id, url, position],
            ).map_err(|e| format!("DB error: {e}"))?;

            let image_id = conn.last_insert_rowid();

            // Update the images JSON field on the listing for backwards compatibility.
            Self::update_listing_images_json(conn, listing_id);

            Ok(image_id)
        })
    }

    /// Get all images for a listing, ordered by position.
    pub fn get_listing_images(&self, listing_id: &str) -> Vec<ListingImage> {
        self.with_conn(|conn| {
            let mut stmt = match conn.prepare(
                "SELECT id, listing_id, url, position, created_at FROM listing_images WHERE listing_id = ?1 ORDER BY position, id"
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            stmt.query_map(params![listing_id], |row| {
                Ok(ListingImage {
                    id: row.get(0)?,
                    listing_id: row.get(1)?,
                    url: row.get(2)?,
                    position: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
    }

    /// Delete an image from a listing. Returns true if deleted.
    pub fn delete_listing_image(&self, image_id: i64, listing_id: &str) -> Result<bool, String> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM listing_images WHERE id = ?1 AND listing_id = ?2",
                params![image_id, listing_id],
            ).map_err(|e| format!("DB error: {e}"))?;

            if rows > 0 {
                Self::update_listing_images_json(conn, listing_id);
            }

            Ok(rows > 0)
        })
    }

    /// Reorder images for a listing by updating their positions.
    pub fn reorder_listing_images(&self, listing_id: &str, image_ids: &[i64]) -> Result<(), String> {
        self.with_conn(|conn| {
            for (pos, &img_id) in image_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE listing_images SET position = ?1 WHERE id = ?2 AND listing_id = ?3",
                    params![pos as i32, img_id, listing_id],
                ).map_err(|e| format!("DB error: {e}"))?;
            }
            Self::update_listing_images_json(conn, listing_id);
            Ok(())
        })
    }

    /// Update the legacy `images` JSON field on the listing from the listing_images table.
    fn update_listing_images_json(conn: &rusqlite::Connection, listing_id: &str) {
        let urls: Vec<String> = conn.prepare(
            "SELECT url FROM listing_images WHERE listing_id = ?1 ORDER BY position, id"
        )
        .and_then(|mut s| {
            let v: Vec<String> = s.query_map(params![listing_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(v)
        })
        .unwrap_or_default();

        let json = if urls.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&urls).unwrap_or_default())
        };
        let _ = conn.execute(
            "UPDATE marketplace_listings SET images = ?1 WHERE id = ?2",
            params![json, listing_id],
        );
    }

    /// Get the seller_key for a listing (used for auth checks on image endpoints).
    pub fn get_listing_seller_key(&self, listing_id: &str) -> Option<String> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT seller_key FROM marketplace_listings WHERE id = ?1",
                params![listing_id],
                |row| row.get(0),
            ).ok()
        })
    }

    // ── Friend Code System ──

    /// Characters for friend codes (no 0/O/1/I/l confusion).
    const FRIEND_CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    /// Create a friend code for a user. Returns the code string.
    /// Rate limited to max 5 active codes per user.
    pub fn create_friend_code(&self, public_key: &str, expires_at: u64, max_uses: i32) -> Result<String, String> {
        self.with_conn(|conn| {
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
        })
    }

    /// Redeem a friend code. Returns Ok(Some((owner_public_key, owner_name))) on success.
    pub fn redeem_friend_code(&self, code: &str) -> Result<Option<(String, Option<String>)>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Clean up expired friend codes.
    pub fn cleanup_expired_friend_codes(&self) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = super::now_millis() as i64;
            let rows = conn.execute("DELETE FROM friend_codes WHERE expires_at < ?1", params![now])?;
            Ok(rows)
        })
    }
}
