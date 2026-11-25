"""Probelauf: das ganze PC-Programm als eigener Prozess, wie beim Doppelklick, gegen ein nachgebautes OLAT und einen nachgebauten
Lernhelfer-Server auf 127.0.0.1. Fängt, was Einzeltests nicht sehen, etwa einen Fehler im Hauptteil, der erst bei der ersten
Warnung den ganzen Abgleich abbricht (25.11.2026). Läuft vor jedem Commit von holen.py (.git/hooks/pre-commit).

  gut:     ein Dokument, drei Testbausteine (Rückgabe ohne ladbare Datei, Rückgabe als HTML, kaputter Link) → alles kommt an
  absturz: der Server lehnt onlinetests.json ab → Fehlerbericht an /api/fehler und „abgebrochen“ in abgleich.json"""
import base64, hashlib, json, os, shutil, subprocess, sys, tempfile, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote

HIER = Path(__file__).parent
DRIN = "<p>Eingeloggt als Probe</p>"
li = lambda i, titel, icon: f'<li data-nodeid="{i}"><div><span class="o_tree_link"><a href="#"><i class="o_icon {icon}"></i> {titel}</a></span></div></li>'
SEITEN = {
    "/auth/MyCoursesSite/0": DRIN,
    "/auth/RepositoryEntry/222": DRIN + '<div id="o_main_center_content_inner"><a href="/files/VO1.txt">Herunterladen</a></div>',
    "/auth/RepositoryEntry/111": DRIN + '<div class="o_tree"><ul><li data-nodeid="0"><div><span class="o_tree_link"><a href="#"><i class="o_icon o_CourseModule_icon"></i> PS</a></span></div>'
        f'<ul>{li(1, "Testergebnisse", "o_gta_icon")}{li(2, "PS-Test 2", "o_gta_icon")}{li(3, "Kaputt", "o_gta_icon")}</ul></li></ul></div>',
    "/auth/RepositoryEntry/111/CourseNode/1": DRIN + '<div id="o_main_center_content_inner"><h4>Rückgabe und Feedback</h4><div id="o_step_review">'
        '<p>Dokumente für Sie</p><a href="javascript:;">Test 1 korrigiert.pdf</a><p>Punkte: 7 von 10</p></div></div>',
    "/auth/RepositoryEntry/111/CourseNode/2": DRIN + '<div id="o_main_center_content_inner"><div id="o_step_review"><h4>Rückgabe und Feedback</h4>'
        '<div><a href="/files/PS2.html">PS2.html</a> <span>20.10.2026</span></div></div></div>',
    "/auth/RepositoryEntry/111/CourseNode/3": DRIN + '<div id="o_main_center_content_inner"><div id="o_step_review"><a href="/files/fehlt.html">fehlt.html</a></div></div>',
    "/files/PS2.html": "<h1>PS-Test 2</h1><p>Erreichte Punktzahl 8 von 10</p>",
}


def attrappe(lehnt_ab=None):
    """OLAT und Lernhelfer-Server in einem; gibt (Server, Speicher, Fehlerberichte) zurück."""
    speicher, berichte = {}, []

    class H(BaseHTTPRequestHandler):
        def antwort(self, code, daten=b"", art="text/html; charset=utf-8", kopf=None):
            self.send_response(code)
            self.send_header("Content-Type", art)
            for k, v in (kopf or {}).items():
                self.send_header(k, v)
            self.end_headers()
            self.wfile.write(daten)

        def do_GET(self):
            p = self.path.split("?")[0].rstrip("/")
            if p == "/files/VO1.txt":
                return self.antwort(200, "Grenzwerte und Ableitungen".encode(), "text/plain", {"Content-Disposition": 'attachment; filename="VO1.txt"'})
            if p == "/api/tarife":
                return self.antwort(200, b'{"olat": true}', "application/json")
            if p.startswith("/api/gh/"):
                pfad = unquote(p.split("/contents/", 1)[1])
                return self.antwort(200, speicher[pfad], "application/json") if pfad in speicher else self.antwort(404, b'{"message": "Not Found"}', "application/json")
            s = SEITEN.get(p)
            self.antwort(200 if s else 404, f"<html><body>{s or 'fehlt'}</body></html>".encode())

        def do_PUT(self):
            pfad = unquote(self.path.split("/contents/", 1)[1])
            daten = base64.b64decode(json.loads(self.rfile.read(int(self.headers["Content-Length"])))["content"])
            if pfad == lehnt_ab:
                return self.antwort(400, b'{"message": "abgelehnt"}', "application/json")
            speicher[pfad] = daten
            self.antwort(201, json.dumps({"content": {"sha": hashlib.sha1(daten).hexdigest()}}).encode(), "application/json")

        def do_POST(self):
            berichte.append((self.headers.get("Authorization"), json.loads(self.rfile.read(int(self.headers["Content-Length"])))))
            self.antwort(200, b'{"ok": true}', "application/json")

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, speicher, berichte


def starten(ordner, port):
    """holen.py frisch in einen leeren Ordner, wie nach dem Einrichten; Kurse und Kurs-IDs vorgegeben, damit er nicht sucht."""
    shutil.copy(HIER / "holen.py", ordner / "holen.py")
    (ordner / "kurse.json").write_text(json.dumps({"Mathematik": [{"url": "222", "name": "VO Mathe", "art": "folien"}]}), "utf-8")
    (ordner / ".stand.json").write_text(json.dumps({"kurs_ids": {"Mathematik": {"111": "PS"}}}), "utf-8")
    (ordner / "zugang.json").write_text(json.dumps({"api": f"http://127.0.0.1:{port}/api/gh", "repo": "lernhelfer/konto", "token": "probe",
                                                    "app": f"http://127.0.0.1:{port}/"}), "utf-8")
    env = {**os.environ, "LH_OLAT": f"http://127.0.0.1:{port}", "PYTHONUTF8": "1"}
    return subprocess.Popen([sys.executable, "holen.py"], cwd=ordner, env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, encoding="utf-8", errors="replace")


def test_probelauf():
    with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
        (gut, g_speicher, g_berichte), (kaputt, k_speicher, k_berichte) = attrappe(), attrappe(lehnt_ab="onlinetests.json")
        laeufe = [starten(Path(a), gut.server_port), starten(Path(b), kaputt.server_port)]  # beide gleichzeitig: halbe Wartezeit
        (g_aus, _), (k_aus, _) = [l.communicate(timeout=300) for l in laeufe]
        g_code, k_code = [l.returncode for l in laeufe]
        gut.shutdown(), kaputt.shutdown()

    assert g_code == 0, f"Probelauf gut endete mit {g_code}:\n{g_aus[-3000:]}"
    abgleich = json.loads(g_speicher["abgleich.json"])
    assert abgleich["ok"] is True, abgleich
    tests = {t["titel"]: t for t in json.loads(g_speicher["onlinetests.json"])}
    assert set(tests) == {"Testergebnisse", "PS2"}, f"{tests.keys()}\n{g_aus[-3000:]}"
    assert "Punkte: 7 von 10" in tests["Testergebnisse"]["startseite"] and "8 von 10" in tests["PS2"]["startseite"]
    assert "Grenzwerte und Ableitungen" in g_speicher["faecher/Mathematik.json"].decode(), "Unterlage kam nicht an"
    assert not g_berichte, f"Fehlerbericht ohne Absturz: {g_berichte}"

    assert k_code != 0, "Absturz-Lauf endete ohne Fehler"
    assert len(k_berichte) == 1, f"genau ein Fehlerbericht erwartet: {k_berichte}\n{k_aus[-3000:]}"
    schluessel, bericht = k_berichte[0]
    assert schluessel == "Bearer probe" and bericht["seite"] == "PC-Programm" and bericht["meldung"].startswith("HTTPError"), bericht
    assert "Zeile " in bericht["text"] and "holen.py " in bericht["geraet"] and "\\Users\\" not in json.dumps(bericht), bericht
    abgleich = json.loads(k_speicher["abgleich.json"])
    assert abgleich["ok"] is False and "abgebrochen" in abgleich["text"], abgleich
    print("ok probelauf (ganzes Programm: gut und Absturz)")


if __name__ == "__main__":
    test_probelauf()
