/**
 * The chat's read-only view for a visitor without a name (2026-11-02, docs/design/chat-access.md
 * section 7; web/chat/read.html). The answer to the operator's "Is there a nonaccount way to view
 * the chat?": the channels anyone may read, each one's posts since it was opened to everyone, as
 * plain words with names only, and no box to type in.
 *
 * Everything comes from this server without a key: GET /api/channels (only the channels that read
 * at Anyone) and GET /api/messages (an Anyone channel's posts since it opened, without keys; any
 * other channel answers 401). The server decides what a visitor sees (src/relay/chat_access.rs);
 * this page only draws it. Every word from the server goes in as text, never as markup, and no
 * address in a post becomes a link or a picture.
 */
(function () {
  'use strict';

  const st = { channels: [], channel: '', cursor: 0 };
  const $ = (id) => document.getElementById(id);
  const t = (key) => (window.i18n ? window.i18n.t(key) : key);

  function status(key) {
    const s = $('read-status');
    if (!s) return;
    s.textContent = key ? t(key) : '';
    s.hidden = !key;
  }

  /** One post as a list row: who, when, the words; all of it text. */
  function row(msg) {
    const li = document.createElement('li');
    li.className = 'read-line';
    const who = document.createElement('span');
    who.className = 'read-who';
    who.textContent = String(msg.from_name || msg.server_name || '');
    const when = document.createElement('span');
    when.className = 'read-when';
    const ts = Number(msg.timestamp) || 0;
    when.textContent = ts ? new Date(ts).toLocaleString() : '';
    const words = document.createElement('div');
    words.className = 'read-words';
    words.textContent = String(msg.content || '');
    li.append(who, when, words);
    return li;
  }

  /** Draw a channel's posts after the cursor (all of them when `fresh`). */
  async function load(fresh) {
    const list = $('read-list');
    if (!st.channel || !list) return;
    if (fresh) { list.textContent = ''; st.cursor = 0; status('chat_read.loading'); }
    let data = null;
    try {
      const q = 'channel=' + encodeURIComponent(st.channel) + '&limit=100' + (st.cursor ? '&after=' + st.cursor : '');
      const res = await fetch('/api/messages?' + q);
      data = res.ok ? await res.json() : null;
    } catch (e) { data = null; }
    if (!data) { status('chat_read.failed'); return; }
    const messages = Array.isArray(data.messages) ? data.messages : [];
    for (const m of messages) list.appendChild(row(m));
    if (Number(data.cursor) > st.cursor) st.cursor = Number(data.cursor);
    status(list.children.length ? '' : 'chat_read.empty');
  }

  function choose(id) {
    st.channel = id;
    const tabs = $('read-tabs');
    if (tabs) tabs.querySelectorAll('.read-tab').forEach((b) => b.setAttribute('aria-selected', String(b.dataset.channel === id)));
    load(true);
  }

  async function start() {
    const tabs = $('read-tabs');
    let channels = [];
    try {
      const res = await fetch('/api/channels');
      const data = res.ok ? await res.json() : null;
      channels = data && Array.isArray(data.channels) ? data.channels : [];
    } catch (e) { channels = []; }
    st.channels = channels;
    if (!channels.length) { status('chat_read.no_channels'); return; }
    const refresh = $('read-refresh');
    for (const c of channels) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'read-tab';
      b.setAttribute('role', 'tab');
      b.dataset.channel = String(c.id);
      b.textContent = '#' + String(c.name || c.id);
      if (window.i18n) window.i18n.hint(b, 'chat_read.channel_hint');
      b.addEventListener('click', () => choose(String(c.id)));
      tabs.insertBefore(b, refresh);
    }
    if (refresh) refresh.addEventListener('click', () => load(false));
    choose(String(channels[0].id));
  }

  if (typeof window !== 'undefined') window.hosChatRead = { start, load, choose, row, state: st };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
  else start();
})();
