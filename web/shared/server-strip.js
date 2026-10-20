// ── server-strip.js ───────────────────────────────────────────────────────
// The server strip (2026-10-12, docs/design/media-round-5-plan.md 3.11, lane 1F):
// a bar fixed just under the top menu that says which server this page is on,
// so a person who scrolls far down a long page can still see it. The
// operator's ask, in his words: "add like a fixed label just below the top
// menu that says what server we're looking at. That way even if I scroll down
// the huge blue section I can still see that I'm on united-humanity.us".
//
// It is the website's mirror of the desktop app's strip
// (src/gui/pages/server_strip.rs, lane 1E), with the same words:
//   "{server name}  {host}"   a dot   Switch
//
// What it does:
//   - The NAME comes from GET /api/server-info (its `name` field: the name the
//     admins set in Server Settings, else the server's configured name). The
//     HOST is location.host, so a port shows too (localhost:8099).
//   - The DOT is green once /api/server-info answered (this server is there),
//     red when it did not, muted while asking. The website has no lasting
//     connection of its own on every page, so "connected" here means "this
//     server answered".
//   - SWITCH lists the servers this one is linked with
//     (GET /api/federation/servers, asked the first time the list is opened),
//     each a link to the SAME page on that server (https://<their host>/chat
//     and so on; an item's page /files/<n> goes to /files, because item
//     numbers differ from server to server), with the line "You sign in on
//     each server's website separately." A browser keeps nothing across sites,
//     so unlike the desktop app (which lists your saved servers) the website
//     can only offer the servers this one knows (decision 6 of the plan).
//   - Only https addresses are linked: an address that is not https (a
//     `javascript:` one included) is left out, never turned into a link.
//   - While the protected setup is on on this device (10h,
//     docs/design/blocking-and-safe-mode.md), other servers are hidden here as
//     they are in Chat's server list, since each leads to another server's
//     public rooms.
//
// Where it sits: right after shell.js's .hub-nav-spacer, as
// `position: sticky; top: var(--hos-nav-h)`. shell.js sets --hos-nav-h on
// <html> to the fixed nav's real height every time the nav changes size (a
// wrapped two-row nav included), so the strip stops exactly under the nav
// and the page scrolls beneath it. shell.js also puts its round help button
// under the strip (its `place()` measures #hos-server-strip when there is
// one), and the strip keeps its right end clear for that button.
//
// Safety: everything is built with createElement and textContent, never
// innerHTML, so a server name such as `<img src=x onerror=alert(1)>` is shown
// as the words it is. Colours are theme tokens (web/shared/theme.css) only.
//
// Pages that load it (each with ?v= so a change reaches browsers past the
// service worker's cache-first rule): /files (web/pages/files.html), /watch,
// /chat (web/chat/index.html, right after shell.js) and /admin.
//
// A classic script in the browser (it mounts itself), a CommonJS module under
// Node, where scripts/tests/server-strip-web.test.js holds it to the plan.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // Every word the strip puts on screen. The desktop strip says the same.
  const STRIP_WORDS = {
    region: 'The server this page is on',
    switch: 'Switch',
    switchHint: 'Open this page on another server',
    menuHead: 'Servers linked with this one',
    signInNote: "You sign in on each server's website separately.",
    loading: 'Looking for linked servers...',
    none: 'This server is not linked with any other server yet.',
    failed: 'Could not get the list of linked servers. Try again in a moment.',
    hidden: 'Other servers are hidden by the protected setup.',
    dotChecking: 'Checking this server',
    dotConnected: 'Connected',
    dotUnreachable: 'This server did not answer',
    linkedOnline: 'Linked and reachable',
    linkedNotSeen: 'Linked, not reached lately',
  };

  // Where the protected setup lives on this device (web/shared/protected.js
  // PROTECTED_STORAGE_KEY; settings-app.js reads the same key the same way).
  const PROTECTED_STORAGE_KEY = 'humanity_protected_setup';

  // The strip's styles: theme tokens only. `top` falls back to 0 on a page
  // without shell.js (no nav to sit under).
  const STRIP_CSS = [
    '.hos-server-strip {',
    '  position: sticky;',
    '  top: var(--hos-nav-h, 0px);',
    /* Above the page's own content as it scrolls beneath, below Chat's
       drawers (10) and every dialog, so those still cover it. */
    '  z-index: 9;',
    '  display: flex; align-items: center; gap: var(--space-md);',
    '  min-height: 28px; box-sizing: border-box; flex-shrink: 0;',
    /* The right 60px stay clear: shell.js's round help button (38px, 44px on
       a phone, 10px from the edge) sits just under the strip there, so
       Switch and its list never open beneath it. */
    '  padding: var(--space-xs) 60px var(--space-xs) var(--space-lg);',
    '  background: var(--bg-secondary);',
    '  border-bottom: 1px solid var(--border);',
    '  color: var(--text-secondary);',
    '  font-size: var(--text-sm);',
    '}',
    '.hos-strip-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: var(--text-muted); }',
    '.hos-strip-dot[data-state="connected"] { background: var(--success); }',
    '.hos-strip-dot[data-state="unreachable"] { background: var(--danger); }',
    /* white-space: pre keeps the two spaces between the name and the host,
       as the desktop strip draws them; a long name is cut with an ellipsis. */
    '.hos-strip-label { flex: 1; min-width: 0; white-space: pre; overflow: hidden; text-overflow: ellipsis; }',
    '.hos-strip-name { color: var(--text); font-weight: 600; }',
    '.hos-strip-host { color: var(--text-muted); }',
    '.hos-strip-switch-wrap { position: relative; flex-shrink: 0; }',
    '.hos-strip-switch { background: var(--bg-input); color: var(--text); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 2px var(--space-md); font: inherit; cursor: pointer; }',
    '.hos-strip-switch:hover, .hos-strip-switch:focus-visible { border-color: var(--accent); outline: none; }',
    '.hos-strip-menu { position: absolute; right: 0; top: calc(100% + 4px); min-width: 260px; max-width: min(360px, calc(100vw - 32px)); background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: var(--space-md); }',
    '.hos-strip-menu[hidden] { display: none; }',
    '.hos-strip-menu-head { margin: 0 0 var(--space-sm); color: var(--text-muted); font-size: var(--text-xs); }',
    '.hos-strip-list { list-style: none; margin: 0; padding: 0; }',
    '.hos-strip-list a { display: flex; align-items: center; gap: var(--space-sm); padding: var(--space-xs) var(--space-sm); border-radius: var(--radius-sm); color: var(--text); text-decoration: none; white-space: pre; overflow: hidden; text-overflow: ellipsis; }',
    '.hos-strip-list a:hover, .hos-strip-list a:focus-visible { background: var(--bg-hover); outline: none; }',
    '.hos-strip-list .hos-strip-dot[data-state="online"] { background: var(--success); }',
    '.hos-strip-note { margin: var(--space-sm) 0 0; color: var(--text-muted); font-size: var(--text-xs); white-space: normal; }',
  ].join('\n');

  // ── The pure rules ──

  /** The strip's words: "{server name}  {host}", or the host alone when the server gave no name. */
  function stripLabel(name, host) {
    const n = typeof name === 'string' ? name.trim() : '';
    const h = String(host == null ? '' : host);
    return n ? n + '  ' + h : h;
  }

  /**
   * The https origin of a linked server's address, or null when it must not be
   * linked: not a URL, not https (a `javascript:` or `data:` address, or plain
   * http), no host, or a user name or password in it. Only the origin is kept,
   * never a path the address carried.
   */
  function linkedOrigin(raw) {
    let u;
    try { u = new URL(String(raw == null ? '' : raw).trim()); } catch (e) { return null; }
    if (u.protocol !== 'https:') return null;
    if (!u.host || u.username || u.password) return null;
    return 'https://' + u.host;
  }

  /**
   * The same page's path on another server: this page's path, except that an
   * item's page (/files/<n>) becomes /files, because item numbers belong to
   * each server. Leading slashes are folded to one, so the path can never be
   * read as another host (`//elsewhere`). The query and the hash are left
   * behind: they hold this server's own ids.
   */
  function samePagePath(pathname) {
    const p = '/' + String(pathname == null ? '' : pathname).replace(/^\/+/, '');
    if (/^\/files\/\d+\/?$/.test(p)) return '/files';
    return p;
  }

  /**
   * Switch's links, from GET /api/federation/servers's answer: one per linked
   * server with an https address, never this server itself, each host once,
   * in the order the server listed them. `online` is the relay's own last word
   * on that server (`status: "online"`).
   */
  function switchLinks(servers, ownHost, pathname) {
    const path = samePagePath(pathname);
    const own = String(ownHost == null ? '' : ownHost).toLowerCase();
    const seen = new Set();
    const out = [];
    for (const s of Array.isArray(servers) ? servers : []) {
      if (!s || typeof s !== 'object') continue;
      const origin = linkedOrigin(s.url);
      if (!origin) continue;
      const host = origin.slice('https://'.length);
      if (host.toLowerCase() === own || seen.has(host)) continue;
      seen.add(host);
      out.push({
        name: typeof s.name === 'string' ? s.name.trim() : '',
        host,
        href: origin + path,
        online: s.status === 'online',
      });
    }
    return out;
  }

  /**
   * Does the protected setup on this device hide other servers? Read from what
   * the device keeps (`raw`, the stored text), the way web/shared/protected.js
   * reads it: absent or turned off (`on: false`) hides nothing; on, it hides
   * them unless its public-rooms rule is "all". Damaged text counts as on with
   * the safe rule, as protected.js treats it.
   */
  function protectedHidesServers(raw) {
    if (raw == null || raw === '') return false;
    let v = null;
    try { v = JSON.parse(raw); } catch (e) { v = null; }
    if (v && typeof v === 'object' && v.on === false) return false;
    const src = v && typeof v === 'object' ? v : {};
    const rooms = typeof src.public_rooms === 'string'
      ? src.public_rooms
      : (src.rules && typeof src.rules === 'object' ? src.rules.public_rooms : null);
    return rooms !== 'all';
  }

  // ── The strip itself ──

  /**
   * Build the strip and put it under the nav. `env`:
   *   document, location (host, pathname), fetch,
   *   protectedRaw()  the stored protected-setup text (or null),
   *   protectedHidesRooms()  optional: Chat's own answer, used when present.
   * Returns { strip, ready, openMenu, closeMenu }: `ready` settles once the
   * server named itself (or did not answer); `openMenu()` opens Switch's list
   * and settles once the list is drawn. Mounting twice returns the first strip.
   */
  function mountServerStrip(env) {
    const doc = env.document;
    const loc = env.location || {};
    const host = String(loc.host || '');
    const existing = doc.getElementById ? doc.getElementById('hos-server-strip') : null;
    if (existing && existing.__hosStrip) return existing.__hosStrip;

    const el = (tag, cls, text) => {
      const e = doc.createElement(tag);
      if (cls) e.className = cls;
      if (text != null) e.textContent = text;
      return e;
    };

    // The styles, once per page.
    if (!(doc.getElementById && doc.getElementById('hos-server-strip-style'))) {
      const style = el('style', '', STRIP_CSS);
      style.setAttribute('id', 'hos-server-strip-style');
      (doc.head || doc.body).appendChild(style);
    }

    const strip = el('div', 'hos-server-strip');
    strip.setAttribute('id', 'hos-server-strip');
    strip.setAttribute('role', 'region');
    strip.setAttribute('aria-label', STRIP_WORDS.region);

    // This server's dot.
    const dot = el('span', 'hos-strip-dot');
    dot.setAttribute('role', 'img');
    const setDot = (state) => {
      const words = state === 'connected' ? STRIP_WORDS.dotConnected
        : state === 'unreachable' ? STRIP_WORDS.dotUnreachable : STRIP_WORDS.dotChecking;
      dot.setAttribute('data-state', state);
      dot.setAttribute('aria-label', words);
      dot.setAttribute('title', words);
    };
    setDot('checking');

    // "{server name}  {host}": the name and the host each in their own span,
    // with the two spaces as text between them, so the label's text is
    // exactly the desktop strip's words.
    const label = el('span', 'hos-strip-label');
    const showName = (name) => {
      const n = typeof name === 'string' ? name.trim() : '';
      label.replaceChildren();
      if (n) {
        label.appendChild(el('span', 'hos-strip-name', n));
        label.appendChild(doc.createTextNode('  '));
      }
      label.appendChild(el('span', 'hos-strip-host', host));
      label.setAttribute('title', stripLabel(n, host));
    };
    showName('');

    // Switch, and its list.
    const wrap = el('span', 'hos-strip-switch-wrap');
    const button = el('button', 'hos-strip-switch', STRIP_WORDS.switch);
    button.setAttribute('type', 'button');
    button.setAttribute('title', STRIP_WORDS.switchHint);
    button.setAttribute('aria-expanded', 'false');
    button.setAttribute('aria-controls', 'hos-strip-menu');
    const menu = el('div', 'hos-strip-menu');
    menu.setAttribute('id', 'hos-strip-menu');
    menu.hidden = true;
    wrap.appendChild(button);
    wrap.appendChild(menu);

    strip.appendChild(dot);
    strip.appendChild(label);
    strip.appendChild(wrap);

    // Under the nav: right after shell.js's spacer, else at the top of the page.
    const spacer = doc.querySelector ? doc.querySelector('.hub-nav-spacer') : null;
    if (spacer && spacer.parentNode) spacer.parentNode.insertBefore(strip, spacer.nextSibling || null);
    else if (doc.body.firstChild) doc.body.insertBefore(strip, doc.body.firstChild);
    else doc.body.appendChild(strip);

    // The server's name. A failure leaves the host alone and the dot red.
    const ready = (async () => {
      try {
        const res = await env.fetch('/api/server-info');
        if (!res || !res.ok) throw new Error('HTTP ' + (res && res.status));
        const info = await res.json();
        showName(info && typeof info.name === 'string' ? info.name : '');
        setDot('connected');
      } catch (e) {
        showName('');
        setDot('unreachable');
      }
    })();

    // The linked servers, asked the first time the list opens; a failed ask
    // is asked again the next time.
    let linked = null;
    const drawMenu = (lines, links) => {
      menu.replaceChildren();
      menu.appendChild(el('p', 'hos-strip-menu-head', STRIP_WORDS.menuHead));
      if (links && links.length) {
        const ul = el('ul', 'hos-strip-list');
        for (const l of links) {
          const li = el('li');
          const a = el('a');
          a.setAttribute('href', l.href);
          // The other server is not told which page of this one the person was on.
          a.setAttribute('rel', 'noreferrer');
          const d = el('span', 'hos-strip-dot');
          d.setAttribute('data-state', l.online ? 'online' : 'unknown');
          d.setAttribute('role', 'img');
          d.setAttribute('aria-label', l.online ? STRIP_WORDS.linkedOnline : STRIP_WORDS.linkedNotSeen);
          a.appendChild(d);
          a.appendChild(doc.createTextNode(stripLabel(l.name, l.host)));
          li.appendChild(a);
          ul.appendChild(li);
        }
        menu.appendChild(ul);
      }
      for (const line of lines) menu.appendChild(el('p', 'hos-strip-note', line));
    };
    const hidesServers = () => {
      if (typeof env.protectedHidesRooms === 'function') {
        try { return !!env.protectedHidesRooms(); } catch (e) { return true; }
      }
      let raw = null;
      try { raw = env.protectedRaw ? env.protectedRaw() : null; } catch (e) { raw = null; }
      return protectedHidesServers(raw);
    };
    const fillMenu = async () => {
      if (hidesServers()) { drawMenu([STRIP_WORDS.hidden], null); return; }
      if (!linked) {
        drawMenu([STRIP_WORDS.loading], null);
        try {
          const res = await env.fetch('/api/federation/servers');
          if (!res || !res.ok) throw new Error('HTTP ' + (res && res.status));
          linked = await res.json();
        } catch (e) {
          drawMenu([STRIP_WORDS.failed], null);
          return;
        }
      }
      const links = switchLinks(linked, host, loc.pathname);
      drawMenu(links.length ? [STRIP_WORDS.signInNote] : [STRIP_WORDS.none, STRIP_WORDS.signInNote], links);
    };
    const openMenu = () => {
      menu.hidden = false;
      button.setAttribute('aria-expanded', 'true');
      return fillMenu();
    };
    const closeMenu = () => {
      menu.hidden = true;
      button.setAttribute('aria-expanded', 'false');
    };
    button.addEventListener('click', (e) => {
      if (e && e.stopPropagation) e.stopPropagation();
      if (menu.hidden) openMenu(); else closeMenu();
    });
    // A click anywhere else, or Escape, closes the list.
    if (doc.addEventListener) {
      doc.addEventListener('click', (e) => {
        if (menu.hidden) return;
        const t = e && e.target;
        if (t && wrap.contains && wrap.contains(t)) return;
        closeMenu();
      });
      doc.addEventListener('keydown', (e) => {
        if (!menu.hidden && e && e.key === 'Escape') { closeMenu(); if (button.focus) button.focus(); }
      });
    }

    const handle = { strip, ready, openMenu, closeMenu };
    strip.__hosStrip = handle;
    return handle;
  }

  const api = {
    STRIP_WORDS, STRIP_CSS, PROTECTED_STORAGE_KEY,
    stripLabel, linkedOrigin, samePagePath, switchLinks, protectedHidesServers, mountServerStrip,
  };

  if (typeof module === 'object' && module && module.exports) {
    module.exports = api;
    return;
  }
  root.hosServerStrip = api;

  // In the browser: mount once the page's body (and shell.js's nav, when the
  // page has one) is there.
  const boot = () => mountServerStrip({
    document: root.document,
    location: root.location,
    fetch: (url, init) => root.fetch(url, init),
    protectedRaw: () => root.localStorage.getItem(PROTECTED_STORAGE_KEY),
    // Chat's own rule (web/chat/chat-protected.js), when this page has it.
    get protectedHidesRooms() { return typeof root.protectedHidesRooms === 'function' ? root.protectedHidesRooms : undefined; },
  });
  const d = root.document;
  if (d.body && (d.readyState !== 'loading' || d.querySelector('.hub-nav-spacer'))) boot();
  else d.addEventListener('DOMContentLoaded', boot);
})(typeof globalThis !== 'undefined' ? globalThis : this);
