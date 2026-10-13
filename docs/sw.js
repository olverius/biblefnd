// Service worker: makes the app installable and usable offline.
//  - the app's own files come from the network, with a cached copy for offline
//  - readings and verses come from the network, falling back to the last
//    cached copy when offline
const VERSION = "biblefnd-v5";
const SHELL = [
  "./",
  "./index.html",
  "./style.css",
  "./app.js",
  "./manifest.webmanifest",
  "./icons/icon.svg",
  "./icons/icon-192.png",
  "./icons/icon-512.png",
  "./icons/apple-touch-icon.png",
];

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(VERSION).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== VERSION).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  if (url.origin === self.location.origin) {
    // app's own files: network first so updates show up immediately,
    // cached copy only when offline
    event.respondWith(
      fetch(req)
        .then((resp) => {
          if (resp.ok) {
            const copy = resp.clone();
            caches.open(VERSION).then((c) => c.put(req, copy));
          }
          return resp;
        })
        .catch(() => caches.match(req, { ignoreSearch: true }).then((hit) => hit || Promise.reject(new Error("offline"))))
    );
    return;
  }

  // Random verses must stay random, so never serve those from cache.
  if (url.pathname.includes("/random")) return;

  // Let the browser load the Universalis readings script itself; passing an
  // opaque cross-site script through the worker is fragile on some browsers.
  if (url.hostname.endsWith("universalis.com")) return;

  // APIs: network first, remember the answer for offline use
  event.respondWith(
    fetch(req)
      .then((resp) => {
        if (resp.ok || resp.type === "opaque") {
          const copy = resp.clone();
          caches.open(`${VERSION}-data`).then((c) => c.put(req, copy));
        }
        return resp;
      })
      .catch(() => caches.match(req).then((hit) => hit || Promise.reject(new Error("offline"))))
  );
});
