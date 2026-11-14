// Hilfeseite: Zahlen kommen aus den Spielregeln des Servers (eine Quelle), nicht aus dem Text.
const r = (await (await fetch('api/registry', { cache: 'no-store' })).json().catch(() => null))?.regeln;
if (r) {
  const werte = { ...r, ausbau1: r.ausbau[1], ausbau2: r.ausbau[2], stufe1: r.ausbauStufe[1], stufe2: r.ausbauStufe[2] };
  for (const e of document.querySelectorAll('[data-r]')) if (werte[e.dataset.r] != null) e.textContent = Number(werte[e.dataset.r]).toLocaleString('de-AT');
}
