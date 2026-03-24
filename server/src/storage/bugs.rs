//! Bug report storage: CRUD for community bug reports with voting.

use super::Storage;
use rusqlite::{params, OptionalExtension};

/// A bug report record from the database.
#[derive(Debug, Clone)]
pub struct BugReport {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub steps: String,
    pub expected: String,
    pub actual: String,
    pub severity: String,
    pub category: String,
    pub reporter_key: String,
    pub reporter_name: String,
    pub browser_info: String,
    pub page_url: String,
    pub version: String,
    pub status: String,
    pub votes: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

fn map_bug_row(row: &rusqlite::Row) -> rusqlite::Result<BugReport> {
    Ok(BugReport {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        steps: row.get(3)?,
        expected: row.get(4)?,
        actual: row.get(5)?,
        severity: row.get(6)?,
        category: row.get(7)?,
        reporter_key: row.get(8)?,
        reporter_name: row.get(9)?,
        browser_info: row.get(10)?,
        page_url: row.get(11)?,
        version: row.get(12)?,
        status: row.get(13)?,
        votes: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

impl Storage {
    // ── Bug Report methods ──

    /// Create a new bug report. Returns the new bug ID.
    pub fn create_bug(
        &self,
        title: &str,
        description: &str,
        steps: &str,
        expected: &str,
        actual: &str,
        severity: &str,
        category: &str,
        reporter_key: &str,
        reporter_name: &str,
        browser_info: &str,
        page_url: &str,
        version: &str,
    ) -> Result<i64, rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO bug_reports (title, description, steps, expected, actual, severity, category, reporter_key, reporter_name, browser_info, page_url, version, status, votes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'open', 0, ?13, ?13)",
                params![title, description, steps, expected, actual, severity, category, reporter_key, reporter_name, browser_info, page_url, version, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get bug reports with optional filtering and pagination.
    pub fn get_bugs(
        &self,
        status: Option<&str>,
        severity: Option<&str>,
        category: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<BugReport>, rusqlite::Error> {
        let limit = limit.min(200) as i64;
        let offset = offset as i64;
        self.with_conn(|conn| {
            let mut where_clauses = Vec::new();
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(s) = status {
                where_clauses.push(format!("status = ?{}", param_values.len() + 1));
                param_values.push(Box::new(s.to_string()));
            }
            if let Some(s) = severity {
                where_clauses.push(format!("severity = ?{}", param_values.len() + 1));
                param_values.push(Box::new(s.to_string()));
            }
            if let Some(c) = category {
                where_clauses.push(format!("category = ?{}", param_values.len() + 1));
                param_values.push(Box::new(c.to_string()));
            }

            let where_sql = if where_clauses.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", where_clauses.join(" AND "))
            };

            let sql = format!(
                "SELECT id, title, description, steps, expected, actual, severity, category,
                        reporter_key, reporter_name, browser_info, page_url, version,
                        status, votes, created_at, updated_at
                 FROM bug_reports
                 {}
                 ORDER BY
                   CASE status WHEN 'open' THEN 0 WHEN 'in_progress' THEN 1 ELSE 2 END,
                   votes DESC, created_at DESC
                 LIMIT ?{} OFFSET ?{}",
                where_sql,
                param_values.len() + 1,
                param_values.len() + 2,
            );

            param_values.push(Box::new(limit));
            param_values.push(Box::new(offset));

            let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|b| b.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_ref.as_slice(), map_bug_row)?;
            rows.collect()
        })
    }

    /// Get a single bug report by ID.
    pub fn get_bug_by_id(&self, id: i64) -> Result<Option<BugReport>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, title, description, steps, expected, actual, severity, category,
                        reporter_key, reporter_name, browser_info, page_url, version,
                        status, votes, created_at, updated_at
                 FROM bug_reports WHERE id = ?1",
                params![id],
                map_bug_row,
            ).optional()
        })
    }

    /// Update a bug report's status. Returns true if a row was updated.
    pub fn update_bug_status(&self, id: i64, status: &str) -> Result<bool, rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            let updated = conn.execute(
                "UPDATE bug_reports SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status, now, id],
            )?;
            Ok(updated > 0)
        })
    }

    /// Upvote a bug report. Uses a separate table to prevent duplicate votes.
    /// Returns (success, new_vote_count).
    pub fn vote_bug(&self, bug_id: i64, voter_key: &str) -> Result<(bool, i64), rusqlite::Error> {
        self.with_conn(|conn| {
            // Check for duplicate vote.
            let already_voted: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM bug_votes WHERE bug_id = ?1 AND voter_key = ?2",
                params![bug_id, voter_key],
                |row| row.get(0),
            )?;

            if already_voted {
                let votes: i64 = conn.query_row(
                    "SELECT votes FROM bug_reports WHERE id = ?1",
                    params![bug_id],
                    |row| row.get(0),
                ).unwrap_or(0);
                return Ok((false, votes));
            }

            conn.execute(
                "INSERT INTO bug_votes (bug_id, voter_key, voted_at) VALUES (?1, ?2, ?3)",
                params![bug_id, voter_key, super::now_millis() as i64],
            )?;

            conn.execute(
                "UPDATE bug_reports SET votes = votes + 1 WHERE id = ?1",
                params![bug_id],
            )?;

            let votes: i64 = conn.query_row(
                "SELECT votes FROM bug_reports WHERE id = ?1",
                params![bug_id],
                |row| row.get(0),
            ).unwrap_or(0);

            Ok((true, votes))
        })
    }
}
