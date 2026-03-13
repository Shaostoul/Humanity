use super::Storage;
use super::{TaskRecord, TaskCommentRecord};
use rusqlite::params;
use std::collections::HashMap;

impl Storage {
    // ── Project Board: Task methods ──

    /// Create a new task. Returns the new task ID.
    pub fn create_task(
        &self,
        title: &str,
        description: &str,
        status: &str,
        priority: &str,
        assignee: Option<&str>,
        created_by: &str,
        labels: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        // Position: max position in status column + 1.
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM project_tasks WHERE status = ?1",
            params![status],
            |row| row.get(0),
        ).unwrap_or(0);
        conn.execute(
            "INSERT INTO project_tasks (title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9)",
            params![title, description, status, priority, assignee, created_by, now, max_pos + 1, labels],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update an existing task.
    pub fn update_task(
        &self,
        id: i64,
        title: &str,
        description: &str,
        priority: &str,
        assignee: Option<&str>,
        labels: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "UPDATE project_tasks SET title = ?1, description = ?2, priority = ?3, assignee = ?4, labels = ?5, updated_at = ?6 WHERE id = ?7",
            params![title, description, priority, assignee, labels, now, id],
        )?;
        Ok(rows > 0)
    }

    /// Move a task to a new status column.
    pub fn move_task(&self, id: i64, new_status: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM project_tasks WHERE status = ?1",
            params![new_status],
            |row| row.get(0),
        ).unwrap_or(0);
        let rows = conn.execute(
            "UPDATE project_tasks SET status = ?1, position = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_status, max_pos + 1, now, id],
        )?;
        Ok(rows > 0)
    }

    /// Delete a task and its comments.
    pub fn delete_task(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM task_comments WHERE task_id = ?1", params![id])?;
        let rows = conn.execute("DELETE FROM project_tasks WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// List all tasks, ordered by status then position.
    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels
             FROM project_tasks
             ORDER BY position ASC, id ASC"
        )?;
        let tasks = stmt.query_map([], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
                priority: row.get(4)?,
                assignee: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                position: row.get(9)?,
                labels: row.get(10)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(tasks)
    }

    /// Get a single task by ID.
    pub fn get_task(&self, id: i64) -> Result<Option<TaskRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT id, title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels
             FROM project_tasks WHERE id = ?1",
            params![id],
            |row| Ok(TaskRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
                priority: row.get(4)?,
                assignee: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                position: row.get(9)?,
                labels: row.get(10)?,
            }),
        ) {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Add a comment to a task.
    pub fn add_task_comment(
        &self,
        task_id: i64,
        author_key: &str,
        author_name: &str,
        content: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT INTO task_comments (task_id, author_key, author_name, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task_id, author_key, author_name, content, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get comments for a task, ordered by created_at ASC.
    pub fn get_task_comments(&self, task_id: i64) -> Result<Vec<TaskCommentRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, author_key, author_name, content, created_at
             FROM task_comments WHERE task_id = ?1 ORDER BY created_at ASC"
        )?;
        let comments = stmt.query_map(params![task_id], |row| {
            Ok(TaskCommentRecord {
                id: row.get(0)?,
                task_id: row.get(1)?,
                author_key: row.get(2)?,
                author_name: row.get(3)?,
                content: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(comments)
    }

    /// Get comment count per task (for display on cards).
    pub fn get_task_comment_counts(&self) -> Result<HashMap<i64, i64>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT task_id, COUNT(*) FROM task_comments GROUP BY task_id"
        )?;
        let counts: HashMap<i64, i64> = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(counts)
    }
}
