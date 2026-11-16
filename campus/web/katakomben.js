// Campus – Prüfungs-Katakomben (Phase 24): Portal-Wahl, Lauf-Anzeige, Drei Weise, Ergebnis.
// Der Server führt den Lauf; hier wird nur angezeigt und gewählt (CONTRACT-21-25 §S-KATA, §W-SPIEL).
let C = null;                       // ctx aus hud.js
let stand = null;                   // letzter Lauf-Stand
let segen = false;
let unser = null;                   // Wurzel-Knoten unseres offenen #dialog-Inhalts

const zahl = n => Number(n || 0).toLocaleString('de-AT');
const zeit = ms => { const s = Math.max(0, Math.floor((Number(ms) || 0) / 1000)); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; };
const noteKlasse = n => { const t = String(n || 'C').toUpperCase(); return t === 'S+' ? 'note-sp' : `note-${['S', 'A', 'B'].includes(t) ? t.toLowerCase() : 'c'}`; };
const knopf = (text, f, klasse = 'knopf klein') => C.el('button', { class: klasse, type: 'button', onclick: f }, text);
function nameVon(id) {
  const r = C.reg() || {}, f = l => (Array.isArray(l) ? l : []).find(x => x?.id === id)?.name;
  return f(r.rpg?.gegenstaende) || f(r.sky?.items) || f(r.bloecke) || f(r.cosmetics) || `Gegenstand ${id}`;
}
const hud = () => {
  let h = C.$('kata-hud');
  if (!h) { h = C.el('aside', { id: 'kata-hud', 'aria-label': 'Prüfungs-Katakomben', hidden: true }); (C.$('hud') || document.body).append(h); }
  return h;
};
function dialog(...kinder) {
  unser = C.el('div', {}, ...kinder);
  C.$('dialog').replaceChildren(C.zuKnopf(), unser);
  C.modal.auf('dialog');
}
const dialogZu = () => { if (unser?.isConnected && !C.$('dialog').hidden) C.modal.zu(); unser = null; };
function segenSetzen(an) {
  if (an === segen) return;
  segen = an;
  try { C.game()?.set_speed?.(an ? 1.25 : 1); } catch {}
}

export function initKatakomben(ctx) { C = ctx; }

export function katakombenHud(j) {
  if (!C || !j) return;
  if (j.t === 'weise') return weise(j);
  if (j.aus) return aus();
  if (j.portal) return portal(j);
  if (j.ende) return ergebnis(j);
  if (j.stufe == null) return;
  stand = j;
  segenSetzen(!!j.segen);
  zeichne();
}

function aus() {
  segenSetzen(false);
  stand = null;
  dialogZu();
  const h = hud();
  h.hidden = true; h.replaceChildren();
}

/* ---------- Portal: Stufe wählen oder einem offenen Lauf beitreten ---------- */
async function los(op, body, text) {
  try { await C.api(op, body); dialogZu(); C.ton(9, 0.7); C.toast(text, 'gut'); } catch (e) { C.fehler(e); }
}
async function portal(j) {
  const laeufe = Array.isArray(j.laeufe) ? j.laeufe : [];
  let best = [];
  try { best = (await C.api('katakomben')).bestzeiten || []; } catch {}
  const bestVon = s => best.filter(b => b.stufe === s).sort((a, b) => a.ms - b.ms)[0];
  dialog(
    C.el('h3', {}, C.el('span', {}, 'Prüfungs-Katakomben'), 'Lauf wählen'),
    C.el('p', {}, 'Neun Kammern, drei Rätsel und der Namenlose Prüfer. Höhere Stufen: stärkere Wesen, bessere Beute.'),
    C.el('div', { class: 'kata-portal', role: 'group', 'aria-label': 'Stufe' }, [1, 2, 3, 4, 5].map(s => {
      const b = bestVon(s);
      return C.el('button', { class: 'kata-stufe', type: 'button', onclick: () => los('katakomben/start', { stufe: s }, `Katakomben Stufe ${s} – viel Erfolg!`) },
        `Stufe ${s}`, C.el('small', {}, b ? `Bestwert: ${b.note} in ${zeit(b.ms)}` : 'Noch kein Lauf'));
    })),
    C.el('h3', {}, 'Offene Läufe'),
    laeufe.length ? C.el('div', { class: 'kata-portal' }, laeufe.map(l => C.el('button', {
      class: 'kata-stufe', type: 'button', onclick: () => los('katakomben/beitreten', { name: String(l.name || '') }, `Du trittst ${l.name}s Gruppe bei.`),
    }, `${l.name} · Stufe ${l.stufe}`, C.el('small', {}, `${zahl(l.n)}/4 in der Gruppe – beitreten`))))
      : C.el('p', { class: 'meta' }, 'Gerade wartet keine Gruppe in der Start-Halle.'));
}

/* ---------- Lauf-Anzeige (nicht modal) ---------- */
function zeile(a, b) { return C.el('div', { class: 'kata-zeile' }, C.el('span', {}, a), C.el('span', {}, b)); }
function balken(name, wert) {
  const w = Math.max(0, Math.min(100, Number(wert) || 0));
  return [zeile(name, `${Math.round(w)}`), C.el('div', { class: 'kata-balken', role: 'meter', 'aria-label': name, 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-valuenow': Math.round(w) },
    C.el('i', { style: `transform:scaleX(${w / 100})` }))];
}
function zeichne() {
  const j = stand, p = j.punkte || {}, h = hud();
  h.replaceChildren(
    C.el('div', { class: 'kata-zeile' }, C.el('span', {}, `Stufe ${j.stufe}`), C.el('span', {}, `Raum ${j.raum_nr ?? '?'}/9`)),
    C.el('b', {}, String(j.raum || '')),
    C.el('div', { class: `kata-note ${noteKlasse(j.note)}`, 'aria-label': `Note ${j.note || 'C'}` }, String(j.note || 'C')),
    zeile('Zeit', zeit(j.zeit_ms)),
    zeile('Tode', zahl(j.tode)),
    zeile('Rätsel', `${j.raetsel | 0}/3${j.geheim ? ' · Schatz-Kammer offen' : ''}`),
    j.wesen ? zeile('Wesen', zahl(j.wesen)) : null,
    j.segen ? zeile('Segen', '+25 % Kraft & Tempo') : null,
    ...balken('Können', p.koennen), ...balken('Tempo', p.tempo), ...balken('Rätsel', p.raetsel),
    j.text ? C.el('p', { class: 'meta', role: 'status' }, String(j.text)) : null,
    knopf('Verlassen', () => C.game()?.say(0, '/leave')));
  h.hidden = false;
}

/* ---------- Drei Weise ---------- */
function weise(j) {
  const nr = Math.max(0, Math.min(2, j.nr | 0));
  dialog(C.el('h3', {}, C.el('span', {}, 'Halle der drei Weisen'), `${['Erster', 'Zweiter', 'Dritter'][nr]} Weiser`),
    C.el('p', {}, `„${String(j.text || '')}“`),
    C.el('p', { class: 'meta' }, 'Nur eine Truhe ist die richtige. Wer die falsche öffnet, wird verletzt und das Rätsel ist verloren.'),
    knopf('Verstanden', () => C.modal.zu(), 'knopf voll'));
  C.ton(5, 0.6);
}

/* ---------- Ergebnis ---------- */
function ergebnis(j) {
  const p = j.punkte || {}, beute = Array.isArray(j.beute) ? j.beute : [], truhen = Array.isArray(j.truhen) ? j.truhen : [];
  const truheName = { smaragd: 'Smaragd-Beutetruhe', obsidian: 'Obsidian-Beutetruhe' };
  dialog(C.el('h3', {}, C.el('span', {}, 'Prüfungs-Katakomben'), 'Lauf beendet'),
    C.el('div', { class: `kata-note ${noteKlasse(j.note)}`, 'aria-label': `Note ${j.note || 'C'}` }, String(j.note || 'C')),
    zeile('Zeit', zeit(j.zeit_ms)),
    ...balken('Können', p.koennen), ...balken('Tempo', p.tempo), ...balken('Rätsel', p.raetsel),
    C.el('h3', {}, 'Beute'),
    beute.length ? C.el('div', {}, beute.map(b => C.el('div', { class: 'zeile' }, nameVon(Number(b?.[0])), C.el('b', {}, `× ${zahl(b?.[1])}`))))
      : C.el('p', { class: 'meta' }, 'Hol dir deine Beute am Beute-Altar.'),
    truhen.length ? C.el('p', { class: 'meta' }, `Freigeschaltet: ${truhen.map(t => truheName[t] || String(t)).join(', ')}.`) : null,
    knopf('Weiter', () => C.modal.zu(), 'knopf voll'));
  C.ton(14);
}
