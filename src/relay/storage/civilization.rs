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

        // Resources (tasks).
        //
        // THESE COUNTED A TABLE THAT DOES NOT EXIST until 2026-09-20. The table
        // is `project_tasks`; `tasks` has never been its name. Every query here
        // ends in `.unwrap_or(0)`, so a `no such table` error was swallowed and
        // the Mission Dashboard reported a flat zero for every task figure, with
        // nothing logged anywhere. The status vocabulary was wrong too:
        // `project_tasks.status` defaults to 'backlog' and the clients write
        // 'backlog', 'in_progress' and 'done' - never 'todo', 'open' or the
        // hyphenated 'in-progress' these asked for.
        let total_tasks: u32 = db
            .query_row("SELECT COUNT(*) FROM project_tasks", [], |r| r.get(0))
            .unwrap_or(0);
        let tasks_completed: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM project_tasks WHERE status = 'done'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tasks_in_progress: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM project_tasks WHERE status = 'in_progress'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tasks_open: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM project_tasks WHERE status = 'backlog'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Social.
        //
        // THERE IS NO FOLLOW COUNT, AND THERE MUST NOT BE ONE. The `follows`
        // table was the last server-side social graph and it was DELETED on
        // 2026-08-24 in the privacy-maximization pass: following is now sealed
        // control DMs plus client-held friendship certificates, verified
        // statelessly, so the relay genuinely cannot know this number. This
        // query survived the deletion and returned 0 through `.unwrap_or(0)`,
        // which read as "nobody follows anybody" rather than as "not knowable".
        // Do not re-add the table to make this work; see the removed-tables note
        // in CLAUDE.md.
        let total_follows: u32 = 0;
        // Same story, twice over: the column is `channel_id`, not `channel`, and
        // DMs have not lived in `messages` since the sealed-sender cutover of
        // 2026-08-23. They are in `dm_mailbox`, which deliberately has no sender
        // column and expires. Counting the mailbox is the honest replacement: it
        // is undelivered-or-unexpired mail, not lifetime DMs, and it cannot be
        // attributed to anyone.
        let total_dms: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM dm_mailbox",
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

#[cfg(test)]
mod civilization_counter_tests {
    use super::*;

    fn test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_civ_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// THE DASHBOARD MUST COUNT TASKS THAT EXIST.
    ///
    /// Every counter in `get_civilization_stats` ends in `.unwrap_or(0)`, so a
    /// query against a table that does not exist returns a perfectly ordinary
    /// zero and nothing is logged. Four of them asked for `FROM tasks`, which
    /// has never been the table's name - it is `project_tasks` - so the
    /// Mission Dashboard reported a flat zero for every task figure and read
    /// to a user as "nobody has made any tasks" rather than as "we asked the
    /// wrong question".
    ///
    /// The status vocabulary was wrong in the same way: these asked for
    /// 'todo', 'open' and the hyphenated 'in-progress', while the schema
    /// defaults to 'backlog' and the clients write 'backlog', 'in_progress'
    /// and 'done'. So even against the right table three of the four would
    /// have stayed at zero.
    ///
    /// PROVEN RED: change any of the four queries back to `FROM tasks`, or a
    /// status back to 'todo' / 'in-progress', and this fails on that figure.
    #[test]
    fn the_dashboard_counts_tasks_that_exist_in_every_status() {
        let db = test_storage();
        db.register_name("Ann", "ann_key").unwrap();
        db.create_task("done one", "", "done", "medium", None, "ann_key", "").unwrap();
        db.create_task("doing one", "", "in_progress", "medium", None, "ann_key", "").unwrap();
        db.create_task("backlog one", "", "backlog", "medium", None, "ann_key", "").unwrap();
        db.create_task("backlog two", "", "backlog", "medium", None, "ann_key", "").unwrap();

        let s = db.get_civilization_stats(0);
        assert_eq!(s.resources.total_tasks, 4, "four tasks exist; a zero here means the query missed the table");
        assert_eq!(s.resources.tasks_completed, 1, "one is done");
        assert_eq!(s.resources.tasks_in_progress, 1, "one is in_progress (underscore, not hyphen)");
        assert_eq!(s.resources.tasks_open, 2, "two are backlog");
    }

    /// The relay CANNOT know a follow count, and must not pretend to.
    ///
    /// `follows` was the last server-side social graph and it was deleted on
    /// 2026-08-24: following is sealed control DMs plus client-held friendship
    /// certificates verified statelessly. The old query survived the deletion
    /// and returned 0 through `.unwrap_or(0)`, which is indistinguishable from
    /// a real zero. This pins the honest answer AND guards the privacy
    /// property: if someone re-adds the table to make a number appear, the
    /// grep in this test fails and they have to read why first.
    #[test]
    fn there_is_no_server_side_follow_graph_to_count() {
        let db = test_storage();
        let s = db.get_civilization_stats(0);
        assert_eq!(s.social.total_follows, 0, "not knowable, by design");

        let exists: i64 = db
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'follows'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            exists, 0,
            "a `follows` table is back. It was deleted on purpose (privacy maximization, \
             2026-08-24) - following lives in sealed control DMs and client-held \
             certificates. See the removed-tables note in CLAUDE.md before re-adding it."
        );
    }
}
