// The service worker never serves yesterday's code while the network answers (2026-11-20,
// BUG-331). web/shared/sw.js answered same-origin scripts cache-first, though its comment said
// images, fonts and icons; the page scripts moved to /pages/*-app.js carry no ?v= stamp, so a push
// without a version bump could leave fresh HTML (pages are never cached) running a stale script
// ("X is not defined"). Scripts and styles are now network-first, the cached copy answering only
// offline (the offline copy is a feature; CLAUDE.md's no-caches rule forbids only stale code).
//
// Run: node --test scripts/tests/sw-fresh-code-web.test.js
//
// The real sw.js runs in a stand-in worker: its fetch handler is handed requests while the cache
// holds an old copy and the network has a new one (or is down), and what it answers is read back.
//
// SEEN RED (2026-11-20) against sw.js before the change (git show 28d1c3596:web/shared/sw.js):
// "/pages/tasks-app.js while online: the network's copy" was answered "old tasks-app.js" (the cached
// copy, with the network up).
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const ORIGIN = 'https://site.test';

/** The worker, loaded fresh, with `cached` (url -> body) in its cache and the network online or not. */
function worker({ cached = {}, online = true } = {}) {
  const listeners = {};
  const store = new Map(Object.entries(cached).map(([u, b]) => [ORIGIN + u, b]));
  const answer = (body) => ({ ok: true, type: 'basic', body, clone() { return answer(body); } });
  const ctx = {
    self: {
      addEventListener: (type, fn) => { listeners[type] = fn; },
      location: { origin: ORIGIN },
      skipWaiting() {}, clients: { claim() {} }, registration: { showNotification() {} },
    },
    caches: {
      match: async (req) => (store.has(req.url) ? answer(store.get(req.url)) : undefined),
      open: async () => ({ put: async (req, res) => { store.set(req.url, res.body); }, addAll: async () => {} }),
      keys: async () => [], delete: async () => true,
    },
    fetch: async (req) => {
      if (!online) throw new TypeError('offline');
      return answer('new ' + path.basename(new URL(req.url).pathname));
    },
    URL, console,
  };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, 'shared', 'sw.js'), 'utf8'), ctx, { filename: 'sw.js' });
  /** What the worker answers for `url` (null: it left the request to the browser). */
  return async (url, destination) => {
    let reply = null;
    const headers = { has: () => false };
    listeners.fetch({ request: { url: ORIGIN + url, destination, headers }, respondWith: (p) => { reply = p; } });
    if (!reply) return null;
    const res = await reply;
    return res ? res.body : undefined;
  };
}

test('scripts and styles come from the network while it answers, and from the cache offline', async () => {
  const cached = {
    '/pages/tasks-app.js': 'old tasks-app.js',
    '/shared/safe-markup.js?v=1': 'old safe-markup.js',
    '/pages/files.css': 'old files.css',
    '/assets/icons/star.svg': 'old star.svg',
  };
  const online = worker({ cached });
  assert.strictEqual(await online('/pages/tasks-app.js', 'script'), 'new tasks-app.js', '/pages/tasks-app.js while online: the network\'s copy');
  assert.strictEqual(await online('/shared/safe-markup.js?v=1', 'script'), 'new safe-markup.js');
  assert.strictEqual(await online('/pages/files.css', 'style'), 'new files.css');
  // Pictures stay cache-first: they change rarely and the cache name's bump clears them.
  assert.strictEqual(await online('/assets/icons/star.svg', 'image'), 'old star.svg');
  const offline = worker({ cached, online: false });
  assert.strictEqual(await offline('/pages/tasks-app.js', 'script'), 'old tasks-app.js', 'offline, the last copy still answers');
  assert.strictEqual(await offline('/pages/files.css', 'style'), 'old files.css');
});

test('the worker still leaves pages, other sites and the relay to the browser', async () => {
  const ask = worker({ cached: { '/chat': 'old page' } });
  assert.strictEqual(await ask('/chat', 'document'), null, 'a page is never answered from here');
  assert.strictEqual(await ask('/api/health', ''), null);
  assert.strictEqual(await ask('/ws', ''), null);
});

test('the check can fail: a cache-first worker serves the old script', async () => {
  // The same stand-in, given a worker that answers every request cache-first.
  const ctx = { self: { addEventListener: (t, fn) => { ctx.fn = fn; }, location: { origin: ORIGIN } } };
  vm.createContext(ctx);
  vm.runInContext("self.addEventListener('fetch', (e) => e.respondWith(Promise.resolve({ body: 'old tasks-app.js' })));", ctx);
  let reply;
  ctx.fn({ request: { url: ORIGIN + '/pages/tasks-app.js', destination: 'script', headers: { has: () => false } }, respondWith: (p) => { reply = p; } });
  assert.notStrictEqual((await reply).body, 'new tasks-app.js');
});
