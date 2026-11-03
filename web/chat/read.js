/**
 * The chat's read-only view for a visitor without a name (2026-11-02, docs/design/chat-access.md
 * section 7; web/chat/read.html). The answer to the operator's "Is there a nonaccount way to view
 * the chat?": the channels anyone may read, each one's posts since it was opened to everyone, as
 * plain words with names only, and no box to type in.
 *
 * Everything comes from this server without a key: GET /api/channels (only the channels that read
 * at Anyone) and GET /api/messages (an Anyone channel's posts since it opened, without keys; any
 * other channel answers 401 and its level, a channel that is not there 404). The server decides
 * what a visitor sees (src/relay/chat_access.rs); this page only draws it. Every word from the
 * server goes in as text, never as markup, and no address in a post becomes a link or a picture.
 *
 * A LINK TO A CHANNEL (2026-11-03, the operator: "Can we make it so I can link people to specific
 * channels like https://united-humanity.us/chat#global or
 * https://united-humanity.us/chat#fire-disciples"). /chat sends a visitor here with the link's
 * channel kept: /chat/read#fire-disciples. A channel anyone may read opens (by its id, else its
 * shown name, in any case: web/shared/chat-links.js). One that is not, the server is asked about:
 * a channel for members or for verified people says so, with Choose a name to join in; one that
 * is not there says that, and no other channel opens in its place. This page names no channel
 * except the ones /api/channels lists and the one in the visitor's own link. With no link, the
 * view opens #global (HosChatAccess.MAIN_CHANNEL), else the first channel listed. The address
 * follows the channel shown, and Choose a name to join in carries it through the welcome step to
 * the chat (sessionStorage `hos_chat_link`, read once by app.js).
 */
(function () {
  'use strict';

  const st = { channels: [], channel: '', cursor: 0, link: '' };
  const $ = (id) => document.getElementById(id);
  const t = (key, vars) => (window.i18n ? window.i18n.t(key, vars) : key);
  /** The channel every server has, opened when no link names one. */
  const MAIN = (window.HosChatAccess && window.HosChatAccess.MAIN_CHANNEL) || 'global';
  /** Where the channel to open after the welcome step waits (web/chat/app.js reads it once). */
  const CARRY_KEY = 'hos_chat_link';

  function status(key, vars) {
    const s = $('read-status');
    if (!s) return;
    s.textContent = key ? t(key, vars) : '';
    s.hidden = !key;
  }

  /** The channel word the address names ('' when none). */
  function linked() {
    const L = window.HosChatLinks;
    return L && typeof location !== 'undefined' ? L.wordOf(location.hash) : '';
  }

  /** Show `id` in the address (replacing it: the view's tabs are not steps of their own yet,
   *  navigation rung 5, docs/design/navigation.md 10.3), or no channel with ''. */
  function showInAddress(id) {
    const L = window.HosChatLinks;
    if (!L || typeof location === 'undefined' || typeof history === 'undefined') return;
    const want = id ? L.hashOf(id) : '';
    if ((location.hash || '') === want) return;
    try { history.replaceState(history.state, '', location.pathname + location.search + want); } catch (e) { /* fine */ }
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
    st.link = '';
    const tabs = $('read-tabs');
    if (tabs) tabs.querySelectorAll('.read-tab').forEach((b) => b.setAttribute('aria-selected', String(b.dataset.channel === id)));
    showInAddress(id);
    load(true);
  }

  /**
   * The link's channel is not one anyone may read here: ask the server which it is (a channel for
   * members, for verified people, or none) and say so. Nothing of it is drawn, and no tab is open.
   */
  async function explain(word) {
    st.channel = '';
    st.link = word;
    const tabs = $('read-tabs');
    if (tabs) tabs.querySelectorAll('.read-tab').forEach((b) => b.setAttribute('aria-selected', 'false'));
    const list = $('read-list');
    if (list) list.textContent = '';
    status('chat_read.loading');
    let answer = 0;
    let level = '';
    try {
      const res = await fetch('/api/messages?channel=' + encodeURIComponent(word) + '&limit=1');
      answer = res.status;
      if (answer === 401) {
        const body = await res.json().catch(() => null);
        level = body && typeof body.read_level === 'string' ? body.read_level : '';
      }
    } catch (e) { answer = 0; }
    if (st.link !== word) return; // another channel was chosen meanwhile
    if (answer === 401) status(level === 'verified' ? 'chat_read.link_verified' : 'chat_read.link_members', { name: word });
    else if (answer === 404) status('chat_read.link_missing', { name: word });
    else status('chat_read.failed');
  }

  /** Open what the link names: its tab when anyone may read it, else what the server says of it. */
  function open(word) {
    const L = window.HosChatLinks;
    const ch = L ? L.find(word, st.channels) : null;
    if (ch) choose(String(ch.id));
    else explain(word);
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
    const word = linked();
    if (!channels.length && !word) { status('chat_read.no_channels'); return; }
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
    if (refresh) refresh.addEventListener('click', () => (st.channel ? load(false) : st.link && explain(st.link)));
    // Choose a name to join in: the channel shown (or the link's) opens in the chat afterwards.
    const join = $('read-join');
    if (join) {
      join.addEventListener('click', () => {
        try { sessionStorage.setItem(CARRY_KEY, st.link || st.channel); } catch (e) { /* storage off */ }
      });
    }
    if (word) { open(word); return; }
    const main = channels.find((c) => String(c.id) === MAIN);
    choose(String((main || channels[0]).id));
  }

  // Someone edits the address, or follows a link to another channel with this view open.
  if (typeof window !== 'undefined' && window.addEventListener) {
    window.addEventListener('hashchange', () => {
      const word = linked();
      if (word && word !== st.channel && word !== st.link) open(word);
    });
  }

  if (typeof window !== 'undefined') window.hosChatRead = { start, load, choose, open, row, state: st };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
  else start();
})();
