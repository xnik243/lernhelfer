//! Phase 24 (Besitz: Agent S-KATA, Vertrag CONTRACT-21-25.md §S-KATA): Prüfungs-Katakomben – Lauf, Wesen, Rätsel, Note, Beute.
//! Reine Rechnung (Generator, Rätsel, Note, Beute) steht in catacombs_gen.rs. LAW 2: alles hängt an `Dungeon` bzw. der
//! Instanz und verschwindet mit ihr; ohne Lauf ruft der Takt hier nichts auf.
use crate::catacombs_gen::{self as gen, Layout, K_BOSS, K_KAMPF, K_RUNEN, K_SCHATZ, K_SPIEGEL, K_START, K_WEISEN};
use crate::game::Game;
use crate::instance::{Inst, MAX_MEMBERS};
use crate::mobs::Mob;
use crate::{no, R};
use campus_shared::combat as cb;
use campus_shared::instance as ins;
use campus_shared::proto::{MobState, MOB_DEAD, MOB_HURT, S2C};
use campus_shared::{items, rpg, world};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Wesen-Nummern der Katakomben beginnen hier (nie gleich mit Zitadellen-Wesen).
const MOB_BASE: u32 = 1_000_000;
const BOSS: u8 = 7;
const RUN_MS: i64 = 30 * 60 * 1000;
const AFTER_BOSS_MS: i64 = 60_000;
const RUNE_MS: i64 = 600;
const SEGEN: f32 = 1.25;
const SAGE_DMG: f32 = 30.0;
const MAX_MINIONS: usize = 3;
type Set = ([i32; 3], u8);

/// Ein laufender Katakomben-Lauf (Schlüssel in Game.active_dungeons = Instanz-Nummer).
pub struct Dungeon {
    pub inst: u32,
    pub stufe: u8,
    /// Name, über den andere beitreten (/dungeon join <Name>)
    pub leader: String,
    pub leader_id: u32,
    pub lay: Layout,
    /// Nordwest-Ecke des Feldes (Welt x, z)
    pub origin: (i32, i32),
    pub start: i64,
    /// Leben/Schaden der Wesen × (1 + 0,35·(Stufe−1))
    pub mult: f32,
    pub left_start: bool,
    pub geheim: bool,
    pub tode: u32,
    /// zuletzt betretener Raum je Mitglied
    pub room_of: HashMap<u32, usize>,
    text: Option<(String, i64)>,
    hud_at: i64,
    last: i64,
    pub mobs: Vec<Mob>,
    next_mob: u32,
    mobs_shown: bool,
    pub cleared: [bool; 9],
    pub active: [bool; 9],
    waves_left: [u8; 9],
    /// Strahl-Zellen (Innenraum der Spiegel-Kammer), wie sie gerade stehen
    beam: Vec<(i32, i32)>,
    pub mirror_done: bool,
    rune_prog: usize,
    rune_show: Option<i64>,
    rune_shown: bool,
    rune_lit: Option<usize>,
    rune_last: HashMap<u32, usize>,
    pub rune_done: bool,
    /// 0 offen, 1 gelöst, 2 verloren
    pub sages: u8,
    pub segen: bool,
    pub boss_open: bool,
    boss_dmg: Vec<(u32, f32)>,
    pub boss_dead_at: i64,
    minion_at: i64,
    /// Note, Punkte, Zeit – fest ab dem Sieg über den Endboss
    pub result: Option<(&'static str, [u32; 3], i64)>,
    got_altar: Vec<u32>,
    got_smaragd: Vec<u32>,
    got_obsidian: Vec<u32>,
    got_schatz: Vec<u32>,
    beute: HashMap<u32, Vec<(u16, u32)>>,
}

fn size_of(kind: u8) -> f32 {
    rpg::mob(kind).map_or(1.0, |d| d.size)
}
fn boss_name() -> &'static str {
    rpg::mob(BOSS).map_or("Der Namenlose Prüfer", |d| d.name)
}
fn hdist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn local_room(o: (i32, i32), p: [f32; 3]) -> Option<usize> {
    gen::room_at(p[0].floor() as i32 - o.0, p[2].floor() as i32 - o.1)
}
/// Darf ein Wesen von Raum `room` hier stehen? (im eigenen Innenraum, Fuß- und Kopfzelle frei)
fn walkable(it: &Inst, o: (i32, i32), room: usize, x: f32, z: f32) -> bool {
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    gen::room_at(bx - o.0, bz - o.1) == Some(room) && !world::solid(it.get(bx, ins::DUNGEON_Y + 1, bz)) && !world::solid(it.get(bx, ins::DUNGEON_Y + 2, bz))
}
fn mob_step(it: &Inst, o: (i32, i32), room: usize, m: &mut Mob, dx: f32, dz: f32) {
    for (sx, sz) in [(dx, dz), (dx, 0.0), (0.0, dz)] {
        if sx == 0.0 && sz == 0.0 {
            continue;
        }
        let (nx, nz) = (m.p[0] + sx, m.p[2] + sz);
        if walkable(it, o, room, nx, nz) {
            m.p[0] = nx;
            m.p[2] = nz;
            return;
        }
    }
}
/// Wesen jagen das nächste Mitglied im selben Raum und schlagen zu (Treffer landen in `hits`).
fn mob_ai(d: &mut Dungeon, it: &Inst, who: &[(u32, [f32; 3])], now: i64, dt: f32, hits: &mut Vec<(u32, f32)>) {
    let (o, mult) = (d.origin, d.mult);
    for m in d.mobs.iter_mut() {
        if m.dead_at > 0 {
            continue;
        }
        let Some(def) = rpg::mob(m.kind) else { continue };
        let Some(room) = local_room(o, m.p) else { continue };
        if m.kb[0] != 0.0 || m.kb[1] != 0.0 {
            let (kx, kz) = (m.kb[0] * dt, m.kb[1] * dt);
            mob_step(it, o, room, m, kx, kz);
            m.kb = [m.kb[0] * 0.7, m.kb[1] * 0.7];
            if m.kb[0].abs() + m.kb[1].abs() < 0.2 {
                m.kb = [0.0, 0.0];
            }
        }
        let mut target: Option<(u32, [f32; 3], f32)> = None;
        for &(id, p) in who {
            if local_room(o, p) != Some(room) {
                continue;
            }
            let dd = hdist(p, m.p);
            if target.map_or(true, |t| dd < t.2) {
                target = Some((id, p, dd));
            }
        }
        let Some((tid, tp, dist)) = target else { continue };
        let (dx, dz) = (tp[0] - m.p[0], tp[2] - m.p[2]);
        let l = dist.max(0.01);
        m.yaw = (-dx).atan2(-dz);
        if dist > def.reach * 0.8 {
            let s = def.speed * dt;
            mob_step(it, o, room, m, dx / l * s, dz / l * s);
        }
        let low = (tp[1] - m.p[1]).abs() < 2.5 + def.size;
        let cd = if m.kind == BOSS && cb::boss_phase(m.hp / m.max.max(1.0)) == 2 { cb::MOB_HIT_CD_MS * 3 / 5 } else { cb::MOB_HIT_CD_MS };
        if hdist(tp, m.p) <= def.reach + 0.3 && low && now - m.hit_at >= cd {
            m.hit_at = now;
            hits.push((tid, def.dmg * mult));
        }
    }
}

impl Dungeon {
    fn new(inst: u32, stufe: u8, leader_id: u32, leader: String, lay: Layout, origin: (i32, i32), now: i64) -> Dungeon {
        let beam = gen::beam(&lay.mirror).0;
        Dungeon {
            inst,
            stufe,
            leader,
            leader_id,
            mult: 1.0 + 0.35 * (stufe.max(1) - 1) as f32,
            lay,
            origin,
            start: now,
            left_start: false,
            geheim: false,
            tode: 0,
            room_of: HashMap::new(),
            text: None,
            hud_at: 0,
            last: 0,
            mobs: Vec::new(),
            next_mob: MOB_BASE,
            mobs_shown: false,
            cleared: [false; 9],
            active: [false; 9],
            waves_left: [0; 9],
            beam,
            mirror_done: false,
            rune_prog: 0,
            rune_show: None,
            rune_shown: false,
            rune_lit: None,
            rune_last: HashMap::new(),
            rune_done: false,
            sages: 0,
            segen: false,
            boss_open: false,
            boss_dmg: Vec::new(),
            boss_dead_at: 0,
            minion_at: 0,
            result: None,
            got_altar: Vec::new(),
            got_smaragd: Vec::new(),
            got_obsidian: Vec::new(),
            got_schatz: Vec::new(),
            beute: HashMap::new(),
        }
    }
    fn room(&self, kind: u8) -> usize {
        gen::find(&self.lay, kind)
    }
    /// Weltkoordinaten der Innenraum-Zelle c von Raum r in der Höhe ly (0 = Boden).
    fn wi(&self, r: usize, c: (i32, i32), ly: i32) -> [i32; 3] {
        let (x, z) = gen::interior(r, c.0, c.1);
        [self.origin.0 + x, ins::DUNGEON_Y + ly, self.origin.1 + z]
    }
    /// Füße auf der Innenraum-Zelle (ix, iz) von Raum r.
    fn feet(&self, r: usize, ix: i32, iz: i32) -> [f32; 3] {
        let w = self.wi(r, (ix, iz), 1);
        [w[0] as f32 + 0.5, w[1] as f32, w[2] as f32 + 0.5]
    }
    fn start_pos(&self) -> [f32; 3] {
        let r = self.room(K_START);
        let (ix, iz) = gen::markers(&self.lay, r, b's').first().copied().unwrap_or((6, 6));
        self.feet(r, ix, iz)
    }
    /// Innenraum-Zelle von Raum r an Welt (x, z), falls dort.
    fn cell_in(&self, r: usize, x: i32, z: i32) -> Option<(i32, i32)> {
        let (lx, lz) = (x - self.origin.0, z - self.origin.1);
        if gen::room_at(lx, lz) != Some(r) {
            return None;
        }
        let (rx, rz) = gen::room_origin(r);
        Some((lx - rx - 1, lz - rz - 1))
    }
    fn rune_w(&self, k: usize) -> [i32; 3] {
        self.wi(self.room(K_RUNEN), gen::rune_cell(k), 0)
    }
    fn solved(&self) -> u32 {
        u32::from(self.mirror_done) + u32::from(self.rune_done) + u32::from(self.sages == 1)
    }
    fn cleared_count(&self) -> usize {
        self.cleared.iter().filter(|&&c| c).count()
    }
    fn alive(&self) -> usize {
        self.mobs.iter().filter(|m| m.dead_at == 0).count()
    }
    fn set_text(&mut self, t: impl Into<String>, now: i64) {
        self.text = Some((t.into(), now + 6000));
    }
    /// Türöffnung zwischen a und b mit `b` füllen.
    fn door_sets(&self, a: usize, b: usize, block: u8) -> Vec<Set> {
        let mut v = Vec::new();
        for (x, z) in gen::door_cells(a, b) {
            for y in 1..=gen::DOOR_H {
                v.push(([self.origin.0 + x, ins::DUNGEON_Y + y, self.origin.1 + z], block));
            }
        }
        v
    }
    /// Gewöhnliche Türen eines Raums schließen (Gitter) oder öffnen; Blut-Portal und Schatz-Gitter bleiben unberührt.
    fn gate_sets(&self, r: usize, closed: bool) -> Vec<Set> {
        let mut v = Vec::new();
        for &(a, b) in &self.lay.doors {
            if (a == r || b == r) && gen::door_fill(&self.lay, a, b) == 0 {
                v.extend(self.door_sets(a, b, if closed { world::KATA_TOR } else { 0 }));
            }
        }
        v
    }
    fn spawn(&mut self, kind: u8, p: [f32; 3]) {
        let Some(def) = rpg::mob(kind) else { return };
        let hp = def.hp * self.mult;
        let id = self.next_mob;
        self.next_mob += 1;
        self.mobs.push(Mob { id, kind, p, hp, max: hp, ..Default::default() });
    }
    fn spawn_wave(&mut self, r: usize) {
        self.waves_left[r] = self.waves_left[r].saturating_sub(1);
        let wave = self.waves_left[r] as usize;
        for (k, (ix, iz)) in gen::markers(&self.lay, r, b'm').into_iter().enumerate() {
            let kind = if (k + wave) % 3 == 2 { 6 } else { 5 };
            let p = self.feet(r, ix, iz);
            self.spawn(kind, p);
        }
    }
    fn waves(&self) -> u8 {
        2 + u8::from(self.stufe >= 4)
    }
    fn punkte(&self, now: i64) -> ([u32; 3], i64) {
        match self.result {
            Some((_, p, ms)) => (p, ms),
            None => {
                let ms = (now - self.start).max(0);
                (gen::score(self.tode, ms, self.solved()), ms)
            }
        }
    }
    fn hud_base(&self, now: i64) -> Value {
        let (p, ms) = self.punkte(now);
        let text = self.text.as_ref().filter(|t| now < t.1).map(|t| t.0.clone());
        json!({ "t": "katakomben", "stufe": self.stufe, "zeit_ms": ms, "tode": self.tode, "raetsel": self.solved(), "geheim": self.geheim,
            "note": gen::grade(p), "punkte": { "koennen": p[0], "tempo": p[1], "raetsel": p[2] }, "segen": self.segen, "wesen": self.alive(), "text": text })
    }
    fn truhen(&self) -> Vec<&'static str> {
        let rank = self.result.map_or(0, |r| gen::grade_rank(r.0));
        let mut v = Vec::new();
        if rank >= 3 {
            v.push("smaragd");
        }
        if rank >= 4 {
            v.push("obsidian");
        }
        v
    }
    fn ende_json(&self, uid: u32, now: i64) -> Value {
        let (p, ms) = self.punkte(now);
        let note = self.result.map_or_else(|| gen::grade(p), |r| r.0);
        let beute: Vec<Value> = self.beute.get(&uid).map(|v| v.iter().map(|&(i, n)| json!([i, n])).collect()).unwrap_or_default();
        json!({ "t": "katakomben", "ende": true, "note": note, "punkte": { "koennen": p[0], "tempo": p[1], "raetsel": p[2] }, "zeit_ms": ms,
            "beute": beute, "truhen": self.truhen() })
    }
}

impl Game {
    /// Instanz-Nummer des Laufs, in dem dieser Spieler steht.
    fn kata_key(&self, id: u32) -> Option<u32> {
        match self.inst_of(id) {
            Some((n, ins::KIND_DUNGEON)) if self.active_dungeons.contains_key(&n) => Some(n),
            _ => None,
        }
    }
    fn kata_sets(&mut self, n: u32, sets: Vec<Set>) {
        for (w, b) in sets {
            self.inst_set(n, w[0], w[1], w[2], b);
        }
    }
    fn kata_members(&self, n: u32) -> Vec<u32> {
        self.inst(n).map(|i| i.members.clone()).unwrap_or_default()
    }
    /// Offene Läufe (Beitritt möglich) für Portal und GET.
    fn kata_runs(&self) -> Vec<(String, u8, usize, bool)> {
        self.active_dungeons
            .values()
            .map(|d| {
                let n = self.inst(d.inst).map_or(0, |i| i.members.len());
                (d.leader.clone(), d.stufe, n, !d.left_start && d.result.is_none() && n < MAX_MEMBERS)
            })
            .collect()
    }

    /// `/dungeon start [1-5]`, `/dungeon join <Name>`, `/dungeon leave`.
    pub fn dungeon_cmd(&mut self, id: u32, rest: &str) -> R<()> {
        let mut w = rest.split_whitespace();
        match w.next().map(str::to_lowercase).as_deref() {
            Some("start" | "starten") => {
                let stufe = w.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(1).clamp(1, 5);
                self.dungeon_start(id, stufe).map(|_| ())
            }
            Some("join" | "beitreten") => {
                let name = w.collect::<Vec<_>>().join(" ");
                if name.is_empty() {
                    return no(400, "Wem willst du folgen? /dungeon join <Name>");
                }
                self.dungeon_join(id, &name)
            }
            Some("leave" | "verlassen") => {
                if self.kata_key(id).is_some() {
                    self.inst_leave(id);
                    Ok(())
                } else {
                    no(409, "Du bist gerade nicht in den Katakomben.")
                }
            }
            _ => {
                let offen: Vec<String> = self.kata_runs().into_iter().filter(|r| r.3).map(|r| format!("{} (Stufe {}, {}/4)", r.0, r.1, r.2)).collect();
                self.system(id, "Prüfungs-Katakomben: /dungeon start [1-5] · /dungeon join <Name> · /dungeon leave");
                let t = if offen.is_empty() { "Gerade wartet keine Gruppe in einer Start-Halle.".to_string() } else { format!("Offene Läufe: {}", offen.join(", ")) };
                self.system(id, &t);
                Ok(())
            }
        }
    }
    /// [E] am Katakomben-Portal der Zitadelle: Hud `katakomben` mit `portal: true`.
    pub fn dungeon_portal(&mut self, id: u32) -> R<()> {
        if !self.players.contains_key(&id) {
            return no(401, "Nicht verbunden.");
        }
        let laeufe: Vec<Value> = self.kata_runs().into_iter().filter(|r| r.3).map(|r| json!({ "name": r.0, "stufe": r.1, "n": r.2 })).collect();
        self.hud(id, json!({ "t": "katakomben", "portal": true, "laeufe": laeufe }));
        Ok(())
    }
    /// Neuen Lauf erzeugen und den Spieler hineinholen. Liefert die Instanz-Nummer.
    pub fn dungeon_start(&mut self, id: u32, stufe: u8) -> R<u32> {
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        if p.inst.is_some() {
            return no(409, "Du bist schon in der Arena oder in den Katakomben. /leave bringt dich zurück.");
        }
        let name = p.name.clone();
        let stufe = stufe.clamp(1, 5);
        let lay = gen::generate(self.rng.next(), stufe);
        let n = self.inst_new(ins::KIND_DUNGEON)?;
        let Some(it) = self.inst_mut(n) else { return no(500, "Die Katakomben ließen sich nicht öffnen.") };
        let (ox, oz) = it.origin();
        gen::build(&lay, |x, y, z, b| it.put(ox + x, ins::DUNGEON_Y + y, oz + z, b));
        let now = self.now();
        let mut d = Dungeon::new(n, stufe, id, name, lay, (ox, oz), now);
        d.set_text("Willkommen in den Prüfungs-Katakomben! Säubert drei Kampf-Kammern, löst die Rätsel und weckt das Blut-Portal.", now);
        let at = d.start_pos();
        let s = d.room(K_START);
        d.room_of.insert(id, s);
        self.active_dungeons.insert(n, d);
        if let Err(e) = self.inst_enter(n, id, at, 0.0) {
            self.inst_close(n);
            return Err(e);
        }
        self.system(id, &format!("Die Prüfungs-Katakomben (Stufe {stufe}) öffnen sich. Andere können mit /dungeon join beitreten, solange ihr in der Start-Halle seid."));
        self.kata_hud(n);
        Ok(n)
    }
    /// Dem Lauf des Spielers `name` beitreten (nur solange dessen Gruppe noch in der Start-Halle ist).
    pub fn dungeon_join(&mut self, id: u32, name: &str) -> R<()> {
        let want = name.trim().to_lowercase();
        let hit = self.active_dungeons.values().find(|d| {
            d.leader.to_lowercase() == want || self.inst(d.inst).is_some_and(|it| it.members.iter().any(|m| self.players.get(m).is_some_and(|p| p.name.to_lowercase() == want)))
        });
        let Some(d) = hit else { return no(404, "Von dieser Person läuft gerade kein Katakomben-Lauf.") };
        if d.left_start || d.result.is_some() {
            return no(409, "Die Gruppe hat die Start-Halle schon verlassen – Beitritt nicht mehr möglich.");
        }
        let (n, at) = (d.inst, d.start_pos());
        self.inst_enter(n, id, at, 0.0)?;
        let now = self.now();
        let who = self.players.get(&id).map_or_else(String::new, |p| p.name.clone());
        if let Some(d) = self.active_dungeons.get_mut(&n) {
            let s = d.room(K_START);
            d.room_of.insert(id, s);
            d.set_text(format!("{who} schließt sich der Gruppe an."), now);
            if d.segen {
                if let Some(p) = self.players.get_mut(&id) {
                    p.boost_until = now + 2 * RUN_MS;
                }
            }
        }
        self.kata_hud(n);
        Ok(())
    }

    /// Nur aufgerufen, wenn Game.active_dungeons nicht leer ist.
    pub fn dungeon_tick(&mut self, now: i64) {
        let keys: Vec<u32> = self.active_dungeons.keys().copied().collect();
        for n in keys {
            self.kata_step(n, now);
        }
    }
    fn kata_step(&mut self, n: u32, now: i64) {
        let Some(d) = self.active_dungeons.get(&n) else { return };
        let timeout = now - d.start >= RUN_MS;
        if timeout || (d.boss_dead_at > 0 && now - d.boss_dead_at >= AFTER_BOSS_MS) {
            if timeout && d.boss_dead_at == 0 {
                for m in self.kata_members(n) {
                    self.system(m, "Die 30 Minuten sind um – die Prüfungs-Katakomben schließen sich.");
                }
            }
            return self.inst_close(n);
        }
        let who: Vec<(u32, [f32; 3])> = self.kata_members(n).into_iter().filter_map(|m| self.players.get(&m).map(|p| (m, p.p))).collect();
        let mut sets: Vec<Set> = Vec::new();
        let mut hits: Vec<(u32, f32)> = Vec::new();
        let (Some(d), Some(it)) = (self.active_dungeons.get_mut(&n), self.instances.list.get(&n)) else { return };
        let dt = if d.last > 0 { ((now - d.last) as f32 / 1000.0).clamp(0.0, 0.25) } else { 0.1 };
        d.last = now;
        let o = d.origin;
        let here: Vec<Option<usize>> = who.iter().map(|w| local_room(o, w.1)).collect();
        let mut hud = now - d.hud_at >= 1000;
        // Kampf-Kammern: Betreten schließt die Gitter, Wellen, leer → offen
        for r in 0..9 {
            if d.lay.rooms.get(r).map_or(true, |x| x.kind != K_KAMPF) || d.cleared[r] {
                continue;
            }
            let inside = here.contains(&Some(r));
            let alive = d.mobs.iter().any(|m| m.dead_at == 0 && local_room(o, m.p) == Some(r));
            if !d.active[r] {
                if inside {
                    d.active[r] = true;
                    d.waves_left[r] = d.waves();
                    sets.extend(d.gate_sets(r, true));
                    d.spawn_wave(r);
                    d.set_text("Die Gitter fallen zu – besiegt alle Wesen dieser Kammer!", now);
                    hud = true;
                }
            } else if !inside {
                // niemand mehr drin: Kammer zurücksetzen, damit niemand ausgesperrt bleibt
                d.mobs.retain(|m| local_room(o, m.p) != Some(r));
                d.active[r] = false;
                sets.extend(d.gate_sets(r, false));
                hud = true;
            } else if !alive {
                if d.waves_left[r] > 0 {
                    d.spawn_wave(r);
                    d.set_text("Die nächste Welle bricht hervor!", now);
                } else {
                    d.active[r] = false;
                    d.cleared[r] = true;
                    sets.extend(d.gate_sets(r, false));
                    let c = d.cleared_count();
                    let t = if c >= 3 { "Alle drei Kampf-Kammern sind gesäubert – das Blut-Portal ist erwacht!".to_string() } else { format!("Kampf-Kammer gesäubert ({c}/3) – die Gitter öffnen sich.") };
                    d.set_text(t, now);
                }
                hud = true;
            }
        }
        // Endboss ruft ab Phase 1 Schatten-Gelehrte
        if let Some(b) = d.mobs.iter().find(|m| m.kind == BOSS && m.dead_at == 0).copied() {
            let br = d.room(K_BOSS);
            let minions = d.mobs.iter().filter(|m| m.kind != BOSS && m.dead_at == 0).count();
            if cb::boss_phase(b.hp / b.max.max(1.0)) >= 1 && now - d.minion_at >= 8000 && minions < MAX_MINIONS {
                let pts = gen::markers(&d.lay, br, b'm');
                if !pts.is_empty() {
                    let (ix, iz) = pts[d.next_mob as usize % pts.len()];
                    let p = d.feet(br, ix, iz);
                    d.spawn(6, p);
                    d.minion_at = now;
                }
            }
        }
        mob_ai(d, it, &who, now, dt, &mut hits);
        d.mobs.retain(|m| m.dead_at == 0 || now - m.dead_at < 600);
        // Runen: Folge zeigen (je 600 ms), danach Eingabe über dungeon_move
        let rr = d.room(K_RUNEN);
        if let Some(t0) = d.rune_show {
            if now >= t0 {
                let s = ((now - t0) / RUNE_MS) as usize;
                let want = d.lay.runes.get(s).map(|&k| k as usize);
                if want != d.rune_lit {
                    if let Some(k) = d.rune_lit {
                        sets.push((d.rune_w(k), world::RUNE));
                    }
                    if let Some(k) = want {
                        sets.push((d.rune_w(k), world::RUNE_AN));
                    }
                    d.rune_lit = want;
                }
                if want.is_none() {
                    d.rune_show = None;
                    d.rune_last.clear();
                    d.set_text("Jetzt ihr: betretet die Runen in derselben Reihenfolge.", now);
                    hud = true;
                }
            }
        } else if !d.rune_done && !d.rune_shown && here.contains(&Some(rr)) {
            d.rune_shown = true;
            d.rune_show = Some(now + 1000);
            d.set_text("Die Runen erwachen – merkt euch die Folge!", now);
            hud = true;
        }
        let states: Vec<MobState> = d
            .mobs
            .iter()
            .map(|m| {
                let mut fl = 0;
                if m.hurt_until > now {
                    fl |= MOB_HURT;
                }
                if m.dead_at > 0 {
                    fl |= MOB_DEAD;
                }
                MobState { id: m.id, kind: m.kind, p: m.p, yaw: m.yaw, hp: (m.hp / m.max.max(1.0) * 1000.0).clamp(0.0, 1000.0) as u16, fl }
            })
            .collect();
        let send = !states.is_empty() || d.mobs_shown;
        d.mobs_shown = !states.is_empty();
        self.kata_sets(n, sets);
        if send {
            self.inst_send(n, &S2C::Mobs(states));
        }
        for (id, dmg) in hits {
            self.hurt_player(id, dmg, now);
        }
        // Leben regenerieren wie in der Zitadelle
        for &(id, _) in &who {
            let Some(p) = self.players.get_mut(&id) else { continue };
            let max = p.stats.max_hp;
            if max <= 0.0 || p.hp >= max {
                continue;
            }
            let before = p.hp;
            p.hp = (p.hp + p.stats.hp_regen * dt).min(max);
            if p.hp.floor() != before.floor() {
                let (hp, shield_ms) = (p.hp, (p.shield_until - now).max(0) as u32);
                self.send(id, &S2C::Health { hp, max, shield_ms });
            }
        }
        if hud {
            self.kata_hud(n);
        }
    }
    /// Hud `katakomben` an alle Mitglieder (Raum je Person).
    fn kata_hud(&mut self, n: u32) {
        let now = self.now();
        let members = self.kata_members(n);
        let Some(d) = self.active_dungeons.get_mut(&n) else { return };
        d.hud_at = now;
        let base = d.hud_base(now);
        let start = d.room(K_START);
        let out: Vec<(u32, Value)> = members
            .iter()
            .map(|&m| {
                let r = d.room_of.get(&m).copied().unwrap_or(start);
                let mut v = base.clone();
                v["raum"] = json!(d.lay.rooms.get(r).map_or("Katakomben", |x| x.name.as_str()));
                v["raum_nr"] = json!(r + 1);
                (m, v)
            })
            .collect();
        for (m, v) in out {
            self.hud(m, v);
        }
    }
    fn kata_boss_hud(&mut self, n: u32) {
        let Some(b) = self.active_dungeons.get(&n).and_then(|d| d.mobs.iter().find(|m| m.kind == BOSS && m.dead_at == 0).copied()) else { return };
        let phase = cb::boss_phase(b.hp / b.max.max(1.0));
        self.inst_hud(n, json!({ "t": "boss", "name": boss_name(), "hp": b.hp.max(0.0).round(), "max": b.max.round(), "phase": phase }));
    }

    pub fn dungeon_move(&mut self, id: u32, now: i64) {
        let Some(n) = self.kata_key(id) else { return };
        let Some(p) = self.players.get(&id).map(|p| p.p) else { return };
        let Some(d) = self.active_dungeons.get_mut(&n) else { return };
        if p[1] < (ins::DUNGEON_Y - 3) as f32 {
            let at = d.start_pos();
            return self.inst_goto(id, at, 0.0);
        }
        let (lx, lz) = (p[0].floor() as i32 - d.origin.0, p[2].floor() as i32 - d.origin.1);
        let Some(r) = gen::room_at(lx, lz) else { return };
        let Some(kind) = d.lay.rooms.get(r).map(|x| x.kind) else { return };
        let mut hud = d.room_of.insert(id, r) != Some(r);
        if kind != K_START {
            d.left_start = true;
        }
        if kind == K_SCHATZ && !d.geheim {
            d.geheim = true;
            d.set_text("Ihr habt die versteckte Schatz-Kammer entdeckt!", now);
        }
        let mut sets: Vec<Set> = Vec::new();
        if kind == K_RUNEN && !d.rune_done && d.rune_shown && d.rune_show.is_none() {
            let (rx, rz) = gen::room_origin(r);
            let on = p[1] < (ins::DUNGEON_Y + 1) as f32 + 0.6;
            match gen::rune_at(lx - rx - 1, lz - rz - 1).filter(|_| on) {
                Some(k) if d.rune_last.get(&id) != Some(&k) => {
                    d.rune_last.insert(id, k);
                    if d.lay.runes.get(d.rune_prog).map(|&x| x as usize) == Some(k) {
                        d.rune_prog += 1;
                        sets.push((d.rune_w(k), world::RUNE_AN));
                        if d.rune_prog >= gen::RUNE_STEPS {
                            d.rune_done = true;
                            for j in 0..9 {
                                sets.push((d.rune_w(j), world::RUNE_AN));
                            }
                            let t = d.room(K_SCHATZ);
                            if let Some(&(a, b)) = d.lay.doors.iter().find(|e| e.0 == t || e.1 == t) {
                                sets.extend(d.door_sets(a, b, 0));
                            }
                            d.set_text("Runen-Rätsel gelöst! Irgendwo hebt sich ein Gitter …", now);
                        }
                    } else {
                        d.rune_prog = 0;
                        for j in 0..9 {
                            sets.push((d.rune_w(j), world::RUNE));
                        }
                        d.rune_lit = None;
                        d.rune_show = Some(now + 1200);
                        d.set_text("Falsche Rune! Die Folge beginnt von vorn – seht noch einmal hin.", now);
                    }
                    hud = true;
                }
                Some(_) => {}
                None => {
                    d.rune_last.remove(&id);
                }
            }
        }
        self.kata_sets(n, sets);
        if hud {
            self.kata_hud(n);
        }
    }

    /// [E] auf Block `b` des Instanz-Gitters an (x, y, z); Reichweite ist geprüft.
    pub fn dungeon_use(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8) -> R<()> {
        let Some(n) = self.kata_key(id) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        let now = self.now();
        let _ = y;
        match b {
            world::PRISMA_A | world::PRISMA_B => self.kata_prism(n, x, z, now),
            world::STATUE => self.kata_statue(n, id, x, z),
            world::WEISEN_TRUHE => self.kata_sage_chest(n, id, x, z, now),
            world::SCHATZ => self.kata_treasure(n, id),
            world::BLUT_PORTAL => self.kata_portal(n, now),
            world::BEUTE_ALTAR | world::SMARAGD_TRUHE | world::OBSIDIAN_TRUHE => self.kata_loot(n, id, b, now),
            _ => no(404, "Hier gibt es nichts zu benutzen."),
        }
    }
    fn kata_prism(&mut self, n: u32, x: i32, z: i32, now: i64) -> R<()> {
        let Some(d) = self.active_dungeons.get_mut(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        if d.mirror_done {
            return no(409, "Der Strahl trifft den Kristall schon – dieses Rätsel ist gelöst.");
        }
        let r = d.room(K_SPIEGEL);
        let Some(c) = d.cell_in(r, x, z) else { return no(404, "Dieses Prisma lässt sich nicht drehen.") };
        let Some(k) = d.lay.mirror.prisms.iter().position(|p| p.0 == c) else { return no(404, "Dieses Prisma lässt sich nicht drehen.") };
        d.lay.mirror.prisms[k].1 = !d.lay.mirror.prisms[k].1;
        let a = d.lay.mirror.prisms[k].1;
        let (path, hit) = gen::beam(&d.lay.mirror);
        let old = std::mem::replace(&mut d.beam, path.clone());
        let mut sets: Vec<Set> = vec![(d.wi(r, c, gen::BEAM_Y), if a { world::PRISMA_A } else { world::PRISMA_B })];
        sets.extend(old.iter().filter(|&&c| !path.contains(&c)).map(|&c| (d.wi(r, c, gen::BEAM_Y), 0)));
        sets.extend(path.iter().map(|&c| (d.wi(r, c, gen::BEAM_Y), world::STRAHL)));
        if hit {
            d.mirror_done = true;
            sets.push((d.wi(r, d.lay.mirror.receiver, gen::BEAM_Y), world::EMPFAENGER_AN));
            d.set_text("Der Lichtstrahl trifft den Kristall – Spiegel-Rätsel gelöst!", now);
        }
        self.kata_sets(n, sets);
        if hit {
            self.kata_hud(n);
        }
        Ok(())
    }
    fn kata_statue(&mut self, n: u32, id: u32, x: i32, z: i32) -> R<()> {
        let Some(d) = self.active_dungeons.get(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        let r = d.room(K_WEISEN);
        let nr = d.cell_in(r, x, z).filter(|c| c.1 == gen::STATUE_Z).and_then(|c| gen::SAGE_X.iter().position(|&v| v == c.0));
        let Some(nr) = nr else { return no(404, "Diese Statue schweigt.") };
        let Some(set) = gen::SAGES.get(d.lay.sages) else { return no(404, "Diese Statue schweigt.") };
        let text = gen::sage_text(set, nr);
        self.hud(id, json!({ "t": "weise", "nr": nr, "text": text }));
        Ok(())
    }
    fn kata_sage_chest(&mut self, n: u32, id: u32, x: i32, z: i32, now: i64) -> R<()> {
        let members = self.kata_members(n);
        let Some(d) = self.active_dungeons.get_mut(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        let r = d.room(K_WEISEN);
        let nr = d.cell_in(r, x, z).filter(|c| c.1 == gen::CHEST_Z).and_then(|c| gen::SAGE_X.iter().position(|&v| v == c.0));
        let Some(nr) = nr else { return no(404, "Diese Truhe gehört keinem Weisen.") };
        if d.sages != 0 {
            return no(409, "Die Truhen der Weisen sind schon geöffnet.");
        }
        let win = nr == d.lay.sage_answer as usize;
        d.sages = if win { 1 } else { 2 };
        let sets: Vec<Set> = gen::SAGE_X.iter().map(|&sx| (d.wi(r, (sx, gen::CHEST_Z), 1), 0)).collect();
        if win {
            d.segen = true;
            d.set_text("Segen der Weisen: +25 % Schaden und Tempo für die ganze Gruppe!", now);
        } else {
            d.set_text("Falsche Truhe! Ein Fluch trifft dich – das Rätsel der Weisen ist verloren.", now);
        }
        self.kata_sets(n, sets);
        if win {
            for m in members {
                if let Some(p) = self.players.get_mut(&m) {
                    p.boost_until = now + 2 * RUN_MS;
                }
            }
        } else {
            self.hurt_player(id, SAGE_DMG, now);
        }
        self.kata_hud(n);
        Ok(())
    }
    fn kata_treasure(&mut self, n: u32, id: u32) -> R<()> {
        let Some(d) = self.active_dungeons.get_mut(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        if !d.rune_done {
            return no(409, "Die Schatz-Kammer ist noch versiegelt.");
        }
        if d.got_schatz.contains(&id) {
            return no(409, "Diesen Schatz hast du schon geplündert.");
        }
        d.got_schatz.push(id);
        let items = gen::treasure_loot(d.stufe);
        if let Err(e) = self.kata_give(n, id, &items, 0) {
            if let Some(d) = self.active_dungeons.get_mut(&n) {
                d.got_schatz.retain(|&u| u != id);
            }
            return Err(e);
        }
        Ok(())
    }
    fn kata_portal(&mut self, n: u32, now: i64) -> R<()> {
        let members = self.kata_members(n);
        let Some(d) = self.active_dungeons.get_mut(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        if d.boss_open {
            return no(409, "Das Blut-Portal ist schon offen.");
        }
        let c = d.cleared_count();
        if c < 3 {
            return no(409, &format!("Das Blut-Portal schläft noch – erst alle drei Kampf-Kammern säubern ({c}/3)."));
        }
        let br = d.room(K_BOSS);
        let Some(&(a, b)) = d.lay.doors.iter().find(|e| e.0 == br || e.1 == br) else { return no(500, "Der Endboss-Saal hat keinen Zugang.") };
        d.boss_open = true;
        let sets = d.door_sets(a, b, 0);
        // Ankunft: Innenraum-Zelle des Saals direkt hinter der Tür
        let cells = gen::door_cells(a, b);
        let (cx, cz) = (cells.iter().map(|c| c.0 as f32).sum::<f32>() / 4.0 + 0.5, cells.iter().map(|c| c.1 as f32).sum::<f32>() / 4.0 + 0.5);
        let (rx, rz) = gen::room_origin(br);
        let (ix, iz) = (((cx - (rx + 1) as f32).floor() as i32).clamp(0, 13), ((cz - (rz + 1) as f32).floor() as i32).clamp(0, 13));
        let at = d.feet(br, ix, iz);
        let (bx, bz) = gen::markers(&d.lay, br, b'B').first().copied().unwrap_or((6, 6));
        let bp = d.feet(br, bx, bz);
        d.spawn(BOSS, bp);
        d.minion_at = now;
        d.boss_dmg.clear();
        d.set_text("Das Blut-Portal öffnet sich – der Namenlose Prüfer erwartet euch!", now);
        let yaw = (at[0] - bp[0]).atan2(at[2] - bp[2]);
        self.kata_sets(n, sets);
        for m in members {
            self.inst_goto(m, at, yaw);
        }
        self.kata_boss_hud(n);
        self.kata_hud(n);
        Ok(())
    }
    fn kata_loot(&mut self, n: u32, id: u32, b: u8, now: i64) -> R<()> {
        let Some(d) = self.active_dungeons.get_mut(&n) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        let Some((note, _, _)) = d.result else { return no(409, "Erst muss der Namenlose Prüfer fallen.") };
        let rank = gen::grade_rank(note);
        let stufe = d.stufe;
        let (list, need) = match b {
            world::BEUTE_ALTAR => (&mut d.got_altar, 0),
            world::SMARAGD_TRUHE => (&mut d.got_smaragd, 3),
            _ => (&mut d.got_obsidian, 4),
        };
        if rank < need {
            return no(403, if need == 3 { "Die Smaragd-Truhe öffnet sich erst ab Note S." } else { "Die Obsidian-Truhe öffnet sich nur bei Note S+." });
        }
        if list.contains(&id) {
            return no(409, "Das hast du schon genommen.");
        }
        list.push(id);
        let (items, taler) = match b {
            world::BEUTE_ALTAR => (gen::altar_loot(stufe), 60 * stufe as i64 * self.taler_mult()),
            world::SMARAGD_TRUHE => (gen::chest_loot(false, stufe, &mut self.rng), 0),
            _ => (gen::chest_loot(true, stufe, &mut self.rng), 0),
        };
        if let Err(e) = self.kata_give(n, id, &items, taler) {
            if let Some(d) = self.active_dungeons.get_mut(&n) {
                match b {
                    world::BEUTE_ALTAR => d.got_altar.retain(|&u| u != id),
                    world::SMARAGD_TRUHE => d.got_smaragd.retain(|&u| u != id),
                    _ => d.got_obsidian.retain(|&u| u != id),
                }
            }
            return Err(e);
        }
        if let Some(v) = self.active_dungeons.get(&n).map(|d| d.ende_json(id, now)) {
            self.hud(id, v);
        }
        Ok(())
    }
    /// Gegenstände (+ Taler) in einer Buchung geben, Inventar melden, in der Beute-Liste des Laufs vermerken.
    fn kata_give(&mut self, n: u32, id: u32, loot: &[(u16, u32)], taler: i64) -> R<()> {
        let totals = self.tx(|g| {
            let mut t = Vec::new();
            for &(item, k) in loot {
                t.push((item, g.gain(id, item, k)?));
            }
            if taler > 0 {
                g.pay(id, taler, "katakomben")?;
            }
            Ok(t)
        })?;
        for (item, k) in totals {
            self.send(id, &S2C::Inv { item, n: k });
        }
        let mut text: Vec<String> = loot.iter().map(|&(i, k)| format!("{k}× {}", items::item_name(i))).collect();
        if taler > 0 {
            text.push(format!("{taler} Taler"));
        }
        self.hud(id, json!({ "t": "aktion", "text": format!("Beute: {}", text.join(", ")) }));
        if let Some(d) = self.active_dungeons.get_mut(&n) {
            d.beute.entry(id).or_default().extend_from_slice(loot);
        }
        Ok(())
    }

    pub fn dungeon_attack(&mut self, id: u32, mob: u32, now: i64) -> R<()> {
        let Some(n) = self.kata_key(id) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        let (pos, swing_at, st) = (p.p, p.swing_at, p.stats);
        let Some(d) = self.active_dungeons.get(&n) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let Some(i) = d.mobs.iter().position(|m| m.id == mob && m.dead_at == 0) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let (m, segen) = (d.mobs[i], d.segen);
        let size = size_of(m.kind);
        let eye = [pos[0], pos[1] + 1.6, pos[2]];
        let c = [m.p[0], m.p[1] + 0.5 * size, m.p[2]];
        let dist = ((eye[0] - c[0]).powi(2) + (eye[1] - c[1]).powi(2) + (eye[2] - c[2]).powi(2)).sqrt();
        if dist > cb::MELEE_REACH + size / 2.0 {
            return no(403, "Zu weit weg.");
        }
        if now - swing_at < cb::SWING_CD_MS {
            return no(429, "Zu schnell.");
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.swing_at = now;
        }
        let (dmg, crit) = cb::player_hit(&st, self.rng.next() as u32);
        self.kata_hit(n, id, i, if segen { dmg * SEGEN } else { dmg }, crit, pos, now);
        Ok(())
    }
    pub fn dungeon_shoot(&mut self, id: u32, dir: [f32; 3], now: i64) -> R<()> {
        let Some(n) = self.kata_key(id) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        let (pos, shot_at, st) = (p.p, p.shot_at, p.stats);
        let held = self.held_item(id);
        if !rpg::is_ranged(held) {
            return no(400, "Dafür brauchst du einen Bogen oder Stab in der Hand.");
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !len.is_finite() || len < 0.5 {
            return no(400, "Ungültige Richtung.");
        }
        let dv = [dir[0] / len, dir[1] / len, dir[2] / len];
        if now - shot_at < cb::SHOT_CD_MS {
            return no(429, "Zu schnell.");
        }
        let staff = held == rpg::ARKANER_STAB;
        if staff {
            self.spend_mana(id, cb::STAFF_MANA)?;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.shot_at = now;
        }
        let eye = [pos[0], pos[1] + 1.6, pos[2]];
        let wall = self.inst(n).and_then(|it| world::raycast_solid(|x, y, z| it.get(x, y, z), eye, dv, cb::RANGED_REACH)).map_or(cb::RANGED_REACH, |(c, _)| {
            let q = [c[0] as f32 + 0.5 - eye[0], c[1] as f32 + 0.5 - eye[1], c[2] as f32 + 0.5 - eye[2]];
            ((q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt() - 0.5).max(0.0)
        });
        let Some(d) = self.active_dungeons.get(&n) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let segen = d.segen;
        let mut best: Option<(usize, f32)> = None;
        for (i, m) in d.mobs.iter().enumerate().filter(|(_, m)| m.dead_at == 0) {
            let size = size_of(m.kind);
            let c = [m.p[0], m.p[1] + 0.5 * size, m.p[2]];
            if let Some(t) = cb::ray_sphere(eye, dv, c, 0.6 * size, wall) {
                if best.map_or(true, |b| t < b.1) {
                    best = Some((i, t));
                }
            }
        }
        let t = best.map_or(wall, |b| b.1);
        let to = [eye[0] + dv[0] * t, eye[1] + dv[1] * t, eye[2] + dv[2] * t];
        self.inst_send(n, &S2C::Shot { from: eye, to, kind: u8::from(staff) });
        if let Some((i, _)) = best {
            let (dmg, crit) = cb::player_hit(&st, self.rng.next() as u32);
            self.kata_hit(n, id, i, if segen { dmg * SEGEN } else { dmg }, crit, pos, now);
        }
        Ok(())
    }
    /// Schaden aufs Wesen i des Laufs n: Rückstoß, Zahl, Boss-Leiste; Tod → Beute (Endboss: Lauf-Ende).
    fn kata_hit(&mut self, n: u32, uid: u32, i: usize, dmg: f32, crit: bool, from: [f32; 3], now: i64) {
        let Some(d) = self.active_dungeons.get_mut(&n) else { return };
        let Some(m) = d.mobs.get_mut(i) else { return };
        let size = size_of(m.kind);
        let dealt = dmg.min(m.hp.max(0.0));
        m.hp -= dmg;
        m.hurt_until = now + cb::FLASH_MS;
        if m.kind != BOSS {
            let (dx, dz) = (m.p[0] - from[0], m.p[2] - from[2]);
            let l = (dx * dx + dz * dz).sqrt().max(0.01);
            let kb = cb::KNOCKBACK / size.max(1.0);
            m.kb = [dx / l * kb, dz / l * kb];
        }
        let dead = m.hp <= 0.0;
        if dead {
            m.hp = 0.0;
            m.dead_at = now;
        }
        let m = *m;
        if m.kind == BOSS {
            match d.boss_dmg.iter().position(|e| e.0 == uid) {
                Some(k) => d.boss_dmg[k].1 += dealt,
                None => d.boss_dmg.push((uid, dealt)),
            }
        }
        let top = [m.p[0], m.p[1] + size * 1.1 + 0.3, m.p[2]];
        self.inst_send(n, &S2C::Damage { at: top, n: dmg.round() as u32, crit, mob: m.id });
        if m.kind == BOSS {
            if dead {
                self.kata_boss_dead(n, m, now);
            } else {
                self.kata_boss_hud(n);
            }
        } else if dead {
            self.loot(uid, m.kind, m.p);
            self.kata_hud(n);
        }
    }
    /// Endboss besiegt: Note festhalten, Bestwert, Beute-Altar (und Truhen), Lauf endet 60 s später.
    fn kata_boss_dead(&mut self, n: u32, m: Mob, now: i64) {
        let members = self.kata_members(n);
        let Some(d) = self.active_dungeons.get_mut(&n) else { return };
        for x in d.mobs.iter_mut().filter(|x| x.dead_at == 0) {
            x.hp = 0.0;
            x.dead_at = now;
        }
        let who = cb::loot_share(&d.boss_dmg);
        d.boss_dmg.clear();
        let ms = (now - d.start).max(0);
        let p = gen::score(d.tode, ms, d.solved());
        let note = gen::grade(p);
        let rank = gen::grade_rank(note);
        d.result = Some((note, p, ms));
        d.boss_dead_at = now;
        let br = d.room(K_BOSS);
        let (bx, bz) = gen::markers(&d.lay, br, b'B').first().copied().unwrap_or((6, 6));
        let mut sets: Vec<Set> = vec![(d.wi(br, (bx, bz), 1), world::BEUTE_ALTAR)];
        if rank >= 3 {
            sets.push((d.wi(br, (bx - 2, bz), 1), world::SMARAGD_TRUHE));
        }
        if rank >= 4 {
            sets.push((d.wi(br, (bx + 2, bz), 1), world::OBSIDIAN_TRUHE));
        }
        d.set_text(format!("{} ist besiegt! Note {note}. Holt eure Beute am Beute-Altar – in 60 Sekunden schließen sich die Katakomben.", boss_name()), now);
        let stufe = d.stufe;
        let ende: Vec<(u32, Value)> = members.iter().map(|&u| (u, d.ende_json(u, now))).collect();
        self.kata_sets(n, sets);
        self.inst_hud(n, json!({ "t": "boss", "aus": true, "name": boss_name() }));
        for u in who {
            self.loot(u, BOSS, m.p);
        }
        for &u in &members {
            let _ = self.db.execute(
                "insert into dungeon_best (user, stufe, note, ms, ts) values (?1, ?2, ?3, ?4, ?5) on conflict (user, stufe) do update set note = excluded.note, ms = excluded.ms, ts = excluded.ts \
                 where excluded.note > dungeon_best.note or (excluded.note = dungeon_best.note and excluded.ms < dungeon_best.ms)",
                rusqlite::params![u, stufe, rank, ms, now],
            );
        }
        for (u, v) in ende {
            self.hud(u, v);
        }
        self.kata_hud(n);
    }

    /// Spieler verlässt den Lauf (er ist noch Mitglied, die Instanz steht noch). Nicht inst_leave(id) aufrufen.
    pub fn dungeon_left(&mut self, inst: u32, id: u32) {
        let next = self.inst(inst).and_then(|it| it.members.iter().copied().find(|&m| m != id)).and_then(|m| self.players.get(&m).map(|p| (m, p.name.clone())));
        let Some(d) = self.active_dungeons.get_mut(&inst) else { return };
        d.room_of.remove(&id);
        d.rune_last.remove(&id);
        if d.leader_id == id {
            if let Some((m, name)) = next {
                d.leader_id = m;
                d.leader = name;
            }
        }
        let boss = d.mobs.iter().any(|m| m.kind == BOSS && m.dead_at == 0);
        self.send(id, &S2C::Mobs(Vec::new()));
        if boss {
            // nur die Leiste ausblenden: „weg“ = verlassen, nicht besiegt (hud.js zeigt dann keinen Sieg)
            self.hud(id, json!({ "t": "boss", "aus": true, "weg": true, "name": boss_name() }));
        }
    }
    pub fn dungeon_died(&mut self, id: u32, now: i64) {
        let Some(n) = self.kata_key(id) else { return };
        let name = self.players.get(&id).map_or_else(String::new, |p| p.name.clone());
        let Some(d) = self.active_dungeons.get_mut(&n) else { return };
        d.tode += 1;
        let at = d.start_pos();
        let s = d.room(K_START);
        d.room_of.insert(id, s);
        d.rune_last.remove(&id);
        d.set_text(format!("{name} ist gefallen – zurück in die Start-Halle. Jeder Tod kostet Können-Punkte."), now);
        self.inst_goto(id, at, 0.0);
        self.kata_hud(n);
    }
    pub fn dungeon_get(&mut self, uid: u32) -> R<Value> {
        let laeufe: Vec<Value> = self.kata_runs().into_iter().map(|r| json!({ "name": r.0, "stufe": r.1, "n": r.2, "offen": r.3 })).collect();
        let mut st = self.db.prepare("select stufe, note, ms from dungeon_best where user = ? order by stufe")?;
        let rows = st.query_map([uid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
        let best: Vec<Value> = rows.filter_map(Result::ok).map(|(s, note, ms)| json!({ "stufe": s, "note": gen::GRADES.get(note.max(0) as usize).copied().unwrap_or("C"), "ms": ms })).collect();
        Ok(json!({ "laeufe": laeufe, "bestzeiten": best }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;

    fn start(g: &mut Game) -> u32 {
        g.dungeon_start(1, 1).unwrap()
    }

    #[test]
    fn lauf_ohne_leck() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        let _b = join(&mut g, 2, "Ben", false);
        let n = start(&mut g);
        g.dungeon_join(2, "anna").unwrap();
        assert_eq!(g.active_dungeons.len(), 1);
        assert_eq!(g.inst(n).unwrap().members.len(), 2);
        let (ox, oz) = g.inst(n).unwrap().origin();
        for _ in 0..5 {
            g.tick();
        }
        g.say(1, 0, "/leave");
        assert_eq!(g.players[&1].inst, None);
        assert_eq!(g.active_dungeons.len(), 1, "eine Person ist noch drin");
        g.drop_player(2);
        assert_eq!(g.active_dungeons.len(), 0);
        assert!(g.instances.list.is_empty());
        g.save().unwrap();
        let rows: i64 = g.db.query_row("select count(*) from chunks where cx >= ?1 and cx < ?1 + 3 and cz >= ?2 and cz < ?2 + 3", [ox >> 4, oz >> 4], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn leerlauf_ohne_lauf() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        for _ in 0..100 {
            g.tick();
        }
        assert!(g.active_dungeons.is_empty() && g.instances.list.is_empty());
    }

    #[test]
    fn wesen_stirbt_gibt_beute() {
        let mut g = game();
        let _a = join(&mut g, 1, "Anna", false);
        let n = start(&mut g);
        let (r, p) = {
            let d = &g.active_dungeons[&n];
            let r = gen::find(&d.lay, K_KAMPF);
            let (ix, iz) = gen::markers(&d.lay, r, b'm')[0];
            (r, d.feet(r, ix, iz))
        };
        put(&mut g, 1, p);
        g.dungeon_move(1, g.now());
        assert!(g.active_dungeons[&n].left_start);
        assert!(g.dungeon_join(1, "Anna").is_err());
        let now = g.now();
        g.dungeon_tick(now);
        let d = &g.active_dungeons[&n];
        assert!(d.active[r] && !d.mobs.is_empty());
        assert!(d.mobs.iter().all(|m| m.id >= 1_000_000));
        let (mid, mp) = {
            let m = &mut g.active_dungeons.get_mut(&n).unwrap().mobs[0];
            m.hp = 0.5;
            (m.id, m.p)
        };
        put(&mut g, 1, [mp[0] + 1.0, mp[1], mp[2]]);
        let before = g.taler(1);
        let now = g.now();
        g.dungeon_attack(1, mid, now).unwrap();
        assert!(g.active_dungeons[&n].mobs.iter().any(|m| m.id == mid && m.dead_at > 0));
        assert!(g.taler(1) > before, "Beute: Taler");
    }

    #[test]
    fn spiegel_weise_boss_und_beute() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Anna", false);
        let n = start(&mut g);
        // Spiegel: falsch stehende Prismen drehen
        let todo: Vec<[i32; 3]> = {
            let d = &g.active_dungeons[&n];
            let r = d.room(K_SPIEGEL);
            d.lay.mirror.prisms.iter().zip(&d.lay.mirror.solution).filter(|(p, s)| p.1 != **s).map(|(p, _)| d.wi(r, p.0, gen::BEAM_Y)).collect()
        };
        assert!(!todo.is_empty() && !g.active_dungeons[&n].mirror_done);
        for w in todo {
            if g.active_dungeons[&n].mirror_done {
                break; // ein anderer Weg traf schon
            }
            let b = g.inst(n).unwrap().get(w[0], w[1], w[2]);
            g.dungeon_use(1, w[0], w[1], w[2], b).unwrap();
        }
        let recv = {
            let d = &g.active_dungeons[&n];
            d.wi(d.room(K_SPIEGEL), d.lay.mirror.receiver, gen::BEAM_Y)
        };
        assert!(g.active_dungeons[&n].mirror_done);
        assert_eq!(g.inst(n).unwrap().get(recv[0], recv[1], recv[2]), world::EMPFAENGER_AN);
        // Weise: die richtige Truhe gibt den Segen
        let w = {
            let d = &g.active_dungeons[&n];
            d.wi(d.room(K_WEISEN), (gen::SAGE_X[d.lay.sage_answer as usize], gen::CHEST_Z), 1)
        };
        g.dungeon_use(1, w[0], w[1], w[2], world::WEISEN_TRUHE).unwrap();
        assert!(g.active_dungeons[&n].segen && g.players[&1].boost_until > g.now());
        // Runen: echte Eingabe – Folge ansehen, ein falscher Schritt setzt zurück, dann die ganze Folge abgehen
        let (rr, runes, aus, show) = {
            let d = &g.active_dungeons[&n];
            let rr = d.room(K_RUNEN);
            let runes: Vec<usize> = d.lay.runes.iter().map(|&k| k as usize).collect();
            // längster Vorlauf: 1,2 s nach einem Fehler (1 s beim ersten Betreten), dann je Rune RUNE_MS
            let show = 1200 + RUNE_MS * runes.len() as i64 + 100;
            (rr, runes, d.feet(rr, 6, 1), show)
        };
        let auf = |g: &mut Game, p: [f32; 3]| {
            put(g, 1, p);
            let now = g.now();
            g.dungeon_move(1, now);
        };
        let rune = |g: &Game, k: usize| {
            let c = gen::rune_cell(k);
            g.active_dungeons[&n].feet(rr, c.0, c.1)
        };
        let zeigen = |g: &mut Game| {
            g.clock_offset += show;
            let now = g.now();
            g.dungeon_tick(now);
        };
        auf(&mut g, aus);
        let now = g.now();
        g.dungeon_tick(now);
        assert!(g.active_dungeons[&n].rune_show.is_some(), "Betreten startet die Vorführung");
        zeigen(&mut g);
        assert!(g.active_dungeons[&n].rune_show.is_none());
        let p0 = rune(&g, runes[0]);
        auf(&mut g, p0);
        assert_eq!(g.active_dungeons[&n].rune_prog, 1);
        let falsch = (0..9).find(|k| *k != runes[0] && *k != runes[1]).unwrap();
        let pf = rune(&g, falsch);
        auf(&mut g, pf);
        assert_eq!(g.active_dungeons[&n].rune_prog, 0, "falsche Rune setzt zurück");
        assert!(g.active_dungeons[&n].rune_show.is_some(), "Folge wird erneut gezeigt");
        zeigen(&mut g);
        for &k in &runes {
            auf(&mut g, aus);
            let pk = rune(&g, k);
            auf(&mut g, pk);
        }
        assert!(g.active_dungeons[&n].rune_done);
        let tuer = {
            let d = &g.active_dungeons[&n];
            let t = d.room(K_SCHATZ);
            let &(a, b) = d.lay.doors.iter().find(|e| e.0 == t || e.1 == t).unwrap();
            d.door_sets(a, b, 0)
        };
        assert!(tuer.iter().all(|(w, _)| g.inst(n).unwrap().get(w[0], w[1], w[2]) == 0), "Schatz-Gitter offen");
        // Kampf-Kammern gelten als gesäubert → Blut-Portal
        {
            let d = g.active_dungeons.get_mut(&n).unwrap();
            for r in 0..9 {
                d.cleared[r] = d.lay.rooms[r].kind == K_KAMPF;
            }
        }
        let door = {
            let d = &g.active_dungeons[&n];
            let br = d.room(K_BOSS);
            let &(a, b) = d.lay.doors.iter().find(|e| e.0 == br || e.1 == br).unwrap();
            d.door_sets(a, b, 0)[0].0
        };
        assert_eq!(g.inst(n).unwrap().get(door[0], door[1], door[2]), world::BLUT_PORTAL);
        g.dungeon_use(1, door[0], door[1], door[2], world::BLUT_PORTAL).unwrap();
        assert_eq!(g.inst(n).unwrap().get(door[0], door[1], door[2]), 0);
        let (bid, bp) = {
            let d = g.active_dungeons.get_mut(&n).unwrap();
            let b = d.mobs.iter_mut().find(|m| m.kind == BOSS).unwrap();
            b.hp = 0.5;
            (b.id, b.p)
        };
        put(&mut g, 1, [bp[0] + 1.5, bp[1], bp[2]]);
        msgs(&mut rx);
        let now = g.now();
        g.dungeon_attack(1, bid, now).unwrap();
        assert_eq!(g.active_dungeons[&n].result.map(|r| r.0), Some("S+"));
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "katakomben" && h["ende"] == json!(true) && h["note"] == "S+"));
        // Altar und beide Truhen
        let (alt, sma, obs) = {
            let d = &g.active_dungeons[&n];
            let br = d.room(K_BOSS);
            let (bx, bz) = gen::markers(&d.lay, br, b'B')[0];
            (d.wi(br, (bx, bz), 1), d.wi(br, (bx - 2, bz), 1), d.wi(br, (bx + 2, bz), 1))
        };
        assert_eq!(g.inst(n).unwrap().get(obs[0], obs[1], obs[2]), world::OBSIDIAN_TRUHE);
        let before = g.taler(1);
        g.dungeon_use(1, alt[0], alt[1], alt[2], world::BEUTE_ALTAR).unwrap();
        assert!(g.taler(1) >= before + 60);
        assert!(g.dungeon_use(1, alt[0], alt[1], alt[2], world::BEUTE_ALTAR).is_err(), "nur einmal");
        g.dungeon_use(1, obs[0], obs[1], obs[2], world::OBSIDIAN_TRUHE).unwrap();
        g.dungeon_use(1, sma[0], sma[1], sma[2], world::SMARAGD_TRUHE).unwrap();
        assert!(g.inv(1, rpg::MEISTER_KERN) >= 1);
        let best = g.dungeon_get(1).unwrap();
        assert_eq!(best["bestzeiten"][0]["note"], "S+");
        // 60 s nach dem Boss endet der Lauf
        g.clock_offset += AFTER_BOSS_MS + 1000;
        let now = g.now();
        g.dungeon_tick(now);
        assert!(g.active_dungeons.is_empty() && g.instances.list.is_empty());
        assert_eq!(g.players[&1].inst, None);
    }
}
