use super::Storage;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct CivilizationStats {
    pub population: PopulationStats,
    pub infrastructure: InfrastructureStats,
    pub economy: EconomyStats,
    pub resources: ResourceStats,
    pub social: SocialStats,
    pub activity: ActivityStats,
}

#[derive(Debug, Serialize)]
pub struct PopulationStats {
    pub total_members: u32,
    pub online_now: u32,
    pub new_this_week: u32,
    pub roles: HashMap<String, u32>,
}

#[derive(Debug, Serialize)]
pub struct InfrastructureStats {
    pub channels: u32,
    pub voice_channels: u32,
    pub projects: u32,
    pub total_messages: u32,
    pub messages_today: u32,
}

#[derive(Debug, Serialize)]
pub struct EconomyStats {
    pub active_listings: u32,
    pub total_trades: u32,
    pub total_reviews: u32,
}

#[derive(Debug, Serialize)]
pub struct ResourceStats {
    pub total_tasks: u32,
    pub tasks_completed: u32,
    pub tasks_in_progress: u32,
    pub tasks_open: u32,
}

#[derive(Debug, Serialize)]
pub struct SocialStats {
    pub total_follows: u32,
    pub total_dms: u32,
}

#[derive(Debug, Serialize)]
pub struct ActivityStats {
    pub most_active_channel: String,
    pub messages_today: u32,
    pub peak_online: u32,
}

impl Storage {
    pub fn get_civilization_stats(&self, online_count: u32) -> CivilizationStats {
        let db = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let one_week_ago = now - 7 * 24 * 60 * 60 * 1000;
        let today_start = now - (now % (24 * 60 * 60 * 1000));

        // Population
        let total_members: u32 = db
            .query_row("SELECT COUNT(*) FROM server_members", [], |r| r.get(0))
            .unwrap_or(0);
        let new_this_week: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM server_members WHERE joined_at > ?1",
                [one_week_ago],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let mut roles = HashMap::new();
        if let Ok(mut stmt) = db.prepare(
            "SELECT COALESCE(role, 'member'), COUNT(*) FROM server_members GROUP BY COALESCE(role, 'member')",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
            }) {
                for row in rows.flatten() {
                    roles.insert(row.0, row.1);
                }
            }
        }

        // Infrastructure
        let channels: u32 = db
            .query_row("SELECT COUNT(*) FROM channels", [], |r| r.get(0))
            .unwrap_or(0);
        let voice_channels: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM channels WHERE name LIKE '%voice%'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let projects: u32 = db
            .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
            .unwrap_or(0);
        let total_messages: u32 = db
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap_or(0);
        let messages_today: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE timestamp > ?1",
                [today_start],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Economy
        let active_listings: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM listings WHERE status = 'active'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| {
                db.query_row("SELECT COUNT(*) FROM listings", [], |r| r.get(0))
                    .unwrap_or(0)
            });
        let total_reviews: u32 = db
            .query_row("SELECT COUNT(*) FROM listing_reviews", [], |r| r.get(0))
            .unwrap_or(0);

        // Resources (tasks)
        let total_tasks: u32 = db
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
            .unwrap_or(0);
        let tasks_completed: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'done'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tasks_in_progress: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'in-progress'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tasks_open: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'todo' OR status = 'open'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Social
        let total_follows: u32 = db
            .query_row("SELECT COUNT(*) FROM follows", [], |r| r.get(0))
            .unwrap_or(0);
        let total_dms: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE channel LIKE 'dm:%'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Activity
        let most_active_channel: String = db
            .query_row(
                "SELECT channel FROM messages GROUP BY channel ORDER BY COUNT(*) DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "general".to_string());

        CivilizationStats {
            population: PopulationStats {
                total_members,
                online_now: online_count,
                new_this_week,
                roles,
            },
            infrastructure: InfrastructureStats {
                channels,
                voice_channels,
                projects,
                total_messages,
                messages_today,
            },
            economy: EconomyStats {
                active_listings,
                total_trades: 0,
                total_reviews,
            },
            resources: ResourceStats {
                total_tasks,
                tasks_completed,
                tasks_in_progress,
                tasks_open,
            },
            social: SocialStats {
                total_follows,
                total_dms,
            },
            activity: ActivityStats {
                most_active_channel,
                messages_today,
                peak_online: online_count,
            },
        }
    }
}
