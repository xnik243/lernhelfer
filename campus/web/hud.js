// Campus – Bedienoberfläche: Eingaben (Tastatur/Maus und Touch), Menüs, Namensschilder, Töne.
// Das Spiel selbst (Welt, Grafik, Netz-Protokoll) läuft in Rust/WebAssembly; hier wird nur bedient und angezeigt.
import * as inventar from './inventar.js?v=__V__';
import * as musik from './musik.js?v=__V__';
import * as foto from './foto.js?v=__V__';
import * as duell from './duell.js?v=__V__';
import * as katakomben from './katakomben.js?v=__V__';
import * as architekt from './architekt.js?v=__V__';
const V = '__V__';
const $ = id => document.getElementById(id);
function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k.startsWith('on')) e[k] = v; else if (v === true) e.setAttribute(k, ''); else if (v !== false && v != null) e.setAttribute(k, v);
  }
  e.append(...kids.flat().filter(k => k != null && k !== false));
  return e;
}
async function api(op, body) {
  const r = await fetch('api/' + op, body === undefined ? { cache: 'no-store' } : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  const j = await r.json().catch(() => ({}));
  if (!r.ok) throw Object.assign(new Error(j.error || `Der Server antwortet ${r.status}.`), { status: r.status });
  return j;
}
const farbe = rgb => `rgb(${rgb[0]},${rgb[1]},${rgb[2]})`;
const zahl = n => Number(n).toLocaleString('de-AT');

let game, ws, me, reg, hier = { id: 0, name: '' }, auswahl = 0, sicht = 0, zone = '';
const inv = new Map(), spieler = new Map(), schilder = new Map();
const TOUCH = matchMedia('(pointer: coarse)').matches;

/* ---------- Einstellungen (nur dieses Gerät, localStorage) ---------- */
const AKTIONEN = { vor: 'Vorwärts', zurueck: 'Rückwärts', links: 'Links', rechts: 'Rechts', springen: 'Springen', sprinten: 'Sprinten', interagieren: 'Sprechen / Öffnen', menue: 'SkyBlock-Menü', chat: 'Chat öffnen', befehl: 'Befehl (/)', kamera: 'Kamera wechseln', faehigkeit: 'Werkzeug-Fähigkeit' };
const STANDARD = {
  v: 2, maus: 1, touch: 1, invertY: false, stuecke: TOUCH ? 6 : 8, fov: 75, fpsMax: 0, aufloesung: 1, grafik: 'auto', himmel: 0, ao: true, wolken: true, partikel: true, laut: 0.8, textur: 16, musik: 0.5, wetter: 0,
  tasten: { vor: 'KeyW', zurueck: 'KeyS', links: 'KeyA', rechts: 'KeyD', springen: 'Space', sprinten: 'ShiftLeft', interagieren: 'KeyE', menue: 'KeyM', chat: 'KeyT', befehl: 'Slash', kamera: 'F5', faehigkeit: 'KeyF' },
};
let opt = STANDARD;
try {
  const o = JSON.parse(localStorage.getItem('campus-optionen') || '{}');
  opt = { ...STANDARD, ...o, tasten: { ...STANDARD.tasten, ...o.tasten } };
  // alte Belegung: E war das Menü – jetzt spricht E mit Personen, das Menü liegt auf M (einmalig umstellen)
  if (!(o.v >= 2)) { if (opt.tasten.menue === 'KeyE') opt.tasten.menue = 'KeyM'; opt.tasten.interagieren = 'KeyE'; opt.v = 2; }
} catch {}
const optSpeichern = () => { try { localStorage.setItem('campus-optionen', JSON.stringify(opt)); } catch {} optAnwenden(); };
function optAnwenden() {
  if (!game) return;
  game.set_options(opt.stuecke, opt.fov, opt.ao, opt.wolken, opt.partikel);
  if (autoStufe < 0) autoStufe = startStufe();
  game.set_graphics(foto.fotoAktiv() ? Math.max(1, stufe()) : stufe(), opt.himmel); // Foto-Modus braucht den Nachbearbeitungs-Durchgang
  ruckel = 0;
  musik.setLaut(Number(opt.musik) || 0);
  wetterAnwenden();
  // Eco rechnet mit 75 % der Pixel
  schaerfe = Math.min(devicePixelRatio || 1, TOUCH ? 1.5 : 2) * opt.aufloesung * (stufe() === 0 ? 0.75 : 1);
  groesse();
}
/** Wetter: eigene Wahl (1..4 = Klar, Sommerregen, Schneefall, Kirschblüten) oder „Auto“ = was der Server meldet. */
let wetterServer = 0;
function wetterAnwenden() {
  // geschlossene Zonen: drinnen regnet und schneit es nicht
  const w = (zone === 'Prüfungs-Katakomben' || zone === 'Kristallminen') ? 0 : (opt.wetter ? opt.wetter - 1 : wetterServer);
  game?.set_weather?.(w);
  musik.setRegen(w === 1);
}
/** 2×-Taler-Event: Banner oben mit Text und Restzeit, verschwindet von selbst. */
let eventBis = 0, eventText = '', eventUhr = 0;
function eventZeigen(j) {
  clearInterval(eventUhr);
  eventBis = j?.an ? Number(j.bis) || 0 : 0; eventText = String(j?.text || '2× Taler-Event');
  const zeig = () => {
    const rest = Math.ceil((eventBis - Date.now()) / 1000), b = $('event-banner');
    b.hidden = !(rest > 0);
    if (rest > 0) b.textContent = `${eventText} · noch ${Math.floor(rest / 60)}:${String(rest % 60).padStart(2, '0')}`;
    else clearInterval(eventUhr);
  };
  if (eventBis) eventUhr = setInterval(zeig, 1000);
  zeig();
}
/* Shader-Stufe: 0 Eco, 1 Hoch, 2 Ultra, 3 Extrem. „Auto“ wählt nach Gerät und geht bei Ruckeln eine Stufe zurück (nie von selbst hoch). */
const STUFEN = ['Eco', 'Hoch', 'Ultra', 'Extrem'];
let autoStufe = -1, ruckel = 0;
const stufe = () => opt.grafik === 'auto' ? autoStufe : opt.grafik;
function startStufe() {
  const gpu = (game.gpu_info() || '').toLowerCase(), webgpu = game.backend() === 'WebGPU';
  if (/swiftshader|llvmpipe|software|basic render|mali-[gt]|powervr|adreno \(tm\) [3-5]/.test(gpu)) return 0;
  if (TOUCH) return webgpu && Math.min(screen.width, screen.height) >= 700 ? 1 : 0;
  if (!webgpu) return 1;
  if (/rtx|radeon rx|rx [5-9]\d{3}|arc a[57]/.test(gpu)) return 3;
  return 2;
}
const stufeText = () => opt.grafik === 'auto' ? `Auto wählt gerade: ${STUFEN[autoStufe] ?? '–'} (${game?.backend() ?? ''}).` : 'Feste Stufe. Mit „Auto“ passt sich die Stufe deinem Gerät an und sinkt bei Ruckeln.';
const aktion = code => Object.keys(opt.tasten).find(a => opt.tasten[a] === code);
const B = id => reg.bloecke.find(b => b.id === id), C = id => reg.cosmetics.find(c => c.id === id);
const S = id => reg.sky.items.find(i => i.id === id);
const G = id => reg.rpg?.gegenstaende?.find(g => g.id === id); // RPG-Register: Name, Seltenheit, Lore aller Gegenstände
const nameVon = item => item === reg.bauplan ? 'Bauplan' : (item < 256 ? B(item) : item >= 1000 ? S(item) : C(item))?.name || G(item)?.name || '?';
const WERKZEUG = id => reg.sky.werkzeuge.find(t => t.id === id);
const NPC_BASE = 1000000;
let sky = {}; // Fähigkeiten, Splitter, Treibstoff, Mana, Quest (api/sky)

/* ---------- Töne: alles im Browser erzeugt, keine Dateien ---------- */
let ac, letzteMuenze = 0;
// Material-Ids wie world.rs: 0 Stein, 1 Erde, 2 Holz, 3 Pflanze, 4 Kristall/Glas, 5 Stoff, 6 Metall, 7 Flüssig
function ton(art, laut = 1, hoehe = 0, mat = 0) {
  laut *= opt.laut;
  if (!ac || laut <= 0.02) return;
  const t = ac.currentTime, g = ac.createGain();
  g.connect(ac.destination);
  const osz = (typ, f0, f1, dauer, pegel, ab = 0) => {
    const o = ac.createOscillator(), og = ac.createGain();
    o.type = typ; o.frequency.setValueAtTime(f0, t + ab); o.frequency.exponentialRampToValueAtTime(f1, t + ab + dauer);
    og.gain.setValueAtTime(0.0001, t); og.gain.setValueAtTime(pegel * laut, t + ab); og.gain.exponentialRampToValueAtTime(0.0008, t + ab + dauer);
    o.connect(og).connect(g); o.start(t); o.stop(t + ab + dauer + 0.02);
  };
  const rauschen = (dauer, pegel, grenze, typ = 'lowpass', f1 = 0) => {
    const buf = ac.createBuffer(1, Math.ceil(ac.sampleRate * dauer), ac.sampleRate), d = buf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / d.length);
    const q = ac.createBufferSource(), f = ac.createBiquadFilter(), og = ac.createGain();
    q.buffer = buf; f.type = typ; f.frequency.setValueAtTime(grenze, t); if (f1) f.frequency.exponentialRampToValueAtTime(f1, t + dauer); og.gain.value = pegel * laut;
    q.connect(f).connect(og).connect(g); q.start(t);
  };
  switch (art) {
    case 'schritt': // hoehe = Material
      if (hoehe === 2) { osz('triangle', 190, 120, 0.07, 0.16); rauschen(0.04, 0.06, 1200); }
      else if (hoehe === 0 || hoehe === 6) { rauschen(0.04, 0.13, 3200, 'bandpass'); osz('square', 140, 90, 0.03, 0.04); }
      else if (hoehe === 4) { osz('sine', 1900, 1700, 0.05, 0.05); rauschen(0.03, 0.06, 4000, 'highpass'); }
      else if (hoehe === 7) rauschen(0.12, 0.09, 900, 'lowpass', 300);
      else rauschen(0.07, 0.12, 650);                                                          // Wiese, Erde, Stoff
      break;
    case 'schlag': { // hoehe = Riss-Stufe 0..7 (Tonhöhe steigt), mat = Material
      const f = 1 + hoehe * 0.09, welle = mat === 2 ? 'triangle' : mat === 0 || mat === 6 ? 'square' : 'sine', basis = mat === 4 ? 420 : mat === 2 ? 170 : 120;
      rauschen(0.07, 0.22, (mat === 1 || mat === 3 ? 700 : 1500) * f, 'bandpass'); osz(welle, basis * f, basis * 0.6 * f, 0.08, mat === 0 || mat === 6 ? 0.08 : 0.18);
      break;
    }
    case 'plopp': // mat = Material
      rauschen(0.14, 0.28, mat === 4 ? 4000 : 1800, 'lowpass', 300); osz('sine', mat === 4 ? 1200 : 520, 140, 0.12, 0.22); osz('triangle', 900, 1400, 0.06, 0.08, 0.05);
      break;
    case 9: rauschen(0.9, 0.3, 300, 'bandpass', 2600); osz('sine', 180, 900, 0.8, 0.12); break;                    // Portal-Whoosh
    case 10: rauschen(0.3, 0.2, 800, 'highpass', 5000); osz('sawtooth', 300, 1600, 0.25, 0.05); break;             // Enterhaken-Zip
    case 11: osz('sine', 140, 620, 0.32, 0.3); osz('triangle', 280, 1240, 0.28, 0.08, 0.02); break;                // Sprungpolster-Boing
    case 12: rauschen(1.6, 0.35, 400, 'bandpass', 1400); rauschen(1.2, 0.2, 2000, 'lowpass', 200); break;          // Wind der Leere
    case 13: osz('square', 1200, 1100, 0.03, 0.05); break;                                                          // UI-Klick
    case 14: [523, 659, 784, 1047, 1319].forEach((f, i) => osz('square', f, f, i === 4 ? 0.45 : 0.12, 0.07, i * 0.09)); break; // Fanfare
    case 6: if (performance.now() - letzteMuenze < 250) return; letzteMuenze = performance.now(); osz('sine', 988, 990, 0.09, 0.14); osz('sine', 1319, 1322, 0.22, 0.14, 0.08); break; // Münze
    case 0: osz('triangle', 230, 130, 0.09, 0.3); rauschen(0.04, 0.12, 2400); break;      // setzen
    case 1: rauschen(0.16, 0.3, 900); osz('sine', 150, 70, 0.12, 0.18); break;             // abbauen
    case 2: osz('sine', 300, 520, 0.07, 0.07); break;                                       // springen
    case 3: osz('sine', 220, 900, 0.22, 0.16); break;                                       // Sprungfeder
    case 4: rauschen(0.05, 0.05, 700); break;                                               // Schritt
    case 5: osz('sine', 880, 878, 0.9, 0.16); osz('sine', 1320, 1316, 0.6, 0.09); osz('sine', 2640, 2630, 0.25, 0.04); break;  // Glocke
    case 7: [523, 659, 784, 1047].forEach((f, i) => setTimeout(() => ton(70 + i, laut), i * 90)); break;  // Fund
    case 70: osz('triangle', 523, 524, 0.2, 0.14); break;
    case 71: osz('triangle', 659, 660, 0.2, 0.14); break;
    case 72: osz('triangle', 784, 785, 0.2, 0.14); break;
    case 73: osz('triangle', 1047, 1049, 0.5, 0.16); break;
    case 8: osz('square', 160, 110, 0.14, 0.07); break;                                     // geht nicht
    // Kampf und Funktionsblöcke
    case 'pop': osz('sine', 600, 1400, 0.08, 0.16); osz('sine', 1400, 1500, 0.05, 0.06, 0.05); break;               // Beute eingesammelt
    case 'treffer': rauschen(0.08, 0.3, 500, 'lowpass', 150); osz('sine', 160, 60, 0.12, 0.3); break;               // dumpfer Treffer
    case 'krit': rauschen(0.06, 0.2, 3000, 'highpass'); osz('triangle', 1400, 2200, 0.12, 0.14); osz('sine', 160, 60, 0.1, 0.22); break; // Krit hell
    case 'autsch': osz('square', 220, 110, 0.15, 0.08); rauschen(0.1, 0.22, 400); break;                            // selbst getroffen
    case 'schwung': rauschen(0.18, 0.18, 600, 'bandpass', 2400); break;                                             // Schwert-Schwung
    case 'schock': osz('sine', 70, 30, 0.9, 0.4); rauschen(0.8, 0.3, 300, 'lowpass', 60); break;                    // Stoßwelle (tief)
    case 'whoosh': rauschen(0.45, 0.28, 400, 'bandpass', 3000); osz('sine', 200, 700, 0.35, 0.08); break;           // Sprungplatte/Teleporter
    case 'tod': [392, 330, 262].forEach((f, i) => osz('triangle', f, f * 0.98, 0.3, 0.12, i * 0.22)); break;
  }
}
const tonAn = () => {
  if (ac) { if (ac.state === 'suspended') ac.resume(); return; }
  try { ac = new (window.AudioContext || window.webkitAudioContext)(); } catch { return; }
  try { musik.start(ac); musik.setLaut(Number(opt.musik) || 0); musik.setStimmung(musik.stimmungVonZone(zone)); wetterAnwenden(); } catch (e) { console.warn('Musik', e); }
};

/* ---------- Meldungen ---------- */
let letzteAbsage = 0;
function toast(text, art = '') {
  const t = el('div', { class: 'toast ' + art }, text);
  $('toasts').append(t);
  setTimeout(() => t.remove(), 4200);
  while ($('toasts').children.length > 4) $('toasts').firstChild.remove();
}
/** Aktionsleiste über der Schnellleiste („+1 Bruchstein | +3 Baukunst-XP“), 2,5 s sichtbar. */
let aktionUhr = 0;
function aktionsleiste(text) {
  const a = $('aktion');
  a.textContent = text; a.classList.remove('weg');
  clearTimeout(aktionUhr); aktionUhr = setTimeout(() => a.classList.add('weg'), 2500);
}
function logZeile(text) {
  const s = el('span', {}, text);
  $('log').append(s);
  while ($('log').children.length > 5) $('log').firstChild.remove();
  setTimeout(() => s.remove(), 7000);
}
const fehler = e => { toast(e.message, 'schlecht'); ton(8); };

/* ---------- Brücke zum Spiel (Rust ruft diese drei Funktionen) ---------- */
window.campusHud = {
  send(b) { if (ws?.readyState === 1) ws.send(b.slice()); },
  // Ereignisse erst nach dem laufenden Spielaufruf verarbeiten: so ruft die Oberfläche nie mitten hinein
  event(k, a, b, s) { queueMicrotask(() => ereignis(k, a, b, s)); },
  plates(a) {
    const w = innerWidth, h = innerHeight, da = new Set();
    for (let i = 0; i < a.length; i += 4) {
      const p = schilder.get(a[i]);
      if (!p) continue;
      da.add(a[i]);
      const s = Math.max(0.55, Math.min(1.1, 9 / a[i + 3]));
      p.style.transform = `translate(${(a[i + 1] * w).toFixed(1)}px, ${(a[i + 2] * h).toFixed(1)}px) translate(-50%, -100%) scale(${s.toFixed(2)})`;
      p.hidden = false;
    }
    for (const [id, p] of schilder) if (!da.has(id)) p.hidden = true;
  },
  // Schadenszahlen: je Zahl [x 0..1, y 0..1, Wert, Krit 0/1, Alter 0..1]; Krit gold mit ✦…!
  numbers(a) {
    const box = $('zahlen'), w = innerWidth, h = innerHeight;
    let n = 0;
    for (let i = 0; i + 4 < a.length; i += 5, n++) {
      let d = box.children[n];
      if (!d) box.append(d = el('b'));
      const krit = a[i + 3] > 0.5, wert = Math.round(a[i + 2]);
      d.className = krit ? 'krit' : '';
      d.textContent = krit ? `✦${wert}!` : String(wert);
      d.style.transform = `translate(${(a[i] * w).toFixed(1)}px, ${(a[i + 1] * h - a[i + 4] * 40).toFixed(1)}px) translate(-50%, -50%)`;
      d.style.opacity = (1 - a[i + 4]).toFixed(2);
      d.hidden = false;
    }
    for (let i = n; i < box.children.length; i++) box.children[i].hidden = true;
  },
};
/** Schilder auf der Insel als Plaketten (Kern-Ereignis `schilder`): id = SIGN_BASE + Index. */
const SIGN_BASE = 2000000;
function schildTexte(json) {
  let l = [];
  try { l = JSON.parse(json || '[]'); } catch {}
  for (const [id, p] of schilder) if (id >= SIGN_BASE) { p.remove(); schilder.delete(id); }
  (Array.isArray(l) ? l : []).forEach((s, k) => {
    const p = el('div', { class: 'plate schildtext', hidden: true }, el('em', {}, String(s?.[3] ?? '')));
    schilder.set(SIGN_BASE + k, p); $('plates').append(p);
  });
}

/** Freie NPCs (Kern-Ereignis `npcs`: [[200+n, Name], …]): Namen an architekt.js, Plaketten NPC_BASE + id neu anlegen. */
function npcsSetzen(json) {
  let l = [];
  try { l = JSON.parse(json || '[]'); } catch {}
  if (!Array.isArray(l)) l = [];
  architekt.npcsSetzen(l);
  for (const [id, p] of schilder) if (id >= NPC_BASE + 200 && id < SIGN_BASE) { p.remove(); schilder.delete(id); spieler.delete(id); }
  for (const e of l) {
    const id = Number(e?.[0]);
    if (!(id >= 200 && id < 256)) continue;
    spieler.set(NPC_BASE + id, { name: String(architekt.npcName(id) ?? e[1] ?? ''), plate: 0 });
    schild(NPC_BASE + id);
  }
}

function schild(id) {
  const sp = spieler.get(id);
  let p = schilder.get(id);
  if (!p) { p = el('div', { class: 'plate', hidden: true }, el('em'), el('small'), el('b')); schilder.set(id, p); $('plates').append(p); }
  const c = C(sp.plate), [blase, titel, name] = p.children;
  if (id >= NPC_BASE + 200) { titel.textContent = ''; name.textContent = sp.name; } // freie NPCs (Architekt): Name unverändert
  else if (id >= NPC_BASE) { // Personen: erstes Wort goldener Titel, Rest weißer Name
    const w = sp.name.split(' ');
    titel.textContent = w.length > 1 ? w[0] : ''; name.textContent = w.length > 1 ? w.slice(1).join(' ') : sp.name;
  } else {
    name.textContent = sp.name;
    titel.textContent = c?.titel || '';
    titel.style.color = c && c.form === 0 ? farbe(c.rgb) : '';
    name.style.borderColor = c && c.form === 1 ? farbe(c.rgb) : 'transparent';
  }
  return blase;
}

function ereignis(k, a, b, s) {
  switch (k) {
    case 'welcome': $('start').hidden = true; $('hud').hidden = false; gehalten = [-1, -1]; dispatchEvent(new Event('campus-willkommen')); ladeMich().then(zahltag); inventar.ladeInventar?.(); break;
    case 'inv': if (b > 0) inv.set(a, b); else inv.delete(a); ladeInvBald(); break;
    // Kampf, Beute, Funktionsblöcke (Kern)
    case 'hp': hpZeigen(a, b, s); break;
    case 'pop': ton('pop', 0.7); break;
    case 'treffer': ton(b ? 'autsch' : a ? 'krit' : 'treffer'); break;
    case 'schwung': ton('schwung', 0.6); break;
    case 'schock': ton('schock'); break;
    case 'schilder': schildTexte(s); break;
    case 'sprung': ton('whoosh'); break;
    case 'join': if (a !== me?.id) { spieler.set(a, { name: s, plate: b }); schild(a); } break;
    case 'leave': spieler.delete(a); schilder.get(a)?.remove(); schilder.delete(a); break;
    case 'chat': {
      const text = reg.sprueche[b], wer = a === me.id ? 'Du' : spieler.get(a)?.name;
      if (!wer) break;
      logZeile(`${wer}: ${text}`);
      if (a !== me.id) { const blase = schild(a); blase.textContent = text; clearTimeout(blase.uhr); blase.uhr = setTimeout(() => { blase.textContent = ''; }, 4500); }
      break;
    }
    case 'emote': if (a !== me.id && spieler.get(a)) logZeile(`${spieler.get(a).name}: ${reg.emotes[b]}`); break;
    case 'sfx': if (a === 4) ton('schritt', b); else ton(a, b); break;
    case 'nein': if (Date.now() - letzteAbsage > 2500) { letzteAbsage = Date.now(); toast(s, 'schlecht'); ton(8); } break;
    case 'here': hier = { id: a, name: s, darf: !!b }; minionsLaden(); tafel(); break;
    case 'world': toast('Die Welt hat sich verändert.', 'gut'); break;
    case 'teleport': uebergang(false); tafel(); break;
    case 'said': chatZeile(a, b, ...s.split('\u001f')); break;
    case 'kompass': menu(true, 'Insel-Kompass'); break;
    case 'kick': modal.zu(true); $('start').hidden = false; $('hud').hidden = true; $('start-text').textContent = s; $('los').hidden = true; ws?.close(); break;
    // Überleben: Abbau, Schritte, Hinweis am Fadenkreuz, Portale, Sprungpolster, Leere, Zonen
    case 'mine': if (TOUCH) { $('ring').hidden = !(a > 0 && a < 1); $('ring').style.setProperty('--p', Math.floor(Math.max(0, a) * 8) / 8); } break;
    case 'hit': ton('schlag', 0.8, a, b); break;
    case 'break': ton('plopp', 0.9, 0, a); $('ring').hidden = true; break;
    case 'step': ton('schritt', b || 0.5, a); break;
    case 'prompt': hinweisDaten = [a, b, s]; hinweisZeigen(); break;
    case 'portal': ton(9); uebergang(true); if (s) aktionsleiste(`Reise: ${s}`); break;
    case 'launch': ton(11); if (s) aktionsleiste(`Sprungpolster: ab zu ${s}!`); break;
    case 'void': ton(12); $('rand').classList.add('an'); setTimeout(() => $('rand').classList.remove('an'), 1400); break;
    case 'zone': if (s && s !== zone) { zone = s; tafel(); aktionsleiste(`~ ${zoneName()} ~`); musik.setStimmung(musik.stimmungVonZone(s)); wetterAnwenden(); foto.kinoPruefen(s); } break;
    case 'kino': foto.kino(!!a, b); if (a) { game?.mine?.(false); eingabe.reset(); } break;
    case 'npcs': npcsSetzen(s); break;
    case 'werkbank': menu(true, 'Werkbank'); break;
    case 'questbrett': menu(true, 'Quests'); break;
    case 'trainer': menu(true, 'Fähigkeiten'); break;
    case 'hud': {
      let j = null;
      try { j = JSON.parse(s); } catch {}
      if (!j || typeof j !== 'object') break;
      if (j.t === 'nein') ereignis('nein', 0, 0, j.text);
      else if (j.t === 'toast') { toast(j.text, j.art || ''); if (j.art === 'gut') ton(6); ladeSky(); }
      else if (j.t === 'oeffne') { if (j.reiter === 'Inventar') inventar.oeffneInventar(); else menu(true, j.reiter); }
      else if (j.t === 'inv') inventar.ladeInventar();
      else if (j.t === 'stash') inventar.stashAnzeigen(j.n);
      else if (j.t === 'staender') inventar.staenderZeigen(j);
      else if (j.t === 'handel') inventar.handelZeigen(j);
      else if (j.t === 'handel_anfrage') frageToast(`${j.name} möchte mit dir handeln.`, () => api('trade/accept', { von: j.von }).catch(fehler));
      else if (j.t === 'werte') { sky.werte = j.werte; werteAnwenden(); tafel(); }
      else if (j.t === 'boss') bossZeigen(j);
      else if (j.t === 'wetter') { wetterServer = Math.min(3, Math.max(0, j.art | 0)); wetterAnwenden(); }
      else if (j.t === 'event') eventZeigen(j);
      else if (j.t === 'duell' || j.t === 'duell_einladung') duell.duellHud(j);
      else if (j.t === 'katakomben' || j.t === 'weise') katakomben.katakombenHud(j);
      else if (j.t === 'architekt' || j.t === 'architekt_export' || j.t === 'architekt_npc' || j.t === 'npc') architekt.architektHud(j);
      else if (j.t === 'tod') todZeigen();
      else if (j.t === 'tele') { ton('whoosh'); uebergang(true); uebergang(false); aktionsleiste('Teleportiert!'); }
      else if (j.t === 'kapitel') {
        if (j.fertig) { ton(14); toast(`Kapitel ${j.nr} „${j.name}“ geschafft!`, 'gut'); } else { ton(5, 0.6); aktionsleiste(`Questbuch: Kapitel ${j.nr}, nächster Schritt`); }
        ladeSky(); if (reiter === 'Questbuch') zeige('Questbuch');
      }
      else if (j.t === 'schild_neu') schildBearbeiten(j);
      else if (j.t === 'schild') schildLesen(j.zeilen || []);
      else if (j.t === 'mana') { sky.mana = j.mana; sky.mana_max = j.max; manaZeit = performance.now(); }
      else if (j.t === 'quest') ladeSky();
      else if (j.t === 'aktion') {
        aktionsleiste(j.text);
        const i = sky.faehigkeiten?.findIndex(f => j.text.includes(f.name)) ?? -1;
        if (i >= 0) xpFach = i;
        ladeSkyBald();
      }
      else if (j.t === 'leere') { toast('Du bist ins Leere gefallen! (-25% ungesicherte Taler)', 'schlecht'); if (j.verlust > 0) toast(`-${zahl(j.verlust)} Taler`, 'schlecht'); ladeMich(); }
      else if (j.t === 'truhe') inventar.truheZeigen(j);
      else if (j.t === 'dialog') { dialog(j.npc); if (j.geschenk) { toast('Dekan Weber schenkt dir 50 Starter-Taler, einen Stein-Minion I, 6 h Lern-Energie und einen Abbau-Hammer!', 'gut'); ton(7); ladeMich(); } ladeSky(); }
      else if (j.t === 'projekt') { toast(j.fertig ? `Großprojekt „${j.name}“ ist fertig gebaut!` : `${j.von} zahlt ins Großprojekt „${j.name}“ ein: ${zahl(j.stand)} von ${zahl(j.ziel)} Talern.`, 'gut'); if (reiter === 'Projekte') zeige('Projekte'); }
      else if (j.t === 'ueberboten') { toast(`Du wurdest überboten (${j.item}). ${zahl(j.taler)} Taler sind zurück.`); ladeMich(); }
      else if (j.t === 'verkauft') { toast(`Verkauft: ${j.item} für ${zahl(j.taler)} Taler nach Gebühr.`, 'gut'); ton(6); ladeMich(); }
      else if (j.t === 'zuschlag') { toast(`Zuschlag! ${j.item} gehört dir.`, 'gut'); ton(6); ladeMich(); }
      else if (j.t === 'abgelaufen') { toast(`Niemand hat geboten: ${j.item} ist zurück in deinem Vorrat.`); ladeMich(); }
      else if (j.t === 'gebiet') { toast(`Wochenziel geschafft! Das Gebiet „${j.name}“ ist offen.`, 'gut'); ton(7); }
      else if (j.t === 'krone') { toast(`Platz ${j.platz} der Wochenwertung: Du trägst 7 Tage die Krone.`, 'gut'); ladeMich(); }
      break;
    }
  }
}

/* ---------- Konto, Zahltag, Leiste ---------- */
/** Anzeigetafel rechts: Zone, Taler, Splitter, Treibstoff, aktuelles Ziel. */
/** Hinweis am Fadenkreuz: „[E] Sprechen: Dekan Weber“ / „[E] Öffnen: Truhe“ (versteckt, solange ein Fenster offen ist). */
let hinweisDaten = [0, 0, ''];
// [E]-Texte der Blöcke 123–145 (world::USE_NEW)
const HINWEIS_NEU = { 123: 'Katakomben betreten', 124: 'Lern-Duell starten', 131: 'Prisma drehen', 132: 'Prisma drehen', 139: 'Weisen befragen', 140: 'Truhe öffnen', 142: 'Truhe öffnen', 143: 'Truhe öffnen', 141: 'Beute nehmen', 145: 'Blut-Portal betreten' };
function hinweisZeigen() {
  const [a, b, s] = hinweisDaten, h = $('hinweis');
  h.hidden = !a || !!offenesFenster();
  if (!h.hidden && a === 2 && HINWEIS_NEU[b]) h.replaceChildren(el('kbd', {}, `[${TOUCH ? 'E' : tname(opt.tasten.interagieren)}]`), ' ', el('b', {}, HINWEIS_NEU[b]));
  else if (!h.hidden) h.replaceChildren(el('kbd', {}, `[${TOUCH ? 'E' : tname(opt.tasten.interagieren)}]`), ` ${a === 1 ? 'Sprechen' : a === 3 ? 'Trainieren' : b === 77 ? 'Benutzen' : b === 103 || b === 107 ? 'Lesen' : b === 120 ? 'Ausrüsten' : b === 121 ? 'Verzaubern' : b === 122 ? 'Beschwören' : 'Öffnen'}: `, el('b', {}, a === 1 && b >= 200 ? s || String(architekt.npcName(b) ?? '') : s));
}
const zoneName = () => zone === 'Privatinsel' && hier.id ? hier.name : zone || (hier.id ? hier.name : 'Campus-Hub');
function tafel() {
  if (!me) return;
  const z = (k, v, art) => [el('span', {}, k), el('b', { class: art, title: v }, v)];
  const spl = sky.splitter ? Object.values(sky.splitter).reduce((s, n) => s + n, 0) : 0;
  const buch = sky.buch, w = sky.werte, set = w?.set != null ? reg.rpg?.sets?.find(x => x.id === w.set) : null;
  $('tafel').replaceChildren(el('h3', {}, 'CAMPUS SKYBLOCK'), el('small', {}, new Date().toLocaleDateString('de-AT')),
    el('div', { class: 'zone' }, el('span', {}, 'Zone'), el('b', { title: zoneName() }, zoneName())), ...z('Geldbörse', `${zahl(me.taler)} T`, 'taler'), ...z('Bank-Guthaben', `${zahl(sky.bank || 0)} T`, 'bank'),
    ...z('Wissens-Splitter', zahl(spl), 'splitter'), ...z('Minion-Treibstoff', sky.treibstoff_h != null ? `${zahl(sky.treibstoff_h)}h` : '—', 'treib'),
    ...(leben ? z('Leben', `♥ ${zahl(Math.ceil(leben.hp))}/${zahl(Math.round(leben.max))}`, 'leben') : []),
    ...(set ? z('Set-Bonus', set.name, 'set') : []),
    el('div', { class: 'ziel' }, el('span', {}, 'Aktuelles Ziel'), el('b', {}, sky.ziel || (sky.weber ? 'Sprich mit einer Person im Hub' : 'Sprich mit Dekan Weber am Spawn-Tempel'))),
    ...(buch?.kapitel ? [el('div', { class: 'ziel buch' }, el('span', {}, `Questbuch · Kapitel ${buch.kapitel}`),
      el('b', {}, buch.fertig ? 'Alle Kapitel geschafft!' : `${buch.text || buch.name || ''}${buch.von > 1 ? ` (${buch.stand}/${buch.von})` : ''}`))] : []));
}
/* ---------- Leben, Boss, Tod, Werte ---------- */
let leben = null, bossUhr = 0;
function hpZeigen(hp, max, schildMs) {
  const alt = leben ? Math.ceil(leben.hp) : -1;
  leben = { hp, max, schild: Number(schildMs) || 0 };
  const h = $('hp');
  h.hidden = false;
  h.style.setProperty('--f', (max > 0 ? Math.min(1, Math.max(0, hp / max)) : 0).toFixed(3));
  h.classList.toggle('schild', leben.schild > 0);
  h.lastChild.textContent = `♥ ${zahl(Math.ceil(hp))} / ${zahl(Math.round(max))}${leben.schild > 0 ? ' · Schutzschild' : ''}`;
  if (alt !== Math.ceil(hp)) tafel();
}
function bossZeigen(j) {
  const b = $('boss');
  clearTimeout(bossUhr);
  if (j.aus) { b.hidden = true; if (j.weg) return; toast(`${j.name || 'Der Große Prüfungs-Archon'} ist besiegt!`, 'gut'); ton(14); return; }
  b.hidden = false;
  b.style.setProperty('--f', (j.max > 0 ? Math.max(0, j.hp / j.max) : 0).toFixed(3));
  b.lastChild.textContent = `${j.name} · Phase ${(j.phase | 0) + 1} · ${zahl(Math.ceil(j.hp))} / ${zahl(Math.round(j.max))}`;
  bossUhr = setTimeout(() => { b.hidden = true; }, 30000); // aus der Zitadelle raus: Leiste verschwindet von selbst
}
function todZeigen() {
  ton('tod');
  $('tod').hidden = false;
  setTimeout(() => { $('tod').hidden = true; }, 2600);
}
const werteAnwenden = () => { if (sky.werte) game?.set_stats?.(Number(sky.werte.abbau) || 0, !!sky.werte.telekinese); };
/** Toast mit [Annehmen] [Ablehnen] (Handelsanfrage). Ablehnen schließt nur. */
function frageToast(text, ja) {
  const t = el('div', { class: 'toast frage' }, el('span', {}, text), el('div', { class: 'reihe' },
    el('button', { class: 'knopf klein voll', type: 'button', onclick: () => { t.remove(); ja(); } }, 'Annehmen'),
    el('button', { class: 'knopf klein', type: 'button', onclick: () => t.remove() }, 'Ablehnen')));
  $('toasts').append(t); ton(5, 0.6);
  setTimeout(() => t.remove(), 20000);
}
/* ---------- Schilder: bearbeiten (4 Zeilen à 24 Zeichen) und lesen ---------- */
function schildBearbeiten(j) {
  const felder = [0, 1, 2, 3].map(i => el('input', { type: 'text', maxlength: 24, value: j.zeilen?.[i] || '', 'aria-label': `Zeile ${i + 1}`, autocomplete: 'off' }));
  const speichern = async () => {
    try { await api('sign', { x: j.x, y: j.y, z: j.z, zeilen: felder.map(f => f.value.slice(0, 24)) }); ton(0); modal.zu(); } catch (e) { fehler(e); }
  };
  $('dialog').replaceChildren(zuKnopf(), el('h3', {}, el('span', {}, 'Schild'), 'beschriften'), el('div', { class: 'schild-zeilen' }, felder),
    el('div', { class: 'reihe' }, el('button', { class: 'knopf voll', type: 'button', onclick: speichern }, 'Speichern'), el('button', { class: 'knopf', type: 'button', onclick: () => modal.zu() }, 'Abbrechen')));
  modal.auf('dialog');
  felder[0].focus();
}
function schildLesen(zeilen) {
  $('dialog').replaceChildren(zuKnopf(), el('h3', {}, el('span', {}, 'Schild')), el('div', { class: 'schild-lesen' }, zeilen.map(z => el('div', {}, z || ' '))));
  modal.auf('dialog');
}
let xpFach = 5, skyUhr = 0; // Fach-Skill, dessen Balken über dem Mana steht (zuletzt EP bekommen)
const ladeSkyBald = () => { clearTimeout(skyUhr); skyUhr = setTimeout(ladeSky, 1200); };
async function ladeSky() {
  try {
    const neu = await api('sky'), alt = sky; // erst nach der Antwort vergleichen: zwei gleichzeitige Abrufe melden nichts doppelt
    sky = neu; manaZeit = performance.now();
    (sky.faehigkeiten || []).forEach((f, i) => {
      const v = alt.faehigkeiten?.[i];
      if (!v) return;
      if (f.ep !== v.ep) xpFach = i;
      if (f.stufe > v.stufe) { ton(14); toast(`${f.name} steigt auf Stufe ${f.stufe}!`, 'gut'); }
    });
    if (alt.quest && alt.quest.name !== sky.quest?.name) { ton(14); toast(`Quest „${alt.quest.name}“ geschafft!`, 'gut'); }
    else if (alt.quest && sky.quest && alt.quest.ziel !== sky.quest.ziel) { ton(5, 0.6); aktionsleiste(`Neues Ziel: ${sky.quest.ziel}`); }
    game?.set_bau_level?.(sky.faehigkeiten?.[5]?.stufe || 0);
    werteAnwenden(); tafel(); xpZeigen();
  } catch {}
}
function xpZeigen() {
  const f = sky.faehigkeiten?.[xpFach];
  $('xp').hidden = !f;
  if (!f) return;
  $('xp').style.setProperty('--f', (f.stufe >= 50 ? 1 : Math.min(1, f.ep / Math.max(1, f.naechste))).toFixed(3));
  $('xp').lastChild.textContent = f.stufe >= 50 ? `${f.name} ${f.stufe} · Höchststufe` : `${f.name} ${f.stufe} · ${zahl(f.ep)}/${zahl(f.naechste)} EP`;
}
let manaZeit = 0;
function manaZeigen() {
  if (sky.mana_max == null) return;
  const m = Math.min(sky.mana_max, sky.mana + (performance.now() - manaZeit) / 500);
  $('mana').style.setProperty('--f', (m / sky.mana_max).toFixed(3));
  $('mana').lastChild.textContent = `${Math.floor(m)}/${sky.mana_max} Fokus-Mana`;
}
async function minionsLaden() {
  if (!game) return;
  if (!hier.id) return game.set_minions(new Float32Array());
  try { const r = await api(`minions/${hier.id}`); hier.minions = r.minions; game.set_minions(new Float32Array(r.minions.flatMap(m => [...m.pos, m.art, m.stufe]))); } catch {}
}

/* ---------- Chat (freier Text, PC und Handy) ---------- */
let kanal = 0;
const KANAELE = ['Hub', 'Insel', 'Gruppe', 'Privat', 'System'];
function chatZeile(id, ch, name, text) {
  const wer = id === 0 ? '' : id === me.id && ch !== 3 ? 'Du' : name;
  const z = el('div', { class: `zeile k${ch}` }, el('small', {}, `[${KANAELE[ch]}] `), wer ? el('b', {}, `${wer}: `) : null, el('span', {}, text));
  $('chat-log').append(z);
  while ($('chat-log').children.length > 80) $('chat-log').firstChild.remove();
  $('chat-log').scrollTop = 1e9;
  if ($('chat').hidden) logZeile(`${wer ? wer + ': ' : ''}${text}`);
  if (id !== me.id && id && spieler.get(id) && ch < 3) { const blase = schild(id); blase.textContent = text.slice(0, 60); clearTimeout(blase.uhr); blase.uhr = setTimeout(() => { blase.textContent = ''; }, 5000); }
}
function chatAuf(an, vorgabe = '') {
  if (!an) return modal.zu();
  modal.auf('chat');
  $('chat-reiter').replaceChildren(...KANAELE.slice(0, 3).map((k, i) => el('button', { type: 'button', class: 'knopf klein', 'aria-pressed': String(i === kanal), onclick: () => { kanal = i; chatAuf(true, $('chat-text').value); } }, k)),
    el('button', { type: 'button', class: 'knopf klein', onclick: () => sprueche(true) }, 'Sprüche & Emotes'));
  $('chat-text').value = vorgabe; $('chat-text').focus();
}

/* ---------- Fenster (ModalManager): immer nur eins offen; ESC, rotes [X] oder Klick daneben schließt ---------- */
const FENSTER = ['menu', 'dialog', 'chat', 'sprueche', 'inventar'];
let menuDurchZeiger = 0; // Zeitpunkt, an dem der Zeigerverlust das Menü geöffnet hat
const offenesFenster = () => FENSTER.find(id => !$(id).hidden);
function zeigerFangen() {
  if (TOUCH || !$('start').hidden) return;
  try { $('c').requestPointerLock?.()?.catch?.(() => {}); } catch {}
}
const modal = {
  auf(id) {
    for (const f of FENSTER) if (f !== id) $(f).hidden = true;
    if (id !== 'menu') reiter = '';
    $(id).hidden = false;
    document.exitPointerLock?.(); game?.mine?.(false); eingabe.reset();
    $('hinweis').hidden = true; $('ring').hidden = true;
  },
  zu(ohneZeiger = false) {
    if (!offenesFenster()) return;
    for (const f of FENSTER) $(f).hidden = true;
    $('chat-text').blur(); reiter = ''; warteTaste = null;
    ton(13, 0.6); hinweisZeigen();
    if (!ohneZeiger) zeigerFangen();
  },
};
const zuKnopf = () => el('button', { class: 'zu', type: 'button', 'aria-label': 'Schließen', onclick: () => modal.zu() }, 'X');
function chatSenden() {
  const t = $('chat-text').value.trim();
  if (t) game.say(kanal, t);
  $('chat-text').value = '';
  chatAuf(false);
}

/* ---------- Gespräche mit Personen ---------- */
function dialog(npcId) {
  const n = reg.sky.npcs.find(x => x.id === npcId);
  if (!n) return;
  let i = 0;
  const k = (text, f) => el('button', { class: 'knopf klein', type: 'button', onclick: f }, text);
  // Knöpfe je Person (CONTRACT Abschnitt 6)
  const knoepfe = {
    1: () => [k('Quest-Tafel', () => menu(true, 'Quests'))],
    2: () => [el('input', { id: 'bank-betrag', type: 'number', min: 1, value: 50, inputmode: 'numeric', 'aria-label': 'Betrag in Talern' }), k('Einzahlen', () => bank(1)), k('Abheben', () => bank(-1)),
      el('span', { class: 'meta' }, `Auf der Bank: ${zahl(sky.bank || 0)} Taler – sicher vor Stürzen ins Leere.`)],
    3: () => [k('Fach-Skills', () => menu(true, 'Fähigkeiten')), k('Sammlungen', () => menu(true, 'Sammlungen'))],
    4: () => [k('Lern-Minions', () => menu(true, 'Minions')), k('Werkbank', () => menu(true, 'Werkbank'))],
    5: () => [k('Bazaar öffnen', () => menu(true, 'Bazaar'))],
    7: () => [k('Werkbank', () => menu(true, 'Werkbank'))],
    8: () => [k('Auktionshaus', () => menu(true, 'Auktionshaus'))],
  };
  const w = n.name.split(' '), titel = w.length > 1 ? w[0] : '', name = w.length > 1 ? w.slice(1).join(' ') : n.name;
  const zeig = () => {
    $('dialog').replaceChildren(zuKnopf(), el('h3', {}, titel ? el('span', {}, titel) : null, name), el('p', {}, n.zeilen[i]),
      el('div', { class: 'reihe' }, i < n.zeilen.length - 1 ? el('button', { class: 'knopf', type: 'button', onclick: () => { i++; zeig(); } }, 'Weiter') : null, ...(knoepfe[n.id]?.() || []),
        k('Tschüss', () => modal.zu())));
  };
  const bank = async s => { try { await api(s > 0 ? 'bank/deposit' : 'bank/withdraw', { taler: Math.max(1, +$('bank-betrag').value | 0) }); ton(6); await ladeMich(); await ladeSky(); zeig(); } catch (e) { fehler(e); } };
  zeig();
  modal.auf('dialog');
}
async function ladeMich() {
  ladeSky();
  const alt = me?.taler;
  me = await api('me');
  if (alt != null && me.taler > alt) ton(6); // Münz-Klirren bei jedem Taler-Zuwachs
  inv.clear();
  for (const [item, n] of me.inv) inv.set(item, n);
  $('b-fund').hidden = !me.funde;
  $('b-fund').textContent = me.funde === 1 ? '1 Fund öffnen' : `${me.funde} Funde öffnen`;
  leisteZeigen();
  tafel();
  return me;
}
async function zahltag() {
  try {
    const z = await api('zahltag', {});
    if (!(z.taler > 0 || z.funde > 0 || z.neu.length)) return;
    $('zahltag').replaceChildren(el('b', {}, `Zahltag! +${zahl(z.taler)} Taler`), el('br'),
      [z.funde ? `${z.funde} ${z.funde === 1 ? 'Fund' : 'Funde'} zum Öffnen. ` : '', z.bauplaene ? `${z.bauplaene} Bauplan. ` : '', z.neu.length ? `Neu erlernt: ${z.neu.join(', ')}.` : ''].join(''));
    $('zahltag').hidden = false;
    setTimeout(() => { $('zahltag').hidden = true; }, 9000);
    ton(6);
    await ladeMich();
  } catch {}  // Lernhelfer gerade nicht erreichbar: Spielen geht trotzdem
}
/** Schnellleiste = Inventar-Plätze 0..8 (inventar.js); daneben der Campus-Stern als eigener Knopf. Wahl → game.held(Platz, Gegenstand). */
let gehalten = [-1, -1], invUhr = 0; // zuletzt an den Kern gemeldet: [Platz, Gegenstand]
const ladeInvBald = () => { clearTimeout(invUhr); invUhr = setTimeout(() => inventar.ladeInventar?.(), 300); };
function leisteZeigen() {
  const p = inventar.plaetze?.() || [];
  $('leiste').replaceChildren(...Array.from({ length: 9 }, (_, i) => {
    const item = p[i]?.[0] || 0, n = p[i]?.[1] || 0;
    return el('button', { class: 'feld' + (item ? seltenheit(item) : ''), type: 'button', role: 'option', 'aria-selected': String(i === auswahl), 'aria-label': `Platz ${i + 1}: ${item ? `${nameVon(item)}, ${n} Stück` : 'leer'}`, title: item ? nameVon(item) : '', onclick: () => waehle(i) },
      item ? icon(item) : null, item && n > 1 ? el('b', {}, n > 999 ? '999+' : n) : null);
  }), el('button', { class: 'feld stern', type: 'button', 'aria-label': 'Campus-Stern: SkyBlock-Menü öffnen (M)', title: 'Campus-Stern (SkyBlock-Menü) · [M]', onclick: () => menu(true) }, pixelBild(BILD.stern), el('b', {}, 'M')));
  const item = p[auswahl]?.[0] || 0;
  if (game && (gehalten[0] !== auswahl || gehalten[1] !== item)) { gehalten = [auswahl, item]; game.held?.(auswahl, item); }
}
function waehle(i) { auswahl = (i + 9) % 9; leisteZeigen(); }
/** Isometrischer Würfel: drei Flächen, aus der Registerfarbe hell (oben), mittel (links), dunkel (rechts) schattiert. */
const schatten = (rgb, f) => farbe(rgb.map(c => Math.min(255, Math.round(c * f))));
const isoBlock = rgb => el('span', { class: 'iso', style: `--t:${schatten(rgb, 1.18)};--l:${schatten(rgb, 0.86)};--r:${schatten(rgb, 0.62)}` }, el('i', { class: 't' }), el('i', { class: 'l' }), el('i', { class: 'r' }));
/** Pixel-Symbole 8×8 (m = Werkzeugfarbe, h Griff, d dunkel, r rot, y gold, w hell). */
const BILD = {
  hacke: ['..mmmm..', '.m..hmm.', '....h.m.', '...h..m.', '..h.....', '.h......', 'h.......', '........'],
  axt: ['....mm..', '...mmmm.', '...hmmm.', '..h.mm..', '..h.....', '.h......', '.h......', 'h.......'],
  hammer: ['.mmmmmm.', '.mmmmmm.', '...hh...', '...hh...', '...hh...', '...hh...', '...hh...', '...hh...'],
  stab: ['......wm', '.....mmm', '....hmm.', '...h....', '..h.....', '.h......', 'h.......', '........'],
  haken: ['..mmm...', '.m...m..', '.m...m..', '..m..h..', '.....h..', '.....h..', '.....h..', '.....h..'],
  kompass: ['..dddd..', '.dmmmmd.', 'dmmrmmmd', 'dmmrmmmd', 'dmmdmmmd', '.dmmmmd.', '..dddd..', '........'],
  stern: ['...yy...', '...yy...', 'yyywwyyy', '.yywwyy.', '..yyyy..', '.yyyyyy.', '.yy..yy.', 'y......y'],
};
const WERKZEUG_BILD = { 1000: 'stab', 1001: 'stab', 1002: 'stab', 1010: 'hammer', 1011: 'haken', 1012: 'stab', 1013: 'kompass', 1014: 'hacke', 1015: 'axt' };
const PIX = { h: '#8a5a2b', d: '#2a2a30', r: '#d8342c', y: '#ffd34d', w: '#fff6c8' };
function pixelBild(muster, rgb = [200, 200, 210]) {
  let r = '';
  muster.forEach((zeile, y) => [...zeile].forEach((ch, x) => { if (ch !== '.') r += `<rect x="${x}" y="${y}" width="1" height="1" fill="${ch === 'm' ? farbe(rgb) : PIX[ch]}"/>`; }));
  const s = el('span', { class: 'pix' });
  s.innerHTML = `<svg viewBox="0 0 8 8" shape-rendering="crispEdges" aria-hidden="true">${r}</svg>`;
  return s;
}
/** Seltenheitsrahmen: Semester = Legende, Fund-Block = selten, Wochenblock = gut; Werkzeuge nach Stufe. */
function seltenheit(item) {
  if (item >= 2000) return ['', ' gut', ' selten', ' legende'][G(item)?.seltenheit | 0] || ''; // Gewöhnlich, Selten blau, Episch lila, Legendär gold
  const f = item < 256 ? B(item)?.f || 0 : 0;
  return f & 256 ? ' legende' : f & 16 ? ' selten' : f & 32 ? ' gut' : item === 1001 ? ' gut' : item === 1002 || item === 1012 ? ' selten' : S(item)?.verdichtet ? ' verdichtet' : '';
}

/* ---------- SkyBlock-Menü: Dashboard mit Kacheln, jede Kachel öffnet eine Seite ---------- */
let reiter = '';
// [Seite oder @Reise, Titel, Untertitel, Bild (Farbe = Würfel, Text = Pixel-Symbol)]
const KACHELN = [
  ['@insel', 'Meine Insel', '/is · zur eigenen Insel', [90, 170, 90]],
  ['@hub', 'Marktplatz-Hub', '/hub · zum Spawn-Tempel', [238, 200, 70]],
  ['@inventar', 'Inventar', '[I] · Rucksack & Ausrüstung', [150, 105, 60]],
  ['Questbuch', 'Questbuch', '[J] · 12 Kapitel', 'stern'],
  ['Insel-Kompass', 'Insel-Besuche', 'Orte & Top-Inseln', 'kompass'],
  ['Verzaubern', 'Verzaubern', 'Am Altar der Bibliothek', [120, 80, 200]],
  ['Duell-Arena', 'Duell-Arena', 'Lern-Duell & Rangliste', [70, 110, 220]],
  ['Lernhelfer', 'Lernhelfer-Belohnungen', 'Lern-Energie & Serie', [60, 100, 200]],
  ['Fähigkeiten', '7 Fach-Skills', 'Stufe 1–50 · Splitter', [130, 90, 190]],
  ['Sammlungen', 'Sammlungen', 'Stufen I–IX', [226, 140, 50]],
  ['Werkbank', 'Werkbank', 'Herstellen & 160× verdichten', [130, 96, 66]],
  ['Minions', 'Lern-Minions', 'Status & Ausbau', [120, 128, 140]],
  ['Figur', 'Charakter & Kosmetik', 'Figur mit Vorschau', [220, 120, 170]],
  ['Einstellungen', 'Einstellungen', 'Steuerung · Grafik · Ton', 'hammer'],
  ['Quests', 'Quests', 'Aktuelles Ziel', 'stern'],
  ['Bazaar', 'Bazaar', 'Aufträge & Sofort-Handel', [50, 150, 160]],
  ['Auktionshaus', 'Auktionshaus', 'Seltenes ersteigern', [190, 60, 60]],
  ['Laden', 'Laden', 'Blöcke & Cosmetics', [236, 236, 230]],
  ['Vorrat', 'Vorrat', 'Alles, was du hast', [40, 44, 56]],
  ['Grundstück', 'Grundstück', 'Ausbau & Mitbauer', [90, 140, 70]],
  ['Lerngruppe', 'Lerngruppe', 'Kasse & Wochenwertung', [220, 120, 170]],
  ['Projekte', 'Großprojekte', 'Gemeinsam bauen', [150, 150, 160]],
  ['Welt', 'Welt', 'Wochenziel & Gebiete', [110, 170, 230]],
  ['Melden', 'Melden', 'Störung melden', [200, 80, 60]],
];
function menu(an, wahl) {
  if (!an) return modal.zu();
  modal.auf('menu');
  zeige(wahl || 'Übersicht');
}
async function zeige(name) {
  if (!SEITEN[name]) name = 'Übersicht';
  reiter = name;
  $('menu-titel').textContent = name === 'Übersicht' ? 'SkyBlock-Menü' : KACHELN.find(k => k[0] === name)?.[1] || name;
  $('menu-zurueck').hidden = name === 'Übersicht';
  try {
    const inhalt = (await SEITEN[name]()).flat(Infinity).filter(Boolean);
    if (reiter === name) $('inhalt').replaceChildren(...inhalt); // inzwischen eine andere Seite gewählt: nichts überschreiben
  } catch (e) { $('inhalt').replaceChildren(el('p', {}, e.message)); }
}
function reisen(art, n) { game.warp(art, n); ton(9, 0.5); uebergang(true); modal.zu(); }
let uebergangUhr = 0;
function uebergang(an) {
  clearTimeout(uebergangUhr);
  if (an) { $('uebergang').classList.add('an'); uebergangUhr = setTimeout(() => uebergang(false), 2500); }
  else uebergangUhr = setTimeout(() => $('uebergang').classList.remove('an'), 350); // erst kurz zeigen, dann ausblenden
}
const tun = (op, body, danach) => async () => { try { const r = await api(op, body()); await ladeMich(); await danach?.(r); if (reiter) await zeige(reiter); } catch (e) { fehler(e); } };
const blockBild = rgb => el('span', { class: 'block' }, el('i', { style: `background:${farbe(rgb)}` }));
const dauer = ms => ms < 3600e3 ? `${Math.max(1, Math.round(ms / 60e3))} min` : `${Math.round(ms / 3600e3)} h`;
const tname = c => c.replace(/^Key|^Digit/, '').replace('ShiftLeft', 'Shift').replace('Space', 'Leertaste').replace('Slash', '/');

let warteTaste = null; // Aktion, die gerade auf eine neue Taste wartet
/** Bild für jeden Gegenstand: Block = Iso-Würfel, Werkzeug = Pixel-Symbol, sonst lila Würfel. */
function icon(item) {
  if (item < 256 && B(item)) return isoBlock(B(item).rgb);
  const w = WERKZEUG(item);
  if (w) return pixelBild(BILD[WERKZEUG_BILD[item] || 'stab'], w.rgb);
  return el('span', { class: `iso ding${S(item)?.verdichtet ? ' verdichtet' : ''}` }, el('i', { class: 't' }), el('i', { class: 'l' }), el('i', { class: 'r' }));
}
const ROEMISCH = ['0', 'I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X', 'XI'];
const SEITEN = {
  'Übersicht'() {
    return [el('div', { class: 'kacheln' }, KACHELN.map(([z, t, u, b]) => el('button', { class: 'kachel gross', type: 'button', onclick: () => z === '@insel' ? reisen(0, 0) : z === '@hub' ? reisen(2, 0) : z === '@inventar' ? inventar.oeffneInventar() : zeige(z) },
        typeof b === 'string' ? pixelBild(BILD[b], [230, 200, 90]) : isoBlock(b), el('span', {}, el('b', {}, t), el('small', {}, u))))),
      el('div', { class: 'reihe', style: 'margin:14px 0 0' }, el('a', { class: 'knopf klein', href: 'hilfe', target: '_blank', rel: 'noopener' }, 'Hilfe'),
        el('a', { class: 'knopf klein', href: '/' }, 'Zum Lernhelfer'), me.admin ? el('a', { class: 'knopf klein', href: 'admin', target: '_blank', rel: 'noopener' }, 'Verwaltung') : null)];
  },
  async 'Fähigkeiten'() {
    await ladeSky();
    return [el('div', { class: 'gitter' }, sky.faehigkeiten.map(f => el('div', { class: 'kachel' }, el('b', {}, `${f.name} ${f.stufe}`),
        el('progress', { max: 1, value: f.stufe >= 50 ? 1 : Math.min(1, f.ep / f.naechste) }), el('span', { class: 'meta' }, f.stufe >= 50 ? 'Höchststufe' : `${zahl(f.ep)} / ${zahl(f.naechste)} EP`)))),
      el('p', { class: 'meta' }, `Fokus-Mana: ${sky.mana_max} · Minion-Plätze: ${sky.minions.plaetze} · Jede Stufe in einem Fach gibt +0,5 % Splitter.`),
      el('h2', {}, 'Wissens-Splitter'), el('div', { class: 'reihe' }, reg.sky.faecher.map((f, i) => el('span', { class: 'chip' }, `${f}: ${zahl(Object.values(sky.splitter)[i])}`)), el('span', { class: 'chip' }, `Fokus-Kristalle: ${zahl(sky.kristalle)}`))];
  },
  async Sammlungen() {
    const r = await api('recipes');
    return [el('div', { class: 'gitter' }, r.sammlungen.sort((a, b) => b.n - a.n).map(c => el('div', { class: 'kachel' }, el('div', { class: 'kopf' }, icon(c.item), el('span', {}, nameVon(c.item), el('br'),
        el('span', { class: 'meta' }, `Stufe ${ROEMISCH[c.stufe] ?? c.stufe} · ${zahl(c.n)}${c.naechste ? ` / ${zahl(c.naechste)}` : ''}`))),
        c.naechste ? el('progress', { max: 1, value: Math.min(1, c.n / c.naechste) }) : null))),
      r.sammlungen.length ? null : el('p', { class: 'meta' }, 'Noch nichts gesammelt. Abbauen, Minions und Lernen füllen deine Sammlungen.')];
  },
  async Lernhelfer() {
    await ladeSky();
    const r = reg.regeln, zellen = inv.get(1120) || 0, h = sky.treibstoff_h || 0;
    return [el('p', {}, 'Taler, Splitter und Lern-Energie gibt es nur fürs Lernen im Lernhelfer – nie für Geld.'),
      el('h2', {}, 'Lern-Energie (Minion-Treibstoff)'), el('div', { class: 'balken' }, el('i', { style: `width:${Math.min(100, h / 72 * 100)}%` })),
      el('p', { class: 'meta' }, `${zahl(h)} h von höchstens 72 h. Mit Lern-Energie arbeiten deine Minions 5× so schnell. 15 Fokus-Minuten oder 20 Karteikarten = 24 h.`),
      zellen ? el('button', { class: 'knopf klein', type: 'button', onclick: tun('fuel/use', () => ({ n: 1 })) }, `Lern-Energie-Zelle einsetzen (+6 h, ${zellen} da)`) : null,
      el('h2', {}, 'Lernserie'), el('p', {}, `${me.serie || 0} ${me.serie === 1 ? 'Tag' : 'Tage'} am Stück. Jeder Serientag gibt +${r.serie} % auf Lern-Taler (höchstens ${r.serieMax} %).`),
      el('h2', {}, 'Belohnungen im Lernhelfer'),
      el('div', { class: 'gitter' }, [['Check-Frage', `${r.check} Taler`], ['Fokus-Minute', `${r.fokus} Taler · Tagesgrenze ${r.fokusTag}`], ['Karteikarte', `${r.karte} Taler · Tagesgrenze ${r.kartenTag}`],
        ['Probeklausur', `${r.klausur} Taler`], ['Challenge', `${r.challenge} Taler`]].map(([a, b]) => el('div', { class: 'kachel' }, el('b', {}, a), el('span', { class: 'meta' }, b)))),
      el('div', { class: 'reihe' }, el('button', { class: 'knopf voll', type: 'button', onclick: async () => { await zahltag(); if (reiter === 'Lernhelfer') zeige('Lernhelfer'); } }, 'Zahltag abholen'),
        me.funde ? el('button', { class: 'knopf', type: 'button', onclick: fundOeffnen }, me.funde === 1 ? '1 Fund öffnen' : `${me.funde} Funde öffnen`) : null,
        el('a', { class: 'knopf', href: '/' }, 'Zum Lernhelfer'))];
  },
  async Quests() {
    await ladeSky();
    const q = sky.quest;
    return [el('div', { class: 'kachel', style: 'margin-bottom:12px' }, el('b', {}, q ? q.name : 'Keine offene Quest'), el('span', {}, q ? q.ziel : sky.ziel || 'Sprich mit Dekan Weber am Spawn-Tempel.'),
        q && q.von > 1 ? el('div', { class: 'balken' }, el('i', { style: `width:${Math.min(100, 100 * q.stand / q.von)}%` })) : null, q && q.von > 1 ? el('span', { class: 'meta' }, `${q.stand} / ${q.von}`) : null),
      el('h2', {}, 'Alle Quests'), reg.sky.quests.map(x => el('div', { class: 'kachel', style: 'margin-bottom:8px' }, el('b', {}, x.name),
        el('ol', { class: 'meta', style: 'margin:0;padding-left:1.6em' }, x.schritte.map(s => el('li', { style: q && x.name === q.name && s === q.ziel ? 'color:#ffd34d' : '' }, s)))))];
  },
  async Questbuch() {
    await ladeSky();
    const q = await api('questbuch').catch(() => null), liste = Array.isArray(q) ? q : q?.kapitel || [];
    const b = sky.buch || {}, akt = b.kapitel || 1, jetzt = b.schritt | 0;
    // Status vom Server (erledigt/aktiv/offen), sonst aus dem Stand des Questbuchs gerechnet
    return [el('p', { class: 'meta' }, 'Zwölf Kapitel führen dich durch Campus. Jedes geschaffte Kapitel bringt Taler und Wissens-Splitter. [J] öffnet das Questbuch.'),
      (reg.rpg?.kapitel || []).map((c, i) => {
        const nr = i + 1, x = liste[i], st = x?.status || (b.fertig || nr < akt ? 'erledigt' : nr === akt ? 'aktiv' : 'offen');
        const erledigt = k => { const s = x?.schritte?.[k]; return typeof s === 'boolean' ? s : s && typeof s === 'object' ? !!(s.fertig ?? s.erledigt ?? s[1]) : st === 'erledigt' || (st === 'aktiv' && k < jetzt); };
        return el('div', { class: `kachel kapitel ${st}`, style: 'margin-bottom:8px' }, el('b', {}, `${st === 'erledigt' ? '✔ ' : ''}Kapitel ${nr}: ${c.name}`), c.text ? el('span', { class: 'meta' }, c.text) : null,
          st === 'offen' ? el('span', { class: 'meta' }, 'Noch verschlossen.') : el('ol', { class: 'schritte' }, (c.schritte || []).map((s, k) => {
            const ok = erledigt(k), dran = st === 'aktiv' && !ok && k === jetzt;
            return el('li', { class: ok ? 'fertig' : dran ? 'jetzt' : '' }, `${ok ? '✔ ' : ''}${s}${dran && b.von > 1 ? ` (${b.stand}/${b.von})` : ''}`);
          })),
          el('span', { class: 'meta' }, `Belohnung: ${zahl(c.taler || 0)} Taler · ${zahl(c.splitter || 0)} Splitter`));
      })];
  },
  async Verzaubern() {
    const r = await inventar.verzaubernSeite();
    return r instanceof Node ? [r] : [...(r || [])];
  },
  async 'Duell-Arena'() {
    const r = await duell.ranglisteSeite();
    return r instanceof Node ? [r] : [...(r || [])];
  },
  async Minions() {
    await ladeSky();
    const eigene = [...inv.keys()].filter(i => i >= 1200 && i < 1400);
    const setzen = item => async () => {
      const t = game.target_prev();
      if (!t.length) return toast('Schau auf den Boden deiner Insel, wo der Minion stehen soll.', 'schlecht');
      try { await api('minion/place', { item, x: t[0], y: t[1], z: t[2] }); ton(6); await ladeMich(); menu(false); minionsLaden(); } catch (e) { fehler(e); }
    };
    const hol = (id, abholen) => tun('minion/collect', () => ({ id, abholen }), r => { if (r.menge) toast(`+${r.menge} ${nameVon(r.item)}`, 'gut'); minionsLaden(); });
    return [el('p', { class: 'meta' }, `Plätze: ${sky.minions.belegt}/${sky.minions.plaetze} · Lern-Energie: ${sky.treibstoff_h} h (mit Treibstoff 5× so schnell; 15 Fokus-Minuten oder 20 Karteikarten im Lernhelfer = 24 h, höchstens 72 h).`),
      inv.get(1120) ? el('button', { class: 'knopf klein', type: 'button', onclick: tun('fuel/use', () => ({ n: 1 })) }, `Lern-Energie-Zelle einsetzen (+6 h, ${inv.get(1120)} da)`) : null,
      el('h2', {}, 'Auf dieser Insel'), el('div', { class: 'gitter' }, (hier.minions || []).map(m => el('div', { class: 'kachel' }, el('b', {}, m.name), el('span', { class: 'meta' }, `Lager ${m.lager}/${m.max}`),
        m.user === me.id ? el('div', { class: 'reihe' }, el('button', { class: 'knopf klein', type: 'button', onclick: hol(m.id, false) }, 'Leeren'), el('button', { class: 'knopf klein', type: 'button', onclick: hol(m.id, true) }, 'Abholen'),
          el('button', { class: 'knopf klein', type: 'button', title: 'Erst abholen, dann an der Werkbank aufwerten', onclick: () => menu(true, 'Werkbank') }, 'Aufwerten')) : null))),
      el('h2', {}, 'Im Vorrat'), el('div', { class: 'reihe' }, eigene.map(i => el('button', { class: 'knopf', type: 'button', onclick: setzen(i) }, `${nameVon(i)} (${inv.get(i)}) aufstellen`)),
        eigene.length ? el('button', { class: 'knopf', type: 'button', onclick: () => menu(true, 'Werkbank') }, 'Aufwerten (Werkbank)') : null),
      eigene.length ? el('p', { class: 'meta' }, 'Erst Menü schließen? Nein: schau vorher auf den Platz, dann hier aufstellen.') : el('p', { class: 'meta' }, 'Keine Minions im Vorrat. Dekan Weber schenkt dir den ersten.')];
  },
  async Werkbank() {
    const r = await api('recipes');
    const reicht = x => x.ins.every(([i, n]) => (inv.get(i) || 0) >= n);
    const liste = r.rezepte.filter(x => x.frei).sort((a, b) => reicht(b) - reicht(a));
    return [el('p', { class: 'meta' }, '160 Stück ergeben 1 verdichtete Stufe, 160 verdichtete eine super-verdichtete. Neue Rezepte schalten Sammlungen frei.'),
      el('div', { class: 'gitter' }, liste.map(x => el('div', { class: 'kachel' }, el('div', { class: 'kopf' }, icon(x.out), el('span', {}, `${x.n > 1 ? x.n + '× ' : ''}${nameVon(x.out)}`, el('br'),
        el('span', { class: 'meta' }, x.ins.map(([i, n]) => `${n}× ${nameVon(i)} (${inv.get(i) || 0})`).join(' + ')))),
        el('button', { class: 'knopf klein', type: 'button', disabled: !reicht(x), onclick: tun('craft', () => ({ item: x.out, mal: 1 }), () => ton(6)) }, 'Herstellen')))),
      el('p', { class: 'meta' }, `${r.rezepte.length - liste.length} Rezepte noch gesperrt.`)];
  },
  async Bazaar() {
    const b = await api('bazaar');
    const handel = (item, kauf, sofort, menge, preis) => tun('bazaar/order', () => ({ item, kauf, sofort, menge, preis }), r => { toast(`${r.gehandelt} gehandelt${r.offen ? `, ${r.offen} als Auftrag offen` : ''}.`, 'gut'); ton(6); });
    const form = w => { const p = el('input', { type: 'number', min: 1, value: w.kauf?.[0] || w.verkauf?.[0] || 1, 'aria-label': 'Preis je Stück' }), m = el('input', { type: 'number', min: 1, value: 16, 'aria-label': 'Menge' });
      return el('div', { class: 'reihe' }, 'Preis', p, 'Menge', m, el('button', { class: 'knopf klein', type: 'button', onclick: () => handel(w.item, true, false, +m.value, +p.value)() }, 'Kaufauftrag'), el('button', { class: 'knopf klein', type: 'button', onclick: () => handel(w.item, false, false, +m.value, +p.value)() }, 'Verkaufsangebot')); };
    const waren = b.waren.filter(w => w.kauf || w.verkauf || inv.get(w.item));
    return [el('p', { class: 'meta' }, `Sofort kaufen zum besten Angebot, sofort verkaufen an den besten Auftrag – oder eigene Aufträge stellen. ${b.steuer_bp / 100} % vom Erlös werden vernichtet.`),
      el('div', { class: 'gitter' }, waren.map(w => el('div', { class: 'kachel' }, el('div', { class: 'kopf' }, icon(w.item), el('span', {}, w.name, el('br'),
        el('span', { class: 'meta' }, `Kaufen ab ${w.kauf ? zahl(w.kauf[0]) : '—'} · Verkaufen für ${w.verkauf ? zahl(w.verkauf[0]) : '—'} · du hast ${inv.get(w.item) || 0}`))),
        el('div', { class: 'reihe' }, el('button', { class: 'knopf klein', type: 'button', disabled: !w.kauf, onclick: handel(w.item, true, true, 1, 0) }, 'Sofort 1 kaufen'),
          el('button', { class: 'knopf klein', type: 'button', disabled: !w.verkauf || !inv.get(w.item), onclick: handel(w.item, false, true, inv.get(w.item) || 1, 0) }, 'Alles sofort verkaufen')), form(w)))),
      waren.length ? null : el('p', { class: 'meta' }, 'Noch keine Angebote. Stell das erste ein: baue Bruchstein ab und biete ihn an.'),
      el('h2', {}, 'Deine Aufträge'), el('div', { class: 'gitter' }, b.meine.map(o => el('div', { class: 'kachel' }, `${o.kauf ? 'Kauf' : 'Verkauf'}: ${o.offen}/${o.menge} ${nameVon(o.item)} zu ${zahl(o.preis)}`,
        el('button', { class: 'knopf klein', type: 'button', onclick: tun('bazaar/cancel', () => ({ id: o.id })) }, 'Zurückziehen'))))];
  },
  async 'Insel-Kompass'() {
    const knopf = (k, t) => el('button', { class: 'knopf klein', type: 'button', 'aria-pressed': String(kompassReiter === k), onclick: () => { kompassReiter = k; zeige('Insel-Kompass'); } }, t);
    const kopf = el('div', { class: 'reihe' }, knopf('reisen', 'Reisen'), knopf('top', 'Top-Inseln'));
    if (kompassReiter === 'top') return [kopf, await topInseln()];
    const k = await api('islands');
    const reise = (art, n) => () => reisen(art, n);
    return [kopf, el('h2', {}, 'Reisen'),
      el('div', { class: 'reihe' }, el('button', { class: 'knopf', type: 'button', onclick: reise(0, 0) }, 'Eigene Insel'), k.orte.map((o, i) => el('button', { class: 'knopf', type: 'button', onclick: reise(2, i) }, o))),
      el('h2', {}, 'Inseln besuchen'),
      el('div', { class: 'gitter' }, k.inseln.filter(i => !i.eigen).map(i => el('button', { class: 'kachel knopf', type: 'button', onclick: reise(1, i.id) }, `${i.gruppe ? 'Gruppe ' : ''}${i.name}`))),
      k.inseln.length < 2 ? el('p', { class: 'meta' }, 'Noch keine offenen Inseln anderer.') : null,
      el('h2', {}, 'Wer darf deine Insel besuchen?'),
      el('div', { class: 'reihe' }, k.modi.map((m, i) => el('button', { class: 'knopf klein', type: 'button', 'aria-pressed': String(i === k.modus), onclick: tun('plot/visit', () => ({ modus: i })) }, m))),
      el('p', { class: 'meta' }, 'Besucher können nichts abbauen oder bauen; das dürfen nur du und deine Mitbauer.')];
  },
  Einstellungen() {
    const regler = (name, key, min, max, step, fmt = v => v) => el('label', { class: 'regler' }, el('span', {}, name), el('b', {}, fmt(opt[key])),
      el('input', { type: 'range', min, max, step, value: opt[key], oninput: e => { opt[key] = +e.target.value; e.target.previousSibling.textContent = fmt(opt[key]); optSpeichern(); } }));
    // Auswahl aus festen Werten als Knopfreihe (gedrückt = aktiv)
    const wahl = (name, key, werte) => el('div', { class: 'stufenwahl', role: 'group', 'aria-label': name }, el('span', {}, name),
      el('div', { class: 'reihe' }, werte.map(([v, text]) => el('button', { class: 'knopf klein', type: 'button', 'aria-pressed': String(opt[key] === v),
        onclick: e => { opt[key] = v; optSpeichern(); for (const b of e.currentTarget.parentNode.children) b.setAttribute('aria-pressed', String(b === e.currentTarget)); const i = $('stufe-info'); if (i) i.textContent = stufeText(); } }, text))));
    const schalter = (name, key) => el('label', { class: 'schalter' }, el('input', { type: 'checkbox', checked: opt[key], onchange: e => { opt[key] = e.target.checked; optSpeichern(); } }), name);
    return [el('h2', {}, 'Steuerung'),
      regler('Maus-Empfindlichkeit', 'maus', 0.2, 3, 0.1, v => v.toFixed(1)), regler('Touch-Empfindlichkeit', 'touch', 0.2, 3, 0.1, v => v.toFixed(1)), schalter('Y-Achse umkehren', 'invertY'),
      el('h2', {}, 'Tasten (anklicken, dann neue Taste drücken; Esc bricht ab)'),
      el('div', { class: 'tasten' }, Object.entries(AKTIONEN).map(([a, name]) => el('button', { class: 'knopf klein', type: 'button', 'data-aktion': a,
        onclick: e => { warteTaste = a; const b = e.currentTarget; b.classList.add('wartet'); b.lastChild.textContent = '…'; } }, el('span', {}, name), el('kbd', {}, tname(opt.tasten[a]))))),
      el('p', { class: 'meta' }, 'Fest: linke Maustaste halten = abbauen bzw. zuschlagen, rechte = setzen, 1–9 und Mausrad = Leiste, I = Inventar, J = Questbuch, Tab = SkyBlock-Menü, Esc = Menü.'),
      el('h2', {}, 'Grafik (nur dein Gerät)'),
      regler('Sichtweite', 'stuecke', 4, 12, 1, v => `${v} Stücke`), regler('Blickwinkel', 'fov', 60, 110, 1, v => `${v}°`),
      wahl('Shader-Qualität', 'grafik', [['auto', 'Auto'], [0, 'Eco'], [1, 'Hoch'], [2, 'Ultra'], [3, 'Extrem']]),
      el('p', { class: 'meta', id: 'stufe-info' }, stufeText()),
      wahl('Tageszeit / Himmel', 'himmel', [[0, 'Dynamisch'], [1, 'Tag'], [2, 'Sonnenuntergang'], [3, 'Nacht']]),
      texturWahl(),
      regler('Bildrate', 'fpsMax', 0, 2, 1, v => ['Max', '60', '30'][v]), regler('Auflösung', 'aufloesung', 0.5, 1, 0.05, v => `${Math.round(v * 100)} %`),
      schalter('Weiche Eckenschatten', 'ao'), schalter('Wolken', 'wolken'), schalter('Partikel', 'partikel'),
      el('h2', {}, 'Ton'), regler('Lautstärke', 'laut', 0, 1, 0.05, v => `${Math.round(v * 100)} %`),
      regler('Musik-Lautstärke', 'musik', 0, 1, 0.05, v => v > 0 ? `${Math.round(v * 100)} %` : 'Aus'),
      el('h2', {}, 'Wetter'), wahl('Wetter', 'wetter', [[0, 'Auto'], [1, 'Klar'], [2, 'Sommerregen'], [3, 'Schneefall'], [4, 'Kirschblüten']]),
      el('p', { class: 'meta' }, '„Auto“ zeigt das Wetter, das die Verwaltung für alle einstellt. F8 öffnet den Foto-Modus.'),
      el('button', { class: 'knopf klein', type: 'button', onclick: () => { opt = structuredClone(STANDARD); optSpeichern(); texturAnwenden(); zeige('Einstellungen'); } }, 'Alles zurücksetzen')];
  },
  Laden() {
    const kaufen = (item, n) => tun('shop/buy', () => ({ item, n }), () => ton(6));
    const kachel = b => el('div', { class: 'kachel' }, el('div', { class: 'kopf' }, isoBlock(b.rgb), el('span', {}, b.name, el('br'), el('span', { class: 'meta' }, `${b.preis} Taler · du hast ${inv.get(b.id) || 0}`))),
      el('div', { class: 'reihe' }, [1, 16, 64].map(n => el('button', { class: 'knopf klein', type: 'button', disabled: me.taler < b.preis * n, onclick: kaufen(b.id, n) }, `+${n}`))));
    const woche = reg.bloecke.filter(b => me.wochenbloecke.includes(b.id)), normal = reg.bloecke.filter(b => b.preis > 0 && !(b.f & 32));
    const stuecke = reg.cosmetics.filter(c => c.weg === 'Shop' && c.preis > 0);
    return [el('p', { class: 'meta' }, `Du hast ${zahl(me.taler)} Taler. Taler gibt es nur fürs Lernen im Lernhelfer, nie für Geld.`),
      el('h2', {}, 'Wochenblöcke (nur diese Woche)'), el('div', { class: 'gitter' }, woche.map(kachel)),
      el('h2', {}, 'Blöcke'), el('div', { class: 'gitter' }, normal.map(kachel)),
      el('h2', {}, 'Cosmetics'), el('div', { class: 'gitter' }, stuecke.map(c => el('div', { class: 'kachel' },
        el('div', { class: 'kopf' }, blockBild(c.rgb), el('span', {}, c.name, el('br'), el('span', { class: 'meta' }, `${reg.plaetze[c.platz]} · ${c.preis} Taler`))),
        inv.has(c.id) ? el('span', { class: 'meta' }, 'Gehört dir') : el('button', { class: 'knopf klein', type: 'button', disabled: me.taler < c.preis, onclick: kaufen(c.id, 1) }, 'Kaufen'))))];
  },
  Vorrat() {
    const handelbar = item => item === reg.bauplan || (item < 256 ? !!(B(item)?.f & 16) : ['Drop', 'Semester'].includes(C(item)?.weg));
    const zeile = ([item, n]) => {
      const bild = item >= 300 && item < 1000 ? blockBild(C(item)?.rgb || [240, 200, 90]) : icon(item);
      return el('div', { class: 'zeile' }, el('span', { class: 'reihe', style: 'margin:0' }, bild, `${nameVon(item)} × ${n}`),
        el('span', { class: 'reihe', style: 'margin:0' },
          handelbar(item) ? el('button', { class: 'knopf klein', type: 'button', onclick: () => anbieten(item, n) }, 'Anbieten') : null));
    };
    return [me.funde ? el('div', { class: 'reihe' }, el('b', {}, `${me.funde} ${me.funde === 1 ? 'Fund wartet' : 'Funde warten'}.`), el('button', { class: 'knopf voll', type: 'button', onclick: fundOeffnen }, 'Fund öffnen')) : el('p', { class: 'meta' }, 'Funde bekommst du für bestandene Check-Fragen. Kaufen kann man sie nicht.'),
      el('h2', {}, 'Dein Vorrat'), el('button', { class: 'knopf klein', type: 'button', onclick: () => inventar.oeffneInventar() }, 'Inventar öffnen [I]'), [...inv].sort((a, b) => a[0] - b[0]).map(zeile)];
  },
  async Auktionshaus() {
    const a = await api('auctions');
    return [el('p', { class: 'meta' }, `Gehandelt werden seltene Blöcke, Baupläne und gefundene Cosmetics. Gebote werden sofort reserviert und beim Überbieten zurückgebucht. ${a.gebuehr} % Gebühr verschwinden aus dem Spiel. Anbieten kannst du im Reiter „Vorrat“.`),
      a.liste.length ? null : el('p', {}, 'Gerade gibt es keine Angebote.'),
      a.liste.map(x => {
        const feld = el('input', { type: 'number', min: x.mindest, value: x.mindest, 'aria-label': 'Gebot in Talern' });
        return el('div', { class: 'zeile' }, el('span', {}, el('b', {}, `${x.name} × ${x.menge}`), el('br'),
          el('span', { class: 'meta' }, `von ${x.von} · ${x.gebot ? `Gebot ${zahl(x.gebot)}` : `Start ${zahl(x.start)}`} Taler${x.fuehre ? ' (du führst)' : ''} · noch ${dauer(x.ende - a.jetzt)}`)),
          x.meins ? el('span', { class: 'meta' }, 'dein Angebot') : el('span', { class: 'reihe', style: 'margin:0' }, feld,
            el('button', { class: 'knopf klein', type: 'button', onclick: tun('auction/bid', () => ({ id: x.id, taler: Number(feld.value) })) }, 'Bieten'),
            x.sofort ? el('button', { class: 'knopf klein voll', type: 'button', onclick: tun('auction/buyout', () => ({ id: x.id }), () => ton(6)) }, `Sofort für ${zahl(x.sofort)}`) : null));
      })];
  },
  Figur() {
    const l = me.look, setze = (k, v) => tun('look', () => ({ ...l, [k]: v }));
    const wahl = (titel, k, liste, alsFarbe) => [el('h2', {}, titel), el('div', { class: alsFarbe ? 'farben' : 'reihe' }, liste.map((x, i) => alsFarbe
      ? el('button', { type: 'button', style: `background:${farbe(x)}`, 'aria-pressed': String(l[k] === i), 'aria-label': `${titel} ${i + 1}`, onclick: setze(k, i) })
      : el('button', { class: 'knopf klein' + (l[k] === i ? ' voll' : ''), type: 'button', onclick: setze(k, i) }, x)))];
    const besitz = new Set([...inv.keys(), ...me.zeit.map(z => z[0]), 360]);
    return [el('div', { class: 'vorschau' }, el('div', {}, avatarVorschau(), el('p', { class: 'meta' }, 'Ziehen dreht die Figur. So sehen dich die anderen.')),
      el('div', {}, wahl('Hautton', 'skin', reg.haut, true), wahl('Frisur', 'hair', reg.haare), wahl('Haarfarbe', 'hair_col', reg.farben, true),
        wahl('Kleidung', 'outfit', reg.kleidung), wahl('Farbe oben', 'col_a', reg.farben, true), wahl('Farbe unten', 'col_b', reg.farben, true))),
      reg.plaetze.map((platz, s) => {
        const meine = reg.cosmetics.filter(c => c.platz === s && besitz.has(c.id));
        return [el('h2', {}, platz), meine.length ? el('div', { class: 'reihe' }, meine.map(c => el('button', { class: 'knopf klein' + (l.slots[s] === c.id ? ' voll' : ''), type: 'button',
          onclick: tun('equip', () => ({ slot: s, item: l.slots[s] === c.id ? 0 : c.id })) }, c.name))) : el('p', { class: 'meta' }, s === 5 ? '' : 'Noch nichts. Erlernen, im Laden kaufen oder in Funden entdecken.')];
      })];
  },
  'Grundstück'() {
    const p = me.plot;
    if (!p) return [el('p', {}, 'Gerade ist kein Grundstück frei. Wenn alle zusammen das Wochenziel schaffen, öffnet sich ein neues Gebiet mit neuen Grundstücken. Bis dahin kannst du im Hub Erz, Holz und Steine abbauen; dort wachsen sie nach.')];
    const feld = el('input', { type: 'text', maxlength: 24, placeholder: 'Name im Campus', 'aria-label': 'Name' });
    const coop = el('input', { type: 'text', maxlength: 24, placeholder: 'Name im Campus', 'aria-label': 'Name für Co-op' });
    return [el('p', {}, `Dein Grundstück ist ${p.size} × ${p.size} Blöcke groß und liegt bei ${p.rect[0]}, ${p.rect[1]}. Die grünen Leuchtlinien zeigen die Grenze.`),
      p.ausbau ? el('div', { class: 'reihe' }, el('button', { class: 'knopf voll', type: 'button', disabled: me.taler < p.ausbau.taler || me.stufe < p.ausbau.stufe, onclick: tun('plot/expand', () => ({})) },
        `Ausbauen auf ${p.ausbau.size} × ${p.ausbau.size}`), el('span', { class: 'meta' }, `${zahl(p.ausbau.taler)} Taler${p.ausbau.bauplaene ? ' und 1 Bauplan' : ''}, ab Stufe ${p.ausbau.stufe} (du: Stufe ${me.stufe})`)) : el('p', { class: 'meta' }, 'Voll ausgebaut.'),
      el('h2', {}, 'Wer mitbauen darf'), p.bauer.map(n => el('div', { class: 'zeile' }, n, el('button', { class: 'knopf klein', type: 'button', onclick: tun('plot/acl', () => ({ name: n, add: false })) }, 'Entfernen'))),
      el('div', { class: 'reihe' }, feld, el('button', { class: 'knopf', type: 'button', onclick: tun('plot/acl', () => ({ name: feld.value.trim(), add: true })) }, 'Eintragen')),
      el('h2', {}, 'Co-op auf deiner Privatinsel'),
      el('p', { class: 'meta' }, 'Co-op-Mitglieder (höchstens 4) bauen, ernten und benutzen Truhen auf deiner ganzen Insel. Im Chat geht es auch: /coop invite Name, /coop remove Name, /coop.'),
      (p.coop || []).map(n => el('div', { class: 'zeile' }, n, el('button', { class: 'knopf klein', type: 'button', onclick: tun('coop', () => ({ name: n, add: false })) }, 'Entfernen'))),
      el('div', { class: 'reihe' }, coop, el('button', { class: 'knopf', type: 'button', onclick: tun('coop', () => ({ name: coop.value.trim(), add: true }), () => toast(`${coop.value.trim()} ist jetzt im Co-op.`, 'gut')) }, 'Einladen'),
        el('button', { class: 'knopf', type: 'button', onclick: tun('coop', () => ({ name: coop.value.trim(), add: false })) }, 'Entfernen'))];
  },
  async Lerngruppe() {
    const [g, r] = await Promise.all([api('groups'), api('ranking')]);
    const feld = el('input', { type: 'text', maxlength: 24, placeholder: 'Name der Lerngruppe', 'aria-label': 'Name der Lerngruppe' }), geld = el('input', { type: 'number', min: 1, value: 50, 'aria-label': 'Taler' });
    return [me.gruppe ? [el('h2', {}, me.gruppe.name), el('p', {}, `Kasse: ${zahl(me.gruppe.kasse)} Taler. Die Kasse zahlt in Großprojekte ein und baut das Gruppen-Grundstück aus; auszahlen lässt sie sich nicht.`),
        el('div', { class: 'reihe' }, geld, el('button', { class: 'knopf', type: 'button', onclick: tun('group/deposit', () => ({ taler: Number(geld.value) })) }, 'In die Kasse einzahlen'),
          me.gruppe.leitung ? el('button', { class: 'knopf', type: 'button', onclick: tun('plot/expand', () => ({ gruppe: true })) }, 'Gruppen-Grundstück ausbauen') : null,
          el('button', { class: 'knopf', type: 'button', onclick: tun('group/leave', () => ({})) }, 'Verlassen')),
        el('h2', {}, 'Diese Woche erlernt'), g.mitglieder.map(m => el('div', { class: 'zeile' }, m.name, el('b', {}, `${zahl(m.woche)} Taler`)))]
      : [el('p', {}, `Eine Lerngruppe bekommt ein eigenes Grundstück im Gruppen-Viertel, eine gemeinsame Kasse und eine Wochenwertung. Gründen kostet ${g.kosten} Taler; noch ${g.frei} ${g.frei === 1 ? 'Platz' : 'Plätze'} frei.`),
        el('div', { class: 'reihe' }, feld, el('button', { class: 'knopf voll', type: 'button', disabled: !g.frei, onclick: tun('group/create', () => ({ name: feld.value.trim() })) }, 'Gründen'))],
      el('h2', {}, 'Wochenwertung der Lerngruppen'), g.liste.length ? g.liste.map(x => el('div', { class: 'zeile' }, `${x.name} (${x.mitglieder})`, el('span', { class: 'reihe', style: 'margin:0' }, el('b', {}, `${zahl(x.woche)} Taler`),
        !me.gruppe ? el('button', { class: 'knopf klein', type: 'button', onclick: tun('group/join', () => ({ id: x.id })) }, 'Beitreten') : null))) : el('p', { class: 'meta' }, 'Noch keine Lerngruppe.'),
      el('h2', {}, 'Wochenwertung (Platz 1 bis 3 tragen nächste Woche die Krone)'), r.spieler.length ? r.spieler.map((s, i) => el('div', { class: 'zeile' }, `${i + 1}. ${s.name}`, el('b', {}, `${zahl(s.taler)} Taler`))) : el('p', { class: 'meta' }, 'Diese Woche hat noch niemand Taler erlernt.')];
  },
  async Projekte() {
    const p = await api('projects');
    return [el('p', { class: 'meta' }, 'Großprojekte baut die ganze Welt zusammen: alle zahlen ein, und wenn der Balken voll ist, steht das Bauwerk.'),
      p.liste.map(x => {
        const feld = el('input', { type: 'number', min: 1, value: Math.min(100, x.ziel - x.stand), 'aria-label': 'Taler' });
        return el('div', { class: 'kachel', style: 'margin-bottom:8px' }, el('b', {}, x.name, x.fertig ? ' – fertig' : ''), el('div', { class: 'balken' }, el('i', { style: `width:${100 * x.stand / x.ziel}%` })),
          el('span', { class: 'meta' }, `${zahl(x.stand)} von ${zahl(x.ziel)} Talern · ${x.spender} ${x.spender === 1 ? 'Konto hat' : 'Konten haben'} eingezahlt`),
          x.offen ? el('div', { class: 'reihe' }, feld, el('button', { class: 'knopf voll', type: 'button', onclick: tun('project/fund', () => ({ key: x.key, taler: Number(feld.value) }), () => ton(6)) }, 'Einzahlen'),
            me.gruppe ? el('button', { class: 'knopf', type: 'button', onclick: tun('project/fund', () => ({ key: x.key, taler: Number(feld.value), gruppe: true })) }, 'Aus der Gruppenkasse') : null) : x.fertig ? null : el('span', { class: 'meta' }, 'Kommt nach dem Projekt davor an die Reihe.'));
      })];
  },
  async Welt() {
    const w = await api('world');
    return [el('h2', {}, 'Wochenziel der Welt'), el('p', {}, `Lernen alle zusammen in einer Woche ${zahl(w.ziel)} Taler, öffnet sich ein neues Gebiet mit neuen Grundstücken.`),
      el('div', { class: 'balken' }, el('i', { style: `width:${Math.min(100, 100 * w.gelernt / w.ziel)}%` })), el('p', { class: 'meta' }, `${zahl(w.gelernt)} von ${zahl(w.ziel)} Talern${w.erreicht ? ' – diese Woche geschafft' : ''}`),
      el('h2', {}, 'Gebiete'), w.namen.map((n, i) => el('div', { class: 'zeile' }, n, el('span', { class: 'meta' }, i < w.gebiete ? 'offen' : 'noch verschlossen'))),
      el('p', { class: 'meta' }, `Gerade online: ${me.online}. Grafik: ${game.backend()}.`)];
  },
  Melden() {
    const grund = el('select', { 'aria-label': 'Grund' }, reg.gruende.map(g => el('option', {}, g)));
    const wen = el('select', { 'aria-label': 'Was melden' }, hier.id ? el('option', { value: 'p' + hier.id }, `Bau auf dem Grundstück von ${hier.name}`) : null, [...spieler].map(([id, s]) => el('option', { value: 'u' + id }, `Spieler ${s.name}`)));
    return [el('p', {}, 'Etwas ist beleidigend, zerstört oder nervt? Melde den Bau (stell dich auf das Grundstück) oder die Person. Die Verwaltung sieht die Meldung und kann den Bau zurücksetzen oder das Konto für Campus sperren.'),
      wen.options.length ? el('div', { class: 'reihe' }, wen, grund, el('button', { class: 'knopf voll', type: 'button', onclick: tun('report', () => ({ user: wen.value[0] === 'u' ? Number(wen.value.slice(1)) : 0, plot: wen.value[0] === 'p' ? Number(wen.value.slice(1)) : 0, grund: grund.value }), () => toast('Danke, die Meldung ist angekommen.', 'gut')) }, 'Melden'))
        : el('p', { class: 'meta' }, 'Gerade ist niemand in der Nähe und du stehst auf keinem Grundstück.')];
  },
};
/* ---------- Insel-Kompass: Top-Inseln mit Likes und Stempeln ---------- */
let kompassReiter = 'reisen';
const STEMPEL = ['♥ Schön', '⚒ Gut gebaut', '✿ Gemütlich', '✦ Kreativ', '☀ Riesig', '★ Lehrreich'];
async function topInseln() {
  const l = await api('top_inseln'), liste = Array.isArray(l) ? l : l.liste || [];
  const stempel = el('select', { 'aria-label': 'Stempel' }, STEMPEL.map((s, i) => el('option', { value: i }, s)));
  const liken = plot => tun('like', () => ({ plot, stempel: Number(stempel.value) }), () => { toast('Danke für dein Like!', 'gut'); ton(6); });
  const knopf = (text, f) => el('button', { class: 'knopf klein', type: 'button', onclick: f }, text);
  return [el('div', { class: 'reihe' }, 'Dein Stempel', stempel, hier.id && !hier.darf ? knopf(`♥ ${hier.name} liken`, liken(hier.id)) : null),
    liste.length ? liste.map((x, i) => el('div', { class: 'zeile' }, el('span', {}, el('b', {}, `${i + 1}. ${x.name}`), el('br'),
      el('span', { class: 'meta' }, [`♥ ${zahl(x.likes || 0)}`, ...STEMPEL.map((s, k) => x.stempel?.[k] ? `${s[0]} ${x.stempel[k]}` : ''), x.besucht ? 'schon besucht' : ''].filter(Boolean).join(' · '))),
      el('span', { class: 'reihe', style: 'margin:0' }, knopf('Besuchen', () => reisen(1, x.id)), knopf('♥ Insel liken', liken(x.id)))))
      : el('p', { class: 'meta' }, 'Noch hat keine offene Insel ein Like. Besuche eine und sei die oder der Erste!')];
}
/* ---------- Textur-Paket: 16/64/128 oder eigenes PNG (Atlas 4×4 Kacheln), nur in diesem Browser gemerkt ---------- */
function idb(modus, f) {
  return new Promise((ok, nein) => {
    const r = indexedDB.open('campus', 1);
    r.onupgradeneeded = () => r.result.createObjectStore('textur');
    r.onerror = () => nein(r.error);
    r.onsuccess = () => {
      const t = r.result.transaction('textur', modus), q = f(t.objectStore('textur'));
      t.oncomplete = () => { ok(q.result); r.result.close(); };
      t.onerror = () => nein(t.error);
    };
  });
}
async function paketMerken(url) {
  try { await idb('readwrite', s => s.put(url, 'paket')); return; } catch {}
  try { localStorage.setItem('campus-textur', url); } catch { toast('Das Textur-Paket ist zu groß zum Merken; es gilt nur bis zum Neuladen.'); }
}
async function paketLaden() {
  try { const u = await idb('readonly', s => s.get('paket')); if (u) return u; } catch {}
  try { return localStorage.getItem('campus-textur'); } catch { return null; }
}
/** Bild (Data-URL) → RGBA → Kern. */
async function paketSetzen(url) {
  const img = new Image();
  img.src = url;
  await img.decode();
  const w = img.naturalWidth, h = img.naturalHeight;
  if (w < 4 || h < 4 || w > 4096 || h > 4096) throw new Error('Das Bild muss zwischen 4 und 4096 Pixel groß sein.');
  const cv = document.createElement('canvas');
  cv.width = w; cv.height = h;
  const g = cv.getContext('2d');
  g.drawImage(img, 0, 0);
  const d = g.getImageData(0, 0, w, h).data, kachel = w / 4;
  game.set_texture(kachel >= 128 ? 128 : kachel >= 64 ? 64 : 16);
  game.set_texture_pack(new Uint8Array(d.buffer), w, h);
}
async function texturAnwenden() {
  if (!game?.set_texture) return;
  if (opt.textur === 'eigen') {
    const url = await paketLaden();
    try { if (url) return await paketSetzen(url); } catch {}
    opt.textur = 16; // Paket fehlt oder kaputt: zurück zu Klassik
  }
  game.clear_texture_pack?.();
  game.set_texture(Number(opt.textur) || 16);
}
function texturWahl() {
  const datei = el('input', { type: 'file', accept: 'image/png', hidden: true, onchange: () => {
    const f = datei.files?.[0];
    if (!f) return;
    const r = new FileReader();
    r.onload = async () => {
      try { await paketSetzen(r.result); opt.textur = 'eigen'; optSpeichern(); await paketMerken(r.result); toast('Eigenes Textur-Paket aktiv.', 'gut'); zeige('Einstellungen'); } catch (e) { fehler(e); }
    };
    r.readAsDataURL(f);
  } });
  const setze = v => () => { if (v === 'eigen') return datei.click(); opt.textur = v; optSpeichern(); texturAnwenden(); zeige('Einstellungen'); };
  return el('div', { class: 'stufenwahl', role: 'group', 'aria-label': 'Textur-Paket' }, el('span', {}, 'Textur-Paket'),
    el('div', { class: 'reihe' }, [[16, '16x Klassik'], [64, '64x HD'], [128, '128x Ultra'], ['eigen', 'Custom PNG']].map(([v, text]) =>
      el('button', { class: 'knopf klein', type: 'button', 'aria-pressed': String(opt.textur === v), onclick: setze(v) }, text)), datei),
    el('p', { class: 'meta' }, 'Custom PNG: ein Bild mit 4 × 4 Kacheln (Muster 0–15), wird mit der Blockfarbe eingefärbt. Bleibt nur in diesem Browser.'));
}
/* ---------- Avatar-Vorschau: kleiner Canvas-2D-Voxel-Zeichner aus me.look + Register, dreht sich, Ziehen dreht mit ---------- */
let avatarWinkel = 0.5;
const hell = (rgb, f) => rgb.map(c => Math.max(0, Math.min(255, c * f)));
/** Quader [x0,y0,z0,x1,y1,z1,rgb] in Pixel-Einheiten; Füße bei y 0, Blick nach +z. t = Zeit für Laufzyklus. */
function figurTeile(l, t) {
  const F = i => reg.farben[i] || [120, 120, 130], haut = reg.haut[l.skin] || [240, 200, 160], haar = F(l.hair_col), oben = F(l.col_a), unten = F(l.col_b);
  const sw = Math.sin(t / 260) * 1.4, b = [], q = (...a) => b.push(a), dunkel = [42, 30, 22];
  const cos = s => C(l.slots?.[s] || 0);
  // Beine mit Schuhen, Arme gegengleich (Laufzyklus)
  q(-4, 0, -2 + sw, 0, 12, 2 + sw, unten); q(0, 0, -2 - sw, 4, 12, 2 - sw, unten);
  q(-4.2, 0, -2.2 + sw, -0.1, 2.2, 2.6 + sw, [70, 48, 32]); q(0.1, 0, -2.2 - sw, 4.2, 2.2, 2.6 - sw, [70, 48, 32]);
  q(-8, 17, -2 - sw, -4, 24, 2 - sw, oben); q(4, 17, -2 + sw, 8, 24, 2 + sw, oben);
  q(-8, 12, -2 - sw, -4, 17, 2 - sw, haut); q(4, 12, -2 + sw, 8, 17, 2 + sw, haut);
  // Rumpf, Jacke/Tunika, Gürtel
  q(-4, 12, -2, 4, 24, 2, oben);
  if (l.outfit === 2) q(-4.3, 4, -2.3, 4.3, 12.5, 2.3, hell(oben, 0.8));                                   // Robe
  if (l.outfit === 1) q(-4, 21, -3.6, 4, 28, -2, hell(oben, 0.8));                                         // Kapuze
  if (l.outfit === 3) { q(-4.2, 13, -2.2, -1, 24, 2.2, hell(oben, 0.6)); q(1, 13, -2.2, 4.2, 24, 2.2, hell(oben, 0.6)); } // Weste
  q(-4.15, 12, -2.15, 4.15, 13.4, 2.15, dunkel); q(-0.8, 12.1, 2.15, 0.8, 13.3, 2.35, [230, 190, 70]);
  // Kopf, Augen, Mund
  q(-4, 24, -4, 4, 32, 4, haut);
  q(-3, 28, 4, -1, 29.5, 4.12, [250, 250, 250]); q(1, 28, 4, 3, 29.5, 4.12, [250, 250, 250]);
  q(-2, 28, 4.12, -1, 29.5, 4.22, [30, 30, 50]); q(1, 28, 4.12, 2, 29.5, 4.22, [30, 30, 50]);
  q(-1.5, 25.6, 4, 1.5, 26.3, 4.12, [150, 60, 60]);
  // Haare als eigene Schicht (Glatze = 4)
  if (l.hair !== 4) {
    const top = l.hair === 3 ? 34 : 32.6;
    q(-4.4, 30.6, -4.4, 4.4, top, 4.4, haar); q(-4.4, l.hair === 1 ? 19 : 25, -4.6, 4.4, 32, -3.6, haar);
    q(-4.5, 27, -4.4, -3.9, 32, 2, haar); q(3.9, 27, -4.4, 4.5, 32, 2, haar); q(-4.4, 30.8, 3.9, 4.4, 31.8, 4.45, haar);
    if (l.hair === 2) q(-1, 19, -6.4, 1, 28, -4.4, hell(haar, 0.85));                                     // Zopf
  }
  // Cosmetics: Umhang (0), Hut (1), Flügel (2), Begleiter (3)
  if (cos(0)) q(-4, 5, -3, 4, 23.5, -2.2, cos(0).rgb);
  if (cos(1)) { q(-4.8, 32, -4.8, 4.8, 33.4, 4.8, cos(1).rgb); q(-3, 33.4, -3, 3, 37, 3, cos(1).rgb2 || cos(1).rgb); }
  if (cos(2)) { q(-12, 14, -3.8, -2, 26, -3, cos(2).rgb); q(2, 14, -3.8, 12, 26, -3, cos(2).rgb); }
  if (cos(3)) { const h = Math.sin(t / 400) * 1.2; q(9, 24 + h, -1, 13, 28 + h, 3, cos(3).rgb); }
  return b;
}
const FLAECHEN = [[[0, 2, 6, 4], [-1, 0, 0]], [[1, 5, 7, 3], [1, 0, 0]], [[0, 4, 5, 1], [0, -1, 0]], [[2, 3, 7, 6], [0, 1, 0]], [[0, 1, 3, 2], [0, 0, -1]], [[4, 6, 7, 5], [0, 0, 1]]];
function avatarVorschau() {
  const cv = el('canvas', { width: 240, height: 320, role: 'img', 'aria-label': 'Vorschau deiner Figur, zum Drehen ziehen' }), g = cv.getContext('2d');
  let zieht = null, weg = 0;
  cv.addEventListener('pointerdown', e => { zieht = e.clientX; cv.setPointerCapture(e.pointerId); });
  cv.addEventListener('pointermove', e => { if (zieht != null) { avatarWinkel += (e.clientX - zieht) * 0.02; zieht = e.clientX; } });
  for (const k of ['pointerup', 'pointercancel']) cv.addEventListener(k, () => { zieht = null; });
  const neig = -0.22, cn = Math.cos(neig), sn = Math.sin(neig), s = 7, licht = [0.45, 0.75, 0.5];
  const male = t => {
    if (!cv.isConnected) { if (++weg > 60) return; } else weg = 0; // Fenster zu: Schleife endet
    requestAnimationFrame(male);
    if (zieht == null) avatarWinkel += 0.01;
    if (!me?.look || !reg) return;
    const ca = Math.cos(avatarWinkel), sa = Math.sin(avatarWinkel);
    const dreh = ([x, y, z]) => { const x1 = x * ca + z * sa, z1 = -x * sa + z * ca; return [x1, y * cn - z1 * sn, y * sn + z1 * cn]; };
    const faces = [];
    for (const [x0, y0, z0, x1, y1, z1, rgb] of figurTeile(me.look, t)) {
      const ecken = [0, 1, 2, 3, 4, 5, 6, 7].map(i => dreh([i & 1 ? x1 : x0, (i & 2 ? y1 : y0) - 17, i & 4 ? z1 : z0]));
      for (const [idx, n] of FLAECHEN) {
        const nn = dreh(n);
        if (nn[2] <= 0.001) continue;
        const p = idx.map(i => ecken[i]), f = 0.55 + 0.45 * Math.max(0, nn[0] * licht[0] + nn[1] * licht[1] + nn[2] * licht[2]);
        faces.push([p.reduce((a, v) => a + v[2], 0) / 4, p, farbe(hell(rgb, f).map(Math.round))]);
      }
    }
    faces.sort((a, b) => a[0] - b[0]);
    g.clearRect(0, 0, cv.width, cv.height);
    for (const [, p, c] of faces) {
      g.beginPath();
      p.forEach(([x, y], i) => g[i ? 'lineTo' : 'moveTo'](120 + x * s, 178 - y * s));
      g.closePath(); g.fillStyle = c; g.strokeStyle = c; g.lineWidth = 0.7; g.fill(); g.stroke();
    }
  };
  requestAnimationFrame(male);
  return cv;
}
async function fundOeffnen() {
  try {
    const f = await api('fund/open', {});
    ton(7);
    toast(`Im Fund: ${f.name} × ${f.menge}${f.selten ? ' – selten!' : ''}`, 'gut');
    await ladeMich();
    if (reiter) zeige(reiter);
  } catch (e) { fehler(e); }
}
function anbieten(item, n) {
  const menge = el('input', { type: 'number', min: 1, max: n, value: 1, 'aria-label': 'Menge' }), start = el('input', { type: 'number', min: 1, value: 20, 'aria-label': 'Startpreis' });
  const sofort = el('input', { type: 'number', min: 1, placeholder: 'kein', 'aria-label': 'Sofortkauf' }), std = el('select', { 'aria-label': 'Laufzeit' }, el('option', { value: 24 }, '24 Stunden'), el('option', { value: 72 }, '72 Stunden'));
  $('inhalt').replaceChildren(el('h2', {}, `${nameVon(item)} anbieten`), el('div', { class: 'reihe' }, 'Menge', menge, 'Startpreis', start, 'Sofortkauf', sofort, std),
    el('p', { class: 'meta' }, 'Die Ware liegt bis zum Ende im Auktionshaus. Vom Verkaufspreis gehen 5 % Gebühr ab.'),
    el('div', { class: 'reihe' }, el('button', { class: 'knopf voll', type: 'button', onclick: tun('auction/create', () => ({ item, menge: Number(menge.value), start: Number(start.value), sofort: sofort.value ? Number(sofort.value) : null, stunden: Number(std.value) }), () => toast('Dein Angebot steht im Auktionshaus.', 'gut')) }, 'Anbieten'),
      el('button', { class: 'knopf', type: 'button', onclick: () => zeige('Vorrat') }, 'Abbrechen')));
}
function sprueche(an) {
  if (!an) return modal.zu();
  modal.auf('sprueche');
  const besitz = new Set([...inv.keys(), 360]);
  $('sprueche').replaceChildren(zuKnopf(), ...reg.sprueche.map((s, i) => el('button', { class: 'knopf klein', type: 'button', onclick: () => { game.chat(i); sprueche(false); } }, s)), el('span', { class: 'trenner' }),
    ...reg.emotes.map((e, i) => el('button', { class: 'knopf klein', type: 'button', disabled: !besitz.has(360 + i), onclick: () => { game.emote(i); if (!sicht) game.set_view(sicht = 1); sprueche(false); } }, e)),
    el('button', { class: 'knopf klein', type: 'button', onclick: () => sprueche(false) }, 'Schließen'));
}

/* ---------- Eingabe ---------- */
const eingabe = {
  tasten: new Set(), links: [0, 0], rechts: [0, 0], sprung: false,
  reset() { this.tasten.clear(); this.links = [0, 0]; this.rechts = [0, 0]; this.sprung = false; },
  senden(dt) {
    const t = this.tasten, k = opt.tasten, x = (t.has(k.rechts) || t.has('ArrowRight') ? 1 : 0) - (t.has(k.links) || t.has('ArrowLeft') ? 1 : 0) + this.links[0];
    const z = (t.has(k.vor) || t.has('ArrowUp') ? 1 : 0) - (t.has(k.zurueck) || t.has('ArrowDown') ? 1 : 0) - this.links[1];
    game.input(x, z, t.has(k.springen) || this.sprung, t.has(k.sprinten) || Math.hypot(...this.links) > 0.92);
    if (this.rechts[0] || this.rechts[1]) game.turn(this.rechts[0] * dt * 2.6 * opt.touch, this.rechts[1] * dt * 1.9 * opt.touch * (opt.invertY ? -1 : 1));
  },
};
/** [E]: erst das Ziel benutzen; ohne Ziel am Fadenkreuz öffnet E das Inventar. */
function benutzen() { game.interact?.(); if (!hinweisDaten[0]) inventar.oeffneInventar(); }
const KAMERA = ['1. Person', 'Hinter dir', 'Von vorn'];
function kameraWechsel() { game.set_view(sicht = (sicht + 1) % 3); toast(`Kamera: ${KAMERA[sicht]}`); }
/** Am Handy/Tablet immer quer: Vollbild + Ausrichtung sperren (geht nur nach einer Berührung). */
function querSperren() {
  if (!TOUCH) return;
  const d = document.documentElement;
  (d.requestFullscreen ? d.requestFullscreen({ navigationUI: 'hide' }) : Promise.resolve()).catch(() => {}).then(() => screen.orientation?.lock?.('landscape')).catch(() => {});
}
function eingabeEinrichten() {
  const c = $('c');
  addEventListener('keydown', e => {
    if (warteTaste) {
      e.preventDefault();
      if (e.code !== 'Escape') { for (const a in opt.tasten) if (opt.tasten[a] === e.code) opt.tasten[a] = opt.tasten[warteTaste]; opt.tasten[warteTaste] = e.code; optSpeichern(); }
      warteTaste = null; zeige('Einstellungen'); return;
    }
    // Foto-Modus und Kamerafahrt zuerst (nicht, während jemand in ein Textfeld tippt)
    if (!e.target.closest?.('textarea, select, input:not([type=range])') && foto.taste(e)) return;
    if (e.code === 'F8') { e.preventDefault(); if (!$('hud').hidden) { if (offenesFenster()) modal.zu(); foto.fotoUmschalten(); } return; }
    if (e.target.id === 'chat-text') { if (e.code === 'Enter') { e.preventDefault(); chatSenden(); } else if (e.code === 'Escape') modal.zu(); return; }
    if (e.code === 'Escape') {
      e.preventDefault();
      if (performance.now() - menuDurchZeiger < 400) return; // dasselbe Esc hat schon über den Zeigerverlust das Menü geöffnet
      if (offenesFenster()) modal.zu(); else if (foto.fotoAktiv()) foto.fotoUmschalten(); else menu(true, 'Einstellungen');
      return;
    }
    if (e.target.closest?.('input, select, textarea')) return;
    const a = aktion(e.code);
    if (a === 'kamera' || e.code === 'F5') e.preventDefault(); // F5 lädt sonst die Seite neu
    if (a === 'menue' || e.code === 'Tab') { e.preventDefault(); if (offenesFenster() === 'menu') modal.zu(); else menu(true); return; }
    // I (oder E bei offenem Inventar) schaltet das Inventar, J das Questbuch
    if (e.code === 'KeyI' || (a === 'interagieren' && offenesFenster() === 'inventar')) { e.preventDefault(); if (offenesFenster() === 'inventar') modal.zu(); else if (!offenesFenster()) inventar.oeffneInventar(); return; }
    if (e.code === 'KeyJ') { e.preventDefault(); if (offenesFenster() === 'menu' && reiter === 'Questbuch') modal.zu(); else menu(true, 'Questbuch'); return; }
    if (offenesFenster()) return;
    if (a === 'chat' || e.code === 'Enter') { e.preventDefault(); chatAuf(true); return; }
    if (a === 'befehl') { e.preventDefault(); chatAuf(true, '/'); return; }
    if (a === 'faehigkeit') { game.use_tool(); return; }
    if (a === 'interagieren') { if (!e.repeat) benutzen(); return; }
    if (a === 'kamera' || e.code === 'KeyV') { kameraWechsel(); return; }
    if (/^Digit[1-9]$/.test(e.code)) return waehle(Number(e.code[5]) - 1);
    if (['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.code)) e.preventDefault();
    eingabe.tasten.add(e.code);
  });
  addEventListener('keyup', e => eingabe.tasten.delete(e.code));
  addEventListener('blur', () => { eingabe.reset(); game?.mine?.(false); });
  c.addEventListener('contextmenu', e => e.preventDefault());
  c.addEventListener('wheel', e => { e.preventDefault(); waehle(auswahl + Math.sign(e.deltaY)); }, { passive: false });
  // Klick neben ein offenes Fenster schließt es (Knöpfe oben, Leiste und Daumenfelder zählen nicht als „daneben“)
  addEventListener('pointerdown', e => {
    const f = offenesFenster();
    if (f && !e.target.closest?.(`#${f}, #oben, #leiste, #touch, #toasts`)) modal.zu();
  }, true);
  // Klicks auf Knöpfe klicken hörbar
  addEventListener('click', e => { if (e.target.closest?.('button') && !e.target.closest('#tasten, .zu')) ton(13, 0.5); });
  if (!TOUCH) {
    c.addEventListener('mousedown', e => {
      tonAn();
      if (document.pointerLockElement !== c) return zeigerFangen();
      if (foto.fotoAktiv()) return; // Freikamera: nichts abbauen oder setzen
      if (e.button === 0) { if (game.mine) game.mine(true); else game.act(0); }
      else if (e.button === 2) game.act(1);
    });
    addEventListener('mouseup', e => { if (e.button === 0) game?.mine?.(false); });
    addEventListener('mousemove', e => { if (document.pointerLockElement === c) game.turn(e.movementX * 0.0024 * opt.maus, e.movementY * 0.0024 * opt.maus * (opt.invertY ? -1 : 1)); });
    // Zeiger verloren (Esc im Browser, Fensterwechsel) und kein Fenster offen: Spiel pausiert mit dem Menü
    document.addEventListener('pointerlockchange', () => {
      if (document.pointerLockElement === c) return;
      game?.mine?.(false); eingabe.reset();
      if (!offenesFenster() && !$('hud').hidden && !foto.fotoAktiv()) { menu(true, 'Einstellungen'); menuDurchZeiger = performance.now(); }
    });
    return;
  }
  // Touch: zwei Daumenfelder und Knöpfe
  $('touch').hidden = false;
  $('kreuz').hidden = false;
  const stick = (id, ziel) => {
    const s = $(id), knopf = s.firstElementChild;
    const setze = e => {
      const r = s.getBoundingClientRect(), dx = e.clientX - r.left - r.width / 2, dy = e.clientY - r.top - r.height / 2, len = Math.hypot(dx, dy), max = r.width / 2 - 14, f = len > max ? max / len : 1;
      knopf.style.transform = `translate(${dx * f}px, ${dy * f}px)`;
      const tot = 0.12, n = Math.min(1, len / max), k = n < tot ? 0 : (n - tot) / (1 - tot) / (len || 1);
      eingabe[ziel] = [dx * k, dy * k];
    };
    s.addEventListener('pointerdown', e => { tonAn(); s.setPointerCapture(e.pointerId); setze(e); });
    s.addEventListener('pointermove', e => { if (s.hasPointerCapture(e.pointerId)) setze(e); });
    const los = () => { knopf.style.transform = ''; eingabe[ziel] = [0, 0]; };
    s.addEventListener('pointerup', los); s.addEventListener('pointercancel', los);
  };
  stick('s-links', 'links'); stick('s-rechts', 'rechts');
  const knopf = (id, runter, hoch) => { const b = $(id); b.addEventListener('pointerdown', e => { e.preventDefault(); tonAn(); runter(); }); if (hoch) for (const k of ['pointerup', 'pointercancel', 'pointerleave']) b.addEventListener(k, hoch); };
  knopf('t-abbau', () => { if (game.mine) game.mine(true); else game.act(0); }, () => game.mine?.(false)); knopf('t-setzen', () => game.act(1));
  knopf('t-e', benutzen);
  knopf('t-inv', () => offenesFenster() === 'inventar' ? modal.zu() : inventar.oeffneInventar());
  knopf('t-sprung', () => { eingabe.sprung = true; }, () => { eingabe.sprung = false; });
  knopf('t-spruch', () => chatAuf($('chat').hidden));
  knopf('t-menue', () => menu(true));
  // mit einem Finger über das Bild wischen dreht ebenfalls die Kamera
  let wisch = null;
  c.addEventListener('pointerdown', e => { tonAn(); wisch = { id: e.pointerId, x: e.clientX, y: e.clientY }; });
  c.addEventListener('pointermove', e => { if (wisch?.id === e.pointerId) { game.turn((e.clientX - wisch.x) * 0.005 * opt.touch, (e.clientY - wisch.y) * 0.005 * opt.touch * (opt.invertY ? -1 : 1)); wisch.x = e.clientX; wisch.y = e.clientY; } });
  for (const k of ['pointerup', 'pointercancel']) c.addEventListener(k, () => { wisch = null; });
}

/* ---------- Bild: Größe, Schleife, Messung ---------- */
let schaerfe = Math.min(devicePixelRatio || 1, TOUCH ? 1.5 : 2), langsam = 0;
function groesse() {
  const c = $('c'), w = Math.max(1, Math.round(c.clientWidth * schaerfe)), h = Math.max(1, Math.round(c.clientHeight * schaerfe));
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; game?.resize(w, h); }
}
const messung = { an: false, zeiten: [], seit: 0 };
let zuletzt = 0;
let gezeichnet = 0;
function bild(t) {
  requestAnimationFrame(bild);
  const cap = [0, 60, 30][opt.fpsMax];
  if (cap && t - gezeichnet < 1000 / cap - 2) return;
  gezeichnet = t;
  const dt = Math.min(0.05, (t - zuletzt) / 1000);
  if (messung.an && zuletzt) messung.zeiten.push(t - zuletzt);
  zuletzt = t;
  eingabe.senden(dt);
  game.frame(t);
  foto.nachBild();
  // läuft es dauerhaft unter 50 Bildern, wird die Auflösung schrittweise gesenkt (wichtig auf älteren iPads)
  const fps = game.info(0);
  // Auto: unter 42 Bildern für 4 s (Tab sichtbar) → eine Shader-Stufe tiefer
  if (opt.grafik === 'auto' && autoStufe > 0 && !document.hidden && $('start').hidden) {
    ruckel = fps < 42 ? ruckel + dt : 0;
    if (ruckel > 4) { autoStufe--; optAnwenden(); toast('Grafik-Stufe automatisch für flüssige 60 FPS optimiert'); }
  }
  // erst wenn keine Shader-Stufe mehr übrig ist: Auflösung schrittweise senken (wichtig auf älteren iPads)
  langsam = fps < 50 ? langsam + dt : 0;
  if (!opt.fpsMax && langsam > 3 && schaerfe > 0.6 && (opt.grafik !== 'auto' || autoStufe === 0)) { schaerfe = Math.max(0.6, schaerfe * 0.85); langsam = 0; groesse(); }
  manaZeigen();
  if ((t | 0) % 30 === 0) $('technik').textContent = `${Math.round(fps)} Bilder/s · ${game.backend()}`;
}
/** Leistungsmessung: 10 Sekunden Rundflug, Ergebnis in window.campusBench (Aufruf mit ?bench=1). */
function messen(sekunden = 10) {
  game.set_demo(true); messung.an = true; messung.zeiten = [];
  setTimeout(() => {
    game.set_demo(false); messung.an = false;
    const z = messung.zeiten.slice(30).sort((a, b) => a - b), mittel = z.reduce((s, x) => s + x, 0) / z.length;
    window.campusBench = { bilder: z.length, fps: +(1000 / mittel).toFixed(1), fps_1prozent: +(1000 / z[Math.floor(z.length * 0.99)]).toFixed(1), schaerfe: +schaerfe.toFixed(2), grafik: game.backend(),
      flaeche: `${$('c').width}×${$('c').height}`, stuecke: game.info(1), rechtecke: game.info(2), instanzen: game.info(3) };
    toast(`Messung: ${window.campusBench.fps} Bilder/s (schwächstes 1 %: ${window.campusBench.fps_1prozent}) · ${window.campusBench.grafik}`, 'gut');
  }, sekunden * 1000);
}

function verbinden() {
  ws = new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/campus/ws');
  ws.binaryType = 'arraybuffer';
  ws.onmessage = e => game.message(new Uint8Array(e.data));
  ws.onclose = () => { if ($('start').hidden) { toast('Verbindung getrennt. Ich verbinde neu …', 'schlecht'); setTimeout(verbinden, 3000); } };
}

async function start() {
  try {
    reg = await (await fetch('api/registry', { cache: 'no-store' })).json();
    me = await api('me');
  } catch (e) {
    $('start-text').replaceChildren(e.status === 401 ? 'Campus gehört zum Lernhelfer. Bitte melde dich dort zuerst an, dann geht es hier ohne zweites Passwort weiter.' : e.message);
    return;
  }
  $('start-text').textContent = `Angemeldet als ${me.name}. Die Welt wird geladen …`;
  inventar.initInventar({ api, el, $, ton, toast, fehler, nameVon, icon, reg: () => reg, me: () => me, game: () => game,
    modalAuf: id => modal.auf(id), modalZu: () => modal.zu(), ladeMich, TOUCH });
  const ctx = { api, el, $, ton, toast, fehler, modal, zuKnopf, menu, TOUCH, game: () => game, me: () => me, reg: () => reg, opt: () => opt, optSpeichern, optAnwenden, stufe };
  foto.initFoto(ctx); duell.initDuell(ctx); katakomben.initKatakomben(ctx); architekt.initArchitekt(ctx);
  try {
    const mod = await import(`./pkg/campus_client.js?v=${V}`);
    await mod.default({ module_or_path: `pkg/campus_client_bg.wasm?v=${V}` });
    groesse();
    game = await mod.Game.create($('c'));
  } catch (e) {
    $('start-text').textContent = `Die Grafik lässt sich auf diesem Gerät nicht starten (${e.message || e}). Campus braucht WebGPU oder WebGL 2.`;
    return;
  }
  addEventListener('resize', groesse);
  eingabeEinrichten();
  addEventListener('campus-inventar', leisteZeigen);
  texturAnwenden();
  $('b-menu').onclick = () => offenesFenster() ? modal.zu() : menu(true, 'Einstellungen');
  $('b-sky').onclick = () => menu(true);
  $('b-kamera').onclick = kameraWechsel;
  $('b-foto').onclick = () => { tonAn(); if (offenesFenster()) modal.zu(); foto.fotoUmschalten(); };
  document.addEventListener('visibilitychange', () => musik.pause(document.hidden));
  $('b-chat').onclick = () => chatAuf($('chat').hidden);
  $('chat-senden').onclick = chatSenden;
  $('chat').querySelector('.zu').onclick = () => modal.zu();
  setInterval(() => { if (hier.id) minionsLaden(); }, 60000);
  for (const n of reg.sky.npcs) spieler.set(NPC_BASE + n.id, { name: n.name, plate: 0 }), schild(NPC_BASE + n.id);
  optAnwenden();
  $('menu-zu').onclick = () => modal.zu();
  $('menu-zurueck').onclick = () => zeige('Übersicht');
  $('b-fund').onclick = fundOeffnen;
  leisteZeigen(); xpZeigen();
  $('start-text').textContent = `Angemeldet als ${me.name}. Grafik: ${game.backend()}.`;
  $('start-hilfe').hidden = false;
  $('los').hidden = false;
  $('los').onclick = () => { tonAn(); querSperren(); verbinden(); $('los').disabled = true; $('los').textContent = 'Verbinde …'; if (!TOUCH) try { $('c').requestPointerLock?.()?.catch?.(() => {}); } catch {} };
  // Sichtweite erst nach dem Willkommen senden (vorher ist die Leitung noch zu)
  addEventListener('campus-willkommen', optAnwenden);
  requestAnimationFrame(bild);
  if (new URLSearchParams(location.search).has('bench')) { $('los').click(); setTimeout(messen, 6000); }
  window.campusMessen = messen;
}
start();
