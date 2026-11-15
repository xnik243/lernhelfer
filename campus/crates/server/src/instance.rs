//! Phasen 23/24 (Architekt, fertig – nicht ändern): EIN Instanz-Mechanismus für Prüfungs-Katakomben und Lern-Duell-Arena,
//! dazu die Weiche der Phasen 21–25 (HTTP, Verwaltungs-Schutz, 2×-Taler).
//! Eine Instanz ist ein 48×48-Feld (3×3 Stücke, volle Welthöhe) in der Leere des Hub-Archipels (campus_shared::instance).
//! Ihre Blöcke liegen NUR hier im Speicher: nie in `Game.chunks`, nie in der Datenbank. Mitglieder bekommen sie als
//! gewöhnliche S2C::Chunk an den echten Koordinaten (Überlagerung), alle anderen sehen dort Leere. Verlässt die letzte
//! Person die Instanz, verschwinden Gitter, Duell und Katakomben-Lauf sofort (LAW 2).
use crate::game::{ci, Game, User};
use crate::{no, E, R};
use axum::body::Bytes;
use campus_shared::instance as ins;
use campus_shared::proto::{self, S2C};
use campus_shared::world;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Höchstzahl Mitglieder einer Instanz.
pub const MAX_MEMBERS: usize = 4;

pub struct Inst {
    pub id: u32,
    /// instance::KIND_DUNGEON oder KIND_ARENA
    pub kind: u8,
    pub slot: u8,
    /// verbundene Mitglieder (Konto-Nummern), Reihenfolge = Beitritt
    pub members: Vec<u32>,
    /// 9 Stücke je world::VOL Blöcke, Index (dz/16)·3 + dx/16
    ch: Vec<Vec<u8>>,
    msg: [Option<Bytes>; 9],
}

impl Inst {
    fn new(id: u32, kind: u8, slot: u8) -> Inst {
        Inst { id, kind, slot, members: Vec::new(), ch: (0..9).map(|_| vec![0; world::VOL]).collect(), msg: Default::default() }
    }
    /// Nordwest-Ecke (x, z) in Weltkoordinaten; durch 16 teilbar.
    pub fn origin(&self) -> (i32, i32) {
        ins::origin(self.kind, self.slot)
    }
    fn at(&self, x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
        let (ox, oz) = self.origin();
        let (dx, dz) = (x - ox, z - oz);
        if !(0..ins::SIZE).contains(&dx) || !(0..ins::SIZE).contains(&dz) || !(0..world::CH).contains(&y) {
            return None;
        }
        Some((((dz >> 4) * 3 + (dx >> 4)) as usize, world::idx(dx & 15, y, dz & 15)))
    }
    /// Block an Weltkoordinaten (außerhalb des Feldes: Luft).
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.at(x, y, z).map_or(0, |(k, i)| self.ch[k][i])
    }
    /// Block still setzen (Aufbau; außerhalb des Feldes wirkungslos). Danach `Game::inst_sync`, damit Mitglieder es sehen.
    pub fn put(&mut self, x: i32, y: i32, z: i32, b: u8) {
        if let Some((k, i)) = self.at(x, y, z) {
            self.ch[k][i] = b;
            self.msg[k] = None;
        }
    }
    /// Quader füllen (beide Ecken einschließlich, Weltkoordinaten).
    pub fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, b: u8) {
        for y in y0.min(y1)..=y0.max(y1) {
            for z in z0.min(z1)..=z0.max(z1) {
                for x in x0.min(x1)..=x0.max(x1) {
                    self.put(x, y, z, b);
                }
            }
        }
    }
    /// Stück-Nummern (Index in Game.chunks) und Stück-Koordinaten der 9 Stücke.
    fn chunk_ids(&self) -> [(usize, i32, i32); 9] {
        let (ox, oz) = self.origin();
        std::array::from_fn(|k| {
            let (cx, cz) = ((ox >> 4) + k as i32 % 3, (oz >> 4) + k as i32 / 3);
            (ci(cx, cz), cx, cz)
        })
    }
    /// Fertige Nachricht für das Stück (cx, cz), wenn es zu diesem Feld gehört.
    fn chunk_msg(&mut self, cx: i32, cz: i32) -> Option<Bytes> {
        let (ox, oz) = self.origin();
        let (dx, dz) = (cx - (ox >> 4), cz - (oz >> 4));
        if !(0..3).contains(&dx) || !(0..3).contains(&dz) {
            return None;
        }
        let k = (dz * 3 + dx) as usize;
        if self.msg[k].is_none() {
            self.msg[k] = Some(Bytes::from(proto::enc(&S2C::Chunk { cx: cx as u8, cz: cz as u8, rle: world::rle(&self.ch[k]) })));
        }
        self.msg[k].clone()
    }
}

#[derive(Default)]
pub struct Instances {
    pub list: HashMap<u32, Inst>,
    next: u32,
}

fn admin_op(op: &str) -> bool {
    matches!(op, "admin/ki" | "admin/ki_test" | "admin/architekt" | "admin/event" | "admin/wetter" | "admin/npc") || op.starts_with("admin/blueprint/")
}

impl Game {
    // ---------- Instanzen ----------
    /// Neue leere Instanz der Art `kind` auf einem freien Feld. 409, wenn alle 6 Felder dieser Art belegt sind.
    pub fn inst_new(&mut self, kind: u8) -> R<u32> {
        let used: Vec<u8> = self.instances.list.values().filter(|i| i.kind == kind).map(|i| i.slot).collect();
        let Some(slot) = (0..ins::SLOTS).find(|s| !used.contains(s)) else { return no(409, "Gerade sind alle Felder belegt. Versuch es gleich noch einmal.") };
        self.instances.next += 1;
        let id = self.instances.next;
        self.instances.list.insert(id, Inst::new(id, kind, slot));
        Ok(id)
    }
    pub fn inst(&self, inst: u32) -> Option<&Inst> {
        self.instances.list.get(&inst)
    }
    pub fn inst_mut(&mut self, inst: u32) -> Option<&mut Inst> {
        self.instances.list.get_mut(&inst)
    }
    /// (Instanz-Nummer, Art) der Instanz, in der dieser Spieler steht.
    pub fn inst_of(&self, id: u32) -> Option<(u32, u8)> {
        let n = self.players.get(&id)?.inst?;
        Some((n, self.instances.list.get(&n)?.kind))
    }
    pub fn in_dungeon(&self, id: u32) -> bool {
        matches!(self.inst_of(id), Some((_, ins::KIND_DUNGEON)))
    }
    /// Stück-Nachricht der Instanz für diesen Spieler (Takt: Überlagerung beim Nachladen).
    pub(crate) fn inst_chunk(&mut self, id: u32, cx: i32, cz: i32) -> Option<Bytes> {
        let n = self.players.get(&id)?.inst?;
        self.instances.list.get_mut(&n)?.chunk_msg(cx, cz)
    }
    /// Alle 9 Stücke der Instanz sofort an diesen Spieler (vor dem Versetzen, damit er nicht durch Luft fällt).
    fn inst_push(&mut self, inst: u32, id: u32) {
        let Some(it) = self.instances.list.get_mut(&inst) else { return };
        let ids = it.chunk_ids();
        let msgs: Vec<Bytes> = ids.iter().filter_map(|&(_, cx, cz)| it.chunk_msg(cx, cz)).collect();
        if let Some(p) = self.players.get_mut(&id) {
            for &(i, _, _) in &ids {
                p.sent[i / 64] |= 1 << (i % 64);
            }
        }
        for m in msgs {
            self.push(id, m);
        }
    }
    /// Spieler in die Instanz holen: merkt die Stelle davor, schickt das Gitter, versetzt nach `at` (Füße) mit Blick `yaw`.
    pub fn inst_enter(&mut self, inst: u32, id: u32, at: [f32; 3], yaw: f32) -> R<()> {
        let Some(p) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        if p.inst.is_some() {
            return no(409, "Du bist schon in der Arena oder in den Katakomben. /leave bringt dich zurück.");
        }
        let back = p.p;
        let Some(it) = self.instances.list.get_mut(&inst) else { return no(404, "Diese Runde gibt es nicht mehr.") };
        if it.members.len() >= MAX_MEMBERS {
            return no(409, "Diese Runde ist voll (höchstens 4).");
        }
        it.members.push(id);
        self.trade_abort(id, "Die andere Person ist gerade nicht erreichbar.");
        if let Some(p) = self.players.get_mut(&id) {
            p.inst = Some(inst);
            p.inst_back = back;
        }
        self.inst_push(inst, id);
        self.inst_goto(id, at, yaw);
        Ok(())
    }
    /// Versetzen innerhalb der Instanz (ohne Suche nach sicherem Boden – die echte Welt ist dort leer).
    pub fn inst_goto(&mut self, id: u32, at: [f32; 3], yaw: f32) {
        let now = self.now();
        if let Some(p) = self.players.get_mut(&id) {
            p.p = at;
            p.yaw = yaw;
            p.last_move = now;
        }
        self.send(id, &S2C::Spawn { p: at, yaw });
    }
    /// Einen Block der Instanz ändern und sofort allen Mitgliedern melden (einzelne Änderungen: Türen, Prismen, Felder).
    pub fn inst_set(&mut self, inst: u32, x: i32, y: i32, z: i32, b: u8) {
        let Some(it) = self.instances.list.get_mut(&inst) else { return };
        if it.get(x, y, z) == b {
            return;
        }
        it.put(x, y, z, b);
        let ids = it.members.clone();
        let m = S2C::Block { x: x as i16, y: y as i16, z: z as i16, b, by: 0 };
        for id in ids {
            self.send(id, &m);
        }
    }
    /// Nach vielen `Inst::put`/`fill` (Aufbau, große Umbauten): alle 9 Stücke neu an alle Mitglieder.
    pub fn inst_sync(&mut self, inst: u32) {
        let ids = self.instances.list.get(&inst).map(|i| i.members.clone()).unwrap_or_default();
        for id in ids {
            self.inst_push(inst, id);
        }
    }
    pub fn inst_send(&mut self, inst: u32, m: &S2C) {
        let ids = self.instances.list.get(&inst).map(|i| i.members.clone()).unwrap_or_default();
        for id in ids {
            self.send(id, m);
        }
    }
    pub fn inst_hud(&mut self, inst: u32, v: Value) {
        self.inst_send(inst, &S2C::Hud(v.to_string()));
    }
    /// Spieler verlässt seine Instanz (Befehl, Warp, Trennung, Ende der Runde): Fach-Logik benachrichtigen, echte Stücke
    /// zurückschicken, an die Stelle davor setzen, Hud `{"t":"duell"|"katakomben","aus":true}`. War es die letzte Person,
    /// werden Gitter, Duell und Katakomben-Lauf sofort verworfen. Liefert true, wenn die Instanz aufgelöst wurde.
    pub fn inst_leave(&mut self, id: u32) -> bool {
        let Some((inst, kind)) = self.inst_of(id) else {
            if let Some(p) = self.players.get_mut(&id) {
                p.inst = None;
            }
            return false;
        };
        // Wertung/Aufgabe, solange die Instanz noch steht und der Spieler noch Mitglied ist
        if kind == ins::KIND_DUNGEON {
            self.dungeon_left(inst, id);
        } else {
            self.duel_left(inst, id);
        }
        let ids = match self.instances.list.get_mut(&inst) {
            Some(it) => {
                it.members.retain(|&m| m != id);
                it.chunk_ids()
            }
            None => return false,
        };
        let back = match self.players.get_mut(&id) {
            Some(p) => {
                p.inst = None;
                p.boost_until = 0;
                p.inst_back
            }
            None => world::SPOTS[0].1,
        };
        for (i, _, _) in ids {
            let m = self.chunk_msg(i);
            self.push(id, m);
        }
        self.spawn_at(id, back, 0.0);
        self.hud(id, json!({ "t": if kind == ins::KIND_DUNGEON { "katakomben" } else { "duell" }, "aus": true }));
        let empty = self.instances.list.get(&inst).is_none_or(|i| i.members.is_empty());
        if empty {
            self.inst_remove(inst);
        }
        empty
    }
    /// Runde beenden: alle Mitglieder zurückschicken und alles verwerfen.
    pub fn inst_close(&mut self, inst: u32) {
        let ids = self.instances.list.get(&inst).map(|i| i.members.clone()).unwrap_or_default();
        for id in ids {
            self.inst_leave(id);
        }
        self.inst_remove(inst);
    }
    fn inst_remove(&mut self, inst: u32) {
        self.instances.list.remove(&inst);
        self.active_dungeons.remove(&inst);
        self.duels.remove(&inst);
    }
    /// Angenommene Bewegung eines Instanz-Mitglieds (statt on_move / Leere-Sturz).
    pub(crate) fn inst_move(&mut self, id: u32, now: i64) {
        match self.inst_of(id) {
            Some((_, ins::KIND_DUNGEON)) => self.dungeon_move(id, now),
            Some(_) => self.duel_move(id, now),
            None => {}
        }
    }
    /// [E] auf einen Block der Instanz (Reichweite ist schon geprüft).
    pub(crate) fn inst_use(&mut self, id: u32, x: i32, y: i32, z: i32) -> R<()> {
        let Some((inst, kind)) = self.inst_of(id) else { return no(404, "Hier gibt es nichts zu benutzen.") };
        let b = self.instances.list.get(&inst).map_or(0, |i| i.get(x, y, z));
        if kind == ins::KIND_DUNGEON {
            self.dungeon_use(id, x, y, z, b)
        } else {
            no(404, "Hier gibt es nichts zu benutzen.")
        }
    }
    /// Leben eines Instanz-Mitglieds auf 0 (mobs::hurt_player hat Leben und Schild schon zurückgesetzt).
    pub(crate) fn inst_died(&mut self, id: u32, now: i64) {
        match self.inst_of(id) {
            Some((_, ins::KIND_DUNGEON)) => self.dungeon_died(id, now),
            Some(_) => self.duel_died(id, now),
            None => {}
        }
    }

    // ---------- Verwaltung, Event ----------
    /// LAW 5: nur wer bei der Anmeldung vom Lernhelfer als Verwaltung gemeldet wurde.
    pub fn admin_only(&self, id: u32) -> R<()> {
        if self.players.get(&id).is_some_and(|p| p.admin) {
            Ok(())
        } else {
            no(403, "Nur für die Verwaltung.")
        }
    }
    /// Faktor für verdiente Taler (Lernen, Quests, Kampf, Duell, Katakomben): 2 während des Events, sonst 1.
    pub fn taler_mult(&self) -> i64 {
        if self.now() < self.event_until {
            2
        } else {
            1
        }
    }
    /// Einstellungen der Verwaltung (Tabelle server_config; hier liegt auch der OpenRouter-Schlüssel – nie ausliefern).
    pub fn cfg(&self, k: &str) -> Option<String> {
        use rusqlite::OptionalExtension;
        self.db.query_row("select v from server_config where k = ?", [k], |r| r.get(0)).optional().ok().flatten()
    }
    pub fn cfg_set(&self, k: &str, v: &str) -> R<()> {
        self.db.execute("insert into server_config (k, v) values (?, ?) on conflict (k) do update set v = excluded.v", [k, v])?;
        Ok(())
    }

    // ---------- HTTP-Weiche der Phasen 21–25 ----------
    pub fn p21_get(&mut self, u: &User, op: &str) -> Option<R<Value>> {
        if admin_op(op) && !u.admin {
            return Some(no(403, "Nur für die Verwaltung."));
        }
        Some(match op {
            "duell/rangliste" => self.duel_ranking(u.id),
            "katakomben" => self.dungeon_get(u.id),
            "admin/ki" => self.ki_get(),
            "admin/architekt" => self.arch_get(),
            o if o.starts_with("admin/blueprint/") => self.arch_blueprint(&o["admin/blueprint/".len()..]),
            _ => return None,
        })
    }
    pub fn p21_post(&mut self, u: &User, op: &str, b: &Value) -> Option<R<Value>> {
        if admin_op(op) && !u.admin {
            return Some(no(403, "Nur für die Verwaltung."));
        }
        let id = u.id;
        Some(match op {
            "instanz/verlassen" => {
                self.inst_leave(id);
                Ok(json!({ "ok": true }))
            }
            "duell/wahl" => self.duel_vote(id, b),
            "duell/antwort" => self.duel_answer(id, b),
            "katakomben/start" => {
                let stufe = b.get("stufe").and_then(Value::as_u64).unwrap_or(1).clamp(1, 5) as u8;
                self.dungeon_start(id, stufe).map(|n| json!({ "ok": true, "lauf": n }))
            }
            "katakomben/beitreten" => match b.get("name").and_then(Value::as_str) {
                Some(name) => self.dungeon_join(id, name).map(|_| json!({ "ok": true })),
                None => Err(E(400, "Feld „name“ fehlt.".into())),
            },
            "admin/ki" => self.ki_post(b),
            "admin/event" => self.event_post(b),
            "admin/wetter" => self.weather_post(b),
            "admin/architekt" => self.arch_post(id, b),
            "admin/npc" => self.npc_post(id, b),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::testkit::*;
    use campus_shared::instance as ins;
    use campus_shared::proto::S2C;
    use campus_shared::world;
    use serde_json::json;

    #[test]
    fn instanz_ueberlagert_und_verschwindet() {
        let mut g = game();
        let mut rx = join(&mut g, 1, "Anna", false);
        let _rx2 = join(&mut g, 2, "Ben", false);
        let before = g.players[&1].p;
        let n = g.inst_new(ins::KIND_DUNGEON).unwrap();
        let (ox, oz) = g.inst(n).unwrap().origin();
        g.inst_mut(n).unwrap().fill(ox, 8, oz, ox + 47, 8, oz + 47, world::FELS);
        msgs(&mut rx);
        g.inst_enter(n, 1, [ox as f32 + 8.5, 9.0, oz as f32 + 8.5], 0.0).unwrap();
        let m = msgs(&mut rx);
        assert_eq!(m.iter().filter(|m| matches!(m, S2C::Chunk { .. })).count(), 9, "alle 9 Stücke vor dem Versetzen");
        assert!(matches!(m.iter().find(|m| matches!(m, S2C::Spawn { .. } | S2C::Chunk { .. })), Some(S2C::Chunk { .. })), "Stücke kommen vor der Ankunft");
        assert_eq!(g.inst_of(1), Some((n, ins::KIND_DUNGEON)));
        // Gitter nur im Speicher: die Welt bleibt dort Luft, save() schreibt nichts davon
        assert_eq!((g.inst(n).unwrap().get(ox + 3, 8, oz + 3), g.block(ox + 3, 8, oz + 3)), (world::FELS, 0));
        g.save().unwrap();
        let rows: i64 = g.db.query_row("select count(*) from chunks where cx >= ?1 and cx < ?1 + 3 and cz >= ?2 and cz < ?2 + 3", [ox >> 4, oz >> 4], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
        g.inst_set(n, ox + 1, 9, oz + 1, world::GLAS);
        assert!(msgs(&mut rx).iter().any(|m| matches!(m, S2C::Block { b, by: 0, .. } if *b == world::GLAS)));
        // zweite Person, dann beide raus: alles weg
        assert!(g.inst_enter(n, 1, [0.0; 3], 0.0).is_err(), "nicht doppelt");
        g.inst_enter(n, 2, [ox as f32 + 9.5, 9.0, oz as f32 + 8.5], 0.0).unwrap();
        assert!(!g.inst_leave(1));
        assert!(huds(&mut rx).iter().any(|h| h["t"] == "katakomben" && h["aus"] == json!(true)));
        assert_eq!(g.players[&1].inst, None);
        assert!((g.players[&1].p[0] - before[0]).abs() < 3.0, "zurück an die Stelle davor");
        g.drop_player(2); // Trennung zählt als Verlassen
        assert!(g.instances.list.is_empty() && g.active_dungeons.is_empty() && g.duels.is_empty());
        // alle Felder belegbar, danach 409
        let all: Vec<u32> = (0..ins::SLOTS).map(|_| g.inst_new(ins::KIND_ARENA).unwrap()).collect();
        assert!(g.inst_new(ins::KIND_ARENA).is_err());
        for n in all {
            g.inst_close(n);
        }
        assert!(g.instances.list.is_empty());
    }

    #[test]
    fn neu_verbinden_verlaesst_instanz() {
        let mut g = game();
        let _alt = join(&mut g, 1, "Anna", false);
        let n = g.inst_new(ins::KIND_DUNGEON).unwrap();
        let (ox, oz) = g.inst(n).unwrap().origin();
        g.inst_enter(n, 1, [ox as f32 + 8.5, 9.0, oz as f32 + 8.5], 0.0).unwrap();
        let alt_session = g.players[&1].session;
        // zweiter Tab / Netzabbruch: neue Sitzung, bevor die alte als getrennt gilt
        let _neu = join(&mut g, 1, "Anna", false);
        assert!(g.instances.list.is_empty(), "kein Geist-Mitglied, Instanz aufgelöst");
        assert_eq!(g.players[&1].inst, None);
        g.leave(1, alt_session); // spätes Schließen der alten Verbindung ändert nichts
        assert!(g.players.contains_key(&1));
    }

    #[test]
    fn verwaltung_wird_geprueft() {
        let mut g = game();
        let _a = join(&mut g, 1, "Chef", true);
        let _b = join(&mut g, 2, "Gast", false);
        assert!(g.admin_only(1).is_ok());
        assert_eq!(g.admin_only(2).unwrap_err().0, 403);
        let gast = crate::game::User { id: 2, name: "Gast".into(), admin: false };
        for op in ["admin/ki", "admin/architekt", "admin/blueprint/turm"] {
            assert_eq!(g.get(&gast, op).unwrap_err().0, 403, "{op}");
        }
        for op in ["admin/ki", "admin/event", "admin/wetter", "admin/architekt", "admin/npc"] {
            assert_eq!(g.post(&gast, op, &json!({})).unwrap_err().0, 403, "{op}");
        }
        assert_eq!(g.taler_mult(), 1);
        g.event_until = g.now() + 60_000;
        assert_eq!(g.taler_mult(), 2);
        g.cfg_set("probe", "1").unwrap();
        assert_eq!(g.cfg("probe").as_deref(), Some("1"));
    }
}
