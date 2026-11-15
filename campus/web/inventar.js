// Campus – Inventar, Truhen, Rüstungsständer, Handel, Verzaubern (Phasen 16–20).
// Jeder Zug ist genau ein POST `inv/move`; der Stapel am Zeiger ist nur Anzeige (CONTRACT-16-20 §4 W-INV).
let C = null;                       // ctx aus hud.js
const SHARD = 1100, FOKUS_KRISTALL = 1110, VERZ_BUCH = 2401; // wie crates/shared (skyblock.rs, rpg.rs)
const ROEM = ['', 'I', 'II', 'III', 'IV', 'V', 'VI'], FOLIANT_VI = 2421;
const SELTEN = ['', ' gut', ' selten', ' legende'];  // Rahmenklassen aus hud.css: weiß, blau, lila, gold
const st = { slots: Array(36).fill(null), gear: Array(7).fill(0), held: 0, stash: [], stashN: 0, werte: null };
const ench = new Map();             // Gegenstand → [6] Stufen (aus GET enchants)
let offen = { art: 'inv' };         // inv | truhe | staender | handel | anfrage
let zeiger = null;                  // { von: Ort, item, n }
let ueber = null;                   // Ort unter dem Zeiger (Tasten 1–9)
let schnell = false, lang = false, langUhr = 0, sperreBis = 0, uhr = 0;
let angebot = [], taler = 0;        // eigenes Handelsangebot

const R = () => C.reg().rpg;
let gMap = null;
const G = item => { if (!gMap) gMap = new Map(R().gegenstaende.map(g => [g.id, g])); return gMap.get(item); };
const E = item => R().ausruestung.find(e => e.id === item);
const name = item => G(item)?.name || C.nameVon(item);
const rahmen = item => SELTEN[G(item)?.seltenheit || 0] || '';
const zahl = n => Number(n).toLocaleString('de-AT');
const box = () => {
  let b = document.getElementById('inventar');
  if (!b) { b = C.el('div', { id: 'inventar', class: 'karte', hidden: true }); document.body.append(b); }
  return b;
};
const auf = () => box().hidden === false;
const knopf = (text, f, label) => C.el('button', { class: 'knopf klein', type: 'button', 'aria-label': label || text, onclick: f }, text);

export function initInventar(ctx) {
  C = ctx;
  // Zeiger-Stapel und Tooltip schweben über allem
  document.body.append(C.el('div', { id: 'inv-zeiger', hidden: true, 'aria-hidden': 'true' }), C.el('div', { id: 'inv-tip', hidden: true, role: 'tooltip' }));
  addEventListener('pointermove', e => folgen(e.clientX, e.clientY));
  addEventListener('pointerdown', e => {
    folgen(e.clientX, e.clientY);
    if (!e.target.closest?.('#inventar .feld')) tipZu();
  }, true);
  // Ziffern 1–9 über einem Platz: mit dem Leisten-Platz tauschen (vor hud.js abfangen)
  addEventListener('keydown', e => {
    if (!auf() || !ueber || !/^Digit[1-9]$/.test(e.code) || offen.art === 'handel') return;
    e.preventDefault(); e.stopImmediatePropagation();
    const nach = { art: 'inv', i: +e.code.slice(5) - 1 };
    if (!(ueber.art === 'inv' && ueber.i === nach.i) && stapel(ueber)) zug({ von: ueber, nach, n: 0 });
  }, true);
}
function folgen(x, y) {
  const z = document.getElementById('inv-zeiger');
  if (!z) return;
  if (zeiger && !auf()) { zeiger = null; }
  z.hidden = !zeiger;
  if (zeiger) z.style.transform = `translate(${x - 22}px, ${y - 22}px)`;
}

/* ---------- Daten ---------- */
export async function ladeInventar() {
  try { uebernehmen(await C.api('inv')); }
  catch (e) { C.fehler(e); return; }
  if (auf()) zeichne();
}
export function plaetze() { return st.slots; }
function uebernehmen(r) {
  for (const k of ['slots', 'gear', 'held', 'stash', 'stashN', 'werte']) if (r[k] !== undefined) st[k] = r[k];
  if (r.truhe && offen.art === 'truhe') Object.assign(offen, { slots: r.truhe.slots, max: r.truhe.max || offen.max });
  if (r.staender && offen.art === 'staender') offen.items = r.staender.items || r.staender;
  stashAnzeigen(st.stashN || 0);
  dispatchEvent(new Event('campus-inventar'));
}
async function ladeEnch() {
  try { const r = await C.api('enchants'); ench.clear(); for (const g of r.gegenstaende || []) ench.set(g.item, g.stufen); return r; }
  catch { return { gegenstaende: [] }; }
}

/* ---------- Orte und Züge ---------- */
const ort = (art, i) => (art === 'truhe' || art === 'staender') ? { art, i, x: offen.x, y: offen.y, z: offen.z } : { art, i };
const gleich = (a, b) => a && b && a.art === b.art && a.i === b.i;
function stapel(o) {
  if (o.art === 'inv') return st.slots[o.i];
  if (o.art === 'gear') return st.gear[o.i] ? [st.gear[o.i], 1] : null;
  if (o.art === 'truhe') return offen.slots?.[o.i] || null;
  if (o.art === 'staender') return offen.items?.[o.i] ? [offen.items[o.i], 1] : null;
  return null;
}
async function zug(body) {
  if (offen.art === 'truhe' && !body.nach) body.ziel = { x: offen.x, y: offen.y, z: offen.z };
  const vorher = offen.art === 'staender' ? [...offen.items] : null;
  try {
    const r = await C.api('inv/move', body);
    // Ständer: Server liefert den neuen Inhalt nicht zwingend mit – dann hier nachziehen
    if (vorher && !r.staender) staenderLokal(body, vorher);
    uebernehmen(r);
    C.ton(13, 0.6);
    return true;
  } catch (e) { C.fehler(e); zeiger = null; return false; }
  finally { if (auf()) zeichne(); }
}
function staenderLokal(b, alt) {
  // Ständer-Plätze halten je ein Stück: ganzer Stapel wandert, belegtes Ziel wird getauscht
  const a = b.von.art === 'staender' ? alt[b.von.i] : stapel(b.von)?.[0] || 0;
  const z = !b.nach ? 0 : b.nach.art === 'staender' ? alt[b.nach.i] : stapel(b.nach)?.[0] || 0;
  if (b.von.art === 'staender') offen.items[b.von.i] = z;
  if (b.nach?.art === 'staender') offen.items[b.nach.i] = a;
}
/** Links nimmt/legt, rechts teilt bzw. legt 1 ab, Umschalt (oder Schnell-Schalter) verschiebt sofort. */
async function klick(o, rechts, umschalt) {
  tipZu();
  if (offen.art === 'handel') return anbieten(o, rechts);
  const s = stapel(o);
  if (!zeiger) {
    if (!s) return;
    if (umschalt || schnell) return zug({ von: o, nach: null, n: 0 });
    zeiger = { von: o, item: s[0], n: rechts ? Math.ceil(s[1] / 2) : s[1] };
    C.ton(13, 0.4);
    return zeichne();
  }
  if (gleich(o, zeiger.von)) { zeiger = null; return zeichne(); }
  const voll = stapel(zeiger.von)?.[1] || 0, n = rechts ? 1 : zeiger.n;
  const z = zeiger;
  if (await zug({ von: z.von, nach: o, n: n >= voll ? 0 : n }) && rechts && z.n > 1 && zeiger === z) z.n--;
  else if (zeiger === z) zeiger = null;
  zeichne();
}

/* ---------- Plätze ---------- */
function feld(o, platzhalter = '') {
  const s = stapel(o), weg = zeiger && gleich(o, zeiger.von);
  const b = C.el('button', {
    class: 'feld' + (s ? rahmen(s[0]) : '') + (weg ? ' weg' : '') + (o.art === 'inv' && o.i === st.held ? ' gehalten' : ''), type: 'button',
    'aria-label': `${platzName(o)}: ${s ? `${name(s[0])}, ${s[1]} Stück` : 'leer'}`,
  }, s ? C.icon(s[0]) : platzhalter ? C.el('i', { class: 'inv-leer' }, platzhalter) : null, s && s[1] > 1 ? C.el('b', {}, s[1] > 999 ? '999+' : s[1]) : null);
  b.onclick = e => { if (lang) { lang = false; return; } klick(o, false, e.shiftKey); };
  b.oncontextmenu = e => { e.preventDefault(); klick(o, true, e.shiftKey); };
  b.onpointerenter = e => { ueber = o; if (s && e.pointerType === 'mouse') tipAuf(s[0], s[1], b); };
  b.onpointerleave = () => { if (gleich(ueber, o)) ueber = null; if (!lang) tipZu(); };
  b.onpointerdown = e => {
    if (e.pointerType === 'mouse' || !s) return;
    clearTimeout(langUhr);
    langUhr = setTimeout(() => { lang = true; tipAuf(s[0], s[1], b); }, 450);
  };
  b.onpointerup = b.onpointercancel = () => clearTimeout(langUhr);
  return b;
}
const GEAR_NAMEN = ['Helm', 'Brustpanzer', 'Beinschutz', 'Stiefel', 'Talisman 1', 'Talisman 2', 'Talisman 3'];
const platzName = o => o.art === 'gear' || o.art === 'staender' ? GEAR_NAMEN[o.i] : o.art === 'truhe' ? `Truhe ${o.i + 1}` : o.i < 9 ? `Leiste ${o.i + 1}` : `Rucksack ${o.i - 8}`;
const reihe = (art, von, bis, klasse = 'inv-gitter') => C.el('div', { class: klasse, role: 'group' }, Array.from({ length: bis - von }, (_, k) => feld(ort(art, von + k))));
function rucksack() {
  return [C.el('div', { class: 'inv-titel' }, C.el('b', {}, 'Rucksack'), sortKnopf()), reihe('inv', 9, 36), C.el('b', { class: 'inv-titel' }, 'Schnellleiste'), reihe('inv', 0, 9, 'inv-gitter leiste')];
}
const sortKnopf = (truhe) => knopf('Sortieren', async () => {
  try { uebernehmen(await C.api('inv/sort', truhe ? { truhe } : {})); C.ton(13); } catch (e) { C.fehler(e); }
  zeichne();
}, truhe ? 'Truhe sortieren' : 'Rucksack sortieren');
function kopf(titel, ...mehr) {
  return [C.el('button', { class: 'zu', type: 'button', 'aria-label': 'Schließen', onclick: () => C.modalZu() }, 'X'), C.el('h3', { class: 'inv-kopf' }, titel, ...mehr)];
}
function fuss() {
  return C.el('div', { class: 'reihe inv-fuss' },
    C.TOUCH ? C.el('button', { class: 'knopf klein', type: 'button', 'aria-pressed': String(schnell), onclick: () => { schnell = !schnell; zeichne(); } }, 'Schnell-Verschieben') : null,
    C.el('span', { class: 'meta' }, C.TOUCH ? 'Tippen nimmt/legt · lange tippen zeigt Details' : 'Links nimmt/legt · Rechts teilt/legt 1 · Umschalt verschiebt · 1–9 tauscht mit der Leiste'));
}

/* ---------- Fenster ---------- */
function zeichne() {
  const b = box();
  const inhalt = offen.art === 'truhe' ? truheInhalt() : offen.art === 'staender' ? staenderInhalt() : offen.art === 'handel' ? handelInhalt()
    : offen.art === 'anfrage' ? anfrageInhalt() : inventarInhalt();
  b.className = 'karte inv-fenster ' + offen.art;
  b.replaceChildren(...inhalt.flat(Infinity).filter(Boolean));
  const z = document.getElementById('inv-zeiger');
  if (z) { z.replaceChildren(...(zeiger ? [C.icon(zeiger.item), zeiger.n > 1 ? C.el('b', {}, zeiger.n) : null].filter(Boolean) : [])); z.hidden = !zeiger; }
}
function oeffne(art, daten = {}) {
  zeiger = null; ueber = null; tipZu();
  offen = { art, ...daten };
  zeichne();
  if (!auf()) C.modalAuf('inventar');
}
export function oeffneInventar() {
  oeffne('inv');
  ladeInventar();
  ladeEnch();
}
function inventarInhalt() {
  const w = st.werte, set = w && w.set != null ? R().sets.find(s => s.id === w.set) : null;
  const wert = (t, v, e = '') => v ? C.el('span', {}, t, ' ', C.el('b', {}, `${zahl(Math.round(v * 10) / 10)}${e}`)) : null;
  return [kopf('Inventar'),
    C.el('div', { class: 'inv-oben' },
      C.el('div', { class: 'inv-gear', role: 'group', 'aria-label': 'Rüstung' }, [0, 1, 2, 3].map(i => feld(ort('gear', i), GEAR_NAMEN[i][0]))),
      C.el('div', { class: 'inv-figur' }, figurCanvas()),
      C.el('div', { class: 'inv-gear', role: 'group', 'aria-label': 'Talismane' }, [4, 5, 6].map(i => feld(ort('gear', i), '✦'))),
      C.el('div', { class: 'inv-werte' }, w ? [wert('♥ Leben', w.leben), wert('Verteidigung', w.verteidigung), wert('Stärke', w.staerke), wert('Krit', w.krit, ' %'),
        wert('Abbau', w.abbau, ' %'), wert('Glück', w.glueck, ' %'), wert('Ernte-Glück', w.ernteGlueck, ' %'), wert('Fokus-Mana', w.mana)] : null,
        set ? C.el('p', { class: 'inv-set' }, C.el('b', {}, `Set-Bonus: ${set.name}`), C.el('br'), set.text) : null,
        st.stashN > 0 ? knopf(`Beutel: ${zahl(st.stashN)} abholen`, abholen, `${st.stashN} Gegenstände aus dem Beutel abholen`) : null)),
    rucksack(), fuss()];
}
export function truheZeigen(j) {
  oeffne('truhe', { x: j.x, y: j.y, z: j.z, max: j.max || 27, slots: j.slots || [], besitzer: j.besitzer });
  ladeInventar();
}
function truheInhalt() {
  const o = { x: offen.x, y: offen.y, z: offen.z };
  const tun = (op, text) => knopf(text, async () => {
    try { const r = await C.api(op, o); uebernehmen(r); C.ton(13); if (!r.truhe && !r.slots) await ladeInventar(); } catch (e) { C.fehler(e); }
    zeichne();
  });
  return [kopf(offen.max > 27 ? 'Große Truhe' : 'Truhe'),
    C.el('div', { class: 'reihe' }, sortKnopf(o), tun('truhe/einlagern', 'Alles einlagern'), offen.besitzer !== false ? tun('truhe/leeren', 'Leeren → Beutel') : null),
    reihe('truhe', 0, offen.max), rucksack(), fuss()];
}
export function staenderZeigen(j) {
  oeffne('staender', { x: j.x, y: j.y, z: j.z, items: (j.items || []).slice(0, 7).concat(Array(7).fill(0)).slice(0, 7) });
  ladeInventar();
}
function staenderInhalt() {
  return [kopf('Rüstungsständer'),
    C.el('div', { class: 'inv-oben' }, C.el('div', { class: 'inv-gear' }, [0, 1, 2, 3].map(i => feld(ort('staender', i), GEAR_NAMEN[i][0]))),
      C.el('div', { class: 'inv-gear' }, [4, 5, 6].map(i => feld(ort('staender', i), '✦'))),
      C.el('p', { class: 'meta' }, 'Nur Rüstung und Talismane. Besucher sehen den Ständer auf deiner Insel.')),
    rucksack(), fuss()];
}

/* ---------- Beutel (/stash) ---------- */
export function stashAnzeigen(n) {
  let a = document.getElementById('stash-abzeichen');
  if (!a) {
    a = C.el('button', { id: 'stash-abzeichen', class: 'knopf', type: 'button', hidden: true, onclick: abholen });
    (document.getElementById('hud') || document.body).append(a);
  }
  st.stashN = n;
  a.hidden = !(n > 0);
  a.textContent = `Beutel voll! ${zahl(n)} Items im /stash – Klicke zum Abholen`;
  a.setAttribute('aria-label', `${n} Gegenstände im Beutel abholen`);
}
async function abholen() {
  try {
    const r = await C.api('stash/claim', {});
    uebernehmen(r.slots ? r : await C.api('inv'));
    C.toast(st.stashN > 0 ? `Abgeholt. ${zahl(st.stashN)} bleiben im Beutel – mach Platz im Rucksack.` : 'Alles aus dem Beutel abgeholt.', 'gut');
    C.ton(13); C.ladeMich?.();
  } catch (e) { C.fehler(e); }
  if (auf()) zeichne();
}

/* ---------- Handel ---------- */
export function handelZeigen(j) {
  if (j.t === 'handel_anfrage') return oeffne('anfrage', { von: j.von, name: j.name });
  if (j.fertig) {
    C.toast('Handel abgeschlossen.', 'gut'); C.ton(6);
    if (offen.art === 'handel') C.modalZu();
    ladeInventar(); C.ladeMich?.();
    return;
  }
  if (j.abbruch) {
    C.toast(j.abbruch, 'schlecht');
    if (offen.art === 'handel' || offen.art === 'anfrage') C.modalZu();
    return;
  }
  angebot = (j.ich?.items || []).map(([i, n]) => [i, n]);
  taler = j.ich?.taler || 0;
  sperreBis = performance.now() + (j.sperre_ms || 0);
  if (offen.art === 'handel') { offen.j = j; zeichne(); } else { oeffne('handel', { j }); ladeInventar(); }
  clearInterval(uhr);
  uhr = setInterval(() => {
    if (!auf() || offen.art !== 'handel') return clearInterval(uhr);
    const rest = Math.max(0, sperreBis - performance.now()), t = document.getElementById('handel-uhr'), k = document.getElementById('handel-ok');
    if (t) t.textContent = rest > 0 ? `Sperre: ${Math.ceil(rest / 1000)} s` : '';
    if (k) k.disabled = !(offen.j.ich?.bereit && offen.j.du?.bereit && rest <= 0);
    if (rest <= 0 && !(offen.j.ich?.bereit && offen.j.du?.bereit)) clearInterval(uhr);
  }, 200);
}
function anfrageInhalt() {
  return [kopf('Handelsanfrage'), C.el('p', {}, `${offen.name || 'Jemand'} möchte mit dir handeln.`),
    C.el('div', { class: 'reihe' },
      knopf('Annehmen', async () => { try { await C.api('trade/accept', { von: offen.von }); } catch (e) { C.fehler(e); C.modalZu(); } }),
      knopf('Ablehnen', async () => { C.modalZu(); try { await C.api('trade/cancel', {}); } catch {} }))];
}
async function senden() {
  try { await C.api('trade/offer', { items: angebot, taler }); C.ton(13, 0.5); } catch (e) { C.fehler(e); }
}
/** Eigener Platz angeklickt: links ganzer Stapel, rechts 1 Stück ins Angebot (höchstens, was man hat). */
function anbieten(o, rechts) {
  const s = o.art === 'inv' && stapel(o);
  if (!s) return;
  const habe = st.slots.reduce((a, x) => a + (x && x[0] === s[0] ? x[1] : 0), 0), da = angebot.find(a => a[0] === s[0]);
  if (!da && angebot.length >= 12) return C.toast('Höchstens 12 Stapel je Angebot.', 'schlecht');
  const neu = Math.min(habe, (da?.[1] || 0) + (rechts ? 1 : s[1]));
  if (da) da[1] = neu; else angebot.push([s[0], neu]);
  senden();
}
function handelInhalt() {
  const j = offen.j, du = j.du || {}, ich = j.ich || {};
  const seite = (items, eigen) => C.el('div', { class: 'handel-gitter', role: 'group' }, Array.from({ length: 12 }, (_, k) => {
    const s = items[k];
    if (!s) return C.el('span', { class: 'feld' });
    const b = C.el('button', { class: 'feld' + rahmen(s[0]), type: 'button', 'aria-label': `${name(s[0])}, ${s[1]} Stück${eigen ? ', anklicken nimmt zurück' : ''}` },
      C.icon(s[0]), s[1] > 1 ? C.el('b', {}, s[1]) : null);
    b.onpointerenter = e => { if (e.pointerType === 'mouse') tipAuf(s[0], s[1], b); };
    b.onpointerleave = tipZu;
    if (eigen) {
      const weg = rechts => { const a = angebot.find(a => a[0] === s[0]); if (!a) return; a[1] -= rechts ? 1 : a[1]; angebot = angebot.filter(a => a[1] > 0); tipZu(); senden(); };
      b.onclick = () => weg(false);
      b.oncontextmenu = e => { e.preventDefault(); weg(true); };
    }
    return b;
  }));
  const tal = C.el('input', { type: 'number', min: 0, value: taler, inputmode: 'numeric', 'aria-label': 'Taler im Angebot' });
  tal.onchange = () => { taler = Math.max(0, Math.min(+tal.value | 0, C.me()?.taler ?? 1e12)); senden(); };
  const bereit = C.el('input', { type: 'checkbox', checked: !!ich.bereit, 'aria-label': 'Ich bin bereit' });
  bereit.onchange = async () => { try { await C.api('trade/ready', { an: bereit.checked }); } catch (e) { C.fehler(e); } };
  const ok = C.el('button', { id: 'handel-ok', class: 'knopf', type: 'button', disabled: !(ich.bereit && du.bereit && performance.now() >= sperreBis) },
    'Handel abschließen');
  ok.onclick = async () => { try { await C.api('trade/confirm', {}); } catch (e) { C.fehler(e); } };
  return [kopf(`Handel mit ${j.mit ?? '?'}`),
    C.el('div', { class: 'handel' },
      C.el('div', {}, C.el('b', { class: 'inv-titel' }, 'Dein Angebot'), seite(angebot, true),
        C.el('label', { class: 'reihe' }, 'Taler', tal), C.el('label', { class: 'schalter' }, bereit, 'Bereit')),
      C.el('div', {}, C.el('b', { class: 'inv-titel' }, `${j.mit ?? 'Gegenüber'} bietet`), seite(du.items || [], false),
        C.el('p', {}, `${zahl(du.taler || 0)} Taler`), C.el('p', { class: du.bereit ? 'handel-bereit' : 'meta' }, du.bereit ? '✔ Bereit' : 'Noch nicht bereit'))),
    C.el('div', { class: 'reihe' }, ok, C.el('span', { id: 'handel-uhr', class: 'meta', 'aria-live': 'polite' }),
      knopf('Abbrechen', async () => { try { await C.api('trade/cancel', {}); } catch (e) { C.fehler(e); } C.modalZu(); })),
    C.el('p', { class: 'meta' }, 'Klicke unten auf deine Gegenstände, um sie anzubieten. Jede Änderung setzt „Bereit“ zurück und startet 3 s Sperre.'),
    rucksack().slice(1)];
}

/* ---------- Tooltip ---------- */
const BONUS = { leben: ['Leben', ''], verteidigung: ['Verteidigung', ''], staerke: ['Stärke', ''], schaden: ['Schaden', ''], krit: ['Krit-Chance', ' %'],
  abbau: ['Abbau-Tempo', ' %'], glueck: ['Glück', ' %'], ernteGlueck: ['Ernte-Glück', ' %'], mana: ['Fokus-Mana', ''], manaRegen: ['Mana je s', ''],
  regen: ['Leben je s', ''], taler: ['Taler', ' %'], manaKosten: ['Mana-Kosten', ' % weniger'] };
const PLATZ = ['Helm', 'Brustpanzer', 'Beinschutz', 'Stiefel', 'Talisman', 'Nahkampfwaffe', 'Fernwaffe'];
export function tooltip(item, n = 1) {
  const g = G(item), e = E(item), sel = g?.seltenheit || 0, set = e && e.set != null ? R().sets.find(s => s.id === e.set) : null;
  const stufen = ench.get(item) || [];
  return C.el('div', { class: 'tip' },
    C.el('b', { class: 'tip-name r' + sel }, name(item)),
    C.el('span', { class: 'tip-art r' + sel }, `${R().seltenheiten[sel] || ''}${e ? ' · ' + PLATZ[e.platz] : ''}`),
    e ? Object.entries(e.bonus || {}).filter(([, v]) => v).map(([k, v]) => C.el('span', { class: 'tip-bonus' }, `${v > 0 ? '+' : ''}${zahl(v)}${BONUS[k]?.[1] ?? ''} ${BONUS[k]?.[0] ?? k}`)) : null,
    stufen.map((l, i) => l ? C.el('span', { class: 'tip-ench' }, `${R().verzauberungen[i]?.name} ${ROEM[l] || l}`) : null),
    set ? C.el('span', { class: 'tip-set' }, `${set.name}: ${set.text}`) : null,
    g?.lore || e?.lore ? C.el('i', { class: 'tip-lore' }, g?.lore || e.lore) : null,
    (g?.wert || e?.wert) ? C.el('span', { class: 'tip-wert' }, `Verkaufswert: ${zahl((g?.wert || e.wert) * n)} Taler${n > 1 ? ` (${n} Stück)` : ''}`) : null);
}
function tipAuf(item, n, an) {
  const t = document.getElementById('inv-tip');
  if (!t || zeiger) return;
  t.replaceChildren(tooltip(item, n));
  t.hidden = false;
  const r = an.getBoundingClientRect(), w = t.offsetWidth, h = t.offsetHeight;
  const x = r.right + 8 + w > innerWidth ? Math.max(4, r.left - w - 8) : r.right + 8;
  t.style.transform = `translate(${x}px, ${Math.max(4, Math.min(innerHeight - h - 4, r.top))}px)`;
}
function tipZu() { const t = document.getElementById('inv-tip'); if (t) t.hidden = true; lang = false; }

/* ---------- Verzaubern (Menü-Seite) ---------- */
export async function verzaubernSeite() {
  const huelle = C.el('div', { class: 'verzaubern' });
  let wahl = 0;
  const zeig = async () => {
    const r = await ladeEnch(), liste = r.gegenstaende || [];
    if (!liste.length) return huelle.replaceChildren(C.el('p', {}, 'Du hast nichts Verzauberbares. Werkzeuge, Waffen, Rüstung und Talismane lassen sich am Altar im Bibliotheks-Turm verzaubern.'));
    if (!liste.some(g => g.item === wahl)) wahl = liste[0].item;
    const g = liste.find(x => x.item === wahl), art = G(wahl)?.verzauberbar || 0;
    const habe = new Map(C.me()?.inv || []), buch = habe.get(VERZ_BUCH) || 0;
    huelle.replaceChildren(
      C.el('h2', {}, 'Gegenstand wählen'),
      C.el('div', { class: 'inv-gitter', role: 'listbox', 'aria-label': 'Verzauberbare Gegenstände' }, liste.map(x => C.el('button', {
        class: 'feld' + rahmen(x.item), type: 'button', role: 'option', 'aria-selected': String(x.item === wahl), 'aria-label': name(x.item),
        onclick: () => { wahl = x.item; zeig(); },
      }, C.icon(x.item)))),
      C.el('div', { class: 'kachel verz-wahl' }, tooltip(wahl, 1)),
      C.el('h2', {}, 'Verzauberungen'),
      buch ? C.el('p', { class: 'meta' }, `Du hast ${buch} Verzauberungsbuch – es ersetzt die Splitter-Kosten.`) : null,
      C.el('div', { class: 'gitter' }, R().verzauberungen.filter(e => e.arten & art).map(e => {
        const l = g.stufen[e.id] || 0, k = l < e.max ? e.kosten[l] : null, sp = SHARD + e.fach;
        const reicht = k && (buch > 0 || (habe.get(sp) || 0) >= k.splitter) && (habe.get(FOKUS_KRISTALL) || 0) >= k.kristalle;
        return C.el('div', { class: 'kachel' },
          C.el('b', {}, `${e.name} ${ROEM[l] || ''}`),
          C.el('span', { class: 'stufen', 'aria-label': `Stufe ${l} von ${e.max}` }, Array.from({ length: e.max }, (_, i) => C.el('i', { class: i < l ? 'an' : '' }, ROEM[i + 1]))),
          C.el('span', { class: 'meta' }, e.text),
          k ? C.el('span', { class: 'meta' }, `Stufe ${ROEM[l + 1]}: ${buch ? '1× Verzauberungsbuch' : `${k.splitter}× ${C.nameVon(sp)}`} + ${k.kristalle}× ${C.nameVon(FOKUS_KRISTALL)}`)
            : C.el('span', { class: 'meta' }, 'Höchste Stufe erreicht.'),
          k ? C.el('button', { class: 'knopf klein', type: 'button', disabled: !reicht, 'aria-label': `${e.name} ${ROEM[l + 1]} auf ${name(wahl)} verzaubern`, onclick: async () => {
            try {
              await C.api('enchant', { item: wahl, ench: e.id });
              C.ton(14); C.toast(`${name(wahl)}: ${e.name} ${ROEM[l + 1]}!`, 'gut');
              await C.ladeMich?.();
            } catch (x) { C.fehler(x); }
            zeig();
          } }, 'Verzaubern') : null,
          // Stufe VI (Phase 24): nur mit einem Folianten aus den Katakomben, der alle anderen Kosten ersetzt
          l === 5 && e.max === 5 && (habe.get(FOLIANT_VI) || 0) > 0 ? C.el('button', { class: 'knopf klein', type: 'button', 'aria-label': `${e.name} VI auf ${name(wahl)} verzaubern, verbraucht einen Folianten der Stufe VI`, onclick: async () => {
            try {
              await C.api('enchant', { item: wahl, ench: e.id });
              C.ton(14);
              await C.ladeMich?.();
            } catch (x) { C.fehler(x); }
            zeig();
          } }, 'Stufe VI (Foliant)') : null);
      })),
      C.el('p', { class: 'meta' }, 'Verzaubern geht nur in der Nähe des Altars im Bibliotheks-Turm.'));
  };
  await zeig();
  return [huelle];
}

/* ---------- Figur-Vorschau mit Rüstung (Canvas 2D, Kopie von hud.js avatarVorschau) ---------- */
let winkel = 0.5, leinwand = null;
const hell = (rgb, f) => rgb.map(c => Math.max(0, Math.min(255, c * f)));
const farbe = rgb => `rgb(${rgb.map(Math.round).join(',')})`;
function teile(l, t) {
  const reg = C.reg(), F = i => reg.farben?.[i] || [120, 120, 130], haut = reg.haut?.[l.skin] || [240, 200, 160], haar = F(l.hair_col), oben = F(l.col_a), unten = F(l.col_b);
  const cos = s => reg.cosmetics?.find(c => c.id === (l.slots?.[s] || 0));
  const ruest = k => (st.gear[k] && E(st.gear[k])?.rgb) || null;
  const sw = Math.sin(t / 260) * 1.4, b = [], q = (...a) => b.push(a), dunkel = [42, 30, 22];
  q(-4, 0, -2 + sw, 0, 12, 2 + sw, unten); q(0, 0, -2 - sw, 4, 12, 2 - sw, unten);
  q(-4.2, 0, -2.2 + sw, -0.1, 2.2, 2.6 + sw, [70, 48, 32]); q(0.1, 0, -2.2 - sw, 4.2, 2.2, 2.6 - sw, [70, 48, 32]);
  q(-8, 17, -2 - sw, -4, 24, 2 - sw, oben); q(4, 17, -2 + sw, 8, 24, 2 + sw, oben);
  q(-8, 12, -2 - sw, -4, 17, 2 - sw, haut); q(4, 12, -2 + sw, 8, 17, 2 + sw, haut);
  q(-4, 12, -2, 4, 24, 2, oben);
  q(-4.15, 12, -2.15, 4.15, 13.4, 2.15, dunkel); q(-0.8, 12.1, 2.15, 0.8, 13.3, 2.35, [230, 190, 70]);
  q(-4, 24, -4, 4, 32, 4, haut);
  q(-3, 28, 4, -1, 29.5, 4.12, [250, 250, 250]); q(1, 28, 4, 3, 29.5, 4.12, [250, 250, 250]);
  q(-2, 28, 4.12, -1, 29.5, 4.22, [30, 30, 50]); q(1, 28, 4.12, 2, 29.5, 4.22, [30, 30, 50]);
  q(-1.5, 25.6, 4, 1.5, 26.3, 4.12, [150, 60, 60]);
  const helm = ruest(0), brust = ruest(1), beine = ruest(2), stiefel = ruest(3);
  if (l.hair !== 4 && !helm) {
    q(-4.4, 30.6, -4.4, 4.4, l.hair === 3 ? 34 : 32.6, 4.4, haar); q(-4.4, l.hair === 1 ? 19 : 25, -4.6, 4.4, 32, -3.6, haar);
    q(-4.5, 27, -4.4, -3.9, 32, 2, haar); q(3.9, 27, -4.4, 4.5, 32, 2, haar);
  }
  // Rüstung als Überzug, etwas größer als die Körperteile
  if (helm) { q(-4.6, 30.4, -4.6, 4.6, 33, 4.6, helm); q(-4.6, 25, -4.6, -3.9, 30.4, 4.6, helm); q(3.9, 25, -4.6, 4.6, 30.4, 4.6, helm); q(-4.6, 25, -4.6, 4.6, 30.4, -3.9, helm); q(-0.5, 26.5, 4.2, 0.5, 30.4, 4.7, hell(helm, 0.8)); }
  if (brust) { q(-4.5, 12.6, -2.5, 4.5, 24.4, 2.5, brust); q(-8.5, 19, -2.5 - sw, -3.9, 24.5, 2.5 - sw, hell(brust, 0.9)); q(3.9, 19, -2.5 + sw, 8.5, 24.5, 2.5 + sw, hell(brust, 0.9)); }
  if (beine) { q(-4.4, 3, -2.4 + sw, -0.05, 12.8, 2.4 + sw, beine); q(0.05, 3, -2.4 - sw, 4.4, 12.8, 2.4 - sw, beine); }
  if (stiefel) { q(-4.5, 0, -2.5 + sw, -0.05, 3.5, 2.9 + sw, stiefel); q(0.05, 0, -2.5 - sw, 4.5, 3.5, 2.9 - sw, stiefel); }
  const tal = [4, 5, 6].map(ruest).find(Boolean);
  if (tal) q(-1, 20, 2.5, 1, 22, 2.95, hell(tal, 1.3));
  if (cos(0)) q(-4, 5, -3, 4, 23.5, -2.6, cos(0).rgb);
  if (cos(2)) { q(-12, 14, -3.8, -2, 26, -3, cos(2).rgb); q(2, 14, -3.8, 12, 26, -3, cos(2).rgb); }
  if (cos(3)) { const h = Math.sin(t / 400) * 1.2; q(9, 24 + h, -1, 13, 28 + h, 3, cos(3).rgb); }
  return b;
}
const FLAECHEN = [[[0, 2, 6, 4], [-1, 0, 0]], [[1, 5, 7, 3], [1, 0, 0]], [[0, 4, 5, 1], [0, -1, 0]], [[2, 3, 7, 6], [0, 1, 0]], [[0, 1, 3, 2], [0, 0, -1]], [[4, 6, 7, 5], [0, 0, 1]]];
function figurCanvas() {
  if (leinwand) { if (!leinwand.dataset.laeuft) starte(leinwand); return leinwand; }
  const cv = leinwand = C.el('canvas', { width: 180, height: 240, role: 'img', 'aria-label': 'Vorschau deiner Figur mit Rüstung, zum Drehen ziehen' });
  let zieht = null;
  cv.addEventListener('pointerdown', e => { zieht = e.clientX; cv.setPointerCapture(e.pointerId); });
  cv.addEventListener('pointermove', e => { if (zieht != null) { winkel += (e.clientX - zieht) * 0.02; zieht = e.clientX; } });
  for (const k of ['pointerup', 'pointercancel']) cv.addEventListener(k, () => { zieht = null; });
  cv.ziehtNicht = () => zieht == null;
  starte(cv);
  return cv;
}
function starte(cv) {
  const g = cv.getContext('2d'), neig = -0.22, cn = Math.cos(neig), sn = Math.sin(neig), s = 5.2, licht = [0.45, 0.75, 0.5];
  let weg = 0;
  cv.dataset.laeuft = '1';
  const male = t => {
    if (!cv.isConnected) { if (++weg > 60) { delete cv.dataset.laeuft; return; } } else weg = 0; // Fenster zu: Schleife endet
    requestAnimationFrame(male);
    if (cv.ziehtNicht()) winkel += 0.01;
    const look = C.me()?.look;
    if (!look || !C.reg()) return;
    const ca = Math.cos(winkel), sa = Math.sin(winkel);
    const dreh = ([x, y, z]) => { const x1 = x * ca + z * sa, z1 = -x * sa + z * ca; return [x1, y * cn - z1 * sn, y * sn + z1 * cn]; };
    const faces = [];
    for (const [x0, y0, z0, x1, y1, z1, rgb] of teile(look, t)) {
      const ecken = [0, 1, 2, 3, 4, 5, 6, 7].map(i => dreh([i & 1 ? x1 : x0, (i & 2 ? y1 : y0) - 17, i & 4 ? z1 : z0]));
      for (const [idx, n] of FLAECHEN) {
        const nn = dreh(n);
        if (nn[2] <= 0.001) continue;
        const p = idx.map(i => ecken[i]), f = 0.55 + 0.45 * Math.max(0, nn[0] * licht[0] + nn[1] * licht[1] + nn[2] * licht[2]);
        faces.push([p.reduce((a, v) => a + v[2], 0) / 4, p, farbe(hell(rgb, f))]);
      }
    }
    faces.sort((a, b) => a[0] - b[0]);
    g.clearRect(0, 0, cv.width, cv.height);
    for (const [, p, c] of faces) {
      g.beginPath();
      p.forEach(([x, y], i) => g[i ? 'lineTo' : 'moveTo'](90 + x * s, 134 - y * s));
      g.closePath(); g.fillStyle = c; g.strokeStyle = c; g.lineWidth = 0.7; g.fill(); g.stroke();
    }
  };
  requestAnimationFrame(male);
}
