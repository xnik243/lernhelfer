//! Wirtschaft und Regeln mit Datenbank: Zahltag, Laden, Funde, Auktionshaus, Grundstücke, Lerngruppen,
//! Großprojekte, Wochenziel, Melden, Admin, Datenexport und Löschen. Reihenfolge überall: erst Datenbank, dann Speicher.
use crate::game::{Game, Plot, User};
use crate::{no, E, R};
use campus_shared::economy::{self as eco, DayUse, Earn, Kind};
use campus_shared::items::{self, Look, BLUEPRINT};
use campus_shared::proto;
use campus_shared::time::{parse_day, DAY_MS};
use campus_shared::world::{self, SlotKind, WorldState};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
pub struct LhEvent {
    pub id: i64,
    pub art: String,
    pub menge: u32,
    #[serde(default)]
    pub fach: String,
    pub tag: String,
    #[serde(default)]
    pub serie: u32,
    #[serde(default)]
    pub pruefung_in: Option<i32>,
    #[serde(default)]
    pub note: Option<u8>,
}

/// Antwort von `GET /api/campus/verdienst` des Lernhelfers.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Verdienst {
    #[serde(default)]
    pub xp: u64,
    #[serde(default)]
    pub serie: u32,
    #[serde(default)]
    pub bester: u32,
    #[serde(default)]
    pub checks: HashMap<String, u32>,
    #[serde(default)]
    pub ereignisse: Vec<LhEvent>,
}

pub const REPORT_REASONS: [&str; 5] = ["Beleidigender Bau", "Zerstörung", "Belästigung", "Schummeln", "Sonstiges"];

pub fn int(b: &Value, k: &str) -> R<i64> {
    b.get(k).and_then(Value::as_i64).filter(|v| (0..=1_000_000_000).contains(v)).ok_or_else(|| E(400, format!("Feld „{k}“ fehlt oder ist ungültig.")))
}
fn item(b: &Value, k: &str) -> R<u16> {
    u16::try_from(int(b, k)?).map_err(|_| E(400, format!("Feld „{k}“ ist ungültig.")))
}
fn text<'a>(b: &'a Value, k: &str) -> R<&'a str> {
    b.get(k).and_then(Value::as_str).filter(|s| s.len() <= 100).ok_or_else(|| E(400, format!("Feld „{k}“ fehlt.")))
}
fn name_ok(s: &str) -> bool {
    let n = s.chars().count();
    (2..=24).contains(&n) && s.trim() == s && s.chars().all(|c| c.is_alphanumeric() || " ._-".contains(c))
}

struct Auction {
    id: i64,
    seller: u32,
    item: u16,
    qty: u32,
    start: i64,
    buyout: Option<i64>,
    bid: Option<i64>,
    bidder: Option<u32>,
    ends: i64,
}

impl Game {
    /// Führt `f` in einer Transaktion aus; bei Fehler wird alles zurückgenommen.
    pub fn tx<T>(&mut self, f: impl FnOnce(&mut Game) -> R<T>) -> R<T> {
        self.db.execute_batch("begin immediate")?;
        match f(self) {
            Ok(v) => {
                self.db.execute_batch("commit")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.db.execute_batch("rollback");
                Err(e)
            }
        }
    }

    // ---------- Konto ----------
    pub fn ensure_user(&mut self, u: &User) -> R<()> {
        let row: Option<(i64, String)> = self.db.query_row("select banned, name from users where id = ?", [u.id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        match row {
            Some((1, _)) => no(403, "Dieses Konto ist für Campus gesperrt."),
            Some((_, name)) => {
                if name != u.name {
                    self.db.execute("update users set name = ? where id = ?", params![u.name, u.id])?;
                    self.load_plots()?;
                }
                Ok(())
            }
            None => {
                let now = self.now();
                self.tx(|g| {
                    g.db.execute("insert into users (id, name, created) values (?, ?, ?)", params![u.id, u.name, now])?;
                    for (b, n) in eco::STARTER_KIT {
                        g.give(u.id, b as u16, n as i64)?;
                    }
                    Ok(())
                })
            }
        }
    }
    pub fn taler(&self, uid: u32) -> i64 {
        self.db.query_row("select taler from users where id = ?", [uid], |r| r.get(0)).unwrap_or(0)
    }
    /// Taler buchen (auch negativ). Schlägt fehl, wenn das Konto unter 0 fiele; jede Bewegung steht im Kassenbuch.
    pub fn pay(&self, uid: u32, delta: i64, reason: &str) -> R<()> {
        if delta == 0 {
            return Ok(());
        }
        if self.db.execute("update users set taler = taler + ?1 where id = ?2 and taler + ?1 >= 0", params![delta, uid])? != 1 {
            return no(402, "Dafür reichen deine Taler nicht.");
        }
        self.db.execute("insert into ledger (user, delta, reason, ts) values (?, ?, ?, ?)", params![uid, delta, reason, self.now()])?;
        Ok(())
    }
    fn user_name(&self, uid: u32) -> String {
        self.db.query_row("select name from users where id = ?", [uid], |r| r.get(0)).unwrap_or_else(|_| "Gelöschtes Konto".into())
    }
    fn look_of(&self, uid: u32) -> Look {
        self.db.query_row("select look from users where id = ?", [uid], |r| r.get::<_, Option<Vec<u8>>>(0)).ok().flatten().and_then(|b| proto::dec(&b)).unwrap_or_default()
    }
    fn save_look(&mut self, uid: u32, look: Look) -> R<()> {
        self.db.execute("update users set look = ? where id = ?", params![proto::enc(&look), uid])?;
        if let Some(p) = self.players.get_mut(&uid) {
            p.look = look;
            self.announce(uid);
        }
        Ok(())
    }
    /// Legt ab, was nicht mehr im Besitz ist (verkauft, Krone abgelaufen).
    pub fn fix_look(&mut self, uid: u32) -> R<()> {
        let mut look = self.look_of(uid);
        let before = look;
        for s in &mut look.slots {
            if *s != 0 && !self.owns(uid, *s) {
                *s = 0;
            }
        }
        if look != before {
            self.save_look(uid, look)?;
        }
        Ok(())
    }

    pub fn me(&self, u: &User) -> R<Value> {
        let id = u.id;
        let (taler, funde, level, streak, grp): (i64, i64, u32, u32, u32) = self.db.query_row("select taler, funde, level, streak, grp from users where id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        let inv: Vec<(u16, u32)> = self.db.prepare("select item, n from inv where user = ? order by item")?.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let timed: Vec<(u16, i64)> = self.db.prepare("select item, until from timed where user = ? and until > ?")?.query_map(params![id, self.now()], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let plot = self.plots.iter().find(|p| p.owner == id).map(|p| {
            let acl: Vec<String> = p.acl.iter().map(|&a| self.user_name(a)).collect();
            json!({ "id": p.id, "slot": [p.slot.0, p.slot.1], "size": p.size, "rect": world::plot_rect(p.slot, p.size), "bauer": acl,
                "ausbau": eco::plot_upgrade(p.size).map(|(s, t, b, l)| json!({ "size": s, "taler": t, "bauplaene": b, "stufe": l })) })
        });
        let group = if grp != 0 { self.db.query_row("select id, name, treasury, owner from groups where id = ?", [grp], |r| Ok(json!({ "id": r.get::<_, u32>(0)?, "name": r.get::<_, String>(1)?, "kasse": r.get::<_, i64>(2)?, "leitung": r.get::<_, u32>(3)? == id }))).optional()? } else { None };
        let look = self.look_of(id);
        Ok(json!({
            "id": id, "name": u.name, "admin": u.admin, "taler": taler, "funde": funde, "stufe": level, "serie": streak,
            "inv": inv, "zeit": timed, "look": look, "plot": plot, "gruppe": group,
            "woche": self.week(), "wochenbloecke": eco::weekly_blocks(self.week()), "online": self.players.len(),
            "welt": { "gebiete": self.st.regions, "projekte": self.st.projects },
        }))
    }

    // ---------- Zahltag ----------
    /// Schreibt gut, was der Lernhelfer seit dem letzten Abruf gezählt hat. `commit = false` rechnet nur vor (Zahltag-Hinweis).
    pub fn claim(&mut self, uid: u32, v: &Verdienst, commit: bool) -> R<Value> {
        self.db.execute_batch("begin immediate")?;
        self.quiet = !commit;
        let r = self.claim_inner(uid, v);
        self.quiet = false;
        if commit && r.is_ok() {
            self.db.execute_batch("commit")?;
            self.goal_check()?;
        } else {
            let _ = self.db.execute_batch("rollback");
        }
        r
    }
    fn claim_inner(&mut self, uid: u32, v: &Verdienst) -> R<Value> {
        let last: i64 = self.db.query_row("select last_ev from users where id = ?", [uid], |r| r.get(0))?;
        let (mut taler, mut funde, mut plaene, mut max) = (0i64, 0u32, 0u32, last);
        let mut neu: Vec<u16> = Vec::new();
        let mut gewinn: Vec<Value> = Vec::new();
        let mut evs: Vec<&LhEvent> = v.ereignisse.iter().filter(|e| e.id > last).collect();
        evs.sort_by_key(|e| e.id);
        for e in evs {
            max = max.max(e.id);
            let (Some(kind), Some(_)) = (Kind::from_key(&e.art), parse_day(&e.tag)) else { continue };
            let (focus, cards): (u32, u32) = self.db.query_row("select focus, cards from daily where user = ? and day = ?", params![uid, e.tag], |r| Ok((r.get(0)?, r.get(1)?))).optional()?.unwrap_or((0, 0));
            let mut used = DayUse { focus, cards };
            let p = eco::payout(&Earn { kind, amount: e.menge, streak: e.serie, exam_in: e.pruefung_in, grade: e.note }, &mut used);
            self.db.execute("insert into daily (user, day, focus, cards) values (?, ?, ?, ?) on conflict (user, day) do update set focus = excluded.focus, cards = excluded.cards", params![uid, e.tag, used.focus, used.cards])?;
            let lohn = p.taler as i64 * self.taler_mult(); // 2×-Taler-Event der Verwaltung
            self.pay(uid, lohn, &format!("lernen:{}", e.art))?;
            gewinn.push(self.learn(uid, e, kind)?);
            if p.blueprints > 0 {
                self.give(uid, BLUEPRINT, p.blueprints as i64)?;
            }
            if p.gold_frame && self.inv(uid, items::PLATE_GOLD) == 0 {
                self.give(uid, items::PLATE_GOLD, 1)?;
                neu.push(items::PLATE_GOLD);
            }
            taler += lohn;
            funde += p.funde;
            plaene += p.blueprints;
        }
        let level = eco::level(v.xp);
        let mut earned = eco::milestones(level, v.bester.max(v.serie));
        earned.extend(v.checks.iter().filter(|(_, n)| **n >= eco::SUBJECT_CAPE_CHECKS).map(|(f, _)| eco::subject_cape(f)));
        for c in earned {
            if self.inv(uid, c) == 0 {
                self.give(uid, c, 1)?;
                neu.push(c);
            }
        }
        self.db.execute("update users set funde = funde + ?, last_ev = ?, level = ?, streak = ?, best_streak = max(best_streak, ?) where id = ?", params![funde, max, level, v.serie, v.bester.max(v.serie), uid])?;
        if taler > 0 {
            self.db.execute("insert into earn_week (user, week, taler) values (?, ?, ?) on conflict (user, week) do update set taler = taler + excluded.taler", params![uid, self.week(), taler])?;
        }
        let names: Vec<&str> = neu.iter().map(|&c| items::item_name(c)).collect();
        Ok(json!({ "taler": taler, "funde": funde, "bauplaene": plaene, "neu": names, "stand": self.taler(uid), "seit": max, "sky": gewinn }))
    }

    /// Wochenziel der Welt: lernen alle zusammen genug, öffnet sich (höchstens einmal je Woche) ein neues Gebiet.
    pub fn goal_check(&mut self) -> R<()> {
        let week = self.week();
        let total: i64 = self.db.query_row("select coalesce(sum(taler), 0) from earn_week where week = ?", [week], |r| r.get(0))?;
        if total < eco::WORLD_GOAL_WEEKLY as i64 || self.st.regions as usize >= eco::REGIONS.len() || self.kv("goal_week").as_deref() == Some(&week.to_string()) {
            return Ok(());
        }
        let name = eco::REGIONS[self.st.regions as usize];
        self.kv_set("goal_week", &week.to_string())?;
        self.apply_state(WorldState { regions: self.st.regions + 1, ..self.st })?;
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            self.assign_plot(id)?;
        }
        self.hud_all(json!({ "t": "gebiet", "name": name }));
        Ok(())
    }
    pub fn world_info(&self) -> R<Value> {
        let week = self.week();
        let total: i64 = self.db.query_row("select coalesce(sum(taler), 0) from earn_week where week = ?", [week], |r| r.get(0))?;
        Ok(json!({ "woche": week, "gelernt": total, "ziel": eco::WORLD_GOAL_WEEKLY, "gebiete": self.st.regions, "namen": eco::REGIONS,
            "erreicht": self.kv("goal_week").as_deref() == Some(&week.to_string()) }))
    }

    /// Wochenwechsel: die drei Besten bekommen die Krone für 7 Tage. Abgelaufene Kronen werden abgelegt.
    pub fn rollover(&mut self) -> R<()> {
        let (cur, now) = (self.week(), self.now());
        let stored: Option<u32> = self.kv("week").and_then(|w| w.parse().ok());
        if stored != Some(cur) {
            if let Some(w) = stored {
                let top: Vec<u32> = self.db.prepare("select user from earn_week where week = ? and taler > 0 order by taler desc, user limit 3")?.query_map([w], |r| r.get(0))?.collect::<Result<_, _>>()?;
                for (i, uid) in top.into_iter().enumerate() {
                    self.db.execute("insert into timed (user, item, until) values (?, ?, ?) on conflict (user, item) do update set until = excluded.until", params![uid, items::HEAD_KRONE, now + eco::CROWN_DAYS * DAY_MS])?;
                    self.hud(uid, json!({ "t": "krone", "platz": i + 1 }));
                }
            }
            self.kv_set("week", &cur.to_string())?;
        }
        let gone: Vec<u32> = self.db.prepare("select user from timed where until <= ?")?.query_map([now], |r| r.get(0))?.collect::<Result<_, _>>()?;
        if !gone.is_empty() {
            self.db.execute("delete from timed where until <= ?", [now])?;
            for uid in gone {
                self.fix_look(uid)?;
            }
        }
        Ok(())
    }
    pub fn ranking(&self) -> R<Value> {
        let week = self.week();
        let users: Vec<Value> = self.db.prepare("select u.name, e.taler from earn_week e join users u on u.id = e.user where e.week = ? and e.taler > 0 order by e.taler desc, e.user limit 10")?.query_map([week], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "taler": r.get::<_, i64>(1)? })))?.collect::<Result<_, _>>()?;
        Ok(json!({ "woche": week, "spieler": users }))
    }

    // ---------- Laden und Funde ----------
    pub fn buy(&mut self, uid: u32, item: u16, n: i64) -> R<Value> {
        let Some(price) = eco::shop_price(item, self.week()) else { return no(404, "Das gibt es gerade nicht im Laden.") };
        if !(1..=640).contains(&n) || (item >= 256 && n != 1) {
            return no(400, "Ungültige Menge.");
        }
        if item >= 256 && self.inv(uid, item) > 0 {
            return no(409, "Das hast du schon.");
        }
        self.tx(|g| {
            g.pay(uid, -(price as i64 * n), "laden")?;
            g.give(uid, item, n)
        })?;
        Ok(json!({ "item": item, "n": self.inv(uid, item), "taler": self.taler(uid) }))
    }
    pub fn open_fund(&mut self, uid: u32) -> R<Value> {
        let day = self.today();
        self.tx(|g| {
            if g.db.execute("update users set funde = funde - 1 where id = ? and funde > 0", [uid])? != 1 {
                return no(402, "Kein Fund zum Öffnen. Funde gibt es nur fürs Lernen.");
            }
            let l = eco::open_fund(&mut g.rng, day);
            let n = g.give(uid, l.item, l.qty as i64)?;
            Ok(json!({ "item": l.item, "name": items::item_name(l.item), "menge": l.qty, "selten": l.rare, "n": n }))
        })
    }

    // ---------- Aussehen ----------
    pub fn set_look(&mut self, uid: u32, b: &Value) -> R<Value> {
        let f = |k: &str| int(b, k).map(|v| v.min(255) as u8);
        let mut look = Look { skin: f("skin")?, hair: f("hair")?, hair_col: f("hair_col")?, outfit: f("outfit")?, col_a: f("col_a")?, col_b: f("col_b")?, slots: [0; 8] }.clamped();
        look.slots = self.look_of(uid).slots;
        self.save_look(uid, look)?;
        Ok(json!({ "look": look }))
    }
    pub fn equip(&mut self, uid: u32, slot: u8, item: u16) -> R<Value> {
        if slot as usize >= 8 {
            return no(400, "Diesen Platz gibt es nicht.");
        }
        if item != 0 {
            let Some(c) = items::cosmetic(item) else { return no(404, "Dieses Stück gibt es nicht.") };
            if c.slot != slot {
                return no(400, "Das Stück gehört an einen anderen Platz.");
            }
            if item != items::EMOTE_WINKEN && !self.owns(uid, item) {
                return no(403, "Das Stück gehört dir nicht.");
            }
        }
        let mut look = self.look_of(uid);
        look.slots[slot as usize] = item;
        self.save_look(uid, look)?;
        Ok(json!({ "look": look }))
    }

    // ---------- Auktionshaus ----------
    fn auction(&self, id: i64) -> R<Auction> {
        self.db
            .query_row("select id, seller, item, qty, start, buyout, bid, bidder, ends from auctions where id = ? and state = 0", [id], |r| {
                Ok(Auction { id: r.get(0)?, seller: r.get(1)?, item: r.get(2)?, qty: r.get(3)?, start: r.get(4)?, buyout: r.get(5)?, bid: r.get(6)?, bidder: r.get(7)?, ends: r.get(8)? })
            })
            .optional()?
            .ok_or_else(|| E(404, "Dieses Angebot ist nicht mehr offen.".into()))
    }
    pub fn auction_create(&mut self, uid: u32, item: u16, qty: i64, start: i64, buyout: Option<i64>, hours: i64) -> R<Value> {
        if !items::tradeable(item) {
            return no(400, "Nur seltene Blöcke, Baupläne und gefundene Cosmetics lassen sich handeln.");
        }
        let max = eco::AUCTION_MAX_PRICE as i64;
        if !(1..=999).contains(&qty) || !(1..=max).contains(&start) || buyout.is_some_and(|b| b < start || b > max) || !eco::AUCTION_HOURS.contains(&(hours as u32)) {
            return no(400, "Menge, Preis oder Laufzeit sind ungültig (24 oder 72 Stunden).");
        }
        let open: i64 = self.db.query_row("select count(*) from auctions where seller = ? and state = 0", [uid], |r| r.get(0))?;
        if open >= 10 {
            return no(429, "Höchstens 10 offene Angebote.");
        }
        let ends = self.now() + hours * 3_600_000;
        let id = self.tx(|g| {
            g.give(uid, item, -qty)?; // der Gegenstand liegt bis zum Ende im Auktionshaus
            g.db.execute("insert into auctions (seller, item, qty, start, buyout, ends) values (?, ?, ?, ?, ?, ?)", params![uid, item, qty, start, buyout, ends])?;
            Ok(g.db.last_insert_rowid())
        })?;
        self.fix_look(uid)?;
        Ok(json!({ "id": id, "ends": ends }))
    }
    pub fn auction_bid(&mut self, uid: u32, id: i64, amount: i64) -> R<Value> {
        let a = self.auction(id)?;
        if a.ends <= self.now() {
            return no(409, "Die Auktion ist abgelaufen.");
        }
        if a.seller == uid {
            return no(403, "Auf eigene Angebote kannst du nicht bieten.");
        }
        let min = eco::min_bid(a.start as u64, a.bid.map(|b| b as u64)) as i64;
        if amount < min {
            return no(400, &format!("Das Mindestgebot liegt bei {min} Talern."));
        }
        if a.buyout.is_some_and(|b| amount >= b) {
            return self.auction_buyout(uid, id);
        }
        self.tx(|g| {
            if let (Some(pb), Some(pid)) = (a.bid, a.bidder) {
                g.pay(pid, pb, "ueberboten")?; // altes Gebot geht sofort zurück
            }
            g.pay(uid, -amount, "gebot")?; // neues Gebot wird sofort reserviert
            g.db.execute("update auctions set bid = ?, bidder = ? where id = ?", params![amount, uid, id])?;
            Ok(())
        })?;
        if let Some(pid) = a.bidder.filter(|&p| p != uid) {
            self.hud(pid, json!({ "t": "ueberboten", "id": id, "item": items::item_name(a.item), "taler": a.bid }));
        }
        Ok(json!({ "id": id, "gebot": amount, "taler": self.taler(uid) }))
    }
    pub fn auction_buyout(&mut self, uid: u32, id: i64) -> R<Value> {
        let a = self.auction(id)?;
        let Some(price) = a.buyout else { return no(400, "Dieses Angebot hat keinen Sofortkauf.") };
        if a.ends <= self.now() {
            return no(409, "Die Auktion ist abgelaufen.");
        }
        if a.seller == uid {
            return no(403, "Eigene Angebote kannst du nicht kaufen.");
        }
        self.tx(|g| {
            if let (Some(pb), Some(pid)) = (a.bid, a.bidder) {
                g.pay(pid, pb, "ueberboten")?;
            }
            g.pay(uid, -price, "sofortkauf")?;
            g.sold(&a, uid, price)
        })?;
        if let Some(pid) = a.bidder.filter(|&p| p != uid) {
            self.hud(pid, json!({ "t": "ueberboten", "id": id, "item": items::item_name(a.item), "taler": a.bid }));
        }
        self.hud(a.seller, json!({ "t": "verkauft", "item": items::item_name(a.item), "taler": price - eco::auction_fee(price as u64) as i64 }));
        Ok(json!({ "id": id, "gekauft": true, "taler": self.taler(uid) }))
    }
    /// Abschluss: Käufer hat schon bezahlt. Verkäufer bekommt den Preis abzüglich 5 %, die Gebühr verschwindet aus dem Spiel.
    fn sold(&mut self, a: &Auction, winner: u32, price: i64) -> R<()> {
        let (fee, now) = (eco::auction_fee(price as u64) as i64, self.now());
        if self.db.query_row("select 1 from users where id = ?", [a.seller], |_| Ok(())).optional()?.is_some() {
            self.pay(a.seller, price - fee, "verkauf")?;
        }
        self.db.execute("insert into ledger (user, delta, reason, ts) values (0, ?, 'gebuehr', ?)", params![fee, now])?;
        self.give(winner, a.item, a.qty as i64)?;
        self.db.execute("update auctions set state = 1, bid = ?, bidder = ? where id = ?", params![price, winner, a.id])?;
        self.db.execute("insert into transfers (from_u, to_u, amount, ts) values (?, ?, ?, ?)", params![winner, a.seller, price, now])?;
        // Auffälligkeiten: überhöhter Preis oder einseitiger Geldfluss zwischen zwei Konten
        if eco::overpriced(a.item, a.qty, price as u64) {
            self.flag("preis", winner, a.seller, &format!("{} × {} für {} Taler (Richtwert {})", a.qty, items::item_name(a.item), price, items::item_value(a.item) * a.qty as u64))?;
        }
        let since = now - eco::FLAG_PAIR_DAYS * DAY_MS;
        let flow = |from: u32, to: u32| -> R<i64> { Ok(self.db.query_row("select coalesce(sum(amount), 0) from transfers where from_u = ? and to_u = ? and ts >= ?", params![from, to, since], |r| r.get(0))?) };
        let (hin, her) = (flow(winner, a.seller)?, flow(a.seller, winner)?);
        if hin >= eco::FLAG_PAIR_SUM as i64 && her * 4 < hin {
            self.flag("einseitig", winner, a.seller, &format!("{hin} Taler in {} Tagen in eine Richtung, {her} zurück", eco::FLAG_PAIR_DAYS))?;
        }
        Ok(())
    }
    fn flag(&self, kind: &str, a: u32, b: u32, detail: &str) -> R<()> {
        if self.db.query_row("select 1 from flags where kind = ? and a = ? and b = ? and done = 0", params![kind, a, b], |_| Ok(())).optional()?.is_none() {
            self.db.execute("insert into flags (kind, a, b, detail, ts) values (?, ?, ?, ?, ?)", params![kind, a, b, detail, self.now()])?;
        }
        Ok(())
    }
    /// Abgelaufene Auktionen abschließen: Zuschlag ans Höchstgebot oder Rückgabe an den Verkäufer.
    pub fn auction_tick(&mut self) -> R<usize> {
        let due: Vec<i64> = self.db.prepare("select id from auctions where state = 0 and ends <= ?")?.query_map([self.now()], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for &id in &due {
            let a = self.auction(id)?;
            self.tx(|g| match (a.bid, a.bidder) {
                (Some(price), Some(w)) => g.sold(&a, w, price),
                _ => {
                    if g.db.query_row("select 1 from users where id = ?", [a.seller], |_| Ok(())).optional()?.is_some() {
                        g.give(a.seller, a.item, a.qty as i64)?;
                    }
                    g.db.execute("update auctions set state = 2 where id = ?", [id])?;
                    Ok(())
                }
            })?;
            match (a.bid, a.bidder) {
                (Some(price), Some(w)) => {
                    self.hud(w, json!({ "t": "zuschlag", "item": items::item_name(a.item), "taler": price }));
                    self.hud(a.seller, json!({ "t": "verkauft", "item": items::item_name(a.item), "taler": price - eco::auction_fee(price as u64) as i64 }));
                }
                _ => self.hud(a.seller, json!({ "t": "abgelaufen", "item": items::item_name(a.item) })),
            }
        }
        Ok(due.len())
    }
    pub fn auctions(&self, uid: u32) -> R<Value> {
        let now = self.now();
        let list: Vec<Value> = self
            .db
            .prepare("select a.id, a.seller, a.item, a.qty, a.start, a.buyout, a.bid, a.bidder, a.ends, coalesce(u.name, '?') from auctions a left join users u on u.id = a.seller where a.state = 0 and a.ends > ? order by a.ends")?
            .query_map([now], |r| {
                let (item, bid, bidder, start): (u16, Option<i64>, Option<u32>, i64) = (r.get(2)?, r.get(6)?, r.get(7)?, r.get(4)?);
                Ok(json!({ "id": r.get::<_, i64>(0)?, "meins": r.get::<_, u32>(1)? == uid, "item": item, "name": items::item_name(item), "menge": r.get::<_, u32>(3)?,
                    "start": start, "sofort": r.get::<_, Option<i64>>(5)?, "gebot": bid, "fuehre": bidder == Some(uid), "mindest": eco::min_bid(start as u64, bid.map(|b| b as u64)),
                    "ende": r.get::<_, i64>(8)?, "von": r.get::<_, String>(9)? }))
            })?
            .collect::<Result<_, _>>()?;
        Ok(json!({ "jetzt": now, "gebuehr": eco::AUCTION_FEE_PCT, "liste": list }))
    }

    // ---------- Grundstücke ----------
    pub fn assign_plot(&mut self, uid: u32) -> R<()> {
        if self.plots.iter().any(|p| p.owner == uid) {
            return Ok(());
        }
        let Some(slot) = world::slots_of(SlotKind::Personal, self.st).into_iter().find(|s| !self.plots.iter().any(|p| p.slot == *s)) else { return Ok(()) };
        self.db.execute("insert into plots (si, sj, size, owner, grp) values (?, ?, ?, ?, 0)", params![slot.0, slot.1, eco::PLOT_SIZES[0], uid])?;
        self.plots.push(Plot { id: self.db.last_insert_rowid() as u32, slot, size: eco::PLOT_SIZES[0], owner: uid, group: 0, acl: Vec::new(), name: self.user_name(uid), visit: 0, owner_grp: 0 });
        self.send_plots();
        Ok(())
    }
    /// Grundstück ausbauen: kostet Taler (und für die letzte Stufe einen Bauplan), verlangt eine Mindeststufe.
    pub fn plot_expand(&mut self, uid: u32, group: bool) -> R<Value> {
        let (grp, level): (u32, u32) = self.db.query_row("select grp, level from users where id = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let Some(p) = self.plots.iter().find(|p| if group { grp != 0 && p.group == grp } else { p.owner == uid }).cloned() else { return no(404, "Du hast noch kein Grundstück.") };
        let Some((size, cost, plaene, need)) = eco::plot_upgrade(p.size) else { return no(409, "Das Grundstück ist schon voll ausgebaut.") };
        if group {
            if self.db.query_row("select owner from groups where id = ?", [grp], |r| r.get::<_, u32>(0))? != uid {
                return no(403, "Nur die Leitung der Lerngruppe baut das Gruppen-Grundstück aus.");
            }
        } else if level < need {
            return no(403, &format!("Dafür brauchst du Stufe {need}."));
        }
        self.tx(|g| {
            if group {
                if g.db.execute("update groups set treasury = treasury - ?1 where id = ?2 and treasury >= ?1", params![cost as i64, grp])? != 1 {
                    return no(402, "In der Gruppenkasse ist zu wenig.");
                }
                g.db.execute("insert into ledger (user, delta, reason, ts) values (0, ?, 'gruppen-grundstueck', ?)", params![cost as i64, g.now()])?;
            } else {
                g.pay(uid, -(cost as i64), "grundstueck")?;
            }
            g.give(uid, BLUEPRINT, -(plaene as i64)).map_err(|_| E(402, "Dafür brauchst du einen Bauplan (gibt es für bestandene Probeklausuren).".into()))?;
            g.db.execute("update plots set size = ? where id = ?", params![size, p.id])?;
            Ok(())
        })?;
        self.load_plots()?;
        self.send_plots();
        Ok(json!({ "size": size, "taler": self.taler(uid) }))
    }
    pub fn plot_acl(&mut self, uid: u32, name: &str, add: bool) -> R<Value> {
        let Some(p) = self.plots.iter().find(|p| p.owner == uid).cloned() else { return no(404, "Du hast noch kein Grundstück.") };
        let Some(other): Option<u32> = self.db.query_row("select id from users where name = ? collate nocase", [name], |r| r.get(0)).optional()? else { return no(404, "Diesen Namen gibt es im Campus nicht.") };
        if other == uid {
            return no(400, "Du darfst hier schon bauen.");
        }
        if add {
            if p.acl.len() >= 8 {
                return no(429, "Höchstens 8 Mitbauer.");
            }
            self.db.execute("insert or ignore into plot_acl (plot, user) values (?, ?)", params![p.id, other])?;
        } else {
            self.db.execute("delete from plot_acl where plot = ? and user = ?", params![p.id, other])?;
        }
        self.load_plots()?;
        self.send_plots();
        Ok(json!({ "ok": true }))
    }

    /// Insel-Kompass: alle Inseln, die `uid` besuchen darf, eigene zuerst.
    pub fn islands(&self, uid: u32) -> R<Value> {
        let mut v: Vec<Value> = self
            .plots
            .iter()
            .filter(|p| self.may_visit(p, uid))
            .map(|p| json!({ "id": p.id, "name": p.name, "eigen": p.owner == uid, "gruppe": p.group != 0, "besuch": p.visit }))
            .collect();
        v.sort_by_key(|x| !x["eigen"].as_bool().unwrap_or(false));
        let spots: Vec<&str> = world::SPOTS.iter().map(|s| s.0).collect();
        let mine = self.plots.iter().find(|p| p.owner == uid).map_or(0, |p| p.visit);
        Ok(json!({ "inseln": v, "orte": spots, "modus": mine, "modi": proto::VISIT_NAMES }))
    }
    pub fn plot_visit(&mut self, uid: u32, modus: i64) -> R<Value> {
        if !(0..=2).contains(&modus) {
            return no(400, "Unbekannter Modus.");
        }
        let Some(p) = self.plots.iter().find(|p| p.owner == uid).map(|p| p.id) else { return no(404, "Du hast noch keine Insel.") };
        self.db.execute("update plots set visit = ? where id = ?", params![modus, p])?;
        self.load_plots()?;
        Ok(json!({ "ok": true }))
    }

    // ---------- Lerngruppen ----------
    fn set_group(&mut self, uid: u32, grp: u32) -> R<()> {
        self.db.execute("update users set grp = ? where id = ?", params![grp, uid])?;
        if let Some(p) = self.players.get_mut(&uid) {
            p.group = grp;
        }
        for p in self.plots.iter_mut().filter(|p| p.owner == uid) {
            p.owner_grp = grp;
        }
        Ok(())
    }
    pub fn group_create(&mut self, uid: u32, name: &str) -> R<Value> {
        if !name_ok(name) {
            return no(400, "Der Name braucht 2 bis 24 Zeichen (Buchstaben, Ziffern, Leerzeichen, . _ -).");
        }
        if self.db.query_row("select grp from users where id = ?", [uid], |r| r.get::<_, u32>(0))? != 0 {
            return no(409, "Du bist schon in einer Lerngruppe.");
        }
        let Some(slot) = world::slots_of(SlotKind::Group, self.st).into_iter().find(|s| !self.plots.iter().any(|p| p.slot == *s)) else { return no(409, "Das Viertel der Lerngruppen ist voll.") };
        // ponytail: vier Plätze im Gruppen-Viertel; mehr Gruppen brauchen ein zweites Viertel in einem neuen Gebiet.
        let gid = self.tx(|g| {
            g.pay(uid, -(eco::GROUP_COST as i64), "gruppe")?;
            g.db.execute("insert into groups (name, owner) values (?, ?)", params![name, uid]).map_err(|_| E(409, "Den Namen gibt es schon.".into()))?;
            let gid = g.db.last_insert_rowid() as u32;
            g.db.execute("insert into plots (si, sj, size, owner, grp) values (?, ?, ?, 0, ?)", params![slot.0, slot.1, eco::PLOT_SIZES[0], gid])?;
            g.db.execute("update users set grp = ? where id = ?", params![gid, uid])?;
            Ok(gid)
        })?;
        self.set_group(uid, gid)?;
        self.load_plots()?;
        self.send_plots();
        Ok(json!({ "id": gid, "taler": self.taler(uid) }))
    }
    pub fn group_join(&mut self, uid: u32, gid: u32) -> R<Value> {
        if self.db.query_row("select grp from users where id = ?", [uid], |r| r.get::<_, u32>(0))? != 0 {
            return no(409, "Du bist schon in einer Lerngruppe.");
        }
        if self.db.query_row("select 1 from groups where id = ?", [gid], |_| Ok(())).optional()?.is_none() {
            return no(404, "Diese Lerngruppe gibt es nicht.");
        }
        let n: i64 = self.db.query_row("select count(*) from users where grp = ?", [gid], |r| r.get(0))?;
        if n >= eco::GROUP_MAX as i64 {
            return no(409, "Die Lerngruppe ist voll.");
        }
        self.set_group(uid, gid)?;
        self.send_plots();
        Ok(json!({ "id": gid }))
    }
    pub fn group_leave(&mut self, uid: u32) -> R<Value> {
        let gid: u32 = self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0))?;
        if gid == 0 {
            return no(409, "Du bist in keiner Lerngruppe.");
        }
        let rest: Option<u32> = self.db.query_row("select id from users where grp = ? and id != ? order by id limit 1", params![gid, uid], |r| r.get(0)).optional()?;
        let reset = self.tx(|g| {
            g.db.execute("update users set grp = 0 where id = ?", [uid])?;
            match rest {
                Some(next) => {
                    g.db.execute("update groups set owner = ? where id = ? and owner = ?", params![next, gid, uid])?;
                    Ok(None)
                }
                None => {
                    // letzte Person: Gruppe löst sich auf, die Kasse geht an sie zurück (kein Taler verschwindet unbemerkt)
                    let kasse: i64 = g.db.query_row("select treasury from groups where id = ?", [gid], |r| r.get(0))?;
                    g.pay(uid, kasse, "gruppe-aufgeloest")?;
                    let slot: Option<(u8, u8, u8)> = g.db.query_row("select si, sj, size from plots where grp = ?", [gid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
                    g.db.execute("delete from plots where grp = ?", [gid])?;
                    g.db.execute("delete from groups where id = ?", [gid])?;
                    Ok(slot)
                }
            }
        })?;
        self.set_group(uid, 0)?;
        if let Some((i, j, size)) = reset {
            self.reset_rect(world::plot_rect((i, j), size));
        }
        self.load_plots()?;
        self.send_plots();
        Ok(json!({ "taler": self.taler(uid) }))
    }
    pub fn group_deposit(&mut self, uid: u32, amount: i64) -> R<Value> {
        let gid: u32 = self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0))?;
        if gid == 0 || amount < 1 {
            return no(400, "Du bist in keiner Lerngruppe.");
        }
        self.tx(|g| {
            g.pay(uid, -amount, "gruppenkasse")?;
            g.db.execute("update groups set treasury = treasury + ? where id = ?", params![amount, gid])?;
            Ok(())
        })?;
        Ok(json!({ "taler": self.taler(uid) }))
    }
    pub fn groups(&self, uid: u32) -> R<Value> {
        let week = self.week();
        let mine: u32 = self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0))?;
        let list: Vec<Value> = self
            .db
            .prepare("select g.id, g.name, g.treasury, (select count(*) from users u where u.grp = g.id), (select coalesce(sum(e.taler), 0) from earn_week e join users u on u.id = e.user where u.grp = g.id and e.week = ?1) w from groups g order by w desc, g.id")?
            .query_map([week], |r| Ok(json!({ "id": r.get::<_, u32>(0)?, "name": r.get::<_, String>(1)?, "kasse": r.get::<_, i64>(2)?, "mitglieder": r.get::<_, i64>(3)?, "woche": r.get::<_, i64>(4)?, "meine": r.get::<_, u32>(0)? == mine })))?
            .collect::<Result<_, _>>()?;
        let members: Vec<Value> = if mine == 0 { Vec::new() } else { self.db.prepare("select u.name, coalesce((select taler from earn_week e where e.user = u.id and e.week = ?2), 0) w from users u where u.grp = ?1 order by w desc")?.query_map(params![mine, week], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "woche": r.get::<_, i64>(1)? })))?.collect::<Result<_, _>>()? };
        Ok(json!({ "liste": list, "meine": mine, "mitglieder": members, "kosten": eco::GROUP_COST, "frei": world::slots_of(SlotKind::Group, self.st).len() - self.plots.iter().filter(|p| p.group != 0).count() }))
    }

    // ---------- Großprojekte ----------
    pub fn projects(&self) -> R<Value> {
        let mut open = true;
        let list: Vec<Value> = eco::PROJECTS
            .iter()
            .map(|&(key, name, goal, _)| {
                let (funded, done): (i64, i64) = self.db.query_row("select funded, done from projects where key = ?", [key], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((0, 0));
                let n: i64 = self.db.query_row("select count(*) from project_gifts where key = ?", [key], |r| r.get(0)).unwrap_or(0);
                let v = json!({ "key": key, "name": name, "ziel": goal, "stand": funded, "fertig": done == 1, "offen": open && done == 0, "spender": n });
                open &= done == 1;
                v
            })
            .collect();
        Ok(json!({ "liste": list }))
    }
    /// Einzahlen ins nächste Großprojekt, aus der eigenen Tasche oder aus der Gruppenkasse. Nie mehr als noch fehlt.
    pub fn project_fund(&mut self, uid: u32, key: &str, amount: i64, from_group: bool) -> R<Value> {
        let Some(&(_, name, goal, bit)) = eco::PROJECTS.iter().find(|p| p.0 == key) else { return no(404, "Dieses Großprojekt gibt es nicht.") };
        let next = eco::PROJECTS.iter().find(|p| self.st.projects & p.3 == 0).map(|p| p.0);
        if next != Some(key) {
            return no(409, "Dieses Großprojekt ist gerade nicht an der Reihe.");
        }
        let funded: i64 = self.db.query_row("select funded from projects where key = ?", [key], |r| r.get(0)).optional()?.unwrap_or(0);
        let amount = amount.min(goal as i64 - funded);
        if amount < 1 {
            return no(400, "Bitte einen Betrag ab 1 Taler.");
        }
        let gid: u32 = self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0))?;
        let done = self.tx(|g| {
            if from_group {
                if gid == 0 || g.db.execute("update groups set treasury = treasury - ?1 where id = ?2 and treasury >= ?1", params![amount, gid])? != 1 {
                    return no(402, "In der Gruppenkasse ist zu wenig.");
                }
                g.db.execute("insert into ledger (user, delta, reason, ts) values (0, ?, 'projekt-aus-kasse', ?)", params![amount, g.now()])?;
            } else {
                g.pay(uid, -amount, "projekt")?;
            }
            g.db.execute("insert into projects (key, funded) values (?, ?) on conflict (key) do update set funded = funded + excluded.funded", params![key, amount])?;
            g.db.execute("insert into project_gifts (key, user, amount) values (?, ?, ?) on conflict (key, user) do update set amount = amount + excluded.amount", params![key, uid, amount])?;
            let done = funded + amount >= goal as i64;
            if done {
                g.db.execute("update projects set done = 1 where key = ?", [key])?;
            }
            Ok(done)
        })?;
        if done {
            self.apply_state(WorldState { projects: self.st.projects | bit, ..self.st })?;
            if bit == world::P_ISLAND {
                self.send_plots();
            }
        }
        self.hud_all(json!({ "t": "projekt", "key": key, "name": name, "stand": funded + amount, "ziel": goal, "fertig": done, "von": self.user_name(uid) }));
        Ok(json!({ "stand": funded + amount, "ziel": goal, "fertig": done, "eingezahlt": amount, "taler": self.taler(uid) }))
    }

    // ---------- Melden und Admin ----------
    pub fn report(&mut self, uid: u32, user: u32, plot: u32, reason: &str) -> R<Value> {
        if !REPORT_REASONS.contains(&reason) || (user == 0 && plot == 0) || user == uid {
            return no(400, "Bitte einen Spieler oder ein Grundstück und einen Grund wählen.");
        }
        let now = self.now();
        if self.report_at.get(&uid).is_some_and(|t| now - t < 30_000) {
            return no(429, "Bitte kurz warten, bevor du wieder etwas meldest.");
        }
        self.report_at.insert(uid, now);
        self.db.execute("insert into reports (reporter, user, plot, reason, ts) values (?, ?, ?, ?, ?)", params![uid, user, plot, reason, now])?;
        Ok(json!({ "ok": true }))
    }
    pub fn admin_overview(&self) -> R<Value> {
        let reports: Vec<Value> = self
            .db
            .prepare("select r.id, r.reporter, r.user, r.plot, r.reason, r.ts from reports r where r.done = 0 order by r.id desc limit 100")?
            .query_map([], |r| {
                let (rep, user, plot): (u32, u32, u32) = (r.get(1)?, r.get(2)?, r.get(3)?);
                let p = self.plots.iter().find(|p| p.id == plot);
                Ok(json!({ "id": r.get::<_, i64>(0)?, "von": self.user_name(rep), "user": user, "name": if user != 0 { self.user_name(user) } else { String::new() },
                    "plot": plot, "plotName": p.map(|p| p.name.clone()), "ort": p.map(|p| world::plot_rect(p.slot, p.size)), "grund": r.get::<_, String>(4)?, "zeit": r.get::<_, i64>(5)? }))
            })?
            .collect::<Result<_, _>>()?;
        let flags: Vec<Value> = self
            .db
            .prepare("select id, kind, a, b, detail, ts from flags where done = 0 order by id desc limit 100")?
            .query_map([], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "art": r.get::<_, String>(1)?, "kaeufer": self.user_name(r.get(2)?), "verkaeufer": self.user_name(r.get(3)?), "a": r.get::<_, u32>(2)?, "b": r.get::<_, u32>(3)?, "text": r.get::<_, String>(4)?, "zeit": r.get::<_, i64>(5)? })))?
            .collect::<Result<_, _>>()?;
        let users: Vec<Value> = self
            .db
            .prepare("select id, name, taler, level, banned from users order by name")?
            .query_map([], |r| {
                let id: u32 = r.get(0)?;
                Ok(json!({ "id": id, "name": r.get::<_, String>(1)?, "taler": r.get::<_, i64>(2)?, "stufe": r.get::<_, u32>(3)?, "gesperrt": r.get::<_, i64>(4)? == 1, "online": self.players.contains_key(&id) }))
            })?
            .collect::<Result<_, _>>()?;
        let plots: Vec<Value> = self.plots.iter().map(|p| json!({ "id": p.id, "name": p.name, "gruppe": p.group != 0, "ort": world::plot_rect(p.slot, p.size) })).collect();
        Ok(json!({ "meldungen": reports, "auffaellig": flags, "konten": users, "grundstuecke": plots, "online": self.players.len(),
            "takt": { "letzter_ms": self.stats.last_us as f64 / 1000.0, "max_ms": self.stats.max_us as f64 / 1000.0, "mittel_ms": self.stats.avg_us / 1000.0, "anzahl": self.stats.ticks } }))
    }
    /// Ein Klick: Bau auf dem Grundstück auf die Grundwelt zurücksetzen. Offene Meldungen dazu sind damit erledigt.
    pub fn admin_rollback(&mut self, plot: u32) -> R<Value> {
        let Some(p) = self.plots.iter().find(|p| p.id == plot).cloned() else { return no(404, "Dieses Grundstück gibt es nicht.") };
        self.reset_rect(world::plot_rect(p.slot, p.size));
        self.db.execute("update reports set done = 1 where plot = ?", [plot])?;
        self.save()?;
        Ok(json!({ "ok": true, "name": p.name }))
    }
    pub fn admin_ban(&mut self, uid: u32, on: bool) -> R<Value> {
        if self.db.execute("update users set banned = ? where id = ?", params![on as i64, uid])? != 1 {
            return no(404, "Dieses Konto gibt es im Campus nicht.");
        }
        if on {
            self.send(uid, &proto::S2C::Kick("Dein Konto wurde für Campus gesperrt.".into()));
            self.drop_player(uid);
            self.db.execute("update reports set done = 1 where user = ?", [uid])?;
        }
        Ok(json!({ "ok": true }))
    }
    pub fn admin_done(&mut self, kind: &str, id: i64) -> R<Value> {
        let table = match kind { "meldung" => "reports", "auffaellig" => "flags", _ => return no(400, "Unbekannte Art.") };
        self.db.execute(&format!("update {table} set done = 1 where id = ?"), [id])?;
        Ok(json!({ "ok": true }))
    }

    // ---------- Datenschutz ----------
    /// Alles, was Campus über ein Konto gespeichert hat (für den Datenexport des Lernhelfers).
    pub fn export_user(&self, uid: u32) -> R<Value> {
        let Some((name, taler, funde, level, created)): Option<(String, i64, i64, u32, i64)> = self.db.query_row("select name, taler, funde, level, created from users where id = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional()? else { return Ok(Value::Null) };
        let rows = |sql: &str| -> R<Vec<Value>> {
            let mut q = self.db.prepare(sql)?;
            let n = q.column_count();
            let cols: Vec<String> = q.column_names().iter().map(|s| s.to_string()).collect();
            let v = q
                .query_map([uid], |r| {
                    let mut o = serde_json::Map::new();
                    for i in 0..n {
                        let v = match r.get_ref(i)? {
                            rusqlite::types::ValueRef::Integer(x) => json!(x),
                            rusqlite::types::ValueRef::Real(x) => json!(x),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            _ => Value::Null,
                        };
                        o.insert(cols[i].clone(), v);
                    }
                    Ok(Value::Object(o))
                })?
                .collect::<Result<_, _>>()?;
            Ok(v)
        };
        let inv: Vec<Value> = self.db.prepare("select item, n from inv where user = ?")?.query_map([uid], |r| Ok(json!({ "gegenstand": items::item_name(r.get(0)?), "anzahl": r.get::<_, u32>(1)? })))?.collect::<Result<_, _>>()?;
        Ok(json!({
            "name": name, "taler": taler, "funde": funde, "stufe": level, "dabei_seit": created, "aussehen": self.look_of(uid), "vorrat": inv,
            "grundstueck": self.plots.iter().find(|p| p.owner == uid).map(|p| json!({ "ort": world::plot_rect(p.slot, p.size), "mitbauer": p.acl.iter().map(|&a| self.user_name(a)).collect::<Vec<_>>() })),
            "kassenbuch": rows("select delta as taler, reason as grund, ts as zeit from ledger where user = ? order by id")?,
            "lerntage": rows("select day as tag, focus as fokus_minuten, cards as karten from daily where user = ? order by day")?,
            "wochen": rows("select week as woche, taler from earn_week where user = ? order by week")?,
            "angebote": rows("select item as gegenstand, qty as menge, start, buyout as sofortkauf, bid as gebot, ends as ende, state as zustand from auctions where seller = ?")?,
            "gebote": rows("select item as gegenstand, bid as gebot, ends as ende, state as zustand from auctions where bidder = ?")?,
            "grossprojekte": rows("select key as projekt, amount as taler from project_gifts where user = ?")?,
            "meldungen": rows("select reason as grund, ts as zeit from reports where reporter = ?")?,
            "lerngruppe": rows("select g.name from groups g join users u on u.grp = g.id where u.id = ?")?,
        }))
    }
    /// Konto restlos aus Campus entfernen (Aufruf des Lernhelfers beim Löschen des Kontos).
    pub fn delete_user(&mut self, uid: u32) -> R<Value> {
        if self.db.query_row("select 1 from users where id = ?", [uid], |_| Ok(())).optional()?.is_none() {
            return Ok(json!({ "ok": true, "war": false }));
        }
        self.drop_player(uid);
        if self.db.query_row("select grp from users where id = ?", [uid], |r| r.get::<_, u32>(0))? != 0 {
            self.group_leave(uid)?;
        }
        let rect = self.plots.iter().find(|p| p.owner == uid).map(|p| world::plot_rect(p.slot, p.size));
        self.tx(|g| {
            // Gebote auf Angebote dieses Kontos gehen zurück, die Angebote verschwinden
            let own: Vec<(Option<i64>, Option<u32>)> = g.db.prepare("select bid, bidder from auctions where seller = ? and state = 0")?.query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
            for (bid, bidder) in own {
                if let (Some(b), Some(p)) = (bid, bidder) {
                    g.pay(p, b, "angebot-geloescht")?;
                }
            }
            g.db.execute("delete from auctions where seller = ?", [uid])?;
            g.db.execute("update auctions set bid = null, bidder = null where bidder = ? and state = 0", [uid])?;
            g.db.execute("update auctions set bidder = 0 where bidder = ?", [uid])?;
            g.db.execute("insert into project_gifts (key, user, amount) select key, 0, amount from project_gifts where user = ?1 on conflict (key, user) do update set amount = amount + excluded.amount", [uid])?;
            for t in ["inv", "timed", "daily", "ledger", "stats", "earn_week", "plot_acl", "duel_elo", "dungeon_best", "npc_rewards"] {
                g.db.execute(&format!("delete from {t} where user = ?"), [uid])?;
            }
            g.db.execute("delete from project_gifts where user = ?", [uid])?;
            g.db.execute("delete from plot_acl where plot in (select id from plots where owner = ?)", [uid])?;
            g.db.execute("delete from plots where owner = ?", [uid])?;
            g.db.execute("delete from reports where reporter = ?1 or user = ?1", [uid])?;
            g.db.execute("delete from flags where a = ?1 or b = ?1", [uid])?;
            g.db.execute("delete from transfers where from_u = ?1 or to_u = ?1", [uid])?;
            g.db.execute("delete from users where id = ?", [uid])?;
            Ok(())
        })?;
        if let Some(r) = rect {
            self.reset_rect(r);
        }
        self.load_plots()?;
        self.send_plots();
        Ok(json!({ "ok": true, "war": true }))
    }

    // ---------- Verteiler ----------
    pub fn get(&mut self, u: &User, op: &str) -> R<Value> {
        if let Some(r) = self.p21_get(u, op) {
            return r; // Phasen 21–25 (instance.rs): Duell, Katakomben, Verwaltung mit 403 für alle anderen
        }
        if let Some(r) = self.sky_get(u.id, op) {
            return r;
        }
        match op {
            "me" => self.me(u),
            "auctions" => self.auctions(u.id),
            "groups" => self.groups(u.id),
            "projects" => self.projects(),
            "world" => self.world_info(),
            "ranking" => self.ranking(),
            "islands" => self.islands(u.id),
            "admin/overview" if u.admin => self.admin_overview(),
            _ => no(404, "Gibt es nicht."),
        }
    }
    pub fn post(&mut self, u: &User, op: &str, b: &Value) -> R<Value> {
        let id = u.id;
        if let Some(r) = self.p21_post(u, op, b) {
            return r;
        }
        if let Some(r) = self.sky_post(id, u.admin, op, b) {
            return r;
        }
        let flag = |k: &str| b.get(k).and_then(Value::as_bool).unwrap_or(false);
        match op {
            "shop/buy" => self.buy(id, item(b, "item")?, int(b, "n")?),
            "fund/open" => self.open_fund(id),
            "look" => self.set_look(id, b),
            "equip" => self.equip(id, int(b, "slot")?.min(255) as u8, item(b, "item")?),
            "auction/create" => self.auction_create(id, item(b, "item")?, int(b, "menge")?, int(b, "start")?, b.get("sofort").filter(|v| !v.is_null()).map(|_| int(b, "sofort")).transpose()?, int(b, "stunden")?),
            "auction/bid" => self.auction_bid(id, int(b, "id")?, int(b, "taler")?),
            "auction/buyout" => self.auction_buyout(id, int(b, "id")?),
            "plot/expand" => self.plot_expand(id, flag("gruppe")),
            "plot/visit" => self.plot_visit(id, int(b, "modus")?),
            "plot/acl" => self.plot_acl(id, text(b, "name")?, b.get("add").and_then(Value::as_bool).unwrap_or(true)),
            "group/create" => self.group_create(id, text(b, "name")?),
            "group/join" => self.group_join(id, int(b, "id")? as u32),
            "group/leave" => self.group_leave(id),
            "group/deposit" => self.group_deposit(id, int(b, "taler")?),
            "project/fund" => self.project_fund(id, text(b, "key")?, int(b, "taler")?, flag("gruppe")),
            "report" => self.report(id, int(b, "user")? as u32, int(b, "plot")? as u32, text(b, "grund")?),
            "admin/rollback" if u.admin => self.admin_rollback(int(b, "plot")? as u32),
            "admin/ban" if u.admin => self.admin_ban(int(b, "user")? as u32, flag("an")),
            "admin/done" if u.admin => self.admin_done(text(b, "art")?, int(b, "id")?),
            _ => no(404, "Gibt es nicht."),
        }
    }
}

/// Alles, was die Bedienoberfläche über Blöcke, Cosmetics und Regeln wissen muss – direkt aus den geteilten Registern.
pub fn registry() -> Value {
    let blocks: Vec<Value> = world::BLOCKS.iter().map(|b| json!({ "id": b.id, "name": b.name, "rgb": b.rgb, "preis": b.price, "f": b.flags })).collect();
    let cos: Vec<Value> = items::COSMETICS.iter().map(|c| json!({ "id": c.id, "platz": c.slot, "name": c.name, "weg": format!("{:?}", c.path), "preis": c.price, "form": c.shape, "rgb": c.rgb, "rgb2": c.rgb2, "titel": items::plate_title(c.id) })).collect();
    let sky_items: Vec<Value> = campus_shared::skyblock::all_items().map(|(id, name)| json!({ "id": id, "name": name, "verdichtet": campus_shared::skyblock::is_compacted(id) })).collect();
    let npcs: Vec<Value> = campus_shared::content::NPCS.iter().map(|n| json!({ "id": n.id, "name": n.name, "zeilen": n.lines, "quest": n.quest })).collect();
    let quests: Vec<Value> = campus_shared::content::QUESTS.iter().map(|q| json!({ "id": q.id, "name": q.name, "schritte": q.steps.iter().map(|s| s.text).collect::<Vec<_>>() })).collect();
    let tools: Vec<Value> = campus_shared::skyblock::TOOLS.iter().map(|t| json!({ "id": t.id, "name": t.name, "mana": t.mana, "rgb": t.rgb })).collect();
    let minions: Vec<Value> = campus_shared::skyblock::MINIONS.iter().map(|m| json!({ "name": m.name, "produkt": m.product, "rgb": m.rgb })).collect();
    json!({
        "sky": { "items": sky_items, "npcs": npcs, "quests": quests, "werkzeuge": tools, "minions": minions, "faecher": campus_shared::skyblock::FAECHER,
            "faehigkeiten": campus_shared::skyblock::SKILLS, "kanaele": ["Hub", "Insel", "Gruppe"], "warp": campus_shared::world::SPOTS.iter().map(|s| s.0).collect::<Vec<_>>() },
        "rpg": crate::rpg::registry_rpg(),
        "bloecke": blocks, "cosmetics": cos, "plaetze": items::SLOT_NAMES, "sprueche": proto::PHRASES, "emotes": ["Winken", "Jubeln", "Hinsetzen", "Tanz"],
        "haut": items::SKINS, "farben": items::PALETTE, "haare": items::HAIR_NAMES, "kleidung": items::OUTFIT_NAMES, "bauplan": BLUEPRINT, "gruende": REPORT_REASONS,
        "regeln": { "fokus": eco::FOCUS_TALER, "fokusTag": eco::FOCUS_DAILY_CAP, "check": eco::CHECK_TALER, "karte": eco::CARD_TALER, "kartenTag": eco::CARD_DAILY_CAP,
            "klausur": eco::EXAM_TALER, "challenge": eco::CHALLENGE_TALER, "serie": eco::STREAK_PCT_PER_DAY, "serieMax": eco::STREAK_PCT_CAP, "pruefungTage": eco::EXAM_PHASE_DAYS,
            "gebuehr": eco::AUCTION_FEE_PCT, "gruppe": eco::GROUP_COST, "wochenziel": eco::WORLD_GOAL_WEEKLY, "grundstueck": eco::PLOT_SIZES, "ausbau": eco::PLOT_COST, "ausbauStufe": eco::PLOT_LEVEL },
    })
}
