/**
 * HumanityOS words, by key, from /data/i18n/{lang}.json (the ELI5 mandate,
 * docs/PRIORITIES.md TIER 3; operator 2026-10-22: English now, every other
 * language ready). The web twin of src/gui/i18n.rs: the same files, the same
 * keys, the same {{name}} placeholders and the same [[term]] glossary marks.
 *
 * ENGLISH IS HERE AT ONCE. web/shared/i18n-en.js (generated from
 * data/i18n/en.json by `node scripts/gen-i18n.js`) sets window.HOS_I18N_EN; a
 * page that loads it and this file before its own script can call i18n.t()
 * while it builds. Another language is fetched, and `languagechange` fires
 * when it arrives so a page can redraw. Any key a language lacks shows the
 * English words; a key English lacks shows the key (scripts/tests/i18n-web.test.js
 * fails the build on any key written in a page that en.json does not have).
 *
 * IN HTML (a static page keeps its English in the markup, so it reads with or
 * without script; scripts/i18n-web.js --html-write keeps it equal to en.json
 * and the test fails when the two differ):
 *   <h1 data-i18n="calculator.title">Calculator</h1>
 *   <button data-i18n="calculator.clear" data-i18n-tip="calculator.clear_hint"
 *           data-tip-desc="Empties the display so you can start again.">C</button>
 *   <input data-i18n-placeholder="notes.search" placeholder="Search notes...">
 * Attributes: data-i18n (the text, glossary marks become [data-term] spans),
 * data-i18n-title, data-i18n-placeholder, data-i18n-aria-label, data-i18n-alt,
 * data-i18n-tip (the plain-language hint: data-tip-desc), data-i18n-tip-title.
 *
 * IN SCRIPT:
 *   i18n.t('calculator.title')                     // "Calculator"
 *   i18n.t('market.listing_by', { name: seller })  // "Listed by {{name}}"
 *   i18n.tn('inventory.items', 3)                  // key.one / key.other, {{n}} = 3
 *   i18n.hint(button, 'calculator.clear_hint')     // the hover hint, by key
 *   i18n.fill(el, 'notes.eli5_simple')             // words with [[term]] marks as [data-term] spans
 *   i18n.html('market.contact_note')               // the same as escaped HTML, for string-built markup
 *   i18n.apply(container)                          // translate [data-i18n*] inside
 */
(function () {
  'use strict';

  var STORAGE_KEY = 'humanity_language';
  var DEFAULT_LANG = 'en';

  var fallback = {};
  var strings = {};
  var currentLang = DEFAULT_LANG;

  function resolve(obj, key) {
    var parts = String(key).split('.');
    var cur = obj;
    for (var i = 0; i < parts.length; i++) {
      if (cur == null || typeof cur !== 'object') return undefined;
      cur = cur[parts[i]];
    }
    return cur;
  }

  /** Split a marked string into text and glossary terms:
   *  "a [[relays|relay]] b" -> [{text:'a '}, {shown:'relays', term:'relay'}, {text:' b'}]. */
  function segments(marked) {
    var out = [];
    var rest = String(marked);
    for (;;) {
      var open = rest.indexOf('[[');
      if (open < 0) break;
      var close = rest.indexOf(']]', open + 2);
      if (close < 0) break;
      var inner = rest.slice(open + 2, close);
      if (open > 0) out.push({ text: rest.slice(0, open) });
      var bar = inner.indexOf('|');
      if (bar >= 0) out.push({ shown: inner.slice(0, bar), term: inner.slice(bar + 1) });
      else out.push({ shown: inner, term: inner });
      rest = rest.slice(close + 2);
    }
    if (rest) out.push({ text: rest });
    return out;
  }

  /** The words alone, glossary marks taken out. */
  function plain(marked) {
    return segments(marked).map(function (s) { return s.text !== undefined ? s.text : s.shown; }).join('');
  }

  function fillVars(str, vars) {
    if (!vars || typeof vars !== 'object') return str;
    return str.replace(/\{\{(\w+)\}\}/g, function (_, name) {
      return vars[name] !== undefined ? String(vars[name]) : '{{' + name + '}}';
    });
  }

  /** The marked string for a key: the language, then English, then undefined. */
  function lookup(key) {
    var val = resolve(strings, key);
    if (typeof val !== 'string') val = resolve(fallback, key);
    return typeof val === 'string' ? val : undefined;
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

  function storedLanguage() {
    try { return localStorage.getItem(STORAGE_KEY) || DEFAULT_LANG; } catch (_) { return DEFAULT_LANG; }
  }

  // ── Translating the page ────────────────────────────────────────────

  var ATTRS = [
    ['data-i18n-title', 'title'],
    ['data-i18n-placeholder', 'placeholder'],
    ['data-i18n-aria-label', 'aria-label'],
    ['data-i18n-alt', 'alt'],
    ['data-i18n-tip', 'data-tip-desc'],
    ['data-i18n-tip-title', 'data-tip-title'],
    ['data-i18n-content', 'content']
  ];

  /** The segments of a marked string with {{name}} values put into its plain
   *  text only, so a value (a listing title, a person's name) can never be read
   *  as a glossary mark. */
  function segmentsWith(marked, vars) {
    return segments(marked).map(function (s) {
      return s.text !== undefined ? { text: fillVars(s.text, vars) } : s;
    });
  }

  function escHtml(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  /** Marked words as HTML for a page that builds with strings: every word
   *  escaped, each glossary mark a [data-term] span. */
  function markedHtml(marked, vars) {
    return segmentsWith(marked, vars).map(function (s) {
      return s.text !== undefined
        ? escHtml(s.text)
        : '<span data-term="' + escHtml(s.term) + '">' + escHtml(s.shown) + '</span>';
    }).join('');
  }

  /** Fill an element with marked words: text nodes, and a [data-term] span for
   *  each glossary mark (glossary.js turns those into the definition card).
   *  Built with text nodes only, never innerHTML, so a translation can never
   *  carry markup into the page. */
  function fillMarked(el, marked, vars) {
    var doc = el.ownerDocument || document;
    while (el.firstChild) el.removeChild(el.firstChild);
    segmentsWith(marked, vars).forEach(function (s) {
      if (s.text !== undefined) {
        el.appendChild(doc.createTextNode(s.text));
      } else {
        var span = doc.createElement('span');
        span.setAttribute('data-term', s.term);
        span.textContent = s.shown;
        el.appendChild(span);
      }
    });
  }

  function each(root, sel, fn) {
    if (!root || !root.querySelectorAll) return;
    if (root.matches && root.matches(sel)) fn(root);
    var list = root.querySelectorAll(sel);
    for (var i = 0; i < list.length; i++) fn(list[i]);
  }

  /** Translate every [data-i18n*] element in `root` (the whole document when
   *  omitted). Safe to call again: it only sets what the key says. */
  function apply(root) {
    root = root || (typeof document !== 'undefined' ? document : null);
    if (!root) return;
    each(root, '[data-i18n]', function (el) {
      var m = lookup(el.getAttribute('data-i18n'));
      if (m !== undefined) fillMarked(el, m);
    });
    ATTRS.forEach(function (pair) {
      each(root, '[' + pair[0] + ']', function (el) {
        var m = lookup(el.getAttribute(pair[0]));
        if (m === undefined) return;
        var target = pair[1];
        // shell.js moves a title into data-native-title so the browser shows one
        // hint, not two; follow it there.
        if (target === 'title' && el.hasAttribute('data-native-title') && !el.hasAttribute('title')) {
          target = 'data-native-title';
        }
        el.setAttribute(target, plain(m));
      });
    });
  }

  var i18n = {
    /**
     * Load a language (English is already here when i18n-en.js loaded first).
     * @param {string} lang - Language code (e.g. 'en', 'es', 'fr', 'zh', 'ja')
     */
    async load(lang) {
      lang = lang || storedLanguage();

      if (Object.keys(fallback).length === 0) {
        var en = (typeof window !== 'undefined' && window.HOS_I18N_EN) || await fetchLang(DEFAULT_LANG);
        if (en) fallback = en;
      }

      if (lang === DEFAULT_LANG) {
        strings = fallback;
      } else {
        var data = await fetchLang(lang);
        strings = data || fallback;
        if (!data) lang = DEFAULT_LANG;
      }

      currentLang = lang;
      try { localStorage.setItem(STORAGE_KEY, lang); } catch (_) { /* private window */ }

      if (typeof document !== 'undefined') {
        var dir = (strings.meta && strings.meta.direction) || 'ltr';
        document.documentElement.setAttribute('dir', dir);
        document.documentElement.setAttribute('lang', lang);
        // English is already in the markup, its glossary marks as [data-term]
        // spans (scripts/i18n-web.js --html-write); another language replaces it.
        if (lang !== DEFAULT_LANG) apply(document);
        window.dispatchEvent(new CustomEvent('languagechange', { detail: { lang: lang } }));
      }
    },

    /**
     * The words for a key, glossary marks taken out. The language, then
     * English, then the key itself (never blank).
     * @param {string} key - e.g. 'nav.home'
     * @param {Object} [vars] - values for {{name}} placeholders
     */
    t(key, vars) {
      var m = lookup(key);
      if (m === undefined) return key;
      return fillVars(plain(m), vars);
    },

    /** The words for a key with their [[term]] marks kept (for fillMarked). */
    tMarked(key) {
      var m = lookup(key);
      return m === undefined ? key : m;
    },

    /** A count: key.one when n is 1, key.other otherwise, {{n}} set to n. */
    tn(key, n, vars) {
      var all = { n: n };
      if (vars) for (var k in vars) all[k] = vars[k];
      return this.t(key + (n === 1 ? '.one' : '.other'), all);
    },

    /** True when English (and so every language) has the key. */
    has(key) {
      return typeof resolve(fallback, key) === 'string';
    },

    /** Give an element its plain-language hover hint by key (shell.js shows
     *  data-tip-desc; the hint modes in Settings decide how). */
    hint(el, key) {
      if (!el || !el.setAttribute) return el;
      el.setAttribute('data-i18n-tip', key);
      el.setAttribute('data-tip-desc', this.t(key));
      return el;
    },

    /** Fill an element with a key's words, glossary marks as [data-term] spans. */
    fill(el, key, vars) {
      if (!el) return el;
      var m = lookup(key);
      fillMarked(el, m === undefined ? key : m, vars);
      return el;
    },

    /** A key's words as HTML, for a page that builds its markup as a string:
     *  every word escaped, each glossary mark a [data-term] span.
     *    el.innerHTML = '<div class="note">' + i18n.html('market.contact_note') + '</div>'; */
    html(key, vars) {
      var m = lookup(key);
      return markedHtml(m === undefined ? key : m, vars);
    },

    apply: apply,
    plain: plain,
    segments: segments,

    async setLanguage(lang) {
      await this.load(lang);
    },

    getLanguage() {
      return currentLang;
    },

    getStoredLanguage() {
      return storedLanguage();
    }
  };

  // English at once, when the generated table loaded first.
  if (typeof window !== 'undefined' && window.HOS_I18N_EN) {
    fallback = window.HOS_I18N_EN;
    strings = fallback;
  }

  if (typeof window !== 'undefined') window.i18n = i18n;
  if (typeof module !== 'undefined' && module.exports) module.exports = i18n;
})();
