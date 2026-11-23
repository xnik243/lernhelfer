// Prüft, dass der Handy-Abgleich (olat-abgleich.user.js) Kurse und Dateien genau wie holen.py benennt: node olat-abgleich.test.mjs
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { semester, kursfach, faecherZuordnen, kursart, sichererName, eintragName, zugangLesen } = createRequire(import.meta.url)('./olat-abgleich.user.js');

const jetzt = new Date(2026, 9, 14);  // dieselben Fälle wie in test_holen.py
assert.equal(semester(jetzt), '2026W'); assert.equal(semester(new Date(2027, 1, 1)), '2026W'); assert.equal(semester(new Date(2027, 2, 1)), '2027S');
for (const [titel, fach] of [['2026W432001 VO Grundlagen der Volkswirtschaft', 'VWL'], ['2026W436307 PS Einführung in die Betriebswirtschaft', 'BWL'],
  ['2026W433302 UE Wirtschaftssprachliche Lehrveranstaltung: Wirtschaftsenglisch 1: Communication In Bu', 'Englisch'],
  ['Mathematik', 'Mathematik'], ['Tech Labs - Informatik in der Praxis', null], ['2025W432001 VO Grundlagen der Volkswirtschaft', null],
  ['2026W400100 VU Einführung in die Soziologie', 'Soziologie'], ['2026W400200 VO Einführung in die Politikwissenschaft', 'Politikwissenschaft'],
  ['2026W401000 VO Grundlagen der Psychologie', 'Psychologie'], ['2026W401100 VU Einführung in Python', 'Python'], ['2026W403000 VO Allgemeine Pädagogik', 'Allgemeine Pädagogik'],
  ['2026W700100 VO Bürgerliches Recht I', 'Recht'], ['2026W700400 VO Grundzüge des Privatrechts', 'Recht'], ['2026W800100 VO Gerechtigkeitstheorien', 'Gerechtigkeitstheorien'],
  ['2026W500100 SE Aufrechte Haltung im Sport', 'Aufrechte Haltung im Sport'], ['2026W612000 VO Physik I: Mechanik und Wärme', 'Physik I'],
  ['2026W650100 VO Allgemeine und Anorganische Chemie', 'Allgemeine und Anorganische Chemie'], ['2026W300101 PR Anatomie des Menschen – Präparierkurs', 'Anatomie des Menschen'],
  ['2026W900100 VO Wirtschafts- und Sozialgeschichte I/II', 'Wirtschafts- und Sozialgeschichte I-II'], ['2026W436308 PS Grundzüge der BWL', 'BWL']])
  assert.equal(kursfach(titel, jetzt), fach, titel);
const meine = [['1', '2026W700100 VO Bürgerliches Recht I'], ['2', '2026W700200 VO Strafrecht'], ['3', '2026W700300 PS Bürgerliches Recht I'],
  ['4', '2026W436307 VO Einführung in die Betriebswirtschaft'], ['5', '2026W436308 PS Grundzüge der BWL'], ['6', '2025W1 VO Alt']];
assert.deepEqual(faecherZuordnen(meine, jetzt).map(x => x[2]), ['Bürgerliches Recht I', 'Strafrecht', 'Bürgerliches Recht I', 'BWL', 'BWL']);
assert.deepEqual(faecherZuordnen(meine, jetzt)[3].slice(2), ['BWL', 'VO']);
assert.ok(kursart('2026W436307 PS Einführung', 'Folien') === 'ps' && kursart('Mathematik', 'Ressourcen VO') === 'folien');
assert.ok(kursart('Mathematik', 'Syllabus') === 'syllabus' && kursart('Mathematik', 'Ressourcen PS') === 'ps');
// Dateinamen wie sicherer_name: kein Pfad, keine verbotenen Zeichen
assert.equal(sichererName('../../holen.py'), 'holen.py'); assert.equal(sichererName('a%2Fb:c.pdf'), 'b-c.pdf'); assert.equal(sichererName('..'), 'datei');
assert.equal(eintragName('VO', 'Folien: Teil 1'), 'VO Folien- Teil 1'); assert.equal(eintragName('', 'Syllabus'), 'Syllabus');
// Verbindungslink: Repo und GitHub-Zugang ja, der OpenRouter-Schlüssel nie
const link = 'https://x/#verbinden=' + Buffer.from(JSON.stringify({ o: 'sk-or-geheim', g: 'github_pat_x', r: 'a/b' })).toString('base64url');
assert.deepEqual(zugangLesen(link), { repo: 'a/b', token: 'github_pat_x', api: null });
assert.equal(zugangLesen('{"repo":"a/b","token":"t","api":"http://boese"}'), null);
console.log('olat-abgleich: alles wie in holen.py');
