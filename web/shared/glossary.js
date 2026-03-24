/**
 * HumanityOS Glossary — Inline term definitions
 *
 * Loads definitions from /data/glossary.json.
 * Auto-scans the page for elements with data-term="word" and adds click handlers.
 * On click, shows a small dark-themed modal/tooltip near the word with the definition.
 *
 * API:
 *   glossary.define(term)   — returns definition string or null
 *   glossary.show(term, el) — manually show tooltip near element
 *   glossary.hide()         — close any open tooltip
 *   glossary.rescan()       — re-scan DOM for new data-term elements
 *
 * Usage in HTML:
 *   <script src="/shared/glossary.js"></script>
 *   <span data-term="orbit">orbit</span>
 *   <span data-term="eccentricity" data-term-link="false">eccentricity</span>
 */
(function () {
  if (window.__HOS_GLOSSARY__) return;
  window.__HOS_GLOSSARY__ = true;

  var terms = {};
  var categories = {};
  var loaded = false;
  var activeTooltip = null;

  // ── Styles ──
  var style = document.createElement('style');
  style.id = 'glossary-styles';
  style.textContent =
    /* Clickable term styling */
    '[data-term]{cursor:help;border-bottom:1px dotted var(--accent,#FF8811);' +
    'transition:border-color 0.15s;}' +
    '[data-term]:hover{border-bottom-color:var(--text,#e0e0e0);}' +

    /* Tooltip card */
    '.glossary-tooltip{position:fixed;z-index:100000;max-width:360px;min-width:220px;' +
    'background:var(--bg-card,#161616);color:var(--text,#e0e0e0);' +
    'border:1px solid var(--border,#333);border-radius:var(--radius,8px);' +
    'padding:14px 16px 12px;box-shadow:0 8px 32px rgba(0,0,0,0.55);' +
    'font:0.88rem/1.55 system-ui,-apple-system,sans-serif;' +
    'opacity:0;transform:translateY(4px);transition:opacity 0.15s,transform 0.15s;' +
    'pointer-events:auto;}' +
    '.glossary-tooltip.visible{opacity:1;transform:translateY(0);}' +

    /* Term heading */
    '.glossary-tooltip__term{font-weight:700;font-size:0.95rem;color:var(--accent,#FF8811);' +
    'margin-bottom:6px;display:flex;align-items:center;justify-content:space-between;}' +

    /* Category badge */
    '.glossary-tooltip__cat{font-size:0.7rem;font-weight:500;' +
    'color:var(--text-muted,#888);background:var(--bg-hover,#252525);' +
    'padding:1px 7px;border-radius:var(--radius-sm,4px);margin-left:8px;white-space:nowrap;}' +

    /* Definition text */
    '.glossary-tooltip__def{color:var(--text,#e0e0e0);line-height:1.55;}' +

    /* Footer row */
    '.glossary-tooltip__foot{margin-top:10px;display:flex;align-items:center;' +
    'justify-content:space-between;gap:8px;}' +

    /* Learn more link */
    '.glossary-tooltip__link{font-size:0.78rem;color:var(--accent,#FF8811);' +
    'text-decoration:none;opacity:0.85;}' +
    '.glossary-tooltip__link:hover{opacity:1;text-decoration:underline;}' +

    /* Close button */
    '.glossary-tooltip__close{background:none;border:none;color:var(--text-muted,#888);' +
    'cursor:pointer;font-size:1.1rem;line-height:1;padding:2px 4px;' +
    'border-radius:var(--radius-sm,4px);}' +
    '.glossary-tooltip__close:hover{color:var(--text,#e0e0e0);background:var(--bg-hover,#252525);}';
  document.head.appendChild(style);

  // ── Load data ──
  function loadGlossary() {
    var xhr = new XMLHttpRequest();
    xhr.open('GET', '/data/glossary.json', true);
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== 4) return;
      if (xhr.status === 200) {
        try {
          var data = JSON.parse(xhr.responseText);
          terms = data.terms || {};
          categories = data.categories || {};
          loaded = true;
          scanPage();
        } catch (e) {
          console.warn('[Glossary] Failed to parse glossary.json:', e);
        }
      } else {
        console.warn('[Glossary] Failed to load glossary.json:', xhr.status);
      }
    };
    xhr.send();
  }

  // ── Lookup ──
  function define(term) {
    if (!loaded) return null;
    var key = String(term).toLowerCase().trim();
    var entry = terms[key];
    return entry ? entry.definition : null;
  }

  function getEntry(term) {
    if (!loaded) return null;
    var key = String(term).toLowerCase().trim();
    return terms[key] || null;
  }

  // ── Tooltip positioning ──
  function positionTooltip(tooltip, anchorRect) {
    var pad = 8;
    var vw = window.innerWidth;
    var vh = window.innerHeight;

    // Temporarily make visible off-screen to measure
    tooltip.style.left = '-9999px';
    tooltip.style.top = '-9999px';
    document.body.appendChild(tooltip);

    var tw = tooltip.offsetWidth;
    var th = tooltip.offsetHeight;

    // Prefer below the element, centered horizontally
    var left = anchorRect.left + (anchorRect.width / 2) - (tw / 2);
    var top = anchorRect.bottom + pad;

    // If it goes below viewport, show above
    if (top + th > vh - pad) {
      top = anchorRect.top - th - pad;
    }
    // If it goes above viewport, clamp to top
    if (top < pad) {
      top = pad;
    }
    // Clamp horizontal
    if (left < pad) left = pad;
    if (left + tw > vw - pad) left = vw - tw - pad;

    tooltip.style.left = left + 'px';
    tooltip.style.top = top + 'px';
  }

  // ── Show tooltip ──
  function show(term, anchorEl) {
    hide(); // close any existing

    var entry = getEntry(term);
    if (!entry) return;

    var tooltip = document.createElement('div');
    tooltip.className = 'glossary-tooltip';
    tooltip.setAttribute('role', 'tooltip');

    // Term heading row
    var head = document.createElement('div');
    head.className = 'glossary-tooltip__term';
    var termSpan = document.createElement('span');
    termSpan.textContent = entry.term;
    head.appendChild(termSpan);

    // Category badge
    if (entry.category && categories[entry.category]) {
      var badge = document.createElement('span');
      badge.className = 'glossary-tooltip__cat';
      badge.textContent = categories[entry.category];
      head.appendChild(badge);
    }

    tooltip.appendChild(head);

    // Definition
    var def = document.createElement('div');
    def.className = 'glossary-tooltip__def';
    def.textContent = entry.definition;
    tooltip.appendChild(def);

    // Footer
    var foot = document.createElement('div');
    foot.className = 'glossary-tooltip__foot';

    // Learn more link
    var hideLink = anchorEl && anchorEl.dataset.termLink === 'false';
    if (entry.link && !hideLink) {
      var link = document.createElement('a');
      link.className = 'glossary-tooltip__link';
      link.href = entry.link;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.textContent = 'Learn more \u2192';
      foot.appendChild(link);
    } else {
      foot.appendChild(document.createElement('span')); // spacer
    }

    // Close button
    var closeBtn = document.createElement('button');
    closeBtn.className = 'glossary-tooltip__close';
    closeBtn.setAttribute('aria-label', 'Close');
    closeBtn.textContent = '\u2715';
    closeBtn.addEventListener('click', function (e) {
      e.stopPropagation();
      hide();
    });
    foot.appendChild(closeBtn);

    tooltip.appendChild(foot);

    // Position and show
    var rect = anchorEl
      ? anchorEl.getBoundingClientRect()
      : { left: window.innerWidth / 2, top: window.innerHeight / 2, width: 0, height: 0, bottom: window.innerHeight / 2, right: window.innerWidth / 2 };
    positionTooltip(tooltip, rect);

    // Animate in
    requestAnimationFrame(function () {
      tooltip.classList.add('visible');
    });

    activeTooltip = tooltip;
  }

  // ── Hide tooltip ──
  function hide() {
    if (activeTooltip) {
      activeTooltip.remove();
      activeTooltip = null;
    }
  }

  // ── Scan page for data-term elements ──
  function scanPage() {
    document.querySelectorAll('[data-term]').forEach(function (el) {
      if (el.dataset.glossaryBound) return;
      el.dataset.glossaryBound = '1';

      el.addEventListener('click', function (e) {
        e.preventDefault();
        e.stopPropagation();
        var term = el.dataset.term;
        if (activeTooltip) {
          hide();
          return;
        }
        show(term, el);
      });
    });
  }

  // ── Global listeners ──
  // Escape key closes tooltip
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && activeTooltip) {
      hide();
    }
  });

  // Click outside closes tooltip
  document.addEventListener('click', function (e) {
    if (activeTooltip && !activeTooltip.contains(e.target) && !e.target.closest('[data-term]')) {
      hide();
    }
  });

  // Re-position on scroll/resize
  window.addEventListener('scroll', hide, true);
  window.addEventListener('resize', hide);

  // ── MutationObserver for dynamically added terms ──
  var observer = new MutationObserver(function (mutations) {
    var needsScan = false;
    for (var i = 0; i < mutations.length; i++) {
      var added = mutations[i].addedNodes;
      for (var j = 0; j < added.length; j++) {
        var node = added[j];
        if (node.nodeType === 1 && (node.hasAttribute && node.hasAttribute('data-term') || node.querySelector && node.querySelector('[data-term]'))) {
          needsScan = true;
          break;
        }
      }
      if (needsScan) break;
    }
    if (needsScan) scanPage();
  });

  if (document.body) {
    observer.observe(document.body, { childList: true, subtree: true });
  } else {
    document.addEventListener('DOMContentLoaded', function () {
      observer.observe(document.body, { childList: true, subtree: true });
    });
  }

  // ── Public API ──
  window.glossary = {
    define: define,
    getEntry: getEntry,
    show: show,
    hide: hide,
    rescan: scanPage,
    get loaded() { return loaded; },
    get terms() { return terms; },
    get categories() { return categories; }
  };

  // ── Init ──
  loadGlossary();
})();
