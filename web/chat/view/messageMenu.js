// view/messageMenu.js, a message's right-click menu (Track W step 3, 2026-10-22).
//
// Mirrors native's context menu on a message row (src/gui/pages/chat.rs, `context_menu` on the
// row): Copy text, ↩ Quote / reply, Pin message, Edit, Delete, then, on someone else's message,
// Report and Block, in plain words. The web keeps one entry the desktop app does not have, Pin for
// me (its personal pin, kept in this browser).
//
// Each entry presses the row's own button for it, the .msg-actions bar app.js builds on every row,
// so the menu can never offer what the row's own rules left out: a private row (a P2P group, the
// scratch pad; BUG-178) has no Pin, Edit or Delete here either, and Delete on someone else's
// message only for an admin or a moderator. Copy, Report and Block come from the caller
// (app.js addChatMessage), which knows the words and the person.
//
// The menu is the page's one context menu element (#user-context-menu, also the member menu, in
// chat-ui.js), so a click anywhere else closes it the same way. It opens from the keyboard too:
// the Menu key or Shift+F10 on a focused control of the row (its Þ) opens it at that control,
// the focus on its first entry; the arrow keys, Home and End move along it; Escape closes it.
// A right-click on a link, a picture, a player, or on words selected in the row is left to the
// browser's own menu (open or copy a link, save a picture, copy the selection).
(function () {
  // What the menu offers for this row. row: the .message element (its .msg-actions buttons
  // decide Reply, Pin, Pin for me, Edit and Delete); o: { text, person } (person: the sender, or
  // null on my own message, a bot's or a bridged one).
  function messageMenuItems(row, o) {
    o = o || {};
    var find = function (sel) { return row && typeof row.querySelector === 'function' ? row.querySelector(sel) : null; };
    var items = [];
    if (o.text) items.push({ key: 'copy', label: 'Copy text' });
    // U+FE0E: the arrow as text, never a colour emoji tile (view/pillPopup.js says why).
    if (find('.reply-btn')) items.push({ key: 'reply', label: '\u21A9\uFE0E Quote / reply', press: '.reply-btn' });
    if (find('.pin-btn')) items.push({ key: 'pin', label: 'Pin message', press: '.pin-btn' });
    if (find('.mypin-btn')) items.push({ key: 'mypin', label: 'Pin for me', press: '.mypin-btn' });
    if (find('.edit-btn')) items.push({ key: 'edit', label: 'Edit', press: '.edit-btn' });
    var del = find('.delete-btn');
    if (del) {
      // Its title says whose delete it is; the site's hover hints (shell.js) move a title to
      // data-native-title, so read either.
      var said = del.title || (del.getAttribute && (del.getAttribute('title') || del.getAttribute('data-native-title'))) || '';
      var mod = /moderator/i.test(said);
      items.push({ key: 'delete', label: mod ? 'Delete (moderator)' : 'Delete', press: '.delete-btn' });
    }
    if (o.person) {
      items.push({ sep: true });
      items.push({ key: 'report', label: 'Report' });
      items.push({ key: 'block', label: 'Block' });
    }
    return items;
  }

  // Native's menu ignores where it was opened; the browser's own menu is the better one on a
  // link, a picture, a player, a field, or a selection inside the row.
  function leaveToBrowser(e, row) {
    var t = e && e.target;
    if (t && typeof t.closest === 'function' && t.closest('a, img, video, audio, input, textarea, select')) return true;
    var sel = typeof window.getSelection === 'function' ? window.getSelection() : null;
    if (sel && String(sel).trim() && sel.anchorNode && typeof row.contains === 'function' && row.contains(sel.anchorNode)) return true;
    return false;
  }

  function menuEl() { return document.getElementById('user-context-menu'); }

  function entries(list) {
    return Array.prototype.filter.call(list.childNodes, function (c) { return c.getAttribute && c.getAttribute('role') === 'menuitem'; });
  }

  function close(menu, back) {
    menu.classList.remove('open');
    if (back && typeof back.focus === 'function') back.focus();
  }

  function run(item, row, o) {
    if (item.key === 'copy') {
      if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
        navigator.clipboard.writeText(o.text).catch(function () {});
      }
      return;
    }
    if (item.key === 'report') { if (o.person && typeof o.person.report === 'function') o.person.report(); return; }
    if (item.key === 'block') { if (o.person && typeof o.person.block === 'function') o.person.block(); return; }
    var btn = row.querySelector(item.press);
    if (btn) btn.click();
  }

  function showMessageMenu(e, row, o) {
    var menu = menuEl();
    if (!menu) return;
    var items = messageMenuItems(row, o);
    if (!items.length) return;
    if (e && e.preventDefault) e.preventDefault();
    if (e && e.stopPropagation) e.stopPropagation();
    if (typeof window.closePillPopup === 'function') window.closePillPopup(false);
    var back = document.activeElement;
    // The entries sit in a list of their own inside the shared element, so the member menu
    // (which writes the element's HTML) never inherits this one's role or keys.
    var list = document.createElement('div');
    list.className = 'ctx-list';
    list.setAttribute('role', 'menu');
    list.setAttribute('aria-label', 'Message');
    menu.replaceChildren(list);
    // Opened with the mouse, the focus stays where it was, so the list's own keys never see a
    // key: Escape closes it from anywhere, and the arrows step into it. Seen 2026-10-22 in the
    // browser: Escape left a right-clicked menu open.
    var onDocKey = function (ev) {
      if (!menu.classList.contains('open') || list.parentNode !== menu) { document.removeEventListener('keydown', onDocKey, true); return; }
      if (list.contains(document.activeElement)) return;
      if (ev.key === 'Escape') {
        if (ev.preventDefault) ev.preventDefault();
        done(null);
      } else if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
        if (ev.preventDefault) ev.preventDefault();
        var all = entries(list);
        var to = ev.key === 'ArrowDown' ? all[0] : all[all.length - 1];
        if (to) to.focus();
      }
    };
    function done(focusBack) {
      document.removeEventListener('keydown', onDocKey, true);
      close(menu, focusBack);
    }
    document.addEventListener('keydown', onDocKey, true);
    items.forEach(function (item) {
      if (item.sep) {
        var s = document.createElement('div');
        s.className = 'ctx-sep';
        s.setAttribute('role', 'separator');
        list.appendChild(s);
        return;
      }
      var d = document.createElement('div');
      d.className = 'ctx-item' + (item.key === 'report' || item.key === 'delete' ? ' ctx-item-danger' : '');
      d.setAttribute('role', 'menuitem');
      d.setAttribute('tabindex', '-1');
      d.dataset.key = item.key;
      d.textContent = item.label;
      d.addEventListener('click', function (ev) {
        if (ev && ev.stopPropagation) ev.stopPropagation();
        done(null);
        run(item, row, o);
      });
      list.appendChild(d);
    });
    list.addEventListener('keydown', function (ev) {
      var all = entries(list);
      var at = all.indexOf(document.activeElement);
      var to = -1;
      if (ev.key === 'ArrowDown') to = at < 0 ? 0 : (at + 1) % all.length;
      else if (ev.key === 'ArrowUp') to = at <= 0 ? all.length - 1 : at - 1;
      else if (ev.key === 'Home') to = 0;
      else if (ev.key === 'End') to = all.length - 1;
      else if (ev.key === 'Enter' || ev.key === ' ') {
        if (at >= 0) { ev.preventDefault(); all[at].click(); }
        return;
      } else if (ev.key === 'Escape' || ev.key === 'Tab') {
        ev.preventDefault();
        done(back);
        return;
      }
      if (to >= 0) { ev.preventDefault(); all[to].focus(); }
    });

    // From the keyboard (the Menu key, Shift+F10) a contextmenu event has no pointer position:
    // open at the control that has the focus, and put the focus on the first entry.
    var fromKeys = !e || (!e.clientX && !e.clientY);
    var x = e && e.clientX;
    var y = e && e.clientY;
    if (fromKeys) {
      var at = (back && back !== document.body && typeof back.getBoundingClientRect === 'function' && row.contains(back)) ? back : row;
      var r = at.getBoundingClientRect();
      x = r.left;
      y = r.bottom;
    }
    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
    menu.classList.add('open');
    // Keep it inside the window (the member menu's own fix, chat-ui.js).
    var rect = menu.getBoundingClientRect();
    var vw = window.innerWidth || 0;
    var vh = window.innerHeight || 0;
    if (rect.right > vw) menu.style.left = Math.max(4, vw - rect.width - 8) + 'px';
    if (rect.bottom > vh) menu.style.top = Math.max(4, vh - rect.height - 8) + 'px';
    if (fromKeys) { var first = entries(list)[0]; if (first) first.focus(); }
  }

  // Hook a drawn row. o: { text, person: { report(), block() } | null }.
  function wireMessageMenu(row, o) {
    if (!row || typeof row.addEventListener !== 'function') return;
    row.addEventListener('contextmenu', function (e) {
      if (leaveToBrowser(e, row)) return;
      showMessageMenu(e, row, o);
    });
  }

  window.messageMenuItems = messageMenuItems;
  window.showMessageMenu = showMessageMenu;
  window.wireMessageMenu = wireMessageMenu;
})();
