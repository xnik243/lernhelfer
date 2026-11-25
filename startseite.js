// Alles Gezeichnete entsteht aus dem echten Layout: Schriftzug, Kästen, Pfeil, Häkchen, Textmarker.
// Kein Font für die Handschrift; die Striche sind Pfade mit kleinen, festen Zufallsabweichungen.
if (location.hash && location.hash !== "#wer") location.replace("app" + location.search + location.hash); // alte Links auf /#… (Mails, Lesezeichen) gehören in die App
const blatt = document.querySelector(".blatt");
const schrift = document.querySelector(".schrift");
const striche = document.querySelector(".striche");
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

function rng(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const n1 = (v) => v.toFixed(1);

// Großbuchstaben als Einzelstriche, Höhe 100, Grundlinie bei 100
const ZEICHEN = {
  L: [50, ["M6 2Q4 52 7 99", "M6 98Q26 95 50 99"]],
  E: [50, ["M7 3Q5 52 7 98", "M8 4Q28 1 48 5", "M8 50Q24 47 41 50", "M7 97Q28 99 50 95"]],
  R: [54, ["M7 99Q5 52 8 3", "M8 4C38-1 56 12 51 29C47 45 28 50 9 49", "M25 50Q38 72 54 99"]],
  N: [58, ["M6 99Q5 52 7 4", "M7 4Q30 52 52 98", "M52 98Q54 52 51 2"]],
  H: [56, ["M6 2Q4 52 7 99", "M51 3Q53 52 50 99", "M7 51Q29 47 51 52"]],
  F: [46, ["M7 99Q5 52 8 3", "M8 4Q28 1 46 6", "M8 51Q23 48 38 51"]],
};

let zeit = 200; // Zeitpunkt des nächsten Strichs im Auftakt, in ms
const zug = (d, cls, dauer, extra = "") => {
  const s = `<path class="zug ${cls}" pathLength="1" d="${d}" data-v="${zeit}" data-t="${dauer}"${extra}/>`;
  zeit += dauer * 0.8;
  return s;
};

function schriftzug() {
  const r = rng(7);
  let x = 6;
  let out = "";
  for (const c of "LERNHELFER") {
    const [w, ps] = ZEICHEN[c];
    const rot = (r() - 0.5) * 5;
    const dy = (r() - 0.5) * 6;
    out += `<g transform="translate(${n1(x)} ${n1(10 + dy)}) rotate(${n1(rot)} ${w / 2} 50)">${ps.map((p) => zug(p, "", 70)).join("")}</g>`;
    x += w + 17;
  }
  schrift.setAttribute("viewBox", `0 0 ${Math.ceil(x)} 124`);
  schrift.innerHTML = `<defs><filter id="rau" x="-5%" y="-5%" width="110%" height="110%"><feTurbulence type="fractalNoise" baseFrequency=".8" numOctaves="1" seed="3"/><feDisplacementMap in="SourceGraphic" scale="2.4"/></filter></defs><g filter="url(#rau)">${out}</g>`;
  document.body.classList.add("gezeichnet");
}

// Lage eines Elements im Blatt, ohne laufende Transformationen der Einblendung
function lage(el) {
  let x = 0;
  let y = 0;
  for (let e = el; e && e !== blatt; e = e.offsetParent) {
    x += e.offsetLeft;
    y += e.offsetTop;
  }
  return { x, y, w: el.offsetWidth, h: el.offsetHeight };
}

// Eine Linie wie mit der Hand: Enden leicht versetzt und überstehend, Mitte leicht gebogen
function hand(r, x1, y1, x2, y2, j = 1.6, ueber = 4) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;
  const a = [x1 - ux * ueber * r() + (r() - 0.5) * j, y1 - uy * ueber * r() + (r() - 0.5) * j];
  const b = [x2 + ux * ueber * r() + (r() - 0.5) * j, y2 + uy * ueber * r() + (r() - 0.5) * j];
  const bog = (r() - 0.5) * Math.min(6, len * 0.03);
  const m = [(a[0] + b[0]) / 2 - uy * bog, (a[1] + b[1]) / 2 + ux * bog];
  return `M${n1(a[0])} ${n1(a[1])}Q${n1(m[0])} ${n1(m[1])} ${n1(b[0])} ${n1(b[1])}`;
}
function kasten(r, b, cls, dauer, pad = 0) {
  const x = b.x - pad;
  const y = b.y - pad;
  const w = b.w + 2 * pad;
  const h = b.h + 2 * pad;
  return [
    [x, y, x + w, y],
    [x + w, y, x + w, y + h],
    [x + w, y + h, x, y + h],
    [x, y + h, x, y],
  ]
    .map(([a, c, d, e]) => zug(hand(r, a, c, d, e), cls, dauer))
    .join("");
}

let start = 0; // Ende des Schriftzugs: dort beginnt die Skizze
function zeichnen() {
  const r = rng(11);
  zeit = start;
  const W = blatt.offsetWidth;
  const H = blatt.offsetHeight;
  let g = "";

  // Unterstrich unter dem Schriftzug und Textmarker auf „das Wichtigste“
  const s = lage(document.querySelector(".wort"));
  g += zug(`M${n1(s.x + 4)} ${n1(s.y + s.h + 6)}C${n1(s.x + s.w * 0.3)} ${n1(s.y + s.h + 1)} ${n1(s.x + s.w * 0.6)} ${n1(s.y + s.h + 10)} ${n1(s.x + s.w * 0.86)} ${n1(s.y + s.h + 2)}`, "blau", 420);
  const wi = lage(document.querySelector(".wichtig"));
  const fs = parseFloat(getComputedStyle(document.querySelector(".claim")).fontSize);
  g += zug(hand(r, wi.x - 3, wi.y + wi.h * 0.6, wi.x + wi.w + 3, wi.y + wi.h * 0.56, 1, 2), "marker", 420, ` stroke-width="${n1(fs * 0.95)}"`);

  // Quelle als kleiner Stapel Blätter, leicht schräg wie das Element selbst
  const qe = document.querySelector(".quelle");
  const q = lage(qe);
  const qc = [q.x + q.w / 2, q.y + q.h / 2];
  g += `<g transform="rotate(-1.2 ${n1(qc[0])} ${n1(qc[1])})">${kasten(r, { x: q.x + 13, y: q.y - 13, w: q.w, h: q.h }, "tinte", 90)}${kasten(r, { x: q.x + 6.5, y: q.y - 6.5, w: q.w, h: q.h }, "tinte", 90)}${kasten(r, q, "tinte", 120)}</g>`;

  // Pfeil von der Quelle zur Woche: nebeneinander quer, untereinander nach unten
  const z = lage(document.querySelector(".ziel"));
  const neben = z.x > q.x + q.w;
  let pfeil;
  let ende;
  let richtung;
  if (neben) {
    const a = [q.x + q.w + 22, q.y + q.h * 0.42];
    ende = [z.x - 18, z.y + 30];
    const mx = (a[0] + ende[0]) / 2;
    pfeil = `M${n1(a[0])} ${n1(a[1])}C${n1(mx - 20)} ${n1(a[1] - 60)} ${n1(mx + 30)} ${n1(ende[1] - 50)} ${n1(ende[0])} ${n1(ende[1])}`;
    richtung = Math.atan2(50, ende[0] - (mx + 30));
  } else {
    const a = [q.x + q.w * 0.35, q.y + q.h + 20];
    ende = [z.x + 54, z.y - 14];
    pfeil = `M${n1(a[0])} ${n1(a[1])}C${n1(a[0] - 26)} ${n1(a[1] + 26)} ${n1(ende[0] - 40)} ${n1(ende[1] - 34)} ${n1(ende[0])} ${n1(ende[1])}`;
    richtung = Math.atan2(34, 40);
  }
  g += zug(pfeil, "blau", 520);
  for (const seite of [-1, 1]) {
    const w = richtung + Math.PI + seite * 0.5;
    g += zug(`M${n1(ende[0])} ${n1(ende[1])}L${n1(ende[0] + Math.cos(w) * 14)} ${n1(ende[1] + Math.sin(w) * 14)}`, "blau", 110);
  }

  // Woche: Kasten, dann jedes Kästchen mit Häkchen
  g += kasten(r, z, "tinte", 130);
  for (const li of document.querySelectorAll(".check li")) {
    const l = lage(li);
    const k = { x: l.x + 4, y: l.y + 9, w: 16, h: 16 }; // an der ersten Zeile, auch wenn der Punkt umbricht
    g += kasten(r, k, "tinte", 45);
    const hx = k.x + 3;
    const hy = k.y + 9;
    g += zug(`M${n1(hx)} ${n1(hy)}Q${n1(hx + 3)} ${n1(hy + 2)} ${n1(hx + 5.5)} ${n1(hy + 6.5)}Q${n1(hx + 9)} ${n1(hy - 4)} ${n1(hx + 17)} ${n1(hy - 13)}`, "blau", 140);
  }

  // Anmelden: Textmarker als Fläche, darüber ein gezeichneter Rahmen
  const go = lage(document.querySelector(".go"));
  g += zug(hand(r, go.x + 4, go.y + go.h / 2, go.x + go.w - 4, go.y + go.h / 2 + 1, 1, 0), "marker", 380, ` stroke-width="${n1(go.h - 10)}"`);
  g += kasten(r, go, "tinte", 110, 2);

  // Wer dahintersteckt: Klebestreifen aufs Foto, Marker auf „Lass es prüfen“, Kasten ums Prüfen, Rahmen um die KI-Knöpfe
  const fo = document.querySelector(".ich:not(.ohne-foto) .foto");
  if (fo) {
    const f = lage(fo);
    const fm = f.x + f.w / 2;
    g += `<g transform="rotate(-2 ${n1(fm)} ${n1(f.y + f.h / 2)})">${zug(hand(r, fm - 42, f.y + 4, fm + 40, f.y - 5, 1, 0), "marker", 160, ' stroke-width="24"')}</g>`;
  }
  const mk = lage(document.querySelector(".markiert"));
  const mfs = parseFloat(getComputedStyle(document.querySelector(".pruefen h3")).fontSize);
  g += zug(hand(r, mk.x - 3, mk.y + mk.h * 0.6, mk.x + mk.w + 3, mk.y + mk.h * 0.55, 1, 2), "marker", 300, ` stroke-width="${n1(mfs * 0.95)}"`);
  g += kasten(r, lage(document.querySelector(".pruefen")), "blau", 120);
  for (const pk of document.querySelectorAll(".pk")) g += kasten(r, lage(pk), "tinte", 70, 1);

  striche.setAttribute("viewBox", `0 0 ${W} ${H}`);
  striche.innerHTML = g;
  for (const el of document.querySelectorAll(".zug")) {
    el.style.setProperty("--v", `${el.dataset.v}ms`);
    el.style.setProperty("--t", `${el.dataset.t}ms`);
  }
}

// Prüfen lassen: ChatGPT bzw. Claude öffnen gleich mit einer kritischen Frage zum öffentlichen Code (gleicher Text wie pruefFrage() in index.html)
const datei = (n) => new URL(n, location.href).href;
const h = location.host;
const frage = `Ein Mitstudent an meiner Uni hat eine Lern-App gebaut („Lernhelfer”, ${h}) und dazu ein kleines Open-Source-Programm, das auf meinem PC oder Mac läuft. Es holt meine Kursunterlagen aus OLAT (dem Lernportal unserer Uni) und lädt ihren Text in mein Konto beim Lernhelfer. Der ganze Code ist öffentlich, bitte lies ihn dir durch:
- ${datei("einrichten.ps1")} richtet es am Windows-PC ein (PowerShell)
- ${datei("einrichten-mac.sh")} richtet es am Mac ein (Bash)
- ${datei("holen.py")} ist das eigentliche Programm (Python, ca. 1800 Zeilen)

Am PC startet eine kurze Datei aus der App das Einrichten: Sie setzt einen persönlichen Schlüssel für meinen Speicher, lädt einrichten.ps1 von dieser Adresse und führt es aus. Am Mac geht dasselbe über einen Terminal-Befehl. Die Stellen im Code, die GitHub nennen, gehören zu einer selbst betriebenen Variante ohne Server; bei ${h} gehen die Daten an diesen Server.

Ich will sichergehen, dass ich dem Programm vertrauen kann. Kannst du mir bestätigen, dass der Code folgende Punkte einhält?
1. Es ist keine Schadsoftware: kein Fernzugriff, keine Tastatur-Aufzeichnung, es liest keine fremden Dateien auf meinem Computer.
2. Mein Uni-Passwort wird, wenn ich den automatischen Login einschalte, mit der Betriebssystem-Verschlüsselung (Windows DPAPI bzw. macOS-Schlüsselbund) gespeichert und nur an die Uni-Anmeldeseite geschickt.
3. Es braucht keine Administratorrechte und lässt sich durch Löschen eines einzigen Ordners vollständig entfernen.

Geh den Code durch und sag mir, ob diese Punkte stimmen. Würdest du mir als Informatik-Laie empfehlen, es zu installieren? Falls du die Links nicht öffnen kannst, sag es, dann lade ich die Dateien hoch.`;
for (const a of document.querySelectorAll(".pk")) {
  a.href = a.dataset.ki === "claude" ? `https://claude.ai/new?q=${encodeURIComponent(frage)}` : `https://chatgpt.com/?hints=search&q=${encodeURIComponent(frage)}`;
}
document.querySelector("#frage-kopieren").addEventListener("click", async (e) => {
  const knopf = e.currentTarget;
  try {
    await navigator.clipboard.writeText(frage);
    knopf.textContent = "kopiert";
  } catch {
    knopf.textContent = "ging nicht, bitte über ChatGPT fragen";
  }
});

// Ohne Foto (ich.jpg fehlt) fällt der Rahmen weg und der Text steht allein
const ich = document.querySelector(".ich");
const bild = ich.querySelector(".foto img");
if (bild.complete && !bild.naturalWidth) ich.classList.add("ohne-foto");
else bild.addEventListener("error", () => { ich.classList.add("ohne-foto"); zeichnen(); });

schriftzug();
start = zeit;
zeichnen();
// Auftakt nur beim Laden; ein Neuzeichnen in dieser Zeit beginnt die Striche einfach neu
if (!reduce) {
  document.body.classList.add("intro");
  setTimeout(() => document.body.classList.remove("intro"), 7000);
}
let letzteBreite = blatt.offsetWidth;
new ResizeObserver(() => {
  if (blatt.offsetWidth === letzteBreite) return;
  letzteBreite = blatt.offsetWidth;
  zeichnen();
}).observe(blatt);
document.fonts?.ready.then(zeichnen); // erst mit geladener Schrift stimmen die Maße
