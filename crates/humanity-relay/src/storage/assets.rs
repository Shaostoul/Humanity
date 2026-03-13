use super::Storage;
use rusqlite::params;

impl Storage {
    // ── Asset Library methods ──

    /// Create an asset record.
    pub fn create_asset(
        &self,
        id: &str,
        owner_key: &str,
        filename: &str,
        file_type: &str,
        category: &str,
        tags: &str,
        size_bytes: i64,
        url: &str,
        description: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO assets (id, owner_key, filename, file_type, category, tags, size_bytes, url, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, owner_key, filename, file_type, category, tags, size_bytes, url, description],
        )?;
        Ok(())
    }

    /// Get assets with optional filters.
    pub fn get_assets(
        &self,
        category: Option<&str>,
        file_type: Option<&str>,
        search: Option<&str>,
        owner: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut conditions = vec!["1=1".to_string()];
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(cat) = category {
            conditions.push(format!("category = ?{}", param_values.len() + 1));
            param_values.push(Box::new(cat.to_string()));
        }
        if let Some(ft) = file_type {
            conditions.push(format!("file_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(ft.to_string()));
        }
        if let Some(o) = owner {
            conditions.push(format!("owner_key = ?{}", param_values.len() + 1));
            param_values.push(Box::new(o.to_string()));
        }
        if let Some(s) = search {
            conditions.push(format!("(filename LIKE ?{0} OR description LIKE ?{0} OR tags LIKE ?{0})", param_values.len() + 1));
            param_values.push(Box::new(format!("%{}%", s)));
        }

        let query = format!(
            "SELECT id, owner_key, filename, file_type, category, tags, size_bytes, url, description, uploaded_at
             FROM assets WHERE {} ORDER BY uploaded_at DESC LIMIT {}",
            conditions.join(" AND "),
            limit,
        );

        let mut stmt = conn.prepare(&query)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "owner_key": row.get::<_, String>(1)?,
                "filename": row.get::<_, String>(2)?,
                "file_type": row.get::<_, String>(3)?,
                "category": row.get::<_, String>(4)?,
                "tags": row.get::<_, String>(5)?,
                "size_bytes": row.get::<_, i64>(6)?,
                "url": row.get::<_, String>(7)?,
                "description": row.get::<_, String>(8).unwrap_or_default(),
                "uploaded_at": row.get::<_, String>(9).unwrap_or_default(),
            }))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete an asset by id. Returns true if deleted.
    pub fn delete_asset(&self, id: &str, owner_key: &str, is_admin: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = if is_admin {
            conn.execute("DELETE FROM assets WHERE id=?1", params![id])?
        } else {
            conn.execute("DELETE FROM assets WHERE id=?1 AND owner_key=?2", params![id, owner_key])?
        };
        Ok(rows > 0)
    }
}
