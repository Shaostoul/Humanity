#!/usr/bin/env node
'use strict';
/**
 * check-ai-catalog.js (in `just preflight`; docs/design/embeddable-ai.md 12.3, rung 2, 2026-11-09):
 * the AI catalog the app compiles in (data/ai/catalog.ron) is whole and honest, WITHOUT needing the
 * gigabyte files (they live on the release, not in git):
 *
 * - it is exactly what scripts/ai-catalog.js writes from its recipe (re-rendered from its own values,
 *   byte for byte), so a hand edit fails; and it lists exactly the recipe's models, with their files,
 *   choices, licences and publisher hashes;
 * - every file has a SHA-256 and a size; a file over the recipe's part_bytes (2,000,000,000, under
 *   GitHub's 2 GiB) is in parts that add up to it, each at most that size with its own hash; a file of
 *   that size or smaller is not split; the app's own copy of the limit (src/ai/catalog.rs PART_LIMIT)
 *   is the recipe's; a model taken unchanged from its publisher has the publisher's hash;
 * - every licence key has its words in data/i18n/en.json, every licence text is in data/, every
 *   credit id is a row of data/credits.ron, every finding exists, every choice is in
 *   data/ai/choices.ron;
 * - each small extra (a licence text, a credit README) has the size and hash of what the recipe makes
 *   it from (those ARE in git), so a changed licence file cannot slip past its compiled-in hash;
 * - the engine's version is the engine crate's and the app's (src/ai/paths.rs);
 * - every voice in data/ai/voices.ron (rung 5) has a source, a set and a licence, none from a forbidden
 *   set; until that file exists there is nothing to check, and it says so.
 *
 *   node scripts/check-ai-catalog.js [catalog.ron]     exit 1 with every problem listed
 *
 * SEEN RED 2026-11-09 before it went into `just preflight`, on a scratch copy of the catalog with
 * Granite's sha256 changed by one digit, before the credit rows and the licence words existed:
 *   FAILED check-ai-catalog: 6 problem(s) in ...ai2-bad-catalog.ron:
 *     - the engine: the licence key ai.licence.engine has no words in data/i18n/en.json
 *     - the engine: the credit llama_cpp is not a row of data/credits.ron
 *     - the engine: the credit llama_cpp_rs is not a row of data/credits.ron
 *     - granite-4-0-1b: the licence key ai.licence.apache2 has no words in data/i18n/en.json
 *     - granite-4-0-1b: the credit granite_4_0_1b is not a row of data/credits.ron
 *     - granite-4-0-1b: its SHA-256 is not its publisher's (22ec0f9c...31d7f1), though it is published unchanged
 * (a hand edit that changes only the layout is the first rule's: the catalog re-rendered from its own
 * values no longer matches the file).
 *
 * SEEN RED 2026-11-11 with Ministral 3 3B's row (two parts), on scratch copies written by
 * catalogRon: with its parts dropped, "ministral-3-3b: 2147023008 bytes is over 2000000000, so it
 * must come in parts"; with its second part one byte short, "ministral-3-3b: its parts add up to
 * 2147023007, not 2147023008"; and before its credit row existed, "ministral-3-3b: the credit
 * ministral_3_3b is not a row of data/credits.ron". The real catalog: no problems.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { catalogRon, TWO_GIB } = require('./ai-catalog');

const ROOT = path.resolve(__dirname, '..');

/** The RON subset ai-catalog.js writes: Name(...), (...), [...], field: value, strings, numbers,
 * true/false, Some(x)/None, // comments, trailing commas. Returns plain JS values. */
function parseRon(text) {
  let i = 0;
  const ws = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text.startsWith('//', i)) {
        while (i < text.length && text[i] !== '\n') i++;
      } else return;
    }
  };
  const err = (m) => {
    throw new Error(`${m} at ${i}: ${JSON.stringify(text.slice(i, i + 30))}`);
  };
  const value = () => {
    ws();
    const c = text[i];
    if (c === '"') {
      let j = i + 1;
      let s = '';
      while (j < text.length && text[j] !== '"') {
        if (text[j] === '\\') {
          s += JSON.parse(`"${text.slice(j, j + 2) === '\\u' ? text.slice(j, j + 6) : text.slice(j, j + 2)}"`);
          j += text.slice(j, j + 2) === '\\u' ? 6 : 2;
        } else s += text[j++];
      }
      i = j + 1;
      return s;
    }
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
      }
    }
    if (c === '(') return struct();
    const m = /^-?\d+(\.\d+)?/.exec(text.slice(i));
    if (m) {
      i += m[0].length;
      return Number(m[0]);
    }
    const w = /^[A-Za-z_][A-Za-z0-9_]*/.exec(text.slice(i));
    if (!w) err('unexpected');
    i += w[0].length;
    if (w[0] === 'true') return true;
    if (w[0] === 'false') return false;
    if (w[0] === 'None') return null;
    ws();
    if (text[i] === '(') {
      if (w[0] === 'Some') {
        i++;
        const v = value();
        ws();
        i++;
        return v;
      }
      return struct();
    }
    return w[0];
  };
  const struct = () => {
    i++; // (
    const out = {};
    for (;;) {
      ws();
      if (text[i] === ')') {
        i++;
        return out;
      }
      const k = /^[A-Za-z_][A-Za-z0-9_]*/.exec(text.slice(i));
      if (!k) err('a field name');
      i += k[0].length;
      ws();
      if (text[i] !== ':') err('a colon');
      i++;
      out[k[0]] = value();
      ws();
      if (text[i] === ',') i++;
    }
  };
  const v = value();
  ws();
  if (i < text.length) err('trailing text');
  return v;
}

const isHash = (s) => typeof s === 'string' && /^[0-9a-f]{64}$/.test(s);
const plain = (s) => typeof s === 'string' && s.length > 0 && !/[\\/:]/.test(s) && !s.startsWith('.');

function enWords() {
  const j = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));
  return (key) => key.split('.').reduce((o, k) => (o && typeof o === 'object' ? o[k] : undefined), j);
}

/** Every problem with `catalogText`, given the repository at `root` (pure but for reads under root). */
function problems(catalogText, root = ROOT) {
  const out = [];
  let cat;
  try {
    cat = parseRon(catalogText);
  } catch (e) {
    return [`data/ai/catalog.ron does not read: ${e.message}`];
  }
  const recipe = JSON.parse(fs.readFileSync(path.join(root, 'scripts', 'ai-catalog-recipe.json'), 'utf8'));
  const words = enWords();
  const credits = fs.readFileSync(path.join(root, 'data', 'credits.ron'), 'utf8');
  const creditIds = new Set([...credits.matchAll(/^\s*id:\s*"([^"]+)"/gm)].map((m) => m[1]));
  const choices = fs.readFileSync(path.join(root, 'data', 'ai', 'choices.ron'), 'utf8');
  const choiceIds = new Set([...choices.matchAll(/^\s*id:\s*"([^"]+)"/gm)].map((m) => m[1]));

  // Exactly what the script writes from these values.
  const rendered = catalogRon(cat);
  if (rendered.replace(/\r\n/g, '\n') !== catalogText.replace(/\r\n/g, '\n')) out.push('data/ai/catalog.ron is not what scripts/ai-catalog.js writes (a hand edit?): run `just ai-catalog`');

  // The largest file published whole (the recipe's part_bytes, under GitHub's 2 GiB), and the app's
  // own copy of the same rule (src/ai/catalog.rs PART_LIMIT): the two must agree.
  const limit = recipe.part_bytes;
  if (!(limit > 0 && limit < TWO_GIB)) out.push(`the recipe's part_bytes ${limit} must be above 0 and under 2 GiB`);
  const appLimit = Number(((/pub const PART_LIMIT: u64 = ([0-9_]+);/.exec(fs.readFileSync(path.join(root, 'src', 'ai', 'catalog.rs'), 'utf8')) || [])[1] || '').replace(/_/g, ''));
  if (appLimit !== limit) out.push(`the recipe splits files over ${limit} bytes, the app (src/ai/catalog.rs PART_LIMIT) over ${appLimit}: they must agree`);
  const file = (what, name, bytes, sha, parts) => {
    if (!plain(name)) out.push(`${what}: ${JSON.stringify(name)} is not a plain file name`);
    if (!(bytes > 0)) out.push(`${what}: no size`);
    if (!isHash(sha)) out.push(`${what}: ${JSON.stringify(sha)} is not a SHA-256`);
    if (bytes > limit && !(parts && parts.length)) out.push(`${what}: ${bytes} bytes is over ${limit}, so it must come in parts`);
    if (bytes <= limit && parts && parts.length) out.push(`${what}: ${limit} bytes or smaller but listed in parts`);
    if (parts && parts.length) {
      const sum = parts.reduce((a, p) => a + p.bytes, 0);
      if (sum !== bytes) out.push(`${what}: its parts add up to ${sum}, not ${bytes}`);
      for (const p of parts) if (!plain(p.file) || !(p.bytes > 0) || p.bytes > limit || !isHash(p.sha256)) out.push(`${what}: the part ${JSON.stringify(p.file)} needs a plain name, a size of at most ${limit} bytes and a SHA-256`);
    }
  };
  const licence = (what, key, text) => {
    if (typeof words(key) !== 'string') out.push(`${what}: the licence key ${key} has no words in data/i18n/en.json`);
    if (!fs.existsSync(path.join(root, 'data', text))) out.push(`${what}: its licence text data/${text} is missing`);
  };
  const credited = (what, ids) => {
    if (!Array.isArray(ids) || !ids.length) out.push(`${what}: no credit row named`);
    for (const id of ids || []) if (!creditIds.has(id)) out.push(`${what}: the credit ${id} is not a row of data/credits.ron`);
  };

  const e = cat.engine || {};
  file('the engine', e.file, e.bytes, e.sha256, e.parts);
  licence('the engine', e.licence, e.licence_text);
  credited('the engine', e.credits);
  const crateVersion = (/^version\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(path.join(root, 'crates', 'humanity-ai-engine', 'Cargo.toml'), 'utf8')) || [])[1];
  const appVersion = (/pub const ENGINE_VERSION: &str = "([^"]+)"/.exec(fs.readFileSync(path.join(root, 'src', 'ai', 'paths.rs'), 'utf8')) || [])[1];
  if (e.version !== crateVersion || e.version !== appVersion) out.push(`the engine is version ${e.version} in the catalog, ${crateVersion} in its Cargo.toml, ${appVersion} in src/ai/paths.rs: they must agree`);
  if (cat.base_url !== recipe.base_url || cat.release !== recipe.release) out.push('the catalog\'s release or address is not the recipe\'s');

  const ids = (cat.models || []).map((m) => m.id);
  const want = recipe.models.map((m) => m.id);
  if (JSON.stringify(ids) !== JSON.stringify(want)) out.push(`the catalog lists ${JSON.stringify(ids)}, the recipe ${JSON.stringify(want)}`);
  for (const m of cat.models || []) {
    const r = recipe.models.find((x) => x.id === m.id) || {};
    file(m.id, m.file, m.bytes, m.sha256, m.parts);
    licence(m.id, m.licence, m.licence_text);
    credited(m.id, m.credits);
    for (const k of ['choice', 'kind', 'model', 'maker', 'licence', 'licence_text', 'finding', 'upstream', 'upstream_sha256', 'file', 'context']) {
      if (JSON.stringify(m[k]) !== JSON.stringify(r[k])) out.push(`${m.id}: its ${k} is ${JSON.stringify(m[k])}, the recipe's ${JSON.stringify(r[k])}`);
    }
    if (JSON.stringify(m.credits) !== JSON.stringify(r.credits)) out.push(`${m.id}: its credits are not the recipe's`);
    if (!choiceIds.has(m.choice)) out.push(`${m.id}: the choice ${m.choice} is not in data/ai/choices.ron`);
    if (!fs.existsSync(path.join(root, m.finding))) out.push(`${m.id}: its finding ${m.finding} does not exist`);
    if (m.upstream_sha256 && !(m.parts && m.parts.length) && m.sha256 !== m.upstream_sha256) out.push(`${m.id}: its SHA-256 is not its publisher's (${m.upstream_sha256}), though it is published unchanged`);
    for (const x of m.extras || []) {
      file(`${m.id}'s ${x.file}`, x.file, x.bytes, x.sha256, []);
      if (!plain(x.install_as)) out.push(`${m.id}: ${JSON.stringify(x.install_as)} is not a plain file name`);
      const rx = (r.extras || []).find((y) => y.file === x.file);
      if (!rx) {
        out.push(`${m.id}: the extra ${x.file} is not in the recipe`);
        continue;
      }
      const data = rx.from ? fs.readFileSync(path.join(root, rx.from)) : Buffer.from(rx.text || '', 'utf8');
      const sha = crypto.createHash('sha256').update(data).digest('hex');
      if (data.length !== x.bytes || sha !== x.sha256) out.push(`${m.id}: the extra ${x.file} is ${data.length} bytes with SHA-256 ${sha} from the recipe, the catalog says ${x.bytes}, ${x.sha256}: run \`just ai-catalog\``);
    }
  }

  // The voices (rung 5): data/ai/voices.ron, when it exists.
  const voices = path.join(root, 'data', 'ai', 'voices.ron');
  if (fs.existsSync(voices)) {
    const forbidden = /expresso|\bears\b|degaulle-2|cosette|"jean"|giovanni|"lola"|juergen|rafael/i;
    const v = parseRon(fs.readFileSync(voices, 'utf8'));
    for (const x of v.voices || []) {
      if (!x.source || !x.set || !x.licence) out.push(`the voice ${x.id}: it needs a source, a set and a licence`);
      if (forbidden.test(`${x.set} ${x.source} "${x.id}"`)) out.push(`the voice ${x.id} comes from a forbidden set (embeddable-ai.md 4.5)`);
    }
  }
  return out;
}

function main(argv) {
  const file = argv[0] ? path.resolve(argv[0]) : path.join(ROOT, 'data', 'ai', 'catalog.ron');
  const list = problems(fs.readFileSync(file, 'utf8'));
  const voices = fs.existsSync(path.join(ROOT, 'data', 'ai', 'voices.ron')) ? '' : ' (no data/ai/voices.ron yet: the voices come with rung 5)';
  if (list.length) {
    console.error(`FAILED check-ai-catalog: ${list.length} problem(s) in ${path.relative(ROOT, file)}:`);
    for (const p of list) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.log(`ok check-ai-catalog: ${path.relative(ROOT, file)} is whole and matches its recipe${voices}`);
}

if (require.main === module) main(process.argv.slice(2));

module.exports = { parseRon, problems };
