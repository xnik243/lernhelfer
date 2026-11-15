# Campus Phasen 21–25 – Vertrag für die Bauteile

Ergänzt CONTRACT.md und CONTRACT-16-20.md (gelten weiter). Regelt Schnittstellen; nichts davon umbenennen. Spieltext Deutsch.
Niemand kompiliert lokal (kein Rust auf Windows), ihr arbeitet gleichzeitig. Darum: Signaturen exakt so, keine neuen
Abhängigkeiten (Cargo.toml nicht anfassen, auch keine Features), kein `unsafe`, kein `unwrap` auf Eingaben von außen, kein git commit.
Das Gerüst kompiliert bereits (`cargo check -p campus-shared -p campus-server --all-targets` sauber): jede Server-Datei unten
existiert als Stumpf mit den endgültigen `pub`-Signaturen – Rümpfe füllen, Signaturen lassen. Eigene Hilfsfunktionen/Felder frei.

## 0. Besitz – nur eigene Dateien ändern
| Agent | Stufe | Dateien (nur diese!) |
|---|---|---|
| S-ARCH | fable | `crates/server/src/architect.rs` |
| S-DUEL | fable | `crates/server/src/duel.rs` |
| S-KI | fable | `crates/server/src/duel_ai.rs`, `web/admin.html`, `web/admin.js` |
| S-KATA | fable | `crates/server/src/catacombs.rs`, `crates/server/src/catacombs_gen.rs` |
| C-MAIN | fable | `crates/client/src/lib.rs`, `crates/client/src/deco.rs`, neu `crates/client/src/kino.rs` |
| C-GFX | fable | `crates/client/src/gfx.rs`, `crates/client/src/shaders/*.wgsl` |
| W-HUD | fable | `web/hud.js`, `web/index.html`, `web/hud.css`, `web/hilfe.html` |
| W-P21 | fable | neu `web/musik.js`, neu `web/foto.js` |
| W-SPIEL | fable | neu `web/duell.js`, neu `web/katakomben.js`, neu `web/architekt.js`, `web/inventar.js` (nur Stufe VI, §W-SPIEL) |
| H-RAEUME | haiku | `crates/server/src/dungeon_rooms_data.rs` |
| H-FRAGEN | haiku | `crates/server/src/duel_fallback_data.rs` |
| H-WEB | haiku | neu `web/musik_daten.js`, neu `web/p21.css` |

Fertig (Architekt, nicht ändern): `crates/shared/**`, `crates/server/src/instance.rs`, `testkit.rs`, alle Einhängepunkte in
`game.rs`, `sky.rs`, `econ.rs`, `http.rs`, `lib.rs`, `island.rs`, `mobs.rs`. Lies `instance.rs` und deinen Stumpf, bevor du baust.

Abweichungen von der Vorgabe (begründet): Takt ist 100 ms (10 Hz), nicht 50. Musik liegt in `web/musik.js` statt
`crates/client/src/music.rs` (der Browser-Kern hat keine WebAudio-Features, Ton lebt schon in hud.js). OpenRouter über
`std::process::Command("curl")` in `spawn_blocking` (tokio hat kein `process`-Feature). Einstellungen in Tabelle `server_config`.
Wetter-Partikel ohne Vertex-Puffer (Instanz-Index im Shader) statt vorbelegtem Puffer – null CPU-Arbeit je Bild.
Kugel/Zylinder sitzen am zuletzt mit dem Stab angeklickten Block (der Server kennt keine Blick-Neigung).

## 1. Geteilt (crates/shared) – fertig
- **Blöcke 123–145** (alle unzerstörbar, nie im Laden): 123 KATA_PORTAL, 124 ARENA_PORTAL, 125–128 QUAD_A/B/C/D (blau, gold, smaragd,
  purpur; `world::QUADS`), 129 QUAD_FALSCH (rot), 130 QUAD_RICHTIG (grün), 131 PRISMA_A („/“), 132 PRISMA_B („\“), 133 STRAHL
  (nicht fest, glüht), 134 STRAHLER, 135 EMPFAENGER, 136 EMPFAENGER_AN, 137 RUNE, 138 RUNE_AN, 139 STATUE, 140 WEISEN_TRUHE,
  141 BEUTE_ALTAR, 142 SMARAGD_TRUHE, 143 OBSIDIAN_TRUHE, 144 KATA_TOR (Gitter), 145 BLUT_PORTAL. `world::USE_NEW` = die mit [E]
  benutzbaren (123, 124, 131, 132, 139–143, 145). `BlockDef.key` = Konstantenname. `KATA_PORTAL_AT = [245,31,104]` (Zitadelle, 2 hoch),
  `ARENA_PORTAL_AT = [249,33,296]` (Hub-Spawn, 2 hoch).
- **instance.rs**: `KIND_DUNGEON 0`, `KIND_ARENA 1`, `SIZE 48`, `SLOTS 6` je Art, `origin(kind, slot) -> (x, z)` (durch 16 teilbar,
  Katakomben x 336..480 / z 0..96, Arena z 96..192), `kind_at`, `slot_at`, `ZONE = ["Prüfungs-Katakomben", "Lern-Duell-Arena"]`
  (`world::zone_name` liefert das dort), `DUNGEON_Y 8` (Boden; Füße auf 9), `DUNGEON_H 24`, `ARENA_Y 40` (Plattform-Oberkante,
  Füße auf 41), `ARENA_FALL_Y 30.0`.
- **worldedit.rs**: `MAX_BLOCKS 32768`, `UNDO_MAX 10`, `Cell = [i32;3]`, `Region::new(a,b)`/`dims`/`volume`/`contains`/`cells()`,
  `in_world(c)`, `protected(block)`, `walls(r)`, `sphere(c, r)` (r 1..=20), `cyl(c, r, h)`, `Clip { size, off, b }` mit
  `Clip::copy(region, stand, get)`, `rotated()` (90° um Y), `cells(at)`, `rust_blueprint(name, &clip) -> String`.
- **npc.rs**: `CUSTOM_NPC_BASE 200` (C2S::Talk(200 + Nummer)), `CUSTOM_NPC_MAX 50`, `NAME_MAX 24`, `TEXT_MAX 160`,
  `NPC_PRESETS: [(&str, Look); 8]`, `preset(i) -> Look`, `NpcAction::{None, Warp(u8), Taler(u32), Item(u16,u32)}`, `parse_action(&str)`.
- **proto.rs (angehängt)**: `C2S::Arch { op: u8, x: i16, y: i16, z: i16 }` (op 1 = Pos1, 2 = Pos2), `S2C::Npcs(Vec<NpcState>)`,
  `NpcState { id: u8 /*200+n*/, p: [f32;3], yaw: f32, look: u8, name: String }`. Alles andere läuft über `S2C::Hud` (JSON, §2).
- **rpg.rs / Daten**: Wesen 5 „Katakomben-Wächter“, 6 „Schatten-Gelehrter“, 7 „Der Namenlose Prüfer“ (Endboss; `ring` 4 = nur
  Katakomben). Gegenstände `SCHATTEN_LATERNE` (2048) und `RUNEN_HERZ` (2049) = Katakomben-Artefakte (Talismane; Beute oder aus
  Meister-Kernen geschmiedet – Rezepte stehen in EQUIP_RECIPES), `MEISTER_KERN` 2420, `FOLIANT_VI` 2421. `enchanting::TIER6 = 6`: Stufe VI gibt es nur über den Folianten (island.rs `enchant` ist fertig:
  bei Stufe 5 + Foliant → 6, sonst 409 wie bisher).

## 2. Server – Gerüst (fertig) und gemeinsame Regeln
`Player` neu: `admin: bool`, `inst: Option<u32>`, `inst_back: [f32;3]`. `Game` neu: `instances`, `duels: HashMap<u32, duel::Duel>`,
`active_dungeons: HashMap<u32, catacombs::Dungeon>`, `arch: architect::Arch`, `event_until: i64`, `jobs_tx`. Tabellen (angelegt):
`server_config(k,v)`, `custom_blueprints(name pk, w, h, d, data blob, by_user, ts)`, `custom_npcs(id pk, x, y, z, yaw, name, look, text, action)`,
`npc_rewards(npc, user, day)`, `duel_question_cache(id, topic, stufe, q, o0..o3, correct, explanation, used, ts, unique(topic,stufe,q))`,
`duel_elo(user pk, elo default 1000, siege, spiele)`, `dungeon_best(user, stufe, note, ms, ts)`.

**instance.rs (fertig, benutzen):**
```rust
pub struct Inst { pub id: u32, pub kind: u8, pub slot: u8, pub members: Vec<u32>, /* Gitter privat */ }
impl Inst { pub fn origin(&self) -> (i32, i32); pub fn get(&self, x: i32, y: i32, z: i32) -> u8;   // Weltkoordinaten
            pub fn put(&mut self, x: i32, y: i32, z: i32, b: u8);                                   // still, für den Aufbau
            pub fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, b: u8); }
impl Game {
  pub fn inst_new(&mut self, kind: u8) -> R<u32>;                 // 409 wenn alle 6 Felder belegt
  pub fn inst(&self, inst: u32) -> Option<&Inst>;  pub fn inst_mut(&mut self, inst: u32) -> Option<&mut Inst>;
  pub fn inst_of(&self, id: u32) -> Option<(u32, u8)>;            // (Instanz, Art) des Spielers
  pub fn inst_enter(&mut self, inst: u32, id: u32, at: [f32; 3], yaw: f32) -> R<()>;  // ≤ 4, schickt Gitter, dann Spawn
  pub fn inst_goto(&mut self, id: u32, at: [f32; 3], yaw: f32);   // Versetzen in der Instanz (S2C::Spawn)
  pub fn inst_set(&mut self, inst: u32, x: i32, y: i32, z: i32, b: u8);  // ein Block + S2C::Block an Mitglieder
  pub fn inst_sync(&mut self, inst: u32);                         // nach vielen put/fill: 9 Stücke neu an alle
  pub fn inst_send(&mut self, inst: u32, m: &S2C);  pub fn inst_hud(&mut self, inst: u32, v: Value);
  pub fn inst_leave(&mut self, id: u32) -> bool;                  // siehe unten
  pub fn inst_close(&mut self, inst: u32);                        // alle raus, alles verwerfen
  pub fn admin_only(&self, id: u32) -> R<()>;  pub fn taler_mult(&self) -> i64;   // 2 während des Events
  pub fn cfg(&self, k: &str) -> Option<String>;  pub fn cfg_set(&self, k: &str, v: &str) -> R<()>;   // server_config
}
```
- `inst_leave` (auch bei `/leave`, POST `instanz/verlassen`, jedem Warp, Trennung) ruft zuerst `duel_left`/`dungeon_left`, schickt die
  echten Stücke, setzt den Spieler an `inst_back`, sendet Hud `{"t":"duell"|"katakomben","aus":true}` und verwirft bei der letzten
  Person Instanz + `duels`/`active_dungeons`-Eintrag. In `*_left` nie `inst_leave`/`inst_close` für dieselbe Instanz aufrufen.
- Mitglieder einer Instanz: Bewegung ruft `duel_move`/`dungeon_move` statt `on_move`; kein Leere-Verlust (Absturz selbst prüfen:
  Arena `p[1] < ARENA_FALL_Y`, Katakomben `p[1] < DUNGEON_Y - 3` → `inst_goto`). [E] (C2S::Chest) ruft `dungeon_use` mit dem Block
  aus dem Gitter. `C2S::Attack`/`Shoot` gehen an `dungeon_attack`/`dungeon_shoot`. `hurt_player` funktioniert; bei Leben ≤ 0 werden
  Leben/Schild zurückgesetzt und `duel_died`/`dungeon_died` gerufen. `self.block()` liest die echte Welt (dort Luft) – in Instanzen
  immer `self.inst(n).get(..)`.
- Takt: `duel_tick(now)`/`dungeon_tick(now)` laufen nur, wenn die jeweilige Karte nicht leer ist. Zeiten nur aus `now`
  (Tests schieben `g.clock_offset` und rufen `g.duel_tick(g.now())`).
- Fehler `no(code, text)`; mehrere Tabellen in `self.tx(|g| …)`; Gegenstände nur über `give`/`gain` + `S2C::Inv`; Taler über
  `pay(uid, n * self.taler_mult(), grund)`. Wiederverwendbar: `self.loot(uid, kind, at)` (mobs.rs, jetzt `pub(crate)`),
  `set_raw`/`resend(ci)`/`push`/`crate::game::ci` (`pub(crate)`), `sky::clean`, `fuel(uid, ms)`, `levels(uid)`, `add_xp`.
- **Tests**: als `#[cfg(test)] mod tests` unten in der eigenen Datei, mit `crate::testkit::{game, join, user, put, msgs, huds, says}`
  (`join(&mut g, id, name, admin) -> Receiver`). Keine neuen Dateien in `tests/`, kein Netz, nie Port 8090, kein echter OpenRouter-Aufruf.
  `g.say()` hat 1,5 s Abklingzeit für Nicht-Verwalter (`g.clock_offset += 2000` dazwischen).

**HTTP (instance.rs `p21_get`/`p21_post`, fertig verdrahtet; Verwaltungs-Ops liefern für alle anderen 403):**
GET `duell/rangliste`, `katakomben`, `admin/ki`, `admin/architekt`, `admin/blueprint/<name>`. POST `instanz/verlassen`, `duell/wahl`,
`duell/antwort`, `katakomben/start {stufe}`, `katakomben/beitreten {name}`, `admin/ki`, `admin/ki_test {thema?, stufe?}` (http.rs,
wartet ohne Sperre), `admin/event`, `admin/wetter`, `admin/architekt`, `admin/npc`.
**Befehle (sky.rs, fertig verdrahtet):** `/architect …` → `arch_cmd`, `//wort rest` → `arch_edit(id, "wort", rest)` (beide nur
Verwaltung, sonst System-Zeile „Nur für die Verwaltung.“), `/duel …` → `duel_cmd`, `/dungeon …` → `dungeon_cmd`, `/leave`.

**Hud-Nachrichten Server → Oberfläche** (`self.hud(id, json!({...}))`; Felder exakt so):
| `t` | Felder | Sender |
|---|---|---|
| `wetter` | `art` 0 Klar, 1 Sommerregen, 2 Schneefall, 3 Kirschblüten | S-ARCH |
| `event` | `an` bool, `text` „2× Taler-Event“, `bis` ms | S-ARCH |
| `architekt` | `an`, `pos1`/`pos2` `[x,y,z]`\|null, `masse` `[x,y,z]`\|null, `bloecke` n, `ablage` `[x,y,z]`\|null, `undo` n, `redo` n, `text` \|null | S-ARCH |
| `architekt_export` | `name` | S-ARCH |
| `architekt_npc` | `npcs` `[{id,name,look,text,aktion,x,y,z}]`, `vorlagen` `[Name ×8]` | S-ARCH |
| `npc` | `id`, `name`, `text`, `aktion` `""\|"warp"\|"taler"\|"item"`, `belohnt` bool | S-ARCH |
| `duell` | `phase` + Felder je Phase (§S-DUEL) oder `aus: true` | S-DUEL / Gerüst |
| `duell_einladung` | `von` Name | S-DUEL |
| `katakomben` | Lauf-Stand, `portal: true`, `ende: true` (§S-KATA) oder `aus: true` | S-KATA / Gerüst |
| `weise` | `nr` 0..2, `text` | S-KATA |
| `boss` | wie bisher (`name`,`hp`,`max`,`phase` \| `aus:true,name`; `weg:true` = nur Leiste aus, kein Sieg) | S-KATA |

### §S-ARCH (architect.rs)
```rust
pub const WETTER: [&str; 4];  #[derive(Default)] pub struct Arch { /* frei */ }  pub fn weather_at(now: i64) -> u8;
impl Game { pub fn arch_join(&mut self, id: u32);  pub fn arch_left(&mut self, id: u32);
  pub fn arch_cmd(&mut self, id: u32, rest: &str) -> R<()>;  pub fn arch_edit(&mut self, id: u32, word: &str, rest: &str) -> R<()>;
  pub fn arch_click(&mut self, id: u32, op: u8, x: i32, y: i32, z: i32) -> R<()>;
  pub fn arch_get(&mut self) -> R<Value>;  pub fn arch_blueprint(&mut self, name: &str) -> R<Value>;
  pub fn arch_post(&mut self, id: u32, b: &Value) -> R<Value>;  pub fn npc_post(&mut self, id: u32, b: &Value) -> R<Value>;
  pub fn npc_talk(&mut self, id: u32, npc: u8) -> R<()>;  pub fn event_post(&mut self, b: &Value) -> R<Value>;
  pub fn weather_post(&mut self, b: &Value) -> R<Value>;  pub fn weather_tick(&mut self); }
```
- Die Aufrufer haben die Verwaltung schon geprüft (403); `arch_*`/`npc_post`/`event_post`/`weather_post` selbst prüfen nicht noch einmal.
- `/architect` schaltet den Modus um (`/architect aus` beendet), `/architect npc` → Hud `architekt_npc`, `/architect wetter
  <auto|klar|regen|schnee|blueten>`, `/architect event <an [Minuten]|aus>`. Nach jeder Änderung Hud `architekt` an den Verwalter.
  POST `admin/architekt {an}` schaltet den Modus des (verbundenen) Verwalters.
- `arch_click`: op 1/2 setzt Pos1/Pos2 (nur im Modus, Zelle `in_world`), merkt „zuletzt geklickt“.
- `//`-Befehle (word klein): `pos1`, `pos2` (Zelle der Füße), `desel`, `set <b>`, `walls <b>`, `replace <von> <nach>`,
  `sphere <b> <r>`, `cyl <b> <r> <h>` (Mitte = zuletzt geklickter Block, sonst Füße), `copy`, `rotate`, `paste` (am Standpunkt),
  `undo`, `redo`, `export <name>`. Block = Nummer (0 = Luft, sonst `world::block(b).is_some()`) oder Konstantenname (`BlockDef.key`,
  Groß/Klein egal). Antwort als System-Zeile („1 204 Blöcke gesetzt.“) + Hud `architekt`.
- Jede Änderung: Zellen sammeln; verwerfen, was nicht `worldedit::in_world`, `world::anchor(x,y,z)`, `worldedit::protected(alt)`
  oder `protected(neu)` ist oder in einem Instanz-Feld liegt (`instance::kind_at(x,z).is_some()`); mehr als `MAX_BLOCKS` Zellen →
  `no(413, …)` ohne Änderung. Schreiben: bis 200 geänderte Zellen `set_block(x,y,z,b,0)`, darüber `set_raw` je Zelle und danach
  `resend(ci(x>>4, z>>4))` je berührtem Stück (sonst läuft der Sende-Kanal über). Rückgängig-Ring je Verwalter: höchstens
  `UNDO_MAX` Schritte aus `(Zelle, alt, neu)` nur wirklich geänderter Zellen; neuer Schritt leert `redo`.
- `//export <name>` (Name `[a-z0-9_-]{1,32}`): Auswahl als `Clip` → `custom_blueprints` (data = `world::rle(&clip.b)`), Hud
  `architekt_export`. GET `admin/blueprint/<name>` → `{"name","w","h","d","rust": worldedit::rust_blueprint(..)}`.
  GET `admin/architekt` → `{"blueprints":[{name,w,h,d,ts}],"npcs":[…wie architekt_npc],"event":{"an","bis"},"wetter":{"art","fest":bool},"vorlagen":[…]}`.
- NPCs: POST `admin/npc {id?, name, look 0..7, text, aktion, hier?: bool, loeschen?: bool}` – ohne `id` neu an Stelle/Blick des
  Verwalters (höchstens `CUSTOM_NPC_MAX`, Nummern 0..49), mit `id` ändern (`hier` = an die jetzige Stelle versetzen) oder löschen.
  `name`/`text` durch `sky::clean`, Längen `NAME_MAX`/`TEXT_MAX`, `aktion` muss `npc::parse_action` bestehen. Danach
  `broadcast(S2C::Npcs(alle))`. `npc_talk(id, 200+n)`: höchstens 8 Blöcke entfernt, Hud `npc`; Warp → `self.warp(id, Warp::Spot(i))`,
  Taler/Gegenstand je Konto und NPC einmal am Tag (`npc_rewards`, `self.today()`).
- Event: POST `admin/event {an, minuten? (1..=1440, sonst 60)}` setzt `self.event_until` + `kv_set("event_2x_until")`, `hud_all` `event`.
  Wetter: `weather_at(now)` wechselt alle 10 min deterministisch (etwa 55 % Klar, 20 % Regen, 13 % Schnee, 12 % Blüten);
  POST `admin/wetter {art: -1 (auto) | 0..3}` legt fest (kv `wetter_fest`). `weather_tick` (alle 5 s): bei Wechsel `hud_all` `wetter`,
  abgelaufenes Event beenden (`hud_all` `event` mit `an:false`). `arch_join`: Hud `wetter`, laufendes `event`, `S2C::Npcs` (wenn nicht leer).
- **Tests**: Nicht-Verwalter: `g.handle(2, C2S::Say{ch:0,text:"//set 5".into()})`, `/architect`, `C2S::Arch` → nichts ändert sich, Antwort
  „Nur für die Verwaltung.“, POST `admin/event`/`admin/npc` → `Err(E(403, _))`. Verwalter: gemischte Blöcke merken, `//set` → alle gleich,
  `//undo` → exakt die alten Nummern, `//redo`; 11 Schritte → nur 10 rückholbar; 20×20×20 ohne Rauswurf, > 32 768 abgelehnt;
  geschützte Blöcke und Instanz-Felder unberührt; copy → rotate → paste; export legt Zeile an und liefert Rust-Text mit `fn name(`;
  NPC setzen/sprechen/Belohnung nur einmal am Tag; Event verdoppelt `taler_mult`; `weather_at` deterministisch, alle 4 Lagen kommen vor.

### §S-DUEL (duel.rs)
```rust
pub struct Duel { pub inst: u32, /* frei */ }
#[derive(Clone, Debug, PartialEq)] pub struct Topic { pub fach: u8 /*0..3, 255 = freies Thema*/, pub thema: String, pub stufe: u8 /*0..2*/ }
pub fn resolve_votes(votes: &[Topic], auto: u8, seed: u64) -> (Topic, Vec<Topic>);  // (Sieger, Stichwahl-Kandidaten; leer = klare Mehrheit)
pub fn auto_fach(levels: &[[u32; 7]]) -> u8;        // gemeinsames schwächstes Fach 0..3 (kleinste Stufen-Summe, bei Gleichstand kleinster Index)
impl Game { pub fn duel_cmd(&mut self, id: u32, rest: &str) -> R<()>;  pub fn duel_tick(&mut self, now: i64);
  pub fn duel_move(&mut self, id: u32, now: i64);  pub fn duel_left(&mut self, inst: u32, id: u32);  pub fn duel_died(&mut self, id: u32, now: i64);
  pub fn duel_vote(&mut self, id: u32, b: &Value) -> R<Value>;  pub fn duel_answer(&mut self, id: u32, b: &Value) -> R<Value>;
  pub fn duel_ranking(&mut self, uid: u32) -> R<Value>;  pub fn duel_questions(&mut self, duel: u32, qs: Vec<Question>); }
```
- Einstieg: `/duel` (oder [E] am Arena-Portal) tritt einer offenen Lobby bei (Phase `lobby`, < 4) oder eröffnet eine
  (`inst_new(KIND_ARENA)`, Arena bauen, `inst_enter`); `/duel solo` eigene Runde gegen den KI-Bot; `/duel <Name>` eröffnet und
  schickt dem Ziel Hud `duell_einladung`; `/duel join <Name>` tritt dessen Lobby bei; `/duel leave` = `inst_leave`. Bleibt bis zum
  Ende der Lobby nur ein Mensch, spielt ein Bot mit („Professor Kalkulus“, antwortet je Stufe mit 70/60/50 % richtig).
- Arena (im Gitter, Oberkante `ARENA_Y`): je Spieler eine 8×8-Plattform aus vier 4×4-Feldern `QUADS` (A NW, B NO, C SW, D SO),
  Rand `GLAS`/`NEON_CYAN`, alle mit Blick auf eine große Anzeigetafel aus Blöcken (≥ 12 breit, ≥ 10 hoch: vier Punktesäulen in
  Spielerfarbe, die mit jedem Punkt wachsen; bei der Stichwahl laufen die Lampen 1,5 s um). Fragetext steht im Hud (kein 3D-Text).
- Phasen (alle Zeiten aus `now`): `lobby` 10 s (Beitritt setzt den Rest auf mindestens 6 s) → bei Gleichstand `roulette` 1,5 s →
  `laden` (nur wenn `questions_request` true liefert; endet mit `duel_questions` oder nach `duel_ai::TIMEOUT_MS`) → 8 × (`frage` 12 s
  → `aufloesung` 3,6 s) → `ende` 8 s → `inst_close`. Fragen: `if !self.questions_request(inst, &thema, stufe) { qs = self.questions_now(..) }`;
  `duel_questions`: weniger als 8 → mit `questions_now` auffüllen. Start nie später als Sperre + 6,5 s (LAW 1).
- Themenwahl: POST `duell/wahl {art: "auto"|"fach"|"frei", fach?: 0..3, thema?: ≤ 60 Zeichen, stufe: 0..2}`; freies Thema durch
  `duel_ai::normalize_topic`; leer → 400. `auto` = `auto_fach` aus `levels(uid)` aller Teilnehmer. `resolve_votes`: meiste Stimmen
  je (fach, thema) gewinnt sofort; Gleichstand → Kandidaten sortiert, Sieger = `kandidaten[seed % n]`; keine Stimme → Auto-Fach,
  Stufe 1; Stufe = häufigste Stufe der Sieger-Stimmen (Gleichstand: kleinere).
- Antwort = Feld unter den Füßen bei 0 s, oder POST `duell/antwort {feld: 0..3}` (Touch/barrierefrei; zählt wie Draufstehen).
  Auflösung: die 3 falschen Felder aller Plattformen `QUAD_FALSCH`, nach 0,6 s Luft (3 s lang), richtiges `QUAD_RICHTIG`; wer falsch
  stand: `hurt_player(id, 20.0, now)`; wer unter `ARENA_FALL_Y` fällt, kommt per `inst_goto` auf seine Plattform zurück (kein
  Taler-Verlust). Punkte: richtig 100 + bis 50 Tempo, Serie +10 je Folge-Treffer; schnellste richtige Antwort +25 und
  `S2C::Shot { kind: 2 }` (Schneeball, harmlos) auf jeden Gegner.
- Ende (in `tx`): Teilnehmer 10 + 3 je richtiger Antwort, Sieger +20 Taler (× `taler_mult`, Grund „duell“); Wissens-Splitter
  `SHARD + fach` (freies Thema: `SHARD + 4`) = richtige/2, Sieger +2 (`gain` + `S2C::Inv`); `fuel` 10 min, Sieger 30 min; Solo: halbe
  Taler, keine Wertung. Wertung (ELO, K = 24, nur ≥ 2 Menschen) in `duel_elo`. GET `duell/rangliste` →
  `{"spieler":[{"name","elo","siege","spiele"} ×≤10],"ich":{"elo","siege","spiele"}|null}`.
- Hud `duell` (an alle Mitglieder bei jedem Phasenwechsel, in `lobby` bei jeder Stimme, sonst höchstens 1×/s):
  `spieler` = `[{"id","name","punkte","serie","bot":bool,"farbe":0..3}]`, immer dabei; dazu je `phase`:
  `lobby`: `rest_ms`, `auto: {"fach","name"}`, `stimmen: [{"id","art","fach","thema","stufe"}]` ·
  `roulette`: `rest_ms`, `kandidaten: [Text]`, `sieger: Text` · `laden`: `thema`, `stufe`, `rest_ms` ·
  `frage`: `runde` 1..8, `von` 8, `frage`, `antworten: [4]`, `rest_ms`, `thema` · `aufloesung`: `runde`, `richtig` 0..3, `erklaerung`,
  `rest_ms`, `schnellster` id\|0 · `ende`: `sieger` id, `belohnung: {"taler","splitter","item"}`, `elo: {"alt","neu"}|null`.
- **Tests**: `resolve_votes` Mehrheit sofort; Gleichstand: gleicher `seed` → gleiches Ergebnis, verschiedene Seeds erreichen jeden
  Kandidaten; `auto_fach`; Solo-Runde komplett durchgetaktet ohne Schlüssel und ohne tokio-Laufzeit (kein Blockieren, Fragen aus
  der Bank): Plattform-Felder liegen unter dem Spieler, richtig stehen gibt Punkte, nach `ende` sind `g.duels` und
  `g.instances.list` leer und Taler gestiegen; Absturz kostet keine Taler; Trennung mitten in der Runde räumt alles ab.

### §S-KI (duel_ai.rs, admin.html, admin.js)
```rust
pub const DEFAULT_MODEL: &str;  pub const DEFAULT_URL: &str;  pub const TIMEOUT_MS: u64 = 6500;  pub const ROUNDS: usize = 8;
pub struct Question { pub q: String, pub options: [String; 4], pub correct: u8, pub explanation: String }
pub struct KiCfg { pub key: String, pub model: String, pub url: String }
pub enum Job { Fragen { duel: u32, thema: String, stufe: u8, res: Result<Vec<Question>, String> } }
pub fn normalize_topic(s: &str) -> String;                       // fertig
pub fn parse_questions(json: &str) -> Result<Vec<Question>, String>;
pub fn fallback(fach: u8, stufe: u8, n: usize, seed: u64) -> Vec<Question>;      // fach 255 = gemischt
pub async fn fetch(cfg: KiCfg, thema: String, stufe: u8) -> Result<Vec<Question>, String>;
impl Game { pub fn ki_cfg(&self) -> KiCfg;  pub fn cache_put(&self, thema: &str, stufe: u8, qs: &[Question]) -> R<usize>;
  pub fn cache_take(&self, thema: &str, stufe: u8, n: usize) -> R<Vec<Question>>;
  pub fn questions_request(&mut self, duel: u32, thema: &str, stufe: u8) -> bool;
  pub fn questions_now(&mut self, thema: &str, fach: u8, stufe: u8) -> Vec<Question>;   // genau ROUNDS, blockiert nie
  pub fn job_done(&mut self, j: Job);  pub fn ki_get(&mut self) -> R<Value>;  pub fn ki_post(&mut self, b: &Value) -> R<Value>;
  pub fn ki_test_done(&mut self, thema: &str, stufe: u8, res: Result<Vec<Question>, String>) -> R<Value>; }
```
- `ki_cfg`: Schlüssel `cfg("openrouter_key")`, sonst Umgebung `OPENROUTER_API_KEY`; Modell `cfg("openrouter_model")`, sonst
  `OPENROUTER_MODEL`, sonst `DEFAULT_MODEL` (nur `[A-Za-z0-9._/:-]`, 1..=80, sonst Standard); `url` = `cfg("openrouter_url")` oder `DEFAULT_URL`.
- `fetch`: `tokio::time::timeout(TIMEOUT_MS, tokio::task::spawn_blocking(..))`; darin `std::process::Command::new("curl")` mit
  `["-sS", "--max-time", "6", "-K", "-"]`, die curl-Konfiguration (url, `header = "Authorization: Bearer …"`, Content-Type, `data`)
  geht über stdin – der Schlüssel steht nie in argv, nie in Fehlertexten, nie in Antworten. Schlüssel mit Anführungszeichen,
  Backslash oder Steuerzeichen ablehnen. Anfrage: EIN Aufruf für `ROUNDS` Fragen, `response_format {"type":"json_object"}`, deutsche
  Anweisung mit Thema, Stufe („Klasse 5–7“, „Klasse 8–10“, „Oberstufe / Uni“) und dem Schema aus der Vorgabe. Antwort:
  `choices[0].message.content` (etwaige ```-Zäune entfernen) → `parse_questions`.
- `parse_questions`: `{"questions":[{q, options[4], correct, explanation}]}`; Frage gültig nur mit genau 4 nicht-leeren Optionen
  und `correct` 0..=3 (Zahl); ungültige Einträge fallen weg, keine gültige → `Err`. Alles durch `sky::clean`; „A) “-Vorsätze entfernen;
  Längen kappen (q 110, Option 60, Erklärung 90 Zeichen).
- `questions_request`: false, wenn der Cache ≥ 8 unbenutzte (`used = 0`) Fragen für (thema, stufe) hat, der Schlüssel leer ist oder
  `tokio::runtime::Handle::try_current()` fehlschlägt; sonst `handle.spawn(async move { tx.send(Job::Fragen{..}) })` mit einer Kopie
  von `self.jobs_tx` und true. `job_done`: Ok → `cache_put`, dann `self.duel_questions(duel, qs)`; Err → `duel_questions(duel, vec![])`.
- `cache_take`: am seltensten benutzte zuerst, zählt `used` hoch. `questions_now`: Cache, Rest aus `fallback(fach, stufe, ..)`
  (zu kleine Bank: andere Stufen/Fächer, notfalls Wiederholung). `fallback` wählt deterministisch je `seed` ohne Doppelte, solange es geht.
- GET `admin/ki` → `{"key_gesetzt":bool,"key_maske":"…abcd" (nur die letzten 4 Zeichen),"quelle":"verwaltung"|"umgebung"|"keine",
  "modell","modelle":[≥ 4 Vorschläge, DEFAULT_MODEL zuerst],"cache": n}`. POST `admin/ki {key?: Text (leer = löschen), modell?}` →
  wie GET. `ki_test_done`: Ok → Cache füllen, `{"ok":true,"anzahl":n,"frage":{"q","antworten":[4],"richtig":i,"erklaerung"},"cache":n}`;
  Err(e) → `{"ok":false,"fehler":e,"cache":n}` (HTTP 200).
- **admin.html/admin.js**: Karte „OpenRouter KI-Konfiguration“ (`<input type=password autocomplete=off>` außerhalb der neu
  gezeichneten Bereiche, Modell-Auswahl aus `modelle` + freies Feld, [Speichern], [Test-Frage generieren] → POST `admin/ki_test`,
  zeigt Frage oder Fehler in `<p role=status>`, Zähler „n Fragen im Cache“); Karte „Events & Welt“: [2× Taler Event an/aus] →
  POST `admin/event {an}`, Wetter-Auswahl → POST `admin/wetter {art}`, [Architekten-Modus] → POST `admin/architekt {an}`, Liste der
  Baupläne (GET `admin/architekt`) mit [Als Rust-Blueprint kopieren] (GET `admin/blueprint/<name>` → `navigator.clipboard.writeText`).
- **Tests**: `parse_questions` gültig; 3 Optionen, `correct: 4`, `correct: "1"`, fehlendes Feld → verworfen bzw. `Err`; `<script>` wird
  entschärft; `normalize_topic`; Cache: 8 ablegen, 8 holen, Doppelte nicht doppelt, im Mittel < 1 ms je Zugriff (100 Wiederholungen);
  ohne Schlüssel und ohne Laufzeit: `questions_request` false, `questions_now` liefert sofort 8; Bank hat je (fach, stufe) ≥ 8;
  `ki_get` enthält den Schlüssel nie im Klartext; die curl-Konfiguration als reine Funktion (Schlüssel nur dort);
  `#[tokio::test]` `fetch` gegen `http://127.0.0.1:9/` endet mit `Err` vor `TIMEOUT_MS`.

### §S-KATA (catacombs.rs, catacombs_gen.rs)
```rust
pub struct Dungeon { pub inst: u32, /* frei */ }
impl Game { pub fn dungeon_cmd(&mut self, id: u32, rest: &str) -> R<()>;  pub fn dungeon_portal(&mut self, id: u32) -> R<()>;
  pub fn dungeon_start(&mut self, id: u32, stufe: u8) -> R<u32>;  pub fn dungeon_join(&mut self, id: u32, name: &str) -> R<()>;
  pub fn dungeon_tick(&mut self, now: i64);  pub fn dungeon_move(&mut self, id: u32, now: i64);
  pub fn dungeon_use(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8) -> R<()>;
  pub fn dungeon_attack(&mut self, id: u32, mob: u32, now: i64) -> R<()>;  pub fn dungeon_shoot(&mut self, id: u32, dir: [f32; 3], now: i64) -> R<()>;
  pub fn dungeon_left(&mut self, inst: u32, id: u32);  pub fn dungeon_died(&mut self, id: u32, now: i64);  pub fn dungeon_get(&mut self, uid: u32) -> R<Value>; }
```
- **catacombs_gen.rs** (ohne `Game`, alles prüfbar; Signaturen dort frei, aber `pub`): `generate(seed, stufe) -> Layout` (3×3 Räume je
  16×16 Zellen, Wände am Rand, Innenraum 14×14 aus `dungeon_rooms_data::ROOMS`; Arten: 1 Start-Halle, 3 Kampf-Kammern, Spiegel-Kammer,
  Runen-Kammer, Halle der drei Weisen, versteckte Schatz-Kammer (nur über die Runen-Kammer erreichbar), Endboss-Saal), Türen 2 breit
  als `KATA_TOR`, `connected(&Layout) -> bool`, `build(&Layout, put)` schreibt lokale Zellen (x, z 0..48, y 0..24), Spiegel-Strahl
  (`beam`), Runen-Folge, Weisen-Rätsel (≥ 4 Sätze-Sets, genau eine Truhe ist richtig), `score`, `grade`, `loot`.
  Welt = (origin.x + lx, `DUNGEON_Y` + ly, origin.z + lz); Füße auf `DUNGEON_Y + 1`.
- Lauf: `/dungeon start [1-5]` oder Portal (Hud `katakomben {portal:true, laeufe:[{name,stufe,n}]}` → Oberfläche POSTet
  `katakomben/start`/`beitreten`); Beitritt nur, solange die Gruppe die Start-Halle nicht verlassen hat. Kampf-Kammer: Betreten
  schließt die Türen, Wellen aus Wesen 5/6 (Leben/Schaden × (1 + 0,35·(Stufe−1))), leer → Türen auf. Nach 3 Kampf-Kammern öffnet das
  `BLUT_PORTAL` ([E]) den Endboss-Saal (Wesen 7, Hud `boss`). Tod: `tode += 1`, zurück in die Start-Halle. Lauf endet 60 s nach dem
  Boss, nach 30 min oder wenn alle gehen (`inst_close`).
- Wesen: eigene Liste im `Dungeon`, Nummern ≥ 1 000 000, Kollision über `inst.get`, 10 Hz `S2C::Mobs` nur an Mitglieder
  (`MobState`, Bits wie Zitadelle); Treffer `combat::player_hit`, Beute `self.loot(uid, kind, at)`.
- Rätsel: **Spiegel** – `STRAHLER` schickt einen Strahl (als `STRAHL`-Blöcke) durch die Kammer, `PRISMA_A`/`PRISMA_B` lenken 90° um,
  [E] dreht ein Prisma (A↔B), trifft der Strahl `EMPFAENGER` → `EMPFAENGER_AN`, gelöst (lösbar erzeugt, anfangs ungelöst).
  **Runen** – 3×3 `RUNE` im Boden, 5 Schritte blinken (`RUNE_AN`, je 600 ms); in dieser Reihenfolge betreten (`dungeon_move`) öffnet
  die Schatz-Kammer; falscher Schritt = von vorn. **Drei Weise** – [E] auf `STATUE` → Hud `weise`; die richtige `WEISEN_TRUHE` gibt
  den Segen (+25 % Schaden, Tempo: `boost_until` weit in der Zukunft, Hud-Feld `segen`), eine falsche 30 Schaden und das Rätsel ist verloren.
- Wertung: Können = max(0, 100 − 25·Tode); Tempo = 100 bis 5 min, linear auf 0 bei 15 min; Rätsel = 100·gelöst/3. Note: alle drei
  100 → „S+“, Mittel ≥ 90 „S“, ≥ 75 „A“, ≥ 55 „B“, sonst „C“. Nach dem Boss erscheint der `BEUTE_ALTAR` ([E], je Mitglied einmal):
  Taler 60·Stufe (× `taler_mult`, Grund „katakomben“), Wissens-Splitter; ab „S“ dazu die `SMARAGD_TRUHE`, bei „S+“ auch die
  `OBSIDIAN_TRUHE` – nur dort fallen `SCHATTEN_LATERNE`, `RUNEN_HERZ`, `FOLIANT_VI`, `MEISTER_KERN`, `VERZ_BUCH` (Chancen steigen mit der Stufe).
  Bestwert je (Konto, Stufe) in `dungeon_best`. GET `katakomben` → `{"laeufe":[{"name","stufe","n","offen":bool}],"bestzeiten":[{"stufe","note","ms"}]}`.
- Hud `katakomben` (1×/s und bei Änderungen): `{"stufe","raum": Name,"raum_nr" 1..9,"zeit_ms","tode","raetsel" 0..3,"geheim":bool,
  "note","punkte":{"koennen","tempo","raetsel"},"segen":bool,"wesen":n,"text":Hinweis|null}`; am Ende `{"ende":true,"note","punkte",
  "zeit_ms","beute":[[item,n]…],"truhen":["smaragd","obsidian"]}`.
- **Tests**: 200 Seeds: `connected`, genau die neun Raumarten, Schatz-Kammer nur über die Runen-Kammer; alle `ROOMS`-Vorlagen gültig
  (14×14, Türfelder frei, zusammenhängend, Marken je Art); Spiegel lösbar und anfangs ungelöst; Runen-Folge; genau eine wahre Truhe;
  0 Tode + < 5 min + 3 Rätsel = „S+“, ein Rätsel weniger < „S+“; **Leck**: Lauf mit 2 Spielern → `active_dungeons.len() == 1`, einer
  `/leave`, einer `drop_player` → `active_dungeons.len() == 0`, `instances.list` leer, keine Zeile in `chunks` für das Feld; 100×
  `g.tick()` ohne Lauf ändert nichts; Wesen stirbt → Beute; Nummern ≥ 1 000 000.

### §H-RAEUME (dungeon_rooms_data.rs) – reine Daten
Struktur und Zeichen stehen im Stumpf (`RoomTpl { kind, name, rows: [&str; 14] }`, `ROOMS`). Liefere **19 Vorlagen**: 3 × `T_START`,
10 × `T_KAMPF`, 3 × `T_SCHATZ`, 3 × `T_BOSS`, jede mit eigenem deutschen Namen. Regeln (werden getestet): genau 14 Zeilen mit genau
14 ASCII-Zeichen aus `.#oLmstB`; Türfelder frei (`.`): Spalten 6 und 7 der Zeilen 0 und 13, Zeilen 6 und 7 der Spalten 0 und 13;
alle begehbaren Felder (alles außer `#`, `o`, `L`) hängen über Kanten zusammen; `T_START` genau ein `s`, kein `m`; `T_KAMPF` 4–6 `m`,
mindestens 3 Felder von jeder Tür; `T_SCHATZ` genau ein `t`; `T_BOSS` genau ein `B` in den mittleren 6×6 Feldern, 2–4 `m`, höchstens
8 `#`/`o`. Abwechslungsreich (Säulenhallen, Gräben aus `o`, Laternen-Alleen), nie die Mitte der Kampf-Kammern zubauen.

### §H-FRAGEN (duel_fallback_data.rs) – reine Daten
Struktur im Stumpf (`FallbackQ { fach, stufe, q, options: [&str; 4], correct, explanation }`, `FALLBACK`). Liefere **mindestens 120
Fragen**: je (fach 0..3, stufe 0..2) mindestens 10. Deutsch, fachlich richtig, genau eine richtige Antwort, `q` ≤ 110 Zeichen,
Optionen ≤ 40 Zeichen ohne „A) “, `explanation` ein Satz ≤ 90 Zeichen, richtige Stelle gleichmäßig über 0..3 verteilt, keine
Doppelten, keine `"` oder `\` in den Texten (deutsche Anführungszeichen „“ sind erlaubt).

## 3. Browser-Kern (WASM)
### §C-MAIN (lib.rs, deco.rs, neu kino.rs) – neue Exporte, alle bisherigen bleiben
```rust
pub fn set_weather(&mut self, kind: u8)        // 0 Klar, 1 Sommerregen, 2 Schneefall, 3 Kirschblüten; blendet je 1 s aus/ein
pub fn set_photo(&mut self, on: bool)          // F8-Freikamera an/aus
pub fn set_filter(&mut self, filter: u8, focal: f32)   // 0 Standard, 1 Retro-Pixel, 2 Goldene Stunde, 3 Kino-Kontrast; focal Blöcke, 0 = aus
pub fn cutscene(&mut self, id: u8) -> bool     // 0 Hub-Marktplatz, 1 Zitadelle, 2 Katakomben, 3 Arena; false = gerade nicht möglich
pub fn skip_cutscene(&mut self)
pub fn set_architect(&mut self, on: bool)      // Stab aktiv
pub fn set_selection(&mut self, on: bool, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32)
pub fn set_speed(&mut self, f: f32)            // Lauf-Tempo-Faktor, geklemmt 1.0..=1.5 (Katakomben-Segen)
```
- **Kamera** (kino.rs): `catmull(pts, t)` (Catmull-Rom, Enden verdoppelt), Fahrt 4,5 s über ≥ 5 Punkte im Umkreis ≤ 40 Blöcke der
  Figur (geladene Stücke), Blick auf ein Ziel; in `frame` direkt nach der Berechnung von `(eye, look)` überschreiben. Währenddessen:
  eigene Figur zeichnen, Hand nicht, Eingaben ruhen. Start/Ende melden: `js_event("kino", 1.0 | 0.0, id as f64, "")`.
  Freikamera: `input()` bewegt die Kamera (x/z, Springen = hoch, Sprint = schneller), `turn()` dreht sie; Abstand zur Figur ≤ 16 Blöcke
  (klemmen); die Figur bleibt stehen, `C2S::Move` sendet weiter ihre Stelle.
- **Env** (Felder baut C-GFX): je Bild `self.env.wx = [art, staerke 0..1, 0.0, 0.0]` (Stärke 0, wenn Partikel aus) und
  `self.env.post = [filter, focal, 0.0, 0.0]` in `fill_env`.
- **Architekt**: bei `set_architect(true)` sendet `mine(true)` statt Abbau `C2S::Arch { op: 1, .. }` auf den anvisierten Block,
  `act(1)` `op: 2`. Auswahl: 12 leuchtende Kanten (cyan, `avatar::world_box(.., glow = true)`) um den Quader min..=max.
- **Freie NPCs**: `S2C::Npcs(v)` merken; zeichnen wie feste NPCs mit `npc::preset(look)`, Plakette `NPC_BASE + id` (id = 200+n),
  `npc_in_sight`/`prompt_now` erweitern, Ansprechen sendet `C2S::Talk(id)`. Namen an die Oberfläche:
  `js_event("npcs", n, 0.0, "[[200,\"Name\"],…]")`.
- **[E]**: `world::USE_NEW` in `interact()` und `prompt_now` aufnehmen (sendet `C2S::Chest`).
- **Wesen** (deco.rs): Art 5 wie der Wächter (Farben aus `rpg::mob`), 6 schwebender Gelehrter (schmal, Kapuze, glühende Augen),
  7 Endboss (Archon-Gestalt, Maßstab aus der Definition, eigener Lebensbalken wie Art 3). `S2C::Shot` `kind 2` = weißer Schneeball.
- `set_speed`: Faktor auf das Lauf-Tempo in `physics`. Nichts an der Leere-Logik ändern (der Server versetzt in Instanzen selbst).

### §C-GFX (gfx.rs, shaders/*.wgsl)
- `Env`: zwei neue Felder **zwischen `scr` und `lpos`**, in Rust und `common.wgsl` gleich: `pub wx: [f32; 4]` (x Art 0..3, y Stärke 0..1),
  `pub post: [f32; 4]` (x Filter 0..3, y Brennweite in Blöcken, 0 = aus). Null = alles aus.
- **Wetter (LAW 4)**: neue Pipeline `wx` (direkt + HDR), Einsprung `vs_wx`/`fs_wx` in scene.wgsl, **kein Vertex-Puffer**:
  `draw(0..6, 0..2048)`, alles aus `vertex_index`/`instance_index` + Hash; Lage `fract((saat - g.cam.xyz) / box) * box` um die Kamera
  (Kasten 32×24×32), Zeit aus `g.cam.w`; Tiefentest an, Tiefe schreiben aus, Alpha-Mischung, nach den Instanzen zeichnen, nur wenn
  `wx.y > 0.01` und Art > 0; Instanzen über dem Stärke-Anteil fallen auf Fläche 0 zusammen. Regen: schräge Striche (22 Blöcke/s),
  Schnee: kleine weiße Würfelchen, die schaukeln, Kirschblüten: rosa Plättchen im Sinus-Wind. Kein CPU-Aufwand je Bild.
- **Nässe/Frost** in `block_color` vor `light_surface`: Regen und `n.y > 0.5` → `rough *= mix(1.0, 0.25, g.wx.y)`, Tropfen-Ringe als
  kleine Normalen-Störung, leicht dunkler; Schnee → Frost-Schimmer `mix(alb, vec3(0.86, 0.92, 1.0), 0.35 * g.wx.y)` auf Oberseiten.
- **Sternschnuppen** in `sky()` bei Nacht (`g.zen.w > 0.01`, nur `disc`-Pfad, `up > 0.3`): alle paar Sekunden ein kurzer heller Strich.
- **final.wgsl**: Filter über `g.post.x` (1 Retro-Pixel: Raster + 5 Farbstufen; 2 Goldene Stunde: warm; 3 Kino-Kontrast: S-Kurve,
  weniger Sättigung, stärkere Vignette); Brennweite `g.post.y > 0`: Unschärfe `smoothstep(0, 4 + 0.6·focal, |Tiefe − focal|)` ab Stufe 1.
  Balken der Zwischensequenz macht die Oberfläche (CSS). Eco hat keinen Nachbearbeitungs-Durchgang (die Oberfläche hebt im Foto-Modus auf Stufe 1).
- WGSL muss naga bestehen (WebGPU und WebGL2).

## 4. Oberfläche (web/)
Gemeinsamer Kontext, den hud.js jedem Modul gibt: `ctx = { api, el, $, ton, toast, fehler, modal, zuKnopf, menu, TOUCH,
game: () => game, me: () => me, reg: () => reg, opt: () => opt, optSpeichern, optAnwenden }`. Text nur über `el()`/`textContent`.
### §W-P21 (musik.js, foto.js)
```js
// musik.js – import { STIMMUNGEN } from './musik_daten.js?v=__V__'
export function start(ac)          // AudioContext aus hud.js (nach Nutzer-Geste); Master → Echo (0,32 s, Rückkopplung) → Ziel
export function setLaut(v)         // 0..1; bei 0 steht der Planer sofort
export function setStimmung(n)     // 0 Insel, 1 Hub, 2 Zitadelle/Katakomben, 3 Arena; 2 s Überblendung
export function stimmungVonZone(z) // "Privatinsel"→0, "Prüfungs-Zitadelle"/"Prüfungs-Katakomben"→2, "Lern-Duell-Arena"→3, sonst 1
export function setRegen(an)       // Regenrauschen: EINE Schleifen-Quelle (rosa Rauschen, Tiefpass), kein Puffer je Aufruf
export function pause(an)          // document.hidden
export function zustand()          // { laeuft, stimmung, geplant } für Prüfungen
// foto.js
export function initFoto(ctx)
export function fotoUmschalten()   // F8 / Knopf #b-foto
export function fotoAktiv()
export function nachBild()         // hud.js ruft das direkt nach game.frame(t): fälliges Foto per canvas.toBlob → Download PNG
export function kino(an, id)       // Kern-Ereignis 'kino'
export function kinoPruefen(zone)  // erster Besuch (localStorage 'campus-kino') → game.cutscene(id)
export function taste(e)           // true = verbraucht (Leertaste überspringt die Fahrt; im Foto-Modus Filter/Foto/Ende)
```
- Musik (LAW 3): Planer mit Vorlauf `ac.currentTime + 0.15`, jeder Oszillator bekommt `stop(ende)`, keine Verweise behalten; Stimme =
  Sinus + Dreieck (2×, Pegel 0,18) → Tiefpass 1800 → 420 Hz in 0,4 s → Hüllkurve mit 2,8 s Ausklang. Vier Stimmungen aus
  `STIMMUNGEN` (BPM 68/88/112/124; Zitadelle mit tiefem Bordun + Staccato, Arena mit Achtel-Bass). Still bei Lautstärke 0 oder verdecktem Tab.
- Foto: Klasse `foto` am `body` (CSS blendet die Oberfläche aus), `game.set_photo(true)`, Leiste `#foto-leiste` (vier Filter, Regler
  Brennweite 0–64, [Screenshot speichern], [Beenden]); hebt die Grafik für die Dauer auf mindestens Stufe 1
  (`game.set_graphics(Math.max(1, opt.grafik), opt.himmel)`, danach `ctx.optAnwenden()`). Fahrt: Klasse `kino`, `#kino` mit Balken und
  „[Leertaste zum Überspringen]“. Zonen → Fahrt: „Marktplatz“ 0, „Prüfungs-Zitadelle“ 1, „Prüfungs-Katakomben“ 2, „Lern-Duell-Arena“ 3.
### §W-SPIEL (duell.js, katakomben.js, architekt.js, inventar.js)
```js
export function initDuell(ctx);  export function duellHud(j);        // t = duell | duell_einladung
export async function ranglisteSeite();                               // Knoten für die Menü-Seite „Duell-Arena“ (GET duell/rangliste, [Duell starten])
export function initKatakomben(ctx);  export function katakombenHud(j);   // t = katakomben | weise
export function initArchitekt(ctx);  export function architektHud(j);     // t = architekt | architekt_export | architekt_npc | npc
export function npcsSetzen(liste);  export function npcName(id);          // aus Kern-Ereignis 'npcs': [[id, name], …]
```
- Duell: `lobby` → Themen-Wahl in `#dialog` (Auto-Karte „… – Empfohlen aus eurem Lernhelfer“, vier Fach-Kacheln, Stufen-Wahl,
  Eingabe `maxlength=60` für das Wunsch-Thema; jede Wahl POST `duell/wahl`), `roulette` als umlaufende Liste, danach nicht-modales
  `#duell-hud` (Frage, vier farbige Felder A–D – anklickbar → POST `duell/antwort` –, Zeitbalken, Punkte), `aufloesung` mit Erklärung,
  `ende` mit Belohnung und Wertung, `aus` räumt auf. `duell_einladung` → Annehmen sendet `game().say(0, '/duel join <Name>')`.
- Katakomben: `portal` → `#dialog` mit Stufe 1–5 und offenen Läufen; `#kata-hud` (Raum, Zeit, Tode, Rätsel n/3, Note groß mit
  Klasse `note-c|b|a|s|sp`, drei Punktebalken); `segen` → `game().set_speed?.(1.25)`, bei `aus` zurück auf 1; `weise` → `#dialog`;
  `ende` → Ergebnis-Fenster.
- Architekt: `architekt` → `#arch-leiste` (Pos1, Pos2, Maße X × Y × Z, Blöcke, Rückgängig-Zähler, Knöpfe senden `//`-Befehle über
  `game().say`), dazu `game().set_architect?.(an)` und `game().set_selection?.(…)`; `architekt_export` → GET `admin/blueprint/<name>`,
  `#dialog` mit `<textarea class=arch-export>` und [Als Rust-Blueprint kopieren]; `architekt_npc` → Formular (POST `admin/npc`);
  `npc` → Dialog mit Name und Text.
- inventar.js: auf der Seite „Verzaubern“ bei Stufe V einen Knopf „Stufe VI (Foliant)“ zeigen, wenn Gegenstand 2421 im Inventar
  liegt (POST `enchant` wie bisher). Sonst nichts ändern.
### §W-HUD (hud.js, index.html, hud.css, hilfe.html) – alle Einhängepunkte
- Importe der fünf Module (`?v=__V__`), `<link rel=stylesheet href="p21.css?v=__V__">`, nach `initInventar` die `init…(ctx)`.
- index.html: `#event-banner`, `#arch-leiste`, `#duell-hud`, `#kata-hud`, `#foto-leiste`, `#kino` (alle `hidden`), Knopf `#b-foto` in `#oben`.
- Einstellungen: `STANDARD.musik = 0.5`, `STANDARD.wetter = 0`; Regler „Musik-Lautstärke“, Wahl „Wetter“ [Auto, Klar, Sommerregen,
  Schneefall, Kirschblüten]. `optAnwenden`: `musik.setLaut(opt.musik)`, Wetter = `opt.wetter ? opt.wetter - 1 : wetterServer` →
  `game.set_weather?.(w)`, `musik.setRegen(w === 1)`. `tonAn()` ruft `musik.start(ac)`. `visibilitychange` → `musik.pause(document.hidden)`.
- Hud-Weiche (`j.t`): `wetter` (merken, anwenden), `event` (Banner mit Text und Restzeit), `duell`/`duell_einladung` → `duell.duellHud`,
  `katakomben`/`weise` → `katakomben.katakombenHud`, `architekt`/`architekt_export`/`architekt_npc`/`npc` → `architekt.architektHud`;
  `boss` mit `aus`: Text aus `j.name`. `JSON.parse` der Hud-Nachricht absichern.
- Kern-Ereignisse: `zone` → `musik.setStimmung(musik.stimmungVonZone(s))`, `foto.kinoPruefen(s)`; `kino` → `foto.kino(a, b)`;
  `npcs` → `architekt.npcsSetzen(JSON.parse(s))`; Plaketten mit id ≥ `NPC_BASE + 200` heißen `architekt.npcName(id - NPC_BASE)`.
- Tasten: `foto.taste(e)` zuerst fragen; F8 → `foto.fotoUmschalten()` (preventDefault); bei verlorenem Zeiger nicht das Menü öffnen,
  wenn `foto.fotoAktiv()`. `bild()`: nach `game.frame(t)` `foto.nachBild()`.
- Hinweis-Texte für [E]: 123 „Katakomben betreten“, 124 „Lern-Duell starten“, 131/132 „Prisma drehen“, 139 „Weisen befragen“,
  140/142/143 „Truhe öffnen“, 141 „Beute nehmen“, 145 „Blut-Portal betreten“. Menü-Kachel + Seite „Duell-Arena“ (`duell.ranglisteSeite`).
- hilfe.html: Abschnitte Musik & Wetter, Foto-Modus (F8), Lern-Duell-Arena, Prüfungs-Katakomben, Architekten-Modus (Verwaltung).
### §H-WEB (musik_daten.js, p21.css) – reine Daten/Gestaltung
- `musik_daten.js`: `export const STIMMUNGEN = [ … 4 Einträge in der Reihenfolge Insel, Hub, Zitadelle, Arena … ]`, je Eintrag
  `{ name, bpm (68, 88, 112, 124), grundton (MIDI 48–62), tonleiter: [Halbtöne ab Grundton, 5–7 Stück], akkorde: [[Stufen-Index…] ×≥8],
  melodien: [[8 Einträge je Takt: Stufen-Index oder null] ×≥12], bass: [[8 Einträge: Stufen-Index oder null] ×≥6], dichte (0.3–0.9),
  hall (0–0.5), bordun: bool, staccato: bool }`. Insel: Lydisch/Dur-Sept, spärlich; Hub: Dorisch/Mixolydisch mit gezupftem Bass;
  Zitadelle: Moll-Pentatonik/Phrygisch, `bordun` und `staccato` an; Arena: treibend, Bass auf allen Achteln. Stufen-Index ≥ Länge der
  Tonleiter = eine Oktave höher. Nur Daten, kein Code.
- `p21.css` (Farben/Variablen und Kanten-Stil aus hud.css übernehmen, `border-radius: 0`): `#event-banner` (oben Mitte, gold, pulsiert),
  `#arch-leiste` (oben links unter `#oben`; `.arch-pos`, `.arch-mass`, `.arch-knopf`), `#duell-hud` (oben Mitte; `.duell-frage`,
  `.duell-antworten` 2×2, `.duell-feld.a` blau `.b` gold `.c` grün `.d` lila, Zustände `.richtig` `.falsch` `.gewaehlt`, `.duell-zeit`,
  `.duell-punkte`, `.duell-spieler.ich`, `.duell-erklaerung`), Wahl-Fenster (`.duell-wahl`, `.duell-auto`, `.duell-faecher`,
  `.duell-stufen`, `.duell-frei`, `.duell-roulette` mit `.aktiv`), `#kata-hud` (rechts; `.kata-zeile`, `.kata-note` groß mit
  `.note-c` grau `.note-b` blau `.note-a` grün `.note-s` gold `.note-sp` gold leuchtend, `.kata-balken`), `.kata-portal`, `.kata-stufe`,
  `#foto-leiste` (unten Mitte; `.foto-filter[aria-pressed=true]`, `.foto-fokus`, `.foto-knopf`), `#kino` (`.kino-balken.oben/.unten`
  je 11 % Höhe, schwarz, fahren weich ein; `.kino-hinweis` unten Mitte), `body.foto` und `body.kino` blenden `#hud`, `#oben`, `#leiste`,
  `#touch`, `#plates`, `#zahlen`, `#toasts` aus, `.arch-export` (Textfeld, Festbreite), `.npc-form`. Reduzierte Bewegung beachten
  (`prefers-reduced-motion`), Handy-Breite ≤ 480 px.
