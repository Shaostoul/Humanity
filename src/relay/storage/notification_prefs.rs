//! Server storage for a person's notification preferences (`notification_prefs`): direct
//! messages, mentions and tasks on or off, the do-not-disturb hours, and (notifications rung 2,
//! 2026-10-31) the quiet-hours exceptions, by public key.
//!
//! A CHANGE names only what it changes (2026-10-30, [`NotifPrefsChange`]): every switch a message
//! leaves out keeps its stored value, and quiet hours sent as null are cleared. Before, every
//! `update_notification_prefs` replaced all five values, a missing switch counting as on, so a
//! client that sent only what it knew (the website's DM cog) wiped the person's quiet hours and
//! turned mentions back on, on every device.
//!
//! WHAT THE SERVER CAN AND CANNOT KNOW for the exceptions (docs/design/notifications.md, section
//! 7). The exceptions are stored here so every device decides the same, and the devices apply
//! them (the desktop and the website's chat tab, through THE rule). For its own Web Push the
//! server knows a push is a mention, so the mentions exception is one it can apply; it can never
//! apply the friends exception to a DM, because DMs are sealed-sender: the server does not know
//! who sent one, and the design keeps it that way rather than tell it.
//! [`NotifPrefs::push_comes_through_quiet_hours`] writes that answer down. The server applies no
//! quiet hours to a push yet (it does not know the person's clock: rung 4), so today every push
//! its switch allows goes out, at any hour.
use super::Storage;
use rusqlite::params;

/// What one `update_notification_prefs` changes: `None` leaves a value as stored; for the quiet
/// hours `Some(None)` clears them (null on the wire) and `Some(Some(t))` sets them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NotifPrefsChange {
    pub dm: Option<bool>,
    pub mentions: Option<bool>,
    pub tasks: Option<bool>,
    pub dnd_start: Option<Option<String>>,
    pub dnd_end: Option<Option<String>>,
    /// The quiet-hours exceptions (rung 2): a friend's DM, a mention or reply.
    pub quiet_friends: Option<bool>,
    pub quiet_mentions: Option<bool>,
}

/// Serde: a field that is present reads as `Some` (its value, or `None` for null); one that is
/// absent stays `None` through `#[serde(default)]`. How the relay tells "clear" from "not sent".
pub fn sent_or_null<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(d).map(Some)
}

/// Notification preference record from the database.
#[derive(Debug, Clone)]
pub struct NotifPrefs {
    pub public_key: String,
    pub dm_enabled: bool,
    pub mentions_enabled: bool,
    pub tasks_enabled: bool,
    pub dnd_start: Option<String>,
    pub dnd_end: Option<String>,
    /// Quiet-hours exception: a friend's DM still notifies (rung 2; off by default).
    pub quiet_friends: bool,
    /// Quiet-hours exception: a mention or a reply still notifies (rung 2; off by default).
    pub quiet_mentions: bool,
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

    /// May a push of `kind` go out during the person's quiet hours, by their exceptions? The
    /// server's half of the exception rule, as the devices apply it (notifications.md, section 7):
    /// a mention with the mentions exception on, yes; a DM, never, whatever the friends exception
    /// says, because a DM is sealed-sender and the server cannot tell a friend's from anyone's.
    /// The server applies no quiet hours to a push until it knows the person's clock (rung 4,
    /// which reads this); until then every push its switch allows goes out at any hour.
    pub fn push_comes_through_quiet_hours(&self, kind: PushKind) -> bool {
        match kind {
            PushKind::Mention => self.quiet_mentions,
            PushKind::Dm => false,
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

    /// Apply `change` to what `public_key` has stored (the defaults when nothing is: every switch
    /// on, no quiet hours), save it, and return the whole stored set, for the answer every one of
    /// their devices receives.
    ///
    /// One statement, so two of the person's devices changing different switches at once cannot
    /// undo each other (a read, then a write, could).
    pub fn change_notification_prefs(&self, public_key: &str, change: &NotifPrefsChange) -> Result<NotifPrefs, rusqlite::Error> {
        let flag = |b: Option<bool>| b.map(|v| v as i32);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO notification_prefs (public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end, quiet_friends, quiet_mentions)
                 VALUES (?1, COALESCE(?2, 1), COALESCE(?3, 1), COALESCE(?4, 1), ?5, ?6, COALESCE(?9, 0), COALESCE(?10, 0))
                 ON CONFLICT(public_key) DO UPDATE SET
                     dm_enabled = COALESCE(?2, dm_enabled),
                     mentions_enabled = COALESCE(?3, mentions_enabled),
                     tasks_enabled = COALESCE(?4, tasks_enabled),
                     dnd_start = CASE WHEN ?7 THEN ?5 ELSE dnd_start END,
                     dnd_end = CASE WHEN ?8 THEN ?6 ELSE dnd_end END,
                     quiet_friends = COALESCE(?9, quiet_friends),
                     quiet_mentions = COALESCE(?10, quiet_mentions)",
                params![
                    public_key,
                    flag(change.dm),
                    flag(change.mentions),
                    flag(change.tasks),
                    change.dnd_start.clone().flatten(),
                    change.dnd_end.clone().flatten(),
                    change.dnd_start.is_some(),
                    change.dnd_end.is_some(),
                    flag(change.quiet_friends),
                    flag(change.quiet_mentions),
                ],
            )?;
            Ok::<(), rusqlite::Error>(())
        })?;
        self.get_notification_prefs(public_key).ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    /// Get notification preferences for a user. Returns None if no prefs are stored.
    pub fn get_notification_prefs(&self, public_key: &str) -> Option<NotifPrefs> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end, quiet_friends, quiet_mentions
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
                        quiet_friends: row.get::<_, i32>(6)? != 0,
                        quiet_mentions: row.get::<_, i32>(7)? != 0,
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
        let all = |dm, mentions, quiet: Option<&str>| NotifPrefsChange {
            dm: Some(dm),
            mentions: Some(mentions),
            tasks: Some(true),
            dnd_start: Some(quiet.map(str::to_string)),
            dnd_end: Some(quiet.map(|_| "07:00".to_string())),
            ..Default::default()
        };
        db.change_notification_prefs("k1", &all(false, true, Some("22:00"))).unwrap();
        assert!(!db.wants_push("k1", PushKind::Dm), "DMs off");
        assert!(db.wants_push("k1", PushKind::Mention), "mentions on");
        db.change_notification_prefs("k1", &all(true, false, None)).unwrap();
        assert!(db.wants_push("k1", PushKind::Dm) && !db.wants_push("k1", PushKind::Mention));
        assert!(db.wants_push("k2", PushKind::Dm), "another person's switches are theirs");
    }

    /// The quiet-hours exceptions (rung 2, 2026-10-31): off until set, kept when a change leaves
    /// them out, and on the server's side a DM never comes through quiet hours by the friends
    /// exception (sealed sender: the server cannot tell a friend), while a mention can by the
    /// mentions exception. Seen red 2026-10-31 with `push_comes_through_quiet_hours` answering
    /// `quiet_friends` for a DM ("the server cannot know a DM's sender").
    #[test]
    fn quiet_hours_exceptions_are_kept_and_the_server_never_excepts_a_dm() {
        let db = Storage::open_temp("notification_prefs_exceptions");
        let p = db.change_notification_prefs("k1", &NotifPrefsChange { tasks: Some(false), ..Default::default() }).unwrap();
        assert!(!p.quiet_friends && !p.quiet_mentions, "off by default");
        db.change_notification_prefs("k1", &NotifPrefsChange { quiet_friends: Some(true), ..Default::default() }).unwrap();
        let p = db.change_notification_prefs("k1", &NotifPrefsChange { quiet_mentions: Some(true), ..Default::default() }).unwrap();
        assert!(p.quiet_friends && p.quiet_mentions, "a change names only what it changes");
        assert!(!p.tasks_enabled, "and the rest stays");
        assert!(!p.push_comes_through_quiet_hours(PushKind::Dm), "the server cannot know a DM's sender");
        assert!(p.push_comes_through_quiet_hours(PushKind::Mention));
        let p = db.change_notification_prefs("k1", &NotifPrefsChange { quiet_mentions: Some(false), ..Default::default() }).unwrap();
        assert!(!p.push_comes_through_quiet_hours(PushKind::Mention) && p.quiet_friends);
    }

    /// A live database from before rung 2 (the table without the two exception columns, a row in
    /// it) opens, gains the columns with their defaults, and keeps the row. BUG-046's class: every
    /// fresh-database test passes while the live one dies at startup. Seen red 2026-10-31 with the
    /// guarded ALTERs skipped: the old row could not be read ("the old row"), its SELECT naming a
    /// column the old table lacks.
    #[test]
    fn opens_a_pre_rung_2_database_and_migrates_it() {
        let path = crate::test_temp::db("notif_prefs_mig");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE notification_prefs (
                    public_key        TEXT PRIMARY KEY,
                    dm_enabled        INTEGER NOT NULL DEFAULT 1,
                    mentions_enabled  INTEGER NOT NULL DEFAULT 1,
                    tasks_enabled     INTEGER NOT NULL DEFAULT 1,
                    dnd_start         TEXT DEFAULT NULL,
                    dnd_end           TEXT DEFAULT NULL
                );
                INSERT INTO notification_prefs (public_key, dm_enabled, dnd_start, dnd_end) VALUES ('carol', 0, '22:00', '07:00');",
            )
            .expect("seed the old table");
        }
        let db = Storage::open(&path).expect("startup must survive a pre-rung-2 database");
        let p = db.get_notification_prefs("carol").expect("the old row");
        assert!(!p.dm_enabled && p.dnd_start.as_deref() == Some("22:00"));
        assert!(!p.quiet_friends && !p.quiet_mentions, "the new columns' defaults");
        let p = db.change_notification_prefs("carol", &NotifPrefsChange { quiet_friends: Some(true), ..Default::default() }).unwrap();
        assert!(p.quiet_friends && !p.dm_enabled);
    }
}
