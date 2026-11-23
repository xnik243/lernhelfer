// Verschlüsselt folien.html (nur lokal, nie im Repo) mit einem Passwort in index.html (öffentlich).
//   node presentation/bauen.mjs "<Passwort>"
// Groß/klein zählt nicht. Danach index.html committen; folien.html bleibt draußen (.git/info/exclude).
import { readFileSync, writeFileSync } from 'node:fs';

const passwort = process.argv.slice(2).join(' ').trim().toLowerCase();
if (!passwort) throw new Error('Aufruf: node presentation/bauen.mjs "<Passwort>"');
const datei = name => new URL(name, import.meta.url);

const iter = 600_000;
const salt = crypto.getRandomValues(new Uint8Array(16)), iv = crypto.getRandomValues(new Uint8Array(12));
const basis = await crypto.subtle.importKey('raw', new TextEncoder().encode(passwort), 'PBKDF2', false, ['deriveKey']);
const key = await crypto.subtle.deriveKey({ name: 'PBKDF2', salt, iterations: iter, hash: 'SHA-256' }, basis, { name: 'AES-GCM', length: 256 }, false, ['encrypt']);
const daten = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, new TextEncoder().encode(readFileSync(datei('folien.html'), 'utf8'))));
const b64 = a => Buffer.from(a).toString('base64');

const tresor = JSON.stringify({ iter, salt: b64(salt), iv: b64(iv), daten: b64(daten) });
const seite = readFileSync(datei('index.html'), 'utf8');
const neu = seite.replace(/(<script id="tresor" type="application\/json">)[\s\S]*?(<\/script>)/, (_, a, b) => a + tresor + b);
if (neu === seite) throw new Error('Platzhalter <script id="tresor"> nicht gefunden');
writeFileSync(datei('index.html'), neu);
console.log(`presentation/index.html verschlüsselt (${Math.round(daten.length / 1024)} KB)`);
