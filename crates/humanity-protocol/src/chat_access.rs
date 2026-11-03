//! Chat access levels (docs/design/chat-access.md, 2026-11-02): who can read and who can post in
//! each channel of a server.
//!
//! What the relay and the desktop app must agree on: the words a level is sent as, the ranks a
//! person can hold on a server, the two answers (may this rank read a channel, may it post there),
//! the rules a line from a member below Verified meets (one link, no attachment), and the words a
//! member signs to read a channel that is not open to everyone. The relay decides; the apps ask
//! the same functions to draw the line under the box you type in and to draw an unverified
//! poster's links as plain words.
//!
//! The website's twin is `web/shared/chat-access.js`; `scripts/tests/chat-access-web.test.js`
//! holds it to the same answers as the tests here.

/// What a person is on one server, lowest first. Worked out by the relay from the key's role and
/// whether it chose a name there (`src/relay/chat_access.rs` `rank_of`); an app works it out the
/// same way from the member list (`rank_for_member`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Rank {
    /// No key, a key with no chosen name on this server, or a banned key.
    #[default]
    Visitor,
    /// A chosen name on this server, not verified.
    Member,
    /// Verified (or a Donor, or a role the admins made with a trust level of 1 or more).
    Verified,
    /// A moderator.
    Moderator,
    /// An admin, or the server's owner.
    Admin,
}

/// Who can read a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReadLevel {
    /// Everyone, without signing up: a visitor to the website, any app, a search engine.
    Anyone,
    /// Anyone with a chosen name on this server. A new channel's level (safe by default).
    #[default]
    Members,
    /// Verified members, moderators and admins.
    Verified,
}

/// Who can post in a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PostLevel {
    /// Anyone with a chosen name on this server. A new channel's level.
    #[default]
    Members,
    /// Verified members, moderators and admins.
    Verified,
    /// Moderators and admins only (what a channel's `read_only` flag meant before 2026-11-02).
    Mods,
}

impl ReadLevel {
    /// Every read level, widest first (the order the pickers list them in).
    pub const ALL: [ReadLevel; 3] = [ReadLevel::Anyone, ReadLevel::Members, ReadLevel::Verified];

    /// The word this level is stored and sent as.
    pub fn as_str(self) -> &'static str {
        match self {
            ReadLevel::Anyone => "anyone",
            ReadLevel::Members => "members",
            ReadLevel::Verified => "verified",
        }
    }

    /// The level a stored or sent word names, or None for a word that names none.
    pub fn parse(word: &str) -> Option<ReadLevel> {
        ReadLevel::ALL.into_iter().find(|l| l.as_str() == word.trim())
    }

    /// The level a stored word names; a word that names none reads as Members, the safe default.
    pub fn from_stored(word: &str) -> ReadLevel {
        ReadLevel::parse(word).unwrap_or_default()
    }

    /// The lowest rank that reads at this level.
    pub fn lowest_rank(self) -> Rank {
        match self {
            ReadLevel::Anyone => Rank::Visitor,
            ReadLevel::Members => Rank::Member,
            ReadLevel::Verified => Rank::Verified,
        }
    }

    /// Whether someone of `rank` may read a channel at this level.
    pub fn admits(self, rank: Rank) -> bool {
        rank >= self.lowest_rank()
    }
}

impl PostLevel {
    /// Every post level, widest first.
    pub const ALL: [PostLevel; 3] = [PostLevel::Members, PostLevel::Verified, PostLevel::Mods];

    /// The word this level is stored and sent as.
    pub fn as_str(self) -> &'static str {
        match self {
            PostLevel::Members => "members",
            PostLevel::Verified => "verified",
            PostLevel::Mods => "mods",
        }
    }

    /// The level a stored or sent word names, or None for a word that names none.
    pub fn parse(word: &str) -> Option<PostLevel> {
        PostLevel::ALL.into_iter().find(|l| l.as_str() == word.trim())
    }

    /// The level a stored word names; a word that names none reads as Members.
    pub fn from_stored(word: &str) -> PostLevel {
        PostLevel::parse(word).unwrap_or_default()
    }

    /// The lowest rank that posts at this level.
    pub fn lowest_rank(self) -> Rank {
        match self {
            PostLevel::Members => Rank::Member,
            PostLevel::Verified => Rank::Verified,
            PostLevel::Mods => Rank::Moderator,
        }
    }

    /// Whether someone of `rank` may post at this level (reading aside: see [`may_post`]).
    pub fn admits(self, rank: Rank) -> bool {
        rank >= self.lowest_rank()
    }

    /// Only moderators and admins post: the "read-only room" the protected setup lists and the
    /// sidebar marks with its eye.
    pub fn staff_only(self) -> bool {
        self == PostLevel::Mods
    }
}

/// The post level a channel really has: never below what it takes to read it, since nobody may
/// post where they cannot read. A channel that reads at Verified posts at Verified or higher.
pub fn normalized_post(read: ReadLevel, post: PostLevel) -> PostLevel {
    if read == ReadLevel::Verified && post == PostLevel::Members {
        PostLevel::Verified
    } else {
        post
    }
}

/// Whether someone of `rank` may post in a channel with these levels: they may read it, and the
/// (normalized) post level admits them.
pub fn may_post(read: ReadLevel, post: PostLevel, rank: Rank) -> bool {
    read.admits(rank) && normalized_post(read, post).admits(rank)
}

/// The rank of someone who chose a name on this server, from their role as stored (`user_roles`)
/// and that role's trust level (`roles.trust_level`; 0 for a role the server does not define).
/// The built-in names decide first; a role the admins made is Verified at trust 1 or more, the
/// built-in Verified role's own trust.
pub fn rank_for_member(role: &str, trust_level: i64) -> Rank {
    match role {
        "admin" | "owner" => Rank::Admin,
        "mod" | "moderator" => Rank::Moderator,
        "verified" | "donor" => Rank::Verified,
        _ if trust_level >= 1 => Rank::Verified,
        _ => Rank::Member,
    }
}

/// How many links a member below Verified may put in one line (the operator: "share 1 URL at a
/// time").
pub const LINKS_BELOW_VERIFIED: usize = 1;

/// How many links a line holds: each `http://` or `https://` in it, in any letter case, which is
/// what both apps turn into a link.
pub fn links_in(content: &str) -> usize {
    let lower = content.to_ascii_lowercase();
    lower.matches("http://").count() + lower.matches("https://").count()
}

/// Whether a line carries an attachment: the address of a file uploaded to a server (`/uploads/`
/// followed by a name) or a private file's marker (`[[hum:file:`). The apps draw either as a file
/// or a picture, which only Verified members and above may post.
pub fn carries_attachment(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    if lower.contains("[[hum:file:") {
        return true;
    }
    lower.match_indices("/uploads/").any(|(at, word)| {
        lower[at + word.len()..].chars().next().is_some_and(|c| !c.is_whitespace())
    })
}

/// Whether a line calls on everyone: `@everyone` or `@here` (any letter case) as a word of its own,
/// so `@hereford` is a name and not a call. Reserved for moderators and admins (the operator, 2026-11-02:
/// "I feel like everyone should be reserved for mods and admins.").
pub fn mass_mention(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    ["@everyone", "@here"].iter().any(|word| {
        lower.match_indices(word).any(|(at, w)| {
            lower[at + w.len()..].chars().next().map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        })
    })
}

/// Why a line is not posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostRefusal {
    /// The poster may not read the channel at all.
    CannotRead(ReadLevel),
    /// The channel's post level is above the poster's rank (the normalized level).
    PostLevel(PostLevel),
    /// `@everyone` or `@here` from someone below Moderator.
    MassMention,
    /// More links than [`LINKS_BELOW_VERIFIED`] from a member below Verified.
    TooManyLinks { links: usize },
    /// An upload's address or a file marker from a member below Verified.
    Attachment,
}

impl PostRefusal {
    /// The `reason` word the relay's `chat_refused` carries, so the sender's own copy is marked
    /// "Not sent" with the right words.
    pub fn reason(self) -> &'static str {
        match self {
            PostRefusal::CannotRead(_) | PostRefusal::PostLevel(_) => "post_level",
            PostRefusal::MassMention => "mass_mention",
            PostRefusal::TooManyLinks { .. } => "too_many_links",
            PostRefusal::Attachment => "attachment",
        }
    }
}

/// Whether a line with `content`, from someone of `rank`, may be posted in a channel with these
/// levels, or why not. The relay asks this for every channel line after the slash commands.
pub fn post_refusal(read: ReadLevel, post: PostLevel, rank: Rank, content: &str) -> Option<PostRefusal> {
    if !read.admits(rank) {
        return Some(PostRefusal::CannotRead(read));
    }
    let post = normalized_post(read, post);
    if !post.admits(rank) {
        return Some(PostRefusal::PostLevel(post));
    }
    if rank < Rank::Moderator && mass_mention(content) {
        return Some(PostRefusal::MassMention);
    }
    if rank < Rank::Verified {
        if carries_attachment(content) {
            return Some(PostRefusal::Attachment);
        }
        let links = links_in(content);
        if links > LINKS_BELOW_VERIFIED {
            return Some(PostRefusal::TooManyLinks { links });
        }
    }
    None
}

/// Whether a poster of `rank` has their links drawn as links and their picture and file
/// addresses as pictures and files (Verified and above), or all of it as plain words (below).
pub fn shows_links(rank: Rank) -> bool {
    rank >= Rank::Verified
}

/// What a member signs to read a channel's `what` (`messages`, `pins` or `reactions`) through the
/// POST routes: these words, then `\n` and the time in milliseconds, the shape every signed
/// request has (the relay's `verify_dilithium_signature`).
pub fn read_purpose(what: &str, channel: &str) -> String {
    format!("chat_read\n{what}\n{channel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_level_reads_and_writes_as_its_word() {
        for l in ReadLevel::ALL {
            assert_eq!(ReadLevel::parse(l.as_str()), Some(l));
        }
        for l in PostLevel::ALL {
            assert_eq!(PostLevel::parse(l.as_str()), Some(l));
        }
        assert_eq!(ReadLevel::parse("public"), None);
        assert_eq!(ReadLevel::from_stored("nonsense"), ReadLevel::Members, "an unknown word is the safe default");
        assert_eq!(PostLevel::from_stored(""), PostLevel::Members);
    }

    #[test]
    fn who_reads_what() {
        assert!(ReadLevel::Anyone.admits(Rank::Visitor));
        assert!(!ReadLevel::Members.admits(Rank::Visitor));
        assert!(ReadLevel::Members.admits(Rank::Member));
        assert!(!ReadLevel::Verified.admits(Rank::Member));
        assert!(ReadLevel::Verified.admits(Rank::Verified));
        assert!(ReadLevel::Verified.admits(Rank::Admin));
    }

    #[test]
    fn nobody_posts_where_they_cannot_read() {
        assert_eq!(normalized_post(ReadLevel::Verified, PostLevel::Members), PostLevel::Verified);
        assert_eq!(normalized_post(ReadLevel::Anyone, PostLevel::Members), PostLevel::Members);
        assert!(!may_post(ReadLevel::Verified, PostLevel::Members, Rank::Member));
        assert!(may_post(ReadLevel::Anyone, PostLevel::Members, Rank::Member));
        assert!(!may_post(ReadLevel::Anyone, PostLevel::Members, Rank::Visitor), "a visitor never posts");
        assert!(!may_post(ReadLevel::Anyone, PostLevel::Mods, Rank::Verified));
        assert!(may_post(ReadLevel::Anyone, PostLevel::Mods, Rank::Moderator));
    }

    #[test]
    fn a_role_gives_its_rank() {
        assert_eq!(rank_for_member("", 0), Rank::Member);
        assert_eq!(rank_for_member("member", 0), Rank::Member);
        assert_eq!(rank_for_member("unverified", 0), Rank::Member);
        assert_eq!(rank_for_member("verified", 1), Rank::Verified);
        assert_eq!(rank_for_member("donor", 2), Rank::Verified);
        assert_eq!(rank_for_member("family", 1), Rank::Verified, "a role the admins made, at Verified's trust");
        assert_eq!(rank_for_member("guest", 0), Rank::Member);
        assert_eq!(rank_for_member("mod", 3), Rank::Moderator);
        assert_eq!(rank_for_member("owner", 0), Rank::Admin);
    }

    #[test]
    fn links_and_attachments_are_counted_as_the_apps_draw_them() {
        assert_eq!(links_in("see https://a.example and HTTP://b.example"), 2);
        assert_eq!(links_in("no link here, just www words"), 0);
        assert!(carries_attachment("look /uploads/abc.png"));
        assert!(carries_attachment("[[hum:file:v1 {}]]"));
        assert!(!carries_attachment("the /uploads/ folder"), "the bare folder word names no file");
        assert!(!carries_attachment("https://a.example/page"));
    }

    #[test]
    fn a_member_below_verified_posts_one_link_and_no_file() {
        let (r, p) = (ReadLevel::Anyone, PostLevel::Members);
        assert_eq!(post_refusal(r, p, Rank::Member, "one https://a.example"), None);
        assert_eq!(
            post_refusal(r, p, Rank::Member, "https://a.example https://b.example"),
            Some(PostRefusal::TooManyLinks { links: 2 })
        );
        assert_eq!(post_refusal(r, p, Rank::Member, "/uploads/x.png"), Some(PostRefusal::Attachment));
        assert_eq!(post_refusal(r, p, Rank::Verified, "https://a.example https://b.example /uploads/x.png"), None);
        assert_eq!(post_refusal(r, PostLevel::Mods, Rank::Member, "hi"), Some(PostRefusal::PostLevel(PostLevel::Mods)));
        assert_eq!(
            post_refusal(ReadLevel::Verified, PostLevel::Members, Rank::Member, "hi"),
            Some(PostRefusal::CannotRead(ReadLevel::Verified))
        );
        assert_eq!(PostRefusal::Attachment.reason(), "attachment");
    }

    #[test]
    fn the_read_purpose_names_what_and_where() {
        assert_eq!(read_purpose("messages", "general"), "chat_read\nmessages\ngeneral");
    }

    fn rank(word: &str) -> Rank {
        match word {
            "visitor" => Rank::Visitor,
            "member" => Rank::Member,
            "verified" => Rank::Verified,
            "moderator" => Rank::Moderator,
            "admin" => Rank::Admin,
            other => panic!("no rank {other}"),
        }
    }

    /// THE SHARED FIXTURE: every row of scripts/tests/fixtures/chat-access-v1.json, the answers the
    /// website's twin (web/shared/chat-access.js) is held to as well (chat-access-web.test.js).
    ///
    /// The donor row is at trust 0 on purpose: the relay passes 0 for every built-in name
    /// (relay/chat_access.rs `rank_of`), so the name alone must make a Donor Verified.
    #[test]
    fn the_shared_fixture_holds() {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/chat-access-v1.json");
        let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is there")).expect("JSON");
        for r in fx["roles"].as_array().unwrap() {
            let (role, trust, want) = (r[0].as_str().unwrap(), r[1].as_i64().unwrap(), r[2].as_str().unwrap());
            assert_eq!(rank_for_member(role, trust), rank(want), "role {role:?} (trust {trust})");
        }
        for r in fx["reads"].as_array().unwrap() {
            let level = ReadLevel::parse(r[0].as_str().unwrap()).unwrap();
            assert_eq!(level.admits(rank(r[1].as_str().unwrap())), r[2].as_bool().unwrap(), "read {r}");
        }
        for r in fx["normalized"].as_array().unwrap() {
            let (read, post) = (ReadLevel::parse(r[0].as_str().unwrap()).unwrap(), PostLevel::parse(r[1].as_str().unwrap()).unwrap());
            assert_eq!(normalized_post(read, post).as_str(), r[2].as_str().unwrap(), "normalized {r}");
        }
        for r in fx["links"].as_array().unwrap() {
            assert_eq!(links_in(r[0].as_str().unwrap()), r[1].as_u64().unwrap() as usize, "links in {:?}", r[0]);
        }
        for r in fx["attachments"].as_array().unwrap() {
            assert_eq!(carries_attachment(r[0].as_str().unwrap()), r[1].as_bool().unwrap(), "attachment in {:?}", r[0]);
        }
        for r in fx["mass_mentions"].as_array().unwrap() {
            assert_eq!(mass_mention(r[0].as_str().unwrap()), r[1].as_bool().unwrap(), "a call on everyone in {:?}", r[0]);
        }
        for p in fx["posts"].as_array().unwrap() {
            let read = ReadLevel::parse(p["read"].as_str().unwrap()).unwrap();
            let post = PostLevel::parse(p["post"].as_str().unwrap()).unwrap();
            let got = post_refusal(read, post, rank(p["rank"].as_str().unwrap()), p["content"].as_str().unwrap()).map(|r| r.reason());
            assert_eq!(got, p["refusal"].as_str(), "post {p}");
        }
        let purpose = fx["purpose"].as_array().unwrap();
        assert_eq!(read_purpose(purpose[0].as_str().unwrap(), purpose[1].as_str().unwrap()), purpose[2].as_str().unwrap());
    }
}
