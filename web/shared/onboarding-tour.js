/**
 * HumanityOS Onboarding Tour, guided first-time user walkthrough.
 * Self-contained IIFE, no external dependencies beyond DOM.
 * Auto-starts for new users; callable via window.startOnboardingTour().
 */
(function () {
  if (window.__HOS_TOUR_INIT__) return;
  window.__HOS_TOUR_INIT__ = true;

  // Steps live in data/onboarding/tour.json (infinite-of-x); the literal below
  // is only the offline fallback shown if the fetch fails.
  var TOUR_STEPS = [
    { target: null, title: 'Welcome to HumanityOS!', text: 'A cooperative platform to end poverty and unite humanity.' }
  ];
  var stepsLoaded = fetch('/data/onboarding/tour.json')
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (d) {
      if (d && Array.isArray(d.steps) && d.steps.length) TOUR_STEPS = d.steps;
    })
    .catch(function () { /* offline: keep the fallback welcome */ });

  var currentStep = 0;
  var overlay = null;
  var popover = null;
  var styleEl = null;

  // ── Inject CSS ──
  function injectStyles() {
    if (styleEl) return;
    styleEl = document.createElement('style');
    styleEl.textContent =
      '#hos-tour-overlay {' +
        'position:fixed;inset:0;z-index:10000;' +
        'pointer-events:none;transition:opacity 0.25s;' +
      '}' +
      '#hos-tour-overlay.active { pointer-events:auto; }' +
      // On a step with no highlight target the highlight (and its 9999px
      // dimming shadow) is switched off, which left the overlay fully
      // transparent AND click-eating: the page looked normal and simply stopped
      // responding. Dim it so the modal state is always visible to the user.
      '#hos-tour-overlay.no-target { background:rgba(0,0,0,0.65); }' +

      '#hos-tour-highlight {' +
        'position:fixed;z-index:10001;' +
        'box-shadow:0 0 0 9999px rgba(0,0,0,0.65);' +
        'border-radius:var(--radius, 6px);' +
        'pointer-events:none;transition:all 0.3s ease;' +
      '}' +

      '#hos-tour-popover {' +
        'position:fixed;z-index:10002;' +
        'background:var(--bg-card, #1a1a2e);' +
        'border:1px solid var(--border, #333);' +
        'border-radius:var(--radius-lg, 10px);' +
        'padding:20px;max-width:340px;width:90vw;' +
        'color:var(--text, #e0e0e0);' +
        'box-shadow:0 8px 32px rgba(0,0,0,0.4);' +
        'transition:opacity 0.25s, transform 0.25s;' +
        'font-family:inherit;' +
      '}' +
      '#hos-tour-popover.entering {' +
        'opacity:0;transform:translateY(8px);' +
      '}' +

      '#hos-tour-popover .tour-title {' +
        'font-size:1rem;font-weight:700;margin-bottom:8px;' +
        'color:var(--text, #fff);' +
      '}' +
      '#hos-tour-popover .tour-text {' +
        'font-size:0.85rem;line-height:1.5;' +
        'color:var(--text-muted, #aaa);margin-bottom:16px;' +
      '}' +
      '#hos-tour-popover .tour-footer {' +
        'display:flex;align-items:center;justify-content:space-between;gap:8px;' +
      '}' +
      '#hos-tour-popover .tour-counter {' +
        'font-size:0.72rem;color:var(--text-muted, #888);' +
      '}' +
      '#hos-tour-popover .tour-btns {' +
        'display:flex;gap:8px;align-items:center;' +
      '}' +
      '#hos-tour-popover .tour-btn {' +
        'padding:6px 16px;border-radius:var(--radius, 6px);' +
        'font-size:0.8rem;cursor:pointer;border:none;font-family:inherit;' +
      '}' +
      '#hos-tour-popover .tour-btn-primary {' +
        'background:var(--accent, #6c5ce7);color:#fff;' +
      '}' +
      '#hos-tour-popover .tour-btn-primary:hover { filter:brightness(1.15); }' +
      '#hos-tour-popover .tour-btn-secondary {' +
        'background:transparent;color:var(--text-muted, #aaa);' +
        'border:1px solid var(--border, #444);' +
      '}' +
      '#hos-tour-popover .tour-btn-secondary:hover { color:var(--text, #fff); }' +
      '#hos-tour-popover .tour-skip {' +
        'background:none;border:none;color:var(--text-muted, #888);' +
        'font-size:0.72rem;cursor:pointer;text-decoration:underline;font-family:inherit;' +
      '}' +
      '#hos-tour-popover .tour-skip:hover { color:var(--text, #fff); }' +

      '#hos-tour-arrow {' +
        'position:absolute;width:12px;height:12px;' +
        'background:var(--bg-card, #1a1a2e);' +
        'border:1px solid var(--border, #333);' +
        'transform:rotate(45deg);' +
      '}' +

      '@media (max-width:600px) {' +
        '#hos-tour-popover { max-width:90vw;padding:16px; }' +
      '}';
    document.head.appendChild(styleEl);
  }

  // ── Build DOM ──
  function createElements() {
    overlay = document.createElement('div');
    overlay.id = 'hos-tour-overlay';
    overlay.innerHTML = '<div id="hos-tour-highlight"></div>';
    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) endTour();
    });
    document.body.appendChild(overlay);

    popover = document.createElement('div');
    popover.id = 'hos-tour-popover';
    popover.setAttribute('role', 'dialog');
    popover.setAttribute('aria-label', 'Onboarding tour');
    document.body.appendChild(popover);
  }

  // ── Positioning ──
  function positionPopover(targetEl) {
    var highlight = document.getElementById('hos-tour-highlight');

    if (!targetEl) {
      // Centered modal (no target element)
      highlight.style.display = 'none';
      if (overlay) overlay.classList.add('no-target');
      popover.style.left = '50%';
      popover.style.top = '50%';
      popover.style.transform = 'translate(-50%, -50%)';
      // Remove any arrow
      var existingArrow = popover.querySelector('#hos-tour-arrow');
      if (existingArrow) existingArrow.remove();
      return;
    }

    var rect = targetEl.getBoundingClientRect();
    var pad = 6;

    // Position highlight around target
    if (overlay) overlay.classList.remove('no-target');
    highlight.style.display = 'block';
    highlight.style.left = (rect.left - pad) + 'px';
    highlight.style.top = (rect.top - pad) + 'px';
    highlight.style.width = (rect.width + pad * 2) + 'px';
    highlight.style.height = (rect.height + pad * 2) + 'px';

    // Reset transform before measuring
    popover.style.transform = 'none';
    popover.style.left = '0';
    popover.style.top = '0';

    var popRect = popover.getBoundingClientRect();
    var vw = window.innerWidth;
    var vh = window.innerHeight;

    // Decide placement: prefer below, then above, then right
    var placement = 'below';
    var left, top;
    var arrowLeft, arrowTop;

    // Try below
    top = rect.bottom + pad + 12;
    left = rect.left + rect.width / 2 - popRect.width / 2;
    if (top + popRect.height > vh - 10) {
      // Try above
      top = rect.top - pad - 12 - popRect.height;
      placement = 'above';
      if (top < 10) {
        // Try right
        left = rect.right + pad + 12;
        top = rect.top + rect.height / 2 - popRect.height / 2;
        placement = 'right';
      }
    }

    // Clamp horizontal
    if (left < 10) left = 10;
    if (left + popRect.width > vw - 10) left = vw - 10 - popRect.width;
    // Clamp vertical
    if (top < 10) top = 10;
    if (top + popRect.height > vh - 10) top = vh - 10 - popRect.height;

    popover.style.left = left + 'px';
    popover.style.top = top + 'px';

    // Arrow
    var existingArrow = popover.querySelector('#hos-tour-arrow');
    if (existingArrow) existingArrow.remove();

    var arrow = document.createElement('div');
    arrow.id = 'hos-tour-arrow';
    if (placement === 'below') {
      arrow.style.top = '-7px';
      arrow.style.left = Math.min(Math.max(rect.left + rect.width / 2 - left - 6, 12), popRect.width - 24) + 'px';
      arrow.style.borderRight = 'none';
      arrow.style.borderBottom = 'none';
    } else if (placement === 'above') {
      arrow.style.bottom = '-7px';
      arrow.style.left = Math.min(Math.max(rect.left + rect.width / 2 - left - 6, 12), popRect.width - 24) + 'px';
      arrow.style.borderLeft = 'none';
      arrow.style.borderTop = 'none';
    } else if (placement === 'right') {
      arrow.style.left = '-7px';
      arrow.style.top = Math.min(Math.max(rect.top + rect.height / 2 - top - 6, 12), popRect.height - 24) + 'px';
      arrow.style.borderTop = 'none';
      arrow.style.borderRight = 'none';
    }
    popover.appendChild(arrow);
  }

  // ── Render step ──
  function renderStep() {
    var step = TOUR_STEPS[currentStep];
    if (!step) return;

    var isFirst = currentStep === 0;
    var isLast = currentStep === TOUR_STEPS.length - 1;

    var html = '<div class="tour-title">' + step.title + '</div>';
    html += '<div class="tour-text">' + step.text + '</div>';
    html += '<div class="tour-footer">';
    html += '<span class="tour-counter">' + (currentStep + 1) + ' of ' + TOUR_STEPS.length + '</span>';
    html += '<div class="tour-btns">';
    if (!isFirst && !isLast) {
      html += '<button class="tour-btn tour-btn-secondary" id="tour-prev">Previous</button>';
    }
    if (!isLast) {
      html += '<button class="tour-skip" id="tour-skip">Skip Tour</button>';
      html += '<button class="tour-btn tour-btn-primary" id="tour-next">Next</button>';
    } else {
      html += '<button class="tour-btn tour-btn-primary" id="tour-finish">Get Started</button>';
    }
    html += '</div></div>';

    popover.classList.add('entering');
    popover.innerHTML = html;

    // Find target element
    var targetEl = null;
    if (step.target) {
      targetEl = document.querySelector('.hub-nav ' + step.target) || document.querySelector(step.target);
    }

    positionPopover(targetEl);

    // Remove entering class after frame to trigger transition
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        popover.classList.remove('entering');
      });
    });

    // Bind buttons
    var nextBtn = document.getElementById('tour-next');
    var prevBtn = document.getElementById('tour-prev');
    var skipBtn = document.getElementById('tour-skip');
    var finishBtn = document.getElementById('tour-finish');

    if (nextBtn) nextBtn.addEventListener('click', nextStep);
    if (prevBtn) prevBtn.addEventListener('click', prevStep);
    if (skipBtn) skipBtn.addEventListener('click', endTour);
    if (finishBtn) finishBtn.addEventListener('click', endTour);
  }

  function nextStep() {
    // Skip steps whose target doesn't exist
    var next = currentStep + 1;
    while (next < TOUR_STEPS.length - 1 && TOUR_STEPS[next].target) {
      var el = document.querySelector('.hub-nav ' + TOUR_STEPS[next].target) || document.querySelector(TOUR_STEPS[next].target);
      if (el) break;
      next++;
    }
    currentStep = next;
    if (currentStep >= TOUR_STEPS.length) {
      endTour();
    } else {
      renderStep();
    }
  }

  function prevStep() {
    var prev = currentStep - 1;
    while (prev > 0 && TOUR_STEPS[prev].target) {
      var el = document.querySelector('.hub-nav ' + TOUR_STEPS[prev].target) || document.querySelector(TOUR_STEPS[prev].target);
      if (el) break;
      prev--;
    }
    if (prev < 0) prev = 0;
    currentStep = prev;
    renderStep();
  }

  function endTour() {
    localStorage.setItem('hos_tour_completed', String(Date.now()));
    if (overlay && overlay.parentNode) overlay.parentNode.removeChild(overlay);
    if (popover && popover.parentNode) popover.parentNode.removeChild(popover);
    overlay = null;
    popover = null;
  }

  // ── Keyboard navigation ──
  function onKeyDown(e) {
    if (!overlay) return;
    if (e.key === 'Escape') { endTour(); return; }
    if (e.key === 'ArrowRight' || e.key === 'Enter') { nextStep(); return; }
    if (e.key === 'ArrowLeft') { prevStep(); return; }
  }

  // ── Public API ──
  function startTour() {
    // Wait for the data-driven steps (resolves immediately once loaded; falls
    // back to the minimal welcome if the fetch failed).
    stepsLoaded.then(startTourNow);
  }

  function startTourNow() {
    if (overlay) return; // already running
    currentStep = 0;
    injectStyles();
    createElements();
    overlay.classList.add('active');
    document.addEventListener('keydown', onKeyDown);
    renderStep();

    // Reposition on resize
    window.addEventListener('resize', function onResize() {
      if (!overlay) {
        window.removeEventListener('resize', onResize);
        return;
      }
      renderStep();
    });
  }

  window.startOnboardingTour = startTour;

  // ── Auto-start for first-time users ──
  // Never on the landing page. The overlay is a full-viewport click catcher, so
  // auto-firing it two seconds after load meant a first-time visitor's tap on
  // "Get the free app" hit the overlay instead of the button, and only
  // dismissed the tour. The front door has to answer "what is this" before it
  // offers a guided tour, so there the tour stays opt-in via the "Take Tour"
  // footer link that shell.js already injects.
  var onLandingPage = location.pathname.replace(/\/index\.html$/, '/') === '/';
  if (!onLandingPage && !localStorage.getItem('hos_tour_completed')) {
    setTimeout(function () {
      // Only auto-start if still no completion flag (user might have set it elsewhere)
      if (!localStorage.getItem('hos_tour_completed')) {
        startTour();
      }
    }, 2000);
  }
})();
