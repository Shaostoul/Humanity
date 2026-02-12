const CACHE_NAME = 'humanity-v3';
const SHELL_URLS = [
  '/shared/shell.js',
  '/shared/theme.css',
  '/shared/manifest.json',
  '/favicon.svg',
  '/favicon.png'
];

// Don't pre-cache /chat — it changes frequently and should always be network-first.

self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(SHELL_URLS))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k)))
    ).then(() => self.clients.claim())
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
  // Never cache WebSocket, API, chat page, or uploads
  const url = event.request.url;
  if (url.includes('/ws') || url.includes('/api/') || url.includes('/chat') || url.includes('/uploads/')) return;

  event.respondWith(
    fetch(event.request)
      .then(response => {
        const clone = response.clone();
        caches.open(CACHE_NAME).then(cache => cache.put(event.request, clone));
        return response;
      })
      .catch(() => caches.match(event.request))
  );
});
