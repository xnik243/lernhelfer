# Campus Abschnitt 8 – Vertrag (Anti-Korn-Shader, Teilblock-Mesher, Ausstattung, WORLD_VERSION 3)

Vorgabe: `SPEC-8.md`. Ergänzt CONTRACT.md, CONTRACT-16-20.md, CONTRACT-21-25.md (gelten weiter, Phasen 21–25 dürfen nicht
brechen). Spieltext Deutsch. Niemand kompiliert (kein Rust lokal), alle arbeiten gleichzeitig: Signaturen exakt so lassen,
keine neuen Abhängigkeiten (Cargo.toml nicht anfassen), kein `unsafe`, kein `unwrap` auf Eingaben von außen, kein git commit.
Das Gerüst ist grün: `cargo check -p campus-shared -p campus-server --all-targets`, `cargo check -p campus-client --target
wasm32-unknown-unknown`, naga-check, und die neuen Tests (`shape`, `bp_furniture`, `blueprints_v2`, `mesh`) laufen.

## 0. Besitz – nur eigene Dateien ändern
| Agent | Stufe | Dateien (nur diese!) |
|---|---|---|
| mesher | opus | `crates/shared/src/mesh.rs` (nur `fn shapes` + eigene Hilfen + Tests), `crates/client/src/world.rs`, `crates/client/src/lib.rs` (nur der Bewegungsblock in `update`, heute Z. ~806–834) |
| shader | opus | `crates/client/src/shaders/scene.wgsl` (alles außer `vpos`, `vs`, `vs_shadow`, `face_of` und der Lampen-Zeile), `crates/client/src/shaders/final.wgsl`, `common.wgsl` |
| migration | opus | `crates/server/src/game.rs`: nur `fn migrate_world` (Fehler beheben, falls der Test welche findet) und ein neues `#[cfg(test)] mod world_v3_tests` am Dateiende |
| hub | opus | `crates/shared/src/blueprints.rs` (Regeln §hub) |
| library | opus | `crates/shared/src/bp_library.rs` |
| interiors | opus | `crates/shared/src/bp_interiors.rs` |
| private | opus | `crates/shared/src/bp_private.rs` |
| flora | haiku | `crates/shared/src/bp_flora.rs` |

Fertig (Architekt, NICHT ändern): `world.rs`, `shape.rs`, `bp_furniture.rs`, `blueprints_v2.rs`, `lib.rs`, in `mesh.rs` das
Eckpunkt-Format (`Vert`, `vert`, `unpack`, Greedy-Teil), `gfx.rs`, in `scene.wgsl` `vpos`/`vs`/`vs_shadow`/`face_of`/Lampen-Zeile,
in `game.rs` `WORLD_BASE`/`WORLD_VERSION`/Einhängepunkt in `open`/`migrate`. Wer eine Änderung daran braucht: im Bericht melden.

## 1. Blöcke (world.rs, fertig)
Neue Flags: `F_SHAPE = 2048` (Teilblock-Modell; nie deckender Vollwürfel; anvisierbar, auch wenn nicht fest),
`F_LAMP = 4096` (Lampenglas, PACKED-Bit 30 → Shader addiert `vec3(3.2, 2.1, 0.9)`).
`opaque(id)` = `is_opaque_full_cube(id)` ist für alle F_SHAPE-Blöcke **false**. `raycast` trifft F_SHAPE auch ohne F_SOLID.

| Id | Konstante | fest? | Modell (shape.rs) |
|---|---|---|---|
| 146 | SCHEIBE (Fensterscheibe) | ja | PANE: 2/16 dicke Glasscheibe (Stoff GLAS → durchsichtig) mit Rahmen+Sprosse (DUNKELBRETT), richtet sich nach Wänden |
| 147 | GITTERSTAB (Tresorgitter) | ja | BARS wie Scheibe, Eisenstäbe |
| 148–151 / 152–155 / 156–159 | TREPPE_EICHE_N/O/S/W, TREPPE_STEIN_*, TREPPE_DACH_* | ja, Stufe | STAIRS: untere Hälfte + obere Hälfte auf der Seite, wohin man hinaufgeht |
| 160–162 | STUFE_EICHE, STUFE_STEIN, STUFE_DACH | ja, Stufe | SLAB 0..8/16 |
| 163 | TRESEN | ja, Stufe | COUNTER 0,5 hoch: Korpus DUNKELBRETT, polierte Platte |
| 164 | TISCH | ja | TABLE: Platte 14..16/16 (0,875–1,0), 4 Beine |
| 165–168 | STUHL_N/O/S/W | ja | CHAIR: Sitz 7..9/16, Lehne gegenüber der Blickrichtung |
| 169–172 | SITZBANK_N/O/S/W | ja | BENCH (nebeneinander = durchgehende Bank) |
| 173–176 | WEBTEPPICH_ROT/GOLD/BLAU/SMARAGD (`TEPPICHE`) | nein | RUG 1/16 dick, Bordüre (Gold bzw. Rot) nur am Rand der Fläche |
| 177 | KRONLEUCHTER | nein, leuchtet | hängt an der Decke (Block darüber): Kette, Ring, 5 Lampengläser |
| 178 | LATERNENMAST | ja, leuchtet | Mast mit Laternenkopf (Lampenglas); `lamp_post` stellt 2 ZAUN darunter |
| 179 | WANDLATERNE | nein, leuchtet | hängt am Block darüber (Decke, Ast) |
| 180, 181 | ZAUN, GELAENDER | ja | FENCE: Pfosten, Riegel zu Zaun-Nachbarn und deckenden Vollwürfeln |
| 182–185 / 186–189 | MARKISE_ROT_N/O/S/W, MARKISE_BLAU_* | ja | AWNING: fällt zur Blickrichtung ab (hinten hoch), Streifen Farbe/Weiß |
| 190 | DEKOFASS | ja | BARREL |
| 191 | BUECHERSTAPEL | nein | BOOKS (auf Tische stellen) |
| 192 | GOLDSTAPEL | ja, Stufe | GOLD_STACK (Bank-Tresor) |
| 193–196 | MOHN, SONNENBLUME, KORNBLUME, LEUCHTKLEE (`BLUMEN`) | nein | Kreuz-Stiel (Stoff HOHES_GRAS) + Blüte; Klee leuchtet |
| 197 | HOHES_GRAS | nein | Kreuz |
| 198 | LAMPENGLAS | ja | gewöhnlicher Würfel; Stoff der Lampen-Teile (F_LAMP), nicht im Laden |

Ausrichtung `_N/_O/_S/_W` = Blickrichtung 0 Nord (−z), 1 Ost (+x), 2 Süd (+z), 3 West (−x); `shape::facing(X_N, f)` liefert den
Block. Treppe: Blickrichtung = wohin man hinaufgeht. Stuhl/Bank: wohin der Sitzende schaut. Markise: wohin sie abfällt.
Alte Würfel (BLUMEN_ROT/GELB, TEPPICH_ROT/BLAU, GLAS, LATERNE …) bleiben als Blöcke bestehen (Spieler besitzen sie), die
Baupläne nehmen statt ihrer die neuen Modelle. `ALTAR_AT` ist jetzt `[393, 32, 240]` (Podest-Mitte im Bibliotheks-Atrium).

## 2. shape.rs (fertig) – eine Tabelle für Mesher und Kollision
- `Part { b: [u8; 6] /*x0,y0,z0,x1,y1,z1 in 1/16*/, mat: u8, cross: bool }`; `cross` = zwei senkrechte, beidseitige Flächen
  über die Diagonalen der Grundfläche (stoßen nie).
- `def(id) -> Option<(&'static Model, u8 /*Drehung*/)>`, `Model { parts, conn: Conn, step: bool }`, `Conn::{None, Fence, Pane, Rug}`.
- `conn_mask(id, n: [u8; 4] /*Nachbarn N, O, S, W auf gleicher Höhe*/) -> u8` (Bits `N=1, O=2, S=4, W=8`; `DIRS` = Versätze),
  `conn_at(&get, id, x, y, z)`.
- `parts(id, conn, &mut |q: Part| …)`: alle Teile fertig gedreht, `mat` aufgelöst (nie 0). `rot(b, k)` 90° im Uhrzeigersinn.
- Kollision: `coll_boxes(&get, x, y, z, &mut |aabb: [f32; 6], step| …)` (Würfel: Vollwürfel; nicht feste Modelle: nichts),
  `body_hits(get, p) -> bool` (Füße p, `HALF_W 0.3`, `BODY_H 1.79`; für Würfel identisch zur alten Prüfung),
  `step_lift(get, p) -> Option<f32>`: blockieren bei p nur Stufen-Kästen (Treppe, Stufe, Tresen, Goldstapel) mit Oberkante
  ≤ `STEP_HEIGHT 0.55` über den Füßen und ist es darüber frei → `Some(h)` (anheben), frei → `Some(0.0)`, sonst `None`.
- Server: es gibt keine Spieler-Kollision auf dem Server (nur Tempo-Prüfung); Mobs/Ankunft nutzen `world::solid` je Zelle.
  Damit stimmen Browser und Server überein: nicht feste Modelle (Blumen, Gras, Teppich, Leuchter, Wandlaterne, Bücher) sind
  für beide begehbar, feste Modelle für Mobs ganze Zellen.

## 3. Eckpunkt-Format (mesh.rs/gfx.rs/scene.wgsl, fertig)
`Vert = [u32; 3]`: `[0] = x16 | y16 << 16`, `[1] = z16 | normale << 16 | ao << 20`, `[2] = PACKED[stoff]`; Orte = Welt × 16.
Normalen 0..5 = Achse·2 + Seite (0 −x, 1 +x, 2 −y, 3 +y, 4 −z, 5 +z); 6 (+1,0,−1)/√2, 7 (−1,0,+1)/√2 (Ebene entlang (1,0,1)),
8 (+1,0,+1)/√2, 9 (−1,0,−1)/√2 (Ebene entlang (1,0,−1)). Rechteck = 4 Eckpunkte gegen den Uhrzeigersinn, von der Seite der
Normale gesehen (gleiche Regel wie der Greedy-Teil; Test `ein_block_sechs_flaechen_nach_aussen` prüft sie). 48 Byte/Rechteck.
Stoff mit F_CLEAR → `out.clear`, sonst `out.solid`. Shader: `face_of` kennt 6..9 (uv = (dot(wp, t), wp.y), `side = true`).

## 4. Bau-Helfer (bp_furniture.rs, fertig) – Blueprints bleiben kurz
Richtungen `ZU_N 0, ZU_O 1, ZU_S 2, ZU_W 3` (≠ Tür-Konstanten NORD 0, SUED 1, WEST 2, OST 3 aus blueprints!).
`chair(p, x, y, z, zu)`, `table_set(p, x, y, z, sides /*shape::N|O|S|W*/, top /*0 oder z. B. BUECHERSTAPEL*/)`,
`bench(p, x0, z0, x1, z1, y, zu)`, `rug(p, x0, z0, x1, z1, y, v)` (nur Luft; y = Schicht über dem Fußboden),
`fence_line(p, x0, z0, x1, z1, y, v)` (nur Luft), `lamp_post(p, x, y, z)` (3 hoch), `window(p, x, y, z, h)` (SCHEIBE-Säule),
`stair_roof(p, x0, z0, x1, z1, y, along_x, north /*TREPPE_*_N*/, ridge /*STUFE_*/) -> i32` (Satteldach, liefert Firsthöhe),
`meadow(p, [x0, z0, x1, z1], y /*Höhe der Wiese*/, pct, seed)` (Blumen/Gras nur auf WIESE mit Luft darüber, nie an keep_free),
`cliff_fence(p, st, [x0, z0, x1, z1], v, skip: &[[i32; 4]])` (Zaun am Inselrand über der Oberfläche, nicht auf Wasser).
Grundregel für alle Baupläne: Entscheidungen nur aus Weltkoordinaten/`surf`/`hash`/`p.get` derselben Spalte, sonst Nähte an
Stückgrenzen. Kronleuchter/Wandlaterne nur direkt unter einen festen Block. Möbel nie auf keep_free-Stellen, NPC-Stand-
plätze (Füße + Kopf frei, Boden fest) und Wege von Portalen/Türen.

## 5. WORLD_VERSION 3 (game.rs, fertig – Agent migration testet)
- `const WORLD_BASE = "3-archipel"` (kv "welt", alter destruktiver Umzug, unverändert) und `pub const WORLD_VERSION: u32 = 3`
  (kv "world_version", fehlt = 2). `open`: nach `migrate()` (setzt bei frischer DB auch world_version = 3), vor dem Laden der
  Stücke: `world_version < 3` → `migrate_world()`.
- `migrate_world`: in EINER Transaktion: gespeicherte Stücke im Hub-Archipel (`in_hub`, deckt Hub, Satelliten, Instanz-Felder)
  löschen → sie entstehen neu aus den Bauplänen. Jedes andere gespeicherte Stück (Privat- und Gruppeninseln):
  `blueprints_v2::merge(saved, blueprints_v2::gen_chunk(cx, cz) /*alte Vorlage, eingefroren*/, world::gen_chunk(cx, cz, st))`
  → zurückschreiben. Regel je Zelle: saved == alt → neue Vorlage, sonst bleibt saved (Spieler-Bau, Abbau, Saat, ABGEBAUT);
  TRUHE, GROSSE_TRUHE, STAENDER, SOCKEL bleiben immer. Dann world_version = 3. Nie gespeicherte Stücke sind ohnehin neu.
  Zweiter Start: nichts passiert (Version), und `merge` ist idempotent.
- `blueprints_v2.rs` = Kopie der heutigen Privatinsel-Vorlage; Test `fingerabdruck` hält sie fest (nie ändern).

## §mesher (opus)
1. `mesh.rs::shapes(p, ox, oz, out)`: für jeden F_SHAPE-Block des Stücks (x, z 0..16, y 0..64 aus dem gepolsterten Feld `p`):
   `conn = shape::conn_mask(id, [N, O, S, W]-Nachbarn aus p)` (nur wenn `def(id).0.conn != Conn::None`), dann `shape::parts`.
   Kasten: 6 Flächen, Normale 0..5, ao 3, Stoff `q.mat` (F_CLEAR des Stoffs → `out.clear`). Fläche weglassen NUR, wenn sie
   genau auf der Zellgrenze liegt (Koordinate 0 bzw. 16) und der Nachbar dort `opaque` ist (z. B. Teppich-Unterseite, Tischbein
   auf dem Boden). Kreuz-Teil: zwei Ebenen (x0,z0)→(x1,z1) mit Normalen 6/7 und (x0,z1)→(x1,z0) mit 8/9, je beide Seiten (= 4
   Rechtecke), Höhe y0..y1. Positionen = (Weltblock·16 + Teilmaß), über `mesh::vert`. Nichts mit F_SHAPE geht in den Greedy-Teil
   (fertig) – und weil `opaque` für F_SHAPE false ist, behält jeder Nachbar seine Fläche (das Anti-Loch-Gesetz).
2. Tests in mesh.rs (Pflicht): (a) „nie Löcher“: Boden-Ebene y 0 aus FELS, darauf je ein TISCH, WEBTEPPICH_ROT, TREPPE_EICHE_N,
   SCHEIBE, MOHN, STUFE_STEIN → die Oberseite (Normale 3, y16 = 16) des Bodens unter jeder dieser Zellen ist vollständig
   bedeckt (z. B. Summe der Rechteckflächen über der Zelle = 16·16 bzw. Greedy-Rechteck enthält die Zelle); Wand (FELS) neben
   einer SCHEIBE behält die Seitenfläche zur Scheibe. (b) SCHEIBE: Glas-Rechtecke in `clear`, Rahmen in `solid`. (c) MOHN: genau
   4 Kreuz-Rechtecke mit Normalen {6,7,8,9}, jedes gegen den Uhrzeigersinn zur eigenen Normale (Kreuzprodukt wie im
   bestehenden Test), plus Blütenkasten. (d) einzelner TISCH auf FELS: 5 Kästen, die 4 Bein-Unterseiten fehlen → 26 Rechtecke.
3. `client/world.rs`: `collides(p)` → `shape::body_hits(|x, y, z| self.get(x, y, z), p.to_array())`; neu
   `pub fn step_lift(&self, p: Vec3) -> Option<f32>` (dünne Hülle um `shape::step_lift`). Glow-Liste unverändert (nutzt `opaque`).
4. `client/lib.rs` Bewegung: in der Achsen-Schleife, wenn eine waagrechte Achse (`axis != 1`) blockiert und die Figur am Boden
   war (`was_ground`, nicht nass, nicht fliegend): `if let Some(h) = self.world.step_lift(p)` mit `h > 0` → `p.y += h; self.pos = p`
   (kein Sprung). Erst sonst `blocked = true`. Das alte Auto-Hüpfen für ganze Blöcke bleibt. Kamera/Drops nutzen weiter `solid`.

## §shader (opus) – SPEC §8A, nur Stufen-Logik, Ergebnis muss naga-check bestehen
1. `fs` und `fs_clear_hdr`: ganz oben, vor jeder Verzweigung/`discard`: `let fw = max(length(dpdx(uv)), length(dpdy(uv)));` mit
   `uv = face_of(i.a, i.wp).uv` (Weltflächen-UV) und `let detail_fade = clamp(1.0 - fw * 18.0, 0.0, 1.0);` → als Parameter an
   `block_color(i, detail_fade)` reichen. Damit alle hochfrequenten Anteile dämpfen: `r`-Sprenkel in `block_mat`, `micro()`,
   Relief-Normalen (`k`), Kristall-Glitzern → bei fade 0 bleibt die glatte Grundfarbe (keine Salz-und-Pfeffer-Pixel).
2. Wiese/Erde/Sand/Laub (Muster 1, 4, 9 sowie ERDE/SAND, die Muster 4 haben – Muster 4 ist auch Fels; für Fels genügt fade):
   kein POM, Normalstärke `0.12 * detail_fade`, `rough 0.92`, `metal 0.0`. POM (Extrem) und scharfe Glanzlichter nur für
   Steinziegel (14), Bruchstein (13), Bretter (3), Kristall (6/15) und nur bis 8,0 m (`dist < 8.0`, weiches Ausblenden 6–8 m).
3. Wiesen-Oberseiten (Muster 1, Normale +y): großflächiger Farbverlauf aus `noise2(world_xz * 0.08)` (eigene 2D-Wertrausch-
   Funktion, z. B. über `noise3(vec3(xz, 0))`), ±8 % Helligkeit/leichter Gelb-Grün-Schub.
4. final.wgsl `gtao`: Mindest-Tiefenunterschied 0.045 – Beiträge mit `abs(Δtiefe) < 0.045` (bzw. Abstand in Blöcken unter dieser
   Schwelle entlang der Normalen) zählen nicht; flache Böden werden nicht mehr gesprenkelt dunkel.
5. Nicht ändern: Eckpunkt-Eingänge, `face_of`, Lampen-Zeile (`PACKED`-Bit 30). Teilblöcke brauchen sonst nichts Eigenes.
6. Gate: `naga-check` über alle Shader (Integrator führt aus). Uniformität: `dpdx/dpdy` nur im obersten Block von `fs`/`fs_clear_hdr`.

## §migration (opus)
Test-Modul `world_v3_tests` am Ende von game.rs (ein Test-Programm, Server hat 2 GB), mit `crate::testkit::game()`:
1. Frische DB: `kv("world_version") == Some("3")`.
2. Pfad über `g.db.path()`; Privatinsel-Stück eines Platzes außerhalb des Hubs (z. B. Stück (cx 30..35, cz 0..5) von Platz (5,0) mit
   Hütte): `blueprints_v2::gen_chunk` + eine Spielerzelle (FELS in Luft) + eine Zelle abgebaut (0 statt Vorlage) + TRUHE, als
   Zeile in `chunks`; dazu eine Hub-Zeile (z. B. Stück (15,15)) voller Unsinn; `kv world_version = 2`; `drop(g)`; `Game::open(&path)`.
3. Erwartet: Hub-Zeile gelöscht, `block()` dort = `world::gen_chunk`; Spielerzelle und Abbau-Lücke und TRUHE bleiben; eine
   unberührte Zelle, an der sich alte und neue Vorlage unterscheiden, trägt die neue Vorlage; world_version = "3".
4. Nochmals öffnen: Zeilen-Daten byte-gleich (idempotent). Gefundene Fehler nur in `migrate_world` beheben.

## §hub (opus) – blueprints.rs
Eigentum: alles in blueprints.rs AUSSER `Put`, Gelände der Privatinseln (`surf`-Zweig außerhalb des Hubs, `ground`),
`touches`/`build` und den Aufrufen von `bp_library::library`, `bp_interiors::{bank, markthalle, werkstatt}`,
`bp_private::private`, `bp_flora::dress` (Reihenfolge lassen). Signaturen der `pub(crate)`-Helfer (house, stall, lantern, tree,
blob, disc, dome, arch, fall, dist, keep_free, main_top, district) bleiben – andere Agenten rufen sie.
1. `house()`: Hülle bleibt (Grundriss, Boden y, 4 hohe Stockwerke, Zwischenböden y + 4f, Tür-Lage/-Breite/-Öffnung, Innenraum
   x0+1..x1−1 × z0+1..z1−1 frei). Neu: Fenster = `window()` (SCHEIBE in der Wandebene, keine Glaswürfel mehr), Blumenkasten
   außen darunter (DUNKELBRETT, darauf eine Blume aus `BLUMEN`), dunkle Eckpfosten, Steinziegel-Sockel, Satteldach mit Überstand
   aus `stair_roof` (DACH → TREPPE_DACH/STUFE_DACH, SCHIEFER/POLIERT/… → TREPPE_STEIN/STUFE_STEIN, DUNKELBRETT → TREPPE_EICHE/
   STUFE_EICHE; Giebelwände wie bisher), Bogen-Eingang (Türöffnung oben gestuft), innen statt LATERNE-Würfel ein KRONLEUCHTER
   unter der Decke jedes Stockwerks. Die Bank-Hülle nutzt `house` (BANKHAUS) – interiors richtet sich nach dieser Hülle.
2. `stall()`: Theke aus TRESEN, DEKOFASS statt FASS, Ware obendrauf, MARKISE_ROT/BLAU (zur Kundenseite abfallend) statt
   Stoffwürfel-Dach, Eckpfosten ZAUN. Signatur bleibt (interiors ruft sie in der Markthalle).
3. Rasen ohne Würfelblumen: alle BLUMEN_ROT/BLUMEN_GELB-Setzungen und -Deckblöcke (main_top SPAWN, hub_surf der Satelliten,
   spawn-Terrassen, avenue, garden-Beete) → WIESE + Kreuzblumen obendrauf (`meadow` oder gezielt `BLUMEN`/HOHES_GRAS).
   Achtung Dichte-Test: „nackte flache Wiese“ < 12 % – Blumen/Gras obendrauf zählen als bedeckt.
4. Wege: Königsallee/Plätze aus Bruchstein/Steinziegel/Pflaster mit Kiesrand (WEG), Laternen = `lamp_post` statt `lantern`
   (lantern darf intern auf lamp_post umgestellt werden), Sitzbänke (`bench`) an der Allee, Brücken (`deck`) mit GELAENDER an
   beiden Seiten (Laternen bleiben), Zitadelle: Fenster des Bergfrieds als SCHEIBE, sonst Kampfzonen (CITADEL, CITADEL_ARENA,
   CITADEL_HALL) frei lassen, Portale/Altäre unverändert.
5. Pflicht-Tests grün halten (world.rs, nicht ändern): `hub_dichte` (≥ 35 000 feste Blöcke, jede Landmarke ≥ 12 feste Blöcke,
   Höhlen ≥ 500, nackte Wiese < 12 %), `ankunft_auf_festem_boden` (SPOTS: 4 Blöcke reine Luft (id 0!) über dem Ankunftspunkt,
   Portale), `npcs_altaere_portale`, `projekte_aendern_nur_ihre_stellen`. Nichts an keep_free-Stellen.

## §library (opus) – bp_library.rs, Bibliotheks-Turm (Mitte 393/240, Insel-Boden y 30, Turm r ≤ 6,4, y 31..62)
Kompletter Neubau des Inneren, Außenhülle (Steinziegel-Rundturm, Dach, Fenster) darf bleiben/aufgewertet werden (Fenster = SCHEIBE):
helles, offenes 3-Etagen-Atrium (Galerien z. B. auf y 38 und y 46 als Ring r 3,6..5,2 aus Brettern, Mitte offen), Bücherregale
nur bündig an der Innenwand (REGAL, Ring r 4,2..5,2 in Regal-Höhen, keine wahllosen Blöcke mehr), **begehbare Wendeltreppe**
aus TREPPE_EICHE (passende Blickrichtung je Stufe = tangential nach oben) mit GELAENDER an der offenen Seite, die vom
Erdgeschoss auf jede Galerie führt (Kopfhöhe ≥ 2 Luft über jeder Stufe, jede Stufe höchstens 1 Block über der vorigen, mit
Halbstufen/STUFE_EICHE als Zwischentritt, wo nötig), Lese-Ecken auf den Galerien (`table_set` mit BUECHERSTAPEL, Stühle,
`rug` rot/gold), je Etage ein KRONLEUCHTER unter der Galerie bzw. Decke, Podest (POLIERT/GOLDZIER, Scheibe r ≤ 2 auf y 31) in
der Mitte mit ALTAR genau auf `ALTAR_AT = [393, 32, 240]`. Eingang West (x 386..389, z 239..241) frei, Bibliothekarin Lux steht
draußen bei (384.5, 31, 240.5), SPOTS[2] (378.5, 31, 240.5) frei; Portal-Bogen `arch(p, 374, 235, 30, false)` und Wasserfall
bleiben. Außen: Laternen als `lamp_post`, Blumenwiese auf der Insel (`meadow`, nicht vor Eingang/Portal), schwebende Regale
dürfen bleiben. Landmarke „Bibliotheks-Turm“ [393, 30, 240]: ≥ 12 feste Blöcke in ±4, y 31..38 (Podest, Altar, Treppe, Wand).
Test (in bp_library.rs, `#[cfg(test)]`): mit `world::gen_chunk` die Wendeltreppe von unten bis zur obersten Galerie mit
`shape::step_lift`/`body_hits` „abgehen“ (jede Stufe ersteigbar oder ≤ 1 Block mit Luft darüber, Kopf frei) – oder gleichwertig.

## §interiors (opus) – bp_interiors.rs (Hüllen bauen `house`/blueprints::market vorher)
1. `bank`: Haus 206..221 × 218..235, Boden y 28, Tür Ost (x 221, z 226..228). Rot-goldener Läufer (`rug` WEBTEPPICH_ROT vom Eingang
   bis zum Schalter, gold daneben), Schalter-Reihe aus TRESEN vor dem Bankier (Goldbach steht bei (212.5, 29, 227.5): Zellen
   (212, 29/30, 227) frei, Spieler muss auf ≤ 8 Blöcke heran – Tresen z. B. auf x 214), Tresor hinten (x 207..210) hinter
   GITTERSTAB mit GOLDSTAPEL und GOLD, Sitzbänke für Wartende, KRONLEUCHTER, Fenster sind schon SCHEIBE aus `house`.
2. `markthalle` (Bazaar-Halle, Pfosten 222..258 × 199..213, Boden y 28): die 8 Stände (`stall`, aufgewertet von hub) bleiben an
   ihren Plätzen; dazu Läufer im Mittelgang, Sitzbänke, Deko-Fässer, Hängelampen (WANDLATERNE unter dem Dachbalken).
   Händler Brandt steht bei (240.5, 29, 215.5) – frei lassen.
3. `werkstatt` (Halle 164..192 × 240..250, Boden y 26; SOCKEL bei SHOWCASE[0] + 2k, SHOWCASE[1], SHOWCASE[2], k 0..10 bleiben
   unverändert und darüber 2 Luft): Werkbänke (WERKBANK), Ambosse (AMBOSS), Kisten/DEKOFASS, Sitzbank, Webteppich-Läufer
   vor den Sockeln, KRONLEUCHTER unter der Decke (y 31), Fenster als SCHEIBE in den Putzwänden.
4. Test (in bp_interiors.rs): Bank enthält TRESEN, WEBTEPPICH_ROT, SCHEIBE, KRONLEUCHTER; Werkstatt-Sockel unverändert.

## §private (opus) – bp_private.rs (Vorlage jeder Privat- und Gruppeninsel; `a, c` = Ecke des Platzes, g = GROUND = 30)
Neue **Starthütte** am alten Ort (Grundriss a+41..a+54 × c+31..c+38 ± 2 Blöcke), Tür Süd: Satteldach aus Treppen (`stair_roof`),
Fenster SCHEIBE, innen TISCH + Stühle, Webteppich, Bett (BETT), Regal (REGAL), Werkbank (WERKBANK) und Truhe (TRUHE) – Truhe
und Werkbank genau wie bisher auf (a+51, g+1, c+32) bzw. (a+50, g+1, c+32) (Migration hält Truhen fest), warme Laterne
(KRONLEUCHTER oder WANDLATERNE unter der Decke). Außen: Naturstein-Gartenweg (BRUCHSTEIN/MOOS_BRUCH mit WEG-Rand) von der
Ankunft zur Hüttentür, zum Minion-Sockel (SOCKEL bleibt auf (a+51, g+1, c+46)), zur Hängebrücke und zum Generator; eine Eiche
mit WANDLATERNE unter einem Ast; Holzzaun (`cliff_fence` mit `skip` für Brücke und Portal) am Inselrand; Wildblumen (`meadow`).
Unverändert lassen: Portal-Bogen `arch(p, a+48, c+62, g, false)`, Hängebrücke, Rohstoffinsel (Generatoren), Holzknoten-Eichen,
Wasserfall. Harte Invarianten (Tests world.rs + e2e, nicht ändern): Ankunft `plot_spawn` = (a+48.5, g+1.05, c+52.5) muss genau
das Ergebnis von `find_safe_spawn` bleiben → Ankerblock (a+48, g, c+52) fest, Spalten a+47..a+49 × c+51..c+53 über g ohne festen
Block bis zum Himmel, x a+46..a+50 × z c+50..c+54 × y g+1..g+4 reine Luft (e2e stellt Minions auf (a+48..a+50, g+1, c+52));
auf GROUND..GROUND+6 müssen PORTAL, TRUHE, WERKBANK, SOCKEL, GENERATOR, SPLITTER_GEN, BETT, REGAL, LATERNE, KNOTEN_EICHE
vorkommen (LATERNE z. B. als Würfel am Generator oder in `lantern`). Grundstück plot_rect(…, 32) bleibt auf GROUND-Gelände.
Test (bp_private.rs): neue Vorlage enthält SCHEIBE, TISCH, STUHL_*, WEBTEPPICH_*, TREPPE_* und Blumen aus BLUMEN.

## §flora (haiku) – bp_flora.rs, Funktion `dress(p, st)` (läuft nach allen Hub-Bauten)
Nur Aufrufe der Helfer aus §4 mit festen Koordinaten (alle Rechtecke `[x0, z0, x1, z1]`, eingeschlossen):
1. `meadow` auf den Wiesen: Hauptinsel Spawn-Viertel [200, 255, 280, 330] (y = `main_top(x, z).0`: in Teilrechtecke gleicher Höhe
   aufteilen: z 267..330 → y 32), Flüsterwald [270, 150, 330, 330] (y je Spalte verschieden → dort `meadow` je Höhenstufe 28..38
   aufrufen, es setzt nur, wo bei y WIESE liegt), Himmels-Garten [62, 212, 118, 268] y 30, Zitadellen-Insel-Rand [207, 52, 273, 118]
   y 30, ISLET (falls Projekt) y 32. pct 25–40, verschiedene `seed`.
2. Sitzbänke (`bench`, 2–3 lang, Blick zum Weg) unter Bäumen im Himmels-Garten und an den Spawn-Terrassen; `lamp_post` alle
   ~10 Blöcke an den Gartenwegen; nur in Luft über festem Boden (`p.get(x, y-1, z)` fest, `p.get(x, y, z) == 0`).
3. `cliff_fence(p, st, rect, ZAUN, &[])` um Himmels-Garten und Zitadellen-Insel, GELAENDER um die Hauptinsel-Ränder der Viertel
   Markt/Spawn (die Bibliothek macht library selbst). Brücken-Enden und Portale schützt keep_free schon; zusätzlich
   die Bogenbrücke der Himmelsruinen (x 196..208, z 172..182) in `skip` aufnehmen.
4. Nichts an keep_free-Stellen, nichts auf Wegen (WEG, PFLASTER, POLIERT, STEINZIEGEL), nichts innerhalb von Gebäuden.
Prüfen lassen sich die Pflicht-Tests aus §hub Punkt 5.

## 6. Gates (Integrator, nach allen Agenten)
1. `cargo check -p campus-shared -p campus-server --all-targets` 2. `cargo check -p campus-client --target wasm32-unknown-unknown`
3. naga-check 4. `cargo test --release --no-fail-fast -p campus-shared -p campus-server -- --test-threads=1` (alle alten Tests +
`shape::*`, `bp_furniture::helfer`, `blueprints_v2::*`, mesh-Tests, `world::tests::npcs_altaere_portale`, library/interiors/private-
Tests, `world_v3_tests`) 5. Sichtprüfung laut SPEC §D (Playwright, braucht Lernhelfer-Login): Wiese ohne Korn mit Kreuzblumen,
Bank innen, Bibliotheks-Atrium mit Treppe/Leuchtern, Privatinsel mit Hütte und Gartenweg.
