// Campus – Architekten-Modus (Phase 22): Leiste mit Auswahl und //-Befehlen, Bauplan-Export, NPC-Formular, NPC-Gespräche.
// Alles prüft der Server (nur Verwaltung); hier wird nur angezeigt und gesendet (CONTRACT-21-25 §S-ARCH, §W-SPIEL).
let C = null;                       // ctx aus hud.js
const namen = new Map();            // 200+n → Name der freien NPCs
let an = false;
let block = 'STEIN';                // zuletzt eingegebener Block (Eingabe bleibt beim Neuzeichnen stehen)

const knopf = (text, f, klasse = 'arch-knopf', titel) => C.el('button', { class: klasse, type: 'button', onclick: f, title: titel || null }, text);
const sag = text => { try { C.game()?.say(0, text); } catch (e) { C.fehler(e); } };
const pos = p => Array.isArray(p) && p.length === 3 ? p.map(n => Number(n) | 0) : null;
const dialog = (...kinder) => { C.$('dialog').replaceChildren(C.zuKnopf(), ...kinder); C.modal.auf('dialog'); };

export function initArchitekt(ctx) { C = ctx; }

/** Kern-Ereignis `npcs`: [[200+n, Name], …]. */
export function npcsSetzen(liste) {
  namen.clear();
  for (const e of Array.isArray(liste) ? liste : []) {
    const id = Number(e?.[0]);
    if (Number.isInteger(id) && id >= 200 && id < 256) namen.set(id, String(e?.[1] ?? '').slice(0, 24));
  }
}
export function npcName(id) { return namen.get(Number(id)); }

export function architektHud(j) {
  if (!C || !j) return;
  if (j.t === 'architekt') return leiste(j);
  if (j.t === 'architekt_export') return exportZeigen(String(j.name || ''));
  if (j.t === 'architekt_npc') return npcFormular(j);
  if (j.t === 'npc') return npcGespraech(j);
}

/* ---------- Leiste ---------- */
function leiste(j) {
  const l = C.$('arch-leiste'), g = C.game();
  an = !!j.an;
  try { g?.set_architect?.(an); } catch {}
  const p1 = pos(j.pos1), p2 = pos(j.pos2);
  try {
    if (an && p1 && p2) g?.set_selection?.(true, p1[0], p1[1], p1[2], p2[0], p2[1], p2[2]);
    else g?.set_selection?.(false, 0, 0, 0, 0, 0, 0);
  } catch {}
  if (!l) return;
  if (!an) { l.hidden = true; l.replaceChildren(); return; }
  const m = pos(j.masse), txt = p => p ? p.join(', ') : '–';
  const eingabe = C.el('input', { type: 'text', value: block, maxlength: 24, size: 9, 'aria-label': 'Block (Nummer oder Name)', autocomplete: 'off', spellcheck: 'false' });
  eingabe.oninput = () => { block = eingabe.value.trim(); };
  eingabe.onkeydown = e => e.stopPropagation();          // WASD im Feld nicht an die Steuerung weitergeben
  const mit = befehl => () => { const b = eingabe.value.trim(); if (!b) return C.toast('Gib zuerst einen Block an.', 'schlecht'); block = b; sag(`//${befehl} ${b}`); };
  l.replaceChildren(
    C.el('b', {}, 'Architekt'),
    C.el('span', { class: 'arch-pos' }, 'Pos1 ', C.el('b', {}, txt(p1))),
    C.el('span', { class: 'arch-pos' }, 'Pos2 ', C.el('b', {}, txt(p2))),
    C.el('span', { class: 'arch-mass' }, 'Maße ', C.el('b', {}, m ? `${m[0]} × ${m[1]} × ${m[2]}` : '–')),
    C.el('span', { class: 'arch-mass' }, 'Blöcke ', C.el('b', {}, Number(j.bloecke || 0).toLocaleString('de-AT'))),
    knopf('Pos1', () => sag('//pos1'), 'arch-knopf', 'Pos1 an deine Füße'),
    knopf('Pos2', () => sag('//pos2'), 'arch-knopf', 'Pos2 an deine Füße'),
    knopf('Abwählen', () => sag('//desel')),
    eingabe,
    knopf('Füllen', mit('set'), 'arch-knopf', '//set'),
    knopf('Wände', mit('walls'), 'arch-knopf', '//walls'),
    knopf('Kopieren', () => sag('//copy')),
    knopf('Drehen', () => sag('//rotate'), 'arch-knopf', '90° um die Hochachse'),
    knopf('Einfügen', () => sag('//paste'), 'arch-knopf', j.ablage ? `Ablage ${pos(j.ablage)?.join(' × ') || ''}` : 'Ablage leer'),
    C.el('button', { class: 'arch-knopf', type: 'button', disabled: !(j.undo > 0), onclick: () => sag('//undo') }, `Rückgängig (${j.undo | 0})`),
    C.el('button', { class: 'arch-knopf', type: 'button', disabled: !(j.redo > 0), onclick: () => sag('//redo') }, `Wiederholen (${j.redo | 0})`),
    knopf('Exportieren', () => {
      const n = (prompt('Name des Bauplans (a–z, 0–9, _ und -, höchstens 32 Zeichen):') || '').trim().toLowerCase();
      if (!n) return;
      if (!/^[a-z0-9_-]{1,32}$/.test(n)) return C.toast('Erlaubt sind a–z, 0–9, _ und -, höchstens 32 Zeichen.', 'schlecht');
      sag(`//export ${n}`);
    }),
    knopf('NPCs', () => sag('/architect npc')),
    knopf('Beenden', () => sag('/architect aus')),
    j.text ? C.el('span', { class: 'meta', role: 'status' }, String(j.text)) : null);
  l.hidden = false;
}

/* ---------- Bauplan-Export ---------- */
async function exportZeigen(name) {
  if (!name) return;
  let r;
  try { r = await C.api(`admin/blueprint/${encodeURIComponent(name)}`); } catch (e) { return C.fehler(e); }
  const feld = C.el('textarea', { class: 'arch-export', readonly: true, rows: 12, spellcheck: 'false', 'aria-label': 'Rust-Blueprint' });
  feld.value = String(r.rust || '');
  const status = C.el('p', { class: 'meta', role: 'status' }, `Gespeichert als „${r.name || name}“ (${r.w | 0} × ${r.h | 0} × ${r.d | 0}).`);
  dialog(C.el('h3', {}, C.el('span', {}, 'Architekt'), 'Bauplan exportiert'), feld, status,
    C.el('div', { class: 'reihe' }, C.el('button', { class: 'knopf voll', type: 'button', onclick: async () => {
      try { await navigator.clipboard.writeText(feld.value); status.textContent = 'In die Zwischenablage kopiert.'; C.ton(13); }
      catch { feld.select(); status.textContent = 'Kopieren nicht erlaubt – Text ist markiert, bitte mit Strg+C kopieren.'; }
    } }, 'Als Rust-Blueprint kopieren')));
}

/* ---------- NPC-Formular ---------- */
function npcFormular(j) {
  const npcs = Array.isArray(j.npcs) ? j.npcs : [], vorlagen = Array.isArray(j.vorlagen) ? j.vorlagen : [];
  const senden = async (body, text) => {
    try { await C.api('admin/npc', body); C.toast(text, 'gut'); sag('/architect npc'); } catch (e) { C.fehler(e); }
  };
  const formular = (n) => {
    const name = C.el('input', { type: 'text', maxlength: 24, required: true, value: n?.name || '', autocomplete: 'off' });
    const look = C.el('select', {}, (vorlagen.length ? vorlagen : Array.from({ length: 8 }, (_, i) => `Vorlage ${i + 1}`)).slice(0, 8)
      .map((v, i) => C.el('option', { value: i, selected: (n?.look | 0) === i }, String(v))));
    const text = C.el('textarea', { maxlength: 160 }); text.value = n?.text || '';
    const aktion = C.el('input', { type: 'text', maxlength: 32, value: n?.aktion || '', placeholder: 'leer, warp:4, taler:25 oder item:1110:2', autocomplete: 'off' });
    const hier = n ? C.el('input', { type: 'checkbox' }) : null;
    for (const f of [name, text, aktion]) f.onkeydown = e => e.stopPropagation();
    const form = C.el('form', { class: 'npc-form', onsubmit: e => {
      e.preventDefault();
      const body = { name: name.value.trim(), look: Number(look.value) | 0, text: text.value.trim(), aktion: aktion.value.trim() };
      if (!body.name) return C.toast('Der NPC braucht einen Namen.', 'schlecht');
      if (n) senden({ ...body, id: n.id, hier: !!hier?.checked }, `${body.name} gespeichert.`);
      else senden(body, `${body.name} steht jetzt an deiner Stelle.`);
    } },
      C.el('label', {}, 'Name', name), C.el('label', {}, 'Aussehen', look), C.el('label', {}, 'Text', text),
      C.el('label', {}, 'Aktion', aktion),
      hier ? C.el('label', {}, C.el('span', {}, hier, ' an meine Stelle versetzen')) : null,
      C.el('div', { class: 'reihe' }, C.el('button', { class: 'knopf voll', type: 'submit' }, n ? 'Speichern' : 'Hier aufstellen'),
        n ? C.el('button', { class: 'knopf', type: 'button', onclick: () => {
          if (confirm(`${n.name} wirklich löschen?`)) senden({ id: n.id, loeschen: true }, `${n.name} gelöscht.`);
        } }, 'Löschen') : null));
    return form;
  };
  dialog(C.el('h3', {}, C.el('span', {}, 'Architekt'), 'NPCs'),
    C.el('h3', {}, 'Neuer NPC'), formular(null),
    C.el('h3', {}, `Aufgestellt (${npcs.length})`),
    npcs.length ? npcs.map(n => C.el('details', {}, C.el('summary', {}, `${n.name} · ${[n.x, n.y, n.z].map(v => Math.round(Number(v) || 0)).join(', ')}`), formular(n)))
      : C.el('p', { class: 'meta' }, 'Noch keine NPCs aufgestellt.'));
}

/* ---------- Gespräch mit einem freien NPC ---------- */
function npcGespraech(j) {
  const name = String(j.name || namen.get(Number(j.id)) || 'Unbekannt');
  const lohn = { taler: 'Du hast eine Belohnung in Talern erhalten.', item: 'Du hast einen Gegenstand erhalten.' };
  const zusatz = j.belohnt && lohn[j.aktion] ? lohn[j.aktion]
    : (j.aktion === 'taler' || j.aktion === 'item') ? 'Deine Belohnung hast du heute schon bekommen.'
      : j.aktion === 'warp' ? 'Du wirst gleich weitergeschickt …' : null;
  dialog(C.el('h3', {}, C.el('span', {}, 'Gespräch'), name),
    C.el('p', {}, String(j.text || '')),
    zusatz ? C.el('p', { class: 'meta' }, zusatz) : null,
    C.el('div', { class: 'reihe' }, C.el('button', { class: 'knopf voll', type: 'button', onclick: () => C.modal.zu() }, 'Tschüss')));
  C.ton(j.belohnt ? 7 : 13, 0.7);
}
