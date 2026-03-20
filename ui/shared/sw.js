// Bump version whenever cached assets change.
// HTML pages are intentionally NEVER cached (they change every deploy).
const CACHE_NAME = 'humanity-v43';
const SHELL_URLS = [
  '/shared/shell.js',
  '/shared/theme.css',
  '/shared/manifest.json',
  '/favicon.svg',
  '/favicon.png'
];

self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(SHELL_URLS))
      .then(() => self.skipWaiting()) // take control immediately
  );
});

self.addEventListener('activate', event => {
  event.waitUntil(
    // Delete all old caches (different version names)
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k))))
      .then(() => self.clients.claim()) // claim existing tabs
  );
});

// ── Push-like Notifications (from main page postMessage) ──
self.addEventListener('message', event => {
  if (event.data && event.data.type === 'notification') {
    self.registration.showNotification(event.data.title, {
      body: event.data.body,
      icon: '/shared/icons/icon-192.png',
      badge: '/shared/icons/icon-192.png',
      tag: event.data.tag || 'humanity',
      data: { url: event.data.url || '/chat' }
    });
  }
});

// ── WebPush handler — shows notification when relay sends a push ──
self.addEventListener('push', event => {
  const data = event.data ? event.data.json() : {};
  event.waitUntil(
    self.registration.showNotification(data.title || 'HumanityOS', {
      body: data.body || 'New message',
      icon: '/shared/icons/icon-192.png',
      badge: '/shared/icons/icon-192.png',
      tag: data.tag || 'humanity',
      data: { url: data.url || '/chat' }
    })
  );
});

self.addEventListener('notificationclick', event => {
  event.notification.close();
  event.waitUntil(
    clients.matchAll({ type: 'window' }).then(cls => {
      if (cls.length > 0) { cls[0].focus(); return; }
      clients.openWindow(event.notification.data.url || '/chat');
    })
  );
});

self.addEventListener('fetch', event => {
  const url = event.request.url;

  // ── Never intercept these — always go to network ──────────────────────────
  // HTML pages (document navigations): these change with every deploy,
  // stale HTML = broken JS. The browser's HTTP cache handles them via ETag.
  if (event.request.destination === 'document') return;

  // API calls, WebSocket upgrades, uploads
  if (url.includes('/ws') || url.includes('/api/') || url.includes('/uploads/')) return;

  // ── Cache-first for static assets (CSS, JS, images, fonts) ────────────────
  // These are safe to cache because they change rarely and when they do,
  // bumping CACHE_NAME above ensures a fresh install wipes the old copies.
  event.respondWith(
    caches.match(event.request).then(cached => {
      if (cached) return cached;
      return fetch(event.request).then(response => {
        // Only cache successful same-origin responses
        if (response.ok && response.type === 'basic') {
          const clone = response.clone();
          caches.open(CACHE_NAME).then(cache => cache.put(event.request, clone));
        }
        return response;
      });
    }).catch(() => caches.match(event.request))
  );
});
