// Campus – Lern-Duell-Arena (Phase 23): Themen-Wahl, Stichwahl, Frage-Anzeige, Ergebnis, Rangliste.
// Der Server führt die Runde; hier wird nur angezeigt und gewählt (CONTRACT-21-25 §S-DUEL, §W-SPIEL).
let C = null;                       // ctx aus hud.js
const FELD = ['A', 'B', 'C', 'D'], KLASSE = ['a', 'b', 'c', 'd'];
const STUFEN = ['Klasse 5–7', 'Klasse 8–10', 'Oberstufe / Uni'];
let d = null;                       // letzte Hud-Nachricht
let frage = null;                   // letzte Frage (die Auflösung bringt Text und Antworten nicht noch einmal mit)
let gewaehlt = -1, ende = 0, dauer = 1, uhr = 0, rouletteUhr = 0;
let wahl = null;                    // Knoten des Wahl-Fensters (bleiben stehen, damit das Wunsch-Thema beim Tippen nicht verschwindet)
let meine = null, stufe = 1;        // eigene Stimme

const zahl = n => Number(n || 0).toLocaleString('de-AT');
const faecher = () => (C.reg()?.sky?.faecher || ['Mathematik', 'Sprachen', 'Naturwissenschaften', 'Gesellschaft']).slice(0, 4);
const knopf = (text, f, klasse = 'knopf klein') => C.el('button', { class: klasse, type: 'button', onclick: f }, text);
const hud = () => {
  let h = C.$('duell-hud');
  if (!h) { h = C.el('div', { id: 'duell-hud', hidden: true }); (C.$('hud') || document.body).append(h); }
  return h;
};
const unserDialog = () => !!wahl && wahl.wurzel.isConnected && !C.$('dialog').hidden;
const dialogZu = () => { clearInterval(rouletteUhr); if (unserDialog()) C.modal.zu(); };
const wer = id => d?.spieler?.find(s => s.id === id);

export function initDuell(ctx) { C = ctx; }

export function duellHud(j) {
  if (!C || !j) return;
  if (j.t === 'duell_einladung') return einladung(j);
  if (j.aus) return aus();
  if (typeof j.phase !== 'string') return;
  const neu = !d || d.phase !== j.phase || d.runde !== j.runde;
  d = j;
  if (j.rest_ms != null) { ende = performance.now() + j.rest_ms; if (neu) dauer = Math.max(1, j.rest_ms); }
  if (j.phase === 'lobby') { if (neu) { meine = null; frage = null; wahlAuf(); } wahlStand(); }
  else if (j.phase === 'roulette') { if (neu) roulette(); }
  else {
    if (neu) dialogZu();
    if (j.phase === 'frage' && neu) { frage = j; gewaehlt = -1; C.ton(13, 0.8); }
    if (j.phase === 'ende' && neu) C.ton(j.sieger === C.me()?.id ? 14 : 13);
  }
  zeichne();
  if (!uhr) uhr = setInterval(takt, 100);
}
function aus() {
  clearInterval(uhr); uhr = 0;
  dialogZu();
  d = frage = wahl = meine = null; gewaehlt = -1;
  const h = hud();
  h.hidden = true; h.replaceChildren();
}
/** Zeitbalken und Rest-Sekunden laufen zwischen den Nachrichten (höchstens 1×/s) weich weiter. */
function takt() {
  const rest = Math.max(0, ende - performance.now());
  for (const b of document.querySelectorAll('.duell-zeit > i')) b.style.width = `${Math.min(100, rest / dauer * 100)}%`;
  for (const t of document.querySelectorAll('.duell-rest')) t.textContent = `${Math.ceil(rest / 1000)} s`;
}
const restText = () => C.el('span', { class: 'duell-rest', 'aria-hidden': 'true' }, `${Math.ceil(Math.max(0, ende - performance.now()) / 1000)} s`);

/* ---------- Einladung ---------- */
function einladung(j) {
  const von = String(j.von || '').slice(0, 32);
  if (!von) return;
  C.$('dialog').replaceChildren(C.zuKnopf(), C.el('h3', {}, C.el('span', {}, 'Lern-Duell'), 'Einladung'),
    C.el('p', {}, `${von} fordert dich zu einem Lern-Duell heraus.`),
    C.el('div', { class: 'reihe' },
      knopf('Annehmen', () => { C.modal.zu(); C.game()?.say(0, `/duel join ${von}`); }, 'knopf voll'),
      knopf('Ablehnen', () => C.modal.zu(), 'knopf')));
  C.modal.auf('dialog');
  C.ton(7, 0.7);
}

/* ---------- Themen-Wahl (Lobby) ---------- */
async function stimme(neu) {
  meine = neu;
  wahlStand();
  try { await C.api('duell/wahl', { ...neu, stufe }); C.ton(13, 0.5); } catch (e) { C.fehler(e); }
}
function wahlAuf() {
  const auto = knopf('', () => stimme({ art: 'auto' }), 'knopf duell-auto');
  const fach = faecher().map((f, i) => knopf(f, () => stimme({ art: 'fach', fach: i }), 'kachel duell-fach'));
  const stufen = STUFEN.map((s, i) => knopf(s, () => { stufe = i; if (meine) stimme(meine); else wahlStand(); }));
  const frei = C.el('input', { type: 'text', maxlength: 60, placeholder: 'z. B. Photosynthese', 'aria-label': 'Wunsch-Thema', autocomplete: 'off' });
  const form = C.el('form', { class: 'duell-frei', onsubmit: e => {
    e.preventDefault();
    const thema = frei.value.trim().slice(0, 60);
    if (!thema) return C.toast('Schreib zuerst ein Wunsch-Thema.', 'schlecht');
    stimme({ art: 'frei', thema });
  } }, frei, C.el('button', { class: 'knopf klein', type: 'submit' }, 'Wunsch-Thema wählen'));
  const stimmen = C.el('div', { class: 'duell-stimmen', 'aria-live': 'polite' });
  const wurzel = C.el('div', { class: 'duell-wahl' },
    C.el('h3', {}, C.el('span', {}, 'Lern-Duell'), 'Themen-Wahl ', restText()),
    auto,
    C.el('div', { class: 'duell-faecher', role: 'group', 'aria-label': 'Fach' }, fach),
    C.el('div', { class: 'duell-stufen reihe', role: 'group', 'aria-label': 'Stufe' }, stufen),
    form, stimmen,
    C.el('p', { class: 'meta' }, 'Die meisten Stimmen gewinnen, bei Gleichstand entscheidet die Stichwahl. Ohne Stimme gilt die Empfehlung.'));
  wahl = { wurzel, auto, fach, stufen, frei, stimmen };
  wahlZeigen();
}
function wahlZeigen() {
  if (!wahl) return;
  C.$('dialog').replaceChildren(C.zuKnopf(), wahl.wurzel);
  C.modal.auf('dialog');
}
function wahlStand() {
  if (!wahl || d?.phase !== 'lobby') return;
  const f = faecher(), ich = C.me()?.id, st = d.stimmen || [];
  const m = st.find(s => s.id === ich);
  if (m && !meine) { meine = { art: m.art, fach: m.fach, thema: m.thema }; if (m.stufe >= 0 && m.stufe <= 2) stufe = m.stufe; }
  wahl.auto.textContent = `${d.auto?.name || f[d.auto?.fach] || 'Auto'} – Empfohlen aus eurem Lernhelfer`;
  wahl.auto.setAttribute('aria-pressed', String(meine?.art === 'auto'));
  wahl.fach.forEach((b, i) => b.setAttribute('aria-pressed', String(meine?.art === 'fach' && meine.fach === i)));
  wahl.stufen.forEach((b, i) => b.setAttribute('aria-pressed', String(i === stufe)));
  wahl.stimmen.replaceChildren(...st.map(s => C.el('div', { class: 'zeile' }, wer(s.id)?.name || '?',
    C.el('b', {}, `${s.art === 'frei' ? s.thema : s.art === 'fach' ? f[s.fach] ?? '?' : 'Empfehlung'} · ${STUFEN[s.stufe] ?? ''}`))));
}

/* ---------- Stichwahl ---------- */
function roulette() {
  clearInterval(rouletteUhr);
  const k = (d.kandidaten || []).map(String), ziel = Math.max(0, k.indexOf(String(d.sieger)));
  const li = k.map(t => C.el('li', {}, t));
  const wurzel = C.el('div', { class: 'duell-wahl' }, C.el('h3', {}, C.el('span', {}, 'Lern-Duell'), 'Stichwahl'),
    C.el('ul', { class: 'duell-roulette', 'aria-label': 'Kandidaten der Stichwahl' }, li),
    C.el('p', { class: 'meta', role: 'status' }));
  wahl = { ...(wahl || {}), wurzel };
  C.$('dialog').replaceChildren(C.zuKnopf(), wurzel);
  C.modal.auf('dialog');
  if (!li.length) return;
  let i = 0;
  const zeig = n => li.forEach((x, m) => x.classList.toggle('aktiv', m === n));
  zeig(0);
  rouletteUhr = setInterval(() => {
    if (ende - performance.now() > 350) { zeig(++i % li.length); C.ton(13, 0.3); return; }
    clearInterval(rouletteUhr);
    zeig(ziel);
    wurzel.lastChild.textContent = `Gewählt: ${k[ziel]}`;
  }, 110);
}

/* ---------- Anzeige während der Runde (nicht modal) ---------- */
async function antworte(i) {
  if (d?.phase !== 'frage') return;
  gewaehlt = i;
  zeichne();
  try { await C.api('duell/antwort', { feld: i }); } catch (e) { C.fehler(e); }
}
function punkte() {
  const ich = C.me()?.id;
  return C.el('div', { class: 'duell-punkte' }, [...(d.spieler || [])].sort((a, b) => b.punkte - a.punkte).map(s =>
    C.el('span', { class: `duell-spieler farbe${s.farbe | 0}${s.id === ich ? ' ich' : ''}` }, `${s.name}${s.bot ? ' (Bot)' : ''} `,
      C.el('b', {}, zahl(s.punkte)), s.serie > 1 ? ` ×${s.serie}` : '')));
}
function zeichne() {
  const j = d, h = hud(), t = [];
  if (j.phase === 'lobby') {
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, 'Lern-Duell – Lobby'), ' Start in ', restText()),
      knopf('Thema wählen', wahlZeigen));
  } else if (j.phase === 'roulette') {
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, 'Stichwahl …')));
  } else if (j.phase === 'laden') {
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, 'Fragen werden vorbereitet …')),
      C.el('p', { class: 'duell-frage' }, `${j.thema || ''} · ${STUFEN[j.stufe] ?? ''}`));
  } else if (j.phase === 'frage' || j.phase === 'aufloesung') {
    const f = frage || {}, los = j.phase === 'aufloesung';
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, `Runde ${j.runde ?? '?'}/${f.von || 8}`), f.thema ? ` · ${f.thema} ` : ' ', los ? null : restText()),
      C.el('p', { class: 'duell-frage' }, f.frage || ''),
      C.el('div', { class: 'duell-antworten', role: 'group', 'aria-label': 'Antworten' }, FELD.map((b, i) => C.el('button', {
        class: `duell-feld ${KLASSE[i]}${i === gewaehlt ? ' gewaehlt' : ''}${los ? (i === j.richtig ? ' richtig' : ' falsch') : ''}`, type: 'button', disabled: los,
        'aria-pressed': String(i === gewaehlt), 'aria-label': `${b}: ${f.antworten?.[i] ?? ''}${los ? (i === j.richtig ? ', richtig' : ', falsch') : ''}`,
        onclick: () => antworte(i),
      }, C.el('b', {}, b), ' ', f.antworten?.[i] ?? ''))));
    if (los) {
      const s = wer(j.schnellster);
      t.push(C.el('p', { class: 'duell-erklaerung', role: 'status' }, C.el('b', {}, `Richtig: ${FELD[j.richtig] ?? '?'}. `), j.erklaerung || '',
        s ? ` Am schnellsten: ${s.name}.` : ''));
    } else {
      t.push(C.el('div', { class: 'duell-zeit', role: 'presentation' }, C.el('i')),
        C.el('p', { class: 'meta' }, C.TOUCH ? 'Tippe auf ein Feld oder stell dich darauf.' : 'Stell dich auf das Feld mit der richtigen Antwort.'));
    }
  } else if (j.phase === 'ende') {
    const s = wer(j.sieger), b = j.belohnung || {}, lohn = [];
    if (b.taler) lohn.push(`+${zahl(b.taler)} Taler`);
    // `item` ist die Splitter-Sorte (Fach), kein eigener Gewinn
    if (b.splitter) lohn.push(`+${zahl(b.splitter)} ${C.reg()?.sky?.items?.find(i => i.id === b.item)?.name || 'Wissens-Splitter'}`);
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, j.sieger === C.me()?.id ? 'Du hast gewonnen!' : s ? `${s.name} gewinnt das Duell.` : 'Das Duell ist vorbei.')),
      C.el('p', { class: 'duell-erklaerung', role: 'status' }, lohn.length ? `Belohnung: ${lohn.join(' · ')}` : 'Keine Belohnung.'),
      j.elo ? C.el('p', { class: 'duell-erklaerung' }, `Duell-Wertung: ${zahl(j.elo.alt)} → ${zahl(j.elo.neu)} (${j.elo.neu >= j.elo.alt ? '+' : '−'}${zahl(Math.abs(j.elo.neu - j.elo.alt))})`)
        : C.el('p', { class: 'meta' }, 'Übungsrunde – ohne Wertung.'));
  }
  t.push(punkte());
  if (j.phase !== 'ende') t.push(knopf('Verlassen', () => C.game()?.say(0, '/leave')));
  h.replaceChildren(...t);
  h.hidden = false;
  takt();
}

/* ---------- Menü-Seite „Duell-Arena“ ---------- */
export async function ranglisteSeite() {
  let r = { spieler: [], ich: null };
  try { r = await C.api('duell/rangliste'); } catch (e) { C.fehler(e); }
  const start = (text, befehl, klasse) => knopf(text, () => { C.modal.zu(); C.game()?.say(0, befehl); }, klasse);
  const ich = r.ich, name = C.me()?.name;
  return [
    C.el('p', {}, 'Acht Fragen, vier farbige Felder: Stell dich auf die richtige Antwort, bevor die Zeit abläuft. Falsche Felder brechen weg.'),
    C.el('div', { class: 'reihe' }, start('Duell starten', '/duel', 'knopf voll'), start('Solo gegen Professor Kalkulus', '/duel solo', 'knopf')),
    C.el('p', { class: 'meta' }, 'Freunde forderst du mit /duel <Name> heraus. Solo-Runden zählen nicht für die Wertung.'),
    C.el('h2', {}, 'Deine Duell-Wertung'),
    ich ? C.el('div', { class: 'reihe' }, C.el('span', { class: 'chip' }, `Wertung: ${zahl(ich.elo)}`), C.el('span', { class: 'chip' }, `Siege: ${zahl(ich.siege)}`), C.el('span', { class: 'chip' }, `Spiele: ${zahl(ich.spiele)}`))
      : C.el('p', { class: 'meta' }, 'Noch kein gewertetes Duell gespielt.'),
    C.el('h2', {}, 'Rangliste'),
    (r.spieler || []).length ? r.spieler.map((s, i) => C.el('div', { class: 'zeile' + (s.name === name ? ' ich' : '') }, `${i + 1}. ${s.name}`,
      C.el('span', {}, C.el('b', {}, zahl(s.elo)), ` · ${zahl(s.siege)} Siege / ${zahl(s.spiele)} Spiele`)))
      : C.el('p', { class: 'meta' }, 'Noch niemand in der Wertung.'),
  ];
}
