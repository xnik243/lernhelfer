# Campus: Plan für die gemeinsame 3D-Lernwelt

Stand 13.11.2026. Arbeitstitel „Campus“. Noch nichts davon ist gebaut.

## 1. Was es wird

Eine dauerhafte 3D-Blockwelt im Browser, die sich alle Lernhelfer-Konten teilen. Wer lernt, verdient Taler. Mit Talern
kauft man Blöcke, baut allein oder mit anderen, handelt im Auktionshaus und schaltet Cosmetics frei.

**Feste Zusagen**

- Der Lernhof bleibt unverändert unter `/game`. Campus bekommt die eigene Adresse `/campus`.
- Am Lernhelfer wird nichts gelöscht oder ersetzt. Campus läuft als eigenes Programm mit eigener Datenbank. Stürzt es
  ab, läuft der Lernhelfer weiter.
- Campus liest Lerndaten nur. Es schreibt nie in Karten, Lernstand, Noten oder Arena.
- Alles ist eigene Arbeit: Blöcke, Figuren, Cosmetics, Namen, Töne. Kein Nachbau von Minecraft oder anderen Spielen.

## 2. Der Kreislauf

1. Im Lernhelfer lernen.
2. **Zahltag:** Der Lernhelfer zeigt „Du hast 140 Taler verdient, 1 Fund wartet“ mit Knopf ins Spiel.
3. Im Spiel: Auszahlung, Fund öffnen, Blöcke kaufen, bauen, handeln, an Großprojekten mitbauen.
4. Andere sehen das Ergebnis. Nächstes Ziel: größeres Grundstück, seltenes Teil, neues Cosmetic.

Herumlaufen, anschauen und handeln geht immer. Nur Bauen und Kaufen kostet Taler.

## 3. Taler verdienen

Zahlen sind Startwerte und werden nach den ersten Wochen angepasst.

| Quelle | Taler | Wer zählt |
|---|---|---|
| Fokus-Minute in der Arena | 1 (höchstens 240 am Tag) | Server, wie heute die XP |
| Check-Frage von der KI bestanden | 15, dazu 1 Fund | Server (siehe Abschnitt 9) |
| Karteikarte gewusst | 2 (höchstens 60 Karten am Tag) | Browser meldet, Server begrenzt |
| Probeklausur bestanden | 100, dazu 1 Bauplan | Server |
| Tages-Challenge der Arena | 25 | Server |
| Lernserie | +5 % je Tag, höchstens +50 % | Server |
| Prüfungszeit (14 Tage vor Klausur) | doppelt im Prüfungsfach | Server |

Es gibt keinen Weg, Taler im Spiel zu erzeugen oder mit Geld zu kaufen.

## 4. Ausgeben und Handeln

- **Laden:** normale Blöcke nach Wahl, feste Preise (1 bis 5 Taler je Block).
- **Funde:** Kiste mit zufälligem Inhalt, meist normal, selten etwas Besonderes (Lichtblock, Mechanik, Farbe,
  Cosmetic-Teil). Nur durchs Lernen, nie zu kaufen.
- **Wochenblöcke:** jede Woche zwei Blöcke, die es nur in dieser Woche im Laden gibt.
- **Auktionshaus:** Angebote für seltene Blöcke, Baupläne und handelbare Cosmetics. Gebot oder Sofortkauf, Laufzeit
  24 oder 72 Stunden, 5 % Gebühr verschwinden aus dem Spiel. Gebote werden beim Bieten reserviert.
- **Grundstück:** wächst mit der Stufe (Start 16 × 16 Blöcke). Erweiterungen kosten Taler.
- **Großprojekte:** Brücke, Turm, neue Insel. Kosten tausende Taler, alle zahlen ein, ein Balken zeigt den Stand.

## 5. Zusammen spielen

- Eine Welt für alle, in Echtzeit. Man sieht Figuren, Bauen und Sprüche der anderen sofort.
- Auf dem eigenen Grundstück baut nur man selbst und wen man einträgt. Gemeinschaftsflächen sind für alle offen.
- **Lerngruppen:** eigenes Viertel, gemeinsame Kasse, Wochenwertung nach verdienten Talern.
- **Wochenziel der Welt:** Lernen alle zusammen genug, öffnet sich ein neues Gebiet.
- Alles funktioniert auch zeitversetzt: Bauten bleiben stehen, Einzahlungen und Gebote laufen ohne die anderen.
- Verständigung über feste Sprüche und Emotes. Freier Chat erst, wenn es Melden und Sperren gibt.

## 6. Cosmetics

Als Anregung für Arten und Aufbau sehe ich mir beim Bauen an, was Badlion, Lunar und LabyMod anbieten. Übernommen
werden nur die Kategorien, alle Designs sind eigene.

| Art | Beispiele |
|---|---|
| Umhang | Fachfarben, Semester-Umhang, animierter Umhang |
| Kopf | Hüte, Kronen, Heiligenschein, Kopfhörer |
| Rücken | Flügel, Rucksack, Bücherstapel |
| Begleiter | kleines Tier oder Schwebeding, das folgt |
| Spur | Funken, Blätter, Tinte beim Laufen |
| Emotes | Winken, Jubeln, Hinsetzen, Tanz |
| Namensschild | Titel, Rahmen, Farbe |
| Bau-Effekt | Aussehen beim Setzen eines Blocks |

**Drei Wege, sie zu bekommen**

1. **Erlernt, nicht handelbar.** Zeigen, was man geleistet hat:
   - Serie 7, 30, 100 Tage: Flammen-Spur in drei Stufen
   - Stufe 5, 10, 20, 30: Umhänge mit steigendem Detail
   - Platz 1 bis 3 der Wochenwertung: Krone für eine Woche
   - Probeklausur mit Note 1: goldener Rahmen am Namensschild
   - 100 bestandene Check-Fragen in einem Fach: Fach-Umhang
2. **Gekauft** mit Talern im Laden.
3. **Gefunden** in Funden, handelbar im Auktionshaus. Dazu Semester-Stücke, die es danach nicht mehr gibt.

Jeder stellt sein Aussehen selbst zusammen: Hautton, Haare, Kleidung, Farben und die Cosmetics.

## 7. Technik

| Teil | Technik | Ort |
|---|---|---|
| Spielserver | Rust (tokio, axum, WebSocket), SQLite | eigener Dienst `campus`, Port 8090 |
| Spiel im Browser | Rust als WebAssembly, Grafik über wgpu (WebGPU, sonst WebGL 2) | `/campus/` |
| Lernhelfer | bleibt Node, bekommt nur kleine Zusätze (Abschnitt 8) | wie bisher |
| Verteilung | Caddy leitet `/campus/*` an Port 8090, alles andere wie bisher | eine Regel in der Caddyfile |

**Auf dem Server vorhanden (13.11.2026):** Debian 13, 2 Kerne, 2 GB RAM, Rust 1.85.1 mit WebAssembly-Ziel aus den
Debian-Paketen. Kein Auslagerungsspeicher. Gebaut wird deshalb mit einem Kern und niedriger Priorität, damit der
Lernhelfer nicht stockt.

**Für Tempo ausgelegt**

- Welt in Stücken von 16 × 16 × 64 Blöcken. Der Browser lädt nur die Stücke in Sichtweite.
- Je Stück ein zusammengefasstes Gitter (benachbarte gleiche Flächen werden eine), neu berechnet nur bei Änderung.
- Server schickt Positionen 10-mal je Sekunde und nur an Spieler in der Nähe. Blockänderungen einzeln.
- Der Server entscheidet alles (Taler, Rechte, Reichweite). Der Browser zeigt nur an.
- Speichern alle 30 Sekunden, nur geänderte Stücke.
- Ziel: 60 Bilder je Sekunde auf einem iPad, 50 Spieler gleichzeitig ohne spürbare Last.

**Bedienung:** Tastatur und Maus am Mac und PC, zwei Daumenfelder und Knöpfe am iPad.

## 8. Anbindung an den Lernhelfer

- **Anmeldung:** Der Spielserver reicht das Sitzungs-Cookie an `GET /api/ich` des Lernhelfers weiter und erfährt so
  das Konto. Kein zweites Passwort, keine Änderung am Lernhelfer nötig.
- **Verdienst:** neuer Abruf `GET /api/campus/verdienst` im Lernhelfer, nur vom Spielserver erreichbar. Er liefert,
  was der Server seit dem letzten Abruf gezählt hat. Der Spielserver schreibt Taler gut.
- **Zahltag-Hinweis:** kleine Anzeige im Lernhelfer nach dem Lernen und ein Eintrag „Campus“ in der Gruppe „Spiel“.
- **Konto löschen und Datenexport:** Der Lernhelfer ruft beim Löschen den Spielserver, der Exportknopf holt die
  Spieldaten dazu.

## 9. Schummeln verhindern

Heute stehen die Antworten auf Karteikarten und das Urteil der Check-Fragen im Browser. Wer sich auskennt, könnte
sie fälschen. Sobald Taler einen Wert haben und gehandelt werden, muss das dicht sein:

- **Check-Fragen:** Die Prüfung wandert auf den Server. Der Server kennt Frage und Musterlösung, ruft die KI und
  merkt sich das Ergebnis selbst. Je Frage zählt nur das erste Bestehen.
- **Karteikarten:** niedrige Taler, Tagesgrenze, und jede Karte zählt höchstens so oft, wie ihr Wiederholplan erlaubt.
- **Fokus-Minuten:** zählt der Server schon heute.
- **Handel:** Verdächtige Geschenke zwischen Konten (ein Konto verschenkt alles an ein anderes) werden gemeldet.

## 10. Recht und Schutz

- **Datenschutzerklärung** ergänzen: Spielstand, Arena-Name, Aussehen, Bauten und Handel sind für andere sichtbar.
  Auch der Lernhof fehlt dort noch.
- **Melden und Sperren:** Jeder Bau und jeder Spieler lässt sich melden. In der Admin-Ecke: Bau zurücksetzen,
  Konto aus dem Spiel sperren.
- **Kein Echtgeld.** Taler, Funde und Cosmetics sind nicht käuflich. Sollten sie es je werden, wären Funde mit
  Zufallsinhalt rechtlich heikel (Glücksspiel-Nähe); das müsste vorher geprüft werden.

## 11. Bauabschnitte

Jeder Abschnitt endet spielbar und geprüft. Der nächste beginnt erst, wenn der vorige unter `/campus` läuft.

| Nr. | Inhalt | Fertig, wenn |
|---|---|---|
| 0 | Gerüst: Rust-Projekt, Dienst, Caddy-Regel, Anmeldung | `/campus` zeigt das eigene Konto, Lernhelfer-Tests grün |
| 1 | Welt allein: Insel, laufen, springen, Blöcke setzen und entfernen, speichern | 60 Bilder je Sekunde im emulierten iPad |
| 2 | Echtzeit: andere sehen, gemeinsames Bauen, Grundstücke, Sprüche | zwei Konten bauen gleichzeitig, nichts geht verloren |
| 3 | Taler: Verdienst aus dem Lernhelfer, Laden, Zahltag, Check-Prüfung auf dem Server | Lernen im Test bringt genau die erwarteten Taler |
| 4 | Aussehen und Cosmetics: Figur, erlernte und kaufbare Stücke, Emotes | Cosmetic erscheint bei beiden Konten |
| 5 | Funde, Wochenblöcke, Auktionshaus | Gebot, Überbieten, Ablauf, Gebühr stimmen auf den Taler |
| 6 | Großprojekte, Wochenziel, Lerngruppen | Projekt wird von zwei Konten fertig gebaut |
| 7 | Melden, Sperren, Datenschutztext, Erklärseite mit Bildern | Admin setzt einen Bau zurück |

Geprüft wird wie beim Lernhof: Unit-Tests für alle Regeln (Taler, Markt, Rechte), Lauf mit zwei Konten im Browser
gegen einen lokalen Server, Lasttest mit 50 simulierten Spielern, und nach jedem Abschnitt die Tests des Lernhelfers.

## 12. Risiken

- **Zu wenige Spieler.** Eine geteilte Welt wirkt mit fünf Leuten leer. Gegenmittel: kleine Startinsel, alles
  funktioniert zeitversetzt, die Welt wächst erst mit den Spielern.
- **Umfang.** Das ist ein Mehrfaches des Lernhofs. Die Abschnitte 0 bis 3 sind der Kern; danach lässt sich
  entscheiden, ob es trägt.
- **Server.** 2 GB RAM teilen sich Lernhelfer, overfitted.xyz und Campus. Der Lasttest in Abschnitt 2 zeigt, ob das
  reicht oder ein größerer Server nötig wird.
- **iPad.** WebGPU ist dort neu. Fällt es aus, läuft die Grafik über WebGL 2; das wird in Abschnitt 1 geprüft.
- **Rust-Version.** Die Debian-Pakete liefern 1.85.1. Verlangt eine Bibliothek Neueres, wird der Browser-Teil auf
  dem PC gebaut oder Rust auf dem Server anders eingerichtet.
- **Wirtschaft.** Preise und Verdienst sind geschätzt. Zu großzügig, und der Markt ist wertlos; zu knapp, und es
  frustriert. Deshalb sind alle Zahlen an einer Stelle einstellbar.

## 13. Was du entscheiden musst

1. Name: „Campus“ oder etwas anderes.
2. Ab welchem Tarif man mitspielen darf (Vorschlag: alle, wie beim Lernhof).
3. Ob die Caddy-Regel für `/campus` und ein Auslagerungsspeicher auf dem Server eingerichtet werden dürfen
   (beides Änderungen am System, beides rückgängig zu machen).
4. Ob ich mit Abschnitt 0 und 1 anfangen soll.
