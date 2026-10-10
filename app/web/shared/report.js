// ── report.js ─────────────────────────────────────────────────────────────
// Reports the admins can check (step D, 2026-10-09,
// docs/design/blocking-and-safe-mode.md section 8 and 10e): the words a report
// signs, the evidence items, the words the Report dialog and the Reports view
// show, and the dialog's help outside this server (10e-ii). Shared by the web chat client (web/chat/crypto.js builds and signs
// a report with it, web/chat/chat-reports.js draws the dialog and the view) and
// by Node, where scripts/tests/report-web.test.js holds it to the relay.
//
// A report names the person by identity key, never by name, and is signed by
// the reporter's Dilithium3 key over
//   hum/report/v1\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}
// where evidence_hash is the lowercase hex BLAKE3 of the `evidence` array's
// JSON exactly as sent (compact, as JSON.stringify writes it; the relay hashes
// the text of the `evidence` value as it received it) and reporter is the
// signed-in socket's key. It goes to the relay as
//   {"type":"report_v2","target","context","reason","note","evidence":[...],"ts","sig"}
// with sig in standard base64, as a DM's inner signature is.
//
// Evidence, chosen by the reporter:
//   {"kind":"dm","from","to","ts","text","sig"}   a DM's verified inner payload,
//       as the client keeps it after opening the seal; the relay checks the DM
//       signature against the reported person's key (from = them, to = me)
//   {"kind":"post","from","timestamp"}             a public post; the relay looks it up
//   {"kind":"group_text","from","ts","text"}       from a P2P group; never proven
// At most 20 items and 64 KB of evidence in all; a note of at most 500 characters.
// Never a message with a file: its text carries the key that opens the file
// (see REPORT_NO_FILES below).
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before crypto.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const REPORT_DOMAIN = 'hum/report/v1';
  // Where the report was made. Must match the relay's list.
  const REPORT_CONTEXTS = ['dm', 'post', 'group', 'profile'];
  // What an admin or mod can decide. Must match the relay's list.
  const REPORT_DECISIONS = ['dismiss', 'warn', 'mute', 'kick', 'ban', 'delete_post'];
  const REPORT_DECISION_LABELS = Object.freeze({
    dismiss: 'Dismiss',
    warn: 'Warn',
    mute: 'Mute',
    kick: 'Kick',
    ban: 'Ban',
    delete_post: 'Delete the post',
  });
  const REPORT_CONTEXT_LABELS = Object.freeze({
    dm: 'Direct messages',
    post: 'A public post',
    group: 'A group',
    profile: 'Their profile',
  });
  const REPORT_MAX_ITEMS = 20;
  const REPORT_MAX_EVIDENCE_BYTES = 64 * 1024;
  const REPORT_NOTE_MAX = 500;
  // The reasons are data (10e): an ordered list of {id, label, help}.
  const REPORT_REASONS_URL = '/data/safety/report_reasons.json';

  // ── Words ──
  // The privacy explanation's sentence (10e), said to everyone.
  const REPORT_PRIVACY_SENTENCE = 'Any direct message you send carries your signature, so the person you sent it to can prove to others that you wrote it.';
  // The short confirmation when the relay answers report_received.
  const REPORT_RECEIVED_LINE = "Report sent. This server's admins and moderators will look at it. The person you reported is not told who reported them.";
  // What a checked signature does not prove (section 8.2), on the Reports view.
  const REPORT_SIGNATURE_LIMITS = 'A checked signature proves that the reported person wrote exactly those words and sent them to the reporter. It does not prove when: the time is the sender\'s own clock. And the reporter chose which messages to include, so messages that give context may be missing.';
  const REPORT_NOT_PROVEN = 'Not proven';
  const REPORT_GROUP_UNPROVEN = 'From a group: this server cannot check who wrote these words, so they are sent as you saw them and marked as not proven.';
  const REPORT_DM_EVIDENCE_HELP = 'Tick the messages to include. Each carries their signature, so the admins can check they wrote it and sent it to you. Nothing else in the conversation is sent.';

  // Messages with files are never evidence. A DM's encrypted-file marker
  // ([[hum:file:v1]], crypto.js FILE_MARKER) carries the key that opens the
  // file, and the signature covers the whole text, so it cannot be cut out: a
  // report would hand an admin the means to open what could be abuse imagery,
  // which belongs with official hotlines, not a server's volunteers. Any
  // version of the marker counts. The relay refuses such evidence too.
  const REPORT_FILE_MARKER_PREFIX = '[[hum:file:';
  const REPORT_NO_FILES = 'Messages with files cannot be included in a report.';
  function reportTextHasFile(text) {
    return typeof text === 'string' && text.includes(REPORT_FILE_MARKER_PREFIX);
  }

  /** The badge on a proven DM item (10e). */
  function reportCheckedBadge(name) {
    return 'Signature checked: sent by ' + name + ' to the reporter';
  }
  /** The badge on a post the server found: it holds the post, signed by its author. */
  function reportPostFoundBadge(name) {
    return 'Checked: posted by ' + name + ' on this server';
  }

  // An identity key as the relay and both clients write it: lowercase hex.
  const REPORT_KEY_RE = /^[0-9a-f]{16,8192}$/;
  // A reason id as the data file writes them.
  const REPORT_REASON_RE = /^[a-z0-9_]{1,64}$/;

  function reportKeyNorm(key) {
    if (typeof key !== 'string') return null;
    const k = key.trim().toLowerCase();
    return REPORT_KEY_RE.test(k) ? k : null;
  }

  /** The words the reporter signs (10e). */
  function reportPreimage(reporter, target, reason, evidenceHash, ts) {
    return `${REPORT_DOMAIN}\n${reporter}\n${target}\n${reason}\n${evidenceHash}\n${ts}`;
  }

  /** The evidence array as it is sent and hashed: compact JSON. */
  function reportEvidenceJson(evidence) {
    return JSON.stringify(evidence);
  }

  function utf8(s) { return new TextEncoder().encode(String(s)); }
  function toHex(bytes) {
    let hex = '';
    for (let i = 0; i < bytes.length; i++) hex += bytes[i].toString(16).padStart(2, '0');
    return hex;
  }
  function toB64(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
  }

  /** The note as it is sent: trimmed, at most 500 characters (Unicode characters, not bytes). */
  function reportNoteNorm(note) {
    const s = String(note == null ? '' : note).trim();
    const chars = Array.from(s);
    return chars.length > REPORT_NOTE_MAX ? chars.slice(0, REPORT_NOTE_MAX).join('') : s;
  }

  /**
   * A DM's verified inner payload as evidence: the fields the relay rebuilds
   * the DM signature from, unchanged. Null when it carries no signature, or
   * carries a file (REPORT_NO_FILES).
   */
  function dmEvidenceItem(m) {
    if (!m || typeof m.sig !== 'string' || !m.sig) return null;
    if (typeof m.from !== 'string' || typeof m.to !== 'string') return null;
    const text = String(m.text == null ? '' : m.text);
    if (reportTextHasFile(text)) return null;
    return { kind: 'dm', from: m.from, to: m.to, ts: Number(m.ts) || 0, text, sig: m.sig };
  }

  /** A public post as evidence: the relay finds it by its author and time. */
  function postEvidenceItem(from, timestamp) {
    return { kind: 'post', from: String(from), timestamp: Number(timestamp) || 0 };
  }

  /** Words seen in a P2P group as evidence: never proven. Null when they carry a file. */
  function groupEvidenceItem(from, ts, text) {
    const t = String(text == null ? '' : text);
    if (reportTextHasFile(t)) return null;
    return { kind: 'group_text', from: String(from), ts: Number(ts) || 0, text: t };
  }

  /** Why this evidence would be refused, or null when it is fine. */
  function reportEvidenceProblem(evidence) {
    if (!Array.isArray(evidence)) return 'The evidence is not a list.';
    if (evidence.length > REPORT_MAX_ITEMS) return `At most ${REPORT_MAX_ITEMS} items can be included. Untick some.`;
    for (const it of evidence) {
      if (!it || typeof it !== 'object') return 'An item of evidence is empty.';
      if (it.kind === 'dm') {
        if (!['from', 'to', 'text', 'sig'].every((k) => typeof it[k] === 'string') || typeof it.ts !== 'number') return 'A message is missing its signature.';
      } else if (it.kind === 'post') {
        if (typeof it.from !== 'string' || typeof it.timestamp !== 'number') return 'The post is not named.';
      } else if (it.kind === 'group_text') {
        if (typeof it.from !== 'string' || typeof it.text !== 'string' || typeof it.ts !== 'number') return 'The group message is not named.';
      } else {
        return 'An item of evidence is of a kind this server does not take.';
      }
      if (reportTextHasFile(it.text)) return REPORT_NO_FILES;
    }
    if (utf8(reportEvidenceJson(evidence)).length > REPORT_MAX_EVIDENCE_BYTES) {
      return 'The messages chosen are too long to send together (64 KB at most). Untick some.';
    }
    return null;
  }

  /**
   * Build the signed `report_v2` frame. `fields`: {reporter, target, context,
   * reason, note, evidence, ts}; `deps`: {blake3(bytes) -> bytes, sign(bytes)
   * -> bytes}, both possibly async. Returns {frame, preimage, evidenceJson,
   * evidenceHash}, or {error} saying why nothing can be sent.
   */
  async function buildReportFrame(fields, deps) {
    const f = fields || {};
    const reporter = reportKeyNorm(f.reporter);
    const target = reportKeyNorm(f.target);
    if (!reporter) return { error: 'Your identity is not ready yet. Try again in a moment.' };
    if (!target) return { error: 'This person cannot be reported from here: their key is not known.' };
    if (reporter === target) return { error: "You can't report yourself." };
    if (!REPORT_CONTEXTS.includes(f.context)) return { error: 'Where this report was made is not known.' };
    if (typeof f.reason !== 'string' || !REPORT_REASON_RE.test(f.reason)) return { error: 'Choose a reason first.' };
    const evidence = Array.isArray(f.evidence) ? f.evidence : [];
    const problem = reportEvidenceProblem(evidence);
    if (problem) return { error: problem };
    const ts = Number(f.ts);
    if (!Number.isSafeInteger(ts) || ts <= 0) return { error: 'The time is not known.' };
    if (!deps || typeof deps.blake3 !== 'function' || typeof deps.sign !== 'function') {
      return { error: 'Signing is not ready yet. Try again in a moment.' };
    }
    const evidenceJson = reportEvidenceJson(evidence);
    const digest = await deps.blake3(utf8(evidenceJson));
    if (!digest || digest.length !== 32) return { error: 'The evidence could not be hashed.' };
    const evidenceHash = toHex(digest);
    const preimage = reportPreimage(reporter, target, f.reason, evidenceHash, ts);
    const sig = await deps.sign(utf8(preimage));
    if (!sig || !sig.length) return { error: 'Your report could not be signed.' };
    const frame = {
      type: 'report_v2',
      target,
      context: f.context,
      reason: f.reason,
      note: reportNoteNorm(f.note),
      evidence,
      ts,
      sig: toB64(sig),
    };
    return { frame, preimage, evidenceJson, evidenceHash };
  }

  /**
   * The reasons from the data file: an ordered list of {id, label, help}. The
   * file may be the list itself or an object holding it (under `reasons`, or
   * the first list of reasons it has, beside notes such as `_purpose`).
   */
  function reportReasonsFrom(data) {
    let list = null;
    if (Array.isArray(data)) list = data;
    else if (data && typeof data === 'object') {
      if (Array.isArray(data.reasons)) list = data.reasons;
      else list = Object.values(data).find((v) => Array.isArray(v) && v.some((r) => r && typeof r.id === 'string')) || null;
    }
    if (!list) return [];
    return list
      .filter((r) => r && typeof r.id === 'string' && REPORT_REASON_RE.test(r.id) && typeof r.label === 'string')
      .map((r) => ({ id: r.id, label: r.label, help: typeof r.help === 'string' ? r.help : '' }));
  }

  /** A time the relay sent, as milliseconds (a value under 1e12 is read as seconds). */
  function reportMs(t) {
    const n = Number(t);
    if (!Number.isFinite(n) || n <= 0) return 0;
    return n < 1e12 ? n * 1000 : n;
  }

  /**
   * One report from the relay's `reports` frame, with the names it may use
   * read in one place. 10e names the fields (id, target key and name,
   * context, reason, note, evidence with `checked`, created time, state,
   * decision, and the reporter for admins only) but not their spellings, so
   * the plain and the _key/_at spellings are both read.
   */
  function reportItemView(item) {
    const it = item || {};
    let evidence = it.evidence;
    if (typeof evidence === 'string') { try { evidence = JSON.parse(evidence); } catch { evidence = []; } }
    if (!Array.isArray(evidence)) evidence = [];
    const dec = it.decision;
    const decObj = dec && typeof dec === 'object' ? dec : null;
    return {
      id: it.id,
      target: String(it.target || it.target_key || ''),
      targetName: String(it.target_name || ''),
      context: String(it.context || ''),
      reason: String(it.reason || ''),
      note: String(it.note || ''),
      created: reportMs(it.created_at != null ? it.created_at : (it.created != null ? it.created : it.ts)),
      state: String(it.state || (dec ? 'decided' : 'open')),
      decision: String((decObj ? decObj.decision : dec) || ''),
      decisionNote: String((decObj ? decObj.note : it.decision_note) || ''),
      reviewer: String((decObj ? (decObj.reviewer || decObj.reviewer_key) : (it.reviewer || it.reviewer_key)) || ''),
      reviewerName: String((decObj ? decObj.reviewer_name : it.reviewer_name) || ''),
      decided: reportMs(decObj ? (decObj.at || decObj.decided_at) : it.decided_at),
      // Only admins are sent who reported (10e); mods see "a member".
      reporter: String(it.reporter || it.reporter_key || ''),
      reporterName: String(it.reporter_name || ''),
      evidence: evidence.filter((e) => e && typeof e === 'object').map((e) => ({
        kind: String(e.kind || ''),
        from: String(e.from || ''),
        to: String(e.to || ''),
        ts: reportMs(e.kind === 'post' ? (e.timestamp != null ? e.timestamp : e.ts) : e.ts),
        text: String(e.text == null ? '' : e.text),
        checked: e.checked === true,
      })),
    };
  }

  /** The badge on one item of evidence: the checked wording or "Not proven". */
  function reportEvidenceBadge(ev, name) {
    if (!ev || ev.checked !== true) return { checked: false, text: REPORT_NOT_PROVEN };
    if (ev.kind === 'post') return { checked: true, text: reportPostFoundBadge(name) };
    return { checked: true, text: reportCheckedBadge(name) };
  }

  // ── Help outside this server (10e-ii, 2026-10-10) ──
  // When the reason is that a child or anyone may be in danger, the dialog
  // shows, under the reason's help, the emergency number and the official place
  // to report a child being exploited online, for a country the person picks.
  // The numbers are data, data/safety/outside_help.json, every entry backed by
  // the dated finding docs/reference/findings/2026-10-09-outside-help-lines.md;
  // nothing here types a number or the date it was checked.
  //
  // The country is the person's choice and stays on the device: never looked up
  // (no IP lookup, no location service), never sent with the report. The first
  // one shown is the last one picked here, else the region of the device's
  // language when that names a listed country, else "Another country".
  const OUTSIDE_HELP_URL = '/data/safety/outside_help.json';
  const OUTSIDE_HELP_REASONS = Object.freeze(['child_danger', 'someone_in_danger']);
  // "Another country" as a choice: lower case, so it can never be a listed
  // country's code (those are upper case), and it is kept when picked so the
  // person who chose it is not moved to their language's country next time.
  const OUTSIDE_HELP_OTHER = 'other';
  // Where the web keeps the last pick (localStorage; native keeps it in its config).
  const OUTSIDE_HELP_SAVED_KEY = 'humanity_outside_help_country';
  const OUTSIDE_HELP_TITLE = 'Help outside this server';
  const OUTSIDE_HELP_OTHER_LABEL = 'Another country';
  const OUTSIDE_HELP_FIND_HOTLINE = 'Find the hotline for your country';
  // Said before the child line's link: what that body takes reports of, in the
  // data file's own description of the field.
  const OUTSIDE_HELP_CHILD_LEAD = 'To report a child being sexually exploited or abused online:';
  const OUTSIDE_HELP_CODE_RE = /^[A-Z]{2}$/;
  const OUTSIDE_HELP_DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

  /** Is the block offered for this reason? Only the two danger reasons (10e-ii). */
  function outsideHelpShows(reason) {
    return OUTSIDE_HELP_REASONS.includes(reason);
  }

  // A link the dialog will open: only https, so a damaged or hostile data file
  // cannot put a javascript: or plain-http address behind a hotline's name.
  function outsideHelpLink(v) {
    if (!v || typeof v !== 'object') return null;
    if (typeof v.name !== 'string' || !v.name.trim()) return null;
    if (typeof v.url !== 'string' || !/^https:\/\/[^\s"'<>]+$/.test(v.url)) return null;
    return { name: v.name.trim(), url: v.url };
  }

  /**
   * The data file read into what the dialog needs, or null when it is not the
   * file 10e-ii describes. Entries that are not whole (no code, name or
   * number) are left out rather than shown half.
   */
  function outsideHelpFrom(data) {
    if (!data || typeof data !== 'object' || !Array.isArray(data.countries)) return null;
    const d = data.default && typeof data.default === 'object' ? data.default : null;
    if (!d || typeof d.emergency_text !== 'string' || !d.emergency_text.trim()) return null;
    const seen = new Set();
    const countries = [];
    for (const c of data.countries) {
      if (!c || typeof c !== 'object') continue;
      if (typeof c.code !== 'string' || !OUTSIDE_HELP_CODE_RE.test(c.code) || seen.has(c.code)) continue;
      if (typeof c.name !== 'string' || !c.name.trim()) continue;
      if (typeof c.emergency !== 'string' || !c.emergency.trim()) continue;
      seen.add(c.code);
      countries.push({
        code: c.code,
        name: c.name.trim(),
        emergency: c.emergency.trim(),
        also: (Array.isArray(c.also) ? c.also : [])
          .filter((a) => a && typeof a.number === 'string' && a.number.trim())
          .map((a) => ({ number: a.number.trim(), for: typeof a.for === 'string' ? a.for.trim() : '' })),
        child: outsideHelpLink(c.child_report),
        note: typeof c.note === 'string' ? c.note.trim() : '',
      });
    }
    return {
      researched: typeof data.researched === 'string' && OUTSIDE_HELP_DATE_RE.test(data.researched) ? data.researched : '',
      default: {
        emergencyText: d.emergency_text.trim(),
        child: outsideHelpLink(d.child_report),
        note: typeof d.note === 'string' ? d.note.trim() : '',
      },
      countries,
    };
  }

  /**
   * The country a language tag names (`navigator.language`, such as "en-GB"),
   * upper case, or null. BCP 47: language, then an optional extended language
   * (three letters) and script (four letters, such as zh-Hant-TW), then the
   * region; a numeric region such as es-419 names no one country.
   */
  function outsideHelpRegion(tag) {
    if (typeof tag !== 'string') return null;
    const parts = tag.trim().split(/[-_]/);
    for (let i = 1; i < parts.length; i++) {
      const p = parts[i];
      if (i === 1 && /^[A-Za-z]{3}$/.test(p)) continue;
      if (/^[A-Za-z]{4}$/.test(p)) continue;
      if (/^[A-Za-z]{2}$/.test(p)) return p.toUpperCase();
      break;
    }
    return null;
  }

  /**
   * Which country the block starts on (10e-ii): the saved pick when it is
   * still a choice (a listed code, or "Another country"), else the language
   * tag's region when it is listed, else "Another country".
   */
  function outsideHelpFirstCountry(help, saved, languageTag) {
    const listed = (code) => !!(help && help.countries.some((c) => c.code === code));
    if (saved === OUTSIDE_HELP_OTHER) return OUTSIDE_HELP_OTHER;
    if (typeof saved === 'string' && listed(saved.trim().toUpperCase())) return saved.trim().toUpperCase();
    const region = outsideHelpRegion(languageTag);
    if (region && listed(region)) return region;
    return OUTSIDE_HELP_OTHER;
  }

  /** The block's last line, with the date read from the file. */
  function outsideHelpDateLine(researched) {
    return `Numbers checked on ${researched}. If one is wrong, tell us.`;
  }

  /**
   * What the block shows for a reason and a chosen country, or null when the
   * reason does not offer it or the file is not loaded. `code` not listed
   * reads as "Another country".
   */
  function outsideHelpView(help, reason, code) {
    if (!outsideHelpShows(reason) || !help) return null;
    const entry = help.countries.find((c) => c.code === code) || null;
    const chosen = entry ? entry.code : OUTSIDE_HELP_OTHER;
    let child = null;
    if (entry && entry.child) child = { name: entry.child.name, url: entry.child.url, fallback: false };
    else if (help.default.child) {
      // A listed country with no body of its own gets the INHOPE directory,
      // named for what it does; "Another country" is the default entry itself,
      // so it shows the default's own name.
      child = entry
        ? { name: OUTSIDE_HELP_FIND_HOTLINE, url: help.default.child.url, fallback: true, title: help.default.child.name }
        : { name: help.default.child.name, url: help.default.child.url, fallback: true };
    }
    return {
      title: OUTSIDE_HELP_TITLE,
      choices: help.countries.map((c) => ({ code: c.code, name: c.name }))
        .concat([{ code: OUTSIDE_HELP_OTHER, name: OUTSIDE_HELP_OTHER_LABEL }]),
      code: chosen,
      name: entry ? entry.name : OUTSIDE_HELP_OTHER_LABEL,
      emergency: entry ? entry.emergency : null,
      emergencyText: entry ? '' : help.default.emergencyText,
      also: entry ? entry.also.map((a) => ({ ...a })) : [],
      child,
      childLead: OUTSIDE_HELP_CHILD_LEAD,
      // For someone in danger the child line is there too, smaller (10e-ii).
      childSmall: reason === 'someone_in_danger',
      note: entry ? entry.note : help.default.note,
      dateLine: help.researched ? outsideHelpDateLine(help.researched) : '',
    };
  }

  const api = {
    REPORT_DOMAIN, REPORT_CONTEXTS, REPORT_DECISIONS, REPORT_DECISION_LABELS, REPORT_CONTEXT_LABELS,
    REPORT_MAX_ITEMS, REPORT_MAX_EVIDENCE_BYTES, REPORT_NOTE_MAX, REPORT_REASONS_URL,
    REPORT_PRIVACY_SENTENCE, REPORT_RECEIVED_LINE, REPORT_SIGNATURE_LIMITS, REPORT_NOT_PROVEN,
    REPORT_GROUP_UNPROVEN, REPORT_DM_EVIDENCE_HELP, REPORT_FILE_MARKER_PREFIX, REPORT_NO_FILES,
    reportTextHasFile, reportCheckedBadge, reportPostFoundBadge, reportKeyNorm, reportPreimage, reportEvidenceJson,
    reportNoteNorm, dmEvidenceItem, postEvidenceItem, groupEvidenceItem, reportEvidenceProblem,
    buildReportFrame, reportReasonsFrom, reportItemView, reportEvidenceBadge, reportMs,
    OUTSIDE_HELP_URL, OUTSIDE_HELP_REASONS, OUTSIDE_HELP_OTHER, OUTSIDE_HELP_SAVED_KEY, OUTSIDE_HELP_TITLE,
    OUTSIDE_HELP_OTHER_LABEL, OUTSIDE_HELP_FIND_HOTLINE, OUTSIDE_HELP_CHILD_LEAD,
    outsideHelpShows, outsideHelpFrom, outsideHelpRegion, outsideHelpFirstCountry, outsideHelpDateLine,
    outsideHelpView,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
