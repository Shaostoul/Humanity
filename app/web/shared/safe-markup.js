// ── safe-markup.js ────────────────────────────────────────────────────────
// Markup without inline script (2026-11-20, the website's script policy, docs/PRIORITIES.md
// item 6b): event handlers written in the markup (data-on-*, below), hosOn.attr for handlers in
// markup a script builds, hosArg for putting data into one, and hosEsc, THE house escape for
// putting text into markup.
//
// hosEsc(s): text made safe for HTML, between tags or inside a quoted attribute (& < > " ' all
// escaped). Every value that did not come from this code (a name, a message, anything a person
// or a server typed) goes through it before it reaches innerHTML, outerHTML or
// insertAdjacentHTML; better still, set textContent. scripts/tests/inner-html-web.test.js counts
// the places that do not, per file, and the count only goes down. (A page's own esc/escapeHtml/
// escHtml helpers count as the same thing when they escape all five characters; that test checks
// each one does.)
//
// WHY. The site's Content-Security-Policy no longer allows 'unsafe-inline' in script-src, so a
// browser refuses every onclick="..." attribute and every <script> block written in a page. The
// pages still want to say, in the markup, what a button does. So a page writes
//
//     <button data-on-click="showSection('profile')">                       (static HTML)
//     '<button ' + hosOn.attr('click', 'openNote(' + hosArg(id) + ')') + '>'  (built in script)
//
// and this file runs it. It is NOT JavaScript: it is a small CALL language, parsed here and never
// evaluated as code.
//
// ONLY HANDLERS THE PAGE ITSELF WROTE RUN (BUG-331, 2026-11-20). A data-on-* attribute does
// nothing because it is in the document: markup slipped in through an unescaped innerHTML write
// (a name, a room title, a message) could otherwise carry its own data-on-error and run it with
// no click. A handler runs only when the page vouched for it, one of two ways:
//  - STATIC HTML. While the browser parses the page (this file is the FIRST script in the
//    <head>, so it sees every element the parser makes), each element's data-on-* values are
//    recorded, and at run time the RECORDED text is used, never the live attribute: an attribute
//    added to a page element later does nothing. Elements a script makes while the page is still
//    loading (createElement, innerHTML and the other ways of turning a string into markup) are
//    marked and never recorded; recording stops when parsing ends.
//  - MARKUP BUILT IN SCRIPT. hosOn.attr(type, text) keeps the handler text in a registry inside
//    this file under a token made fresh for each page load (crypto.getRandomValues), and writes
//    data-on-<type>="<token>". The runner looks the text up by token and type; the DOM holds only
//    the token. Stored or relayed markup cannot know a token, and an attacker's data-on-click
//    placed before the page's own in the same tag (the parser keeps the first) is not one.
// Any other data-on-* value is ignored, with one console warning per value.
//
// THE LANGUAGE. One or more calls separated by ";". A call is a name path and arguments:
//     name(...)   a.b.c(...)   this.closest('.row').remove()   event.stopPropagation()
// An argument is a string ('..' or ".." with backslash escapes), a number, true, false, null,
// undefined, an object {key: value, ...}, a list [a, b], or a path, which may itself call:
//     this  this.value  this.checked  this.dataset.id  event  event.key
// A path starts at `this` (the element carrying the attribute, as in an inline handler), `event`
// (the event), `hosOn` (the helpers below, and only those), or a name the page defined on window
// (a function declared at a script's top level, a `var`, or a `window.x =` assignment; a top-level
// `let` or `const` is NOT on window, so it cannot be named). A path never starts at a literal.
// There are no operators, no assignments and no `if`: anything more than calls belongs in a named
// function in the page's script, which the attribute then calls.
//
// WHAT A HANDLER CAN REACH, even one the page wrote (defence in depth, BUG-331):
//  - a window name only when some handler the page vouched for names it, and never one the
//    browser already had when this file loaded (document, location, localStorage, fetch, Math,
//    String, console...): those belong in a named page function;
//  - never the window, the document, the address, storage or indexedDB as a value, by any path
//    (this.ownerDocument.defaultView, event.view, event.composedPath(), a parentNode chain...),
//    nor a property on the DENIED list below;
//  - never a function that runs code, writes markup or defines properties (eval, Function,
//    timers, Object.assign, Reflect, bind/call/apply, document.write, insertAdjacentHTML...).
//
// INTERPOLATING DATA into a handler built in script: every value goes through hosArg(v), which
// writes String(v) as a string literal of the language (escaped for a quoted attribute too, so it
// is safe either way), or hosNum(v) for a number (an index, a numeric id).
// scripts/tests/inline-script-web.test.js refuses a data-on-* attribute written in a script
// string at all, and any interpolation inside a hosOn.attr text other than these two:
//     hosOn.attr('click', 'openNote(' + hosArg(note.id) + ')')
//     hosOn.attr('click', `castVote(${hosNum(p.id)}, 'pass')`)
//
// HOW IT RUNS, as close to an inline handler as script can be: one capturing listener per event
// type on the document sees each event first, and for every element on the event's path carrying
// data-on-<type> adds, once, an ordinary listener on THAT element. The event then reaches the
// element and its listener runs in the element's own place in the bubbling order, so
// event.stopPropagation() in an inner element's handler still keeps an outer one from running,
// and a listener elsewhere sees the event after the element's handler, as before.
//
// Load it as the FIRST script in the <head> of every page that writes data-on-* or loads a script
// that does (the test above checks that). A classic script in the browser (window.hosOn, hosArg,
// hosNum, hosEsc); a CommonJS module under Node for the tests.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The events a page may handle this way. Each is a data-on-<type> attribute.
  var TYPES = [
    'click', 'dblclick', 'contextmenu', 'change', 'input', 'submit', 'keydown', 'keyup',
    'mouseover', 'mouseout', 'mouseenter', 'mouseleave', 'focus', 'blur', 'error', 'load',
  ];

  // Names a path may never start at, said here so a reader need not reason it out (a name the
  // browser had at load is refused anyway, below).
  var DENIED_NAMES = {
    eval: 1, Function: 1, setTimeout: 1, setInterval: 1, constructor: 1, window: 1, self: 1,
    globalThis: 1, document: 1, location: 1, localStorage: 1, sessionStorage: 1, indexedDB: 1,
    opener: 1, parent: 1, top: 1, frames: 1, fetch: 1, Reflect: 1, Object: 1,
  };
  // Properties a path may never read (each would reach the window, the document, storage or a way
  // to run code or write markup), at parse time and again at run time.
  var DENIED_PROPS = {
    constructor: 1, __proto__: 1, prototype: 1, __defineGetter__: 1, __defineSetter__: 1,
    __lookupGetter__: 1, __lookupSetter__: 1, defaultView: 1, view: 1, ownerDocument: 1,
    location: 1, cookie: 1, localStorage: 1, sessionStorage: 1, indexedDB: 1, opener: 1,
    parent: 1, top: 1, frames: 1, window: 1, self: 1, globalThis: 1, document: 1,
    getRootNode: 1, composedPath: 1, path: 1, contentWindow: 1, contentDocument: 1,
    bind: 1, call: 1, apply: 1, then: 1, assign: 1, defineProperty: 1, defineProperties: 1,
    setPrototypeOf: 1, innerHTML: 1, outerHTML: 1, insertAdjacentHTML: 1, setHTMLUnsafe: 1,
    srcdoc: 1, write: 1, writeln: 1, execCommand: 1, createContextualFragment: 1,
    parseFromString: 1, setAttribute: 1, setAttributeNS: 1, setAttributeNode: 1,
  };

  // Every name the global object had when this file loaded: what the browser (or Node) provides.
  // A handler path may not start at one of these, so a handler reaches only names the page's own
  // scripts defined after this file, the first script on the page.
  var BUILTIN = Object.create(null);
  for (var o = root; o; o = Object.getPrototypeOf(o)) {
    var own = Object.getOwnPropertyNames(o);
    for (var b = 0; b < own.length; b++) BUILTIN[own[b]] = 1;
  }

  /** Parse handler text into a list of call nodes, or throw a SyntaxError naming the place. */
  function parse(src) {
    src = String(src);
    var i = 0;
    var n = src.length;
    function fail(msg) {
      throw new SyntaxError('data-on handler: ' + msg + ' at character ' + i + ' of: ' + src);
    }
    function ws() { while (i < n && /\s/.test(src[i])) i++; }
    function peek() { ws(); return i < n ? src[i] : ''; }
    function eat(ch) { if (peek() !== ch) fail('expected "' + ch + '"'); i++; }
    function name() {
      ws();
      var m = /^[A-Za-z_$][\w$]*/.exec(src.slice(i));
      if (!m) fail('expected a name');
      i += m[0].length;
      return m[0];
    }
    function string() {
      var q = src[i++];
      var out = '';
      while (i < n && src[i] !== q) {
        var c = src[i++];
        if (c !== '\\') { out += c; continue; }
        if (i >= n) break;
        var e = src[i++];
        if (e === 'n') out += '\n';
        else if (e === 't') out += '\t';
        else if (e === 'r') out += '\r';
        else if (e === 'b') out += '\b';
        else if (e === 'f') out += '\f';
        else if (e === 'u') {
          var h = src.substr(i, 4);
          if (!/^[0-9a-fA-F]{4}$/.test(h)) fail('a \\u escape needs four hex digits');
          out += String.fromCharCode(parseInt(h, 16));
          i += 4;
        } else out += e;
      }
      if (i >= n) fail('a string is not closed');
      i++;
      return out;
    }
    function number() {
      var m = /^-?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/.exec(src.slice(i));
      if (!m) fail('expected a number');
      i += m[0].length;
      return Number(m[0]);
    }
    function list(close) {
      var items = [];
      while (peek() !== close) {
        items.push(expr());
        if (peek() === ',') i++; else break;
      }
      eat(close);
      return items;
    }
    function primary() {
      var c = peek();
      if (c === "'" || c === '"') return { k: 'lit', v: string() };
      if (c === '-' || c === '.' || (c >= '0' && c <= '9')) return { k: 'lit', v: number() };
      if (c === '[') { i++; return { k: 'arr', items: list(']') }; }
      if (c === '{') {
        i++;
        var props = [];
        while (peek() !== '}') {
          var d = peek();
          var key = d === "'" || d === '"' ? string() : name();
          if (DENIED_PROPS[key]) fail('"' + key + '" may not be set');
          eat(':');
          props.push([key, expr()]);
          if (peek() === ',') i++; else break;
        }
        eat('}');
        return { k: 'obj', props: props };
      }
      var id = name();
      if (id === 'true') return { k: 'lit', v: true };
      if (id === 'false') return { k: 'lit', v: false };
      if (id === 'null') return { k: 'lit', v: null };
      if (id === 'undefined') return { k: 'lit', v: undefined };
      if (id === 'this' || id === 'event') return { k: id };
      if (DENIED_NAMES[id]) fail('"' + id + '" may not be called from markup');
      return { k: 'global', name: id };
    }
    function expr() {
      var e = primary();
      // A path starts at a name, never at a value written in the handler: no 'x'.concat(...).
      var literal = e.k === 'lit' || e.k === 'arr' || e.k === 'obj';
      for (;;) {
        var c = peek();
        if ((c === '.' || c === '(') && literal) fail('a path may not start at a written value');
        if (c === '.') {
          i++;
          var p = name();
          if (DENIED_PROPS[p]) fail('"' + p + '" may not be read');
          e = { k: 'get', obj: e, name: p };
        } else if (c === '(') {
          i++;
          e = { k: 'call', fn: e, args: list(')') };
        } else return e;
      }
    }
    var calls = [];
    for (;;) {
      var c = peek();
      if (c === '') break;
      if (c === ';') { i++; continue; }
      var s = expr();
      if (s.k !== 'call') fail('each statement must be a call');
      calls.push(s);
      c = peek();
      if (c !== '' && c !== ';') fail('expected ";" or the end');
    }
    return calls;
  }

  /** The global names a handler starts a path at (for the tests' check that each exists). */
  function globalsOf(calls) {
    var out = [];
    (function walk(node) {
      if (!node || typeof node !== 'object') return;
      if (node.k === 'global') { if (out.indexOf(node.name) < 0) out.push(node.name); return; }
      if (Array.isArray(node)) { node.forEach(walk); return; }
      walk(node.obj); walk(node.fn);
      if (node.args) node.args.forEach(walk);
      if (node.items) node.items.forEach(walk);
      if (node.props) node.props.forEach(function (p) { walk(p[1]); });
    })(calls);
    return out;
  }

  // ── What the page vouched for ──
  // ALLOWED: the window names some vouched-for handler starts a path at (and the names its
  // hosOn.maybe('x') calls ask for). TOKENS: token -> {type, text} for markup built in script.
  // STATIC: element -> {type: text} recorded while the parser built the page.
  var ALLOWED = Object.create(null);
  var TOKENS = new Map();
  var TOKEN_OF = new Map();
  var STATIC = new WeakMap();

  function vouch(calls) {
    globalsOf(calls).forEach(function (g) { ALLOWED[g] = 1; });
    (function walk(node) {
      if (!node || typeof node !== 'object') return;
      if (Array.isArray(node)) { node.forEach(walk); return; }
      if (node.k === 'call' && node.fn.k === 'get' && node.fn.name === 'maybe' && node.fn.obj.k === 'global' &&
          node.fn.obj.name === 'hosOn' && node.args[0] && node.args[0].k === 'lit' && typeof node.args[0].v === 'string') {
        ALLOWED[node.args[0].v] = 1;
      }
      walk(node.obj); walk(node.fn);
      if (node.args) node.args.forEach(walk);
      if (node.items) node.items.forEach(walk);
      if (node.props) node.props.forEach(function (p) { walk(p[1]); });
    })(calls);
  }
  function nameAllowed(name) {
    return ALLOWED[name] === 1 && BUILTIN[name] !== 1 && !DENIED_NAMES[name];
  }

  function describe(node) {
    if (node.k === 'global') return node.name;
    if (node.k === 'this' || node.k === 'event') return node.k;
    if (node.k === 'get') return describe(node.obj) + '.' + node.name;
    if (node.k === 'call') return describe(node.fn) + '(...)';
    return 'a value';
  }

  /** Is `v` the window, a document, the address, storage, indexedDB or the like? */
  var FORBIDDEN_TAG = /^\[object (Window|Location|Storage|IDBFactory|HTMLDocument|Document|XMLDocument|Navigator|History|CacheStorage|Crypto|SubtleCrypto|CookieStore)\]$/;
  function forbidden(v) {
    if (v === null || (typeof v !== 'object' && typeof v !== 'function')) return false;
    if (v === root || v === api) return true;
    try {
      if (v.window === v || v.nodeType === 9) return true;
      return FORBIDDEN_TAG.test(Object.prototype.toString.call(v));
    } catch (e) {
      return true; // a value that refuses inspection (another site's window) is refused
    }
  }
  function guard(v, node) {
    if (forbidden(v)) throw new TypeError('a data-on handler may not reach ' + describe(node) + ' (the window, the document, the address or storage)');
    return v;
  }

  // Functions a handler may never call, by identity, however it reached them: each runs code,
  // writes markup, defines properties, or sends something somewhere.
  var REFUSED = [];
  function refuse(obj, names) {
    if (!obj) return;
    names.forEach(function (k) {
      try { if (typeof obj[k] === 'function') REFUSED.push(obj[k]); } catch (e) { /* not there */ }
    });
  }
  refuse(root, ['eval', 'Function', 'setTimeout', 'setInterval', 'setImmediate', 'queueMicrotask', 'fetch', 'open', 'postMessage', 'importScripts']);
  refuse(Object, ['assign', 'defineProperty', 'defineProperties', 'setPrototypeOf', 'create']);
  if (typeof Reflect === 'object') refuse(Reflect, Object.getOwnPropertyNames(Reflect));
  refuse(Function.prototype, ['call', 'apply', 'bind']);
  function protoOf(name) { var C = root[name]; return C && C.prototype; }
  refuse(protoOf('Document'), ['write', 'writeln', 'open', 'execCommand']);
  refuse(protoOf('Element'), ['insertAdjacentHTML', 'setHTMLUnsafe', 'setAttribute', 'setAttributeNS', 'setAttributeNode']);
  refuse(protoOf('Range'), ['createContextualFragment']);
  refuse(protoOf('DOMParser'), ['parseFromString']);
  refuse(protoOf('Storage'), ['getItem', 'setItem', 'removeItem', 'clear', 'key']);
  refuse(protoOf('Navigator'), ['sendBeacon']);

  function value(node, el, ev) {
    switch (node.k) {
      case 'lit': return node.v;
      case 'this': return el;
      case 'event': return ev;
      case 'global':
        if (node.name === 'hosOn') return HELPERS;
        if (!nameAllowed(node.name)) throw new ReferenceError(node.name + ' is not a name this page\'s own handlers use (a data-on handler reaches only names the page defined)');
        if (!(node.name in root)) throw new ReferenceError(node.name + ' is not defined on window (a data-on handler can only reach window names)');
        return guard(root[node.name], node);
      case 'get': {
        var o = value(node.obj, el, ev);
        if (o === null || o === undefined) throw new TypeError('cannot read "' + node.name + '" of ' + describe(node.obj) + ', which is ' + o);
        if (DENIED_PROPS[node.name]) throw new TypeError('"' + node.name + '" may not be read');
        return guard(o[node.name], node);
      }
      case 'arr': return node.items.map(function (x) { return value(x, el, ev); });
      case 'obj': {
        var r = {};
        node.props.forEach(function (p) { r[p[0]] = value(p[1], el, ev); });
        return r;
      }
      case 'call': {
        var recv;
        var fn;
        if (node.fn.k === 'get') {
          recv = value(node.fn.obj, el, ev);
          if (recv === null || recv === undefined) throw new TypeError('cannot call ' + describe(node.fn) + ': ' + describe(node.fn.obj) + ' is ' + recv);
          if (DENIED_PROPS[node.fn.name]) throw new TypeError('"' + node.fn.name + '" may not be called');
          fn = recv[node.fn.name];
        } else {
          fn = value(node.fn, el, ev);
        }
        if (typeof fn !== 'function') throw new TypeError(describe(node.fn) + ' is not a function');
        if (REFUSED.indexOf(fn) >= 0) throw new TypeError('data-on handlers may not call ' + describe(node.fn));
        var args = node.args.map(function (x) { return value(x, el, ev); });
        return guard(fn.apply(recv, args), node);
      }
    }
    throw new TypeError('unknown handler part');
  }

  /**
   * Run handler text as `el`'s handler for `ev`. Errors are reported, as an inline handler's.
   * The text is held to the same rules as any handler: it may only start paths at names a
   * handler the page vouched for already uses (for the page's own code re-running a handler it
   * got from hosOn.handlerOf).
   */
  function run(src, el, ev) {
    var calls = parse(src);
    for (var k = 0; k < calls.length; k++) value(calls[k], el, ev);
  }

  // ── Handlers in markup built in script ──
  function decodeEntities(s) {
    return s.replace(/&(#x[0-9a-fA-F]+|#\d+|quot|apos|lt|gt|amp);/g, function (m, e) {
      if (e[0] === '#') return String.fromCharCode(e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
      return { quot: '"', apos: "'", lt: '<', gt: '>', amp: '&' }[e];
    });
  }
  function freshToken() {
    var c = root.crypto;
    if (!c || typeof c.getRandomValues !== 'function') throw new Error('safe-markup.js needs crypto.getRandomValues for its handler tokens');
    var bytes = new Uint8Array(16);
    c.getRandomValues(bytes);
    var hex = 'h';
    var any = 0;
    for (var k = 0; k < bytes.length; k++) { any |= bytes[k]; hex += (bytes[k] < 16 ? '0' : '') + bytes[k].toString(16); }
    // Sixteen zero bytes is a stand-in that filled nothing, never a real draw (odds 2^-128).
    if (!any) throw new Error('safe-markup.js: crypto.getRandomValues filled nothing; its handler tokens need real randomness');
    return hex;
  }
  /**
   * The attribute a script writes into markup for a handler: data-on-<type>="<token>". The text
   * is what the attribute used to hold (hosArg/hosNum values in it; HTML entities in it are
   * decoded, as the browser decoded them in an attribute). It is parsed now, so a mistake shows
   * where the markup is built, and the names it starts at become names handlers may reach.
   * The same type and text always get the same token, so re-rendering a list does not grow the
   * registry.
   */
  function attr(type, text) {
    if (TYPES.indexOf(type) < 0) throw new TypeError('hosOn.attr: "' + type + '" is not an event safe-markup.js handles');
    text = decodeEntities(String(text));
    var key = type + '\u0000' + text;
    var token = TOKEN_OF.get(key);
    if (!token) {
      vouch(parse(text));
      token = freshToken();
      TOKENS.set(token, { type: type, text: text });
      TOKEN_OF.set(key, token);
    }
    return 'data-on-' + type + '="' + token + '"';
  }

  var warned = Object.create(null);
  var warnedCount = 0;
  /**
   * The handler text `el` runs for an event of `type`, if the page vouched for it: the text
   * registered under the token in its attribute, or the text recorded for it from the page's
   * static HTML. Null when the element has no data-on-<type> attribute, or one the page did not
   * write (that is ignored, with one console warning per value).
   */
  function handlerOf(el, type) {
    if (!el || typeof el.getAttribute !== 'function') return null;
    var live = el.getAttribute('data-on-' + type);
    if (live === null || live === undefined) return null;
    var reg = TOKENS.get(live);
    if (reg && reg.type === type) return reg.text;
    var rec = STATIC.get(el);
    if (rec && Object.prototype.hasOwnProperty.call(rec, type)) return rec[type];
    if (!warned[live] && warnedCount < 50 && root.console && typeof root.console.warn === 'function') {
      warned[live] = 1;
      warnedCount++;
      root.console.warn('safe-markup.js: ignored a data-on-' + type + ' handler the page did not write: ' + String(live).slice(0, 120));
    }
    return null;
  }

  /** Record a static element's data-on-* values (the first time it is seen) and vouch for them. */
  function recordElement(el) {
    if (STATIC.has(el) || !el.attributes) return;
    var rec = null;
    for (var k = 0; k < el.attributes.length; k++) {
      var a = el.attributes[k];
      if (a.name.lastIndexOf('data-on-', 0) !== 0) continue;
      var type = a.name.slice(8);
      if (TYPES.indexOf(type) < 0) continue;
      rec = rec || Object.create(null);
      rec[type] = a.value;
      try { vouch(parse(a.value)); } catch (e) { /* reported when it runs */ }
    }
    if (rec) STATIC.set(el, rec);
  }

  /** Text made safe for HTML attributes (the five characters). */
  function attrEscape(lit) {
    return lit.replace(/[&<>"']/g, function (c) {
      return c === '&' ? '&amp;' : c === '<' ? '&lt;' : c === '>' ? '&gt;' : c === '"' ? '&quot;' : '&#39;';
    });
  }
  /**
   * Write `v` as a STRING literal of the handler language (String(v), as '${v}' made it), also
   * escaped for a quoted HTML attribute. With hosNum, the ONLY ways a script puts data into a
   * handler text.
   */
  function hosArg(v) { return attrEscape(JSON.stringify(String(v))); }
  /** Write `v` as a NUMBER literal (an index, a numeric id), as a bare ${v} made it. */
  function hosNum(v) {
    var n = Number(v);
    return isFinite(n) ? String(n) : 'null';
  }

  /** Text made safe for HTML, between tags or in a quoted attribute. */
  function hosEsc(s) {
    return String(s === null || s === undefined ? '' : s).replace(/[&<>"']/g, function (c) {
      return c === '&' ? '&amp;' : c === '<' ? '&lt;' : c === '>' ? '&gt;' : c === '"' ? '&quot;' : '&#39;';
    });
  }

  // ── Small helpers a handler calls for what an inline handler did with an `if` or an `=` ──
  // Each says what it does, so the markup still reads as what the control does. In a handler,
  // `hosOn` is these and nothing else.

  /** Call fn(...args) only when the event happened on the element itself, not on something inside
   *  it: a dialog's backdrop closes on a press outside the box (was: if(event.target===this)fn()). */
  function ifSelf(ev, fn) {
    if (ev.target !== ev.currentTarget) return undefined;
    return fn.apply(undefined, Array.prototype.slice.call(arguments, 2));
  }
  function keyMatches(ev, keys) {
    return (Array.isArray(keys) ? keys : [keys]).indexOf(ev.key) >= 0;
  }
  /** Call fn(...args) when the key pressed is `keys` (a name, or a list of names). */
  function onKey(ev, keys, fn) {
    if (!keyMatches(ev, keys)) return undefined;
    return fn.apply(undefined, Array.prototype.slice.call(arguments, 3));
  }
  /** As onKey, and the key does nothing else (event.preventDefault()). */
  function onKeyPrevent(ev, keys, fn) {
    if (!keyMatches(ev, keys)) return undefined;
    ev.preventDefault();
    return fn.apply(undefined, Array.prototype.slice.call(arguments, 3));
  }
  // Properties set() refuses: markup, handlers and addresses, the ways set() could put script or
  // markup in, and the names that reach an object's machinery.
  var UNSETTABLE = /^(innerHTML|outerHTML|srcdoc|on[a-z]*|href|src|action|formAction|data|codebase|__proto__|constructor|prototype)$/i;
  /** target[prop] = value (was: x.value=..., x.style.display='none'). Not for markup or links. */
  function set(target, prop, value) {
    prop = String(prop);
    if (UNSETTABLE.test(prop) || DENIED_PROPS[prop]) throw new TypeError('data-on handlers may not set ' + prop);
    if (target === null || target === undefined || forbidden(target) || target === HELPERS) throw new TypeError('data-on handlers may not set ' + prop + ' on that');
    target[prop] = value;
  }
  /** Call window[name](...args) when the page has it (was: if (window.x) x()). The name is one a
   *  vouched-for handler asked for in writing (hosOn.maybe('x')), never one the browser had. */
  function maybe(name) {
    name = String(name);
    if (!nameAllowed(name)) throw new ReferenceError('hosOn.maybe: ' + name + ' is not a name this page\'s handlers ask for');
    var fn = root[name];
    if (typeof fn !== 'function' || REFUSED.indexOf(fn) >= 0) return undefined;
    return fn.apply(undefined, Array.prototype.slice.call(arguments, 1));
  }
  /** Take a class off an element (with ifSelf: a backdrop that closes by losing 'open'). */
  function removeClass(el, cls) { el.classList.remove(cls); }
  /** The element with this id, or null (was: document.getElementById('x'); a handler may not
   *  reach the document itself). */
  function byId(id) {
    var d = root.document;
    return d && typeof d.getElementById === 'function' ? d.getElementById(String(id)) : null;
  }

  // What `hosOn` is inside a handler: the helpers, on an object with no prototype.
  var HELPERS = Object.freeze(Object.assign(Object.create(null), {
    ifSelf: ifSelf, onKey: onKey, onKeyPrevent: onKeyPrevent, set: set, maybe: maybe, removeClass: removeClass, byId: byId,
  }));

  var api = {
    TYPES: TYPES, parse: parse, globalsOf: globalsOf, run: run, attr: attr, handlerOf: handlerOf,
    hosArg: hosArg, hosNum: hosNum, hosEsc: hosEsc,
    ifSelf: ifSelf, onKey: onKey, onKeyPrevent: onKeyPrevent, set: set, maybe: maybe, removeClass: removeClass, byId: byId,
  };

  if (typeof module === 'object' && module && module.exports) {
    module.exports = api;
    return;
  }

  root.hosOn = api;
  root.hosArg = hosArg;
  root.hosNum = hosNum;
  root.hosEsc = hosEsc;

  // (A page's tests load this file into a stand-in document that may have no listeners at all.)
  var doc = root.document;
  if (!doc || typeof doc.addEventListener !== 'function') return;

  // ── Recording the page's static handlers while the parser builds it ──
  // Every element the HTML parser adds is recorded the first time it is seen. Elements a script
  // makes while the page loads are marked as they are made and never recorded: every way script
  // turns a string into elements, or makes or copies one, is wrapped below until parsing ends,
  // then put back. (The parser's own additions reach the observer before any script runs: the
  // browser delivers them at the checkpoint before each script and at the end of each task.)
  if (doc.readyState === 'loading' && typeof root.MutationObserver === 'function') {
    var made = new WeakSet();
    var recording = true;
    var undo = [];
    var markTree = function (node) {
      if (!node || typeof node !== 'object') return;
      if (node.nodeType === 1) made.add(node);
      if (node.nodeType === 1 && node.content && node.content.nodeType === 11) markTree(node.content);
      var kids = node.childNodes;
      if (kids) for (var k = 0; k < kids.length; k++) markTree(kids[k]);
    };
    var markInside = function (node) {
      if (!node) return;
      if (node.content && node.content.nodeType === 11) markTree(node.content);
      var kids = node.childNodes;
      if (kids) for (var k = 0; k < kids.length; k++) markTree(kids[k]);
    };
    var recordTree = function (node) {
      if (!node || typeof node !== 'object') return;
      if (node.nodeType === 1) {
        if (made.has(node)) return;
        recordElement(node);
      } else if (node.nodeType !== 9 && node.nodeType !== 11) return;
      var kids = node.childNodes;
      if (kids) for (var k = 0; k < kids.length; k++) recordTree(kids[k]);
    };
    var take = function (records) {
      for (var r = 0; r < records.length; r++) {
        var added = records[r].addedNodes;
        for (var k = 0; k < added.length; k++) recordTree(added[k]);
      }
    };
    var markAdded = function (records) {
      for (var r = 0; r < records.length; r++) {
        var added = records[r].addedNodes;
        for (var k = 0; k < added.length; k++) markTree(added[k]);
      }
    };
    var observer = new root.MutationObserver(take);
    recordTree(doc.documentElement);
    observer.observe(doc, { childList: true, subtree: true });

    // Wrap owner[name] (a method, or a property's setter) so that, while the page loads, what it
    // makes is marked. `around(orig, self, args)` calls the original and marks.
    var patch = function (owner, name, around) {
      if (!owner) return;
      var d = Object.getOwnPropertyDescriptor(owner, name);
      if (!d || !d.configurable) return;
      var nd = { enumerable: d.enumerable, configurable: true };
      if (typeof d.value === 'function') {
        var orig = d.value;
        nd.writable = d.writable;
        nd.value = function () { return recording ? around(orig, this, arguments) : orig.apply(this, arguments); };
      } else if (typeof d.set === 'function') {
        var origSet = d.set;
        nd.get = d.get;
        nd.set = function (v) { if (recording) around(origSet, this, [v]); else origSet.call(this, v); };
      } else return;
      Object.defineProperty(owner, name, nd);
      undo.push(function () {
        var cur = Object.getOwnPropertyDescriptor(owner, name);
        if (cur && cur.value === nd.value && cur.set === nd.set) Object.defineProperty(owner, name, d);
      });
    };
    var marksResult = function (orig, self, args) { var r = orig.apply(self, args); markTree(r); return r; };
    var marksInside = function (orig, self, args) { var r = orig.apply(self, args); markInside(self); return r; };
    var childSet = function (p) {
      var s = new Set();
      if (p && p.childNodes) for (var k = 0; k < p.childNodes.length; k++) s.add(p.childNodes[k]);
      return s;
    };
    var marksNewChildren = function (parentOf) {
      return function (orig, self, args) {
        var p = parentOf(self, args);
        var before = childSet(p);
        var r = orig.apply(self, args);
        if (p && p.childNodes) for (var k = 0; k < p.childNodes.length; k++) if (!before.has(p.childNodes[k])) markTree(p.childNodes[k]);
        return r;
      };
    };
    var marksPending = function (orig, self, args) { var r = orig.apply(self, args); markAdded(observer.takeRecords()); return r; };
    var P = function (name) { return root[name] && root[name].prototype; };
    patch(P('Document'), 'createElement', marksResult);
    patch(P('Document'), 'createElementNS', marksResult);
    patch(P('Document'), 'importNode', marksResult);
    patch(P('Node'), 'cloneNode', marksResult);
    patch(P('Range'), 'createContextualFragment', marksResult);
    patch(P('Range'), 'cloneContents', marksResult);
    patch(P('Range'), 'extractContents', marksResult);
    patch(P('DOMParser'), 'parseFromString', marksResult);
    patch(root.Document, 'parseHTMLUnsafe', marksResult);
    patch(P('Element'), 'innerHTML', marksInside);
    patch(P('ShadowRoot'), 'innerHTML', marksInside);
    patch(P('Element'), 'setHTMLUnsafe', marksInside);
    patch(P('ShadowRoot'), 'setHTMLUnsafe', marksInside);
    patch(P('Element'), 'outerHTML', marksNewChildren(function (self) { return self.parentNode; }));
    patch(P('Element'), 'insertAdjacentHTML', marksNewChildren(function (self, args) {
      return /^(afterbegin|beforeend)$/i.test(String(args[0])) ? self : self.parentNode;
    }));
    ['Document', 'HTMLDocument'].forEach(function (d) {
      patch(P(d), 'write', marksPending);
      patch(P(d), 'writeln', marksPending);
      patch(P(d), 'execCommand', marksPending);
    });

    var stop = function () {
      if (!recording || doc.readyState === 'loading') return;
      take(observer.takeRecords());
      observer.disconnect();
      recording = false;
      for (var k = 0; k < undo.length; k++) undo[k]();
      doc.removeEventListener('readystatechange', stop);
    };
    doc.addEventListener('readystatechange', stop);
  }

  // One listener per element and type, added the first time an event of that type passes it.
  var bound = new WeakMap();
  function handlerFor(type) {
    return function (ev) {
      var src = handlerOf(this, type);
      if (src !== null) run(src, this, ev);
    };
  }
  var HANDLERS = {};
  TYPES.forEach(function (type) {
    HANDLERS[type] = handlerFor(type);
    var attrName = 'data-on-' + type;
    doc.addEventListener(type, function (ev) {
      var path = ev.composedPath ? ev.composedPath() : [];
      for (var p = 0; p < path.length; p++) {
        var node = path[p];
        if (!node || node.nodeType !== 1 || !node.hasAttribute(attrName)) continue;
        var seen = bound.get(node);
        if (!seen) { seen = {}; bound.set(node, seen); }
        if (seen[type]) continue;
        seen[type] = true;
        node.addEventListener(type, HANDLERS[type]);
      }
    }, true);
  });
})(typeof self !== 'undefined' ? self : globalThis);
