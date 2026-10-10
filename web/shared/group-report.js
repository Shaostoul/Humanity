// ── group-report.js ───────────────────────────────────────────────────────
// A report about a group reaches the group's creator (10j, 2026-10-10,
// docs/design/blocking-and-safe-mode.md): the words, the marker, the limits,
// the choices the Report dialog offers for a group message, and the creator's
// checks. Shared by the web chat client (web/chat/chat-reports.js draws the
// dialog and the Safety section, web/chat/crypto.js seals the report,
// web/chat/chat-groups-p2p.js reads the creator's own copy of the group) and
// by Node, where scripts/tests/group-report-web.test.js holds it to the spec.
//
// Why a group's creator: a peer-to-peer group's messages are encrypted for the
// group, so a server's admins can neither read them nor check who wrote them,
// and they cannot remove anyone from the group. The creator holds the group's
// messages in their own copy and is the one who admits and removes members.
//
// To the creator, a report is an ordinary signed, sealed v2 DM (so the server
// sees nothing of it), deposited with `"group_report": true` on the `dm_put`,
// whose text is
//   [[hum:group-report:v1]]{"group_id","group_name","target","reason","note","items":[{"id","from","ts","text"}]}
// where each item names a group message by its signed-object id and repeats
// its sender, time and text as the reporter's copy shows them. At most 20
// items and 16 KB of text; never a message with a file (step D's rule,
// reportTextHasFile in /shared/report.js). The DM's own signature says who
// sent it, so the reporter is named to the creator, and the dialog says so.
//
// The creator's client checks each item against its own copy of the group: an
// item is found when an object with that id is there, signed (the signature
// checked here, over the object's own bytes, whose hash is the id) by the
// item's sender, at the item's time, holding the item's text. Anything else is
// "Not found in your copy", shown and never dropped. A report about a group
// the creator does not hold as its creator, or from someone not in it, is
// dropped.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load after /shared/report.js and before crypto.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // Step D's rules (the file rule, the key and reason forms, the note): one
  // source, so a report to the creator and one to the admins agree.
  const isNode = typeof module === 'object' && module && module.exports;
  // eslint-disable-next-line global-require
  const rep = isNode ? require('./report.js') : root;

  // The marker the text starts with. Must match the desktop app.
  const GROUP_REPORT_MARKER = '[[hum:group-report:v1]]';
  const GROUP_REPORT_MAX_ITEMS = 20;
  // Of inner text: the marker and the JSON, in UTF-8 bytes.
  const GROUP_REPORT_MAX_BYTES = 16 * 1024;
  // A group's id, and a group message's id: the BLAKE3 of the signed object.
  const GROUP_REPORT_ID_RE = /^[0-9a-f]{64}$/;

  // ── Words ──
  // Who the report goes to (10j), in the dialog's order. "creator" is the default.
  const GROUP_REPORT_DESTINATIONS = ['creator', 'admins', 'both'];
  const GROUP_REPORT_DESTINATION_LABELS = Object.freeze({
    creator: "The group's creator",
    admins: "This server's admins",
    both: 'Both',
  });
  const GROUP_REPORT_SEND_TO = 'Send this report to';
  // The two cases where only the admins remain (10j's sentences).
  const GROUP_REPORT_YOU_CREATED = "You created this group: remove them from the group's member list.";
  const GROUP_REPORT_THEY_CREATED = "The person you are reporting created this group, so this goes to the server's admins.";
  // Not in 10j: the group's creator could not be found out here (the group's
  // own record did not load, or its signature did not check).
  const GROUP_REPORT_CREATOR_UNKNOWN = "Who created this group could not be found out here, so this goes to the server's admins.";
  const GROUP_REPORT_FINDING_CREATOR = 'Finding out who created this group...';
  // Said before sending whenever the creator is chosen (10j).
  const GROUP_REPORT_CREATOR_SEES = "The group's creator will see that you sent this.";
  // Under the group message, when the creator is chosen (beside step D's
  // REPORT_GROUP_UNPROVEN, which stays for the admins).
  const GROUP_REPORT_CREATOR_CHECKS = "The group's creator can check these words against their own copy of the group.";
  // The reporter's confirmation, and the line when the server did not let it through.
  const GROUP_REPORT_SENT_LINE = "Report sent to the group's creator.";
  const GROUP_REPORT_REFUSED_LINE = "Your report did not reach the group's creator: this server did not let it through. You can send it to this server's admins instead.";

  // The creator's side: Settings > Safety.
  const GROUP_REPORTS_TITLE = 'Reports about your groups';
  const GROUP_REPORTS_HELP = 'Members of a group you created can report one of its messages to you. Each report names who sent it. Reports stay on this device until you dismiss them, and are never sent to any server.';
  const GROUP_REPORTS_NONE = 'No reports about your groups.';
  const GROUP_REPORT_NOT_FOUND = 'Not found in your copy';
  const GROUP_REPORT_ACTION_LABELS = Object.freeze({
    remove: 'Remove them from the group',
    block: 'Block them',
    dismiss: 'Dismiss',
  });
  const GROUP_REPORT_REMOVED = 'Removed from the group.';
  const GROUP_REPORT_NOT_IN_GROUP = 'They are not in the group now.';
  const GROUP_REPORT_BLOCKED = 'You have blocked them.';
  const GROUP_REPORT_NO_ITEMS = 'No messages were included.';

  /** The badge on an item found in the creator's copy (10j). */
  function groupReportFoundBadge(name) {
    return 'Found in your copy of the group, signed by ' + name;
  }
  /** The line the creator is shown when a report arrives. */
  function groupReportArrivedLine(reporterName, targetName, groupName) {
    return `${reporterName} sent you a report about ${targetName} in ${groupName}. It is in Safety, under Reports about your groups.`;
  }
  /** The count on the group in the group list: its tooltip. */
  function groupReportCountTitle(n) {
    return n === 1
      ? '1 report about this group. See Reports about your groups in Safety.'
      : `${n} reports about this group. See Reports about your groups in Safety.`;
  }
  /** The confirmation before a removal (removal cannot be undone; they need a new invite ticket to return). */
  function groupReportRemoveConfirm(name, groupName) {
    return `Remove ${name} from "${groupName}"? They can come back only with a new invite ticket.`;
  }

  // ── Rules ──

  function utf8Length(s) { return new TextEncoder().encode(String(s)).length; }

  /** A file marker anywhere fails closed: with report.js missing, nothing counts as safe. */
  function hasFile(text) {
    if (typeof rep.reportTextHasFile !== 'function') return true;
    return rep.reportTextHasFile(text);
  }

  function keyNorm(key) {
    return typeof rep.reportKeyNorm === 'function' ? rep.reportKeyNorm(key) : null;
  }

  function idNorm(id) {
    if (typeof id !== 'string') return null;
    const v = id.trim().toLowerCase();
    return GROUP_REPORT_ID_RE.test(v) ? v : null;
  }

  function sameKey(a, b) {
    const x = keyNorm(a);
    return !!x && x === keyNorm(b);
  }

  /**
   * Which destinations the dialog offers for a group message (10j), and why
   * when only the admins remain. `who`: {me, target, creator, finding}: the
   * creator's key once found ('' or null when it could not be), `finding`
   * true while it is being found. Returns {choices, chosen, line}: `choices`
   * in the dialog's order, `chosen` the default, `line` the sentence shown
   * (null when all three are offered). While finding: choices empty.
   */
  function groupReportChoices(who) {
    const w = who || {};
    if (w.finding) return { choices: [], chosen: null, line: GROUP_REPORT_FINDING_CREATOR, finding: true };
    const creator = keyNorm(w.creator);
    const adminsOnly = (line) => ({ choices: ['admins'], chosen: 'admins', line, finding: false });
    if (!creator) return adminsOnly(GROUP_REPORT_CREATOR_UNKNOWN);
    if (sameKey(creator, w.me)) return adminsOnly(GROUP_REPORT_YOU_CREATED);
    if (sameKey(creator, w.target)) return adminsOnly(GROUP_REPORT_THEY_CREATED);
    return { choices: GROUP_REPORT_DESTINATIONS.slice(), chosen: 'creator', line: null, finding: false };
  }

  /** Does this destination include the creator, the admins? */
  function groupReportToCreator(dest) { return dest === 'creator' || dest === 'both'; }
  function groupReportToAdmins(dest) { return dest === 'admins' || dest === 'both'; }

  /**
   * One group message as an item: {id, from, ts, text}. Null when the id or
   * the sender is not one, or the text carries a file (step D's rule).
   */
  function groupReportItem(id, from, ts, text) {
    const i = idNorm(id);
    const f = keyNorm(from);
    const t = Number(ts);
    const words = String(text == null ? '' : text);
    if (!i || !f || !Number.isSafeInteger(t) || t < 0) return null;
    if (hasFile(words)) return null;
    return { id: i, from: f, ts: t, text: words };
  }

  /**
   * The text of a report to the creator: {text} (the marker, then the JSON in
   * 10j's order), or {error} saying why nothing can be sent. `fields`:
   * {group_id, group_name, target, reason, note, items}.
   */
  function groupReportText(fields) {
    const f = fields || {};
    const groupId = idNorm(f.group_id);
    if (!groupId) return { error: 'This group is not known here.' };
    const target = keyNorm(f.target);
    if (!target) return { error: 'This person cannot be reported from here: their key is not known.' };
    if (typeof f.reason !== 'string' || !/^[a-z0-9_]{1,64}$/.test(f.reason)) return { error: 'Choose a reason first.' };
    const items = Array.isArray(f.items) ? f.items : [];
    if (items.length > GROUP_REPORT_MAX_ITEMS) return { error: `At most ${GROUP_REPORT_MAX_ITEMS} messages can be included. Untick some.` };
    const clean = [];
    for (const it of items) {
      if (it && hasFile(it.text)) return { error: rep.REPORT_NO_FILES || 'Messages with files cannot be included in a report.' };
      const item = it ? groupReportItem(it.id, it.from, it.ts, it.text) : null;
      if (!item) return { error: 'A group message is not named.' };
      clean.push(item);
    }
    const note = typeof rep.reportNoteNorm === 'function' ? rep.reportNoteNorm(f.note) : String(f.note || '').trim();
    if (hasFile(note)) return { error: rep.REPORT_NO_FILES || 'Messages with files cannot be included in a report.' };
    const body = {
      group_id: groupId,
      group_name: String(f.group_name == null ? '' : f.group_name),
      target,
      reason: f.reason,
      note,
      items: clean.map((it) => ({ id: it.id, from: it.from, ts: it.ts, text: it.text })),
    };
    const text = GROUP_REPORT_MARKER + JSON.stringify(body);
    if (utf8Length(text) > GROUP_REPORT_MAX_BYTES) {
      return { error: "This report is too long to send to the group's creator (16 KB at most). Shorten the note or include fewer messages." };
    }
    return { text };
  }

  /** Is this DM text a report to a group's creator (whatever follows the marker)? Such text is never shown as a message. */
  function isGroupReportText(text) {
    return typeof text === 'string' && text.startsWith(GROUP_REPORT_MARKER);
  }

  /**
   * Read a report's text: the fields, normalised, or null when it is not one
   * this client would send: the wrong shape, more than 20 items, over 16 KB,
   * or a file anywhere in it.
   */
  function groupReportParse(text) {
    if (!isGroupReportText(text)) return null;
    if (utf8Length(text) > GROUP_REPORT_MAX_BYTES) return null;
    if (hasFile(text)) return null;
    let v;
    try { v = JSON.parse(text.slice(GROUP_REPORT_MARKER.length)); } catch { return null; }
    if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
    const groupId = idNorm(v.group_id);
    const target = keyNorm(v.target);
    if (!groupId || !target) return null;
    if (typeof v.reason !== 'string' || !/^[a-z0-9_]{1,64}$/.test(v.reason)) return null;
    if (typeof v.group_name !== 'string' || typeof v.note !== 'string') return null;
    if (Array.from(v.note).length > (rep.REPORT_NOTE_MAX || 500)) return null;
    if (!Array.isArray(v.items) || v.items.length > GROUP_REPORT_MAX_ITEMS) return null;
    const items = [];
    for (const it of v.items) {
      if (!it || typeof it !== 'object' || typeof it.text !== 'string') return null;
      const item = groupReportItem(it.id, it.from, it.ts, it.text);
      if (!item) return null;
      items.push(item);
    }
    return { group_id: groupId, group_name: v.group_name, target, reason: v.reason, note: v.note, items };
  }

  /**
   * Does the creator keep this report? `ctx`: {me, from, to, report, groups},
   * `from` and `to` the DM's signed sender and recipient, `groups` the
   * creator's own groups ({group_id, name, members, is_creator}). Returns
   * {ok: true, group} or {ok: false, why}: not addressed to me, a group I do
   * not hold as its creator, a reporter who is not in it, or a report about me.
   */
  function groupReportAccepts(ctx) {
    const c = ctx || {};
    const r = c.report;
    if (!r) return { ok: false, why: 'not_a_report' };
    if (!sameKey(c.to, c.me)) return { ok: false, why: 'not_to_me' };
    const group = (Array.isArray(c.groups) ? c.groups : []).find((g) => g && idNorm(g.group_id) === r.group_id) || null;
    if (!group) return { ok: false, why: 'not_held' };
    if (!group.is_creator) return { ok: false, why: 'not_creator' };
    const members = Array.isArray(group.members) ? group.members : [];
    if (!members.some((m) => sameKey(m, c.from))) return { ok: false, why: 'reporter_not_in_group' };
    if (sameKey(r.target, c.me)) return { ok: false, why: 'about_me' };
    return { ok: true, group };
  }

  /**
   * Check each item against the creator's own copy of the group (10j).
   * `objects`: the group's signed message objects as this client holds them
   * (the shape the relay serves: object_id, object_type, author_public_key_b64,
   * created_at, references, payload_b64, signature_b64, ...). `deps`:
   *   verify(obj) -> {ok, objectId, authorPubHex, createdAt, references, payload}
   *     (pq-object.js verifyObjectSubmission: the signature checked over the
   *     object's own bytes, objectId their hash), possibly async;
   *   open(payload) -> the message's text, or null (the group key), possibly async.
   * Returns the items, each with `found` and `signer` (the signing key when found).
   */
  async function groupReportCheck(report, objects, deps) {
    const list = Array.isArray(objects) ? objects : [];
    const out = [];
    for (const it of (report && Array.isArray(report.items)) ? report.items : []) {
      let found = false;
      let signer = '';
      const candidates = list.filter((o) => o && typeof o.object_id === 'string' && o.object_id.toLowerCase() === it.id);
      for (const o of candidates) {
        let v = null;
        try { v = await deps.verify(o); } catch { v = null; }
        if (!v || v.ok !== true) continue;                        // no valid signature on the stored object
        if (v.objectId !== it.id) continue;                       // its bytes are not the id named
        if (o.object_type !== 'group_msg_v1') continue;           // (covered by the signature)
        if (!Array.isArray(v.references) || v.references[0] !== report.group_id) continue;
        if (!sameKey(v.authorPubHex, it.from)) continue;          // another sender
        if (Number(v.createdAt) !== it.ts) continue;              // another time
        let words = null;
        try { words = await deps.open(v.payload); } catch { words = null; }
        if (words !== it.text) continue;                          // other words
        found = true;
        signer = keyNorm(v.authorPubHex);
        break;
      }
      out.push({ id: it.id, from: it.from, ts: it.ts, text: it.text, found, signer });
    }
    return out;
  }

  /** The badge on one checked item: found (with the signer's name) or not. */
  function groupReportItemBadge(item, name) {
    if (item && item.found === true) return { found: true, text: groupReportFoundBadge(name) };
    return { found: false, text: GROUP_REPORT_NOT_FOUND };
  }

  /** How many kept reports are about this group (the count on the group). */
  function groupReportCount(reports, groupId) {
    const id = idNorm(groupId);
    if (!id) return 0;
    return (Array.isArray(reports) ? reports : []).filter((r) => r && r.group_id === id).length;
  }

  const api = {
    GROUP_REPORT_MARKER, GROUP_REPORT_MAX_ITEMS, GROUP_REPORT_MAX_BYTES, GROUP_REPORT_ID_RE,
    GROUP_REPORT_DESTINATIONS, GROUP_REPORT_DESTINATION_LABELS, GROUP_REPORT_SEND_TO,
    GROUP_REPORT_YOU_CREATED, GROUP_REPORT_THEY_CREATED, GROUP_REPORT_CREATOR_UNKNOWN,
    GROUP_REPORT_FINDING_CREATOR, GROUP_REPORT_CREATOR_SEES, GROUP_REPORT_CREATOR_CHECKS,
    GROUP_REPORT_SENT_LINE, GROUP_REPORT_REFUSED_LINE,
    GROUP_REPORTS_TITLE, GROUP_REPORTS_HELP, GROUP_REPORTS_NONE, GROUP_REPORT_NOT_FOUND,
    GROUP_REPORT_ACTION_LABELS, GROUP_REPORT_REMOVED, GROUP_REPORT_NOT_IN_GROUP, GROUP_REPORT_BLOCKED,
    GROUP_REPORT_NO_ITEMS,
    groupReportFoundBadge, groupReportArrivedLine, groupReportCountTitle, groupReportRemoveConfirm,
    groupReportChoices, groupReportToCreator, groupReportToAdmins, groupReportItem, groupReportText,
    isGroupReportText, groupReportParse, groupReportAccepts, groupReportCheck, groupReportItemBadge,
    groupReportCount,
  };
  if (isNode) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
