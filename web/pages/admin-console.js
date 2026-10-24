/**
 * The admin page's console on the server's action registry (2026-10-22;
 * docs/design/in-app-ops.md). Four panels, the web mirror of the desktop
 * app's Server Settings > ADMIN (src/gui/pages/server_settings/):
 *
 *  - Alert channels: where this server's warnings go (alerts.*). The first
 *    panel that writes. An address or token is typed once and never shown
 *    whole again: the server answers with its host and last characters.
 *  - Federation: peers, trust, paused or linked, when last heard from, and
 *    which channels travel (federation.*).
 *  - Every admin action: drawn from the server's own list
 *    (GET /api/admin/actions), so an action added to the registry appears
 *    here with no change to this file. Runs through the registry, or types
 *    the chat command for one that is still a command.
 *  - The admin log (log.recent).
 *
 * Every action goes through HosAdminActions.adminRun (/shared/admin-actions.js):
 * POST /api/admin/actions/run, signed with this browser's Dilithium3 key.
 * Everything from the server is put in the page as text, never as markup.
 */
(function () {
  'use strict';

  const A = window.HosAdminActions;

  // ── Small DOM helpers (text only) ──

  // `tip: [name, explanation]` sets the shell's rich tooltip outright (data-tip-title,
  // data-tip-desc). A plain `title` is not used: the shell guesses a description from the words in
  // it, and guessed "Sends your current message to the active channel." for Send test (2026-10-23).
  function el(tag, attrs, kids) {
    const n = document.createElement(tag);
    for (const k in (attrs || {})) {
      if (k === 'text') n.textContent = attrs[k];
      else if (k === 'cls') n.className = attrs[k];
      else if (k === 'on') { for (const ev in attrs.on) n.addEventListener(ev, attrs.on[ev]); }
      else if (k === 'tip') { n.setAttribute('data-tip-title', attrs.tip[0]); n.setAttribute('data-tip-desc', attrs.tip[1]); }
      else n.setAttribute(k, attrs[k]);
    }
    for (const c of (kids || [])) if (c) n.appendChild(c);
    return n;
  }
  function btn(label, onClick, cls, explanation) {
    const attrs = { type: 'button', text: label, cls: cls || '', on: { click: onClick } };
    if (explanation) attrs.tip = [label, explanation];
    return el('button', attrs);
  }
  function status(id, words, ok) {
    const s = document.getElementById(id);
    if (!s) return;
    s.textContent = words || '';
    s.className = 'fed-status' + (words ? (ok ? ' ok' : ' err') : '');
  }

  /** Run an action, put its sentence in `statusId`, hand the answer to `then`. */
  async function run(action, inputs, statusId, then, confirm) {
    status(statusId, 'Working...', true);
    try {
      const res = await A.adminRun(action, inputs, confirm);
      // A list answers with what the panel then draws, so its sentence would only repeat it.
      status(statusId, action.endsWith('.list') ? '' : (res.message || 'Done.'), true);
      if (then) then(res);
      if (logOpen && !action.endsWith('.list') && action !== 'log.recent') loadLog();
      return res;
    } catch (e) {
      status(statusId, e && e.message ? e.message : String(e), false);
      return null;
    }
  }

  // ── Alert channels ──

  const HINTS = {
    ntfy: 'https://ntfy.sh/a-long-random-name',
    discord: 'https://discord.com/api/webhooks/...',
    telegram: 'bot token, 123456:ABC-DEF...',
    webhook: 'https://hooks.example.com/...',
  };

  function renderAlerts(view) {
    const box = document.getElementById('alerts-list');
    if (!box) return;
    box.textContent = '';
    const channels = (view && view.channels) || [];
    if (!channels.length) {
      box.appendChild(el('p', { cls: 'fed-help', text: 'No alert channels yet: this server\'s warnings go nowhere until you add one.' }));
    }
    channels.forEach(function (ch) {
      const i = ch.index;
      const on = el('input', { type: 'checkbox', tip: ['On or off', 'On: alerts go to this channel. Off: kept, and sent nothing.'] });
      on.checked = !!ch.enabled;
      on.addEventListener('change', function () {
        run('alerts.set_enabled', { index: i, enabled: on.checked }, 'alerts-status', function (r) { renderAlerts(r.data); });
      });
      let where = A.adminMaskedWords(ch.address);
      if (ch.kind === 'telegram') where += '; chat ' + A.adminMaskedWords(ch.chat_id);
      const editRow = el('div', { cls: 'fed-add-row', style: 'display:none' });
      const card = el('div', { cls: 'fed-card' }, [
        on,
        el('div', {}, [
          el('div', { cls: 'fed-name', text: ch.label }),
          el('div', { cls: 'fed-url', text: ch.kind + ' · ' + where }),
        ]),
        el('div', { cls: 'fed-actions' }, [
          btn('Send test', function () { run('alerts.test', { index: i }, 'alerts-status'); }, '', 'Send one real message to this channel now, to see that it arrives.'),
          btn('Change', function () { editRow.style.display = editRow.style.display === 'none' ? '' : 'none'; }, '', 'Rename it, or give it a new address or token.'),
          btn('Remove', function () {
            if (!window.confirm('Remove the ' + ch.kind + ' channel "' + ch.label + '"? Its address is forgotten.')) return;
            run('alerts.remove', { index: i }, 'alerts-status', function (r) { renderAlerts(r.data); });
          }, 'danger', 'Forget this channel and its address. Asks once.'),
        ]),
      ]);
      const label = el('input', { type: 'text', placeholder: 'new name', autocomplete: 'off' });
      const addr = el('input', { type: 'password', placeholder: 'new address or token', autocomplete: 'off' });
      const chat = el('input', { type: 'text', placeholder: 'new chat number', autocomplete: 'off' });
      editRow.appendChild(label);
      editRow.appendChild(addr);
      if (ch.kind === 'telegram') editRow.appendChild(chat);
      editRow.appendChild(btn('Save', function () {
        const inputs = { index: i };
        if (label.value.trim()) inputs.label = label.value.trim();
        if (addr.value.trim()) inputs.address = addr.value.trim();
        if (chat.value.trim()) inputs.chat_id = chat.value.trim();
        run('alerts.update', inputs, 'alerts-status', function (r) { renderAlerts(r.data); });
      }, '', 'Empty boxes keep what the channel has.'));
      box.appendChild(card);
      box.appendChild(editRow);
    });
  }

  function wireAlertsAdd() {
    const kind = document.getElementById('alert-add-kind');
    const addr = document.getElementById('alert-add-address');
    const chat = document.getElementById('alert-add-chat');
    if (!kind) return;
    const sync = function () {
      addr.placeholder = HINTS[kind.value] || '';
      chat.style.display = kind.value === 'telegram' ? '' : 'none';
    };
    kind.addEventListener('change', sync);
    sync();
    document.getElementById('alert-add-btn').addEventListener('click', function () {
      const inputs = { kind: kind.value, address: addr.value.trim(), enabled: true };
      const label = document.getElementById('alert-add-label').value.trim();
      if (label) inputs.label = label;
      if (kind.value === 'telegram' && chat.value.trim()) inputs.chat_id = chat.value.trim();
      if (!inputs.address) { status('alerts-status', 'Give the address or token.', false); return; }
      run('alerts.add', inputs, 'alerts-status', function (r) {
        addr.value = '';
        document.getElementById('alert-add-label').value = '';
        chat.value = '';
        renderAlerts(r.data);
      });
    });
    document.getElementById('alerts-refresh').addEventListener('click', loadAlerts);
  }

  function loadAlerts() {
    run('alerts.list', {}, 'alerts-status', function (r) { renderAlerts(r.data); });
  }

  // ── Federation ──

  const TIERS = { 0: '0, untrusted', 1: '1, Accord but unverified', 2: '2, verified (federates)', 3: '3, verified and Accord (federates)' };

  function renderFederation(view) {
    const list = document.getElementById('federation-list');
    if (!list) return;
    list.textContent = '';
    const peers = (view && view.peers) || [];
    const key = view && view.own_key;
    const keyBox = document.getElementById('fed-own-key');
    if (keyBox) {
      keyBox.textContent = '';
      if (key) {
        keyBox.appendChild(el('span', { cls: 'fed-tier-note', text: 'This server\'s federation key: ' }));
        keyBox.appendChild(el('code', { text: key.slice(0, 12) + '...' + key.slice(-6) }));
        keyBox.appendChild(btn('Copy', function () {
          if (navigator.clipboard) navigator.clipboard.writeText(key).then(function () { status('fed-status', 'Copied the whole key.', true); });
        }, '', 'Copy the whole key, for another server\'s admin to add this one by key.'));
      }
    }
    if (!peers.length) {
      list.appendChild(el('p', { cls: 'fed-help', text: 'No peers yet. This server works fully on its own until you add one.' }));
    }
    peers.forEach(function (p) {
      const id = p.server_id;
      const sel = el('select', { tip: ['Trust', 'Federation begins at 2: below it, no link is made in either direction.'] });
      [0, 1, 2, 3].forEach(function (t) {
        const o = el('option', { value: String(t), text: TIERS[t] });
        if (t === p.trust_tier) o.selected = true;
        sel.appendChild(o);
      });
      sel.addEventListener('change', function () {
        run('federation.trust', { server_id: id, tier: sel.value }, 'fed-status', function (r) { renderFederation(r.data); });
      });
      const actions = [el('span', { cls: 'fed-tier-note', text: 'Trust' }), sel];
      if (p.paused) {
        actions.push(btn('Resume', function () {
          run('federation.resume', { server_id: id }, 'fed-status', function (r) { renderFederation(r.data); });
        }, '', 'Federate with it again, at the trust it had.'));
      } else {
        actions.push(btn('Pause', function () {
          if (!window.confirm('Pause federating with ' + (p.name || id) + '? An open link closes within 15 seconds; its trust is kept.')) return;
          run('federation.pause', { server_id: id }, 'fed-status', function (r) { renderFederation(r.data); });
        }, '', 'Stop federating with it for now. Asks once.'));
        if (!p.by_key) {
          actions.push(btn('Re-check key', function () {
            run('federation.recheck', { server_id: id }, 'fed-status', function (r) { renderFederation(r.data); });
          }, '', 'Fetch its key again from its own address and pin it, for when its key changed and the link keeps dropping.'));
        }
      }
      actions.push(btn('Remove', function () {
        if (!window.confirm('Stop federating with ' + (p.name || id) + ' and forget it?')) return;
        run('federation.remove', { server_id: id }, 'fed-status', function (r) { renderFederation(r.data); });
      }, 'danger', 'Stop federating and forget it. Asks once.'));
      const dot = el('div', { cls: 'fed-dot ' + (p.linked ? 'online' : (p.paused ? 'paused' : 'offline')) });
      list.appendChild(el('div', { cls: 'fed-card' }, [
        dot,
        el('div', {}, [
          el('div', { cls: 'fed-name', text: (p.name || id) + (p.accord_compliant ? '  (accord)' : '') }),
          el('div', { cls: 'fed-url', text: (p.by_key ? 'added by key, dials in' : p.url) + ' · ' + A.adminPeerState(p) }),
          A.adminPeerLink(p) ? el('div', { cls: 'fed-url', text: A.adminPeerLink(p) }) : null,
        ]),
        el('div', { cls: 'fed-actions' }, actions),
      ]));
    });

    const chans = document.getElementById('fed-channels');
    if (chans) {
      chans.textContent = '';
      ((view && view.channels) || []).forEach(function (c) {
        const box = el('input', { type: 'checkbox' });
        box.checked = !!c.federated;
        box.disabled = c.can_federate === false && !c.federated;
        box.addEventListener('change', function () {
          run('federation.channel', { channel: c.id, federated: box.checked }, 'fed-status', function (r) { renderFederation(r.data); });
        });
        const title = c.can_federate === false ? 'The guaranteed local-only room never federates while that guarantee is on.'
          : 'Ticked: its new messages travel to every linked peer that federates it too.';
        chans.appendChild(el('label', { cls: 'fed-chan', tip: ['#' + c.name, title] }, [box, document.createTextNode(' #' + c.name)]));
      });
    }
    renderSeedServers(peers);
  }

  /**
   * Suggested first peers, read from data/federation/seed_servers.json. The
   * list is DATA, and nothing here joins itself: each entry is a button. A
   * seed list that auto-peered would quietly make whoever ships it the root
   * of the network. A suggestion already in the peer table is hidden.
   */
  async function renderSeedServers(existing) {
    const box = document.getElementById('fed-seed-list');
    if (!box) return;
    let data = null;
    try {
      const res = await fetch('/data/federation/seed_servers.json', { cache: 'no-cache' });
      if (res.ok) data = await res.json();
    } catch (e) { /* handled below */ }
    box.textContent = '';
    if (!data || !Array.isArray(data.servers)) {
      box.appendChild(el('p', { cls: 'fed-help', text: 'No suggestion list shipped with this build. Add a peer by address below.' }));
      return;
    }
    const have = new Set((existing || []).map(function (s) { return String(s.url || '').replace(/\/+$/, ''); }));
    const rows = data.servers.filter(function (s) { return !have.has(String(s.url || '').replace(/\/+$/, '')); });
    if (!rows.length) {
      box.appendChild(el('p', { cls: 'fed-help', text: 'You are already peered with every suggested server.' }));
      return;
    }
    rows.forEach(function (s) {
      box.appendChild(el('div', { cls: 'fed-seed' }, [
        el('div', {}, [
          el('div', { cls: 'fed-name', text: s.name || s.url }),
          el('div', { cls: 'fed-url', text: s.url + (s.operator ? ' · run by ' + s.operator : '') }),
          el('div', { cls: 'fed-desc', text: s.description || '' }),
        ]),
        btn('Add', function () {
          run('federation.add', { url: s.url, name: s.name || '' }, 'fed-status', function (r) { renderFederation(r.data); });
        }),
      ]));
    });
  }

  function wireFederationAdd() {
    const urlBtn = document.getElementById('fed-add-url-btn');
    if (urlBtn) {
      urlBtn.addEventListener('click', function () {
        const url = (document.getElementById('fed-add-url').value || '').trim();
        if (!/^https?:\/\/.+/.test(url)) { status('fed-status', 'Enter a full address starting with http:// or https://', false); return; }
        const inputs = { url: url };
        const name = (document.getElementById('fed-add-url-name').value || '').trim();
        if (name) inputs.name = name;
        run('federation.add', inputs, 'fed-status', function (r) {
          document.getElementById('fed-add-url').value = '';
          document.getElementById('fed-add-url-name').value = '';
          renderFederation(r.data);
        });
      });
    }
    const keyBtn = document.getElementById('fed-add-key-btn');
    if (keyBtn) {
      keyBtn.addEventListener('click', function () {
        const key = (document.getElementById('fed-add-key').value || '').trim().toLowerCase();
        // A server key is Dilithium3 since 2026-09-06: 3,904 hex characters (handlers/federation.rs SERVER_KEY_HEX_LEN).
        if (!/^[0-9a-f]{3904}$/.test(key)) { status('fed-status', 'A server key is 3,904 hexadecimal characters: copy it from the other server\'s Host Node page.', false); return; }
        const inputs = { key: key };
        const name = (document.getElementById('fed-add-key-name').value || '').trim();
        if (name) inputs.name = name;
        run('federation.add_key', inputs, 'fed-status', function (r) {
          document.getElementById('fed-add-key').value = '';
          document.getElementById('fed-add-key-name').value = '';
          renderFederation(r.data);
        });
      });
    }
    const refresh = document.getElementById('fed-refresh');
    if (refresh) refresh.addEventListener('click', loadFederation);
    const connect = document.getElementById('fed-connect');
    if (connect) connect.addEventListener('click', function () {
      run('federation.connect', {}, 'fed-status', function (r) { renderFederation(r.data); });
    });
  }

  function loadFederation() {
    run('federation.list', {}, 'fed-status', function (r) { renderFederation(r.data); });
  }

  // ── Every admin action ──

  const ROLE_WORDS = { admin: 'for admins', mod: 'for moderators', member: 'for anyone' };
  const DANGER_WORDS = { destructive: 'cannot be undone', caution: 'asks first', read: 'changes nothing' };

  function actionForm(a) {
    const box = el('div', { cls: 'act-form' });
    const fields = {};
    (a.inputs || []).forEach(function (i) {
      let input;
      if (i.kind === 'bool') {
        input = el('input', { type: 'checkbox' });
      } else if (i.kind === 'choice') {
        input = el('select', {});
        input.appendChild(el('option', { value: '', text: 'choose' }));
        (i.choices || []).forEach(function (c) { input.appendChild(el('option', { value: c, text: c })); });
      } else {
        input = el('input', { type: i.kind === 'secret' ? 'password' : 'text', placeholder: i.hint || '', autocomplete: 'off' });
      }
      fields[i.name] = input;
      box.appendChild(el('label', { cls: 'act-field' }, [
        el('span', { text: i.label + (i.required ? '' : ' (optional)') }), input,
      ]));
    });
    let confirmBox = null;
    if (A.adminNeedsConfirm(a)) {
      confirmBox = el('input', { type: 'text', autocomplete: 'off' });
      box.appendChild(el('label', { cls: 'act-field act-danger' }, [
        el('span', { text: 'This cannot be undone. Type ' + a.confirm_word + ' to go ahead:' }), confirmBox,
      ]));
    }
    const result = el('div', { cls: 'fed-status' });
    const values = function () {
      const v = {};
      for (const n in fields) v[n] = fields[n].type === 'checkbox' ? fields[n].checked : fields[n].value;
      return v;
    };
    const say = function (words, ok) { result.textContent = words; result.className = 'fed-status ' + (ok ? 'ok' : 'err'); };
    let go;
    if (a.run === 'here') {
      go = btn('Run', async function () {
        if (confirmBox && !A.adminConfirmOk(a, confirmBox.value)) { say('Type ' + a.confirm_word + ' first.', false); return; }
        const f = A.adminInputsFromForm(a, values());
        if (!f.ok) { say(f.error, false); return; }
        say('Working...', true);
        try {
          const res = await A.adminRun(a.id, f.inputs, confirmBox ? a.confirm_word : undefined);
          say(res.message || 'Done.', true);
          if (a.id.startsWith('alerts.') && res.data && res.data.channels) renderAlerts(res.data);
          if (a.id.startsWith('federation.') && res.data && res.data.peers) renderFederation(res.data);
          if (a.id === 'log.recent' && res.data) renderLog(res.data.entries || []);
          if (logOpen && a.danger !== 'read') loadLog();
        } catch (e) { say(e.message || String(e), false); }
      }, '', 'Send it to the server now, signed with your key.');
    } else if (a.chat) {
      go = btn('Copy the chat command', function () {
        if (confirmBox && !A.adminConfirmOk(a, confirmBox.value)) { say('Type ' + a.confirm_word + ' first.', false); return; }
        const f = A.adminInputsFromForm(a, values());
        if (!f.ok) { say(f.error, false); return; }
        const text = A.adminChatText(a, f.inputs);
        if (navigator.clipboard) navigator.clipboard.writeText(text);
        say('Copied ' + text + ': paste it in Chat (in the channel it is for). This one is still a chat command.', true);
      }, '', 'This action is still a chat command: copy it, then send it in Chat.');
    } else {
      go = el('span', { cls: 'fed-help', text: 'Runs at ' + a.route + ': its panel is elsewhere on this page.' });
    }
    box.appendChild(el('div', { cls: 'fed-add-row' }, [go]));
    box.appendChild(result);
    return box;
  }

  function renderRegistry(reg) {
    const box = document.getElementById('actions-list');
    if (!box) return;
    const filter = (document.getElementById('actions-filter').value || '').trim().toLowerCase();
    box.textContent = '';
    (reg.categories || []).forEach(function (cat) {
      const actions = (reg.actions || []).filter(function (a) {
        return a.category === cat.id && (!filter || a.label.toLowerCase().includes(filter) || (a.description || '').toLowerCase().includes(filter) || a.id.includes(filter));
      });
      if (!actions.length) return;
      const details = el('details', { cls: 'fed-isolated' }, [el('summary', { text: cat.label + ' (' + actions.length + ')' })]);
      if (filter) details.open = true;
      details.appendChild(el('p', { cls: 'fed-help', text: cat.description || '' }));
      actions.forEach(function (a) {
        const row = el('div', { cls: 'act-row' });
        let form = null;
        row.appendChild(el('div', { cls: 'act-head' }, [
          el('span', { cls: 'fed-name', text: a.label }),
          el('span', { cls: 'fed-tier-note', text: ROLE_WORDS[a.role] || '' }),
          el('span', { cls: 'act-danger-' + a.danger, text: DANGER_WORDS[a.danger] || '' }),
          btn('Open', function (ev) {
            if (form) { form.remove(); form = null; ev.target.textContent = 'Open'; return; }
            form = actionForm(a);
            row.appendChild(form);
            ev.target.textContent = 'Close';
          }, '', 'Show or hide this action\'s form.'),
        ]));
        row.appendChild(el('div', { cls: 'fed-url', text: a.description || '' }));
        details.appendChild(row);
      });
      box.appendChild(details);
    });
  }

  let registry = null;
  async function loadRegistry() {
    try {
      registry = await A.adminFetchRegistry();
      renderRegistry(registry);
      status('actions-status', '', true);
    } catch (e) {
      status('actions-status', e.message || String(e), false);
    }
  }

  // ── The admin log ──

  let logOpen = false;
  function renderLog(entries) {
    const body = document.getElementById('adminlog-rows');
    if (!body) return;
    body.textContent = '';
    if (!entries.length) {
      body.appendChild(el('tr', {}, [el('td', { colspan: '4', text: 'Nothing has been changed through the admin actions yet.' })]));
    }
    entries.slice(0, 50).forEach(function (e) {
      body.appendChild(el('tr', { cls: e.ok === false ? 'act-failed' : '' }, [
        el('td', { text: A.adminTimeAgo(e.at) }),
        el('td', { text: e.who || '' }),
        el('td', { text: e.label || e.action }),
        el('td', { text: e.summary || '' }),
      ]));
    });
  }
  function loadLog() {
    logOpen = true;
    run('log.recent', {}, 'adminlog-status', function (r) { renderLog((r.data && r.data.entries) || []); });
  }

  // ── Start ──

  function init() {
    if (!A || !document.getElementById('alerts-panel')) return;
    wireAlertsAdd();
    wireFederationAdd();
    const filter = document.getElementById('actions-filter');
    if (filter) filter.addEventListener('input', function () { if (registry) renderRegistry(registry); });
    const logBtn = document.getElementById('adminlog-show');
    if (logBtn) logBtn.addEventListener('click', loadLog);
    loadAlerts();
    loadFederation();
    loadRegistry();
  }

  // The dashboard is revealed once the stats sign-in works (admin-app.js); the
  // console starts when it is shown, so a non-admin is not asked to sign twice.
  window.hosAdminConsoleStart = init;
})();
