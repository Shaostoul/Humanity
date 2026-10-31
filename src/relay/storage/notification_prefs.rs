//! Server storage for a person's notification preferences (`notification_prefs`): direct
//! messages, mentions and tasks on or off, and the do-not-disturb hours, by public key.
use super::Storage;
use rusqlite::params;

/// Notification preference record from the database.
#[derive(Debug, Clone)]
pub struct NotifPrefs {
    pub public_key: String,
    pub dm_enabled: bool,
    pub mentions_enabled: bool,
    pub tasks_enabled: bool,
    pub dnd_start: Option<String>,
    pub dnd_end: Option<String>,
}

/// What a Web Push is about, for the person's switch that allows it (docs/design/notifications.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushKind {
    Dm,
    Mention,
}

impl NotifPrefs {
    /// Does this person's switch allow a push of `kind`? (Quiet hours are by the person's own clock,
    /// which the server does not know: rung 4 of the plan.)
    pub fn allows(&self, kind: PushKind) -> bool {
        match kind {
            PushKind::Dm => self.dm_enabled,
            PushKind::Mention => self.mentions_enabled,
        }
    }
}

impl Storage {
    /// Does `public_key` want a push of `kind`? Yes when they never saved preferences (the
    /// switches' defaults are on), otherwise as their switch says (2026-10-30: before, every push
    /// went out whatever the switches said).
    pub fn wants_push(&self, public_key: &str, kind: PushKind) -> bool {
        self.get_notification_prefs(public_key).map_or(true, |p| p.allows(kind))
    }

    /// Save notification preferences for a user (upsert).
    pub fn save_notification_prefs(
        &self,
        public_key: &str,
        dm: bool,
        mentions: bool,
        tasks: bool,
        dnd_start: Option<&str>,
        dnd_end: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO notification_prefs (public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(public_key) DO UPDATE SET
                     dm_enabled = excluded.dm_enabled,
                     mentions_enabled = excluded.mentions_enabled,
                     tasks_enabled = excluded.tasks_enabled,
                     dnd_start = excluded.dnd_start,
                     dnd_end = excluded.dnd_end",
                params![public_key, dm as i32, mentions as i32, tasks as i32, dnd_start, dnd_end],
            )?;
            Ok(())
        })
    }

    /// Get notification preferences for a user. Returns None if no prefs are stored.
    pub fn get_notification_prefs(&self, public_key: &str) -> Option<NotifPrefs> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end
                 FROM notification_prefs WHERE public_key = ?1",
                params![public_key],
                |row| {
                    Ok(NotifPrefs {
                        public_key: row.get(0)?,
                        dm_enabled: row.get::<_, i32>(1)? != 0,
                        mentions_enabled: row.get::<_, i32>(2)? != 0,
                        tasks_enabled: row.get::<_, i32>(3)? != 0,
                        dnd_start: row.get(4)?,
                        dnd_end: row.get(5)?,
                    })
                },
            )
            .ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A push goes out only when the person's switch for it is on; someone who never saved
    /// preferences gets both (the switches' defaults). Seen red 2026-10-30 with `wants_push`
    /// answering true whatever was stored: a person with DMs off still got the DM push.
    #[test]
    fn a_push_needs_its_switch_on() {
        let db = Storage::open_temp("notification_prefs");
        assert!(db.wants_push("k1", PushKind::Dm) && db.wants_push("k1", PushKind::Mention), "nothing saved: on");
        db.save_notification_prefs("k1", false, true, true, Some("22:00"), Some("07:00")).unwrap();
        assert!(!db.wants_push("k1", PushKind::Dm), "DMs off");
        assert!(db.wants_push("k1", PushKind::Mention), "mentions on");
        db.save_notification_prefs("k1", true, false, true, None, None).unwrap();
        assert!(db.wants_push("k1", PushKind::Dm) && !db.wants_push("k1", PushKind::Mention));
        assert!(db.wants_push("k2", PushKind::Dm), "another person's switches are theirs");
    }
}
