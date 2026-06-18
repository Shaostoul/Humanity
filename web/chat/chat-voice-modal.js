// Per-user voice control modal (v0.484). Opened by clicking a person in a voice
// channel roster. Three role-tiered sections:
//   STANDARD (red, anyone, local only): per-user Volume 0 to 200 percent, local
//     Mute, Squelch (noise gate), plus View Profile, Direct Message, Follow,
//     Block, Report.
//   MOD (green, mod and admin, affects the user for everyone): Text mute, Kick
//     from server. (Server voice-mute and Disconnect-from-voice arrive with the
//     relay handlers; shown disabled until then.)
//   ADMIN (blue, admin only, heavy or permanent): Ban, Unban, Game ban, Verify,
//     Promote to Mod, Demote.
//
// The audio controls drive a per-peer Web Audio gain graph set up at the voice
// mesh ontrack site (chat-voice-rooms.js calls window.setupPeerAudio). The
// moderation and admin actions reuse the existing per-user functions (they act
// on window.ctxMenuTarget, which we set before calling). CSP-safe: all DOM is
// built with createElement and addEventListener, no inline handlers. No em dashes.

(function () {
  'use strict';

  // ---- Per-peer audio graph (called from chat-voice-rooms.js ontrack) ----

  function peerPrefKey(key) { return 'humanity-vc-peer-' + key; }

  function loadPeerPrefs(key) {
    var d = { volume: 100, muted: false, squelch: false, squelchThreshold: 12 };
    try {
      var raw = localStorage.getItem(peerPrefKey(key));
      if (raw) { var p = JSON.parse(raw); Object.assign(d, p); }
    } catch (e) { /* defaults */ }
    return d;
  }
  function savePeerPrefs(key) {
    var pa = (window._peerAudio || {})[key];
    if (!pa) return;
    try {
      localStorage.setItem(peerPrefKey(key), JSON.stringify({
        volume: pa.volume, muted: pa.muted, squelch: pa.squelch, squelchThreshold: pa.squelchThreshold,
      }));
    } catch (e) { /* storage full or disabled */ }
  }

  // Build (or rebuild) the gain graph for a peer and apply its saved prefs.
  window.setupPeerAudio = function (peerKey, audioEl, stream) {
    window._peerAudio = window._peerAudio || {};
    var prefs = loadPeerPrefs(peerKey);
    var entry = { audioEl: audioEl, gain: null, analyser: null, gated: false,
                  volume: prefs.volume, muted: prefs.muted,
                  squelch: prefs.squelch, squelchThreshold: prefs.squelchThreshold };
    try {
      var ctx = window._voiceAudioCtx ||
        (window._voiceAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
      if (ctx.state === 'suspended') { ctx.resume().catch(function () {}); }
      var src = ctx.createMediaStreamSource(stream);
      var gain = ctx.createGain();
      var analyser = ctx.createAnalyser();
      analyser.fftSize = 256;
      src.connect(gain);
      src.connect(analyser); // tap for squelch level, does not affect output
      gain.connect(ctx.destination);
      entry.gain = gain;
      entry.analyser = analyser;
      // Play through the gain graph and mute the element so we do not double
      // play, BUT only while the AudioContext is actually running. If the
      // browser keeps it suspended (autoplay policy), leave the element audible
      // so voice is never lost, and re-mute once a user gesture resumes it.
      var syncMuteToCtx = function () {
        var running = ctx.state === 'running';
        audioEl.muted = running; // element silent only when the gain graph is live
        if (!running) {
          ctx.resume().catch(function () {});
        }
      };
      syncMuteToCtx();
      setTimeout(syncMuteToCtx, 600);
      if (!window._voiceCtxGestureHooked) {
        window._voiceCtxGestureHooked = true;
        var resumeAll = function () {
          if (window._voiceAudioCtx) window._voiceAudioCtx.resume().catch(function () {});
          var all = window._peerAudio || {};
          Object.keys(all).forEach(function (k) {
            var p = all[k];
            if (p.gain && window._voiceAudioCtx && window._voiceAudioCtx.state === 'running' && p.audioEl) {
              p.audioEl.muted = true;
            }
          });
        };
        document.addEventListener('click', resumeAll);
        document.addEventListener('touchstart', resumeAll);
      }
    } catch (e) {
      console.warn('Per-peer gain graph unavailable, using element volume', e);
      // Fallback: element only (caps at 100 percent, no squelch).
    }
    window._peerAudio[peerKey] = entry;
    applyPeerGain(peerKey);
    ensureSquelchPoll();
  };

  window.teardownAllPeerAudio = function () {
    var all = window._peerAudio || {};
    Object.keys(all).forEach(function (k) {
      var pa = all[k];
      try { if (pa.gain) pa.gain.disconnect(); } catch (e) {}
      try { if (pa.analyser) pa.analyser.disconnect(); } catch (e) {}
    });
    window._peerAudio = {};
  };

  function applyPeerGain(peerKey) {
    var pa = (window._peerAudio || {})[peerKey];
    if (!pa) return;
    var g = pa.muted ? 0 : (pa.volume / 100);
    if (pa.squelch && pa.gated) g = 0;
    if (pa.gain) {
      pa.gain.gain.value = g;
    } else if (pa.audioEl) {
      pa.audioEl.muted = pa.muted;
      pa.audioEl.volume = Math.min(1, Math.max(0, pa.volume / 100));
    }
  }

  // One shared poll gates squelched peers below their threshold.
  function ensureSquelchPoll() {
    if (window._squelchPollTimer) return;
    window._squelchPollTimer = setInterval(function () {
      var all = window._peerAudio || {};
      var any = false;
      Object.keys(all).forEach(function (k) {
        var pa = all[k];
        if (!pa.squelch || !pa.analyser) return;
        any = true;
        var buf = new Uint8Array(pa.analyser.frequencyBinCount);
        pa.analyser.getByteFrequencyData(buf);
        var sum = 0;
        for (var i = 0; i < buf.length; i++) sum += buf[i];
        var avg = sum / buf.length;
        var gated = avg < pa.squelchThreshold;
        if (gated !== pa.gated) { pa.gated = gated; applyPeerGain(k); }
      });
      if (!any) { clearInterval(window._squelchPollTimer); window._squelchPollTimer = null; }
    }, 80);
  }

  // ---- The modal ----

  function myRole() {
    var r = (typeof peerData !== 'undefined' && peerData[myKey] && peerData[myKey].role) ||
            window.myPeerRole || '';
    return String(r).toLowerCase();
  }

  // Reused per-user actions act on window.ctxMenuTarget; set it, then call.
  function withTarget(name, key, fn) {
    window.ctxMenuTarget = { name: name, publicKey: key, key: key };
    try { fn(); } catch (e) { console.warn('voice modal action failed', e); }
  }

  function row(parent, label, tier, onClick, opts) {
    opts = opts || {};
    var b = document.createElement('button');
    b.className = 'vmodal-action vmodal-' + tier + (opts.danger ? ' vmodal-danger' : '');
    b.textContent = label;
    if (opts.disabled) {
      b.disabled = true;
      if (opts.title) b.title = opts.title;
    } else {
      b.addEventListener('click', onClick);
    }
    parent.appendChild(b);
    return b;
  }

  function section(parent, title, tierClass) {
    var s = document.createElement('div');
    s.className = 'vmodal-section ' + tierClass;
    var h = document.createElement('h3');
    h.textContent = title;
    s.appendChild(h);
    parent.appendChild(s);
    return s;
  }

  window.openVoiceUserModal = function (name, key) {
    if (!key) return;
    var existing = document.getElementById('voiceuser-overlay');
    if (existing) existing.remove();

    var overlay = document.createElement('div');
    overlay.id = 'voiceuser-overlay';
    overlay.className = 'profile-modal-overlay open';

    var modal = document.createElement('div');
    modal.className = 'profile-modal vmodal';
    overlay.appendChild(modal);

    var close = document.createElement('button');
    close.className = 'close-btn';
    close.setAttribute('aria-label', 'Close');
    close.innerHTML = '&times;';
    close.addEventListener('click', closeVoiceUserModal);
    modal.appendChild(close);

    var h2 = document.createElement('h2');
    h2.textContent = name || 'User';
    modal.appendChild(h2);

    var isMe = key === myKey;
    var role = myRole();
    var amMod = role === 'mod' || role === 'admin' || role === 'owner';
    var amAdmin = role === 'admin' || role === 'owner';

    // STANDARD (red): local audio + personal actions.
    var std = section(modal, 'You', 'vmodal-tier-standard');
    if (!isMe) {
      var pa = (window._peerAudio || {})[key] || loadPeerPrefs(key);
      // Volume 0 to 200 percent.
      var vWrap = document.createElement('div');
      vWrap.className = 'vmodal-slider';
      var vLabel = document.createElement('label');
      vLabel.textContent = 'Volume';
      var vVal = document.createElement('span');
      vVal.className = 'vmodal-val';
      vVal.textContent = (pa.volume || 100) + '%';
      var slider = document.createElement('input');
      slider.type = 'range'; slider.min = '0'; slider.max = '200'; slider.step = '5';
      slider.value = String(pa.volume != null ? pa.volume : 100);
      slider.addEventListener('input', function () {
        var v = parseInt(slider.value, 10);
        vVal.textContent = v + '%';
        var e = (window._peerAudio || {})[key];
        if (e) { e.volume = v; applyPeerGain(key); savePeerPrefs(key); }
        else { persistOnly(key, { volume: v }); }
      });
      vWrap.appendChild(vLabel); vWrap.appendChild(slider); vWrap.appendChild(vVal);
      std.appendChild(vWrap);
      // Mute (local) toggle.
      toggleRow(std, 'Mute (you stop hearing them)', !!pa.muted, function (on) {
        var e = (window._peerAudio || {})[key];
        if (e) { e.muted = on; applyPeerGain(key); savePeerPrefs(key); }
        else { persistOnly(key, { muted: on }); }
      });
      // Squelch toggle.
      toggleRow(std, 'Squelch (cut background noise)', !!pa.squelch, function (on) {
        var e = (window._peerAudio || {})[key];
        if (e) { e.squelch = on; e.gated = false; applyPeerGain(key); savePeerPrefs(key); ensureSquelchPoll(); }
        else { persistOnly(key, { squelch: on }); }
      });
      if (!(window._peerAudio || {})[key]) {
        var note = document.createElement('p');
        note.className = 'vmodal-note';
        note.textContent = 'Audio controls take effect once they are speaking in your channel.';
        std.appendChild(note);
      }
      row(std, 'View profile', 'tier-standard', function () { withTarget(name, key, function () { if (typeof requestViewProfile === 'function') requestViewProfile(key); else if (typeof viewProfileFromCtx === 'function') viewProfileFromCtx(); }); });
      row(std, 'Direct message', 'tier-standard', function () { closeVoiceUserModal(); withTarget(name, key, function () { if (typeof dmFromCtx === 'function') dmFromCtx(); else if (typeof openDmConversation === 'function') openDmConversation(key, name); }); });
      // Follow / Unfollow: same wording + state as the right-side user list and
      // the right-click menu (operator: keep terminology consistent).
      var isFollowing = (typeof myFollowing !== 'undefined' && myFollowing.has(key));
      row(std, isFollowing ? 'Unfollow' : 'Follow', 'tier-standard', function () { withTarget(name, key, function () { if (typeof followFromCtx === 'function') followFromCtx(!isFollowing); }); });
      var blocked = (typeof isBlocked === 'function' && isBlocked(name));
      row(std, blocked ? 'Unblock' : 'Block', 'tier-standard', function () { withTarget(name, key, function () { if (blocked) { if (typeof unblockFromCtx === 'function') unblockFromCtx(); } else { if (typeof blockFromCtx === 'function') blockFromCtx(); } }); }, { danger: !blocked });
      row(std, 'Report', 'tier-standard', function () { withTarget(name, key, function () { if (typeof reportUser === 'function') reportUser(); }); }, { danger: true });
    } else {
      var meNote = document.createElement('p');
      meNote.className = 'vmodal-note';
      meNote.textContent = 'This is you. You are connected to this channel\'s voice.';
      std.appendChild(meNote);
    }

    // MOD (green): server moderation.
    if (amMod && !isMe) {
      var mod = section(modal, 'Moderation', 'vmodal-tier-mod');
      row(mod, 'Voice mute (their mic off for everyone)', 'tier-mod', null, { disabled: true, title: 'Arrives with the voice moderation update.' });
      row(mod, 'Disconnect from voice', 'tier-mod', null, { disabled: true, title: 'Arrives with the voice moderation update.' });
      row(mod, 'Text mute', 'tier-mod', function () { withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/mute'); }); });
      row(mod, 'Kick from server', 'tier-mod', function () { closeVoiceUserModal(); withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/kick'); }); }, { danger: true });
    }

    // ADMIN (blue): heavy or permanent.
    if (amAdmin && !isMe) {
      var adm = section(modal, 'Admin', 'vmodal-tier-admin');
      var banNote = document.createElement('p');
      banNote.className = 'vmodal-note';
      banNote.textContent = 'Chat is a right, so ban from chat sparingly. A game ban blocks the 3D world only and leaves chat untouched.';
      adm.appendChild(banNote);
      row(adm, 'Ban from chat', 'tier-admin', function () { closeVoiceUserModal(); withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/ban'); }); }, { danger: true });
      row(adm, 'Unban from chat', 'tier-admin', function () { withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/unban'); }); });
      row(adm, 'Game ban (3D world only)', 'tier-admin', function () { closeVoiceUserModal(); if (typeof sendGameBan === 'function') sendGameBan(key, ''); }, { danger: true });
      row(adm, 'Verify', 'tier-admin', function () { withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/verify'); }); });
      row(adm, 'Promote to mod', 'tier-admin', function () { withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/mod'); }); });
      row(adm, 'Demote', 'tier-admin', function () { withTarget(name, key, function () { if (typeof ctxCommand === 'function') ctxCommand('/unmod'); }); });
    }

    overlay.addEventListener('click', function (e) { if (e.target === overlay) closeVoiceUserModal(); });
    document.body.appendChild(overlay);
  };

  function persistOnly(key, patch) {
    var cur = loadPeerPrefs(key);
    Object.assign(cur, patch);
    try { localStorage.setItem(peerPrefKey(key), JSON.stringify(cur)); } catch (e) {}
  }

  function toggleRow(parent, label, on, onChange) {
    var wrap = document.createElement('label');
    wrap.className = 'vmodal-toggle';
    var cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = !!on;
    cb.addEventListener('change', function () { onChange(cb.checked); });
    var txt = document.createElement('span');
    txt.textContent = label;
    wrap.appendChild(cb); wrap.appendChild(txt);
    parent.appendChild(wrap);
  }

  function closeVoiceUserModal() {
    var o = document.getElementById('voiceuser-overlay');
    if (o) o.remove();
  }
  window.closeVoiceUserModal = closeVoiceUserModal;
})();
