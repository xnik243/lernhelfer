// Campus – Foto-Modus (F8: Freikamera, Filter, Brennweite, Screenshot) und Kamerafahrten beim ersten Besuch einer Zone.
let ctx = null, aktiv = false, filter = 0, fokus = 0, faellig = false, faehrt = false, frei = 0;
const FILTER = ['Standard', 'Retro-Pixel', 'Goldene Stunde', 'Kino-Kontrast'];
const FAHRT = { 'Marktplatz': 0, 'Prüfungs-Zitadelle': 1, 'Prüfungs-Katakomben': 2, 'Lern-Duell-Arena': 3 };
const MERK = 'campus-kino';

const leiste = () => ctx.$('foto-leiste');

function gesehen() {
  try { const l = JSON.parse(localStorage.getItem(MERK) || '[]'); return Array.isArray(l) ? l : []; } catch { return []; }
}

function anwenden() {
  ctx.game()?.set_filter?.(filter, fokus);
  const l = leiste();
  if (!l) return;
  l.querySelectorAll('.foto-filter').forEach((k, i) => k.setAttribute('aria-pressed', String(i === filter)));
  const r = l.querySelector('input.foto-fokus'), w = l.querySelector('.foto-fokus-wert');
  if (r) r.value = String(fokus);
  if (w) w.textContent = fokus ? `${fokus} Blöcke` : 'aus';
}
function setFilter(n) { filter = n; anwenden(); }
function setFokus(v) { fokus = Math.min(64, Math.max(0, Math.round(Number(v) || 0))); anwenden(); }
function knipsen() { faellig = true; }

export function initFoto(c) {
  ctx = c;
  const { el, $ } = ctx;
  leiste()?.replaceChildren(
    ...FILTER.map((name, i) => el('button', { type: 'button', class: 'foto-filter', 'aria-pressed': String(i === filter), title: `Taste ${i + 1}`, onclick: () => setFilter(i) }, name)),
    el('label', { class: 'foto-fokus' }, 'Brennweite ',
      el('input', { type: 'range', class: 'foto-fokus', min: 0, max: 64, step: 1, value: fokus, 'aria-label': 'Brennweite in Blöcken, 0 ist aus', oninput: e => setFokus(e.target.value) }),
      el('span', { class: 'foto-fokus-wert' }, 'aus')),
    el('button', { type: 'button', class: 'foto-knopf', title: 'Eingabetaste', onclick: knipsen }, 'Screenshot speichern'),
    el('button', { type: 'button', class: 'foto-knopf', title: 'F8', onclick: fotoUmschalten }, 'Beenden'),
    ctx.TOUCH ? '' : el('span', { class: 'foto-tipp' }, 'Klick ins Bild: umsehen · Esc: Zeiger frei'));
  // Zeiger losgelassen: merken, damit dasselbe Esc nicht auch gleich den Modus beendet
  document.addEventListener('pointerlockchange', () => { if (!document.pointerLockElement) frei = performance.now(); });
  const k = $('kino');
  if (k) {
    k.replaceChildren(el('div', { class: 'kino-balken oben' }), el('div', { class: 'kino-balken unten' }),
      el('div', { class: 'kino-hinweis' }, ctx.TOUCH ? '[Tippen zum Überspringen]' : '[Leertaste zum Überspringen]'));
    k.onclick = () => ctx.game()?.skip_cutscene?.();
  }
  const b = $('b-foto');
  if (b) b.onclick = fotoUmschalten;
}

export function fotoUmschalten() {
  const g = ctx?.game();
  if (!g || (faehrt && !aktiv)) return;
  aktiv = !aktiv;
  document.body.classList.toggle('foto', aktiv);
  if (leiste()) leiste().hidden = !aktiv;
  g.set_photo?.(aktiv);
  if (aktiv) {
    ctx.modal.zu(true);
    document.exitPointerLock?.();   // Zeiger frei für die Leiste; Klick ins Bild fängt ihn wieder zum Umsehen
    ctx.optAnwenden();   // hebt bei fotoAktiv() auf mindestens Stufe 1 (Eco hat keinen Nachbearbeitungs-Durchgang)
    anwenden();
  } else {
    faellig = false;
    g.set_filter?.(0, 0);
    ctx.optAnwenden();
    if (!ctx.TOUCH) try { ctx.$('c').requestPointerLock?.()?.catch?.(() => {}); } catch {}
  }
}

export function fotoAktiv() { return aktiv; }

/** Direkt nach game.frame(t): nur dann steht das frische Bild noch im Zeichenpuffer. */
export function nachBild() {
  if (!faellig) return;
  faellig = false;
  const c = ctx.$('c');
  if (!c?.toBlob) return;
  c.toBlob(blob => {
    if (!blob) return ctx.toast('Das Foto konnte nicht gespeichert werden.', 'schlecht');
    const url = URL.createObjectURL(blob), d = new Date(), z = n => String(n).padStart(2, '0');
    const a = ctx.el('a', { href: url, download: `campus-${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}-${z(d.getHours())}${z(d.getMinutes())}${z(d.getSeconds())}.png` });
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
    ctx.toast('Foto gespeichert.');
  }, 'image/png');
}

export function kino(an, id) {
  faehrt = !!an;
  if (faehrt && aktiv) fotoUmschalten();
  document.body.classList.toggle('kino', faehrt);
  const k = ctx?.$('kino');
  if (k) k.hidden = !faehrt;
  if (faehrt && ctx) ctx.modal.zu(true);
}

export function kinoPruefen(zone) {
  const id = Object.hasOwn(FAHRT, zone) ? FAHRT[zone] : -1, g = ctx?.game();
  if (id < 0 || !g || aktiv || faehrt) return;
  if (id === 3 && ctx.$('dialog')?.hidden === false) return;   // Themenwahl der Duell-Lobby offen: Fahrt erst beim nächsten Besuch
  const l = gesehen();
  if (l.includes(id) || !g.cutscene?.(id)) return;   // false = gerade nicht möglich → beim nächsten Besuch wieder versuchen
  try { localStorage.setItem(MERK, JSON.stringify([...l, id])); } catch {}
}

export function taste(e) {
  if (e.type !== 'keydown' || !ctx || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target?.tagName || '')) return false;
  if (faehrt) {
    if (e.code !== 'Space') return false;
    ctx.game()?.skip_cutscene?.();
    return true;
  }
  if (!aktiv) return false;
  const n = ['Digit1', 'Digit2', 'Digit3', 'Digit4'].indexOf(e.code);
  if (n >= 0) setFilter(n);
  else if (e.code === 'Enter' || e.code === 'NumpadEnter') {
    if (e.target?.tagName === 'BUTTON') return false;   // Eingabetaste auf einem Knopf der Leiste drückt nur den Knopf
    if (!e.repeat) knipsen();
  }
  else if (e.code === 'ArrowUp') setFokus(fokus + 2);
  else if (e.code === 'ArrowDown') setFokus(fokus - 2);
  else if (e.code === 'Escape') {
    if (document.pointerLockElement) document.exitPointerLock();   // erst Zeiger frei, zweites Esc beendet
    else if (performance.now() - frei > 300) fotoUmschalten();
  }
  else return false;
  return true;
}
