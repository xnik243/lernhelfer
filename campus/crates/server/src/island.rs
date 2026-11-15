//! Phasen 17/19/20 (Besitz: Agent ISLAND): Ackerbau, Funktionsblöcke (Sprungplatte, Teleporter, Schild, Ständer-Platz),
//! Verzaubern, Co-op, Likes, Campus-Questbuch. Signaturen sind Vertrag (CONTRACT-16-20.md §S-ISLAND).
//! Nichts hier läuft im Takt: Pflanzen-Stufen kommen allein aus Zeitstempeln.
use crate::econ::int;
use crate::game::{plot_may, Game, Plot};
use crate::{no, E, R};
use campus_shared::enchanting::{self as en, N_ENCH};
use campus_shared::farming;
use campus_shared::farming_data::CROPS;
use campus_shared::items::item_name;
use campus_shared::proto::{CropState, SignState, StandState, S2C};
use campus_shared::questline_data::CHAPTERS;
use campus_shared::rpg;
use campus_shared::skyblock::{self as sb, Goal};
use campus_shared::stats::SET_BAUERN;
use campus_shared::world;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

/// Fähigkeit Naturwissenschaften (sb::SKILLS[2]) bekommt die Ernte-EP.
const SK_NATUR: u8 = 2;
const TELE_LOCK_MS: i64 = 1500;
const ALTAR_REACH: f32 = 8.0;
const TILL_REACH: f32 = 5.0;
const SIGN_LINES: usize = 4;
const SIGN_CHARS: usize = 24;
const COOP_MAX: i64 = 4;
const STAMPS: usize = 6;
const ROMAN: [&str; 7] = ["", "I", "II", "III", "IV", "V", "VI"];

fn sign_lines(d: &str) -> Vec<String> {
    serde_json::from_str(d).unwrap_or_default()
}
fn stand_items(d: &str) -> [u16; 7] {
    serde_json::from_str(d).unwrap_or([0; 7])
}

impl Game {
    // ---------- Grundlagen ----------
    fn grp_of(&self, uid: u32) -> u32 {
        match self.players.get(&uid) {
            Some(p) => p.group,
            None => self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0)).unwrap_or(0),
        }
    }
    fn plot_here(&self, x: i32, z: i32) -> Option<&Plot> {
        world::slot_at(x, z).and_then(|s| self.plots.iter().find(|p| p.slot == s))
    }
    /// Insel-Recht (Besitz, Co-op/Mitbauer, Gruppe) an dieser Stelle.
    fn may_here(&self, uid: u32, x: i32, z: i32) -> bool {
        let grp = self.grp_of(uid);
        self.plot_here(x, z).is_some_and(|p| plot_may(p, uid, grp))
    }

    // ---------- Bewegung: Sprungplatte, Teleporter, Insel betreten ----------
    pub fn on_move(&mut self, id: u32, now: i64) {
        let Some((p, sent, pad_at)) = self.players.get(&id).map(|pl| (pl.p, pl.island_sent, pl.pad_at)) else { return };
        let (x, y, z) = (p[0].floor() as i32, (p[1] - 0.2).floor() as i32, p[2].floor() as i32);
        let under = self.block(x, y, z);
        if under == world::SPRUNGPLATTE {
            if let Some(pl) = self.players.get_mut(&id) {
                pl.boost_until = pl.boost_until.max(now + 4000);
            }
        } else if world::TELEPORTER.contains(&under) {
            // Sperre läuft weiter, solange man auf einem Pad steht (der Browser meldet 10× je Sekunde) – kein Ping-Pong
            if now - pad_at >= TELE_LOCK_MS {
                self.tele_from(id, x, y, z, under);
            }
            if let Some(pl) = self.players.get_mut(&id) {
                pl.pad_at = now;
            }
        }
        if world::slot_at(p[0] as i32, p[2] as i32) != sent {
            self.island_refresh_for(id);
        }
    }
    fn tele_from(&mut self, id: u32, x: i32, y: i32, z: i32, kind: u8) {
        let Some(plot) = self.plot_here(x, z).map(|p| p.id) else { return };
        let Ok(pads) = self.pads(plot, kind) else { return };
        let to = match pads.iter().position(|&q| q == [x, y, z]) {
            Some(i) => pads[(i + 1) % pads.len()],
            None => match pads.first() {
                Some(&q) => q,
                None => return,
            },
        };
        if to == [x, y, z] || self.block(to[0], to[1], to[2]) != kind {
            return;
        }
        let yaw = self.players.get(&id).map_or(0.0, |p| p.yaw);
        self.spawn_at(id, [to[0] as f32 + 0.5, to[1] as f32 + 1.05, to[2] as f32 + 0.5], yaw);
        self.hud(id, json!({ "t": "tele" }));
    }
    fn pads(&self, plot: u32, kind: u8) -> R<Vec<[i32; 3]>> {
        Ok(self
            .db
            .prepare_cached("select x, y, z from island_functional_blocks where plot = ? and kind = ? order by x, y, z")?
            .query_map(params![plot, kind], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]))?
            .collect::<Result<_, _>>()?)
    }

    // ---------- Insel-Inhalt (S2C::Island) ----------
    pub fn island_join(&mut self, id: u32) {
        self.island_refresh_for(id);
    }
    fn island_msg(&self, plot: u32) -> R<S2C> {
        let crops: Vec<CropState> = self
            .db
            .prepare_cached("select x, y, z, kind, planted, fert from island_crops where plot = ?")?
            .query_map([plot], |r| {
                let (kind, fert): (usize, u32) = (r.get(3)?, r.get(5)?);
                let grow = farming::grow_ms(kind, fert).clamp(0, u32::MAX as i64) as u32;
                Ok(CropState { x: r.get(0)?, y: r.get(1)?, z: r.get(2)?, kind: kind as u8, planted: r.get(4)?, grow_ms: grow })
            })?
            .collect::<Result<_, _>>()?;
        let rows: Vec<(i16, i16, i16, u8, String)> = self
            .db
            .prepare_cached("select x, y, z, kind, data from island_functional_blocks where plot = ? and kind in (?, ?)")?
            .query_map(params![plot, world::SCHILD, world::STAENDER], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<Result<_, _>>()?;
        let (mut signs, mut stands) = (Vec::new(), Vec::new());
        for (x, y, z, kind, data) in rows {
            if kind == world::SCHILD {
                signs.push(SignState { x, y, z, lines: sign_lines(&data) });
            } else {
                stands.push(StandState { x, y, z, items: stand_items(&data) });
            }
        }
        Ok(S2C::Island { now: self.now(), crops, signs, stands })
    }
    /// Inhalt der Insel `plot` neu an alle, die gerade auf ihrem Platz stehen.
    pub fn island_refresh(&mut self, plot: u32) {
        let Some(slot) = self.plots.iter().find(|p| p.id == plot).map(|p| p.slot) else { return };
        let Ok(m) = self.island_msg(plot) else { return };
        let ids: Vec<u32> = self.players.values().filter(|p| world::slot_at(p.p[0] as i32, p.p[2] as i32) == Some(slot)).map(|p| p.id).collect();
        for id in ids {
            self.send(id, &m);
            if let Some(p) = self.players.get_mut(&id) {
                p.island_sent = Some(slot);
            }
        }
    }
    /// Inhalt des Platzes unter `id` an `id` (leer im Hub bzw. auf freiem Platz).
    pub fn island_refresh_for(&mut self, id: u32) {
        let Some(p) = self.players.get(&id).map(|p| p.p) else { return };
        let slot = world::slot_at(p[0] as i32, p[2] as i32);
        let plot = slot.and_then(|s| self.plots.iter().find(|q| q.slot == s)).map(|q| q.id);
        let m = plot.and_then(|pl| self.island_msg(pl).ok()).unwrap_or(S2C::Island { now: self.now(), crops: Vec::new(), signs: Vec::new(), stands: Vec::new() });
        self.send(id, &m);
        if let Some(pl) = self.players.get_mut(&id) {
            pl.island_sent = slot;
        }
    }

    // ---------- Setzen und Abbauen ----------
    pub fn before_place(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8) -> R<()> {
        if b == world::ALTAR || b == world::BOSS_ALTAR {
            return no(400, "Diesen Block kannst du nicht setzen.");
        }
        if farming::crop_of(b).is_some() {
            if !self.may_here(id, x, z) {
                return no(403, "Hier darfst du nicht pflanzen.");
            }
            if self.block(x, y - 1, z) != world::ACKERBODEN {
                return no(409, "Saat wächst nur auf Ackerboden. Hacke erst Wiese oder Erde um.");
            }
        }
        Ok(())
    }
    pub fn after_place(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8) {
        let now = self.now();
        let Some((plot, own)) = self.plot_here(x, z).map(|p| (p.id, p.owner == id)) else { return };
        let mut refresh = false;
        if let Some(c) = farming::crop_of(b) {
            let bauer = own && self.players.get(&id).is_some_and(|p| p.stats.set == SET_BAUERN);
            let fert = if bauer { farming::FERT_BAUERN } else { farming::FERT_BASE };
            let _ = self.db.execute(
                "insert or replace into island_crops (x, y, z, plot, kind, planted, fert) values (?, ?, ?, ?, ?, ?, ?)",
                params![x, y, z, plot, c as i64, now, fert],
            );
            self.quest_hit(id, |g| matches!(g, Goal::Plant(_)).then_some(1));
            refresh = true;
        } else if b == world::TRUHE || b == world::GROSSE_TRUHE {
            self.quest_hit(id, |g| (g == Goal::Chest).then_some(1));
        } else if b == world::SCHILD || b == world::STAENDER || b == world::SPRUNGPLATTE || world::TELEPORTER.contains(&b) {
            let data = match b {
                world::SCHILD => "[]",
                world::STAENDER => "[0,0,0,0,0,0,0]",
                _ => "",
            };
            let _ = self.db.execute("insert or replace into island_functional_blocks (x, y, z, plot, kind, data) values (?, ?, ?, ?, ?, ?)", params![x, y, z, plot, b, data]);
            if b == world::SCHILD {
                self.hud(id, json!({ "t": "schild_neu", "x": x, "y": y, "z": z, "zeilen": [] }));
            }
            refresh = b == world::SCHILD || b == world::STAENDER;
        }
        if own {
            self.quest_hit(id, |g| matches!(g, Goal::Place(_)).then_some(1));
        }
        if refresh {
            self.island_refresh(plot);
        }
    }
    /// true = Abbau schon vollständig erledigt (z. B. Ernte), sonst läuft der normale Abbau weiter.
    pub fn before_break(&mut self, id: u32, x: i32, y: i32, z: i32, cur: u8) -> R<bool> {
        if cur == world::TRUHE || cur == world::GROSSE_TRUHE {
            self.chest_can_break(id, x, y, z)?;
            return Ok(false);
        }
        if let Some(kind) = farming::crop_of(cur) {
            self.harvest_at(id, x, y, z, cur, kind)?;
            return Ok(true);
        }
        let row: Option<(u32, String)> =
            self.db.query_row("select plot, data from island_functional_blocks where x = ? and y = ? and z = ?", params![x, y, z], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let Some((plot, data)) = row else { return Ok(false) };
        if cur == world::STAENDER && stand_items(&data).iter().any(|&i| i != 0) {
            return no(409, "Der Rüstungsständer ist nicht leer. Nimm erst die Ausrüstung ab.");
        }
        self.db.execute("delete from island_functional_blocks where x = ? and y = ? and z = ?", params![x, y, z])?;
        if cur == world::SCHILD || cur == world::STAENDER {
            self.island_refresh(plot);
        }
        Ok(false)
    }
    /// Pflanze abbauen: reif → Ertrag + EP (Segens-Hacke: Pflanze bleibt und wächst neu), unreif → nur die Saat zurück.
    fn harvest_at(&mut self, id: u32, x: i32, y: i32, z: i32, seed: u8, kind0: usize) -> R<()> {
        if !self.may_here(id, x, z) {
            return no(403, "Diese Pflanze gehört nicht dir.");
        }
        let now = self.now();
        let row: Option<(usize, i64, u32, u32)> = self
            .db
            .query_row("select kind, planted, fert, plot from island_crops where x = ? and y = ? and z = ?", params![x, y, z], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?;
        let here = self.plot_here(x, z).map_or(0, |p| p.id);
        let (kind, planted, fert, plot) = row.unwrap_or((kind0, now, farming::FERT_BASE, here));
        let crop = CROPS.get(kind).ok_or(E(500, "Unbekannte Pflanze.".into()))?;
        let ripe = farming::stage(planted, now, farming::grow_ms(kind, fert)) == farming::STAGES - 1;
        let keep = ripe && self.held_item(id) == rpg::SEGEN_HACKE;
        let fortune = self.players.get(&id).map_or(0.0, |p| p.stats.crop_fortune);
        let roll = self.rng.next() as u32;
        let n = if ripe { farming::harvest(kind, fortune, roll) } else { 0 };
        self.tx(|g| {
            if n > 0 {
                g.gain(id, crop.product, n)?;
                g.add_xp(id, SK_NATUR, crop.xp)?;
            }
            if keep {
                g.db.execute("update island_crops set planted = ? where x = ? and y = ? and z = ?", params![now, x, y, z])?;
            } else {
                g.db.execute("delete from island_crops where x = ? and y = ? and z = ?", params![x, y, z])?;
                g.give(id, seed as u16, 1)?;
            }
            Ok(())
        })?;
        if !keep {
            self.set_block(x, y, z, 0, id);
            let k = self.inv(id, seed as u16);
            self.send(id, &S2C::Inv { item: seed as u16, n: k });
        }
        if n > 0 {
            let k = self.inv(id, crop.product);
            self.send(id, &S2C::Inv { item: crop.product, n: k });
            self.hud(id, json!({ "t": "aktion", "text": format!("+{n} {} | +{} Naturwissenschaften-XP", item_name(crop.product), crop.xp) }));
        }
        if ripe {
            self.quest_hit(id, |g| matches!(g, Goal::Harvest(_)).then_some(1));
        }
        self.island_refresh(plot);
        Ok(())
    }

    // ---------- [E] benutzen, Hacke ----------
    pub fn use_block(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8) -> R<()> {
        match b {
            world::ALTAR => {
                self.hud(id, json!({ "t": "oeffne", "reiter": "Verzaubern" }));
                Ok(())
            }
            world::BOSS_ALTAR => self.boss_summon(id),
            world::KATA_PORTAL => self.dungeon_portal(id),
            world::ARENA_PORTAL => self.duel_cmd(id, ""),
            world::STAENDER => self.stand_open(id, x, y, z),
            world::SCHILD => {
                let zeilen = self.sign_at(x, y, z);
                if self.may_here(id, x, z) {
                    self.hud(id, json!({ "t": "schild_neu", "x": x, "y": y, "z": z, "zeilen": zeilen }));
                } else {
                    self.hud(id, json!({ "t": "schild", "zeilen": zeilen }));
                }
                Ok(())
            }
            _ => no(404, "Hier steht keine Truhe."),
        }
    }
    fn sign_at(&self, x: i32, y: i32, z: i32) -> Vec<String> {
        self.db
            .query_row("select data from island_functional_blocks where x = ? and y = ? and z = ? and kind = ?", params![x, y, z, world::SCHILD], |r| r.get::<_, String>(0))
            .map(|d| sign_lines(&d))
            .unwrap_or_default()
    }
    /// Hacke: Wiese/Erde im Blick (5 Blöcke) wird Ackerboden.
    pub fn till(&mut self, id: u32, tool: u16, dir: [f32; 3]) -> R<()> {
        let t = sb::tool(tool).ok_or(E(400, "Unbekanntes Werkzeug.".into()))?;
        let now = self.now();
        let ti = sb::TOOLS.iter().position(|x| x.id == tool).unwrap_or(0);
        let Some((pos, grp, last)) = self.players.get(&id).map(|p| (p.p, p.group, p.tool_at[ti])) else { return no(401, "Nicht verbunden.") };
        if self.inv(id, tool) == 0 {
            return no(403, "Diese Hacke hast du nicht.");
        }
        if now - last < t.cooldown_ms {
            return no(429, "Noch nicht wieder bereit.");
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !len.is_finite() || len < 0.5 {
            return no(400, "Richtung fehlt.");
        }
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let eye = [pos[0], pos[1] + 1.62, pos[2]];
        let (hit, _) = world::raycast(|x, y, z| self.block(x, y, z), eye, d, TILL_REACH).ok_or(E(400, "Zielt auf keinen Block.".into()))?;
        let [x, y, z] = hit;
        let cur = self.block(x, y, z);
        if cur != world::WIESE && cur != world::ERDE {
            return no(400, "Umhacken geht nur bei Wiese oder Erde.");
        }
        if world::solid(self.block(x, y + 1, z)) {
            return no(409, "Über dem Boden muss Platz sein.");
        }
        if world::anchor(x, y, z) || !self.may_edit(id, grp, x, z, cur, false) {
            return no(403, "Hier darfst du nicht ackern.");
        }
        self.set_block(x, y, z, world::ACKERBODEN, id);
        if let Some(p) = self.players.get_mut(&id) {
            p.tool_at[ti] = now;
        }
        self.quest_hit(id, |g| (g == Goal::UseTool(tool)).then_some(1));
        Ok(())
    }

    // ---------- Verzaubern ----------
    pub fn ench(&self, uid: u32, item: u16) -> [u8; N_ENCH] {
        let mut l = [0u8; N_ENCH];
        if let Ok(mut q) = self.db.prepare_cached("select ench, level from item_ench where user = ? and item = ?") {
            if let Ok(rows) = q.query_map(params![uid, item], |r| Ok((r.get::<_, usize>(0)?, r.get::<_, u8>(1)?))) {
                for (e, lv) in rows.flatten() {
                    if e < N_ENCH {
                        l[e] = lv;
                    }
                }
            }
        }
        l
    }
    fn enchant(&mut self, uid: u32, item: u16, e: u8) -> R<Value> {
        if e as usize >= N_ENCH || !en::applies(e, item) {
            return no(400, "Diese Verzauberung passt nicht zu diesem Gegenstand.");
        }
        if self.inv(uid, item) == 0 && !self.gear(uid).contains(&item) {
            return no(403, "Diesen Gegenstand hast du nicht.");
        }
        let p = self.players.get(&uid).map(|p| p.p).ok_or(E(401, "Nicht verbunden.".into()))?;
        let a = world::ALTAR_AT;
        let d2 = (p[0] - a[0] as f32 - 0.5).powi(2) + (p[1] - a[1] as f32).powi(2) + (p[2] - a[2] as f32 - 0.5).powi(2);
        if d2 > ALTAR_REACH * ALTAR_REACH {
            return no(403, "Geh näher an den Verzauberungs-Altar in der Bibliothek.");
        }
        let level = self.ench(uid, item)[e as usize].saturating_add(1);
        // Stufe VI (Phase 24): nur mit einem Folianten aus den Katakomben, der alle anderen Kosten ersetzt
        if level == en::TIER6 && en::def(e).is_some_and(|d| d.max == 5) {
            self.tx(|g| {
                g.give(uid, rpg::FOLIANT_VI, -1).map_err(|_| E(402, "Stufe VI braucht einen Folianten der Stufe VI aus den Prüfungs-Katakomben.".into()))?;
                g.db.execute("insert into item_ench (user, item, ench, level) values (?, ?, ?, ?) on conflict (user, item, ench) do update set level = excluded.level", params![uid, item, e, level])?;
                Ok(())
            })?;
            let n = self.inv(uid, rpg::FOLIANT_VI);
            self.send(uid, &S2C::Inv { item: rpg::FOLIANT_VI, n });
            self.refresh_stats(uid);
            self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("{} VI auf {}!", en::def(e).map_or("", |d| d.name), item_name(item)) }));
            return Ok(json!({ "ok": true, "item": item, "stufen": self.ench(uid, item), "buch": false }));
        }
        let (fach, splitter, kristalle) = en::cost(e, level).ok_or(E(409, "Diese Verzauberung hat schon die höchste Stufe.".into()))?;
        let shard = sb::SHARD + fach as u16;
        let book = self.inv(uid, rpg::VERZ_BUCH) > 0;
        self.tx(|g| {
            if book {
                g.give(uid, rpg::VERZ_BUCH, -1)?;
            } else {
                g.give(uid, shard, -(splitter as i64)).map_err(|_| E(402, format!("Dafür brauchst du {splitter} {}.", item_name(shard))))?;
            }
            g.give(uid, sb::FOKUS_KRISTALL, -(kristalle as i64)).map_err(|_| E(402, format!("Dafür brauchst du {kristalle} Fokus-Kristall{}.", if kristalle == 1 { "" } else { "e" })))?;
            g.db.execute(
                "insert into item_ench (user, item, ench, level) values (?, ?, ?, ?) on conflict (user, item, ench) do update set level = excluded.level",
                params![uid, item, e, level],
            )?;
            Ok(())
        })?;
        for i in [rpg::VERZ_BUCH, shard, sb::FOKUS_KRISTALL] {
            let n = self.inv(uid, i);
            self.send(uid, &S2C::Inv { item: i, n });
        }
        self.quest_hit(uid, |g| (g == Goal::Enchant).then_some(1));
        self.refresh_stats(uid);
        let name = en::def(e).map_or("", |d| d.name);
        let stufe = ROMAN.get(level as usize).copied().unwrap_or("");
        self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("{name} {stufe} auf {}!", item_name(item)) }));
        Ok(json!({ "ok": true, "item": item, "stufen": self.ench(uid, item), "buch": book }))
    }
    fn enchant_post(&mut self, uid: u32, b: &Value) -> R<Value> {
        let item = u16::try_from(int(b, "item")?).map_err(|_| E(400, "Gegenstand ungültig.".into()))?;
        let e = u8::try_from(int(b, "ench")?).map_err(|_| E(400, "Verzauberung ungültig.".into()))?;
        self.enchant(uid, item, e)
    }
    fn enchants(&self, uid: u32) -> R<Value> {
        let mut items: Vec<u16> = self.db.prepare("select item from inv where user = ? and n > 0")?.query_map([uid], |r| r.get(0))?.collect::<Result<_, _>>()?;
        items.extend(self.gear(uid).iter().copied().filter(|&i| i != 0));
        items.retain(|&i| en::kind(i) != 0);
        items.sort_unstable();
        items.dedup();
        let list: Vec<Value> = items.iter().map(|&i| json!({ "item": i, "stufen": self.ench(uid, i) })).collect();
        let splitter: Vec<u32> = (0..5u16).map(|f| self.inv(uid, sb::SHARD + f)).collect();
        Ok(json!({ "gegenstaende": list, "kosten": { "buecher": self.inv(uid, rpg::VERZ_BUCH), "splitter": splitter, "kristalle": self.inv(uid, sb::FOKUS_KRISTALL) } }))
    }

    // ---------- Schilder ----------
    fn sign(&mut self, uid: u32, b: &Value) -> R<Value> {
        let (x, y, z) = (int(b, "x")? as i32, int(b, "y")? as i32, int(b, "z")? as i32);
        if self.block(x, y, z) != world::SCHILD {
            return no(404, "Hier steht kein Schild.");
        }
        let plot = match self.plot_here(x, z) {
            Some(p) if self.may_here(uid, x, z) => p.id,
            _ => return no(403, "Dieses Schild darfst du nicht beschriften."),
        };
        let zeilen: Vec<String> = b
            .get("zeilen")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(SIGN_LINES).map(|v| crate::sky::clean(v.as_str().unwrap_or("")).chars().take(SIGN_CHARS).collect::<String>()).collect())
            .unwrap_or_default();
        let data = serde_json::to_string(&zeilen).unwrap_or_else(|_| "[]".into());
        self.db.execute(
            "insert into island_functional_blocks (x, y, z, plot, kind, data) values (?, ?, ?, ?, ?, ?) on conflict (x, y, z) do update set plot = excluded.plot, kind = excluded.kind, data = excluded.data",
            params![x, y, z, plot, world::SCHILD, data],
        )?;
        self.island_refresh(plot);
        Ok(json!({ "ok": true, "zeilen": zeilen }))
    }

    // ---------- Co-op ----------
    pub fn coop_cmd(&mut self, id: u32, rest: &str) -> R<()> {
        let (word, name) = rest.split_once(' ').map_or((rest, ""), |(a, b)| (a, b.trim()));
        match word.to_lowercase().as_str() {
            "invite" | "einladen" | "add" => {
                self.coop(id, name, true)?;
                self.system(id, &format!("{name} ist jetzt Co-op-Partner deiner Insel."));
            }
            "remove" | "entfernen" | "kick" => {
                self.coop(id, name, false)?;
                self.system(id, &format!("{name} ist kein Co-op-Partner mehr."));
            }
            "" | "list" | "liste" => {
                let v = self.coop_info(id)?;
                let names: Vec<&str> = v["partner"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                let text = if names.is_empty() { "Noch keine Co-op-Partner. /coop invite <Name> lädt jemanden ein.".to_string() } else { format!("Co-op-Partner: {}", names.join(", ")) };
                self.system(id, &text);
            }
            _ => return no(400, "So: /coop invite <Name> · /coop remove <Name> · /coop"),
        }
        Ok(())
    }
    fn coop_list(&self, plot: u32) -> R<Vec<String>> {
        Ok(self
            .db
            .prepare("select u.name from coop_members c join users u on u.id = c.user where c.plot = ? order by c.since, u.name")?
            .query_map([plot], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
    fn coop_info(&self, uid: u32) -> R<Value> {
        let Some(plot) = self.plots.iter().find(|p| p.owner == uid).map(|p| p.id) else { return no(404, "Du hast noch keine Insel.") };
        Ok(json!({ "partner": self.coop_list(plot)?, "max": COOP_MAX }))
    }
    fn coop(&mut self, uid: u32, name: &str, add: bool) -> R<Value> {
        let Some((plot, owner_name)) = self.plots.iter().find(|p| p.owner == uid).map(|p| (p.id, p.name.clone())) else { return no(404, "Du hast noch keine Insel.") };
        let other: Option<u32> = self.db.query_row("select id from users where name = ? collate nocase", [name], |r| r.get(0)).optional()?;
        let Some(other) = other else { return no(404, "Diesen Namen gibt es im Campus nicht.") };
        if other == uid {
            return no(400, "Das ist deine eigene Insel.");
        }
        if add {
            let n: i64 = self.db.query_row("select count(*) from coop_members where plot = ? and user != ?", params![plot, other], |r| r.get(0))?;
            if n >= COOP_MAX {
                return no(429, "Höchstens 4 Co-op-Partner.");
            }
            self.db.execute("insert or ignore into coop_members (plot, user, since) values (?, ?, ?)", params![plot, other, self.now()])?;
        } else {
            self.db.execute("delete from coop_members where plot = ? and user = ?", params![plot, other])?;
        }
        self.load_plots()?;
        self.send_plots();
        if self.players.contains_key(&other) {
            let text = if add { format!("Du bist jetzt Co-op-Partner auf der Insel von {owner_name}.") } else { format!("Du bist kein Co-op-Partner mehr auf der Insel von {owner_name}.") };
            self.system(other, &text);
        }
        Ok(json!({ "ok": true, "partner": self.coop_list(plot)? }))
    }

    // ---------- Likes ----------
    pub fn like_here(&mut self, id: u32) -> R<()> {
        let p = self.players.get(&id).map(|p| p.p).ok_or(E(401, "Nicht verbunden.".into()))?;
        let plot = self.plot_here(p[0] as i32, p[2] as i32).map(|q| q.id).ok_or(E(404, "Du stehst auf keiner Insel.".into()))?;
        let v = self.like(id, plot, 0)?;
        let text = if v["neu"].as_bool().unwrap_or(false) { "Danke! Du hast diese Insel geliked." } else { "Diese Insel hast du schon geliked." };
        self.hud(id, json!({ "t": "toast", "art": "gut", "text": text }));
        Ok(())
    }
    fn like(&mut self, uid: u32, plot: u32, stamp: i64) -> R<Value> {
        let own = match self.plots.iter().find(|p| p.id == plot) {
            Some(p) => p.owner == uid || p.acl.contains(&uid),
            None => return no(404, "Diese Insel gibt es nicht."),
        };
        if own {
            return no(400, "Die eigene Insel kannst du nicht liken.");
        }
        if !(0..STAMPS as i64).contains(&stamp) {
            return no(400, "Diesen Stempel gibt es nicht.");
        }
        let neu = self.db.query_row("select 1 from island_likes where plot = ? and user = ?", params![plot, uid], |_| Ok(())).optional()?.is_none();
        self.db.execute(
            "insert into island_likes (plot, user, stamp, ts) values (?, ?, ?, ?) on conflict (plot, user) do update set stamp = excluded.stamp",
            params![plot, uid, stamp, self.now()],
        )?;
        if neu {
            self.quest_hit(uid, |g| (g == Goal::Like).then_some(1));
        }
        let likes: i64 = self.db.query_row("select count(*) from island_likes where plot = ?", [plot], |r| r.get(0))?;
        Ok(json!({ "ok": true, "neu": neu, "likes": likes }))
    }
    // ponytail: liest alle Likes und zählt im Speicher; bei sehr vielen Inseln/Likes per group by in SQL zählen
    fn top_islands(&self, uid: u32) -> R<Value> {
        let rows: Vec<(u32, u32, usize)> = self.db.prepare("select plot, user, stamp from island_likes")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let mut v: Vec<(usize, Value)> = self
            .plots
            .iter()
            .filter(|p| p.visit == 0)
            .map(|p| {
                let mut st = [0u32; STAMPS];
                let (mut n, mut mine) = (0usize, false);
                for r in rows.iter().filter(|r| r.0 == p.id) {
                    n += 1;
                    mine |= r.1 == uid;
                    if r.2 < STAMPS {
                        st[r.2] += 1;
                    }
                }
                (n, json!({ "id": p.id, "name": p.name, "likes": n, "stempel": st, "besucht": mine }))
            })
            .collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(Value::Array(v.into_iter().take(50).map(|x| x.1).collect()))
    }

    // ---------- Campus-Questbuch ----------
    fn book_row(&self, uid: u32) -> (usize, usize, u32) {
        self.db.query_row("select chapter, step, n from questbook where user = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap_or((1, 0, 0))
    }
    fn book_save(&self, uid: u32, ch: usize, step: usize, n: u32) {
        let _ = self.db.execute(
            "insert into questbook (user, chapter, step, n) values (?, ?, ?, ?) on conflict (user) do update set chapter = excluded.chapter, step = excluded.step, n = excluded.n",
            params![uid, ch as i64, step as i64, n],
        );
    }
    /// Ziele, die schon vor dem Schritt erfüllt sind, zählen sofort.
    fn book_already(&self, uid: u32, goal: Goal) -> bool {
        match goal {
            Goal::Talk(n) if n == sb::DEKAN => self.db.query_row("select weber from sky where user = ?", [uid], |r| r.get::<_, i64>(0)).unwrap_or(0) != 0,
            Goal::Craft(i) => self.owns(uid, i),
            Goal::Equip(k) => self.gear(uid)[..4].iter().filter(|&&g| g != 0).count() as u32 >= k,
            Goal::PlaceMinion => self.db.query_row("select 1 from minions where user = ?", [uid], |_| Ok(())).is_ok(),
            _ => false,
        }
    }
    /// Wie quest_hit, aber für das Campus-Questbuch. Ruft NIE quest_hit (quest_hit ruft uns).
    pub fn book_hit(&mut self, uid: u32, f: &dyn Fn(Goal) -> Option<u32>) {
        let (ch, step, n) = self.book_row(uid);
        let Some(c) = CHAPTERS.get(ch.wrapping_sub(1)) else { return };
        let Some(&(goal, _)) = c.steps.get(step) else { return };
        let Some(add) = f(goal) else { return };
        let n = n.saturating_add(add);
        let need = goal.need();
        if n < need {
            self.book_save(uid, ch, step, n);
            self.hud(uid, json!({ "t": "kapitel", "nr": ch, "schritt": step, "stand": n, "von": need }));
            return;
        }
        self.book_next(uid, ch, step);
    }
    /// Schritt `step` von Kapitel `ch` ist erledigt: weiter, Kapitelende belohnen, schon erfüllte Ziele überspringen.
    fn book_next(&mut self, uid: u32, mut ch: usize, mut step: usize) {
        loop {
            step += 1;
            let Some(c) = CHAPTERS.get(ch.wrapping_sub(1)) else { return };
            if step >= c.steps.len() {
                let _ = self.pay(uid, c.taler as i64, &format!("kapitel:{}", c.id));
                if c.shards > 0 {
                    let item = sb::SHARD + 4;
                    if let Ok(k) = self.gain(uid, item, c.shards) {
                        self.send(uid, &S2C::Inv { item, n: k });
                    }
                }
                self.hud(uid, json!({ "t": "kapitel", "nr": c.id, "name": c.name, "fertig": true, "taler": c.taler, "splitter": c.shards }));
                ch += 1;
                step = 0;
                match CHAPTERS.get(ch - 1) {
                    None => {
                        self.book_save(uid, ch, 0, 0);
                        return;
                    }
                    Some(nc) if nc.steps.first().is_some_and(|s| self.book_already(uid, s.0)) => continue,
                    Some(_) => {}
                }
            } else if self.book_already(uid, c.steps[step].0) {
                continue;
            }
            self.book_save(uid, ch, step, 0);
            self.hud(uid, json!({ "t": "kapitel", "nr": ch, "schritt": step }));
            return;
        }
    }
    pub fn book_state(&self, uid: u32) -> Value {
        let (ch, step, n) = self.book_row(uid);
        match CHAPTERS.get(ch.wrapping_sub(1)) {
            None => json!({ "kapitel": CHAPTERS.len(), "name": CHAPTERS.last().map_or("", |c| c.name), "schritt": 0, "text": "Alle Kapitel geschafft!", "stand": 0, "von": 0, "fertig": true }),
            Some(c) => {
                let s = c.steps.get(step);
                json!({ "kapitel": c.id, "name": c.name, "schritt": step, "text": s.map_or("", |s| s.1), "stand": n, "von": s.map_or(1, |s| s.0.need()), "fertig": false })
            }
        }
    }
    fn questbook(&self, uid: u32) -> Value {
        let (ch, step, n) = self.book_row(uid);
        let list: Vec<Value> = CHAPTERS
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let nr = i + 1;
                let status = if nr < ch { "erledigt" } else if nr == ch { "aktiv" } else { "offen" };
                let schritte: Vec<Value> = c
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(k, s)| {
                        let need = s.0.need();
                        let done = nr < ch || (nr == ch && k < step);
                        let stand: u32 = if done { need } else if nr == ch && k == step { n } else { 0 };
                        json!({ "text": s.1, "fertig": done, "stand": stand, "von": need })
                    })
                    .collect();
                json!({ "nr": c.id, "name": c.name, "text": c.text, "taler": c.taler, "splitter": c.shards, "status": status, "schritte": schritte })
            })
            .collect();
        json!({ "aktiv": ch, "kapitel": list, "stand": self.book_state(uid) })
    }

    // ---------- HTTP ----------
    pub fn island_get(&mut self, uid: u32, op: &str) -> Option<R<Value>> {
        Some(match op {
            "questbuch" => Ok(self.questbook(uid)),
            "enchants" => self.enchants(uid),
            "top_inseln" => self.top_islands(uid),
            "coop" => self.coop_info(uid),
            _ => return None,
        })
    }
    pub fn island_post(&mut self, uid: u32, op: &str, b: &Value) -> Option<R<Value>> {
        Some(match op {
            "enchant" => self.enchant_post(uid, b),
            "sign" => self.sign(uid, b),
            "coop" => {
                let name = b.get("name").and_then(Value::as_str).unwrap_or("").trim().chars().take(32).collect::<String>();
                let add = b.get("add").and_then(Value::as_bool).unwrap_or(true);
                self.coop(uid, &name, add)
            }
            "like" => match int(b, "plot") {
                Ok(p) => self.like(uid, p as u32, int(b, "stempel").unwrap_or(0)),
                Err(e) => Err(e),
            },
            _ => return None,
        })
    }
}
