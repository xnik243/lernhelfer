# Campus-Archipel – Vertrag für die drei Bauteile

Gilt für den Umbau „schwebende Inseln, Überlebens-Abbau, Voxel-Oberfläche, NPCs/Portale“. Der Vertrag regelt, WIE die Teile
zusammenpassen; WAS gebaut wird, steht in der Aufgabe. Alles Spieltext Deutsch, nur eigene Namen/Grafik. Titel bleibt „Campus“.

## 0. Besitz (nur eigene Dateien ändern; niemand kann kompilieren – Signaturen hier sind bindend)
| Agent | Dateien |
|---|---|
| WORLD | `crates/shared/**` außer `proto.rs`, `crates/server/**` (inkl. `tests/e2e.rs`) |
| CLIENT | `crates/client/**` |
| WEB | `web/**` |
Schon erledigt (Architekt): `shared/src/proto.rs` (neue Nachrichten), `shared/src/world.rs` (Konstanten, Lage, Blöcke 58–106,
Abbau-/Spawn-/Portal-Helfer, Test `abbauen_und_ankunft` – grün auf dem Server). Diese Signaturen nicht ändern; Werte (Koordinaten)
darf WORLD anpassen, solange der Test grün bleibt. Alte Tests `insel_und_plaetze`, `projekte_aendern_nur_ihre_stellen` sind rot → WORLD schreibt sie neu.

## 1. Weltlage (world.rs, eine gemeinsame Welt, Inseln durch Leere getrennt)
- `WC = 60`, `WORLD = 960`, `CH = 64`, `SLOT = 96`, `SLOTS = 10`. Mesh packt x/z in 10 Bit → WORLD ≤ 1024.
- Hub-Archipel = Plätze `i < 5 && j < 5` (`in_hub(x,z)`, Blöcke 0..480). Alles andere: Privatinsel-Raster (`slot_at(x,z)`), 75 Plätze.
- Außerhalb jeder Insel: Spalte komplett Luft (y 0..63). Unter jeder Insel ein Kegel (Fels, Moos-Bruchstein, Tiefenfels-Tropfsteine,
  Kristalladern, Wasserfälle über den Rand).
- **Hub-Hauptinsel** `HUB_MAIN = [150,150,330,330]` (180×180), Mitte `MID = 240`. Höhen: Spawn-Zitadelle Oberfläche y=32, Marktplatz 28,
  Schmiedeviertel 26, Wald 27–38, Minen y 6–24 (`MINE_Y = 24`), Ruinen 35–60.
- `DISTRICTS` (erster Treffer): Himmelsruinen `[150,150,205,192]`, Spawn-Zitadelle `[200,255,280,330]`, Marktplatz `[210,200,270,255]`
  (45×45-Platz `[218,208,263,253]`, Kristallbrunnen 12 hoch in der Mitte 240/230), Schmiedeviertel `[150,192,210,330]`,
  Flüsterwald `[270,150,330,330]`, Kristallminen `[205,150,270,200]` (+ alles unter y 24). Allee: x 236..244 (9 breit), z 253..292.
- **Satelliten** `SATS` (Oberfläche y=30, 40 Blöcke Leere zur Hauptinsel, je schmale Himmelsbrücke + Portal):
  Bibliotheks-Turm x0 370 z0 218 (45), Prüfungs-Zitadelle 215/60 (50), Himmels-Garten 70/220 (40).
- **Privatinsel** im Platz (sx = i·96, sz = j·96): Hauptinsel 38×38 `[sx+29,sx+67)×[sz+29,sz+67)` auf GROUND=30; Hängebrücke
  `x [sx+67,sx+73), z [sz+47,sz+50)`; Rohstoffinsel 16×16 `[sx+73,sx+89)×[sz+40,sz+56)` mit GENERATOR + SPLITTER_GEN (Wasser/Lava-Deko),
  KNOTEN_EICHE-Bäume. Auf der Hauptinsel: Fachwerk-Starthütte (BETT, REGAL, LATERNE), TRUHE, WERKBANK, SOCKEL, Eichen, Portal (→ Hub).
  `plot_rect(slot,size)` bleibt (mittig, liegt in der Hauptinsel), `plot_spawn(slot)` muss unter freiem Himmel liegen.
- `SPOTS: [(name, [x,y,z], yaw); 8]` – 0 Campus-Hub (240.5,33.05,296.5, yaw 0 = Blick nach Norden die Allee hinauf), 1 Bazaar,
  2 Bibliotheks-Turm, 3 Minion-Werkstatt, 4 Prüfungs-Zitadelle, 5 Himmels-Garten, 6 Himmelsruinen, 7 Kristallminen. Index 0..4 fest.
  yaw: Blick = (-sin yaw, -cos yaw); 0 Nord, π/2 West, -π/2 Ost, π Süd. Jeder Spot unter freiem Himmel (find_safe_spawn sucht von oben).
- `PORTALS: [(x,z,ziel)]`, `PORTAL_HOME = 255`, `portal_dest(x,z) -> Option<u8>` (Privatinsel → `Some(0)`). WORLD baut die Portalrahmen
  (PORTAL-Blöcke) genau an diese Mitten (±6 Blöcke). `LAUNCH_PADS: [(x,z,ziel)]` 3×3 SPRUNGPAD-Blöcke um (x,z) in den Ruinen;
  `launch_pad(x,z,r)`, `launch_velocity(from,to)`, `GRAVITY = 28`, `LAUNCH_MAX_SPEED = 60`.
- `SHOWCASE = [168,27,244]`: SOCKEL-Block für Minion-Stufe I, Stufe k bei x+2(k-1) (11 Stück); CLIENT zeichnet animierte Minions darauf (y+1).
- `zone_name(x,y,z) -> &str` (Viertel / Satellit / „Privatinsel“ / „Himmelsbrücken“ / „Leere“), `nearest_spot(x,z) -> u8`.
- `find_safe_spawn(get, x, z) -> Option<[f32;3]>`: von y 63 abwärts höchster fester Block, 3×3×4 frei (kein Portal/Wasser), Ringe bis r=4.

## 2. Blöcke (BLOCKS, Ids fest; Muster 0..15 wie bisher, kein neues Shader-Muster nötig)
58 MOOS_BRUCH · 59 TIEFENFELS · 60 POLIERT · 61 MOOS_ZIEGEL · 62 DUNKELHOLZ · 63 DUNKELBRETT · 64 BIRKE · 65 BIRKENLAUB · 66 KIRSCHLAUB ·
67 GLUEHLAUB (leuchtet) · 68–72 STOFF_ROT/BLAU/GRUEN/GELB/WEISS (Markisen, Streifen = abwechselnd setzen) · 73 FASS · 74 KISTE ·
75 TRUHE · 76 SCHATZ · 77 WERKBANK · 78 BETT · 79 ABGEBAUT (Platzhalter beim Nachwachsen) · 80 SPRUNGPAD · 81 LAVA (flüssig, leuchtet) ·
82 GLUT · 83–85 ERZ_KOHLE/EISEN/GOLD · 86–88 ADER_SAPHIR/GOLD/SMARAGD · 89 KOHLE · 90 EISEN · 91 ROHGOLD · 92–93 KNOTEN_EICHE/BIRKE ·
94 SPLITTER_GEN · 95 SPLITTER_ADER · 96–97 BLUMEN_ROT/GELB · 98 HECKE · 99 PILZHUT · 100 PILZSTIEL · 101 WEG · 102 SCHIENE ·
103 QUESTBRETT · 104 AMBOSS · 105 SOCKEL · 106 GOLDZIER. Nachwachsend (F_REGEN): GENERATOR, 83–88, 92–95. Keine neuen F_RARE.
Legenden-Zeichen für Baupläne wählt WORLD (in `legend`).

## 3. Abbauen (Überleben)
- `hardness(b)`: Pflanzen 0,25 s · Erde/Wiese/Sand 0,4 · Holz 0,7 · Stein 1,1 · Erze 1,4 · Kristall 1,6 · Glas/Stoff 0,3 ·
  unendlich = nicht abbaubar (Luft, Wasser, Lava, PORTAL, ABGEBAUT, SCHATZ, SPRUNGPAD).
- `material(b) -> M_*` (0 Stein, 1 Erde, 2 Holz, 3 Pflanze, 4 Kristall/Glas, 5 Stoff, 6 Metall, 7 Flüssig) – Geräusche + Werkzeug.
- `mine_secs(b, tool, bau_level)`: SPITZHACKE (1014) ×0,4 auf Stein/Kristall/Metall, AXT (1015) ×0,4 auf Holz, HAMMER ×0,6 auf Stein/Kristall/
  Metall/Holz; geteilt durch (1 + 0,02·Baukunst-Stufe). `mine_ok(secs, ms)` = Server-Prüfung (70 % + 150 ms). `mine_xp(b)` Baukunst-EP.
- `regen_drop(b, roll) -> Option<u16>` Ertrag nachwachsender Blöcke (Splitter nur mit kleiner Quote). `REGEN_MS = 2500`.
- `edit_allowed(x,z,cur,place,may,in_plot)`: Hub-Archipel nur F_REGEN abbauen, nie setzen; Privatinsel nur mit Recht (`may`),
  setzen/abbauen im plot_rect, F_REGEN auf der ganzen eigenen Insel.
- **Ablauf:** Taste gedrückt → CLIENT sendet `C2S::Dig{x,y,z}` für jeden neuen Zielblock, zählt `dt / mine_secs(b, gehaltenes Werkzeug,
  bau_level)` hoch (8 Riss-Stufen). Bei 100 %: sagt den Block voraus (F_REGEN → ABGEBAUT, sonst 0), sendet `C2S::Block{b:0}`.
  Server: `dig` passt zu Stelle und `mine_ok(min über besessene Werkzeuge [0, HAMMER, SPITZHACKE, AXT] von mine_secs, now - dig_t)`,
  sonst „Zu schnell.“ + aktueller Block zurück (bestehender Fehlerweg).

## 4. Protokoll (proto.rs, angehängt → bincode bleibt kompatibel)
- `C2S::Dig { x: i16, y: i16, z: i16 }` – Abbaubeginn. Server merkt `(x,y,z,now)` im Player, keine Antwort.
- `C2S::Chest { x: i16, y: i16, z: i16 }` – [E] auf TRUHE oder SCHATZ (Reichweite REACH).
- `S2C::Spawn { p: [f32;3], yaw: f32 }` – versetzt mit Blickrichtung (Warp, Portal, Leere, Anmeldung). `S2C::Teleport` bleibt (Sprungstab).
- Sturz ins Leere braucht KEINE Nachricht: Server erkennt `Move` mit `p[1] < VOID_Y` (-25).
- Portale brauchen keine: CLIENT sendet `C2S::Warp(Warp::Home)` bzw. `Warp::Spot(d)` aus `portal_dest`.

## 5. Server (WORLD) – Regeln
- `WORLD_VERSION = "3-archipel"`: bei Wechsel `delete from chunks`, `update users set x = null, y = null, z = null`, alle Pläne neu auf
  `slots_of(...)` verteilen (`update plots set si, sj`, Reihenfolge id), Minions zurück ins Lager (Minion-Gegenstand + gelagerte Erträge), Zeilen löschen.
- Speicher: `Chunk.b` leer (`Vec::new()`) = reine Luft; `block()` liefert 0, `set_raw` legt bei Bedarf an; `gen_chunk` steigt sofort aus,
  wenn das Stück keine Insel-/Brücken-Box berührt; leere Stücke nicht speichern. `Player.sent`: `vec![0u64; ((WC*WC+63)/64)]`.
  `tool_at: [i64; 16]` (TOOLS wächst um SPITZHACKE, AXT: mana 0, cooldown 0, ohne Fähigkeit; `pub use world::{SPITZHACKE, AXT}` in skyblock, Rezepte in content).
- `spawn_at(id, hint:[f32;3], yaw)`: `find_safe_spawn(|x,y,z| self.block(x,y,z), hint.x, hint.z).unwrap_or(hint)`, Position + last_move setzen,
  `S2C::Spawn` senden. Warp::Spot(i) → SPOTS[i]; Home/Plot → plot_spawn + PLOT_YAW. Anmeldung: gespeicherte Stelle, sonst SPOTS[0]; nach
  Welcome immer `Spawn` schicken. `/hub` → Spot 0, `/is` → Home (Quest-Ziel `Goal::Home` neu).
- Move: y erlaubt -64..200. `p[1] < VOID_Y` und letzter Sturz > 2 s her → Verlust = taler·25/100 (`pay(-v, "leere")`), Ziel = `slot_at` →
  plot_spawn, sonst `SPOTS[nearest_spot]`; `spawn_at`; Hud `{"t":"leere","verlust":v}`. Bank bleibt unberührt.
- Sprungpolster: liegt die letzte angenommene Stelle mit `launch_pad(x,z,4.0)` am Polster → `boost_until = now+6000`, Tempo-Grenze
  während Boost `LAUNCH_MAX_SPEED`.
- Block b=0: Regeln Abschnitt 3; F_REGEN → `regen_drop(cur, rng)` ins Lager, `set_block(ABGEBAUT)`, Eintrag (now+REGEN_MS, x,y,z,cur) in
  einer Warteliste, `tick` stellt fällige Blöcke zurück (`set_block(..., by 0)`); `Game::open` ersetzt nach dem Laden jedes ABGEBAUT durch
  den Grundwelt-Block. Sonst Block weg, `gain(cur)`. Immer `add_xp(SK_BAU, mine_xp(cur))`, Hud `{"t":"aktion","text":"+1 Bruchstein | +3 Baukunst-XP"}`,
  `quest_hit(Mine)`. Setzen: `edit_allowed(place)`, keine F_REGEN-Blöcke und keine mit unendlicher Härte. `can_build` → `edit_allowed`.
  Hammer-Fähigkeit: gleiche Regeln (unendliche Härte überspringen, F_REGEN → ABGEBAUT + Warteliste).
- `C2S::Chest`: TRUHE → nur mit `plot_may` auf dem Platz; Hud `{"t":"truhe","x","y","z","max":27,"slots":[[item,n],…]}`. Eine Truhe je Insel
  (alle TRUHE-Blöcke einer Insel teilen sie): Tabelle `chest(plot, item, n, primary key(plot,item))`. HTTP `POST /campus/api/truhe
  {x,y,z,item,n}` (n>0 einlagern, n<0 entnehmen, höchstens 27 verschiedene) → `{slots,max}`. SCHATZ → je Spieler+Stelle einmal am Tag
  (Tabelle `schatz(user, pos, day)`), Belohnung aus `skyblock::treasure(roll) -> (taler, item, n)`, Hud toast „gut“; sonst „nein“
  „Diese Schatztruhe hast du heute schon geöffnet.“
- Chat-Kanal „Insel“: Hub = alle im Hub-Archipel, sonst gleicher `slot_at`.
- Großprojekte (P_BRIDGE/P_TOWER/P_ISLAND) bleiben als Weltzustand, werden an neue Hub-Stellen verlegt.

## 6. NPCs (content.rs; `Npc.at` wird absolut `[f32;3]` Füße, `npc_pos(n) -> [f32;3]` bleibt Signatur)
| id | Name | Ort (Vorschlag) | [E]/Klick öffnet (WEB) | Server |
|---|---|---|---|---|
| 1 | Dekan Weber | vor dem Spawn-Tempel 240.5,33,292.5 yaw π, Questtafel daneben | Dialog, Startpaket, Quest 1 | talk() vorhanden |
| 2 | Bankier Goldbach | Bank (Westseite Markt) | Einzahlen/Abheben | bank/deposit, bank/withdraw vorhanden |
| 3 | Bibliothekarin Lux | Bibliotheks-Turm | Fähigkeiten & Sammlungen | vorhanden |
| 4 | Meisterin Kolbe (Minion-Meister) | Minion-Werkstatt | Minions (Stufen über Rezepte) | vorhanden |
| 5 | Händler Brandt (Bazaar-Meister) | Bazaar-Halle | Bazaar | vorhanden |
| 6 | Torwächterin Vega | Prüfungs-Zitadelle | Dialog, Quest 6 | vorhanden |
| 7 | Schmied Hammer | Schmiede | Werkbank (Bau-Stab, Hammer, Haken, Sprungstab, Spitzhacke, Axt) | Rezepte neu |
| 8 | Auktionator Pfeiffer | Auktionshaus | Auktionshaus | NEU (nur Daten) |
| 9 | Förster Eichholz | Holzfällerhütte | Dialog | NEU (nur Daten) |
| 10 | Gärtnerin Flora | Himmels-Garten | Dialog (Hinweis auf versteckte Truhen) | NEU (nur Daten) |
Marktbewohner sind Deko: CLIENT zeichnet 6 Figuren auf festen Runden um (240, 230), y 29, ohne Id/Server.
**Quest 1** (5 Schritte, Text ≤ 60 Zeichen): Talk(1) „Hole dein Starter-Paket bei Dekan Weber“ → Home „Reise durch das Portal auf
deine Privatinsel“ → MineMinion(BRUCHSTEIN,10) „Stelle deinen Stein-Minion auf & baue 10 Bruchstein ab“ (zählt erst mit aufgestelltem Minion) → Bazaar → Check(1) „Löse 1 Check-Frage
im Lernhelfer“ (Abschluss gibt zusätzlich 5 Lern-Energie-Zellen = „5× Minion-Turbo“). Startpaket unverändert (50 Taler, Stein-Minion I, 6 h, Hammer).

## 7. WASM ⇄ JS
**Exporte (CLIENT), bestehende bleiben:** `create, backend, resize, input(x,z,jump,sprint), turn, select(block), select_tool(item),
target_prev, say, set_minions, use_tool, set_view(0|1|2), set_options(chunks 4..12, fov 60..110, ao, clouds, particles), warp(kind,n)
(kind 2: n = SPOTS-Index 0..7), set_demo, info, chat, emote, message, frame`. Geändert: `act(0)` spricht nur noch NPCs an (kein Sofort-Abbau),
`act(1)` setzen/Werkzeug wie bisher. Neu:
- `mine(on: bool)` – linke Taste / Abbau-Knopf gedrückt bzw. losgelassen (auch bei Fokusverlust/Modal `false`). Bei `true` mit NPC im Blick → wie act(0).
- `interact()` – Taste E: NPC ≤ 4 Blöcke im Blick → Talk; Zielblock TRUHE/SCHATZ → `C2S::Chest`; WERKBANK → Ereignis `werkbank`; QUESTBRETT → `questbrett`.
- `set_bau_level(level: u32)` – WEB ruft nach jedem `api('sky')` mit `sky.faehigkeiten[5].stufe`.
**Ereignisse `campusHud.event(kind, a, b, s)` (neu):**
| kind | a | b | s | WEB tut |
|---|---|---|---|---|
| `mine` | Fortschritt 0..1, -1 = abgebrochen | Block-Id | – | optional Ring (Handy) |
| `hit` | Riss-Stufe 0..7 | Material | – | Schlag-Ton, Tonhöhe steigt mit a |
| `break` | Material | Block-Id | – | Knack-/Plopp-Ton |
| `step` | Material unter den Füßen | Lautstärke 0..1 | – | Schritt-Ton je Material (ersetzt sfx 4) |
| `prompt` | 0 nichts, 1 NPC, 2 Block | NPC- bzw. Block-Id | Name | „[E] Sprechen: Dekan Weber“ / „[E] Öffnen: Truhe“ am Fadenkreuz |
| `portal` | Ziel (255 = eigene Insel) | 0 | Zielname | Whoosh + Bildschirm-Übergang |
| `launch` | 0 | 0 | Zielname | Boing |
| `void` | 0 | 0 | – | Wind + roter Rand (Toast kommt vom Server) |
| `zone` | 0 | 0 | zone_name | Anzeigetafel „Zone“ (Privatinsel: Name aus `here`) |
| `werkbank` / `questbrett` | 0 | 0 | – | Werkbank bzw. Quest-Kachel im Menü öffnen |
Bestehende Ereignisse bleiben (welcome, inv, join, leave, chat, emote, sfx, nein, here, world, teleport, said, npc, kompass, kick, hud).
`teleport` kommt auch bei `S2C::Spawn`. `sfx`-Arten neu: 9 Portal, 10 Enterhaken-Zip (CLIENT sendet 10 statt 3 beim Haken), 11 Boing,
12 Wind, 13 UI-Klick, 14 Fanfare (WEB spielt 13/14 selbst).
**Hud-JSON (`hud`-Ereignis) neu:** `{"t":"aktion","text"}` Aktionsleiste über der Leiste (2,5 s) · `{"t":"leere","verlust"}` roter Toast
„Du bist ins Leere gefallen! (-25% ungesicherte Taler)“ + „-v Taler“ · `{"t":"truhe",…}` Truhen-Fenster (POST truhe).
**Wer zeichnet was:** Fadenkreuz „+“ (2 px, weiß, `mix-blend-mode: difference`) = WEB; dünner schwarzer Rahmen um Zielblock (Reichweite 5,0),
8-stufiger Pixel-Riss, Splitter-Partikel, erste-Person-Arm/Werkzeug mit Laufwippen + Schwung, Avatar, Minions, NPCs, Wolken = CLIENT;
WebAudio, Leiste mit isometrischen CSS-Würfeln (Farbe aus `reg.bloecke[].rgb`), Avatar-Vorschau im Charakter-Fenster (eigenes Canvas-2D,
dreht per Ziehen, aus `me.look` + `reg`) = WEB.

## 8. CLIENT – Pflichten
- `world.get(y < 0)` liefert 0 (kein unsichtbarer Boden); alten „y<0 → spawn“-Rücksprung entfernen. Bei `y < VOID_Y`: einmal `void`,
  weiter Move senden, Höhe bei VOID_Y-15 anhalten, auf `Spawn/Teleport` warten (nach 3 s selbst zu `self.spawn`).
- `S2C::Spawn`: pos, yaw, pitch 0, vel 0, spawn = pos, `teleport`-Ereignis. Welcome: spawn = SPOTS[0].1.
- Portal: im PORTAL-Block (Füße oder Brust), 3 s Sperre → `portal_dest` → Warp senden + `portal`. Alte Spot-Raterei weg.
- SPRUNGPAD unter den Füßen + `launch_pad(x,z,2.0)` → `vel = launch_velocity(pos, ziel)`, bis zur Landung keine Luftsteuerung und keine -45-Grenze; `launch`.
- `may_build` → `edit_allowed` (Rechte aus `plots`: `slot_at` + `p.may` + `in_rect(plot_rect)`); Schwerkraft `w::GRAVITY`.
- Abbau-Zustand (Abschnitt 3), Ziel-Reichweite 5,0, schwarzer Drahtrahmen, Riss-Overlay, Partikel, `hit/break/step/prompt/zone` (nur bei Änderung).
- Sichtfeld: `opts.fov` live, beim Sprinten +8° weich; Hand mit Spitzhacke/Axt/Hammer-Modell; Avatar mit Haar-Schicht, Augen/Mund,
  Jacke, Gürtel, Arme/Beine mit Laufzyklus, Schuhe, Umhang/Hut/Flügel/Begleiter; Minions auf SHOWCASE (Stufe I–XI, wippend/drehend).
- Wolken als Schicht unter den Inseln (y ≈ 8–16) plus wenige darüber; Himmel bis zum Horizont, damit Inselränder in die Leere fallen.

## 9. WEB – Pflichten
- hud.css: `border-radius: 0` überall, Fläche #242428, Fase (oben/links 3px #5a5a62, rechts/unten 3px #121214), vertiefte Felder #161618
  mit umgekehrter Fase, Pixel-/Monospace-Schrift mit `1px 1px 0 #000`, feste CSS-Grids.
- Leiste: Plätze 1–8 Gegenstände (isometrische Würfel, Werkzeug-Pixelsymbole, Zahl unten rechts, Seltenheitsrahmen), Platz 9 fest
  „Campus-Stern (SkyBlock-Menü)“ (leuchtet, [9] oder M öffnet Dashboard). Darüber Fokus-Mana-Balken („100/100 Fokus-Mana“), Skill-EP-Balken, Aktionsleiste.
- Dashboard-Kacheln: Meine Insel (`game.warp(0,0)`), Marktplatz-Hub (`game.warp(2,0)`), Insel-Besuche (öffentliche Inseln → `game.warp(1,id)`),
  Lernhelfer-Belohnungen & Lern-Energie, 7 Fach-Skills, Sammlungen, Werkbank, Lern-Minions, Charakter & Kosmetik (Vorschau-Canvas),
  Einstellungen (Empfindlichkeit, Tasten-Belegung, FOV 60–110, Sichtweite 4–12, Lautstärke, FPS-Grenze in der rAF-Schleife).
- Oben links: [ESC] Menü, [T] Chat, [F5] Kamera, [M] SkyBlock-Menü. Rechts Anzeigetafel: CAMPUS SKYBLOCK, Zone, Geldbörse, Bank,
  Wissens-Splitter, Minion-Treibstoff (Xh), gelbe Box „Aktuelles Ziel“ (`sky.ziel`).
- Tasten: `menue: 'KeyM'`, neu `interagieren: 'KeyE'` (gespeichertes `menue: 'KeyE'` einmalig auf KeyM umstellen), Digit9 = Dashboard.
  Linke Maus: down `game.mine(true)`, up `game.mine(false)`; rechte Maus `game.act(1)`. Handy: Abbau-Knopf hält `mine`, neuer [E]-Knopf `interact()`.
- ModalManager: ein Öffnen/Schließen für alle Fenster (Menü, Dialog, Truhe, Chat); öffnen = `exitPointerLock`, `game.mine(false)`,
  `eingabe.reset()`; schließen per ESC, rotem [X] oben rechts oder Klick daneben; danach Zeiger wieder fangen (PC). Querformat-Sperre bleibt.
- NPC-Schilder: erstes Wort goldener Titel, Rest weißer Name, keine schwarzen Pillen. Dialog-Knöpfe je NPC laut Tabelle Abschnitt 6.
- Töne (WebAudio, prozedural): Schritte je Material, Abbau-Schläge mit steigender Tonhöhe + Plopp, Münz-Klirren bei Taler-Zuwachs,
  Fanfare bei Stufe/Quest, UI-Klick, Haken-Zip, Boing, Wind, Portal-Whoosh.

## 10. Dichte-Prüfung (WORLD, Pflicht)
Generator liefert `pub fn hub_landmarks() -> &'static [(&'static str, [i32; 3])]` (≥ 15 echte Mehrblock-Bauten). `#[test] hub_dichte`
erzeugt alle Stücke des Hub-Archipels und prüft: ≥ 35 000 feste Blöcke, ≥ 15 Landmarken, Luft-Hohlräume unter y 22 innerhalb der
Hauptinsel (mit Fels darüber), < 12 % der Oberflächen-Spalten der Hauptinsel nackte flache Wiese (WIESE oben, 4 Nachbarn gleich hoch,
nichts darüber). Gemessene Werte per `eprintln!` ausgeben. Außerdem Test: jeder SPOTS-Eintrag und jedes plot_spawn liefert per
`find_safe_spawn` einen Punkt höchstens 3 Blöcke über/unter dem Vorschlag; jede PORTALS-Mitte hat PORTAL-Blöcke im Umkreis 6.

## 11. Checklisten
**WORLD** – M1: Konstanten/Lage übernehmen, Inselkegel + Leere, Hub 6 Viertel + 3 Satelliten + Brücken/Portale/Polster, Privatinsel-Layout,
WORLD_VERSION + Umzug, Speicher sparsam, `hub_dichte` grün, e2e auf neue Lage. M2: Dig/mine_ok, Nachwachsen, Leere-Verlust, Sprung-Boost,
Spitzhacke/Axt. M4: NPC-Tabelle, Quest 1 + Goal::Home, Truhe/Schatz, `spawn_at`. Alle Tests `cargo test --release` grün.
**CLIENT** – M1: kein Boden unter y 0, Spawn-Nachricht, Wolken unten. M2: Leere, Abbau, Rahmen, Riss, Partikel, FOV, Hand, Avatar, Polster.
M4: Portale, interact/prompt, Minion-Schaukasten, Marktbewohner, Schritt-/Abbau-Ereignisse.
**WEB** – M2: Fadenkreuz, FOV-/Sicht-Einstellungen, Leere-Anzeige. M3: Voxel-Theme, Leiste + Stern, Dashboard, Anzeigetafel, Knöpfe,
ModalManager, Avatar-Vorschau, Tasten-Belegung. M4: Dialog-Knöpfe, Truhen-Fenster, prompt, Portal-Übergang, alle Töne.
