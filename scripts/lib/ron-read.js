// A reader for the RON the game's data files use (data/music/*.ron and the like), for scripts that
// turn that data into JSON for the website. It reads: Name(...) and (...) structs (field: value),
// anonymous tuples (a, b, c) as arrays, [lists], "strings" with escapes, numbers, true/false,
// Some(x) as x and None as null, bare identifiers (enum variants) as their names, // and /* */
// comments, and trailing commas. Not the whole of RON (no maps, chars or raw strings): what the
// data uses. A file it cannot read throws with the place it stopped.
'use strict';

function parseRon(text) {
  let i = 0;
  const ws = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text.startsWith('//', i)) {
        while (i < text.length && text[i] !== '\n') i++;
      } else if (text.startsWith('/*', i)) {
        const end = text.indexOf('*/', i + 2);
        i = end < 0 ? text.length : end + 2;
      } else return;
    }
  };
  const err = (m) => {
    throw new Error(`${m} at ${i}: ${JSON.stringify(text.slice(i, i + 40))}`);
  };
  const ident = () => {
    const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(text.slice(i, i + 200));
    return m ? m[0] : null;
  };
  const str = () => {
    let j = i + 1;
    let s = '';
    while (j < text.length && text[j] !== '"') {
      if (text[j] === '\\') {
        const e = text[j + 1];
        if (e === 'u' && text[j + 2] === '{') {
          const close = text.indexOf('}', j);
          s += String.fromCodePoint(parseInt(text.slice(j + 3, close), 16));
          j = close + 1;
          continue;
        }
        s += { n: '\n', t: '\t', r: '\r', '"': '"', '\\': '\\', '0': '\0' }[e] ?? e;
        j += 2;
      } else s += text[j++];
    }
    if (j >= text.length) err('an unclosed string');
    i = j + 1;
    return s;
  };
  // After "(": a struct when the next thing is `name:`, else a tuple (an array).
  const group = () => {
    i++;
    ws();
    const save = i;
    const k = ident();
    if (k) {
      i += k.length;
      ws();
      if (text[i] === ':' && text[i + 1] !== ':') {
        i = save;
        return struct();
      }
    }
    i = save;
    const out = [];
    for (;;) {
      ws();
      if (text[i] === ')') {
        i++;
        return out;
      }
      out.push(value());
      ws();
      if (text[i] === ',') i++;
      else if (text[i] !== ')') err('a comma or a closing bracket');
    }
  };
  const struct = () => {
    const out = {};
    for (;;) {
      ws();
      if (text[i] === ')') {
        i++;
        return out;
      }
      const k = ident();
      if (!k) err('a field name');
      i += k.length;
      ws();
      if (text[i] !== ':') err('a colon');
      i++;
      out[k] = value();
      ws();
      if (text[i] === ',') i++;
      else if (text[i] !== ')') err('a comma or a closing bracket');
    }
  };
  const value = () => {
    ws();
    const c = text[i];
    if (c === '"') return str();
    if (c === '[') {
      i++;
      const out = [];
      for (;;) {
        ws();
        if (text[i] === ']') {
          i++;
          return out;
        }
        out.push(value());
        ws();
        if (text[i] === ',') i++;
        else if (text[i] !== ']') err('a comma or a closing bracket');
      }
    }
    if (c === '(') return group();
    const m = /^[-+]?(\d+(\.\d*)?|\.\d+)([eE][-+]?\d+)?/.exec(text.slice(i, i + 60));
    if (m) {
      i += m[0].length;
      return Number(m[0]);
    }
    const w = ident();
    if (!w) err('a value');
    i += w.length;
    if (w === 'true') return true;
    if (w === 'false') return false;
    if (w === 'None') return null;
    ws();
    if (text[i] === '(') {
      if (w === 'Some') {
        i++;
        const v = value();
        ws();
        if (text[i] !== ')') err('the end of Some(...)');
        i++;
        return v;
      }
      return group();
    }
    return w;
  };
  const v = value();
  ws();
  if (i < text.length) err('text after the value');
  return v;
}

module.exports = { parseRon };
