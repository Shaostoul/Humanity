// view/pillPopup.js, the expanded timestamp pill (Track W step 3, 2026-10-22).
//
// Mirrors native's `pill_expand` and `pill_ts_expand` Areas (src/gui/pages/chat.rs, the message
// loop). The Þ in a message's timestamp pill is a pull tab: hovering it opens a bar that reads as
// the pill growing to the right, holding the message's functions (↩ Reply, Pin, Edit), a Þ, the
// quick reactions and ∞ for all of them; and the full date and time, "YYYY-MM-DD HH:MM:SS UTC",
// shows to the left of the Þ. The bar stays open while the pointer is on the pill or on the bar,
// so the pointer can travel across the pill to it, and closes once the pointer has left both.
// Hovering the message's words never opens it (native: only the Þ does, so text stays easy to
// select and copy). Until 2026-10-22 the web had a separate emoji strip above the row's right
// edge instead, which the message list's top edge cut in half on the first message.
//
// What the web adds, because a browser has more than a mouse:
//   - a click or a tap on the Þ opens it too (a touch screen has no hover);
//   - Enter or Space on the focused Þ opens it with the focus on its first button; the arrow keys,
//     Home and End move along its buttons; Escape closes it and gives the focus back to the Þ;
//   - it hangs off <body> (position: fixed), so the message list's scrolling edge never cuts it,
//     and where there is no room to the right of the pill (a phone) it opens under the pill.
//
// A private row (a P2P group, the scratch pad; native is_private_channel) offers Reply alone: no
// reaction, pin or edit, each of which would send the message's text or who reacted to it to the
// server (BUG-178). The caller says which functions the row has (app.js addChatMessage), from the
// same rules its other controls follow; Pin is offered to admins and moderators only, the people
// the server lets pin (native offers it to everyone and the server refuses the rest).
//
// Exposed: wirePillPopup(row, opts), openPillPopup(row, how), closePillPopup(giveFocusBack),
// pillPopupOpen(), and the pure parts the tests check: pillPopupItems, pillPopupPlace,
// pillTimestampPlace, fullTimestamp.
(function () {
  // How long the pointer may be off both the pill and the bar before it closes: long enough to
  // cross the gap between them, short enough to feel like native's immediate close.
  var CLOSE_DELAY_MS = 250;
  var EDGE = 8; // keep this far inside the window

  var open = null; // the one open: { row, pill, thorn, bar, ts, opts, how, timer, hovering, onDoc }

  function pad(n) { return (n < 10 ? '0' : '') + n; }

  // native format_full_timestamp: always UTC, always the full form, whatever the display setting.
  function fullTimestamp(ms) {
    var d = new Date(Number(ms));
    if (!isFinite(d.getTime())) return '';
    return d.getUTCFullYear() + '-' + pad(d.getUTCMonth() + 1) + '-' + pad(d.getUTCDate()) + ' '
      + pad(d.getUTCHours()) + ':' + pad(d.getUTCMinutes()) + ':' + pad(d.getUTCSeconds()) + ' UTC';
  }

  // What the bar holds, in native's order. o: { privateRow, canPin, canEdit, top, all }.
  function pillPopupItems(o) {
    o = o || {};
    // U+FE0E asks for the arrow as text: the site's font has no ↩, and the fallback the browser
    // finds first is often a colour emoji font, which draws a blue keycap tile.
    var functions = [{ key: 'reply', label: '\u21A9\uFE0E', name: 'Reply', tip: 'Quote this message in your reply.' }];
    if (!o.privateRow && o.canPin) functions.push({ key: 'pin', label: 'Pin', name: 'Pin message', tip: 'Pins it at the top of this channel, for everyone.' });
    if (!o.privateRow && o.canEdit) functions.push({ key: 'edit', label: 'Edit', name: 'Edit message', tip: 'Change what you wrote.' });
    return {
      functions: functions,
      reactions: o.privateRow ? [] : (o.top || []).slice(),
      more: o.privateRow ? [] : (o.all || []).slice(),
    };
  }

  // Native anchors the bar at the pill's right edge, centred on the pill. Where that runs off the
  // window, under the pill (above it when there is no room under), kept inside the window.
  // pill: a rect {left, right, top, bottom}; size: {w, h}; view: {w, h}. Answers {left, top, below}.
  function pillPopupPlace(pill, size, view) {
    var mid = (pill.top + pill.bottom) / 2;
    if (pill.right + size.w <= view.w - EDGE) {
      var top = Math.max(EDGE, Math.min(mid - size.h / 2, view.h - EDGE - size.h));
      return { left: pill.right, top: top, below: false };
    }
    var left = Math.max(EDGE, Math.min(pill.left, view.w - EDGE - size.w));
    var under = pill.bottom + 2;
    var y = under + size.h <= view.h - EDGE ? under : Math.max(EDGE, pill.top - 2 - size.h);
    return { left: left, top: y, below: true };
  }

  // The full time, its right edge 4 px left of the Þ and centred on it (native pill_ts_expand),
  // kept inside the window. Where there is no room left of the Þ (a phone, a short name) it
  // would cover the Þ itself, so it goes just above it instead.
  function pillTimestampPlace(thorn, size, view) {
    var top = Math.max(EDGE, Math.min((thorn.top + thorn.bottom) / 2 - size.h / 2, view.h - EDGE - size.h));
    var left = thorn.left - 4 - size.w;
    if (left >= EDGE) return { left: left, top: top, above: false };
    return {
      left: Math.max(EDGE, Math.min(thorn.right - size.w, view.w - EDGE - size.w)),
      top: Math.max(EDGE, thorn.top - 4 - size.h),
      above: true,
    };
  }

  function make(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }

  // A button named for screen readers. With `tip`, the site's hover hint (shell.js) says its name
  // and that line, as native's on_hover_text names its functions; without one it shows no hint
  // (data-no-tip), as native shows none on a reaction.
  function button(cls, label, name, tip) {
    var b = make('button', cls, label);
    b.setAttribute('type', 'button');
    b.setAttribute('aria-label', name);
    if (tip) b.setAttribute('data-tip-desc', tip);
    else b.setAttribute('data-no-tip', '');
    return b;
  }

  // The bar's buttons that take part in arrow-key moves: the row of them, not the ∞ panel's.
  function rowButtons(bar) {
    var row = bar.firstChild;
    if (!row) return [];
    return Array.prototype.filter.call(row.childNodes, function (c) { return c.tagName === 'BUTTON'; });
  }

  function build(opts) {
    var items = pillPopupItems(opts);
    var full = fullTimestamp(opts.ts);
    var bar = make('div', 'pill-popup');
    bar.id = 'pill-popup';
    bar.setAttribute('role', 'toolbar');
    bar.setAttribute('aria-label', full ? 'Message actions, sent ' + full : 'Message actions');

    var row = make('div', 'pill-popup-row');
    items.functions.forEach(function (f) {
      var b = button('pill-popup-fn pill-popup-' + f.key, f.label, f.name, f.tip);
      b.dataset.fn = f.key;
      b.addEventListener('click', function (e) {
        if (e && e.stopPropagation) e.stopPropagation();
        closePillPopup(false);
        var run = f.key === 'reply' ? opts.onReply : f.key === 'pin' ? opts.onPin : opts.onEdit;
        if (typeof run === 'function') run();
      });
      row.appendChild(b);
    });
    var sep = make('span', 'pill-popup-thorn', 'Þ');
    sep.setAttribute('aria-hidden', 'true');
    row.appendChild(sep);

    function react(emoji) {
      if (typeof opts.onReact === 'function') opts.onReact(emoji);
      // The pill grew or shrank by a badge: keep the bar at its edge.
      place();
    }
    items.reactions.forEach(function (emoji) {
      var b = button('pill-popup-react', emoji, 'React with ' + emoji);
      b.dataset.emoji = emoji;
      b.addEventListener('click', function (e) {
        if (e && e.stopPropagation) e.stopPropagation();
        react(emoji);
      });
      row.appendChild(b);
    });
    bar.appendChild(row);

    if (items.more.length) {
      var more = button('pill-popup-more', '∞', 'All reactions', 'Every reaction, to pick from.');
      more.setAttribute('aria-expanded', 'false');
      row.appendChild(more);
      var panel = make('div', 'pill-popup-all');
      panel.hidden = true;
      panel.setAttribute('role', 'group');
      panel.setAttribute('aria-label', 'All reactions');
      items.more.forEach(function (emoji) {
        var b = button('pill-popup-react', emoji, 'React with ' + emoji);
        b.dataset.emoji = emoji;
        b.addEventListener('click', function (e) {
          if (e && e.stopPropagation) e.stopPropagation();
          panel.hidden = true;
          more.setAttribute('aria-expanded', 'false');
          react(emoji);
          more.focus();
        });
        panel.appendChild(b);
      });
      more.addEventListener('click', function (e) {
        if (e && e.stopPropagation) e.stopPropagation();
        panel.hidden = !panel.hidden;
        more.setAttribute('aria-expanded', String(!panel.hidden));
        place();
        if (!panel.hidden && panel.firstChild) panel.firstChild.focus();
      });
      bar.appendChild(panel);
    }

    bar.addEventListener('keydown', function (e) {
      var list = rowButtons(bar);
      var at = list.indexOf(document.activeElement);
      var to = -1;
      if (e.key === 'ArrowRight') to = at < 0 ? 0 : Math.min(list.length - 1, at + 1);
      else if (e.key === 'ArrowLeft') to = at < 0 ? 0 : Math.max(0, at - 1);
      else if (e.key === 'Home') to = 0;
      else if (e.key === 'End') to = list.length - 1;
      if (to >= 0 && list[to]) {
        if (e.preventDefault) e.preventDefault();
        list[to].focus();
      }
    });

    var ts = null;
    if (full) {
      ts = make('div', 'pill-ts-expand', full);
      ts.setAttribute('aria-hidden', 'true');
    }
    return { bar: bar, ts: ts };
  }

  function rectOf(el) {
    var r = el.getBoundingClientRect();
    return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, w: r.width, h: r.height };
  }

  function view() {
    var de = document.documentElement || {};
    return { w: window.innerWidth || de.clientWidth || 0, h: window.innerHeight || de.clientHeight || 0 };
  }

  // Put the bar and the full time where they belong for the pill as it is now. A pill no longer
  // in the page (the message was deleted, the channel changed) closes it.
  function place() {
    if (!open) return;
    if (open.row.isConnected === false) { closePillPopup(false); return; }
    var v = view();
    var pill = rectOf(open.pill);
    if (pill.bottom < 0 || pill.top > v.h) { closePillPopup(false); return; }
    var b = rectOf(open.bar);
    var at = pillPopupPlace(pill, { w: b.w, h: b.h }, v);
    open.bar.style.left = Math.round(at.left) + 'px';
    open.bar.style.top = Math.round(at.top) + 'px';
    open.bar.classList.toggle('pill-popup-below', !!at.below);
    if (open.ts) {
      var t = rectOf(open.ts);
      var tp = pillTimestampPlace(rectOf(open.thorn), { w: t.w, h: t.h }, v);
      open.ts.style.left = Math.round(tp.left) + 'px';
      open.ts.style.top = Math.round(tp.top) + 'px';
    }
  }

  function inside(node, el) {
    return !!(el && node && (el === node || (typeof el.contains === 'function' && el.contains(node))));
  }

  function cancelClose() {
    if (open && open.timer) { clearTimeout(open.timer); open.timer = null; }
  }

  function closeSoon() {
    if (!open) return;
    cancelClose();
    var mine = open;
    mine.timer = setTimeout(function () {
      if (open === mine && !mine.hovering && !inside(document.activeElement, mine.bar)) closePillPopup(false);
    }, CLOSE_DELAY_MS);
  }

  // A pointer counts for hover only when it is a mouse or a pen: a finger's tap also sends
  // pointerenter and pointerleave, and leaving would close what the tap just opened.
  function isHoverPointer(e) { return !e || e.pointerType !== 'touch'; }

  // how: 'hover' (the pointer reached the Þ), 'click' (a click or a tap), 'key' (Enter or Space).
  function openPillPopup(row, how) {
    var opts = row && row._pillPopup;
    if (!opts) return;
    var thorn = row.querySelector('.ts-thorn');
    var pill = row.querySelector('.ts-pill');
    if (!thorn || !pill) return;
    if (open && open.row === row) {
      // Native: a click on the Þ has no action of its own; a hover-opened bar just stays open.
      if (how === 'hover') { cancelClose(); return; }
      if (open.how === 'hover') { open.how = how; if (how === 'key') focusFirst(); return; }
      closePillPopup(true);
      return;
    }
    closePillPopup(false);
    var built = build(opts);
    open = { row: row, pill: pill, thorn: thorn, bar: built.bar, ts: built.ts, opts: opts, how: how, timer: null, hovering: how === 'hover' };
    var mine = open;
    if (built.ts) document.body.appendChild(built.ts);
    document.body.appendChild(built.bar);
    thorn.setAttribute('aria-expanded', 'true');
    thorn.classList.add('ts-thorn-live');
    // The reactions in the same emoji pictures as the pill's; never the ↩ (twemoji would turn
    // it into a blue keycap tile, where native draws the plain arrow).
    if (window.twemoji) {
      Array.prototype.forEach.call(built.bar.querySelectorAll('.pill-popup-react'), function (b) { window.twemoji.parse(b); });
    }

    var enter = function (e) { if (open === mine && isHoverPointer(e)) { mine.hovering = true; cancelClose(); } };
    var leave = function (e) { if (open === mine && isHoverPointer(e)) { mine.hovering = false; closeSoon(); } };
    built.bar.addEventListener('pointerenter', enter);
    built.bar.addEventListener('pointerleave', leave);
    pill.addEventListener('pointerenter', enter);
    pill.addEventListener('pointerleave', leave);
    mine.unhook = function () {
      pill.removeEventListener('pointerenter', enter);
      pill.removeEventListener('pointerleave', leave);
    };
    built.bar.addEventListener('focusout', function () {
      setTimeout(function () {
        if (open !== mine) return;
        var f = document.activeElement;
        if (!inside(f, mine.bar) && f !== mine.thorn && !mine.hovering) closePillPopup(false);
      }, 0);
    });

    // Anywhere else pressed closes it; Escape closes it and gives the focus back to the Þ; the
    // page scrolling or resizing moves it with the pill.
    mine.onDown = function (e) {
      if (open !== mine) return;
      if (inside(e.target, mine.bar) || inside(e.target, mine.pill)) return;
      closePillPopup(false);
    };
    mine.onKey = function (e) {
      if (open === mine && e.key === 'Escape') {
        if (e.preventDefault) e.preventDefault();
        closePillPopup(true);
      }
    };
    mine.onMove = function () { place(); };
    document.addEventListener('pointerdown', mine.onDown, true);
    document.addEventListener('keydown', mine.onKey, true);
    document.addEventListener('scroll', mine.onMove, true);
    window.addEventListener('resize', mine.onMove);

    place();
    if (how === 'key') focusFirst();
  }

  function focusFirst() {
    if (!open) return;
    var first = rowButtons(open.bar)[0];
    if (first) first.focus();
  }

  function closePillPopup(giveFocusBack) {
    if (!open) return;
    var was = open;
    open = null;
    if (was.timer) clearTimeout(was.timer);
    if (was.unhook) was.unhook();
    document.removeEventListener('pointerdown', was.onDown, true);
    document.removeEventListener('keydown', was.onKey, true);
    document.removeEventListener('scroll', was.onMove, true);
    window.removeEventListener('resize', was.onMove);
    var hadFocus = inside(document.activeElement, was.bar);
    if (was.bar.parentNode) was.bar.parentNode.removeChild(was.bar);
    if (was.ts && was.ts.parentNode) was.ts.parentNode.removeChild(was.ts);
    was.thorn.setAttribute('aria-expanded', 'false');
    was.thorn.classList.remove('ts-thorn-live');
    if (giveFocusBack || hadFocus) was.thorn.focus();
  }

  function pillPopupOpen() { return open ? open.row : null; }

  // Hook a drawn row's Þ. opts: { ts, privateRow, canPin, canEdit, top, all, onReply, onPin,
  // onEdit, onReact } (top and all may be functions, read when the bar opens, since the palette
  // arrives after the first rows are drawn).
  function wirePillPopup(row, opts) {
    var thorn = row && row.querySelector('.ts-thorn');
    if (!thorn) return;
    row._pillPopup = {
      ts: opts.ts,
      privateRow: !!opts.privateRow,
      canPin: !!opts.canPin,
      canEdit: !!opts.canEdit,
      get top() { return typeof opts.top === 'function' ? opts.top() : opts.top; },
      get all() { return typeof opts.all === 'function' ? opts.all() : opts.all; },
      onReply: opts.onReply,
      onPin: opts.onPin,
      onEdit: opts.onEdit,
      onReact: opts.onReact,
    };
    thorn.addEventListener('pointerenter', function (e) { if (isHoverPointer(e)) openPillPopup(row, 'hover'); });
    thorn.addEventListener('click', function (e) {
      if (e && e.stopPropagation) e.stopPropagation();
      // A click the keyboard made (Enter, Space) has no pointer position and detail 0.
      openPillPopup(row, e && e.detail === 0 ? 'key' : 'click');
    });
  }

  window.fullTimestamp = fullTimestamp;
  window.pillPopupItems = pillPopupItems;
  window.pillPopupPlace = pillPopupPlace;
  window.pillTimestampPlace = pillTimestampPlace;
  window.wirePillPopup = wirePillPopup;
  window.openPillPopup = openPillPopup;
  window.closePillPopup = closePillPopup;
  window.pillPopupOpen = pillPopupOpen;
  // The pill changed size (a reaction came in): keep an open bar at its edge.
  window.repositionPillPopup = place;
})();
