# Campus Phasen 16–20 – Vertrag für die Bauteile

Ergänzt CONTRACT.md (gilt weiter). Regelt Schnittstellen; nichts davon umbenennen. Spieltext Deutsch, eigene Namen/Grafik.
Niemand kompiliert lokal (kein Rust auf Windows). Der Architekt prüft am Ende auf dem Server. Darum: Signaturen exakt so,
keine neuen Abhängigkeiten (Cargo.toml nicht ändern), keine `unsafe`, keine `unwrap` auf Eingaben von außen.

## 0. Besitz – nur eigene Dateien ändern
| Agent | Dateien (nur diese!) |
|---|---|
| S-INV | `crates/server/src/inv.rs`, neu `crates/server/tests/rpg_inv.rs` |
| S-ISLAND | `crates/server/src/island.rs`, neu `crates/server/tests/rpg_island.rs` |
| S-MOBS | `crates/server/src/mobs.rs`, neu `crates/server/tests/rpg_mobs.rs` |
| C-MAIN | `crates/client/src/lib.rs`, `crates/client/src/world.rs`, neu `crates/client/src/fx.rs` |
| C-DECO | `crates/client/src/avatar.rs`, neu `crates/client/src/deco.rs` |
| C-GFX | `crates/client/src/gfx.rs`, `crates/client/src/shaders/*.wgsl` |
| W-INV | neu `web/inventar.js`, neu `web/inventar.css` |
| W-HUD | `web/hud.js`, `web/index.html`, `web/hud.css`, `web/hilfe.html` |
Fertig (Architekt, nicht ändern): `crates/shared/**` (rpg, inventory, stats, enchanting, farming, combat, proto, *_data, world-Blöcke 108–122),
Server-Einhängepunkte in `game.rs`, `sky.rs`, `econ.rs`, `rpg.rs` (Weiche + Werte + Register). Lies diese Dateien, bevor du baust.

## 1. Geteilt (crates/shared) – Kurzüberblick
- **Blöcke** 108 GROSSE_TRUHE, 109 ACKERBODEN, 110–114 SAAT_* (Flag `F_INST`: nicht im Gitter gezeichnet, nicht fest, aber anvisierbar
  – der Browser zeichnet die Pflanze je Stufe), 115 SPRUNGPLATTE, 116–119 TELE_ROT/BLAU/GRUEN/GOLD, 120 STAENDER, 121 ALTAR (Bibliothek,
  `ALTAR_AT`), 122 BOSS_ALTAR (Zitadelle, `BOSS_ALTAR_AT`), 107 SCHILD wird setzbar. `SAATEN`, `TELEPORTER`, `CITADEL`, `CITADEL_HALL`,
  `CITADEL_ARENA`, `CITADEL_Y`, `in_citadel(x,y,z)`, `anchor(x,y,z)`, `mine_secs_speed(b,tool,bau,speed_pct)`. `edit_allowed`: eigene/Co-op-Insel
  überall auf dem ganzen Platz (96×96, y 1..63) setzen/abbauen; Hub und fremde Inseln gesperrt; Ankerblock unter `plot_spawn` geschützt.
- **rpg.rs**: Nummern ARMOR(2000+Set·4+Teil), ARTIFACT 2040..2047, Waffen 2100–2104 (HOLZ_KLINGE, SAPHIR_KLINGE, ZITADELLEN_SCHWERT nah;
  FOKUS_BOGEN, ARKANER_STAB fern), ERNTE_HACKE 2105, SEGEN_HACKE 2106 (beide in skyblock::TOOLS), ERNTE 2300+Pflanze, Drops 2400–2412.
  `equip(id)`, `set_def`, `mob(id)`, `crop(i)`, `is_weapon`, `is_ranged`, `rarity`, `sell_value`, `lore`. Daten: equipment_data (SETS, EQUIP,
  EQUIP_RECIPES – in skyblock::RECIPES eingehängt), mobs_data::MOBS (0 Schleim, 1 Golem, 2 Wächter, 3 Archon-Boss, 4 Diener),
  enchants_data::ENCHANTS, farming_data::CROPS (Weizen 600 s …), questline_data::CHAPTERS (12 Kapitel, Schritte = (Goal, Text)).
- **inventory.rs**: `Stack{item,n}`, INV_SLOTS 36 (0..9 Leiste), CHEST_SMALL 27, CHEST_LARGE 54, GEAR_SLOTS 7 (0 Helm,1 Brust,2 Beine,3 Stiefel,
  4..7 Talismane), `in_slots`, `stack_max`, `gear_fits`, `count`, `totals`, `room`, `add`, `remove`, `reconcile`, `xfer`, `move_in`, `quick_in`,
  `quick_between`, `sort`, `quick_stack`. Alle Züge erhalten Summen (Test).
- **stats.rs**: `Stats`, `compute(levels, gear, held, ench)`, `reduction`, `full_set`, `mana_cost`, SET_* Nummern, BASE_HP 100.
- **enchanting.rs**: N_ENCH 6 (EFFIZIENZ, GLUECK, SCHAERFE, SCHUTZ, WEISHEIT, TELEKINESE), `kind`, `applies`, `cost(ench, level)->(fach,
  splitter, kristalle)`, `effect`. Verzauberung gilt je (Spieler, Gegenstands-Nummer).
- **farming.rs**: `crop_of(block)`, `grow_ms(crop, fert)`, `stage(planted, now, grow)` 0..3, `harvest(crop, crop_fortune, roll)`, FERT_BASE/FERT_BAUERN.
- **combat.rs**: MELEE_REACH 4, RANGED_REACH 28, SWING_CD_MS 450, SHOT_CD_MS 600, STAFF_MANA, KNOCKBACK, FLASH_MS 150, SHIELD_MS 5000,
  MOB_HIT_CD_MS, BOSS_EVERY_MS 300 000, LOOT_SHARE_PCT 10, OUTER_MOBS, HALL_MOBS, BOSS_MINIONS, SHOCK_* , TELEGRAPH_MS, CHARGE_SPEED,
  `player_hit(stats, roll)->(dmg, crit)`, `taken(raw, def)`, `boss_phase`, `ray_sphere`, `loot_share`.
- **proto.rs (angehängt)**: C2S `Held(u8)`, `Attack{mob}`, `Shoot{dir}`. S2C `Mobs(Vec<MobState>)`, `Damage{at,n,crit,mob}`,
  `Gear{id,items:[u16;8]}` (0..4 Rüstung, 4..7 Talismane, 7 gehalten), `Island{now,crops,signs,stands}`, `Health{hp,max,shield_ms}`,
  `Shock{at,r}`, `Shot{from,to,kind}`, `Drops{at,items}`. MobState{id,kind,p,yaw,hp(Promille),fl}, Bits MOB_HURT 1, MOB_WINDUP 2,
  MOB_SLAM 4, MOB_DEAD 8. `C2S::Chest{x,y,z}` heißt jetzt allgemein „[E] benutzen“.
- **skyblock::Goal** neu: Place(n), Fuel, Equip(n), Plant(n), Harvest(n), Compact, Enchant, UseTool(id), Like, Kill(kind,n), Chest; `goal.need()`.

## 2. Server (schon verdrahtet, siehe game.rs/sky.rs/rpg.rs)
Player neu: `held, hp, shield_until, swing_at, shot_at, hurt_at, pad_at, island_sent, stats` (Cache!). Game neu: `mobs: mobs::Mobs`,
`trades: inv::Trades`, `stash_notes: RefCell<Vec<u32>>`. Tabellen (alle schon angelegt in game.rs RPG_SCHEMA): inv_layout(user,data blob),
player_stash(user,item,n), player_equipment(user,slot,item), item_ench(user,item,ench,level), island_chests(x,y,z,plot,size,data blob),
island_crops(x,y,z,plot,kind,planted,fert), island_functional_blocks(x,y,z,plot,kind,data text), coop_members(plot,user,since),
island_likes(plot,user,stamp,ts), questbook(user,chapter,step,n), trade_log. Blobs = `proto::enc(&Vec<Stack>)`.
`rpg.rs` (fertig): `stats(uid)` (frisch), `refresh_stats(id)` (Cache + Hud `werte` + S2C::Health), `stats_json`, `mine_speed`, Weiche
`rpg_get/rpg_post` → inv_get/inv_post → island_get/island_post → mobs_get. Fehler immer als `crate::E(code, text)` / `no(code, text)`.
Mutationen über mehrere Tabellen immer in `self.tx(|g| …)` (Rollback bei Fehler). Niemals Gegenstände an `give()` vorbei bewegen
(einzige Ausnahme: InventoryTx selbst). `self.hud(id, json)` für Oberfläche, `self.send(id, &S2C::…)` für den Browser-Kern.

### §S-INV (inv.rs)
- `InventoryTx::execute(g:&Game, user, item, delta)->R<u32>`: Zählung `inv` + Plätze `inv_layout` gleich halten. delta<0: Zählung prüfen,
  `inventory::remove` aus den Plätzen (bevorzugt der gehaltene Leisten-Platz `players[user].held` wenn verbunden). delta>0 und
  `in_slots(item)`: so viel wie `room` erlaubt in Plätze+Zählung, Rest in `player_stash`, `g.stash_notes.borrow_mut().push(user)`. Cosmetics
  (nicht in_slots): nur Zählung wie bisher. Liefert neue Zählung. Plätze laden: fehlt die Zeile oder passt sie nicht zur Zählung →
  `inventory::reconcile` (Überlauf: Zählung senken, in den Beutel). Muss in einer schon offenen Transaktion funktionieren (nur `g.db`).
- `inv_join(id)`: Plätze angleichen, Hud `inv` schicken, Hud `stash{n}` wenn Beutel nicht leer, S2C::Gear aller anderen an id und eigene an alle,
  `refresh_stats(id)`.
- `flush_notes()`: `stash_notes` leeren, je Konto Hud `{"t":"stash","n":<Summe>}`.
- `gear(uid)->[u16;7]` aus player_equipment. `held_item(uid)`: Plätze[held] (0 wenn leer/offline). `set_held(id,i)`: i<9 setzen,
  `refresh_stats`, S2C::Gear an alle.
- Truhen: `chest_open_new(id,x,y,z)`: Block TRUHE (27) oder GROSSE_TRUHE (54); Recht `plot_may` (Besitz/Co-op/Gruppe) auf dem Platz,
  Reichweite eco::REACH; Zeile bei Bedarf anlegen; alte Insel-Truhe (Tabelle `chest(plot,item,n)`) beim ersten Öffnen der ersten Truhe dieser
  Insel übernehmen (Überlauf → Beutel des Besitzers), danach alte Zeilen löschen. Hud `{"t":"truhe","x","y","z","max","slots":[[item,n]|null…]}`.
  `chest_can_break(id,x,y,z)`: nicht leer → `no(409,"Die Truhe ist nicht leer. Räume sie erst aus.")`; leer → Zeile löschen.
  Besitzer darf mit POST `truhe/leeren` alles in seinen Beutel räumen.
- Rüstungsständer: Inhalt (7 Plätze, nur Ausrüstung) in island_functional_blocks.data als JSON `[u16;7]` (kind 120). `stand_open` Hud
  `{"t":"staender","x","y","z","items":[…7]}` (nur Insel-Recht darf ändern, Besucher sehen über S2C::Island). Nach Änderung:
  `self.island_refresh(plot)` aufrufen (stellt S2C::Island neu zu – liefert S-ISLAND).
- HTTP (über inv_get/inv_post, Antwort JSON):
  - GET `inv` → `{"slots":[[item,n]|null ×36],"gear":[7],"held":i,"stash":[[item,n]…],"stashN":n,"werte":stats_json}`.
  - POST `inv/move {von:Ort, nach:Ort|null, n}` – Ort = `{"art":"inv"|"truhe"|"gear"|"staender","i":Platz,"x","y","z"}`; n = 0 ganzer Stapel,
    sonst Menge (Rechtsklick = Hälfte bzw. 1, rechnet der Browser). `nach:null` = Schnell-Verschieben: inv-Leiste↔Rucksack, oder inv↔offene
    Truhe (dann `ziel:{x,y,z}` im Body), gear→inv. Ablauf: alle beteiligten Behälter laden, `inventory::*` anwenden, Summen vorher==nachher
    prüfen, Zählungen (`inv`) für inv↔Behälter-Züge per direkter Zählungs-Buchung (InventoryTx) mitbuchen, alles in EINER `self.tx`.
    Ausrüstung: `gear_fits`; Antwort wie GET `inv` plus `"truhe":{slots,max}` wenn beteiligt. Danach `refresh_stats`, Gear an alle, bei 4 Teilen
    `quest_hit(Goal::Equip(k))` mit `Some(k)` wenn ≥ k getragen.
  - POST `inv/sort {truhe?:{x,y,z}}` (ohne truhe: Rucksack 9..36), POST `truhe/einlagern {x,y,z}` (quick_stack), POST `stash/claim`
    (so viel wie passt zurück; Rest bleibt), POST `truhe {x,y,z,item,n}` (alt: n>0 einlagern, n<0 entnehmen – über dieselbe Logik).
  - Handel: POST `trade/request {name}`, `trade/accept {von}`, `trade/offer {items:[[item,n]…≤12], taler}`, `trade/ready {an}`, `trade/confirm`,
    `trade/cancel`. Regeln: beide verbunden, Abstand egal; jede Änderung an Angebot/Taler setzt beide `bereit=false` und startet die 3-s-Sperre
    neu; `confirm` nur wenn beide bereit und 3 s seit letzter Änderung; Ausführung in EINER `tx`: Angebote gegen Zählungen prüfen,
    give/pay in beide Richtungen, Eintrag trade_log; Abbruch bei Trennung (`trade_abort`) ohne Bewegung. Hud an beide nach jeder Änderung
    `{"t":"handel","id","mit","ich":{items,taler,bereit},"du":{items,taler,bereit},"sperre_ms"}`, Abschluss `{"t":"handel","fertig":true}`,
    Abbruch `{"t":"handel","abbruch":text}`. `/trade <Name>` = request (Hud `{"t":"handel_anfrage","von","name"}` an Ziel).
- Tests `tests/rpg_inv.rs` (Game::open auf Temp-DB, ensure_user, ohne WebSocket wo möglich): Plätze/Zählung bleiben gleich über 100
  zufällige inv/move-, Truhen-, Sortier-, Beutel- und Handelsfälle; Handel mit Abbruch mitten drin bewegt nichts; Taler-Summe erhalten.

### §S-ISLAND (island.rs)
- `on_move(id, now)`: Block unter den Füßen: SPRUNGPLATTE → `boost_until = now+4000` (Browser springt selbst, siehe C-MAIN);
  TELE_* → passenden zweiten Teleporter gleicher Farbe auf demselben Platz suchen (island_functional_blocks kind 116..119), `pad_at` 1,5 s Sperre,
  `spawn_at(ziel über dem Pad)`, Hud `{"t":"tele"}`. Platz gewechselt (`slot_at` ≠ `island_sent`) → `island_refresh_for(id)`.
- `island_join(id)`, `island_refresh(plot)` (an alle auf diesem Platz), S2C::Island{now, crops, signs, stands} für den Platz.
- `before_place`: Saat nur auf ACKERBODEN darunter; nur auf Privatinsel mit Recht. `after_place`: Saat → island_crops (fert 2000, wenn Setzer volle
  Bauern-Tracht trägt **und** es seine eigene Insel ist, sonst 1000), `quest_hit(Plant(1))`; TRUHE/GROSSE_TRUHE → `quest_hit(Chest)`;
  SCHILD → Zeile (kind 107, leere Zeilen) + Hud `{"t":"schild_neu","x","y","z"}`; STAENDER/TELE_*/SPRUNGPLATTE → Zeile eintragen; jeder
  gesetzte Block auf eigener Insel → `quest_hit(Place(1))`; danach `island_refresh`.
- `before_break`: zuerst `chest_can_break` bei Truhen; Saat: Stufe aus Zeit; Stufe 3 → Ernte `farming::harvest` (Ernte-Glück aus stats)
  in Zählung (`gain`), Naturwissenschaften-EP, `quest_hit(Harvest(1))`; mit gehaltener SEGEN_HACKE bleibt die Pflanze (planted=now) → true;
  sonst Block weg + Saat zurück (unreif: nur Saat zurück). Ständer nicht leer → Fehler. Zeile löschen, island_refresh. Rückgabe true wenn erledigt.
- `use_block(id,x,y,z,b)`: ALTAR → Hud `{"t":"oeffne","reiter":"Verzaubern"}`; BOSS_ALTAR → `self.boss_summon(id)`; STAENDER → `self.stand_open`;
  SCHILD → Besitzer/Co-op: Hud `schild_neu` (bearbeiten), sonst Hud `{"t":"schild","zeilen":[…]}`; sonst `no(404, "Hier steht keine Truhe.")`.
- `till(id, tool, dir)`: Strahl 5 Blöcke, WIESE/ERDE mit Recht → ACKERBODEN (`set_block`), Abklingzeit über `tool_at`.
- Verzaubern: `ench(uid,item)->[u8;6]` aus item_ench. POST `enchant {item, ench}`: besitzt den Gegenstand (Zählung oder getragen), `applies`,
  nächste Stufe ≤ max, Kosten `enchanting::cost` (Splitter SHARD+fach, FOKUS_KRISTALL) – ein VERZ_BUCH ersetzt die Splitter-Kosten, wenn
  vorhanden; nur in Reichweite 8 des ALTAR_AT; `tx`; `quest_hit(Enchant)`; `refresh_stats`. GET `enchants` → `{"gegenstaende":[{item,stufen:[6]}],
  "kosten":…}` für alle verzauberbaren eigenen Gegenstände.
- Schild: POST `sign {x,y,z,zeilen:[≤4]}` – je Zeile `sky::clean`, höchstens 24 Zeichen; nur Insel-Recht; island_refresh.
- Co-op: `/coop invite <Name>` (nur Besitzer, höchstens 4, Konto muss existieren) → coop_members + `load_plots()` + `send_plots()`;
  `/coop remove <Name>`; `/coop` listet. POST `coop {name, add}` gleich. Besucher ohne Recht: nur ansehen, Schild lesen, liken.
- Likes: `like_here(id)` = Insel unter den Füßen; POST `like {plot, stempel 0..5}`: nicht eigene Insel, je Konto+Insel einmal (Stempel änderbar),
  `quest_hit(Like)`. GET `top_inseln` → `[{"id","name","likes","stempel":[6 Zähler],"besucht":bool}]` nach Likes, nur offene Inseln (`visit==0`).
- Questbuch: `book_hit(uid, f)` wie quest_hit, aber auf questbook (Kapitel 1..12, Schritt, Zähler; `goal.need()`); Kapitelende: Taler + Splitter
  (SHARD+4), Hud `{"t":"kapitel","nr","name","fertig":true}` + Fanfare im Browser; Schritt: Hud `{"t":"kapitel","nr","schritt"}`.
  `book_state(uid)` → `{"kapitel","name","schritt","text","stand","von","fertig":bool}`. GET `questbuch` → alle 12 mit Status (erledigt/aktiv/offen,
  Schritte mit Häkchen). Hinweis: Ziele, die schon vor dem Kapitel erfüllt waren (Talk Dekan), dürfen beim Kapitelstart sofort zählen.
- Tests `tests/rpg_island.rs`: Stufe 0→3 nur über Zeit (+600 s, ohne tick), Ernte gibt Ertrag, Saat nur auf Acker, Fremde dürfen nicht
  bauen/ernten, Co-op darf, Likes einmalig, Questbuch rückt vor, Verzaubern kostet richtig und erhöht Werte.

### §S-MOBS (mobs.rs)
- `Mobs` (Default): Wesen-Liste, nächste id, Boss-Zustand (Schaden je Spieler), letzte Boss-Zeit, `ai_steps`, Stoßwellen. **Keine Allokation
  im Leerlauf.** `mob_tick(now)` beginnt mit: Zähle Spieler mit `in_citadel(p)`; `if citadel_active == 0 { Wesen leeren (clear, nicht
  neu anlegen) einmalig; return; }` – danach KI (ai_steps += 1): Nachfüllen auf OUTER_MOBS im Außenring (CITADEL minus HALL/ARENA, Arten 0/1),
  HALL_MOBS Wächter (2) in CITADEL_HALL; Boss (3) alle BOSS_EVERY_MS oder per `boss_summon` in der ARENA mit Dienern (4) ab Phase 1.
  Bewegung: auf nächsten Spieler in der Zitadelle zu, nur innerhalb ihres Rechtecks, Füße auf CITADEL_Y, kein Durchlaufen fester Blöcke
  (einfach: Zielschritt verwerfen, wenn `world::solid(block(x, y, z))` auf Fuß- oder Kopfhöhe). Wächter: TELEGRAPH_MS ausholen (fl MOB_WINDUP),
  dann Ansturm mit CHARGE_SPEED. Boss: Stoßwelle (fl MOB_SLAM vorher 1 s), S2C::Shock; Spieler am Boden (y − CITADEL_Y < SHOCK_JUMP_Y) im Band
  nehmen SHOCK_DMG. Treffer auf Spieler: `combat::taken(dmg, stats.defense)`, `hurt_at`, Schild schützt, S2C::Health + S2C::Damage{mob:0}.
  HP ≤ 0: Wiederbelebung bei `SPOTS[4]` (`spawn_at`), volle HP, `shield_until = now+SHIELD_MS`, Hud `{"t":"tod"}` – kein Gegenstandsverlust.
  Leben-Regeneration (stats.hp_regen) in der Zitadelle je Takt. S2C::Mobs alle 100 ms an Spieler mit Abstand < 64 zur Zitadelle; beim Verlassen
  einmal leere Liste.
- `attack(id, mob, now)`: Wesen existiert, Abstand Auge→Wesen ≤ MELEE_REACH + Größe/2, `now - swing_at ≥ SWING_CD_MS`; Schaden
  `combat::player_hit(stats, rng)`; Rückstoß, fl MOB_HURT für FLASH_MS; S2C::Damage an alle in der Nähe; Tod → Beute: je Drop (Promille) für
  den Töter (Boss: für jeden mit `loot_share`), `gain`, Taler (`pay(…,"kampf")`), EP Fähigkeit 3 (Gesellschaft), S2C::Drops an den
  Empfänger (nicht mit Telekinese), `quest_hit(Kill(kind,1))`. Boss: Hud an alle in der Zitadelle `{"t":"boss","name","hp","max","phase"}` bei
  jedem Treffer und `{"t":"boss","aus":true}` beim Tod.
- `shoot(id, dir, now)`: gehaltene Fernwaffe (`is_ranged(held_item)`), SHOT_CD_MS, ARKANER_STAB kostet `spend_mana(STAFF_MANA)`; erstes Wesen
  per `ray_sphere` bis RANGED_REACH, Blöcke blocken (world::raycast-Abstand); S2C::Shot an alle in der Nähe; Treffer wie attack.
- `boss_summon(id)`: nur wenn kein Boss lebt und Rufer in der Zitadelle, Hud an alle dort `{"t":"toast","art":"gut","text":"Der Große Prüfungs-Archon erwacht!"}`.
- `combat_join(id)`: `refresh_stats(id)`, S2C::Health.
- GET `kampf` → `{"wesen":Anzahl,"boss":{…}|null,"ki_schritte":ai_steps}` (für Tests/Verwaltung).
- Tests `tests/rpg_mobs.rs`: 1000 Takte ohne Spieler in der Zitadelle → `ai_steps == 0` und keine Wesen; Spieler betritt → Wesen erscheinen;
  Schlag mit Reichweite/Abklingzeit, Rückstoß, Tod gibt Beute; Boss-Beute nur ab 10 % Anteil; Tod des Spielers → Wiederbelebung ohne Verlust.

## 3. Browser-Kern (WASM)
### Exporte (C-MAIN, alle bisherigen bleiben)
`held(slot: u8, item: u16)` – Leisten-Wahl: sendet `C2S::Held(slot)`, setzt Block (item<256) / Werkzeug (TOOLS) / Waffe (rpg::is_weapon) in der Hand.
`set_stats(mining_speed: f32, telekinese: bool)` – aus Hud `werte`. `set_texture(res: u32)` (16/64/128 → Gfx), `set_texture_pack(rgba: &[u8],
w: u32, h: u32)`, `clear_texture_pack()`. `mine(on)`: hält man eine Nahkampfwaffe oder zielt auf ein Wesen in MELEE_REACH → Schlag
(`C2S::Attack`, wiederholt alle SWING_CD_MS solange gedrückt, Schwung-Animation); Fernwaffe → `C2S::Shoot` (Blickrichtung); sonst Abbau wie bisher.
`interact()`: zusätzlich GROSSE_TRUHE, ALTAR, BOSS_ALTAR, STAENDER, SCHILD → `C2S::Chest`.
### Ereignisse `campusHud.event(kind,a,b,s)` neu
| kind | a | b | s |
|---|---|---|---|
| `hp` | hp | max | Schild-ms als Text |
| `pop` | 0 | 0 | – (Beute eingesammelt) |
| `treffer` | 1 = krit | 0 eigener Treffer, 1 man selbst getroffen | – |
| `schwung` | 0 | 0 | – |
| `schock` | 0 | 0 | – (Stoßwelle startet) |
| `schilder` | Anzahl | 0 | JSON `[[x,y,z,"Zeile1\nZeile2…"],…]` (Reihenfolge = Plakette-Nummer) |
| `sprung` | 0 | 0 | – (Sprungplatte) |
### Überlagerung (DOM, W-HUD)
- `campusHud.plates(a)` bleibt [id, x, y, w]; neu: Schilder als id `SIGN_BASE + k` (SIGN_BASE = 2 000 000, k = Index aus `schilder`),
  nur ≤ 10 Blöcke entfernt.
- neu `campusHud.numbers(a: &[f32])` je Zahl [x 0..1, y 0..1, wert, krit 0/1, alter 0..1] (Schadenszahlen, schweben 0,9 s nach oben).
### Interne Aufteilung
- **C-MAIN** (`lib.rs`, `world.rs`, neu `fx.rs`): Nachrichten Mobs/Damage/Gear/Island/Health/Shock/Shot/Drops verarbeiten;
  **Schnappschuss-Interpolation**: je Fernspieler und Wesen Ringpuffer (≥ 4) von (Zeit, Pos, Gier); gezeichnet wird bei `jetzt − 100 ms`, Position
  linear, Gier über kürzesten Winkel, Laufzyklus aus der Geschwindigkeit; Spieler-Zeit = lokale Ankunftszeit (Server schickt 10 Hz).
  Schadenszahlen, Beute-Magnet (Würfelchen fliegt im Bogen, ab 3,5 Blöcke zur Figur gezogen, `pop`; auch beim eigenen Abbau, außer Telekinese),
  Pflanzen je Stufe (`farming::stage(planted, server_now + (lokal − Empfang), grow_ms)`), Rüstungsständer, Schilder (Plaketten), Stoßwellen-Ring,
  Geschosse, Lebensbalken über Wesen, Boss 3× Größe. Sprungplatte: steht man auf SPRUNGPLATTE (am Boden) → `vel = Blickrichtung waagrecht ×
  12 + y 16`, `sprung`. Teleporter macht der Server (Spawn). Erste Person: Waffe in der Hand mit Schwung-Bogen. Rot-Aufblitzen 150 ms.
- **C-DECO** (`avatar.rs`, neu `deco.rs`) – reine Zeichenfunktionen (Quader in `Vec<Inst>`), keine Spiellogik:
  - `Pose` bekommt `pub gear: [u16; 8]` (0..4 Rüstung, 4..7 Talismane, 7 gehalten) und `pub hurt: f32` (0..1 Rot-Anteil). `avatar()` zeichnet
    Rüstung als Überzug (Helm, Brustplatte, Beinschienen, Stiefel in `rpg::equip(id).rgb`, leicht größer als Körperteile), gehaltene Waffe in der
    rechten Hand (wenn `held` None und gear[7] eine Waffe), Zitadellen-Set (`stats::full_set == SET_ZITADELLE`) mit goldenen Glüh-Würfelchen.
  - `deco.rs`: `pub fn mob(out:&mut Vec<Inst>, kind:u8, pos:Vec3, yaw:f32, walk:f32, hurt:f32, fl:u8, time:f32)` (Schleim = hüpfender Würfel,
    Golem = klobig, Wächter = Ritter mit Schild/Lanze, Archon = 3× großer schwebender Magier mit Krone/Ringen, Diener klein; Farben aus
    `rpg::mob(kind)`, `size`); `pub fn health_bar(out, top:Vec3, frac:f32, width:f32)`; `pub fn crop(out, base:Vec3, kind:usize, stage:u8, time:f32)`
    (Stufe 0 Keim … 3 reif mit Frucht in rgb2, glühend bei Beere/Kristall); `pub fn drop_item(out, pos:Vec3, item:u16, spin:f32, scale:f32)`;
    `pub fn stand(out, pos:Vec3, items:&[u16;7], time:f32)` (Ständerfigur + Rüstung); `pub fn shock_ring(out, at:Vec3, r:f32)`;
    `pub fn shot(out, from:Vec3, to:Vec3, t:f32, kind:u8)`; `pub fn weapon(out, m:Mat4, item:u16)` (Klinge/Bogen/Stab für Hand, Ursprung = Griff);
    `pub fn item_rgb(item:u16)->[u8;3]`.
- **C-GFX** (`gfx.rs`, Shader): `pub fn set_tex_res(&mut self, res: f32)` (16/64/128 → `Env.par2.w`, Gfx setzt es wie par/scr selbst),
  `pub fn set_texture_pack(&mut self, rgba:&[u8], w:u32, h:u32)` (Atlas 4×4 Kacheln = Muster 0..15, multipliziert mit der Blockfarbe; eigene
  Textur+Sampler-Bindung), `pub fn clear_texture_pack(&mut self)`. Shader: alle `16.0`-Pixelstufen der Blockmuster über `g.par2.w`;
  ab 64 zusätzlich feine Mehr-Oktaven-Details (Steinkorn, Holzfasern, Fugen-Fasen, Kristallfacetten). Nachbearbeitung (Halo-Fix): Tiefenunschärfe
  nur bei linearer Tiefe > 48 (glatter Übergang 48..96, nie davor); Bloom-Hochrechnung mit tiefenbewusstem Kantengewicht
  `exp(-abs(d_mitte - d_abgriff) * 14.0)` (lineare Tiefen) beim Einmischen von t_q1/t_q2; Hell-Schwelle `threshold = 1.4`, `soft_knee = 0.3`
  in `fs_bright`. Nach der Arbeit: WGSL muss für naga gültig sein (WebGPU + WebGL2/GLSL ES).

## 4. Oberfläche (web/)
### W-INV (`inventar.js` ES-Modul, `inventar.css`) – Schnittstelle (exakt):
```js
export function initInventar(ctx)   // ctx = { api, el, $, ton, toast, fehler, nameVon, icon, reg:()=>reg, me:()=>me, game:()=>game,
                                    //         modalAuf:(id)=>void, modalZu:()=>void, ladeMich:()=>Promise, TOUCH }
export function oeffneInventar()    // Taste E auf nichts / I / Knopf: Fenster #inventar (Ausrüstung+Avatar-Vorschau links, 27 Rucksack, 9 Leiste)
export function truheZeigen(j)      // Hud 'truhe' (oben Truhe 27/54, unten Rucksack+Leiste, [Sortieren] [Alles einlagern])
export function staenderZeigen(j)   // Hud 'staender'
export function handelZeigen(j)     // Hud 'handel' / 'handel_anfrage'
export async function verzaubernSeite()  // DOM-Knoten-Liste für die SkyBlock-Menü-Seite 'Verzaubern'
export function stashAnzeigen(n)    // Hud 'stash': Abzeichen "Beutel voll! n Items im /stash – Klicke zum Abholen" (#stash-abzeichen)
export async function ladeInventar()// GET inv; danach ctx-Ereignis window.dispatchEvent(new Event('campus-inventar'))
export function plaetze()           // aktuelle 36 Plätze [[item,n]|null] (für die Leiste)
export function tooltip(item, n)    // DOM-Element: Name in Seltenheitsfarbe, Lore, Werte-Boni, Verzauberungen, Verkaufswert
```
Bedienung: Linksklick nimmt/legt/legt zusammen (Stapel „am Zeiger“, schwebt mit), Rechtsklick teilt (Hälfte nehmen) bzw. legt 1 ab,
Umschalt+Klick (Touch: Schalter „Schnell-Verschieben“) schnell verschieben, Ziffern 1–9 über einem Platz tauschen mit Leisten-Platz, Tooltip bei
Hover/langem Tippen. Jeder Zug = ein POST `inv/move` (Zeiger-Stapel ist nur Anzeige; Ablegen schickt von→nach). Seltenheitsfarben: Gewöhnlich
weiß, Selten blau, Episch lila, Legendär gold. Ausrüstung 4 Plätze + 3 Talismane neben einer 3D-Avatar-Vorschau (Canvas 2D wie hud.js
`avatarVorschau`, eigene Kopie, mit Rüstungsfarben). Handelsfenster 4×3 je Seite + Taler, [Bereit]-Häkchen, 3-s-Countdown, [Handel abschließen].
### W-HUD (`hud.js`, `index.html`, `hud.css`, `hilfe.html`)
- `import * as inventar from './inventar.js?v=__V__'`, `<link rel=stylesheet href="inventar.css?v=__V__">`, neues Fenster `#inventar` in FENSTER.
- Leiste: 9 Plätze = `inventar.plaetze()[0..9]`, Tasten 1–9 + Mausrad, Wahl → `game.held(i, item)`; Campus-Stern wandert als eigener Knopf neben
  die Leiste (M/Tab öffnen das Menü, 9 wählt jetzt Platz 9). Taste I (oder E ohne Ziel) öffnet das Inventar.
- Hud-Ereignisse: `inv` → ladeInventar; `stash`, `truhe`, `staender`, `handel`, `handel_anfrage` (Annehmen/Ablehnen-Toast), `werte`
  (→ `game.set_stats(abbau, telekinese)`, Tafel), `boss` (Boss-Leiste oben Mitte), `tod`, `tele`, `kapitel` (Fanfare ton(14), Tafel), `schild_neu`
  (Fenster: 4 Zeilen à 24 Zeichen → POST sign), `schild` (lesen), `oeffne` mit `Verzaubern`/`Questbuch`.
- Kern-Ereignisse: `hp` (Herz-Leiste `♥ 240 / 240` neben Mana), `pop`, `treffer`, `schwung`, `schock`, `schilder` (Texte für Plaketten),
  `sprung`; `campusHud.numbers` → Schadenszahlen (weiß, Krit gold mit ✦…!).
- Menü-Seiten: `Verzaubern` (inventar.verzaubernSeite), `Questbuch` (Taste J; 12 Kapitel mit Häkchen, aktives hervorgehoben),
  `Insel-Kompass` mit Reiter „Top-Inseln“ (GET top_inseln, [♥ Insel liken]), Co-op in „Grundstück“ (POST coop).
- Anzeigetafel rechts: aktives Questbuch-Ziel (`sky.buch`), Leben, Set-Bonus.
- Einstellungen → Grafik: `Textur-Paket [16x Klassik | 64x HD | 128x Ultra | Custom PNG]` → `game.set_texture(16|64|128)`; Custom: `<input type=file
  accept=image/png>` → Bild per Canvas in RGBA → `game.set_texture_pack(rgba,w,h)`, in IndexedDB (Fallback localStorage als DataURL) merken,
  beim Start wieder laden; alles nur im Browser.
- Töne (WebAudio, neu): pop, Treffer (dumpf / Krit hell), Schwung, Stoßwelle (tief), Whoosh (Sprungplatte/Teleporter), Fanfare (vorhanden ton 14).
- hilfe.html: Abschnitte Inventar & Truhen, Ausrüstung & Verzaubern, Kampf & Zitadelle, Ackerbau, Funktionsblöcke, Co-op & Handel, Questbuch.
