// Moved out of web/pages/wallet.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
  /* Load qrcode.js if available (non-blocking) */
  (function() {
    var s = document.createElement('script');
    s.src = '/chat/qrcode.js';
    s.onerror = function() { console.log('qrcode.js not available, QR codes disabled'); };
    document.head.appendChild(s);
  })();
