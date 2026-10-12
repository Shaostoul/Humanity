// ── files-in-messages.js ──────────────────────────────────────────────────
// "Files in messages": the person's own settings for files sent in direct
// messages and groups (2026-10-11, docs/design/media-showcase.md, "Files on
// the sender's device and in messages, exact", part 3; the operator's
// answer 6). Shared by the web chat (chat-privacy.js draws the settings in
// Settings > Safety beside "Who can reach me", chat-dms.js applies them to
// every file received, chat-messages.js reads the attach button setting and
// tells a sender what the other person accepts, chat-profile.js publishes
// `accepts_files`) and by Node, where scripts/tests/private-files-web.test.js
// holds it to the spec.
//
// The three settings, kept on this device (localStorage, in try/catch):
//   Accept files from:                   Nobody / Friends / Anyone I talk to (default Friends)
//   Show pictures without asking from:   Friends / Nobody (default Friends; the protected
//                                        setup is always Nobody)
//   Show the attach button in private conversations: on or off (default on)
//
// The RECEIVING app enforces them: a file from someone the setting does not
// allow is never downloaded until its Accept is pressed. The server cannot
// enforce them and must not try: with sealed sender it cannot tell which DM
// carries a file, and being able to would tell it who sends files to whom.
// "Accept files from" is published in the person's profile (`accepts_files`:
// "nobody", "friends" or "anyone"), so a sender's app can say so first.
//
// "Friends" is the click-to-load rule's friend (chat-dms.js
// privateFileFromFriend): a mutual follow I gave a pass. "Anyone I talk to" is
// everyone whose message reached me: who may message me at all is "Who can
// reach me", which the server enforces.
//
// A classic script in the browser (HosFilesInMessages lands on window), a
// CommonJS module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const FILES_SETTINGS_KEY = 'hos_files_in_messages';
  const ACCEPT_CHOICES = Object.freeze(['nobody', 'friends', 'anyone']);
  const PICTURE_CHOICES = Object.freeze(['friends', 'nobody']);
  const DEFAULTS = Object.freeze({ accept: 'friends', pictures: 'friends', attach: true });

  const WORDS = Object.freeze({
    group: 'Files in messages',
    intro: 'Files sent to you in direct messages and groups. This device decides what it downloads: the server cannot see which messages carry a file.',
    accept: 'Accept files from',
    acceptChoices: Object.freeze({ nobody: 'Nobody', friends: 'Friends', anyone: 'Anyone I talk to' }),
    acceptExplain: Object.freeze({
      nobody: 'No file sent to you is downloaded until you press Accept on it, a friend\'s included. You can still send files.',
      friends: 'Files from your friends open as usual. Anyone else\'s waits until you press Accept on it.',
      anyone: 'Files from anyone who can message you open as usual.',
    }),
    published: 'People who send you a file are told who you accept files from, so they know before they send.',
    pictures: 'Show pictures without asking from',
    pictureChoices: Object.freeze({ friends: 'Friends', nobody: 'Nobody' }),
    picturesExplain: Object.freeze({
      friends: 'A friend\'s picture shows at once. Anyone else\'s waits for a click.',
      nobody: 'Every picture waits for a click, a friend\'s too.',
    }),
    picturesProtected: 'With the protected setup on, every picture waits for a click.',
    attach: 'Show the attach button in private conversations',
    attachOffExplain: 'The attach button is hidden in direct messages and groups, and files pasted or dropped there are not sent. You still receive files as set above.',
    attachOff: 'Sending files in direct messages and groups is turned off in Safety, under Files in messages.',
    accepted: 'Accept',
  });

  /** The settings from what was kept: anything missing or not one of the choices is the default. */
  function settingsFrom(raw) {
    let v = raw;
    if (typeof raw === 'string') {
      try { v = JSON.parse(raw); } catch (e) { v = null; }
    }
    const src = v && typeof v === 'object' ? v : {};
    return {
      accept: ACCEPT_CHOICES.includes(src.accept) ? src.accept : DEFAULTS.accept,
      pictures: PICTURE_CHOICES.includes(src.pictures) ? src.pictures : DEFAULTS.pictures,
      attach: typeof src.attach === 'boolean' ? src.attach : DEFAULTS.attach,
    };
  }

  /** The settings this device keeps (the defaults when nothing is kept or storage cannot be read). */
  function readSettings(storage) {
    let raw = null;
    try { raw = storage ? storage.getItem(FILES_SETTINGS_KEY) : null; } catch (e) { raw = null; }
    return settingsFrom(raw);
  }

  /** Has the person ever chosen here (so the profile carries their choice even with nothing else in it)? */
  function settingsChosen(storage) {
    try { return !!(storage && storage.getItem(FILES_SETTINGS_KEY)); } catch (e) { return false; }
  }

  /** Keep the settings on this device. Returns the settings kept, or null when storage refused. */
  function writeSettings(storage, settings) {
    const s = settingsFrom(settings);
    try {
      if (!storage) return null;
      storage.setItem(FILES_SETTINGS_KEY, JSON.stringify(s));
      return s;
    } catch (e) {
      return null;
    }
  }

  /** Is `next` more open than `prev` for "Accept files from" (which the protected setup's PIN guards)? */
  function acceptLooser(prev, next) {
    return ACCEPT_CHOICES.indexOf(next) > ACCEPT_CHOICES.indexOf(prev);
  }

  /** Is a file from this sender downloaded without asking? `who`: {isMe, isFriend}. */
  function acceptsFrom(settings, who) {
    const s = settingsFrom(settings);
    const w = who || {};
    if (w.isMe) return true;
    if (s.accept === 'anyone') return true;
    if (s.accept === 'friends') return !!w.isFriend;
    return false;
  }

  /** Is a picture from this sender shown at once, without a click? Mine always are. */
  function picturesWithoutAsking(settings, who, protectedOn) {
    const s = settingsFrom(settings);
    const w = who || {};
    if (w.isMe) return true;
    if (protectedOn) return false;
    return s.pictures === 'friends' && !!w.isFriend;
  }

  /** The card's words for a file that was not downloaded: "Ann sent a file. You accept files only from friends. Accept this one?" */
  function askWords(name, settings) {
    const s = settingsFrom(settings);
    const who = String(name || '').trim() || 'Someone';
    const rule = s.accept === 'nobody' ? 'You do not accept files in messages.' : 'You accept files only from friends.';
    return `${who} sent a file. ${rule} Accept this one?`;
  }

  /** What the profile publishes: "nobody", "friends" or "anyone". */
  function profileValue(settings) {
    return settingsFrom(settings).accept;
  }

  /** A profile's `accepts_files` as one of the three words, or null when it says nothing usable. */
  function acceptsFilesOf(value) {
    return ACCEPT_CHOICES.includes(value) ? value : null;
  }

  /**
   * The sender's line in a DM's attach step, from the other person's published `accepts_files`
   * and whether they count me a friend (they gave me a pass, and we follow each other): "Ann
   * accepts files only from friends." Null when the file will open for them as usual, or when
   * their choice is not known.
   */
  function senderLine(name, theirValue, iAmTheirFriend) {
    const v = acceptsFilesOf(theirValue);
    const who = String(name || '').trim() || 'They';
    if (v === 'nobody') return `${who} does not accept files in messages. Their app will ask them before it downloads this one.`;
    if (v === 'friends' && !iAmTheirFriend) return `${who} accepts files only from friends. Their app will ask them before it downloads this one.`;
    return null;
  }

  const api = {
    FILES_SETTINGS_KEY, ACCEPT_CHOICES, PICTURE_CHOICES, DEFAULTS, WORDS,
    settingsFrom, readSettings, settingsChosen, writeSettings, acceptLooser, acceptsFrom,
    picturesWithoutAsking, askWords, profileValue, acceptsFilesOf, senderLine,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosFilesInMessages = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
