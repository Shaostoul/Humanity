/**
 * Humanity Shared Shell — Nav + Footer
 * Include this script at the top of <body> on any page.
 * Set data-active="chat" (or home/reality/fantasy/streams/debug) on the <script> tag
 * to highlight the active tab. If omitted, auto-detects from URL.
 *
 * Usage:
 *   <script src="/shared/shell.js" data-active="chat"></script>
 */
(function () {
  // ── Detect active tab ──
  const scriptTag = document.currentScript;
  let active = scriptTag && scriptTag.getAttribute('data-active');
  if (!active) {
    const p = location.pathname;
    if (p === '/') active = 'home';
    else if (p.startsWith('/chat')) active = 'chat';
    else if (p.startsWith('/map')) active = 'map';
    else if (p.startsWith('/board')) active = 'board';
    else if (p.startsWith('/reality')) active = 'reality';
    else if (p.startsWith('/fantasy')) active = 'fantasy';
    else if (p.startsWith('/streams')) active = 'streams';
    else if (p.startsWith('/market')) active = 'market';
    else if (p.startsWith('/browse')) active = 'browse';
    else if (p.startsWith('/dashboard')) active = 'dashboard';
    else if (p.startsWith('/info')) active = 'info';
    else if (p.startsWith('/source')) active = 'source';
    else if (p.startsWith('/debug')) active = 'debug';
    else if (p.startsWith('/download')) active = 'download';
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
      padding: 0 1rem;
      height: 40px;
      gap: 0.25rem;
      flex-shrink: 0;
      z-index: 200;
    }
    .hub-nav .brand {
      font-size: 1.1rem;
      font-weight: 900;
      color: #FF8811;
      padding: 0.3rem 0.6rem;
      border-radius: 6px;
      box-shadow: inset 0 0 0 1px #2a6;
      text-decoration: none;
      display: flex;
      align-items: center;
      justify-content: center;
      margin-right: 0.75rem;
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
    .hub-nav .tab {
      display: flex;
      align-items: center;
      gap: 0.35rem;
      padding: 0.3rem 0.75rem;
      height: 28px;
      color: #888;
      cursor: pointer;
      font-size: 0.8rem;
      border-radius: 6px;
      transition: color 0.15s ease;
      user-select: none;
      text-decoration: none;
      background: transparent;
      box-shadow: inset 0 0 0 1px #2a6;
    }
    .hub-nav .tab:hover {
      color: #e0e0e0;
      box-shadow: inset 0 0 0 2px #48f, 0 0 8px rgba(68,136,255,0.3);
    }
    .hub-nav .tab.active {
      color: #fff;
      animation: channeling 3s linear infinite;
    }
    .hub-nav .tab svg {
      width: 14px;
      height: 14px;
      fill: currentColor;
      vertical-align: middle;
      flex-shrink: 0;
    }
    .hub-nav .tab .tab-label { display: inline; }
    .hub-nav.compact .tab .tab-label { display: none; }
    .hub-nav.compact .tab {
      min-width: 30px;
      justify-content: center;
      padding: 0.2rem 0.45rem;
      position: relative;
    }
    .hub-nav.compact .tab:hover::after {
      content: attr(data-tip);
      position: absolute;
      top: calc(100% + 6px);
      left: 50%;
      transform: translateX(-50%);
      background: rgba(10,10,10,0.95);
      color: #ddd;
      border: 1px solid #333;
      border-radius: 6px;
      padding: 0.18rem 0.45rem;
      font-size: 0.68rem;
      white-space: nowrap;
      z-index: 1200;
      pointer-events: none;
    }
    .hub-nav .spacer { flex: 1; }
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
      border-top: 1px solid #333;
      z-index: 100;
      text-align: center;
      font-size: 0.8rem;
      color: #888;
      transition: transform 0.3s ease;
    }
    .site-footer.collapsed .footer-content { display: none; }
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
    .footer-toggle {
      position: absolute;
      top: -20px;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(13, 13, 13, 0.95);
      border: 1px solid #333;
      border-bottom: none;
      border-radius: 8px 8px 0 0;
      color: #888;
      cursor: pointer;
      padding: 1px 16px;
      font-size: 0.7rem;
      line-height: 1;
      z-index: 101;
    }
    .footer-toggle:hover { color: #FF8811; }

    /* Mobile */
    @media (max-width: 768px) {
      .hub-nav {
        overflow-x: auto;
        -webkit-overflow-scrolling: touch;
        padding: 0 0.4rem;
        gap: 0.2rem;
        height: 36px;
      }
      .hub-nav .brand {
        margin-right: 0.35rem;
        padding: 0.2rem 0.45rem;
        font-size: 0.95rem;
      }
      .hub-nav .tab {
        white-space: nowrap;
        flex-shrink: 0;
        height: 24px;
        padding: 0.2rem 0.5rem;
        font-size: 0.72rem;
        gap: 0.25rem;
      }
      .hub-nav .tab svg {
        width: 12px;
        height: 12px;
      }
    }
  `;
  document.head.appendChild(style);

  // ── Inject Nav ──
  const nav = document.createElement('div');
  nav.innerHTML =
    '<nav class="hub-nav">' +
      '<a href="/" class="brand' + (active === 'home' ? ' active' : '') + '">H</a>' +
      '<a href="/chat" class="' + cls('chat') + '">💬 Chat</a>' +
      '<a href="/map" class="' + cls('map') + '">🗺️ Map</a>' +
      '<a href="/board" class="' + cls('board') + '">📋 Board</a>' +
      '<a href="/reality" class="' + cls('reality') + '">🟢 Reality</a>' +
      '<a href="/fantasy" class="' + cls('fantasy') + '">✨ Fantasy</a>' +
      '<a href="/market" class="' + cls('market') + '">🛒 Market</a>' +
      '<a href="/browse" class="' + cls('browse') + '">🌐 Browse</a>' +
      '<a href="/dashboard" class="' + cls('dashboard') + '">📊 Dashboard</a>' +
      '<a href="/streams" class="' + cls('streams') + '">🎬 Streams</a>' +
      '<a href="/info" class="' + cls('info') + '">📖 Info</a>' +
      '<a href="/source" class="' + cls('source') + '">📜 Source</a>' +
      '<a href="/debug" class="' + cls('debug') + '">🔧 Debug</a>' +
      '<span class="spacer"></span>' +
      '<a href="/download" class="' + cls('download') + '" onclick="if(typeof openWebviewTab===\'function\'){openWebviewTab(\'/download\',\'Download\');return false;}">' + dlIcon + ' Download</a>' +
      '<a href="https://github.com/Shaostoul/Humanity" class="tab" onclick="openWebviewTab(\'https://github.com/Shaostoul/Humanity\',\'GitHub\');return false;">' + ghIcon + ' GitHub</a>' +
    '</nav>' +
    '<div id="webview-tabs-bar" style="display:none;height:32px;background:rgba(13,13,13,0.95);border-bottom:1px solid #333;align-items:center;padding:0 0.5rem;gap:0.3rem;overflow-x:auto;"></div>' +
    '<div class="nav-separator"></div>';
  document.body.prepend(nav);

  function applyNavLabelWrappingAndCompaction() {
    var navEl = document.querySelector('.hub-nav');
    if (!navEl) return;

    navEl.querySelectorAll('a.tab').forEach(function(a) {
      if (a.querySelector('.tab-label')) return;
      var txt = (a.textContent || '').replace(/\s+/g, ' ').trim();
      // Keep icon-like first token, label is the rest.
      var parts = txt.split(' ');
      var label = parts.length > 1 ? parts.slice(1).join(' ') : txt;
      if (!label) return;
      a.setAttribute('data-tip', label);
      a.setAttribute('title', label);
      // Replace trailing text node with wrapped label while preserving existing SVG/icon HTML.
      var html = a.innerHTML;
      var stripped = html.replace(/\s+[^<\s][^<]*$/m, '');
      a.innerHTML = stripped + ' <span class="tab-label">' + label + '</span>';
    });

    var shouldCompact = navEl.scrollWidth > navEl.clientWidth + 4 || window.innerWidth < 1180;
    navEl.classList.toggle('compact', shouldCompact);
  }

  applyNavLabelWrappingAndCompaction();
  window.addEventListener('resize', applyNavLabelWrappingAndCompaction);

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
    const link = e.target.closest('a[href]');
    if (!link) return;
    const href = link.getAttribute('href');
    /* NOTE: nginx hub regex needs /market added */
    const hubPaths = ['/map', '/board', '/reality', '/fantasy', '/market', '/browse', '/dashboard', '/streams', '/info', '/source', '/debug'];
    const currentIsHub = hubPaths.some(function(p) { return location.pathname === p; });
    const targetIsHub = hubPaths.some(function(p) { return href === p; });

    // Streaming continuity guard: when live on /streams, keep the stream page loaded
    // and open other hub pages in a webview tab instead of switching away.
    if (location.pathname === '/streams' && targetIsHub && href !== '/streams' && isLikelyLiveStreamingSession()) {
      e.preventDefault();
      var title = (link.textContent || href).trim();
      if (typeof openWebviewTab === 'function') {
        openWebviewTab(href, title);
      } else {
        // Browser fallback: explicit warning (switching route can break media capture state).
        if (confirm('A live stream appears active. Switching tabs may interrupt your stream. Continue?')) {
          location.href = href;
        }
      }
      return;
    }

    if (currentIsHub && targetIsHub && href !== location.pathname) {
      e.preventDefault();
      history.pushState({}, '', href);
      window.dispatchEvent(new PopStateEvent('popstate'));
      document.querySelectorAll('.hub-nav a').forEach(function(a) { a.classList.remove('active'); });
      link.classList.add('active');
    }
  });

  // ── Inject Footer ──
  const footer = document.createElement('div');
  footer.innerHTML =
    '<footer class="site-footer" id="site-footer">' +
      '<button class="footer-toggle" id="footer-toggle">▼</button>' +
      '<div class="footer-content" id="footer-content">' +
        '<span>Humanity — Public domain · <a href="https://creativecommons.org/publicdomain/zero/1.0/" target="_blank">CC0 1.0</a></span>' +
        '<div class="footer-links">' +
          '<a href="/">Home</a>' +
          '<a href="/chat">Chat</a>' +
          '<a href="https://shaostoul.github.io/Humanity" onclick="openWebviewTab(\'https://shaostoul.github.io/Humanity\',\'Docs\');return false;">Docs</a>' +
          '<a href="https://github.com/Shaostoul/Humanity" target="_blank">GitHub</a>' +
          '<a href="https://discord.gg/9XxmmeQnWC" target="_blank">Discord</a>' +
          '<a href="https://youtube.com/@Shaostoul" target="_blank">YouTube</a>' +
          '<a href="https://x.com/Shaostoul" target="_blank">X</a>' +
        '</div>' +
      '</div>' +
    '</footer>';
  document.body.appendChild(footer);

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
  document.getElementById('footer-toggle').addEventListener('click', function () {
    const ft = document.getElementById('site-footer');
    ft.classList.toggle('collapsed');
    this.textContent = ft.classList.contains('collapsed') ? '▲' : '▼';
    localStorage.setItem('footer_collapsed', ft.classList.contains('collapsed'));
  });

  // Restore collapsed state
  if (localStorage.getItem('footer_collapsed') === 'true') {
    var ft = document.getElementById('site-footer');
    var btn = document.getElementById('footer-toggle');
    if (ft) ft.classList.add('collapsed');
    if (btn) btn.textContent = '▲';
  }
})();
