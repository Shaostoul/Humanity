// Moved out of web/pages/laws.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    'use strict';
    (function () {
      var logic = window.hosLawsLogic;
      var lawsData = null;
      var state = {
        location: '',
        kindTab: 0,
        category: '',
        search: ''
      };
      // Expanded rule ids survive re-renders (filter/search changes).
      var expanded = {};

      var listEl = document.getElementById('laws-list');
      var selectEl = document.getElementById('laws-location');
      var breadcrumbEl = document.getElementById('laws-breadcrumb');
      var disclaimerEl = document.getElementById('laws-disclaimer');
      var categoriesEl = document.getElementById('laws-categories');
      var searchEl = document.getElementById('laws-search');

      function escapeHtml(s) {
        return String(s).replace(/[&<>"']/g, function (c) {
          return ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c];
        });
      }

      function renderJurisdictionSelect() {
        var html = '';
        (lawsData.jurisdictions || []).forEach(function (j) {
          var label = j.level ? i18n.t('laws.location_option', { name: j.name, level: j.level }) : j.name;
          html += '<option value="' + escapeHtml(j.id) + '"' +
            (j.id === state.location ? ' selected' : '') + '>' +
            escapeHtml(label) + '</option>';
        });
        selectEl.innerHTML = html;
      }

      function renderBreadcrumb() {
        breadcrumbEl.textContent = logic.locationBreadcrumb(lawsData, state.location);
      }

      function renderCategories() {
        var cats = lawsData.categories || [];
        if (!cats.length) { categoriesEl.hidden = true; return; }
        categoriesEl.hidden = false;
        var html = '<button class="category-pill' + (state.category === '' ? ' active' : '') +
          '" type="button" data-category="" data-i18n-tip="laws.all_categories_hint" data-tip-desc="' +
          escapeHtml(i18n.t('laws.all_categories_hint')) + '">' + escapeHtml(i18n.t('laws.all_categories')) + '</button>';
        var catHint = escapeHtml(i18n.t('laws.category_hint'));
        cats.forEach(function (cat) {
          html += '<button class="category-pill' + (state.category === cat ? ' active' : '') +
            '" type="button" data-category="' + escapeHtml(cat) + '" data-i18n-tip="laws.category_hint" data-tip-desc="' +
            catHint + '">' + escapeHtml(cat) + '</button>';
        });
        categoriesEl.innerHTML = html;
      }

      function renderRules() {
        var rules = logic.filterRules(lawsData, state.location, {
          kindTab: state.kindTab,
          category: state.category,
          search: state.search
        });
        if (!rules.length) {
          var msg = (lawsData.rules || []).length
            ? i18n.t('laws.no_match')
            : i18n.t('laws.no_data');
          listEl.innerHTML = '<div class="empty-state">' + escapeHtml(msg) + '</div>';
          return;
        }
        // Grouped under jurisdiction headers, broadest (Humanity) first,
        // exactly like the native page.
        var html = '';
        var lastJur = null;
        var rowHint = escapeHtml(i18n.t('laws.row_hint'));
        rules.forEach(function (r) {
          if (r.jurisdiction !== lastJur) {
            lastJur = r.jurisdiction;
            html += '<h3 class="jur-header">' +
              escapeHtml(logic.jurisdictionName(lawsData.jurisdictions, r.jurisdiction)) + '</h3>';
          }
          var base = logic.isBase(r);
          var isOpen = !!expanded[r.id];

          // Provenance travels WITH the entry, because the entry is what
          // travels: people screenshot one row, or open a deep link to it, and
          // a caveat that lives at the top of the page does not go with it.
          var prov;
          if (r.verified && r.verified.on) {
            prov = escapeHtml(i18n.t(r.verified.human_reviewed ? 'laws.checked_reviewed' : 'laws.checked_not_reviewed',
              { date: r.verified.on }));
          } else {
            prov = escapeHtml(i18n.t('laws.never_checked'));
          }

          // The gate. For rules whose failure mode is prison, deportation or
          // death (use of force, weapons carry and discharge, protection
          // orders, mandatory reporting, immigration), a SUMMARY IS THE DEFECT:
          // the compression is exactly what removes the condition that would
          // have saved the reader. If nobody has checked it, do not show it in
          // the answer position at all. Show the citation and a human to ask.
          var gated = r.risk_class === 'liberty_or_life' && !(r.verified && r.verified.on);
          var preview = gated
            ? escapeHtml(i18n.t('laws.gated_preview'))
            : escapeHtml(r.summary);
          html += '<div class="law-row">' +
            '<button class="law-row-head" type="button" data-rule-id="' + escapeHtml(r.id) + '"' +
              ' aria-expanded="' + (isOpen ? 'true' : 'false') + '" data-i18n-tip="laws.row_hint" data-tip-desc="' +
              rowHint + '">' +
              '<span class="kind-chip ' + (base ? 'base' : 'real') + '">' +
              escapeHtml(i18n.t(base ? 'laws.badge_base' : 'laws.badge_real')) + '</span>' +
              (r.category ? '<span class="law-cat">' + escapeHtml(r.category) + '</span>' : '') +
              '<span class="law-title">' + escapeHtml(r.title) + '</span>' +
              '<span class="law-summary-preview">' + preview + '</span>' +
            '</button>' +
            '<div class="law-detail"' + (isOpen ? '' : ' hidden') + '>' +
              (gated
                ? '<p><strong>' + escapeHtml(i18n.t('laws.gated_lead')) + '</strong> ' + escapeHtml(i18n.t('laws.gated_rest')) + '</p>'
                : '<p>' + escapeHtml(r.summary) + '</p>') +
              (r.source ? '<p class="law-source">' + escapeHtml(i18n.t('laws.source', { source: r.source })) + '</p>' : '') +
              '<p class="law-source">' + prov + '</p>' +
              (r.tags && r.tags.length
                ? '<p class="law-tags">' + escapeHtml(i18n.t('laws.tags', { tags: r.tags.join(', ') })) + '</p>'
                : '') +
            '</div>' +
          '</div>';
        });
        listEl.innerHTML = html;
      }

      function renderAll() {
        renderBreadcrumb();
        renderCategories();
        renderRules();
      }

      // ── Event wiring ──
      selectEl.addEventListener('change', function () {
        state.location = selectEl.value;
        try { localStorage.setItem('humanity_laws_location', state.location); } catch (e) { /* private mode */ }
        renderAll();
      });

      document.getElementById('laws-kind-tabs').addEventListener('click', function (e) {
        var btn = e.target.closest('.kind-tab');
        if (!btn) return;
        state.kindTab = parseInt(btn.dataset.kind, 10) || 0;
        document.querySelectorAll('.kind-tab').forEach(function (b) {
          b.classList.toggle('active', b === btn);
        });
        renderRules();
      });

      categoriesEl.addEventListener('click', function (e) {
        var btn = e.target.closest('.category-pill');
        if (!btn) return;
        var cat = btn.dataset.category || '';
        // Click the active category again to clear it (native chip behavior).
        state.category = (cat !== '' && state.category === cat) ? '' : cat;
        renderCategories();
        renderRules();
      });

      searchEl.addEventListener('input', function () {
        state.search = searchEl.value;
        renderRules();
      });

      listEl.addEventListener('click', function (e) {
        var head = e.target.closest('.law-row-head');
        if (!head) return;
        var id = head.dataset.ruleId;
        expanded[id] = !expanded[id];
        head.setAttribute('aria-expanded', expanded[id] ? 'true' : 'false');
        var detail = head.parentElement.querySelector('.law-detail');
        if (detail) detail.hidden = !expanded[id];
      });

      // ── Load the shared data file (same JSON the native app reads) ──
      fetch('/data/laws/laws.json', { cache: 'no-cache' })
        .then(function (r) { return r.ok ? r.json() : Promise.reject('HTTP ' + r.status); })
        .then(function (data) {
          lawsData = data || {};
          if (data.disclaimer) {
            // The file's own words now; a later language switch must not put
            // the fallback back over them.
            disclaimerEl.removeAttribute('data-i18n');
            disclaimerEl.textContent = data.disclaimer;
            disclaimerEl.hidden = false;
          }
          // Default: most local entry (last in the file), like the native page.
          // Restore a previously chosen jurisdiction when it still exists.
          var saved = null;
          try { saved = localStorage.getItem('humanity_laws_location'); } catch (e) { /* private mode */ }
          state.location = (saved && logic.findJurisdiction(data.jurisdictions, saved))
            ? saved
            : logic.defaultLocation(data);
          renderJurisdictionSelect();
          renderAll();
        })
        .catch(function (err) {
          console.warn('[HOS] Could not load laws data:', err);
          listEl.innerHTML = '<div class="empty-state">' + escapeHtml(i18n.t('laws.load_failed')) + '</div>';
        });
    })();
