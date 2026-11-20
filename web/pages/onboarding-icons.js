// Moved out of web/pages/onboarding.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // Render page icons (pg-ico pattern used across the site)
    document.querySelectorAll('.pg-ico[data-icon]').forEach(function(el) {
      if (window.hosIcon) el.innerHTML = hosIcon(el.dataset.icon, parseInt(el.dataset.size) || 20);
    });
