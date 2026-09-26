#!/usr/bin/env node
// Refuses a websites database that claims more than it records.
//
// WHY THIS EXISTS
// data/web/sites.json is where HumanityOS writes down whether a site's pages
// may be shown inside the app (the readable web today, in-world screens
// next). That record is only worth anything if a decided status cannot be
// typed in without the evidence behind it. So: a status other than
// needs_review must carry who reviewed it, when, and on what clause; an
// affiliate tag must carry the disclosure a person sees; our own domains
// must be allowed (they are ours); ids must be unique; URLs must be http(s)
// because the reader refuses anything else before a request is made.
//
// It also shape-checks data/web/readability.json, the parser's drop hints,
// and refuses a domain name there: that file is markup conventions only.
//
// PROVEN RED: flip any needs_review to "allowed" without filling in the
// review fields and this names the record and the missing field.
//
//   node scripts/check-web-sites.js

const fs = require('fs');

const SITES = 'data/web/sites.json';
const RULES = 'data/web/readability.json';
const STATUSES = ['needs_review', 'allowed', 'forbidden', 'unknown'];
const COLORS = ['accent', 'info', 'success', 'warning', 'danger'];
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

const problems = [];
function fail(where, what) { problems.push(where + ': ' + what); }

// ── sites.json ──
let db;
try {
  db = JSON.parse(fs.readFileSync(SITES, 'utf8'));
} catch (e) {
  console.log(SITES + ': cannot read or parse (' + e.message + ')');
  process.exit(1);
}

const own = Array.isArray(db.own_domains) ? db.own_domains : [];
if (!own.length) fail(SITES, 'own_domains must list our own URL prefixes');
for (const o of own) {
  if (!/^https?:\/\//.test(o)) fail(SITES, 'own_domains entry is not an http(s) URL prefix: ' + o);
}

const cats = Array.isArray(db.categories) ? db.categories : [];
if (!cats.length) fail(SITES, 'categories must be a non-empty array');
const catIds = new Set();
for (const c of cats) {
  const where = SITES + ' category ' + JSON.stringify(c && c.id);
  if (!c || typeof c.id !== 'string' || !c.id) { fail(where, 'missing id'); continue; }
  if (catIds.has(c.id)) fail(where, 'duplicate category id');
  catIds.add(c.id);
  if (typeof c.name !== 'string' || !c.name) fail(where, 'missing name');
  if (!COLORS.includes(c.color)) fail(where, 'color must be one of ' + COLORS.join(', ') + ', got ' + JSON.stringify(c.color));
}

const sites = Array.isArray(db.sites) ? db.sites : [];
if (!sites.length) fail(SITES, 'sites must be a non-empty array');
const ids = new Set();
const urls = new Set();
for (const s of sites) {
  const where = SITES + ' site ' + JSON.stringify(s && s.id);
  if (!s || typeof s.id !== 'string' || !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(s.id)) { fail(where, 'id must be kebab-case'); continue; }
  if (ids.has(s.id)) fail(where, 'duplicate id');
  ids.add(s.id);
  if (typeof s.name !== 'string' || !s.name) fail(where, 'missing name');
  if (typeof s.url !== 'string' || !/^https?:\/\/[^\s/]+/.test(s.url)) fail(where, 'url must be http(s) with a host, got ' + JSON.stringify(s.url));
  else {
    const k = s.url.replace(/^https?:\/\//, '').replace(/^www\./, '').replace(/\/+$/, '').toLowerCase();
    if (urls.has(k)) fail(where, 'duplicate url ' + s.url);
    urls.add(k);
  }
  if (!catIds.has(s.category)) fail(where, 'category ' + JSON.stringify(s.category) + ' is not in categories[]');
  if (typeof s.icon === 'string' && s.icon.includes('️')) fail(where, 'icon carries U+FE0F, which renders as tofu in the app font');

  // embed: the legality record.
  const e = s.embed;
  if (!e || typeof e !== 'object') { fail(where, 'missing embed record'); continue; }
  if (!STATUSES.includes(e.status)) fail(where, 'embed.status must be one of ' + STATUSES.join(', ') + ', got ' + JSON.stringify(e.status));
  const isOwn = own.some(o => typeof s.url === 'string' && s.url.startsWith(o));
  if (isOwn && e.status !== 'allowed') fail(where, 'is one of our own domains and must be embed.status "allowed", got ' + JSON.stringify(e.status));
  if (e.status === 'needs_review') {
    if (e.reviewed_on !== null || e.reviewed_by !== null) fail(where, 'needs_review must not carry reviewed_on/reviewed_by (either it was reviewed or it was not)');
    if (e.terms_url !== null) fail(where, 'needs_review must not carry terms_url; record the review or leave it null');
  } else {
    if (typeof e.basis !== 'string' || !e.basis.trim() || /not yet reviewed/i.test(e.basis)) fail(where, 'embed.status ' + e.status + ' needs a real basis (the clause it rests on)');
    if (typeof e.reviewed_on !== 'string' || !ISO_DATE.test(e.reviewed_on)) fail(where, 'embed.status ' + e.status + ' needs reviewed_on as YYYY-MM-DD');
    if (typeof e.reviewed_by !== 'string' || !e.reviewed_by.trim()) fail(where, 'embed.status ' + e.status + ' needs reviewed_by');
    if (e.status !== 'unknown' && (typeof e.terms_url !== 'string' || !/^https?:\/\//.test(e.terms_url))) fail(where, 'embed.status ' + e.status + ' needs terms_url (http(s))');
  }
  // attribution: the credit line the licence asks for, shown on every page
  // of an allowed site. Optional, but never an empty promise.
  if ('attribution' in e && (typeof e.attribution !== 'string' || !e.attribution.trim())) fail(where, 'embed.attribution, when present, must be a non-empty string');
  if (e.status === 'allowed' && !isOwn && !(typeof e.attribution === 'string' && e.attribution.trim())) fail(where, 'an allowed third-party site needs embed.attribution (the credit line its licence asks for)');

  // affiliate: the transparency record.
  const a = s.affiliate;
  if (!a || typeof a !== 'object') { fail(where, 'missing affiliate record'); continue; }
  for (const k of ['program', 'tag']) {
    if (!(k in a)) fail(where, 'affiliate.' + k + ' must be present (null when none)');
    else if (a[k] !== null && (typeof a[k] !== 'string' || !a[k].trim())) fail(where, 'affiliate.' + k + ' must be null or a non-empty string');
  }
  if (typeof a.disclosure !== 'string') fail(where, 'affiliate.disclosure must be a string (empty when no tag)');
  else if (a.tag && !a.disclosure.trim()) fail(where, 'affiliate.tag is set, so affiliate.disclosure (the line shown to the reader) is required');
  else if (!a.tag && a.disclosure.trim()) fail(where, 'affiliate.disclosure is set but there is no tag; a disclosure with nothing to disclose misleads');
}

// ── readability.json ──
let rules;
try {
  rules = JSON.parse(fs.readFileSync(RULES, 'utf8'));
} catch (e) {
  fail(RULES, 'cannot read or parse (' + e.message + ')');
}
if (rules) {
  for (const k of ['drop_classes', 'drop_ids']) {
    const arr = rules[k];
    if (!Array.isArray(arr)) { fail(RULES, k + ' must be an array'); continue; }
    const seen = new Set();
    for (const v of arr) {
      if (typeof v !== 'string' || !v.trim() || /\s/.test(v)) fail(RULES, k + ' entry must be one token, got ' + JSON.stringify(v));
      else if (seen.has(v)) fail(RULES, k + ' repeats ' + JSON.stringify(v));
      // A dot in a token is how a domain gets smuggled in. Conventions only.
      else if (/\.[a-z]{2,}$/i.test(v)) fail(RULES, k + ' entry looks like a domain, which does not belong here: ' + v);
      seen.add(v);
    }
  }
}

if (problems.length) {
  console.log('');
  console.log('Websites database problems:');
  for (const p of problems) console.log('  ' + p);
  console.log('');
  console.log('  A decided embed status must record who reviewed the terms, when, and on');
  console.log('  what clause; an affiliate tag must carry its disclosure. See');
  console.log('  schemas/web_sites.toml and docs/design/readable-web.md.');
  console.log('');
}

const decided = sites.filter(s => s.embed && s.embed.status !== 'needs_review').length;
console.log('Checked ' + sites.length + ' sites in ' + cats.length + ' categories (' + decided +
  ' with a recorded embed decision, ' + (sites.length - decided) + ' awaiting review). Problems: ' + problems.length);
process.exit(problems.length ? 1 : 0);
