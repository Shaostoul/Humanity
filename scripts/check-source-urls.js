#!/usr/bin/env node
// Checks that every URL in data/sources/registry.json still resolves to the
// thing it claims to be.
//
// WHY THIS EXISTS
// On 2026-09-15 a writer researching insulation found that the whole of DOE
// Energy Saver had been retired: energy.gov/energysaver/energy-saver returns
// 404, and energy.gov/energysaver redirects to the department homepage. Two
// shipped citations pointed at it. On the same day the FPL Wood Handbook link
// turned out to answer 200 with the FPL homepage rather than the handbook, and
// a MIL-HDBK-5J link did the same with a DLA portal page.
//
// That is the failure that matters and a status check does not catch it. A dead
// link that 404s is obvious. A dead link that answers 200 with somebody's
// homepage looks fine forever, and a reader who follows it just quietly fails to
// find the fact we said was there. So this checks three things:
//
//   1. the status is 2xx or 3xx,
//   2. a URL ending in .pdf actually returns a PDF, not HTML,
//   3. a URL that HAD a path did not end up at the bare domain root.
//
// Rule 3 is the Energy Saver case, and it is the reason this script exists
// rather than a one-line curl loop.
//
// This talks to the network, so it is NOT part of `just verify` or
// `just preflight`, both of which must work offline. Run it when you add
// sources, and periodically, because links rot whether or not anyone is looking.
//
//   node scripts/check-source-urls.js
//   node scripts/check-source-urls.js --quiet   # only the summary

const fs = require('fs');

const REGISTRY = 'data/sources/registry.json';
const QUIET = process.argv.includes('--quiet');

// Several federal sites (USDA, ATSDR, FSIS) and GBIF refuse a default client
// with 403. They are not broken; they just will not talk to something that
// looks like a script. A browser string gets an honest answer, which is the
// point: a false alarm every run trains people to ignore the run.
const UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36';

const TIMEOUT_MS = 25000;

async function probe(url) {
  const opts = {
    redirect: 'follow',
    headers: { 'user-agent': UA, accept: '*/*' },
    signal: AbortSignal.timeout(TIMEOUT_MS),
  };
  // HEAD first because it is cheap; some servers refuse it, so fall back to GET.
  try {
    const r = await fetch(url, { ...opts, method: 'HEAD' });
    if (r.status !== 405 && r.status !== 403 && r.status < 500) return r;
  } catch (_) { /* fall through to GET */ }
  return fetch(url, opts);
}

function pathOf(u) {
  try { return new URL(u).pathname.replace(/\/+$/, ''); } catch { return ''; }
}

(async () => {
  const reg = JSON.parse(fs.readFileSync(REGISTRY, 'utf8'));
  const rows = (reg.sources || []).filter(s => s.url);
  const problems = [];
  const blocked = [];

  // Sequential on purpose. This is a courtesy check against public agency
  // servers, not a load test, and 60 requests take well under a minute.
  for (const s of rows) {
    let verdict = 'ok';
    let detail = '';
    try {
      const r = await probe(s.url);
      const ctype = (r.headers.get('content-type') || '').toLowerCase();
      const landed = r.url || s.url;
      if (r.status === 403 || r.status === 429) {
        // NOT evidence the document is gone. Some publishers (NIH, GBIF and
        // Canada's Drug Agency at the time of writing) refuse any automated
        // client regardless of headers, apparently on TLS fingerprint, while
        // serving the page perfectly to a browser. Reporting it is right;
        // failing the run on it would cry wolf every time and train everyone to
        // ignore this script, which is worse than not having it.
        verdict = 'BLOCKED';
        detail = 'HTTP ' + r.status + ', refuses automated clients; check by hand';
      } else if (r.status >= 400) {
        verdict = 'DEAD';
        detail = 'HTTP ' + r.status;
      } else if (/\.pdf($|\?)/i.test(s.url) && ctype.includes('text/html')) {
        verdict = 'SOFT 404';
        detail = 'asked for a PDF, got HTML';
      } else if (pathOf(s.url) && !pathOf(landed)) {
        verdict = 'SOFT 404';
        detail = 'redirected to the site root: ' + landed;
      }
    } catch (err) {
      verdict = 'UNREACHABLE';
      detail = err.name === 'TimeoutError' ? 'timed out' : err.message;
    }
    if (verdict !== 'ok') {
      (verdict === 'BLOCKED' ? blocked : problems).push({ id: s.id, url: s.url, verdict, detail });
      if (!QUIET) console.log(verdict.padEnd(12) + s.id.padEnd(26) + s.url + '  (' + detail + ')');
    }
  }

  console.log(
    'Checked ' + rows.length + ' source URLs in ' + REGISTRY + '. Problems: ' + problems.length +
    (blocked.length ? ', plus ' + blocked.length + ' that refuse automated clients (not failures)' : '')
  );
  if (problems.length) {
    console.log('');
    console.log('A citation that does not resolve is not a citation. Find where the');
    console.log('document moved, update the entry, and if the publisher retired it');
    console.log('outright, say so in the note so nobody re-adds the old link.');
  }
  process.exit(problems.length ? 1 : 0);
})();
