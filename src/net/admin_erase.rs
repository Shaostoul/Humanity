//! An admin erases another person's data (section 10i of docs/design/blocking-and-safe-mode.md,
//! 2026-10-10): the desktop app's rules, with no socket and no GUI so each one is unit tested.
//! The action on a row of Server Settings' member list, its confirm and the receipt are drawn by
//! src/gui/pages/server_settings/admin_erase.rs. The server's receipt reaches `on_frame` from the
//! message pump's catch-all arm (engine/frame_ws_poll.rs sits at its size budget, so nothing more
//! goes there).
//!
//! Why it exists: a server operator who learns a member is under 13 is expected to delete that
//! child's information (docs/reference/findings/2026-10-10-childrens-online-safety-rules.md,
//! judgement call 7), and until now only a person could erase their own data. The same tool serves
//! someone who asks an admin to remove them when they cannot reach their own device.
//!
//! THE PROTOCOL (10i, fixed so the relay and both clients agree):
//! - sent: `{"type":"admin_erase","target":"<their public key>","confirm_name":"<their name, typed>"}`;
//! - answered, to the admin: `{"type":"admin_erase_done","name","receipt":[["<table>",<rows>],...],"partial"}`,
//!   the same per-table counts the self-erase reports;
//! - refused (not an admin, themselves, an admin or the owner, no account here, a wrong name): a
//!   `private` notice, which the app shows in Chat, and nothing changed;
//! - the erased person's own clients get `account_erased` with `by_admin: true`, and say so
//!   (gui/connections.rs `EraseOutcome::ErasedByAdmin`).

use serde_json::Value;

/// The member list's action, on a row this viewer may erase. The words in this file are the web
/// client's (web/shared/admin-erase.js), byte for byte, so both apps say the same.
pub const ACTION_LABEL: &str = "Erase their data";

/// The line over the confirm's name field.
pub const TYPE_LABEL: &str = "Type their name exactly to confirm";

/// What the confirm says, word for word from 10i: what the erase does and, as important, what it
/// does not do, so an admin who wants the person kept out knows a ban is a separate step, and
/// knows the moderation records about them stay (corrected 2026-10-10: the first wording said
/// "everything", but reports, bans and mutes about the person are kept, as with the self-erase).
pub fn confirm_words(name: &str) -> String {
    format!(
        "This deletes everything this server stores about {name}: their messages, profile, uploads, \
         membership and settings. Reports, bans and mutes about them are kept, as when someone \
         erases their own account. It cannot be undone. It does not touch anything on their own \
         devices, and it does not stop them joining again (ban them too for that)."
    )
}

/// An admin or the owner of this server: the only roles that may erase someone else. Read the way
/// the web client reads a role (trimmed, any letter case).
fn is_admin(role: &str) -> bool {
    matches!(role.trim().to_ascii_lowercase().as_str(), "admin" | "owner")
}

/// Whether the member list offers the action on a row. Only an admin or the owner may erase: a
/// moderator may not, because erasing cannot be undone and it is the admins who answer for the
/// server. Never on the viewer's own row (the ordinary erase in Settings is for that), never on
/// an admin's or the owner's row (demote them first, the rule the self-erase has), and never on a
/// bot's (`bot_` keys), as on the web. The relay refuses the people ones anyway; the app does not
/// offer what would only be refused.
pub fn may_offer(viewer_role: &str, viewer_key: &str, target_key: &str, target_role: &str) -> bool {
    let target = target_key.trim();
    is_admin(viewer_role)
        && !target.is_empty()
        && !target.starts_with("bot_")
        && !target.eq_ignore_ascii_case(viewer_key.trim())
        && !is_admin(target_role)
}

/// The confirm's Erase button is enabled only when the typed name, trimmed, is exactly their
/// name. Exactly: case counts, as it does on the relay, so a slip of the keyboard cannot erase a
/// different member with a similar name.
pub fn name_matches(typed: &str, name: &str) -> bool {
    let typed = typed.trim();
    !typed.is_empty() && typed == name
}

/// The frame that asks this server to erase `target`'s data. The typed name goes trimmed, the way
/// Settings' own erase sends it and the relay compares it.
pub fn erase_frame(target: &str, typed: &str) -> Value {
    serde_json::json!({ "type": "admin_erase", "target": target, "confirm_name": typed.trim() })
}

/// The confirm while it is open: whose data (their key and name) and what has been typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EraseConfirm {
    pub target: String,
    pub name: String,
    pub typed: String,
}

/// The server's `admin_erase_done`: the name the admin typed, the rows each table lost, and
/// whether part of the erase failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EraseReceipt {
    pub name: String,
    pub counts: Vec<(String, u64)>,
    pub partial: bool,
}

/// What the member list shows of this feature on the active server. Reset on a server switch
/// (gui/connections.rs `reset_per_server_transients`): an open confirm names one server's member,
/// and must never be sent to another.
#[derive(Debug, Clone, Default)]
pub struct AdminEraseUi {
    /// The confirm, while it is open.
    pub confirm: Option<EraseConfirm>,
    /// A line under the list: the erase sent and not answered yet, or why nothing was sent.
    pub status: String,
    /// The last receipt, until it is dismissed or another arrives.
    pub receipt: Option<EraseReceipt>,
    /// The member list's search box: a name or part of a key. The list draws at most
    /// `MEMBER_ROWS_SHOWN` rows, so without it the action could never reach anyone past them.
    pub member_search: String,
}

/// The most member rows Server Settings > Members draws at once: a server can list thousands, and
/// every row is drawn every frame. Anyone past them is reached with the search box.
pub const MEMBER_ROWS_SHOWN: usize = 50;

/// The members the list shows for the search box's `query`: the indexes, in order, of every
/// `(name, key)` whose name or key holds the typed text, letter case ignored (spaces around it
/// trimmed); everyone when nothing is typed. Found 2026-10-10: the list drew only the first 50
/// members and had no search, so "Erase their data" could not reach anyone after them.
pub fn members_matching(members: &[(String, String)], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    members
        .iter()
        .enumerate()
        .filter(|(_, (name, key))| q.is_empty() || name.to_lowercase().contains(&q) || key.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// Read an `admin_erase_done` frame. None for any other frame. A receipt row that is not a
/// `[label, number]` pair is skipped rather than failing the whole receipt, and only rows above
/// zero are kept, as the self-erase reports them (a failed table is a `<table>_FAILED` row of 1,
/// relay storage/account.rs). `partial` is also true when such a row is there, in case a server
/// says one and not the other.
pub fn parse_done(frame: &Value) -> Option<EraseReceipt> {
    if frame.get("type").and_then(|t| t.as_str()) != Some("admin_erase_done") {
        return None;
    }
    let name = frame.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let counts: Vec<(String, u64)> = frame
        .get("receipt")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let pair = row.as_array()?;
                    Some((pair.first()?.as_str()?.to_string(), pair.get(1)?.as_u64()?))
                })
                .filter(|(_, n)| *n > 0)
                .collect()
        })
        .unwrap_or_default();
    let partial = frame.get("partial").and_then(|v| v.as_bool()) == Some(true)
        || counts.iter().any(|(label, _)| label.ends_with("_FAILED"));
    Some(EraseReceipt { name, counts, partial })
}

/// A frame the message pump did not handle. True when it was an `admin_erase_done`, kept here for
/// the member list to show.
pub fn on_frame(ui: &mut AdminEraseUi, frame: &Value) -> bool {
    let Some(receipt) = parse_done(frame) else { return false };
    ui.status.clear();
    ui.receipt = Some(receipt);
    true
}

/// The receipt in words, for the admin: whose data, the per-table counts as the self-erase
/// reports them (the tables that held something, a failed one as `<table>_FAILED: 1`), and, when
/// part of it failed, that it did not finish and how to finish it. Byte for byte the web client's
/// `adminEraseReceiptText`.
pub fn receipt_words(r: &EraseReceipt) -> String {
    let who = if r.name.is_empty() { "this person" } else { r.name.as_str() };
    let summary: Vec<String> = r.counts.iter().filter(|(_, n)| *n > 0).map(|(l, n)| format!("{l}: {n}")).collect();
    let summary = if summary.is_empty() { "nothing was stored".to_string() } else { summary.join(", ") };
    if r.partial {
        return format!("The erase of {who}'s data did not finish: part of it failed ({summary}). Use {ACTION_LABEL} again to finish it.");
    }
    format!("Erased {who}'s data from this server ({summary}). Anything on their own devices is untouched.")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10i's "who": only an admin or the owner is offered the action, never a moderator (either
    /// spelling the relay accepts) or a plain member, and never on the viewer's own row, an
    /// admin's or the owner's. A moderator's row is offered: the relay refuses only admin targets.
    ///
    /// Seen red 2026-10-10 with `may_offer` also accepting a "mod" or "moderator" viewer: "a
    /// moderator is offered the erase".
    #[test]
    fn only_an_admin_or_the_owner_is_offered_it_and_never_on_an_admins_row() {
        assert!(may_offer("admin", "me", "ann", "member"), "an admin on a member's row");
        assert!(may_offer("owner", "me", "ann", ""), "the owner on an unverified member's row");
        assert!(may_offer("admin", "me", "cy", "mod"), "an admin on a moderator's row");
        assert!(!may_offer("mod", "me", "ann", "member"), "a moderator is offered the erase");
        assert!(!may_offer("moderator", "me", "ann", "member"), "a moderator is offered the erase");
        assert!(!may_offer("member", "me", "ann", "member"), "a member is offered the erase");
        assert!(!may_offer("", "me", "ann", "member"), "an unknown role is offered the erase");
        assert!(!may_offer("admin", "me", "me", "admin"), "an admin is offered it on their own row");
        assert!(!may_offer("owner", "me", "me", "owner"), "the owner is offered it on their own row");
        assert!(!may_offer("admin", "me", "bob", "admin"), "an admin is offered it on another admin");
        assert!(!may_offer("owner", "me", "bob", "admin"), "the owner is offered it on an admin");
        assert!(!may_offer("admin", "me", "zed", "owner"), "an admin is offered it on the owner");
        assert!(!may_offer("admin", "me", "", "member"), "a row with no key");
        // Read as the web reads them (web/shared/admin-erase.js `adminEraseOffered`).
        assert!(may_offer(" Owner ", "me", "ann", "member"), "a role's spelling hid the action from the owner");
        assert!(!may_offer("admin", "AB12", "ab12", "member"), "an admin is offered it on their own row");
        assert!(!may_offer("admin", "me", "bob", "Admin"), "an admin is offered it on another admin");
        assert!(!may_offer("admin", "me", "bot_weather", "member"), "an admin is offered it on a bot");
    }

    /// The Erase button needs their exact name: trimmed, but case and every letter count.
    ///
    /// Seen red 2026-10-10 with the comparison made case-blind (`eq_ignore_ascii_case`): "a name
    /// typed in the wrong case enables Erase".
    #[test]
    fn the_erase_button_needs_their_exact_name() {
        assert!(name_matches("Dana", "Dana"));
        assert!(name_matches("  Dana \t", "Dana"), "spaces around the typed name are trimmed");
        assert!(!name_matches("dana", "Dana"), "a name typed in the wrong case enables Erase");
        assert!(!name_matches("Dan", "Dana"), "part of the name enables Erase");
        assert!(!name_matches("Dana2", "Dana"));
        assert!(!name_matches("", ""), "nothing typed for an empty name enables Erase");
        assert!(!name_matches("   ", ""));
    }

    /// The frame is 10i's, key for key, with nothing added.
    ///
    /// Seen red 2026-10-10 with the name sent as `name` instead of `confirm_name`: the
    /// assertion's left side carried `"name":"Dana"`.
    #[test]
    fn the_frame_is_the_protocols() {
        assert_eq!(
            erase_frame("ab12", " Dana "),
            serde_json::json!({ "type": "admin_erase", "target": "ab12", "confirm_name": "Dana" })
        );
    }

    /// The receipt: the per-table counts above zero, the whole and the partial case (by `partial`
    /// or by a `<table>_FAILED` row alone), in the web client's words byte for byte
    /// (web/shared/admin-erase.js `adminEraseReceiptText`), and nothing else taken for one.
    ///
    /// Seen red 2026-10-10 with `on_frame` keeping the receipt without clearing the waiting line:
    /// "the waiting line stays after the receipt".
    #[test]
    fn the_receipt_is_read_and_said() {
        let done = serde_json::json!({
            "type": "admin_erase_done",
            "name": "Dana",
            "receipt": [["messages", 12], ["profile", 1], ["reactions", 0], ["upload_files_removed", 3]],
            "partial": false
        });
        let mut ui = AdminEraseUi { status: "Asked the server to erase Dana's data.".into(), ..Default::default() };
        assert!(on_frame(&mut ui, &done));
        assert!(ui.status.is_empty(), "the waiting line stays after the receipt");
        let r = ui.receipt.clone().expect("the receipt is kept");
        assert_eq!(r.counts.len(), 3, "a table that held nothing is counted");
        assert!(!r.partial);
        assert_eq!(
            receipt_words(&r),
            "Erased Dana's data from this server (messages: 12, profile: 1, upload_files_removed: 3). Anything on their own devices is untouched."
        );

        let part = serde_json::json!({
            "type": "admin_erase_done",
            "name": "Dana",
            "receipt": [["messages", 12], ["user_uploads_FAILED", 1]],
            "partial": true
        });
        let r = parse_done(&part).unwrap();
        assert!(r.partial);
        assert_eq!(
            receipt_words(&r),
            "The erase of Dana's data did not finish: part of it failed (messages: 12, user_uploads_FAILED: 1). Use Erase their data again to finish it."
        );
        let mut failed_only = part.clone();
        failed_only["partial"] = serde_json::Value::Bool(false);
        assert!(parse_done(&failed_only).unwrap().partial, "a failed table without `partial` read as a whole erase");
        let nothing = parse_done(&serde_json::json!({ "type": "admin_erase_done", "name": " ", "receipt": [], "partial": false })).unwrap();
        assert_eq!(
            receipt_words(&nothing),
            "Erased this person's data from this server (nothing was stored). Anything on their own devices is untouched."
        );

        let mut ui = AdminEraseUi::default();
        assert!(!on_frame(&mut ui, &serde_json::json!({ "type": "account_erased", "partial": false })));
        assert!(ui.receipt.is_none(), "another frame was taken for a receipt");
    }

    /// The confirm's words are 10i's, READ FROM THE SPEC (the quoted sentence, its line wrapping
    /// undone), with the name in its place, so a correction to the spec (2026-10-10: moderation
    /// records about the person are kept) cannot leave the app saying the old words.
    ///
    /// Seen red 2026-10-10 with "cannot" written "can not": the two sentences side by side; and
    /// again the same day against the corrected spec with the old words put back here: "the app
    /// says the spec's words" failed, the spec's sentence holding "Reports, bans and mutes about
    /// them are kept" and the app's not.
    #[test]
    fn the_confirm_says_what_it_does_and_does_not_do() {
        let spec = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/design/blocking-and-safe-mode.md")).expect("the spec");
        let ten_i = &spec[spec.find("## 10i.").expect("10i")..spec.find("## 10j.").expect("10j")];
        let flat = ten_i.split_whitespace().collect::<Vec<_>>().join(" ");
        let start = flat.find("\"This deletes everything").expect("the confirm's sentence in 10i") + 1;
        let end = start + flat[start..].find("(ban them too for that).\"").expect("its end") + "(ban them too for that).".len();
        let quoted = flat[start..end].replace("<name>", "Dana");
        assert!(quoted.contains("Reports, bans and mutes about them are kept"), "the corrected sentence: {quoted}");
        assert_eq!(confirm_words("Dana"), quoted, "the app says the spec's words");
    }
}
