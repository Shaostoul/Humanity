use super::Storage;
use rusqlite::params;

/// A project record from the database.
#[derive(Debug, Clone)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_key: String,
    pub visibility: String,
    pub color: String,
    pub icon: String,
    pub created_at: String,
}

fn map_project_row(row: &rusqlite::Row) -> rusqlite::Result<ProjectRecord> {
    Ok(ProjectRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        owner_key: row.get(3)?,
        visibility: row.get(4)?,
        color: row.get(5)?,
        icon: row.get(6)?,
        created_at: row.get(7)?,
    })
}

impl Storage {
    // ── Projects: CRUD methods ──

    /// Create a new project.
    pub fn create_project(
        &self,
        id: &str,
        name: &str,
        description: &str,
        owner_key: &str,
        visibility: &str,
        color: &str,
        icon: &str,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO projects (id, name, description, owner_key, visibility, color, icon, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![id, name, description, owner_key, visibility, color, icon],
            )?;
            Ok(())
        })
    }

    /// Get all projects visible to a given viewer.
    /// Returns public projects + projects owned by the viewer.
    /// Each record includes a task_count computed via subquery.
    pub fn get_projects(
        &self,
        visibility_filter: Option<&str>,
        owner_key: Option<&str>,
    ) -> Result<Vec<(ProjectRecord, i64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            // Build dynamic query based on filters.
            let mut sql = String::from(
                "SELECT p.id, p.name, p.description, p.owner_key, p.visibility, p.color, p.icon, p.created_at,
                        COUNT(t.id) as task_count
                 FROM projects p
                 LEFT JOIN project_tasks t ON t.project = p.id
                 WHERE 1=1"
            );
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(vis) = visibility_filter {
                let n = param_values.len() + 1;
                sql.push_str(&format!(" AND p.visibility = ?{n}"));
                param_values.push(Box::new(vis.to_string()));
            }

            if let Some(key) = owner_key {
                let n = param_values.len() + 1;
                sql.push_str(&format!(" AND p.owner_key = ?{n}"));
                param_values.push(Box::new(key.to_string()));
            }

            sql.push_str(" GROUP BY p.id ORDER BY p.name");

            let mut stmt = conn.prepare(&sql)?;
            let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                let project = map_project_row(row)?;
                let task_count: i64 = row.get(8)?;
                Ok((project, task_count))
            })?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
    }

    /// Get all projects visible to a specific viewer (public + own projects).
    pub fn get_projects_visible_to(
        &self,
        viewer_key: &str,
    ) -> Result<Vec<(ProjectRecord, i64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.name, p.description, p.owner_key, p.visibility, p.color, p.icon, p.created_at,
                        COUNT(t.id) as task_count
                 FROM projects p
                 LEFT JOIN project_tasks t ON t.project = p.id
                 WHERE p.visibility = 'public'
                    OR p.owner_key = ?1
                 GROUP BY p.id
                 ORDER BY p.name"
            )?;
            let rows = stmt.query_map(params![viewer_key], |row| {
                let project = map_project_row(row)?;
                let task_count: i64 = row.get(8)?;
                Ok((project, task_count))
            })?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
    }

    /// Get a single project by ID.
    pub fn get_project_by_id(&self, id: &str) -> Result<Option<ProjectRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT id, name, description, owner_key, visibility, color, icon, created_at
                 FROM projects WHERE id = ?1",
                params![id],
                map_project_row,
            ) {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Update a project. Owner or admin can update.
    pub fn update_project(
        &self,
        id: &str,
        owner_key: &str,
        name: &str,
        description: &str,
        visibility: &str,
        color: &str,
        icon: &str,
        is_admin: bool,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = if is_admin {
                conn.execute(
                    "UPDATE projects SET name=?1, description=?2, visibility=?3, color=?4, icon=?5
                     WHERE id=?6",
                    params![name, description, visibility, color, icon, id],
                )?
            } else {
                conn.execute(
                    "UPDATE projects SET name=?1, description=?2, visibility=?3, color=?4, icon=?5
                     WHERE id=?6 AND owner_key=?7",
                    params![name, description, visibility, color, icon, id, owner_key],
                )?
            };
            Ok(rows > 0)
        })
    }

    /// Delete a project. Reassigns its tasks to 'default'. Owner or admin can delete.
    /// The 'default' project cannot be deleted.
    pub fn delete_project(
        &self,
        id: &str,
        owner_key: &str,
        is_admin: bool,
    ) -> Result<bool, rusqlite::Error> {
        if id == "default" {
            return Ok(false);
        }
        self.with_conn(|conn| {
            // Reassign tasks from this project to 'default'.
            conn.execute(
                "UPDATE project_tasks SET project = 'default' WHERE project = ?1",
                params![id],
            )?;
            let rows = if is_admin {
                conn.execute("DELETE FROM projects WHERE id=?1", params![id])?
            } else {
                conn.execute(
                    "DELETE FROM projects WHERE id=?1 AND owner_key=?2",
                    params![id, owner_key],
                )?
            };
            Ok(rows > 0)
        })
    }
}
