// Moved out of web/pages/watch.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    (function(){
      'use strict';

      // ── Wire format (must match src/relay/live.rs and src/net/live.rs) ──
      //   [1 byte tag][8 bytes PTS micros, big-endian][payload]
      // tag 0 = codec config, 1 = keyframe, 2 = delta, 3 = audio.
      // v1 is MJPEG: every frame is a self-contained JPEG tagged as a keyframe.
      // When the H.264 encoder lands, only the decode call below changes.
      const TAG_CONFIG = 0, TAG_KEY = 1, TAG_DELTA = 2;
      const HEADER_BYTES = 9;

      const $ = (id) => document.getElementById(id);
      const canvas = $('w-canvas');
      const ctx = canvas.getContext('2d');

      const params = new URLSearchParams(location.search);
      // Accept ?s=name (canonical), ?u=name (the per-user spelling people
      // share: a stream id IS the streamer's registered name), and also a
      // trailing path segment (/watch/name) if nginx ever routes it that way.
      // Guard against the page's own filename: /watch.html with no query is
      // the DIRECTORY view, not a request to watch a stream called ".html".
      let pathTail = location.pathname.replace(/^\/watch\/?/, '');
      if (pathTail === '.html' || pathTail === 'watch.html') pathTail = '';
      const streamId = (params.get('s') || params.get('u') || pathTail || '').trim().toLowerCase();

      let socket = null;
      let frames = 0;
      let bytes = 0;
      let startedAt = 0;
      // Guard against decoding faster than we can paint: if a frame is still being
      // decoded when the next arrives, drop the new one. Without this a slow client
      // builds an unbounded backlog of pending decodes and the tab dies.
      let decoding = false;

      function setStatus(live, detail) {
        const badge = $('w-status');
        badge.textContent = live ? 'LIVE' : 'OFFLINE';
        badge.className = 'badge ' + (live ? 'badge-live' : 'badge-off');
        $('w-msg').classList.toggle('hidden', live);
        $('w-stats').hidden = !live;
        if (detail) $('w-msg-detail').textContent = detail;
      }

      function paint(blob) {
        if (decoding) return;   // see `decoding` above
        decoding = true;
        createImageBitmap(blob).then((bmp) => {
          if (canvas.width !== bmp.width || canvas.height !== bmp.height) {
            canvas.width = bmp.width;
            canvas.height = bmp.height;
            $('s-res').textContent = bmp.width + 'x' + bmp.height;
          }
          ctx.drawImage(bmp, 0, 0);
          // Release the bitmap's memory immediately. Skipping this is the classic
          // way to OOM a tab in under a minute.
          bmp.close();
          decoding = false;
        }).catch(() => { decoding = false; });
      }

      function updateStats() {
        $('s-frames').textContent = frames;
        const secs = Math.max(1, (Date.now() - startedAt) / 1000);
        $('s-bitrate').textContent = Math.round(bytes * 8 / secs / 1000) + ' kbps';
      }

      function connect(id) {
        if (socket) { socket.close(); socket = null; }
        frames = 0; bytes = 0; startedAt = Date.now();

        const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(proto + '//' + location.host + '/ws/live/sub/' + encodeURIComponent(id));
        socket.binaryType = 'arraybuffer';

        socket.onmessage = (ev) => {
          // A text frame is the relay telling us something went wrong.
          if (typeof ev.data === 'string') {
            try {
              const v = JSON.parse(ev.data);
              if (v.ok === false) setStatus(false, v.error === 'not live'
                ? id + ' is not streaming right now.'
                : String(v.error || 'Stream unavailable.'));
            } catch (_) { /* ignore anything unparseable */ }
            return;
          }

          const buf = new Uint8Array(ev.data);
          if (buf.length < HEADER_BYTES) return;
          const tag = buf[0];
          const payload = buf.subarray(HEADER_BYTES);

          if (tag === TAG_KEY || tag === TAG_DELTA) {
            setStatus(true);
            frames++;
            bytes += buf.length;
            updateStats();
            paint(new Blob([payload], { type: 'image/jpeg' }));
          } else if (tag === TAG_CONFIG) {
            // MJPEG needs no codec config. This is here for the H.264 upgrade,
            // where it carries the avcC / SPS+PPS a VideoDecoder must be
            // configured with before the first frame can be decoded.
          }
        };

        socket.onclose = () => setStatus(false, 'The stream ended.');
        socket.onerror = () => setStatus(false, 'Could not connect to the stream.');
      }

      // ── Directory of what is live, refreshed every 10s ──
      async function refreshList() {
        let streams = [];
        try {
          const r = await fetch('/api/live');
          if (r.ok) streams = (await r.json()).streams || [];
        } catch (_) { /* offline: fall through to the empty state */ }

        const list = $('w-list');
        list.innerHTML = '';
        $('w-list-empty').hidden = streams.length > 0;

        for (const s of streams) {
          const a = document.createElement('a');
          a.className = 'stream-card';
          a.href = '/watch.html?s=' + encodeURIComponent(s.id);
          const n = document.createElement('div'); n.className = 'n'; n.textContent = s.id;
          const t = document.createElement('div'); t.className = 't'; t.textContent = s.title || '';
          const v = document.createElement('div'); v.className = 'v';
          v.textContent = s.viewers + ' watching';
          // The stream's bound chat room (v0.1151): declared at go-live,
          // default #live-<name>. Shown so a viewer knows where the
          // conversation lives; the chat page is one tab away.
          if (s.chat) {
            const c = document.createElement('div'); c.className = 'v';
            c.textContent = 'chat: ' + s.chat;
            v.append(document.createTextNode(' '));
            a.append(n, t, v, c);
          } else {
            a.append(n, t, v);
          }
          list.appendChild(a);
        }

        // Keep the viewer count on the badge honest for the stream we are watching.
        if (streamId) {
          const mine = streams.find((s) => s.id === streamId);
          const badge = $('w-viewers');
          badge.hidden = !mine;
          if (mine) {
            badge.textContent = mine.viewers + ' watching';
            $('w-sub').textContent = mine.title || ('Live from ' + streamId);
          }
        }
      }

      if (streamId) {
        $('w-title').textContent = streamId;
        setStatus(false, 'Connecting...');
        connect(streamId);
      }
      refreshList();
      setInterval(refreshList, 10000);
    })();
