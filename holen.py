"""Holt neue Unterlagen aus OLAT und lädt ihren Text in den eigenen Speicher: mit dem Lernhelfer-Server (lernhelfer.xyz) auf
diesen Server, nur in der selbst betriebenen Variante ohne Server in ein privates GitHub-Repo (über die GitHub-API, ohne git).

Einrichten: in der App unter Einrichten bzw. Konto. Windows: einrichten.ps1 (Python, dieses Skript und die Aufgabenplanung nach
%LOCALAPPDATA%\\Lernhelfer); Mac: einrichten-mac.sh (nach ~/Library/Lernhelfer, launchd statt Aufgabenplanung).
Start von Hand: python holen.py [Fach ...]; --auto = Lauf aus der Aufgabenplanung, --kurse = nur Kurse suchen.
Die Aufgabenplanung ruft alle 30 Minuten auf, OLAT sieht davon aber nur einen Abgleich etwa alle 12 Stunden, tagsüber und mit
Pausen zwischen den Seiten wie beim Klicken von Hand (faellig(), pause()); dazwischen geht keine einzige Anfrage an OLAT.
Ist die OLAT-Sitzung abgelaufen, öffnet sich Chrome (am Mac ohne Chrome: Chromium): dort selbst einloggen (SSO + Authenticator).
Automatischer Login: aus, bis man ihn in der App einschaltet und den Hinweisen zustimmt. Dann fragt der nächste Start von Hand
einmal Kennung, Passwort und 2FA-Schlüssel ab, speichert sie verschlüsselt auf diesem Computer (Windows: DPAPI, Mac: Schlüsselbund)
und schickt sie nur an die Login-Seite der Uni, nie an den Lernhelfer. Schalter wieder aus = gespeicherte Daten gelöscht (autologin()).

Tests: Gelesen werden die Startseiten (Zeitraum, Punkte, Versuche), nach Ablauf stehen Fragen und Lösung dort. Selbsttests
werden einmal angesehen und mit „Test unterbrechen“ + „Ok“ verlassen. Bewertete Tests startet er nie. Ein Wächter im
Browser lässt nur Lesen und genau den jeweils nächsten erlaubten Schritt durch – nie Speichern, Beenden oder Abbrechen, siehe erlaubt().
"""
import atexit
import base64
import getpass
import hashlib
import hmac
import html
import json
import logging
import math
import os
import random
import re
import shutil
import signal
import struct
import subprocess
import sys
import threading
import time
import traceback
import unicodedata
import zipfile
from datetime import datetime, timedelta
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qs, quote, unquote, urljoin, urlparse
from urllib.request import Request, urlopen

if sys.platform == "darwin":  # Mac: Zertifikate aus dem Schlüsselbund wie in Safari (truststore lädt das Einrichten mit)
    try:
        import truststore
        truststore.inject_into_ssl()
    except ImportError:
        pass

def json_lesen(pfad, leer):
    """Eigene Zustandsdatei lesen; kaputt (Strom weg beim Schreiben) zählt als leer, statt jeden Lauf still abstürzen zu lassen."""
    try:
        return json.loads(pfad.read_text("utf-8")) if pfad.exists() else leer
    except ValueError:
        return leer


def json_schreiben(pfad, daten, **kw):
    """Erst in eine Nebendatei, dann umbenennen: geht der PC mitten im Schreiben aus, bleibt die alte Fassung ganz."""
    neu = pfad.with_name(pfad.name + ".neu")
    neu.write_text(json.dumps(daten, ensure_ascii=False, **kw), "utf-8")
    os.replace(neu, pfad)


BASIS = Path(__file__).parent
MATERIAL = BASIS / "material"
KURSDATEI = BASIS / "kurse.json"  # fehlt sie, sucht der erste Abgleich die Kurse selbst (kurse_finden)
KURSE = json_lesen(KURSDATEI, None)
# {"repo", "token", "api", "app"}, schreibt das Einrichten. Mit Server: token öffnet nur den eigenen Speicher dort (kein Konto-Passwort),
# api und app zeigen auf den Server. Selbst betriebene Variante: repo = privates Daten-Repo, token = GitHub-Token nur dafür, api = null.
ZUGANG = BASIS / "zugang.json"
STAND = BASIS / ".stand.json"  # letzter Abgleich, hochgeladene Dateiversionen, wann das Login-Fenster zuletzt kam
APP = "https://xnik243.github.io/lernhelfer/"  # nur ohne Server; mit Server steht die Adresse als "app" in zugang.json
API = os.environ.get("LH_API", "https://api.github.com")  # ebenso, mit Server gilt "api" aus zugang.json; der Prüfstand setzt hier eine Attrappe ein
# Angemeldet erkennt man an der Kopfzeile; auch mit OLAT auf Englisch, sonst gälte die Sitzung immer als abgelaufen
EINGELOGGT = re.compile(r"Eingeloggt als|Logged in as", re.I)
API_PAUSE = 5  # Sekunden vor dem ersten Nachfassen, wenn das Hochladen am Netz scheitert (Tests setzen 0)
SITZUNG = BASIS / ".olat-sitzung"  # OLAT-Cookies, verschlüsselt wie der Login (sitzung_speichern)
SITZUNG_ALT = BASIS / ".olat-sitzung.json"  # frühere Klartext-Fassung: wird noch gelesen, beim nächsten Speichern gelöscht
PROFIL = BASIS / ".olat-chrome"  # eigenes Chrome-Profil nur fürs OLAT-Login
if sys.platform == "darwin":  # Mac: Chrome, sonst das Chromium von Playwright (lädt das Einrichten, wenn Chrome fehlt)
    KANAL = "chrome" if Path("/Applications/Google Chrome.app").exists() else None
else:
    KANAL = "chrome" if any(Path(p, r"Google\Chrome\Application\chrome.exe").exists() for p in
                            (r"C:\Program Files", r"C:\Program Files (x86)", Path.home() / r"AppData\Local")) else "msedge"
BROWSER = {"chrome": "Chrome", "msedge": "Edge"}.get(KANAL, "Chromium")
GERAET, ABLAGE = ("Mac", "Schreibtisch") if sys.platform == "darwin" else ("PC", "Desktop")
HOCHGELADEN = "Hochgeladen/"  # Dateien, die die App selbst hochgeladen hat (iPad, Mac ohne Programm): bleiben beim Abgleich erhalten
OLAT = os.environ.get("LH_OLAT", "") if re.fullmatch(r"http://127\.0\.0\.1:\d+", os.environ.get("LH_OLAT", "")) else "https://lms.uibk.ac.at"  # nur die lokale Attrappe von test_probelauf.py
FASSUNG = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()[:8]  # für Fehlerberichte; beim Start gelesen, das Selbst-Update tauscht die Datei mitten im Lauf
ARTEN = ["folien", "ps", "syllabus"]
TESTS = "Tests"  # material/<Fach>/Tests: Startseiten der Onlinetests, Fragen der Selbsttests
# Knöpfe, die im Test vor jedem Klick aus der Seite entfernt werden („Test unterbrechen“ bleibt, ist aber bis zum Schluss gesperrt)
GEFAHR = ".o_sel_assessment_item_submit, .o_sel_end_testpart, .o_sel_cancel_test, .o_sel_next_question"
logging.getLogger("pypdf").setLevel(logging.ERROR)  # harmlose Warnungen bei schief gebauten PDFs
TAGE = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
MONATE = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"]
# Konsole: gebaut wird mit modernen Zeichen, die alte Windows-Konsole (Rasterschrift, nur Codepage 850) bekommt Ersatz
ALT = str.maketrans("✓✗━┄╭╮╰╯…„“–", "■×═·┌┐└┘.»«-")
FACH_WIN = {"Mathematik": 80, "VWL": 214, "BWL": 183, "Englisch": 218}  # Fachfarben der App als 256er-Farben
FACH_ECHT = {"Mathematik": "94;234;212", "VWL": "251;191;36", "BWL": "196;181;253", "Englisch": "249;168;212"}
FACH_MAC = {"Mathematik": 37, "VWL": 172, "BWL": 141, "Englisch": 205}  # Terminal.app ist oft hell
ERSATZ_WIN, ERSATZ_MAC = (117, 156, 210, 223, 151, 181), (74, 71, 167, 137, 103, 132)  # andere Fächer, stabil je Name
TESTICONS = ("o_iqtest_icon", "o_iqself_icon", "o_ms_icon")


def uhr(s):
    s = int(s)
    return f"{s // 60}:{s % 60:02d}"


def groesse(b):
    return f"{b / 1e6:.1f} MB".replace(".", ",") if b >= 1e6 else f"{max(1, round(b / 1e3))} KB"


def anzahl(n, eins, mehr):
    return f"{n} {eins if n == 1 else mehr}"


def bytes_von(p):
    """Dateigröße nur für die Anzeige: ein Virenscanner kann die Datei schon weggeräumt haben."""
    try:
        return p.stat().st_size
    except OSError:
        return None


def wann(dt, jetzt=None):
    """heute / gestern / morgen / Wochentag (bis 6 Tage) / 12.11."""
    tage = (dt.date() - (jetzt or datetime.now()).date()).days
    return {0: "heute", -1: "gestern", 1: "morgen"}.get(tage) or (TAGE[dt.weekday()] if abs(tage) < 7 else f"{dt:%d.%m.}")


def grund(e):
    t = fehlertext(e)
    if "imeout" in t:
        return "OLAT hat nicht rechtzeitig geantwortet"
    return "keine Verbindung zu OLAT" if "net::" in t else t


def _sauber(s):
    """Jede Zelle genau ein Zeichen breit: ohne Steuerzeichen, breite Zeichen (Emoji, CJK) werden zu „?“."""
    s = unicodedata.normalize("NFC", str(s))
    return "".join("?" if unicodedata.east_asian_width(c) in "WF" else c for c in s
                   if unicodedata.category(c) not in ("Cc", "Cf", "Mn", "Me"))


class Anzeige:
    """Konsolenfenster beim Abgleich von Hand („Lernhelfer OLAT“, Einrichten): Kopf mit Logo und letztem Lauf, darunter ein
    Logbuch (✓-Zeilen, Kurse, Funde) und ganz unten ein Live-Bereich: auf der Spur fährt das Logo als Chip mit, Funde bleiben
    als Marken in Fachfarbe liegen, Pausen zwischen OLAT-Seiten sind sichtbar. Am Ende eine Karte mit Abzeichen.
    Übernimmt sys.stdout nur bei echter Konsole; ins Log (--auto) und in test_holen.py geht alles wie bisher, die Methoden
    schreiben dann nichts (außer melden/warnung/fehler, die wie früher print() machen). Andere print()-Zeilen landen über dem Bereich."""
    RAD, RAD_ALT = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏", "░▒▓█▓▒"
    BLAU, GRUEN, ROT, GRAU, DUNKEL = "\x1b[38;5;69m", "\x1b[38;5;78m", "\x1b[38;5;203m", "\x1b[38;5;244m", "\x1b[38;5;240m"
    FETT, AUS, CHIP = "\x1b[1m", "\x1b[0m", "\x1b[48;5;{}m\x1b[1;38;5;16m"
    KAPPE, TASTE = "\x1b[48;5;238m\x1b[38;5;252m", "Esc" if sys.platform == "win32" else "ctrl C"  # Abbrechen: esc_abbruch() bzw. Strg+C

    def __init__(self):
        self.an, self.raus, self.alt, self.echt, self.mac, self.leiste = False, None, False, False, False, False
        self.abbruch, self.von_hand = "abbrechen", False  # Tastenkappe rechts im Bereich; von_hand: Esc/Strg+C statt Absturz
        self.lock = threading.RLock()
        self.start = time.monotonic()
        self.offen = False  # angefangene Zeile ohne Zeilenende (Eingabe-Frage): dann nicht dazwischenzeichnen
        self.bild, self.breiten, self.osc, self.stufe = None, [], None, None
        self.z = {"pausen": 0, "pause_s": 0.0, "stellen": 0, "gesehen": 0, "gelesen": 0, "tests": 0, "bytes": 0, "fehler": 0}
        self.funde, self.tests_neu, self.testnamen, self.groesster = {}, {}, [], None
        self.verdeckt, self.pro_stelle, self.geschickt, self.alle_faecher = None, 0, [], []
        self.pop_bis = self.pop_n = 0
        self.schritt(None)

    # ---------- Ausgabe ----------
    def einschalten(self):
        if not (sys.stdout and sys.stdout.isatty()):
            return
        if sys.platform == "win32":
            os.system("")  # schaltet in der alten Windows-Konsole Farben und Steuerzeichen ein
        self.raus, self.an = sys.stdout, True
        pty = False  # Windows Terminal als Standardterminal (Doppelklick): kein WT_SESSION, aber ein Pseudo-Konsolenfenster
        if sys.platform == "win32" and not os.environ.get("WT_SESSION"):
            try:
                import ctypes
                puffer = ctypes.create_unicode_buffer(64)
                ctypes.windll.user32.GetClassNameW(ctypes.windll.kernel32.GetConsoleWindow(), puffer, 64)
                pty = puffer.value == "PseudoConsoleWindow"
            except Exception:
                pass
        vscode = os.environ.get("TERM_PROGRAM") == "vscode"
        self.leiste = bool(os.environ.get("WT_SESSION")) or (pty and not vscode)
        self.alt = sys.platform == "win32" and not self.leiste and not vscode
        self.echt = self.leiste or os.environ.get("COLORTERM") in ("truecolor", "24bit")
        self.mac = sys.platform == "darwin"
        sys.stdout = sys.stderr = self
        atexit.register(self.schluss)
        threading.Thread(target=self._drehen, daemon=True).start()
        self._kopf()

    def _kopf(self):
        j = datetime.now()
        try:
            st = json_lesen(STAND, {})
            a = st.get("abgleich") or {}
            t = datetime.fromisoformat(a["zeit"])
            drei = f"zuletzt {wann(t)} {t:%H:%M} · " + (a.get("text", "") if a.get("ok") is not False else "ging nicht: " + a.get("text", ""))
            drei += f" · dauerte {uhr(st['dauer'])}" if st.get("dauer") and a.get("ok") is not False else ""
        except Exception:
            drei = f"erster Abgleich auf diesem {GERAET} · dauert ein paar Minuten"
        g, aus = self.CHIP.format(227), self.AUS
        haken = "\x1b[38;5;26m▄▀ " if self.alt else "\x1b[38;5;26m✓  "
        platz = shutil.get_terminal_size().columns - 13
        drei = self._kuerzen([(drei, self.GRAU)], platz)[0]
        self.write(f"\n  {g}      {aus}   {self.FETT}Lernhelfer{aus}\n"  # das Logo: gelbes Feld, schwarzes L, blauer Haken
                   f"  {g}  L{haken}{aus}   {self.GRAU}OLAT-Abgleich · {TAGE[j.weekday()]}, {j.day}. {MONATE[j.month - 1]} · {j:%H:%M}{aus}\n"
                   f"  {g}      {aus}   {drei}\n\n")

    def _raus(self, s):
        """Einzige Stelle, die in die echte Konsole schreibt."""
        if self.alt:
            s = s.translate(ALT).encode("cp850", "replace").decode("cp850")
        self.raus.write(s)

    def write(self, s):
        with self.lock:
            self._weg()
            self._raus(s)
            if s:
                self.offen = not s.endswith("\n")
            if self.offen:
                self._raus("\x1b[?25h")  # Eingabe: Cursor sichtbar
            self._zeichnen()
            self.raus.flush()
        return len(s)

    def flush(self):
        self.raus.flush()

    def __getattr__(self, name):  # isatty, encoding, fileno … wie die echte Konsole
        return getattr(self.raus, name)

    def _hoch(self):
        """Zeilen über dem Cursor im Live-Bereich, auch wenn das Fenster inzwischen schmaler ist und neu umbrochen hat."""
        sp = max(1, shutil.get_terminal_size().columns)
        return sum(max(1, -(-b // sp)) for b in self.breiten) - 1

    def _weg(self):
        if self.breiten:
            h = self._hoch()
            self._raus("\r" + (f"\x1b[{h}A" if h > 0 else "") + "\x1b[J")
            self.breiten, self.bild = [], None

    def _drehen(self):
        while True:
            time.sleep(0.1)
            with self.lock:
                self._zeichnen()

    def _zeichnen(self):
        if not self.an or self.offen:
            return
        try:
            zeilen = self._bereich() if self.titel else []
        except Exception:
            return  # ponytail: die Anzeige darf den Abgleich nie stoppen; dann fällt ein Bild aus
        bild = "\n".join(t for t, _ in zeilen)
        if bild == self.bild:
            return
        if not zeilen:  # Bereich weg: Cursor wieder an (input() und getpass schreiben an write() vorbei), Titel und Taskleiste zurück
            self._weg()
            self._raus(self._titel_osc() + "\x1b[?25h")
            self.raus.flush()
            self.bild = ""
            return
        h = self._hoch() if self.breiten else 0
        s = "\r" + (f"\x1b[{h}A" if h > 0 else "") + "\x1b[?25l" + "\x1b[K\n".join(t for t, _ in zeilen) + "\x1b[K\x1b[J"
        if self.echt:
            s = "\x1b[?2026h" + s + "\x1b[?2026l"  # Windows Terminal: das Bild auf einmal zeigen, ohne Flackern
        self._raus(self._titel_osc() + s)
        self.raus.flush()
        self.bild, self.breiten = bild, [b for _, b in zeilen]

    def _titel_osc(self):
        """Fenstertitel mit Fortschritt; im Windows Terminal auch am Taskleisten-Symbol."""
        s = ""
        titel = (f"Lernhelfer · {self.titel}" + (f" {self.wert}/{self.gesamt}" if self.gesamt else "")) if self.titel else "Lernhelfer"
        if titel != self.osc:
            s, self.osc = f"\x1b]0;{titel}\x07", titel
        stufe = (f"1;{100 * min(self.wert, self.gesamt) // self.gesamt}" if self.gesamt else "3;0") if self.titel else "0;0"
        if self.leiste and stufe != self.stufe:
            s, self.stufe = s + f"\x1b]9;4;{stufe}\x07", stufe
        return s

    def _kuerzen(self, teile, platz):
        """[(Text, Farbe)] → (Zeile mit Farben, sichtbare Breite); was nicht passt, wird mit … abgeschnitten."""
        aus, breite = "", 0
        for text, farbe in teile:
            text = _sauber(text)
            if len(text) > platz - breite:
                rest = platz - breite
                text = text[:rest - 1] + "…" if rest > 0 else ""
            breite += len(text)
            aus += f"{farbe}{text}{self.AUS}" if farbe and text else text
        return aus, breite

    def _farbe(self, fach, hg=False):
        art = 48 if hg else 38
        if self.echt and fach in FACH_ECHT:
            return f"\x1b[{art};2;{FACH_ECHT[fach]}m"
        tabelle, ersatz = (FACH_MAC, ERSATZ_MAC) if self.mac else (FACH_WIN, ERSATZ_WIN)
        return f"\x1b[{art};5;{tabelle.get(fach) or ersatz[sum(map(ord, fach or '')) % len(ersatz)]}m"

    def _kante(self):
        return min(shutil.get_terminal_size().columns - 1, 96)

    # ---------- Live-Bereich ----------
    def _bereich(self):
        groesse_t = shutil.get_terminal_size()
        sp, t = groesse_t.columns, time.monotonic()
        n = int(t * 10)
        pause = t < self.pause_bis
        rad = (self.RAD_ALT[(n // 3) % len(self.RAD_ALT)] if self.alt else self.RAD[n % len(self.RAD)])
        lauf = uhr(t - self.start)
        if sp < 40 or groesse_t.lines < 8:  # winziges Fenster: eine Zeile wie früher
            return [self._kuerzen([("  ", ""), ("·" if pause else rad, self.BLAU), (" " + self.titel, self.FETT),
                                   (f"  {self.wert}/{self.gesamt}" if self.gesamt else "", ""), (f"  {lauf}", self.GRAU),
                                   (f"   {self.ort} {self.tun}", self.GRAU)], sp - 1)]
        # Z1: Titel, Spur mit Logo-Chip, Zähler, Zeit
        links = f"  {self.titel}   "
        zahl = f"  {self.wert}/{self.gesamt}" if self.gesamt else ""
        if self.frist:
            zeit = f"   noch {uhr(max(0, self.frist - (t - self.seit)))}   {lauf}"
        else:
            zeit = f"   {lauf}" + (f" · davon {uhr(self.z['pause_s'] - max(0, self.pause_bis - t))} Pause" if self.z["pausen"] else "")
        w = min(40, sp - 1 - len(links) - len(zahl) - len(zeit))
        if w < 12 and not self.frist:
            zeit = f"   {lauf}"
            w = min(40, sp - 1 - len(links) - len(zahl) - len(zeit))
        z1 = [(links, self.FETT), *(self._spur(w, n, t, pause) if w >= 12 else []), (zahl, ""), (zeit, self.GRAU)]
        # Z2: was gerade passiert; in einer Pause steht das Rad und ein Balken leert sich
        if pause:
            rest = self.pause_bis - t
            voll = max(0, min(5, math.ceil(5 * rest / max(self.pause_dauer, 0.1))))
            z2 = [("   · ", self.DUNKEL), (self.ort, self.FETT), ("  Pause ", self.GRAU), ("■" * voll, self.BLAU),
                  ("·" * (5 - voll), self.DUNKEL), (f" {rest:.1f} s".replace(".", ",") + " · wie beim Klicken von Hand", self.GRAU)]
        else:
            z2 = [("   ", ""), (rad, self.BLAU), (" ", ""), (self.ort, self.FETT), (("  " if self.ort else "") + self.tun, self.GRAU)]
        kappe = f" {self.TASTE} "  # rechts an der Kante, wenn Platz ist: welche Taste abbricht
        frei = self._kante() - sum(len(_sauber(x)) for x, _ in z2) - len(kappe) - 1 - len(self.abbruch)
        if self.abbruch and frei >= 3:
            z2 += [(" " * frei, ""), (kappe, self.KAPPE), (" " + self.abbruch, self.GRAU)]
        zeilen = [self._kuerzen(z1, sp - 1), self._kuerzen(z2, sp - 1)]
        # Z3: Neues je Fach und ein Hinweis zur Phase
        if sp >= 72 and (self.faecher or self.hinweis):
            zeilen.append(self._z3(sp))
        return zeilen

    def _spur(self, w, n, t, pause):
        chip = self.CHIP.format(229 if pause and (n // 5) % 2 else 227)
        chip_text = f"+{min(self.pop_n, 9)} " if t < self.pop_bis else " L "
        laenge = w - 3
        if not self.gesamt:  # Dauer unbekannt: der Chip pendelt
            laenge = min(16, w) - 3
            p = abs((n // 2) % (2 * laenge) - laenge)
            return [("┄" * p, self.DUNKEL), (chip_text, chip), ("┄" * (laenge - p), self.DUNKEL)]
        ziel = round(min(self.wert / self.gesamt, 1) * laenge)
        self.pos = ziel if ziel < self.pos else min(ziel, self.pos + 1)  # gleitet, nie vor dem echten Fortschritt
        zellen = [("━", self.BLAU)] * self.pos
        for anteil, fach in self.marken:
            i = min(int(anteil * laenge), self.pos - 1)
            if i >= 0:
                zellen[i] = ("■", self._farbe(fach))
        return [*zellen, (chip_text, chip), ("┄" * (laenge - self.pos), self.DUNKEL)]

    def _z3(self, sp):
        kante = self._kante()
        hinweis = (self.hinweis, self.GRAU) if self.hinweis else ("", "")
        if not self.faecher:
            return self._kuerzen([("   ", ""), hinweis], sp - 1)
        for ohne_null, mit_hinweis in ((False, True), (True, True), (True, False)):
            teile = [("   Neu", self.GRAU)]
            for f in self.faecher:
                k = self.funde.get(f, 0) + self.tests_neu.get(f, 0)
                if k or not ohne_null:
                    teile += [("  ■", self._farbe(f)), (f" {f} ", ""), (str(k), self.FETT if k else self.GRAU)]
            breite = sum(len(_sauber(x)) for x, _ in teile)
            h = len(self.hinweis) if mit_hinweis and self.hinweis else 0
            if breite + (h + 2 if h else 0) <= kante:
                if h:
                    teile += [(" " * (kante - breite - h), ""), hinweis]
                return self._kuerzen(teile, sp - 1)
        return self._kuerzen(teile, sp - 1)

    # ---------- Zustand ----------
    def schritt(self, titel, gesamt=0, hinweis="", frist=0, faecher=None):
        """Neuer Abschnitt im Live-Bereich (Spur, wenn gesamt > 0, sonst pendelt der Chip); None räumt ihn weg."""
        with self.lock:
            self.titel, self.gesamt, self.wert, self.hinweis, self.frist = titel, gesamt, 0, hinweis, frist
            self.faecher = list(faecher or [])
            if self.faecher:
                self.alle_faecher = list(dict.fromkeys(self.alle_faecher + self.faecher))
            self.ort = self.tun = ""
            self.pos, self.marken, self.seit, self.pause_bis, self.pause_dauer = 0, [], time.monotonic(), 0, 0
            self._zeichnen()

    def jetzt(self, tun, ort=None):
        """Was gerade passiert (Z2); `ort` bleibt stehen, bis ein neuer kommt."""
        self.tun = tun
        if ort is not None:
            self.ort = ort
        self.pause_bis = 0

    def pause(self, ms):
        self.pause_dauer, self.pause_bis = ms / 1000, time.monotonic() + ms / 1000
        self.z["pausen"] += 1
        self.z["pause_s"] += ms / 1000

    def plus(self):
        with self.lock:
            self._verdeckt_zeile()
            self.wert += 1
            self.pro_stelle = 0

    # ---------- Logbuch ----------
    def _rechts(self, teile, rechts):
        """Bleibende Zeile, `rechts` (grau) an der Kante; passt beides nicht, kommt die Zeile ohne rechten Teil."""
        kante = self._kante()
        s, b = self._kuerzen(teile, 10_000)
        if rechts and b + 2 + len(rechts) <= kante:
            s += " " * (kante - b - len(rechts)) + f"{self.GRAU}{rechts}{self.AUS}"
        self.write(s + "\n")

    def zeile(self, text, grau="", zeichen="✓", farbe=None):
        if self.an:
            self._rechts([("  ", ""), (zeichen, farbe or self.GRUEN), (" " + text, ""), (" · " + grau if grau else "", self.GRAU)],
                         uhr(time.monotonic() - self.start))

    def ok(self, text, grau=""):
        """Abschnitt fertig: Bereich weg, eine bleibende ✓-Zeile. Nur in der Konsole, das Log bleibt wie bisher."""
        with self.lock:
            self._verdeckt_zeile()
            self.titel = None
            self.zeile(text, grau)

    def melden(self, text, konsole=None, grau=""):
        """✓-Zeile in der Konsole, im Log der Text wie bisher."""
        self.ok(konsole or text.rstrip("."), grau) if self.an else print(text, flush=True)

    def fehler(self, text, log=""):
        if self.an:
            with self.lock:
                self.titel = None
                self.zeile(text, zeichen="✗", farbe=self.ROT)
        else:
            print(text, log, flush=True)  # Log Zeichen für Zeichen wie bisher

    def warnung(self, text, konsole=None, zaehlt=False):
        """Etwas ging nicht, der Lauf geht weiter. `zaehlt`: kommt in der Karte als Stelle, die nicht ging."""
        if not self.an:
            return print(text, flush=True)
        self.z["fehler"] += zaehlt
        self.write(f"    {self.ROT}!{self.AUS} {_sauber(konsole or text)}\n")

    def kurs(self, fach, typ, titel, ordner, dokumente, tests):
        if self.gesamt:
            self.marken.append(((self.wert + 0.5) / self.gesamt, fach))
        if not self.an:
            return
        info = " · ".join(anzahl(k, w, ww) for k, w, ww in
                          ((ordner, "Ordner", "Ordner"), (dokumente, "Dokument", "Dokumente"), (tests, "Test", "Tests")) if k) or "keine Unterlagen"
        vorne = [("    ", ""), ("■", self._farbe(fach)), (f" {_sauber(fach):<13} ", ""), (f"{typ or '':<3} ", self.FETT)]
        tw = max(10, min(44, self._kante() - sum(len(x) for x, _ in vorne) - 2 - len(info)))
        titel = _sauber(titel)
        titel = titel if len(titel) <= tw else titel[:tw - 1] + "…"
        self._rechts([*vorne, (titel, "")], info)

    def fund(self, fach, was, art="", groesse_b=None, name=None):
        """Neue Datei (art leer) oder neuer/geänderter Test: Zeile im Logbuch, +1 auf dem Chip, Marke auf der Spur."""
        with self.lock:
            if art:
                self.tests_neu[fach] = self.tests_neu.get(fach, 0) + 1
                self.testnamen.append(f"{name or was} ({fach})")
            else:
                self.funde[fach] = self.funde.get(fach, 0) + 1
                self.z["bytes"] += groesse_b or 0
                if groesse_b and (not self.groesster or groesse_b > self.groesster[1]):
                    self.groesster = (was.rsplit(" / ", 1)[-1], groesse_b)
            t = time.monotonic()
            self.pop_n = self.pop_n + 1 if t < self.pop_bis else 1
            self.pop_bis = t + 1.2
            if self.gesamt:
                self.marken.append((min(self.wert + 0.5, self.gesamt) / self.gesamt, fach))
            if not self.an:
                return
            self.pro_stelle += 1
            if self.pro_stelle > 3 and not art:  # höchstens 3 Zeilen je Ordner, der Rest als Sammelzeile
                v = self.verdeckt or {"fach": fach, "stelle": was.split(" / ")[0], "n": 0, "bytes": 0}
                v["n"] += 1
                v["bytes"] += groesse_b or 0
                self.verdeckt = v
                return
            self._rechts([("    ", ""), ("+", self._farbe(fach) + self.FETT), (f" {fach:<13} ", ""), (was, "")],
                         art or (groesse(groesse_b) if groesse_b else ""))

    def _verdeckt_zeile(self):
        v, self.verdeckt = self.verdeckt, None
        if v and self.an:
            self._rechts([("    ", ""), ("+", self._farbe(v["fach"]) + self.FETT), (f" {v['fach']:<13} ", ""),
                          (f"… und {v['n']} weitere in {v['stelle']}", "")], groesse(v["bytes"]) if v["bytes"] else "")

    def hoch(self, pfad, geschickt):
        self.geschickt.append((pfad, geschickt))
        self.plus()

    # ---------- Schluss ----------
    def karte(self, vorher, stand):
        """Schlusskarte: Kennzahlen, je Fach, ein Abzeichen, Statistik, nächster automatischer Lauf."""
        if not self.an:
            return
        with self.lock:
            self.schritt(None)
            dateien, tests = sum(self.funde.values()), sum(self.tests_neu.values())
            g, aus = self.CHIP.format(227), self.AUS
            haken = "\x1b[38;5;26m▄▀ " if self.alt else "\x1b[38;5;26m✓  "
            logo = [[(f"{g}      {aus}", "LOGO")], [(f"{g}  L{haken}{aus}", "LOGO")], [(f"{g}      {aus}", "LOGO")]]
            if dateien or tests:
                zwei = " · ".join(x for x in (dateien and ("1 neue Unterlage" if dateien == 1 else f"{dateien} neue Unterlagen"),
                                              tests and (f"{tests} Test" + ("s" if tests > 1 else "") + " neu oder geändert"),
                                              self.z["bytes"] and groesse(self.z["bytes"])) if x)
                drei = "Auf dem iPad die App neu laden, dann ist alles da."
            else:
                zwei, drei = f"Nichts Neues in OLAT · {anzahl(self.z['gesehen'], 'Datei', 'Dateien')} geprüft", "Deine App ist schon auf dem neuesten Stand."
            innen = [[], [*logo[0], ("   ", ""), (f"Fertig in {uhr(time.monotonic() - self.start)}", self.FETT)],
                     [*logo[1], ("   ", ""), (zwei, "")], [*logo[2], ("   ", ""), (drei, self.GRAU)], []]
            if dateien or tests:
                for f in self.alle_faecher:
                    k, tt = self.funde.get(f, 0), self.tests_neu.get(f, 0)
                    perlen = ("■" * min(k + tt, 10) + ("+" if k + tt > 10 else "")) if k + tt else "·"
                    text = " · ".join(x for x in (k and f"{k} neu", tt and (f"{tt} Test" + ("s" if tt > 1 else ""))) if x) or "nichts Neues"
                    innen.append([(f"{f:<12} ", ""), (f"{perlen:<12}", self._farbe(f) if k + tt else self.DUNKEL), (" " + text, self.GRAU)])
                innen.append([])
            abzeichen = self._abzeichen(vorher, dateien, tests)
            innen += [[(f" {abzeichen[0]} ", self.CHIP.format(227)), ("  " + abzeichen[1], self.GRAU)], [],
                      [(f"{self.z['stellen']} Ordner und Dokumente · {anzahl(self.z['gesehen'], 'Datei', 'Dateien')} geprüft · "
                        f"{anzahl(self.z['pausen'], 'Pause', 'Pausen')}, zusammen {uhr(self.z['pause_s'])}", self.GRAU)]]
            if self.z["fehler"]:
                k = self.z["fehler"]
                innen.append([("! ", self.ROT), (f"{k} Stelle{'n' if k > 1 else ''} ging{'en' if k > 1 else ''} nicht, "
                                                 "der nächste Abgleich versucht es wieder", "")])
            if stand.get("naechster") and not (BASIS / ".git").exists():
                d = datetime.fromisoformat(stand["naechster"])
                if d.hour < 7:
                    d = d.replace(hour=7, minute=0)
                elif d.hour >= 23 or (sys.platform == "win32" and d.hour == 22 and d.minute > 30):  # Aufgabe prüft nur zur vollen und halben Stunde
                    d = (d + timedelta(days=1)).replace(hour=7, minute=0)
                innen.append([(f"Von selbst wieder: {wann(d)} ab {d:%H:%M}, wenn der {GERAET} an ist", self.GRAU)])
            innen.append([])
            breite = min(max(sum(6 if f == "LOGO" else len(_sauber(x)) for x, f in z) for z in innen),
                         shutil.get_terminal_size().columns - 11)  # Rahmen und Ränder: 10 Zellen, nie umbrechen
            zeilen = [f"  {self.BLAU}╭{'─' * (breite + 6)}╮{aus}"]
            for z in innen:
                s, b = self._karten_inhalt(z, breite)
                zeilen.append(f"  {self.BLAU}│{aus}   {s}{' ' * (breite - b)}   {self.BLAU}│{aus}")
            zeilen.append(f"  {self.BLAU}╰{'─' * (breite + 6)}╯{aus}")
            self.write("\n" + "\n".join(zeilen) + "\n")
            self.osc = None
            fertig = f"fertig, {dateien + tests} neu" if dateien or tests else "nichts Neues"
            self._raus(f"\x1b]0;Lernhelfer · {fertig}\x07")

    def _karten_inhalt(self, z, platz):
        """Eine Kartenzeile; ein Teil mit Farbe "LOGO" ist das fertige Logo-Feld (6 Zellen)."""
        s, b = "", 0
        for x, f in z:
            if f == "LOGO":
                if b + 6 <= platz:
                    s, b = s + x, b + 6
            else:
                t, w = self._kuerzen([(x, f)], platz - b)
                s, b = s + t, b + w
        return s, b

    def _abzeichen(self, vorher, dateien, tests):
        if not vorher.get("zeit"):
            return "Erster Fang", (f"dein Semester ist jetzt in der App: {anzahl(self.z['gesehen'], 'Datei', 'Dateien')} aus "
                                   f"{self.z['stellen']} Ordnern und Dokumenten")
        if dateien >= 10:
            return "Volle Tasche", f"{dateien} neue Dateien in einem Lauf"
        if tests:
            return "Testleser", ", ".join(self.testnamen)
        if dateien and self.groesster:
            return "Größter Fund", f"{self.groesster[0]}, {groesse(self.groesster[1])}"
        if dateien:
            return "Neues dabei", f"{dateien} neue Unterlage" + ("n" if dateien > 1 else "")
        t = datetime.fromisoformat(vorher["zeit"])
        return "Auf dem Stand", f"nichts Neues seit {wann(t)} {t:%H:%M}. Mehr Zeit zum Lernen."

    def schluss(self):
        """Programmende (auch Abbruch, Strg+C): Bereich weg, bei Abbruch eine ✗-Zeile, Cursor und Taskleiste zurück."""
        with self.lock:
            titel, ort = self.titel, self.ort
            self.titel = None
            self._weg()
            if titel or self.von_hand:
                self._raus(f"  {self.ROT}✗{self.AUS} Abgebrochen" + (f" bei {titel}" if titel else "") + (f" · {_sauber(ort)}" if ort and titel else "")
                           + f" · {uhr(time.monotonic() - self.start)}\n\x1b]0;Lernhelfer · abgebrochen\x07")
            if self.von_hand:
                self._raus(f"    {self.GRAU}Nichts kaputt: Schon Geladenes bleibt am {GERAET} und kommt beim nächsten Abgleich in die App.{self.AUS}\n")
            self._raus("\x1b[?25h" + ("\x1b]9;4;0;0\x07" if self.leiste else ""))
            self.raus.flush()


ANZEIGE = Anzeige()


def natur(s):
    return [int(t) if t.isdigit() else t.lower() for t in re.split(r"(\d+)", s)]


ZIP_MAX = 50_000_000  # entpackte Bytes Text in einer .docx/.pptx; echte Foliensätze haben ein paar MB


def text(f):
    endung = f.suffix.lower()
    if endung == ".pdf":
        from pypdf import PdfReader
        return "\n".join(f"[Seite {i}]\n{p.extract_text() or ''}" for i, p in enumerate(PdfReader(f).pages, 1))  # Marken: die Ausgabe nennt ihre Quelle
    if endung in (".docx", ".pptx"):
        z = zipfile.ZipFile(f)
        teile = sorted((n for n in z.namelist() if re.fullmatch(r"word/document\.xml|ppt/slides/slide\d+\.xml", n)), key=natur)
        if sum(z.getinfo(n).file_size for n in teile) > ZIP_MAX:  # Zip-Bombe aus einem Gruppenordner: nicht entpacken
            return "(Datei zu groß zum Auslesen)"
        xml = "\n".join((f"[Folie {i}]\n" if n.startswith("ppt/") else "") + z.read(n).decode("utf-8") for i, n in enumerate(teile, 1))
        return html.unescape(re.sub(r"<[^>]+>", "", re.sub(r"</[aw]:p>", "\n", xml)))
    if endung in (".html", ".htm", ".txt"):
        roh = re.sub(r"(?s)<(script|style)\b.*?</\1>", "", f.read_text("utf-8", errors="ignore"))
        return html.unescape(re.sub(r"<[^>]+>", " ", roh))
    return ""  # Bilder, Videos usw. haben keinen Text


def fach_json(fach, hochgeladen=()):
    """Alle geladenen Dateien eines Fachs samt Text, Folien zuerst; in der App Hochgeladenes kommt unverändert ans Ende."""
    dateien = []
    alle = [(q, f) for q in sorted(KURSE[fach], key=lambda q: ARTEN.index(q["art"]))
            for f in sorted((MATERIAL / fach / q["name"]).iterdir() if (MATERIAL / fach / q["name"]).exists() else [], key=lambda f: natur(f.name))
            if not f.name.endswith(".part")]  # halbe Downloads (speichern_als) sind keine Unterlagen
    for i, (q, f) in enumerate(alle, 1):
        ANZEIGE.jetzt(f"liest den Text aus „{f.name}“ ({i} von {len(alle)})", fach)
        ANZEIGE.z["gelesen"] += 1
        try:
            t = text(f)
        except Exception as e:  # kaputte Datei soll den Rest nicht aufhalten
            t = f"(Text nicht lesbar: {e})"
        dateien.append({"datei": f"{q['name']}/{f.name}", "art": q["art"], "text": t})
    for f in sorted((MATERIAL / fach / TESTS).glob("*.txt"), key=lambda f: natur(f.name)):
        dateien.append({"datei": f"{TESTS}/{f.name}", "art": "test", "text": f.read_text("utf-8")})
    if fach == "Mathematik" and "mathe4wiwi" in zusatzquellen():
        dateien += mathe4wiwi(datetime.now())
    ANZEIGE.plus()
    return {"dateien": dateien + list(hochgeladen)}  # ohne Zeitstempel: unveränderte Fächer werden nicht neu hochgeladen


QUELLEN = BASIS / "quellen.json"  # nur von Hand angelegt, etwa ["mathe4wiwi"]: privat, das Einrichten verteilt sie nie


def zusatzquellen():
    try:
        return json.loads(QUELLEN.read_text("utf-8")) if QUELLEN.exists() else []
    except ValueError:
        return []


def mathe4wiwi(jetzt):
    """Wochenseiten von mathe4wiwi.org (Mathematik für Wiwi an der LFU: Überblick, Quiz mit Lösungen, PS-Übungsaufgabe, Video-Links).
    Lizenz CC BY-NC-SA 4.0: nur nicht-kommerziell, darum nur über quellen.json auf dem eigenen PC, nie für zahlende Konten.
    Nimmt die Wochen rund um die aktuelle Semesterwoche (Woche 1 = erster Montag im Oktober bzw. März)."""
    erster = datetime(jetzt.year if jetzt.month >= 3 else jetzt.year - 1, 10 if jetzt.month >= 10 or jetzt.month <= 2 else 3, 1)
    erster += timedelta(days=-erster.weekday() % 7)
    woche = (jetzt - erster).days // 7 + 1
    dateien = []
    for n in range(max(1, woche - 2), min(14, woche + 1) + 1):
        url = f"https://www.mathe4wiwi.org/video/uibk{n:02d}.html"
        try:
            roh = urlopen(Request(url, headers={"User-Agent": "Lernhelfer (private Nutzung)"}), timeout=30).read().decode("utf-8", "ignore")
        except Exception as e:  # ponytail: fehlt die Seite oder ist die Seite down, fehlt diese Woche eben
            logging.info("mathe4wiwi Woche %s: %s", n, e)
            continue
        videos = re.findall(r'src="https://www\.youtube\.com/embed/([\w-]{11})"[^>]*title="([^"]*)"', roh)
        roh = re.sub(r"(?s)<(script|style|nav|header|footer)\b.*?</\1>", "", roh)
        text = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", roh))).strip()
        dateien.append({"datei": f"mathe4wiwi/Woche {n:02d}.html", "art": "folien",
                        "text": f"Zusatzquelle mathe4wiwi.org, Woche {n} ({url}); Lizenz CC BY-NC-SA 4.0, A. Zeileis, B. Rudloff, R. Stauffer.\n{text}\n"
                                + "Videos dieser Woche: " + "; ".join(f"{t}: https://www.youtube.com/watch?v={v}" for v, t in videos)})
    return dateien


def faellig(stand, jetzt):
    """Automatischer Abgleich wie von Hand: nur zwischen 7 und 23 Uhr und erst ab dem nächsten Termin (siehe weiter())."""
    return 7 <= jetzt.hour < 23 and jetzt >= datetime.fromisoformat(stand.get("naechster", "2000-01-01"))


def weiter(stand, stunden):
    """Nächster automatischer Abgleich in `stunden` plus bis zu 45 Minuten Zufall, damit es nicht wie ein Uhrwerk aussieht."""
    stand["naechster"] = (datetime.now() + timedelta(hours=stunden, minutes=random.randint(0, 45))).isoformat(timespec="minutes")


def pause(page):
    """2 bis 5 Sekunden zwischen zwei Seiten, wie beim Klicken von Hand; schont OLAT."""
    ms = random.uniform(2000, 5000)
    ANZEIGE.pause(ms)
    page.wait_for_timeout(ms)


class Abgelaufen(Exception):
    pass


# Automatischer Login (Schalter in der App): Kennung, Passwort und 2FA-Schlüssel liegen nur auf diesem Computer, verschlüsselt
# mit dem Windows-Konto (DPAPI) bzw. im Schlüsselbund des Macs; nie in zugang.json, im Stand, im Log oder im Daten-Repo.
SSO = OLAT + "/Shibboleth.sso/Login?SAMLDS=1&target=https://lms.uibk.ac.at/shib/&entityID=https%3A%2F%2Fidp.uibk.ac.at%2Fidp%2Fshibboleth"
IDP = "idp.uibk.ac.at"
CODEFELD = ("input:visible:not([type=hidden]):not([type=checkbox]):not([type=radio]):not([type=submit]):not([type=button])"
            ":not(#username):not(#password)")
LOGIN = BASIS / ".olat-login"  # Windows: DPAPI-verschlüsselt, nur dieses Windows-Konto kann es lesen
SCHLUESSELBUND = "Lernhelfer OLAT"  # Mac: Eintrag im Anmelde-Schlüsselbund


def totp(geheim, t=None):
    """Der 6-stellige Code der Authenticator-App: TOTP nach RFC 6238 (SHA-1, 30 Sekunden), wie an der Uni Innsbruck."""
    g = re.sub(r"[\s=-]", "", geheim).upper()
    h = hmac.new(base64.b32decode(g + "=" * (-len(g) % 8)), struct.pack(">Q", int((time.time() if t is None else t) // 30)), "sha1").digest()
    o = h[-1] & 15
    return f"{(int.from_bytes(h[o:o + 4], 'big') & 0x7FFFFFFF) % 1_000_000:06d}"


def _dpapi(daten, verschluesseln):
    import ctypes
    from ctypes import wintypes

    class Blob(ctypes.Structure):
        _fields_ = [("n", wintypes.DWORD), ("p", ctypes.POINTER(ctypes.c_char))]
    puffer = ctypes.create_string_buffer(daten, len(daten))
    rein, raus = Blob(len(daten), ctypes.cast(puffer, ctypes.POINTER(ctypes.c_char))), Blob()
    crypt = ctypes.WinDLL("crypt32")
    f = crypt.CryptProtectData if verschluesseln else crypt.CryptUnprotectData
    if not f(ctypes.byref(rein), None, None, None, None, 1, ctypes.byref(raus)):  # 1 = nie mit einem Dialog nachfragen
        raise ctypes.WinError()
    try:
        return ctypes.string_at(raus.p, raus.n)
    finally:
        frei = ctypes.WinDLL("kernel32").LocalFree
        frei.argtypes = [ctypes.c_void_p]
        frei(raus.p)


def _bund_lesen(dienst):
    """Mac: Base64-Wert aus dem Anmelde-Schlüsselbund oder None."""
    r = subprocess.run(["security", "find-generic-password", "-s", dienst, "-w"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def _bund_setzen(dienst, wert):
    """Mac: Befehl über stdin an `security -i`, damit der Wert nie in der Prozessliste (ps) steht; Base64 braucht keine Anführungszeichen."""
    subprocess.run(["security", "-i"], input=f'add-generic-password -U -s "{dienst}" -a lernhelfer -w {wert}\n', text=True, capture_output=True)
    if _bund_lesen(dienst) != wert:  # ponytail: ungetestet auf echtem Mac; klappt -i nicht, wie bisher über die Argumente (kurz in ps sichtbar)
        subprocess.run(["security", "add-generic-password", "-U", "-s", dienst, "-a", "lernhelfer", "-w", wert], check=True, capture_output=True)


def login_lesen():
    try:
        if sys.platform == "win32" and LOGIN.exists():
            return json.loads(_dpapi(LOGIN.read_bytes(), False))
        if sys.platform == "darwin":
            wert = _bund_lesen(SCHLUESSELBUND)
            return json.loads(base64.b64decode(wert)) if wert else None
    except (OSError, ValueError):
        return None
    return None


def login_speichern(login):
    roh = json.dumps(login).encode()
    if sys.platform == "win32":
        LOGIN.write_bytes(_dpapi(roh, True))
    else:
        _bund_setzen(SCHLUESSELBUND, base64.b64encode(roh).decode())


# OLAT-Sitzung (Cookies): Windows verschlüsselt sie mit DPAPI. Am Mac ist sie zu groß für den Schlüsselbund; dort liegt nur ein
# zufälliger Schlüssel darin, die Datei wird damit versiegelt. Python hat kein AES eingebaut, darum HMAC-SHA256 als Schlüsselstrom
# (Zählermodus) und danach HMAC über alles (Encrypt-then-MAC).
# ponytail: Eigenbau aus Standard-Bausteinen; auf AES-GCM (Paket cryptography) umstellen, falls das Paket einmal ohnehin dazukommt.
SCHLUESSELBUND_SITZUNG = "Lernhelfer OLAT Sitzung"


def _teilschluessel(schluessel):
    return hmac.new(schluessel, b"lernhelfer-enc", "sha256").digest(), hmac.new(schluessel, b"lernhelfer-mac", "sha256").digest()


def _strom(schluessel, nonce, daten):
    s = b"".join(hmac.new(schluessel, nonce + i.to_bytes(8, "big"), "sha256").digest() for i in range(-(-len(daten) // 32)))
    return (int.from_bytes(daten, "big") ^ int.from_bytes(s[:len(daten)], "big")).to_bytes(len(daten), "big")


def versiegeln(schluessel, daten):
    ke, km = _teilschluessel(schluessel)
    nonce = os.urandom(16)
    c = _strom(ke, nonce, daten)
    return nonce + c + hmac.new(km, nonce + c, "sha256").digest()


def entsiegeln(schluessel, blob):
    ke, km = _teilschluessel(schluessel)
    nonce, c, tag = blob[:16], blob[16:-32], blob[-32:]
    if len(blob) < 48 or not hmac.compare_digest(tag, hmac.new(km, nonce + c, "sha256").digest()):
        raise ValueError("Sitzung verändert oder mit einem anderen Schlüssel versiegelt")
    return _strom(ke, nonce, c)


def _sitzungsschluessel():
    wert = _bund_lesen(SCHLUESSELBUND_SITZUNG)
    if wert:
        return base64.b64decode(wert)
    neu = os.urandom(32)
    _bund_setzen(SCHLUESSELBUND_SITZUNG, base64.b64encode(neu).decode())
    return neu


def sitzung_lesen():
    """Gespeicherte OLAT-Anmeldung für Playwright (storage_state) oder None, dann wird neu eingeloggt."""
    try:
        if SITZUNG.exists():
            roh = SITZUNG.read_bytes()
            return json.loads(_dpapi(roh, False) if sys.platform == "win32" else entsiegeln(_sitzungsschluessel(), roh))
        if SITZUNG_ALT.exists():
            return json.loads(SITZUNG_ALT.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        pass
    return None


def sitzung_speichern(ctx):
    """Nie im Klartext: Klappt das Verschlüsseln nicht (z. B. Schlüsselbund gesperrt), bleibt die Sitzung ungespeichert."""
    roh = json.dumps(ctx.storage_state()).encode()
    neu = SITZUNG.with_name(SITZUNG.name + ".neu")  # abgebrochen mitten im Schreiben: die alte Anmeldung bleibt ganz
    try:
        neu.write_bytes(_dpapi(roh, True) if sys.platform == "win32" else versiegeln(_sitzungsschluessel(), roh))
        os.replace(neu, SITZUNG)
    except (OSError, subprocess.CalledProcessError) as e:
        logging.info("Sitzung nicht gespeichert: %s", e)
    SITZUNG_ALT.unlink(missing_ok=True)


def login_loeschen():
    LOGIN.unlink(missing_ok=True)
    if sys.platform == "darwin":
        subprocess.run(["security", "delete-generic-password", "-s", SCHLUESSELBUND], capture_output=True)


def auf_idp(url):
    """Nur die echte Login-Seite der Uni bekommt Passwort und Code: genau dieser Host, nur HTTPS (nicht „…idp.uibk.ac.at…“ irgendwo in der Adresse)."""
    u = urlparse(url)
    return u.scheme == "https" and u.hostname == IDP


def auto_einloggen(seite, login):
    """Einloggen wie von Hand: Kennung und Passwort auf idp.uibk.ac.at, dann der 2FA-Code. None = geklappt, sonst der Grund.
    Ausnahmen (Netz, Zeitüberschreitung) gehen an den Aufrufer: Dabei wurde nichts Falsches abgeschickt."""
    ANZEIGE.jetzt("Sitzung abgelaufen, ich logge dich selbst ein", "Uni-Login")
    seite.goto(SSO)
    if auf_idp(seite.url):  # sonst kennt der IdP die Anmeldung noch und schickt gleich zurück zu OLAT
        if not seite.locator("#password").count():
            return "die Login-Seite der Uni sah anders aus als erwartet"
        ANZEIGE.jetzt("Kennung und Passwort")
        seite.fill("#username", login["benutzer"])
        seite.fill("#password", login["passwort"])
        with seite.expect_navigation():
            seite.click("button[name=_eventId_proceed]")
        for _ in range(30):  # bis zu 15 s auf die Code-Seite warten
            if not auf_idp(seite.url) or seite.locator(CODEFELD).count() or seite.locator("#password:visible").count():
                break
            seite.wait_for_timeout(500)
        if auf_idp(seite.url):
            if seite.locator("#password:visible").count():
                return "Benutzerkennung oder Passwort stimmt nicht"
            # ponytail: nimmt das erste sichtbare Eingabefeld; fragt die Uni zuerst nach Security Key/Hello-PIN, gibt es keins
            feld = seite.locator(CODEFELD).first
            if not feld.count():
                return "die 2FA-Seite der Uni sah anders aus als erwartet"
            if 30 - time.time() % 30 < 3:  # keinen Code abschicken, der gleich abläuft
                ANZEIGE.jetzt("wartet 3 s auf einen frischen 2FA-Code")
                seite.wait_for_timeout(3500)
            ANZEIGE.jetzt("trägt den 2FA-Code ein")
            feld.fill(totp(login["geheim"]))
            feld.press("Enter")
            try:
                seite.wait_for_url(lambda u: not auf_idp(u), timeout=30_000)
            except Exception:
                return "der 2FA-Code wurde nicht angenommen (2FA-Schlüssel prüfen)"
    ANZEIGE.jetzt("zurück zu OLAT")
    seite.goto(OLAT + "/auth/MyCoursesSite/0")
    return None if seite.get_by_text(EINGELOGGT).count() else "OLAT hat die Anmeldung nicht angenommen"


def profil(p, unsichtbar):
    """Das eigene Lernhelfer-Profil (Cookies der Uni-Anmeldung bleiben darin), sichtbar zum Einloggen von Hand."""
    return p.chromium.launch_persistent_context(PROFIL, channel=KANAL, headless=unsichtbar, handle_sigint=False,  # Strg+C: Python räumt auf
                                                ignore_default_args=["--enable-automation", "--disable-extensions"])


ZUSTIMMUNG = 1  # Fassung der Hinweise, die man in der App abhakt (ZUSTIMMUNG in index.html)


def schalter():
    """Schalter „Automatisch bei OLAT einloggen“ in der App (olat.json im eigenen Speicher). An nur, wenn er an ist und die
    Hinweise in der aktuellen Fassung abgehakt sind; ohne Datei aus. None = keine Antwort."""
    try:
        d = json.loads(api("GET", "olat.json", roh=True))
        return d.get("autologin") is True and type(d.get("zustimmung")) is int and d["zustimmung"] >= ZUSTIMMUNG
    except HTTPError as e:
        return False if e.code == 404 else None
    except Exception:
        return None


def zwischenablage():
    """Text aus der Windows-Zwischenablage, leer wenn keiner drin ist."""
    import ctypes
    u, k = ctypes.WinDLL("user32"), ctypes.WinDLL("kernel32")
    u.GetClipboardData.restype = k.GlobalLock.restype = ctypes.c_void_p
    k.GlobalLock.argtypes = k.GlobalUnlock.argtypes = [ctypes.c_void_p]
    if not u.OpenClipboard(None):
        return ""
    try:
        h = u.GetClipboardData(13)  # CF_UNICODETEXT
        z = k.GlobalLock(h) if h else None
        if not z:
            return ""
        try:
            return ctypes.wstring_at(z)
        finally:
            k.GlobalUnlock(h)
    finally:
        u.CloseClipboard()


def verdeckt(frage):
    """Wie getpass, aber jedes Zeichen als * (man sieht, dass das Einfügen geklappt hat), Tab zeigt/versteckt den Text,
    und Strg+V fügt auch in der alten Windows-Konsole ein (die gibt es dort nur als Steuerzeichen ^V weiter)."""
    if not (sys.stdin and sys.stdin.isatty()):
        return getpass.getpass(frage)
    win = sys.platform == "win32"
    if win:
        import msvcrt
        taste, zurueck = msvcrt.getwch, lambda: None
    else:
        import termios
        import tty
        fd = sys.stdin.fileno()
        vorher = termios.tcgetattr(fd)
        tty.setcbreak(fd)  # Taste für Taste, ohne Echo; Strg+C geht weiter
        taste, zurueck = lambda: sys.stdin.read(1), lambda: termios.tcsetattr(fd, termios.TCSADRAIN, vorher)
    text, zeigen = "", False
    try:
        while True:
            # ponytail: \r malt nur die letzte Zeile neu; ist Frage + Text breiter als das Fenster, bleibt oben ein Rest stehen
            sys.stdout.write(f"\r{frage}{text if zeigen else '*' * len(text)}\x1b[K")
            sys.stdout.flush()
            c = taste()
            if c in ("", "\r", "\n"):
                break
            if c == "\x03":
                raise KeyboardInterrupt
            if win and c in "\x00\xe0":  # Pfeil- und F-Tasten kommen als zwei Zeichen
                taste()
            elif c == "\t":
                zeigen = not zeigen
            elif c in "\b\x7f":
                text = text[:-1]
            elif c == "\x16" and win:
                text += "".join(z for z in zwischenablage().strip().split("\n")[0] if z.isprintable())
            elif c.isprintable():
                text += c
    finally:
        zurueck()
        sys.stdout.write(f"\r{frage}{'*' * len(text)}\x1b[K\n")  # nichts Lesbares bleibt im Fenster stehen
        sys.stdout.flush()
    return text


def autologin_einrichten(p, stand):
    """Einmal von Hand: Zugangsdaten abfragen, mit einem echten Login prüfen und erst dann verschlüsselt ablegen."""
    if sys.platform not in ("win32", "darwin"):
        print("Den automatischen Login gibt es nur unter Windows und macOS.")
        return None
    a = ANZEIGE
    f, b, gr, r, g, x = (a.FETT, a.BLAU, a.GRUEN, a.ROT, a.GRAU, a.AUS) if a.an else ("",) * 6
    print(f"""
  {f}Automatischer OLAT-Login{x}
  {g}Der Lernhelfer loggt sich dann selbst bei OLAT ein, auch mit dem 2FA-Code.{x}

  {b}Sicherheit{x}
    {gr}✓{x} Kennung, Passwort und 2FA-Schlüssel bleiben verschlüsselt auf diesem {GERAET}
      {g}({'mit deinem Windows-Konto' if sys.platform == 'win32' else 'im Schlüsselbund'}) und gehen nirgendwohin.{x}
    {r}!{x} Wer an deinen entsperrten {GERAET} kommt, kommt auch in deinen Uni-Account.
      {g}Darum den {GERAET} immer mit Passwort sperren.{x}
    {r}!{x} Notbremse, z. B. {GERAET} gestohlen: im Account Portal den Token löschen,
      {g}den du gleich anlegst, oder das Uni-Passwort ändern.
      Dann sind die gespeicherten Daten wertlos.{x}

  {b}So bekommst du den 2FA-Schlüssel{x}
    {f}1{x}  https://accounts.uibk.ac.at öffnen und einloggen
       Oben rechts auf Deutsch stellen {g}(auf Englisch fehlt der Link aus Schritt 2){x}
       Bei Zwei-Faktor-Authentifizierung auf „Verwalten“, dann „Neuer Token“
    {f}2{x}  Unter dem QR-Code auf „hier“ klicken: kopiert den geheimen Schlüssel
    {f}3{x}  Unten einfügen. Dann zeigt der Lernhelfer einen 6-stelligen Code.
       Den im Account Portal eingeben und „Verifizieren“ klicken
       {g}(nicht den Code aus der Handy-App){x}

  {g}Deine Authenticator-App am Handy funktioniert weiter wie bisher.
  Überspringen: einfach Enter drücken, dann loggst du dich wie bisher selbst ein.{x}
""")
    benutzer = input("  Benutzerkennung (z. B. csab1234): ").strip()
    if not benutzer:
        return None
    print(f"  {g}Passwort und Schlüssel erscheinen als *. Mit Tab siehst du sie, "
          f"{'Strg+V' if sys.platform == 'win32' else '⌘V'} fügt ein.{x}")
    passwort = verdeckt("  Passwort: ")
    geheim = verdeckt("  2FA-Schlüssel: ").strip()
    if geheim.startswith("otpauth://"):
        geheim = parse_qs(urlparse(geheim).query).get("secret", [""])[0]
    try:
        totp(geheim)
        assert len(re.sub(r"[\s=-]", "", geheim)) >= 16
    except (ValueError, AssertionError):
        print(f"  {r}!{x} Das sieht nicht nach einem 2FA-Schlüssel aus (Buchstaben A–Z und Ziffern 2–7). Nichts gespeichert.")
        return None
    while True:
        print(f"\n  Code fürs Account Portal:  {f}{totp(geheim)}{x}  {g}gilt noch {30 - int(time.time() % 30)} Sekunden{x}")
        antwort = input("  Angenommen (oder war er schon eingerichtet)? Dann ok tippen, sonst Enter für einen frischen Code: ")
        if antwort.strip().lower() in ("ok", "j", "ja"):
            break
    login = {"benutzer": benutzer, "passwort": passwort, "geheim": geheim}
    print(f"\n  {g}Teste den Login bei OLAT …{x}", flush=True)
    fenster = profil(p, unsichtbar=True)
    try:
        fenster.clear_cookies(domain=IDP)  # sonst kennt der IdP die Anmeldung noch und prüft gar nichts
        grund = auto_einloggen(fenster.pages[0] if fenster.pages else fenster.new_page(), login)
        if grund is None:
            sitzung_speichern(fenster)
    except Exception as e:
        grund = f"unterbrochen ({type(e).__name__})"
    finally:
        fenster.close()
    if grund:
        print(f"  {r}!{x} Das hat nicht geklappt: {grund}. Nichts gespeichert, du loggst dich wie bisher selbst ein.")
        return None
    login_speichern(login)
    stand.pop("autologin_fehler", None)
    print(f"  {gr}✓{x} Gespeichert. Ab jetzt loggt sich der Lernhelfer selbst bei OLAT ein.\n")
    return login


def autologin(p, stand, von_hand):
    """Zugangsdaten für den automatischen Login, solange der Schalter in der App an ist. Von Hand gestartet richtet er ihn
    einmal ein (bzw. nach einem Fehler neu); Schalter aus = gespeicherte Zugangsdaten löschen.
    Nach einem Fehlschlag versucht er es nicht von selbst nochmal, damit die Uni den Account nicht sperrt."""
    an = schalter() if ZUGANG.exists() else False
    stand["autologin_an"] = an = stand.get("autologin_an", False) if an is None else an
    login = login_lesen()
    if not an:
        if login:
            login_loeschen()
            print("Automatischer Login ist in der App aus: gespeicherte Zugangsdaten gelöscht.")
        stand.pop("autologin_fehler", None)
        return None
    if von_hand and sys.stdin and sys.stdin.isatty() and (not login or stand.get("autologin_fehler")):
        if stand.get("autologin_fehler"):
            print(f"Der automatische Login ging zuletzt nicht: {stand['autologin_fehler']}.")
            if input("Zugangsdaten neu eintragen? (j/n): ").strip().lower() not in ("j", "ja"):
                return None
        return autologin_einrichten(p, stand)
    return None if stand.get("autologin_fehler") else login


def olat_login(p, sichtbar=True, fenster_auf=lambda: None, login=None, login_fehler=lambda grund: None):
    """Erst unsichtbar mit gespeicherter Sitzung. Ist die abgelaufen und der automatische Login eingerichtet, loggt er sich
    unsichtbar selbst ein. Sonst öffnet sich Chrome mit eigenem Lernhelfer-Profil zum Einloggen (SSO + Authenticator);
    dort gespeicherte Passwörter füllt Chrome beim nächsten Mal selbst aus. Wartet höchstens 10 Minuten (Playwright-TimeoutError)."""
    ANZEIGE.schritt("Verbinde mit OLAT")
    ANZEIGE.jetzt(f"startet {BROWSER} unsichtbar", "OLAT")
    browser = p.chromium.launch(channel=KANAL, headless=True, handle_sigint=False)  # bei Strg+C schließt Python den Browser, nicht der Treiber mittendrin
    for versuch in (1, 2):
        if versuch == 2:
            ANZEIGE.schritt("Verbinde mit OLAT")
        ANZEIGE.jetzt("öffnet lms.uibk.ac.at", "OLAT")
        ctx = browser.new_context(storage_state=sitzung_lesen(), accept_downloads=True)
        page = ctx.new_page()
        page.goto(OLAT + "/auth/MyCoursesSite/0")
        if page.get_by_text(EINGELOGGT).count():  # OLAT zeigt die Login-Seite unter derselben URL
            if versuch == 1:
                ANZEIGE.ok("Bei OLAT eingeloggt", "deine gespeicherte Anmeldung gilt noch")
            return browser, ctx, page
        ctx.close()
        if versuch == 1 and login:
            fenster = profil(p, unsichtbar=True)
            try:
                grund = auto_einloggen(fenster.pages[0] if fenster.pages else fenster.new_page(), login)
                if grund is None:
                    sitzung_speichern(fenster)
                    ANZEIGE.melden("Automatisch bei OLAT eingeloggt.", "Automatisch eingeloggt", "Kennung, Passwort und 2FA-Code, ohne dein Handy")
                    continue
                login_fehler(grund)
            except Exception as e:  # Netz, Zeitüberschreitung: nichts Falsches abgeschickt, nächstes Mal wieder automatisch
                ANZEIGE.warnung(" ".join(["Automatischer Login unterbrochen:", type(e).__name__, str(e).split("\n")[0][:200]]))  # nur die erste Zeile, ohne Seiteninhalt
            finally:
                fenster.close()
        if versuch == 2 or not sichtbar:
            browser.close()
            raise Abgelaufen
        fenster_auf()
        fenster = profil(p, unsichtbar=False)
        seite = fenster.pages[0] if fenster.pages else fenster.new_page()
        seite.goto(OLAT + "/auth/MyCoursesSite/0")
        print(f"Bitte im {BROWSER}-Fenster bei OLAT einloggen (10 Minuten Zeit) …", flush=True)
        ANZEIGE.schritt("Warte auf deinen Login", frist=600, hinweis="Kein Stress, das Fenster wartet bis zu 10 Minuten.")
        ANZEIGE.jetzt("dort mit Uni-Kennung und Authenticator-App einloggen", f"{BROWSER}-Fenster")
        seite.get_by_text(EINGELOGGT).first.wait_for(timeout=600_000)
        ANZEIGE.melden("Eingeloggt. Das Fenster schließt sich, der Abgleich läuft unsichtbar weiter.")
        sitzung_speichern(fenster)
        fenster.close()


FACHNAMEN = {"BWL": r"betriebswirt|\bbwl\b", "VWL": r"volkswirt|\bvwl\b", "Mathematik": r"mathemati", "Statistik": r"statisti",
             "Englisch": r"englisch|english", "Recht": r"\brecht|(?<!ge)recht(s)?\b",  # Strafrecht, des Privatrechts; nicht Gerechtigkeit, aufrechte
             "Rechnungswesen": r"rechnungswesen|buchhalt", "Wirtschaftsinformatik": r"wirtschaftsinformatik"}
NICHT_LADEN = re.compile(r"video|aufzeichnung|livestream|einstieg", re.I)  # Dokument-Knoten ohne Text
FUELLE = re.compile(r"^(?:(?:einführung|grundlagen|grundzüge|grundbegriffe|überblick)\s+(?:in|zu|zur|zum|über)?\s*(?:die|das|den|der|des|dem)?\s+)+", re.I)


def semester(jetzt):
    """Semesterkürzel wie in den LFU-Kurstiteln: September 2026 bis Februar 2027 = 2026W (im September stehen schon die Kurse
    fürs Wintersemester), März bis August = S."""
    return f"{jetzt.year}S" if 3 <= jetzt.month <= 8 else f"{jetzt.year - (jetzt.month <= 2)}W"


def kursfach(titel, jetzt):
    """Fach zu einem Kurstitel aus „Meine Kurse“, None = nicht abgleichen (anderes Semester, kein Fach erkennbar)."""
    code = re.match(r"(\d{4}[WS])\d+\s+", titel)
    if code and code.group(1) != semester(jetzt):
        return None
    for fach, muster in FACHNAMEN.items():
        if re.search(muster, titel, re.I):
            return fach
    return kursname(titel) if code else None


def kursname(titel):
    """Fach nach dem Kurstitel selbst: ohne Code, LV-Art und „Einführung in die …“, nur bis zum ersten Doppelpunkt bzw.
    Gedankenstrich, höchstens vier Wörter. Wird Ordnername: „Physik I: Mechanik“ ginge unter Windows gar nicht."""
    rest = re.sub(r"^\d{4}[WS]\d+\s+(?:[A-Z]{2}\s+)?", "", titel)
    rest = FUELLE.sub("", rest) or rest  # sonst landen „Einführung in die Soziologie“ und „… die Politikwissenschaft“ im selben Fach
    rest = re.split(r":|\s[–-]\s", rest)[0]
    return re.sub(r'[\\/:*?"<>|]', "-", " ".join(rest.split()[:4])).strip(" .-–") or None


def faecher_zuordnen(kurse, jetzt, alt=None):
    """[(Kurs, Titel, Fach, LV-Art)] für die Kurse aus „Meine Kurse“. Ein Kurzname wie „Recht“ würfe im Jus-Studium alle Kurse
    in ein Fach: Steht darin eine LV-Art doppelt (zwei VO), heißen diese Kurse nach ihrem Titel. Kurse aus `alt` (bisherige
    kurse.json) behalten ihr Fach, damit Lernstand und Ausgaben in der App nicht plötzlich unter neuem Namen stehen. Gibt es das
    Kurzfach schon (etwa „VWL“), kommt ein später dazugekommener zweiter VO-Kurs dazu, statt als eigenes Fach „Volkswirtschaft“."""
    bekannt = {q["url"].split("/")[0]: f for f, liste in (alt or {}).items() for q in liste if not re.search(r'[\\/:*?"<>|]', f)}  # „Physik I: …“ ging nie
    liste = [(k, t, f, (re.match(r"\d{4}[WS]\d+\s+([A-Z]{2})\b", t) or [None, ""])[1]) for k, t in kurse if (f := kursfach(t, jetzt))]
    arten = {}
    for _, _, f, typ in liste:
        arten.setdefault(f, []).append(typ)
    doppelt = {f for f, a in arten.items() if len(a) > len(set(a))} - set(bekannt.values())
    return [(k, t, bekannt.get(k) or (f in doppelt and kursname(t)) or f, typ) for k, t, f, typ in liste]


def kurse_suchen(stand, jetzt):
    """Selbst gefundene Kurse jede Woche neu suchen: neue Kurse, neue Bausteine und Unterordner. Eine von Hand gepflegte
    kurse.json (kein „kurse_semester“ im Stand) bleibt, wie sie ist."""
    return "kurse_semester" in stand and (stand["kurse_semester"] != semester(jetzt)
                                          or jetzt - datetime.fromisoformat(stand.get("kurse_zeit", "2000-01-01")) > timedelta(days=7))


def kursart(kurs, knoten_titel):
    if re.search(r"syllabus|general information|organisator|faq", knoten_titel, re.I):
        return "syllabus"
    return "ps" if re.match(r"(\d{4}[WS]\d+\s+)?PS\b", kurs) or re.search(r"\bPS\b|proseminar|aufgabe", knoten_titel, re.I) else "folien"


def kurse_finden(page, jetzt=None, alt=None, ids=None):
    """Baut kurse.json aus „Meine Kurse → Aktiv“: Ordner (bis 3 Unterordner-Ebenen), Dokumente und Seiten oben im Kurs.
    Klausurarchiv, Foren, Abgaben, Videos und Tests (die liest tests()) bleiben draußen. `alt`: bisherige kurse.json;
    `ids` bekommt {Fach: {Kurs: LV-Art}} aller Kurse, auch solcher ohne Dateien."""
    jetzt = jetzt or datetime.now()
    ANZEIGE.jetzt("liest „Meine Kurse“", "OLAT")
    page.goto(OLAT + "/auth/MyCoursesSite/0/My/0")
    page.wait_for_load_state("networkidle")
    kurse = page.evaluate(r"""() => [...new Map([...document.querySelectorAll('#o_main_center_content_inner a[href*="RepositoryEntry/"]')]
      .filter(a => a.innerText.trim() && a.innerText.trim() !== 'Öffnen')
      .map(a => [a.href.match(/RepositoryEntry\/(\d+)/)[1], a.innerText.trim()])).entries()]""")
    neu = {}
    liste = faecher_zuordnen(kurse, jetzt, alt)
    ANZEIGE.zeile("Meine Kurse gelesen", f"{anzahl(len(kurse), 'Kurs', 'Kurse')}, {len(liste)} aus diesem Semester ({semester(jetzt)})")
    ANZEIGE.gesamt = len(liste)
    for kurs, titel, fach, typ in liste:
        neu.setdefault(fach, [])
        if ids is not None:  # auch Kurse ohne eine einzige Datei: ein PS-Kurs hat oft nur Tests und Bewertungen, die liest tests()
            ids.setdefault(fach, {})[kurs] = typ
        kurz = re.sub(r"^\d{4}[WS]\d+\s+(?:[A-Z]{2}\s+)?", "", titel)
        ANZEIGE.jetzt("liest den Kursbaum", f"{fach} · {typ} {kurz}".replace("  ", " "))
        alle = knoten(page, kurs)
        wurzel = alle[0]["titel"] if alle else None
        ordner = dokumente = 0
        for k in alle:
            if k["eltern"] in ("Klausurarchiv", "Abgaben") or not k["titel"]:
                continue
            url = f"{kurs}/CourseNode/{k['id']}"
            if k["icon"] == "o_bc_icon":
                eintraege = ordner_baum(page, url, k["titel"])
                ordner += len(eintraege)
            elif (k["icon"] is None and not NICHT_LADEN.search(k["titel"])) or (k["icon"] == "o_sp_icon" and k["eltern"] == wurzel):
                eintraege = [(k["titel"], url)]
                dokumente += 1
            else:
                continue
            for name, adresse in eintraege:
                neu.setdefault(fach, []).append({"name": re.sub(r'[\\/:*?"<>|]', "-", f"{typ} {name}".strip()),  # wird Ordnername
                                                 "art": kursart(titel, name), "url": adresse})
        ANZEIGE.kurs(fach, typ, kurz, ordner, dokumente, sum(k["icon"] in TESTICONS and k["eltern"] != "Klausurarchiv" for k in alle))
        ANZEIGE.plus()
    for fach, liste in neu.items():  # gleiche Namen (z. B. zweimal „Syllabus“) bekommen eine Nummer
        gesehen = {}
        for q in liste:
            gesehen[q["name"]] = gesehen.get(q["name"], 0) + 1
            if gesehen[q["name"]] > 1:
                q["name"] += f" {gesehen[q['name']]}"
    return neu


def ordner_baum(page, url, name, pfad=(), tiefe=3):
    """Ein Ordner und seine Unterordner bis `tiefe` Ebenen tief, jeder als eigener Eintrag [(Name, Adresse)].
    Jede LV ordnet anders (VO/Woche 3/Übung …); OLAT schreibt / im Ordnerpfad der Adresse als ~~."""
    adresse = url + (f"/path%3D~~{'~~'.join(quote(p) for p in pfad)}/0" if pfad else "")
    if not tiefe:
        return [(name, adresse)]
    ANZEIGE.jetzt(f"Ordner „{pfad[-1] if pfad else name}“ (Ebene {len(pfad) + 1} von 3)")
    pause(page)
    page.goto(f"{OLAT}/auth/RepositoryEntry/{adresse}")
    page.wait_for_load_state("networkidle")
    unter = [t.strip() for t in page.locator("#o_main_center_content_inner .o_folder_card:has(.o_filetype_folder) .o_folder_card_title").all_inner_texts()]
    return [(name, adresse)] + [x for u in unter if not NICHT_LADEN.search(u) for x in ordner_baum(page, url, f"{name} {u}", pfad + (u,), tiefe - 1)]


def oeffne(page, url):
    pause(page)
    page.goto(url)
    page.wait_for_load_state("networkidle")
    groesse = page.locator("#o_main_center_content_inner .o_table_pagination .dropdown-toggle")
    if groesse.count() and groesse.inner_text().strip() != "250":  # Ordner zeigen sonst nur 20 Dateien
        groesse.click()
        page.locator(".o_table_pagination .dropdown-menu a", has_text="250").click()
        page.locator(".o_table_pagination .dropdown-toggle", has_text="250").wait_for()


def sicherer_name(roh):
    """Dateiname aus OLAT (Kartentitel, Download, Adresse) ohne Pfad: In Gruppenordnern laden auch Mitstudierende hoch, und
    „../../holen.py“ darf nie das Programm überschreiben."""
    n = unquote(str(roh)).replace("\\", "/").rsplit("/", 1)[-1]  # nicht Path: Windows liest „a:“ als Laufwerk
    return re.sub(r'[\\/:*?"<>|\x00-\x1f]', "-", n).strip(". ")[:150] or "datei"


def fehlertext(e):
    """Erste Zeile einer Ausnahme fürs Log, ohne Seiteninhalt; leere Meldung (assert) = Name der Ausnahme."""
    return (str(e).splitlines() or [type(e).__name__])[0][:200]


def sync(ctx, page, fach):
    """Neue Dateien aller Einträge eines Fachs. Ein kaputter Eintrag (in OLAT gelöscht, Zeitüberschreitung) hält den Rest
    nicht auf, sonst käme bis zum nächsten Abgleich gar nichts in der App an."""
    neu = []
    for q in KURSE[fach]:
        ANZEIGE.jetzt("öffnet in OLAT", f"{fach} · {q['name']}")
        ANZEIGE.z["stellen"] += 1
        try:
            neu += eintrag_laden(ctx, page, fach, q)
        except Exception as e:
            ANZEIGE.warnung(f"{fach}: {q['name']} nicht geladen ({fehlertext(e)})", f"{fach} · {q['name']} nicht geladen: {grund(e)}", zaehlt=True)
        ANZEIGE.plus()
    return neu


def speichern_als(download, pfad):
    """Erst fertig laden, dann umbenennen: ein halber Download (PC geht aus) zählte sonst für immer als „schon da“."""
    teil = pfad.with_name(pfad.name + ".part")
    download.save_as(teil)
    os.replace(teil, pfad)


def eintrag_laden(ctx, page, fach, q):
    neu = []
    ziel = MATERIAL / fach / q["name"]
    ziel.mkdir(parents=True, exist_ok=True)
    url = f"{OLAT}/auth/RepositoryEntry/{q['url']}"
    oeffne(page, url)
    if not page.get_by_text(EINGELOGGT).count():  # Sitzung mitten im Lauf abgelaufen: sonst hieße es still „Nichts Neues“
        raise RuntimeError("OLAT-Sitzung abgelaufen")
    main = page.locator("#o_main_center_content_inner")
    herunterladen = main.locator("a:visible", has_text="Herunterladen")
    seite = main.locator("iframe[src*='/m/']:not([src*='viewer.html'])")  # HTML-Seite, kein PDF-Betrachter
    karten = main.locator(".o_folder_card:not(:has(.o_filetype_folder))")
    if main.locator(".o_folder_card").count():  # Ordner
        # Unterordner haben eigene Einträge (ordner_baum, bis 3 Ebenen tief);
        # vorhandene Dateien nicht neu geladen – geänderte Datei mit gleichem Namen: lokal löschen.
        namen = [n.strip() for n in karten.locator(".o_folder_card_title").all_inner_texts()]
        fehlt = [n for n in namen if not (ziel / sicherer_name(n)).exists()]
        ANZEIGE.z["gesehen"] += len(namen)
        lage = (f"{anzahl(len(namen), 'Datei', 'Dateien')}, {len(namen) - len(fehlt)} schon da" if fehlt else
                f"alle {len(namen)} Dateien schon da" if len(namen) > 1 else "die Datei ist schon da" if namen
                else "nur Unterordner, die kommen als Nächstes")
        ANZEIGE.jetzt(lage)
        for i, titel in enumerate(fehlt, 1):
            ANZEIGE.jetzt(f"{lage} · lädt {titel} ({i} von {len(fehlt)})")
            oeffne(page, url)  # OLAT lädt die Seite nach jedem Download neu
            karten.filter(has=page.get_by_text(titel, exact=True)).locator("a:has(.o_icon_actions)").click()  # Karte über den Originaltitel
            with page.expect_download() as dl:
                page.get_by_role("link", name="Herunterladen", exact=True).last.click()
            speichern_als(dl.value, ziel / sicherer_name(titel))
            neu.append(f"{fach}: {q['name']}/{sicherer_name(titel)}")
            ANZEIGE.fund(fach, f"{q['name']} / {sicherer_name(titel)}", groesse_b=bytes_von(ziel / sicherer_name(titel)))
    elif herunterladen.count():  # einzelne Datei
        ANZEIGE.z["gesehen"] += 1
        ANZEIGE.jetzt("lädt die Datei")
        with page.expect_download() as dl:
            herunterladen.first.click()
        name = sicherer_name(dl.value.suggested_filename)
        ist_neu = not (ziel / name).exists()
        if ist_neu:
            neu.append(f"{fach}: {q['name']}/{name}")
        speichern_als(dl.value, ziel / name)
        if ist_neu:
            ANZEIGE.fund(fach, f"{q['name']} / {name}", groesse_b=bytes_von(ziel / name))
    elif seite.count():  # eingebettete HTML-Seite
        ANZEIGE.z["gesehen"] += 1
        ANZEIGE.jetzt("liest die Seite")
        src = seite.first.get_attribute("src")
        name = sicherer_name(src.rsplit("/", 1)[-1])
        ist_neu = not (ziel / name).exists()
        if ist_neu:
            neu.append(f"{fach}: {q['name']}/{name}")
        r = ctx.request.get(urljoin(OLAT, src))
        if not r.ok:  # sonst landete eine Fehler- oder Login-Seite als „Syllabus“ in der App
            raise RuntimeError(f"OLAT antwortete {r.status}")
        (ziel / name).write_bytes(r.body())
        if ist_neu:
            ANZEIGE.fund(fach, f"{q['name']} / {name}", groesse_b=bytes_von(ziel / name))
    return neu


def felder(body):
    """Formularfelder einer OLAT-Anfrage (urlencoded oder multipart)."""
    if "Content-Disposition" in body:
        return dict(re.findall(r'name="([^"]+)"\r\n\r\n([^\r]*)', body))
    return {k: v[0] for k, v in parse_qs(body).items()}


def erlaubt(methode, url, body, frei):
    """Testschutz für jede Anfrage der Test-Sitzung: Lesen (GET) geht durch, sonst nur genau die eine Aktion,
    die gerade in `frei` steht ({cid: Adresse oder Formular-Element}; Formular-Knöpfe ohne cid wie 'Test
    unterbrechen' unter None), und nie mit einer eingetragenen Antwort. 'Antwort speichern', 'Test beenden',
    'Abbrechen', 'Nächste Frage' und das Autosave stehen nie in `frei` – der Browser bricht sie ab, bevor sie OLAT erreichen."""
    if methode == "GET":
        return True
    f = felder(body or "")
    ziel = frei.get(f.get("cid"))
    antwort = any(v for k, v in f.items() if k.startswith("qtiworks_") and not k.startswith("qtiworks_presented_"))
    return ziel is not None and ziel in (f.get("dispatchuri"), unquote(urlparse(url).path)) and not antwort


KNOTEN_JS = r"""() => [...document.querySelectorAll('.o_tree li[data-nodeid]')].map(li => {
  const a = li => li?.querySelector(':scope > div .o_tree_link a');
  return {id: li.dataset.nodeid, titel: a(li)?.innerText.trim(),
          icon: [...(a(li)?.querySelector('i')?.classList || [])].find(c => /^o_\w+_icon$/.test(c)),
          // hat Unterbausteine: schon aufgeklappt oder mit Pfeil vor dem Link (auch Ordner wie „PS-Tests“ mit den Bewertungen darunter)
          kinder: !!li.querySelector(':scope > ul li') || [...li.querySelectorAll(':scope > div :is(a, i)')].some(x => !x.closest('.o_tree_link')),
          eltern: a(li.parentElement.closest('li[data-nodeid]'))?.innerText.trim()}; })"""
AUFGABE = ("o_gta_icon", "o_ta_icon")
# Dateien der Betreuer:innen unter „Rückgabe und Feedback“ (geprüft an Mathe › PS-2026-W › Testergebnisse, 25.11.2026)
RUECKGABE = "#o_step_review_content li.o_assessment_document"


def ereignis(a):
    """(Adresse, cid) eines OLAT-Links, der statt einer Adresse o_XHREvent(...) aufruft: damit gibt der Testschutz genau ihn frei."""
    m = re.search(r"o_XHREvent\('([^']+)'.*?'cid','([^']+)'", a.get_attribute("onclick") or "")
    if not m:  # OLAT hat seinen Klick-Aufruf geändert: verständlich melden statt „'NoneType' object has no attribute 'groups'“
        raise ValueError(f"OLAT hat den Knopf „{a.inner_text().strip()[:40]}“ umgebaut, das PC-Programm muss angepasst werden")
    return m.groups()
# Eingaben als [Wert] bzw. [x] in den Text, zugeklappte Lösungen aufklappen (nur in der eigenen Seitenkopie)
LESEN_JS = r"""m => {
  m.querySelectorAll('input[type=text], textarea').forEach(i => i.replaceWith(` [${i.value}] `));
  m.querySelectorAll('input:checked').forEach(i => i.replaceWith(' [x] '));
  m.querySelectorAll('.collapse').forEach(c => c.style.display = 'block');
  return m.innerText.replace(/\n\s*\n\s*/g, '\n\n').trim(); }"""
# angezeigte Frage hat kein ausgefülltes Antwortfeld
LEER_JS = r"""() => ![...document.querySelectorAll('.o_assessmentitem_wrapper :is(input, textarea, select)')]
  .some(i => ['radio', 'checkbox'].includes(i.type) ? i.checked : i.type !== 'hidden' && i.value)"""


def knoten(page, kurs):
    """Alle Kursbausteine; Strukturknoten werden per Adresse geöffnet, damit der Baum aufklappt."""
    alle, offen, seite = {}, [""], 0
    while offen:
        seite += 1
        ANZEIGE.jetzt(f"liest den Kursbaum, Seite {seite}")
        pause(page)
        page.goto(f"{OLAT}/auth/RepositoryEntry/{kurs}{offen.pop()}")
        page.wait_for_load_state("networkidle")
        for k in page.evaluate(KNOTEN_JS):
            if k["id"] not in alle:
                alle[k["id"]] = k
                if k["icon"] == "o_st_icon" or k.get("kinder") and k.get("eltern"):  # ohne Eltern: die Kurswurzel, die ist schon offen
                    offen.append(f"/CourseNode/{k['id']}")
    return list(alle.values())


def test_lesen(page, frei, startknopf):
    """Startet bzw. setzt einen Test fort, zeigt jede Frage einmal an und unterbricht ihn danach mit Bestätigung.
    Freigegeben ist immer nur der nächste Schritt: Start → 'Frage X anzeigen' → 'Test unterbrechen' → 'Ok'.
    Speichern-, Beenden-, Abbrechen- und Weiter-Knöpfe werden vor jedem Klick aus der Seite entfernt."""
    knopf = page.locator("a.o_sel_start_qti21assessment")
    assert knopf.count() == 1 and knopf.inner_text().strip() == startknopf, "kein passender Startknopf"
    try:
        pfad, _ = ereignis(knopf)
        frei["start"] = pfad
        knopf.click()
        links = page.locator("a[onclick*=\"'cid','selectItem'\"]")
        links.first.wait_for()
        frei.clear()
        frei["selectItem"] = re.search(r"'(o_fi\d+)'", links.first.get_attribute("onclick")).group(1)
        fragen, leer = [], []

        def zeige(i):
            page.evaluate(f"() => document.querySelectorAll('{GEFAHR}').forEach(e => e.remove())")
            links.nth(i).evaluate("a => a.click()")
            page.wait_for_load_state("networkidle")
            page.wait_for_timeout(800)

        anzahl = links.count()
        for i in range(anzahl):
            ANZEIGE.jetzt(f"sieht die Fragen an (Frage {i + 1} von {anzahl})")
            zeige(i)
            if page.evaluate(LEER_JS):
                leer.append(i)
            frage = page.locator(".o_assessmentitem_wrapper").first  # mit Kopf: Status und Punkte der Frage
            fragen.append(f"Frage {i + 1}\n" + frage.evaluate(LESEN_JS))
        fragen = "\n\n".join(fragen)
        # 'Unterbrechen' schickt die Felder der angezeigten Frage mit – darum auf einer Frage ohne Antwort unterbrechen
        if not leer:
            print("Alle Fragen haben schon Antworten: Test bleibt ohne Unterbrechen offen.", flush=True)
            return fragen
        try:
            if leer[-1] != links.count() - 1:
                zeige(leer[-1])
            frei.clear()
            unterbrechen = page.locator("a.o_sel_suspend_test")
            assert unterbrechen.count() == 1 and unterbrechen.inner_text().strip() == "Test unterbrechen"
            frei[None] = unterbrechen.get_attribute("id")
            unterbrechen.evaluate("a => a.click()")
            dialog = page.locator(".modal-dialog", has=page.get_by_text("Wollen Sie wirklich unterbrechen?"))
            dialog.wait_for()
            ok = dialog.locator("a.btn", has_text=re.compile(r"^\s*Ok\s*$"))
            assert dialog.locator("h4").inner_text().strip() == "Test unterbrechen" and ok.count() == 1
            frei.clear()
            pfad, cid = ereignis(ok)
            frei[cid] = pfad
            ok.evaluate("a => a.click()")
            page.locator("a.o_sel_start_qti21assessment").wait_for()  # zurück auf der Startseite = unterbrochen
        except Exception as e:  # manche Selbsttests lassen sich nicht unterbrechen: bleibt offen (unbewertet), die Fragen sind gelesen
            print(f"Selbsttest ohne Unterbrechen offen gelassen ({fehlertext(e)})", flush=True)
        return fragen
    finally:
        frei.clear()


def zeitraum(seite):
    """(von, bis) aus „Verfügbarkeit: 15.10.2026, 16:30 bis 19.10.2026, 18:00“, „Bis …“, „Abgelaufen am …“ oder „Ab …“."""
    zeile = (re.search(r"Verfügbarkeit:([^\n]*)", seite) or [None, ""])[1]
    d = [datetime.strptime(f"{t}, {u}", "%d.%m.%Y, %H:%M") for t, u in re.findall(r"(\d\d\.\d\d\.\d{4}), (\d\d:\d\d)", zeile)]
    if len(d) == 1:
        return (d[0], None) if re.match(r"\s*ab\b", zeile, re.I) else (None, d[0])
    return (d[0], d[1]) if len(d) == 2 else (None, None)


def testliste(fach):
    """Alle Tests eines Fachs für den Onlinetest-Überblick der App: Zeitraum, Punkte, Versuche, Fragen.
    Den Stand (läuft, vorbei, erledigt) rechnet die App selbst aus der Uhrzeit."""
    ordner, aus = MATERIAL / fach / TESTS, []
    for f in sorted(ordner.glob("*.txt"), key=lambda f: natur(f.name)):
        if f.stem.endswith(" – Fragen"):
            continue
        t, fragen = f.read_text("utf-8"), ordner / f"{f.stem} – Fragen.txt"
        if t.startswith("OLAT-Bewertung „"):  # Bewertungs-Baustein (PS-Test, Präsentation …): Punkte liest die App aus dem Text
            lv, am = re.match(r"OLAT-Bewertung „.*?“ \((\w*)\)", t), re.search(r"^Rückgabe vom (\d\d)\.(\d\d)\.(\d{4})$", t, re.M)
            aus.append({"fach": fach, "titel": f.stem, "art": "bewertung", "lv": lv and lv.group(1) or None, "startseite": t,
                        "bis": am and f"{am[3]}-{am[2]}-{am[1]}"})
            continue
        if t.startswith("Selbsttest „"):  # Selbsttest-Baustein: die Datei enthält schon die Fragen
            aus.append({"fach": fach, "titel": f.stem, "art": "selbsttest", "fragen": t})
            continue
        von, bis = zeitraum(t)
        zahl = lambda muster: (m := re.search(muster, t)) and float(m.group(1).replace(",", "."))
        aus.append({"fach": fach, "titel": f.stem, "art": "test", "startseite": t,
                    "fragen": fragen.read_text("utf-8") if fragen.exists() else None,
                    "von": von and von.isoformat(timespec="minutes"), "bis": bis and bis.isoformat(timespec="minutes"),
                    "punkte": zahl(r"Max\. Punktzahl: ([\d.,]+)"), "verbleibend": zahl(r"Verbleibend: (\d+)"),
                    "ergebnis": "Dies sind Ihre Testresultate" in t,
                    "uebung": bool(re.search(r"nicht relevant für die Leistungsbeurteilung|ohne Notendruck|nur zur Übung", t)
                                   or "Selbsttest" in f.stem)})
    return aus


def rueckgabe(page, ziel, lv, frei):
    """Dateien unter „Rückgabe und Feedback“ eines Aufgaben-Bausteins laden und je Datei eine Bewertung bauen [(Datei, Text, Titel)].
    OLAT gibt sie nur per Klick heraus (o_XHREvent 'download', keine Adresse): der Testschutz lässt genau diesen einen Klick durch.
    Die Punkte liest die App aus dem Text, wie bei einem Bewertungs-Baustein; das Original bleibt unter Tests/Rückgabe."""
    aus = []
    for doc in page.locator(RUECKGABE).all():
        roh = ziel / "Rückgabe" / sicherer_name(doc.locator("h5").inner_text().strip())
        if not roh.exists():  # ponytail: schon geladen = gleicher Name; eine korrigierte Fassung unter gleichem Namen käme nicht an
            knopf = doc.locator(".o_assessment_document_actions a", has_text=re.compile(r"^\s*Herunterladen\s*$"))
            pfad, cid = ereignis(knopf)
            frei[cid] = pfad
            try:
                with page.expect_download() as dl:
                    knopf.click()
                roh.parent.mkdir(exist_ok=True)
                speichern_als(dl.value, roh)
            finally:
                frei.clear()
        datum = (re.search(r"\d\d\.\d\d\.\d{4}", doc.inner_text()) or [None])[0]  # „erstellt von … am 20.10.2026, 23:56“
        titel = roh.stem
        inhalt = re.sub(r"\s*\n\s*", "\n", re.sub(r"[ \t\xa0]+", " ", text(roh))).strip()
        # Kopf der Klausurergebnisse (Mathe-PS): Name, Matrikelnummer und Klausur-ID braucht die App nicht, sie bleiben am PC
        inhalt = re.sub(r"(?m)^(Name|Matrikelnummer|Klausur-ID):.*\n?", "", inhalt)
        aus.append((ziel / f"{titel}.txt", f"OLAT-Bewertung „{titel}“ ({lv})\n" + (f"Rückgabe vom {datum}\n" if datum else "") + "\n" + inhalt, titel))
    return aus


def tests(browser, fach, kurs_ids=None):
    """Onlinetests, Selbsttests und Bewertungen eines Fachs nach material/<Fach>/Tests – in eigener Sitzung mit Testschutz.
    `kurs_ids` ({Kurs: LV-Art}): auch Kurse ohne Dateien, etwa ein PS-Kurs, in dem nur die Punkte der PS-Tests stehen."""
    frei, neu = {}, []

    def waechter(route):
        r = route.request
        body = r.post_data_buffer.decode(errors="ignore") if r.post_data_buffer else ""
        if "mathjax" in r.url.lower() or not erlaubt(r.method, r.url, body, frei):  # ohne MathJax bleibt LaTeX lesbar
            return route.abort()
        route.continue_()

    ctx = browser.new_context(storage_state=sitzung_lesen())
    ctx.route("**/*", waechter)
    page = ctx.new_page()
    ziel = MATERIAL / fach / TESTS
    ziel.mkdir(parents=True, exist_ok=True)
    # Art der Lehrveranstaltung je Kurs (VO, PS …) aus den Namen in kurse.json, für die Bewertungen
    arten = {q["url"].split("/")[0]: q["name"].split()[0] for q in KURSE.get(fach, []) if re.match(r"(VO|PS|UE|VU|SE|UV|KS) ", q["name"])}
    arten.update({k: t for k, t in (kurs_ids or {}).items() if t})
    namen = set()  # „Onlinetest 1“ in VO und PS desselben Fachs: sonst überschreiben sie sich bei jedem Abgleich gegenseitig
    for kurs in sorted({q["url"].split("/")[0] for q in KURSE.get(fach, [])} | set(kurs_ids or {})):
        ANZEIGE.jetzt("liest den Kursbaum", fach + (f" · {arten[kurs]}" if arten.get(kurs) else ""))
        for k in knoten(page, kurs):
            # ponytail: Altklausuren (Klausurarchiv) übersprungen – eigener Prüfungsmodus, falls gewünscht
            if k["icon"] not in (*TESTICONS, *AUFGABE) or k["eltern"] == "Klausurarchiv":
                continue
            name = re.sub(r'[\\/:*?"<>|]', "-", k["titel"])
            if name in namen:
                name += f" ({arten.get(kurs) or kurs})"
            namen.add(name)
            datei, fragen = ziel / f"{name}.txt", ziel / f"{name} – Fragen.txt"
            if k["icon"] == "o_iqself_icon" and datei.exists():  # Selbsttests ändern sich nicht
                continue
            # PS-Tests liegen oft im VO-Kurs (VWL: Proseminare › PS-Tests) oder in einem Kurs ohne Art im Namen (Mathe: PS-2026-W)
            lv = "PS" if re.search(r"\bPS\b|Proseminar", f"{k['titel']} {k['eltern'] or ''}") else arten.get(kurs, "")
            ANZEIGE.jetzt({"o_ms_icon": "liest die Bewertung", "o_iqself_icon": "sieht die Fragen an"}.get(k["icon"], "liest die Rückgabe" if k["icon"] in AUFGABE
                          else "liest die Startseite"), f"{fach} · {k['titel']}")
            try:  # auch das Öffnen: eine Zeitüberschreitung auf einer Testseite hält die anderen nicht auf
                pause(page)
                page.goto(f"{OLAT}/auth/RepositoryEntry/{kurs}/CourseNode/{k['id']}")
                page.wait_for_load_state("networkidle")
                if k["icon"] in AUFGABE:  # Aufgabe: Punkte stehen in den Dateien unter „Rückgabe und Feedback“ (Mathe-PS-Tests als HTML)
                    aus = rueckgabe(page, ziel, lv, frei)
                    if not aus and page.get_by_text(re.compile(r"Dokumente für Sie|Beurteilungen herunterladen")).count():
                        # OLAT zeigt die Dateiliste anders als in RUECKGABE erwartet: lieber der Seitentext als gar nichts
                        aus = [(datei, f"OLAT-Bewertung „{k['titel']}“ ({lv})\n\n" + page.locator("#o_main_center_content_inner").evaluate(LESEN_JS), k["titel"])]
                elif k["icon"] == "o_ms_icon":  # Bewertung: nur lesen, die Seite hat für Teilnehmende nichts zum Abschicken
                    seite = page.locator("#o_main_center_content_inner").evaluate(LESEN_JS)
                    aus = [(datei, f"OLAT-Bewertung „{k['titel']}“ ({lv})\n\n" + seite, k["titel"])]
                elif k["icon"] == "o_iqself_icon":
                    aus = [(datei, f"Selbsttest „{k['titel']}“ (anonyme, unbewertete Übung) – Fragen ohne Lösung\n\n" + test_lesen(page, frei, "Selbsttest starten"), k["titel"])]
                else:
                    seite = page.locator("#o_main_center_content_inner").evaluate(LESEN_JS)
                    aus = [(datei, f"OLAT-Test „{k['titel']}“ – Startseite (ob er bewertet wird, steht im Text)\n\n" + seite, k["titel"])]
                    # nach Abschluss stehen Fragen samt Lösung auf der Startseite; Fragen-Dateien älterer Fassungen gehen weg
                    fragen.unlink(missing_ok=True)
            except Exception as e:  # ein Test soll den Rest nicht aufhalten
                ANZEIGE.warnung(f"{fach}: {k['titel']} nicht lesbar ({fehlertext(e)})", f"{fach} · {k['titel']} nicht lesbar: {grund(e)}", zaehlt=True)
                continue
            for datei, t, titel in aus:
                ANZEIGE.z["tests"] += 1
                if not datei.exists() or datei.read_text("utf-8") != t:
                    vorher = datei.read_text("utf-8") if datei.exists() else None
                    art = ("neu" if vorher is None else "Ergebnis ist da" if "Testresultate" in t and "Testresultate" not in vorher
                           else "Punkte geändert" if t.startswith("OLAT-Bewertung") else "geändert")
                    neu.append(f"{fach}: {TESTS}/{datei.name}")
                    ANZEIGE.fund(fach, f"Test „{titel}“", art=art, name=titel)
                    datei.write_text(t, "utf-8")
    ctx.close()
    return neu


KALENDER = "kalender/OLAT.ics"  # die App liest alle kalender/*.ics und zeigt doppelte Termine (auch aus LFU:online) nur einmal


def kalender(page):
    """OLAT-Kalender mit allen Kurs-, Gruppen- und eigenen Terminen: Kalender → „Persönlicher aggregierter iCal Feed-Link“
    (am echten OLAT 26.11.2026 geprüft). Der Link trägt ein Token und bleibt am PC, in die App geht nur die .ics."""
    oeffne(page, OLAT + "/auth/MyCoursesSite/0")
    oeffne(page, OLAT + page.locator("a.o_sel_user_tools-calendar").first.get_attribute("href"))
    page.locator(".fc-aggregatedfeed-button").click()
    url = page.locator(".o_cal_event_callout input[type=text]").first.input_value()
    if not url.startswith(OLAT + "/ical/"):  # nie eine fremde Adresse abrufen
        raise RuntimeError("OLAT zeigte keinen iCal-Link")
    r = page.context.request.get(url)
    if not r.ok or b"BEGIN:VCALENDAR" not in r.body():
        raise RuntimeError(f"OLAT lieferte keine Kalenderdatei (HTTP {r.status})")
    return r.body()


def api(methode, pfad, body=None, roh=False):
    """GitHub-API mit dem Token aus zugang.json (darf nur das eigene Daten-Repo lesen und schreiben); roh: Dateiinhalt als Bytes, auch über 1 MB."""
    z = json.loads(ZUGANG.read_text("utf-8"))
    anfrage = Request(f"{z.get('api') or API}/repos/{z['repo']}/contents/{quote(pfad)}", method=methode, data=body and json.dumps(body).encode(),
                      headers={"Authorization": "Bearer " + z["token"], "Accept": "application/vnd.github.raw+json" if roh else "application/vnd.github+json",
                               "User-Agent": "Lernhelfer"})
    for versuch in range(3):  # Netz weg, Server startet gerade neu, GitHub bremst: zweimal nachfassen statt den ganzen Abgleich zu verlieren
        try:
            with urlopen(anfrage, timeout=120) as r:
                return r.read() if roh else json.loads(r.read())
        except HTTPError as e:
            if e.code not in (429, 500, 502, 503, 504) or versuch == 2:
                raise
        except (URLError, TimeoutError):
            if versuch == 2:
                raise
        time.sleep(API_PAUSE * (versuch + 1))


def hochgeladene(fach):
    """Was die App unter „Unterlagen“ ins Fach hochgeladen hat; der Abgleich überschreibt die Datei und nimmt das mit."""
    ANZEIGE.jetzt("holt, was du in der App selbst hochgeladen hast", fach)
    try:
        d = json.loads(api("GET", f"faecher/{fach}.json", roh=True))
    except HTTPError as e:
        if e.code == 404:
            return []
        raise  # lieber gar nicht hochladen als hochgeladene Unterlagen verlieren
    return [x for x in d.get("dateien", []) if str(x.get("datei", "")).startswith(HOCHGELADEN)]


def hochladen(stand, pfad, daten):
    """Eine Datei ins Daten-Repo; unverändert seit dem letzten Hochladen = nichts tun (Git-Prüfsumme lokal gerechnet).
    Die Prüfsummen gelten nur für das Ziel, für das sie gemerkt wurden: nach dem Wechsel von GitHub zum Server liegt dort noch nichts."""
    z = json.loads(ZUGANG.read_text("utf-8"))
    ziel = f"{z.get('api') or API}/{z['repo']}"
    if stand.get("ziel") != ziel:
        stand["ziel"], stand["sha"] = ziel, {}
    shas = stand["sha"]
    if shas.get(pfad) == hashlib.sha1(b"blob %d\0" % len(daten) + daten).hexdigest():
        ANZEIGE.jetzt(f"{pfad} unverändert", "App")
        return ANZEIGE.hoch(pfad, False)
    ANZEIGE.jetzt(f"schickt {pfad} · {groesse(len(daten))}", "App")
    for versuch in (1, 2):
        body = {"message": f"OLAT-Abgleich {datetime.now():%d.%m.%Y %H:%M}: {pfad}", "content": base64.b64encode(daten).decode()}
        if shas.get(pfad):
            body["sha"] = shas[pfad]
        try:
            shas[pfad] = api("PUT", pfad, body)["content"]["sha"]
            return ANZEIGE.hoch(pfad, True)
        except HTTPError as e:  # 409/422: Datei gibt es schon in anderer Fassung (erster Lauf, anderes Gerät) → aktuelle holen
            if e.code not in (409, 422) or versuch == 2:
                raise
            try:
                shas[pfad] = api("GET", pfad)["sha"]
            except HTTPError as e2:
                if e2.code != 404:
                    raise
                shas.pop(pfad, None)


# Selbst-Update nur mit Signatur (Ed25519, RFC 8032): Wer GitHub oder den Server übernimmt, kann trotzdem keine fremde holen.py
# verteilen, die gespeicherte Zugangsdaten liest. Der private Schlüssel liegt nur am PC des Entwicklers (signieren.mjs).
SIGNATUR = "8c6bd24b77cc4aa64b4fe9086dbf723a72b10e058cd0a7ca6b09852de9a4be39"
_P, _L = 2**255 - 19, 2**252 + 27742317777372353535851937790883648493
_D = -121665 * pow(121666, _P - 2, _P) % _P


def _plus(a, b):
    (x1, y1, z1, t1), (x2, y2, z2, t2) = a, b
    a, b, c, d = (y1 - x1) * (y2 - x2) % _P, (y1 + x1) * (y2 + x2) % _P, 2 * t1 * t2 * _D % _P, 2 * z1 * z2 % _P
    e, f, g, h = b - a, d - c, d + c, b + a
    return e * f % _P, g * h % _P, f * g % _P, e * h % _P


def _mal(s, q):
    r = (0, 1, 1, 0)
    while s:
        if s & 1:
            r = _plus(r, q)
        q, s = _plus(q, q), s >> 1
    return r


def _punkt(b):
    y = int.from_bytes(b, "little")
    vz, y = y >> 255, y & ((1 << 255) - 1)
    if len(b) != 32 or y >= _P:
        raise ValueError
    xx = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P) % _P
    x = pow(xx, (_P + 3) // 8, _P)
    if (x * x - xx) % _P:
        x = x * pow(2, (_P - 1) // 4, _P) % _P
    if (x * x - xx) % _P or (x == 0 and vz):
        raise ValueError
    if x & 1 != vz:
        x = _P - x
    return x, y, 1, x * y % _P


def signatur_gilt(daten, sig, schluessel=None):
    """Ed25519-Prüfung nach RFC 8032 Abschnitt 6 (die Standardbibliothek kann das nicht, ein Zusatzpaket wäre selbst ein Risiko)."""
    try:
        a = bytes.fromhex(schluessel or SIGNATUR)
        r, s = _punkt(sig[:32]), int.from_bytes(sig[32:], "little")
        if len(sig) != 64 or s >= _L:
            return False
        h = int.from_bytes(hashlib.sha512(sig[:32] + a + daten).digest(), "little") % _L
        links, rechts = _mal(s, _punkt((4 * pow(5, _P - 2, _P) % _P).to_bytes(32, "little"))), _plus(r, _mal(h, _punkt(a)))
        return (links[0] * rechts[2] - rechts[0] * links[2]) % _P == 0 and (links[1] * rechts[2] - rechts[1] * links[2]) % _P == 0
    except ValueError:
        return False


def aktualisieren():
    """Installierte Kopien holen sich neue Fassungen von holen.py selbst (gilt ab dem nächsten Lauf); die Entwicklungskopie nicht.
    Nur über HTTPS und nur mit gültiger Signatur (holen.py.sig), sonst läuft die alte Fassung weiter."""
    if (BASIS / ".git").exists():
        return
    try:
        app = ((json.loads(ZUGANG.read_text("utf-8")).get("app") if ZUGANG.exists() else None) or APP).rstrip("/")  # Server-Version: von dort
        if not app.startswith("https://"):
            raise ValueError(f"nur über HTTPS, nicht {app}")
        neu = urlopen(app + "/holen.py", timeout=30).read()
        if neu == Path(__file__).read_bytes():
            return
        if not signatur_gilt(neu, base64.b64decode(urlopen(app + "/holen.py.sig", timeout=30).read().strip())):
            raise ValueError("Signatur passt nicht, die bisherige Fassung läuft weiter")
        compile(neu, "holen.py", "exec")
        tmp = Path(__file__).with_name("holen.py.neu")  # nie eine halb geschriebene holen.py: die könnte sich nicht mehr selbst reparieren
        tmp.write_bytes(neu)
        os.replace(tmp, __file__)
        print("holen.py aktualisiert.")
        return True
    except Exception as e:
        print("Aktualisierung übersprungen:", e)


def olat_erlaubt():
    """Schalter in der Admin-Ecke des Lernhelfer-Servers: aus = kein Abgleich mit OLAT, bis er wieder an ist.
    Ohne Server (eigene GitHub-Version) oder ohne Antwort gleicht er ab wie bisher."""
    try:
        api = json.loads(ZUGANG.read_text("utf-8")).get("api") if ZUGANG.exists() else None
        if not api:
            return True
        return json.loads(urlopen(api.rsplit("/api/", 1)[0] + "/api/tarife", timeout=15).read()).get("olat", True) is not False
    except Exception:
        return True


def zugang_kaputt():
    """Text, wenn der Speicher den Schlüssel aus zugang.json ablehnt (401/403): dann gar nicht erst zu OLAT, die Dateien kämen
    ohnehin nicht an. Netzfehler, fehlende Datei usw.: None, der Abgleich läuft wie bisher."""
    try:
        api("GET", "abgleich.json", roh=True)
    except HTTPError as e:
        if e.code in (401, 403):
            server = json.loads(ZUGANG.read_text("utf-8")).get("api")
            return (f"Der Schlüssel dieses PC-Programms gilt nicht mehr: In der App wurde inzwischen ein neues PC-Programm geladen "
                    f"(oder überall abgemeldet). Am {GERAET} die App öffnen, unter Konto „PC-Programm neu laden“ und die Datei ausführen."
                    if server else "GitHub lehnt den Schlüssel ab (abgelaufen oder gelöscht). In der App unter Einrichten einen neuen "
                                   "GitHub-Schlüssel eintragen und das PC-Programm neu laden.")
    except Exception:
        pass
    return None


def absturz_bericht(stand, e, schritt):
    """Bricht ein Abgleich unerwartet ab, geht ein Fehlerbericht an den Lernhelfer-Server (Admin-Ecke und Mail an den Betreiber):
    Art, Programmzeilen und Fassung, ohne Pfade und Seiteninhalte. Derselbe Fehler nur einmal, bis ein Lauf wieder durchgeht.
    Die eigene GitHub-Version hat keinen Server und meldet nichts."""
    z = json.loads(ZUGANG.read_text("utf-8"))
    # nicht __file__: im sys.excepthook ist es schon weg (Python 3.14, fand test_probelauf.py)
    zeilen = [f"Zeile {f.lineno} in {f.name}" for f in traceback.extract_tb(e.__traceback__) if Path(f.filename).name == "holen.py"]
    kennung = f"{type(e).__name__} {FASSUNG} {' '.join(zeilen)}"
    if not z.get("api") or stand.get("absturz") == kennung:
        return
    ohne_pfade = lambda x: re.sub(r"[A-Za-z]:\\\S*|/(?:Users|home)/\S*", "…", fehlertext(x))  # der Windows-Benutzername steckt im Pfad
    davor = f"\nwährend: {type(e.__context__).__name__}: {ohne_pfade(e.__context__)}" if e.__context__ else ""
    body = {"seite": "PC-Programm", "meldung": f"{type(e).__name__}: {ohne_pfade(e)}", "text": f"Abgebrochen bei: {schritt or '?'}{davor}\n" + "\n".join(zeilen),
            "geraet": f"{sys.platform} · Python {sys.version.split()[0]} · holen.py {FASSUNG}"}
    urlopen(Request(z["api"].rsplit("/api/", 1)[0] + "/api/fehler", json.dumps(body).encode(),
                    {"Authorization": "Bearer " + z["token"], "Content-Type": "application/json", "User-Agent": "Lernhelfer"}), timeout=15).close()
    stand["absturz"] = kennung


def sperre():
    """Nur ein Abgleich gleichzeitig: Aufgabe bzw. launchd und der Doppelklick teilen Sitzung, Chrome-Profil und Stand.
    Gibt die offene Sperrdatei zurück (bis zum Ende behalten) oder None, wenn schon einer läuft."""
    f = open(BASIS / ".laeuft", "a+")
    try:
        f.seek(0)
        if sys.platform == "win32":
            import msvcrt
            msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        f.close()
        return None
    try:
        (BASIS / ".laeuft.pid").write_text(str(os.getpid()))  # wer die Sperre hält: für hintergrund_stoppen()
    except OSError:
        pass
    return f


def hintergrund_stoppen():
    """Taste im Wartebildschirm: beendet den Abgleich, der gerade die Sperre hält, samt Browser. Dabei geht nichts kaputt:
    Stand, Sitzung, Downloads und holen.py werden nur über Nebendatei und Umbenennen geschrieben, als ginge der PC aus.
    False, wenn er sich nicht finden lässt (ältere Fassung ohne .laeuft.pid) oder die Nummer kein Lernhelfer-Python ist."""
    try:
        pid = int((BASIS / ".laeuft.pid").read_text())
    except (OSError, ValueError):
        return False
    if pid == os.getpid():
        return False
    if sys.platform == "win32":  # /T: auch Playwright-Treiber und Chrome darunter
        name = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"], capture_output=True).stdout  # Bytes: Meldungen in OEM-Codepage
        return name.strip().lower().startswith(b'"python') and \
            subprocess.run(["taskkill", "/PID", str(pid), "/T", "/F"], capture_output=True).returncode == 0
    if b"holen.py" not in subprocess.run(["ps", "-p", str(pid), "-o", "command="], capture_output=True).stdout:
        return False
    os.kill(pid, signal.SIGTERM)  # der Playwright-Treiber schließt seinen Browser selbst, sobald Python weg ist
    return True


class Abbruch(BaseException):
    """Esc oder Strg+C im Fenster, kein Absturz. Kein KeyboardInterrupt: damit endete Python mit dem Strg+C-Code und cmd fragte
    hinterher „Batchvorgang abbrechen (J/N)?“. BaseException, damit ihn kein „except Exception“ schluckt."""


def abbrechen(*_):
    raise Abbruch


def still(typ, e, tb):
    """Abbruch von Hand: kein Traceback und kein Fehlerbericht, ANZEIGE.schluss() sagt, dass nichts kaputt ist."""
    if not issubclass(typ, (Abbruch, KeyboardInterrupt)):
        return sys.__excepthook__(typ, e, tb)
    ANZEIGE.von_hand = True
    logging.getLogger("asyncio").disabled = True  # Playwright-Reste beim Aufräumen: „Future exception was never retrieved“


def esc_abbruch():
    """Windows: Esc im Konsolenfenster wirkt wie Strg+C (über abbrechen()), nur ohne die Rückfrage von cmd.
    Wartet auf einen Druck und nur, solange der Live-Bereich steht: input() und getpass (Login einrichten) bekommen jede Taste."""
    if sys.platform != "win32" or not ANZEIGE.an:
        return
    import _thread
    import msvcrt

    def warten():
        while not (ANZEIGE.titel and not ANZEIGE.offen and msvcrt.kbhit() and msvcrt.getwch() == "\x1b"):
            time.sleep(0.1)
        _thread.interrupt_main()
    threading.Thread(target=warten, daemon=True).start()


if __name__ == "__main__":
    from playwright.sync_api import TimeoutError as Zeitueberschreitung, sync_playwright

    auto = "--auto" in sys.argv  # aus der Aufgabenplanung bzw. launchd: ohne Konsole, Ausgaben ins Log
    if auto and not faellig(json_lesen(STAND, {}), datetime.now()):
        sys.exit()  # noch nicht dran: OLAT bleibt in Ruhe, auch kein Wachhalten der Sitzung
    if not auto and aktualisieren():  # von Hand gleich mit der neuen Fassung, sonst liefe ein schon behobener Fehler bis zum nächsten automatischen Lauf weiter
        sys.exit(subprocess.run([sys.executable, __file__, *sys.argv[1:]]).returncode)
    if auto:
        sys.stdout = sys.stderr = open(BASIS / "holen.log", "a", encoding="utf-8", buffering=1)
        print(f"--- {datetime.now():%d.%m.%Y %H:%M}")
    else:
        ANZEIGE.einschalten()
        signal.signal(signal.SIGINT, abbrechen)
        sys.excepthook = still
    esc_abbruch()
    SPERRE = sperre()
    gewartet = gestoppt = False
    bis = time.monotonic() + (0 if auto else 1200)  # von Hand: bis zu 20 Minuten warten, bis der Lauf im Hintergrund fertig ist
    while not SPERRE and time.monotonic() < bis:
        if not gewartet:
            print(f"Gerade gleicht der Lernhelfer im Hintergrund ab. Warte, bis er fertig ist, oder brich ihn mit {ANZEIGE.TASTE.replace(' ', '+')} ab.", flush=True)
            ANZEIGE.schritt("Warte auf den Abgleich im Hintergrund", frist=1200)
            ANZEIGE.jetzt("wartet, bis er fertig ist", "Hintergrund")
            ANZEIGE.abbruch = "abbrechen und selbst abgleichen"
            gewartet = True
        try:
            time.sleep(0.5)
        except Abbruch:  # Esc bzw. Strg+C: hier den Lauf im Hintergrund, danach gleicht dieser ab
            ANZEIGE.jetzt("bricht ihn ab", "Hintergrund")
            gestoppt = hintergrund_stoppen()
            if not gestoppt:
                ANZEIGE.warnung("Der Abgleich im Hintergrund lässt sich nicht abbrechen (ältere Fassung). Warte, bis er fertig ist.")
                ANZEIGE.abbruch = "abbrechen"
                ANZEIGE.jetzt("wartet, bis er fertig ist", "Hintergrund")
            esc_abbruch()
        SPERRE = sperre()
    ANZEIGE.abbruch = "abbrechen"
    if not SPERRE:
        ANZEIGE.schritt(None)
        print("Es läuft schon ein Abgleich.")
        sys.exit(0 if auto else 1)
    if gewartet:
        ANZEIGE.ok("Hintergrund-Abgleich abgebrochen, jetzt du" if gestoppt else "Hintergrund-Abgleich fertig, jetzt du")
    ANZEIGE.schritt("Prüfe deine App")
    ANZEIGE.jetzt("fragt deinen Speicher, ob der Schlüssel noch gilt", "App")
    if not olat_erlaubt():
        ANZEIGE.schritt(None)
        print("Der Abgleich mit OLAT ist gerade pausiert (Lernhelfer-Server). Er läuft von selbst wieder, sobald er freigegeben ist.", flush=True)
        if auto:
            aktualisieren()  # neue Fassungen kommen trotzdem an, ohne OLAT zu berühren
        sys.exit(0 if auto else 1)
    if kaputt := zugang_kaputt():  # nicht „grund“: das überschrieb die Funktion grund() und jede Warnung brach den ganzen Abgleich ab
        ANZEIGE.fehler(kaputt)
        if auto:
            aktualisieren()
        sys.exit(0 if auto else 1)
    ANZEIGE.schritt(None)  # vor autologin(): dessen Fragen laufen ohne Live-Bereich
    stand = json_lesen(STAND, {})
    alt = stand.get("abgleich", {})
    jetzt = datetime.now()

    def melde(ok, text, dateien=()):
        stand["abgleich"] = {"zeit": datetime.now().isoformat(timespec="minutes"), "ok": ok, "text": text,
                             "autologin": stand.get("autologin_an") and {"eingerichtet": bool(login_lesen()), "fehler": stand.get("autologin_fehler")}}
        try:
            for pfad, inhalt in [*dateien, ("abgleich.json", stand["abgleich"])]:
                hochladen(stand, pfad, inhalt if isinstance(inhalt, bytes) else json.dumps(inhalt, ensure_ascii=False).encode())
        finally:
            json_schreiben(STAND, stand)

    def absturz(typ, e, tb):
        """Unerwarteter Abbruch: Traceback wie bisher, dann Fehlerbericht an den Server und ein Hinweis in der App,
        sonst stünde dort weiter der letzte gute Abgleich und niemand merkte etwas."""
        still(typ, e, tb)  # Traceback wie bisher; von Hand abgebrochen (Esc, Strg+C): nichts melden
        if issubclass(typ, (Abbruch, KeyboardInterrupt)):
            return
        schritt = " · ".join(x for x in (ANZEIGE.titel, ANZEIGE.ort) if x)
        for tun in (lambda: absturz_bericht(stand, e, schritt),
                    lambda: melde(False, f"Abgleich am {GERAET} abgebrochen{' bei ' + schritt if schritt else ''} ({typ.__name__}). "
                                         f"Nochmal versuchen: am {GERAET} „Lernhelfer OLAT“ auf dem {ABLAGE} doppelklicken.")):
            try:
                tun()
            except Exception:  # kein Netz, Server weg: das Fenster zeigt den Fehler trotzdem
                pass
    sys.excepthook = absturz

    def aufzaehlung(namen):
        return namen[0] if len(namen) == 1 else ", ".join(namen[:-1]) + " und " + namen[-1]

    with sync_playwright() as p:
        # Login-Fenster aus der Aufgabenplanung höchstens alle 8 Stunden (z. B. morgens nach dem Einschalten), sonst nervt es
        def fenster_auf():
            stand["fenster"] = datetime.now().isoformat(timespec="minutes")
            json_schreiben(STAND, stand)

        def login_fehler(grund):
            ANZEIGE.warnung(f"Automatischer Login ging nicht: {grund}")
            stand["autologin_fehler"] = grund
            json_schreiben(STAND, stand)
        try:
            browser, ctx, page = olat_login(p, sichtbar=not auto or jetzt - datetime.fromisoformat(stand.get("fenster", "2000-01-01")) > timedelta(hours=8),
                                            fenster_auf=fenster_auf, login=autologin(p, stand, von_hand=not auto), login_fehler=login_fehler)
        except (Abgelaufen, Zeitueberschreitung) as e:
            ANZEIGE.fehler("OLAT-Anmeldung abgelaufen. Starte den Abgleich neu und logge dich ein." if ANZEIGE.an else "OLAT-Anmeldung abgelaufen.",
                           type(e).__name__)
            weiter(stand, 4)  # abgelaufen: in rund 4 Stunden wieder versuchen, von Hand jederzeit
            json_schreiben(STAND, stand)
            hinweis = (f"Automatischer OLAT-Login ging nicht ({stand['autologin_fehler']}) – am {GERAET} „Lernhelfer OLAT“ auf dem {ABLAGE} doppelklicken"
                       " und die Zugangsdaten neu eintragen oder selbst einloggen." if stand.get("autologin_fehler") else
                       f"OLAT-Anmeldung am {GERAET} abgelaufen – am {GERAET} „Lernhelfer OLAT“ auf dem {ABLAGE} doppelklicken und einloggen.")
            if (alt.get("ok") is not False or alt.get("text") != hinweis) and ZUGANG.exists():
                melde(False, hinweis)
            if auto:
                aktualisieren()  # sonst käme eine neue Fassung erst nach dem nächsten Login an
            sys.exit(0 if auto else 1)
        sitzung_speichern(ctx)
        weiter(stand, 12)  # gleich speichern: bricht der Lauf ab, kommt OLAT trotzdem erst in 12 Stunden wieder dran
        json_schreiben(STAND, stand)
        alte_ids = {k for d in stand.get("kurs_ids", {}).values() for k in d}
        if KURSE is None or "--kurse" in sys.argv or kurse_suchen(stand, jetzt):
            such_zeit = jetzt
            try:
                ANZEIGE.schritt("Suche deine Kurse", hinweis="zum ersten Mal: dauert etwas länger" if KURSE is None else "einmal pro Woche: neue Kurse und Ordner")
                ids = {}
                gefunden = kurse_finden(page, jetzt, KURSE, ids)
                if not gefunden and KURSE:  # OLAT zeigte keine Kurse (Seite geändert, zu langsam, neues Semester noch leer): nichts wegwerfen
                    such_zeit = jetzt - timedelta(days=6)  # morgen nochmal statt in einer Woche
                    raise RuntimeError("OLAT zeigte unter „Meine Kurse“ keinen Kurs an")
                KURSE = gefunden
                json_schreiben(KURSDATEI, KURSE, indent=2)
                stand["kurse_semester"], stand["kurs_ids"] = semester(jetzt), ids
                if ANZEIGE.an:
                    neu_dabei = [f"{f} {t or ''}".strip() for f, d in ids.items() for k, t in d.items() if alte_ids and k not in alte_ids]
                    ANZEIGE.ok(f"{anzahl(sum(map(len, ids.values())), 'Kurs', 'Kurse')} in {anzahl(len(KURSE), 'Fach', 'Fächern')}",
                               f"{sum(map(len, KURSE.values()))} Ordner und Dokumente" + (f" · neu dabei: {', '.join(neu_dabei)}" if neu_dabei else ""))
                else:
                    print("Gefundene Kurse:", *(f"  {f}: {', '.join(q['name'] for q in l)}" for f, l in KURSE.items()), sep="\n")
            except Exception as e:  # gibt es schon Kurse, gleicht er mit denen ab, statt ganz auszufallen; nächster Versuch in einer Woche
                if KURSE is None:
                    raise
                ANZEIGE.warnung(f"Kurse neu suchen ging nicht, es bleibt bei den bisherigen: {fehlertext(e)}")
            stand["kurse_zeit"] = such_zeit.isoformat(timespec="minutes")
            if "--kurse" in sys.argv:
                json_schreiben(STAND, stand)
                ANZEIGE.schritt(None)  # planmäßiges Ende, kein „Abgebrochen“
                sys.exit()
        elif stand.get("kurse_semester") and stand.get("kurse_zeit"):
            k = datetime.fromisoformat(stand["kurse_zeit"])  # wie kurse_suchen(): nach 7 Tagen oder ab dem neuen Semester
            d = min(k + timedelta(days=7), datetime(k.year + (k.month >= 9), 3, 1) if semester(k).endswith("W") else datetime(k.year, 9, 1))
            w = wann(d, jetzt)
            ANZEIGE.zeile("Kurse wie gespeichert", " · ".join(KURSE) + " · sucht "
                          + ({"heute": f"heute ab {d:%H:%M}", "morgen": "morgen"}.get(w) or f"am {w}") + " wieder neu")
        else:
            ANZEIGE.zeile("Kurse aus kurse.json", " · ".join(KURSE))
        if auto:
            aktualisieren()
        faecher = [a for a in sys.argv[1:] if not a.startswith("--")] or list(KURSE)
        ANZEIGE.schritt("Unterlagen", sum(len(KURSE.get(f, [])) for f in faecher), hinweis="geladen wird nur, was neu ist", faecher=faecher)
        neu = [n for f in faecher for n in sync(ctx, page, f)]
        z = ANZEIGE.z
        gefunden = sum(ANZEIGE.funde.values())  # wie die Karte: auch Dateien aus einem Ordner, der danach abbrach
        ANZEIGE.ok("Unterlagen", f"{z['stellen']} Ordner und Dokumente geprüft, {anzahl(z['gesehen'], 'Datei', 'Dateien')}, "
                   + (f"{gefunden} neu" + (f" ({groesse(z['bytes'])})" if z["bytes"] else "") if gefunden else "nichts Neues"))
        sitzung_speichern(ctx)
        ANZEIGE.schritt("Tests und Punkte", len(faecher), hinweis="Testschutz an: nur lesen, nichts abschicken", faecher=faecher)
        neu_tests = []
        for f in faecher:
            try:
                neu_tests += tests(browser, f, stand.get("kurs_ids", {}).get(f))
            except Exception as e:  # Tests sind Beiwerk: die Unterlagen kommen trotzdem in die App
                ANZEIGE.warnung(f"{f}: Tests nicht gelesen ({fehlertext(e)})", f"{f} · Tests nicht gelesen: {grund(e)}", zaehlt=True)
            ANZEIGE.plus()
        ANZEIGE.ok("Tests und Punkte", f"{z['tests']} gelesen, " + (f"{len(neu_tests)} neu oder geändert" if neu_tests else "nichts geändert")
                   + ", keine Antwort abgeschickt")
        neu += neu_tests
        ANZEIGE.schritt("Kalender")
        ANZEIGE.jetzt("holt deine Termine", "OLAT-Kalender")
        try:
            ics = kalender(page)
            ANZEIGE.ok("Kalender", anzahl(ics.count(b"BEGIN:VEVENT"), "Termin", "Termine"))
        except Exception as e:  # Beiwerk wie die Tests: Unterlagen kommen trotzdem an
            ics = None
            ANZEIGE.warnung(f"OLAT-Kalender nicht gelesen ({fehlertext(e)})", zaehlt=True)
        browser.close()
    ANZEIGE.schritt("Lese deine Dateien", len(faecher), hinweis="nur der Text geht in die App, die Dateien bleiben am PC")
    tests_alle = [t for f in KURSE for t in testliste(f)]
    dateien = [(f"faecher/{f}.json", fach_json(f, hochgeladene(f))) for f in faecher] + [("onlinetests.json", tests_alle)] + ([(KALENDER, ics)] if ics else [])
    ANZEIGE.ok(f"Text aus {anzahl(ANZEIGE.z['gelesen'], 'Datei', 'Dateien')} gelesen")
    stand["dauer"] = int(time.monotonic() - ANZEIGE.start)
    ANZEIGE.schritt("Lade alles in deine App", len(dateien) + 1)
    stand.pop("absturz", None)  # derselbe Fehler darf wieder gemeldet werden, falls er zurückkommt
    try:
        melde(True, anzahl(len(neu), "neue Unterlage", "neue Unterlagen") if neu else "Nichts Neues in OLAT", dateien)
    except Exception:
        weiter(stand, 2)  # Hochladen ging trotz Nachfassen nicht (Netz, Server): in 2 Stunden nochmal statt erst in 12
        json_schreiben(STAND, stand)
        raise
    if ANZEIGE.an:
        name = lambda pfad: {"onlinetests.json": "Tests", KALENDER: "Kalender"}.get(pfad) or pfad.removeprefix("faecher/").removesuffix(".json")
        geschickt = [name(pf) for pf, g in ANZEIGE.geschickt if g and pf != "abgleich.json"]
        gleich = [name(pf) for pf, g in ANZEIGE.geschickt if not g and pf != "abgleich.json"]
        if geschickt:
            ANZEIGE.ok("In deine App geladen", f"{aufzaehlung(geschickt)} neu" + (f", {aufzaehlung(gleich)} unverändert" if gleich else ""))
        else:
            ANZEIGE.ok("Deine App war schon auf dem Stand")
        ANZEIGE.karte(alt, stand)
    else:
        print("\n".join(neu) or "Nichts Neues in OLAT.", "\nFertig – auf dem iPad die App neu laden.", sep="\n")
