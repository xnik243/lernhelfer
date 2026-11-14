# Campus – Stand (max. 40 Zeilen, nach jedem Abschnitt aktualisieren)

Quelle: `Z:\workflow\campus` → `sh campus/deploy/sync.sh` → Server `/opt/campus` (root@62.83.21.224). Kein lokales Rust. Verträge: CONTRACT.md, CONTRACT-16-20.md.
Bauen nur dort, nie zwei cargo gleichzeitig, immer `CARGO_BUILD_JOBS=1 nice -n 19 ionice -c 3` (deploy/build.sh macht das selbst).
Prüfen: `cargo check -p campus-server --all-targets` + `cargo check -p campus-client --target wasm32-unknown-unknown` + naga-check
`/opt/naga-check/target/release/naga-check /opt/campus/crates/client/src/shaders`. Tests `cargo test --release --no-fail-fast -p campus-shared -p campus-server -- --test-threads=1`.
Ausliefern: `sh deploy/build.sh`, dann `systemctl restart campus` (Port 8090, hinter Caddy unter lernhelfer.xyz/campus, Login über Lernhelfer).
Web-Module vor dem Ausliefern mit `node --input-type=module --check < datei.js` prüfen (normales `node --check` übersieht Fehler).

## Abschnitte
- [x] 0–7 Grundspiel · [x] SkyBlock 8–13 · [x] Archipel M1–M4 · [x] Phasen 16–20 (RPG: Inventar/Truhen/Stash, Werte/Rüstung/Verzaubern,
  Kampf/Zitadelle/Boss, Ackerbau/Funktionsblöcke/Interpolation, Co-op/Handel/Likes/Questbuch, Halo-Fix, Textur-Pakete, freies Bauen)

## Welt (shared/world.rs, blueprints.rs)
- WORLD 960, CH 64, Hub-Archipel (Plätze i,j < 5) mit Satelliten Bibliothek (ALTAR_AT), Prüfungs-Zitadelle (CITADEL, BOSS_ALTAR_AT), Himmels-Garten.
- Privatinsel je Platz (SLOT 96): `edit_allowed` = Recht + `slot_at` (ganze Insel bebaubar), nur `anchor` (Block unter plot_spawn) geschützt.
- Blöcke 58–122: 108 GROSSE_TRUHE, 109 ACKERBODEN, 110–114 Saaten (F_INST, nicht gemesht), 115 SPRUNGPLATTE, 116–119 Teleporter, 120 STAENDER, 121 ALTAR, 122 BOSS_ALTAR.

## Aufbau Phasen 16–20
- shared: rpg.rs (Item-IDs 2000+), inventory.rs (36 Plätze, Truhe 27/54, reine Verschiebe-Funktionen), stats.rs (compute), enchanting.rs,
  farming.rs (Stufe aus Zeitstempel), combat.rs (Konstanten, taken, loot_share), *_data.rs (Sets, Wesen, Verzauberungen, Pflanzen, 12 Kapitel).
- server: inv.rs (InventoryTx = einziger Weg für Gegenstände; inv-Zähler ist Wahrheit, inv_layout ist Anordnung, Überlauf → player_stash;
  Truhen island_chests, Ständer, Handel mit 3-s-Sperre), island.rs (Ackerbau, Sprungplatte/Teleporter/Schild/Ständer, Verzaubern, Co-op, Likes,
  Questbuch), mobs.rs (LAW 2: mob_tick steigt ohne Spieler in der Zitadelle sofort aus; Boss alle 5 min), rpg.rs (Werte-Cache, Registry `rpg`).
- client: fx.rs (Snapshot-Interpolation, Zahlen, Drops, Schockwellen), deco.rs (Wesen, Pflanzen, Ständer, Waffen), avatar.rs (Rüstung, Aura).
  WASM neu: held, set_stats, set_texture(res), set_texture_pack, clear_texture_pack. Shader: par2.w = ±tex_res, DoF erst ab 48, Bloom bilateral.
- web: inventar.js/.css (Inventar, Truhe, Ständer, Handel, Verzaubern, Stash-Abzeichen), hud.js (Leiste = Plätze 0–8, I/J/E, Questbuch, Top-Inseln, Co-op, Textur-Paket).

## Tests (alle grün, 61)
e2e 15, rpg_inv (100 Zufallsfälle inkl. Handel-Abbruch/Trennung), rpg_island 5 (Ackerbau +600 s, Verzaubern, Co-op), rpg_mobs 4 (0 KI-Schritte ohne Spieler), shared 35.

## Offen / Merker
- Sichtprüfung im Browser braucht Lernhelfer-Login (Screenshots Inventar/Kampf/Questbuch/128x noch offen).
- Pack-Textur ohne Mipmaps (128x flimmert in der Ferne). Kein Test für Teleporter/Sprungplatte. Tests nie gegen Port 8090.
- Altes `chest`-Table wird beim ersten Öffnen einer Insel-Truhe übernommen und gelöscht.
