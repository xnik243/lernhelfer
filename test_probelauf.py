"""Probelauf: das ganze PC-Programm als eigener Prozess, wie beim Doppelklick, gegen ein nachgebautes OLAT und einen nachgebauten
Lernhelfer-Server auf 127.0.0.1. Fängt, was Einzeltests nicht sehen, etwa einen Fehler im Hauptteil, der erst bei der ersten
Warnung den ganzen Abgleich abbricht (25.11.2026). Läuft vor jedem Commit von holen.py (.git/hooks/pre-commit).

  gut:     ein Dokument, drei Testbausteine (Rückgabe ohne ladbare Datei, Rückgabe als HTML, kaputter Link), der OLAT-Kalender → alles kommt an
  absturz: der Server lehnt onlinetests.json ab → Fehlerbericht an /api/fehler und „abgebrochen“ in abgleich.json"""
import base64, hashlib, json, os, shutil, subprocess, sys, tempfile, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote

HIER = Path(__file__).parent
DRIN = "<p>Eingeloggt als Probe</p>"
li = lambda i, titel, icon: f'<li data-nodeid="{i}"><div><span class="o_tree_link"><a href="#"><i class="o_icon {icon}"></i> {titel}</a></span></div></li>'
SEITEN = {
    "/auth/MyCoursesSite/0": DRIN + '<a class="o_sel_user_tools-calendar" href="/auth/HomeSite/1/calendar/0">Kalender</a>',
    # wie die echte Seite (Kalender › „Persönlicher aggregierter iCal Feed-Link“, 26.11.2026): der Knopf öffnet eine Sprechblase mit dem Link
    "/auth/HomeSite/1/calendar/0": DRIN + '<button class="fc-aggregatedfeed-button" onclick="feed()">Feed</button>'
        "<script>function feed() { const d = document.createElement('div'); d.className = 'o_callout_content o_cal_event_callout';"
        " d.innerHTML = '<input type=text>'; d.firstChild.value = location.origin + '/ical/paggregated/1/geheim'; document.body.append(d); }</script>",
    "/auth/RepositoryEntry/222": DRIN + '<div id="o_main_center_content_inner"><a href="/files/VO1.txt">Herunterladen</a></div>',
    "/auth/RepositoryEntry/111": DRIN + '<div class="o_tree"><ul><li data-nodeid="0"><div><span class="o_tree_link"><a href="#"><i class="o_icon o_CourseModule_icon"></i> PS</a></span></div>'
        f'<ul>{li(1, "Testergebnisse", "o_gta_icon")}{li(2, "PS-Test 2", "o_gta_icon")}{li(3, "Kaputt", "o_gta_icon")}</ul></li></ul></div>',
    # OLAT hat die Dateiliste anders aufgebaut als erwartet: dann der Seitentext
    "/auth/RepositoryEntry/111/CourseNode/1": DRIN + '<div id="o_main_center_content_inner"><h4>Rückgabe und Feedback</h4><div id="o_step_review">'
        '<p>Dokumente für Sie</p><a href="javascript:;">Test 1 korrigiert.pdf</a><p>Punkte: 7 von 10</p></div></div>',
    # wie die echte Seite (Mathe › PS-2026-W › Testergebnisse, 25.11.2026): Links ohne Adresse, Laden nur per o_XHREvent-Klick
    "/auth/RepositoryEntry/111/CourseNode/2": DRIN + '<div id="o_main_center_content_inner"><div id="o_step_review_content">'
        '<p>Ihr:e Betreuer:in hat folgende Dokumente für Sie hinzugefügt:</p><ul class="list-unstyled"><li class="o_assessment_document">'
        '<div class="o_flex_item_max"><h5><a href="javascript:;" onclick="o_XHREvent(\'/auth/1:1:7:2:1/\',true,true,\'cid\',\'preview\')">PS2.html</a></h5>'
        'erstellt von Muster, Moritz am 20.10.2026, 23:56</div><div class="o_flex_item_fix o_assessment_document_actions">'
        '<a href="javascript:;" onclick="o_XHREvent(\'/auth/1:1:7:2:1/\',true,true,\'cid\',\'preview\')">Öffnen</a> '
        '<a href="javascript:;" onclick="o_XHREvent(\'/auth/1:1:8:2:1/\',true,true,\'cid\',\'download\')">Herunterladen</a></div></li></ul></div></div>'
        "<script>function o_XHREvent(pfad, a, b, k, cid) { fetch(pfad, {method: 'POST', headers: {'Content-Type': 'application/x-www-form-urlencoded'},"
        " body: 'cid=' + cid}).then(r => r.text()).then(u => location.href = u); }</script>",
    # OLAT hat seinen Klick-Aufruf geändert: nur dieser Test fällt aus, mit Warnung (der Fehlerweg, der am 25.11. alles abbrach)
    "/auth/RepositoryEntry/111/CourseNode/3": DRIN + '<div id="o_main_center_content_inner"><div id="o_step_review_content"><ul><li class="o_assessment_document">'
        '<h5><a href="javascript:;">fehlt.html</a></h5><div class="o_assessment_document_actions"><a href="javascript:;" onclick="neueApi()">Herunterladen</a>'
        '</div></li></ul></div></div>',
}
DATEIEN = {
    "/files/VO1.txt": "Grenzwerte und Ableitungen",
    "/ical/paggregated/1/geheim": "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Europe/Vienna:20261005T094500\r\nSUMMARY:Teamtreffen\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    # Aufbau wie die echten Klausurergebnisse der Mathe-PS-Tests (Prüfungsbeleg als eingebettetes Bild), Person erfunden
    "/files/PS2.html": "<h3>Klausurergebnisse</h3><table>\n<tr><td>Name:</td><td>Muster Max</td></tr>\n<tr><td>Matrikelnummer:</td><td>00000000</td></tr>\n"
        "<tr><td>Klausur-ID:</td><td>26111600000</td></tr>\n<tr><td>Punkte:</td><td>8</td></tr>\n</table>\n<h3>Auswertung</h3><table>\n"
        "<tr><td>Frage</td><td>Punkte</td></tr>\n<tr><td>1</td><td>8</td></tr>\n</table>\n<h3>Prüfungsbeleg</h3><img src=\"data:image/png;base64,iVBORw0KGgo=\">",
}


def attrappe(lehnt_ab=None):
    """OLAT und Lernhelfer-Server in einem; gibt (Server, Speicher, Fehlerberichte, OLAT-Klicks) zurück."""
    speicher, berichte, klicks = {}, [], []

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
            if p in DATEIEN:
                return self.antwort(200, DATEIEN[p].encode(), "application/octet-stream", {"Content-Disposition": f'attachment; filename="{p.rsplit("/", 1)[1]}"'})
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
            body = self.rfile.read(int(self.headers["Content-Length"]))
            if self.path.startswith("/auth/"):  # OLAT-Klick: „download“ antwortet mit der Adresse der Datei
                klicks.append(body.decode())
                return self.antwort(200, b"/files/PS2.html" if body == b"cid=download" else b"/", "text/plain")
            berichte.append((self.headers.get("Authorization"), json.loads(body)))
            self.antwort(200, b'{"ok": true}', "application/json")

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, speicher, berichte, klicks


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
        (gut, g_speicher, g_berichte, g_klicks), (kaputt, k_speicher, k_berichte, _) = attrappe(), attrappe(lehnt_ab="onlinetests.json")
        laeufe = [starten(Path(a), gut.server_port), starten(Path(b), kaputt.server_port)]  # beide gleichzeitig: halbe Wartezeit
        (g_aus, _), (k_aus, _) = [l.communicate(timeout=300) for l in laeufe]
        g_code, k_code = [l.returncode for l in laeufe]
        gut.shutdown(), kaputt.shutdown()

    assert g_code == 0, f"Probelauf gut endete mit {g_code}:\n{g_aus[-3000:]}"
    abgleich = json.loads(g_speicher["abgleich.json"])
    assert abgleich["ok"] is True, abgleich
    tests = {t["titel"]: t for t in json.loads(g_speicher["onlinetests.json"])}
    assert set(tests) == {"Testergebnisse", "PS2"}, f"{tests.keys()}\n{g_aus[-3000:]}"
    assert "Knopf „Herunterladen“ umgebaut" in g_aus and "NoneType" not in g_aus, f"Warnung zu „Kaputt“ unverständlich:\n{g_aus[-3000:]}"
    assert "Punkte: 7 von 10" in tests["Testergebnisse"]["startseite"] and "Punkte: 8" in tests["PS2"]["startseite"]
    hoch = g_speicher["onlinetests.json"].decode()
    assert not any(x in hoch for x in ("Muster Max", "00000000", "26111600000", "iVBOR")), "Name, Matrikelnummer, Klausur-ID oder Scan in der App"
    assert tests["PS2"]["bis"] == "2026-10-20", tests["PS2"]
    assert g_klicks == ["cid=download"], f"Testschutz: nur der eine Herunterladen-Klick darf OLAT erreichen, nicht {g_klicks}"
    assert "Grenzwerte und Ableitungen" in g_speicher["faecher/Mathematik.json"].decode(), "Unterlage kam nicht an"
    assert b"SUMMARY:Teamtreffen" in g_speicher.get("kalender/OLAT.ics", b""), f"OLAT-Kalender kam nicht an\n{g_aus[-3000:]}"
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
