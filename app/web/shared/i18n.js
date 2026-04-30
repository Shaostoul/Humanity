/**
 * HumanityOS Internationalization (i18n) Module
 *
 * Loads translation JSON files from /data/i18n/{lang}.json
 * Falls back to English for missing keys.
 *
 * Usage:
 *   await i18n.load('es');
 *   i18n.t('nav.home');  // "Inicio"
 */
(function () {
  'use strict';

  var STORAGE_KEY = 'humanity_language';
  var DEFAULT_LANG = 'en';

  var strings = {};
  var fallback = {};
  var currentLang = DEFAULT_LANG;

  function resolve(obj, key) {
    var parts = key.split('.');
    var cur = obj;
    for (var i = 0; i < parts.length; i++) {
      if (cur == null || typeof cur !== 'object') return undefined;
      cur = cur[parts[i]];
    }
    return cur;
  }

  async function fetchLang(code) {
    try {
      var resp = await fetch('/data/i18n/' + code + '.json');
      if (!resp.ok) return null;
      return await resp.json();
    } catch (_) {
      return null;
    }
  }

  var i18n = {
    /**
     * Load a language. Loads English fallback on first call.
     * @param {string} lang - Language code (e.g. 'en', 'es', 'fr', 'zh', 'ja')
     */
    async load(lang) {
      lang = lang || localStorage.getItem(STORAGE_KEY) || DEFAULT_LANG;

      // Always ensure English fallback is loaded
      if (Object.keys(fallback).length === 0) {
        var en = await fetchLang(DEFAULT_LANG);
        if (en) fallback = en;
      }

      if (lang === DEFAULT_LANG) {
        strings = fallback;
      } else {
        var data = await fetchLang(lang);
        strings = data || fallback;
      }

      currentLang = lang;
      localStorage.setItem(STORAGE_KEY, lang);

      // Set dir attribute for RTL languages
      var dir = (strings.meta && strings.meta.direction) || 'ltr';
      document.documentElement.setAttribute('dir', dir);
      document.documentElement.setAttribute('lang', lang);

      // Dispatch event so components can react
      window.dispatchEvent(new CustomEvent('languagechange', { detail: { lang: lang } }));
    },

    /**
     * Translate a key. Returns the translated string or the key itself if not found.
     * @param {string} key - Dot-separated key, e.g. 'nav.home'
     * @param {Object} [vars] - Optional interpolation variables (placeholder: {{name}})
     * @returns {string}
     */
    t(key, vars) {
      var val = resolve(strings, key);
      if (val === undefined) val = resolve(fallback, key);
      if (val === undefined) return key;
      if (typeof val !== 'string') return key;

      // Simple interpolation: {{varName}}
      if (vars && typeof vars === 'object') {
        val = val.replace(/\{\{(\w+)\}\}/g, function (_, name) {
          return vars[name] !== undefined ? vars[name] : '{{' + name + '}}';
        });
      }
      return val;
    },

    /**
     * Change the active language. Reloads strings.
     * @param {string} lang
     */
    async setLanguage(lang) {
      await this.load(lang);
    },

    /**
     * Get the current language code.
     * @returns {string}
     */
    getLanguage() {
      return currentLang;
    },

    /**
     * Get the stored language preference.
     * @returns {string}
     */
    getStoredLanguage() {
      return localStorage.getItem(STORAGE_KEY) || DEFAULT_LANG;
    }
  };

  window.i18n = i18n;
})();
