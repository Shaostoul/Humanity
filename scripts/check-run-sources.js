#!/usr/bin/env node
// Every row of the run catalogs names where its numbers come from (spline routing rung 0,
// 2026-11-02, docs/design/spline-routing.md sections 5.3 and 7.1).
//
// The catalogs: data/piping/pipe_materials.ron, run_rules.ron, fittings.ron, anchors.ron and
// benders.ron. A row's `source` is a list of R(n) (a finding of
// docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md), D(n) (of
// 2026-11-01-manual-door-releases.md) or Ours("why"). This check fails on:
//   - a row with no `source`, or an empty one;
//   - an Ours whose reason is under 12 characters ("ours" alone says nothing);
//   - an R(n) or D(n) the document does not define. The ids are read from the documents' own
//     numbered paragraphs (`**13. Cable supports in a manufactured home ...**`), never listed here,
//     so a finding added to a document is known at once and a typo or a renumbering is caught.
//
// WHY A NODE CHECK AS WELL AS THE RUST TESTS. The typed loaders' tests (`*_cite_real_findings`,
// `shipped_*_registry_parses_and_is_sound`, in `just validate-data`) hold the same rules and also
// refuse a misspelt field; but CI runs no lib tests (its verify workflow checks the relay build and
// runs std-only lints and node checks), so this is the half that runs there, and in `just lints`.
//
// WHAT A ROW IS, by its shape rather than by a list of files: any struct with an `id` field (a
// material, a size, a link, a profile, a rule, a fitting, an anchor, a bender), except a vocabulary
// row whose only fields are `id` and `meaning` (the declared tags and equipment); and any struct
// with `method` and `min_m` (a size's bend radius). Rows inside rows count too. So a new catalog
// file or a new kind of row is checked without editing this script; a mapping row with no id (the
// `kinds` rows, a bender's per-size radius) carries its parent's source.
//
// Usage: node scripts/check-run-sources.js [--root DIR] [file.ron ...]
//   With no files: the five catalogs under data/piping/. --root: the checkout (tests point it at a
//   fixture tree). Exit 0 with ">> lint ok: ..." or exit 1 naming each problem as file:line.
// The test: scripts/tests/run-sources.test.js (in `just rig-tests`).

'use strict';
const fs = require('fs');
const path = require('path');

const CATALOGS = ['pipe_materials', 'run_rules', 'fittings', 'anchors', 'benders'].map((f) => `data/piping/${f}.ron`);
const DOCS = {
  R: 'docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md',
  D: 'docs/reference/findings/2026-11-01-manual-door-releases.md',
};
const OURS_REASON_MIN_CHARS = 12;

// ── The findings documents' numbered paragraphs ─────────────────────────────────────────────────
function findingNumbers(markdown) {
  const out = new Set();
  for (const line of markdown.split(/\r?\n/)) {
    const m = /^\s*\*\*(\d+)\./.exec(line);
    if (m) out.add(Number(m[1]));
  }
  return out;
}

// ── A small RON reader ──────────────────────────────────────────────────────────────────────────
// Values: {t:'struct', name, fields: Map, line} for `Name(k: v, ...)` or `(k: v)`;
// {t:'tuple', name, items, line} for `Name(a, b)` or `(a, b)`; {t:'list', items}; {t:'map'};
// {t:'str', v}; {t:'num', v}; {t:'ident', v}.
function parseRon(text, file) {
  let i = 0;
  const n = text.length;
  const lineAt = (pos) => text.slice(0, pos).split('\n').length;
  const fail = (msg) => {
    throw new Error(`${file}:${lineAt(i)}: ${msg}`);
  };
  const skip = () => {
    for (;;) {
      while (i < n && /\s/.test(text[i])) i++;
      if (text.startsWith('//', i)) {
        while (i < n && text[i] !== '\n') i++;
      } else if (text.startsWith('/*', i)) {
        const end = text.indexOf('*/', i + 2);
        i = end < 0 ? n : end + 2;
      } else if (text.startsWith('#![', i)) {
        while (i < n && text[i] !== '\n') i++;
      } else return;
    }
  };
  const ident = () => {
    const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(text.slice(i, i + 200));
    if (!m) fail(`expected a name, found ${JSON.stringify(text.slice(i, i + 12))}`);
    i += m[0].length;
    return m[0];
  };
  const str = () => {
    i++; // opening quote
    let s = '';
    while (i < n && text[i] !== '"') {
      if (text[i] === '\\') {
        s += text[i + 1];
        i += 2;
      } else s += text[i++];
    }
    if (i >= n) fail('a string that never ends');
    i++;
    return { t: 'str', v: s };
  };
  // After '(' : a struct when the first thing is `name:` (not `name::`), else a tuple.
  const parenBody = (name, line) => {
    skip();
    const save = i;
    let isStruct = false;
    if (/[A-Za-z_]/.test(text[i] || '')) {
      ident();
      skip();
      isStruct = text[i] === ':' && text[i + 1] !== ':';
    }
    i = save;
    if (isStruct) {
      const fields = new Map();
      for (;;) {
        skip();
        if (text[i] === ')') {
          i++;
          break;
        }
        const key = ident();
        skip();
        if (text[i] !== ':') fail(`expected ':' after ${key}`);
        i++;
        if (fields.has(key)) fail(`field ${key} given twice`);
        fields.set(key, value());
        skip();
        if (text[i] === ',') i++;
        else if (text[i] !== ')') fail(`expected ',' or ')' after field ${key}`);
      }
      return { t: 'struct', name, fields, line };
    }
    const items = [];
    for (;;) {
      skip();
      if (text[i] === ')') {
        i++;
        break;
      }
      items.push(value());
      skip();
      if (text[i] === ',') i++;
      else if (text[i] !== ')') fail("expected ',' or ')' in a tuple");
    }
    return { t: 'tuple', name, items, line };
  };
  const value = () => {
    skip();
    const c = text[i];
    const line = lineAt(i);
    if (c === '(') {
      i++;
      return parenBody(null, line);
    }
    if (c === '[') {
      i++;
      const items = [];
      for (;;) {
        skip();
        if (text[i] === ']') {
          i++;
          break;
        }
        items.push(value());
        skip();
        if (text[i] === ',') i++;
        else if (text[i] !== ']') fail("expected ',' or ']' in a list");
      }
      return { t: 'list', items, line };
    }
    if (c === '{') {
      i++;
      const entries = [];
      for (;;) {
        skip();
        if (text[i] === '}') {
          i++;
          break;
        }
        const k = value();
        skip();
        if (text[i] !== ':') fail("expected ':' in a map");
        i++;
        entries.push([k, value()]);
        skip();
        if (text[i] === ',') i++;
        else if (text[i] !== '}') fail("expected ',' or '}' in a map");
      }
      return { t: 'map', entries, line };
    }
    if (c === '"') return str();
    if (/[-+0-9.]/.test(c || '')) {
      const m = /^[-+]?[0-9][0-9_]*(\.[0-9_]*)?([eE][-+]?[0-9]+)?|^[-+]?\.[0-9]+/.exec(text.slice(i, i + 64));
      if (!m) fail('a malformed number');
      i += m[0].length;
      return { t: 'num', v: Number(m[0].replace(/_/g, '')) };
    }
    const name = ident();
    skip();
    if (text[i] === '(') {
      i++;
      return parenBody(name, line);
    }
    return { t: 'ident', v: name };
  };
  const root = value();
  skip();
  if (i < n) fail('text after the end of the value');
  return root;
}

// ── The check ───────────────────────────────────────────────────────────────────────────────────
function rowName(s) {
  const id = s.fields.get('id');
  if (id && id.t === 'str') return `row \`${id.v}\``;
  const method = s.fields.get('method');
  if (method) return `a ${method.v || method.name || ''} bend radius`;
  return 'a row';
}

function isRow(s) {
  if (s.t !== 'struct') return false;
  const keys = [...s.fields.keys()];
  if (keys.includes('id')) return !keys.every((k) => k === 'id' || k === 'meaning');
  return keys.includes('method') && keys.includes('min_m');
}

function checkSources(file, root, findings) {
  const problems = [];
  let rows = 0;
  const visit = (v) => {
    if (!v || typeof v !== 'object') return;
    if (v.t === 'struct') {
      if (isRow(v)) {
        rows++;
        const where = `${file}:${v.line}: ${rowName(v)}`;
        const src = v.fields.get('source');
        if (!src) problems.push(`${where} has no source: name a finding (R1.., D1..) or give Ours("why")`);
        else if (src.t !== 'list') problems.push(`${where}: its source is not a list`);
        else if (src.items.length === 0) problems.push(`${where} has an empty source: name a finding (R1.., D1..) or give Ours("why")`);
        else {
          for (const s of src.items) {
            if (s.t === 'tuple' && (s.name === 'R' || s.name === 'D') && s.items.length === 1 && s.items[0].t === 'num') {
              const num = s.items[0].v;
              if (!findings[s.name].has(num)) problems.push(`${where} cites ${s.name}${num}, which ${DOCS[s.name]} does not define`);
            } else if (s.t === 'tuple' && s.name === 'Ours' && s.items.length === 1 && s.items[0].t === 'str') {
              if (s.items[0].v.trim().length < OURS_REASON_MIN_CHARS) problems.push(`${where}: Ours("${s.items[0].v}") does not say why the number is ours`);
            } else {
              problems.push(`${where}: a source that is not R(n), D(n) or Ours("why")`);
            }
          }
        }
      }
      for (const f of v.fields.values()) visit(f);
    } else if (v.t === 'tuple' || v.t === 'list') v.items.forEach(visit);
    else if (v.t === 'map') v.entries.forEach(([k, val]) => {
      visit(k);
      visit(val);
    });
  };
  visit(root);
  return { problems, rows };
}

function main(argv) {
  let root = path.resolve(__dirname, '..');
  const files = [];
  for (let a = 0; a < argv.length; a++) {
    if (argv[a] === '--root') root = path.resolve(argv[++a]);
    else files.push(argv[a]);
  }
  const findings = {};
  for (const [letter, doc] of Object.entries(DOCS)) {
    findings[letter] = findingNumbers(fs.readFileSync(path.join(root, doc), 'utf8'));
    if (findings[letter].size === 0) {
      console.error(`LINT FAILED: run sources: ${doc} defines no numbered findings (has its form changed?)`);
      return 1;
    }
  }
  const problems = [];
  let rows = 0;
  const list = files.length ? files : CATALOGS;
  for (const rel of list) {
    let tree;
    try {
      tree = parseRon(fs.readFileSync(path.join(root, rel), 'utf8').replace(/\r\n/g, '\n'), rel);
    } catch (e) {
      problems.push(`${e.message} (the file does not read as RON)`);
      continue;
    }
    const r = checkSources(rel, tree, findings);
    rows += r.rows;
    problems.push(...r.problems);
  }
  if (problems.length) {
    console.error(`LINT FAILED: run sources: ${problems.length} problem(s)`);
    for (const p of problems) console.error('  ' + p);
    return 1;
  }
  console.log(`>> lint ok: run sources (${rows} rows in ${list.length} catalog file(s), each citing a finding the documents define (R ${findings.R.size}, D ${findings.D.size}) or a reason of its own)`);
  return 0;
}

module.exports = { findingNumbers, parseRon, checkSources, isRow };

if (require.main === module) process.exit(main(process.argv.slice(2)));
