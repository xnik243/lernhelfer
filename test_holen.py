import inspect
import tempfile
import zipfile
from pathlib import Path
from urllib.error import HTTPError

import holen


def test():
    assert sorted(["vo10.pdf", "vo2.pdf", "vo1.pdf"], key=holen.natur) == ["vo1.pdf", "vo2.pdf", "vo10.pdf"]
    with tempfile.TemporaryDirectory() as t:
        t = Path(t)
        with zipfile.ZipFile(t / "s.docx", "w") as z:
            z.writestr("word/document.xml", "<w:p><w:t>Hallo</w:t></w:p><w:p><w:t>Welt &amp; Co</w:t></w:p>")
        assert holen.text(t / "s.docx").split() == ["Hallo", "Welt", "&", "Co"]
        with zipfile.ZipFile(t / "bombe.pptx", "w", zipfile.ZIP_DEFLATED) as z:  # Zip-Bombe: wenige KB gepackt, ausgepackt zu groß
            z.writestr("ppt/slides/slide1.xml", "0" * (holen.ZIP_MAX + 1))
        assert (t / "bombe.pptx").stat().st_size < 1_000_000 and holen.text(t / "bombe.pptx") == "(Datei zu groß zum Auslesen)"
        (t / "s.html").write_text("<script>x()</script><p>Termin &amp; Thema</p>", "utf-8")
        assert holen.text(t / "s.html").split() == ["Termin", "&", "Thema"]

        holen.MATERIAL = t / "m"
        holen.KURSE = {"F": [{"name": "Syl", "art": "syllabus"}, {"name": "VO", "art": "folien"}]}
        for n in ("VO/vo10.pdf", "VO/vo2.txt", "Syl/a.html", "Tests/Thema 2.txt"):
            (t / "m/F" / n).parent.mkdir(parents=True, exist_ok=True)
            (t / "m/F" / n).write_bytes(b"x")
        (t / "m/F/Tests/Thema 2.txt").write_text("f(x)<3", "utf-8")
        d = holen.fach_json("F")["dateien"]
        assert [x["datei"] for x in d] == ["VO/vo2.txt", "VO/vo10.pdf", "Syl/a.html", "Tests/Thema 2.txt"]
        assert d[1]["text"].startswith("(Text nicht lesbar")  # kaputtes PDF bricht nicht ab
        assert d[3]["art"] == "test" and d[3]["text"] == "f(x)<3"  # Testtext bleibt unverändert
        # Rückgabe aus einem Aufgaben-Baustein: Art und Datum kommen aus dem Kopf, die Punkte liest die App
        (t / "m/F/Tests/PS-2a-Test.txt").write_text("OLAT-Bewertung „PS-2a-Test“ (PS)\nRückgabe vom 20.10.2026\n\nPunkte: 7", "utf-8")
        b = [x for x in holen.testliste("F") if x["art"] == "bewertung"]
        assert [(x["titel"], x["lv"], x["bis"]) for x in b] == [("PS-2a-Test", "PS", "2026-10-20")]

    # Testschutz mit echten OLAT-Anfragen (Selbsttest Mathematik, 14.10.2026)
    frei = {"start": "/auth/7:1:1109209465:2:1/", "selectItem": "o_fi1109214309"}
    form = "https://lms.uibk.ac.at/auth/7:1:1109214304:1:1:ofo_:fid/"
    erlaubt = holen.erlaubt
    assert erlaubt("GET", "https://lms.uibk.ac.at/auth/RepositoryEntry/1", None, {})
    assert erlaubt("POST", "https://lms.uibk.ac.at/auth/7:1:1109209465:2:1/", "cid=start&_csrf=x", frei)
    assert not erlaubt("POST", "https://lms.uibk.ac.at/auth/7:1:1109209465:2:1/", "cid=start&_csrf=x", {})  # bewertet: nie starten
    assert not erlaubt("POST", "https://lms.uibk.ac.at/auth/8:1:999:2:1/", "cid=start&_csrf=x", frei)  # fremder Start
    assert erlaubt("POST", form, "_csrf=x&dispatchuri=o_fi1109214309&dispatchevent=2&cid=selectItem&item=a:63:1", frei)
    for gefahr in ("dispatchuri=o_fi1109151020&dispatchevent=2",  # Antwort speichern
                   "dispatchuri=o_fi1109151024&dispatchevent=2",  # Test beenden
                   "dispatchuri=o_fi1109151026&dispatchevent=2",  # Test abbrechen
                   "dispatchuri=o_fi1109151027&dispatchevent=2",  # Test unterbrechen
                   "dispatchuri=o_fi1109214309&cid=nextItem",  # Nächste Frage
                   "dispatchuri=o_fi1109214309&cid=mark&item=a",  # Markierung
                   "dispatchuri=o_fi1109151020&cid=selectItem",  # Speichern-Knopf mit fremdem cid
                   "dispatchuri=o_fi1109214309&cid=selectItem&qtiworks_response_1=47",  # Navigation mit Antwort
                   "_csrf=x&oow=1109141558", ""):  # Autosave, leerer POST
        assert not erlaubt("POST", form, gefahr, frei), gefahr
    mp = lambda uri: (f'--b\r\nContent-Disposition: form-data; name="dispatchuri"\r\n\r\n{uri}\r\n'
                      '--b\r\nContent-Disposition: form-data; name="cid"\r\n\r\nselectItem\r\n--b--')
    assert erlaubt("POST", form, mp("o_fi1109214309"), frei) and not erlaubt("POST", form, mp("o_fi1109151020"), frei)

    # Unterbrechen (Formular-Knopf ohne cid) und „Ok“ im Dialog – echte Anfragen vom 14.10.2026
    presented = "&qtiworks_presented_ooMathe_item_63_RESPONSE_1=1"
    unterbrechen = {None: "o_fi1110563221"}
    assert erlaubt("POST", form, "dispatchuri=o_fi1110563221&dispatchevent=2&_csrf=x" + presented, unterbrechen)
    assert erlaubt("POST", form, "dispatchuri=o_fi1110563221&dispatchevent=2&qtiworks_response_1=", unterbrechen)  # leeres Feld
    for gefahr in ("dispatchuri=o_fi1110563221&dispatchevent=2&qtiworks_response_1=47",  # Unterbrechen mit Antwort
                   "dispatchuri=o_fi1110563220&dispatchevent=2",  # Test beenden, während Unterbrechen frei ist
                   "_csrf=x&oow=1109141558"):  # Autosave
        assert not erlaubt("POST", form, gefahr, unterbrechen), gefahr
    assert not erlaubt("POST", form, "dispatchuri=o_fi1110563221&dispatchevent=2", frei)  # Unterbrechen erst am Schluss
    ok = {"link_0": "/auth/22:1:1110563449:2:1/"}
    assert erlaubt("POST", "https://lms.uibk.ac.at/auth/22:1:1110563449:2:1/", "cid=link_0&_csrf=x", ok)
    assert not erlaubt("POST", "https://lms.uibk.ac.at/auth/22:1:1110563450:2:1/", "cid=link_1&_csrf=x", ok)  # Abbrechen
    assert not erlaubt("POST", "https://lms.uibk.ac.at/auth/22:1:1110563451:2:1/", "cid=link_0&_csrf=x", ok)  # anderer Dialog

    # Bewertete Tests startet das PC-Programm nie, nur Selbsttests
    assert not hasattr(holen, "laeuft") and '"Test starten"' not in inspect.getsource(holen.tests)

    # Zeitraum von der Startseite
    d = holen.datetime
    vwl2 = "Informationen\n\nMax. Punktzahl: 3 Punkte\n\nVerfügbarkeit: 15.10.2026, 16:30 bis 19.10.2026, 18:00\n\nMax. Lösungsversuche: 1 | Verbleibend: 1\n\nTest starten"
    assert holen.zeitraum("Informationen\n\nVerfügbarkeit: 15.10.2026, 16:30 bis 19.10.2026, 18:00 (wird automatisch eingezogen)") \
        == (d(2026, 10, 15, 16, 30), d(2026, 10, 19, 18, 0))
    assert holen.zeitraum("Informationen\nVerfügbarkeit: Bis 19.10.2026, 08:00 (wird automatisch eingezogen)") == (None, d(2026, 10, 19, 8, 0))
    assert holen.zeitraum("Informationen\nMax. Lösungsversuche: 100 | Verbleibend: 100") == (None, None)
    assert holen.zeitraum("Informationen\nVerfügbarkeit: Abgelaufen am 12.10.2026, 18:00") == (None, d(2026, 10, 12, 18, 0))
    assert holen.zeitraum("Informationen\nVerfügbarkeit: Ab 21.10.2026, 15:00") == (d(2026, 10, 21, 15, 0), None)

    # Automatisch nur etwa alle 12 Stunden und nur tagsüber
    assert holen.faellig({}, d(2026, 10, 20, 9, 0)) and not holen.faellig({}, d(2026, 10, 20, 3, 0)), "nachts nie"
    stand = {}
    holen.weiter(stand, 12)
    assert holen.timedelta(hours=11, minutes=59) < d.fromisoformat(stand["naechster"]) - d.now() <= holen.timedelta(hours=12, minutes=45)
    stand = {"naechster": "2026-10-20T21:10"}
    assert not holen.faellig(stand, d(2026, 10, 20, 21, 0)) and holen.faellig(stand, d(2026, 10, 20, 21, 15))
    assert not holen.faellig(stand, d(2026, 10, 20, 23, 30)) and holen.faellig(stand, d(2026, 10, 21, 7, 5))

    # Kurse finden: Fach aus dem Titel, nur aktuelles Semester
    jetzt = d(2026, 10, 14)
    assert holen.semester(jetzt) == "2026W" and holen.semester(d(2027, 2, 1)) == "2026W" and holen.semester(d(2027, 3, 1)) == "2027S"
    for titel, fach in [("2026W432001 VO Grundlagen der Volkswirtschaft", "VWL"), ("2026W436307 PS Einführung in die Betriebswirtschaft", "BWL"),
                        ("2026W433302 UE Wirtschaftssprachliche Lehrveranstaltung: Wirtschaftsenglisch 1: Communication In Bu", "Englisch"),
                        ("Mathematik", "Mathematik"), ("Tech Labs - Informatik in der Praxis", None),
                        ("2025W432001 VO Grundlagen der Volkswirtschaft", None), ("2026W400100 VU Einführung in die Soziologie", "Soziologie"),
                        ("2026W400200 VO Einführung in die Politikwissenschaft", "Politikwissenschaft"), ("2026W401000 VO Grundlagen der Psychologie", "Psychologie"),
                        ("2026W401100 VU Einführung in Python", "Python"), ("2026W403000 VO Allgemeine Pädagogik", "Allgemeine Pädagogik"),
                        # andere Studien: Recht nur als Wort, Namen ohne Doppelpunkt (Windows-Ordner) und nicht mitten im Wort abgeschnitten
                        ("2026W700100 VO Bürgerliches Recht I", "Recht"), ("2026W700400 VO Grundzüge des Privatrechts", "Recht"),
                        ("2026W800100 VO Gerechtigkeitstheorien", "Gerechtigkeitstheorien"), ("2026W500100 SE Aufrechte Haltung im Sport", "Aufrechte Haltung im Sport"),
                        ("2026W612000 VO Physik I: Mechanik und Wärme", "Physik I"), ("2026W650100 VO Allgemeine und Anorganische Chemie", "Allgemeine und Anorganische Chemie"),
                        ("2026W300101 PR Anatomie des Menschen – Präparierkurs", "Anatomie des Menschen"),
                        ("2026W900100 VO Wirtschafts- und Sozialgeschichte I/II", "Wirtschafts- und Sozialgeschichte I-II"), ("2026W436308 PS Grundzüge der BWL", "BWL")]:
        assert holen.kursfach(titel, jetzt) == fach, (titel, holen.kursfach(titel, jetzt))
    # Kurzname fasst nur zusammen, solange keine LV-Art doppelt ist (Jus: zwei VO); bekannte Kurse behalten ihr Fach
    meine = [("1", "2026W700100 VO Bürgerliches Recht I"), ("2", "2026W700200 VO Strafrecht"), ("3", "2026W700300 PS Bürgerliches Recht I"),
             ("4", "2026W436307 VO Einführung in die Betriebswirtschaft"), ("5", "2026W436308 PS Grundzüge der BWL"), ("6", "2025W1 VO Alt")]
    assert [f for _, _, f, _ in holen.faecher_zuordnen(meine, jetzt)] == ["Bürgerliches Recht I", "Strafrecht", "Bürgerliches Recht I", "BWL", "BWL"]
    assert holen.faecher_zuordnen(meine, jetzt, {"Recht": [{"url": "1/CourseNode/9"}]})[0][2:] == ("Recht", "VO")
    assert holen.faecher_zuordnen(meine, jetzt, {"Bürgerl: R": [{"url": "1/CourseNode/9"}]})[0][2] == "Bürgerliches Recht I"
    # Ein PS-Kurs ohne Dateien (nur Tests und Bewertungen) bleibt drin, damit tests() ihn besucht
    class OhneDateien:
        def goto(self, url): pass
        def wait_for_load_state(self, *_): pass
        def wait_for_timeout(self, ms): pass
        def evaluate(self, js): return [] if "o_tree" in js else [["7", "2026W433001 PS Mathematik für WiWi"]]
    ids = {}
    assert holen.kurse_finden(OhneDateien(), jetzt, None, ids) == {"Mathematik": []} and ids == {"Mathematik": {"7": "PS"}}
    # Kurse wöchentlich neu suchen, von Hand gepflegte kurse.json nie
    assert not holen.kurse_suchen({}, jetzt) and holen.kurse_suchen({"kurse_semester": "2026W"}, jetzt)
    assert not holen.kurse_suchen({"kurse_semester": "2026W", "kurse_zeit": "2026-10-10T09:00"}, jetzt)
    assert holen.kurse_suchen({"kurse_semester": "2026W", "kurse_zeit": "2026-10-06T09:00"}, jetzt)
    assert holen.kurse_suchen({"kurse_semester": "2026S", "kurse_zeit": "2026-10-13T09:00"}, jetzt)
    assert holen.fehlertext(AssertionError()) == "AssertionError" and holen.fehlertext(ValueError("a\nb")) == "a"
    assert holen.kursart("2026W436307 PS Einführung", "Folien") == "ps" and holen.kursart("Mathematik", "Ressourcen VO") == "folien"
    assert holen.kursart("Mathematik", "Syllabus") == "syllabus" and holen.kursart("Mathematik", "Ressourcen PS") == "ps"

    # Hochladen: unverändert = keine Anfrage; veraltete Prüfsumme (409) → aktuelle holen, nochmal
    anfragen, antworten = [], [HTTPError("u", 409, "", {}, None), {"sha": "alt2"}, {"content": {"sha": "neu"}}]
    def api(methode, pfad, body=None):
        anfragen.append((methode, body and body.get("sha")))
        a = antworten.pop(0)
        if isinstance(a, Exception):
            raise a
        return a
    zugang = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
    zugang.write('{"repo": "n/d", "token": "t", "api": "https://server/api/gh"}')
    zugang.close()
    holen.ZUGANG = Path(zugang.name)
    holen.api, stand = api, {"ziel": "https://server/api/gh/n/d", "sha": {"x.json": "alt"}}
    holen.hochladen(stand, "x.json", b"{}")
    assert anfragen == [("PUT", "alt"), ("GET", None), ("PUT", "alt2")] and stand["sha"]["x.json"] == "neu"
    stand["sha"]["x.json"] = holen.hashlib.sha1(b"blob 2\0{}").hexdigest()
    holen.hochladen(stand, "x.json", b"{}")
    assert len(anfragen) == 3  # gleiche Datei: nichts hochgeladen
    stand["ziel"] = "https://api.github.com/n/d"  # Prüfsummen aus der GitHub-Zeit: auf dem Server trotzdem hochladen
    antworten.append({"content": {"sha": "s"}})
    holen.hochladen(stand, "x.json", b"{}")
    assert anfragen[3] == ("PUT", None) and stand == {"ziel": "https://server/api/gh/n/d", "sha": {"x.json": "s"}}
    # Schalter in der Admin-Ecke: aus = kein OLAT; ohne Antwort wie bisher
    echt, gefragt = holen.urlopen, []
    class Antwort:
        def read(self):
            return b'{"olat": false}'
    holen.urlopen = lambda url, timeout: gefragt.append(url) or Antwort()
    assert holen.olat_erlaubt() is False and gefragt == ["https://server/api/tarife"]
    def kein_netz(url, timeout):
        raise OSError("kein Netz")
    holen.urlopen = kein_netz
    assert holen.olat_erlaubt() is True
    holen.urlopen = echt
    Path(zugang.name).unlink()

    # Testliste für den Onlinetest-Überblick der App
    with tempfile.TemporaryDirectory() as t:
        holen.MATERIAL = Path(t)
        (Path(t) / "M/Tests").mkdir(parents=True)
        (Path(t) / "M/Tests/Onlinetest 2.txt").write_text("OLAT-Test „Onlinetest 2“ – Startseite\n\n" + vwl2, "utf-8")
        (Path(t) / "M/Tests/Onlinetest 2 – Fragen.txt").write_text("Frage 1\nMax. 1 Punkt", "utf-8")
        (Path(t) / "M/Tests/Thema 1.txt").write_text("Selbsttest „Thema 1“ (anonyme, unbewertete Übung)\n\nFrage 1", "utf-8")
        ot, th = holen.testliste("M")
        assert (ot["von"], ot["bis"], ot["punkte"], ot["verbleibend"], ot["ergebnis"], ot["uebung"]) == ("2026-10-15T16:30", "2026-10-19T18:00", 3, 1, False, False)
        assert ot["fragen"] == "Frage 1\nMax. 1 Punkt" and th["art"] == "selbsttest" and th["fragen"].startswith("Selbsttest")

    # In der App Hochgeladenes bleibt beim Abgleich erhalten (roh gelesen, auch über 1 MB); fehlt die Datei, gibt es nichts
    roh = [b'{"dateien": [{"datei": "VO/a.pdf", "art": "folien", "text": "alt"}, {"datei": "Hochgeladen/b.pdf", "art": "folien", "text": "neu"}]}',
           HTTPError("u", 404, "", {}, None)]
    def api_roh(methode, pfad, body=None, roh_=None, **k):
        assert methode == "GET" and k.get("roh") and pfad.startswith("faecher/")
        a = roh.pop(0)
        if isinstance(a, Exception):
            raise a
        return a
    holen.api = api_roh
    assert holen.hochgeladene("F") == [{"datei": "Hochgeladen/b.pdf", "art": "folien", "text": "neu"}]
    assert holen.hochgeladene("G") == []
    with tempfile.TemporaryDirectory() as t:
        holen.MATERIAL, holen.KURSE = Path(t), {"F": []}
        assert holen.fach_json("F", [{"datei": "Hochgeladen/b.pdf"}])["dateien"] == [{"datei": "Hochgeladen/b.pdf"}]

    # Ordnerbaum: jeder Unterordner (bis 3 Ebenen) ein eigener Eintrag, Pfad mit ~~ statt /; Video-Ordner bleiben draußen
    baum = {"": ["VO", "PS"], "~~VO": ["Woche 1", "Aufzeichnungen"], "~~VO~~Woche%201": ["Übung"]}
    class Seite:
        def goto(self, url): self.pfad = url.split("path%3D")[1][:-2] if "path%3D" in url else ""
        def wait_for_load_state(self, *_): pass
        def wait_for_timeout(self, ms): assert 2000 <= ms <= 5000, "Pause wie von Hand"
        def locator(self, _): return self
        def all_inner_texts(self): return baum.get(self.pfad, [])
    assert holen.ordner_baum(Seite(), "1/CourseNode/2", "Material") == [
        ("Material", "1/CourseNode/2"), ("Material VO", "1/CourseNode/2/path%3D~~VO/0"),
        ("Material VO Woche 1", "1/CourseNode/2/path%3D~~VO~~Woche%201/0"),
        ("Material VO Woche 1 Übung", "1/CourseNode/2/path%3D~~VO~~Woche%201~~%C3%9Cbung/0"),
        ("Material PS", "1/CourseNode/2/path%3D~~PS/0")]

    # Nur ein Abgleich gleichzeitig
    with tempfile.TemporaryDirectory() as t:
        holen.BASIS = Path(t)
        erst = holen.sperre()
        assert erst and holen.sperre() is None, "zweiter Lauf wartet"
        erst.close()
        zweit = holen.sperre()
        assert zweit, "nach dem Ende wieder frei"
        zweit.close()
    # 2FA-Code nach RFC 6238 (Testwerte aus dem Anhang B, die letzten 6 Ziffern), Leerzeichen und Kleinschreibung egal
    geheim = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
    assert holen.totp(geheim, 59) == "287082" and holen.totp(geheim, 1111111109) == "081804"
    assert holen.totp("gezd gnbv gy3t qojq gezd gnbv gy3t qojq", 59) == "287082"
    if holen.sys.platform == "win32":  # DPAPI hin und zurück
        assert holen._dpapi(holen._dpapi(b'{"a": 1}', True), False) == b'{"a": 1}'
    # Versiegeln (Mac-Sitzung): hin und zurück, jede Änderung und ein fremder Schlüssel fallen auf
    k = bytes(range(32))
    for daten in (b"", b"x", b'{"cookies": [1]}' * 500):
        blob = holen.versiegeln(k, daten)
        assert holen.entsiegeln(k, blob) == daten and (len(daten) < 8 or daten not in blob)  # ein Byte steht zufällig oft im Chiffrat
        for falsch in (blob[:-1] + bytes([blob[-1] ^ 1]), bytes([blob[0] ^ 1]) + blob[1:], blob[:20]):
            try:
                holen.entsiegeln(k, falsch)
                raise AssertionError("verändert, aber angenommen")
            except ValueError:
                pass
        try:
            holen.entsiegeln(bytes(32), blob)
            raise AssertionError("fremder Schlüssel angenommen")
        except ValueError:
            pass
    assert holen.versiegeln(k, b"abc") != holen.versiegeln(k, b"abc")  # jedes Mal neue Nonce
    if holen.sys.platform == "win32":  # Sitzung: alte Klartext-Datei wird gelesen, beim Speichern verschlüsselt ersetzt
        with tempfile.TemporaryDirectory() as t:
            alt_s, alt_a = holen.SITZUNG, holen.SITZUNG_ALT
            holen.SITZUNG, holen.SITZUNG_ALT = Path(t) / ".olat-sitzung", Path(t) / ".olat-sitzung.json"
            try:
                assert holen.sitzung_lesen() is None
                holen.SITZUNG_ALT.write_text('{"cookies": [{"name": "JSESSIONID", "value": "geheim123"}]}', "utf-8")
                zustand = holen.sitzung_lesen()
                assert zustand["cookies"][0]["value"] == "geheim123"
                holen.sitzung_speichern(type("Ctx", (), {"storage_state": lambda self: zustand})())
                assert not holen.SITZUNG_ALT.exists() and b"geheim123" not in holen.SITZUNG.read_bytes()
                assert holen.sitzung_lesen() == zustand
                holen.SITZUNG.write_bytes(b"kaputt")
                assert holen.sitzung_lesen() is None  # dann eben neu einloggen
            finally:
                holen.SITZUNG, holen.SITZUNG_ALT = alt_s, alt_a

    # Dateinamen aus OLAT: nie ein Pfad, normale Namen bleiben
    for roh, rein in (("../../holen.py", "holen.py"), ("..%2F..%2Fx.bat", "x.bat"), ("a\\b.pdf", "b.pdf"), (".versteckt", "versteckt"),
                      ("..", "datei"), ("VO 3 – Märkte & Preise.pdf", "VO 3 – Märkte & Preise.pdf"), ("a:b?.pdf", "a-b-.pdf")):
        assert holen.sicherer_name(roh) == rein, (roh, holen.sicherer_name(roh))

    # Passwort und Code nur auf der echten Login-Seite der Uni
    assert holen.auf_idp("https://idp.uibk.ac.at/idp/profile/SAML2/Redirect/SSO?execution=e1s1")
    for falsch in ("https://boese.example/?idp.uibk.ac.at", "http://idp.uibk.ac.at/", "https://idp.uibk.ac.at.boese.example/",
                   "https://boese.example/idp.uibk.ac.at"):
        assert not holen.auf_idp(falsch), falsch
    getippt = []

    class Falsch:  # leitet auf eine fremde Seite um, die nur „idp.uibk.ac.at“ in der Adresse hat
        url = "https://boese.example/login?von=idp.uibk.ac.at"
        def goto(self, url): pass
        def locator(self, _): return self
        def get_by_text(self, _): return self
        def count(self): return 0  # auch kein „Eingeloggt als“
        def fill(self, *a): getippt.append(a)
    assert holen.auto_einloggen(Falsch(), {"benutzer": "csab1234", "passwort": "geheim", "geheim": geheim}) and not getippt

    # Selbst-Update nur mit gültiger Signatur (Testvektor 1 aus RFC 8032: leere Nachricht)
    rfc = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    sig = bytes.fromhex("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
    assert holen.signatur_gilt(b"", sig, rfc)
    assert not holen.signatur_gilt(b"x", sig, rfc) and not holen.signatur_gilt(b"", sig[:-1] + b"\0", rfc) and not holen.signatur_gilt(b"", sig[:40], rfc)
    assert not holen.signatur_gilt(b"", sig), "fremder Schlüssel"
    sig_datei = Path(holen.__file__).with_name("holen.py.sig")
    if sig_datei.exists():  # die Signatur im Repo passt zum eingetragenen Schlüssel (signiert wird der Git-Index, vor jedem Commit)
        import subprocess
        index = subprocess.run(["git", "show", ":holen.py"], capture_output=True, cwd=sig_datei.parent).stdout
        assert holen.signatur_gilt(index, holen.base64.b64decode(sig_datei.read_bytes())), "holen.py.sig veraltet: node signieren.mjs"
    with tempfile.TemporaryDirectory() as t:
        t, alt = Path(t), (holen.BASIS, holen.ZUGANG, holen.SIGNATUR, holen.urlopen, holen.__file__)
        holen.BASIS, holen.ZUGANG, holen.SIGNATUR, holen.__file__ = t, t / "zugang.json", rfc, str(t / "holen.py")

        def probe(app, inhalt, signatur):
            (t / "holen.py").write_bytes(b"# alt\n")
            holen.ZUGANG.write_text(holen.json.dumps({"app": app}), "utf-8")
            holen.urlopen = lambda url, timeout: type("A", (), {"read": lambda s: holen.base64.b64encode(signatur) if url.endswith(".sig") else inhalt})()
            holen.aktualisieren()
            return (t / "holen.py").read_bytes()
        assert probe("https://lernhelfer.example/", b"", sig) == b"", "gültig signiert → übernommen"
        assert probe("https://lernhelfer.example/", b"import os", sig) == b"# alt\n", "verändert → bleibt alt"
        assert probe("http://lernhelfer.example/", b"", sig) == b"# alt\n", "ohne HTTPS → bleibt alt"
        holen.BASIS, holen.ZUGANG, holen.SIGNATUR, holen.urlopen, holen.__file__ = alt

    # Automatischer Login: Standard aus; an nur mit Schalter UND abgehakten Hinweisen in der aktuellen Fassung
    def olat(antwort):
        def api(methode, pfad, body=None, roh=False):
            if isinstance(antwort, Exception):
                raise antwort
            return antwort
        holen.api = api
        return holen.schalter()
    assert olat(HTTPError("u", 404, "", {}, None)) is False, "ohne olat.json aus"
    assert olat('{"autologin": true}') is False, "alter Schalter ohne Zustimmung: aus"
    assert olat('{"autologin": true, "zustimmung": "1"}') is False
    assert olat('{"autologin": false, "zustimmung": 1}') is False
    assert olat('{"autologin": true, "zustimmung": %d}' % holen.ZUSTIMMUNG) is True
    assert olat(HTTPError("u", 500, "", {}, None)) is None, "keine Antwort"
    print("ok")


def vt(text, breite):
    """Mini-Terminal: \\r, \\n, ESC[nA, ESC[K, ESC[J; andere Steuerzeichen werden übergangen. → (Zeilen, Umbrüche)"""
    import re
    zeilen, r, c, i, umbrueche = [""], 0, 0, 0, 0
    while i < len(text):
        if text[i] == "\x1b":
            m = re.match(r"\x1b\[([0-9;?]*)([A-Za-z])|\x1b\][^\x07]*\x07", text[i:])
            if m.group(2) == "A":
                r = max(0, r - int(m.group(1) or 1))
            elif m.group(2) in ("K", "J"):
                zeilen[r] = zeilen[r][:c]
                if m.group(2) == "J":
                    del zeilen[r + 1:]
            i += len(m.group(0))
            continue
        ch = text[i]
        if ch == "\r":
            c = 0
        elif ch == "\n":
            r, c = r + 1, 0
        else:
            if c >= breite:
                r, c, umbrueche = r + 1, 0, umbrueche + 1
                zeilen += [""] * (r + 1 - len(zeilen))
            z = zeilen[r].ljust(c)
            zeilen[r] = z[:c] + ch + z[c + 1:]
            c += 1
        zeilen += [""] * (r + 1 - len(zeilen))
        i += 1
    return zeilen, umbrueche


def test_anzeige():
    """Konsolen-Anzeige: Live-Bereich bricht nie um, Fragen und Warnungen bleiben ganz, alte Konsole nur cp850, ohne Konsole still."""
    import io
    import os
    import re
    os.environ["LINES"] = "30"
    ohne = lambda s: re.sub(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07", "", s)

    def neu(spalten, alt=False):
        os.environ["COLUMNS"] = str(spalten)
        a = holen.Anzeige()
        a.raus, a.an, a.alt = io.StringIO(), True, alt
        return a

    for sp in (30, 40, 60, 72, 100, 140):  # jede Zeile des Bereichs passt, auch mit Emoji im Namen und in der Pause
        a = neu(sp)
        a.schritt("Unterlagen", 31, hinweis="geladen wird nur, was neu ist", faecher=["Mathematik", "VWL", "BWL", "Englisch", "Statistik"])
        a.jetzt("12 Dateien, 11 schon da · lädt " + "x" * 90 + ".pdf (1 von 1)", "VWL · PS Übungsblätter 🎓")
        a.wert, a.z["pausen"], a.z["pause_s"] = 14, 3, 9.5
        a.fund("VWL", "PS / Blatt05.pdf", groesse_b=310_000)
        for _ in range(2):
            for s, b in a._bereich():
                assert len(ohne(s)) == b <= sp - 1, (sp, ohne(s))
            a.pause(3000)
        a.schritt("Warte auf deinen Login", frist=600, hinweis="Kein Stress, das Fenster wartet bis zu 10 Minuten.")
        for s, b in a._bereich():
            assert len(ohne(s)) == b <= sp - 1, (sp, ohne(s))
        a.karte({}, {"naechster": "2026-11-23T23:40"})
        bild, umbrueche = vt(a.raus.getvalue(), sp)
        assert sp < 60 or not umbrueche, (sp, bild)  # bleibende Zeilen dürfen nur in schmalen Fenstern umbrechen

    a = neu(100)
    a.write("Frage: ")  # wie input(): Zeile offen, kein Bereich dazwischen
    a.schritt("Verbinde mit OLAT")
    a.write("ja\n")
    a.warnung("BWL: X nicht geladen")
    bild = [z for z in vt(a.raus.getvalue(), 100)[0] if z.strip()]
    assert "Frage: ja" in bild and "! BWL: X nicht geladen" in bild[-3] and "Verbinde mit OLAT" in bild[-2], bild

    a = neu(100)  # höchstens 3 Fundzeilen je Ordner, dann eine Sammelzeile
    a.schritt("Unterlagen", 2, faecher=["BWL"])
    for i in range(5):
        a.fund("BWL", f"VO Folien / f{i}.pdf", groesse_b=1_200_000)
    a.plus()
    a.ok("Unterlagen")
    bild = vt(a.raus.getvalue(), 100)[0]
    assert sum("+ BWL" in z for z in bild) == 4 and any("… und 2 weitere in VO Folien" in z and "2,4 MB" in z for z in bild), bild
    assert not any("Unterlagen " in z and "┄" in z for z in bild), "Bereich nach ok() weg"

    for sp in (80, 100, 120):  # Kurszeile: langer Titel wird gekürzt, die Angabe rechts bleibt
        a = neu(sp)
        a.kurs("Mathematik", "VO", "Mathematik für Wirtschaftswissenschaften und noch viel mehr Text", 4, 1, 3)
        assert "4 Ordner · 1 Dokument · 3 Tests" in ohne(a.raus.getvalue()), (sp, ohne(a.raus.getvalue()))

    a = neu(100)  # Bereich weg vor einer Frage (input/getpass schreiben an write() vorbei): Cursor wieder sichtbar
    a.schritt("Prüfe deine App")
    a.schritt(None)
    s = a.raus.getvalue()
    assert s.rfind("\x1b[?25h") > s.rfind("\x1b[?25l") >= 0, repr(s)

    a = neu(100, alt=True)  # alte Windows-Konsole: alles in Codepage 850
    a._kopf()
    a.schritt("Unterlagen", 3, hinweis="„x“ …", faecher=["Mathematik"])
    a.jetzt("lädt „x“ …", "Mathematik · VO")
    a.fund("Mathematik", "VO / a.pdf", groesse_b=2_400_000)
    a._zeichnen()
    a.zeile("fertig", "grau")
    a.fund("Mathematik", "Test „T“", art="neu", name="T")
    a.karte({"zeit": "2026-11-20T09:12"}, {})
    a.schritt("x")
    a.schluss()
    ohne(a.raus.getvalue()).encode("cp850")
    assert "Abgebrochen bei x" in a.raus.getvalue()

    b = holen.Anzeige()  # ohne Konsole (Log, Tests): raus ist None, jede Ausgabe würde scheitern
    b.schritt("x", 2)
    b.fund("BWL", "a / b.pdf", groesse_b=5)
    b.kurs("BWL", "VO", "t", 1, 0, 0)
    b.zeile("x")
    b.plus()
    b.ok("x")
    b.karte({}, {})
    assert b.funde == {"BWL": 1}
    print("ok anzeige")


def test_zugang():
    """Abgelehnter Schlüssel (neues PC-Programm geladen): klare Meldung vor OLAT statt Traceback am Ende."""
    with tempfile.TemporaryDirectory() as t:
        holen.ZUGANG = Path(t) / "zugang.json"
        holen.ZUGANG.write_text('{"api": "https://x/api/gh", "repo": "r", "token": "t"}', "utf-8")
        for fehler, erwartet in ((HTTPError("u", 401, "", {}, None), "PC-Programm neu laden"), (HTTPError("u", 404, "", {}, None), None),
                                 (OSError("offline"), None)):
            def api(*_, fehler=fehler, **__):
                raise fehler
            holen.api = api
            grund = holen.zugang_kaputt()
            assert (erwartet in grund) if erwartet else grund is None, grund
    print("ok zugang")


def test_schatten():
    """Keine Zuweisung auf Modulebene (auch im __main__-Block) überschreibt eine Funktion: „if grund := …“ machte grund() zu None,
    und die erste Warnung (Test nicht lesbar) brach jeden Abgleich ab."""
    import ast
    baum = ast.parse(Path(holen.__file__).read_text("utf-8"))
    funktionen = {n.name for n in baum.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
    eigener_bereich = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Lambda, ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)

    def modulebene(k):
        for c in ast.iter_child_nodes(k):
            if not isinstance(c, eigener_bereich):
                yield c
                yield from modulebene(c)
    schatten = [(n.id, n.lineno) for n in modulebene(baum) if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Store) and n.id in funktionen]
    assert not schatten, f"überschreibt Funktionen: {schatten}"
    print("ok schatten")


if __name__ == "__main__":
    test()
    test_anzeige()
    test_zugang()
    test_schatten()


ECHT_API = holen.api  # test_zugang ersetzt holen.api durch eine Attrappe


def test_robust():
    """Zustand übersteht Stromausfall beim Schreiben; das Hochladen fasst bei Netzfehlern nach, statt den Abgleich zu verlieren."""
    with tempfile.TemporaryDirectory() as t:
        p = Path(t) / "stand.json"
        p.write_text('{"halb', "utf-8")  # PC ging mitten im Schreiben aus
        assert holen.json_lesen(p, {}) == {} and holen.json_lesen(Path(t) / "fehlt.json", None) is None
        holen.json_schreiben(p, {"ä": 1})
        assert holen.json_lesen(p, {}) == {"ä": 1} and not (Path(t) / "stand.json.neu").exists()

        z = Path(t) / "zugang.json"
        z.write_text('{"repo": "a/b", "token": "x", "api": "http://test"}', "utf-8")
        alt = holen.ZUGANG, holen.urlopen, holen.API_PAUSE
        holen.ZUGANG, holen.API_PAUSE = z, 0
        antworten = [HTTPError("u", 503, "weg", {}, None), holen.URLError("kein Netz")]

        class Antwort:
            def __enter__(self): return self
            def __exit__(self, *a): pass
            def read(self): return b'{"ok": 1}'

        def urlopen(anfrage, timeout):
            if antworten:
                raise antworten.pop(0)
            return Antwort()
        try:
            holen.urlopen = urlopen
            assert ECHT_API("GET", "x.json") == {"ok": 1}, "nach 503 und Netzfehler klappt der dritte Versuch"
            antworten[:] = [HTTPError("u", 404, "fehlt", {}, None)]
            try:
                ECHT_API("GET", "x.json")
                assert False, "404 wird nicht wiederholt"
            except HTTPError as e:
                assert e.code == 404
        finally:
            holen.ZUGANG, holen.urlopen, holen.API_PAUSE = alt
