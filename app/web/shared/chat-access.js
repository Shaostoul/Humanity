/**
 * Chat access levels on the website (2026-11-02, docs/design/chat-access.md): the twin of
 * crates/humanity-protocol/src/chat_access.rs, held to the same answers by
 * scripts/tests/chat-access-web.test.js.
 *
 * Every channel has a read level (anyone, members, verified) and a post level (members, verified,
 * mods: moderators and admins). The relay decides and enforces (src/relay/chat_access.rs); this
 * file only says the same rules so the chat can draw by them: the line above the box you type in,
 * whether the box is open, an unverified poster's links as plain words, and the words a member
 * signs to read a channel that is not open to everyone.
 *
 * Loaded by /chat (before app.js), /chat/read and /admin. window.HosChatAccess; module.exports
 * for node.
 */
(function () {
  'use strict';

  const READ_LEVELS = Object.freeze(['anyone', 'members', 'verified']);
  const POST_LEVELS = Object.freeze(['members', 'verified', 'mods']);
  /** The ranks a person can hold on one server, lowest first. */
  const RANKS = Object.freeze({ visitor: 0, member: 1, verified: 2, moderator: 3, admin: 4 });
  /** How many links a member below Verified may put in one line. */
  const LINKS_BELOW_VERIFIED = 1;
  /** The channel every server has: #global (the old #general, renamed 2026-11-02). The chat lands
   *  in it when the room it was in is gone; the Rust twin is MAIN_CHANNEL. */
  const MAIN_CHANNEL = 'global';

  /** A stored or sent word as a read level; a word that names none is the safe default. */
  function readLevel(word) {
    const w = String(word || '').trim();
    return READ_LEVELS.includes(w) ? w : 'members';
  }

  function postLevel(word) {
    const w = String(word || '').trim();
    return POST_LEVELS.includes(w) ? w : 'members';
  }

  /** The lowest rank that reads at a level. */
  function readRank(level) {
    return { anyone: RANKS.visitor, members: RANKS.member, verified: RANKS.verified }[readLevel(level)];
  }

  function postRank(level) {
    return { members: RANKS.member, verified: RANKS.verified, mods: RANKS.moderator }[postLevel(level)];
  }

  /** Nobody posts where they cannot read: a channel that reads at verified posts at verified or higher. */
  function normalizedPost(read, post) {
    return readLevel(read) === 'verified' && postLevel(post) === 'members' ? 'verified' : postLevel(post);
  }

  function mayRead(read, rank) {
    return rank >= readRank(read);
  }

  function mayPost(read, post, rank) {
    return mayRead(read, rank) && rank >= postRank(normalizedPost(read, post));
  }

  /**
   * The rank of someone with a chosen name on this server, from their role and that role's trust
   * level (0 for a role the server does not define): the built-in names first, then a role the
   * admins made counts as verified at trust 1 or more.
   */
  function rankForMember(role, trustLevel) {
    const r = String(role || '');
    if (r === 'admin' || r === 'owner') return RANKS.admin;
    if (r === 'mod' || r === 'moderator') return RANKS.moderator;
    if (r === 'verified' || r === 'donor') return RANKS.verified;
    if (Number(trustLevel) >= 1) return RANKS.verified;
    return RANKS.member;
  }

  /** How many links a line holds: each http:// or https://, in any letter case. */
  function linksIn(content) {
    const lower = String(content || '').toLowerCase();
    return (lower.match(/http:\/\//g) || []).length + (lower.match(/https:\/\//g) || []).length;
  }

  /** Whether a line carries an upload's address (/uploads/ and a name) or a private file's marker. */
  function carriesAttachment(content) {
    const lower = String(content || '').toLowerCase();
    return lower.includes('[[hum:file:') || /\/uploads\/\S/.test(lower);
  }

  /** Whether a line calls on everyone: @everyone or @here as a word of its own (moderators and admins only). */
  function massMention(content) {
    return /@(everyone|here)(?![A-Za-z0-9_-])/i.test(String(content || ''));
  }

  /**
   * Why a line from someone of `rank` would be refused in a channel with these levels, as the
   * relay's chat_refused reason ('post_level', 'mass_mention', 'too_many_links', 'attachment'), or null.
   */
  function postRefusal(read, post, rank, content) {
    if (!mayPost(read, post, rank)) return 'post_level';
    if (rank < RANKS.moderator && massMention(content)) return 'mass_mention';
    if (rank < RANKS.verified) {
      if (carriesAttachment(content)) return 'attachment';
      if (linksIn(content) > LINKS_BELOW_VERIFIED) return 'too_many_links';
    }
    return null;
  }

  /** Whether a poster of `rank` has their links drawn as links and pictures (verified and up). */
  function showsLinks(rank) {
    return rank >= RANKS.verified;
  }

  /** Only moderators and admins post: the read-only room the protected setup lists. */
  function staffOnly(channel) {
    return !!channel && postLevel(channel.post_level) === 'mods';
  }

  /** The words a member signs to read a channel's `what` (messages, pins, reactions). */
  function readPurpose(what, channel) {
    return 'chat_read\n' + what + '\n' + channel;
  }

  /**
   * The keys of the line above the box you type in: who can read the channel, then (marked for
   * the glossary) why you cannot post, or the one-link rule below verified, or null.
   */
  function audienceKeys(read, post, rank) {
    const reads = { anyone: 'chat.access_read_anyone', members: 'chat.access_read_members', verified: 'chat.access_read_verified' }[readLevel(read)];
    let more = null;
    if (!mayPost(read, post, rank)) {
      more = { mods: 'chat.access_post_mods', verified: 'chat.access_post_verified', members: 'chat.access_post_members' }[normalizedPost(read, post)];
    } else if (rank < RANKS.verified) {
      more = 'chat.access_one_link';
    }
    return { reads, more };
  }

  const api = {
    READ_LEVELS, POST_LEVELS, RANKS, LINKS_BELOW_VERIFIED, MAIN_CHANNEL,
    readLevel, postLevel, normalizedPost, mayRead, mayPost, rankForMember,
    linksIn, carriesAttachment, massMention, postRefusal, showsLinks, staffOnly, readPurpose, audienceKeys,
  };
  if (typeof window !== 'undefined') window.HosChatAccess = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
