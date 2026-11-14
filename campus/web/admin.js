// Verwaltung: Meldungen ansehen, Bau mit einem Klick zurücksetzen, Konto sperren, auffälligen Handel prüfen.
const $ = id => document.getElementById(id);
function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) if (k.startsWith('on')) e[k] = v; else if (v != null && v !== false) e.setAttribute(k, v === true ? '' : v);
  e.append(...kids.flat().filter(k => k != null && k !== false));
  return e;
}
async function api(op, body) {
  const r = await fetch('api/' + op, body === undefined ? { cache: 'no-store' } : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  const j = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(j.error || `Der Server antwortet ${r.status}.`);
  return j;
}
const zeit = t => new Date(t).toLocaleString('de-AT', { dateStyle: 'short', timeStyle: 'short' });
const knopf = (text, op, body, frage, voll) => el('button', { type: 'button', class: voll ? 'voll' : null, onclick: async () => {
  if (frage && !confirm(frage)) return;
  try { const r = await api(op, body); $('status').textContent = r.name ? `Erledigt: ${r.name}.` : 'Erledigt.'; await laden(); } catch (e) { $('status').textContent = e.message; }
} }, text);
const liste = (id, zeilen, leer) => $(id).replaceChildren(...zeilen.length ? zeilen : [el('p', { class: 'meta' }, leer)]);

async function laden() {
  const d = await api('admin/overview');
  $('takt').textContent = `${d.online} online · Server-Takt: zuletzt ${d.takt.letzter_ms.toFixed(2)} ms, Mittel ${d.takt.mittel_ms.toFixed(2)} ms, höchstens ${d.takt.max_ms.toFixed(2)} ms (Ziel unter 50 ms)`;
  liste('meldungen', d.meldungen.map(m => el('div', { class: 'zeile' },
    el('span', {}, el('b', {}, m.grund), ` · gemeldet von ${m.von} am ${zeit(m.zeit)}`, el('br'), m.plot ? `Bau auf dem Grundstück von ${m.plotName ?? '?'}${m.ort ? ` (bei ${m.ort[0]}, ${m.ort[1]})` : ''}` : `Spieler ${m.name}`),
    el('span', {}, m.plot ? knopf('Bau zurücksetzen', 'admin/rollback', { plot: m.plot }, 'Den Bau auf diesem Grundstück wirklich auf den Urzustand zurücksetzen? Das lässt sich nicht rückgängig machen.', true) : knopf('Konto sperren', 'admin/ban', { user: m.user, an: true }, `${m.name} wirklich für Campus sperren?`, true),
      ' ', knopf('Ohne Folgen erledigt', 'admin/done', { art: 'meldung', id: m.id })))), 'Keine offenen Meldungen.');
  liste('auffaellig', d.auffaellig.map(f => el('div', { class: 'zeile' },
    el('span', {}, el('b', {}, f.art === 'preis' ? 'Überhöhter Preis' : 'Einseitiger Geldfluss'), ` · ${f.kaeufer} → ${f.verkaeufer} · ${zeit(f.zeit)}`, el('br'), f.text),
    el('span', {}, knopf('Geprüft', 'admin/done', { art: 'auffaellig', id: f.id })))), 'Nichts Auffälliges.');
  liste('grundstuecke', d.grundstuecke.map(p => el('div', { class: 'zeile' }, el('span', {}, `${p.gruppe ? 'Lerngruppe ' : ''}${p.name} · bei ${p.ort[0]}, ${p.ort[1]} · ${p.ort[2]} × ${p.ort[2]}`),
    knopf('Bau zurücksetzen', 'admin/rollback', { plot: p.id }, `Den Bau von ${p.name} wirklich auf den Urzustand zurücksetzen?`))), 'Noch keine Grundstücke.');
  liste('konten', d.konten.map(k => el('div', { class: 'zeile' }, el('span', {}, el('b', {}, k.name), ` · ${k.taler.toLocaleString('de-AT')} Taler · Stufe ${k.stufe}${k.online ? ' · online' : ''}${k.gesperrt ? ' · gesperrt' : ''}`),
    k.gesperrt ? knopf('Sperre aufheben', 'admin/ban', { user: k.id, an: false }) : knopf('Für Campus sperren', 'admin/ban', { user: k.id, an: true }, `${k.name} wirklich für Campus sperren?`))), 'Noch keine Konten.');
}
laden().catch(e => { $('status').textContent = e.message; });

// Chat stummschalten: Minuten 0 hebt die Stummschaltung auf.
const stummKnopf = $('stumm-knopf');
stummKnopf.onclick = async () => {
  const user = +$('stumm-user').value | 0;
  const minuten = Math.max(0, +$('stumm-minuten').value | 0);
  if (!user) { $('stumm-status').textContent = 'Bitte eine Konto-ID eintragen.'; return; }
  stummKnopf.disabled = true;
  try {
    const r = await api('admin/mute', { user, minuten });
    $('stumm-status').textContent = minuten === 0 ? 'Aufgehoben.' : `Stumm bis ${zeit(r.bis ?? Date.now() + minuten * 60000)}.`;
  } catch (e) { $('stumm-status').textContent = e.message; }
  finally { stummKnopf.disabled = false; }
};
