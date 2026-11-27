---
name: Lernhelfer
description: Ein Lernheft auf Punktraster, mit Fineliner gezeichnet, Textmarker für das, was gerade dran ist.
colors:
  papier: "#fbfbf9"
  papier-2: "#f2f2ee"
  papier-3: "#e6e6e0"
  tinte: "#121212"
  tinte-2: "#4f535a"
  tinte-3: "#666a71"
  linie: "rgb(18 18 18 / 0.15)"
  linie-2: "rgb(18 18 18 / 0.45)"
  punktraster: "rgb(24 32 56 / 0.17)"
  koenigsblau: "#2446c8"
  koenigsblau-tief: "#1b389f"
  textmarker: "#ffe93a"
  zettel: "#fff2a8"
  ok: "#146c43"
  warn: "#8f4700"
  fehler: "#b3261e"
  fach-mathe: "#0f766e"
  fach-vwl: "#b45309"
  fach-bwl: "#7c3aed"
  fach-englisch: "#be185d"
  graphit: "#151515"
  graphit-2: "#1c1c1c"
  graphit-3: "#262626"
  elfenbein: "#e9e6df"
  elfenbein-2: "#a9a59d"
  champagner: "#cbb894"
typography:
  seitentitel:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "2.25rem"
    fontWeight: 800
    lineHeight: 1.15
    letterSpacing: "-0.01em"
    fontVariation: "\"wdth\" 86"
  ueberschrift:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 750
    lineHeight: 1.3
    letterSpacing: "-0.005em"
    fontVariation: "\"wdth\" 92"
  schlagzeile:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "1.75rem"
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: "-0.01em"
    fontVariation: "\"wdth\" 94"
  lesetext:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "1.0625rem"
    fontWeight: 400
    lineHeight: 1.65
  bedienung:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 450
    lineHeight: 1.5
  meta:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "\"tnum\""
  etikett:
    fontFamily: "Archivo, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 600
    lineHeight: 1.3
    fontFeature: "\"tnum\""
  formel:
    fontFamily: "CMU Serif, Georgia, serif"
    fontSize: "1.0625rem"
    fontWeight: 400
    lineHeight: 1.65
rounded:
  hand-s: "3px 5px 4px 6px / 5px 3px 6px 4px"
  hand: "4px 7px 5px 8px / 7px 4px 8px 5px"
  hand-xl: "6px 9px 7px 10px / 9px 6px 10px 7px"
  pille: "999px"
spacing:
  rand-handy: "16px"
  rand-desktop: "clamp(24px, 4vw, 56px)"
  raster: "22px"
  block: "20px"
  teil: "48px"
  lesemass: "68ch"
  inhalt-max: "1180px"
components:
  knopf:
    backgroundColor: "{colors.papier}"
    textColor: "{colors.tinte}"
    rounded: "{rounded.hand}"
    padding: "8px 14px"
    height: "40px"
  knopf-voll:
    backgroundColor: "{colors.papier}"
    textColor: "{colors.tinte}"
    rounded: "{rounded.hand}"
    padding: "8px 14px"
    height: "40px"
  navi-aktiv:
    backgroundColor: "{colors.textmarker}"
    textColor: "{colors.tinte}"
  pille:
    backgroundColor: "{colors.papier-3}"
    textColor: "{colors.tinte-2}"
    typography: "{typography.etikett}"
    rounded: "{rounded.pille}"
    padding: "1px 8px"
  eingabe:
    backgroundColor: "{colors.papier}"
    textColor: "{colors.tinte}"
    rounded: "{rounded.hand}"
    padding: "10px 12px"
    height: "44px"
---

# Design System: Lernhelfer

Quelle der Wahrheit ist `skizze.css` (liegt als Ebene über den Grundregeln im `<style>` von `index.html`). Diese Datei beschreibt den Stand vom 27.11.2026. Die frühere „lachsrosa Wirtschaftszeitung“ und die „ruhige App“ sind abgelöst.

## Overview

**Creative North Star: „Skizze auf Punktraster“**

Der Lernhelfer sieht aus wie ein gutes Lernheft: helles Papier mit feinem Punktraster, Kästen mit Fineliner gezogen, deren Striche an den Ecken leicht überstehen, und ein gelber Textmarker für das, was gerade dran ist. Alles wirkt von Hand gemacht, aber nie verspielt. Die Ecken sind ungleich gerundet wie bei einem echten Strich, ohne Comic-Wackeln. Die App soll sich anfühlen wie die eigenen Notizen, nur aufgeräumter.

Dichte: kompakt und lesbar. Am Handy normale Schriftgrößen und dichte Abstände, nur die Tippflächen sind groß. Gelesen wird in einer Spalte mit höchstens 68 Zeichen pro Zeile. Ein einziger Handlungs-Akzent (Königsblau), Gelb nur als Textmarker, Fachfarben nur zum Wiedererkennen.

Dunkel ist kein umgefärbtes Hell, sondern ein eigenes Heft: Graphit, Elfenbein und Champagner. Ruhig, nichts leuchtet. Blau und Gelb gibt es dort nicht.

**Key Characteristics:**
- Punktraster (22 px) als Grund auf jeder Seite.
- Gezeichnete Rahmen (SVG, 9-teilig gedehnt) statt CSS-Linien bei Knöpfen und antippbaren Karten.
- Gelber Textmarker für „hier bist du“ (aktive Seite, aktiver Tab, Auswahl).
- Königsblauer Unterstrich unter jedem Seitentitel.
- Archivo für alles, mit der Breiten-Achse für Titel; CMU Serif nur für Formeln.

## Colors

Weißes Papier, schwarze Tinte, ein Blau, ein Gelb. Alles andere ist Grau oder Fachfarbe.

### Primary
- **Königsblau** (koenigsblau): Handlung und Zustand. Rahmen des Hauptknopfs („Neu erstellen“, „Erklär's mir“), Unterstrich unter Seitentiteln, Links, gesetzte Häkchen, Fokusring, Eingabe-Cursor. Hover: Königsblau tief.

### Secondary
- **Textmarker-Gelb** (textmarker): Nur als gezeichneter Markerstrich, nie als Fläche oder Schriftfarbe. Aktiver Eintrag in Seitenleiste und Tab-Leiste, gewählte Option, Textauswahl, „Das Wichtigste“-Kasten (als Zettel-Gelb).

### Tertiary
- **Fachfarben** (fach-mathe, fach-vwl, fach-bwl, fach-englisch): Nur zum Wiedererkennen: Punkt in der Seitenleiste, Fach-Symbol, kleine Marken. Nie für Knöpfe, Flächen oder Text.

### Neutral
- **Papier** (papier, papier-2, papier-3): Seitengrund, ruhige Flächen (Heute-Woche, Hinweise), Pillen.
- **Tinte** (tinte, tinte-2, tinte-3): Text in drei Stufen: Inhalt, Nebentext, Meta.
- **Linie** (linie, linie-2): Ruhige 1-px-Linien (linie) und kräftigere Ränder von Feldern und Häkchen (linie-2).
- **Status** (ok, warn, fehler): Nur für Meldungen und Fristen, jeweils mit eigener heller Fläche.

### Dunkel
- **Graphit** (graphit, graphit-2, graphit-3) als Grund und Flächen, **Elfenbein** (elfenbein, elfenbein-2) als Schrift, **Champagner** (champagner) ersetzt Blau und Gelb: Rahmen des Hauptknopfs, Unterstrich, Marker (bei 20 % Deckkraft). Punktraster und Striche nur angedeutet (5–22 % Deckkraft).

**The One Blue Rule.** Königsblau heißt „hier passiert etwas“. Pro Bildschirm gibt es genau einen blau gerahmten Hauptknopf. Ist alles blau, ist nichts wichtig.

**The Marker Rule.** Gelb ist immer ein gezeichneter Markerstrich und immer „das hier“: aktiv, gewählt, wichtig. Keine gelben Flächen, Knöpfe oder Pillen.

## Typography

**Schrift:** Archivo (variabel, Gewicht 100–900, Breite 62–125 %), selbst gehostet in `fonts/`. Fallback: system-ui.
**Formeln:** CMU Serif (Regular, Italic, Bold), nur in MathML.

**Character:** Eine einzige Grotesk für alles. Hierarchie entsteht über Gewicht und Breite: Titel laufen schmal und schwer wie mit dickem Stift geschrieben, Lesetext breit und ruhig.

### Hierarchy
- **Seitentitel** (800, 2.25rem am Desktop, 1.5rem am Handy, Breite 86 %): Ein Titel pro Seite, immer mit blauem Unterstrich.
- **Schlagzeile** (600, 1.5rem, ab 900 px 1.75rem, Breite 94 %): Die Schlagzeile einer Wochenausgabe und der Fach-Karte.
- **Überschrift** (750, 1.125rem, Breite 92 %): Blöcke auf Heute, Teile einer Ausgabe, Dialoge.
- **Lesetext** (400, 1.0625rem, Zeilenhöhe 1.65, höchstens 68ch): Zusammenfassungen, Antworten, Anleitungen.
- **Bedienung** (450, 0.9375rem): Grundgröße der App: Navigation, Knöpfe, Listen.
- **Meta** (400, 0.8125rem, Tabellenziffern): Datum, Zähler, Hinweise unter Feldern, „Nicht in dieser Ausgabe“.
- **Etikett** (600, 0.75rem, Tabellenziffern): Pillen und Zähler. Tab-Beschriftung am Handy 0.6875rem.

**The Width Axis Rule.** Hierarchie über Breite und Gewicht, nicht über neue Schriften. Kein Kursiv-Display, keine Monospace-Etiketten, keine Großbuchstaben-Zeilen außer bei Legenden in Auswahl-Dialogen.

## Layout

- **Gerüst:** Ab 900 px Seitenleiste links (Rand als gezeichneter senkrechter Strich), darunter eine Tab-Leiste unten mit vier Einträgen plus „Mehr“ (Rand als gezeichneter waagrechter Strich). „Mehr“ öffnet die ganze Seitenleiste als Blatt.
- **Inhalt:** höchstens 1180 px breit. Seitenrand am Handy 16 px (plus Safe-Area), am Desktop clamp(24px, 4vw, 56px).
- **Heute:** eine Spalte; ab 1100 px zwei Spalten (Inhalt + 320–380 px Nebenspalte). Am Handy stehen die Fächer direkt unter dem Seitenkopf.
- **Fach-Ausgabe:** Kopf mit Titel links, drei Bedienelementen rechts (Mehr, Fassung, Neu erstellen). Darunter eine klebende Sprungleiste zu den Teilen. Ab 1360 px steht neben der Zusammenfassung eine 320-px-Spalte mit „Auf einen Blick“, nächstem PS und nächster VO.
- **Rhythmus:** 20 px zwischen Blöcken, 48 px vor jedem neuen Teil einer Ausgabe, 12 px unter Überschriften.
- **Breakpoints:** 640 px (Handy-Feinschliff), 900 px (Seitenleiste), 1100 px (Heute zweispaltig), 1360 px (Ausgabe mit Nebenspalte).

## Elevation & Depth

Flach. Tiefe entsteht durch gezeichnete Rahmen und ein zweites Blatt, nicht durch Schatten. `--schatten` ist im Skizzen-Stil leer.

### Shadow Vocabulary
- **Zweites Blatt** (`box-shadow: 6px -6px 0 -1.5px var(--bg), 6px -6px 0 0 var(--linie-2)`): Nur auf antippbaren Karten (Fach-Karten, „Weiterlernen“). Hinter dem Kasten liegt ein versetztes zweites Blatt; beim Hover rutscht es auf 9 px.
- **Schwebend** (`box-shadow: 0 2px 4px rgb(18 18 18 / 0.08), 0 30px 60px -28px rgb(18 18 18 / 0.45)`): Nur für Dinge über der Seite: Dialoge, Popover (Fassung, Mehr-Menü).

**The Second Sheet Rule.** Das versetzte zweite Blatt heißt „antippen öffnet etwas“. Nichts anderes bekommt es.

## Shapes

- **Handecken:** Ecken sind absichtlich ungleich gerundet (`4px 7px 5px 8px / 7px 4px 8px 5px`), wie ein gezogener Strich. Klein für Häkchen und Pillen-artige Felder, groß für Dialoge.
- **Gezeichnete Rahmen:** Knöpfe und antippbare Karten tragen statt einer CSS-Linie einen SVG-Rahmen mit überstehenden Strichen (`border-image`, 9-teilig gedehnt, Breite 1.5 px). Es gibt zwei Varianten, die sich bei geraden und ungeraden Geschwistern abwechseln, damit Reihen nicht kopiert aussehen, plus eine blaue (bzw. champagnerfarbene) für den Hauptknopf.
- **Ruhige Linie:** Reine Inhalts-Kästen ohne Aktion (Lerneinheiten, „Nächstes Proseminar“, Nebenspalte der Ausgabe) haben eine dünne 1-px-Linie in `linie`.
- **Gezeichnete Zeichen:** Häkchen, Kreis um das heutige Datum, Unterstrich, Textmarker und Trennstriche sind SVG-Striche, keine Unicode-Zeichen.
- **Symbole:** eigene Linien-Icons, 20 px (18 px in der Seitenleiste, 24 px in der Tab-Leiste), Strich 1.75, runde Enden.

**The Frame Rule.** Gezeichneter Rahmen = man kann es antippen. Ruhige Linie = nur zum Lesen. Ein Kasten, der nichts tut, bekommt keinen gezeichneten Rahmen.

## Components

### Buttons
- **Knopf:** Papier-Grund, Tinte, gezeichneter schwarzer Rahmen, 40 px hoch (44 px bei Touch), Innenabstand 8 × 14 px, Gewicht 600.
- **Hauptknopf (`knopf voll`):** derselbe Knopf mit blauem gezeichnetem Rahmen und Gewicht 650. Höchstens einer pro Bildschirm.
- **Leise (`knopf leise`):** ohne Rahmen, für Nebenaktionen in Listen.
- **Fokus:** 2 px Königsblau mit 2 px Abstand. **Hover:** Hauch von Gelb (`gelb-hauch`); der Hauptknopf bekommt beim Hover den vollen Textmarker.
- **Gruppen am Handy:** Reihen mit unterschiedlich langen Beschriftungen werden zum Raster: Hauptaktion über die volle Breite, Nebenaktionen gleich breit darunter.

### Menüs und Popover
- **Popover** (Fassung wählen, „Mehr“ im Fach-Kopf): gezeichneter Rahmen, schwebender Schatten, wächst in 180 ms aus dem Knopf (`cubic-bezier(0.23, 1, 0.32, 1)`), schließt schneller (120 ms). Öffnet unter dem Knopf, nie über den Bildschirmrand.
- **Menüpunkte:** 44 px hoch, ohne Rahmen, Hover in `papier-2`. Gewählte Optionen bekommen den Textmarker auf dem Namen.

### Cards / Containers
- **Fach-Karte:** gezeichneter Rahmen und zweites Blatt, Fach-Symbol in Fachfarbe, Schlagzeile, Meta-Zeile mit Datum und Lernstand.
- **Inhalts-Kasten:** ruhige Linie, Innenabstand 16 × 18 px. Fehlt der Inhalt, gibt es keinen leeren Kasten, sondern eine Meta-Zeile („Nicht in dieser Ausgabe: …“).
- **Zettel:** „Das Wichtigste“ steht auf gelbem Zettel (`zettel`), ohne Rahmen.

### Inputs / Fields
- **Felder:** 44 px Mindesthöhe, Rand 1.5 px in `linie-2`, Handecken. Das Hauptfeld einer Seite („Was willst du verstehen?“) trägt den blauen gezeichneten Rahmen.
- **Häkchen:** 20 px (22 px bei Touch) Kästchen mit Tinte-Rand und Handecken; gesetzt mit gezeichnetem blauem Haken. Steht ein Häkchen allein (Frist erledigt), bekommt es bei Touch eine unsichtbare Tippfläche von 44 px.

### Navigation
- **Seitenleiste:** gezeichnete Wortmarke, Gruppen (Fächer, Lernen, Austausch, Planen, Englisch) mit kleinen Überschriften; aktiver Eintrag mit Textmarker. Hover: Gelb-Hauch.
- **Tab-Leiste (Handy):** fünf gleich breite Felder (vier Ziele + Mehr), Symbol über Beschriftung; aktiver Tab mit Markerstrich unter der Beschriftung. Beschriftungen kurz und eindeutig: Heute, Thema, Kalender, Arena, Mehr.

### Sprungleiste der Ausgabe
- Klebt oben, Rand unten als gezeichneter Strich; der Teil, in dem man gerade liest, trägt den Textmarker.

## Do's and Don'ts

### Do:
- **Do** neue Bereiche aus den vorhandenen Bausteinen bauen (Knopf, Fach-Karte, Inhalts-Kasten, Popover, Meldung) und in hell, dunkel und am Handy ansehen.
- **Do** einen Begriff in der ganzen App gleich verwenden: was in der Tab-Leiste „Thema“ heißt, heißt überall so.
- **Do** am Handy nur die Tippfläche vergrößern (Padding, unsichtbare Fläche), nicht die Optik.
- **Do** leere Bereiche als eine Meta-Zeile zeigen oder mit dem einen passenden Knopf.

### Don't:
- **Don't** dunkle Verlaufs-Köpfe, Pillen-Streifen, Akkordeons mit Icon-Kreisen oder Ablaufgrafiken einbauen. So sah ein abgelehnter Entwurf aus, der wie eine fremde App wirkte.
- **Don't** einen zweiten Akzent neben Königsblau einführen oder Fachfarben für Flächen und Knöpfe benutzen.
- **Don't** Gelb als Fläche, Schrift- oder Knopffarbe verwenden.
- **Don't** gezeichnete Rahmen um Kästen legen, die nichts tun, oder Kästen zeigen, in denen nur „nicht vorhanden“ steht.
- **Don't** mehr als einen Hauptknopf und mehr als drei Knöpfe nebeneinander vor den eigentlichen Inhalt stellen; Seltenes kommt unter „Mehr“.
