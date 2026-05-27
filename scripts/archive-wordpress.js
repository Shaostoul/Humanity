#!/usr/bin/env node
/**
 * archive-wordpress.js — one-shot archiver for a WordPress site via its REST API.
 *
 * Pulls every Page, Post, and Media item from <BASE>/wp-json/wp/v2/* and writes:
 *   <OUT>/pages/<slug>.html  + .txt   (rendered content of each page)
 *   <OUT>/posts/<slug>.html  + .txt   (rendered content of each blog post)
 *   <OUT>/media/<filename>            (every uploaded image/file, original bytes)
 *   <OUT>/index.json                  (manifest: type, id, title, slug, date, url, file)
 *
 * Why the REST API instead of wget: it returns a complete, structured inventory
 * (no theme/nav cruft, no crawl gaps) and needs nothing installed — plain Node.
 *
 * Usage:
 *   node scripts/archive-wordpress.js <baseUrl> <outDir> [mode]
 *   mode = texts | media | all   (default: all)
 *
 * Example:
 *   node scripts/archive-wordpress.js https://shaostoul.com C:/Humanity/_pu-archive texts
 */
const https = require('https');
const fs = require('fs');
const path = require('path');

const BASE = (process.argv[2] || 'https://shaostoul.com').replace(/\/+$/, '');
const OUT  = process.argv[3] || path.join(process.cwd(), '_pu-archive');
const MODE = (process.argv[4] || 'all').toLowerCase();

// --- tiny GET helpers (no external deps, works on any Node version) ---
function getJSON(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { 'User-Agent': 'pu-archiver/1.0' } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return resolve(getJSON(res.headers.location));
      }
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => {
        try { resolve({ headers: res.headers, json: JSON.parse(data) }); }
        catch (e) { reject(new Error(`JSON parse ${url}: ${e.message}`)); }
      });
    }).on('error', reject);
  });
}
function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https.get(url, { headers: { 'User-Agent': 'pu-archiver/1.0' } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        file.close(); try { fs.unlinkSync(dest); } catch (_) {}
        return resolve(download(res.headers.location, dest));
      }
      if (res.statusCode !== 200) {
        file.close(); try { fs.unlinkSync(dest); } catch (_) {}
        return reject(new Error(`HTTP ${res.statusCode}`));
      }
      res.pipe(file);
      file.on('finish', () => file.close(() => resolve()));
    }).on('error', (e) => { try { fs.unlinkSync(dest); } catch (_) {} reject(e); });
  });
}
// Naive HTML -> plain text, good enough for grepping/sifting.
const stripTags = (html) => (html || '')
  .replace(/<script[\s\S]*?<\/script>/gi, '')
  .replace(/<style[\s\S]*?<\/style>/gi, '')
  .replace(/<\/(p|div|h[1-6]|li|br|tr)>/gi, '\n')
  .replace(/<[^>]+>/g, '')
  .replace(/&#8211;/g, '-').replace(/&#8212;/g, '--')
  .replace(/&#8217;/g, "'").replace(/&#8216;/g, "'")
  .replace(/&#8220;/g, '"').replace(/&#8221;/g, '"')
  .replace(/&amp;/g, '&').replace(/&nbsp;/g, ' ')
  .replace(/&#?\w+;/g, '')
  .replace(/[ \t]+\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
const safe = (s) => (s || 'untitled').toString().replace(/[^a-z0-9._-]+/gi, '-').replace(/^-+|-+$/g, '').slice(0, 90) || 'untitled';

async function fetchAll(type) {
  let page = 1, all = [];
  while (true) {
    const r = await getJSON(`${BASE}/wp-json/wp/v2/${type}?per_page=100&page=${page}`);
    if (!Array.isArray(r.json) || r.json.length === 0) break;
    all = all.concat(r.json);
    const totalPages = parseInt(r.headers['x-wp-totalpages'] || '1', 10);
    if (page >= totalPages) break;
    page++;
  }
  return all;
}

(async () => {
  ['pages', 'posts', 'media'].forEach((d) => fs.mkdirSync(path.join(OUT, d), { recursive: true }));
  const manifestPath = path.join(OUT, 'index.json');
  let manifest = [];
  if (fs.existsSync(manifestPath)) { try { manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8')); } catch (_) {} }

  if (MODE === 'texts' || MODE === 'all') {
    manifest = manifest.filter((m) => m.type !== 'pages' && m.type !== 'posts');
    for (const type of ['pages', 'posts']) {
      const items = await fetchAll(type);
      console.log(`${type}: ${items.length}`);
      for (const it of items) {
        const slug = safe(it.slug || it.id);
        const title = (it.title && it.title.rendered) || slug;
        const html = (it.content && it.content.rendered) || '';
        fs.writeFileSync(path.join(OUT, type, `${slug}.html`),
          `<!-- id=${it.id} | ${it.link || ''} | ${it.date} -->\n<h1>${title}</h1>\n${html}`);
        fs.writeFileSync(path.join(OUT, type, `${slug}.txt`),
          `# ${stripTags(title)}\n(${it.link || ''} | ${it.date})\n\n${stripTags(html)}\n`);
        manifest.push({ type, id: it.id, title: stripTags(title), slug, date: it.date, link: it.link });
      }
    }
  }

  if (MODE === 'media' || MODE === 'all') {
    manifest = manifest.filter((m) => m.type !== 'media');
    const media = await fetchAll('media');
    console.log(`media: ${media.length} — downloading...`);
    let ok = 0, fail = 0;
    for (const m of media) {
      const src = m.source_url;
      if (!src) { fail++; continue; }
      // Lossless filename: prefix with the WP media id (guarantees uniqueness,
      // no silent overwrite of distinct files that sanitize to the same name)
      // and preserve the real extension (don't let the length cap eat ".png").
      const base = path.basename(new URL(src).pathname);
      const ext = path.extname(base);
      const stem = safe(base.slice(0, base.length - ext.length)).slice(0, 70);
      const fname = `${m.id}-${stem}${ext.toLowerCase()}`;
      try { await download(src, path.join(OUT, 'media', fname)); ok++; }
      catch (e) { fail++; console.warn(`  media fail ${src}: ${e.message}`); }
      manifest.push({ type: 'media', id: m.id, title: stripTags((m.title && m.title.rendered) || ''), mime: m.mime_type, date: m.date, src, file: `media/${fname}` });
      if ((ok + fail) % 25 === 0) console.log(`  ...${ok + fail}/${media.length}`);
    }
    console.log(`media downloaded: ${ok} ok, ${fail} fail`);
  }

  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  console.log(`DONE (${MODE}). manifest=${manifest.length} items. Output: ${OUT}`);
})().catch((e) => { console.error('FATAL', e); process.exit(1); });
