// ── backups-admin.js ──────────────────────────────────────────────────────
// The admin page's Backups panel (2026-10-22): the web mirror of the desktop
// app's Server Settings > ADMIN > Backups (src/gui/pages/server_settings/
// continuous_backup.rs). Both talk to the relay's one endpoint,
// POST /api/admin/backups (src/relay/backup_stream/api.rs), so they show and do
// the same things:
//
// - the copies in the server's backups folder, and Back up now (a sealed copy
//   there, and a fresh full copy to the continuous backup's destination);
// - continuous backup: where the copies go, how far behind, what the
//   destination can restore (from when to when), its size, the history kept,
//   the backup key's fingerprint, and the Restore drill (the server restores
//   from the destination into a scratch file and compares it with the live
//   database, which it only reads);
// - the destination editor: a folder, an S3-compatible bucket, or another
//   HumanityOS server, tested before it is saved. A secret left empty keeps the
//   saved one; the server never sends a secret back;
// - which other servers may keep their backups here, with the space each gets.
//
// Every request is signed with the admin's identity (getPqSignedAuth, from
// /shared/pq-relay-auth.js) over `admin_backups\n<action>\n<SHA-256 hex of the
// params text>`, so a signature for one request cannot carry other settings.
// The params go in the JSON body, never in the address.
(function (root) {
  'use strict';

  function hex(buf) {
    return Array.from(new Uint8Array(buf)).map(function (b) { return b.toString(16).padStart(2, '0'); }).join('');
  }

  /** SHA-256 of a text, hex. */
  async function sha256Hex(text) {
    const subtle = (root.crypto && root.crypto.subtle) || (typeof require === 'function' ? require('crypto').webcrypto.subtle : null);
    return hex(await subtle.digest('SHA-256', new TextEncoder().encode(text)));
  }

  /** The words the admin signs for one request (the relay's `api::purpose`). */
  async function purpose(action, params) {
    return 'admin_backups\n' + action + '\n' + (await sha256Hex(params || ''));
  }

  /** The destination as the form holds it, in the relay's shape (null when Off). */
  function destinationFromForm(f) {
    switch (f.kind) {
      case 'folder':
        return { kind: 'folder', path: (f.path || '').trim() };
      case 's3':
        return {
          kind: 's3',
          endpoint: (f.endpoint || '').trim(),
          region: (f.region || '').trim(),
          bucket: (f.bucket || '').trim(),
          prefix: (f.prefix || '').trim(),
          access_key_id: (f.access_key_id || '').trim(),
          secret_access_key: f.secret_access_key || '',
          path_style: f.path_style !== false,
        };
      case 'server':
        return { kind: 'server', url: (f.url || '').trim() };
      default:
        return null;
    }
  }

  /** The settings text a Save sends; null and a reason when a number is not a whole number. */
  function settingsParams(f) {
    const n = function (v) { return /^\s*\d+\s*$/.test(String(v)) ? parseInt(v, 10) : null; };
    const r = n(f.retention_hours), s = n(f.snapshot_every_hours), a = n(f.alert_after_minutes);
    if (r === null || s === null || a === null) return { error: 'Hours and minutes are whole numbers.' };
    return {
      params: JSON.stringify({
        enabled: f.kind !== 'off',
        // Off sends no destination: the relay keeps the saved one for next time.
        destination: destinationFromForm(f),
        retention_hours: r,
        snapshot_every_hours: s,
        alert_after_minutes: a,
      }),
    };
  }

  function utc(ms) {
    if (!ms) return 'never';
    return new Date(ms).toISOString().replace('T', ' ').replace(/\.\d+Z$/, ' UTC');
  }

  function ago(now, then) {
    if (!then) return 'never';
    const s = Math.max(0, Math.floor((now - then) / 1000));
    if (s < 90) return s + ' s ago';
    if (s < 90 * 60) return Math.floor(s / 60) + ' min ago';
    if (s < 48 * 3600) return Math.floor(s / 3600) + ' h ago';
    return Math.floor(s / 86400) + ' days ago';
  }

  /** How far behind, as the relay words it: seconds under two minutes, then minutes, then hours. */
  function lagWords(s) {
    if (!s) return 'nothing';
    if (s < 120) return s + ' s';
    if (s < 7200) return Math.floor(s / 60) + ' min';
    return Math.floor(s / 3600) + ' h';
  }

  function bytes(n) {
    n = n || 0;
    if (n >= 1073741824) return (n / 1073741824).toFixed(1) + ' GB';
    if (n >= 1048576) return (n / 1048576).toFixed(1) + ' MB';
    if (n >= 1024) return Math.round(n / 1024) + ' KB';
    return n + ' bytes';
  }

  const STATES = {
    up_to_date: ['Up to date', 'ok'],
    catching_up: ['Sending', 'info'],
    starting: ['Starting', 'info'],
    failing: ['Retrying', 'warn'],
    behind: ['Behind', 'err'],
    refused: ['Refused', 'err'],
    off: ['Off', 'muted'],
  };

  /** What the panel shows for a status: the label, its tone and the detail rows. */
  function describe(status, now) {
    const st = (status && status.stream) || {};
    const word = STATES[st.state] ? st.state : 'off';
    const rows = [];
    if (word !== 'off') {
      const dest = status.config && status.config.destination;
      rows.push(['Where the copies go', dest && dest.summary ? dest.summary : 'nowhere yet']);
      rows.push(['Newest copy there', st.restorable_to_ms ? utc(st.restorable_to_ms) + ' (' + ago(now, st.restorable_to_ms) + ')' : 'none yet']);
      rows.push(['Behind by', lagWords(st.lag_secs)]);
      rows.push(['Can restore', st.restorable_from_ms ? 'any moment from ' + utc(st.restorable_from_ms) + ' to ' + utc(st.restorable_to_ms) : 'not yet']);
      rows.push(['At the destination', bytes(st.dest_bytes) + ' in ' + (st.dest_objects || 0) + ' objects, ' + (st.snapshots || 0) + ' full copies']);
      rows.push(['Waiting to send', (st.pending_objects || 0) + ' parts (' + bytes(st.pending_bytes) + ')']);
      rows.push(['History kept', (st.retention_hours || 0) + ' hours; a full copy every ' + (st.snapshot_every_hours || 0) + ' hours; alert after ' + (st.alert_after_minutes || 0) + ' min behind']);
      rows.push(['Last copy sent', ago(now, st.last_copy_ms)]);
      rows.push(['Backup key', st.key_fingerprint || '']);
      if (st.last_alert) rows.push(['Last alert', st.last_alert + ' (' + ago(now, st.last_alert_ms) + ')']);
    }
    return { label: STATES[word][0], tone: STATES[word][1], words: st.words || '', rows: rows };
  }

  const api = { sha256Hex, purpose, destinationFromForm, settingsParams, describe, utc, ago, bytes };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
    return;
  }
  root.HosBackupsAdmin = api;

  // ── The page ──────────────────────────────────────────────────────────────

  let last = null;
  let busy = false;
  let timer = null;
  let editorLoaded = false;

  function $(id) { return document.getElementById(id); }

  function esc(s) {
    const el = document.createElement('span');
    el.textContent = s == null ? '' : String(s);
    return el.innerHTML;
  }

  function say(msg, ok) {
    const el = $('bk-message');
    if (!el) return;
    el.textContent = msg || '';
    el.className = 'bk-message' + (msg ? (ok ? ' ok' : ' err') : '');
  }

  async function call(action, params, working) {
    if (typeof root.getPqSignedAuth !== 'function') {
      say('Signing is not loaded on this page.', false);
      return null;
    }
    params = params || '';
    const auth = await root.getPqSignedAuth(await purpose(action, params));
    if (!auth) {
      say('No Humanity identity found in this browser. Sign in via Chat first.', false);
      return null;
    }
    busy = true;
    setButtons();
    if (working) say(working, true);
    try {
      const res = await fetch('/api/admin/backups', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ key: auth.key, timestamp: auth.timestamp, sig: auth.sig, action: action, params: params }),
      });
      let data = null;
      try { data = await res.json(); } catch (e) { data = null; }
      if (!data) {
        say(res.status === 404 ? 'This server does not have continuous backup yet (it needs an update).' : 'The server answered HTTP ' + res.status + '.', false);
        return null;
      }
      if (data.status) render(data.status);
      if (data.message) say(data.message, !!data.ok);
      else if (working) say('', true);
      return data;
    } catch (e) {
      say('Network error: ' + (e && e.message ? e.message : e), false);
      return null;
    } finally {
      busy = false;
      setButtons();
    }
  }

  function setButtons() {
    document.querySelectorAll('#backups-panel button').forEach(function (b) { b.disabled = busy; });
  }

  function form() {
    const kind = (document.querySelector('input[name="bk-kind"]:checked') || {}).value || 'off';
    return {
      kind: kind,
      path: $('bk-path').value,
      endpoint: $('bk-endpoint').value,
      region: $('bk-region').value,
      bucket: $('bk-bucket').value,
      prefix: $('bk-prefix').value,
      access_key_id: $('bk-keyid').value,
      secret_access_key: $('bk-secret').value,
      path_style: $('bk-pathstyle').checked,
      url: $('bk-url').value,
      retention_hours: $('bk-retention').value,
      snapshot_every_hours: $('bk-snapshot').value,
      alert_after_minutes: $('bk-alert').value,
    };
  }

  function showKind() {
    const kind = form().kind;
    ['folder', 's3', 'server', 'off'].forEach(function (k) {
      const el = $('bk-fields-' + k);
      if (el) el.style.display = k === kind ? '' : 'none';
    });
    const test = $('bk-test');
    if (test) test.style.display = kind === 'off' ? 'none' : '';
  }

  function loadEditor(status) {
    const c = status.config || {};
    const d = c.destination || {};
    const kind = c.enabled && d.kind ? d.kind : 'off';
    const radio = document.querySelector('input[name="bk-kind"][value="' + kind + '"]');
    if (radio) radio.checked = true;
    if (d.kind === 'folder') $('bk-path').value = d.path || '';
    if (d.kind === 's3') {
      $('bk-endpoint').value = d.endpoint || '';
      $('bk-region').value = d.region || 'us-east-1';
      $('bk-bucket').value = d.bucket || '';
      $('bk-prefix').value = d.prefix || '';
      $('bk-pathstyle').checked = d.path_style !== false;
      $('bk-keyid').placeholder = d.access_key_hint ? 'saved: ' + d.access_key_hint : 'access key ID';
      $('bk-secret').placeholder = d.secret_set ? 'saved (leave empty to keep it)' : 'secret access key';
    }
    if (d.kind === 'server') $('bk-url').value = d.url || '';
    if (c.retention_hours) $('bk-retention').value = c.retention_hours;
    if (c.snapshot_every_hours) $('bk-snapshot').value = c.snapshot_every_hours;
    if (c.alert_after_minutes) $('bk-alert').value = c.alert_after_minutes;
    showKind();
    editorLoaded = true;
  }

  function render(status) {
    last = status;
    const now = Date.now();
    const d = describe(status, now);
    $('bk-state').innerHTML = '<span class="bk-dot ' + d.tone + '"></span><strong class="bk-' + d.tone + '">' + esc(d.label) + '</strong> <span class="bk-words">' + esc(d.words) + '</span>';
    $('bk-rows').innerHTML = d.rows.map(function (r) {
      return '<tr><td class="bk-k">' + esc(r[0]) + '</td><td>' + esc(r[1]) + '</td></tr>';
    }).join('');
    const drill = (status.stream && status.stream.drill) || {};
    const dEl = $('bk-drill');
    dEl.textContent = drill.words ? 'Restore drill: ' + drill.words : '';
    dEl.className = 'bk-drill' + (drill.ok === true ? ' bk-ok' : drill.ok === false ? ' bk-err' : '');
    $('bk-drill-btn').style.display = d.label === 'Off' ? 'none' : '';
    // The backups folder.
    const local = status.local_backups || [];
    $('bk-local').innerHTML = local.length
      ? local.map(function (b) {
          return '<tr><td>' + esc(b.file) + '</td><td>' + esc(bytes(b.size_bytes)) + '</td><td>' + esc(ago(now, b.modified_epoch * 1000)) + '</td></tr>';
        }).join('')
      : '<tr><td colspan="3" class="bk-muted">No copies in the backups folder yet. Back up now makes one.</td></tr>';
    $('bk-local-note').textContent = 'Back up now keeps the newest ' + (status.manual_kept || 10) + ' of its own copies; the scheduled ones rotate on their own.';
    // Whose backups this server keeps.
    const key = status.sending_key || '';
    $('bk-sending').textContent = key ? key.slice(0, 16) + '...' + key.slice(-16) + ' (' + key.length + ' characters)' : '';
    const senders = (status.host && status.host.senders) || [];
    $('bk-senders').innerHTML = senders.length
      ? senders.map(function (s) {
          return '<div class="bk-sender"><strong>' + esc(s.label) + '</strong> <span class="bk-muted">' + esc(bytes(s.used_bytes)) + ' of ' + esc(bytes(s.quota_bytes)) + ' used, ' + esc(s.objects) + ' objects</span> '
            + '<button type="button" data-bk-keep="' + esc(s.fingerprint) + '">Stop, keep their copies</button> '
            + '<button type="button" class="danger" data-bk-delete="' + esc(s.fingerprint) + '">Stop and delete their copies</button></div>';
        }).join('')
      : '<p class="bk-muted">No other server keeps its backups here.</p>';
    document.querySelectorAll('[data-bk-keep]').forEach(function (b) {
      b.addEventListener('click', function () { removeSender(b.getAttribute('data-bk-keep'), false); });
    });
    document.querySelectorAll('[data-bk-delete]').forEach(function (b) {
      b.addEventListener('click', function () { removeSender(b.getAttribute('data-bk-delete'), true); });
    });
    if (!editorLoaded) loadEditor(status);
    setButtons();
    schedule();
  }

  function removeSender(fp, del) {
    const what = del ? 'Stop keeping this server\'s backups AND delete every copy kept here? This cannot be undone.' : 'Stop keeping this server\'s backups? The copies already here stay.';
    if (!root.confirm(what)) return;
    call('host_remove', JSON.stringify({ fingerprint: fp, delete_copies: !!del }), 'Removing...');
  }

  function schedule() {
    if (timer) clearTimeout(timer);
    const drilling = last && last.stream && last.stream.drill && last.stream.drill.running;
    timer = setTimeout(function () {
      if (document.visibilityState === 'visible' && !busy) call('status', '', '');
      else schedule();
    }, drilling ? 2000 : 10000);
  }

  function wire() {
    $('bk-refresh').addEventListener('click', function () { call('status', '', 'Refreshing...'); });
    $('bk-now').addEventListener('click', function () { call('back_up_now', '', 'Backing up...'); });
    $('bk-drill-btn').addEventListener('click', function () { call('restore_drill', '', 'Starting the restore drill...'); });
    document.querySelectorAll('input[name="bk-kind"]').forEach(function (r) { r.addEventListener('change', showKind); });
    $('bk-test').addEventListener('click', function () {
      call('test_destination', JSON.stringify({ destination: destinationFromForm(form()) }), 'Testing...');
    });
    $('bk-save').addEventListener('click', function () {
      const p = settingsParams(form());
      if (p.error) { say(p.error, false); return; }
      $('bk-secret').value = '';
      $('bk-keyid').value = '';
      editorLoaded = false;
      call('set_destination', p.params, 'Testing and saving...');
    });
    $('bk-copy').addEventListener('click', function () {
      const key = last && last.sending_key;
      if (!key) return;
      navigator.clipboard.writeText(key).then(function () { say('Copied the backup sending key.', true); }, function () { say('Could not copy: select it from the desktop app instead.', false); });
    });
    $('bk-allow').addEventListener('click', function () {
      const gb = parseFloat($('bk-host-gb').value);
      if (!(gb > 0)) { say('Give the space in GB, more than 0.', false); return; }
      const params = JSON.stringify({ label: $('bk-host-label').value.trim(), public_key: $('bk-host-key').value.trim(), quota_gb: gb });
      $('bk-host-key').value = '';
      call('host_allow', params, 'Adding...');
    });
  }

  function init() {
    if (!$('backups-panel')) return;
    wire();
    showKind();
    call('status', '', '');
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})(typeof window !== 'undefined' ? window : this);
