// Child of reach_tests.rs (#[path], so `use super::*` reaches its helpers and, through them,
// everything reach.rs has): a report about a group reaches the group's creator (2026-10-10,
// docs/design/blocking-and-safe-mode.md 10j, its Proof list). The relay's whole part is one
// narrow exception to "who can reach me" for a `dm_put` carrying `group_report: true`, with a
// daily budget of its own, and nothing kept about who reported whom. A file of its own so the
// step B tests above stay readable.

use super::*;
use std::collections::BTreeMap;

/// `sender` sends `target` a report about a group (an ordinary sealed DM with `group_report:
/// true`), presenting `cert`: (did it land in the target's mailbox, what the sender heard back).
/// The rate limiter is reset first, as `try_reach` does: it has tests of its own.
fn report(st: &Arc<RelayState>, sender: &str, target: &str, cert: Option<&str>) -> (bool, Vec<String>) {
    report_after(st, sender, target, cert, true)
}

/// `report`, leaving the rate limiter as it is when `reset_limiter` is false.
fn report_after(st: &Arc<RelayState>, sender: &str, target: &str, cert: Option<&str>, reset_limiter: bool) -> (bool, Vec<String>) {
    let mut rx = st.broadcast_tx.subscribe();
    let before = st.db.mailbox_fetch(target, 0, 1_000).unwrap().len();
    block(async {
        if reset_limiter {
            st.rate_limits.write().await.remove(sender);
        }
        handle_dm_put(st, sender, target.to_string(), envelope(), cert.map(str::to_string), DmAsk::GroupReport).await;
    });
    (st.db.mailbox_fetch(target, 0, 1_000).unwrap().len() > before, heard(&mut rx, sender))
}

/// Group reports `sender` spent today.
fn reports_spent(st: &RelayState, sender: &str) -> u32 {
    st.reach.group_reports.lock().unwrap().get(sender).map_or(0, |(_, n)| *n)
}

/// Knocks `sender` spent today.
fn knocks_spent(st: &RelayState, sender: &str) -> u32 {
    block(st.dm_knocks.read()).get(sender).map_or(0, |(_, n)| *n)
}

/// Contact requests `sender` spent today.
fn contact_requests_spent(st: &RelayState, sender: &str) -> u32 {
    st.reach.contact_requests.lock().unwrap().get(sender).map_or(0, |(_, n)| *n)
}

/// What a sender refused at `to`'s message door hears: the refusal everyone gets.
fn refusal(to: &Person) -> Vec<String> {
    vec![format!("reach_refused message {}", to.key)]
}

/// A signed P2P group object about `gid` by `by`, with `payload` (the shapes of
/// storage/groups_p2p.rs), put into the relay as a client would publish it.
fn publish(st: &Arc<RelayState>, by: &Person, object_type: &str, gid: &str, payload: Vec<(ciborium::Value, ciborium::Value)>) {
    use crate::relay::core::object::ObjectBuilder;
    let object = ObjectBuilder::new(object_type)
        .reference(gid)
        .created_at(1009)
        .payload_cbor(&ciborium::Value::Map(payload))
        .unwrap()
        .sign(&by.keypair())
        .unwrap();
    st.db.put_signed_object(&object, None).unwrap();
}

/// `who` leaves group `gid`: their own signed remove of themselves, which any member may publish,
/// the creator included (storage/groups_p2p.rs `index_group_member`).
fn leave(st: &Arc<RelayState>, gid: &str, who: &Person) {
    use ciborium::Value;
    let me = who.keypair().public_key().to_vec();
    publish(st, who, "group_member_v1", gid, vec![
        (Value::Text("action".into()), Value::Text("remove".into())),
        (Value::Text("subject".into()), Value::Bytes(me.clone())),
    ]);
    assert!(!st.db.p2p_group_has_member(gid, &me).unwrap(), "precondition: left the group");
}

/// The creator disbands group `gid`.
fn disband(st: &Arc<RelayState>, gid: &str, creator: &Person) {
    publish(st, creator, "group_disband_v1", gid, vec![]);
    assert!(st.db.p2p_groups_for_member(&creator.keypair().public_key()).unwrap().iter().all(|(g, _)| g != gid), "precondition: disbanded");
}

/// A report about a group reaches the group's creator from a member of it who holds no pass: at
/// the default (`friends`) and under `chosen`, there even from a member whose pass from the
/// creator leaves messages out. Nothing is said back to the reporter. Each spends one of the
/// day's group reports and no knock or contact request. Under `groups` and `anyone`, where a
/// member would get in on a knock, it spends a group report instead (its own budget, 10j).
///
/// Seen red 2026-10-10 with `dm_gate` never applying the exception (an `&& false` after its
/// `created_a_group_with` call): "at the default (friends)", left (false, ["reach_refused message
/// <the creator's key>"]), right (true, []).
#[test]
fn a_group_report_reaches_the_groups_creator_under_friends_and_chosen() {
    let st = fresh_state("group_report_through");
    let (creator, reporter, other) = (person(191), person(192), person(193));
    for p in [&creator, &reporter, &other] {
        connect(&st, &p.key);
    }
    group(&st, &creator, &[&reporter, &other], "Allotment");
    assert_eq!(
        try_reach(&st, Kind::Message, &reporter.key, &creator.key, None),
        (false, refusal(&creator)),
        "precondition: the same DM without the flag is refused"
    );

    assert_eq!(report(&st, &reporter.key, &creator.key, None), (true, vec![]), "at the default (friends)");
    set(&st, &creator.key, Kind::Message, Audience::Chosen);
    assert_eq!(report(&st, &reporter.key, &creator.key, None), (true, vec![]), "under chosen");
    let calls_only = pass(&st, &creator, &reporter.key, &["call"]);
    assert!(
        !try_reach(&st, Kind::Message, &reporter.key, &creator.key, Some(&calls_only)).0,
        "precondition: under chosen, a pass that leaves messages out does not let a message in"
    );
    assert_eq!(
        report(&st, &reporter.key, &creator.key, Some(&calls_only)),
        (true, vec![]),
        "under chosen, from a member whose pass leaves messages out"
    );
    assert_eq!(reports_spent(&st, &reporter.key), 3, "each spent one of the day's group reports");
    assert_eq!(knocks_spent(&st, &reporter.key), 0, "and no knock");
    assert_eq!(contact_requests_spent(&st, &reporter.key), 0, "and no contact request");

    for audience in [Audience::Groups, Audience::Anyone] {
        set(&st, &creator.key, Kind::Message, audience);
        assert_eq!(report(&st, &other.key, &creator.key, None), (true, vec![]), "under {audience:?}");
    }
    assert_eq!(
        (reports_spent(&st, &other.key), knocks_spent(&st, &other.key)),
        (2, 0),
        "where a member would get in on a knock, a group report is spent instead"
    );
}

/// `nobody` means nobody, a group report included: a creator who takes messages from nobody is
/// not reached by a member of their group, who hears the refusal everyone gets, nor by one who
/// also holds a pass allowing everything (as any DM under `nobody`). Nothing is spent.
///
/// Seen red 2026-10-10 with the `nobody` check taken out of `dm_gate`'s group-report exception:
/// "under nobody", left (true, []), right (false, ["reach_refused message <the creator's key>"]).
#[test]
fn nobody_refuses_a_group_report() {
    let st = fresh_state("group_report_nobody");
    let (creator, reporter) = (person(194), person(195));
    connect(&st, &creator.key);
    connect(&st, &reporter.key);
    group(&st, &creator, &[&reporter], "Choir");
    set(&st, &creator.key, Kind::Message, Audience::Nobody);
    assert_eq!(report(&st, &reporter.key, &creator.key, None), (false, refusal(&creator)), "under nobody");
    let everything = pass(&st, &creator, &reporter.key, &["call", "invite", "message", "trade", "voice_message"]);
    assert_eq!(
        report(&st, &reporter.key, &creator.key, Some(&everything)),
        (false, refusal(&creator)),
        "under nobody, holding a pass that allows everything"
    );
    assert_eq!(reports_spent(&st, &reporter.key), 0, "a refused report spends nothing");
}

/// The exception reaches only the CREATOR of a group the sender and the recipient are BOTH
/// active members of NOW. Each of these is refused as the same DM without the flag would be, with
/// the refusal everyone gets and nothing spent: a report to someone who is merely another member
/// of the group; to someone who created a group the reporter is not in, while the two share a
/// group someone else created; to the creator after the reporter left the group; to a creator who
/// left their own group; and to the creator of a group that was disbanded. Each of the last three
/// is first seen reaching the creator, so it is the leaving that refuses it.
///
/// Seen red 2026-10-10 five times, each left (true, []): with `created_a_group_with` asking only
/// whether the two share a group (`share_a_group`), "to someone who is merely another member";
/// with it asking only whether the recipient created a group they are in (the reporter's
/// membership not checked), "to someone who created a group the reporter is not in, and shares
/// another group with them"; and with each condition of the storage query
/// (`p2p_group_created_by_with`) dropped in turn: the member's `active = 1`, "after the reporter
/// left the group"; the creator's `active = 1`, "after the creator left their own group";
/// `disbanded = 0`, "after the group was disbanded".
#[test]
fn a_group_report_only_reaches_the_creator_of_a_group_both_are_in_now() {
    let st = fresh_state("group_report_not_creator");
    let (creator, reporter, member, host, founder, guest) = (person(201), person(202), person(203), person(204), person(205), person(206));
    for p in [&creator, &reporter, &member, &host, &founder, &guest] {
        connect(&st, &p.key);
    }
    let allotment = group(&st, &creator, &[&reporter, &member], "Allotment");
    let book_club = group(&st, &host, &[&guest], "Book club");
    let street = group(&st, &founder, &[&host, &reporter], "Street");

    assert_eq!(report(&st, &reporter.key, &member.key, None), (false, refusal(&member)), "to someone who is merely another member");
    assert_eq!(
        report(&st, &reporter.key, &host.key, None),
        (false, refusal(&host)),
        "to someone who created a group the reporter is not in, and shares another group with them"
    );

    assert_eq!(report(&st, &reporter.key, &creator.key, None), (true, vec![]), "precondition: reaches the creator while both are in");
    leave(&st, &allotment, &reporter);
    assert_eq!(report(&st, &reporter.key, &creator.key, None), (false, refusal(&creator)), "after the reporter left the group");

    assert_eq!(report(&st, &host.key, &founder.key, None), (true, vec![]), "precondition: reaches the founder while both are in");
    leave(&st, &street, &founder);
    assert_eq!(report(&st, &host.key, &founder.key, None), (false, refusal(&founder)), "after the creator left their own group");

    assert_eq!(report(&st, &guest.key, &host.key, None), (true, vec![]), "precondition: reaches the host while the group stands");
    disband(&st, &book_club, &host);
    assert_eq!(report(&st, &guest.key, &host.key, None), (false, refusal(&host)), "after the group was disbanded");

    assert_eq!(
        [reports_spent(&st, &reporter.key), reports_spent(&st, &host.key), reports_spent(&st, &guest.key)],
        [1, 1, 1],
        "only the reports that arrived were spent"
    );
}

/// Three group reports a day per sender, separate from the 20 knocks and the 5 contact requests:
/// a sender who spent every knock and every contact request still has three, a report the rate
/// limiter slows down spends none (it is paid for after the limiter), the fourth is refused with
/// a reason and nothing delivered, another sender has their own three, and the knocks and
/// contact requests were not touched.
///
/// Seen red 2026-10-10 with the budget check reading `entry.1 > limit` (one too many): "the fourth
/// is refused", left (true, []). (The same break turned the contact-request budget test red, as
/// both budgets share `spend_daily`.)
#[test]
fn group_reports_are_three_a_day_per_sender_and_separate_from_knocks_and_contact_requests() {
    let st = fresh_state("group_report_budget");
    let (creator, reporter, second, open, asked) = (person(211), person(212), person(213), person(214), person(215));
    for p in [&creator, &reporter, &second, &open, &asked] {
        connect(&st, &p.key);
    }
    group(&st, &creator, &[&reporter, &second], "Workshop");

    // The reporter spends today's knocks on someone who takes anyone's mail, and today's contact
    // requests on someone else.
    set(&st, &open.key, Kind::Message, Audience::Anyone);
    for _ in 0..DM_KNOCKS_PER_DAY {
        assert!(try_reach(&st, Kind::Message, &reporter.key, &open.key, None).0);
    }
    assert!(!try_reach(&st, Kind::Message, &reporter.key, &open.key, None).0, "precondition: the knocks are spent");
    for _ in 0..CONTACT_REQUESTS_PER_DAY {
        block(async {
            st.rate_limits.write().await.remove(&reporter.key);
            handle_dm_put(&st, &reporter.key, asked.key.clone(), envelope(), None, DmAsk::ContactRequest).await;
        });
    }
    assert_eq!(contact_requests_spent(&st, &reporter.key), CONTACT_REQUESTS_PER_DAY, "precondition: the contact requests are spent");

    let landed = || st.db.mailbox_fetch(&creator.key, 0, 100).unwrap().len() as u32;
    assert!(report(&st, &reporter.key, &creator.key, None).0, "a sender whose knocks and contact requests are spent still has group reports");
    let (delivered, back) = report_after(&st, &reporter.key, &creator.key, None, false);
    assert!(!delivered && back.iter().any(|m| m.contains("Slow down")), "precondition: slowed down: {back:?}");
    for _ in 1..GROUP_REPORTS_PER_DAY {
        assert!(report(&st, &reporter.key, &creator.key, None).0);
    }
    assert_eq!(landed(), GROUP_REPORTS_PER_DAY, "three a day, the slowed-down one not counted");
    assert_eq!(
        report(&st, &reporter.key, &creator.key, None),
        (false, vec![format!("You have sent today's {GROUP_REPORTS_PER_DAY} reports to group creators. You can send more tomorrow.")]),
        "the fourth is refused"
    );
    assert_eq!(landed(), GROUP_REPORTS_PER_DAY, "and not delivered");
    assert!(report(&st, &second.key, &creator.key, None).0, "another sender has their own three");
    assert_eq!(
        (knocks_spent(&st, &reporter.key), contact_requests_spent(&st, &reporter.key)),
        (DM_KNOCKS_PER_DAY, CONTACT_REQUESTS_PER_DAY),
        "the knocks and contact requests were not touched"
    );
}

/// Anything else carrying the flag is treated as the same DM without it (10j). To a friend it
/// arrives on their pass, free, as a friend's DM does: a friend who is also in the creator's group
/// sends five that day and spends no group report, and a friend who shares no group is let in too.
/// To someone who takes anyone's mail and created no group with the sender, it arrives on a knock,
/// as a stranger's DM does. To someone at the default who created no group with the sender, it is
/// refused with the refusal everyone gets.
///
/// Seen red 2026-10-10 with `dm_gate` applying the exception before the ordinary gate (so a friend
/// paid for their reports from the group-report budget): "a friend's report number 4", left
/// (false, ["You have sent today's 3 reports to group creators. You can send more tomorrow."]),
/// right (true, []).
#[test]
fn a_group_report_to_a_friend_works_as_any_dm_does() {
    let st = fresh_state("group_report_as_dm");
    let (creator, friend, plain, open, stranger) = (person(221), person(222), person(223), person(224), person(225));
    for p in [&creator, &friend, &plain, &open, &stranger] {
        connect(&st, &p.key);
    }
    group(&st, &creator, &[&friend], "Garden");
    let friends_pass = pass(&st, &creator, &friend.key, &DEFAULT_MAY);
    for n in 1..=GROUP_REPORTS_PER_DAY + 2 {
        assert_eq!(report(&st, &friend.key, &creator.key, Some(&friends_pass)), (true, vec![]), "a friend's report number {n}");
    }
    assert_eq!(reports_spent(&st, &friend.key), 0, "a friend's reports are free, as their DMs are");

    let plains_pass = pass(&st, &plain, &stranger.key, &DEFAULT_MAY);
    assert_eq!(report(&st, &stranger.key, &plain.key, Some(&plains_pass)), (true, vec![]), "to a friend who shares no group with them");
    set(&st, &open.key, Kind::Message, Audience::Anyone);
    assert_eq!(report(&st, &stranger.key, &open.key, None), (true, vec![]), "to someone who takes anyone's mail");
    assert_eq!(
        (knocks_spent(&st, &stranger.key), reports_spent(&st, &stranger.key)),
        (1, 0),
        "a knock, as a stranger's DM spends, and no group report"
    );
    assert_eq!(report(&st, &stranger.key, &creator.key, None), (false, refusal(&creator)), "to someone at the default who created no group with them");
}

/// Every row of every table, as text, by table name: the database as a test can compare it. The
/// mailbox's own row counter (its `sqlite_sequence` row, AUTOINCREMENT) is left out, as it moves
/// with every mail and says nothing about who sent it.
fn every_table(st: &RelayState) -> BTreeMap<String, Vec<Vec<String>>> {
    let conn = st.db.conn.lock().unwrap();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut out = BTreeMap::new();
    for name in names {
        let sql = if name == "sqlite_sequence" {
            "SELECT * FROM sqlite_sequence WHERE name <> 'dm_mailbox'".to_string()
        } else {
            format!("SELECT * FROM \"{name}\"")
        };
        let mut stmt = conn.prepare(&sql).unwrap();
        let columns = stmt.column_count();
        // Text as itself (so a test can look for a key or an envelope in it), anything else as
        // its debug form.
        let cell = |v: rusqlite::types::Value| match v {
            rusqlite::types::Value::Text(s) => s,
            other => format!("{other:?}"),
        };
        let rows: Vec<Vec<String>> = stmt
            .query_map([], |row| (0..columns).map(|i| row.get::<_, rusqlite::types::Value>(i).map(cell)).collect())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        out.insert(name, rows);
    }
    out
}

/// The relay stores nothing about who reported whom (10j). A group report changes no table but
/// the mailbox, which gains exactly one row: the sealed envelope, addressed to the creator, with
/// nothing in it naming the reporter. The only thing kept in memory is the reporter's own count
/// for the day, under their own key, with nothing naming the creator.
///
/// Seen red 2026-10-10 with the group-report arm of `pay` also saving a row for the sender (a
/// `set_reach_settings` call, standing in for a budget kept on disk): "no table but the mailbox
/// changed", the `reach_settings` table gained the reporter's row.
#[test]
fn a_group_report_leaves_nothing_behind_but_the_sealed_mail() {
    let st = fresh_state("group_report_nothing_stored");
    let (creator, reporter) = (person(231), person(232));
    connect(&st, &creator.key);
    connect(&st, &reporter.key);
    group(&st, &creator, &[&reporter], "Quilters");

    let mut before = every_table(&st);
    assert_eq!(report(&st, &reporter.key, &creator.key, None), (true, vec![]), "precondition: the report arrives");
    let mut after = every_table(&st);
    let mail_before = before.remove("dm_mailbox").expect("the mailbox table");
    let mail_after = after.remove("dm_mailbox").expect("the mailbox table");
    assert_eq!(after, before, "no table but the mailbox changed");
    let new: Vec<&Vec<String>> = mail_after.iter().filter(|row| !mail_before.contains(row)).collect();
    assert_eq!(new.len(), 1, "the mailbox gained exactly one row: {new:?}");
    assert_eq!(mail_after.len(), mail_before.len() + 1, "and lost none");
    let row = new[0].join(" ");
    assert!(row.contains(&creator.key) && row.contains(&envelope()), "the sealed envelope, addressed to the creator");
    assert!(!row.contains(&reporter.key), "nothing in it names the reporter");

    let counts = st.reach.group_reports.lock().unwrap();
    assert_eq!(counts.len(), 1, "one count kept in memory");
    assert_eq!(counts.get(&reporter.key).map(|(_, n)| *n), Some(1), "the reporter's own count for the day, under their own key");
}
