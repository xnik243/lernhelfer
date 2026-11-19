"""Holt neue Unterlagen aus OLAT und lädt ihren Text ins private Daten-Repo (über die GitHub-API, ohne git).

Einrichten: in der App unter Einrichten bzw. Konto. Windows: einrichten.ps1 (Python, dieses Skript und die Aufgabenplanung nach
%LOCALAPPDATA%\\Lernhelfer); Mac: einrichten-mac.sh (nach ~/Library/Lernhelfer, launchd statt Aufgabenplanung).
Start von Hand: python holen.py [Fach ...]; --auto = Lauf aus der Aufgabenplanung, --kurse = nur Kurse suchen.
Die Aufgabenplanung ruft alle 30 Minuten auf, OLAT sieht davon aber nur einen Abgleich etwa alle 12 Stunden, tagsüber und mit
Pausen zwischen den Seiten wie beim Klicken von Hand (faellig(), pause()); dazwischen geht keine einzige Anfrage an OLAT.
Ist die OLAT-Sitzung abgelaufen, öffnet sich Chrome (am Mac ohne Chrome: Chromium): dort selbst einloggen (SSO + Authenticator).
Automatischer Login (Standard, abschaltbar in der App): Der nächste Start von Hand fragt einmal Kennung, Passwort und
2FA-Schlüssel ab; danach loggt er sich unsichtbar selbst ein (siehe autologin()).

Tests: Gelesen werden die Startseiten (Zeitraum, Punkte, Versuche), nach Ablauf stehen Fragen und Lösung dort. Selbsttests
werden einmal angesehen und mit „Test unterbrechen“ + „Ok“ verlassen. Bewertete Tests startet er nie. Ein Wächter im
Browser lässt nur Lesen und genau den jeweils nächsten erlaubten Schritt durch – nie Speichern, Beenden oder Abbrechen, siehe erlaubt().
"""
import base64
import getpass
import hashlib
import hmac
import html
import json
import logging
import os
import random
import re
import struct
import subprocess
import sys
import time
import zipfile
from datetime import datetime, timedelta
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import parse_qs, quote, unquote, urljoin, urlparse
from urllib.request import Request, urlopen

if sys.platform == "darwin":  # Mac: Zertifikate aus dem Schlüsselbund wie in Safari (truststore lädt das Einrichten mit)
    try:
        import truststore
        truststore.inject_into_ssl()
    except ImportError:
        pass

BASIS = Path(__file__).parent
MATERIAL = BASIS / "material"
KURSDATEI = BASIS / "kurse.json"  # fehlt sie, sucht der erste Abgleich die Kurse selbst (kurse_finden)
KURSE = json.loads(KURSDATEI.read_text("utf-8")) if KURSDATEI.exists() else None
ZUGANG = BASIS / "zugang.json"  # {"repo": "name/lernhelfer-daten", "token": "github_pat_…", "api": Lernhelfer-Server oder null, "app": Adresse der App}, schreibt das Einrichten
STAND = BASIS / ".stand.json"  # letzter Abgleich, hochgeladene Dateiversionen, wann das Login-Fenster zuletzt kam
APP = "https://xnik243.github.io/lernhelfer/"
API = os.environ.get("LH_API", "https://api.github.com")  # der Prüfstand setzt hier eine Attrappe ein
SITZUNG = BASIS / ".olat-sitzung.json"
PROFIL = BASIS / ".olat-chrome"  # eigenes Chrome-Profil nur fürs OLAT-Login
if sys.platform == "darwin":  # Mac: Chrome, sonst das Chromium von Playwright (lädt das Einrichten, wenn Chrome fehlt)
    KANAL = "chrome" if Path("/Applications/Google Chrome.app").exists() else None
else:
    KANAL = "chrome" if any(Path(p, r"Google\Chrome\Application\chrome.exe").exists() for p in
                            (r"C:\Program Files", r"C:\Program Files (x86)", Path.home() / r"AppData\Local")) else "msedge"
BROWSER = {"chrome": "Chrome", "msedge": "Edge"}.get(KANAL, "Chromium")
GERAET, ABLAGE = ("Mac", "Schreibtisch") if sys.platform == "darwin" else ("PC", "Desktop")
HOCHGELADEN = "Hochgeladen/"  # Dateien, die die App selbst hochgeladen hat (iPad, Mac ohne Programm): bleiben beim Abgleich erhalten
OLAT = "https://lms.uibk.ac.at"
ARTEN = ["folien", "ps", "syllabus"]
TESTS = "Tests"  # material/<Fach>/Tests: Startseiten der Onlinetests, Fragen der Selbsttests
# Knöpfe, die im Test vor jedem Klick aus der Seite entfernt werden („Test unterbrechen“ bleibt, ist aber bis zum Schluss gesperrt)
GEFAHR = ".o_sel_assessment_item_submit, .o_sel_end_testpart, .o_sel_cancel_test, .o_sel_next_question"
logging.getLogger("pypdf").setLevel(logging.ERROR)  # harmlose Warnungen bei schief gebauten PDFs


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
    for q in sorted(KURSE[fach], key=lambda q: ARTEN.index(q["art"])):
        ordner = MATERIAL / fach / q["name"]
        for f in sorted(ordner.iterdir() if ordner.exists() else [], key=lambda f: natur(f.name)):
            try:
                t = text(f)
            except Exception as e:  # kaputte Datei soll den Rest nicht aufhalten
                t = f"(Text nicht lesbar: {e})"
            dateien.append({"datei": f"{q['name']}/{f.name}", "art": q["art"], "text": t})
    for f in sorted((MATERIAL / fach / TESTS).glob("*.txt"), key=lambda f: natur(f.name)):
        dateien.append({"datei": f"{TESTS}/{f.name}", "art": "test", "text": f.read_text("utf-8")})
    if fach == "Mathematik" and "mathe4wiwi" in zusatzquellen():
        dateien += mathe4wiwi(datetime.now())
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
    page.wait_for_timeout(random.uniform(2000, 5000))


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


def login_lesen():
    try:
        if sys.platform == "win32" and LOGIN.exists():
            return json.loads(_dpapi(LOGIN.read_bytes(), False))
        if sys.platform == "darwin":
            r = subprocess.run(["security", "find-generic-password", "-s", SCHLUESSELBUND, "-w"], capture_output=True, text=True)
            return json.loads(base64.b64decode(r.stdout.strip())) if r.returncode == 0 else None
    except (OSError, ValueError):
        return None
    return None


def login_speichern(login):
    roh = json.dumps(login).encode()
    if sys.platform == "win32":
        LOGIN.write_bytes(_dpapi(roh, True))
    else:  # Befehl über stdin an `security -i`, damit der Wert nie in der Prozessliste (ps) steht; Base64 braucht keine Anführungszeichen
        wert = base64.b64encode(roh).decode()
        subprocess.run(["security", "-i"], input=f'add-generic-password -U -s "{SCHLUESSELBUND}" -a lernhelfer -w {wert}\n',
                       text=True, capture_output=True)
        if login_lesen() != login:  # ponytail: ungetestet auf echtem Mac; klappt -i nicht, wie bisher über die Argumente (kurz in ps sichtbar)
            subprocess.run(["security", "add-generic-password", "-U", "-s", SCHLUESSELBUND, "-a", "lernhelfer", "-w", wert],
                           check=True, capture_output=True)


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
    seite.goto(SSO)
    if auf_idp(seite.url):  # sonst kennt der IdP die Anmeldung noch und schickt gleich zurück zu OLAT
        if not seite.locator("#password").count():
            return "die Login-Seite der Uni sah anders aus als erwartet"
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
                seite.wait_for_timeout(3500)
            feld.fill(totp(login["geheim"]))
            feld.press("Enter")
            try:
                seite.wait_for_url(lambda u: not auf_idp(u), timeout=30_000)
            except Exception:
                return "der 2FA-Code wurde nicht angenommen (2FA-Schlüssel prüfen)"
    seite.goto(OLAT + "/auth/MyCoursesSite/0")
    return None if seite.get_by_text("Eingeloggt als").count() else "OLAT hat die Anmeldung nicht angenommen"


def profil(p, unsichtbar):
    """Das eigene Lernhelfer-Profil (Cookies der Uni-Anmeldung bleiben darin), sichtbar zum Einloggen von Hand."""
    return p.chromium.launch_persistent_context(PROFIL, channel=KANAL, headless=unsichtbar,
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


def autologin_einrichten(p, stand):
    """Einmal von Hand: Zugangsdaten abfragen, mit einem echten Login prüfen und erst dann verschlüsselt ablegen."""
    if sys.platform not in ("win32", "darwin"):
        print("Den automatischen Login gibt es nur unter Windows und macOS.")
        return None
    print(f"""
=== Automatischer OLAT-Login ===
Der Lernhelfer loggt sich dann selbst bei OLAT ein, auch mit dem 2FA-Code. Kennung, Passwort und 2FA-Schlüssel bleiben
verschlüsselt auf diesem {GERAET} ({'mit deinem Windows-Konto' if sys.platform == 'win32' else 'im Schlüsselbund'}) und gehen nirgendwohin.
Wer an deinen entsperrten {GERAET} kommt, kommt damit auch in deinen Uni-Account: {GERAET} immer mit Passwort sperren.
Notbremse (z. B. {GERAET} gestohlen): im Account Portal den Token löschen, den du gleich anlegst, oder das Uni-Passwort
ändern. Dann sind die gespeicherten Daten wertlos, egal wo sie gerade sind.

Den 2FA-Schlüssel bekommst du so:
  1. https://accounts.uibk.ac.at öffnen, einloggen, bei der Zwei-Faktor-Authentifizierung auf „Verwalten“, dann „Neuer Token“.
  2. Unter dem QR-Code auf „hier“ klicken. Das kopiert den geheimen Schlüssel.
  3. Hier einfügen. Danach zeigt der Lernhelfer einen 6-stelligen Code: den im Account Portal eingeben und „Verifizieren“.
Deine Authenticator-App am Handy funktioniert weiter wie bisher.
Nichts eingeben und Enter = überspringen, dann loggst du dich wie bisher selbst ein.
""")
    benutzer = input("Benutzerkennung (z. B. csab1234): ").strip()
    if not benutzer:
        return None
    passwort = getpass.getpass("Passwort (bleibt beim Tippen unsichtbar): ")
    geheim = getpass.getpass("2FA-Schlüssel (einfügen, bleibt unsichtbar): ").strip()
    if geheim.startswith("otpauth://"):
        geheim = parse_qs(urlparse(geheim).query).get("secret", [""])[0]
    try:
        totp(geheim)
        assert len(re.sub(r"[\s=-]", "", geheim)) >= 16
    except (ValueError, AssertionError):
        print("Das sieht nicht nach einem 2FA-Schlüssel aus (Buchstaben A–Z und Ziffern 2–7). Nichts gespeichert.")
        return None
    while True:
        print(f"\nCode fürs Account Portal: {totp(geheim)}  (gilt noch {30 - int(time.time() % 30)} Sekunden)")
        antwort = input("Hat das Portal ihn angenommen (oder war der Schlüssel schon eingerichtet)? Dann „ok“, sonst Enter für den aktuellen Code: ")
        if antwort.strip().lower() in ("ok", "j", "ja"):
            break
    login = {"benutzer": benutzer, "passwort": passwort, "geheim": geheim}
    print("Teste den Login bei OLAT …", flush=True)
    fenster = profil(p, unsichtbar=True)
    try:
        fenster.clear_cookies(domain=IDP)  # sonst kennt der IdP die Anmeldung noch und prüft gar nichts
        grund = auto_einloggen(fenster.pages[0] if fenster.pages else fenster.new_page(), login)
        if grund is None:
            fenster.storage_state(path=SITZUNG)
    except Exception as e:
        grund = f"unterbrochen ({type(e).__name__})"
    finally:
        fenster.close()
    if grund:
        print(f"Das hat nicht geklappt: {grund}. Nichts gespeichert, du loggst dich wie bisher selbst ein.")
        return None
    login_speichern(login)
    stand.pop("autologin_fehler", None)
    print("Gespeichert. Ab jetzt loggt sich der Lernhelfer selbst bei OLAT ein.\n")
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
    browser = p.chromium.launch(channel=KANAL, headless=True)
    for versuch in (1, 2):
        ctx = browser.new_context(storage_state=SITZUNG if SITZUNG.exists() else None, accept_downloads=True)
        page = ctx.new_page()
        page.goto(OLAT + "/auth/MyCoursesSite/0")
        if page.get_by_text("Eingeloggt als").count():  # OLAT zeigt die Login-Seite unter derselben URL
            return browser, ctx, page
        ctx.close()
        if versuch == 1 and login:
            fenster = profil(p, unsichtbar=True)
            try:
                grund = auto_einloggen(fenster.pages[0] if fenster.pages else fenster.new_page(), login)
                if grund is None:
                    fenster.storage_state(path=SITZUNG)
                    print("Automatisch bei OLAT eingeloggt.", flush=True)
                    continue
                login_fehler(grund)
            except Exception as e:  # Netz, Zeitüberschreitung: nichts Falsches abgeschickt, nächstes Mal wieder automatisch
                print("Automatischer Login unterbrochen:", type(e).__name__, str(e).split("\n")[0][:200], flush=True)  # nur die erste Zeile, ohne Seiteninhalt
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
        seite.get_by_text("Eingeloggt als").wait_for(timeout=600_000)
        fenster.storage_state(path=SITZUNG)
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
    kurse.json) behalten ihr Fach, damit Lernstand und Ausgaben in der App nicht plötzlich unter neuem Namen stehen."""
    bekannt = {q["url"].split("/")[0]: f for f, liste in (alt or {}).items() for q in liste if not re.search(r'[\\/:*?"<>|]', f)}  # „Physik I: …“ ging nie
    liste = [(k, t, f, (re.match(r"\d{4}[WS]\d+\s+([A-Z]{2})\b", t) or [None, ""])[1]) for k, t in kurse if (f := kursfach(t, jetzt))]
    arten = {}
    for _, _, f, typ in liste:
        arten.setdefault(f, []).append(typ)
    doppelt = {f for f, a in arten.items() if len(a) > len(set(a))}
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


def kurse_finden(page, jetzt=None, alt=None):
    """Baut kurse.json aus „Meine Kurse → Aktiv“: Ordner (bis 3 Unterordner-Ebenen), Dokumente und Seiten oben im Kurs.
    Klausurarchiv, Foren, Abgaben, Videos und Tests (die liest tests()) bleiben draußen. `alt`: bisherige kurse.json."""
    jetzt = jetzt or datetime.now()
    page.goto(OLAT + "/auth/MyCoursesSite/0/My/0")
    page.wait_for_load_state("networkidle")
    kurse = page.evaluate(r"""() => [...new Map([...document.querySelectorAll('#o_main_center_content_inner a[href*="RepositoryEntry/"]')]
      .filter(a => a.innerText.trim() && a.innerText.trim() !== 'Öffnen')
      .map(a => [a.href.match(/RepositoryEntry\/(\d+)/)[1], a.innerText.trim()])).entries()]""")
    neu = {}
    for kurs, titel, fach, typ in faecher_zuordnen(kurse, jetzt, alt):
        alle = knoten(page, kurs)
        wurzel = alle[0]["titel"] if alle else None
        for k in alle:
            if k["eltern"] in ("Klausurarchiv", "Abgaben") or not k["titel"]:
                continue
            url = f"{kurs}/CourseNode/{k['id']}"
            if k["icon"] == "o_bc_icon":
                eintraege = ordner_baum(page, url, k["titel"])
            elif (k["icon"] is None and not NICHT_LADEN.search(k["titel"])) or (k["icon"] == "o_sp_icon" and k["eltern"] == wurzel):
                eintraege = [(k["titel"], url)]
            else:
                continue
            for name, adresse in eintraege:
                neu.setdefault(fach, []).append({"name": re.sub(r'[\\/:*?"<>|]', "-", f"{typ} {name}".strip()),  # wird Ordnername
                                                 "art": kursart(titel, name), "url": adresse})
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
        try:
            neu += eintrag_laden(ctx, page, fach, q)
        except Exception as e:
            print(f"{fach}: {q['name']} nicht geladen ({fehlertext(e)})", flush=True)
    return neu


def eintrag_laden(ctx, page, fach, q):
    neu = []
    ziel = MATERIAL / fach / q["name"]
    ziel.mkdir(parents=True, exist_ok=True)
    url = f"{OLAT}/auth/RepositoryEntry/{q['url']}"
    oeffne(page, url)
    main = page.locator("#o_main_center_content_inner")
    herunterladen = main.locator("a:visible", has_text="Herunterladen")
    seite = main.locator("iframe[src*='/m/']:not([src*='viewer.html'])")  # HTML-Seite, kein PDF-Betrachter
    karten = main.locator(".o_folder_card:not(:has(.o_filetype_folder))")
    if main.locator(".o_folder_card").count():  # Ordner
        # Unterordner haben eigene Einträge (ordner_baum, bis 3 Ebenen tief);
        # vorhandene Dateien nicht neu geladen – geänderte Datei mit gleichem Namen: lokal löschen.
        namen = [n.strip() for n in karten.locator(".o_folder_card_title").all_inner_texts()]
        for titel in [n for n in namen if not (ziel / sicherer_name(n)).exists()]:
            oeffne(page, url)  # OLAT lädt die Seite nach jedem Download neu
            karten.filter(has=page.get_by_text(titel, exact=True)).locator("a:has(.o_icon_actions)").click()  # Karte über den Originaltitel
            with page.expect_download() as dl:
                page.get_by_role("link", name="Herunterladen", exact=True).last.click()
            dl.value.save_as(ziel / sicherer_name(titel))
            neu.append(f"{fach}: {q['name']}/{sicherer_name(titel)}")
    elif herunterladen.count():  # einzelne Datei
        with page.expect_download() as dl:
            herunterladen.first.click()
        name = sicherer_name(dl.value.suggested_filename)
        if not (ziel / name).exists():
            neu.append(f"{fach}: {q['name']}/{name}")
        dl.value.save_as(ziel / name)
    elif seite.count():  # eingebettete HTML-Seite
        src = seite.first.get_attribute("src")
        name = sicherer_name(src.rsplit("/", 1)[-1])
        if not (ziel / name).exists():
            neu.append(f"{fach}: {q['name']}/{name}")
        (ziel / name).write_bytes(ctx.request.get(urljoin(OLAT, src)).body())
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
          eltern: a(li.parentElement.closest('li[data-nodeid]'))?.innerText.trim()}; })"""
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
    alle, offen = {}, [""]
    while offen:
        pause(page)
        page.goto(f"{OLAT}/auth/RepositoryEntry/{kurs}{offen.pop()}")
        page.wait_for_load_state("networkidle")
        for k in page.evaluate(KNOTEN_JS):
            if k["id"] not in alle:
                alle[k["id"]] = k
                if k["icon"] == "o_st_icon":
                    offen.append(f"/CourseNode/{k['id']}")
    return list(alle.values())


def test_lesen(page, frei, startknopf):
    """Startet bzw. setzt einen Test fort, zeigt jede Frage einmal an und unterbricht ihn danach mit Bestätigung.
    Freigegeben ist immer nur der nächste Schritt: Start → 'Frage X anzeigen' → 'Test unterbrechen' → 'Ok'.
    Speichern-, Beenden-, Abbrechen- und Weiter-Knöpfe werden vor jedem Klick aus der Seite entfernt."""
    knopf = page.locator("a.o_sel_start_qti21assessment")
    assert knopf.count() == 1 and knopf.inner_text().strip() == startknopf, "kein passender Startknopf"
    ereignis = lambda a: re.search(r"o_XHREvent\('([^']+)'.*?'cid','([^']+)'", a.get_attribute("onclick")).groups()
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

        for i in range(links.count()):
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
            lv = re.match(r"OLAT-Bewertung „.*?“ \((\w*)\)", t)
            aus.append({"fach": fach, "titel": f.stem, "art": "bewertung", "lv": lv and lv.group(1) or None, "startseite": t})
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


def tests(browser, fach):
    """Onlinetests und Selbsttests eines Fachs nach material/<Fach>/Tests – in eigener Sitzung mit Testschutz."""
    frei, neu = {}, []

    def waechter(route):
        r = route.request
        body = r.post_data_buffer.decode(errors="ignore") if r.post_data_buffer else ""
        if "mathjax" in r.url.lower() or not erlaubt(r.method, r.url, body, frei):  # ohne MathJax bleibt LaTeX lesbar
            return route.abort()
        route.continue_()

    ctx = browser.new_context(storage_state=SITZUNG)
    ctx.route("**/*", waechter)
    page = ctx.new_page()
    ziel = MATERIAL / fach / TESTS
    ziel.mkdir(parents=True, exist_ok=True)
    # Art der Lehrveranstaltung je Kurs (VO, PS …) aus den Namen in kurse.json, für die Bewertungen
    arten = {q["url"].split("/")[0]: q["name"].split()[0] for q in KURSE[fach] if re.match(r"(VO|PS|UE|VU|SE|UV|KS) ", q["name"])}
    namen = set()  # „Onlinetest 1“ in VO und PS desselben Fachs: sonst überschreiben sie sich bei jedem Abgleich gegenseitig
    for kurs in sorted({q["url"].split("/")[0] for q in KURSE[fach]}):
        for k in knoten(page, kurs):
            # ponytail: Altklausuren (Klausurarchiv) übersprungen – eigener Prüfungsmodus, falls gewünscht
            if k["icon"] not in ("o_iqtest_icon", "o_iqself_icon", "o_ms_icon") or k["eltern"] == "Klausurarchiv":
                continue
            name = re.sub(r'[\\/:*?"<>|]', "-", k["titel"])
            if name in namen:
                name += f" ({arten.get(kurs) or kurs})"
            namen.add(name)
            datei, fragen = ziel / f"{name}.txt", ziel / f"{name} – Fragen.txt"
            if k["icon"] == "o_iqself_icon" and datei.exists():  # Selbsttests ändern sich nicht
                continue
            pause(page)
            page.goto(f"{OLAT}/auth/RepositoryEntry/{kurs}/CourseNode/{k['id']}")
            page.wait_for_load_state("networkidle")
            try:
                if k["icon"] == "o_ms_icon":  # Bewertung: nur lesen, die Seite hat für Teilnehmende nichts zum Abschicken
                    # ponytail: nur Bewertungs-Bausteine; Punkte in Aufgaben-Bausteinen (o_gta_icon) erst, wenn eine echte Seite dazu vorliegt
                    seite = page.locator("#o_main_center_content_inner").evaluate(LESEN_JS)
                    t = f"OLAT-Bewertung „{k['titel']}“ ({arten.get(kurs, '')})\n\n" + seite
                elif k["icon"] == "o_iqself_icon":
                    t = f"Selbsttest „{k['titel']}“ (anonyme, unbewertete Übung) – Fragen ohne Lösung\n\n" + test_lesen(page, frei, "Selbsttest starten")
                else:
                    seite = page.locator("#o_main_center_content_inner").evaluate(LESEN_JS)
                    t = f"OLAT-Test „{k['titel']}“ – Startseite (ob er bewertet wird, steht im Text)\n\n" + seite
                    # nach Abschluss stehen Fragen samt Lösung auf der Startseite; Fragen-Dateien älterer Fassungen gehen weg
                    fragen.unlink(missing_ok=True)
            except Exception as e:  # ein Test soll den Rest nicht aufhalten
                print(f"{fach}: {k['titel']} nicht lesbar ({fehlertext(e)})", flush=True)
                continue
            if not datei.exists() or datei.read_text("utf-8") != t:
                neu.append(f"{fach}: {TESTS}/{datei.name}")
                datei.write_text(t, "utf-8")
    ctx.close()
    return neu


def api(methode, pfad, body=None, roh=False):
    """GitHub-API mit dem Token aus zugang.json (darf nur das eigene Daten-Repo lesen und schreiben); roh: Dateiinhalt als Bytes, auch über 1 MB."""
    z = json.loads(ZUGANG.read_text("utf-8"))
    anfrage = Request(f"{z.get('api') or API}/repos/{z['repo']}/contents/{quote(pfad)}", method=methode, data=body and json.dumps(body).encode(),
                      headers={"Authorization": "Bearer " + z["token"], "Accept": "application/vnd.github.raw+json" if roh else "application/vnd.github+json",
                               "User-Agent": "Lernhelfer"})
    with urlopen(anfrage, timeout=120) as r:
        return r.read() if roh else json.loads(r.read())


def hochgeladene(fach):
    """Was die App unter „Unterlagen“ ins Fach hochgeladen hat; der Abgleich überschreibt die Datei und nimmt das mit."""
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
        return
    for versuch in (1, 2):
        body = {"message": f"OLAT-Abgleich {datetime.now():%d.%m.%Y %H:%M}: {pfad}", "content": base64.b64encode(daten).decode()}
        if shas.get(pfad):
            body["sha"] = shas[pfad]
        try:
            shas[pfad] = api("PUT", pfad, body)["content"]["sha"]
            return
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
        Path(__file__).write_bytes(neu)
        print("holen.py aktualisiert.")
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
        return f
    except OSError:
        f.close()
        return None


if __name__ == "__main__":
    from playwright.sync_api import TimeoutError as Zeitueberschreitung, sync_playwright

    auto = "--auto" in sys.argv  # aus der Aufgabenplanung bzw. launchd: ohne Konsole, Ausgaben ins Log
    if auto and not faellig(json.loads(STAND.read_text("utf-8")) if STAND.exists() else {}, datetime.now()):
        sys.exit()  # noch nicht dran: OLAT bleibt in Ruhe, auch kein Wachhalten der Sitzung
    if auto:
        sys.stdout = sys.stderr = open(BASIS / "holen.log", "a", encoding="utf-8", buffering=1)
        print(f"--- {datetime.now():%d.%m.%Y %H:%M}")
    SPERRE = sperre()
    for i in range(0 if auto else 240):  # von Hand: bis zu 20 Minuten warten, bis der Lauf im Hintergrund fertig ist
        if SPERRE:
            break
        if not i:
            print("Gerade gleicht der Lernhelfer im Hintergrund ab. Warte, bis er fertig ist …", flush=True)
        time.sleep(5)
        SPERRE = sperre()
    if not SPERRE:
        print("Es läuft schon ein Abgleich.")
        sys.exit(0 if auto else 1)
    if not olat_erlaubt():
        print("Der Abgleich mit OLAT ist gerade pausiert (Lernhelfer-Server). Er läuft von selbst wieder, sobald er freigegeben ist.", flush=True)
        if auto:
            aktualisieren()  # neue Fassungen kommen trotzdem an, ohne OLAT zu berühren
        sys.exit(0 if auto else 1)
    stand = json.loads(STAND.read_text("utf-8")) if STAND.exists() else {}
    alt = stand.get("abgleich", {})
    jetzt = datetime.now()

    def melde(ok, text, dateien=()):
        stand["abgleich"] = {"zeit": datetime.now().isoformat(timespec="minutes"), "ok": ok, "text": text,
                             "autologin": stand.get("autologin_an") and {"eingerichtet": bool(login_lesen()), "fehler": stand.get("autologin_fehler")}}
        try:
            for pfad, inhalt in [*dateien, ("abgleich.json", stand["abgleich"])]:
                hochladen(stand, pfad, json.dumps(inhalt, ensure_ascii=False).encode())
        finally:
            STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")

    with sync_playwright() as p:
        # Login-Fenster aus der Aufgabenplanung höchstens alle 8 Stunden (z. B. morgens nach dem Einschalten), sonst nervt es
        def fenster_auf():
            stand["fenster"] = datetime.now().isoformat(timespec="minutes")
            STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")

        def login_fehler(grund):
            print("Automatischer Login ging nicht:", grund, flush=True)
            stand["autologin_fehler"] = grund
            STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")
        try:
            browser, ctx, page = olat_login(p, sichtbar=not auto or jetzt - datetime.fromisoformat(stand.get("fenster", "2000-01-01")) > timedelta(hours=8),
                                            fenster_auf=fenster_auf, login=autologin(p, stand, von_hand=not auto), login_fehler=login_fehler)
        except (Abgelaufen, Zeitueberschreitung) as e:
            print("OLAT-Anmeldung abgelaufen.", type(e).__name__)
            weiter(stand, 4)  # abgelaufen: in rund 4 Stunden wieder versuchen, von Hand jederzeit
            STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")
            text = (f"Automatischer OLAT-Login ging nicht ({stand['autologin_fehler']}) – am {GERAET} „Lernhelfer OLAT“ auf dem {ABLAGE} doppelklicken"
                    " und die Zugangsdaten neu eintragen oder selbst einloggen." if stand.get("autologin_fehler") else
                    f"OLAT-Anmeldung am {GERAET} abgelaufen – am {GERAET} „Lernhelfer OLAT“ auf dem {ABLAGE} doppelklicken und einloggen.")
            if (alt.get("ok") is not False or alt.get("text") != text) and ZUGANG.exists():
                melde(False, text)
            if auto:
                aktualisieren()  # sonst käme eine neue Fassung erst nach dem nächsten Login an
            sys.exit(0 if auto else 1)
        ctx.storage_state(path=SITZUNG)
        weiter(stand, 12)  # gleich speichern: bricht der Lauf ab, kommt OLAT trotzdem erst in 12 Stunden wieder dran
        STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")
        if KURSE is None or "--kurse" in sys.argv or kurse_suchen(stand, jetzt):
            try:
                KURSE = kurse_finden(page, jetzt, KURSE)
                KURSDATEI.write_text(json.dumps(KURSE, ensure_ascii=False, indent=2), "utf-8")
                stand["kurse_semester"] = semester(jetzt)
                print("Gefundene Kurse:", *(f"  {f}: {', '.join(q['name'] for q in l)}" for f, l in KURSE.items()), sep="\n")
            except Exception as e:  # gibt es schon Kurse, gleicht er mit denen ab, statt ganz auszufallen; nächster Versuch in einer Woche
                if KURSE is None:
                    raise
                print("Kurse neu suchen ging nicht, es bleibt bei den bisherigen:", fehlertext(e), flush=True)
            stand["kurse_zeit"] = jetzt.isoformat(timespec="minutes")
            if "--kurse" in sys.argv:
                STAND.write_text(json.dumps(stand, ensure_ascii=False), "utf-8")
                sys.exit()
        if auto:
            aktualisieren()
        faecher = [a for a in sys.argv[1:] if not a.startswith("--")] or list(KURSE)
        neu = [n for f in faecher for n in sync(ctx, page, f)]
        ctx.storage_state(path=SITZUNG)
        neu += [n for f in faecher for n in tests(browser, f)]
        browser.close()
    tests_alle = [t for f in KURSE for t in testliste(f)]
    melde(True, f"{len(neu)} neue Unterlagen" if neu else "Nichts Neues in OLAT",
          [(f"faecher/{f}.json", fach_json(f, hochgeladene(f))) for f in faecher] + [("onlinetests.json", tests_alle)])
    print("\n".join(neu) or "Nichts Neues in OLAT.", "\nFertig – auf dem iPad die App neu laden.", sep="\n")
