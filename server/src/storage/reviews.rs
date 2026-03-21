use super::Storage;
use rusqlite::params;

/// A marketplace listing review record.
#[derive(Debug, Clone)]
pub struct ReviewRecord {
    pub id: i64,
    pub listing_id: String,
    pub reviewer_key: String,
    pub reviewer_name: Option<String>,
    pub rating: i32,
    pub comment: String,
    pub created_at: String,
}

fn map_review_row(row: &rusqlite::Row) -> rusqlite::Result<ReviewRecord> {
    Ok(ReviewRecord {
        id: row.get(0)?,
        listing_id: row.get(1)?,
        reviewer_key: row.get(2)?,
        reviewer_name: row.get(3)?,
        rating: row.get(4)?,
        comment: row.get(5)?,
        created_at: row.get(6)?,
    })
}

impl Storage {
    // ── Review methods ──

    /// Create a review for a listing. Enforces one review per listing per user
    /// and prevents reviewing own listings.
    pub fn create_review(
        &self,
        listing_id: &str,
        reviewer_key: &str,
        reviewer_name: &str,
        rating: i32,
        comment: &str,
    ) -> Result<i64, String> {
        if !(1..=5).contains(&rating) {
            return Err("Rating must be between 1 and 5.".to_string());
        }

        self.with_conn(|conn| {
            // Check listing exists and get seller key.
            let seller_key: String = conn.query_row(
                "SELECT seller_key FROM marketplace_listings WHERE id=?1",
                params![listing_id],
                |row| row.get(0),
            ).map_err(|_| "Listing not found.".to_string())?;

            // Prevent reviewing own listing.
            if seller_key == reviewer_key {
                return Err("Cannot review your own listing.".to_string());
            }

            // Insert review (UNIQUE constraint enforces one per listing per user).
            conn.execute(
                "INSERT INTO listing_reviews (listing_id, reviewer_key, reviewer_name, rating, comment, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![listing_id, reviewer_key, reviewer_name, rating, comment],
            ).map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    "You have already reviewed this listing.".to_string()
                } else {
                    format!("DB error: {e}")
                }
            })?;

            let review_id = conn.last_insert_rowid();

            // Update seller aggregate rating.
            Self::recalculate_seller_rating_conn(conn, &seller_key);

            Ok(review_id)
        })
    }

    /// Get reviews for a listing, ordered by newest first.
    pub fn get_reviews(&self, listing_id: &str, limit: usize) -> Result<Vec<ReviewRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, listing_id, reviewer_key, reviewer_name, rating, comment, created_at
                 FROM listing_reviews WHERE listing_id=?1
                 ORDER BY created_at DESC LIMIT ?2"
            )?;
            let reviews = stmt.query_map(params![listing_id, limit], map_review_row)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(reviews)
        })
    }

    /// Get a seller's aggregate rating: (avg_rating, review_count).
    pub fn get_seller_rating(&self, seller_key: &str) -> (f64, i64) {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT avg_rating, review_count FROM seller_ratings WHERE seller_key=?1",
                params![seller_key],
                |row| Ok((row.get::<_, f64>(0)?, row.get::<_, i64>(1)?)),
            ).unwrap_or((0.0, 0))
        })
    }

    /// Delete a review. Only the reviewer or an admin can delete.
    pub fn delete_review(&self, review_id: i64, reviewer_key: &str, is_admin: bool) -> Result<bool, String> {
        self.with_conn(|conn| {
            // Get review info before deleting (need listing_id to recalculate seller rating).
            let info = conn.query_row(
                "SELECT listing_id, reviewer_key FROM listing_reviews WHERE id=?1",
                params![review_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            );

            let (listing_id, actual_reviewer) = match info {
                Ok(i) => i,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
                Err(e) => return Err(format!("DB error: {e}")),
            };

            if !is_admin && actual_reviewer != reviewer_key {
                return Err("You can only delete your own reviews.".to_string());
            }

            let rows = conn.execute(
                "DELETE FROM listing_reviews WHERE id=?1",
                params![review_id],
            ).map_err(|e| format!("DB error: {e}"))?;

            if rows > 0 {
                // Get seller key and recalculate.
                if let Ok(seller_key) = conn.query_row(
                    "SELECT seller_key FROM marketplace_listings WHERE id=?1",
                    params![listing_id],
                    |row| row.get::<_, String>(0),
                ) {
                    Self::recalculate_seller_rating_conn(conn, &seller_key);
                }
            }

            Ok(rows > 0)
        })
    }

    /// Recalculate a seller's aggregate rating from all their listing reviews.
    fn recalculate_seller_rating_conn(conn: &rusqlite::Connection, seller_key: &str) {
        let result = conn.query_row(
            "SELECT COALESCE(AVG(CAST(r.rating AS REAL)), 0), COUNT(r.id)
             FROM listing_reviews r
             JOIN marketplace_listings l ON r.listing_id = l.id
             WHERE l.seller_key = ?1",
            params![seller_key],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, i64>(1)?)),
        );

        if let Ok((avg, count)) = result {
            let _ = conn.execute(
                "INSERT INTO seller_ratings (seller_key, avg_rating, review_count)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(seller_key) DO UPDATE SET avg_rating=?2, review_count=?3",
                params![seller_key, avg, count],
            );
        }
    }

    /// Get a single review by ID.
    pub fn get_review_by_id(&self, review_id: i64) -> Result<Option<ReviewRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT id, listing_id, reviewer_key, reviewer_name, rating, comment, created_at
                 FROM listing_reviews WHERE id=?1",
                params![review_id],
                map_review_row,
            ) {
                Ok(r) => Ok(Some(r)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}
