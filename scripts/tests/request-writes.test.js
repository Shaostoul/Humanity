// No script writes a game request file except through scripts/lib/write-request.js.
//
// Run: node --test scripts/tests/request-writes.test.js   (in `just rig-tests`)
//
// WHY. The game polls each dev-IPC request (debug/showcase_request.json, camera_request.json,
// record_cancel.json, ...) every frame, reads it, and deletes it. A plain fs.writeFileSync
// creates the file EMPTY and then fills it, so a frame in between reads nothing, deletes it, and
// the request is lost with no error anywhere. On 2026-10-20 that lost verify-falls' `illness`
// request, and a Forgiving leg ran in Realistic. (The game now leaves an EMPTY request for a
// 250 ms grace, engine/ipc.rs take_request_text, but it still cannot tell a half-filled request
// from a whole one, so writing by rename stays the rule.) writeRequest writes a side file and renames it
// into place (scripts/tests/write-request.test.js proves it against a racing reader). This test
// keeps every rig on it: a new rig that hand-rolls `fs.writeFileSync(path.join(DEBUG,
// "x_request.json"))` fails here, not in a sweep three weeks later.
//
// WHAT IT READS. Every .js/.mjs/.cjs file under scripts/, comments and string contents blanked
// so a comment or a log line that names a request is not a write. It flags, wherever the
// destination names a request (a "..._request.json" or "record_cancel.json" literal, or a
// variable set from one in the same function):
//   - a plain write onto it: writeFileSync, writeFile, appendFile(Sync), createWriteStream,
//     copyFile(Sync), cpSync, or open/openSync with a write flag;
//   - a call to a WRAPPER of one: a function (in the same file, or under scripts/lib/) that does a
//     plain write onto a path built from one of its own parameters, like every rig's old
//     `function req(name, body) { fs.writeFileSync(path.join(DEBUG, name), ...) }`, and a function
//     that hands its parameter on to such a wrapper.
// scripts/lib/write-request.js itself is not read: it is the one place allowed to write a side
// file and rename it. A hand-rolled side file and rename elsewhere is flagged too, deliberately:
// one writer, so the next fix to it reaches every rig.
//
// WHAT IT CANNOT SEE: a writer passed around as a value (`run(fs.writeFileSync)`), a request name
// built at run time from pieces that are not one literal, or a write through a module outside
// scripts/. If a rig ever needs one of those, route it through writeRequest instead.
//
// A line that genuinely must write a request another way carries
// `// request-write-exempt: <why>` (at least ten characters of why). There are none today.
//
// The request names come from the game: the last test reads every `const ..REQ..` /
// `..CANCEL..: &str = "debug/<name>"` under src/ and fails if one is a name this test (and
// write-request.js) would not recognise, so a request named some new way cannot slip past.
//
// RED FIRST (2026-10-20). Run against the tree before the rigs were converted (exit 1):
//   ✔ the scanner catches each way a request has been written, and passes what is not a write
//   ✖ no script under scripts/ writes a game request file except through writeRequest
//     AssertionError [ERR_ASSERTION]: 88 places in 9 scripts write a game request file without
//     scripts/lib/write-request.js, so the game can read one half-written, delete it, and lose it:
//       scripts/boot-timing.js:221  fs.writeFileSync(path.join(DEBUG, "autopilot_request.json"), ...
//           -> writeFileSync straight onto "autopilot_request.json"
//       scripts/make-clips.js:309  req("record_cancel.json", {});
//           -> req (scripts/make-clips.js:206) writes its file with writeFileSync at line 207
//       ... 86 more: make-clips 11 in all, photograph-home 8, probe-hot-ab 8, probe-sweep 25,
//           verify-copresence 22, verify-falls 4, verify-live-screen 5, verify-screens 4, the same
//           count as a grep of each file's req("..._request.json") calls ...
//       scripts/verify-falls.js:409  req("showcase_request.json", body);
//           -> req (scripts/verify-falls.js:368) writes its file with writeFileSync at line 370
//   ✖ the rigs sweep a left-behind side file up with the requests
//     AssertionError [ERR_ASSERTION]: scripts/verify-falls.js clears debug/ at setup but not
//     through WR.isRequestLeftover
//   ✔ every request the game polls is named the way the helper and this scan know
//   ℹ tests 4   ℹ pass 2   ℹ fail 2
// (verify-falls' hand-rolled side file and rename counted: the right thing, done a second way.)
// After the conversion all four pass. Two more reds, each made on purpose and put back:
//   REQUEST_NAME_RE without record_cancel.json:
//     ✖ every request the game polls ...: the game polls request files this scan would not
//       recognise, so a plain write of one would pass unseen:
//         src/engine/movie.rs: CANCEL_PATH = "debug/record_cancel.json"
//     (and the made-up arrow-wrapper case, which writes record_cancel.json, went unflagged)
//   boot-timing's write hidden behind `const drop = [WR.writeRequest][0]; drop(...)`:
//     ✖ no script ...: scripts/boot-timing.js no longer calls writeRequest: how does it write
//       its requests now?

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const WR = require("../lib/write-request.js");

const REPO = path.join(__dirname, "..", "..");
const SCRIPTS = path.join(REPO, "scripts");
const HELPER = path.join(SCRIPTS, "lib", "write-request.js");
const EXEMPT_RE = /\/\/\s*request-write-exempt:(.*)$/;

// ── A small JavaScript lexer: comments, string contents and regex literals blanked ──────────────
// Positions and line breaks are kept, so an index into `code` is an index into the source.

const REGEX_AFTER = new Set("(,=:[!&|?{};+-*%<>~^".split(""));
const REGEX_KEYWORDS = new Set(["return", "typeof", "case", "do", "else", "in", "of", "new", "delete", "void", "throw", "yield", "await", "instanceof"]);

function lex(src) {
  const n = src.length;
  const out = src.split("");
  const strings = [];
  const blank = (a, b) => {
    for (let k = a; k < b && k < n; k++) if (out[k] !== "\n" && out[k] !== "\r") out[k] = " ";
  };
  let i = 0;
  if (src.startsWith("#!")) {
    while (i < n && src[i] !== "\n") i++;
    blank(0, i);
  }
  const frames = []; // templates suspended at `${`
  let depth = 0;
  let last = "";
  let mode = "code";
  let tok = null;
  while (i < n) {
    const c = src[i];
    if (mode === "tmpl") {
      if (c === "\\") {
        tok.value += src[i + 1] || "";
        blank(i, i + 2);
        i += 2;
      } else if (c === "`") {
        tok.end = i + 1;
        strings.push(tok);
        tok = null;
        mode = "code";
        last = "a";
        i++;
      } else if (c === "$" && src[i + 1] === "{") {
        tok.value += "${}";
        frames.push({ tok, depth });
        tok = null;
        blank(i, i + 2);
        depth++;
        mode = "code";
        last = "(";
        i += 2;
      } else {
        tok.value += c;
        blank(i, i + 1);
        i++;
      }
      continue;
    }
    if (c === " " || c === "\t" || c === "\n" || c === "\r") {
      i++;
    } else if (c === "/" && src[i + 1] === "/") {
      let j = i;
      while (j < n && src[j] !== "\n") j++;
      blank(i, j);
      i = j;
    } else if (c === "/" && src[i + 1] === "*") {
      let j = src.indexOf("*/", i + 2);
      j = j < 0 ? n : j + 2;
      blank(i, j);
      i = j;
    } else if (c === "/") {
      let done = false;
      if (last === "" || last === "kw" || REGEX_AFTER.has(last)) {
        let j = i + 1;
        let inClass = false;
        while (j < n) {
          const d = src[j];
          if (d === "\\") {
            j += 2;
            continue;
          }
          if (d === "\n") break;
          if (d === "[") inClass = true;
          else if (d === "]") inClass = false;
          else if (d === "/" && !inClass) {
            j++;
            while (j < n && /[a-z]/i.test(src[j])) j++;
            blank(i, j);
            last = "a";
            i = j;
            done = true;
            break;
          }
          j++;
        }
      }
      if (!done) {
        last = "/";
        i++;
      }
    } else if (c === '"' || c === "'") {
      let j = i + 1;
      let value = "";
      while (j < n && src[j] !== c && src[j] !== "\n") {
        if (src[j] === "\\") {
          value += src[j + 1] || "";
          j += 2;
          continue;
        }
        value += src[j];
        j++;
      }
      strings.push({ start: i, end: j + 1, value });
      blank(i + 1, j);
      last = "a";
      i = j + 1;
    } else if (c === "`") {
      tok = { start: i, end: n, value: "" };
      mode = "tmpl";
      i++;
    } else if (/[A-Za-z_$]/.test(c)) {
      let j = i + 1;
      while (j < n && /[\w$]/.test(src[j])) j++;
      last = REGEX_KEYWORDS.has(src.slice(i, j)) ? "kw" : "a";
      i = j;
    } else if (/[0-9]/.test(c)) {
      let j = i + 1;
      while (j < n && /[\w.]/.test(src[j])) j++;
      last = "a";
      i = j;
    } else if (c === "{") {
      depth++;
      last = "{";
      i++;
    } else if (c === "}") {
      const top = frames[frames.length - 1];
      if (top && top.depth === depth - 1) {
        frames.pop();
        depth--;
        blank(i, i + 1);
        tok = top.tok;
        mode = "tmpl";
      } else {
        depth--;
        last = "}";
      }
      i++;
    } else {
      last = c;
      i++;
    }
  }
  if (tok) strings.push(tok);
  for (const f of frames) strings.push(f.tok);
  strings.sort((a, b) => a.start - b.start);
  return { code: out.join(""), strings };
}

const OPEN = "([{";
const CLOSE = ")]}";

function closeOf(code, open) {
  let d = 0;
  for (let k = open; k < code.length; k++) {
    const ch = code[k];
    if (OPEN.includes(ch)) d++;
    else if (CLOSE.includes(ch) && --d === 0) return k;
  }
  return code.length;
}

/** The argument spans [start, end) of the call whose `(` is at `open`. */
function argsOf(code, open) {
  const close = closeOf(code, open);
  const args = [];
  let d = 0;
  let s = open + 1;
  for (let k = open + 1; k < close; k++) {
    const ch = code[k];
    if (OPEN.includes(ch)) d++;
    else if (CLOSE.includes(ch)) d--;
    else if (ch === "," && d === 0) {
      args.push([s, k]);
      s = k + 1;
    }
  }
  if (args.length || code.slice(s, close).trim() !== "") args.push([s, close]);
  return { args, close };
}

/** Where the expression starting at `from` ends: `;`, `,` or a line break at its own depth (a
 *  line break next to an operator continues it), or a bracket closing around it. */
function exprEnd(code, from) {
  let d = 0;
  for (let k = from; k < code.length; k++) {
    const ch = code[k];
    if (OPEN.includes(ch)) d++;
    else if (CLOSE.includes(ch)) {
      if (d === 0) return k;
      d--;
    } else if (d === 0 && (ch === ";" || ch === ",")) return k;
    else if (d === 0 && ch === "\n") {
      const before = code.slice(from, k).trimEnd();
      const after = code.slice(k + 1).match(/^\s*(\S)/);
      if (before === "" || "=+-*/?:|&(".includes(before[before.length - 1])) continue;
      if (after && ".?:+-*/|&".includes(after[1])) continue;
      return k;
    }
  }
  return code.length;
}

function lineStarts(src) {
  const s = [0];
  for (let k = 0; k < src.length; k++) if (src[k] === "\n") s.push(k + 1);
  return s;
}
function lineOf(starts, pos) {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid] <= pos) lo = mid;
    else hi = mid - 1;
  }
  return lo + 1;
}

/** Every function: name (or null), parameter names, body span. */
function findFunctions(code) {
  const fns = [];
  const seen = new Set();
  const add = (name, paramsOpen, start) => {
    if (seen.has(paramsOpen)) return;
    seen.add(paramsOpen);
    const { args, close } = argsOf(code, paramsOpen);
    const params = args.map(([a, b]) => {
      const m = code.slice(a, b).trim().match(/^(?:\.\.\.)?([A-Za-z_$][\w$]*)/);
      return m ? m[1] : null;
    });
    let k = close + 1;
    while (k < code.length && /\s/.test(code[k])) k++;
    if (code.startsWith("=>", k)) {
      k += 2;
      while (k < code.length && /\s/.test(code[k])) k++;
    }
    const bodyEnd = code[k] === "{" ? closeOf(code, k) + 1 : exprEnd(code, k);
    fns.push({ name, params, start, bodyStart: k, bodyEnd });
  };
  let m;
  const decl = /\bfunction\b\s*\*?\s*([A-Za-z_$][\w$]*)?\s*\(/g;
  while ((m = decl.exec(code))) {
    let name = m[1] || null;
    if (!name) {
      const mm = code.slice(Math.max(0, m.index - 80), m.index).match(/([A-Za-z_$][\w$]*)\s*[=:]\s*(?:async\s+)?$/);
      if (mm) name = mm[1];
    }
    add(name, m.index + m[0].length - 1, m.index);
  }
  // Arrows with a parenthesised parameter list, named when assigned (`const req = (n, b) =>`).
  const arrow = /\(/g;
  while ((m = arrow.exec(code))) {
    const close = closeOf(code, m.index);
    if (!/^\s*=>/.test(code.slice(close + 1, close + 8))) continue;
    const mm = code.slice(Math.max(0, m.index - 80), m.index).match(/([A-Za-z_$][\w$]*)\s*[=:]\s*(?:async\s*)?$/);
    add(mm ? mm[1] : null, m.index, m.index);
  }
  // One bare parameter: `const drop = n => ...`, `files.map(f => ...)`.
  const arrow1 = /(?<![\w$.])([A-Za-z_$][\w$]*)\s*=>/g;
  while ((m = arrow1.exec(code))) {
    const mm = code.slice(Math.max(0, m.index - 80), m.index).match(/([A-Za-z_$][\w$]*)\s*[=:]\s*(?:async\s+)?$/);
    // A fake parameter list over the bare name: argsOf needs a `(`; build the record by hand.
    const key = `b${m.index}`;
    if (seen.has(key)) continue;
    seen.add(key);
    let k = m.index + m[0].length;
    while (k < code.length && /\s/.test(code[k])) k++;
    const bodyEnd = code[k] === "{" ? closeOf(code, k) + 1 : exprEnd(code, k);
    fns.push({ name: mm ? mm[1] : null, params: [m[1]], start: m.index, bodyStart: k, bodyEnd });
  }
  return fns;
}

const WRITE_RE = /(?<![\w$])(writeFileSync|writeFile|appendFileSync|appendFile|createWriteStream|copyFileSync|copyFile|cpSync|openSync|open)\s*\(/g;
const CALL_RE = /(?<![\w$])([A-Za-z_$][\w$]*)\s*\(/g;
const ASSIGN_RE = /(?<![\w$.])([A-Za-z_$][\w$]*)\s*=(?![=>])/g;
const ID_RE = /(?<![\w$.])[A-Za-z_$][\w$]*/g;

/** Is the `(` that ends the match at `open` a function DEFINITION rather than a call? */
function isDefinition(code, nameAt, open) {
  if (/\bfunction\s*\*?\s*$/.test(code.slice(Math.max(0, nameAt - 20), nameAt))) return true;
  const close = closeOf(code, open);
  return /^\s*(\{|=>)/.test(code.slice(close + 1, close + 6));
}

/**
 * Read one file. `lib` is the wrappers found under scripts/lib/ (name -> wrapper). Returns
 * { violations, wrappers, exempt, writeRequestCalls }.
 */
function analyse(rel, src, lib = new Map()) {
  const { code, strings } = lex(src);
  const starts = lineStarts(src);
  const srcLines = src.split("\n");
  const line = (pos) => lineOf(starts, pos);
  const fns = findFunctions(code);
  const named = new Set(fns.filter((f) => f.name).map((f) => f.name));

  const innermost = (pos) => {
    let best = null;
    for (const f of fns) if (pos >= f.bodyStart && pos < f.bodyEnd && (!best || f.bodyEnd - f.bodyStart < best.bodyEnd - best.bodyStart)) best = f;
    return best;
  };
  const stringsIn = (a, b) => {
    let lo = 0;
    let hi = strings.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (strings[mid].start < a) lo = mid + 1;
      else hi = mid;
    }
    const out = [];
    for (let k = lo; k < strings.length && strings[k].start < b; k++) if (strings[k].end <= b) out.push(strings[k]);
    return out;
  };
  const idsIn = (a, b) => {
    const out = [];
    const seg = code.slice(a, b);
    ID_RE.lastIndex = 0;
    let m;
    while ((m = ID_RE.exec(seg))) out.push({ name: m[0], pos: a + m.index });
    return out;
  };

  // Assignments, and which of them hold a request's path (in their own function's scope).
  const assigns = [];
  let m;
  ASSIGN_RE.lastIndex = 0;
  while ((m = ASSIGN_RE.exec(code))) {
    let a = m.index + m[0].length;
    while (a < code.length && /\s/.test(code[a])) a++;
    const b = exprEnd(code, a);
    const rhs = code.slice(a, b);
    const isFn = /^(async\b|function\b)/.test(rhs) || /^(\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/.test(rhs);
    assigns.push({ name: m[1], pos: m.index, a, b, isFn, fn: innermost(m.index) });
  }
  const reqVars = [];
  const reqVarAt = (name, pos) => reqVars.find((v) => v.name === name && pos >= v.scope[0] && pos < v.scope[1]);
  /** What in [a, b) names a request: a description, or null. */
  const namesRequest = (a, b) => {
    const lit = stringsIn(a, b).find((s) => WR.isRequestFile(s.value));
    if (lit) return JSON.stringify(lit.value.replace(/\$\{\}/g, "${...}"));
    for (const id of idsIn(a, b)) {
      const v = reqVarAt(id.name, id.pos);
      if (v) return `${id.name} (set to ${v.what} at line ${v.line})`;
    }
    return null;
  };
  for (let grew = true; grew; ) {
    grew = false;
    for (const as of assigns) {
      if (as.isFn || as.req) continue;
      const what = namesRequest(as.a, as.b);
      if (!what) continue;
      as.req = true;
      reqVars.push({ name: as.name, what, line: line(as.pos), scope: as.fn ? [as.fn.bodyStart, as.fn.bodyEnd] : [0, code.length] });
      grew = true;
    }
  }

  // Which of a function's parameters reach a span, directly or through a local set from one.
  const derivedCache = new Map();
  const derivedOf = (f) => {
    if (derivedCache.has(f)) return derivedCache.get(f);
    const d = new Map();
    derivedCache.set(f, d);
    const mine = assigns.filter((as) => as.pos >= f.bodyStart && as.pos < f.bodyEnd && !as.isFn);
    for (let grew = true; grew; ) {
      grew = false;
      for (const as of mine) {
        if (d.has(as.name) || f.params.includes(as.name)) continue;
        const idx = paramsReaching(f, as.a, as.b, d);
        if (idx.size) {
          d.set(as.name, idx);
          grew = true;
        }
      }
    }
    return d;
  };
  function paramsReaching(f, a, b, d = derivedOf(f)) {
    const idx = new Set();
    for (const id of idsIn(a, b)) {
      const k = f.params.indexOf(id.name);
      if (k >= 0) idx.add(k);
      const via = d.get(id.name);
      if (via) for (const x of via) idx.add(x);
    }
    return idx;
  }

  const violations = new Map(); // pos -> violation
  let exempt = 0;
  const flag = (pos, why) => {
    if (violations.has(pos)) return;
    const ln = line(pos);
    const text = (srcLines[ln - 1] || "").trim();
    const ex = text.match(EXEMPT_RE);
    if (ex && ex[1].trim().length >= 10) {
      exempt++;
      return;
    }
    violations.set(pos, {
      file: rel,
      line: ln,
      text: text.length > 90 ? `${text.slice(0, 87)}...` : text,
      why: ex ? `${why} (its request-write-exempt has no reason: say why, ten characters at least)` : why,
    });
  };

  // Plain writes: straight onto a request, or onto a path built from a parameter (a wrapper).
  const wrappers = new Map();
  const addWrapper = (f, idx, via) => {
    const w = wrappers.get(f.name);
    if (!w) {
      wrappers.set(f.name, { name: f.name, idx: new Set(idx), via, file: rel, line: line(f.start) });
      return true;
    }
    let grew = false;
    for (const k of idx) if (!w.idx.has(k)) {
      w.idx.add(k);
      grew = true;
    }
    return grew;
  };
  WRITE_RE.lastIndex = 0;
  while ((m = WRITE_RE.exec(code))) {
    const fnName = m[1];
    const open = m.index + m[0].length - 1;
    const before = code.slice(Math.max(0, m.index - 20), m.index);
    if (fnName === "open" && !/\b(?:fs|fsp|promises)\s*\.\s*$/.test(before)) continue;
    if (isDefinition(code, m.index, open)) continue;
    const { args } = argsOf(code, open);
    if (fnName === "open" || fnName === "openSync") {
      const flags = args[1];
      if (!flags || !stringsIn(flags[0], flags[1]).some((s) => /[wa+]/.test(s.value))) continue;
    }
    const dest = args[/^(copyFileSync|copyFile|cpSync)$/.test(fnName) ? 1 : 0];
    if (!dest) continue;
    const what = namesRequest(dest[0], dest[1]);
    if (what) {
      flag(m.index, `${fnName} straight onto ${what}`);
      continue;
    }
    const f = innermost(m.index);
    if (f && f.name) {
      const idx = paramsReaching(f, dest[0], dest[1]);
      if (idx.size) addWrapper(f, idx, `${fnName} at line ${line(m.index)}`);
    }
  }

  // Calls to wrappers, here or under scripts/lib/; a function handing its parameter on to a
  // wrapper is one too.
  const wrapperFor = (name) => wrappers.get(name) || (!named.has(name) ? lib.get(name) : undefined);
  for (let grew = true; grew; ) {
    grew = false;
    CALL_RE.lastIndex = 0;
    while ((m = CALL_RE.exec(code))) {
      const w = wrapperFor(m[1]);
      if (!w) continue;
      const open = m.index + m[0].length - 1;
      if (isDefinition(code, m.index, open)) continue;
      const { args } = argsOf(code, open);
      const f = innermost(m.index);
      for (const k of w.idx) {
        const arg = args[k];
        if (!arg) continue;
        const what = namesRequest(arg[0], arg[1]);
        if (what) flag(m.index, `${w.name} (${w.file}:${w.line}) writes its file with ${w.via}`);
        else if (f && f.name && f.name !== w.name) {
          const idx = paramsReaching(f, arg[0], arg[1]);
          if (idx.size && addWrapper(f, idx, `${w.name}, which writes with ${w.via}`)) grew = true;
        }
      }
    }
  }

  let writeRequestCalls = 0;
  CALL_RE.lastIndex = 0;
  while ((m = CALL_RE.exec(code))) if (m[1] === "writeRequest" && !isDefinition(code, m.index, m.index + m[0].length - 1)) writeRequestCalls++;

  return { violations: [...violations.values()].sort((a, b) => a.line - b.line), wrappers, exempt, writeRequestCalls };
}

function scriptFiles(dir = SCRIPTS) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name !== "node_modules") out.push(...scriptFiles(p));
    } else if (/\.(c|m)?js$/.test(e.name) && path.resolve(p) !== path.resolve(HELPER)) out.push(p);
  }
  return out.sort();
}

const relOf = (p) => path.relative(REPO, p).replace(/\\/g, "/");

/** The whole scan: every file under scripts/, the lib's wrappers first. */
function scanScripts() {
  const files = scriptFiles();
  const libDir = path.join(SCRIPTS, "lib") + path.sep;
  const lib = new Map();
  for (const p of files.filter((p) => p.startsWith(libDir))) {
    for (const [name, w] of analyse(relOf(p), fs.readFileSync(p, "utf8")).wrappers) if (!lib.has(name)) lib.set(name, w);
  }
  const perFile = new Map();
  const violations = [];
  for (const p of files) {
    const src = fs.readFileSync(p, "utf8");
    // A file that never spells a request name cannot write one (a request name in a literal,
    // a template or a variable's value all carry it): skip the full read, most files are web tests.
    if (!/_request|record_cancel/.test(src)) continue;
    const r = analyse(relOf(p), src, lib);
    perFile.set(relOf(p), r);
    violations.push(...r.violations);
  }
  return { files, violations, perFile, lib };
}

function report(violations) {
  const files = new Set(violations.map((v) => v.file));
  return (
    `${violations.length} place${violations.length === 1 ? "" : "s"} in ${files.size} script${files.size === 1 ? "" : "s"} ` +
    `write a game request file without scripts/lib/write-request.js, so the game can read one half-written, delete it, and lose it:\n` +
    violations.map((v) => `  ${v.file}:${v.line}  ${v.text}\n      -> ${v.why}`).join("\n") +
    `\n\nWhy it matters: the game reads each request the frame it appears and deletes it. A plain write creates the file\n` +
    `empty and then fills it; a frame in between reads nothing and the request is lost with no error (verify-falls lost\n` +
    `its \`illness\` request this way on 2026-10-20, and a Forgiving leg ran in Realistic).\n` +
    `Fix: write it through the shared helper, which writes a side file and renames it into place:\n` +
    `    const { writeRequest } = require("./lib/write-request.js");\n` +
    `    writeRequest(path.join(DEBUG, "showcase_request.json"), body);\n` +
    `  (a rig's own \`req(name, body)\` can stay, as long as its body is that one call). Then: just rig-tests`
  );
}

// ── The scanner on made-up code: it must catch each way a rig has written a request, and pass
// each way that is not a write. Without these the scan below could pass by seeing nothing. ──────

const BAD = {
  "a plain write straight onto a request": [
    'fs.writeFileSync(path.join(DEBUG, "autopilot_request.json"), JSON.stringify({}));',
  ],
  "a wrapper like every rig's old req()": [
    "function req(name, body) {",
    "  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));",
    "}",
    'req("showcase_request.json", { time: "12" });',
  ],
  "a variable set from a request name": ['const ap = path.join(DEBUG, "camera_request.json");', 'fs.writeFileSync(ap, "{}");'],
  "an arrow wrapper and the recorder's stop file": [
    "const drop = (n, b) => fs.writeFileSync(path.join(DEBUG, n), JSON.stringify(b));",
    'drop("record_cancel.json", {});',
  ],
  "a destructured writeFileSync": ['const { writeFileSync } = require("fs");', 'writeFileSync(path.join(D, "screen_request.json"), "{}");'],
  "a function handing its parameter on to a wrapper": [
    "function req(n, b) { fs.writeFileSync(path.join(D, n), JSON.stringify(b)); }",
    "function send(name, body) { req(name, body); }",
    'send("ui_request.json", { find: "Settings" });',
  ],
  "a template literal name": ["fs.writeFileSync(`${DEBUG}/${kind}_request.json`, '{}');"],
  "a copy onto a request": ['fs.copyFileSync(saved, path.join(DEBUG, "showcase_request.json"));'],
  "an append, a stream, and open with a write flag": [
    'fs.appendFileSync(path.join(D, "door_points_request.json"), "{}");',
    'const s = fs.createWriteStream(path.join(D, "screenshot_request.json"));',
    'const fd = fs.openSync(path.join(D, "cloudmap_request.json"), "w");',
  ],
  "a hand-rolled side file and rename (a second writer)": [
    "function req(name, body) {",
    "  const p = path.join(DEBUG, name);",
    "  fs.writeFileSync(`${p}.part`, JSON.stringify(body));",
    "  fs.renameSync(`${p}.part`, p);",
    "}",
    'req("showcase_request.json", {});',
  ],
  "a write after a regex with quotes in it (the lexer recovers)": [
    "if (/[\"'`]/.test(x) || /_request\\.json$/.test(f)) fs.unlinkSync(f);",
    'fs.writeFileSync(path.join(D, "remote_players_request.json"), "{}");',
  ],
  "an exemption with no reason": ['fs.writeFileSync(path.join(D, "camera_request.json"), "{}"); // request-write-exempt: ok'],
};
const BAD_COUNTS = { "an append, a stream, and open with a write flag": 3 };

const GOOD = {
  "writeRequest itself, bare and through a module": [
    'writeRequest(path.join(DEBUG, "showcase_request.json"), {});',
    'WR.writeRequest(path.join(DEBUG, "camera_request.json"), { station: "home" });',
  ],
  "a rig's req() whose body is writeRequest": [
    "function req(name, body) {",
    "  writeRequest(path.join(DEBUG, name), body);",
    "}",
    'req("showcase_request.json", {});',
  ],
  "reading, waiting on and deleting a request": [
    'const p = path.join(DEBUG, "showcase_request.json");',
    "while (fs.existsSync(p)) await sleep(50);",
    "fs.unlinkSync(p); fs.rmSync(p, { force: true }); JSON.parse(fs.readFileSync(p, 'utf8'));",
    'const fd = fs.openSync(path.join(DEBUG, "screen_request.json"), "r");',
  ],
  "a comment and a log line that name a request": [
    '// fs.writeFileSync(path.join(DEBUG, "showcase_request.json"), "{}")',
    "/* req(\"camera_request.json\", {}) */",
    'log(`dropped showcase_request.json; fs.writeFileSync("x_request.json")`);',
  ],
  "a done file and a manifest": [
    'fs.writeFileSync(path.join(DEBUG, "camera_done.json"), "{}");',
    'fs.writeFileSync(path.join(DEBUG, "ui_request_done.json"), "{}");',
    'fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(m, null, 2));',
  ],
  "the same variable name holding a request in one function and a manifest in another": [
    'function a() { const p = path.join(D, "showcase_request.json"); return fs.existsSync(p); }',
    'function b() { const p = path.join(OUT, "manifest.json"); fs.writeFileSync(p, "{}"); }',
  ],
  "a generic writer called with a non-request": [
    "function save(file, obj) { fs.writeFileSync(file, JSON.stringify(obj)); }",
    'save(path.join(OUT, "manifest.json"), m);',
  ],
  "a line exempt with its reason": [
    'fs.writeFileSync(path.join(D, "camera_request.json"), "{}"); // request-write-exempt: a fixture for a test of the game\'s reader',
  ],
};

test("the scanner catches each way a request has been written, and passes what is not a write", () => {
  for (const [name, lines] of Object.entries(BAD)) {
    const r = analyse(`bad:${name}`, lines.join("\n"));
    const want = BAD_COUNTS[name] || 1;
    assert.strictEqual(r.violations.length, want, `"${name}" should be flagged ${want} time(s), was ${r.violations.length}:\n${lines.join("\n")}`);
  }
  for (const [name, lines] of Object.entries(GOOD)) {
    const r = analyse(`good:${name}`, lines.join("\n"));
    assert.deepStrictEqual(r.violations, [], `"${name}" is not a plain request write:\n${lines.join("\n")}\n${report(r.violations)}`);
  }
  assert.strictEqual(analyse("x", GOOD["a line exempt with its reason"].join("\n")).exempt, 1, "the exemption was counted");
  // A wrapper under scripts/lib/ is followed into the rig that calls it.
  const lib = analyse("scripts/lib/drop.js", "function dropFile(p, b) { fs.writeFileSync(p, JSON.stringify(b)); }\nmodule.exports = { dropFile };").wrappers;
  const r = analyse("rig", 'const L = require("./lib/drop.js");\nL.dropFile(path.join(DEBUG, "showcase_request.json"), {});', lib);
  assert.strictEqual(r.violations.length, 1, "a lib wrapper called from a rig");
  assert.match(r.violations[0].why, /dropFile \(scripts\/lib\/drop\.js:1\) writes its file with writeFileSync/);
});

// The rigs that drive the game through request files. Each must write them through writeRequest
// (the scan below proves there is no other write; this proves the rigs still write at all, so a
// rig rewritten some new way the scanner cannot read does not pass by going quiet).
const RIGS = [
  "verify-falls.js",
  "probe-sweep.js",
  "verify-copresence.js",
  "verify-screens.js",
  "verify-live-screen.js",
  "make-clips.js",
  "photograph-home.js",
  "probe-hot-ab.js",
  "boot-timing.js",
];

test("no script under scripts/ writes a game request file except through writeRequest", () => {
  const { files, violations, perFile } = scanScripts();
  assert.ok(files.length > 100, `only ${files.length} scripts found under scripts/: is the walk broken?`);
  if (violations.length) assert.fail(report(violations));
  for (const rig of RIGS) {
    const r = perFile.get(`scripts/${rig}`);
    assert.ok(r, `scripts/${rig} is gone: take it out of RIGS here if it was retired on purpose`);
    assert.ok(r.writeRequestCalls > 0, `scripts/${rig} no longer calls writeRequest: how does it write its requests now?`);
  }
});

test("the rigs sweep a left-behind side file up with the requests", () => {
  // A rig killed between the write and the rename leaves `<request>.<pid>-<n>.part`. Harmless (the
  // game never reads one) but it would pile up; each rig that clears its debug/ folder at setup
  // clears these too, through the helper's own test.
  for (const rig of RIGS) {
    const src = fs.readFileSync(path.join(SCRIPTS, rig), "utf8");
    if (!/_request\\?\.json/.test(src) || !/readdirSync\(DEBUG\)/.test(src)) continue;
    assert.ok(/isRequestLeftover\(/.test(src), `scripts/${rig} clears debug/ at setup but not through WR.isRequestLeftover`);
  }
});

test("every request the game polls is named the way the helper and this scan know", () => {
  const found = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith(".rs")) {
        const src = fs.readFileSync(p, "utf8");
        const re = /const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"debug\/([^"]+)"/g;
        let m;
        while ((m = re.exec(src))) if (/REQ|CANCEL/.test(m[1])) found.push({ file: relOf(p), constant: m[1], name: m[2] });
      }
    }
  };
  walk(path.join(REPO, "src"));
  assert.ok(found.length >= 12, `only ${found.length} request constants found under src/: did the game's naming change?`);
  const odd = found.filter((f) => !WR.isRequestFile(f.name));
  assert.deepStrictEqual(
    odd,
    [],
    `the game polls request files this scan would not recognise, so a plain write of one would pass unseen:\n` +
      odd.map((f) => `  ${f.file}: ${f.constant} = "debug/${f.name}"`).join("\n") +
      `\nAdd the name to REQUEST_NAME_RE in scripts/lib/write-request.js (and its test), or name the file "<what>_request.json".`
  );
});

module.exports = { lex, analyse, scanScripts };
