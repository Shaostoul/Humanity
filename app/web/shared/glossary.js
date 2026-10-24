/**
 * HumanityOS Glossary, Inline term definitions
 *
 * Loads definitions from /data/glossary.json.
 * Auto-scans the page for elements with data-term="word" and adds click handlers.
 * On click, shows a small dark-themed modal/tooltip near the word with the definition.
 *
 * API:
 *   glossary.define(term), returns definition string or null
 *   glossary.show(term, el), manually show tooltip near element
 *   glossary.hide(), close any open tooltip
 *   glossary.rescan(), re-scan DOM for new data-term elements
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
    '[data-term]:focus-visible{outline:2px solid var(--border-focus,var(--accent,#ed8c24));outline-offset:2px;border-radius:2px;}' +
    '[data-term]{cursor:help;border-bottom:1px dotted var(--accent,#ed8c24);' +
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
    '.glossary-tooltip__term{font-weight:700;font-size:0.95rem;color:var(--accent,#ed8c24);' +
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
    '.glossary-tooltip__link{font-size:0.78rem;color:var(--accent,#ed8c24);' +
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
    // A card that can hold a link and a close button: a small dialog named by its
    // word (the word that opens it says aria-haspopup=dialog).
    tooltip.setAttribute('role', 'dialog');
    tooltip.setAttribute('aria-label', entry.term);

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
      link.textContent = words('glossary.learn_more') + ' \u2192';
      link.setAttribute('data-i18n-tip', 'glossary.learn_more_hint');
      link.setAttribute('data-tip-desc', words('glossary.learn_more_hint'));
      foot.appendChild(link);
    } else {
      foot.appendChild(document.createElement('span')); // spacer
    }

    // Close button
    var closeBtn = document.createElement('button');
    closeBtn.className = 'glossary-tooltip__close';
    closeBtn.setAttribute('aria-label', words('common.close'));
    closeBtn.setAttribute('data-i18n-tip', 'glossary.close_hint');
    closeBtn.setAttribute('data-tip-desc', words('glossary.close_hint'));
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

  // The card's own words, by key (data/i18n/; shell.js loads i18n.js on every page).
  function words(key) {
    return window.i18n && typeof window.i18n.t === 'function' ? window.i18n.t(key) : key;
  }

  // ── Hide tooltip ──
  function hide() {
    pinned = false;
    if (activeTooltip) {
      activeTooltip.remove();
      activeTooltip = null;
    }
  }

  // ── Scan page for data-term elements ──
  // A word marked as a glossary term (2026-10-23, "the glossary on every page"):
  // resting the pointer on it shows its card, as on the desktop app; a click or
  // Enter pins the card open (a second one closes it); Escape closes it. It is
  // reachable with Tab and read as a button, so a keyboard or screen-reader
  // user gets the same definition. The shell's own hover hint stays out of its
  // way (data-no-tip): the card is its hint.
  var hoverTimer = null;
  var pinned = false;
  function scanPage() {
    document.querySelectorAll('[data-term]').forEach(function (el) {
      // The definition is read out with the word. Set on every scan, not only the first,
      // because a word can be bound before /data/glossary.json has arrived.
      var entry = getEntry(el.dataset.term);
      if (entry && !el.hasAttribute('aria-description')) el.setAttribute('aria-description', entry.definition);
      if (el.dataset.glossaryBound) return;
      el.dataset.glossaryBound = '1';
      if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '0');
      if (!el.hasAttribute('role')) el.setAttribute('role', 'button');
      el.setAttribute('aria-haspopup', 'dialog');
      el.setAttribute('data-no-tip', '');

      function toggle() {
        if (activeTooltip && pinned && activeTooltip.__for === el) { hide(); return; }
        show(el.dataset.term, el);
        pinned = true;
        if (activeTooltip) activeTooltip.__for = el;
      }
      el.addEventListener('click', function (e) {
        e.preventDefault();
        e.stopPropagation();
        toggle();
      });
      el.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggle(); }
      });
      el.addEventListener('mouseenter', function () {
        if (pinned) return;
        clearTimeout(hoverTimer);
        hoverTimer = setTimeout(function () {
          show(el.dataset.term, el);
          if (activeTooltip) activeTooltip.__for = el;
        }, 350);
      });
      el.addEventListener('mouseleave', function () {
        clearTimeout(hoverTimer);
        if (!pinned) hide();
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
