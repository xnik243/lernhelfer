// Lernhelfer ohne Netz lesen (Hörsaal, Zug): immer zuerst das Netz, nur ohne Verbindung die letzte Antwort aus dem Gerät.
// Nur GET und nur App-Dateien und Daten (Ausgaben, Konto); Anmelden, Speichern und KI gehen nie hierüber.
// Abmelden und Anmelden leeren den Speicher (neuLaden in index.html), damit nichts vom vorigen Konto bleibt.
// ponytail: der Speicher wächst mit jeder gelesenen Datei; bei Platzmangel räumt der Browser selbst, sonst Größe deckeln
const SPEICHER = 'lernhelfer';
const APP = /(\/app|\/index\.html|\/skizze\.css|\/icon-180\.png|\/manifest\.webmanifest|\/(lib|fonts)\/[\w.-]+)$/;
const DATEN = /\/api\/(gh\/|ich$|tarife$)/;

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', e => e.waitUntil(self.clients.claim()));
self.addEventListener('fetch', e => {
  const r = e.request, u = new URL(r.url);
  const eigen = u.origin === location.origin && (u.pathname === new URL(self.registration.scope).pathname || APP.test(u.pathname) || DATEN.test(u.pathname));
  if (r.method !== 'GET' || !eigen && !(u.origin === 'https://api.github.com' && u.pathname.includes('/contents/'))) return;
  // gh() holt dieselbe Datei mal roh, mal als JSON (mit sha): getrennt merken
  const art = r.headers.get('Accept') || '', schluessel = art.includes('github') ? `${u.href}${u.search ? '&' : '?'}lh=${art.includes('raw') ? 'roh' : 'json'}` : r;
  e.respondWith(fetch(r).then(a => {
    if (a.ok && a.type !== 'opaque') { const kopie = a.clone(); e.waitUntil(caches.open(SPEICHER).then(c => c.put(schluessel, kopie))); }
    return a;
  }, () => caches.match(schluessel, { ignoreVary: true }).then(a => a || Response.error())));
});
