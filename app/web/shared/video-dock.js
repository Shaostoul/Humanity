/* shared/video-dock.js: the chat's video dock on the website (2026-11-06), the twin of the desktop
 * app's src/gui/pages/chat/video_dock.rs (docs/design/media-player.md, "Rung 3: the anchored dock").
 *
 * The operator: "When we scroll them off page can we add like an anchored picture-in-picture mode
 * that's stuck to the side of the chat it's at? It could be like a collapsible tap that has minimize
 * (keeps playing unless paused) and close (closes anchored section.)"
 *
 * A PLAYING video whose card is scrolled out of view (web/chat/app.js's IntersectionObserver) moves
 * into a small box at the top right of the message list, just under the channel header: the SAME
 * <video> element, moved in one task. HTML's removing steps for a media element wait for a stable
 * state and do nothing when the element is in a document again by then, and inserting a media
 * element whose network state is not empty runs no load, so it plays on: nothing fetched again,
 * nothing restarted (seen in Chromium, docs/design/media-player.md). Its card keeps a line the size
 * of the player saying where it is, so nothing in the list moves. The browser's own controls on the
 * video give Play or Pause, the time and Mute; the box adds:
 *
 *  - Minimize: the box folds into a slim tab on the same edge ("Video playing"); the video plays on,
 *    sound and all, unless paused. Pressing the tab opens it again.
 *  - Close: the video stops (paused) and goes back into its card, at the place it reached; the box
 *    closes.
 *  - Go to message: the chat opens at the video's channel (app.js switches to it when another is
 *    open) and its card is scrolled into view, where the video goes back.
 *  - Float it, only where the browser has its own floating picture-in-picture
 *    (document.pictureInPictureEnabled: Chromium and Safari; Firefox has its own button on the
 *    video and no such call): one line, `video.requestPictureInPicture()`.
 *
 * Scrolling the card back into view gives the video back (app.js watches the line in the card). A
 * card drawn again for the same video (its channel opened again) takes the line's place and the
 * video back the same way (adopt). On a phone-sized window the box starts as the tab. The website's
 * pages are separate documents, so the box lives on the chat page; the desktop app's follows the
 * person round its pages.
 *
 * Words and hints by key from data/i18n/en.json (`chat_video_dock.*`). Pure enough to test in node
 * with a small stand-in document: scripts/tests/video-dock-web.test.js.
 */
(function (root) {
  'use strict';

  /** The words of `key` (with `vars`), or nothing without the i18n module. */
  function words(i18n, key, vars) {
    return i18n && typeof i18n.t === 'function' ? String(i18n.t(key, vars)) : '';
  }

  /**
   * The dock for the chat page. `opts`: `document`; `host`, the element it is put in (the chat
   * column); `messages`, the message list it stands at the top right of; `i18n`; `narrow()`, true
   * on a phone-sized window (the box starts as the tab); `onGoTo(origin)`, which opens the video's
   * channel when another is open; `findCard(url)`, the card drawn for the video's address, or null;
   * `onBack(video, line)`, told when the video went back into its card (to watch it again).
   */
  function create(opts) {
    var doc = opts.document;
    var host = opts.host;
    var i18n = opts.i18n;
    var narrow = typeof opts.narrow === 'function' ? opts.narrow : function () { return false; };
    var onGoTo = typeof opts.onGoTo === 'function' ? opts.onGoTo : function () {};
    var findCard = typeof opts.findCard === 'function' ? opts.findCard : function () { return null; };
    var onBack = typeof opts.onBack === 'function' ? opts.onBack : function () {};
    var box = null;
    var tab = null;
    var from = null;
    var picture = null;
    // What it holds: { video, url, card, spot (the line in its card), origin, label }.
    var held = null;
    var minimized = false;
    // Go to message asked before the card was drawn (a channel opening): scrolled to once it is.
    var goToWaiting = false;

    /** A button with its words and its hint by key. */
    function control(cls, key, hintKey, onPress) {
      var b = doc.createElement('button');
      b.type = 'button';
      b.className = cls;
      b.textContent = words(i18n, key);
      b.setAttribute('data-i18n', key);
      b.setAttribute('data-i18n-tip', hintKey);
      b.setAttribute('data-tip-desc', words(i18n, hintKey));
      b.addEventListener('click', onPress);
      return b;
    }

    function build() {
      if (box) return;
      box = doc.createElement('div');
      box.className = 'chat-video-dock';
      box.setAttribute('role', 'region');
      box.setAttribute('aria-label', words(i18n, 'chat_video_dock.label'));
      box.hidden = true;
      var head = doc.createElement('div');
      head.className = 'cvd-head';
      from = doc.createElement('span');
      from.className = 'cvd-from';
      head.appendChild(from);
      head.appendChild(control('cvd-minimize', 'chat_video_dock.minimize', 'chat_video_dock.minimize_hint', minimize));
      head.appendChild(control('cvd-close', 'common.close', 'chat_video_dock.close_hint', close));
      picture = doc.createElement('div');
      picture.className = 'cvd-picture';
      var foot = doc.createElement('div');
      foot.className = 'cvd-foot';
      foot.appendChild(control('cvd-go-to', 'chat_video_dock.go_to', 'chat_video_dock.go_to_hint', goTo));
      if (doc.pictureInPictureEnabled) foot.appendChild(control('cvd-float', 'chat_video_dock.pip', 'chat_video_dock.pip_hint', floatIt));
      box.appendChild(head);
      box.appendChild(picture);
      box.appendChild(foot);
      tab = control('chat-video-dock-tab', 'chat_video_dock.tab_playing', 'chat_video_dock.tab_hint', open);
      tab.hidden = true;
      host.appendChild(box);
      host.appendChild(tab);
    }

    /** Stand the box and the tab at the message list's top right, a small margin in. */
    function place() {
      var list = opts.messages;
      if (!list || typeof list.getBoundingClientRect !== 'function') return;
      var r = list.getBoundingClientRect();
      var gap = 8;
      var view = (root && root.innerWidth) || r.right;
      var width = Math.min(Math.max(240, r.width * 0.4), 420, Math.max(0, r.width - 2 * gap));
      for (var el of [box, tab]) {
        el.style.top = Math.round(r.top + gap) + 'px';
        el.style.right = Math.round(view - r.right + gap) + 'px';
      }
      box.style.width = Math.round(width) + 'px';
      box.style.maxHeight = Math.max(0, Math.round(r.height - 2 * gap)) + 'px';
      // The picture gets the height the line above it and the buttons under it leave.
      if (held) held.video.style.maxHeight = pictureRoom(r) + 'px';
    }

    /** How tall the picture may be in the list's box `r`: its height less the margins, the line
     *  saying where it came from and the buttons (about 84 px together), at most 315 px. */
    function pictureRoom(r) {
      return Math.max(0, Math.min(315, Math.round(r.height - 16 - 84)));
    }

    /** Whether the list is too small for a picture (narrower than 240 px, or under 72 px of
     *  picture): then the box starts as the tab, as the desktop app's does. */
    function tooSmall() {
      var list = opts.messages;
      if (!list || typeof list.getBoundingClientRect !== 'function') return false;
      var r = list.getBoundingClientRect();
      return r.width - 16 < 240 || pictureRoom(r) < 72;
    }

    /** The tab's words: playing or paused. */
    function sayState() {
      if (!tab || !held) return;
      var key = held.video.paused ? 'chat_video_dock.tab_paused' : 'chat_video_dock.tab_playing';
      tab.textContent = words(i18n, key);
      tab.setAttribute('data-i18n', key);
    }

    function show() {
      place();
      box.hidden = minimized;
      tab.hidden = !minimized;
      sayState();
    }

    function hide() {
      if (box) box.hidden = true;
      if (tab) tab.hidden = true;
    }

    /** The line left in a card in place of `video`, as big as the player was. */
    function lineFor(video) {
      var spot = doc.createElement('div');
      spot.className = 'file-video-away';
      spot.textContent = words(i18n, 'chat_video_dock.row_away');
      var h = video && video.offsetHeight;
      var w = video && video.offsetWidth;
      if (h) spot.style.height = h + 'px';
      if (w) spot.style.width = w + 'px';
      return spot;
    }

    /**
     * A playing `video` in `card` was scrolled out of view: it moves into the box, the same element,
     * and its card keeps a line saying where it is. `origin` ({channel}) and `label` ("From
     * #global") say where it came from. Answers the line (to watch for coming back), or null when it
     * is not taken (paused, ended, or the box holds one already).
     */
    function take(video, card, origin, label) {
      if (!video || held || video.paused || video.ended || !video.parentNode) return null;
      build();
      var spot = lineFor(video);
      video.parentNode.insertBefore(spot, video);
      picture.appendChild(video); // moved in one task: it plays on
      var url = card && card.dataset && card.dataset.fileUrl ? card.dataset.fileUrl : video.getAttribute('src');
      held = { video: video, url: url, card: card, spot: spot, origin: origin || {}, label: label || '' };
      from.textContent = held.label;
      from.title = held.label;
      video.addEventListener('play', sayState);
      video.addEventListener('pause', sayState);
      minimized = !!narrow() || tooSmall();
      show();
      return spot;
    }

    /** Let the held video go: its listeners off, the box hidden, nothing held. */
    function letGo() {
      var v = held.video;
      v.removeEventListener('play', sayState);
      v.removeEventListener('pause', sayState);
      v.style.maxHeight = ''; // its card's own rules again
      held = null;
      goToWaiting = false;
      hide();
      return v;
    }

    /**
     * The video goes back into its card, at its line (the line scrolled back into view, or Go to
     * message reached it): the same element, moved in one task, playing on. Answers the video, or
     * null when nothing is held or its line is not drawn.
     */
    function giveBack() {
      if (!held || !held.spot || !held.spot.parentNode) return null;
      var spot = held.spot;
      spot.parentNode.replaceChild(held.video, spot);
      var video = letGo();
      onBack(video, spot);
      return video;
    }

    /**
     * A card drawn again for the held video (its channel opened again, so the old card is gone):
     * its own new player gives way to a line, where the held video goes back. Answers the line (to
     * watch), or null when the card is not the held video's.
     */
    function adopt(card) {
      if (!held || !card || card === held.card || !card.dataset || card.dataset.fileUrl !== held.url) return null;
      if (held.card && held.card.isConnected && held.spot && held.spot.isConnected) return null;
      var fresh = typeof card.querySelector === 'function' ? card.querySelector('video') : null;
      if (!fresh || !fresh.parentNode) return null;
      var spot = lineFor(held.video);
      fresh.parentNode.replaceChild(spot, fresh);
      held.card = card;
      held.spot = spot;
      if (goToWaiting) {
        goToWaiting = false;
        reach(card);
      }
      return spot;
    }

    /** Scroll `card` into view and give the video back there. */
    function reach(card) {
      if (typeof card.scrollIntoView === 'function') card.scrollIntoView({ block: 'center' });
      giveBack();
    }

    function minimize() {
      if (!held) return;
      minimized = true;
      show();
    }

    function open() {
      if (!held) return;
      minimized = false;
      show();
    }

    /** Close: the video stops and goes back into its card (or goes, when its card is not drawn). */
    function close() {
      if (!held) return;
      var v = held.video;
      try { v.pause(); } catch (e) { /* nothing to pause */ }
      if (held.spot && held.spot.parentNode) held.spot.parentNode.replaceChild(v, held.spot);
      else if (v.parentNode) v.parentNode.removeChild(v);
      letGo();
    }

    /** Go to message: the video's channel (app.js), its card scrolled into view, the video back. */
    function goTo() {
      if (!held) return;
      onGoTo(held.origin);
      var card = held.card && held.card.isConnected ? held.card : findCard(held.url);
      if (card && card !== held.card) adopt(card);
      if (card && held && held.card === card) reach(card);
      else if (held) goToWaiting = true;
    }

    /** The browser's own floating picture-in-picture, where it has one. */
    function floatIt() {
      if (held && typeof held.video.requestPictureInPicture === 'function') held.video.requestPictureInPicture().catch(function () { /* refused: the box stays */ });
    }

    return {
      take: take,
      giveBack: giveBack,
      adopt: adopt,
      minimize: minimize,
      open: open,
      close: close,
      goTo: goTo,
      place: function () { if (held) show(); },
      /** The video held, or null. */
      held: function () { return held ? held.video : null; },
      /** The line in the held video's card, or null. */
      line: function () { return held ? held.spot : null; },
      minimized: function () { return !!held && minimized; },
      /** The box and the tab (tests). */
      parts: function () { return { box: box, tab: tab, from: from, picture: picture }; },
    };
  }

  var api = { create: create };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (root) root.HosVideoDock = api;
})(typeof window !== 'undefined' ? window : null);
