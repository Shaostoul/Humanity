// Moved out of web/pages/files.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    (function () {
      var framed = false;
      try { framed = window.self !== window.top; } catch (e) { framed = true; }
      if (framed && /(?:^|[?&])window=(?:upload|item)(?:&|$)/.test(location.search.slice(1))) document.documentElement.setAttribute('data-files-window', '');
    })();
