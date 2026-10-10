// ── chat-reports.js ───────────────────────────────────────────────────────
// Reports the admins can check (step D, 2026-10-09,
// docs/design/blocking-and-safe-mode.md section 8 and 10e), mirroring the
// desktop app.
//
// The Report dialog, from a message's menu (the post itself, or the words seen
// in a group), a DM conversation's header (their messages to tick as
// evidence, the most recent ticked, never one with a file: its text carries the
// key that opens the file) and a person's entry in the member list:
// the reasons from /data/safety/report_reasons.json with their help text shown
// when chosen, an optional note, and "Also block them", ticked for DM reports,
// which runs Block (chat-privacy.js blockKey). The report is signed with my
// identity key (crypto.js pqBuildReport over /shared/report.js) and sent as
// `report_v2`; the relay answers `report_received`, shown as one line, or
// refuses with a notice (a bad signature, an unknown reason, myself, more than
// 3 an hour, the same person again within a day).
//
// Help outside this server (10e-ii, 2026-10-10): when the reason is that a
// child or anyone may be in danger, a block under the reason's help gives the
// emergency number and the official place to report a child being exploited
// online, for a country picked in the block, from
// /data/safety/outside_help.json (words and rules in /shared/report.js). The
// first country is the last one picked on this device (localStorage), else
// the region of navigator.language when it is listed, else "Another country".
// The location is never looked up, and the country is never sent anywhere:
// it is not part of the report.
//
// A report about a group reaches the group's creator (10j, 2026-10-10): for a
// P2P group message the dialog asks "Send this report to", with "The group's
// creator" (the default), "This server's admins" and "Both"; only the admins
// remain when I created the group, when the person reported did, or when the
// creator cannot be found out (each with its sentence). Choosing the creator
// says "The group's creator will see that you sent this." and sends a sealed
// DM flagged `group_report` (crypto.js pqBuildGroupReport over
// /shared/group-report.js). On the creator's side such a DM is checked against
// their own copy of the group and listed in Settings > Safety under "Reports
// about your groups" (and as a count on the group), with Remove them from the
// group, Block them and Dismiss; kept on that device only.
//
// The Reports view for admins and mods: open and decided reports
// (`reports_list` -> `reports`), each piece of evidence with "Signature
// checked: sent by <name> to the reporter" or "Not proven", what a checked
// signature does not prove, and the decisions (`report_decide`), which the
// relay carries out through its moderation path and its rules. It lives here,
// in the chat app, and not on the Admin page (web/pages/admin.html): the Admin
// page is open to admins only and signs its requests over HTTP, while the
// report frames go over the signed-in chat socket, and moderators review
// reports too. The Admin page points here. Opened from the command palette's
// Moderation section (View Reports) or by typing /reports.
//
// Depends on: /shared/report.js, crypto.js (pqBuildReport), app.js (ws, myKey,
// peerData, handleMessage, addSystemMessage, shortKey, isBlockedKey),
// chat-dm-store.js (hosDmStore), chat-privacy.js (blockKey, blockKeyForName).
// ─────────────────────────────────────────────────────────────────────────

// HTML-escape for the strings this file builds (pure: no DOM needed).
function repEsc(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function repSay(text) {
  if (typeof addSystemMessage === 'function') addSystemMessage(text);
}

/** The member list's name for a key, or a short key. */
function repName(key) {
  const peers = (typeof peerData !== 'undefined' && peerData) ? peerData : {};
  const p = key ? peers[key] : null;
  if (p && (p.display_name || p.name)) return p.display_name || p.name;
  return typeof shortKey === 'function' ? shortKey(key) : String(key || '').slice(0, 8);
}

function repSameKey(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a !== '' && a.toLowerCase() === b.toLowerCase();
}

function repWhen(ms) {
  if (!ms) return '';
  const d = new Date(ms);
  try { return d.toLocaleString(undefined, { year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }); }
  catch { return d.toISOString().slice(0, 16).replace('T', ' '); }
}

/** What a message's text looks like in a list: an attachment by its name, never its key. */
function repTextLabel(text) {
  const fm = (typeof pqParseFileMarker === 'function') ? pqParseFileMarker(text) : null;
  if (fm) return 'An encrypted file: ' + (fm.name || 'file');
  return String(text == null ? '' : text);
}

function repSocketOpen() {
  return typeof ws !== 'undefined' && ws && ws.readyState === 1;
}

// ── The reasons (data, 10e) ──────────────────────────────────────────────

let reportReasons = null;        // [{id, label, help}] once loaded
let reportReasonsLoading = null; // the fetch in flight

/** The reasons from the data file; an empty list when it could not be read. */
function loadReportReasons() {
  if (reportReasons && reportReasons.length) return Promise.resolve(reportReasons);
  if (!reportReasonsLoading) {
    reportReasonsLoading = Promise.resolve()
      .then(() => fetch(REPORT_REASONS_URL, { cache: 'no-cache' }))
      .then((r) => (r && r.ok ? r.json() : null))
      .then((j) => reportReasonsFrom(j))
      .catch(() => [])
      .then((list) => {
        reportReasonsLoading = null;
        if (list.length) reportReasons = list;
        return list;
      });
  }
  return reportReasonsLoading;
}

// ── Help outside this server (data, 10e-ii) ──────────────────────────────

let outsideHelp = null;        // outsideHelpFrom() output, once loaded
let outsideHelpLoading = null; // the fetch in flight

/**
 * The outside help lines from the data file, or null when it could not be
 * read. A failure is not kept, so the next dialog asks again.
 */
function loadOutsideHelp() {
  if (outsideHelp) return Promise.resolve(outsideHelp);
  if (!outsideHelpLoading) {
    outsideHelpLoading = Promise.resolve()
      .then(() => fetch(OUTSIDE_HELP_URL, { cache: 'no-cache' }))
      .then((r) => (r && r.ok ? r.json() : null))
      .then((j) => outsideHelpFrom(j))
      .catch(() => null)
      .then((help) => {
        outsideHelpLoading = null;
        if (help) outsideHelp = help;
        return help;
      });
  }
  return outsideHelpLoading;
}

/** The country last picked in the block on this device, or null. Storage can be off or full. */
function outsideHelpSaved() {
  try {
    const v = localStorage.getItem(OUTSIDE_HELP_SAVED_KEY);
    return typeof v === 'string' ? v : null;
  } catch { return null; }
}

function outsideHelpSave(code) {
  try { localStorage.setItem(OUTSIDE_HELP_SAVED_KEY, code); } catch { /* kept for this dialog only */ }
}

/** The device's language setting, the only hint used for the first country. */
function outsideHelpLanguage() {
  try {
    const lang = typeof navigator !== 'undefined' && navigator ? navigator.language : null;
    return typeof lang === 'string' ? lang : null;
  } catch { return null; }
}

// ── The Report dialog ────────────────────────────────────────────────────

// How many of their messages the DM picker lists (newest first). At most
// REPORT_MAX_ITEMS of them can be ticked.
const REPORT_PICK_SHOWN = 50;

let reportDialog = null; // the open dialog's state, or null

/**
 * Their messages in my conversation with them that can be evidence: sent by
 * them to me, kept with their signature, newest first. Notes the clients
 * send each other (follow notices, passes, contact requests) are not
 * messages. A message with a file is NEVER offered (REPORT_NO_FILES in
 * /shared/report.js): its text carries the key that opens the file, and the
 * signature covers the whole text, so it cannot be sent without the key.
 */
function reportDmCandidates(target) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store) return [];
  const me = typeof myKey === 'string' ? myKey : '';
  return store.conversation(target)
    .filter((m) => m && repSameKey(m.from, target) && repSameKey(m.to, me) && typeof m.sig === 'string' && m.sig)
    .filter((m) => !reportTextHasFile(m.text) && !String(m.text || '').startsWith('[[hum:'))
    .slice()
    .sort((a, b) => b.ts - a.ts)
    .slice(0, REPORT_PICK_SHOWN);
}

/**
 * Open the Report dialog. `opts`: {target (identity key), name, context
 * ('dm' | 'post' | 'group' | 'profile'), message: {timestamp, text} for a
 * post or a group message}. Resolves to the dialog's state, or null when this
 * person cannot be reported from here.
 */
async function openReportDialog(opts) {
  const o = opts || {};
  const target = reportKeyNorm(o.target);
  if (!target) { repSay('This person cannot be reported from here: their key is not known here yet.'); return null; }
  if (typeof myKey === 'string' && repSameKey(target, myKey)) { repSay("You can't report yourself."); return null; }
  const context = REPORT_CONTEXTS.includes(o.context) ? o.context : 'profile';
  const alreadyBlocked = typeof isBlockedKey === 'function' && isBlockedKey(target);
  const state = {
    target,
    name: o.name || repName(target),
    context,
    reason: '',
    note: '',
    // "Also block them": ticked by default for DM reports (10e), offered on every report.
    alsoBlock: context === 'dm' && !alreadyBlocked,
    alreadyBlocked,
    // The DM picker: their messages, the most recent ticked.
    picks: [],
    // The post or group message the report is about (not a choice).
    fixed: null,
    reasons: null,
    // Help outside this server (10e-ii): the data once loaded, and the country
    // shown, which stays on this device and is never part of the report.
    outsideHelp: null,
    country: null,
    error: '',
    sending: false,
  };
  if (context === 'dm') {
    state.picks = reportDmCandidates(target).map((m) => ({ item: dmEvidenceItem(m), picked: false })).filter((p) => p.item);
    if (state.picks.length) state.picks[0].picked = true;
  } else if ((context === 'post' || context === 'group') && o.message) {
    const ts = Number(o.message.timestamp) || 0;
    const item = context === 'post' ? postEvidenceItem(target, ts) : groupEvidenceItem(target, ts, o.message.text);
    // A group message with a file is never included (groupEvidenceItem gives null).
    if (item) state.fixed = { item, text: String(o.message.text == null ? '' : o.message.text) };
    else state.fileLeftOut = true;
  }
  // A P2P group message (10j): who the report goes to. Its group and the
  // message's signed-object id come with the menu (app.js); the creator is
  // found from the group's own signed record before anything can be sent.
  if (context === 'group' && typeof groupReportChoices === 'function') {
    const m = o.message || {};
    const groupId = typeof m.groupId === 'string' ? m.groupId : '';
    state.group = { id: groupId, name: typeof m.groupName === 'string' ? m.groupName : '' };
    // The item the creator checks; null for a file (the step D rule) or an unknown id.
    state.groupItem = groupReportItem(m.id, target, m.timestamp, m.text);
    state.creator = null;
    state.finding = !!groupId;
    state.sendTo = state.finding ? null : groupReportChoices({ me: repMe(), target, creator: null }).chosen;
  }
  reportDialog = state;
  renderReportDialog();
  if (state.finding) findGroupCreator(state);
  // Not awaited: the reasons must never wait on this file, and a dialog with
  // no outside help still sends reports.
  loadOutsideHelp().then((help) => {
    if (!help || reportDialog !== state) return;
    state.outsideHelp = help;
    state.country = outsideHelpFirstCountry(help, outsideHelpSaved(), outsideHelpLanguage());
    renderReportDialog();
  });
  const reasons = await loadReportReasons();
  if (reportDialog === state) {
    state.reasons = reasons;
    renderReportDialog();
  }
  return state;
}

function repMe() {
  return typeof myKey === 'string' ? myKey : '';
}

/**
 * Find the group's creator for the open dialog (10j): from the group's own
 * signed record (chat-groups-p2p.js p2pGroupCreatorKey), else, for my own
 * groups, my group list's word that I created it. Then the destinations are
 * known and the default is chosen.
 */
async function findGroupCreator(state) {
  let creator = null;
  try {
    if (typeof p2pGroupCreatorKey === 'function') creator = await p2pGroupCreatorKey(state.group.id);
  } catch { creator = null; }
  if (!creator) {
    const mine = (Array.isArray(window._p2pGroups) ? window._p2pGroups : []).find((g) => g && g.group_id === state.group.id);
    if (mine && mine.is_creator && repMe()) creator = repMe().toLowerCase();
  }
  state.creator = creator;
  state.finding = false;
  state.sendTo = reportGroupChoices(state).chosen;
  if (reportDialog === state) renderReportDialog();
  return state;
}

/**
 * The destinations the dialog offers now (/shared/group-report.js
 * groupReportChoices), or null when the report is not about a group message.
 */
function reportGroupChoices(st) {
  if (!st || st.context !== 'group' || !st.group || typeof groupReportChoices !== 'function') return null;
  return groupReportChoices({ me: repMe(), target: st.target, creator: st.creator, finding: st.finding });
}

/** Where the report goes: 'admins' unless a group report chose otherwise; null while the creator is being found. */
function reportDialogDestination(st) {
  const c = reportGroupChoices(st);
  if (!c) return 'admins';
  if (c.finding) return null;
  return c.choices.includes(st.sendTo) ? st.sendTo : c.chosen;
}

/** Choose who a group report goes to: 'creator', 'admins' or 'both', when offered. */
function reportDialogSendTo(dest) {
  const st = reportDialog;
  const c = reportGroupChoices(st);
  if (!c || !c.choices.includes(dest)) return false;
  st.sendTo = dest;
  st.error = '';
  renderReportDialog();
  return true;
}

/** The evidence the dialog would send now. */
function reportDialogEvidence(state) {
  const st = state || reportDialog;
  if (!st) return [];
  if (st.context === 'dm') return st.picks.filter((p) => p.picked && p.item).map((p) => p.item);
  return st.fixed ? [st.fixed.item] : [];
}

function reportDialogChoose(id) {
  const st = reportDialog;
  if (!st || !st.reasons || !st.reasons.some((r) => r.id === id)) return false;
  st.reason = id;
  st.error = '';
  renderReportDialog();
  return true;
}

function reportDialogSetNote(text) {
  if (!reportDialog) return;
  reportDialog.note = String(text == null ? '' : text);
}

/** Tick or untick message `i` of the DM picker; refused past REPORT_MAX_ITEMS. */
function reportDialogToggle(i, on) {
  const st = reportDialog;
  const p = st && st.picks[i];
  if (!p) return false;
  if (on && !p.picked && st.picks.filter((x) => x.picked).length >= REPORT_MAX_ITEMS) {
    st.error = `At most ${REPORT_MAX_ITEMS} messages can be included.`;
    renderReportDialog();
    return false;
  }
  p.picked = !!on;
  st.error = '';
  renderReportDialog();
  return true;
}

function reportDialogSetBlock(on) {
  if (!reportDialog || reportDialog.alreadyBlocked) return;
  reportDialog.alsoBlock = !!on;
}

/**
 * Pick the country the outside help block shows: a listed code or "Another
 * country". Kept on this device for next time (10e-ii), never sent.
 */
function reportDialogSetCountry(code) {
  const st = reportDialog;
  if (!st || !st.outsideHelp) return false;
  const ok = code === OUTSIDE_HELP_OTHER || st.outsideHelp.countries.some((c) => c.code === code);
  if (!ok) return false;
  st.country = code;
  outsideHelpSave(code);
  renderReportDialog();
  return true;
}

/** The outside help block's HTML for a view (outsideHelpView); pure. */
function outsideHelpHtml(v) {
  const muted = 'color:var(--text-muted);font-size:var(--text-xs);line-height:1.4;margin:var(--space-xs) 0 0;';
  let html = `<div class="report-outside-help" data-outside-help="${repEsc(v.code)}" style="border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-sm) var(--space-md);margin:var(--space-sm) 0;color:var(--text);">`
    + `<div style="font-weight:600;font-size:0.9rem;">${repEsc(v.title)}</div>`
    + '<label style="display:flex;align-items:center;gap:var(--space-sm);margin:var(--space-xs) 0;color:var(--text);font-size:0.85rem;">Country:'
    + '<select data-report-country aria-label="Country" style="background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:2px var(--space-xs);">'
    + v.choices.map((c) => `<option value="${repEsc(c.code)}"${c.code === v.code ? ' selected' : ''}>${repEsc(c.name)}</option>`).join('')
    + '</select></label>';
  html += v.emergency
    ? `<div class="report-outside-emergency" style="font-size:var(--text-lg);font-weight:700;margin:var(--space-xs) 0;">Emergency: ${repEsc(v.emergency)}</div>`
    : `<div class="report-outside-emergency-text" style="font-size:1rem;font-weight:600;margin:var(--space-xs) 0;">${repEsc(v.emergencyText)}</div>`;
  for (const a of v.also) {
    html += `<div class="report-outside-also" style="font-size:0.85rem;"><strong>${repEsc(a.number)}</strong>${a.for ? ': ' + repEsc(a.for) : ''}</div>`;
  }
  if (v.child) {
    // Opens in a new tab and tells that site nothing about this page.
    const size = v.childSmall ? 'var(--text-xs)' : '0.9rem';
    html += `<div class="report-outside-child${v.childSmall ? ' report-outside-child-small' : ''}" style="font-size:${size};margin-top:var(--space-sm);line-height:1.4;">`
      + `${repEsc(v.childLead)} <a href="${repEsc(v.child.url)}" target="_blank" rel="noopener noreferrer"${v.child.title ? ` title="${repEsc(v.child.title)}"` : ''} style="color:var(--accent);">${repEsc(v.child.name)}</a></div>`;
  }
  if (v.note) html += `<p class="report-outside-note" style="${muted}">${repEsc(v.note)}</p>`;
  if (v.dateLine) html += `<p class="report-outside-checked" style="${muted}">${repEsc(v.dateLine)}</p>`;
  return html + '</div>';
}

/** The dialog's first line: who reads the report (10j adds the group's creator). */
function reportDialogLead(st) {
  const tail = ` ${st.name} is not told who reported them.`;
  const dest = reportDialogDestination(st);
  if (dest === 'creator') return "This goes to the group's creator." + tail;
  if (dest === 'both') return "This goes to the group's creator and to this server's admins and moderators." + tail;
  return "This goes to this server's admins and moderators." + tail;
}

/** The dialog's HTML for a state (pure, so it can be checked without a browser). */
function reportDialogHtml(st) {
  const muted = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';
  const h3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
  let html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
    + `<h2 style="margin:0;">Report ${repEsc(st.name)}</h2>`
    + '<button class="vr-btn" data-report-cancel style="font-size:0.75rem;">Close</button></div>'
    + `<p class="report-lead" style="${muted}margin-top:var(--space-sm);">${repEsc(reportDialogLead(st))}</p>`;

  // A group message (10j): who the report goes to, and why when only the admins remain.
  const dest = reportDialogDestination(st);
  const choices = reportGroupChoices(st);
  if (choices) {
    html += `<h3 style="${h3}">${repEsc(GROUP_REPORT_SEND_TO)}</h3>`;
    if (choices.finding) {
      html += `<p class="report-send-to-finding" style="${muted}">${repEsc(choices.line)}</p>`;
    } else {
      html += '<div role="radiogroup" aria-label="' + repEsc(GROUP_REPORT_SEND_TO) + '">'
        + choices.choices.map((d) =>
          '<label class="report-send-to" style="display:flex;align-items:center;gap:var(--space-sm);margin:0;padding:var(--space-xs) 0;cursor:pointer;color:var(--text);font-size:0.9rem;">'
          + `<input type="radio" name="report-send-to" value="${d}" data-report-send-to="${d}"${d === dest ? ' checked' : ''}>`
          + `<span>${repEsc(GROUP_REPORT_DESTINATION_LABELS[d])}</span></label>`).join('')
        + '</div>';
      if (choices.line) html += `<p class="report-send-to-why" style="${muted}margin-top:var(--space-xs);">${repEsc(choices.line)}</p>`;
      if (dest === 'creator' || dest === 'both') {
        html += `<p class="report-creator-sees" style="border-left:3px solid var(--info);padding-left:var(--space-sm);color:var(--text);font-size:var(--text-sm);line-height:1.4;margin:var(--space-xs) 0 0;">${repEsc(GROUP_REPORT_CREATOR_SEES)}</p>`;
      }
    }
  }

  html += `<h3 style="${h3}">What happened</h3>`;
  if (st.reasons === null) {
    html += `<p class="report-reasons-loading" style="${muted}">Loading the reasons...</p>`;
  } else if (!st.reasons.length) {
    html += `<p class="report-reasons-failed" style="${muted}color:var(--danger);">The list of reasons could not be loaded from this server, so a report cannot be sent yet. Close this and try again in a moment.</p>`;
  } else {
    html += st.reasons.map((r) =>
      // margin:0 and the font size: .profile-modal label (modals.css) spaces form labels apart.
      '<label class="report-reason" style="display:flex;align-items:center;gap:var(--space-sm);margin:0;padding:var(--space-xs) 0;cursor:pointer;color:var(--text);font-size:0.9rem;">'
      + `<input type="radio" name="report-reason" value="${repEsc(r.id)}" data-report-reason="${repEsc(r.id)}"${r.id === st.reason ? ' checked' : ''}>`
      + `<span>${repEsc(r.label)}</span></label>`).join('');
    const chosen = st.reasons.find((r) => r.id === st.reason);
    if (chosen && chosen.help) {
      html += `<div class="report-help" data-report-help="${repEsc(chosen.id)}" style="border-left:3px solid var(--warning);background:var(--bg-secondary);padding:var(--space-sm) var(--space-md);margin:var(--space-sm) 0;color:var(--text);font-size:var(--text-sm);line-height:1.45;">${repEsc(chosen.help)}</div>`;
    }
    // Under the reason's help, for the two danger reasons only (10e-ii).
    const outside = chosen ? outsideHelpView(st.outsideHelp, chosen.id, st.country) : null;
    if (outside) html += outsideHelpHtml(outside);
  }

  if (st.context === 'dm') {
    const n = st.picks.filter((p) => p.picked).length;
    html += `<h3 style="${h3}">Their messages to include</h3>`
      + `<p style="${muted}">${repEsc(REPORT_DM_EVIDENCE_HELP)}</p>`
      + `<p class="report-no-files" style="${muted}">${repEsc(REPORT_NO_FILES)}</p>`;
    if (!st.picks.length) {
      html += `<p class="report-no-evidence" style="${muted}">None of their messages in this conversation can be included: only messages they sent you that this device keeps with their signature can be.</p>`;
    } else {
      html += st.picks.map((p, i) =>
        `<label class="report-evidence" style="display:flex;align-items:flex-start;gap:var(--space-sm);margin:0;padding:var(--space-xs) 0;border-top:1px solid var(--border);cursor:pointer;font-size:0.9rem;">`
        + `<input type="checkbox" data-report-evidence="${i}"${p.picked ? ' checked' : ''} style="margin-top:3px;">`
        + '<span style="flex:1;min-width:0;">'
        + `<span style="display:block;color:var(--text-muted);font-size:var(--text-xs);">${repEsc(repWhen(p.item.ts))}</span>`
        + `<span style="display:block;color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;">${repEsc(repTextLabel(p.item.text))}</span>`
        + '</span></label>').join('')
        + `<p class="report-evidence-count" style="${muted}margin-top:var(--space-xs);">${n} of at most ${REPORT_MAX_ITEMS} chosen.</p>`;
    }
  } else if (st.fixed) {
    const label = st.context === 'post' ? 'The post' : 'The group message';
    html += `<h3 style="${h3}">${label}</h3>`
      + `<div class="report-fixed" style="border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-sm) var(--space-md);color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;">${repEsc(repTextLabel(st.fixed.text))}</div>`;
    if (st.context === 'post') {
      html += `<p style="${muted}margin-top:var(--space-xs);">The server finds this post itself, so the admins see it as it was posted.</p>`;
    } else {
      // The admins cannot check a group's words (step D); the creator can (10j).
      if (dest === null || dest === 'admins' || dest === 'both') html += `<p class="report-group-unproven" style="${muted}margin-top:var(--space-xs);">${repEsc(REPORT_GROUP_UNPROVEN)}</p>`;
      if (dest === 'creator' || dest === 'both') html += `<p class="report-creator-checks" style="${muted}margin-top:var(--space-xs);">${repEsc(GROUP_REPORT_CREATOR_CHECKS)}</p>`;
    }
  } else if (st.fileLeftOut) {
    html += `<h3 style="${h3}">The group message</h3>`
      + `<p class="report-no-files" style="${muted}">${repEsc(REPORT_NO_FILES)} Describe what happened in the note instead.</p>`;
  }

  const noteFor = { creator: ["A note for the group's creator", "What the group's creator should know"], both: ["A note for the group's creator and the admins", 'What they should know'] }[dest]
    || ['A note for the admins', 'What the admins should know'];
  html += `<h3 style="${h3}">Anything to add (optional)</h3>`
    + `<textarea data-report-note maxlength="${REPORT_NOTE_MAX}" rows="3" aria-label="${repEsc(noteFor[0])}" placeholder="${repEsc(noteFor[1])}" style="width:100%;box-sizing:border-box;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-sm);">${repEsc(st.note)}</textarea>`;

  if (st.alreadyBlocked) {
    html += `<p style="${muted}margin-top:var(--space-sm);">You have blocked them.</p>`;
  } else {
    html += '<label class="report-also-block" style="display:flex;align-items:flex-start;gap:var(--space-sm);margin:var(--space-md) 0 0;cursor:pointer;color:var(--text);font-size:0.9rem;">'
      + `<input type="checkbox" data-report-block${st.alsoBlock ? ' checked' : ''} style="margin-top:3px;">`
      + `<span>Also block them<span style="display:block;${muted}margin:0;">You will not see anything from them, and they are not told.</span></span></label>`;
  }

  if (st.error) html += `<p class="report-error" role="alert" style="color:var(--danger);font-size:var(--text-sm);margin:var(--space-sm) 0 0;">${repEsc(st.error)}</p>`;
  const canSend = !st.sending && !!st.reason && !!(st.reasons && st.reasons.length) && dest !== null;
  html += '<div style="display:flex;justify-content:flex-end;gap:var(--space-sm);margin-top:var(--space-lg);">'
    + '<button class="vr-btn" data-report-cancel style="font-size:0.8rem;">Cancel</button>'
    + `<button class="vr-btn" data-report-send${canSend ? '' : ' disabled'} style="font-size:0.8rem;color:var(--danger);">${st.sending ? 'Sending...' : 'Send report'}</button>`
    + '</div>';
  return html;
}

/** Draw the dialog (or take it down when none is open). */
function renderReportDialog() {
  if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
  let overlay = document.getElementById('report-overlay');
  const st = reportDialog;
  if (!st) {
    if (overlay && overlay.classList) overlay.classList.remove('open');
    return;
  }
  let card = document.getElementById('report-card');
  if (!overlay) {
    // Built once; the ids find them again after that.
    overlay = document.createElement('div');
    overlay.id = 'report-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.style.zIndex = '10000';
    overlay.onclick = (e) => { if (e && e.target === overlay) closeReportDialog(); };
    card = document.createElement('div');
    card.id = 'report-card';
    card.className = 'profile-modal';
    card.setAttribute('role', 'dialog');
    card.setAttribute('aria-label', 'Report');
    card.onclick = (e) => { if (e) e.stopPropagation(); };
    overlay.appendChild(card);
    if (document.body) document.body.appendChild(overlay);
  }
  if (overlay.classList) overlay.classList.add('open');
  if (!card) return;
  card.innerHTML = reportDialogHtml(st);
  if (typeof card.querySelectorAll !== 'function') return;
  card.querySelectorAll('[data-report-reason]').forEach((r) => { r.onchange = () => reportDialogChoose(r.dataset.reportReason); });
  card.querySelectorAll('[data-report-send-to]').forEach((r) => { r.onchange = () => reportDialogSendTo(r.dataset.reportSendTo); });
  const country = card.querySelector('[data-report-country]');
  if (country) country.onchange = () => reportDialogSetCountry(country.value);
  const note = card.querySelector('[data-report-note]');
  if (note) note.oninput = () => reportDialogSetNote(note.value);
  card.querySelectorAll('[data-report-evidence]').forEach((c) => {
    c.onchange = () => reportDialogToggle(Number(c.dataset.reportEvidence), c.checked);
  });
  const blk = card.querySelector('[data-report-block]');
  if (blk) blk.onchange = () => reportDialogSetBlock(blk.checked);
  card.querySelectorAll('[data-report-cancel]').forEach((b) => { b.onclick = () => closeReportDialog(); });
  const send = card.querySelector('[data-report-send]');
  if (send) send.onclick = () => submitReportDialog();
}

function closeReportDialog() {
  reportDialog = null;
  renderReportDialog();
}

/**
 * Send the open report, then Block when "Also block them" is ticked. To the
 * admins: signed, as `report_v2`. To a group's creator (10j): a sealed DM
 * flagged `group_report` (crypto.js pqBuildGroupReport). For "Both", both are
 * built before either is sent, so a report never goes half. Resolves true
 * when the report went out.
 */
async function submitReportDialog() {
  const st = reportDialog;
  if (!st || st.sending) return false;
  const fail = (msg) => { st.sending = false; st.error = msg; if (reportDialog === st) renderReportDialog(); return false; };
  if (!st.reason) return fail('Choose a reason first.');
  const dest = reportDialogDestination(st);
  if (dest === null) return fail('Still finding out who created this group. Try again in a moment.');
  if (!repSocketOpen()) return fail('Not connected to the server, so the report was not sent. Try again once connected.');
  st.sending = true;
  st.error = '';
  renderReportDialog();
  // The note is mine: never my recovery phrase (step F, chat-warnings.js). Said in the dialog.
  if (typeof recoveryPhraseIn === 'function' && await recoveryPhraseIn(st.note)) return fail(PHRASE_GUARD_SENTENCE);
  let toCreator = null;
  if (dest === 'creator' || dest === 'both') {
    toCreator = typeof pqBuildGroupReport === 'function'
      ? await pqBuildGroupReport({
        creator: st.creator, group_id: st.group.id, group_name: st.group.name, target: st.target,
        reason: st.reason, note: st.note, items: st.groupItem ? [st.groupItem] : [],
      })
      : { error: 'Reporting is not loaded on this page.' };
    if (!toCreator || !toCreator.put) return fail((toCreator && toCreator.error) || 'Your report could not be built.');
  }
  let toAdmins = null;
  if (dest === 'admins' || dest === 'both') {
    toAdmins = typeof pqBuildReport === 'function'
      ? await pqBuildReport({ target: st.target, context: st.context, reason: st.reason, note: st.note, evidence: reportDialogEvidence(st) })
      : { error: 'Reporting is not loaded on this page.' };
    if (!toAdmins || !toAdmins.frame) return fail((toAdmins && toAdmins.error) || 'Your report could not be built.');
  }
  if (!repSocketOpen()) return fail('Not connected to the server, so the report was not sent. Try again once connected.');
  if (toCreator) {
    ws.send(JSON.stringify(toCreator.put));
    groupReportsSent.set(st.creator, Date.now());
    repSay(GROUP_REPORT_SENT_LINE);
  }
  if (toAdmins) ws.send(JSON.stringify(toAdmins.frame));
  if (reportDialog === st) closeReportDialog();
  if (st.alsoBlock && !st.alreadyBlocked && typeof blockKey === 'function') await blockKey(st.target);
  return true;
}

// A report to a group's creator that the server did not let through comes
// back as `reach_refused` for a message to them, with nothing else to tell it
// from an ordinary message; one sent in the last minute takes it, so the
// reporter is told about the report instead of being offered a contact request.
const groupReportsSent = new Map(); // creator key -> when the report went
const GROUP_REPORT_REFUSAL_WINDOW_MS = 60000;

function groupReportRefused(msg) {
  if (!msg || msg.kind !== 'message' || typeof msg.to !== 'string') return false;
  const to = msg.to.toLowerCase();
  const at = groupReportsSent.get(to);
  if (!at || Date.now() - at > GROUP_REPORT_REFUSAL_WINDOW_MS) return false;
  groupReportsSent.delete(to);
  repSay(GROUP_REPORT_REFUSED_LINE);
  return true;
}

/** The DM header's Report (chat-dms.js). */
function reportActiveDm() {
  if (typeof activeDmPartner === 'undefined' || !activeDmPartner) return null;
  return openReportDialog({ target: activeDmPartner, name: (typeof activeDmPartnerName === 'string' && activeDmPartnerName) || repName(activeDmPartner), context: 'dm' });
}

/** `/report <name>`: the member list's name (letter case aside) or a whole key. */
function reportByName(name) {
  const key = typeof blockKeyForName === 'function' ? blockKeyForName(name, false) : null;
  if (!key) { repSay(`No one called "${name}" is in the member list.`); return Promise.resolve(null); }
  return openReportDialog({ target: key, context: 'profile' });
}

// ── The Reports view (admins and mods) ───────────────────────────────────

const reportsView = {
  open: false,
  tab: 'open',                         // 'open' | 'decided'
  lists: { open: null, decided: null }, // reportItemView[] per list, null until the relay answers
  requested: [],                       // the lists asked for and not answered yet, oldest first
  notes: {},                           // report id -> the decision note being typed
};

function reportsMyRole() {
  const peers = (typeof peerData !== 'undefined' && peerData) ? peerData : {};
  const me = typeof myKey === 'string' ? peers[myKey] : null;
  return (me && me.role) || window.myPeerRole || '';
}

function reportsAmStaff() {
  const r = reportsMyRole();
  return r === 'admin' || r === 'mod';
}

/** Ask the relay for one list (`reports_list`). */
function requestReports(state) {
  const which = state === 'decided' ? 'decided' : 'open';
  if (!repSocketOpen()) return false;
  ws.send(JSON.stringify({ type: 'reports_list', state: which }));
  reportsView.requested.push(which);
  return true;
}

/** Open the Reports view (the command palette, /reports). */
function openReportsView() {
  if (!reportsAmStaff()) {
    repSay("Reports are for this server's admins and moderators.");
    return false;
  }
  reportsView.open = true;
  renderReportsView();
  if (!requestReports(reportsView.tab)) repSay('Not connected to the server, so the reports cannot be listed yet.');
  return true;
}

function closeReportsView() {
  reportsView.open = false;
  renderReportsView();
}

function reportsShowTab(tab) {
  reportsView.tab = tab === 'decided' ? 'decided' : 'open';
  renderReportsView();
  requestReports(reportsView.tab);
}

/**
 * The relay's `reports` frame. 10e's answer names no list, so which one it is
 * comes from the frame's `state` if it has one, else from its reports' own
 * states when they agree, else from the order the lists were asked for.
 */
function onReportsFrame(msg) {
  const asked = reportsView.requested.shift();
  const items = (msg && Array.isArray(msg.items)) ? msg.items.map(reportItemView) : [];
  const states = new Set(items.map((v) => v.state));
  let which = asked || reportsView.tab;
  if (msg && (msg.state === 'open' || msg.state === 'decided')) which = msg.state;
  else if (states.size === 1 && (states.has('open') || states.has('decided'))) which = [...states][0];
  reportsView.lists[which] = items;
  renderReportsView();
}

/**
 * Decide a report (`report_decide`); the relay carries it out through its
 * moderation path and its rules, and records who decided and when. The open
 * list is asked for again so the report moves to Decided.
 */
function decideReport(id, decision) {
  if (!REPORT_DECISIONS.includes(decision)) return false;
  if (!repSocketOpen()) { repSay('Not connected to the server, so nothing was decided.'); return false; }
  const note = reportNoteNorm(reportsView.notes[String(id)] || '');
  ws.send(JSON.stringify({ type: 'report_decide', id, decision, note }));
  delete reportsView.notes[String(id)];
  requestReports(reportsView.tab);
  return true;
}

/** What the Reports view shows: names, labels and badges worked out (pure apart from the member list). */
function reportsViewModel() {
  const items = reportsView.lists[reportsView.tab];
  const reasonLabel = (id) => {
    const r = (reportReasons || []).find((x) => x.id === id);
    return r ? r.label : id;
  };
  return {
    tab: reportsView.tab,
    loading: items === null,
    items: (items || []).map((v) => {
      const targetLabel = v.targetName || repName(v.target);
      return {
        ...v,
        targetLabel,
        reporterLabel: v.reporter ? (v.reporterName || repName(v.reporter)) : 'a member',
        reviewerLabel: v.reviewer ? (v.reviewerName || repName(v.reviewer)) : (v.reviewerName || ''),
        reasonLabel: reasonLabel(v.reason),
        contextLabel: REPORT_CONTEXT_LABELS[v.context] || v.context,
        createdLabel: repWhen(v.created),
        decidedLabel: repWhen(v.decided),
        decisionLabel: REPORT_DECISION_LABELS[v.decision] || v.decision,
        decisions: REPORT_DECISIONS.filter((d) => d !== 'delete_post' || v.context === 'post'),
        evidence: v.evidence.map((ev) => {
          const fromLabel = repSameKey(ev.from, v.target) ? targetLabel : (ev.from ? repName(ev.from) : targetLabel);
          return { ...ev, fromLabel, badge: reportEvidenceBadge(ev, fromLabel), timeLabel: repWhen(ev.ts), textLabel: repTextLabel(ev.text) };
        }),
      };
    }),
  };
}

/** The Reports view's HTML for a model (pure, so it can be checked without a browser). */
function reportsViewHtml(model) {
  const muted = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';
  const tabBtn = (tab, label) => `<button class="vr-btn" data-reports-tab="${tab}" aria-pressed="${model.tab === tab}" style="font-size:0.75rem;${model.tab === tab ? 'border-color:var(--accent);color:var(--accent);' : ''}">${label}</button>`;
  let html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
    + '<h2 style="margin:0;">Reports</h2>'
    + '<button class="vr-btn" data-reports-close style="font-size:0.75rem;">Close</button></div>'
    + `<p style="${muted}margin-top:var(--space-sm);">Reports members sent to this server's admins and moderators. The reported person is never told who reported them.</p>`
    + `<p class="reports-limits" style="${muted}border-left:3px solid var(--info);padding-left:var(--space-sm);">${repEsc(REPORT_SIGNATURE_LIMITS)}</p>`
    + '<div style="display:flex;gap:var(--space-sm);margin:var(--space-sm) 0;">'
    + tabBtn('open', 'Open') + tabBtn('decided', 'Decided')
    + '<button class="vr-btn" data-reports-refresh style="font-size:0.75rem;margin-left:auto;">Refresh</button></div>';
  if (model.loading) return html + `<p style="${muted}">Asking the server...</p>`;
  if (!model.items.length) return html + `<p style="${muted}">${model.tab === 'open' ? 'No open reports.' : 'No decided reports.'}</p>`;
  for (const it of model.items) {
    const id = repEsc(String(it.id));
    html += `<div class="report-item" data-report-id="${id}" style="border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-md);margin-bottom:var(--space-md);">`
      + `<div style="font-weight:600;color:var(--text);">${repEsc(it.targetLabel)}: ${repEsc(it.reasonLabel)}</div>`
      + `<div style="${muted}margin:var(--space-xs) 0;">${repEsc(it.contextLabel)} · reported by ${repEsc(it.reporterLabel)}${it.createdLabel ? ' · ' + repEsc(it.createdLabel) : ''}</div>`;
    if (it.note) html += `<div style="color:var(--text);margin:var(--space-xs) 0;white-space:pre-wrap;overflow-wrap:anywhere;"><span style="color:var(--text-muted);">Their note: </span>${repEsc(it.note)}</div>`;
    if (it.evidence.length) {
      html += it.evidence.map((ev) =>
        `<div class="report-evidence-item" data-checked="${ev.badge.checked}" style="border-top:1px solid var(--border);padding:var(--space-xs) 0;">`
        + `<span class="report-badge ${ev.badge.checked ? 'report-badge-checked' : 'report-badge-unproven'}" style="display:inline-block;font-size:var(--text-xs);padding:1px var(--space-sm);border-radius:var(--radius-sm);border:1px solid ${ev.badge.checked ? 'var(--success)' : 'var(--warning)'};color:${ev.badge.checked ? 'var(--success)' : 'var(--warning)'};">${repEsc(ev.badge.text)}</span>`
        + (ev.timeLabel ? ` <span style="color:var(--text-muted);font-size:var(--text-xs);">${repEsc(ev.timeLabel)}</span>` : '')
        + `<div style="color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;margin-top:2px;">${repEsc(ev.textLabel)}</div></div>`).join('');
    } else {
      html += `<div style="${muted}">No messages were included.</div>`;
    }
    if (model.tab === 'open') {
      html += `<input type="text" data-report-decide-note="${id}" maxlength="${REPORT_NOTE_MAX}" placeholder="A note on your decision (optional)" aria-label="A note on your decision" style="width:100%;box-sizing:border-box;margin:var(--space-sm) 0;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-xs) var(--space-sm);">`
        + '<div style="display:flex;flex-wrap:wrap;gap:var(--space-xs);">'
        + it.decisions.map((d) => `<button class="vr-btn" data-report-decide="${id}" data-decision="${d}" style="font-size:0.75rem;${d === 'dismiss' ? '' : 'color:var(--danger);'}">${repEsc(REPORT_DECISION_LABELS[d])}</button>`).join('')
        + '</div>';
    } else {
      html += `<div class="report-decision" style="${muted}margin-top:var(--space-sm);">Decided: ${repEsc(it.decisionLabel || 'unknown')}`
        + (it.reviewerLabel ? ` by ${repEsc(it.reviewerLabel)}` : '')
        + (it.decidedLabel ? `, ${repEsc(it.decidedLabel)}` : '')
        + (it.decisionNote ? `. ${repEsc(it.decisionNote)}` : '') + '</div>';
    }
    html += '</div>';
  }
  return html;
}

/** Draw the Reports view (or take it down). */
function renderReportsView() {
  if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
  let overlay = document.getElementById('reports-overlay');
  if (!reportsView.open) {
    if (overlay && overlay.classList) overlay.classList.remove('open');
    return;
  }
  let card = document.getElementById('reports-card');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'reports-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.style.zIndex = '10000';
    overlay.onclick = (e) => { if (e && e.target === overlay) closeReportsView(); };
    card = document.createElement('div');
    card.id = 'reports-card';
    card.className = 'profile-modal';
    card.setAttribute('role', 'dialog');
    card.setAttribute('aria-label', 'Reports');
    card.onclick = (e) => { if (e) e.stopPropagation(); };
    overlay.appendChild(card);
    if (document.body) document.body.appendChild(overlay);
  }
  if (overlay.classList) overlay.classList.add('open');
  if (!card) return;
  const model = reportsViewModel();
  card.innerHTML = reportsViewHtml(model);
  if (typeof card.querySelectorAll !== 'function') return;
  const close = card.querySelector('[data-reports-close]');
  if (close) close.onclick = () => closeReportsView();
  card.querySelectorAll('[data-reports-tab]').forEach((b) => { b.onclick = () => reportsShowTab(b.dataset.reportsTab); });
  const refresh = card.querySelector('[data-reports-refresh]');
  if (refresh) refresh.onclick = () => requestReports(reportsView.tab);
  card.querySelectorAll('[data-report-decide-note]').forEach((inp) => {
    inp.value = reportsView.notes[inp.dataset.reportDecideNote] || '';
    inp.oninput = () => { reportsView.notes[inp.dataset.reportDecideNote] = inp.value; };
  });
  card.querySelectorAll('[data-report-decide]').forEach((b) => {
    b.onclick = () => {
      const item = model.items.find((x) => String(x.id) === b.dataset.reportDecide);
      if (!item) return;
      b.disabled = true;
      decideReport(item.id, b.dataset.decision);
    };
  });
}

// ── Reports about my groups (10j, the creator's side) ────────────────────
// A member of a group I created sent me a report as a sealed DM. It is
// opened and its signature checked like any DM (app.js); here it is read
// (/shared/group-report.js), dropped unless it is about a group I hold as its
// creator and comes from someone in it, its items checked against my own copy
// of the group (chat-groups-p2p.js checkGroupReportItems), and kept in the
// local store, encrypted, until I dismiss it: never sent to any server. Shown
// in Settings > Safety under "Reports about your groups", with a count on the
// group in the group list, and the actions Remove them from the group (a new
// group key for everyone else, then the same signed remove a leave uses),
// Block them, Dismiss.

function repStore() {
  return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
}

/**
 * A DM (opened, its signature checked) whose text is a report about a group:
 * returns true when it was one, and then the caller neither stores nor shows
 * it as a message. Kept only when it passes the creator's checks.
 */
async function ingestGroupReport(inner) {
  if (!inner || typeof isGroupReportText !== 'function' || !isGroupReportText(inner.text)) return false;
  // My own, from this or another of my devices: nothing to keep.
  if (repSameKey(inner.from, repMe())) return true;
  const report = groupReportParse(inner.text);
  const store = repStore();
  if (!report || !store || typeof inner.sig !== 'string' || !inner.sig) return true;
  // One record per report, however often the mailbox hands it over.
  const id = await store._sha256hex('group-report\n' + inner.sig);
  if (store.groupReports[id]) return true;
  // My groups as the server lists them now: who is in which, and which I created.
  if (typeof loadP2pGroups === 'function') {
    try { await loadP2pGroups(); } catch { /* the list as it was */ }
  }
  const verdict = groupReportAccepts({
    me: repMe(), from: inner.from, to: inner.to, report,
    groups: Array.isArray(window._p2pGroups) ? window._p2pGroups : [],
  });
  if (!verdict.ok) return true;
  const items = typeof checkGroupReportItems === 'function'
    ? await checkGroupReportItems(report)
    : report.items.map((it) => ({ ...it, found: false, signer: '' }));
  const rec = {
    id,
    from: String(inner.from).toLowerCase(),
    ts: Number(inner.ts) || Date.now(),
    group_id: report.group_id,
    // The name my own group list has for it, not the one the report claims.
    group_name: verdict.group.name || report.group_name,
    target: report.target,
    reason: report.reason,
    note: report.note,
    items,
    removed: false,
  };
  if (store.addGroupReport(rec)) {
    repSay(groupReportArrivedLine(repName(rec.from), repName(rec.target), rec.group_name));
    if (typeof notifyNewMessage === 'function') notifyNewMessage(repName(rec.from), 'A report about ' + rec.group_name, true);
    loadReportReasons().then(() => renderSafetyPanelIfOpen());
  }
  renderGroupReportsEverywhere();
  return true;
}

/** How many kept reports are about this group (the count on it in the group list). */
function groupReportCountFor(groupId) {
  const store = repStore();
  if (!store || typeof groupReportCount !== 'function') return 0;
  return groupReportCount(store.groupReportList(), groupId);
}

function renderSafetyPanelIfOpen() {
  if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
}

function renderGroupReportsEverywhere() {
  if (typeof renderGroupList === 'function') {
    try { renderGroupList(); } catch { /* the group list is not drawn yet */ }
  }
  renderSafetyPanelIfOpen();
}

/**
 * What Settings > Safety shows under "Reports about your groups", or null
 * when there is nothing to show (no reports, and no group I created).
 */
function groupReportsModel() {
  const store = repStore();
  if (!store || typeof GROUP_REPORTS_TITLE !== 'string') return null;
  const list = store.groupReportList();
  const groups = Array.isArray(window._p2pGroups) ? window._p2pGroups : [];
  if (!list.length && !groups.some((g) => g && g.is_creator)) return null;
  const reasonLabel = (id) => {
    const r = (reportReasons || []).find((x) => x.id === id);
    return r ? r.label : id;
  };
  return {
    items: list.map((r) => {
      const group = groups.find((g) => g && g.group_id === r.group_id) || null;
      const inGroup = !!(group && Array.isArray(group.members) && group.members.some((m) => repSameKey(m, r.target)));
      return {
        id: r.id,
        groupName: r.group_name || 'A group',
        targetName: repName(r.target),
        reporterName: repName(r.from),
        reasonLabel: reasonLabel(r.reason),
        note: r.note || '',
        when: repWhen(r.ts),
        removed: !!r.removed,
        inGroup,
        blocked: typeof isBlockedKey === 'function' && isBlockedKey(r.target),
        items: (r.items || []).map((it) => ({
          ...it,
          badge: groupReportItemBadge(it, repName(it.signer || it.from)),
          timeLabel: repWhen(it.ts),
          textLabel: repTextLabel(it.text),
        })),
      };
    }),
  };
}

/** The Safety section's HTML for a model (pure, so it can be checked without a browser). */
function groupReportsSafetyHtml(model) {
  if (!model) return '';
  const h3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
  const muted = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';
  const btn = 'font-size:0.7rem;';
  let html = `<h3 style="${h3}">${repEsc(GROUP_REPORTS_TITLE)}</h3>`
    + `<p style="${muted}">${repEsc(GROUP_REPORTS_HELP)}</p>`;
  if (!model.items.length) return html + `<p class="group-reports-none" style="${muted}">${repEsc(GROUP_REPORTS_NONE)}</p>`;
  for (const it of model.items) {
    const id = repEsc(it.id);
    html += `<div class="group-report" data-group-report-id="${id}" style="border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-sm) var(--space-md);margin-bottom:var(--space-sm);">`
      + `<div style="font-weight:600;color:var(--text);overflow-wrap:anywhere;">${repEsc(it.targetName)}: ${repEsc(it.reasonLabel)}</div>`
      + `<div style="${muted}margin:var(--space-xs) 0;overflow-wrap:anywhere;">In ${repEsc(it.groupName)} · reported by ${repEsc(it.reporterName)}${it.when ? ' · ' + repEsc(it.when) : ''}</div>`;
    if (it.note) html += `<div style="color:var(--text);margin:var(--space-xs) 0;white-space:pre-wrap;overflow-wrap:anywhere;"><span style="color:var(--text-muted);">Their note: </span>${repEsc(it.note)}</div>`;
    if (it.items.length) {
      html += it.items.map((ev) =>
        `<div class="group-report-item" data-found="${ev.badge.found}" style="border-top:1px solid var(--border);padding:var(--space-xs) 0;">`
        + `<span class="report-badge ${ev.badge.found ? 'report-badge-checked' : 'report-badge-unproven'}" style="display:inline-block;font-size:var(--text-xs);padding:1px var(--space-sm);border-radius:var(--radius-sm);border:1px solid ${ev.badge.found ? 'var(--success)' : 'var(--warning)'};color:${ev.badge.found ? 'var(--success)' : 'var(--warning)'};">${repEsc(ev.badge.text)}</span>`
        + (ev.timeLabel ? ` <span style="color:var(--text-muted);font-size:var(--text-xs);">${repEsc(ev.timeLabel)}</span>` : '')
        + `<div style="color:var(--text);white-space:pre-wrap;overflow-wrap:anywhere;margin-top:2px;">${repEsc(ev.textLabel)}</div></div>`).join('');
    } else {
      html += `<div style="${muted}">${repEsc(GROUP_REPORT_NO_ITEMS)}</div>`;
    }
    html += '<div style="display:flex;flex-wrap:wrap;align-items:center;gap:var(--space-xs) var(--space-sm);margin-top:var(--space-sm);">';
    if (it.removed) html += `<span class="group-report-removed" style="${muted}margin:0;">${repEsc(GROUP_REPORT_REMOVED)}</span>`;
    else if (it.inGroup) html += `<button class="vr-btn" data-group-report-remove="${id}" style="${btn}color:var(--danger);">${repEsc(GROUP_REPORT_ACTION_LABELS.remove)}</button>`;
    else html += `<span class="group-report-not-in" style="${muted}margin:0;">${repEsc(GROUP_REPORT_NOT_IN_GROUP)}</span>`;
    if (it.blocked) html += `<span class="group-report-blocked" style="${muted}margin:0;">${repEsc(GROUP_REPORT_BLOCKED)}</span>`;
    else html += `<button class="vr-btn" data-group-report-block="${id}" style="${btn}color:var(--danger);">${repEsc(GROUP_REPORT_ACTION_LABELS.block)}</button>`;
    html += `<button class="vr-btn" data-group-report-dismiss="${id}" style="${btn}">${repEsc(GROUP_REPORT_ACTION_LABELS.dismiss)}</button>`;
    html += '</div></div>';
  }
  return html;
}

/** Hook the section's buttons inside `container` (the Safety page). */
function wireGroupReportButtons(container) {
  if (!container || typeof container.querySelectorAll !== 'function') return;
  container.querySelectorAll('[data-group-report-remove]').forEach((b) => {
    b.onclick = () => { b.disabled = true; groupReportRemove(b.dataset.groupReportRemove); };
  });
  container.querySelectorAll('[data-group-report-block]').forEach((b) => {
    b.onclick = () => { b.disabled = true; groupReportBlock(b.dataset.groupReportBlock); };
  });
  container.querySelectorAll('[data-group-report-dismiss]').forEach((b) => {
    b.onclick = () => groupReportDismiss(b.dataset.groupReportDismiss);
  });
}

/**
 * Remove them from the group: confirmed with a short hold (they can come back
 * only with a new invite ticket), then a new group key for everyone else and
 * the signed remove (chat-groups-p2p.js removeP2pMember). Resolves true when
 * they were removed.
 */
async function groupReportRemove(id) {
  const store = repStore();
  const rec = store && store.groupReports[id];
  if (!rec) return false;
  const name = repName(rec.target);
  if (typeof holdConfirm === 'function' && !await holdConfirm(groupReportRemoveConfirm(name, rec.group_name), { seconds: 3 })) {
    renderGroupReportsEverywhere();
    return false;
  }
  let ok = false;
  try {
    ok = typeof removeP2pMember === 'function' ? await removeP2pMember(rec.group_id, rec.target) : false;
  } catch (e) {
    repSay(`Could not remove ${name} from ${rec.group_name}: ${(e && e.message) || 'the server refused it'}.`);
  }
  if (ok) {
    store.setGroupReportRemoved(id);
    repSay(`${name} was removed from ${rec.group_name}.`);
  }
  renderGroupReportsEverywhere();
  return ok;
}

/** Block them (chat-privacy.js blockKey, step C). The report stays until dismissed. */
async function groupReportBlock(id) {
  const store = repStore();
  const rec = store && store.groupReports[id];
  if (!rec || typeof blockKey !== 'function') return false;
  const ok = await blockKey(rec.target);
  renderGroupReportsEverywhere();
  return ok;
}

/** Dismiss: the report is gone from this device. Nobody is told. */
function groupReportDismiss(id) {
  const store = repStore();
  const ok = !!(store && store.removeGroupReport(id));
  renderGroupReportsEverywhere();
  return ok;
}

// The relay's frames for this file.
const _origHandleMessageReports = handleMessage;
handleMessage = function (msg) {
  if (msg && msg.type === 'report_received') { repSay(REPORT_RECEIVED_LINE); return; }
  if (msg && msg.type === 'reports') { onReportsFrame(msg); return; }
  // A report to a group's creator the server did not let through (10j).
  if (msg && msg.type === 'reach_refused' && groupReportRefused(msg)) return;
  return _origHandleMessageReports(msg);
};

window.openReportDialog = openReportDialog;
window.submitReportDialog = submitReportDialog;
window.closeReportDialog = closeReportDialog;
window.reportActiveDm = reportActiveDm;
window.reportByName = reportByName;
window.openReportsView = openReportsView;
window.closeReportsView = closeReportsView;
window.decideReport = decideReport;
window.reportDialogSendTo = reportDialogSendTo;
window.ingestGroupReport = ingestGroupReport;
window.groupReportCountFor = groupReportCountFor;
window.groupReportsModel = groupReportsModel;
window.groupReportsSafetyHtml = groupReportsSafetyHtml;
window.wireGroupReportButtons = wireGroupReportButtons;
window.groupReportRemove = groupReportRemove;
window.groupReportBlock = groupReportBlock;
window.groupReportDismiss = groupReportDismiss;
