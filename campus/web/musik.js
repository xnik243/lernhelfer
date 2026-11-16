// Campus – erzeugte Hintergrundmusik (WebAudio, keine Dateien): ein Planer setzt Achtel für Achtel Stimmen aus
// STIMMUNGEN; dazu eine einzige Regen-Schleife auf eigenem Bus (eigener Regler). Still bei Lautstärke 0 oder verdecktem Tab.
import { STIMMUNGEN } from './musik_daten.js?v=__V__';

const VORLAUF = 0.15, TAKT_MS = 50, BLENDE = 2, PEGEL = 0.6;
let ac = null, master = null, wetterBus = null, nass = null, busse = [], regenLaut = null;
let laut = 0.5, wetterLaut = 0.25, stimmung = 1, pausiert = false, regen = false;
let uhr = 0, naechste = 0, schritt = 0, geplant = 0, takt = { akk: [], mel: [], bass: [] };

const hoerbar = () => !!ac && laut > 0 && !pausiert;
const WETTER_PEGEL = 0.12;   // Regen bleibt leise im Hintergrund
const wahl = l => (Array.isArray(l) && l.length ? l[Math.floor(Math.random() * l.length)] : []);
const reihe = (l, i) => (Array.isArray(l) && l.length ? l[i % l.length] : []);

/** Stufen-Index → MIDI; Index ≥ Länge der Tonleiter = eine Oktave höher. */
function midi(S, stufe) {
  const n = S.tonleiter.length;
  return S.grundton + S.tonleiter[((stufe % n) + n) % n] + 12 * Math.floor(stufe / n);
}

/** Eine Stimme: Sinus + Dreieck (2×, Pegel 0,18) → Tiefpass 1800 → 420 Hz in 0,4 s → Hüllkurve. Nichts wird behalten. */
function stimme(ziel, t, note, dauer, pegel, ausklang = 2.8) {
  const f = 440 * 2 ** ((note - 69) / 12), ende = t + dauer + ausklang;
  const tp = ac.createBiquadFilter(), h = ac.createGain(), o1 = ac.createOscillator(), o2 = ac.createOscillator(), g2 = ac.createGain();
  tp.type = 'lowpass';
  tp.frequency.setValueAtTime(1800, t); tp.frequency.exponentialRampToValueAtTime(420, t + 0.4);
  h.gain.setValueAtTime(0, t); h.gain.linearRampToValueAtTime(pegel, t + 0.02);
  h.gain.setTargetAtTime(0, t + dauer, ausklang / 5);   // nach fünf Zeitkonstanten unhörbar
  o1.type = 'sine'; o1.frequency.value = f;
  o2.type = 'triangle'; o2.frequency.value = f * 2; g2.gain.value = 0.18;
  o1.connect(tp); o2.connect(g2).connect(tp); tp.connect(h).connect(ziel);
  o1.start(t); o2.start(t); o1.stop(ende); o2.stop(ende);
  geplant++;
}

function achtel(S, t, dauer) {
  const bus = busse[stimmung], pos = schritt % 8, nr = Math.floor(schritt / 8);
  if (pos === 0) {
    takt = { akk: reihe(S.akkorde, nr), bass: reihe(S.bass, nr), mel: Math.random() < 0.4 + 0.6 * S.dichte ? wahl(S.melodien) : [] };
    for (const s of takt.akk) stimme(bus, t, midi(S, s), dauer * 6, 0.028, 1.6);
    if (S.bordun) stimme(bus, t, S.grundton - 12, dauer * 8, 0.06, 1.2);
  }
  const m = takt.mel[pos], b = takt.bass[pos];
  if (Number.isInteger(m) && Math.random() < S.dichte) stimme(bus, t, midi(S, m) + 12, S.staccato ? dauer * 0.35 : dauer * 0.9, 0.085, S.staccato ? 0.22 : 2.8);
  if (Number.isInteger(b)) stimme(bus, t, midi(S, b) - 12, dauer * 0.6, 0.11, 0.45);
  schritt++;
}

function plane() {
  const S = STIMMUNGEN[stimmung];
  if (!S || !Array.isArray(S.tonleiter) || !S.tonleiter.length || !(S.bpm > 0)) return;
  const dauer = 30 / S.bpm, bis = ac.currentTime + VORLAUF;
  if (naechste < ac.currentTime) naechste = ac.currentTime + 0.05;   // nach einer Pause nichts nachholen
  while (naechste < bis) { achtel(S, naechste, dauer); naechste += dauer; }
}

/** Planer und Pegel an den Zustand angleichen (Lautstärke 0 oder verdeckt = steht sofort). */
function regeln() {
  if (!ac) return;
  const an = hoerbar(), t = ac.currentTime;
  master.gain.cancelScheduledValues(t);
  master.gain.setTargetAtTime(an ? laut * PEGEL : 0, t, 0.05);
  wetterBus.gain.cancelScheduledValues(t);
  wetterBus.gain.setTargetAtTime(ac && !pausiert ? wetterLaut * WETTER_PEGEL : 0, t, 0.05);
  if (an && !uhr) { naechste = t + VORLAUF; uhr = setInterval(plane, TAKT_MS); }
  else if (!an && uhr) { clearInterval(uhr); uhr = 0; }
}

function blende(g, ziel, t) {
  g.gain.cancelScheduledValues(t); g.gain.setValueAtTime(g.gain.value, t); g.gain.linearRampToValueAtTime(ziel, t + BLENDE);
}

/** Regen: EINE Schleifen-Quelle (rosa Rauschen, Tiefpass), einmal gebaut; danach nur noch der Pegel. */
function regenBauen() {
  const n = ac.sampleRate * 2, puf = ac.createBuffer(1, n, ac.sampleRate), d = puf.getChannelData(0);
  let b0 = 0, b1 = 0, b2 = 0;
  for (let i = 0; i < n; i++) {   // rosa Rauschen (Paul Kellet, sparsame Fassung)
    const w = Math.random() * 2 - 1;
    b0 = 0.99765 * b0 + w * 0.0990460; b1 = 0.96300 * b1 + w * 0.2965164; b2 = 0.57000 * b2 + w * 1.0526913;
    d[i] = (b0 + b1 + b2 + w * 0.1848) * 0.18;
  }
  const q = ac.createBufferSource(), tp = ac.createBiquadFilter();
  q.buffer = puf; q.loop = true; tp.type = 'lowpass'; tp.frequency.value = 1400;
  regenLaut = ac.createGain(); regenLaut.gain.value = 0;
  q.connect(tp).connect(regenLaut).connect(wetterBus);
  q.start();
}

export function start(kontext) {
  if (ac || !kontext) return;
  ac = kontext;
  master = ac.createGain(); master.gain.value = 0;
  const echo = ac.createDelay(1), rueck = ac.createGain();
  nass = ac.createGain();
  echo.delayTime.value = 0.32; rueck.gain.value = 0.35; nass.gain.value = STIMMUNGEN[stimmung]?.hall ?? 0.25;
  master.connect(ac.destination);
  wetterBus = ac.createGain(); wetterBus.gain.value = 0; wetterBus.connect(ac.destination);   // eigener Regler, ohne Echo
  master.connect(echo); echo.connect(rueck).connect(echo); echo.connect(nass).connect(ac.destination);
  busse = [0, 1, 2, 3].map(i => { const g = ac.createGain(); g.gain.value = i === stimmung ? 1 : 0; g.connect(master); return g; });
  if (regen) setRegen(true);
  regeln();
}

export function setLaut(v) {
  laut = Math.min(1, Math.max(0, Number(v) || 0));
  regeln();
}

/** Wetter-Lautstärke (Regen/Wind) unabhängig von der Musik. */
export function setWetterLaut(v) {
  wetterLaut = Math.min(1, Math.max(0, Number(v) || 0));
  regeln();
}

export function setStimmung(n) {
  n = Math.floor(Number(n));
  if (!(n >= 0 && n <= 3) || n === stimmung) return;
  const alt = stimmung;
  stimmung = n; schritt = 0;
  if (!ac) return;
  const t = ac.currentTime;
  blende(busse[alt], 0, t); blende(busse[n], 1, t);
  nass.gain.setTargetAtTime(STIMMUNGEN[n]?.hall ?? 0.25, t, 0.5);
}

export function stimmungVonZone(z) {
  return z === 'Privatinsel' ? 0 : z === 'Prüfungs-Zitadelle' || z === 'Prüfungs-Katakomben' ? 2 : z === 'Lern-Duell-Arena' ? 3 : 1;
}

export function setRegen(an) {
  regen = !!an;
  if (!ac) return;
  if (!regenLaut) { if (!regen) return; regenBauen(); }
  regenLaut.gain.setTargetAtTime(regen ? 1 : 0, ac.currentTime, 0.6);
}

export function pause(an) {
  pausiert = !!an;
  regeln();
}

export function zustand() {
  return { laeuft: !!uhr, stimmung, geplant };
}
