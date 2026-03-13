use super::Storage;
use rusqlite::params;

impl Storage {
    // ── Stream methods ──

    /// Create a new stream record. Returns the stream ID.
    pub fn create_stream(&self, streamer_key: &str, title: &str, category: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis() as i64;
        conn.execute(
            "INSERT INTO streams (streamer_key, title, category, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![streamer_key, title, category, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// End a stream (set ended_at and viewer_peak).
    pub fn end_stream(&self, stream_id: i64, viewer_peak: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis() as i64;
        conn.execute(
            "UPDATE streams SET ended_at = ?1, viewer_peak = MAX(viewer_peak, ?2) WHERE id = ?3",
            params![now, viewer_peak, stream_id],
        )?;
        Ok(())
    }

    /// Update the viewer peak for an active stream.
    pub fn update_stream_viewer_peak(&self, stream_id: i64, peak: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE streams SET viewer_peak = MAX(viewer_peak, ?1) WHERE id = ?2",
            params![peak, stream_id],
        )?;
        Ok(())
    }

    /// Store a stream chat message.
    pub fn store_stream_chat(&self, stream_id: i64, content: &str, from_name: &str, source: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = super::now_millis() as i64;
        conn.execute(
            "INSERT INTO stream_chat (stream_id, content, from_name, source, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![stream_id, content, from_name, source, now],
        )?;
        Ok(())
    }

    /// Get recent streams (for history display).
    pub fn get_recent_streams(&self, limit: usize) -> Result<Vec<(i64, String, String, String, i64, Option<i64>, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, streamer_key, title, category, started_at, ended_at, viewer_peak FROM streams ORDER BY started_at DESC LIMIT ?1"
        )?;
        let streams = stmt.query_map(params![limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(streams)
    }
}
