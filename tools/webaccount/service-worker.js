// Cache only public, content-versioned game artifacts. Navigation and account
// traffic always use the network. A new page selects its own exact hashes.
const CACHE = 'earth-two-artifacts-v1';
const FILES = new Set(['account.js', 'wasm_exec.js', 'fs.js', 'raylib.js', 'raylib.wasm', 'raylib.data', 'jolt.js', 'jolt.wasm', 'game.wasm']);
self.addEventListener('install', event => event.waitUntil(self.skipWaiting()));
self.addEventListener('activate', event => event.waitUntil(self.clients.claim()));
self.addEventListener('fetch', event => {
  const request = event.request;
  const url = new URL(request.url);
  const base = new URL(self.registration.scope);
  const name = url.pathname.slice(base.pathname.length);
  if (request.method !== 'GET' || request.headers.has('Range') || url.origin !== base.origin ||
      !url.pathname.startsWith(base.pathname) || !FILES.has(name) ||
      !/^[a-f0-9]{64}$/.test(url.searchParams.get('v') || '') || [...url.searchParams.keys()].some(key => key !== 'v')) return;
  let saved = Promise.resolve();
  const result = (async () => {
    let cache;
    try {
      cache = await caches.open(CACHE);
      const hit = await cache.match(request);
      if (hit) return hit;
    } catch (_) { /* Private mode or unavailable storage: use the network. */ }
    const response = await fetch(request);
    if (cache && response.status === 200 && !response.redirected) {
      // A quota failure must never turn a successful download into a load error.
      saved = cache.put(request, response.clone()).then(async () => {
        // Keep the two newest versions of each artifact, allowing an older tab
        // to finish loading while avoiding unbounded accumulation of releases.
        const keys = (await cache.keys()).filter(key => new URL(key.url).pathname === url.pathname);
        await Promise.all(keys.slice(0, -2).map(key => cache.delete(key)));
      }).catch(() => {});
    }
    return response;
  })();
  event.respondWith(result);
  event.waitUntil(result.then(() => saved).catch(() => {}));
});
