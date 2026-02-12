const CACHE_NAME = 'humanity-v2';
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

self.addEventListener('fetch', event => {
  // Never cache WebSocket, API, or the chat page itself
  const url = event.request.url;
  if (url.includes('/ws') || url.includes('/api/') || url.includes('/chat')) return;

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
