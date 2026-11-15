//! Phase 22 + Wetter/Event (Besitz: Agent S-ARCH, Vertrag CONTRACT-21-25.md §S-ARCH): Architekten-Modus der Verwaltung
//! (Auswahl, `//`-Befehle, Zwischenablage, Rückgängig-Ring, Export), frei gesetzte NPCs, 2×-Taler-Event und Wetter.
//! Der Verwaltungs-Schutz (403) sitzt an den Einhängepunkten (sky.rs command, game.rs C2S::Arch, instance.rs p21_get/p21_post).
use crate::game::{ci, Game};
use crate::sky::clean;
use crate::{no, E, R};
use campus_shared::instance as ins;
use campus_shared::npc::{self, NpcAction, CUSTOM_NPC_BASE, CUSTOM_NPC_MAX, NAME_MAX, TEXT_MAX};
use campus_shared::proto::{NpcState, Warp, S2C};
use campus_shared::world;
use campus_shared::worldedit::{self as we, Cell, Clip, Region, MAX_BLOCKS, UNDO_MAX};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

/// Namen der Wetterlagen, Index = `art` in Hud `wetter`.
pub const WETTER: [&str; 4] = ["Klar", "Sommerregen", "Schneefall", "Kirschblüten"];
/// Wetterwechsel alle 10 Minuten.
const WETTER_MS: i64 = 600_000;
/// Bis zu so vielen geänderten Zellen einzeln melden (S2C::Block), darüber ganze Stücke neu schicken.
const EINZELN_MAX: usize = 200;
const EVENT_TEXT: &str = "2× Taler-Event";

/// Ein Rückgängig-Schritt: (Zelle, alt, neu) nur wirklich geänderter Zellen.
type Step = Vec<(Cell, u8, u8)>;

/// Zustand eines Verwalters im Architekten-Modus (nur im Speicher, weg bei Trennung).
#[derive(Default)]
struct Sel {
    on: bool,
    pos1: Option<Cell>,
    pos2: Option<Cell>,
    /// zuletzt mit dem Stab angeklickter Block (Mitte für //sphere, //cyl)
    last: Option<Cell>,
    clip: Option<Clip>,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
}

/// Zustand des Architekten-Modus aller Verwalter (nur im Speicher).
#[derive(Default)]
pub struct Arch {
    admins: HashMap<u32, Sel>,
    /// zuletzt an alle gemeldetes Wetter
    wetter: Option<u8>,
}

/// Wetter zum Zeitpunkt `now` (ms), wenn die Verwaltung nichts festgelegt hat. Rein und deterministisch:
/// je 10-Minuten-Abschnitt ein gemischter Zufallswert (splitmix64), etwa 55 % Klar, 20 % Regen, 13 % Schnee, 12 % Blüten.
pub fn weather_at(now: i64) -> u8 {
    let mut h = (now.div_euclid(WETTER_MS) as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    match h % 100 {
        0..=54 => 0,
        55..=74 => 1,
        75..=87 => 2,
        _ => 3,
    }
}

/// Freier NPC aus `custom_npcs` (`n` = Nummer 0..49, Talk-Id = 200 + n).
struct NpcRow {
    n: u8,
    p: [f32; 3],
    yaw: f32,
    name: String,
    look: u8,
    text: String,
    action: String,
}

fn zahl(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
fn bloecke(n: usize) -> String {
    if n == 1 {
        "1 Block".into()
    } else {
        format!("{} Blöcke", zahl(n))
    }
}
fn fits(n: usize) -> R<()> {
    if n > MAX_BLOCKS {
        return no(413, &format!("Zu viele Blöcke: {} (höchstens {}).", zahl(n), zahl(MAX_BLOCKS)));
    }
    Ok(())
}
/// Block als Nummer (0 = Luft) oder Konstantenname (`BlockDef.key`, Groß/Klein egal); geschützte Blöcke nie.
fn block_arg(s: &str) -> R<u8> {
    if s.is_empty() {
        return no(400, "Block fehlt (Nummer oder Name, z. B. FELS).");
    }
    let b = match s.parse::<u8>() {
        Ok(n) => n,
        Err(_) => world::BLOCKS.iter().find(|d| d.key.eq_ignore_ascii_case(s)).map(|d| d.id).ok_or(E(404, format!("Unbekannter Block „{s}“.")))?,
    };
    if b != 0 && world::block(b).is_none() {
        return no(404, &format!("Unbekannter Block „{s}“."));
    }
    if we::protected(b) {
        return no(400, "Dieser Block ist geschützt (Truhen, Ständer, Schilder, Saat, Teleporter …).");
    }
    Ok(b)
}
fn push_undo(ring: &mut VecDeque<Step>, step: Step) {
    if step.is_empty() {
        return;
    }
    ring.push_back(step);
    while ring.len() > UNDO_MAX {
        ring.pop_front();
    }
}
fn event_json(an: bool, bis: i64) -> Value {
    json!({ "t": "event", "an": an, "text": EVENT_TEXT, "bis": bis })
}
fn aktion_name(a: NpcAction) -> &'static str {
    match a {
        NpcAction::None => "",
        NpcAction::Warp(_) => "warp",
        NpcAction::Taler(_) => "taler",
        NpcAction::Item(..) => "item",
    }
}
fn npc_states(rows: &[NpcRow]) -> Vec<NpcState> {
    rows.iter().map(|r| NpcState { id: CUSTOM_NPC_BASE + r.n, p: r.p, yaw: r.yaw, look: r.look, name: r.name.clone() }).collect()
}
fn npc_list(rows: &[NpcRow]) -> Vec<Value> {
    rows.iter()
        .map(|r| json!({ "id": CUSTOM_NPC_BASE + r.n, "name": r.name, "look": r.look, "text": r.text, "aktion": r.action, "x": r.p[0], "y": r.p[1], "z": r.p[2] }))
        .collect()
}
fn vorlagen() -> Vec<&'static str> {
    npc::NPC_PRESETS.iter().map(|p| p.0).collect()
}
const AN_TEXT: &str = "Architekten-Modus an – Stab-Klicks oder //pos1, //pos2 setzen die Auswahl, //help zeigt die Befehle.";
const AUS_TEXT: &str = "Architekten-Modus aus.";

impl Game {
    fn sel(&mut self, id: u32) -> &mut Sel {
        self.arch.admins.entry(id).or_default()
    }
    /// Hud `architekt` an den Verwalter.
    fn arch_hud(&mut self, id: u32, text: Option<&str>) {
        let s = self.arch.admins.get(&id);
        let (pos1, pos2) = (s.and_then(|s| s.pos1), s.and_then(|s| s.pos2));
        let r = pos1.zip(pos2).map(|(a, b)| Region::new(a, b));
        let v = json!({
            "t": "architekt",
            "an": s.is_some_and(|s| s.on),
            "pos1": pos1,
            "pos2": pos2,
            "masse": r.map(|r| r.dims()),
            "bloecke": r.map_or(0, |r| r.volume()),
            "ablage": s.and_then(|s| s.clip.as_ref()).map(|c| c.size),
            "undo": s.map_or(0, |s| s.undo.len()),
            "redo": s.map_or(0, |s| s.redo.len()),
            "text": text,
        });
        self.hud(id, v);
    }
    fn npc_rows(&self) -> R<Vec<NpcRow>> {
        let v: Vec<NpcRow> = self
            .db
            .prepare("select id, x, y, z, yaw, name, look, text, action from custom_npcs where id >= 0 and id < 50 order by id")?
            .query_map([], |r| Ok(NpcRow { n: r.get(0)?, p: [r.get(1)?, r.get(2)?, r.get(3)?], yaw: r.get(4)?, name: r.get(5)?, look: r.get::<_, u8>(6)?.min(7), text: r.get(7)?, action: r.get(8)? }))?
            .collect::<Result<_, _>>()?;
        Ok(v)
    }
    fn npc_hud(&self) -> R<Value> {
        Ok(json!({ "t": "architekt_npc", "npcs": npc_list(&self.npc_rows()?), "vorlagen": vorlagen() }))
    }
    /// (Wetter, festgelegt?)
    fn weather_now(&self) -> (u8, bool) {
        match self.kv("wetter_fest").and_then(|v| v.parse::<i64>().ok()) {
            Some(a @ 0..=3) => (a as u8, true),
            _ => (weather_at(self.now()), false),
        }
    }
    /// Schreibt Zellen nach den Regeln des Vertrags und liefert, was sich wirklich geändert hat.
    fn arch_write(&mut self, cells: Vec<(Cell, u8)>) -> Step {
        let step: Step = cells
            .into_iter()
            .filter_map(|(c, b)| {
                if !we::in_world(c) || world::anchor(c[0], c[1], c[2]) || ins::kind_at(c[0], c[2]).is_some() || we::protected(b) {
                    return None;
                }
                let old = self.block(c[0], c[1], c[2]);
                (old != b && !we::protected(old)).then_some((c, old, b))
            })
            .collect();
        if step.len() <= EINZELN_MAX {
            for &(c, _, b) in &step {
                self.set_block(c[0], c[1], c[2], b, 0);
            }
        } else {
            let mut touched: Vec<usize> = Vec::new();
            for &(c, _, b) in &step {
                self.set_raw(c[0], c[1], c[2], b);
                touched.push(ci(c[0] >> 4, c[2] >> 4));
            }
            touched.sort_unstable();
            touched.dedup();
            for i in touched {
                self.resend(i);
            }
        }
        step
    }
    /// Neue Änderung: Grenze prüfen, schreiben, als Rückgängig-Schritt merken (leert `redo`).
    fn arch_change(&mut self, id: u32, cells: Vec<(Cell, u8)>) -> R<String> {
        fits(cells.len())?;
        let step = self.arch_write(cells);
        let n = step.len();
        let s = self.sel(id);
        if !step.is_empty() {
            s.redo.clear();
            push_undo(&mut s.undo, step);
        }
        Ok(format!("{} gesetzt.", bloecke(n)))
    }

    /// Nach der Anmeldung: Hud `wetter`, Hud `event` (wenn es läuft), S2C::Npcs.
    pub fn arch_join(&mut self, id: u32) {
        let (art, _) = self.weather_now();
        self.hud(id, json!({ "t": "wetter", "art": art }));
        let bis = self.event_until;
        if self.now() < bis {
            self.hud(id, event_json(true, bis));
        }
        let rows = self.npc_rows().unwrap_or_default();
        if !rows.is_empty() {
            self.send(id, &S2C::Npcs(npc_states(&rows)));
        }
    }
    /// Bei Trennung: Modus, Auswahl, Zwischenablage und Rückgängig-Ring dieses Verwalters verwerfen.
    pub fn arch_left(&mut self, id: u32) {
        self.arch.admins.remove(&id);
    }
    /// `/architect [an|aus|npc|wetter <…>|event <an [Minuten]|aus>]` – Aufrufer hat die Verwaltung schon geprüft.
    pub fn arch_cmd(&mut self, id: u32, rest: &str) -> R<()> {
        const SO: &str = "So: /architect [an|aus|npc|wetter auto|klar|regen|schnee|blueten|event an [Minuten]|event aus]";
        let low = rest.trim().to_lowercase();
        let mut w = low.split_whitespace();
        let (a, b, c) = (w.next().unwrap_or(""), w.next(), w.next());
        let text: String = match (a, b) {
            ("", _) => {
                let s = self.sel(id);
                s.on = !s.on;
                (if s.on { AN_TEXT } else { AUS_TEXT }).into()
            }
            ("an" | "ein", None) => {
                self.sel(id).on = true;
                AN_TEXT.into()
            }
            ("aus", None) => {
                self.sel(id).on = false;
                AUS_TEXT.into()
            }
            ("npc" | "npcs", None) => {
                let v = self.npc_hud()?;
                self.hud(id, v);
                "NPC-Verwaltung geöffnet.".into()
            }
            ("wetter", Some(x)) => {
                let art: i64 = match x {
                    "auto" => -1,
                    "klar" => 0,
                    "regen" => 1,
                    "schnee" => 2,
                    "blueten" | "blüten" => 3,
                    _ => return no(400, SO),
                };
                self.weather_post(&json!({ "art": art }))?;
                format!("Wetter: {}.", if art < 0 { "automatisch" } else { WETTER[art as usize] })
            }
            ("event", Some("an")) => {
                let min = c.and_then(|m| m.parse::<i64>().ok()).unwrap_or(60);
                let v = self.event_post(&json!({ "an": true, "minuten": min }))?;
                format!("{EVENT_TEXT} läuft {} Minuten.", v["minuten"].as_i64().unwrap_or(60))
            }
            ("event", Some("aus")) => {
                self.event_post(&json!({ "an": false }))?;
                format!("{EVENT_TEXT} beendet.")
            }
            _ => return no(400, SO),
        };
        self.system(id, &text);
        self.arch_hud(id, Some(&text));
        Ok(())
    }
    /// `//set 5` → word = "set", rest = "5" – Aufrufer hat die Verwaltung schon geprüft.
    pub fn arch_edit(&mut self, id: u32, word: &str, rest: &str) -> R<()> {
        let feet = self.players.get(&id).map(|p| p.p.map(|v| v.floor() as i32)).ok_or(E(401, "Nicht verbunden.".into()))?;
        let a: Vec<&str> = rest.split_whitespace().collect();
        let arg = |i: usize| a.get(i).copied().unwrap_or("");
        let (pos1, pos2, last) = {
            let s = self.sel(id);
            (s.pos1, s.pos2, s.last)
        };
        let sel_region = || -> R<Region> {
            match (pos1, pos2) {
                (Some(p), Some(q)) => Ok(Region::new(p, q)),
                _ => no(400, "Erst eine Auswahl setzen: //pos1 und //pos2 (oder Stab-Klicks)."),
            }
        };
        let cmd = word.to_lowercase();
        let text: String = match cmd.as_str() {
            "pos1" | "pos2" => {
                if !we::in_world(feet) {
                    return no(400, "Hier liegt keine bebaubare Zelle.");
                }
                let s = self.sel(id);
                if cmd == "pos1" {
                    s.pos1 = Some(feet);
                } else {
                    s.pos2 = Some(feet);
                }
                format!("{} gesetzt: {} {} {}", if cmd == "pos1" { "Pos1" } else { "Pos2" }, feet[0], feet[1], feet[2])
            }
            "desel" => {
                let s = self.sel(id);
                s.pos1 = None;
                s.pos2 = None;
                "Auswahl aufgehoben.".into()
            }
            "set" => {
                let b = block_arg(arg(0))?;
                let r = sel_region()?;
                fits(r.volume())?;
                self.arch_change(id, r.cells().map(|c| (c, b)).collect())?
            }
            "walls" => {
                let b = block_arg(arg(0))?;
                let r = sel_region()?;
                let d = r.dims();
                let inner = if d[0] > 2 && d[2] > 2 { (d[0] - 2) as usize * (d[2] - 2) as usize * d[1] as usize } else { 0 };
                fits(r.volume() - inner)?;
                self.arch_change(id, we::walls(r).into_iter().map(|c| (c, b)).collect())?
            }
            "replace" => {
                let (from, to) = (block_arg(arg(0))?, block_arg(arg(1))?);
                let r = sel_region()?;
                fits(r.volume())?;
                let cells: Vec<(Cell, u8)> = r.cells().filter(|c| self.block(c[0], c[1], c[2]) == from).map(|c| (c, to)).collect();
                self.arch_change(id, cells)?
            }
            "sphere" | "cyl" => {
                let so = if cmd == "sphere" { "So: //sphere <Block> <Radius 1–20>" } else { "So: //cyl <Block> <Radius 1–30> <Höhe 1–62>" };
                let b = block_arg(arg(0))?;
                let r: i32 = arg(1).parse().map_err(|_| E(400, so.into()))?;
                let mid = last.unwrap_or(feet);
                let cells = if cmd == "sphere" {
                    if !(1..=20).contains(&r) {
                        return no(400, so);
                    }
                    we::sphere(mid, r)
                } else {
                    let h: i32 = arg(2).parse().map_err(|_| E(400, so.into()))?;
                    if !(1..=30).contains(&r) || !(1..=62).contains(&h) {
                        return no(400, so);
                    }
                    we::cyl(mid, r, h)
                };
                self.arch_change(id, cells.into_iter().map(|c| (c, b)).collect())?
            }
            "copy" => {
                let r = sel_region()?;
                fits(r.volume())?;
                let clip = Clip::copy(r, feet, |x, y, z| self.block(x, y, z));
                let n = clip.b.len();
                self.sel(id).clip = Some(clip);
                format!("{} kopiert.", bloecke(n))
            }
            "rotate" => {
                let s = self.sel(id);
                let rot = s.clip.as_ref().map(Clip::rotated);
                match rot {
                    Some(c) => {
                        s.clip = Some(c);
                        "Zwischenablage um 90° gedreht.".into()
                    }
                    None => return no(409, "Die Zwischenablage ist leer (//copy)."),
                }
            }
            "paste" => {
                let clip = self.sel(id).clip.clone().ok_or(E(409, "Die Zwischenablage ist leer (//copy).".into()))?;
                let cells: Vec<(Cell, u8)> = clip.cells(feet).collect();
                self.arch_change(id, cells)?
            }
            "undo" => {
                let step = self.sel(id).undo.pop_back().ok_or(E(409, "Nichts zum Rückgängigmachen.".into()))?;
                let n = self.arch_write(step.iter().rev().map(|&(c, old, _)| (c, old)).collect()).len();
                self.sel(id).redo.push(step);
                format!("Rückgängig: {}.", bloecke(n))
            }
            "redo" => {
                let step = self.sel(id).redo.pop().ok_or(E(409, "Nichts zum Wiederholen.".into()))?;
                let n = self.arch_write(step.iter().map(|&(c, _, new)| (c, new)).collect()).len();
                push_undo(&mut self.sel(id).undo, step);
                format!("Wiederholt: {}.", bloecke(n))
            }
            "export" => {
                let name = arg(0).to_lowercase();
                if name.is_empty() || name.len() > 32 || !name.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-') {
                    return no(400, "Name: 1–32 Zeichen aus a–z, 0–9, _ und -.");
                }
                let r = sel_region()?;
                fits(r.volume())?;
                let clip = Clip::copy(r, r.min, |x, y, z| self.block(x, y, z));
                let now = self.now();
                self.db.execute(
                    "insert or replace into custom_blueprints (name, w, h, d, data, by_user, ts) values (?, ?, ?, ?, ?, ?, ?)",
                    params![name, clip.size[0], clip.size[1], clip.size[2], world::rle(&clip.b), id, now],
                )?;
                self.hud(id, json!({ "t": "architekt_export", "name": name }));
                format!("Bauplan „{name}“ gespeichert ({} × {} × {}).", clip.size[0], clip.size[1], clip.size[2])
            }
            "help" | "hilfe" => "Befehle: //pos1 · //pos2 · //desel · //set <Block> · //walls <Block> · //replace <von> <nach> · //sphere <Block> <r> · //cyl <Block> <r> <h> · //copy · //rotate · //paste · //undo · //redo · //export <name>".into(),
            _ => return no(404, "Unbekannter Architekten-Befehl. //help zeigt alle."),
        };
        self.system(id, &text);
        self.arch_hud(id, Some(&text));
        Ok(())
    }
    /// Stab-Klick (C2S::Arch): op 1 = Pos1, 2 = Pos2 – Aufrufer hat die Verwaltung schon geprüft.
    pub fn arch_click(&mut self, id: u32, op: u8, x: i32, y: i32, z: i32) -> R<()> {
        let c = [x, y, z];
        if !self.arch.admins.get(&id).is_some_and(|s| s.on) {
            return no(409, "Der Architekten-Modus ist aus (/architect).");
        }
        if !we::in_world(c) {
            return no(400, "Außerhalb der bebaubaren Welt.");
        }
        let s = self.sel(id);
        match op {
            1 => s.pos1 = Some(c),
            2 => s.pos2 = Some(c),
            _ => return no(400, "Unbekannte Stab-Aktion."),
        }
        s.last = Some(c);
        let text = format!("Pos{op}: {x} {y} {z}");
        self.arch_hud(id, Some(&text));
        Ok(())
    }
    pub fn arch_get(&mut self) -> R<Value> {
        let bps: Vec<Value> = self
            .db
            .prepare("select name, w, h, d, ts from custom_blueprints order by ts desc, name")?
            .query_map([], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "w": r.get::<_, i64>(1)?, "h": r.get::<_, i64>(2)?, "d": r.get::<_, i64>(3)?, "ts": r.get::<_, i64>(4)? })))?
            .collect::<Result<_, _>>()?;
        let (art, fest) = self.weather_now();
        let bis = self.event_until;
        let an = self.now() < bis;
        let npcs = npc_list(&self.npc_rows()?);
        Ok(json!({
            "blueprints": bps,
            "npcs": npcs,
            "event": { "an": an, "bis": bis },
            "wetter": { "art": art, "fest": fest },
            "vorlagen": vorlagen(),
        }))
    }
    pub fn arch_blueprint(&mut self, name: &str) -> R<Value> {
        let row: Option<(i32, i32, i32, Vec<u8>)> =
            self.db.query_row("select w, h, d, data from custom_blueprints where name = ?", [name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
        let (w, h, d, data) = row.ok_or(E(404, "Diesen Bauplan gibt es nicht.".into()))?;
        if w < 1 || h < 1 || d < 1 || w as usize * h as usize * d as usize > MAX_BLOCKS {
            return no(500, "Dieser Bauplan ist beschädigt.");
        }
        let mut b = vec![0u8; w as usize * h as usize * d as usize];
        if !world::unrle(&data, &mut b) {
            return no(500, "Dieser Bauplan ist beschädigt.");
        }
        let clip = Clip { size: [w, h, d], off: [0; 3], b };
        Ok(json!({ "name": name, "w": w, "h": h, "d": d, "rust": we::rust_blueprint(name, &clip) }))
    }
    pub fn arch_post(&mut self, id: u32, b: &Value) -> R<Value> {
        if !self.players.contains_key(&id) {
            return no(409, "Erst den Campus betreten – der Modus gilt für die verbundene Figur.");
        }
        let s = self.sel(id);
        let on = b.get("an").and_then(Value::as_bool).unwrap_or(!s.on);
        s.on = on;
        let text = if on { AN_TEXT } else { AUS_TEXT };
        self.system(id, text);
        self.arch_hud(id, Some(text));
        Ok(json!({ "ok": true, "an": on }))
    }
    /// POST `admin/npc {id?, name, look, text, aktion, hier?, loeschen?}`. `id` darf die Talk-Id (200 + n) oder die Nummer n sein.
    pub fn npc_post(&mut self, id: u32, b: &Value) -> R<Value> {
        let rows = self.npc_rows()?;
        let num = match b.get("id").and_then(Value::as_u64) {
            Some(v) => {
                let n = if v >= CUSTOM_NPC_BASE as u64 { v - CUSTOM_NPC_BASE as u64 } else { v };
                if n >= CUSTOM_NPC_MAX as u64 {
                    return no(404, "Diesen NPC gibt es nicht.");
                }
                Some(n as u8)
            }
            None => None,
        };
        let here = self.players.get(&id).map(|p| (p.p, p.yaw));
        if b["loeschen"].as_bool() == Some(true) {
            let n = num.ok_or(E(400, "Feld „id“ fehlt.".into()))?;
            if self.db.execute("delete from custom_npcs where id = ?", [n])? == 0 {
                return no(404, "Diesen NPC gibt es nicht.");
            }
            self.db.execute("delete from npc_rewards where npc = ?", [n])?;
        } else {
            let old = match num {
                Some(n) => Some(rows.iter().find(|r| r.n == n).ok_or(E(404, "Diesen NPC gibt es nicht.".into()))?),
                None => None,
            };
            let name = b.get("name").and_then(Value::as_str).map(str::to_string).or_else(|| old.map(|o| o.name.clone())).unwrap_or_default();
            let name: String = clean(&name).chars().take(NAME_MAX).collect::<String>().trim().to_string();
            if name.is_empty() {
                return no(400, "Der NPC braucht einen Namen.");
            }
            let text = b.get("text").and_then(Value::as_str).map(str::to_string).or_else(|| old.map(|o| o.text.clone())).unwrap_or_default();
            let text: String = clean(&text).chars().take(TEXT_MAX).collect::<String>().trim().to_string();
            let look = match b.get("look").and_then(Value::as_u64) {
                Some(l) if l < npc::NPC_PRESETS.len() as u64 => l as u8,
                Some(_) => return no(400, "Aussehen: Vorlage 0 bis 7."),
                None => old.map_or(0, |o| o.look),
            };
            let aktion = b.get("aktion").and_then(Value::as_str).map(|s| s.trim().to_string()).or_else(|| old.map(|o| o.action.clone())).unwrap_or_default();
            if npc::parse_action(&aktion).is_none() {
                return no(400, "Unbekannte Aktion (leer, warp:<Ort>, taler:<1–500>, item:<Nr>:<1–64>).");
            }
            match old {
                Some(o) => {
                    let (p, yaw) = if b["hier"].as_bool() == Some(true) { here.ok_or(E(409, "Erst den Campus betreten.".into()))? } else { (o.p, o.yaw) };
                    self.db.execute(
                        "update custom_npcs set x = ?, y = ?, z = ?, yaw = ?, name = ?, look = ?, text = ?, action = ? where id = ?",
                        params![p[0], p[1], p[2], yaw, name, look, text, aktion, o.n],
                    )?;
                }
                None => {
                    if rows.len() >= CUSTOM_NPC_MAX {
                        return no(409, "Es gibt schon 50 freie NPCs.");
                    }
                    let n = (0..CUSTOM_NPC_MAX as u8).find(|n| !rows.iter().any(|r| r.n == *n)).ok_or(E(409, "Es gibt schon 50 freie NPCs.".into()))?;
                    let (p, yaw) = here.ok_or(E(409, "Erst den Campus betreten – der NPC entsteht an deiner Stelle.".into()))?;
                    self.db.execute(
                        "insert into custom_npcs (id, x, y, z, yaw, name, look, text, action) values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![n, p[0], p[1], p[2], yaw, name, look, text, aktion],
                    )?;
                }
            }
        }
        let rows = self.npc_rows()?;
        self.broadcast(&S2C::Npcs(npc_states(&rows)));
        let list = npc_list(&rows);
        self.hud(id, json!({ "t": "architekt_npc", "npcs": list, "vorlagen": vorlagen() }));
        Ok(json!({ "ok": true, "npcs": list }))
    }
    pub fn npc_talk(&mut self, id: u32, nid: u8) -> R<()> {
        let n = nid.checked_sub(CUSTOM_NPC_BASE).ok_or(E(404, "Diese Person gibt es nicht.".into()))?;
        let row = self.npc_rows()?.into_iter().find(|r| r.n == n).ok_or(E(404, "Diese Person gibt es nicht.".into()))?;
        let p = self.players.get(&id).map(|p| p.p).ok_or(E(401, "Nicht verbunden.".into()))?;
        if (0..3).map(|k| (p[k] - row.p[k]).powi(2)).sum::<f32>() > 8.0 * 8.0 {
            return no(403, "Geh näher heran.");
        }
        let act = npc::parse_action(&row.action).unwrap_or(NpcAction::None);
        let mut belohnt = false;
        if matches!(act, NpcAction::Taler(_) | NpcAction::Item(..)) {
            let today = self.today();
            let last: Option<i64> = self.db.query_row("select day from npc_rewards where npc = ? and user = ?", params![n, id], |r| r.get(0)).optional()?;
            if last != Some(today) {
                let mult = self.taler_mult();
                let inv = self.tx(|g| {
                    g.db.execute("insert into npc_rewards (npc, user, day) values (?, ?, ?) on conflict (npc, user) do update set day = excluded.day", params![n, id, today])?;
                    match act {
                        NpcAction::Taler(t) => {
                            g.pay(id, t as i64 * mult, &format!("npc:{n}"))?;
                            Ok(None)
                        }
                        NpcAction::Item(i, k) => Ok(Some((i, g.give(id, i, k as i64)?))),
                        _ => Ok(None),
                    }
                })?;
                if let Some((item, total)) = inv {
                    self.send(id, &S2C::Inv { item, n: total });
                }
                belohnt = true;
            }
        }
        self.hud(id, json!({ "t": "npc", "id": nid, "name": row.name, "text": row.text, "aktion": aktion_name(act), "belohnt": belohnt }));
        if let NpcAction::Warp(i) = act {
            self.warp(id, Warp::Spot(i))?;
        }
        Ok(())
    }
    pub fn event_post(&mut self, b: &Value) -> R<Value> {
        let an = b.get("an").and_then(Value::as_bool).ok_or(E(400, "Feld „an“ fehlt.".into()))?;
        let min = b.get("minuten").and_then(Value::as_i64).filter(|m| (1..=1440).contains(m)).unwrap_or(60);
        let bis = if an { self.now() + min * 60_000 } else { 0 };
        self.kv_set("event_2x_until", &bis.to_string())?;
        self.event_until = bis;
        self.hud_all(event_json(an, bis));
        let minuten = if an { min } else { 0 };
        Ok(json!({ "ok": true, "an": an, "bis": bis, "minuten": minuten }))
    }
    pub fn weather_post(&mut self, b: &Value) -> R<Value> {
        let art = b.get("art").and_then(Value::as_i64).filter(|a| (-1..=3).contains(a)).ok_or(E(400, "Feld „art“: -1 (automatisch) oder 0 bis 3.".into()))?;
        self.kv_set("wetter_fest", &art.to_string())?;
        let (now, fest) = self.weather_now();
        self.arch.wetter = Some(now);
        self.hud_all(json!({ "t": "wetter", "art": now }));
        Ok(json!({ "ok": true, "art": now, "fest": fest }))
    }
    /// Alle 5 s (lib.rs): Wetterwechsel melden, abgelaufenes Event beenden.
    pub fn weather_tick(&mut self) {
        let (art, _) = self.weather_now();
        if self.arch.wetter != Some(art) {
            self.arch.wetter = Some(art);
            self.hud_all(json!({ "t": "wetter", "art": art }));
        }
        if self.event_until != 0 && self.now() >= self.event_until {
            self.event_until = 0;
            let _ = self.kv_set("event_2x_until", "0");
            self.hud_all(event_json(false, 0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use campus_shared::proto::C2S;

    fn fill(g: &mut Game, a: Cell, b: Cell, f: impl Fn(Cell) -> u8) {
        for c in Region::new(a, b).cells() {
            g.set_raw(c[0], c[1], c[2], f(c));
        }
    }
    fn select(g: &mut Game, id: u32, a: Cell, b: Cell) {
        let s = g.sel(id);
        s.pos1 = Some(a);
        s.pos2 = Some(b);
    }
    fn all(g: &Game, a: Cell, b: Cell, f: impl Fn(Cell) -> u8) -> bool {
        Region::new(a, b).cells().all(|c| g.block(c[0], c[1], c[2]) == f(c))
    }

    #[test]
    fn nur_fuer_die_verwaltung() {
        let mut g = game();
        let _a = join(&mut g, 1, "Chef", true);
        let mut rx = join(&mut g, 2, "Gast", false);
        g.set_raw(100, 40, 600, world::GLAS);
        put(&mut g, 2, [100.5, 40.0, 600.5]);
        msgs(&mut rx);
        g.handle(2, C2S::Say { ch: 0, text: "//set 5".into() });
        assert!(says(&mut rx).iter().any(|s| s == "Nur für die Verwaltung."));
        g.clock_offset += 2000;
        g.handle(2, C2S::Say { ch: 0, text: "/architect".into() });
        assert!(says(&mut rx).iter().any(|s| s == "Nur für die Verwaltung."));
        g.handle(2, C2S::Arch { op: 1, x: 100, y: 40, z: 600 });
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "nein" && h["text"] == "Nur für die Verwaltung."));
        assert!(!g.arch.admins.contains_key(&2));
        assert_eq!(g.block(100, 40, 600), world::GLAS);
        let gast = user(2, "Gast", false);
        for op in ["admin/event", "admin/npc", "admin/wetter", "admin/architekt"] {
            assert_eq!(g.post(&gast, op, &json!({ "an": true, "name": "X", "art": 1 })).unwrap_err().0, 403, "{op}");
        }
        assert_eq!(g.taler_mult(), 1);
        assert!(g.arch_get().unwrap()["npcs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn setzen_rueckgaengig_wiederholen() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Chef", true);
        let (a, b) = ([100, 40, 600], [103, 42, 602]);
        let mix = |c: Cell| [world::FELS, world::GLAS, 0][((c[0] + c[1] + c[2]) % 3) as usize];
        fill(&mut g, a, b, mix);
        // Stab im Modus
        assert_eq!(g.arch_click(1, 1, 100, 40, 600).unwrap_err().0, 409);
        g.arch_cmd(1, "").unwrap();
        g.handle(1, C2S::Arch { op: 1, x: a[0] as i16, y: a[1] as i16, z: a[2] as i16 });
        g.handle(1, C2S::Arch { op: 2, x: b[0] as i16, y: b[1] as i16, z: b[2] as i16 });
        assert_eq!((g.arch.admins[&1].pos1, g.arch.admins[&1].pos2), (Some(a), Some(b)));
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "architekt" && h["masse"] == json!([4, 3, 3]) && h["bloecke"] == 36));
        g.arch_edit(1, "set", "glas").unwrap();
        assert!(all(&g, a, b, |_| world::GLAS));
        assert!(says(&mut rx).iter().any(|s| s.ends_with("gesetzt.")));
        g.arch_edit(1, "undo", "").unwrap();
        assert!(all(&g, a, b, mix), "exakt die alten Nummern");
        g.arch_edit(1, "redo", "").unwrap();
        assert!(all(&g, a, b, |_| world::GLAS));
        assert_eq!(g.arch_edit(1, "redo", "").unwrap_err().0, 409);
        g.arch_edit(1, "replace", &format!("GLAS {}", world::FELS)).unwrap();
        assert!(all(&g, a, b, |_| world::FELS));
        assert_eq!(g.arch_edit(1, "set", "quatsch").unwrap_err().0, 404);
        // Ring: 11 Schritte, nur 10 rückholbar
        let c = [110, 40, 600];
        g.set_raw(c[0], c[1], c[2], 0);
        select(&mut g, 1, c, c);
        for k in 0..11 {
            g.arch_edit(1, "set", if k % 2 == 0 { "FELS" } else { "GLAS" }).unwrap();
        }
        assert_eq!(g.arch.admins[&1].undo.len(), UNDO_MAX);
        for _ in 0..10 {
            g.arch_edit(1, "undo", "").unwrap();
        }
        assert_eq!(g.block(110, 40, 600), world::FELS, "Stand nach dem ersten Schritt");
        assert_eq!(g.arch_edit(1, "undo", "").unwrap_err().0, 409);
        g.arch_left(1);
        assert!(g.arch.admins.is_empty());
    }

    #[test]
    fn grosse_mengen_geschuetzt_und_instanzen() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Chef", true);
        let (a, b) = ([200, 40, 700], [219, 59, 719]);
        fill(&mut g, a, b, |_| world::GLAS);
        g.set_raw(205, 45, 705, world::TRUHE);
        select(&mut g, 1, a, b);
        msgs(&mut rx);
        g.arch_edit(1, "set", "FELS").unwrap();
        assert!(!msgs(&mut rx).iter().any(|m| matches!(m, S2C::Block { .. })), "Massen-Änderung ohne Einzel-Meldungen");
        assert!(g.players.contains_key(&1));
        assert_eq!(g.block(205, 45, 705), world::TRUHE, "geschützt");
        assert_eq!(Region::new(a, b).cells().filter(|c| g.block(c[0], c[1], c[2]) == world::FELS).count(), 7999);
        select(&mut g, 1, [200, 40, 700], [239, 60, 739]);
        assert_eq!(g.arch_edit(1, "set", "GLAS").unwrap_err().0, 413);
        assert_eq!(g.block(200, 40, 700), world::FELS, "abgelehnt ohne Änderung");
        assert_eq!(g.arch_edit(1, "sphere", "FELS 20").unwrap_err().0, 413);
        assert!(g.arch_edit(1, "set", "TRUHE").is_err());
        // Instanz-Felder bleiben Luft
        select(&mut g, 1, [330, 40, 10], [341, 40, 10]);
        g.arch_edit(1, "set", "FELS").unwrap();
        assert_eq!((g.block(335, 40, 10), g.block(336, 40, 10), g.block(341, 40, 10)), (world::FELS, 0, 0));
    }

    #[test]
    fn ablage_drehen_einfuegen_export() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Chef", true);
        let ids: Vec<u8> = world::BLOCKS.iter().map(|d| d.id).filter(|&b| b != 0 && !we::protected(b)).take(6).collect();
        let (a, b) = ([100, 40, 600], [102, 40, 601]);
        for (k, c) in Region::new(a, b).cells().enumerate() {
            g.set_raw(c[0], c[1], c[2], ids[k]);
        }
        select(&mut g, 1, a, b);
        put(&mut g, 1, [99.5, 40.0, 600.5]);
        g.arch_edit(1, "copy", "").unwrap();
        g.arch_edit(1, "rotate", "").unwrap();
        assert_eq!(g.arch.admins[&1].clip.as_ref().map(|c| c.size), Some([2, 1, 3]));
        put(&mut g, 1, [200.5, 40.0, 600.5]);
        g.arch_edit(1, "paste", "").unwrap();
        // +x wird +z: (102, 600) landet bei (200, 603), (100, 601) bei (199, 601)
        assert_eq!(g.block(200, 40, 603), g.block(102, 40, 600));
        assert_eq!(g.block(199, 40, 601), g.block(100, 40, 601));
        msgs(&mut rx);
        g.arch_edit(1, "export", "turm").unwrap();
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "architekt_export" && h["name"] == "turm"));
        let n: i64 = g.db.query_row("select count(*) from custom_blueprints where name = 'turm'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        let v = g.arch_blueprint("turm").unwrap();
        assert!(v["rust"].as_str().unwrap().contains("fn turm("), "{v}");
        assert_eq!((v["w"].clone(), v["h"].clone(), v["d"].clone()), (json!(3), json!(1), json!(2)));
        assert_eq!(g.arch_get().unwrap()["blueprints"][0]["name"], "turm");
        assert_eq!(g.arch_edit(1, "export", "Böse Name!").unwrap_err().0, 400);
        assert_eq!(g.arch_blueprint("fehlt").unwrap_err().0, 404);
    }

    #[test]
    fn npc_setzen_sprechen_belohnung_einmal_am_tag() {
        let mut g = game();
        let _a = join(&mut g, 1, "Chef", true);
        let mut rx = join(&mut g, 2, "Gast", false);
        put(&mut g, 1, [240.5, 33.0, 296.5]);
        put(&mut g, 2, [241.5, 33.0, 296.5]);
        msgs(&mut rx);
        let v = g.npc_post(1, &json!({ "name": "Lotte", "look": 2, "text": "Hallo!", "aktion": "taler:10" })).unwrap();
        assert_eq!(v["npcs"][0]["id"], 200);
        assert!(msgs(&mut rx).iter().any(|m| matches!(m, S2C::Npcs(v) if v.len() == 1 && v[0].id == 200 && v[0].name == "Lotte")));
        assert_eq!(g.npc_post(1, &json!({ "name": "X", "aktion": "quatsch" })).unwrap_err().0, 400);
        assert_eq!(g.npc_post(1, &json!({ "name": "X", "look": 9 })).unwrap_err().0, 400);
        let t0 = g.taler(2);
        g.npc_talk(2, 200).unwrap();
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "npc" && h["name"] == "Lotte" && h["aktion"] == "taler" && h["belohnt"] == true));
        assert_eq!(g.taler(2), t0 + 10);
        g.npc_talk(2, 200).unwrap();
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "npc" && h["belohnt"] == false));
        assert_eq!(g.taler(2), t0 + 10, "nur einmal am Tag");
        g.clock_offset += campus_shared::time::DAY_MS;
        g.npc_talk(2, 200).unwrap();
        assert_eq!(g.taler(2), t0 + 20, "am nächsten Tag wieder");
        put(&mut g, 2, [300.5, 33.0, 296.5]);
        assert_eq!(g.npc_talk(2, 200).unwrap_err().0, 403);
        assert_eq!(g.npc_talk(2, 201).unwrap_err().0, 404);
        // ändern (Aktion bleibt), Neuankömmling sieht ihn, löschen
        g.npc_post(1, &json!({ "id": 200, "name": "Lotta" })).unwrap();
        let mut rx3 = join(&mut g, 3, "Neu", false);
        assert!(msgs(&mut rx3).iter().any(|m| matches!(m, S2C::Npcs(v) if v[0].name == "Lotta")));
        assert_eq!(g.arch_get().unwrap()["npcs"][0]["aktion"], "taler:10");
        g.npc_post(1, &json!({ "id": 200, "loeschen": true })).unwrap();
        assert!(g.arch_get().unwrap()["npcs"].as_array().unwrap().is_empty());
        assert_eq!(g.npc_post(1, &json!({ "id": 200, "loeschen": true })).unwrap_err().0, 404);
    }

    #[test]
    fn event_und_wetter() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Chef", true);
        msgs(&mut rx);
        g.event_post(&json!({ "an": true, "minuten": 5 })).unwrap();
        assert_eq!(g.taler_mult(), 2);
        assert_eq!(g.kv("event_2x_until").and_then(|v| v.parse::<i64>().ok()), Some(g.event_until));
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "event" && h["an"] == true && h["text"] == EVENT_TEXT));
        g.clock_offset += 6 * 60_000;
        g.weather_tick();
        assert_eq!((g.taler_mult(), g.event_until), (1, 0));
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "event" && h["an"] == false));
        g.arch_cmd(1, "event an 30").unwrap();
        assert_eq!(g.taler_mult(), 2);
        g.arch_cmd(1, "event aus").unwrap();
        assert_eq!(g.taler_mult(), 1);
        // Wetter festlegen und wieder freigeben
        msgs(&mut rx);
        g.arch_cmd(1, "wetter schnee").unwrap();
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "wetter" && h["art"] == 2));
        assert_eq!(g.arch_get().unwrap()["wetter"], json!({ "art": 2, "fest": true }));
        g.weather_post(&json!({ "art": -1 })).unwrap();
        assert_eq!(g.arch_get().unwrap()["wetter"]["fest"], false);
        assert_eq!(g.weather_post(&json!({ "art": 7 })).unwrap_err().0, 400);
        // deterministisch, alle vier Lagen, Klar am häufigsten
        let mut seen = [0usize; 4];
        for k in 0..2000i64 {
            let t = k * WETTER_MS + 1234;
            assert_eq!(weather_at(t), weather_at(t + 500_000));
            seen[weather_at(t) as usize] += 1;
        }
        assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
        assert!(seen[0] > 900 && seen[0] < 1300, "{seen:?}");
        assert_eq!(zahl(1204), "1 204");
    }
}
