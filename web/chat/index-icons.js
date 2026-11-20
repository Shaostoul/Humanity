// Moved out of web/chat/index.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
  (function populateIcons() {
    if (typeof hosIcon !== 'function') { setTimeout(populateIcons, 50); return; }
    document.querySelectorAll('.pg-ico[data-icon]').forEach(function(el) {
      el.innerHTML = hosIcon(el.dataset.icon, parseInt(el.dataset.size) || 20);
    });
  })();
