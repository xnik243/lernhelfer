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
                        ("2026W401100 VU Einführung in Python", "Python"), ("2026W403000 VO Allgemeine Pädagogik", "Allgemeine Pädagogik")]:
        assert holen.kursfach(titel, jetzt) == fach, titel
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
    print("ok")


if __name__ == "__main__":
    test()
