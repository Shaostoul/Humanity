// Moved out of web/pages/settings.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // Populate sidebar icons from icons.js
    document.querySelectorAll('.s-ico[data-icon]').forEach(function(el) {
      if (window.hosIcon) el.innerHTML = hosIcon(el.dataset.icon, 16);
    });
