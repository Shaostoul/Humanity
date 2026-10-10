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
  reportDialog = state;
  renderReportDialog();
  const reasons = await loadReportReasons();
  if (reportDialog === state) {
    state.reasons = reasons;
    renderReportDialog();
  }
  return state;
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

/** The dialog's HTML for a state (pure, so it can be checked without a browser). */
function reportDialogHtml(st) {
  const muted = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';
  const h3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
  let html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
    + `<h2 style="margin:0;">Report ${repEsc(st.name)}</h2>`
    + '<button class="vr-btn" data-report-cancel style="font-size:0.75rem;">Close</button></div>'
    + `<p style="${muted}margin-top:var(--space-sm);">This goes to this server's admins and moderators. ${repEsc(st.name)} is not told who reported them.</p>`;

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
    html += st.context === 'post'
      ? `<p style="${muted}margin-top:var(--space-xs);">The server finds this post itself, so the admins see it as it was posted.</p>`
      : `<p style="${muted}margin-top:var(--space-xs);">${repEsc(REPORT_GROUP_UNPROVEN)}</p>`;
  } else if (st.fileLeftOut) {
    html += `<h3 style="${h3}">The group message</h3>`
      + `<p class="report-no-files" style="${muted}">${repEsc(REPORT_NO_FILES)} Describe what happened in the note instead.</p>`;
  }

  html += `<h3 style="${h3}">Anything to add (optional)</h3>`
    + `<textarea data-report-note maxlength="${REPORT_NOTE_MAX}" rows="3" aria-label="A note for the admins" placeholder="What the admins should know" style="width:100%;box-sizing:border-box;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-sm);">${repEsc(st.note)}</textarea>`;

  if (st.alreadyBlocked) {
    html += `<p style="${muted}margin-top:var(--space-sm);">You have blocked them.</p>`;
  } else {
    html += '<label class="report-also-block" style="display:flex;align-items:flex-start;gap:var(--space-sm);margin:var(--space-md) 0 0;cursor:pointer;color:var(--text);font-size:0.9rem;">'
      + `<input type="checkbox" data-report-block${st.alsoBlock ? ' checked' : ''} style="margin-top:3px;">`
      + `<span>Also block them<span style="display:block;${muted}margin:0;">You will not see anything from them, and they are not told.</span></span></label>`;
  }

  if (st.error) html += `<p class="report-error" role="alert" style="color:var(--danger);font-size:var(--text-sm);margin:var(--space-sm) 0 0;">${repEsc(st.error)}</p>`;
  const canSend = !st.sending && !!st.reason && !!(st.reasons && st.reasons.length);
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
 * Send the open report: signed, as `report_v2`, then Block when "Also block
 * them" is ticked. Resolves true when the report went out.
 */
async function submitReportDialog() {
  const st = reportDialog;
  if (!st || st.sending) return false;
  const fail = (msg) => { st.sending = false; st.error = msg; if (reportDialog === st) renderReportDialog(); return false; };
  if (!st.reason) return fail('Choose a reason first.');
  if (!repSocketOpen()) return fail('Not connected to the server, so the report was not sent. Try again once connected.');
  st.sending = true;
  st.error = '';
  renderReportDialog();
  const built = typeof pqBuildReport === 'function'
    ? await pqBuildReport({ target: st.target, context: st.context, reason: st.reason, note: st.note, evidence: reportDialogEvidence(st) })
    : { error: 'Reporting is not loaded on this page.' };
  if (!built || !built.frame) return fail((built && built.error) || 'Your report could not be built.');
  if (!repSocketOpen()) return fail('Not connected to the server, so the report was not sent. Try again once connected.');
  ws.send(JSON.stringify(built.frame));
  if (reportDialog === st) closeReportDialog();
  if (st.alsoBlock && !st.alreadyBlocked && typeof blockKey === 'function') await blockKey(st.target);
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

// The relay's frames for this file.
const _origHandleMessageReports = handleMessage;
handleMessage = function (msg) {
  if (msg && msg.type === 'report_received') { repSay(REPORT_RECEIVED_LINE); return; }
  if (msg && msg.type === 'reports') { onReportsFrame(msg); return; }
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
