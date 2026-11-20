// Moved out of web/pages/ops.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
(function () {
  var ORDER = ['native-app', 'web-page', 'chat-command', 'config-file', 'vps-shell'];
  var LABEL = {
    'native-app': 'In the HumanityOS app', 'web-page': 'On the website',
    'chat-command': 'Chat /commands', 'config-file': 'Server config file',
    'vps-shell': 'VPS shell only (needs SSH)'
  };
  function esc(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }
  fetch('/data/admin/ops_registry.json')
    .then(function (r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
    .then(function (reg) {
      var el = document.getElementById('ops-admin-map');
      var h = '';
      ORDER.forEach(function (surface) {
        var acts = reg.actions.filter(function (a) { return a.where === surface; });
        if (!acts.length) return;
        h += '<details><summary><strong>' + LABEL[surface] + ' (' + acts.length + ')</strong>';
        if (reg.surfaces && reg.surfaces[surface]) h += ' <small>' + esc(reg.surfaces[surface]) + '</small>';
        h += '</summary><ul>';
        acts.forEach(function (a) {
          h += '<li><strong>' + esc(a.name) + '</strong><br><small>' + esc(a.what) +
               '</small><br><small><em>How: ' + esc(a.how) + '</em></small></li>';
        });
        h += '</ul></details>';
      });
      if (reg.planned && reg.planned.length) {
        h += '<details><summary><strong>Coming to the app (' + reg.planned.length + ')</strong></summary><ul>';
        reg.planned.forEach(function (p) {
          h += '<li><strong>' + esc(p.name) + '</strong> [' + esc(p.effort) + ']<br><small>Today: ' +
               esc(p.today) + ' Planned: ' + esc(p.proposal) + '</small></li>';
        });
        h += '</ul></details>';
      }
      if (reg.vps_only && reg.vps_only.length) {
        h += '<details><summary><strong>Stays on the VPS (' + reg.vps_only.length + ')</strong></summary><ul>';
        reg.vps_only.forEach(function (v) {
          h += '<li><strong>' + esc(v.name) + '</strong><br><small>' + esc(v.reason) + '</small></li>';
        });
        h += '</ul></details>';
      }
      el.innerHTML = h;
    })
    .catch(function (e) {
      document.getElementById('ops-admin-map').innerHTML =
        '<em>Could not load the admin map (' + esc(e.message) + '). It lives at /data/admin/ops_registry.json.</em>';
    });
})();
