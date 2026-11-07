/* shared/video-volume.js: what volume a video starts at on the website (2026-11-06), the twin of
 * the desktop app's Settings > Audio > Videos (crates/humanity-core/src/audio/start_volume.rs).
 *
 * The operator: "Let's add the toggle option maintain last used volume setting between app resets
 * or don't. Another could be all videos start at custom % volume. So I could set them at say 5% for
 * every new video I start because, not everyone balances audio the same."
 *
 * Settings > Audio & Video > Videos keeps the choice in this browser (the shared preferences,
 * `humanity_settings`): 'video-start' is 'last' (each new video starts where the last video's
 * volume was left, kept across visits) or 'set' (every new video starts at 'video-start-pct'
 * percent). The last volume is kept under its own key, `hos_video_last_volume`. This file listens,
 * on the whole page, for a <video> starting to play for the first time and gives it the start
 * volume, and for a person moving a video's volume, which becomes the last volume. The browser's
 * own player draws the volume bar; nothing here draws anything. Mute is not remembered (a video
 * starting silent the next day, with no reason shown, reads as broken).
 *
 * Loaded by the pages that play videos (chat/index.html, pages/files.html). Pure enough to test in
 * node: scripts/tests/video-volume-web.test.js.
 */
(function (root) {
  'use strict';

  var PREFS_KEY = 'humanity_settings';
  var LAST_KEY = 'hos_video_last_volume';

  function clamp01(v) {
    v = Number(v);
    if (!isFinite(v)) return 1;
    return Math.min(1, Math.max(0, v));
  }

  /** The choice from the stored preferences: { at: 'last' | 'set', set: 0..1 }. */
  function choiceFrom(prefs) {
    var at = prefs && prefs['video-start'] === 'set' ? 'set' : 'last';
    var pct = prefs && prefs['video-start-pct'] !== undefined ? Number(prefs['video-start-pct']) : 100;
    return { at: at, set: clamp01(pct / 100) };
  }

  /** The volume a video starting now starts at (0..1): the last volume, or the set one. */
  function startVolume(prefs, last) {
    var c = choiceFrom(prefs);
    if (c.at === 'set') return c.set;
    return last === null || last === undefined || last === '' ? 1 : clamp01(last);
  }

  function readPrefs(storage) {
    try { return JSON.parse(storage.getItem(PREFS_KEY)) || {}; } catch (e) { return {}; }
  }

  function readLast(storage) {
    try { var v = storage.getItem(LAST_KEY); return v === null ? null : Number(v); } catch (e) { return null; }
  }

  /**
   * Listen on `doc` for videos playing and volumes moving, keeping the last volume in `storage`.
   * Returns the two handlers (for tests).
   */
  function install(doc, storage) {
    function isVideo(el) {
      return el && el.tagName && String(el.tagName).toLowerCase() === 'video';
    }
    function onPlay(e) {
      var v = e && e.target;
      if (!isVideo(v) || v.hosVolumeGiven) return;
      v.hosVolumeGiven = true;
      var start = startVolume(readPrefs(storage), readLast(storage));
      // Setting the volume fires a volumechange of our own; marked so it is not taken for the
      // person's. Only when the value moves: an unchanged volume fires nothing.
      if (Math.abs((Number(v.volume) || 0) - start) > 0.001) {
        v.hosSettingVolume = true;
        v.volume = start;
      }
    }
    function onVolume(e) {
      var v = e && e.target;
      if (!isVideo(v)) return;
      if (v.hosSettingVolume) { v.hosSettingVolume = false; return; }
      try { storage.setItem(LAST_KEY, String(clamp01(v.volume))); } catch (err) { /* storage refused: not kept */ }
    }
    if (doc && typeof doc.addEventListener === 'function') {
      doc.addEventListener('play', onPlay, true);
      doc.addEventListener('volumechange', onVolume, true);
    }
    return { onPlay: onPlay, onVolume: onVolume };
  }

  var api = { choiceFrom: choiceFrom, startVolume: startVolume, install: install, LAST_KEY: LAST_KEY };
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  } else {
    root.HosVideoVolume = api;
    try { install(root.document, root.localStorage); } catch (e) { /* no storage: videos keep the browser's volume */ }
  }
})(typeof window !== 'undefined' ? window : this);
