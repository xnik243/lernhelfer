//! Phase 23 (Besitz: Agent S-DUEL, Vertrag CONTRACT-21-25.md §S-DUEL): Lern-Duell-Arena.
//! Eine Runde lebt in einer Arena-Instanz (instance.rs): je Spieler eine 8×8-Plattform aus vier farbigen Antwort-Feldern
//! vor einer großen Anzeigetafel. Ablauf: lobby (Themenwahl) → [roulette] → [laden] → 8 × (frage → aufloesung) → ende.
//! Alle Zeiten kommen aus `now`; der Takt läuft nur, solange `Game.duels` nicht leer ist.
use crate::duel_ai::{normalize_topic, Question, ROUNDS, TIMEOUT_MS};
use crate::game::Game;
use crate::instance::Inst;
use crate::{no, R};
use campus_shared::instance as ins;
use campus_shared::proto::S2C;
use campus_shared::skyblock as sb;
use campus_shared::world;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::cmp::{Ordering, Reverse};

/// Anzeigenamen der vier Schnell-Kategorien (zugleich Thema für Cache/KI).
pub const FACH_NAMEN: [&str; 4] = ["Mathematik & Logik", "Sprachen & Vokabeln", "Naturwissenschaften", "Geschichte & Gesellschaft"];
/// Spieler-Nummer des KI-Bots (0 heißt im Hud „niemand“).
pub const BOT_ID: u32 = u32::MAX;
pub const BOT_NAME: &str = "Professor Kalkulus";

/// Themenwahl: genug Zeit, um in Ruhe aus den Lernhelfer-Vorschlägen zu wählen.
const LOBBY_MS: i64 = 40_000;
const LOBBY_MIN_MS: i64 = 15_000;
const ROULETTE_MS: i64 = 1_500;
const FRAGE_MS: i64 = 12_000;
const DROP_MS: i64 = 600;
/// Felder fallen nach DROP_MS weg und bleiben volle 3 s Luft (Spec: „für 3 Sekunden ins Void“).
const AUFL_MS: i64 = DROP_MS + 3_000;
const ENDE_MS: i64 = 8_000;
const HURT: f32 = 20.0;
const ELO_K: f64 = 24.0;

// Aufbau relativ zur Nordwest-Ecke der Instanz: Plattformen in einer Reihe (Süden), Tafel im Norden (Blick yaw 0 = −z).
const PX0: i32 = 2;
const PSTEP: i32 = 12;
const PZ: i32 = 28;
const BX0: i32 = 8;
const BX1: i32 = 39;
const BZ: i32 = 6;
const BH: i32 = 13;
const COLUMN: [u8; 4] = [world::SAPHIR, world::SPRACHGOLD, world::SMARAGD, world::KRISTALL];

#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    /// 0..3, 255 = freies Thema
    pub fach: u8,
    pub thema: String,
    /// Lernhelfer-Inhalte (Themen/Unterthemen) für die KI; leer = nur das Thema
    pub kontext: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Lobby,
    Roulette,
    Laden,
    Frage,
    Aufloesung,
    Ende,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Lobby => "lobby",
            Phase::Roulette => "roulette",
            Phase::Laden => "laden",
            Phase::Frage => "frage",
            Phase::Aufloesung => "aufloesung",
            Phase::Ende => "ende",
        }
    }
}

struct Seat {
    id: u32,
    name: String,
    farbe: u8,
    bot: bool,
    gone: bool,
    punkte: u32,
    serie: u32,
    richtig: u32,
    /// Feld unter den Füßen beim letzten Schritt
    feet: Option<u8>,
    /// zuletzt betretenes Feld in dieser Frage und wann
    stand: Option<(u8, i64)>,
    /// Antwort per Klick (POST duell/antwort) bzw. Bot-Antwort
    klick: Option<(u8, i64)>,
}

struct Vote {
    id: u32,
    art: &'static str,
    topic: Topic,
}

/// Ein laufendes Duell (Schlüssel in Game.duels = Instanz-Nummer).
pub struct Duel {
    pub inst: u32,
    phase: Phase,
    start: i64,
    until: i64,
    /// `/duel solo`: niemand kann beitreten
    solo: bool,
    /// höchstens ein Mensch bei Rundenbeginn: halbe Taler, keine Wertung
    solo_round: bool,
    seats: Vec<Seat>,
    votes: Vec<Vote>,
    auto: u8,
    topic: Option<Topic>,
    kandidaten: Vec<Topic>,
    /// Themen, für die schon in der Lobby KI-Fragen angefragt wurden (je Thema höchstens einmal)
    vorab: Vec<String>,
    sperre: i64,
    /// KI-Auftrag läuft, Fragen fehlen noch
    waiting: bool,
    qs: Vec<Question>,
    runde: usize,
    correct: u8,
    schnellster: u32,
    dropped: bool,
    last_hud: i64,
}

impl Duel {
    pub fn wartet(&self) -> bool {
        self.waiting
    }
    fn new(inst: u32, solo: bool, now: i64) -> Duel {
        Duel {
            inst,
            phase: Phase::Lobby,
            start: now,
            until: now + LOBBY_MS,
            solo,
            solo_round: solo,
            seats: Vec::new(),
            votes: Vec::new(),
            auto: 0,
            topic: None,
            kandidaten: Vec::new(),
            vorab: Vec::new(),
            sperre: 0,
            waiting: false,
            qs: Vec::new(),
            runde: 0,
            correct: 0,
            schnellster: 0,
            dropped: false,
            last_hud: now,
        }
    }
    fn humans(&self) -> usize {
        self.seats.iter().filter(|s| !s.bot && !s.gone).count()
    }
    fn free_farbe(&self) -> Option<u8> {
        (0..4u8).find(|f| !self.seats.iter().any(|s| s.farbe == *f))
    }
    fn spieler(&self) -> Value {
        Value::Array(
            self.seats
                .iter()
                .filter(|s| !s.gone)
                .map(|s| json!({ "id": s.id, "name": s.name, "punkte": s.punkte, "serie": s.serie, "bot": s.bot, "farbe": s.farbe }))
                .collect(),
        )
    }
    /// Hud `duell` für alle Phasen außer `ende` (das bekommt jeder einzeln mit seiner Belohnung).
    fn hud(&self, now: i64) -> Value {
        let mut v = json!({ "t": "duell", "phase": self.phase.name(), "spieler": self.spieler() });
        let rest = (self.until - now).max(0);
        let thema = self.topic.as_ref().map_or(String::new(), |t| t.thema.clone());
        match self.phase {
            Phase::Lobby => {
                v["rest_ms"] = json!(rest);
                v["auto"] = json!({ "fach": self.auto, "name": FACH_NAMEN[self.auto.min(3) as usize] });
                v["stimmen"] = Value::Array(self.votes.iter().map(|x| json!({ "id": x.id, "art": x.art, "fach": x.topic.fach, "thema": x.topic.thema })).collect());
            }
            Phase::Roulette => {
                v["rest_ms"] = json!(rest);
                v["kandidaten"] = json!(self.kandidaten.iter().map(|t| t.thema.clone()).collect::<Vec<_>>());
                v["sieger"] = json!(thema);
            }
            Phase::Laden => {
                v["thema"] = json!(thema);
                v["rest_ms"] = json!(rest);
            }
            Phase::Frage => {
                v["runde"] = json!(self.runde + 1);
                v["von"] = json!(ROUNDS);
                if let Some(q) = self.qs.get(self.runde) {
                    v["frage"] = json!(q.q);
                    v["antworten"] = json!(q.options);
                }
                v["rest_ms"] = json!(rest);
                v["thema"] = json!(thema);
            }
            Phase::Aufloesung => {
                v["runde"] = json!(self.runde + 1);
                v["richtig"] = json!(self.correct);
                v["erklaerung"] = json!(self.qs.get(self.runde).map_or(String::new(), |q| q.explanation.clone()));
                v["rest_ms"] = json!(rest);
                v["schnellster"] = json!(self.schnellster);
            }
            Phase::Ende => {}
        }
        v
    }
}

fn fach_topic(fach: u8) -> Topic {
    let f = fach.min(3);
    Topic { fach: f, thema: FACH_NAMEN[f as usize].into(), kontext: String::new() }
}

/// Themenwahl: meiste Stimmen je (fach, thema) gewinnt; Gleichstand → Kandidaten sortiert, Sieger = `kandidaten[seed % n]`;
/// keine Stimme → Auto-Fach. Der Kontext kommt von der ersten Stimme für das Sieger-Thema.
/// Liefert (Sieger, Stichwahl-Kandidaten; leer = klare Mehrheit).
pub fn resolve_votes(votes: &[Topic], auto: u8, seed: u64) -> (Topic, Vec<Topic>) {
    if votes.is_empty() {
        return (fach_topic(auto), Vec::new());
    }
    let mut keys: Vec<(u8, &str, usize)> = Vec::new();
    for v in votes {
        match keys.iter_mut().find(|k| k.0 == v.fach && k.1 == v.thema) {
            Some(k) => k.2 += 1,
            None => keys.push((v.fach, &v.thema, 1)),
        }
    }
    let max = keys.iter().map(|k| k.2).max().unwrap_or(0);
    let mut cand: Vec<(u8, &str)> = keys.iter().filter(|k| k.2 == max).map(|k| (k.0, k.1)).collect();
    cand.sort();
    let pick = |(f, t): (u8, &str)| votes.iter().find(|v| v.fach == f && v.thema == t).cloned().unwrap_or_else(|| fach_topic(f));
    if cand.len() == 1 {
        return (pick(cand[0]), Vec::new());
    }
    let all: Vec<Topic> = cand.into_iter().map(pick).collect();
    let w = all[(seed % all.len() as u64) as usize].clone();
    (w, all)
}

/// Gemeinsames schwächstes Fach 0..3 (kleinste Stufen-Summe, bei Gleichstand kleinster Index).
pub fn auto_fach(levels: &[[u32; 7]]) -> u8 {
    (0..4u8).min_by_key(|&f| (levels.iter().map(|l| l[f as usize] as u64).sum::<u64>(), f)).unwrap_or(0)
}

/// Neue ELO-Zahl für Spieler `i` (mehrere Gegner: paarweise, gemittelt).
fn elo_new(olds: &[i64], pts: &[u32], i: usize) -> i64 {
    let n = olds.len();
    if n < 2 {
        return olds.get(i).copied().unwrap_or(1000);
    }
    let (mut s, mut e) = (0.0f64, 0.0f64);
    for j in 0..n {
        if j == i {
            continue;
        }
        s += match pts[i].cmp(&pts[j]) {
            Ordering::Greater => 1.0,
            Ordering::Equal => 0.5,
            Ordering::Less => 0.0,
        };
        e += 1.0 / (1.0 + 10f64.powf((olds[j] - olds[i]) as f64 / 400.0));
    }
    olds[i] + (ELO_K * (s - e) / (n - 1) as f64).round() as i64
}

// ---------- Aufbau ----------
fn plat(o: (i32, i32), farbe: u8) -> (i32, i32) {
    (o.0 + PX0 + PSTEP * farbe as i32, o.1 + PZ)
}
/// Nordwest-Ecke des Feldes `q` (A NW, B NO, C SW, D SO).
fn quad_at(px: i32, pz: i32, q: u8) -> (i32, i32) {
    (px + (q as i32 % 2) * 4, pz + (q as i32 / 2) * 4)
}
/// Ankunft: Mitte des südlichen Glasrands, Blick zur Tafel.
fn spawn(o: (i32, i32), farbe: u8) -> [f32; 3] {
    let (px, pz) = plat(o, farbe);
    [px as f32 + 4.0, (ins::ARENA_Y + 1) as f32, pz as f32 + 8.5]
}
/// Schneeball-Rückstoß: 1,5 Blöcke waagrecht von `from` weg, festgehalten im 4×4-Feld mit Nordwest-Ecke `q`.
fn knock(from: [f32; 3], p: [f32; 3], q: (i32, i32)) -> [f32; 3] {
    let (dx, dz) = (p[0] - from[0], p[2] - from[2]);
    let l = (dx * dx + dz * dz).sqrt();
    let (dx, dz) = if l > 0.01 { (dx / l, dz / l) } else { (0.0, 1.0) };
    let (x0, z0) = (q.0 as f32 + 0.3, q.1 as f32 + 0.3);
    [(p[0] + 1.5 * dx).clamp(x0, x0 + 3.4), p[1], (p[2] + 1.5 * dz).clamp(z0, z0 + 3.4)]
}
#[cfg(test)]
fn quad_center(o: (i32, i32), farbe: u8, q: u8) -> [f32; 3] {
    let (px, pz) = plat(o, farbe);
    let (x, z) = quad_at(px, pz, q);
    [x as f32 + 2.0, (ins::ARENA_Y + 1) as f32, z as f32 + 2.0]
}
fn lamps(o: (i32, i32)) -> Vec<(i32, i32, i32)> {
    (o.0 + BX0..=o.0 + BX1).step_by(2).map(|x| (x, ins::ARENA_Y + BH, o.1 + BZ)).collect()
}
fn feet_quad(it: &Inst, p: [f32; 3]) -> Option<u8> {
    let y = ins::ARENA_Y as f32;
    if !(y + 0.5..y + 3.0).contains(&p[1]) {
        return None;
    }
    let b = it.get(p[0].floor() as i32, ins::ARENA_Y, p[2].floor() as i32);
    world::QUADS.iter().position(|&q| q == b).map(|i| i as u8)
}
fn build(it: &mut Inst) {
    let o = it.origin();
    let y = ins::ARENA_Y;
    for f in 0..4u8 {
        let (px, pz) = plat(o, f);
        it.fill(px - 1, y - 1, pz - 1, px + 8, y - 1, pz + 8, world::NEON_CYAN);
        it.fill(px, y - 1, pz, px + 7, y - 1, pz + 7, 0);
        it.fill(px - 1, y, pz - 1, px + 8, y, pz + 8, world::GLAS);
        for q in 0..4u8 {
            let (x, z) = quad_at(px, pz, q);
            it.fill(x, y, z, x + 3, y, z + 3, world::QUADS[q as usize]);
        }
    }
    // Anzeigetafel: schwarze Wand mit Neon-Rahmen, Lampen oben, vier Punktesäulen davor
    let (x0, x1, y1, z) = (o.0 + BX0, o.0 + BX1, y + BH, o.1 + BZ);
    it.fill(x0, y, z, x1, y1, z, world::SCHWARZ);
    it.fill(x0, y, z, x1, y, z, world::NEON_CYAN);
    it.fill(x0, y1, z, x1, y1, z, world::NEON_CYAN);
    it.fill(x0, y, z, x0, y1, z, world::NEON_CYAN);
    it.fill(x1, y, z, x1, y1, z, world::NEON_CYAN);
    for (x, ly, lz) in lamps(o) {
        it.put(x, ly, lz, world::GLUEH);
    }
    for f in 0..4 {
        let cx = x0 + 3 + 7 * f;
        it.fill(cx, y + 1, z + 1, cx + 3, y + 1, z + 1, world::GLAS);
    }
}
fn fill_up(qs: &mut Vec<Question>, more: Vec<Question>) {
    qs.truncate(ROUNDS);
    for doppelt in [false, true] {
        for q in &more {
            if qs.len() < ROUNDS && (doppelt || !qs.iter().any(|x| x.q == q.q)) {
                qs.push(q.clone());
            }
        }
    }
}

impl Game {
    /// `/duel`, `/duel solo`, `/duel <Name>`, `/duel join <Name>`, `/duel leave`; auch [E] am Arena-Portal (rest = "").
    pub fn duel_cmd(&mut self, id: u32, rest: &str) -> R<()> {
        let rest = rest.trim();
        let (word, arg) = rest.split_once(' ').map_or((rest, ""), |(a, b)| (a, b.trim()));
        let w = word.to_lowercase();
        if matches!(w.as_str(), "leave" | "verlassen" | "raus") {
            return if self.inst_of(id).is_some_and(|(_, k)| k == ins::KIND_ARENA) {
                self.inst_leave(id);
                Ok(())
            } else {
                no(409, "Du bist gerade in keinem Duell.")
            };
        }
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        if p.inst.is_some() {
            return no(409, "Du bist schon in der Arena oder in den Katakomben. /leave bringt dich zurück.");
        }
        let now = self.now();
        match w.as_str() {
            "" => {
                let open = self.duels.values().filter(|d| d.phase == Phase::Lobby && !d.solo && d.humans() < 4 && d.free_farbe().is_some()).map(|d| d.inst).min();
                match open {
                    Some(inst) => self.duel_seat(inst, id, now),
                    None => self.duel_open(id, false, now).map(|_| ()),
                }
            }
            "solo" => self.duel_open(id, true, now).map(|_| ()),
            "join" | "beitreten" => {
                if arg.is_empty() {
                    return no(400, "Mit wem? /duel join <Name>");
                }
                let Some((tid, tinst)) = self.players.values().find(|p| p.name.eq_ignore_ascii_case(arg)).map(|p| (p.id, p.inst)) else {
                    return no(404, "Diese Person ist gerade nicht im Campus.");
                };
                match tinst.and_then(|n| self.duels.get(&n)) {
                    Some(d) if tid != id && d.phase == Phase::Lobby && !d.solo => {
                        let inst = d.inst;
                        self.duel_seat(inst, id, now)
                    }
                    _ => no(409, "Diese Person wartet gerade nicht in einer offenen Duell-Lobby."),
                }
            }
            _ => {
                let Some((tid, tname)) = self.players.values().find(|p| p.name.eq_ignore_ascii_case(rest)).map(|p| (p.id, p.name.clone())) else {
                    return no(404, "Diese Person ist gerade nicht im Campus.");
                };
                if tid == id {
                    return no(400, "Du kannst dich nicht selbst herausfordern. /duel solo spielt gegen den Bot.");
                }
                let me = self.players.get(&id).map_or(String::new(), |p| p.name.clone());
                self.duel_open(id, false, now)?;
                self.hud(tid, json!({ "t": "duell_einladung", "von": me }));
                self.system(id, &format!("Einladung an {tname} geschickt."));
                Ok(())
            }
        }
    }
    fn duel_open(&mut self, id: u32, solo: bool, now: i64) -> R<u32> {
        let inst = self.inst_new(ins::KIND_ARENA)?;
        if let Some(it) = self.inst_mut(inst) {
            build(it);
        }
        self.duels.insert(inst, Duel::new(inst, solo, now));
        if let Err(e) = self.duel_seat(inst, id, now) {
            self.inst_close(inst);
            return Err(e);
        }
        Ok(inst)
    }
    fn duel_seat(&mut self, inst: u32, id: u32, now: i64) -> R<()> {
        let name = self.players.get(&id).map_or(String::new(), |p| p.name.clone());
        let Some(d) = self.duels.get(&inst) else { return no(404, "Diese Runde gibt es nicht mehr.") };
        let Some(farbe) = d.free_farbe() else { return no(409, "Diese Runde ist voll (höchstens 4).") };
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return no(404, "Diese Runde gibt es nicht mehr.") };
        self.inst_enter(inst, id, spawn(o, farbe), 0.0)?;
        if let Some(d) = self.duels.get_mut(&inst) {
            d.seats.push(Seat { id, name, farbe, bot: false, gone: false, punkte: 0, serie: 0, richtig: 0, feet: None, stand: None, klick: None });
            d.until = d.until.max(now + LOBBY_MIN_MS);
        }
        self.duel_auto(inst);
        self.duel_hud(inst, now);
        Ok(())
    }
    /// Auto-Fach aus den Lernständen aller Menschen neu bestimmen (auch die Auto-Stimmen folgen).
    fn duel_auto(&mut self, inst: u32) {
        let ids: Vec<u32> = self.duels.get(&inst).map(|d| d.seats.iter().filter(|s| !s.bot && !s.gone).map(|s| s.id).collect()).unwrap_or_default();
        let lv: Vec<[u32; 7]> = ids.iter().map(|&u| self.levels(u)).collect();
        let auto = auto_fach(&lv);
        if let Some(d) = self.duels.get_mut(&inst) {
            d.auto = auto;
            for v in d.votes.iter_mut().filter(|v| v.art == "auto") {
                v.topic = fach_topic(auto);
            }
        }
    }
    fn duel_hud(&mut self, inst: u32, now: i64) {
        let Some(d) = self.duels.get_mut(&inst) else { return };
        if d.phase == Phase::Ende {
            return;
        }
        d.last_hud = now;
        let v = d.hud(now);
        self.inst_hud(inst, v);
    }

    /// Nur aufgerufen, wenn Game.duels nicht leer ist.
    pub fn duel_tick(&mut self, now: i64) {
        let ids: Vec<u32> = self.duels.keys().copied().collect();
        for inst in ids {
            if self.inst(inst).is_none() {
                self.duels.remove(&inst);
                continue;
            }
            let Some(mut d) = self.duels.remove(&inst) else { continue };
            let (close, hurts) = self.duel_step(&mut d, now);
            self.duels.insert(inst, d);
            for id in hurts {
                self.hurt_player(id, HURT, now);
            }
            if close {
                self.inst_close(inst);
            }
        }
    }
    /// Ein Takt eines Duells (das Duell ist dabei aus `self.duels` herausgenommen). Liefert (schließen, verletzte Spieler).
    fn duel_step(&mut self, d: &mut Duel, now: i64) -> (bool, Vec<u32>) {
        let inst = d.inst;
        let mut hurts = Vec::new();
        match d.phase {
            Phase::Roulette if now < d.until => self.duel_lamps(inst, Some(((now - d.start).max(0) / 100) as usize)),
            Phase::Aufloesung if !d.dropped && now >= d.start + DROP_MS => {
                d.dropped = true;
                let c = d.correct;
                self.duel_quads(inst, |k| if k == c { world::QUAD_RICHTIG } else { 0 });
            }
            _ => {}
        }
        if now < d.until {
            if matches!(d.phase, Phase::Lobby | Phase::Laden | Phase::Frage) && now - d.last_hud >= 1000 {
                d.last_hud = now;
                let v = d.hud(now);
                self.inst_hud(inst, v);
            }
            return (false, hurts);
        }
        match d.phase {
            Phase::Lobby => self.duel_lock(d, now),
            Phase::Roulette => {
                self.duel_lamps(inst, None);
                self.duel_after_lock(d, now);
            }
            Phase::Laden => {
                if d.waiting || d.qs.len() < ROUNDS {
                    let (thema, fach) = d.topic.as_ref().map_or((FACH_NAMEN[0].to_string(), 0), |t| (t.thema.clone(), t.fach));
                    let more = self.questions_now(&thema, fach);
                    fill_up(&mut d.qs, more);
                    d.waiting = false;
                }
                self.duel_frage(d, now);
            }
            Phase::Frage => hurts = self.duel_resolve(d, now),
            Phase::Aufloesung => {
                self.duel_quads(inst, |k| world::QUADS[k as usize]);
                d.runde += 1;
                if d.runde >= d.qs.len().min(ROUNDS) {
                    self.duel_end(d, now);
                } else {
                    self.duel_frage(d, now);
                }
            }
            Phase::Ende => return (true, hurts),
        }
        if d.phase != Phase::Ende {
            d.last_hud = now;
            let v = d.hud(now);
            self.inst_hud(inst, v);
        }
        (false, hurts)
    }
    /// Ende der Lobby: Bot dazu, Thema festlegen, Fragen anfordern (Sperre).
    fn duel_lock(&mut self, d: &mut Duel, now: i64) {
        let humans = d.humans();
        d.solo_round = d.solo || humans <= 1;
        if humans <= 1 && !d.seats.iter().any(|s| s.bot) {
            if let Some(farbe) = d.free_farbe() {
                d.seats.push(Seat { id: BOT_ID, name: BOT_NAME.into(), farbe, bot: true, gone: false, punkte: 0, serie: 0, richtig: 0, feet: None, stand: None, klick: None });
            }
        }
        let votes: Vec<Topic> = d.votes.iter().map(|v| v.topic.clone()).collect();
        let (t, kand) = resolve_votes(&votes, d.auto, (now as u64) ^ (d.inst as u64).wrapping_mul(0x9E37_79B9));
        d.sperre = now;
        d.waiting = self.questions_request(d.inst, &t.thema, &t.kontext);
        if !d.waiting {
            d.qs = self.questions_now(&t.thema, t.fach);
        }
        d.topic = Some(t);
        if kand.is_empty() {
            self.duel_after_lock(d, now);
        } else {
            d.kandidaten = kand;
            d.phase = Phase::Roulette;
            d.start = now;
            d.until = now + ROULETTE_MS;
        }
    }
    fn duel_after_lock(&mut self, d: &mut Duel, now: i64) {
        if d.waiting {
            d.phase = Phase::Laden;
            d.start = now;
            // LAW 1: nie später als Sperre + 6,5 s
            d.until = (d.sperre + TIMEOUT_MS as i64).max(now);
        } else {
            self.duel_frage(d, now);
        }
    }
    fn duel_frage(&mut self, d: &mut Duel, now: i64) {
        let inst = d.inst;
        d.phase = Phase::Frage;
        d.start = now;
        d.until = now + FRAGE_MS;
        d.schnellster = 0;
        d.dropped = false;
        let correct = d.qs.get(d.runde).map_or(0, |q| q.correct.min(3));
        for s in d.seats.iter_mut() {
            s.stand = None;
            s.klick = None;
            s.feet = None;
            if s.bot {
                let hit = self.rng.below(100) < 60;
                let a = if hit { correct } else { (correct + 1 + self.rng.below(3) as u8) % 4 };
                s.klick = Some((a, now + 2000 + self.rng.below(8000) as i64));
            } else if let (Some(p), Some(it)) = (self.players.get(&s.id), self.instances.list.get(&inst)) {
                s.feet = feet_quad(it, p.p);
            }
        }
    }
    /// 0 s: Antworten werten, Felder färben, Punkte, Schneeball des Schnellsten.
    fn duel_resolve(&mut self, d: &mut Duel, now: i64) -> Vec<u32> {
        let inst = d.inst;
        let mut hurts = Vec::new();
        let Some(c) = d.qs.get(d.runde).map(|q| q.correct.min(3)) else { return hurts };
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return hurts };
        let (start, until) = (d.start, d.until);
        let mut best: Option<(i64, usize)> = None;
        for (i, s) in d.seats.iter_mut().enumerate() {
            if s.gone {
                continue;
            }
            let feet = if s.bot { None } else { self.players.get(&s.id).and_then(|p| self.instances.list.get(&inst).and_then(|it| feet_quad(it, p.p))) };
            // (Feld, Zeit, zählt fürs Tempo); die jüngste Handlung gilt
            let stand = feet.map(|f| match s.stand {
                Some((k, t)) if k == f => (f, t, true),
                _ => (f, start, false),
            });
            let klick = s.klick.filter(|k| k.1 <= until).map(|(k, t)| (k, t, true));
            let ans = match (stand, klick) {
                (Some(a), Some(k)) => Some(if k.1 >= a.1 { k } else { a }),
                (a, k) => a.or(k),
            };
            match ans {
                Some((a, t, tempo)) if a == c => {
                    let bonus = if tempo { (50 * (until - t).clamp(0, FRAGE_MS) / FRAGE_MS) as u32 } else { 0 };
                    s.punkte += 100 + bonus + 10 * s.serie;
                    s.serie += 1;
                    s.richtig += 1;
                    if tempo && best.is_none_or(|(bt, _)| t < bt) {
                        best = Some((t, i));
                    }
                }
                _ => {
                    s.serie = 0;
                    if feet.is_some_and(|f| f != c) {
                        hurts.push(s.id);
                    }
                }
            }
        }
        d.correct = c;
        d.schnellster = 0;
        if let Some((_, i)) = best {
            d.seats[i].punkte += 25;
            d.schnellster = d.seats[i].id;
            let from = self.seat_pos(o, &d.seats[i]);
            let to: Vec<[f32; 3]> = d.seats.iter().enumerate().filter(|(j, s)| *j != i && !s.gone).map(|(_, s)| self.seat_pos(o, s)).collect();
            for t in to {
                self.inst_send(inst, &S2C::Shot { from, to: t, kind: 2 });
            }
            // Rückstoß (harmlos, nach der Wertung): menschliche Gegner 1,5 Blöcke vom Werfer weg, aber im Feld, auf dem sie stehen
            let pushes: Vec<(u32, [f32; 3], f32)> = d
                .seats
                .iter()
                .enumerate()
                .filter(|(j, s)| *j != i && !s.gone && !s.bot)
                .filter_map(|(_, s)| {
                    let p = self.players.get(&s.id)?;
                    let k = feet_quad(self.instances.list.get(&inst)?, p.p)?;
                    let (px, pz) = plat(o, s.farbe);
                    Some((s.id, knock(from, p.p, quad_at(px, pz, k)), p.yaw))
                })
                .collect();
            for (id, p, yaw) in pushes {
                self.inst_goto(id, p, yaw);
            }
        }
        self.duel_quads(inst, |k| if k == c { world::QUAD_RICHTIG } else { world::QUAD_FALSCH });
        self.duel_columns(inst, d);
        d.phase = Phase::Aufloesung;
        d.start = now;
        d.until = now + AUFL_MS;
        d.dropped = false;
        hurts
    }
    fn seat_pos(&self, o: (i32, i32), s: &Seat) -> [f32; 3] {
        let p = if s.bot { spawn(o, s.farbe) } else { self.players.get(&s.id).map_or(spawn(o, s.farbe), |p| p.p) };
        [p[0], p[1] + 1.5, p[2]]
    }
    /// Alle Antwort-Felder aller vier Plattformen: Feld k bekommt `b(k)`.
    fn duel_quads(&mut self, inst: u32, b: impl Fn(u8) -> u8) {
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return };
        for f in 0..4u8 {
            let (px, pz) = plat(o, f);
            for k in 0..4u8 {
                let (x0, z0) = quad_at(px, pz, k);
                let blk = b(k);
                for z in z0..z0 + 4 {
                    for x in x0..x0 + 4 {
                        self.inst_set(inst, x, ins::ARENA_Y, z, blk);
                    }
                }
            }
        }
    }
    /// Punktesäulen der Anzeigetafel (eine Reihe je 150 Punkte, höchstens 10).
    fn duel_columns(&mut self, inst: u32, d: &Duel) {
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return };
        for s in &d.seats {
            let h = s.punkte.div_ceil(150).min(10) as i32;
            let x0 = o.0 + BX0 + 3 + 7 * s.farbe as i32;
            for r in 0..10 {
                let b = if r < h { COLUMN[s.farbe as usize % 4] } else { 0 };
                for x in x0..x0 + 4 {
                    self.inst_set(inst, x, ins::ARENA_Y + 2 + r, o.1 + BZ + 1, b);
                }
            }
        }
    }
    /// Lampen der Tafel: `Some(k)` = Lauflicht (Stichwahl), `None` = alle an.
    fn duel_lamps(&mut self, inst: u32, k: Option<usize>) {
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return };
        let ls = lamps(o);
        let n = ls.len().max(1);
        for (i, (x, y, z)) in ls.into_iter().enumerate() {
            let b = match k {
                Some(k) if k % n != i => world::NEON_PINK,
                _ => world::GLUEH,
            };
            self.inst_set(inst, x, y, z, b);
        }
    }
    /// Ende: Belohnung und Wertung in einer Transaktion, Hud `ende` je Spieler.
    fn duel_end(&mut self, d: &mut Duel, now: i64) {
        d.phase = Phase::Ende;
        d.start = now;
        d.until = now + ENDE_MS;
        let sieger = d.seats.iter().enumerate().filter(|(_, s)| !s.gone).max_by_key(|(i, s)| (s.punkte, !s.bot, Reverse(*i))).map_or(0, |(_, s)| s.id);
        let humans: Vec<(u32, u32, u32)> = d.seats.iter().filter(|s| !s.bot && !s.gone).map(|s| (s.id, s.richtig, s.punkte)).collect();
        let rated = humans.len() >= 2;
        let solo = d.solo_round || humans.len() < 2;
        let mult = self.taler_mult();
        let fach = d.topic.as_ref().map_or(4, |t| if t.fach > 3 { 4 } else { t.fach });
        let item = sb::SHARD + fach as u16;
        let res = self.tx(|g| {
            let mut olds = Vec::new();
            for h in &humans {
                olds.push(g.db.query_row("select elo from duel_elo where user = ?", [h.0], |r| r.get::<_, i64>(0)).optional()?.unwrap_or(1000));
            }
            let pts: Vec<u32> = humans.iter().map(|h| h.2).collect();
            let mut out = Vec::new();
            for (i, &(uid, richtig, _)) in humans.iter().enumerate() {
                let win = uid == sieger;
                let mut taler = (10 + 3 * richtig as i64 + if win { 20 } else { 0 }) * mult;
                if solo {
                    taler /= 2;
                }
                g.pay(uid, taler, "duell")?;
                let n = richtig / 2 + if win { 2 } else { 0 };
                let total = if n > 0 { Some(g.gain(uid, item, n)?) } else { None };
                g.fuel(uid, (if win { 30 } else { 10 }) * 60_000)?;
                let elo = if rated {
                    let neu = elo_new(&olds, &pts, i);
                    g.db.execute(
                        "insert into duel_elo (user, elo, siege, spiele) values (?1, ?2, ?3, 1) on conflict (user) do update set elo = excluded.elo, siege = siege + excluded.siege, spiele = spiele + 1",
                        params![uid, neu, win as i64],
                    )?;
                    Some((olds[i], neu))
                } else {
                    None
                };
                out.push((uid, taler, n, total, elo));
            }
            Ok(out)
        });
        let out = res.unwrap_or_else(|e| {
            eprintln!("duel: Belohnung fehlgeschlagen: {}", e.1);
            humans.iter().map(|h| (h.0, 0, 0, None, None)).collect()
        });
        self.duel_columns(d.inst, d);
        let spieler = d.spieler();
        for (uid, taler, n, total, elo) in out {
            if let Some(t) = total {
                self.send(uid, &S2C::Inv { item, n: t });
            }
            let elo = elo.map_or(Value::Null, |(a, b)| json!({ "alt": a, "neu": b }));
            self.hud(uid, json!({ "t": "duell", "phase": "ende", "spieler": spieler, "sieger": sieger, "belohnung": { "taler": taler, "splitter": n, "item": item }, "elo": elo }));
        }
    }

    pub fn duel_move(&mut self, id: u32, now: i64) {
        let Some(p) = self.players.get(&id).map(|p| p.p) else { return };
        let Some((inst, _)) = self.inst_of(id) else { return };
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return };
        let Some(d) = self.duels.get_mut(&inst) else { return };
        let Some(s) = d.seats.iter_mut().find(|s| s.id == id) else { return };
        if p[1] < ins::ARENA_FALL_Y {
            // Absturz: zurück auf die eigene Plattform, ohne Verlust
            let at = spawn(o, s.farbe);
            s.feet = None;
            return self.inst_goto(id, at, 0.0);
        }
        if d.phase != Phase::Frage {
            return;
        }
        let q = self.instances.list.get(&inst).and_then(|it| feet_quad(it, p));
        let Some(s) = self.duels.get_mut(&inst).and_then(|d| d.seats.iter_mut().find(|s| s.id == id)) else { return };
        if q != s.feet {
            s.feet = q;
            if let Some(q) = q {
                s.stand = Some((q, now));
            }
        }
    }
    /// Spieler verlässt die Arena (er ist noch Mitglied, die Instanz steht noch). Nicht inst_leave(id) aufrufen.
    pub fn duel_left(&mut self, inst: u32, id: u32) {
        let now = self.now();
        let Some(d) = self.duels.get_mut(&inst) else { return };
        d.votes.retain(|v| v.id != id);
        if d.phase == Phase::Lobby {
            d.seats.retain(|s| s.id != id);
        } else if let Some(s) = d.seats.iter_mut().find(|s| s.id == id) {
            s.gone = true;
        }
        self.duel_auto(inst);
        self.duel_hud(inst, now);
    }
    pub fn duel_died(&mut self, id: u32, _now: i64) {
        let Some((inst, _)) = self.inst_of(id) else { return };
        let Some(o) = self.inst(inst).map(|i| i.origin()) else { return };
        let farbe = self.duels.get(&inst).and_then(|d| d.seats.iter().find(|s| s.id == id)).map_or(0, |s| s.farbe);
        self.inst_goto(id, spawn(o, farbe), 0.0);
    }
    /// POST duell/wahl {art: "auto"|"fach"|"frei", fach?, thema?, kontext?}; bei "frei" ist `fach` (0..3) nur die Kategorie für Ersatzfragen.
    pub fn duel_vote(&mut self, id: u32, b: &Value) -> R<Value> {
        let Some((inst, ins::KIND_ARENA)) = self.inst_of(id) else { return no(409, "Du bist in keinem Duell.") };
        let Some(d) = self.duels.get(&inst) else { return no(404, "Diese Runde gibt es nicht mehr.") };
        if d.phase != Phase::Lobby {
            return no(409, "Die Themenwahl ist schon vorbei.");
        }
        let (art, topic) = match b.get("art").and_then(Value::as_str).unwrap_or("") {
            "auto" => ("auto", fach_topic(d.auto)),
            "fach" => match b.get("fach").and_then(Value::as_u64) {
                Some(f) if f <= 3 => ("fach", fach_topic(f as u8)),
                _ => return no(400, "Das Fach muss 0 bis 3 sein."),
            },
            "frei" => {
                let t = normalize_topic(b.get("thema").and_then(Value::as_str).unwrap_or(""));
                if t.is_empty() {
                    return no(400, "Bitte ein Wunsch-Thema eingeben.");
                }
                let kontext: String = crate::sky::clean(b.get("kontext").and_then(Value::as_str).unwrap_or("")).chars().take(400).collect();
                let fach = b.get("fach").and_then(Value::as_u64).filter(|&f| f <= 3).map_or(255, |f| f as u8);
                ("frei", Topic { fach, thema: t, kontext })
            }
            _ => return no(400, "Unbekannte Wahl (auto, fach oder frei)."),
        };
        let thema = topic.thema.clone();
        // Freie Themen: KI-Fragen schon während der Lobby anfragen (40 s), damit sie zum Start im Cache liegen
        let vorab = art == "frei" && self.duels.get(&inst).is_some_and(|d| !d.vorab.contains(&thema));
        let kontext = topic.kontext.clone();
        if let Some(d) = self.duels.get_mut(&inst) {
            d.votes.retain(|v| v.id != id);
            d.votes.push(Vote { id, art, topic });
        }
        if vorab && self.questions_request(inst, &thema, &kontext) {
            if let Some(d) = self.duels.get_mut(&inst) {
                d.vorab.push(thema.clone());
            }
        }
        let now = self.now();
        self.duel_hud(inst, now);
        Ok(json!({ "ok": true, "thema": thema }))
    }
    /// POST duell/antwort {feld: 0..3} – zählt wie Draufstehen.
    pub fn duel_answer(&mut self, id: u32, b: &Value) -> R<Value> {
        let Some(feld) = b.get("feld").and_then(Value::as_u64).filter(|&f| f <= 3) else { return no(400, "Das Feld muss 0 bis 3 sein.") };
        let Some((inst, ins::KIND_ARENA)) = self.inst_of(id) else { return no(409, "Du bist in keinem Duell.") };
        let now = self.now();
        let Some(d) = self.duels.get_mut(&inst) else { return no(404, "Diese Runde gibt es nicht mehr.") };
        if d.phase != Phase::Frage || now > d.until {
            return no(409, "Gerade läuft keine Frage.");
        }
        let Some(s) = d.seats.iter_mut().find(|s| s.id == id) else { return no(409, "Du spielst in dieser Runde nicht mit.") };
        s.klick = Some((feld as u8, now));
        Ok(json!({ "ok": true, "feld": feld }))
    }
    /// GET duell/rangliste
    pub fn duel_ranking(&mut self, uid: u32) -> R<Value> {
        let spieler: Vec<Value> = self
            .db
            .prepare("select coalesce(u.name, ''), e.elo, e.siege, e.spiele from duel_elo e left join users u on u.id = e.user order by e.elo desc, e.siege desc, e.user limit 10")?
            .query_map([], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "elo": r.get::<_, i64>(1)?, "siege": r.get::<_, i64>(2)?, "spiele": r.get::<_, i64>(3)? })))?
            .collect::<Result<_, _>>()?;
        let ich: Option<Value> = self
            .db
            .query_row("select elo, siege, spiele from duel_elo where user = ?", [uid], |r| Ok(json!({ "elo": r.get::<_, i64>(0)?, "siege": r.get::<_, i64>(1)?, "spiele": r.get::<_, i64>(2)? })))
            .optional()?;
        Ok(json!({ "spieler": spieler, "ich": ich }))
    }
    /// KI-Fragen sind angekommen (duel_ai::job_done ruft das; `duel` kann schon weg sein). Zu spät = verworfen.
    pub fn duel_questions(&mut self, duel: u32, qs: Vec<Question>) {
        let Some(d) = self.duels.get(&duel) else { return };
        if !d.waiting {
            return;
        }
        let (thema, fach) = d.topic.as_ref().map_or((FACH_NAMEN[0].to_string(), 0), |t| (t.thema.clone(), t.fach));
        let mut qs = qs;
        if qs.len() < ROUNDS {
            let more = self.questions_now(&thema, fach);
            fill_up(&mut qs, more);
        }
        qs.truncate(ROUNDS);
        let now = self.now();
        if let Some(d) = self.duels.get_mut(&duel) {
            d.qs = qs;
            d.waiting = false;
            if d.phase == Phase::Laden {
                d.until = now; // der nächste Takt startet die erste Frage
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;

    fn t(fach: u8, thema: &str) -> Topic {
        Topic { fach, thema: thema.into(), kontext: String::new() }
    }
    fn seat(g: &crate::game::Game, inst: u32, id: u32) -> (u8, u32) {
        let s = g.duels[&inst].seats.iter().find(|s| s.id == id).unwrap();
        (s.farbe, s.punkte)
    }
    fn tick(g: &mut crate::game::Game, ms: i64) {
        g.clock_offset += ms;
        let now = g.now();
        g.duel_tick(now);
    }

    #[test]
    fn themenwahl_mehrheit_und_stichwahl() {
        let m = fach_topic(0);
        let mut f = t(255, "photosynthese");
        f.kontext = "Lichtreaktion, Calvin-Zyklus".into();
        // Mehrheit sofort; Kontext der Sieger-Stimme bleibt erhalten
        let (w, k) = resolve_votes(&[fach_topic(0), fach_topic(0), f.clone()], 3, 9);
        assert_eq!((w.fach, k.len()), (0, 0));
        let (w, _) = resolve_votes(&[f.clone(), f.clone(), m.clone()], 0, 1);
        assert_eq!(w.kontext, f.kontext);
        // keine Stimme: Auto-Fach
        assert_eq!(resolve_votes(&[], 2, 5), (fach_topic(2), vec![]));
        // Gleichstand: gleicher Seed → gleiches Ergebnis, jeder Kandidat erreichbar
        let votes = [f.clone(), m.clone(), fach_topic(3)];
        assert_eq!(resolve_votes(&votes, 0, 42), resolve_votes(&votes, 0, 42));
        let (_, k) = resolve_votes(&votes, 0, 0);
        assert_eq!(k.len(), 3);
        for c in &k {
            assert!((0..20u64).any(|s| resolve_votes(&votes, 0, s).0 == *c), "{c:?} nie gewählt");
        }
    }

    #[test]
    fn auto_fach_und_elo() {
        assert_eq!(auto_fach(&[]), 0);
        assert_eq!(auto_fach(&[[5, 3, 4, 9, 0, 0, 0], [5, 3, 1, 1, 9, 9, 9]]), 2);
        assert_eq!(auto_fach(&[[2, 2, 2, 2, 1, 1, 1]]), 0, "Gleichstand: kleinster Index");
        assert_eq!(elo_new(&[1000, 1000], &[300, 100], 0), 1012);
        assert_eq!(elo_new(&[1000, 1000], &[300, 100], 1), 988);
        assert_eq!(elo_new(&[1000, 1000], &[100, 100], 0), 1000);
    }

    #[test]
    fn solo_runde_komplett() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Anna", false);
        let vorher = g.taler(1);
        g.duel_cmd(1, "solo").unwrap();
        let (inst, kind) = g.inst_of(1).unwrap();
        assert_eq!((kind, g.duels.len()), (ins::KIND_ARENA, 1));
        let o = g.inst(inst).unwrap().origin();
        let (farbe, _) = seat(&g, inst, 1);
        let p = g.players[&1].p;
        assert_eq!(g.inst(inst).unwrap().get(p[0].floor() as i32, ins::ARENA_Y, p[2].floor() as i32), world::GLAS, "Ankunft auf dem Glasrand");
        for k in 0..4u8 {
            let c = quad_center(o, farbe, k);
            assert_eq!(g.inst(inst).unwrap().get(c[0].floor() as i32, ins::ARENA_Y, c[2].floor() as i32), world::QUADS[k as usize]);
        }
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "duell" && h["phase"] == "lobby"));
        assert!(g.duel_vote(1, &json!({ "art": "frei", "thema": "   " })).is_err());
        assert!(g.duel_vote(1, &json!({ "art": "fach", "fach": 7 })).is_err());
        g.duel_vote(1, &json!({ "art": "fach", "fach": 0 })).unwrap();
        // Absturz kostet keine Taler
        put(&mut g, 1, [p[0], 20.0, p[2]]);
        let now = g.now();
        g.duel_move(1, now);
        assert!(g.players[&1].p[1] > ins::ARENA_FALL_Y);
        assert_eq!(g.taler(1), vorher);
        // Lobby endet: ohne Schlüssel und Laufzeit sofort Fragen aus der Bank, Bot spielt mit
        tick(&mut g, LOBBY_MS + 1);
        assert_eq!(g.duels[&inst].phase, Phase::Frage);
        assert!(g.duels[&inst].seats.iter().any(|s| s.bot));
        assert_eq!(g.duels[&inst].qs.len(), ROUNDS);
        for runde in 0..ROUNDS {
            let c = g.duels[&inst].qs[runde].correct;
            if runde % 2 == 0 {
                let at = quad_center(o, farbe, c);
                put(&mut g, 1, at);
                let now = g.now();
                g.duel_move(1, now);
            } else {
                g.duel_answer(1, &json!({ "feld": c })).unwrap();
            }
            let (_, vor) = seat(&g, inst, 1);
            tick(&mut g, FRAGE_MS + 1);
            assert_eq!(g.duels[&inst].phase, Phase::Aufloesung);
            assert!(seat(&g, inst, 1).1 >= vor + 100, "Runde {runde}: richtig gibt Punkte");
            let cc = quad_center(o, farbe, c);
            let w = quad_center(o, farbe, (c + 1) % 4);
            assert_eq!(g.inst(inst).unwrap().get(cc[0].floor() as i32, ins::ARENA_Y, cc[2].floor() as i32), world::QUAD_RICHTIG);
            tick(&mut g, DROP_MS + 100);
            assert_eq!(g.inst(inst).unwrap().get(w[0].floor() as i32, ins::ARENA_Y, w[2].floor() as i32), 0, "falsches Feld fällt weg");
            tick(&mut g, AUFL_MS);
        }
        assert_eq!(g.duels[&inst].phase, Phase::Ende);
        assert!(huds(&mut rx).iter().any(|h| h["phase"] == "ende" && h["belohnung"]["taler"].as_i64().unwrap_or(0) > 0));
        tick(&mut g, ENDE_MS + 1);
        assert!(g.duels.is_empty() && g.instances.list.is_empty());
        assert_eq!(g.players[&1].inst, None);
        assert!(g.taler(1) > vorher);
        // Solo: keine Wertung
        assert_eq!(g.duel_ranking(1).unwrap()["ich"], Value::Null);
    }

    #[test]
    fn einladung_und_trennung_raeumen_ab() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        let mut b = join(&mut g, 2, "Ben", false);
        g.duel_cmd(1, "Ben").unwrap();
        assert!(huds(&mut b).iter().any(|h| h["t"] == "duell_einladung" && h["von"] == "Anna"));
        g.duel_cmd(2, "join anna").unwrap();
        let inst = g.inst_of(1).unwrap().0;
        assert_eq!(g.inst_of(2).map(|x| x.0), Some(inst));
        assert_ne!(seat(&g, inst, 1).0, seat(&g, inst, 2).0, "eigene Plattform");
        tick(&mut g, LOBBY_MS + 1);
        assert_eq!(g.duels[&inst].phase, Phase::Frage);
        assert!(!g.duels[&inst].seats.iter().any(|s| s.bot), "zwei Menschen: kein Bot");
        g.drop_player(1);
        assert!(!g.duels.is_empty());
        tick(&mut g, FRAGE_MS + 1);
        g.drop_player(2);
        assert!(g.duels.is_empty() && g.instances.list.is_empty());
        // Solo-Runde mitten in der Frage getrennt
        let _c = join(&mut g, 3, "Cleo", false);
        g.duel_cmd(3, "").unwrap();
        tick(&mut g, LOBBY_MS + 1);
        g.drop_player(3);
        assert!(g.duels.is_empty() && g.instances.list.is_empty());
    }

    #[test]
    fn wertung_bei_zwei_menschen() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        let _b = join(&mut g, 2, "Ben", false);
        g.duel_cmd(1, "").unwrap();
        g.duel_cmd(2, "").unwrap();
        let inst = g.inst_of(1).unwrap().0;
        tick(&mut g, LOBBY_MS + 1);
        // nur Anna antwortet richtig
        for _ in 0..ROUNDS {
            let c = g.duels[&inst].qs[g.duels[&inst].runde].correct;
            g.duel_answer(1, &json!({ "feld": c })).unwrap();
            tick(&mut g, FRAGE_MS + 1);
            tick(&mut g, AUFL_MS + 1);
        }
        assert_eq!(g.duels[&inst].phase, Phase::Ende);
        let r = g.duel_ranking(1).unwrap();
        assert_eq!((r["ich"]["elo"].as_i64(), r["ich"]["siege"].as_i64()), (Some(1012), Some(1)));
        assert_eq!(r["spieler"][0]["name"], "Anna");
        assert_eq!(g.duel_ranking(2).unwrap()["ich"]["elo"].as_i64(), Some(988));
    }

    #[test]
    fn ki_zeitueberschreitung_faellt_zurueck() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        g.duel_cmd(1, "solo").unwrap();
        let inst = g.inst_of(1).unwrap().0;
        tick(&mut g, LOBBY_MS + 1);
        // steht für einen laufenden KI-Auftrag, der nie zurückkommt (ohne tokio-Laufzeit)
        let sperre = g.now();
        let off0 = g.clock_offset; // gemessen wird die simulierte Zeit; echte Laufzeit (ms-Takt) machte den Test wackelig
        {
            let d = g.duels.get_mut(&inst).unwrap();
            d.waiting = true;
            d.qs.clear();
            d.runde = 0;
            d.phase = Phase::Laden;
            d.start = sperre;
            d.sperre = sperre;
            d.until = sperre + TIMEOUT_MS as i64;
        }
        tick(&mut g, TIMEOUT_MS as i64 - 100);
        assert_eq!(g.duels[&inst].phase, Phase::Laden, "wartet, blockiert aber nichts");
        tick(&mut g, 100);
        assert!(g.clock_offset - off0 <= 6_500, "LAW 1: spätestens Sperre + 6,5 s");
        let d = &g.duels[&inst];
        assert_eq!((d.phase, d.qs.len(), d.waiting), (Phase::Frage, ROUNDS, false));
        let vorher: Vec<String> = d.qs.iter().map(|q| q.q.clone()).collect();
        let thema = d.topic.as_ref().map_or(String::new(), |t| t.thema.clone());
        // zu spät angekommene KI-Fragen werden verworfen
        let spaet = (0..ROUNDS).map(|i| Question { q: format!("Spät {i}?"), options: ["a", "b", "c", "d"].map(String::from), correct: 0, explanation: String::new() }).collect();
        g.job_done(crate::duel_ai::Job::Fragen { duel: inst, thema, kontext: String::new(), res: Ok(spaet) });
        let d = &g.duels[&inst];
        assert_eq!(d.qs.iter().map(|q| q.q.clone()).collect::<Vec<_>>(), vorher);
        assert_eq!(d.phase, Phase::Frage);
    }

    #[test]
    fn schneeball_stoesst_und_felder_drei_sekunden_weg() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        let mut b = join(&mut g, 2, "Ben", false);
        g.duel_cmd(1, "").unwrap();
        g.duel_cmd(2, "").unwrap();
        let inst = g.inst_of(1).unwrap().0;
        let o = g.inst(inst).unwrap().origin();
        tick(&mut g, LOBBY_MS + 1);
        let c = g.duels[&inst].qs[0].correct;
        g.duel_answer(1, &json!({ "feld": c })).unwrap();
        let w = (c + 1) % 4;
        let at = quad_center(o, seat(&g, inst, 2).0, w);
        put(&mut g, 2, at);
        let now = g.now();
        g.duel_move(2, now);
        msgs(&mut b);
        tick(&mut g, FRAGE_MS + 1);
        assert_eq!(g.duels[&inst].schnellster, 1);
        let p = g.players[&2].p;
        let m = msgs(&mut b);
        assert!(m.iter().any(|m| matches!(m, S2C::Shot { kind: 2, .. })));
        assert!(m.iter().any(|m| matches!(m, S2C::Spawn { .. })), "Rückstoß versetzt");
        assert!(((p[0] - at[0]).powi(2) + (p[2] - at[2]).powi(2)).sqrt() >= 1.0, "mindestens 1 Block weg");
        let (px, pz) = plat(o, seat(&g, inst, 2).0);
        let (qx, qz) = quad_at(px, pz, w);
        assert!((qx as f32..qx as f32 + 4.0).contains(&p[0]) && (qz as f32..qz as f32 + 4.0).contains(&p[2]), "bleibt auf seinem Feld");
        // falsche Felder: ab 0,6 s volle 3 s Luft
        let x = |g: &crate::game::Game| g.inst(inst).unwrap().get(at[0].floor() as i32, ins::ARENA_Y, at[2].floor() as i32);
        tick(&mut g, DROP_MS + 1);
        assert_eq!(x(&g), 0);
        tick(&mut g, 2_900);
        assert_eq!((x(&g), g.duels[&inst].phase), (0, Phase::Aufloesung), "nach 3,5 s noch Luft");
        tick(&mut g, 200);
        assert_eq!(x(&g), world::QUADS[w as usize]);
    }
}
