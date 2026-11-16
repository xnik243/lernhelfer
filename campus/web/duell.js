// Campus – Lern-Duell-Arena (Phase 23): Themen-Wahl, Stichwahl, Frage-Anzeige, Ergebnis, Rangliste.
// Der Server führt die Runde; hier wird nur angezeigt und gewählt (CONTRACT-21-25 §S-DUEL, §W-SPIEL).
let C = null;                       // ctx aus hud.js
const FELD = ['A', 'B', 'C', 'D'], KLASSE = ['a', 'b', 'c', 'd'];
let d = null;                       // letzte Hud-Nachricht
let frage = null;                   // letzte Frage (die Auflösung bringt Text und Antworten nicht noch einmal mit)
let gewaehlt = -1, ende = 0, dauer = 1, uhr = 0, rouletteUhr = 0;
let wahl = null;                    // Knoten des Wahl-Fensters (bleiben stehen, damit das Wunsch-Thema beim Tippen nicht verschwindet)
let meine = null;                   // eigene Stimme
let lernhelfer = null;              // Promise der Vorschläge aus dem Lernhelfer (einmal je Sitzung)

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
  for (const b of document.querySelectorAll('.duell-zeit:not(.gen) > i')) b.style.width = `${Math.min(100, rest / dauer * 100)}%`;
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

/* ---------- Lernhelfer: Fächer, Wochen-Ausgaben, Themen ---------- */
// Gleiche Herkunft wie der Lernhelfer: dessen Speicher-Schnittstelle (GitHub-Contents-Format) mit dem Anmelde-Cookie.
const LH = '/api/gh/repos/lernhelfer/konto/contents/';
async function lh(pfad, roh) {
  const r = await fetch(LH + pfad.split('/').map(encodeURIComponent).join('/'), { cache: 'no-store', credentials: 'same-origin',
    headers: { Accept: roh ? 'application/vnd.github.raw+json' : 'application/vnd.github+json' } });
  if (!r.ok) throw new Error(String(r.status));
  return roh ? r.text() : r.json();
}
/** Ausgabe zerlegen: ###-Themen der Zusammenfassung, dazu je Thema die Begriffe, die darin vorkommen. */
function zerlege(text) {
  const block = text.match(/```json\s*([\s\S]*?)```\s*$/);
  let daten = {};
  try { daten = block ? JSON.parse(block[1]) : {}; } catch {}
  const begriffe = (Array.isArray(daten.begriffe) ? daten.begriffe : []).map(b => String(b?.begriff || '')).filter(Boolean);
  const zus = (block ? text.slice(0, block.index) : text).split(/^## /m).find(t => /^Zusammenfassung/i.test(t)) || '';
  const themen = zus.split(/^### /m).slice(1).map(t => {
    const [kopf, ...rest] = t.split('\n'), inhalt = rest.join('\n').toLowerCase();
    return { name: kopf.trim(), unter: begriffe.filter(b => inhalt.includes(b.toLowerCase())).slice(0, 8) };
  }).filter(t => t.name);
  return { schlagzeile: String(daten.schlagzeile || ''), themen };
}
const kurzDatum = d => `${d.slice(8, 10)}.${d.slice(5, 7)}.`;
const mitUnter = t => t.name + (t.unter.length ? ` (${t.unter.join(', ')})` : '');
/** Vorschläge: je Fach „Aktuelle Woche“, „Letzte Woche“ und die einzelnen Themen der aktuellen Woche. */
async function vorschlaegeLaden() {
  const faecher = (await lh('lernseiten')).filter(x => x.type === 'dir').map(x => x.name);
  const proFach = await Promise.all(faecher.map(async fach => {
    const tage = (await lh(`lernseiten/${fach}`).catch(() => [])).map(x => x.name).filter(n => /^\d{4}-\d{2}-\d{2}\.md$/.test(n)).sort().reverse().slice(0, 2);
    const wochen = await Promise.all(tage.map(async n => ({ datum: n.slice(0, 10), ...zerlege(await lh(`lernseiten/${fach}/${n}`, true).catch(() => '')) })));
    const v = [];
    wochen.forEach((w, i) => {
      if (w.themen.length) v.push({ art: 'woche', fach, titel: `${i ? 'Letzte' : 'Aktuelle'} Woche: ${fach}`, datum: w.datum, schlagzeile: w.schlagzeile, themen: w.themen,
        thema: `${fach} Woche ${kurzDatum(w.datum)}`, kontext: `${fach}: ${w.themen.map(mitUnter).join('; ')}` });
    });
    for (const t of wochen[0]?.themen || []) v.push({ art: 'thema', fach, titel: t.name, datum: wochen[0].datum, themen: [t],
      thema: `${fach}: ${t.name}`, kontext: `${fach}: ${mitUnter(t)}` });
    return v;
  }));
  return proFach.flat();
}

/* ---------- Themen-Wahl (Lobby) ---------- */
async function stimme(neu) {
  meine = neu;
  wahlStand();
  try { await C.api('duell/wahl', neu); C.ton(13, 0.5); } catch (e) { C.fehler(e); }
}
function vorschau(v) {
  if (!wahl) return;
  wahl.vorschau.hidden = false;
  wahl.vorschau.replaceChildren(C.el('b', {}, v.titel), C.el('span', { class: 'meta' }, ` · ${v.fach}${v.datum ? ` · Ausgabe vom ${kurzDatum(v.datum)}` : ''}`),
    v.schlagzeile ? C.el('p', {}, v.schlagzeile) : null,
    C.el('ul', {}, v.themen.map(t => C.el('li', {}, C.el('b', {}, t.name), t.unter.length ? C.el('span', { class: 'meta' }, ` – ${t.unter.join(', ')}`) : null))),
    C.el('p', { class: 'meta' }, v.datum ? 'Die Fragen kommen aus genau diesen Lernhelfer-Inhalten.' : 'Die Fragen schreibt die KI zu diesem Thema.'));
}
// Reihenfolge der Fächer (Rest alphabetisch dahinter); Kategorie 0..3 = Ersatzfragen, falls die KI nicht liefert
const ORDNUNG = ['mathematik', 'vwl', 'bwl vo', 'englisch', 'bwl'];
const rang = f => { const i = ORDNUNG.indexOf(f.toLowerCase()); return i < 0 ? 99 : i; };
const kategorie = f => /^mathe/i.test(f) ? 0 : /^englisch/i.test(f) ? 1 : /^(vwl|bwl)/i.test(f) ? 3 : undefined;
const ALLGEMEIN = [
  ['Mix', 'bunt gemischt: Geografie, Geschichte, Wissenschaft, Kultur, Sport und Alltag'],
  ['Flaggen & Länder', 'Flaggen an Farben und Symbolen erkennen, Länder und ihre Besonderheiten'],
  ['Hauptstädte', 'Hauptstädte der Welt, Europa zuerst, auch ein paar knifflige'],
  ['Geschichte', 'wichtige Ereignisse, Personen und Jahreszahlen der Weltgeschichte', 3],
  ['Naturwissenschaft & Technik', 'Physik, Chemie, Biologie und Erfindungen im Alltag', 2],
  ['Weltall', 'Planeten, Sterne, Raumfahrt', 2],
  ['Tiere & Natur', 'erstaunliche Fakten über Tiere, Pflanzen und Rekorde der Natur', 2],
  ['Sport', 'Sportarten, Regeln, große Turniere und Rekorde'],
  ['Film, Musik & Kunst', 'bekannte Filme, Musik, Bands, Gemälde und Kunstwerke'],
].map(([titel, text, fach]) => ({ art: 'allgemein', fach: 'Allgemeinwissen', kat: fach, titel, themen: [{ name: titel, unter: [text] }],
  thema: `Allgemeinwissen: ${titel}`, kontext: `Allgemeinwissen, ${titel}: ${text}. Interessant und abwechslungsreich, nicht zu leicht.` }));
function chipsFuellen(liste) {
  if (!wahl) return;
  const gruppen = {};
  for (const v of liste) (gruppen[v.fach] ??= []).push(v);
  const reihe = Object.keys(gruppen).sort((a, b) => rang(a) - rang(b) || a.localeCompare(b, 'de'));
  wahl.chipKnoepfe = [];
  const chip = v => {
    const b = knopf(v.titel, () => { vorschau(v); stimme({ art: 'frei', thema: v.thema, kontext: v.kontext, fach: v.kat ?? kategorie(v.fach) }); }, `duell-chip${v.art === 'woche' ? ' woche' : ''}`);
    wahl.chipKnoepfe.push([b, v]);
    return b;
  };
  // Je Fach zum Ausklappen; offen bleibt nur, was man selbst aufklappt
  const gruppe = (name, vs) => C.el('details', { class: 'duell-chipgruppe' }, C.el('summary', {}, name, C.el('span', { class: 'meta' }, ` · ${vs.length}`)),
    C.el('div', { class: 'duell-chipreihe' }, vs.map(chip)));
  wahl.chips.replaceChildren(...reihe.map(f => gruppe(f, gruppen[f])), gruppe('Allgemeinwissen', ALLGEMEIN),
    liste.length ? null : C.el('p', { class: 'meta' }, 'Im Lernhelfer gibt es noch keine Wochen-Ausgabe.'));
  wahlStand();
}
function wahlAuf() {
  const frei = C.el('input', { type: 'text', maxlength: 60, placeholder: 'z. B. Photosynthese', 'aria-label': 'Wunsch-Thema', autocomplete: 'off' });
  const form = C.el('form', { class: 'duell-frei', onsubmit: e => {
    e.preventDefault();
    const thema = frei.value.trim().slice(0, 60);
    if (!thema) return C.toast('Schreib zuerst ein Wunsch-Thema.', 'schlecht');
    stimme({ art: 'frei', thema });
  } }, frei, C.el('button', { class: 'knopf klein', type: 'submit' }, 'Wunsch-Thema wählen'));
  const stimmen = C.el('div', { class: 'duell-stimmen', 'aria-live': 'polite' });
  const chips = C.el('div', { class: 'duell-chips', role: 'group', 'aria-label': 'Vorschläge aus dem Lernhelfer' }, C.el('p', { class: 'meta' }, 'Lernhelfer-Themen werden geladen …'));
  const vorschauBox = C.el('div', { class: 'duell-vorschau', 'aria-live': 'polite', hidden: true });
  const gen = C.el('p', { class: 'duell-gen', role: 'status' });
  const wurzel = C.el('div', { class: 'duell-wahl' },
    C.el('h3', {}, C.el('span', {}, 'Lern-Duell'), 'Themen-Wahl ', restText()),
    C.el('h4', {}, 'Aus deinem Lernhelfer'), chips, vorschauBox, gen,
    C.el('h4', {}, 'Eigenes Thema'),
    form, stimmen,
    C.el('p', { class: 'meta' }, 'Die meisten Stimmen gewinnen, bei Gleichstand entscheidet die Stichwahl. Ohne Stimme gilt die Empfehlung.'));
  wahl = { wurzel, frei, stimmen, chips, vorschau: vorschauBox, gen, chipKnoepfe: [] };
  wahlZeigen();
  (lernhelfer ??= vorschlaegeLaden().catch(() => null)).then(l => {
    if (wahl?.chips !== chips) return;  // inzwischen eine neue Lobby
    chipsFuellen(l || []);
    if (!l) chips.prepend(C.el('p', { class: 'meta' }, 'Der Lernhelfer ist gerade nicht erreichbar (angemeldet?). Wähle Allgemeinwissen oder ein eigenes Thema.'));
  });
}
function wahlZeigen() {
  if (!wahl) return;
  C.$('dialog').replaceChildren(C.zuKnopf(), wahl.wurzel);
  C.modal.auf('dialog');
}
// Der Server speichert freie Themen vereinheitlicht (klein, Leerraum zusammengefasst)
const gleich = (a, b) => String(a || '').toLowerCase().replace(/\s+/g, ' ').trim() === String(b || '').toLowerCase().replace(/\s+/g, ' ').trim();
function wahlStand() {
  if (!wahl || d?.phase !== 'lobby') return;
  const f = faecher(), ich = C.me()?.id, st = d.stimmen || [];
  const m = st.find(s => s.id === ich);
  if (m && !meine) meine = { art: m.art, fach: m.fach, thema: m.thema };
  for (const [b, v] of wahl.chipKnoepfe) b.setAttribute('aria-pressed', String(meine?.art === 'frei' && gleich(meine.thema, v.thema)));
  const g = meine?.thema ? (d.gen || []).find(x => gleich(x.thema, meine.thema)) : null;
  wahl.gen.textContent = g ? `KI schreibt deine Fragen: ${genText(g)}` : '';
  const name = s => wahl.chipKnoepfe.find(([, v]) => gleich(v.thema, s.thema))?.[1].titel || s.thema;
  wahl.stimmen.replaceChildren(...st.map(s => C.el('div', { class: 'zeile' }, wer(s.id)?.name || '?',
    C.el('b', {}, s.art === 'frei' ? name(s) : s.art === 'fach' ? f[s.fach] ?? '?' : 'Empfehlung'))));
}

/* ---------- Formeln: $…$ als MathML über KaTeX aus dem Lernhelfer (/lib), sonst Klartext ---------- */
let katexLaden = null;
function katexHolen() {
  return katexLaden ??= window.katex ? Promise.resolve(window.katex) : new Promise(ok => {
    const s = document.createElement('script');
    s.src = '/lib/katex-0.16.11.min.js'; s.onload = () => ok(window.katex || null); s.onerror = () => ok(null);
    document.head.append(s);
  });
}
/** Text mit $…$ als Knoten-Liste; Formeln als <span class="formel">. KaTeX maskiert die Eingabe selbst (trust: false). */
function mitFormeln(text) {
  const teile = String(text ?? '').split(/(\$[^$\n]{1,200}\$)/g).filter(Boolean);
  const formel = t => t.length > 2 && t[0] === '$' && t.at(-1) === '$';
  if (!teile.some(formel)) return [String(text ?? '')];
  if (!window.katex) { katexHolen().then(k => { if (k && d) zeichne(); }); return teile.map(t => formel(t) ? t.slice(1, -1) : t); }
  return teile.map(t => {
    if (!formel(t)) return t;
    const x = C.el('span', { class: 'formel' });
    x.innerHTML = window.katex.renderToString(t.slice(1, -1), { output: 'mathml', throwOnError: false, trust: false });
    return x;
  });
}

/* ---------- Live-Stand der KI-Fragen ---------- */
// n = Frage, die gerade entsteht (0 = KI denkt noch); Rest grob aus dem bisherigen Tempo, vorher eine Schätzung
function genText(g) {
  if (!g) return 'Gleich geht es los …';
  const von = g.von || 8, fertig = Math.max(0, g.n - 1), s = Math.round(g.ms / 1000);
  const rest = fertig ? g.ms / fertig * (von - fertig) : Math.max(5000, 25000 - g.ms);
  return `${g.n ? `Frage ${g.n}/${von} wird generiert` : 'KI denkt nach'} · ${s} s · noch ca. ${Math.max(1, Math.ceil(rest / 1000))} s`;
}
const genAnteil = g => g ? Math.min(100, Math.max(0, g.n - 1) / (g.von || 8) * 100) : 0;

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
      C.el('p', { class: 'duell-frage' }, j.thema || ''),
      C.el('p', { class: 'duell-gen', role: 'status' }, genText(j.gen)),
      C.el('div', { class: 'duell-zeit gen', role: 'presentation' }, C.el('i', { style: `width:${genAnteil(j.gen)}%` })));
  } else if (j.phase === 'frage' || j.phase === 'aufloesung') {
    const f = frage || {}, los = j.phase === 'aufloesung';
    t.push(C.el('div', { class: 'duell-kopf' }, C.el('b', {}, `Runde ${j.runde ?? '?'}/${f.von || 8}`), f.thema ? ` · ${f.thema} ` : ' ', los ? null : restText()),
      f.hinweis ? C.el('p', { class: 'duell-hinweis', role: 'status' }, f.hinweis) : null,
      C.el('p', { class: 'duell-frage gross' }, ...mitFormeln(f.frage)),
      C.el('div', { class: 'duell-antworten', role: 'group', 'aria-label': 'Antworten' }, FELD.map((b, i) => C.el('button', {
        class: `duell-feld ${KLASSE[i]}${i === gewaehlt ? ' gewaehlt' : ''}${los ? (i === j.richtig ? ' richtig' : ' falsch') : ''}`, type: 'button', disabled: los,
        'aria-pressed': String(i === gewaehlt), 'aria-label': `${b}: ${f.antworten?.[i] ?? ''}${los ? (i === j.richtig ? ', richtig' : ', falsch') : ''}`,
        onclick: () => antworte(i),
      }, C.el('b', {}, b), ' ', ...mitFormeln(f.antworten?.[i] ?? '')))));
    if (los) {
      const s = wer(j.schnellster);
      t.push(C.el('p', { class: 'duell-erklaerung', role: 'status' }, C.el('b', {}, `Richtig: ${FELD[j.richtig] ?? '?'}. `), ...mitFormeln(j.erklaerung),
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
