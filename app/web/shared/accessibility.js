/**
 * HumanityOS Accessibility (a11y) Module
 *
 * Provides high-contrast mode, reduced motion, font scaling,
 * and colorblind mode filters. Persists preferences in localStorage
 * and auto-applies on page load.
 */
(function () {
  'use strict';

  var KEYS = {
    highContrast: 'a11y_high_contrast',
    reducedMotion: 'a11y_reduced_motion',
    fontScale: 'a11y_font_scale',
    colorblindMode: 'a11y_colorblind_mode'
  };

  function getBool(key) {
    return localStorage.getItem(key) === 'true';
  }

  function getFloat(key, fb) {
    var v = localStorage.getItem(key);
    if (v == null) return fb;
    var n = parseFloat(v);
    return isNaN(n) ? fb : n;
  }

  function getString(key, fb) {
    return localStorage.getItem(key) || fb;
  }

  /**
   * Inject SVG color-blindness filter definitions into the page.
   * Referenced by CSS filter: url(#protanopia), etc.
   */
  function ensureFilters() {
    if (document.getElementById('a11y-cb-filters')) return;
    var svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('id', 'a11y-cb-filters');
    svg.setAttribute('aria-hidden', 'true');
    svg.style.position = 'absolute';
    svg.style.width = '0';
    svg.style.height = '0';
    svg.style.overflow = 'hidden';
    svg.innerHTML = [
      '<defs>',
      '  <filter id="protanopia">',
      '    <feColorMatrix type="matrix" values="',
      '      0.567, 0.433, 0,     0, 0',
      '      0.558, 0.442, 0,     0, 0',
      '      0,     0.242, 0.758, 0, 0',
      '      0,     0,     0,     1, 0"/>',
      '  </filter>',
      '  <filter id="deuteranopia">',
      '    <feColorMatrix type="matrix" values="',
      '      0.625, 0.375, 0,     0, 0',
      '      0.7,   0.3,   0,     0, 0',
      '      0,     0.3,   0.7,   0, 0',
      '      0,     0,     0,     1, 0"/>',
      '  </filter>',
      '  <filter id="tritanopia">',
      '    <feColorMatrix type="matrix" values="',
      '      0.95, 0.05,  0,     0, 0',
      '      0,    0.433, 0.567, 0, 0',
      '      0,    0.475, 0.525, 0, 0',
      '      0,    0,     0,     1, 0"/>',
      '  </filter>',
      '</defs>'
    ].join('\n');
    document.body.appendChild(svg);
  }

  var a11y = {
    /**
     * Toggle high-contrast mode.
     * @param {boolean} on
     */
    setHighContrast: function (on) {
      on = !!on;
      localStorage.setItem(KEYS.highContrast, String(on));
      if (on) {
        document.body.setAttribute('data-high-contrast', '');
      } else {
        document.body.removeAttribute('data-high-contrast');
      }
    },

    /** @returns {boolean} */
    getHighContrast: function () {
      return getBool(KEYS.highContrast);
    },

    /**
     * Toggle reduced-motion mode.
     * @param {boolean} on
     */
    setReducedMotion: function (on) {
      on = !!on;
      localStorage.setItem(KEYS.reducedMotion, String(on));
      if (on) {
        document.body.setAttribute('data-reduced-motion', '');
      } else {
        document.body.removeAttribute('data-reduced-motion');
      }
    },

    /** @returns {boolean} */
    getReducedMotion: function () {
      return getBool(KEYS.reducedMotion);
    },

    /**
     * Set font scale factor (0.8 to 1.5).
     * @param {number} factor
     */
    setFontScale: function (factor) {
      factor = Math.max(0.8, Math.min(1.5, Number(factor) || 1));
      localStorage.setItem(KEYS.fontScale, String(factor));
      document.documentElement.style.setProperty('--a11y-font-scale', String(factor));
      document.documentElement.style.fontSize = (factor * 100) + '%';
    },

    /** @returns {number} */
    getFontScale: function () {
      return getFloat(KEYS.fontScale, 1);
    },

    /**
     * Set colorblind simulation filter.
     * @param {'none'|'protanopia'|'deuteranopia'|'tritanopia'} mode
     */
    setColorblindMode: function (mode) {
      var valid = ['none', 'protanopia', 'deuteranopia', 'tritanopia'];
      if (valid.indexOf(mode) === -1) mode = 'none';
      localStorage.setItem(KEYS.colorblindMode, mode);

      if (mode === 'none') {
        document.body.removeAttribute('data-colorblind');
      } else {
        ensureFilters();
        document.body.setAttribute('data-colorblind', mode);
      }
    },

    /** @returns {string} */
    getColorblindMode: function () {
      return getString(KEYS.colorblindMode, 'none');
    },

    /**
     * Apply all saved preferences. Called automatically on load.
     */
    apply: function () {
      this.setHighContrast(this.getHighContrast());
      this.setReducedMotion(this.getReducedMotion());
      this.setFontScale(this.getFontScale());
      this.setColorblindMode(this.getColorblindMode());

      // Respect OS prefers-reduced-motion on first visit
      if (localStorage.getItem(KEYS.reducedMotion) == null) {
        var mq = window.matchMedia('(prefers-reduced-motion: reduce)');
        if (mq.matches) this.setReducedMotion(true);
      }
    }
  };

  window.a11y = a11y;

  // Auto-apply when DOM is ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () { a11y.apply(); });
  } else {
    a11y.apply();
  }
})();
