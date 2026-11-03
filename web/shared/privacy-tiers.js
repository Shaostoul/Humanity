// ── privacy-tiers.js ──────────────────────────────────────────────────────
// The privacy levels on the website (2026-11-03, BUG-237): one place for what a level sets and
// how a choice is kept, used by the chat's first-connect chooser (/chat/chat-privacy.js) and by
// Settings > Privacy (/pages/settings-privacy.js). The desktop twin is src/gui/pages/privacy.rs.
//
// A level (data/gui/privacy_tiers.json, the file the desktop reads too) presets two switches:
//  - presence (`hide_presence`): told to a server as {"type":"privacy_update","hide_presence":b}
//    on a signed-in socket (src/relay/relay.rs RelayMessage::PrivacyUpdate), which keeps it for
//    this account on that server (storage/members.rs set_hide_presence);
//  - the member directory (`directory_unlisted`): privacy.directory = "listed" or "unlisted" in
//    the profile the chat sends (chat-profile.js pushProfileToRelay), recorded as changed in this
//    browser (/shared/profile-changes.js) so it goes on top of the server's own copy. Only the
//    word "listed" lists (BUG-241, 2026-11-03): a profile without it, someone who never chose, is
//    left out of the directory, so the box draws unticked until they tick it.
// Kept in this browser: the level last used ('humanity_privacy_tier') and the presence switch
// ('humanity_hide_presence', "1" hidden, "0" shown). The flags are per server, so the chat tells
// each server it connects to (reassertFrame). The directory choice lives in the profile
// ('humanity_profile').
//
// A level's `name`, `tagline` and `description` are keys in data/i18n/en.json: the cards read
// the same in both apps. A level whose key is missing, or a file that cannot be read, gives the
// one Private level instead (fail-private, as the desktop's load_tiers).
//
// A classic script in the browser (window.HosPrivacyTiers) and a CommonJS module under Node,
// where scripts/tests/privacy-web.test.js drives it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const TIER_KEY = 'humanity_privacy_tier';
  const PRESENCE_KEY = 'humanity_hide_presence';
  const PROFILE_KEY = 'humanity_profile';
  const TIERS_URL = '/data/gui/privacy_tiers.json';
  const WORD_FIELDS = ['name', 'tagline', 'description'];
  // The directory words, the relay's DIRECTORY_LISTED and DIRECTORY_UNLISTED
  // (crates/humanity-protocol/src/profile_copy.rs; /shared/profile-copy.js has the same, and
  // scripts/tests/privacy-web.test.js holds the three to one spelling). Copied, not read from
  // profile-copy.js: Settings loads this file without it.
  const LISTED = 'listed';
  const UNLISTED = 'unlisted';

  /** The one level used when the file cannot be read or is refused: the most private. */
  function fallback() {
    return {
      defaultTier: 'private',
      tiers: [{
        id: 'private',
        name: 'privacy.tier_private_name',
        tagline: 'privacy.tier_private_tagline',
        description: 'privacy.tier_private_description',
        hide_presence: true,
        directory_unlisted: true,
      }],
    };
  }

  /**
   * The levels in a parsed privacy_tiers.json, or null when it is refused: no levels, a level
   * without an id, a word that is not a key (`hasKey(key)`, i18n.has in the browser), or a
   * default that is not one of its levels.
   */
  function parseTiers(data, hasKey) {
    if (!data || !Array.isArray(data.tiers) || !data.tiers.length) return null;
    for (const t of data.tiers) {
      if (!t || typeof t.id !== 'string' || !t.id) return null;
      for (const f of WORD_FIELDS) {
        if (typeof t[f] !== 'string' || (hasKey && !hasKey(t[f]))) return null;
      }
    }
    if (!data.tiers.some((t) => t.id === data.default_tier)) return null;
    return { defaultTier: data.default_tier, tiers: data.tiers };
  }

  /** Fetch and parse the levels; the fallback on any failure. */
  async function loadTiers(fetchFn, hasKey) {
    try {
      const res = await fetchFn(TIERS_URL);
      if (res && res.ok !== false) {
        const parsed = parseTiers(await res.json(), hasKey);
        if (parsed) return parsed;
      }
    } catch (e) { /* the fallback below */ }
    return fallback();
  }

  function read(storage, key) {
    try { return storage ? storage.getItem(key) : null; } catch (e) { return null; }
  }
  function write(storage, key, value) {
    try { storage.setItem(key, value); return true; } catch (e) { return false; }
  }
  function tierById(tiers, id) {
    return (tiers || []).find((t) => t && t.id === id) || null;
  }

  /** The id of the level last used in this browser, or '' when none was chosen. */
  function chosenTierId(storage) {
    return read(storage, TIER_KEY) || '';
  }

  /** Whether this browser last chose to hide presence: true, false, or null when it never chose. */
  function presenceHidden(storage) {
    const v = read(storage, PRESENCE_KEY);
    if (v === '1') return true;
    if (v === '0') return false;
    return null;
  }

  /** The frame that tells a server the presence choice. */
  function presenceFrame(hidden) {
    return { type: 'privacy_update', hide_presence: !!hidden };
  }

  /** The frame a fresh connection sends (the flags are per server), or null when never chosen. */
  function reassertFrame(storage) {
    const hidden = presenceHidden(storage);
    return hidden === null ? null : presenceFrame(hidden);
  }

  /** Keep the presence choice; returns the frame to send. */
  function setPresence(storage, hidden) {
    write(storage, PRESENCE_KEY, hidden ? '1' : '0');
    return presenceFrame(hidden);
  }

  function storedProfile(storage) {
    try {
      const p = JSON.parse(read(storage, PROFILE_KEY) || '{}');
      return p && typeof p === 'object' && !Array.isArray(p) ? p : {};
    } catch (e) { return {}; }
  }

  /**
   * Whether this browser's profile lists the person in the member directory: its privacy map says
   * "listed". Never chosen (no word, as every profile saved before 2026-11-03 is) is not listed,
   * which is what every server does with it (BUG-241).
   */
  function directoryListed(storage) {
    const p = storedProfile(storage);
    return !!(p.privacy && typeof p.privacy === 'object' && p.privacy.directory === LISTED);
  }

  /**
   * Keep the directory choice in this browser's profile, recorded as changed (`changes`, the
   * HosProfileChanges module) so the chat sends it on top of the server's copy: the word "listed"
   * or "unlisted", always written, so the server can tell a choice from never choosing. Every
   * other field of the stored profile is kept. True when the choice changed; nothing is written
   * when it did not.
   */
  function setDirectory(storage, listed, changes) {
    const before = storedProfile(storage);
    const after = JSON.parse(JSON.stringify(before));
    after.privacy = after.privacy && typeof after.privacy === 'object' && !Array.isArray(after.privacy) ? after.privacy : {};
    after.privacy.directory = listed ? LISTED : UNLISTED;
    const changed = changes ? changes.changedFields(before, after) : [];
    if (!changed.length && directoryListed(storage) === !!listed) return false;
    if (changes) changes.add(storage, changed);
    write(storage, PROFILE_KEY, JSON.stringify(after));
    return true;
  }

  /**
   * Use level `tier`: keep it as the level last used, set both switches from it, and return
   * the presence frame to send (the directory goes with the profile, from the chat).
   */
  function applyTier(storage, tier, changes) {
    write(storage, TIER_KEY, tier.id);
    setDirectory(storage, !tier.directory_unlisted, changes);
    return setPresence(storage, !!tier.hide_presence);
  }

  /** Whether the two switches are what `tier` sets them to. */
  function switchesMatch(tier, hidden, listed) {
    return !!tier && !!tier.hide_presence === !!hidden && !tier.directory_unlisted === !!listed;
  }

  // The server's answer to privacy_update (src/relay/relay.rs, the PrivacyUpdate arm): a private
  // notice starting with one of these. scripts/tests/privacy-web.test.js reads them back there.
  const ANSWER_HIDDEN = 'Presence hidden:';
  const ANSWER_SHOWN = 'Presence visible:';

  /** What a server's frame says about presence: 'hidden', 'shown', or null for anything else. */
  function presenceAnswer(msg) {
    if (!msg || msg.type !== 'private' || typeof msg.message !== 'string') return null;
    if (msg.message.startsWith(ANSWER_HIDDEN)) return 'hidden';
    if (msg.message.startsWith(ANSWER_SHOWN)) return 'shown';
    return null;
  }

  const api = {
    TIER_KEY, PRESENCE_KEY, PROFILE_KEY, TIERS_URL, ANSWER_HIDDEN, ANSWER_SHOWN, LISTED, UNLISTED,
    fallback, parseTiers, loadTiers, tierById, chosenTierId, presenceHidden, presenceFrame,
    reassertFrame, setPresence, directoryListed, setDirectory, applyTier, switchesMatch,
    presenceAnswer,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosPrivacyTiers = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
