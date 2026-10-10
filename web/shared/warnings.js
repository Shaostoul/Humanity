// ── warnings.js ───────────────────────────────────────────────────────────
// Warnings on messages, and the recovery-phrase guard (step F, 2026-10-10,
// docs/design/blocking-and-safe-mode.md 6.3 and 10g): the matcher, the words
// on screen, and the rule that holds a stranger's links. Shared by the web chat
// client (web/chat/chat-warnings.js draws the warnings, the link line and the
// Safety switch, and runs the guard before every send) and by Node, where
// scripts/tests/warnings-web.test.js runs this matcher over the cases the
// desktop app's matcher also runs (scripts/tests/fixtures/warning-cases.json).
//
// Matching, identical on both clients (10g): lower-case a text, turn every run
// of characters that are not letters or digits into one space, and trim; do the
// same to each phrase. A text matches an entry when " " + text + " " contains
// " " + phrase + " " for any phrase in its `any`. So "I'm an admin!!" matches
// "i'm an admin" (both become "i m an admin"), and "admin" never matches inside
// "administer". Letters and digits are Unicode's, the same on both clients:
// the JavaScript class \p{Alphabetic}\p{N} is Rust's char::is_alphanumeric.
//
// The guard uses the same normalising: a text that holds 4 or more of my
// recovery phrase's words in a row, in the phrase's order, is never sent.
//
// The patterns are data, /data/safety/warnings.json, read by both clients.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before the chat scripts.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The patterns (10g): {about, warnings: [{id, applies_to, title, explain, advice, any}]}.
  const WARNINGS_URL = '/data/safety/warnings.json';
  // Who an entry is shown for: `friends` is a mutual follow (I follow them and
  // they follow me), everyone else is `strangers`.
  const WARNING_AUDIENCES = ['strangers', 'friends'];
  // The shortest run of my phrase's words, in order, that stops a send.
  const PHRASE_RUN_MIN = 4;

  // ── Words on screen ──
  // What a stopped send says (10g). There is no "send anyway".
  const PHRASE_GUARD_SENTENCE = 'This is your recovery phrase. Anyone who has it owns your identity and everything in it. Nobody legitimate will ever ask for it. Remove it to send the rest.';
  const WARNINGS_SWITCH_LABEL = 'Warnings on messages';
  const WARNINGS_SWITCH_HELP = 'Explains common tricks, such as asking for money or for your recovery phrase, under direct and group messages from people who are not your friends. Some show on friends\' messages too, because an account can be taken over. A warning never blocks a message and never reports anything.';
  // Section 6.5: said on screen in Settings > Safety, not only in docs.
  const WARNINGS_ON_DEVICE_SENTENCE = 'Messages are end-to-end encrypted. Nobody, including server admins, can read them to look for danger. Warnings are checked on this device only.';
  const WARNING_GOT_IT = 'Got it';
  const LINK_OPEN = 'Open';

  /** The line under a stranger's direct message that holds a link (10g). */
  function strangerLinkLine(name) {
    return `${name} is not your friend. Links open only when you choose.`;
  }

  // ── Matching ──
  // Every run of characters that are not a letter or a digit, exactly as the
  // desktop app's Rust `char::is_alphanumeric` decides: the Alphabetic property
  // or a numeric category. (Not \p{L}: that splits a word at a combining mark
  // such as a Devanagari vowel sign, which Rust keeps in the word; the shared
  // cases hold one that tells the two apart. 10g, corrected 2026-10-10.)
  const NOT_LETTER_OR_DIGIT = /[^\p{Alphabetic}\p{N}]+/gu;

  /** A text as both clients compare it: lower-cased, runs of anything else one space, trimmed. */
  function warningNormalize(text) {
    return String(text == null ? '' : text).toLowerCase().replace(NOT_LETTER_OR_DIGIT, ' ').trim();
  }

  /** `stranger` / `friend` (as the shared cases write them) or the plural, as the file writes it; null when neither. */
  function warningAudience(who) {
    if (who === 'stranger' || who === 'strangers') return 'strangers';
    if (who === 'friend' || who === 'friends') return 'friends';
    return null;
  }

  /**
   * The entries from the data file, in its order, each phrase normalised.
   * An entry missing its words, or with an audience this client does not know,
   * is left out; so is a phrase that normalises to nothing (it would match
   * every text).
   */
  function warningsFrom(data) {
    const list = Array.isArray(data) ? data : (data && Array.isArray(data.warnings) ? data.warnings : null);
    if (!list) return [];
    const out = [];
    for (const w of list) {
      if (!w || typeof w.id !== 'string' || !w.id) continue;
      if (typeof w.title !== 'string' || typeof w.explain !== 'string' || typeof w.advice !== 'string') continue;
      if (!Array.isArray(w.applies_to) || !Array.isArray(w.any)) continue;
      const appliesTo = w.applies_to.map(warningAudience).filter(Boolean);
      if (!appliesTo.length) continue;
      const phrases = [];
      for (const p of w.any) {
        if (typeof p !== 'string') continue;
        const n = warningNormalize(p);
        if (n && !phrases.includes(n)) phrases.push(n);
      }
      if (!phrases.length) continue;
      out.push({ id: w.id, applies_to: appliesTo, title: w.title, explain: w.explain, advice: w.advice, any: phrases });
    }
    return out;
  }

  /** Does this (normalised) phrase occur in this (normalised) text as whole words? */
  function containsWords(normText, normPhrase) {
    return !!normPhrase && (' ' + normText + ' ').includes(' ' + normPhrase + ' ');
  }

  /**
   * The entries a message from `who` (strangers or friends) matches, in the
   * file's order, each once. `list` is warningsFrom()'s output.
   */
  function warningMatches(text, who, list) {
    const audience = warningAudience(who);
    if (!audience || !Array.isArray(list)) return [];
    const norm = warningNormalize(text);
    if (!norm) return [];
    return list.filter((w) => w.applies_to.includes(audience) && w.any.some((p) => containsWords(norm, p)));
  }

  /**
   * Does `text` hold `min` (4) or more of the phrase's words in a row, in the
   * phrase's order? `phrase` is the words (an array) or the phrase as one
   * string; both are normalised the way messages are.
   */
  function phraseRunFound(text, phrase, min) {
    const need = Number.isInteger(min) && min > 0 ? min : PHRASE_RUN_MIN;
    const words = (Array.isArray(phrase) ? phrase.map(warningNormalize).join(' ') : warningNormalize(phrase))
      .split(' ').filter(Boolean);
    if (words.length < need) return false;
    // Number-only tokens are left out (2026-10-10, as on the desktop): a phrase pasted as a
    // numbered list, the way backup screens show it ("1. word 2. word"), has a number between
    // every pair of words, so without this no four of its words stood next to each other.
    const norm = warningNormalize(text).split(' ').filter((w) => w && !/^\p{N}+$/u.test(w)).join(' ');
    if (!norm) return false;
    for (let i = 0; i + need <= words.length; i++) {
      if (containsWords(norm, words.slice(i, i + need).join(' '))) return true;
    }
    return false;
  }

  // ── A stranger's links (10g) ──
  // What the chat turns into something that opens: a web address, or a file
  // on this server (app.js formatBody).
  const LINK_RE = /https?:\/\/[^\s<]|\/uploads\/[^\s<]/i;

  /** Does a message's text hold a link? */
  function messageHasLink(text) {
    return typeof text === 'string' && LINK_RE.test(text);
  }

  /**
   * A message body's HTML with every link held: `href` becomes
   * `data-held-href` (and an audio or video player's `src`, `data-held-src`),
   * so nothing opens or loads until the reader presses Open, which puts back
   * the HTML as it was.
   */
  function holdLinksHtml(html) {
    return String(html == null ? '' : html)
      .replace(/<a\b([^>]*?)\shref=/gi, '<a$1 data-held-href=')
      .replace(/<(audio|video)\b([^>]*?)\ssrc=/gi, '<$1$2 data-held-src=');
  }

  const api = {
    WARNINGS_URL, WARNING_AUDIENCES, PHRASE_RUN_MIN,
    PHRASE_GUARD_SENTENCE, WARNINGS_SWITCH_LABEL, WARNINGS_SWITCH_HELP, WARNINGS_ON_DEVICE_SENTENCE,
    WARNING_GOT_IT, LINK_OPEN,
    strangerLinkLine, warningNormalize, warningAudience, warningsFrom, warningMatches, phraseRunFound,
    messageHasLink, holdLinksHtml,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
