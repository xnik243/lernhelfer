//! Phase 18 (Besitz: Agent MOBS): Kampf, Wesen der Prüfungs-Zitadelle, Boss, Leben/Wiederbelebung.
//! LAW 2: mob_tick zählt zuerst die Spieler in der Zitadelle und steigt ohne sie sofort aus (keine Allokation im Leerlauf).
use crate::game::Game;
use crate::{no, E, R};
use campus_shared::combat as cb;
use campus_shared::proto::{MobState, MOB_DEAD, MOB_HURT, MOB_SLAM, MOB_WINDUP, S2C};
use campus_shared::rpg;
use campus_shared::skyblock::Goal;
use campus_shared::world;
use serde_json::{json, Value};

/// Fähigkeit „Gesellschaft“ bekommt die Kampf-EP.
const SK_KAMPF: u8 = 3;
/// Bis hierhin (waagrecht um die Zitadelle) bekommt man S2C::Mobs.
const VIEW: f32 = 64.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Mob {
    pub id: u32,
    pub kind: u8,
    pub p: [f32; 3],
    pub yaw: f32,
    pub hp: f32,
    pub max: f32,
    pub hurt_until: i64,
    /// letzter eigener Treffer (MOB_HIT_CD_MS)
    pub hit_at: i64,
    /// Rückstoß (Blöcke/s waagrecht), klingt ab
    pub kb: [f32; 2],
    /// Wächter: holt aus bis / stürmt bis, Richtung
    pub windup_until: i64,
    pub charge_until: i64,
    pub charge: [f32; 2],
    /// Boss: Stoßwelle lädt bis / nächste frühestens
    pub slam_at: i64,
    pub next_slam: i64,
    /// besiegt seit (bleibt kurz für die Anzeige)
    pub dead_at: i64,
}

pub struct Shock {
    pub at: [f32; 3],
    pub start: i64,
    pub hit: Vec<u32>,
}

#[derive(Default)]
pub struct Mobs {
    /// Zahl der KI-Schritte seit Start (Test: bleibt 0, solange niemand in der Zitadelle ist)
    pub ai_steps: u64,
    pub list: Vec<Mob>,
    pub next_id: u32,
    /// Boss-Schaden je Konto (für combat::loot_share)
    pub boss_dmg: Vec<(u32, f32)>,
    /// letzte Boss-Erscheinung / Boss-Tod (0 = Zone noch nie aktiv)
    pub boss_at: i64,
    pub minion_at: i64,
    pub shocks: Vec<Shock>,
    last: i64,
    sent_at: i64,
    // wiederverwendete Puffer (keine Allokation je Takt)
    who: Vec<(u32, [f32; 3])>,
    hits: Vec<(u32, f32)>,
    ids: Vec<u32>,
    seen: Vec<u32>,
    view: Vec<u32>,
    states: Vec<MobState>,
}

fn inr(r: [i32; 4], x: f32, z: f32) -> bool {
    x >= r[0] as f32 && z >= r[1] as f32 && x < r[2] as f32 && z < r[3] as f32
}
/// Rechteck je Art: Außenring (0/1), Halle (2), Arena (Boss, Diener).
fn zone_ok(kind: u8, x: f32, z: f32) -> bool {
    match kind {
        0 | 1 => inr(world::CITADEL, x, z) && !inr(world::CITADEL_HALL, x, z) && !inr(world::CITADEL_ARENA, x, z),
        2 => inr(world::CITADEL_HALL, x, z),
        _ => inr(world::CITADEL_ARENA, x, z),
    }
}
fn hdist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
/// Waagrechter Abstand zum Zitadellen-Rechteck (0 = drin).
fn citadel_dist(p: [f32; 3]) -> f32 {
    let [x0, z0, x1, z1] = world::CITADEL;
    let dx = (x0 as f32 - p[0]).max(p[0] - x1 as f32).max(0.0);
    let dz = (z0 as f32 - p[2]).max(p[2] - z1 as f32).max(0.0);
    (dx * dx + dz * dz).sqrt()
}
fn in_cit(p: [f32; 3]) -> bool {
    world::in_citadel(p[0], p[1], p[2])
}
fn size_of(kind: u8) -> f32 {
    rpg::mob(kind).map_or(1.0, |d| d.size)
}

impl Game {
    // ---------- Takt ----------
    pub fn mob_tick(&mut self, now: i64) {
        let active = self.players.values().filter(|p| in_cit(p.p)).count();
        if active == 0 {
            if !self.mobs.list.is_empty() || !self.mobs.seen.is_empty() || !self.mobs.shocks.is_empty() {
                self.mobs_idle();
            }
            return;
        }
        self.mobs.ai_steps += 1;
        let dt = if self.mobs.last > 0 { ((now - self.mobs.last) as f32 / 1000.0).clamp(0.0, 0.25) } else { 0.1 };
        self.mobs.last = now;
        let who = &mut self.mobs.who;
        who.clear();
        who.extend(self.players.values().filter(|p| in_cit(p.p)).map(|p| (p.id, p.p)));
        self.mob_spawn(now);
        self.mob_move(now, dt);
        self.shock_tick(now);
        self.apply_hits(now);
        self.hp_regen(now, dt);
        self.mobs.list.retain(|m| m.dead_at == 0 || now - m.dead_at < 600);
        if now - self.mobs.sent_at >= 100 {
            self.mobs.sent_at = now;
            self.mobs_send(now);
        }
    }

    /// Zone leer: Wesen weg (Speicher bleibt), Zuschauer bekommen einmal eine leere Liste.
    fn mobs_idle(&mut self) {
        let boss = self.mobs.list.iter().any(|m| m.kind == 3 && m.dead_at == 0);
        self.mobs.list.clear();
        self.mobs.shocks.clear();
        self.mobs.boss_dmg.clear();
        self.mobs.last = 0;
        self.mobs.boss_at = 0; // Boss-Uhr startet beim nächsten Betreten neu
        let seen = std::mem::take(&mut self.mobs.seen);
        for &id in &seen {
            self.send(id, &S2C::Mobs(Vec::new()));
            if boss {
                self.hud(id, json!({ "t": "boss", "aus": true }));
            }
        }
        self.mobs.seen = seen;
        self.mobs.seen.clear();
    }

    fn mob_spawn(&mut self, now: i64) {
        let alive = |m: &&Mob| m.dead_at == 0;
        let outer = self.mobs.list.iter().filter(alive).filter(|m| m.kind <= 1).count();
        if outer < cb::OUTER_MOBS {
            let kind = if self.rng.below(3) == 0 { 1 } else { 0 };
            if let Some(p) = self.free_spot(kind) {
                self.spawn_mob(kind, p, now);
            }
        }
        let hall = self.mobs.list.iter().filter(alive).filter(|m| m.kind == 2).count();
        if hall < cb::HALL_MOBS {
            if let Some(p) = self.free_spot(2) {
                self.spawn_mob(2, p, now);
            }
        }
        if self.mobs.boss_at == 0 {
            self.mobs.boss_at = now;
        }
        let boss = self.mobs.list.iter().find(|m| m.kind == 3 && m.dead_at == 0).map(|m| m.hp / m.max.max(1.0));
        match boss {
            None if now - self.mobs.boss_at >= cb::BOSS_EVERY_MS => self.spawn_boss(now),
            Some(frac) if cb::boss_phase(frac) >= 1 && now - self.mobs.minion_at >= 6000 => {
                let minions = self.mobs.list.iter().filter(alive).filter(|m| m.kind == 4).count();
                if minions < cb::BOSS_MINIONS {
                    if let Some(p) = self.free_spot(4) {
                        self.spawn_mob(4, p, now);
                        self.mobs.minion_at = now;
                    }
                }
            }
            _ => {}
        }
    }
    /// Zufällige freie Stelle (Fuß- und Kopfhöhe nicht fest) im Rechteck der Art; ein paar Versuche.
    fn free_spot(&mut self, kind: u8) -> Option<[f32; 3]> {
        let r = match kind {
            0 | 1 => world::CITADEL,
            2 => world::CITADEL_HALL,
            _ => world::CITADEL_ARENA,
        };
        for _ in 0..6 {
            let x = r[0] as f32 + self.rng.below((r[2] - r[0]).max(1) as u64) as f32 + 0.5;
            let z = r[1] as f32 + self.rng.below((r[3] - r[1]).max(1) as u64) as f32 + 0.5;
            if zone_ok(kind, x, z) && self.free_at(x, z) {
                return Some([x, world::CITADEL_Y, z]);
            }
        }
        None
    }
    fn free_at(&self, x: f32, z: f32) -> bool {
        let (bx, by, bz) = (x.floor() as i32, world::CITADEL_Y as i32, z.floor() as i32);
        !world::solid(self.block(bx, by, bz)) && !world::solid(self.block(bx, by + 1, bz))
    }
    fn spawn_mob(&mut self, kind: u8, p: [f32; 3], now: i64) {
        let Some(def) = rpg::mob(kind) else { return };
        self.mobs.next_id = self.mobs.next_id.max(1);
        let id = self.mobs.next_id;
        self.mobs.next_id += 1;
        self.mobs.list.push(Mob { id, kind, p, hp: def.hp, max: def.hp, next_slam: now + 5000, ..Default::default() });
    }
    fn spawn_boss(&mut self, now: i64) {
        let [x0, z0, x1, z1] = world::CITADEL_ARENA;
        let p = [(x0 + x1) as f32 / 2.0, world::CITADEL_Y, (z0 + z1) as f32 / 2.0 - 2.0];
        self.spawn_mob(3, p, now);
        self.mobs.boss_at = now;
        self.mobs.minion_at = now;
        self.mobs.boss_dmg.clear();
        self.boss_hud();
    }

    /// Schritt mit einfacher Kollision; klemmt er, wird je Achse einzeln versucht (Gleiten an Wänden).
    fn mob_step(&self, m: &mut Mob, dx: f32, dz: f32) -> bool {
        for (sx, sz) in [(dx, dz), (dx, 0.0), (0.0, dz)] {
            if sx == 0.0 && sz == 0.0 {
                continue;
            }
            let (nx, nz) = (m.p[0] + sx, m.p[2] + sz);
            if zone_ok(m.kind, nx, nz) && self.free_at(nx, nz) {
                m.p[0] = nx;
                m.p[2] = nz;
                return sx == dx && sz == dz;
            }
        }
        false
    }

    fn mob_move(&mut self, now: i64, dt: f32) {
        for i in 0..self.mobs.list.len() {
            let mut m = self.mobs.list[i];
            let Some(def) = rpg::mob(m.kind) else { continue };
            if m.dead_at > 0 {
                continue;
            }
            // Rückstoß klingt ab
            if m.kb[0] != 0.0 || m.kb[1] != 0.0 {
                let (kx, kz) = (m.kb[0] * dt, m.kb[1] * dt);
                self.mob_step(&mut m, kx, kz);
                m.kb = [m.kb[0] * 0.7, m.kb[1] * 0.7];
                if m.kb[0].abs() + m.kb[1].abs() < 0.2 {
                    m.kb = [0.0, 0.0];
                }
            }
            // nächster Spieler in der Zitadelle
            let aggro = match m.kind {
                3 => 30.0,
                2 => 12.0,
                _ => 14.0,
            };
            let mut target: Option<(u32, [f32; 3], f32)> = None;
            for &(id, p) in &self.mobs.who {
                let d = hdist(p, m.p);
                if d < aggro && target.map_or(true, |t| d < t.2) {
                    target = Some((id, p, d));
                }
            }
            let Some((tid, tp, d)) = target else {
                m.windup_until = 0;
                m.charge_until = 0;
                self.mobs.list[i] = m;
                continue;
            };
            let (dx, dz) = (tp[0] - m.p[0], tp[2] - m.p[2]);
            let l = d.max(0.01);
            let dir = [dx / l, dz / l];
            m.yaw = (-dx).atan2(-dz);
            let low = (tp[1] - world::CITADEL_Y).abs() < 2.5 + def.size;
            let in_reach = d <= def.reach + 0.3 && low;
            let phase = cb::boss_phase(m.hp / m.max.max(1.0));
            let cd = if m.kind == 3 && phase == 2 { cb::MOB_HIT_CD_MS * 3 / 5 } else { cb::MOB_HIT_CD_MS };
            let mut chase = true;
            if m.kind == 2 {
                // Wächter: ausholen (TELEGRAPH_MS), dann Ansturm
                if m.charge_until > now {
                    chase = false;
                    let s = cb::CHARGE_SPEED * dt;
                    let (cx, cz) = (m.charge[0] * s, m.charge[1] * s);
                    if !self.mob_step(&mut m, cx, cz) {
                        m.charge_until = 0;
                    }
                    if hdist(tp, m.p) <= def.reach + 0.3 && low && now - m.hit_at >= cd {
                        m.hit_at = now;
                        m.charge_until = 0;
                        self.mobs.hits.push((tid, def.dmg * 1.5));
                    }
                } else if m.windup_until > 0 {
                    chase = false;
                    if now >= m.windup_until {
                        m.windup_until = 0;
                        m.charge_until = now + 700;
                        m.charge = dir;
                    }
                } else if (3.0..10.0).contains(&d) && now - m.hit_at >= cd * 2 {
                    chase = false;
                    m.windup_until = now + cb::TELEGRAPH_MS;
                }
            } else if m.kind == 3 {
                // Boss: Stoßwelle mit 1 s Vorwarnung
                if m.slam_at > 0 {
                    chase = false;
                    if now >= m.slam_at {
                        m.slam_at = 0;
                        m.next_slam = now + if phase == 2 { 4000 } else { 7000 };
                        let at = m.p;
                        self.mobs.shocks.push(Shock { at, start: now, hit: Vec::new() });
                        self.send_near(at, VIEW, &S2C::Shock { at, r: cb::SHOCK_MAX_R });
                    }
                } else if now >= m.next_slam && d < cb::SHOCK_MAX_R {
                    chase = false;
                    m.slam_at = now + 1000;
                }
            }
            if chase {
                if d > def.reach * 0.8 {
                    let s = def.speed * dt;
                    self.mob_step(&mut m, dir[0] * s, dir[1] * s);
                }
                if in_reach && now - m.hit_at >= cd {
                    m.hit_at = now;
                    self.mobs.hits.push((tid, def.dmg));
                }
            }
            self.mobs.list[i] = m;
        }
    }

    /// Stoßwellen wachsen; wer am Boden im Band steht, nimmt einmal Schaden (Springen weicht aus).
    fn shock_tick(&mut self, now: i64) {
        let r_of = |s: &Shock| (now - s.start).max(0) as f32 / 1000.0 * cb::SHOCK_SPEED;
        for s in 0..self.mobs.shocks.len() {
            let (at, r) = (self.mobs.shocks[s].at, r_of(&self.mobs.shocks[s]));
            for k in 0..self.mobs.who.len() {
                let (id, p) = self.mobs.who[k];
                let grounded = p[1] - world::CITADEL_Y < cb::SHOCK_JUMP_Y;
                if grounded && (hdist(p, at) - r).abs() < cb::SHOCK_BAND && !self.mobs.shocks[s].hit.contains(&id) {
                    self.mobs.shocks[s].hit.push(id);
                    self.mobs.hits.push((id, cb::SHOCK_DMG));
                }
            }
        }
        self.mobs.shocks.retain(|s| r_of(s) <= cb::SHOCK_MAX_R + cb::SHOCK_BAND);
    }
    fn apply_hits(&mut self, now: i64) {
        for k in 0..self.mobs.hits.len() {
            let (id, dmg) = self.mobs.hits[k];
            self.hurt_player(id, dmg, now);
        }
        self.mobs.hits.clear();
    }
    fn hp_regen(&mut self, now: i64, dt: f32) {
        for k in 0..self.mobs.who.len() {
            let id = self.mobs.who[k].0;
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
    }

    /// S2C::Mobs an alle bis VIEW um die Zitadelle; wer den Bereich verlässt, bekommt einmal eine leere Liste.
    fn mobs_send(&mut self, now: i64) {
        let mut view = std::mem::take(&mut self.mobs.view);
        view.clear();
        view.extend(self.players.values().filter(|p| citadel_dist(p.p) < VIEW).map(|p| p.id));
        let mut seen = std::mem::take(&mut self.mobs.seen);
        for &id in &seen {
            if !view.contains(&id) {
                self.send(id, &S2C::Mobs(Vec::new()));
            }
        }
        let mut st = std::mem::take(&mut self.mobs.states);
        st.clear();
        st.extend(self.mobs.list.iter().map(|m| {
            let mut fl = 0;
            if m.hurt_until > now {
                fl |= MOB_HURT;
            }
            if m.windup_until > 0 {
                fl |= MOB_WINDUP;
            }
            if m.slam_at > 0 {
                fl |= MOB_SLAM;
            }
            if m.dead_at > 0 {
                fl |= MOB_DEAD;
            }
            MobState { id: m.id, kind: m.kind, p: m.p, yaw: m.yaw, hp: (m.hp / m.max.max(1.0) * 1000.0).clamp(0.0, 1000.0) as u16, fl }
        }));
        let msg = S2C::Mobs(st);
        for &id in &view {
            self.send(id, &msg);
        }
        if let S2C::Mobs(v) = msg {
            self.mobs.states = v;
        }
        seen.clear();
        self.mobs.seen = view;
        self.mobs.view = seen;
    }

    fn send_near(&mut self, at: [f32; 3], r: f32, m: &S2C) {
        let mut ids = std::mem::take(&mut self.mobs.ids);
        ids.clear();
        ids.extend(self.players.values().filter(|p| hdist(p.p, at) < r).map(|p| p.id));
        for &id in &ids {
            self.send(id, m);
        }
        self.mobs.ids = ids;
    }
    fn hud_citadel(&mut self, v: Value) {
        let mut ids = std::mem::take(&mut self.mobs.ids);
        ids.clear();
        ids.extend(self.players.values().filter(|p| in_cit(p.p)).map(|p| p.id));
        for &id in &ids {
            self.hud(id, v.clone());
        }
        self.mobs.ids = ids;
    }
    fn boss_hud(&mut self) {
        let Some(m) = self.mobs.list.iter().find(|m| m.kind == 3 && m.dead_at == 0).copied() else { return };
        let name = rpg::mob(3).map_or("Boss", |d| d.name);
        let phase = cb::boss_phase(m.hp / m.max.max(1.0));
        self.hud_citadel(json!({ "t": "boss", "name": name, "hp": m.hp.max(0.0).round(), "max": m.max, "phase": phase }));
    }

    // ---------- Spieler nimmt Schaden ----------
    /// Treffer auf einen Spieler (roh, vor Verteidigung). Schild schützt; Leben ≤ 0 → Wiederbelebung ohne Verlust.
    pub fn hurt_player(&mut self, id: u32, raw: f32, now: i64) {
        let Some(p) = self.players.get_mut(&id) else { return };
        if now < p.shield_until {
            return;
        }
        let dmg = cb::taken(raw, p.stats.defense);
        p.hp -= dmg;
        p.hurt_at = now;
        let max = if p.stats.max_hp > 0.0 { p.stats.max_hp } else { campus_shared::stats::BASE_HP };
        let (hp, pos) = (p.hp, p.p);
        self.send(id, &S2C::Damage { at: [pos[0], pos[1] + 2.0, pos[2]], n: dmg.round() as u32, crit: false, mob: 0 });
        if hp <= 0.0 {
            self.player_died(id, now);
        } else {
            self.send(id, &S2C::Health { hp, max, shield_ms: 0 });
        }
    }
    fn player_died(&mut self, id: u32, now: i64) {
        let Some(p) = self.players.get_mut(&id) else { return };
        let max = if p.stats.max_hp > 0.0 { p.stats.max_hp } else { campus_shared::stats::BASE_HP };
        p.hp = max;
        p.shield_until = now + cb::SHIELD_MS;
        let s = world::SPOTS[4];
        self.spawn_at(id, s.1, s.2);
        self.hud(id, json!({ "t": "tod" }));
        self.send(id, &S2C::Health { hp: max, max, shield_ms: cb::SHIELD_MS as u32 });
    }

    // ---------- Angriffe ----------
    pub fn attack(&mut self, id: u32, mob: u32, now: i64) -> R<()> {
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        let (pos, swing_at, st) = (p.p, p.swing_at, p.stats);
        let Some(i) = self.mobs.list.iter().position(|m| m.id == mob && m.dead_at == 0) else { return no(404, "Dieses Wesen gibt es nicht mehr.") };
        let m = self.mobs.list[i];
        let size = size_of(m.kind);
        let eye = [pos[0], pos[1] + 1.6, pos[2]];
        let c = [m.p[0], m.p[1] + 0.5 * size, m.p[2]];
        let d = ((eye[0] - c[0]).powi(2) + (eye[1] - c[1]).powi(2) + (eye[2] - c[2]).powi(2)).sqrt();
        if d > cb::MELEE_REACH + size / 2.0 {
            return no(403, "Zu weit weg.");
        }
        if now - swing_at < cb::SWING_CD_MS {
            return no(429, "Zu schnell.");
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.swing_at = now;
        }
        let (dmg, crit) = cb::player_hit(&st, self.rng.next() as u32);
        self.hit_mob(id, i, dmg, crit, pos, now);
        Ok(())
    }

    pub fn shoot(&mut self, id: u32, dir: [f32; 3], now: i64) -> R<()> {
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
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
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
        // Blöcke blocken: Abstand bis zum ersten festen Block
        let wall = world::raycast(|x, y, z| self.block(x, y, z), eye, d, cb::RANGED_REACH).map_or(cb::RANGED_REACH, |(c, _)| {
            let q = [c[0] as f32 + 0.5 - eye[0], c[1] as f32 + 0.5 - eye[1], c[2] as f32 + 0.5 - eye[2]];
            ((q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt() - 0.5).max(0.0)
        });
        let mut best: Option<(usize, f32)> = None;
        for (i, m) in self.mobs.list.iter().enumerate().filter(|(_, m)| m.dead_at == 0) {
            let size = size_of(m.kind);
            let c = [m.p[0], m.p[1] + 0.5 * size, m.p[2]];
            if let Some(t) = cb::ray_sphere(eye, d, c, 0.6 * size, wall) {
                if best.map_or(true, |b| t < b.1) {
                    best = Some((i, t));
                }
            }
        }
        let t = best.map_or(wall, |b| b.1);
        let to = [eye[0] + d[0] * t, eye[1] + d[1] * t, eye[2] + d[2] * t];
        self.send_near(eye, VIEW, &S2C::Shot { from: eye, to, kind: u8::from(staff) });
        if let Some((i, _)) = best {
            let (dmg, crit) = cb::player_hit(&st, self.rng.next() as u32);
            self.hit_mob(id, i, dmg, crit, pos, now);
        }
        Ok(())
    }

    /// Schaden aufs Wesen, Rückstoß, Aufblitzen, Zahl an alle in der Nähe; Boss-Leiste; Tod → Beute.
    fn hit_mob(&mut self, uid: u32, i: usize, dmg: f32, crit: bool, from: [f32; 3], now: i64) {
        let mut m = self.mobs.list[i];
        let size = size_of(m.kind);
        let dealt = dmg.min(m.hp.max(0.0));
        m.hp -= dmg;
        m.hurt_until = now + cb::FLASH_MS;
        if m.kind != 3 {
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
        self.mobs.list[i] = m;
        let top = [m.p[0], m.p[1] + size * 1.1 + 0.3, m.p[2]];
        self.send_near(top, 32.0, &S2C::Damage { at: top, n: dmg.round() as u32, crit, mob: m.id });
        if m.kind == 3 {
            match self.mobs.boss_dmg.iter().position(|e| e.0 == uid) {
                Some(k) => self.mobs.boss_dmg[k].1 += dealt,
                None => self.mobs.boss_dmg.push((uid, dealt)),
            }
            if !dead {
                self.boss_hud();
            }
        }
        if dead {
            self.mob_died(uid, m, now);
        }
    }
    fn mob_died(&mut self, uid: u32, m: Mob, now: i64) {
        if m.kind != 3 {
            return self.loot(uid, m.kind, m.p);
        }
        let who = cb::loot_share(&self.mobs.boss_dmg);
        self.mobs.boss_dmg.clear();
        self.mobs.boss_at = now;
        self.hud_citadel(json!({ "t": "boss", "aus": true }));
        for u in who {
            self.loot(u, 3, m.p);
        }
    }
    /// Beute für einen Empfänger: Drops (Promille), Taler, Gesellschaft-EP, Questziel.
    fn loot(&mut self, uid: u32, kind: u8, at: [f32; 3]) {
        let Some(def) = rpg::mob(kind) else { return };
        let mut items: Vec<u16> = Vec::new();
        for &(item, n, chance) in def.drops {
            if (self.rng.below(1000) as u32) < chance {
                if let Ok(total) = self.gain(uid, item, n) {
                    self.send(uid, &S2C::Inv { item, n: total });
                    items.extend(std::iter::repeat(item).take(n.min(8) as usize));
                }
            }
        }
        let st = self.players.get(&uid).map(|p| p.stats).filter(|s| s.max_hp > 0.0).unwrap_or_else(|| self.stats(uid));
        let taler = (def.taler as f32 * (1.0 + st.taler_bonus / 100.0)).round().max(0.0) as i64;
        let _ = self.pay(uid, taler, "kampf");
        let _ = self.add_xp(uid, SK_KAMPF, def.xp);
        self.hud(uid, json!({ "t": "aktion", "text": format!("{} besiegt! +{taler} Taler | +{} Gesellschaft-EP", def.name, def.xp) }));
        if !st.telekinese && !items.is_empty() {
            self.send(uid, &S2C::Drops { at: [at[0], at[1] + 0.5, at[2]], items });
        }
        self.quest_hit(uid, |g| matches!(g, Goal::Kill(k, _) if k == kind).then_some(1));
    }

    pub fn boss_summon(&mut self, id: u32) -> R<()> {
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        if !in_cit(p.p) {
            return no(403, "Den Prüfungs-Altar kannst du nur in der Zitadelle benutzen.");
        }
        if self.mobs.list.iter().any(|m| m.kind == 3 && m.dead_at == 0) {
            return no(409, "Der Archon ist schon erwacht.");
        }
        let now = self.now();
        self.spawn_boss(now);
        self.hud_citadel(json!({ "t": "toast", "art": "gut", "text": "Der Große Prüfungs-Archon erwacht!" }));
        Ok(())
    }

    pub fn combat_join(&mut self, id: u32) {
        self.refresh_stats(id);
        let now = self.now();
        let Some(p) = self.players.get(&id) else { return };
        let (hp, max, shield_ms) = (p.hp, p.stats.max_hp, (p.shield_until - now).max(0) as u32);
        self.send(id, &S2C::Health { hp, max, shield_ms });
    }

    pub fn mobs_get(&mut self, _uid: u32, op: &str) -> Option<R<Value>> {
        if op != "kampf" {
            return None;
        }
        let wesen = self.mobs.list.iter().filter(|m| m.dead_at == 0).count();
        let boss = self.mobs.list.iter().find(|m| m.kind == 3 && m.dead_at == 0).map_or(Value::Null, |m| {
            json!({ "name": rpg::mob(3).map_or("Boss", |d| d.name), "hp": m.hp.round(), "max": m.max, "phase": cb::boss_phase(m.hp / m.max.max(1.0)),
                "teilnehmer": self.mobs.boss_dmg.len() })
        });
        Some(Ok::<Value, E>(json!({ "wesen": wesen, "boss": boss, "ki_schritte": self.mobs.ai_steps })))
    }
}
