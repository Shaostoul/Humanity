// chat-live.js: the web mirror of the native Chat live strip (v0.1151).
//
// Polls GET /api/live (the MJPEG broadcast plane's JSON directory; see
// src/relay/live.rs) and renders a "Live now" section in the chat sidebar,
// each row linking to the /watch viewer. This is deliberately SEPARATE from
// chat-voice-streaming.js, which drives the older WebRTC screen-share plane
// (#stream-sidebar): that plane is peer-to-peer signalling for rooms; this
// one is the relay broadcast anyone can watch at /watch?s=<name>.
//
// Each stream carries its bound chat room ("chat", default #live-<name>),
// declared at go-live per docs/design/studio-watch.md: the pairing is
// metadata, never guesswork.
//
// No dependencies. The container is created next to #stream-sidebar so the
// two live surfaces sit together; if that anchor is missing (markup drift),
// the module quietly does nothing rather than breaking chat.

(function () {
  'use strict';

  var POLL_MS = 10000;
  var timer = null;

  function container() {
    var el = document.getElementById('live-now');
    if (el) return el;
    var anchor = document.getElementById('stream-sidebar');
    if (!anchor || !anchor.parentNode) return null;
    el = document.createElement('div');
    el.id = 'live-now';
    el.style.display = 'none';
    var h = document.createElement('h4');
    h.textContent = 'Live now';
    var list = document.createElement('div');
    list.id = 'live-now-list';
    el.append(h, list);
    anchor.parentNode.insertBefore(el, anchor);
    return el;
  }

  function render(streams) {
    var el = container();
    if (!el) return;
    var list = document.getElementById('live-now-list');
    list.innerHTML = '';
    el.style.display = streams.length ? '' : 'none';
    streams.forEach(function (s) {
      var row = document.createElement('a');
      row.href = '/watch?s=' + encodeURIComponent(s.id);
      row.target = '_blank';
      row.rel = 'noopener';
      row.style.cssText =
        'display:block;padding:0.3rem 0.4rem;border-radius:4px;text-decoration:none;color:var(--text);font-size:0.78rem;';
      var name = document.createElement('span');
      name.textContent = s.id;
      name.style.fontWeight = '600';
      var meta = document.createElement('span');
      meta.textContent =
        ' ' + s.viewers + ' watching' + (s.chat ? ' · ' + s.chat : '');
      meta.style.color = 'var(--text-muted)';
      row.append(name, meta);
      list.appendChild(row);
    });
  }

  function poll() {
    fetch('/api/live')
      .then(function (r) { return r.ok ? r.json() : { streams: [] }; })
      .then(function (j) { render(j.streams || []); })
      .catch(function () { render([]); });
  }

  document.addEventListener('DOMContentLoaded', function () {
    poll();
    timer = setInterval(poll, POLL_MS);
  });

  // Expose for tests/devtools.
  window.chatLive = { poll: poll, stop: function () { clearInterval(timer); } };
})();
