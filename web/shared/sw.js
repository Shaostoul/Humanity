// Bump version whenever cached assets change.
// HTML pages are intentionally NEVER cached (they change every deploy).
const CACHE_NAME = 'humanity-v1572';
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

// ── WebPush handler, shows notification when relay sends a push ──
self.addEventListener('push', event => {
  const data = event.data ? event.data.json() : {};
  event.waitUntil(
    self.registration.showNotification(data.title || 'HumanityOS', {
      body: data.body || 'New message',
      icon: '/shared/icons/icon-192.png',
      badge: '/shared/icons/icon-192.png',
      tag: data.tag || 'humanity',
      data: { url: data.url || '/chat' },
      actions: [
        { action: 'reply', title: 'Reply' },
        { action: 'mark-read', title: 'Mark Read' }
      ]
    })
  );
});

self.addEventListener('notificationclick', event => {
  event.notification.close();
  const action = event.action;
  const url = (event.notification.data && event.notification.data.url) || '/chat';

  if (action === 'mark-read') {
    // Mark as read: notify open clients to clear unread badge for this conversation.
    event.waitUntil(
      clients.matchAll({ type: 'window' }).then(cls => {
        cls.forEach(client => {
          client.postMessage({
            type: 'notification-action',
            action: 'mark-read',
            tag: event.notification.tag
          });
        });
      })
    );
    return;
  }

  // Default click or 'reply' action: focus or open the relevant page.
  event.waitUntil(
    clients.matchAll({ type: 'window' }).then(cls => {
      if (cls.length > 0) {
        cls[0].focus();
        // Tell the client to open the reply box if action was 'reply'.
        if (action === 'reply') {
          cls[0].postMessage({
            type: 'notification-action',
            action: 'reply',
            tag: event.notification.tag,
            url: url
          });
        }
        return;
      }
      clients.openWindow(url);
    })
  );
});

self.addEventListener('fetch', event => {
  const url = event.request.url;

  // ── Never intercept these, always go to network ──────────────────────────
  // HTML pages (document navigations): these change with every deploy,
  // stale HTML = broken JS. The browser's HTTP cache handles them via ETag.
  if (event.request.destination === 'document') return;

  // API calls, WebSocket upgrades, uploads
  if (url.includes('/ws') || url.includes('/api/') || url.includes('/uploads/')) return;

  // ── Network-first for the app shell (shell.js, theme.css, manifest) ───────
  // These change on web deploys, so serving them cache-first hid new nav/theme
  // until a full SW reinstall (the v0.469.1 "website didn't change" report). The
  // server already sends `no-cache, must-revalidate` on them, so prefer the
  // network (a cheap ETag revalidate -> 304 when unchanged) and fall back to the
  // cache ONLY when offline. New deploys now appear on the next reload. (v0.469.2)
  const isShell = SHELL_URLS.some(s => url.endsWith(s));
  if (isShell) {
    event.respondWith(
      fetch(event.request).then(response => {
        if (response.ok && response.type === 'basic') {
          const clone = response.clone();
          caches.open(CACHE_NAME).then(cache => cache.put(event.request, clone));
        }
        return response;
      }).catch(() => caches.match(event.request))
    );
    return;
  }

  // ── Cache-first for other static assets (images, fonts, icons) ────────────
  // Safe to cache because they change rarely; bumping CACHE_NAME wipes old copies.
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
