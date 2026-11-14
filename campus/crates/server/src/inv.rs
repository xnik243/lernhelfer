//! Phase 16 + 20 (Besitz: Agent INV): Inventar-Plätze, Beutel (/stash), Truhen 27/54, Ausrüstung, Rüstungsständer-Inhalt,
//! Handel. Vertrag: CONTRACT-16-20.md §S-INV. Die Zählung `inv(user,item,n)` bleibt die Wahrheit, die Plätze sind ihre Anordnung.
use crate::game::{plot_may, Game};
use crate::{no, E, R};
use campus_shared::inventory::{self as iv, Stack, CHEST_LARGE, CHEST_SMALL, GEAR_SLOTS, HOTBAR, INV_SLOTS};
use campus_shared::items::item_name;
use campus_shared::proto::{self, S2C};
use campus_shared::skyblock::Goal;
use campus_shared::world;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Sperre nach jeder Änderung am Angebot, bevor abgeschlossen werden darf.
const LOCK_MS: i64 = 3000;
const OFFER_MAX: usize = 12;

#[derive(Clone, Default)]
struct Seite {
    uid: u32,
    items: Vec<(u16, u32)>,
    taler: i64,
    bereit: bool,
}
#[derive(Clone)]
struct Trade {
    id: u32,
    s: [Seite; 2],
    changed: i64,
}

/// Laufende Handelsfenster (im Speicher; Abbruch bei Trennung).
#[derive(Default)]
pub struct Trades {
    list: Vec<Trade>,
    /// offene Anfragen (von, an)
    asks: Vec<(u32, u32)>,
    next: u32,
}

/// Der einzige Weg, wie Gegenstände ihre Zählung ändern (LAW 1). Hält `inv` (Zählung) und `inv_layout` (Plätze) gleich;
/// was nicht in die 36 Plätze passt, landet im Beutel `player_stash` und wird über `Game::stash_notes` gemeldet.
pub struct InventoryTx;
impl InventoryTx {
    pub fn execute(g: &Game, user: u32, item: u16, delta: i64) -> R<u32> {
        if !iv::in_slots(item) {
            // Cosmetics: nur Zählung
            let have = count_of(g, user, item)? as i64;
            let n = have + delta;
            if n < 0 {
                return no(402, "Nicht genug im Vorrat.");
            }
            set_count(g, user, item, n as u64)?;
            return Ok(n as u32);
        }
        let mut s = layout(g, user)?;
        let have = count_of(g, user, item)?;
        let new = if delta < 0 {
            let k = delta.unsigned_abs();
            if have < k {
                return no(402, "Nicht genug im Vorrat.");
            }
            let prefer = g.players.get(&user).map(|p| p.held as usize);
            iv::remove(&mut s, item, k as u32, prefer).map_err(|e| E(402, e.to_string()))?;
            have - k
        } else if delta > 0 {
            let d = delta as u64;
            let k = d.min(iv::room(&s, item, 0..INV_SLOTS));
            iv::add(&mut s, item, k as u32, 0..INV_SLOTS);
            stash_add(g, user, item, d - k)?;
            have + k
        } else {
            return Ok(have as u32);
        };
        set_count(g, user, item, new)?;
        save_slots(g, user, &s)?;
        Ok(new as u32)
    }
}

// ---------- Hilfen ohne &mut (laufen auch in einer offenen Transaktion) ----------
fn count_of(g: &Game, uid: u32, item: u16) -> R<u64> {
    let n: Option<i64> = g.db.query_row("select n from inv where user = ? and item = ?", params![uid, item], |r| r.get(0)).optional()?;
    Ok(n.unwrap_or(0).max(0) as u64)
}
fn set_count(g: &Game, uid: u32, item: u16, n: u64) -> R<()> {
    if n == 0 {
        g.db.execute("delete from inv where user = ? and item = ?", params![uid, item])?;
    } else {
        g.db.execute("insert into inv (user, item, n) values (?, ?, ?) on conflict (user, item) do update set n = excluded.n", params![uid, item, n as i64])?;
    }
    Ok(())
}
fn stash_add(g: &Game, uid: u32, item: u16, n: u64) -> R<()> {
    if n == 0 {
        return Ok(());
    }
    g.db.execute("insert into player_stash (user, item, n) values (?, ?, ?) on conflict (user, item) do update set n = n + excluded.n", params![uid, item, n as i64])?;
    g.stash_notes.borrow_mut().push(uid);
    Ok(())
}
fn stash_sum(g: &Game, uid: u32) -> u64 {
    g.db.query_row("select coalesce(sum(n), 0) from player_stash where user = ?", [uid], |r| r.get::<_, i64>(0)).unwrap_or(0).max(0) as u64
}
fn slots_raw(g: &Game, uid: u32) -> Vec<Stack> {
    g.db.query_row("select data from inv_layout where user = ?", [uid], |r| r.get::<_, Vec<u8>>(0)).ok().and_then(|b| proto::dec(&b)).unwrap_or_default()
}
fn save_slots(g: &Game, uid: u32, s: &[Stack]) -> R<()> {
    g.db.execute("insert into inv_layout (user, data) values (?, ?) on conflict (user) do update set data = excluded.data", params![uid, proto::enc(&s.to_vec())])?;
    Ok(())
}
/// Plätze laden und an die Zählung angleichen; Überlauf senkt die Zählung und wandert in den Beutel.
fn layout(g: &Game, uid: u32) -> R<Vec<Stack>> {
    let mut s = slots_raw(g, uid);
    s.resize(INV_SLOTS, Stack::EMPTY);
    let rows: Vec<(u16, i64)> = g.db.prepare_cached("select item, n from inv where user = ? and n > 0")?.query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let c: BTreeMap<u16, u64> = rows.into_iter().filter(|r| iv::in_slots(r.0)).map(|(i, n)| (i, n as u64)).collect();
    if iv::totals(&s) != c {
        for (item, rest) in iv::reconcile(&mut s, &c) {
            set_count(g, uid, item, c.get(&item).copied().unwrap_or(0).saturating_sub(rest))?;
            stash_add(g, uid, item, rest)?;
        }
        save_slots(g, uid, &s)?;
    }
    Ok(s)
}
fn slots_json(s: &[Stack]) -> Vec<Value> {
    s.iter().map(|x| if x.is_empty() { Value::Null } else { json!([x.item, x.n]) }).collect()
}
fn one(item: u16) -> Stack {
    if item == 0 {
        Stack::EMPTY
    } else {
        Stack { item, n: 1 }
    }
}
fn ids(s: &[Stack]) -> Vec<u16> {
    s.iter().map(|x| if x.is_empty() { 0 } else { x.item }).collect()
}
fn xyz(v: &Value) -> R<(i32, i32, i32)> {
    let k = |n: &str| v.get(n).and_then(Value::as_i64).and_then(|a| i32::try_from(a).ok()).ok_or_else(|| E(400, format!("Feld „{n}“ fehlt.")));
    Ok((k("x")?, k("y")?, k("z")?))
}
fn e409(s: &'static str) -> E {
    E(409, s.to_string())
}
/// Grundstück an (x, z), wenn `uid` dort Recht hat (Besitz, Co-op, Gruppe): (Nummer, Besitzer).
fn plot_of(g: &Game, uid: u32, x: i32, z: i32, msg: &str) -> R<(u32, u32)> {
    let grp = match g.players.get(&uid) {
        Some(p) => p.group,
        None => g.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0)).optional()?.unwrap_or(0),
    };
    match world::slot_at(x, z).and_then(|s| g.plots.iter().find(|p| p.slot == s)) {
        Some(p) if plot_may(p, uid, grp) => Ok((p.id, p.owner)),
        _ => no(403, msg),
    }
}
/// Truhe laden (Zeile bei Bedarf anlegen; erste Truhe einer Insel übernimmt die alte Insel-Truhe `chest`).
fn chest_load(g: &Game, uid: u32, x: i32, y: i32, z: i32) -> R<(u32, Vec<Stack>)> {
    let size = match g.block(x, y, z) {
        world::TRUHE => CHEST_SMALL,
        world::GROSSE_TRUHE => CHEST_LARGE,
        _ => return no(404, "Hier steht keine Truhe."),
    };
    let (plot, owner) = plot_of(g, uid, x, z, "Diese Truhe gehört nicht dir.")?;
    let row: Option<Vec<u8>> = g.db.query_row("select data from island_chests where x = ? and y = ? and z = ?", params![x, y, z], |r| r.get(0)).optional()?;
    let mut s: Vec<Stack> = match row {
        Some(d) => proto::dec(&d).unwrap_or_default(),
        None => {
            let mut s = vec![Stack::EMPTY; size];
            let first: i64 = g.db.query_row("select count(*) from island_chests where plot = ?", [plot], |r| r.get(0))?;
            if first == 0 {
                let old: Vec<(u16, i64)> = g.db.prepare("select item, n from chest where plot = ?")?.query_map([plot], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
                let to = if owner != 0 { owner } else { uid };
                for (item, n) in old {
                    let n = n.clamp(0, u32::MAX as i64) as u32;
                    let rest = if iv::in_slots(item) { iv::add(&mut s, item, n, 0..size) } else { n };
                    stash_add(g, to, item, rest as u64)?;
                }
                g.db.execute("delete from chest where plot = ?", [plot])?;
            }
            g.db.execute("insert into island_chests (x, y, z, plot, size, data) values (?, ?, ?, ?, ?, ?)", params![x, y, z, plot, size as i64, proto::enc(&s)])?;
            s
        }
    };
    s.resize(size, Stack::EMPTY);
    Ok((plot, s))
}
/// Rüstungsständer laden (7 Plätze wie die Ausrüstung).
fn stand_load(g: &Game, uid: u32, x: i32, y: i32, z: i32) -> R<(u32, Vec<Stack>)> {
    if g.block(x, y, z) != world::STAENDER {
        return no(404, "Hier steht kein Rüstungsständer.");
    }
    let (plot, _) = plot_of(g, uid, x, z, "Dieser Rüstungsständer gehört nicht dir.")?;
    let d: Option<String> = g.db.query_row("select data from island_functional_blocks where x = ? and y = ? and z = ?", params![x, y, z], |r| r.get(0)).optional()?;
    let items: [u16; GEAR_SLOTS] = d.as_deref().and_then(|d| serde_json::from_str(d).ok()).unwrap_or([0; GEAR_SLOTS]);
    if d.is_none() {
        g.db.execute("insert or ignore into island_functional_blocks (x, y, z, plot, kind, data) values (?, ?, ?, ?, ?, ?)", params![x, y, z, plot, world::STAENDER, "[0,0,0,0,0,0,0]"])?;
    }
    Ok((plot, items.iter().map(|&i| one(i)).collect()))
}

/// Ein Behälter, an dem ein Zug beteiligt ist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ort {
    Inv,
    Gear,
    Truhe(i32, i32, i32),
    Staender(i32, i32, i32),
}
struct Beh {
    o: Ort,
    s: Vec<Stack>,
    alt: BTreeMap<u16, u64>,
}
fn open(g: &Game, uid: u32, o: Ort) -> R<Beh> {
    let s = match o {
        Ort::Inv => layout(g, uid)?,
        Ort::Gear => g.gear(uid).iter().map(|&i| one(i)).collect(),
        Ort::Truhe(x, y, z) => chest_load(g, uid, x, y, z)?.1,
        Ort::Staender(x, y, z) => stand_load(g, uid, x, y, z)?.1,
    };
    Ok(Beh { alt: iv::totals(&s), o, s })
}
fn close(g: &Game, uid: u32, b: &Beh) -> R<()> {
    match b.o {
        Ort::Inv => {
            // Zählung mitbuchen: vorher waren Plätze == Zählung, also gilt die neue Platz-Summe
            let neu = iv::totals(&b.s);
            for item in b.alt.keys().chain(neu.keys()) {
                if b.alt.get(item) != neu.get(item) {
                    set_count(g, uid, *item, neu.get(item).copied().unwrap_or(0))?;
                }
            }
            save_slots(g, uid, &b.s)
        }
        Ort::Gear => {
            g.db.execute("delete from player_equipment where user = ?", [uid])?;
            for (k, &i) in ids(&b.s).iter().enumerate().filter(|x| *x.1 != 0) {
                g.db.execute("insert into player_equipment (user, slot, item) values (?, ?, ?)", params![uid, k as i64, i])?;
            }
            Ok(())
        }
        Ort::Truhe(x, y, z) => {
            g.db.execute("update island_chests set data = ? where x = ? and y = ? and z = ?", params![proto::enc(&b.s), x, y, z])?;
            Ok(())
        }
        Ort::Staender(x, y, z) => {
            let d = serde_json::to_string(&ids(&b.s)).unwrap_or_default();
            g.db.execute("update island_functional_blocks set data = ? where x = ? and y = ? and z = ?", params![d, x, y, z])?;
            Ok(())
        }
    }
}
fn sum_all(bs: &[Beh]) -> BTreeMap<u16, u64> {
    let mut m = BTreeMap::new();
    for b in bs {
        for (k, v) in iv::totals(&b.s) {
            *m.entry(k).or_insert(0) += v;
        }
    }
    m
}
/// Darf `item` in Platz `slot` dieses Behälters?
fn fits(o: Ort, slot: usize, item: u16) -> bool {
    match o {
        Ort::Gear | Ort::Staender(..) => iv::gear_fits(slot, item),
        _ => iv::in_slots(item),
    }
}
fn ort(v: &Value) -> R<(Ort, usize)> {
    let i = v.get("i").and_then(Value::as_u64).unwrap_or(0).min(1000) as usize;
    let o = match v.get("art").and_then(Value::as_str) {
        Some("inv") => Ort::Inv,
        Some("gear") => Ort::Gear,
        Some("truhe") => {
            let (x, y, z) = xyz(v)?;
            Ort::Truhe(x, y, z)
        }
        Some("staender") => {
            let (x, y, z) = xyz(v)?;
            Ort::Staender(x, y, z)
        }
        _ => return no(400, "Ort fehlt oder ist ungültig."),
    };
    Ok((o, i))
}
fn idx(s: &[Stack], i: usize) -> R<()> {
    if i < s.len() {
        Ok(())
    } else {
        no(400, "Diesen Platz gibt es nicht.")
    }
}
fn two(s: &mut [Stack], a: usize, b: usize) -> (&mut Stack, &mut Stack) {
    if a < b {
        let (l, r) = s.split_at_mut(b);
        (&mut l[a], &mut r[0])
    } else {
        let (l, r) = s.split_at_mut(a);
        (&mut r[0], &mut l[b])
    }
}

impl Game {
    // ---------- Behälter-Züge ----------
    /// Behälter laden, `f` anwenden, Summen prüfen, alles in EINER Transaktion speichern; Antwort wie GET `inv`.
    fn with(&mut self, uid: u32, orte: &[Ort], f: impl FnOnce(&mut [Beh]) -> R<()>) -> R<Value> {
        if orte.len() == 2 && orte[0] == orte[1] {
            return no(400, "Ungültiger Zug.");
        }
        let res = self.tx(|g| {
            let mut bs = orte.iter().map(|&o| open(g, uid, o)).collect::<R<Vec<Beh>>>()?;
            let before = sum_all(&bs);
            f(&mut bs)?;
            if sum_all(&bs) != before {
                return no(500, "Dabei wäre etwas verloren gegangen – abgebrochen.");
            }
            for b in &bs {
                close(g, uid, b)?;
            }
            Ok(bs.into_iter().map(|b| (b.o, b.s)).collect::<Vec<_>>())
        })?;
        self.after_move(uid, &res)
    }
    fn after_move(&mut self, uid: u32, res: &[(Ort, Vec<Stack>)]) -> R<Value> {
        self.refresh_stats(uid);
        self.gear_all(uid);
        let mut v = self.inv_json(uid)?;
        for (o, s) in res {
            match *o {
                Ort::Gear => {
                    let worn = self.gear(uid)[..4].iter().filter(|&&i| i != 0).count() as u32;
                    if worn > 0 {
                        self.quest_hit(uid, move |g| match g {
                            Goal::Equip(k) if worn >= k => Some(k),
                            _ => None,
                        });
                    }
                }
                Ort::Truhe(x, y, z) => v["truhe"] = json!({ "x": x, "y": y, "z": z, "max": s.len(), "slots": slots_json(s) }),
                Ort::Staender(x, y, z) => {
                    v["staender"] = json!({ "x": x, "y": y, "z": z, "items": ids(s) });
                    if let Some(p) = world::slot_at(x, z).and_then(|sl| self.plots.iter().find(|p| p.slot == sl)).map(|p| p.id) {
                        self.island_refresh(p);
                    }
                }
                Ort::Inv => {}
            }
        }
        Ok(v)
    }
    fn inv_json(&mut self, uid: u32) -> R<Value> {
        let s = self.tx(|g| layout(g, uid))?;
        let stash: Vec<[i64; 2]> = self.db.prepare_cached("select item, n from player_stash where user = ? order by item")?.query_map([uid], |r| Ok([r.get(0)?, r.get(1)?]))?.collect::<Result<_, _>>()?;
        let n: i64 = stash.iter().map(|x| x[1]).sum();
        let held = self.players.get(&uid).map_or(0, |p| p.held);
        Ok(json!({ "slots": slots_json(&s), "gear": self.gear(uid), "held": held, "stash": stash, "stashN": n, "werte": self.stats_json(uid) }))
    }
    fn inv_move(&mut self, uid: u32, b: &Value) -> R<Value> {
        let (vo, vi) = ort(b.get("von").unwrap_or(&Value::Null))?;
        let nach = match b.get("nach") {
            Some(v) if !v.is_null() => Some(ort(v)?),
            _ => None,
        };
        let n = b.get("n").and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32;
        match nach {
            Some((no_, ni)) if no_ == vo => self.with(uid, &[vo], move |bs| {
                let s = &mut bs[0].s;
                idx(s, vi)?;
                idx(s, ni)?;
                if vi == ni {
                    return Ok(());
                }
                let (a, c) = two(s, vi, ni);
                iv::xfer(a, c, n, |it| fits(vo, ni, it), |it| fits(vo, vi, it)).map_err(e409)
            }),
            Some((no_, ni)) => self.with(uid, &[vo, no_], move |bs| {
                let (l, r) = bs.split_at_mut(1);
                let (a, c) = (&mut l[0].s, &mut r[0].s);
                idx(a, vi)?;
                idx(c, ni)?;
                iv::xfer(&mut a[vi], &mut c[ni], n, |it| fits(no_, ni, it), |it| fits(vo, vi, it)).map_err(e409)
            }),
            None => {
                // Schnell-Verschieben: Leiste↔Rucksack, inv→Ziel (offene Truhe/Ausrüstung/Ständer), sonst zurück ins Inventar
                let ziel = match b.get("ziel").filter(|v| !v.is_null()) {
                    Some(v) if v.get("art").is_some() => Some(ort(v)?.0),
                    Some(v) => {
                        let (x, y, z) = xyz(v)?;
                        Some(Ort::Truhe(x, y, z))
                    }
                    None => None,
                };
                let to = match (vo, ziel) {
                    (Ort::Inv, z) => z,
                    _ => Some(Ort::Inv),
                };
                match to {
                    None => self.with(uid, &[Ort::Inv], move |bs| {
                        idx(&bs[0].s, vi)?;
                        iv::quick_in(&mut bs[0].s, vi, if vi < HOTBAR { HOTBAR..INV_SLOTS } else { 0..HOTBAR });
                        Ok(())
                    }),
                    Some(t) => self.with(uid, &[vo, t], move |bs| {
                        let (l, r) = bs.split_at_mut(1);
                        let (a, c) = (&mut l[0].s, &mut r[0].s);
                        idx(a, vi)?;
                        if matches!(t, Ort::Gear | Ort::Staender(..)) {
                            let it = a[vi].item;
                            match (0..c.len()).find(|&j| c[j].is_empty() && iv::gear_fits(j, it)) {
                                Some(j) => iv::xfer(&mut a[vi], &mut c[j], 0, |_| true, |_| true).map_err(e409),
                                None => no(409, "Dafür ist kein passender Platz frei."),
                            }
                        } else {
                            iv::quick_between(&mut a[..], vi, &mut c[..]);
                            Ok(())
                        }
                    }),
                }
            }
        }
    }
    fn inv_sort(&mut self, uid: u32, b: &Value) -> R<Value> {
        match b.get("truhe").filter(|v| !v.is_null()) {
            Some(t) => {
                let (x, y, z) = xyz(t)?;
                self.with(uid, &[Ort::Truhe(x, y, z)], |bs| {
                    let l = bs[0].s.len();
                    iv::sort(&mut bs[0].s, 0..l);
                    Ok(())
                })
            }
            None => self.with(uid, &[Ort::Inv], |bs| {
                iv::sort(&mut bs[0].s, HOTBAR..INV_SLOTS);
                Ok(())
            }),
        }
    }
    /// Besitzer räumt eine Truhe komplett in seinen Beutel.
    fn chest_clear(&mut self, uid: u32, x: i32, y: i32, z: i32) -> R<Value> {
        let s = self.tx(|g| {
            let (plot, s) = chest_load(g, uid, x, y, z)?;
            if g.plots.iter().find(|p| p.id == plot).map_or(true, |p| p.owner != uid) {
                return no(403, "Nur der Besitzer darf die Truhe ausräumen.");
            }
            for st in s.iter().filter(|st| !st.is_empty()) {
                stash_add(g, uid, st.item, st.n as u64)?;
            }
            let empty = vec![Stack::EMPTY; s.len()];
            close(g, uid, &Beh { o: Ort::Truhe(x, y, z), s: empty.clone(), alt: BTreeMap::new() })?;
            Ok(empty)
        })?;
        self.flush_notes();
        self.after_move(uid, &[(Ort::Truhe(x, y, z), s)])
    }
    fn stash_claim(&mut self, uid: u32) -> R<Value> {
        let (got, rest) = self.tx(|g| {
            let rows: Vec<(u16, i64)> = g.db.prepare("select item, n from player_stash where user = ?")?.query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
            let before: i64 = rows.iter().map(|r| r.1).sum();
            g.db.execute("delete from player_stash where user = ?", [uid])?;
            for (item, n) in rows {
                g.give(uid, item, n)?; // passt es nicht, legt give den Rest zurück in den Beutel
            }
            let rest = stash_sum(g, uid) as i64;
            Ok((before - rest, rest))
        })?;
        self.stash_notes.borrow_mut().retain(|&u| u != uid);
        self.hud(uid, json!({ "t": "stash", "n": rest }));
        self.hud(uid, json!({ "t": "inv" }));
        let mut v = self.after_move(uid, &[])?;
        v["geholt"] = json!(got);
        Ok(v)
    }
    fn gear8(&self, uid: u32) -> [u16; 8] {
        let mut items = [0u16; 8];
        items[..GEAR_SLOTS].copy_from_slice(&self.gear(uid));
        items[7] = self.held_item(uid);
        items
    }
    /// Sichtbare Ausrüstung an alle melden.
    fn gear_all(&mut self, uid: u32) {
        if self.players.contains_key(&uid) {
            let items = self.gear8(uid);
            self.broadcast(&S2C::Gear { id: uid, items });
        }
    }

    pub fn inv_join(&mut self, id: u32) {
        if let Err(E(_, why)) = self.tx(|g| layout(g, id).map(|_| ())) {
            eprintln!("Inventar angleichen ({id}): {why}");
        }
        self.hud(id, json!({ "t": "inv" }));
        let n = stash_sum(self, id);
        if n > 0 {
            self.hud(id, json!({ "t": "stash", "n": n }));
        }
        let others: Vec<u32> = self.players.keys().copied().filter(|&o| o != id).collect();
        for o in others {
            let items = self.gear8(o);
            self.send(id, &S2C::Gear { id: o, items });
        }
        self.gear_all(id);
        self.refresh_stats(id);
    }
    /// Hud `stash` an alle, deren Beutel sich gefüllt hat (aus `stash_notes`); im Takt aufgerufen.
    pub fn flush_notes(&mut self) {
        let mut ids = std::mem::take(&mut *self.stash_notes.borrow_mut());
        if ids.is_empty() {
            return;
        }
        ids.sort_unstable();
        ids.dedup();
        for uid in ids {
            let n = stash_sum(self, uid);
            self.hud(uid, json!({ "t": "stash", "n": n }));
        }
    }
    pub fn gear(&self, uid: u32) -> [u16; 7] {
        let mut out = [0u16; GEAR_SLOTS];
        if let Ok(mut st) = self.db.prepare_cached("select slot, item from player_equipment where user = ?") {
            if let Ok(rows) = st.query_map([uid], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u16>(1)?))) {
                for (s, i) in rows.flatten() {
                    if (s as usize) < GEAR_SLOTS {
                        out[s as usize] = i;
                    }
                }
            }
        }
        out
    }
    pub fn held_item(&self, uid: u32) -> u16 {
        let Some(h) = self.players.get(&uid).map(|p| p.held as usize) else { return 0 };
        slots_raw(self, uid).get(h).filter(|s| !s.is_empty()).map_or(0, |s| s.item)
    }
    pub fn set_held(&mut self, id: u32, i: u8) {
        if (i as usize) >= HOTBAR {
            return;
        }
        let Some(p) = self.players.get_mut(&id) else { return };
        p.held = i;
        self.refresh_stats(id);
        self.gear_all(id);
    }
    pub fn stash_claim_cmd(&mut self, id: u32) -> R<()> {
        let v = self.stash_claim(id)?;
        let (got, rest) = (v["geholt"].as_i64().unwrap_or(0), v["stashN"].as_i64().unwrap_or(0));
        let text = if rest > 0 { format!("{got} Stück aus dem Beutel geholt, {rest} bleiben drin (Inventar voll).") } else { format!("{got} Stück aus dem Beutel geholt.") };
        self.system(id, &text);
        Ok(())
    }

    // ---------- Truhen und Ständer ----------
    pub fn chest_open_new(&mut self, id: u32, x: i32, y: i32, z: i32) -> R<()> {
        let (_, s) = self.tx(|g| chest_load(g, id, x, y, z))?;
        self.flush_notes();
        self.hud(id, json!({ "t": "truhe", "x": x, "y": y, "z": z, "max": s.len(), "slots": slots_json(&s) }));
        Ok(())
    }
    pub fn chest_can_break(&mut self, _id: u32, x: i32, y: i32, z: i32) -> R<()> {
        if !matches!(self.block(x, y, z), world::TRUHE | world::GROSSE_TRUHE) {
            return Ok(());
        }
        let d: Option<Vec<u8>> = self.db.query_row("select data from island_chests where x = ? and y = ? and z = ?", params![x, y, z], |r| r.get(0)).optional()?;
        if let Some(d) = d {
            let s: Vec<Stack> = proto::dec(&d).unwrap_or_default();
            if s.iter().any(|x| !x.is_empty()) {
                return no(409, "Die Truhe ist nicht leer. Räume sie erst aus.");
            }
            self.db.execute("delete from island_chests where x = ? and y = ? and z = ?", params![x, y, z])?;
        }
        Ok(())
    }
    /// Alte Truhen-Schnittstelle: n > 0 einlagern, n < 0 entnehmen (über dieselben Plätze).
    pub fn truhe_post(&mut self, uid: u32, b: &Value) -> R<Value> {
        let (x, y, z) = xyz(b)?;
        let item = b.get("item").and_then(Value::as_u64).and_then(|v| u16::try_from(v).ok()).unwrap_or(0);
        let n = b.get("n").and_then(Value::as_i64).unwrap_or(0).clamp(-1_000_000, 1_000_000);
        if n != 0 && !iv::in_slots(item) {
            return no(400, "Das kann nicht in eine Truhe.");
        }
        self.with(uid, &[Ort::Inv, Ort::Truhe(x, y, z)], move |bs| {
            if n == 0 {
                return Ok(());
            }
            let (l, r) = bs.split_at_mut(1);
            let (src, dst) = if n > 0 { (&mut l[0].s, &mut r[0].s) } else { (&mut r[0].s, &mut l[0].s) };
            let k = n.unsigned_abs() as u32;
            let len = dst.len();
            if iv::room(&dst[..], item, 0..len) < k as u64 {
                return no(409, if n > 0 { "Die Truhe ist voll." } else { "Dein Inventar ist voll." });
            }
            iv::remove(&mut src[..], item, k, None).map_err(|_| E(402, "So viel ist nicht da.".into()))?;
            iv::add(&mut dst[..], item, k, 0..len);
            Ok(())
        })
    }
    pub fn stand_open(&mut self, id: u32, x: i32, y: i32, z: i32) -> R<()> {
        let (_, s) = self.tx(|g| stand_load(g, id, x, y, z))?;
        self.hud(id, json!({ "t": "staender", "x": x, "y": y, "z": z, "items": ids(&s) }));
        Ok(())
    }

    // ---------- Handel ----------
    fn trade_of(&self, uid: u32) -> Option<usize> {
        self.trades.list.iter().position(|t| t.s.iter().any(|s| s.uid == uid))
    }
    fn trade_view(&self, t: &Trade, me: usize) -> Value {
        let side = |s: &Seite| json!({ "items": s.items.iter().map(|&(i, n)| json!([i, n])).collect::<Vec<_>>(), "taler": s.taler, "bereit": s.bereit });
        let other = &t.s[1 - me];
        let mit = self.players.get(&other.uid).map(|p| p.name.clone()).unwrap_or_default();
        json!({ "t": "handel", "id": t.id, "mit": mit, "ich": side(&t.s[me]), "du": side(other), "sperre_ms": (LOCK_MS - (self.now() - t.changed)).max(0) })
    }
    /// Stand an beide schicken; liefert die Sicht von `uid`.
    fn trade_show(&mut self, k: usize, uid: u32) -> Value {
        let t = self.trades.list[k].clone();
        let mut mine = Value::Null;
        for me in 0..2 {
            let v = self.trade_view(&t, me);
            if t.s[me].uid == uid {
                mine = v.clone();
            }
            self.hud(t.s[me].uid, v);
        }
        mine
    }
    pub fn trade_request(&mut self, id: u32, name: &str) -> R<()> {
        let name = name.trim();
        if name.is_empty() {
            return no(400, "So: /trade <Name>");
        }
        let me = self.players.get(&id).map(|p| p.name.clone()).ok_or_else(|| E(401, "Nicht verbunden.".into()))?;
        let to = self.players.values().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.id).ok_or_else(|| E(404, "Diese Person ist gerade nicht im Campus.".into()))?;
        if to == id {
            return no(400, "Mit dir selbst kannst du nicht handeln.");
        }
        if self.trade_of(id).is_some() || self.trade_of(to).is_some() {
            return no(409, "Einer von euch handelt gerade schon.");
        }
        self.trades.asks.retain(|a| a.0 != id);
        self.trades.asks.push((id, to));
        self.hud(to, json!({ "t": "handel_anfrage", "von": id, "name": me }));
        self.hud(id, json!({ "t": "toast", "art": "gut", "text": format!("Handelsanfrage an {name} geschickt.") }));
        Ok(())
    }
    fn trade_accept(&mut self, uid: u32, von: u32) -> R<Value> {
        let Some(k) = self.trades.asks.iter().position(|&a| a == (von, uid)) else { return no(404, "Keine Handelsanfrage von dieser Person.") };
        self.trades.asks.swap_remove(k);
        if !self.players.contains_key(&von) || !self.players.contains_key(&uid) {
            return no(404, "Diese Person ist gerade nicht im Campus.");
        }
        if self.trade_of(von).is_some() || self.trade_of(uid).is_some() {
            return no(409, "Einer von euch handelt gerade schon.");
        }
        self.trades.next += 1;
        let t = Trade { id: self.trades.next, s: [Seite { uid: von, ..Default::default() }, Seite { uid, ..Default::default() }], changed: self.now() };
        self.trades.list.push(t);
        let k = self.trades.list.len() - 1;
        Ok(self.trade_show(k, uid))
    }
    fn trade_offer(&mut self, uid: u32, b: &Value) -> R<Value> {
        let k = self.trade_of(uid).ok_or_else(|| E(404, "Du handelst gerade nicht.".into()))?;
        let list = b.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut items: Vec<(u16, u32)> = Vec::new();
        for e in &list {
            let item = e.get(0).and_then(Value::as_u64).and_then(|v| u16::try_from(v).ok()).filter(|&i| i != 0);
            let n = e.get(1).and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).filter(|&n| n > 0);
            let (Some(item), Some(n)) = (item, n) else { return no(400, "Angebot ungültig.") };
            match items.iter_mut().find(|x| x.0 == item) {
                Some(x) => x.1 = x.1.saturating_add(n),
                None => items.push((item, n)),
            }
        }
        if items.len() > OFFER_MAX {
            return no(400, "Höchstens 12 Gegenstände je Angebot.");
        }
        for &(item, n) in &items {
            if (self.inv(uid, item) as u64) < n as u64 {
                return no(402, &format!("So viel {} hast du nicht.", item_name(item)));
            }
        }
        let taler = b.get("taler").and_then(Value::as_i64).unwrap_or(0);
        if taler < 0 || taler > self.taler(uid) {
            return no(402, "So viele Taler hast du nicht.");
        }
        let now = self.now();
        let t = &mut self.trades.list[k];
        for s in t.s.iter_mut() {
            s.bereit = false;
            if s.uid == uid {
                s.items = items.clone();
                s.taler = taler;
            }
        }
        t.changed = now;
        Ok(self.trade_show(k, uid))
    }
    fn trade_ready(&mut self, uid: u32, an: bool) -> R<Value> {
        let k = self.trade_of(uid).ok_or_else(|| E(404, "Du handelst gerade nicht.".into()))?;
        for s in self.trades.list[k].s.iter_mut().filter(|s| s.uid == uid) {
            s.bereit = an;
        }
        Ok(self.trade_show(k, uid))
    }
    fn trade_confirm(&mut self, uid: u32) -> R<Value> {
        let k = self.trade_of(uid).ok_or_else(|| E(404, "Du handelst gerade nicht.".into()))?;
        let t = self.trades.list[k].clone();
        if !t.s.iter().all(|s| s.bereit) {
            return no(409, "Beide müssen erst bereit sein.");
        }
        let now = self.now();
        if now - t.changed < LOCK_MS {
            return no(409, "Noch kurz warten – das Angebot hat sich eben geändert.");
        }
        if t.s.iter().any(|s| !self.players.contains_key(&s.uid)) {
            self.trade_abort(uid, "Die andere Person ist nicht mehr im Campus.");
            return no(404, "Die andere Person ist nicht mehr im Campus.");
        }
        let detail = json!([{ "uid": t.s[0].uid, "items": t.s[0].items, "taler": t.s[0].taler }, { "uid": t.s[1].uid, "items": t.s[1].items, "taler": t.s[1].taler }]).to_string();
        self.tx(|g| {
            for me in 0..2 {
                let (s, o) = (&t.s[me], &t.s[1 - me]);
                for &(item, n) in &s.items {
                    g.give(s.uid, item, -(n as i64))?;
                    g.give(o.uid, item, n as i64)?;
                }
                if s.taler > 0 {
                    g.pay(s.uid, -s.taler, "handel")?;
                    g.pay(o.uid, s.taler, "handel")?;
                }
            }
            g.db.execute("insert into trade_log (a, b, detail, ts) values (?, ?, ?, ?)", params![t.s[0].uid, t.s[1].uid, detail, now])?;
            Ok(())
        })?;
        if let Some(k) = self.trade_of(uid) {
            self.trades.list.swap_remove(k);
        }
        self.flush_notes();
        for s in &t.s {
            self.hud(s.uid, json!({ "t": "handel", "fertig": true }));
            self.hud(s.uid, json!({ "t": "inv" }));
            self.refresh_stats(s.uid);
            self.gear_all(s.uid);
        }
        let mut v = self.inv_json(uid)?;
        v["fertig"] = json!(true);
        Ok(v)
    }
    pub fn trade_abort(&mut self, id: u32, why: &str) {
        self.trades.asks.retain(|a| a.0 != id && a.1 != id);
        let Some(k) = self.trade_of(id) else { return };
        let t = self.trades.list.swap_remove(k);
        for s in &t.s {
            self.hud(s.uid, json!({ "t": "handel", "abbruch": why }));
        }
    }

    // ---------- HTTP ----------
    pub fn inv_get(&mut self, uid: u32, op: &str) -> Option<R<Value>> {
        match op {
            "inv" => Some(self.inv_json(uid)),
            _ => None,
        }
    }
    pub fn inv_post(&mut self, uid: u32, op: &str, b: &Value) -> Option<R<Value>> {
        Some(match op {
            "inv/move" => self.inv_move(uid, b),
            "inv/sort" => self.inv_sort(uid, b),
            "truhe/einlagern" => xyz(b).and_then(|(x, y, z)| {
                self.with(uid, &[Ort::Inv, Ort::Truhe(x, y, z)], |bs| {
                    let (l, r) = bs.split_at_mut(1);
                    iv::quick_stack(&mut l[0].s, &mut r[0].s);
                    Ok(())
                })
            }),
            "truhe/leeren" => xyz(b).and_then(|(x, y, z)| self.chest_clear(uid, x, y, z)),
            "stash/claim" => self.stash_claim(uid),
            "trade/request" => {
                let name = b.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                self.trade_request(uid, &name).map(|_| json!({ "ok": true }))
            }
            "trade/accept" => match b.get("von").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()) {
                Some(von) => self.trade_accept(uid, von),
                None => no(400, "Feld „von“ fehlt."),
            },
            "trade/offer" => self.trade_offer(uid, b),
            "trade/ready" => self.trade_ready(uid, b.get("an").and_then(Value::as_bool).unwrap_or(true)),
            "trade/confirm" => self.trade_confirm(uid),
            "trade/cancel" => {
                self.trade_abort(uid, "Der Handel wurde abgebrochen.");
                Ok(json!({ "ok": true }))
            }
            _ => return None,
        })
    }
}
