# Campus – Stand (max. 40 Zeilen, nach jedem Abschnitt aktualisieren)

Quelle: `Z:\workflow\campus` → `sh campus/deploy/sync.sh` → Server `/opt/campus` (root@62.83.21.224). Kein lokales Rust. Verträge: CONTRACT.md, CONTRACT-16-20.md, CONTRACT-21-25.md.
Bauen nur dort, nie zwei cargo gleichzeitig, immer `CARGO_BUILD_JOBS=1 nice -n 19 ionice -c 3` (deploy/build.sh macht das selbst).
Prüfen: `cargo check -p campus-shared -p campus-server --all-targets` + `cargo check -p campus-client --target wasm32-unknown-unknown` + naga-check
`/opt/naga-check/target/release/naga-check /opt/campus/crates/client/src/shaders`. Tests `cargo test --release --no-fail-fast -p campus-shared -p campus-server -- --test-threads=1`.
Ausliefern: `sh deploy/build.sh`, dann `systemctl restart campus` (Port 8090, hinter Caddy unter lernhelfer.xyz/campus, Login über Lernhelfer).
Web-Module vor dem Ausliefern mit `node --input-type=module --check < datei.js` prüfen (normales `node --check` übersieht Fehler).

## Abschnitte
- [x] 0–7 Grundspiel · [x] SkyBlock 8–13 · [x] Archipel M1–M4 · [x] Phasen 16–20 (RPG)
- [x] Phasen 21–25: integriert, Review-Befunde behoben, alle Gates grün (noch nicht ausgeliefert, keine Sichtprüfung im Browser).
- [ ] Abschnitt 8 (SPEC-8.md, CONTRACT-8.md): Gerüst grün (shape.rs, Blöcke 146–198, Eckpunkt 3×u32 in 1/16, bp_*.rs, WORLD_VERSION 3 + blueprints_v2); Agenten offen.

## Welt (shared/world.rs, blueprints.rs)
- WORLD 960, CH 64, Hub-Archipel (Plätze i,j < 5) mit Satelliten Bibliothek, Prüfungs-Zitadelle, Himmels-Garten; Privatinsel je Platz (SLOT 96).
- Blöcke 58–122 (Phasen bis 20), 123–145 (Portale, Antwort-Felder, Prismen, Runen, Statuen, Beutetruhen, Gitter; unzerstörbar).
- Instanz-Felder (shared/instance.rs): Leere x 336..480, Katakomben z 0..96, Arena z 96..192, je 6 Felder 48×48. Portale: KATA_PORTAL_AT, ARENA_PORTAL_AT.

## Phasen 21–25 – Aufbau
- shared: instance.rs, worldedit.rs (Quader, Wände, Kugel, Zylinder, Clip + Drehung, Rust-Export), npc.rs, proto `C2S::Arch`/`S2C::Npcs`,
  Wesen 5–7, Gegenstände 2048/2049/2420/2421, enchanting::TIER6 (Foliant → Stufe VI in island.rs).
- server/instance.rs: EIN Instanz-Mechanismus, Gitter nur im Speicher, Überlagerung als S2C::Chunk; `inst_leave` räumt bei der letzten Person
  alles ab (LAW 2). `join` ruft bei bestehendem Eintrag (Neu-Verbinden/zweiter Tab) erst `drop_player` – kein Geist-Mitglied.
- architect.rs (Architekten-Modus, //-Befehle), duel.rs + duel_ai.rs (Lern-Duell, OpenRouter mit 6,5-s-Fallback auf Cache/Bank),
  catacombs.rs + catacombs_gen.rs + dungeon_rooms_data.rs (Prüfungs-Katakomben: Spiegel, Runen, Weise, Kampf-Kammern, Endboss, Noten C..S+).
- Duell: Auflösung 3,6 s (Felder ab 0,6 s volle 3 s Luft); Schneeball des Schnellsten stößt menschliche Gegner 1,5 Blöcke weg, nur
  innerhalb des Feldes, auf dem sie stehen (harmlos, nach der Wertung).
- Web: architekt.js, duell.js, katakomben.js, foto.js (Foto-Modus, Kamerafahrten; #kino fängt Tippen zum Überspringen), musik.js, p21.css.
  Wetter ist in Prüfungs-Katakomben und Kristallminen aus. Hud `boss` mit `weg:true` = Leiste aus ohne Sieg-Meldung.
- Abweichungen: Takt 100 ms; Musik in web/musik.js statt music.rs; curl über std::process in spawn_blocking; Wetter ohne Vertex-Puffer.

## Tests (grün, 101): shared 39, server lib 37 (u. a. Neu-Verbinden, Schneeball-Rückstoß + 3-s-Luft, KI-Zeitüberschreitung im Duell,
  Runen-Eingabe echt im S+-Lauf), e2e 15, rpg_inv 1, rpg_island 5, rpg_mobs 4. cargo check (Server + wasm) und naga-check sauber.

## Offen / Merker
- OpenRouter: Schlüssel über /campus/admin (server_config) oder Umgebung; campus.service hat keine OPENROUTER_*-Zeilen.
- Sichtprüfung im Browser braucht Lernhelfer-Login (Kamerafahrten, Touch-Überspringen, Wetter drinnen, Rückstoß im Client). Shader-Fehler
  zeigt nur naga-check/Browser, nicht cargo check.
- Pack-Textur ohne Mipmaps. Kein Test für Teleporter/Sprungplatte. Tests nie gegen Port 8090. Instanz-Nachbarn sehen sich als ferne Figuren.
- Duell-Kamerafahrt: wird übersprungen, solange die Themenwahl offen ist (kommt beim nächsten Besuch).
