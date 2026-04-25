/**
 * HumanityOS Shared Shell — Nav + Footer
 * Place as the FIRST child of <body> on any page.
 * Set data-active="<key>" on the <script> tag to highlight the matching nav tab.
 * If omitted, active tab is auto-detected from the current URL.
 *
 * Valid active keys: landing, chat, tasks, map, market, settings, download, civilization
 *
 * Usage:
 *   <script src="/shared/shell.js" data-active="gear"></script>
 */
(function () {
  if (window.__HOS_SHELL_INIT__) return;
  window.__HOS_SHELL_INIT__ = true;

  // ── Global error boundary — prevent white-screen-of-death ──
  function showErrorBanner(msg) {
    if (document.getElementById('hos-error-banner')) return;
    var banner = document.createElement('div');
    banner.id = 'hos-error-banner';
    banner.style.cssText = 'position:fixed;bottom:0;left:0;right:0;z-index:999999;background:#991b1b;color:#fff;padding:10px 16px;font:14px/1.4 sans-serif;display:flex;align-items:center;justify-content:space-between;gap:12px;';
    var text = document.createElement('span');
    text.textContent = 'Something went wrong. Try refreshing.';
    var btn = document.createElement('button');
    btn.textContent = 'Dismiss';
    btn.style.cssText = 'background:#fff;color:#991b1b;border:none;padding:4px 12px;border-radius:4px;cursor:pointer;font:inherit;font-weight:600;';
    btn.onclick = function() { banner.remove(); };
    banner.appendChild(text);
    banner.appendChild(btn);
    document.body.appendChild(banner);
  }
  window.onerror = function(message, source, lineno, colno, error) {
    console.error('[HOS] Uncaught error:', message, 'at', source, lineno + ':' + colno, error);
    showErrorBanner();
    return true; // Prevent default browser error handling
  };
  window.onunhandledrejection = function(event) {
    console.error('[HOS] Unhandled promise rejection:', event.reason);
    showErrorBanner();
  };

  // ── Universal help modal ──
  // Any page can call window.hosHelp.register(id, title, content) and
  // window.hosHelp.show(id). Buttons with [data-help-id] open the modal
  // automatically. Styling uses theme CSS vars so it themes with the rest.
  var helpRegistry = {};
  function showHelp(id) {
    var entry = helpRegistry[id];
    if (!entry) return console.warn('[HOS] No help registered for: ' + id);
    if (!document.getElementById('hos-help-styles')) {
      var st = document.createElement('style');
      st.id = 'hos-help-styles';
      st.textContent =
        '.hos-help-backdrop{position:fixed;inset:0;z-index:10050;background:rgba(0,0,0,0.65);display:flex;align-items:center;justify-content:center;padding:20px;animation:hos-help-fade 0.15s ease-out;}' +
        '@keyframes hos-help-fade{from{opacity:0}to{opacity:1}}' +
        '.hos-help-modal{background:var(--bg-card,#161616);border:1px solid var(--border,#333);border-left:3px solid var(--accent,#FF8811);border-radius:var(--radius,8px);max-width:480px;width:100%;padding:28px;color:var(--text,#e0e0e0);box-shadow:0 12px 40px rgba(0,0,0,0.5);font-family:inherit;}' +
        '.hos-help-modal h3{margin:0 0 14px;font-size:1.15rem;font-weight:700;color:var(--text,#fff);}' +
        '.hos-help-body{font-size:0.95rem;line-height:1.65;color:var(--text-muted,#bbb);margin-bottom:22px;}' +
        '.hos-help-body p{margin:0 0 12px;}' +
        '.hos-help-body p:last-child{margin-bottom:0;}' +
        '.hos-help-body strong{color:var(--text,#fff);}' +
        '.hos-help-close{background:var(--accent,#FF8811);color:#000;border:none;padding:10px 22px;border-radius:var(--radius,6px);font-weight:600;cursor:pointer;font-family:inherit;font-size:0.92rem;transition:filter 0.15s;}' +
        '.hos-help-close:hover{filter:brightness(1.1);}' +
        '.hos-help-btn{display:inline-flex;align-items:center;justify-content:center;width:22px;height:22px;border-radius:50%;background:transparent;border:1px solid var(--border,#333);color:var(--text-muted,#888);font-size:0.72rem;font-weight:700;cursor:pointer;font-family:inherit;margin-left:8px;transition:all 0.15s;line-height:1;padding:0;flex-shrink:0;}' +
        '.hos-help-btn:hover{border-color:var(--accent,#FF8811);color:var(--accent,#FF8811);}' +
        '.context-wrap{display:inline-flex;align-items:center;}';
      document.head.appendChild(st);
    }
    var existing = document.getElementById('hos-help-modal-root');
    if (existing) existing.remove();
    var root = document.createElement('div');
    root.id = 'hos-help-modal-root';
    root.className = 'hos-help-backdrop';
    root.innerHTML =
      '<div class="hos-help-modal" role="dialog" aria-modal="true">' +
        '<h3>' + entry.title + '</h3>' +
        '<div class="hos-help-body">' + entry.content + '</div>' +
        '<button class="hos-help-close" type="button">Got it</button>' +
      '</div>';
    function closeHelp() {
      root.remove();
      document.removeEventListener('keydown', onHelpKey);
    }
    function onHelpKey(e) { if (e.key === 'Escape') closeHelp(); }
    root.addEventListener('click', function(e) { if (e.target === root) closeHelp(); });
    root.querySelector('.hos-help-close').addEventListener('click', closeHelp);
    document.addEventListener('keydown', onHelpKey);
    document.body.appendChild(root);
  }
  window.hosHelp = {
    register: function(id, title, content) { helpRegistry[id] = { title: title, content: content }; },
    show: showHelp
  };
  // Load help topics from data/help/topics.json (shared with native app).
  // This is the canonical source so both UIs show the same help content.
  (function loadHelpTopics() {
    fetch('/data/help/topics.json', { cache: 'no-cache' })
      .then(function (r) { return r.ok ? r.json() : Promise.reject('HTTP ' + r.status); })
      .then(function (data) {
        if (!data || !data.topics) return;
        Object.keys(data.topics).forEach(function (id) {
          var entry = data.topics[id];
          var html = (entry.body || [])
            .map(function (p) { return '<p>' + p + '</p>'; })
            .join('');
          window.hosHelp.register(id, entry.title || id, html);
        });
      })
      .catch(function (err) {
        console.warn('[HOS] Could not load help topics:', err);
        // Fallback: register a minimal real-sim topic so the ? icon still works.
        window.hosHelp.register('real-sim', 'Real mode vs. Sim mode',
          '<p>Real mode uses real-life data. Sim mode uses game-world data. Same tools, different context.</p>');
      });
  })();

  // ── Load shared icon system ──
  if (!window.hosIcon) {
    // Synchronous load so hosIcon() is available for nav tab rendering
    try {
      var xhr = new XMLHttpRequest();
      xhr.open('GET', '/shared/icons.js', false);
      xhr.send();
      if (xhr.status === 200) {
        var s = document.createElement('script');
        s.textContent = xhr.responseText;
        document.head.appendChild(s);
      }
    } catch(e) {
      console.warn('[HOS] Sync icon load failed:', e);
    }
    // If sync load didn't work, try async
    if (!window.hosIcon) {
      var iconsScript = document.createElement('script');
      iconsScript.src = '/shared/icons.js';
      document.head.appendChild(iconsScript);
    }
  }

  // If prior shell artifacts somehow exist, remove them before injecting once.
  // Also remove the old standalone #footer-toggle that existed before the toggle
  // was moved inside .site-footer.
  document.querySelectorAll('.hub-nav, .nav-separator, .site-footer, #webview-tabs-bar, #footer-toggle').forEach(function(el){
    if (el && el.parentNode) el.parentNode.removeChild(el);
  });

  // ── Detect active tab ──
  const scriptTag = document.currentScript;
  let active = scriptTag && scriptTag.getAttribute('data-active');
  if (!active) {
    const p = location.pathname;
    if (p === '/') active = 'landing';
    else if (p.startsWith('/chat'))      active = 'chat';
    else if (p.startsWith('/activities/game')) active = 'games';
    else if (p.startsWith('/dashboard')) active = 'dashboard';
    else if (p.startsWith('/profile'))   active = 'profile';
    else if (p.startsWith('/home'))      active = 'home';
    else if (p.startsWith('/inventory')) active = 'gear';
    else if (p.startsWith('/tasks'))     active = 'tasks';
    else if (p.startsWith('/calendar'))  active = 'calendar';
    else if (p.startsWith('/notes'))     active = 'notes';
    else if (p.startsWith('/systems'))   active = 'home';
    else if (p.startsWith('/maps'))      active = 'map';
    else if (p.startsWith('/market'))    active = 'market';
    else if (p.startsWith('/wallet'))    active = 'wallet';
    else if (p.startsWith('/web'))       active = 'web';
    else if (p.startsWith('/settings'))  active = 'settings';
    else if (p.startsWith('/ops') || p.startsWith('/pages/ops'))  active = 'ops';
    else if (p.startsWith('/download'))  active = 'download';
    else if (p.startsWith('/dev'))       active = 'dev';
    else if (p.startsWith('/roadmap'))   active = 'roadmap';
    else if (p.startsWith('/projects'))  active = 'projects';
    else if (p.startsWith('/activities/gardening')) active = 'garden';
    else if (p.startsWith('/donate'))    active = 'donate';
    else if (p.startsWith('/data'))     active = 'data';
    else if (p.startsWith('/crafting')) active = 'crafting';
    else if (p.startsWith('/civilization')) active = 'civilization';
    else if (p.startsWith('/resources')) active = 'resources';
    else if (p.startsWith('/bugs'))     active = 'bugs';
    else active = '';
  }

  function cls(tab) { return tab === active ? 'tab active' : 'tab'; }

  // ── GitHub SVG icon ──
  const ghIcon = '<svg viewBox="0 0 16 16"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>';

  // ── Download SVG icon ──
  const dlIcon = '<svg viewBox="0 0 16 16"><path d="M8 12L3 7h3V1h4v6h3L8 12zm-6 2h12v1.5H2V14z"/></svg>';

  // ── Inject CSS ──
  const style = document.createElement('style');
  style.textContent = `
    /* ── Hub Nav ── */
    .hub-nav {
      display: flex;
      align-items: center;
      background: rgba(13, 13, 13, 0.95);
      backdrop-filter: blur(12px);
      padding: 0 0.5rem;
      height: 40px;
      gap: 0.2rem;
      flex-shrink: 0;
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      z-index: 5500;
      isolation: isolate;
      font-size: 15px !important; /* fixed so global font-size slider doesn't break nav */
    }
    /* Spacer pushes page content below the fixed nav */
    .hub-nav-spacer {
      height: 41px; /* 40px nav + 1px separator */
      flex-shrink: 0;
      pointer-events: none;
    }

    /* ── Brand ── */
    .hub-nav .brand {
      font-size: 1.1rem;
      font-weight: 900;
      color: #FF8811;
      width: 32px;
      height: 28px;
      border-radius: var(--radius);
      box-shadow: inset 0 0 0 1px #2a6;
      text-decoration: none;
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
      margin-right: 0.3rem;
      cursor: pointer;
      transition: box-shadow 0.15s ease;
    }
    .hub-nav .brand:hover {
      box-shadow: inset 0 0 0 2px #48f, 0 0 8px rgba(68,136,255,0.3);
    }
    .hub-nav .brand.active {
      color: #fff;
      animation: channeling 3s linear infinite;
    }

    /* ── Tab (icon-only by default) ── */
    .hub-nav .tab {
      position: relative;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 30px;
      height: 28px;
      padding: 0;
      color: var(--text-muted);
      cursor: pointer;
      border-radius: var(--radius);
      user-select: none;
      text-decoration: none;
      flex-shrink: 0;
      transition: color 0.1s, box-shadow 0.1s;
      overflow: visible;
    }
    .hub-nav .tab .tab-icon {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }
    .hub-nav .tab .tab-icon img,
    .hub-nav .tab .tab-icon svg {
      width: var(--icon-size, 20px);
      height: var(--icon-size, 20px);
      max-width: 24px;
      max-height: 24px;
      object-fit: contain;
      display: block;
      opacity: 0.65;
      transition: opacity 0.1s, width 0.15s, height 0.15s;
    }

    /* Label hidden on inactive tabs — only visible when active */
    .hub-nav .tab .tab-label {
      display: none;
      font-size: 0.76rem;
      font-weight: 600;
      white-space: nowrap;
      margin-left: 0.3rem;
      color: #fff;
    }

    /* ── Hover: tooltip below, icon stays fully visible ── */
    .hub-nav .tab:not(.active):hover {
      box-shadow: inset 0 0 0 2px #48f, 0 0 8px rgba(68,136,255,0.3);
      color: var(--text);
    }
    .hub-nav .tab:not(.active):hover .tab-icon img,
    .hub-nav .tab:not(.active):hover .tab-icon svg { opacity: 1; }
    .hub-nav .tab:not(.active):hover::after {
      content: attr(data-tip);
      position: absolute;
      /* sits below the button with a gap so the icon is never obscured */
      top: calc(100% + 10px);
      left: 50%;
      transform: translateX(-50%);
      background: rgba(8,8,10,0.97);
      color: var(--text);
      border: 1px solid var(--border);
      border-radius: var(--radius);
      padding: 0.22rem 0.55rem;
      font-size: 0.7rem;
      font-weight: 600;
      white-space: nowrap;
      z-index: 9000;
      pointer-events: none;
      letter-spacing: 0.03em;
    }

    /* ── Active: expand to show icon + label, RGB border ── */
    .hub-nav .tab.active {
      width: auto;
      padding: 0 0.55rem 0 0.4rem;
      gap: 0;
      color: #fff;
      animation: channeling 3s linear infinite;
    }
    .hub-nav .tab.active .tab-label { display: inline; }
    .hub-nav .tab.active .tab-icon img,
    .hub-nav .tab.active .tab-icon svg { opacity: 1; }
    /* No ::after tooltip on active tab */
    .hub-nav .tab.active::after { display: none !important; }

    /* ── Update-ready: RGB border on download button when a new version exists ── */
    .hub-nav .tab.tab-update-ready {
      animation: channeling 3s linear infinite;
    }
    .hub-nav .tab.tab-update-ready .tab-icon img,
    .hub-nav .tab.tab-update-ready .tab-icon svg {
      opacity: 1;
    }
    /* Notification badge on download tab when update is available */
    .hub-nav .tab .tab-update-badge {
      position: absolute; top: 2px; right: 2px;
      width: 14px; height: 14px; border-radius: 50%;
      background: #e33; color: #fff; font-size: 9px;
      font-weight: 700; line-height: 14px; text-align: center;
      pointer-events: none; z-index: 2;
      animation: badgePulse 2s ease-in-out infinite;
    }
    @keyframes badgePulse {
      0%, 100% { transform: scale(1); }
      50% { transform: scale(1.2); }
    }

    [data-theme="light"] .hub-nav { background: rgba(244,244,244,0.95); border-bottom-color: #ccc; }
    [data-theme="light"] .hub-nav .tab { color: var(--text-muted); box-shadow: inset 0 0 0 1px #2a6; }
    [data-theme="light"] .hub-nav .tab.active { color: #1a1a1a; }
    [data-theme="light"] .hub-nav .nav-divider { background: #ccc; }

    /* ── Divider between nav groups — negative margin cancels the double flex-gap ── */
    .hub-nav .nav-divider {
      width: 1px;
      height: 18px;
      background: rgba(255,255,255,0.15);
      flex-shrink: 0;
      margin: 0 -0.1rem; /* gap is 0.2rem; -0.1rem each side keeps visual spacing even */
    }

    /* ── Spacer pushes right-side items to the right ── */
    /* min-width:0 ensures the spacer collapses before right-side tabs get pushed off-screen */
    .hub-nav .spacer { flex: 1; min-width: 0; }

    /* ── Mobile hamburger — hidden on desktop ── */
    .hub-nav .mobile-menu-btn {
      display: none;
      background: transparent;
      border: 1px solid #2a6;
      color: var(--text);
      padding: 0.24rem 0.55rem;
      border-radius: var(--radius);
      cursor: pointer;
      font-size: 0.82rem;
      line-height: 1;
      touch-action: manipulation;
      flex-shrink: 0;
    }
    .hub-nav .mobile-menu-btn:hover {
      box-shadow: inset 0 0 0 2px #48f, 0 0 8px rgba(68,136,255,0.3);
      color: #fff;
    }

    /* ── RGB animations ── */
    @keyframes channeling {
      0%   { box-shadow: inset 0 0 0 2px #f44, 0 0 10px rgba(255,68,68,0.4); }
      16%  { box-shadow: inset 0 0 0 2px #f80, 0 0 10px rgba(255,136,0,0.4); }
      33%  { box-shadow: inset 0 0 0 2px #ff0, 0 0 10px rgba(255,255,0,0.4); }
      50%  { box-shadow: inset 0 0 0 2px #0f4, 0 0 10px rgba(0,255,68,0.4); }
      66%  { box-shadow: inset 0 0 0 2px #08f, 0 0 10px rgba(0,136,255,0.4); }
      83%  { box-shadow: inset 0 0 0 2px #80f, 0 0 10px rgba(136,0,255,0.4); }
      100% { box-shadow: inset 0 0 0 2px #f44, 0 0 10px rgba(255,68,68,0.4); }
    }
    .nav-separator {
      height: 1px;
      width: 100%;
      animation: rgb-separator 3s linear infinite;
      flex-shrink: 0;
    }
    @keyframes rgb-separator {
      0%   { background: #ff0000; }
      16%  { background: #ff8800; }
      33%  { background: #ffff00; }
      50%  { background: #00ff44; }
      66%  { background: #0088ff; }
      83%  { background: #8800ff; }
      100% { background: #ff0000; }
    }

    /* ── Footer ── */
    .site-footer {
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      background: rgba(13, 13, 13, 0.95);
      backdrop-filter: blur(12px);
      border-top: 1px solid var(--border);
      z-index: 2100;
      text-align: center;
      font-size: 0.8rem;
      color: var(--text-muted);
      transition: transform 0.3s ease;
    }
    /* Footer slides off-screen when collapsed instead of hiding content */
    .site-footer { transition: transform 0.25s ease; }
    .site-footer.collapsed { transform: translateY(100%); }
    .site-footer .footer-content { padding: 10px 16px; }
    .site-footer .footer-content a { color: var(--accent); text-decoration: none; }
    .site-footer .footer-content a:hover { color: var(--accent-hover); }
    .site-footer .footer-links {
      display: flex;
      gap: 16px;
      justify-content: center;
      flex-wrap: wrap;
      margin-top: 6px;
    }
    .site-footer .footer-links a { color: var(--text-muted); text-decoration: none; font-size: 0.8rem; display: inline-flex; align-items: center; gap: 4px; }
    .site-footer .footer-links a:hover { color: var(--accent); }
    .site-footer .footer-links svg { width: 14px; height: 14px; fill: currentColor; vertical-align: middle; }
    /* Toggle is its own fixed element — always visible above all content */
    /* Toggle lives INSIDE .site-footer so it slides with the panel.
       bottom:100% makes its bottom edge flush with the footer's top edge — no gap. */
    @keyframes footer-toggle-rgb {
      0%   { border-color: #f44; box-shadow: 0 -2px 6px rgba(255,68,68,0.25); }
      16%  { border-color: #f80; box-shadow: 0 -2px 6px rgba(255,136,0,0.25); }
      33%  { border-color: #ff0; box-shadow: 0 -2px 6px rgba(255,255,0,0.25); }
      50%  { border-color: #0f4; box-shadow: 0 -2px 6px rgba(0,255,68,0.25); }
      66%  { border-color: #08f; box-shadow: 0 -2px 6px rgba(0,136,255,0.25); }
      83%  { border-color: #80f; box-shadow: 0 -2px 6px rgba(136,0,255,0.25); }
      100% { border-color: #f44; box-shadow: 0 -2px 6px rgba(255,68,68,0.25); }
    }
    .footer-toggle {
      position: absolute;
      bottom: 100%; /* flush: toggle bottom = footer top, zero gap */
      top: auto;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(18, 18, 18, 0.97);
      border: 1px solid #f44; /* start of RGB cycle */
      border-bottom: none;
      border-radius: var(--radius) var(--radius) 0 0;
      color: var(--accent);
      cursor: pointer;
      padding: 5px 28px;
      font-size: 0.78rem;
      font-weight: 700;
      line-height: 1;
      z-index: 1;
      animation: footer-toggle-rgb 10s linear infinite; /* slow pulse when collapsed */
      transition: color 0.15s, background 0.15s;
      white-space: nowrap;
      letter-spacing: 0.05em;
    }
    /* Speed up RGB when footer is open */
    .site-footer:not(.collapsed) .footer-toggle {
      animation-duration: 3s;
    }
    .footer-toggle:hover { color: #fff; background: rgba(30,20,10,0.98); }

    /* ── Mobile drawer ── */
    #mobile-hub-backdrop {
      position: fixed;
      inset: 0;
      background: rgba(0,0,0,0.45);
      z-index: 7600;
      display: none;
    }
    #mobile-hub-drawer {
      position: fixed;
      top: 0;
      left: 0;
      width: 100vw;
      height: 100vh;
      background: rgba(13,13,13,0.92);
      z-index: 7700;
      transform: translateX(100%);
      transition: transform 0.2s ease;
      overflow-y: auto;
      padding: 0.65rem 0.6rem 1rem;
      box-sizing: border-box;
      backdrop-filter: blur(2px);
    }
    #mobile-hub-drawer.open { transform: translateX(0); }
    #mobile-hub-backdrop.open { display: block; }
    .mobile-hub-group { margin-bottom: 0.65rem; border:1px solid var(--border); border-radius:var(--radius); }
    .mobile-hub-group h4 { margin:0; padding:0.45rem 0.55rem; font-size:0.72rem; color:var(--text-muted); border-bottom:1px solid var(--border); text-transform:uppercase; letter-spacing:.08em; }
    .mobile-hub-group a { display:block; color:var(--text); text-decoration:none; padding:0.5rem 0.55rem; font-size:0.86rem; border-bottom:1px solid var(--bg-secondary); }
    .mobile-hub-group a:last-child { border-bottom:none; }
    .mobile-hub-group a:hover { background: rgba(255,255,255,0.05); }
    .mobile-hub-group a.active {
      color: #fff;
      background: rgba(255,255,255,0.06);
      animation: channeling 3s linear infinite;
      border-radius: var(--radius);
      margin: 0.15rem;
    }

    /* ── Nav group wrappers ── */
    .hub-nav .nav-group-red,
    .hub-nav .nav-group-green,
    .hub-nav .nav-group-blue {
      display: inline-flex;
      align-items: center;
      gap: 0.2rem;
    }

    /* ── Responsive: hide flat tabs on mobile, show hamburger ── */
    @media (max-width: 768px) {
      .hub-nav { padding: 0 0.4rem; gap: 0.15rem; height: 36px; }
      .hub-nav .tab { display: none !important; }
      .hub-nav .nav-divider { display: none !important; }
      .hub-nav .nav-group-red,
      .hub-nav .nav-group-green,
      .hub-nav .nav-group-blue { display: none !important; }
      .hub-nav .spacer { display: none !important; }
      .hub-nav .context-toggle { display: none !important; }
      .hub-nav .brand { margin-right: 0.25rem; }
      .hub-nav .mobile-menu-btn { display: inline-flex; align-items:center; justify-content:center; margin-left:auto; }
      .hub-nav-spacer { height: 37px; }
    }
  `;
  document.head.appendChild(style);

  // Helper: build a nav tab anchor.
  // icon is a hosIcon name (e.g. 'network') rendered as inline SVG.
  function navTab(href, icon, label, activeKey) {
    var isActive = active === activeKey;
    var cls = 'tab' + (isActive ? ' active' : '');
    var iconHtml = '<span class="tab-icon">' + (window.hosIcon ? hosIcon(icon, 15) : '') + '</span>';
    return '<a href="' + href + '" class="' + cls + '" data-tip="' + label + '">' +
      iconHtml +
      '<span class="tab-label">' + label + '</span>' +
    '</a>';
  }

  // ── Context toggle (Real / Game) ──
  var savedContext = localStorage.getItem('humanity_context') || 'real';
  // Backward compat: treat legacy "game" as "sim"
  if (savedContext === 'game') { savedContext = 'sim'; localStorage.setItem('humanity_context', 'sim'); }
  if (savedContext !== 'real' && savedContext !== 'sim') savedContext = 'real';
  Object.defineProperty(window, 'hos_context', {
    get: function() {
      var v = localStorage.getItem('humanity_context') || 'real';
      if (v === 'game') { v = 'sim'; localStorage.setItem('humanity_context', 'sim'); }
      return v;
    },
    configurable: true
  });

  function buildContextToggle() {
    var ctx = window.hos_context;
    return '<div class="context-wrap">' +
      '<div class="context-toggle ctx-' + ctx + '" id="hos-context-toggle">' +
        '<span class="ctx-seg' + (ctx === 'real' ? ' active' : '') + '" data-ctx="real">Real</span>' +
        '<span class="ctx-seg' + (ctx === 'sim' ? ' active' : '') + '" data-ctx="sim">Sim</span>' +
      '</div>' +
      '<button class="hos-help-btn" type="button" aria-label="What does Real/Sim mean?" data-help-id="real-sim" title="What does this do?">?</button>' +
    '</div>';
  }

  // ── Inject Nav ──
  const nav = document.createElement('div');
  nav.innerHTML =
    '<nav class="hub-nav">' +
      /* Brand */
      '<a href="/" class="brand' + (active === 'landing' ? ' active' : '') + '" data-tip="Home">H</a>' +

      '<div class="nav-divider"></div>' +

      /* Red group: core identity (never changes with context) */
      '<span class="nav-group-red">' +
        navTab('/chat',     'network',  'Chat',     'chat') +
        navTab('/wallet',   'coin',     'Wallet',   'wallet') +
        navTab('/donate',   'heart',    'Donate',   'donate') +
      '</span>' +

      '<div class="nav-divider"></div>' +

      /* Green group: context-sensitive (data changes with Real/Game) */
      '<span class="nav-group-green">' +
        navTab('/profile',   'profile',    'Profile',   'profile') +
        navTab('/civilization', 'globe',   'Civilization', 'civilization') +
        navTab('/tasks',     'tasklist',   'Tasks',     'tasks') +
        navTab('/inventory', 'inventory',  'Inventory', 'gear') +
        navTab('/maps',      'map',        'Maps',      'map') +
        navTab('/market',    'market',     'Market',    'market') +
      '</span>' +

      '<div class="nav-divider"></div>' +

      /* Blue group: system/config */
      '<span class="nav-group-blue">' +
        navTab('/projects', 'folder',    'Projects',  'projects') +
        navTab('/settings', 'settings',  'Settings',  'settings') +
        navTab('/download', 'download', 'Download', 'download') +
        navTab('/ops',      'ops',       'Ops',       'ops') +
        navTab('/bugs',     'bug',       'Bugs',      'bugs') +
        navTab('/dev',      'dev',       'Dev',       'dev') +
      '</span>' +

      /* Spacer pushes context toggle to the right */
      '<div class="spacer"></div>' +

      /* Context toggle — right-aligned */
      buildContextToggle() +

      /* Mobile hamburger — only visible on small screens */
      '<button class="mobile-menu-btn" id="mobile-hub-menu-btn" type="button" aria-label="Open menu">' + (window.hosIcon ? hosIcon('menu', 18) : '☰') + '</button>' +
    '</nav>' +
    '<div id="webview-tabs-bar" style="display:none;height:32px;background:rgba(13,13,13,0.95);border-bottom:1px solid var(--border);align-items:center;padding:0 var(--space-xl);gap:var(--space-sm);overflow-x:auto;"></div>' +
    '<div class="nav-separator"></div>';
  document.body.prepend(nav);
  // Spacer so fixed nav doesn't overlap page content
  var navSpacer = document.createElement('div');
  navSpacer.className = 'hub-nav-spacer';
  nav.insertAdjacentElement('afterend', navSpacer);

  // Mobile drawer fallback menu (for reliable touch nav)
  var mobileBackdrop = document.createElement('div');
  mobileBackdrop.id = 'mobile-hub-backdrop';
  var mobileDrawer = document.createElement('aside');
  mobileDrawer.id = 'mobile-hub-drawer';
  function mobileLink(path, label) {
    var current = location.pathname;
    var isActive = current === path || (path !== '/' && current.startsWith(path + '/'));
    return '<a href="' + path + '"' + (isActive ? ' class="active"' : '') + '>' + label + '</a>';
  }

  mobileDrawer.innerHTML =
    '<div class="mobile-hub-group group-red"><h4>Identity</h4>' +
      mobileLink('/chat',      'Chat') +
      mobileLink('/profile',   'Profile') +
      mobileLink('/wallet',    'Wallet') +
      mobileLink('/donate',    'Donate') +
    '</div>' +
    '<div class="mobile-hub-group group-green"><h4>Activities</h4>' +
      mobileLink('/civilization', 'Civilization') +
      mobileLink('/tasks',     'Tasks') +
      mobileLink('/inventory', 'Inventory') +
      mobileLink('/maps',      'Maps') +
      mobileLink('/market',    'Market') +
    '</div>' +
    '<div class="mobile-hub-group group-blue"><h4>System</h4>' +
      mobileLink('/settings',             'Settings') +
      mobileLink('/bugs',                  'Bug Reports') +
      mobileLink('/download',   'Download') +
    '</div>' +
    '<div class="mobile-hub-group"><h4>Context</h4>' +
      '<div style="padding:0.5rem 0.55rem;">' + buildContextToggle() + '</div>' +
    '</div>';
  document.body.appendChild(mobileBackdrop);
  document.body.appendChild(mobileDrawer);

  var mobileMenuBtn = document.getElementById('mobile-hub-menu-btn');
  function closeMobileDrawer() {
    mobileBackdrop.classList.remove('open');
    mobileDrawer.classList.remove('open');
  }
  function openMobileDrawer() {
    mobileBackdrop.classList.add('open');
    mobileDrawer.classList.add('open');
  }
  if (mobileMenuBtn) {
    mobileMenuBtn.addEventListener('click', function(e) {
      e.preventDefault();
      e.stopPropagation();
      if (mobileDrawer.classList.contains('open')) closeMobileDrawer();
      else openMobileDrawer();
    });
    mobileMenuBtn.addEventListener('touchend', function(e) {
      e.preventDefault();
      e.stopPropagation();
      if (mobileDrawer.classList.contains('open')) closeMobileDrawer();
      else openMobileDrawer();
    }, { passive: false });
  }
  mobileBackdrop.addEventListener('click', closeMobileDrawer);
  mobileDrawer.addEventListener('click', function(e) {
    const link = e.target.closest('a[href]');
    if (link) closeMobileDrawer();
  });

  // ── Help button click handler ──
  document.addEventListener('click', function(e) {
    var helpBtn = e.target.closest('.hos-help-btn[data-help-id]');
    if (!helpBtn) return;
    e.preventDefault();
    e.stopPropagation();
    window.hosHelp.show(helpBtn.dataset.helpId);
  });

  // ── Context toggle handler ──
  document.addEventListener('click', function(e) {
    // Click anywhere on the toggle to switch (not just the text)
    var toggle = e.target.closest('.context-toggle');
    if (!toggle) return;
    // Clicking anywhere on the pill toggles to the other context
    var newCtx = window.hos_context === 'real' ? 'sim' : 'real';
    if (!newCtx) return;
    localStorage.setItem('humanity_context', newCtx);
    // Update all toggle instances on the page
    document.querySelectorAll('.context-toggle .ctx-seg').forEach(function(el) {
      el.classList.toggle('active', el.getAttribute('data-ctx') === newCtx);
    });
    // Update color coding on toggle containers
    document.querySelectorAll('.context-toggle').forEach(function(el) {
      el.classList.toggle('ctx-real', newCtx === 'real');
      el.classList.toggle('ctx-sim', newCtx === 'sim');
    });
    // Dispatch event so pages can react
    window.dispatchEvent(new CustomEvent('hos-context-change', { detail: { context: newCtx } }));
  });

  // Rich tooltips (label + short explanation)
  function defaultTooltipDescription(label) {
    var l = (label || '').toLowerCase();
    if (l.includes('mute')) return 'Silences your microphone so others cannot hear you.';
    if (l.includes('disconnect') || l.includes('leave')) return 'Immediately exits the current voice/chat session.';
    if (l.includes('volume')) return 'Adjusts how loud incoming audio is for you.';
    if (l.includes('camera')) return 'Turns your camera stream on or off for others.';
    if (l.includes('screen')) return 'Shares your screen so others can watch your display.';
    if (l.includes('search')) return 'Finds messages or content in the current context.';
    if (l.includes('users') || l.includes('people')) return 'Opens the people panel with presence and stream controls.';
    if (l.includes('send')) return 'Sends your current message to the active channel.';
    if (l.includes('attach') || l.includes('file')) return 'Adds a file to your message or upload flow.';
    if (l.includes('commands')) return 'Opens command tools and quick actions.';
    if (l.includes('help')) return 'Shows guidance, docs, and available actions.';
    if (l.includes('chat') || l.includes('network')) return 'Chat, voice, and collaboration hub.';
    if (l.includes('games')) return 'Launch the 3D game and explore the universe.';
    if (l.includes('garden')) return 'Tend your garden: plant, water, and harvest crops.';
    if (l.includes('donate')) return 'Support the project with donations and funding.';
    if (l.includes('inventory')) return 'Your items, equipment, and resource storage.';
    if (l.includes('crafting')) return 'Combine materials to create tools and equipment.';
    if (l.includes('profile')) return 'Your identity, skills, and social links.';
    if (l.includes('gear')) return 'Your inventory and equipment loadouts.';
    if (l.includes('tasks')) return 'Kanban board, quests, and project planning.';
    if (l.includes('journal')) return 'Encrypted notes and timestamped log entries.';
    if (l.includes('web')) return 'Browse curated websites and bookmarks.';
    if (l.includes('systems')) return 'Game systems and infrastructure overview.';
    if (l.includes('civilization')) return 'The big picture: population, infrastructure, resources, and governance.';
    if (l.includes('maps')) return 'Interactive maps from local to galactic scale.';
    if (l.includes('market')) return 'Buy, sell, and trade with other users.';
    if (l.includes('wallet')) return 'Solana wallet: send, receive, swap, and stake crypto.';
    if (l.includes('calendar')) return 'Events, schedules, and recurring plans.';
    if (l.includes('home')) return 'Manage your homes, rooms, and property.';
    return 'Tap or click to use this control.';
  }

  function initRichTooltips() {
    if (window.__HOS_RICH_TOOLTIPS__) return;
    window.__HOS_RICH_TOOLTIPS__ = true;

    var tip = document.createElement('div');
    tip.id = 'hos-rich-tooltip';
    tip.style.cssText = 'position:fixed;z-index:9000;pointer-events:none;max-width:300px;background:rgba(8,8,10,0.97);border:1px solid rgba(130,130,140,0.35);border-radius:var(--radius);padding:8px 11px;color:var(--text);font-size:12px;line-height:1.4;box-shadow:0 8px 24px rgba(0,0,0,0.55);display:none;';
    document.body.appendChild(tip);

    /** Strip native title to prevent browser double-tooltip. */
    function stripTitle(el) {
      var t = el.getAttribute && el.getAttribute('title');
      if (t && !el.getAttribute('data-native-title')) {
        el.setAttribute('data-native-title', t);
        el.removeAttribute('title');
      }
    }

    function esc(s) { return String(s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;'); }

    function showFor(el, x, y) {
      if (!el) return;
      var name = el.getAttribute('data-tip-title') || el.getAttribute('data-native-title') ||
                 el.getAttribute('aria-label') || el.getAttribute('data-tip') ||
                 (el.textContent || '').trim().slice(0, 48);
      if (!name) return;
      var hotkey = el.getAttribute('data-tip-hotkey') || '';
      var desc   = el.getAttribute('data-tip-desc')   || defaultTooltipDescription(name);
      var detail = el.getAttribute('data-tip-detail')  || '';

      var html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:' + (desc ? '4' : '0') + 'px;">' +
        '<span style="font-weight:600;color:#fff;font-size:12.5px;">' + esc(name) + '</span>';
      if (hotkey) {
        html += '<kbd style="font-size:10px;background:#0e2010;border:1px solid #2a4a2a;border-radius:var(--radius-sm);padding:1px 6px;color:#5d5;white-space:nowrap;flex-shrink:0;">' + esc(hotkey) + '</kbd>';
      }
      html += '</div>';
      if (desc) {
        html += '<div style="color:#b9c2d0;margin-bottom:' + (detail ? '4' : '0') + 'px;">' + esc(desc) + '</div>';
      }
      if (detail) {
        html += '<div style="color:#687888;font-size:10.5px;border-top:1px solid rgba(100,110,120,0.2);padding-top:3px;margin-top:1px;">' + esc(detail) + '</div>';
      }

      tip.innerHTML = html;
      tip.style.display = 'block';
      // Keep tooltip inside viewport, prefer below-right of cursor
      var tw = 320, th = tip.offsetHeight || 80;
      var tx = x + 14;
      var ty = y + 14;
      if (tx + tw > window.innerWidth  - 8) tx = x - tw - 8;
      if (ty + th > window.innerHeight - 8) ty = y - th - 8;
      tip.style.left = Math.max(8, tx) + 'px';
      tip.style.top  = Math.max(8, ty) + 'px';
    }

    function hideTip() { tip.style.display = 'none'; }

    // Strip all existing title attributes at init time
    document.querySelectorAll('[title]').forEach(stripTitle);

    // Watch for dynamically-added elements (dynamic UI, voice bar shown/hidden, modals)
    // so they never show a native browser tooltip alongside the rich one.
    var obs = new MutationObserver(function(mutations) {
      mutations.forEach(function(m) {
        if (m.type === 'attributes') { stripTitle(m.target); return; }
        m.addedNodes.forEach(function(node) {
          if (node.nodeType !== 1) return;
          stripTitle(node);
          node.querySelectorAll('[title]').forEach(stripTitle);
        });
      });
    });
    obs.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['title'] });

    var SELECTOR = '[data-native-title],[data-tip-title],[data-tip],[aria-label],button,a,[role="button"]';
    document.addEventListener('mouseover', function(e) {
      var el = e.target.closest(SELECTOR);
      if (!el) return;
      if (el.closest('.hub-nav')) return; // nav tabs use CSS ::after tooltips
      showFor(el, e.clientX || 8, e.clientY || 8);
    });
    document.addEventListener('mousemove', function(e) {
      if (tip.style.display !== 'block') return;
      var tw = 320, th = tip.offsetHeight || 80;
      var tx = (e.clientX || 8) + 14, ty = (e.clientY || 8) + 14;
      if (tx + tw > window.innerWidth  - 8) tx = (e.clientX || 8) - tw - 8;
      if (ty + th > window.innerHeight - 8) ty = (e.clientY || 8) - th - 8;
      tip.style.left = Math.max(8, tx) + 'px';
      tip.style.top  = Math.max(8, ty) + 'px';
    });
    document.addEventListener('mouseout', function(e) {
      if (e.target && e.target.closest && e.target.closest(SELECTOR)) hideTip();
    });
    document.addEventListener('focusin', function(e) {
      var el = e.target.closest(SELECTOR);
      if (!el) return;
      var r = el.getBoundingClientRect();
      showFor(el, r.left + 8, r.bottom + 8);
    });
    document.addEventListener('focusout', hideTip);
    document.addEventListener('scroll', hideTip, true);
  }

  setTimeout(initRichTooltips, 0);

  // ── Fix external links for Tauri desktop app ──
  // target="_blank" doesn't work in Tauri's single webview; window.open() opens system browser
  document.body.addEventListener('click', function(e) {
    var link = e.target.closest('a[href]');
    if (!link) return;
    var href = link.getAttribute('href');
    if (href && (href.startsWith('http://') || href.startsWith('https://')) && link.target === '_blank') {
      e.preventDefault();
      window.open(href, '_blank');
    }
  });

  function isInActiveVoiceRoom() {
    return !!(window._currentRoomId && window._roomLocalStream);
  }

  // ── Voice Room Navigation Guard ──
  // Navigating away from /chat while in a voice channel would destroy WebSocket
  // and WebRTC connections. Warn the user or open in a webview tab instead.
  document.querySelector('.hub-nav').addEventListener('click', function(e) {
    if (e.target.closest('.mobile-menu-btn')) return;

    const link = e.target.closest('a[href]');
    if (!link) return;
    const href = link.getAttribute('href');

    if (location.pathname === '/chat' && isInActiveVoiceRoom() && href !== '/chat') {
      e.preventDefault();
      var title = (link.getAttribute('data-tip-title') || link.textContent || href).trim();
      if (typeof openWebviewTab === 'function') {
        openWebviewTab(href, title);
      } else {
        if (confirm('You are in a voice channel. Leaving this page will disconnect you. Continue?')) location.href = href;
      }
    }
    // All other nav clicks: let the browser do a normal full-page navigation.
    // Every page is a standalone HTML file — no SPA routing needed.
  });

  // ── Inject Footer ──
  // The toggle button lives INSIDE the footer so it slides with the panel
  // (position:absolute, top:-28px). This fixes the gap between the tab and
  // the panel that appeared when the footer was open in the old design.
  var footerEl = document.createElement('footer');
  footerEl.className = 'site-footer';
  footerEl.id = 'site-footer';
  // footer needs overflow:visible so the absolute toggle can peek above it
  footerEl.style.overflow = 'visible';
  footerEl.innerHTML =
    '<button class="footer-toggle" id="footer-toggle" aria-label="Toggle footer">▲</button>' +
    '<div class="footer-content" id="footer-content">' +
      '<span id="hos-footer-label">HumanityOS — Public domain · <a href="https://creativecommons.org/publicdomain/zero/1.0/" target="_blank">CC0 1.0</a></span>' +
      '<div class="footer-links">' +
        '<a href="https://github.com/Shaostoul/Humanity" target="_blank">' + ghIcon + ' GitHub</a>' +
        '<a href="#" id="hos-take-tour" style="margin-left:var(--space-lg, 12px);font-size:0.72rem;">Take Tour</a>' +
      '</div>' +
    '</div>';
  document.body.appendChild(footerEl);

  // ── Webview Tab System ──
  // Desktop app: uses native Tauri webview panels (real browser, no iframe restrictions)
  // Web browser: opens external links in new tabs (iframes blocked by most sites)
  var webviewTabs = {};
  var webviewCounter = 0;
  var activeWebviewTab = null;
  var NAV_HEIGHT = 42; // px — height of the hub-nav bar

  /** Check if Tauri native webview API is available. */
  function hasTauriWebview() {
    return !!(window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);
  }

  window.openWebviewTab = function(url, title) {
    // Check if already open with this URL
    for (var id in webviewTabs) {
      if (webviewTabs[id].url === url) { switchWebviewTab(id); return; }
    }

    // Web browser fallback: open in new tab
    if (!hasTauriWebview()) {
      window.open(url, '_blank');
      return;
    }

    var tabId = 'browser-' + (++webviewCounter);
    webviewTabs[tabId] = { url: url, title: title || new URL(url).hostname || url };

    // Create the native webview via Tauri command
    var w = window.innerWidth;
    var h = window.innerHeight - NAV_HEIGHT;

    window.__TAURI__.core.invoke('open_browser_webview', {
      label: tabId,
      url: url,
      x: 0,
      y: NAV_HEIGHT,
      width: w,
      height: h
    }).then(function() {
      activeWebviewTab = tabId;
      // Hide the main page content so the webview is visible
      var pageApp = document.getElementById('page-app') || document.getElementById('chat-screen');
      if (pageApp) pageApp.style.display = 'none';
      renderWebviewTabBar();
    }).catch(function(err) {
      console.error('Failed to open browser webview:', err);
      delete webviewTabs[tabId];
      // Fallback: open in system browser
      window.__TAURI__.core.invoke('open_external_url', { url: url }).catch(function() {});
    });
  };

  function switchWebviewTab(tabId) {
    if (!hasTauriWebview()) return;
    // For native webviews, we just track which is active — Tauri handles visibility
    // TODO: when multi-tab is fully supported, hide/show webviews here
    activeWebviewTab = tabId;
    renderWebviewTabBar();
  }

  window.closeWebviewTab = function(tabId) {
    if (hasTauriWebview()) {
      window.__TAURI__.core.invoke('close_browser_webview', { label: tabId }).catch(function(err) {
        console.warn('close_browser_webview failed:', err);
      });
    }
    delete webviewTabs[tabId];
    if (activeWebviewTab === tabId) {
      var keys = Object.keys(webviewTabs);
      activeWebviewTab = keys.length > 0 ? keys[keys.length - 1] : null;
    }
    // If no more tabs, show the main content again
    if (Object.keys(webviewTabs).length === 0) {
      var pageApp = document.getElementById('page-app') || document.getElementById('chat-screen');
      if (pageApp) pageApp.style.display = '';
    }
    renderWebviewTabBar();
  };

  window.webviewNavigate = function(tabId, url) {
    if (!hasTauriWebview()) return;
    webviewTabs[tabId].url = url;
    window.__TAURI__.core.invoke('navigate_browser_webview', { label: tabId, url: url }).catch(function(err) {
      console.error('navigate failed:', err);
    });
    renderWebviewTabBar();
  };

  // Resize browser webviews when window resizes
  window.addEventListener('resize', function() {
    if (!hasTauriWebview()) return;
    var w = window.innerWidth;
    var h = window.innerHeight - NAV_HEIGHT;
    for (var id in webviewTabs) {
      window.__TAURI__.core.invoke('resize_browser_webview', {
        label: id, x: 0, y: NAV_HEIGHT, width: w, height: h
      }).catch(function() {});
    }
  });

  function renderWebviewTabBar() {
    var bar = document.getElementById('webview-tabs-bar');
    if (!bar) return;
    var keys = Object.keys(webviewTabs);
    if (keys.length === 0) { bar.style.display = 'none'; return; }
    bar.style.display = 'flex';
    bar.innerHTML = '';
    keys.forEach(function(id) {
      var tab = webviewTabs[id];
      var btn = document.createElement('button');
      btn.style.cssText = 'display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) var(--space-lg);border-radius:var(--radius-sm);border:1px solid ' + (id===activeWebviewTab?'var(--accent)':'var(--border)') + ';background:' + (id===activeWebviewTab?'var(--accent-dim)':'transparent') + ';color:' + (id===activeWebviewTab?'var(--accent)':'var(--text-muted)') + ';font-size:0.72rem;cursor:pointer;white-space:nowrap;font-family:inherit;';
      var titleText = (tab.title||'Tab');
      if (titleText.length > 24) titleText = titleText.substring(0, 24) + '…';
      btn.innerHTML = '<span>' + titleText + '</span><span onclick="event.stopPropagation();closeWebviewTab(\'' + id + '\')" style="margin-left:var(--space-sm);color:var(--danger,#e55);font-weight:700;cursor:pointer;">✕</span>';
      btn.onclick = function() { switchWebviewTab(id); };
      bar.appendChild(btn);
    });
  }

  // ── Footer toggle logic ──
  setTimeout(function () {
    var ft  = footerEl;
    var btn = document.getElementById('footer-toggle'); // toggle is inside footerEl
    if (!ft || !btn) return;

    function setCollapsed(collapsed) {
      if (collapsed) {
        ft.classList.add('collapsed');
        btn.textContent = '▲'; // arrow up = "expand footer"
      } else {
        ft.classList.remove('collapsed');
        btn.textContent = '▼'; // arrow down = "collapse footer"
      }
      localStorage.setItem('footer_collapsed', String(collapsed));
    }

    // Default: start collapsed so it doesn't cover content; user can expand
    var saved = localStorage.getItem('footer_collapsed');
    setCollapsed(saved === null ? true : saved === 'true');

    btn.addEventListener('click', function () {
      setCollapsed(!ft.classList.contains('collapsed'));
    });
  }, 0);

  // ── PWA Service Worker registration (skip in Tauri — local files, no caching) ──
  if ('serviceWorker' in navigator && !window.__TAURI__) {
    window.addEventListener('load', function () {
      navigator.serviceWorker.register('/shared/sw.js').then(function (reg) {
        console.log('[SW] Registered, scope:', reg.scope);
      }).catch(function (e) {
        console.warn('[SW] Registration failed:', e);
      });
    });
  }

  // ── Keyboard shortcut panel (? key) ──
  (function () {
    var overlay = document.createElement('div');
    overlay.id = 'hos-shortcut-overlay';
    overlay.style.cssText = [
      'display:none;position:fixed;inset:0;background:rgba(0,0,0,.75);z-index:9000',
      'align-items:center;justify-content:center;font-family:\'Segoe UI\',system-ui,sans-serif'
    ].join(';');

    var shortcuts = [
      ['Navigation', [
        ['?', 'Open / close this shortcut panel'],
        ['Esc', 'Close modals and panels'],
        ['Alt + ←', 'Browser back'],
        ['Alt + →', 'Browser forward'],
      ]],
      ['Chat (/chat)', [
        ['Enter', 'Send message'],
        ['Shift + Enter', 'New line in message'],
        ['↑ (empty input)', 'Edit your last message'],
        ['Ctrl + K', 'Open command palette'],
        ['/ (or !)  ', 'Slash commands'],
        ['Tab', 'Autocomplete mention / command'],
      ]],
      ['Tasks (/tasks)', [
        ['Click card', 'Open detail panel'],
        ['Esc', 'Close detail panel'],
        ['← →', 'Scroll scope tabs'],
      ]],
      ['Notes (/notes)', [
        ['Ctrl + S', 'Force save (auto-saves on pause)'],
      ]],
      ['Global', [
        ['🌙 / ☀️ nav button', 'Toggle light / dark theme'],
      ]],
    ];

    var html = '<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-lg);' +
      'padding:var(--space-2xl) var(--space-3xl);width:100%;max-width:640px;max-height:85vh;overflow-y:auto;color:var(--text)">' +
      '<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-2xl)">' +
      '<h2 style="font-size:1rem;font-weight:700;color:var(--accent)">Keyboard Shortcuts</h2>' +
      '<button onclick="document.getElementById(\'hos-shortcut-overlay\').style.display=\'none\'" ' +
      'style="background:none;border:none;color:var(--text-muted);font-size:1.1rem;cursor:pointer">✕</button>' +
      '</div>';

    shortcuts.forEach(function (group) {
      html += '<div style="margin-bottom:var(--space-2xl)">';
      html += '<div style="font-size:.65rem;font-weight:700;letter-spacing:.1em;color:var(--text-muted);' +
        'text-transform:uppercase;margin-bottom:var(--space-lg)">' + group[0] + '</div>';
      html += '<table style="width:100%;border-collapse:collapse">';
      group[1].forEach(function (row) {
        html += '<tr style="border-bottom:1px solid var(--bg-input)">' +
          '<td style="padding:var(--space-sm) var(--space-xl);font-family:monospace;font-size:.78rem;color:var(--accent);white-space:nowrap">' + row[0] + '</td>' +
          '<td style="padding:var(--space-sm) var(--space-xl);font-size:.78rem;color:var(--text-muted)">' + row[1] + '</td></tr>';
      });
      html += '</table></div>';
    });
    html += '</div>';
    overlay.innerHTML = html;
    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) overlay.style.display = 'none';
    });
    document.body.appendChild(overlay);

    document.addEventListener('keydown', function (e) {
      // ? key — but NOT when focus is in an input/textarea
      if (e.key === '?' && !['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement.tagName)) {
        e.preventDefault();
        var vis = overlay.style.display;
        overlay.style.display = (vis === 'none' || vis === '') ? 'flex' : 'none';
      }
      if (e.key === 'Escape') overlay.style.display = 'none';
    });
  })();

  // ── Debug Overlay ──
  // Shows when Settings > Advanced > "Show Debug Panel" is enabled.
  // Displays page path, WebSocket state, identity key prefix, and SW cache version.
  (function () {
    function debugEnabled() {
      try { var s = JSON.parse(localStorage.getItem('hos_settings_v1')); return !!(s && s['debug-panel']); }
      catch (_) { return false; }
    }
    if (!debugEnabled()) return;

    var dbg = document.createElement('div');
    dbg.id = 'hos-debug-overlay';
    dbg.style.cssText = 'position:fixed;bottom:48px;right:10px;z-index:8800;background:rgba(0,4,0,0.93);border:1px solid #1a4a1a;border-radius:var(--radius);padding:6px 10px;font-size:11px;font-family:monospace;color:#3d3;line-height:1.6;pointer-events:none;width:210px;box-shadow:0 4px 14px rgba(0,0,0,0.5);white-space:nowrap;overflow:hidden;';
    document.body.appendChild(dbg);

    var WS_LABELS = ['CONNECTING','OPEN','CLOSING','CLOSED'];
    function wsLabel(state) { return WS_LABELS[state] || '—'; }
    function wsColor(state) { return state === 1 ? '#3d3' : state === 0 ? '#fa0' : '#f55'; }
    function keyPrefix() {
      try { var id = JSON.parse(localStorage.getItem('hos_identity')); if (id && id.publicKeyHex) return id.publicKeyHex.slice(0,10) + '…'; } catch (_) {}
      return '—';
    }

    function update() {
      var ws = window.App && window.App.ws;
      var state = ws ? ws.readyState : null;
      dbg.innerHTML =
        '<b style="color:#6f6">🐛 Debug</b><br>' +
        '<span style="color:var(--text-muted)">Page: </span>' + location.pathname + '<br>' +
        '<span style="color:var(--text-muted)">WS: </span><span style="color:' + (state !== null ? wsColor(state) : 'var(--text-muted)') + '">' + (state !== null ? wsLabel(state) : '—') + '</span><br>' +
        '<span style="color:var(--text-muted)">Key: </span>' + keyPrefix() + '<br>' +
        '<span style="color:var(--text-muted)">SW: </span>humanity-v9';
    }
    update();
    setInterval(update, 2000);
  })();

  // ── Theme toggle (light/dark) ──
  (function () {
    var THEME_KEY = 'hos_theme';
    var root = document.documentElement;

    function applyTheme(theme) {
      var btn = document.getElementById('hos-theme-toggle');
      var mobileLink = document.getElementById('mobile-theme-link');
      if (theme === 'light') {
        root.setAttribute('data-theme', 'light');
        if (btn) btn.textContent = '☀️ Theme';
        if (mobileLink) mobileLink.textContent = '☀️ Toggle Theme';
      } else {
        root.removeAttribute('data-theme');
        if (btn) btn.textContent = '🌙 Theme';
        if (mobileLink) mobileLink.textContent = '🌙 Toggle Theme';
      }
      localStorage.setItem(THEME_KEY, theme);
    }

    // Expose globally so the onclick="hosToggleTheme()" in the nav can reach it
    window.hosToggleTheme = function () {
      var current = root.getAttribute('data-theme') === 'light' ? 'dark' : 'light';
      applyTheme(current);
    };

    // Apply saved preference immediately on page load
    applyTheme(localStorage.getItem(THEME_KEY) || 'dark');
  })();

  // ── Update Checker ─────────────────────────────────────────────────────────
  // WHY: Light up the download button with RGB when a new version is available
  // so the user knows at a glance. Checks GitHub releases once per session.
  (function updateChecker() {
    var CURRENT_VERSION = '0.111.0';
    var CACHE_KEY = 'hos_latest_version';
    var CACHE_TS_KEY = 'hos_latest_version_ts';
    var CHECK_INTERVAL = 30 * 60 * 1000; // 30 min

    function compareVersions(a, b) {
      var pa = a.replace(/^v/, '').split('.').map(Number);
      var pb = b.replace(/^v/, '').split('.').map(Number);
      for (var i = 0; i < 3; i++) {
        if ((pa[i] || 0) < (pb[i] || 0)) return -1;
        if ((pa[i] || 0) > (pb[i] || 0)) return 1;
      }
      return 0;
    }

    function markUpdateReady(latestTag) {
      var dlTab = document.querySelector('a.tab[href="/download"]');
      if (!dlTab) return;
      dlTab.classList.add('tab-update-ready');
      dlTab.setAttribute('data-tip', 'Update Available! v' + latestTag.replace(/^v/, ''));

      // Override click: navigate to download page where the update flow lives.
      dlTab.addEventListener('click', function(e) {
        e.preventDefault();
        window.location.href = '/download';
      });
    }

    function checkUpdate() {
      // Use cached result if fresh enough.
      var cached = localStorage.getItem(CACHE_KEY);
      var cachedTs = parseInt(localStorage.getItem(CACHE_TS_KEY) || '0', 10);
      if (cached && (Date.now() - cachedTs) < CHECK_INTERVAL) {
        if (compareVersions(CURRENT_VERSION, cached) < 0) markUpdateReady(cached);
        return;
      }

      fetch('https://api.github.com/repos/Shaostoul/Humanity/releases/latest')
        .then(function(r) { return r.json(); })
        .then(function(d) {
          if (d && d.tag_name) {
            localStorage.setItem(CACHE_KEY, d.tag_name);
            localStorage.setItem(CACHE_TS_KEY, String(Date.now()));
            if (compareVersions(CURRENT_VERSION, d.tag_name) < 0) {
              markUpdateReady(d.tag_name);
            }
          }
        })
        .catch(function() { /* offline or rate-limited — skip silently */ });
    }

    // Desktop app: Tauri injects __HOS_UPDATE_READY after its background check.
    // When detected, override the download button to invoke the Tauri install command
    // directly from the nav bar (no navigation to /download needed).
    function markDesktopUpdate(version) {
      var dlTab = document.querySelector('a.tab[href="/download"]');
      if (!dlTab || dlTab.classList.contains('tab-update-ready')) return;
      dlTab.classList.add('tab-update-ready');
      dlTab.setAttribute('data-tip', 'Update available — v' + version + ' (click to install)');

      // Add notification badge dot
      var badge = document.createElement('span');
      badge.className = 'tab-update-badge';
      badge.textContent = '!';
      dlTab.style.position = 'relative';
      dlTab.appendChild(badge);

      dlTab.addEventListener('click', function(e) {
        e.preventDefault();
        if (!window.__TAURI__?.core?.invoke) {
          window.location.href = '/download';
          return;
        }
        dlTab.setAttribute('data-tip', 'Downloading v' + version + '...');
        // Remove badge during install
        var b = dlTab.querySelector('.tab-update-badge');
        if (b) b.remove();

        window.__TAURI__.core.invoke('install_update')
          .then(function(v) {
            dlTab.setAttribute('data-tip', 'Installed v' + v + '! Restarting...');
            dlTab.classList.remove('tab-update-ready');
          })
          .catch(function(err) {
            dlTab.setAttribute('data-tip', 'Update failed — click to retry');
            console.error('Update install failed:', err);
          });
      });
    }

    // Show version info in footer when running in desktop app
    var footerLabel = document.getElementById('hos-footer-label');
    if (footerLabel && window.__HOS_APP_VERSION) {
      footerLabel.innerHTML = 'HumanityOS — App v' + window.__HOS_APP_VERSION +
        ' · Web v' + CURRENT_VERSION +
        ' — Public domain · <a href="https://creativecommons.org/publicdomain/zero/1.0/" target="_blank">CC0 1.0</a>';
    }

    // Delay the check so it doesn't compete with page load.
    setTimeout(function() {
      if (window.__HOS_UPDATE_READY) {
        markDesktopUpdate(window.__HOS_UPDATE_VERSION);
      } else {
        checkUpdate(); // Web: check GitHub API
      }
    }, 3000);

    // Poll for late Tauri signal (its 5s check fires after our 3s timeout).
    var _hosUpdatePoll = setInterval(function() {
      if (window.__HOS_UPDATE_READY) {
        clearInterval(_hosUpdatePoll);
        markDesktopUpdate(window.__HOS_UPDATE_VERSION);
      }
    }, 2000);
    setTimeout(function() { clearInterval(_hosUpdatePoll); }, 30000);
  })();

  // ── Onboarding Tour ──
  // Load the tour script; it auto-starts for first-time users.
  var tourScript = document.createElement('script');
  tourScript.src = '/shared/onboarding-tour.js';
  document.head.appendChild(tourScript);

  // "Take Tour" footer link
  var takeTourLink = document.getElementById('hos-take-tour');
  if (takeTourLink) {
    takeTourLink.addEventListener('click', function(e) {
      e.preventDefault();
      if (typeof window.startOnboardingTour === 'function') {
        window.startOnboardingTour();
      }
    });
  }

})();
