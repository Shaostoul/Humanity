/**
 * HumanityOS Shared Shell — Nav + Footer
 * Place as the FIRST child of <body> on any page.
 * Set data-active="<key>" on the <script> tag to highlight the matching nav tab.
 * If omitted, active tab is auto-detected from the current URL.
 *
 * Valid active keys: chat, profile, home, skills, inventory, equipment, quests,
 *   calendar, logbook, board, map, market, browse, info, streams, debug, download,
 *   settings, reality, fantasy
 *
 * Usage:
 *   <script src="/shared/shell.js" data-active="inventory"></script>
 */
(function () {
  if (window.__HOS_SHELL_INIT__) return;
  window.__HOS_SHELL_INIT__ = true;

  // If prior shell artifacts somehow exist, remove them before injecting once.
  document.querySelectorAll('.hub-nav, .nav-separator, .site-footer, #webview-tabs-bar').forEach(function(el){
    if (el && el.parentNode) el.parentNode.removeChild(el);
  });

  // ── Detect active tab ──
  const scriptTag = document.currentScript;
  let active = scriptTag && scriptTag.getAttribute('data-active');
  if (!active) {
    const p = location.pathname;
    if (p === '/') active = 'home';
    else if (p.startsWith('/chat'))      active = 'chat';
    else if (p.startsWith('/profile'))   active = 'profile';
    else if (p.startsWith('/home'))      active = 'home';
    else if (p.startsWith('/skills'))    active = 'skills';
    else if (p.startsWith('/inventory')) active = 'inventory';
    else if (p.startsWith('/equipment')) active = 'equipment';
    else if (p.startsWith('/quests'))    active = 'quests';
    else if (p.startsWith('/calendar'))  active = 'calendar';
    else if (p.startsWith('/logbook'))   active = 'logbook';
    else if (p.startsWith('/board'))     active = 'board';
    else if (p.startsWith('/map'))       active = 'map';
    else if (p.startsWith('/market'))    active = 'market';
    else if (p.startsWith('/browse'))    active = 'browse';
    else if (p.startsWith('/info'))      active = 'info';
    else if (p.startsWith('/streams'))   active = 'streams';
    else if (p.startsWith('/settings'))  active = 'settings';
    else if (p.startsWith('/debug'))     active = 'debug';
    else if (p.startsWith('/download'))  active = 'download';
    // Legacy app.html SPA routes — kept for backward compat while those tabs still exist
    else if (p.startsWith('/reality'))   active = 'reality';   // old profile/reality tab
    else if (p.startsWith('/fantasy'))   active = 'skills';    // superseded by /skills
    else if (p.startsWith('/source'))    active = 'equipment'; // superseded by /equipment
    else if (p.startsWith('/dashboard')) active = 'home';      // superseded by /home
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
      position: sticky;
      top: 0;
      z-index: 5500;
      isolation: isolate;
    }

    /* ── Brand ── */
    .hub-nav .brand {
      font-size: 1.1rem;
      font-weight: 900;
      color: #FF8811;
      width: 32px;
      height: 28px;
      border-radius: 6px;
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
      color: #888;
      cursor: pointer;
      border-radius: 6px;
      user-select: none;
      text-decoration: none;
      flex-shrink: 0;
      box-shadow: inset 0 0 0 1px #2a6;
      transition: color 0.1s, box-shadow 0.1s;
      overflow: visible;
    }
    .hub-nav .tab .tab-icon {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }
    .hub-nav .tab .tab-icon img {
      width: 15px;
      height: 15px;
      object-fit: contain;
      display: block;
      opacity: 0.65;
      transition: opacity 0.1s;
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
      color: #e0e0e0;
    }
    .hub-nav .tab:not(.active):hover .tab-icon img { opacity: 1; }
    .hub-nav .tab:not(.active):hover::after {
      content: attr(data-tip);
      position: absolute;
      /* sits below the button with a gap so the icon is never obscured */
      top: calc(100% + 10px);
      left: 50%;
      transform: translateX(-50%);
      background: rgba(8,8,10,0.97);
      color: #ddd;
      border: 1px solid #3a3a3a;
      border-radius: 6px;
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
    .hub-nav .tab.active .tab-icon img { opacity: 1; }
    /* No ::after tooltip on active tab */
    .hub-nav .tab.active::after { display: none !important; }

    /* ── Divider between nav groups — negative margin cancels the double flex-gap ── */
    .hub-nav .nav-divider {
      width: 1px;
      height: 18px;
      background: rgba(255,255,255,0.15);
      flex-shrink: 0;
      margin: 0 -0.1rem; /* gap is 0.2rem; -0.1rem each side keeps visual spacing even */
    }

    /* ── Spacer pushes right-side items to the right ── */
    .hub-nav .spacer { flex: 1; }

    /* ── Mobile hamburger — hidden on desktop ── */
    .hub-nav .mobile-menu-btn {
      display: none;
      background: transparent;
      border: 1px solid #2a6;
      color: #ddd;
      padding: 0.24rem 0.55rem;
      border-radius: 6px;
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
      border-top: 1px solid #444;
      z-index: 2100;
      text-align: center;
      font-size: 0.8rem;
      color: #888;
      transition: transform 0.3s ease;
    }
    /* Footer slides off-screen when collapsed instead of hiding content */
    .site-footer { transition: transform 0.25s ease; }
    .site-footer.collapsed { transform: translateY(100%); }
    .site-footer .footer-content { padding: 10px 16px; }
    .site-footer .footer-content a { color: #FF8811; text-decoration: none; }
    .site-footer .footer-content a:hover { color: #ff9f40; }
    .site-footer .footer-links {
      display: flex;
      gap: 16px;
      justify-content: center;
      flex-wrap: wrap;
      margin-top: 6px;
    }
    .site-footer .footer-links a { color: #888; text-decoration: none; font-size: 0.8rem; }
    .site-footer .footer-links a:hover { color: #FF8811; }
    /* Toggle is its own fixed element — always visible above all content */
    .footer-toggle {
      position: fixed;
      bottom: 0;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(18, 18, 18, 0.97);
      border: 1px solid #666;
      border-bottom: none;
      border-radius: 8px 8px 0 0;
      color: #FF8811;
      cursor: pointer;
      padding: 5px 28px;
      font-size: 0.78rem;
      font-weight: 700;
      line-height: 1;
      z-index: 5200; /* above everything except the nav (5500) */
      transition: color 0.15s, border-color 0.15s, background 0.15s;
      white-space: nowrap;
      letter-spacing: 0.05em;
    }
    .footer-toggle:hover { color: #fff; border-color: #FF8811; background: rgba(30,20,10,0.98); }

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
    .mobile-hub-group { margin-bottom: 0.65rem; border:1px solid #2a2a2a; border-radius:8px; }
    .mobile-hub-group h4 { margin:0; padding:0.45rem 0.55rem; font-size:0.72rem; color:#9aa; border-bottom:1px solid #2a2a2a; text-transform:uppercase; letter-spacing:.08em; }
    .mobile-hub-group a { display:block; color:#ddd; text-decoration:none; padding:0.5rem 0.55rem; font-size:0.86rem; border-bottom:1px solid #1d1d1d; }
    .mobile-hub-group a:last-child { border-bottom:none; }
    .mobile-hub-group a:hover { background: rgba(255,255,255,0.05); }
    .mobile-hub-group a.active {
      color: #fff;
      background: rgba(255,255,255,0.06);
      animation: channeling 3s linear infinite;
      border-radius: 6px;
      margin: 0.15rem;
    }

    /* ── Responsive: hide flat tabs on mobile, show hamburger ── */
    @media (max-width: 768px) {
      .hub-nav { padding: 0 0.4rem; gap: 0.15rem; height: 36px; }
      .hub-nav .tab { display: none !important; }
      .hub-nav .nav-divider { display: none !important; }
      .hub-nav .spacer { display: none !important; }
      .hub-nav .brand { margin-right: 0.25rem; }
      .hub-nav .mobile-menu-btn { display: inline-flex; align-items:center; justify-content:center; margin-left:auto; }
    }
  `;
  document.head.appendChild(style);

  // Helper: build a nav tab anchor
  function navTab(href, icon, label, activeKey) {
    var isActive = active === activeKey;
    var cls = 'tab' + (isActive ? ' active' : '');
    return '<a href="' + href + '" class="' + cls + '" data-tip="' + label + '">' +
      '<span class="tab-icon"><img src="/shared/ui-icons/' + icon + '" alt="" onerror="this.onerror=null;this.src=\'/shared/ui-icons/warning.png\';"></span>' +
      '<span class="tab-label">' + label + '</span>' +
    '</a>';
  }

  // ── Inject Nav ──
  const nav = document.createElement('div');
  nav.innerHTML =
    '<nav class="hub-nav">' +
      /* Brand */
      '<a href="/" class="brand' + (active === 'home' ? ' active' : '') + '" data-tip="Home">H</a>' +

      /* Network (always first, most important) */
      navTab('/chat',      'chat.png',      'Network',   'chat') +
      '<div class="nav-divider"></div>' +

      /* Private — personal pages */
      navTab('/profile',   'profile.png',   'Profile',   'profile') +
      navTab('/home',      'home.png',      'Home',      'home') +
      navTab('/skills',    'skills.png',    'Skills',    'skills') +
      navTab('/inventory', 'inventory.png', 'Inventory', 'inventory') +
      navTab('/equipment', 'components.png','Equipment', 'equipment') +
      navTab('/quests',    'tasklist.png',  'Quests',    'quests') +
      navTab('/calendar',  'calendar.png',  'Calendar',  'calendar') +
      navTab('/logbook',   'logs.png',      'Logbook',   'logbook') +
      '<div class="nav-divider"></div>' +

      /* Public — community pages */
      navTab('/board',     'systems.png',   'Systems',   'board') +
      navTab('/map',       'map.png',       'Maps',      'map') +
      navTab('/market',    'market.png',    'Market',    'market') +
      navTab('/browse',    'website.png',   'Learn',     'browse') +
      navTab('/info',      'codex.png',     'Knowledge', 'info') +
      navTab('/streams',   'audio.png',     'Streams',   'streams') +

      /* Spacer pushes ops/account to the right */
      '<div class="spacer"></div>' +

      /* Right side — ops, download, settings */
      navTab('/debug',    'system.png',   'Ops',      'debug') +
      navTab('/download', 'save.png',     'Download', 'download') +
      navTab('/settings', 'settings.png', 'Settings', 'settings') +

      /* Mobile hamburger — only visible on small screens */
      '<button class="mobile-menu-btn" id="mobile-hub-menu-btn" type="button" aria-label="Open menu">☰</button>' +
    '</nav>' +
    '<div id="webview-tabs-bar" style="display:none;height:32px;background:rgba(13,13,13,0.95);border-bottom:1px solid #333;align-items:center;padding:0 0.5rem;gap:0.3rem;overflow-x:auto;"></div>' +
    '<div class="nav-separator"></div>';
  document.body.prepend(nav);

  // Mobile drawer fallback menu (for reliable touch nav)
  var mobileBackdrop = document.createElement('div');
  mobileBackdrop.id = 'mobile-hub-backdrop';
  var mobileDrawer = document.createElement('aside');
  mobileDrawer.id = 'mobile-hub-drawer';
  function mobileLink(path, label) {
    var current = pathname || '/';
    var isActive = current === path || (path !== '/' && current.startsWith(path + '/'));
    return '<a href="' + path + '"' + (isActive ? ' class="active"' : '') + '>' + label + '</a>';
  }

  mobileDrawer.innerHTML =
    '<div class="mobile-hub-group"><h4>Network</h4>' +
      mobileLink('/chat', 'Network Chat') +
    '</div>' +
    '<div class="mobile-hub-group"><h4>Private</h4>' +
      mobileLink('/profile',   'Profile') +
      mobileLink('/home',      'Home') +
      mobileLink('/skills',    'Skills') +
      mobileLink('/inventory', 'Inventory') +
      mobileLink('/equipment', 'Equipment') +
      mobileLink('/quests',    'Quests') +
      mobileLink('/calendar',  'Calendar') +
      mobileLink('/logbook',   'Logbook') +
    '</div>' +
    '<div class="mobile-hub-group"><h4>Public</h4>' +
      mobileLink('/board',   'Systems') +
      mobileLink('/map',     'Maps') +
      mobileLink('/market',  'Market') +
      mobileLink('/browse',  'Learn') +
      mobileLink('/info',    'Knowledge') +
      mobileLink('/streams', 'Streams') +
    '</div>' +
    '<div class="mobile-hub-group"><h4>App</h4>' +
      mobileLink('/debug',    'Ops') +
      mobileLink('/download', 'Download') +
      mobileLink('/settings', 'Settings') +
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
    if (l.includes('network')) return 'Takes you to communication and connection controls.';
    return 'Tap or click to use this control.';
  }

  function initRichTooltips() {
    if (window.__HOS_RICH_TOOLTIPS__) return;
    window.__HOS_RICH_TOOLTIPS__ = true;

    var tip = document.createElement('div');
    tip.id = 'hos-rich-tooltip';
    tip.style.cssText = 'position:fixed;z-index:9000;pointer-events:none;max-width:260px;background:rgba(8,8,10,0.96);border:1px solid rgba(130,130,140,0.35);border-radius:8px;padding:6px 8px;color:#ddd;font-size:12px;line-height:1.35;box-shadow:0 8px 20px rgba(0,0,0,0.45);display:none;';
    document.body.appendChild(tip);

    function showFor(el, x, y) {
      if (!el) return;
      var title = el.getAttribute('data-tip-title') || el.getAttribute('data-native-title') || el.getAttribute('aria-label') || el.getAttribute('data-tip') || (el.textContent || '').trim();
      if (!title) return;
      var desc = el.getAttribute('data-tip-desc') || defaultTooltipDescription(title);
      tip.innerHTML = '<div style="font-weight:600;color:#fff;margin-bottom:2px;">' + title.replace(/</g,'&lt;') + '</div><div style="color:#b9c2d0;">' + desc.replace(/</g,'&lt;') + '</div>';
      tip.style.display = 'block';
      var tx = Math.min(window.innerWidth - 280, Math.max(8, x + 12));
      var ty = Math.min(window.innerHeight - 90, Math.max(8, y + 12));
      tip.style.left = tx + 'px';
      tip.style.top = ty + 'px';
    }

    function hideTip() { tip.style.display = 'none'; }

    document.querySelectorAll('[title]').forEach(function(el) {
      var t = el.getAttribute('title');
      if (t && !el.getAttribute('data-native-title')) {
        el.setAttribute('data-native-title', t);
        el.removeAttribute('title');
      }
    });

    document.addEventListener('mouseover', function(e) {
      var el = e.target.closest('[data-native-title],[data-tip],[aria-label],button,a,[role="button"]');
      if (!el) return;
      showFor(el, e.clientX || 8, e.clientY || 8);
    });
    document.addEventListener('mousemove', function(e) {
      if (tip.style.display !== 'block') return;
      var tx = Math.min(window.innerWidth - 280, Math.max(8, (e.clientX || 8) + 12));
      var ty = Math.min(window.innerHeight - 90, Math.max(8, (e.clientY || 8) + 12));
      tip.style.left = tx + 'px';
      tip.style.top = ty + 'px';
    });
    document.addEventListener('mouseout', function(e) {
      if (e.target && e.target.closest && e.target.closest('[data-native-title],[data-tip],[aria-label],button,a,[role="button"]')) hideTip();
    });
    document.addEventListener('focusin', function(e) {
      var el = e.target.closest('[data-native-title],[data-tip],[aria-label],button,a,[role="button"]');
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

  function isLikelyLiveStreamingSession() {
    var stopBtn = document.getElementById('stream-stop-btn');
    if (!stopBtn) return false;
    var display = stopBtn.style && stopBtn.style.display ? stopBtn.style.display : '';
    return display !== 'none';
  }

  // ── SPA Navigation for Hub Tabs ──
  document.querySelector('.hub-nav').addEventListener('click', function(e) {
    if (e.target.closest('.mobile-menu-btn')) return;

    const link = e.target.closest('a[href]');
    if (!link) return;
    const href = link.getAttribute('href');
    // Only app.html SPA routes here — standalone HTML pages need real full-page
    // navigation so their data-active attribute fires on load and sets active state.
    const hubPaths = ['/map', '/board', '/reality', '/fantasy', '/market', '/browse', '/dashboard',
                      '/streams', '/info', '/source', '/debug'];
    const currentIsHub = hubPaths.some(function(p) { return location.pathname === p; });
    const targetIsHub  = hubPaths.some(function(p) { return href === p || href.startsWith(p + '#') || href.startsWith(p + '?'); });

    // Streaming continuity guard
    if (location.pathname === '/streams' && targetIsHub && href !== '/streams' && isLikelyLiveStreamingSession()) {
      e.preventDefault();
      var title = (link.getAttribute('data-tip') || href).trim();
      if (typeof openWebviewTab === 'function') {
        openWebviewTab(href, title);
      } else {
        if (confirm('A live stream appears active. Switching may interrupt it. Continue?')) location.href = href;
      }
      return;
    }

    if (currentIsHub && targetIsHub && href.split('#')[0] !== location.pathname) {
      e.preventDefault();
      history.pushState({}, '', href);
      window.dispatchEvent(new PopStateEvent('popstate'));
      document.querySelectorAll('.hub-nav a.tab').forEach(function(a) { a.classList.remove('active'); });
      link.classList.add('active');
    }
  });

  // ── Inject Footer ──
  // Toggle and footer are SEPARATE direct body children so neither can be
  // clipped, buried, or displaced by page-level overflow/transform/stacking.
  var footerToggle = document.createElement('button');
  footerToggle.className = 'footer-toggle';
  footerToggle.id = 'footer-toggle';
  footerToggle.setAttribute('aria-label', 'Toggle footer');
  footerToggle.textContent = '▲';
  document.body.appendChild(footerToggle);

  var footerEl = document.createElement('footer');
  footerEl.className = 'site-footer';
  footerEl.id = 'site-footer';
  footerEl.innerHTML =
    '<div class="footer-content" id="footer-content">' +
      '<span>HumanityOS — Public domain · <a href="https://creativecommons.org/publicdomain/zero/1.0/" target="_blank">CC0 1.0</a></span>' +
      '<div class="footer-links">' +
        '<a href="/">Home</a>' +
        '<a href="/chat">Chat</a>' +
        '<a href="/download" onclick="if(typeof openWebviewTab===\'function\'){openWebviewTab(\'/download\',\'Download\');return false;}">Download</a>' +
        '<a href="https://shaostoul.github.io/Humanity" onclick="if(typeof openWebviewTab===\'function\'){openWebviewTab(\'https://shaostoul.github.io/Humanity\',\'Docs\');return false;}">Docs</a>' +
        '<a href="https://github.com/Shaostoul/Humanity" target="_blank">GitHub</a>' +
        '<a href="https://discord.gg/9XxmmeQnWC" target="_blank">Discord</a>' +
        '<a href="https://youtube.com/@Shaostoul" target="_blank">YouTube</a>' +
        '<a href="https://x.com/Shaostoul" target="_blank">X</a>' +
      '</div>' +
    '</div>';
  document.body.appendChild(footerEl);

  // ── Webview Tab System ──
  var webviewTabs = {};
  var webviewCounter = 0;
  var activeWebviewTab = null;

  window.openWebviewTab = function(url, title) {
    // Check if already open with this URL
    for (var id in webviewTabs) {
      if (webviewTabs[id].url === url) { switchWebviewTab(id); return; }
    }
    var tabId = 'wv-' + (++webviewCounter);
    webviewTabs[tabId] = { url: url, title: title || url };

    // Create content container
    var content = document.createElement('div');
    content.id = 'webview-content-' + tabId;
    content.className = 'webview-tab-content';
    content.style.cssText = 'display:none;flex-direction:column;height:calc(100vh - 80px);position:fixed;top:0;left:0;right:0;bottom:0;z-index:150;background:var(--bg,#0a0a0a);';
    content.innerHTML =
      '<div style="display:flex;gap:0.3rem;padding:0.3rem 0.5rem;border-bottom:1px solid #333;align-items:center;background:rgba(13,13,13,0.95);height:36px;flex-shrink:0;">' +
        '<button onclick="webviewBack(\'' + tabId + '\')" style="background:none;border:1px solid #333;color:#888;padding:0.15rem 0.5rem;border-radius:4px;cursor:pointer;font-size:0.85rem;">←</button>' +
        '<button onclick="webviewForward(\'' + tabId + '\')" style="background:none;border:1px solid #333;color:#888;padding:0.15rem 0.5rem;border-radius:4px;cursor:pointer;font-size:0.85rem;">→</button>' +
        '<button onclick="webviewRefresh(\'' + tabId + '\')" style="background:none;border:1px solid #333;color:#888;padding:0.15rem 0.5rem;border-radius:4px;cursor:pointer;font-size:0.85rem;">↻</button>' +
        '<input type="text" readonly value="' + url.replace(/"/g, '&quot;') + '" style="flex:1;background:#1a1a1a;border:1px solid #333;color:#aaa;padding:0.25rem 0.6rem;border-radius:4px;font-size:0.78rem;font-family:monospace;">' +
        '<button onclick="closeWebviewTab(\'' + tabId + '\')" style="background:none;border:1px solid #333;color:#e55;padding:0.15rem 0.5rem;border-radius:4px;cursor:pointer;font-size:0.85rem;">✕</button>' +
      '</div>' +
      '<iframe src="' + url.replace(/"/g, '&quot;') + '" style="flex:1;border:none;width:100%;" sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"></iframe>';
    document.body.appendChild(content);

    switchWebviewTab(tabId);
    renderWebviewTabBar();
  };

  function switchWebviewTab(tabId) {
    // Hide all webview contents
    for (var id in webviewTabs) {
      var el = document.getElementById('webview-content-' + id);
      if (el) el.style.display = 'none';
    }
    var el = document.getElementById('webview-content-' + tabId);
    if (el) el.style.display = 'flex';
    activeWebviewTab = tabId;
    renderWebviewTabBar();
  }

  window.closeWebviewTab = function(tabId) {
    var el = document.getElementById('webview-content-' + tabId);
    if (el) el.remove();
    delete webviewTabs[tabId];
    if (activeWebviewTab === tabId) {
      var keys = Object.keys(webviewTabs);
      activeWebviewTab = keys.length > 0 ? keys[keys.length - 1] : null;
      if (activeWebviewTab) switchWebviewTab(activeWebviewTab);
    }
    renderWebviewTabBar();
  };

  window.webviewBack = function(tabId) {
    var el = document.getElementById('webview-content-' + tabId);
    if (el) { var iframe = el.querySelector('iframe'); try { iframe.contentWindow.history.back(); } catch(e){} }
  };
  window.webviewForward = function(tabId) {
    var el = document.getElementById('webview-content-' + tabId);
    if (el) { var iframe = el.querySelector('iframe'); try { iframe.contentWindow.history.forward(); } catch(e){} }
  };
  window.webviewRefresh = function(tabId) {
    var el = document.getElementById('webview-content-' + tabId);
    if (el) { var iframe = el.querySelector('iframe'); iframe.src = iframe.src; }
  };

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
      btn.style.cssText = 'display:flex;align-items:center;gap:0.3rem;padding:0.15rem 0.6rem;border-radius:4px;border:1px solid ' + (id===activeWebviewTab?'#FF8811':'#333') + ';background:' + (id===activeWebviewTab?'rgba(255,136,17,0.15)':'transparent') + ';color:' + (id===activeWebviewTab?'#FF8811':'#888') + ';font-size:0.72rem;cursor:pointer;white-space:nowrap;';
      btn.innerHTML = '<span onclick="switchWebviewTab(\'' + id + '\')">' + (tab.title||'Tab').substring(0,20) + '</span><span onclick="event.stopPropagation();closeWebviewTab(\'' + id + '\')" style="margin-left:0.3rem;color:#e55;font-weight:700;">✕</span>';
      btn.onclick = function() { switchWebviewTab(id); };
      bar.appendChild(btn);
    });
  }

  // ── Footer toggle logic ──
  // Both elements are already in the DOM (appended above) — no getElementById needed.
  setTimeout(function () {
    var ft  = footerEl;
    var btn = footerToggle;
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
})();
