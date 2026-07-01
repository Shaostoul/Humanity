/**
 * HumanityOS Laws logic (pure functions, no DOM).
 *
 * Web mirror of the native jurisdiction-chain + filter logic in
 * src/gui/laws.rs (v0.496) and src/gui/pages/laws.rs. Keep the semantics
 * byte-for-byte compatible with the Rust side:
 *
 *   - pathToRoot walks location -> parent -> ... -> root (Humanity),
 *     guards against cycles (max 32 hops), unknown id yields [].
 *   - applicableRules returns every rule attached to a jurisdiction in the
 *     path, ordered BROADEST first (Humanity) down to the most local, and
 *     preserves the data file's rule order within each jurisdiction.
 *   - locationBreadcrumb is most-local first, joined with ", ".
 *   - Filters: kind tab (0=All, 1=base, 2=real), one category ("" = all),
 *     free-text search over "title summary category tags" lowercased.
 *
 * Loaded by web/pages/laws.html in the browser (window.hosLawsLogic) and by
 * node test scripts via require() so the logic is verifiable headlessly.
 */
(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.hosLawsLogic = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  /**
   * Chain of jurisdiction ids from `location` up to the root (Humanity),
   * e.g. ["silverdale","kitsap","wa","usa","earth","humanity"].
   * Mirrors Laws::path_to_root: cycle guard at 32 hops, unknown id -> [].
   */
  function pathToRoot(jurisdictions, location) {
    var chain = [];
    var cur = String(location == null ? '' : location);
    for (var i = 0; i < 32; i++) {
      var j = findJurisdiction(jurisdictions, cur);
      if (!j) break;
      chain.push(j.id);
      if (j.parent != null && j.parent !== '') cur = j.parent;
      else break;
    }
    return chain;
  }

  function findJurisdiction(jurisdictions, id) {
    var list = jurisdictions || [];
    for (var i = 0; i < list.length; i++) {
      if (list[i] && list[i].id === id) return list[i];
    }
    return null;
  }

  /**
   * All rules that apply at `location`: every rule attached to a jurisdiction
   * in its path to the root, ordered BROADEST (Humanity) first down to the
   * most local. Mirrors Laws::applicable_rules exactly, including preserving
   * the data file's rule order inside each jurisdiction.
   */
  function applicableRules(data, location) {
    var path = pathToRoot(data.jurisdictions || [], location);
    path.reverse(); // root (Humanity) first
    var out = [];
    var rules = data.rules || [];
    for (var p = 0; p < path.length; p++) {
      for (var r = 0; r < rules.length; r++) {
        if (rules[r] && rules[r].jurisdiction === path[p]) out.push(rules[r]);
      }
    }
    return out;
  }

  /** Display name for a jurisdiction id; falls back to the id itself. */
  function jurisdictionName(jurisdictions, id) {
    var j = findJurisdiction(jurisdictions, id);
    return j ? j.name : String(id);
  }

  /**
   * Readable breadcrumb, most local first, e.g.
   * "Silverdale, Kitsap County, Washington, United States, Earth, Humanity".
   */
  function locationBreadcrumb(data, location) {
    var ids = pathToRoot(data.jurisdictions || [], location);
    var names = [];
    for (var i = 0; i < ids.length; i++) {
      names.push(jurisdictionName(data.jurisdictions || [], ids[i]));
    }
    return names.join(', ');
  }

  /** "base" (case-insensitive) = the HumanityOS base set. Mirrors Rule::is_base. */
  function isBase(rule) {
    return String(rule && rule.kind || '').toLowerCase() === 'base';
  }

  /**
   * One rule against the page filters. Mirrors the native draw loop:
   *   kindTab: 0 = All, 1 = HumanityOS base only, 2 = Real laws only.
   *   category: exact match against rule.category; "" = all categories.
   *   search: lowercased substring over "title summary category tags".
   */
  function matchesFilters(rule, opts) {
    var o = opts || {};
    var kindTab = o.kindTab | 0;
    if (kindTab === 1 && !isBase(rule)) return false;
    if (kindTab === 2 && isBase(rule)) return false;
    var category = o.category || '';
    if (category !== '' && (rule.category || '') !== category) return false;
    var q = String(o.search || '').trim().toLowerCase();
    if (q !== '') {
      var hay = [
        rule.title || '',
        rule.summary || '',
        rule.category || '',
        (rule.tags || []).join(' ')
      ].join(' ').toLowerCase();
      if (hay.indexOf(q) === -1) return false;
    }
    return true;
  }

  /** Convenience: applicableRules + matchesFilters in one call. */
  function filterRules(data, location, opts) {
    var rules = applicableRules(data, location);
    var out = [];
    for (var i = 0; i < rules.length; i++) {
      if (matchesFilters(rules[i], opts)) out.push(rules[i]);
    }
    return out;
  }

  /**
   * Default selected jurisdiction: the LAST entry in the data file (most
   * local, e.g. Silverdale). Mirrors the native page's jurisdictions.last().
   */
  function defaultLocation(data) {
    var list = (data && data.jurisdictions) || [];
    return list.length ? list[list.length - 1].id : '';
  }

  return {
    pathToRoot: pathToRoot,
    findJurisdiction: findJurisdiction,
    applicableRules: applicableRules,
    jurisdictionName: jurisdictionName,
    locationBreadcrumb: locationBreadcrumb,
    isBase: isBase,
    matchesFilters: matchesFilters,
    filterRules: filterRules,
    defaultLocation: defaultLocation
  };
}));
