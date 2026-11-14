"""Ergänzt die Datenschutzerklärung (recht.md oder recht-vorlage.md) um Lernhof und Campus.
Aufruf: python recht.py <Datei>. Läuft nur einmal (erkennt den Campus-Absatz) und legt vorher <Datei>.vor-campus an."""
import shutil
import sys

LERNHOF = ("- **Lernhof (`/game`, nur wenn du spielst):** Dein Spielstand liegt als Datei `lernhof.json` in deinem Speicher. Das Spiel liest deinen Lernstand "
           "(Karteikarten, erledigte Teile, Noten, Arena-Werte), um Belohnungen zu berechnen, und ändert ihn nicht. An gemeinsamen Orten sehen andere, die gerade "
           "spielen, deine Figur mit deinem Arena-Namen (ohne Arena-Namen als „Gast“). Diese Positionen stehen nur wenige Sekunden im Arbeitsspeicher und werden nicht gespeichert.")
CAMPUS = ("- **Campus (`/campus`, nur wenn du spielst):** Campus ist eine gemeinsame 3D-Welt mit eigener Datenbank auf demselben Server. Gespeichert werden: dein Name im Spiel "
          "(dein Arena-Name, sonst „Spieler“ mit deiner Kontonummer), Taler, Funde und Vorrat, das Aussehen deiner Figur, dein Grundstück und wer dort mitbauen darf, deine Bauten, "
          "Angebote und Gebote im Auktionshaus, Einzahlungen in Großprojekte und Gruppenkasse, deine Lerngruppe, deine Stufe und Lernserie, ein Kassenbuch aller Taler-Bewegungen "
          "und Meldungen, die du abschickst. **Für alle sichtbar, die im Campus sind:** dein Name, deine Figur mit Cosmetics und Namensschild, wo du gerade stehst, deine Bauten, "
          "deine Sprüche und Emotes, deine Angebote im Auktionshaus und dein Platz in der Wochenwertung (erlernte Taler der Woche). Deine E-Mail-Adresse sieht dort niemand. "
          "Damit Taler nicht fälschbar sind, zählt der Lernhelfer mit, was du lernst (Fokus-Minuten, bestandene Check-Fragen je Fach, gewusste fällige Karteikarten, bestandene "
          "Probeklausuren, Tages-Challenges, jeweils mit Tag, Lernserie und Abstand zur nächsten Prüfung im Fach), und gibt das an Campus weiter. Check-Fragen prüft dafür der Server: "
          "Deine Antwort geht mit Frage und Musterlösung an die KI. Die Verwaltung sieht im Campus zusätzlich Meldungen und auffällige Geschäfte (ungewöhnlich hohe Preise, einseitige "
          "Taler-Flüsse zwischen zwei Konten) und kann Bauten zurücksetzen und Konten für Campus sperren. **Kein Echtgeld:** Taler, Funde (Kisten mit zufälligem Inhalt) und Cosmetics "
          "gibt es nur fürs Lernen oder für Taler; sie lassen sich weder kaufen noch in Geld tauschen. Campus ist kein Glücksspiel um Geld.")
ZWECK = ("- Im Campus: Kassenbuch, Meldungen und die Anzeige auffälliger Geschäfte, um Schummeln und Belästigung zu erkennen und die Welt für alle spielbar zu halten "
         "(berechtigtes Interesse, Art. 6 Abs. 1 lit. f DSGVO). Dagegen kannst du jederzeit widersprechen.")
DAUER = ("- Lernhof und Campus: solange dein Konto besteht. Löschst du dein Konto, wird dein Campus-Spielstand mitgelöscht (Konto im Spiel, Vorrat, Kassenbuch, Grundstück samt "
         "Bauten darauf, Angebote, Meldungen); der Lernhof-Spielstand liegt in deinem Speicher und geht mit ihm. Was du auf der Gemeinschaftsfläche oder auf Grundstücken anderer "
         "gebaut hast, bleibt als Teil der Welt stehen, ohne deinen Namen. Einzahlungen in Großprojekte bleiben als Summe ohne Bezug zu dir.")
KEKS = " Lernhof und Campus nutzen dasselbe Cookie und merken sich Einstellungen (etwa die Block-Leiste) im Speicher deines Browsers."

pfad = sys.argv[1]
text = open(pfad, encoding="utf-8").read()
if "**Campus (`/campus`" in text:
    sys.exit("Schon ergänzt.")
aus, fehlt = [], []
getan = set()
for z in text.split("\n"):
    if z.startswith("- Konto, Unterlagen, alles, was du erstellst,") and "und Arena:" in z:
        z = z.replace("und Arena:", "Arena, Lernhof und Campus:")
        getan.add("zweck1")
    if z.startswith("**Deine Rechte:**") and ", dazu alles in deinem Speicher (" in z:
        z = z.replace(", dazu alles in deinem Speicher (", ", deinen Campus-Spielstand als `campus.json`, dazu alles in deinem Speicher (auch den Lernhof-Spielstand; ")
        getan.add("rechte")
    if z.startswith("**Cookies und Speicher im Browser:**"):
        z += KEKS
        getan.add("keks")
    aus.append(z)
    if z.startswith("- **Arena (nur wenn du mitmachst):**"):
        aus += [LERNHOF, CAMPUS]
        getan.add("daten")
    elif z.startswith("- Anmeldezeit, KI-Kosten pro Konto"):
        aus.append(ZWECK)
        getan.add("zweck2")
    elif z.startswith("- Arena: Die Wochenwerte"):
        aus.append(DAUER)
        getan.add("dauer")
fehlt = {"daten", "zweck1", "zweck2", "dauer", "rechte", "keks"} - getan
if "daten" in fehlt:
    sys.exit("Der Arena-Absatz wurde nicht gefunden; nichts geändert.")
shutil.copyfile(pfad, pfad + ".vor-campus")
open(pfad, "w", encoding="utf-8", newline="\n").write("\n".join(aus))
print("Ergänzt." + (f" Nicht gefunden (bitte von Hand prüfen): {', '.join(sorted(fehlt))}" if fehlt else ""))
