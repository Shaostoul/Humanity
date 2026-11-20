// ── safe-markup.js ────────────────────────────────────────────────────────
// Markup without inline script (2026-11-20, the website's script policy, docs/PRIORITIES.md
// item 6b): event handlers written in the markup (data-on-*, below), hosArg for putting data into
// one, and hosEsc, THE house escape for putting text into markup.
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
// browser refuses every onclick="..." attribute and every <script> block written in a page: an
// injected one can never run. The pages still want to say, in the markup, what a button does.
// So a page writes
//
//     <button data-on-click="showSection('profile')">          (static HTML)
//     '<button data-on-click="openNote(' + hosArg(id) + ')">'  (markup built in script)
//
// and this file runs it. It is NOT JavaScript: it is a small CALL language, parsed here and never
// evaluated as code, so nothing written in an attribute can do more than call a function.
//
// THE LANGUAGE. One or more calls separated by ";". A call is a name path and arguments:
//     name(...)   a.b.c(...)   this.closest('.row').remove()   event.stopPropagation()
// An argument is a string ('..' or ".." with backslash escapes), a number, true, false, null,
// undefined, an object {key: value, ...}, a list [a, b], or a path, which may itself call:
//     this  this.value  this.checked  this.dataset.id  event  event.key
//     document.getElementById('x').value
// A path starts at `this` (the element carrying the attribute, as in an inline handler), `event`
// (the event), or a name on window (a function declared at a script's top level, a `var`, or a
// `window.x =` assignment; a top-level `let` or `const` is NOT on window, so it cannot be named).
// There are no operators, no assignments and no `if`: anything more than calls belongs in a named
// function in the page's script, which the attribute then calls.
//
// INTERPOLATING DATA in markup built in script: every value goes through hosArg(v), which writes
// String(v) as a string literal of the language and escapes it for a quoted HTML attribute, so a
// name with a quote in it stays one string; or hosNum(v) for a number (an index, a numeric id).
// scripts/tests/inline-script-web.test.js refuses any other interpolation inside a data-on-*
// attribute:
//     'data-on-click="openNote(' + hosArg(note.id) + ')"'
//     `data-on-click="castVote(${hosNum(p.id)}, 'pass')"`
//
// HOW IT RUNS, as close to an inline handler as script can be: one capturing listener per event
// type on the document sees each event first, and for every element on the event's path carrying
// data-on-<type> adds, once, an ordinary listener on THAT element. The event then reaches the
// element and its listener runs in the element's own place in the bubbling order, so
// event.stopPropagation() in an inner element's handler still keeps an outer one from running,
// and a listener elsewhere sees the event after the element's handler, as before. The attribute is
// read when the event happens, so markup written later or changed later works with no extra step.
//
// Load it in the <head> of every page that writes data-on-* (the test above checks that), before
// anything can be pressed. A classic script in the browser (window.hosOn, hosArg, hosNum, hosEsc); a
// CommonJS module under Node for the tests.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The events a page may handle this way. Each is a data-on-<type> attribute.
  var TYPES = [
    'click', 'dblclick', 'contextmenu', 'change', 'input', 'submit', 'keydown', 'keyup',
    'mouseover', 'mouseout', 'mouseenter', 'mouseleave', 'focus', 'blur', 'error', 'load',
  ];

  // Names a handler may never reach, though nothing could run through them under the policy
  // (no 'unsafe-eval' either): said here so a reader need not reason it out.
  var DENIED_NAMES = { eval: 1, Function: 1, setTimeout: 1, setInterval: 1, constructor: 1 };
  var DENIED_PROPS = { constructor: 1, __proto__: 1, prototype: 1, __defineGetter__: 1, __defineSetter__: 1 };

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
      for (;;) {
        var c = peek();
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

  function describe(node) {
    if (node.k === 'global') return node.name;
    if (node.k === 'this' || node.k === 'event') return node.k;
    if (node.k === 'get') return describe(node.obj) + '.' + node.name;
    if (node.k === 'call') return describe(node.fn) + '(...)';
    return 'a value';
  }

  function value(node, el, ev, win) {
    switch (node.k) {
      case 'lit': return node.v;
      case 'this': return el;
      case 'event': return ev;
      case 'global':
        if (!(node.name in win)) throw new ReferenceError(node.name + ' is not defined on window (a data-on handler can only reach window names)');
        return win[node.name];
      case 'get': {
        var o = value(node.obj, el, ev, win);
        if (o === null || o === undefined) throw new TypeError('cannot read "' + node.name + '" of ' + describe(node.obj) + ', which is ' + o);
        return o[node.name];
      }
      case 'arr': return node.items.map(function (x) { return value(x, el, ev, win); });
      case 'obj': {
        var r = {};
        node.props.forEach(function (p) { r[p[0]] = value(p[1], el, ev, win); });
        return r;
      }
      case 'call': {
        var recv;
        var fn;
        if (node.fn.k === 'get') {
          recv = value(node.fn.obj, el, ev, win);
          if (recv === null || recv === undefined) throw new TypeError('cannot call ' + describe(node.fn) + ': ' + describe(node.fn.obj) + ' is ' + recv);
          fn = recv[node.fn.name];
        } else {
          fn = value(node.fn, el, ev, win);
        }
        if (typeof fn !== 'function') throw new TypeError(describe(node.fn) + ' is not a function');
        if (fn === win.eval || fn === win.Function) throw new TypeError('data-on handlers may not run code');
        var args = node.args.map(function (x) { return value(x, el, ev, win); });
        return fn.apply(recv, args);
      }
    }
    throw new TypeError('unknown handler part');
  }

  /** Run handler text as `el`'s handler for `ev`. Errors are reported, as an inline handler's. */
  function run(src, el, ev) {
    var win = root;
    var calls = parse(src);
    for (var k = 0; k < calls.length; k++) value(calls[k], el, ev, win);
  }

  function attrEscape(lit) {
    return lit.replace(/[&<>"']/g, function (c) {
      return c === '&' ? '&amp;' : c === '<' ? '&lt;' : c === '>' ? '&gt;' : c === '"' ? '&quot;' : '&#39;';
    });
  }
  /**
   * Write `v` as a STRING literal of the handler language (String(v), as '${v}' made it), escaped
   * for a double- or single-quoted HTML attribute. With hosNum, the ONLY ways markup built in
   * script puts data into a data-on-* attribute.
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
  // Each says what it does, so the markup still reads as what the control does.

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
  // Properties set() refuses: markup and handlers, the ways set() could put script or markup in.
  var UNSETTABLE = /^(innerHTML|outerHTML|srcdoc|on[a-z]+|href|src|action|formAction)$/;
  /** target[prop] = value (was: x.value=..., x.style.display='none'). Not for markup or links. */
  function set(target, prop, value) {
    if (UNSETTABLE.test(String(prop))) throw new TypeError('data-on handlers may not set ' + prop);
    target[prop] = value;
  }
  /** Call window[name](...args) when the page has it (was: if (window.x) x()). */
  function maybe(name) {
    var fn = root[name];
    if (typeof fn !== 'function') return undefined;
    return fn.apply(undefined, Array.prototype.slice.call(arguments, 1));
  }
  /** Take a class off an element (with ifSelf: a backdrop that closes by losing 'open'). */
  function removeClass(el, cls) { el.classList.remove(cls); }

  var api = {
    TYPES: TYPES, parse: parse, globalsOf: globalsOf, run: run, hosArg: hosArg, hosNum: hosNum, hosEsc: hosEsc,
    ifSelf: ifSelf, onKey: onKey, onKeyPrevent: onKeyPrevent, set: set, maybe: maybe, removeClass: removeClass,
  };

  if (typeof module === 'object' && module && module.exports) {
    module.exports = api;
    return;
  }

  root.hosOn = api;
  root.hosArg = hosArg;
  root.hosNum = hosNum;
  root.hosEsc = hosEsc;

  // One listener per element and type, added the first time an event of that type passes it.
  // (A page's tests load this file into a stand-in document that may have no listeners at all.)
  if (!root.document || typeof root.document.addEventListener !== 'function') return;
  var bound = new WeakMap();
  function handlerFor(type) {
    return function (ev) {
      var src = this.getAttribute('data-on-' + type);
      if (src) run(src, this, ev);
    };
  }
  var HANDLERS = {};
  TYPES.forEach(function (type) {
    HANDLERS[type] = handlerFor(type);
    var attr = 'data-on-' + type;
    root.document.addEventListener(type, function (ev) {
      var path = ev.composedPath ? ev.composedPath() : [];
      for (var p = 0; p < path.length; p++) {
        var node = path[p];
        if (!node || node.nodeType !== 1 || !node.hasAttribute(attr)) continue;
        var seen = bound.get(node);
        if (!seen) { seen = {}; bound.set(node, seen); }
        if (seen[type]) continue;
        seen[type] = true;
        node.addEventListener(type, HANDLERS[type]);
      }
    }, true);
  });
})(typeof self !== 'undefined' ? self : globalThis);
