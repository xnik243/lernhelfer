//! SkyBlock-Spielregeln auf dem Server (Phasen 9–13): Chat und Befehle, NPCs und Quests, Lernhelfer-Splitter und Lern-Energie,
//! Fähigkeiten, Sammlungen, Lern-Minions, Verdichten, Bazaar, Bank, Fokus-Mana und Werkzeuge.
//! Nichts davon läuft im Takt: Minions, Zinsen und Mana werden beim nächsten Zugriff aus Zeitstempeln nachgerechnet.
use crate::econ::{int, LhEvent};
use crate::game::Game;
use crate::{no, E, R};
use campus_shared::economy::Kind;
use campus_shared::proto::{Warp, S2C};
use campus_shared::skyblock::{self as sb, Goal};
use campus_shared::world::{self, CH, WORLD};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub const SCHEMA: &str = "
create table if not exists sky (user integer primary key, fuel_until integer not null default 0, mana integer not null default 100, mana_at integer not null default 0,
  bank integer not null default 0 check (bank >= 0), bank_at integer not null default 0, last_study integer not null default 0,
  quest integer not null default 0, qstep integer not null default 0, qn integer not null default 0, weber integer not null default 0, muted integer not null default 0);
create table if not exists skills (user integer not null, skill integer not null, xp integer not null, primary key (user, skill)) without rowid;
create table if not exists coll (user integer not null, item integer not null, n integer not null, primary key (user, item)) without rowid;
create table if not exists quests_done (user integer not null, quest integer not null, primary key (user, quest)) without rowid;
create table if not exists minions (id integer primary key, user integer not null, kind integer not null, tier integer not null, x integer not null, y integer not null, z integer not null,
  last integer not null, acc integer not null default 0, stored integer not null default 0);
create table if not exists orders (id integer primary key, user integer not null, item integer not null, buy integer not null, price integer not null check (price > 0),
  qty integer not null, open integer not null check (open >= 0), ts integer not null);
create index if not exists orders_book on orders (item, buy, price);
create table if not exists chest (plot integer not null, item integer not null, n integer not null check (n > 0), primary key (plot, item)) without rowid;
create table if not exists schatz (user integer not null, pos integer not null, day integer not null, primary key (user, pos)) without rowid;
";
/// Plätze einer Insel-Truhe (verschiedene Gegenstände).
pub const CHEST_MAX: usize = 27;

pub const CHAT_MAX: usize = 160;
// ponytail: kurze feste Liste; bei Bedarf aus der Verwaltung pflegbar machen
const BAD: [&str; 16] = ["arschloch", "fotze", "hurensohn", "wichser", "ficken", "fick dich", "hure", "schlampe", "missgeburt", "spast", "nazi", "bitch", "fuck", "shit", "cunt", "nigger"];

/// Text säubern: Steuerzeichen raus, spitze Klammern entschärft, Länge begrenzt, Schimpfwörter gesternt.
pub fn clean(s: &str) -> String {
    let mut t: String = s.chars().filter(|c| !c.is_control()).map(|c| match c { '<' => '‹', '>' => '›', _ => c }).take(CHAT_MAX).collect();
    t = t.trim().to_string();
    let low = t.to_lowercase();
    for w in BAD {
        let mut from = 0;
        while let Some(i) = low[from..].find(w).map(|i| i + from) {
            // gleiche Byte-Stellen, weil to_lowercase bei diesen ASCII-Wörtern die Länge nicht ändert
            if t.is_char_boundary(i) && t.is_char_boundary(i + w.len()) {
                t.replace_range(i..i + w.len(), &"*".repeat(w.len()));
            }
            from = i + w.len();
        }
    }
    t
}

impl Game {
    // ---------- Grundlagen ----------
    pub fn sky_row(&self, uid: u32) -> R<()> {
        let now = self.now();
        self.db.execute("insert or ignore into sky (user, mana_at, bank_at) values (?, ?, ?)", params![uid, now, now])?;
        Ok(())
    }
    fn sky_i64(&self, uid: u32, col: &str) -> R<i64> {
        self.sky_row(uid)?;
        Ok(self.db.query_row(&format!("select {col} from sky where user = ?"), [uid], |r| r.get(0))?)
    }
    pub fn levels(&self, uid: u32) -> [u32; 7] {
        let mut l = [1u32; 7];
        if let Ok(mut q) = self.db.prepare_cached("select skill, xp from skills where user = ?") {
            if let Ok(rows) = q.query_map([uid], |r| Ok((r.get::<_, u8>(0)?, r.get::<_, u64>(1)?))) {
                for (s, xp) in rows.flatten() {
                    if (s as usize) < 7 {
                        l[s as usize] = sb::level_of(xp);
                    }
                }
            }
        }
        l
    }
    pub fn add_xp(&mut self, uid: u32, skill: u8, xp: u64) -> R<()> {
        if xp == 0 {
            return Ok(());
        }
        let before = self.levels(uid)[skill as usize];
        self.db.execute("insert into skills (user, skill, xp) values (?, ?, ?) on conflict (user, skill) do update set xp = xp + excluded.xp", params![uid, skill, xp as i64])?;
        let after = self.levels(uid)[skill as usize];
        if after > before {
            self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("{} erreicht Stufe {after}!", sb::SKILLS[skill as usize]) }));
        }
        Ok(())
    }
    /// Gegenstände gewinnen (Abbau, Minion, Lernen): ins Lager und in die Sammlung.
    pub fn gain(&mut self, uid: u32, item: u16, n: u32) -> R<u32> {
        let total = self.give(uid, item, n as i64)?;
        let base = sb::compact_of(item).map_or(item, |_| item);
        let old: u64 = self.db.query_row("select n from coll where user = ? and item = ?", params![uid, base], |r| r.get(0)).optional()?.unwrap_or(0);
        self.db.execute("insert into coll (user, item, n) values (?, ?, ?) on conflict (user, item) do update set n = n + excluded.n", params![uid, base, n as i64])?;
        let (t0, t1) = (sb::coll_tier(old), sb::coll_tier(old + n as u64));
        if t1 > t0 {
            let neu: Vec<&str> = sb::RECIPES.iter().filter(|r| r.unlock.is_some_and(|(i, t)| i == base && t > t0 && t <= t1)).filter_map(|r| sb::name(r.out)).collect();
            self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("Sammlung {} Stufe {t1}!{}", campus_shared::items::item_name(base), if neu.is_empty() { String::new() } else { format!(" Neu: {}", neu.join(", ")) }) }));
        }
        Ok(total)
    }

    // ---------- Chat und Befehle ----------
    pub fn say(&mut self, id: u32, ch: u8, text: &str) {
        let now = self.now();
        let Some(pl) = self.players.get_mut(&id) else { return };
        if now - pl.last_chat < campus_shared::economy::CHAT_COOLDOWN_MS as i64 {
            return self.system(id, "Langsam – höchstens eine Nachricht alle 1,5 Sekunden.");
        }
        pl.last_chat = now;
        let (name, pos, grp) = (pl.name.clone(), pl.p, pl.group);
        if self.sky_i64(id, "muted").unwrap_or(0) > now {
            return self.system(id, "Du bist im Chat gerade stummgeschaltet.");
        }
        let t = clean(text);
        if t.is_empty() {
            return;
        }
        if let Some(cmd) = t.strip_prefix('/') {
            return self.command(id, cmd);
        }
        // Insel-Kanal: im Hub-Archipel alle dort, sonst alle auf demselben Platz
        let slot = |p: [f32; 3]| world::slot_at(p[0] as i32, p[2] as i32);
        let to: Vec<u32> = match ch {
            1 => self.players.values().filter(|p| slot(p.p) == slot(pos)).map(|p| p.id).collect(),
            2 if grp != 0 => self.players.values().filter(|p| p.group == grp).map(|p| p.id).collect(),
            2 => return self.system(id, "Du bist in keiner Lerngruppe."),
            _ => self.players.keys().copied().collect(),
        };
        let m = S2C::Said { id, ch: ch.min(2), name, text: t };
        for o in to {
            self.send(o, &m);
        }
    }
    pub fn system(&mut self, id: u32, text: &str) {
        self.send(id, &S2C::Said { id: 0, ch: 4, name: String::new(), text: text.into() });
    }
    fn command(&mut self, id: u32, cmd: &str) {
        let (word, rest) = cmd.split_once(' ').map_or((cmd, ""), |(a, b)| (a, b.trim()));
        let r: R<()> = match word.to_lowercase().as_str() {
            "help" | "hilfe" => {
                self.system(id, "Befehle: /is · /hub · /visit <Name> · /bazaar · /ah · /msg <Name> <Text> · /stash · /trade <Name> · /coop invite|remove <Name> · /like · /quests · /help");
                Ok(())
            }
            "stash" | "beutel" => self.stash_claim_cmd(id),
            "trade" | "handel" => self.trade_request(id, rest),
            "coop" => self.coop_cmd(id, rest),
            "like" => self.like_here(id),
            "quests" | "questbuch" | "q" => {
                self.hud(id, json!({ "t": "oeffne", "reiter": "Questbuch" }));
                Ok(())
            }
            "hub" => self.warp(id, Warp::Spot(0)),
            "is" | "insel" | "home" => self.warp(id, Warp::Home),
            "visit" | "besuch" => {
                let p = self.plots.iter().find(|p| p.owner != 0 && p.name.eq_ignore_ascii_case(rest)).map(|p| p.id);
                match p {
                    Some(p) => self.warp(id, Warp::Plot(p)),
                    None => no(404, "Keine Insel mit diesem Namen."),
                }
            }
            "bazaar" | "bz" => {
                self.hud(id, json!({ "t": "oeffne", "reiter": "Bazaar" }));
                Ok(())
            }
            "ah" | "auktion" => {
                self.hud(id, json!({ "t": "oeffne", "reiter": "Auktionshaus" }));
                Ok(())
            }
            "msg" | "w" => {
                let (who, text) = rest.split_once(' ').unwrap_or((rest, ""));
                let target = self.players.values().find(|p| p.name.eq_ignore_ascii_case(who)).map(|p| p.id);
                match (target, text.trim()) {
                    (_, "") => no(400, "So: /msg <Name> <Text>"),
                    (None, _) => no(404, "Diese Person ist gerade nicht im Campus."),
                    (Some(t), text) => {
                        let name = self.players[&id].name.clone();
                        let m = S2C::Said { id, ch: 3, name, text: text.into() };
                        self.send(t, &m);
                        if t != id {
                            let to = self.players[&t].name.clone();
                            self.send(id, &S2C::Said { id, ch: 3, name: format!("an {to}"), text: text.into() });
                        }
                        Ok(())
                    }
                }
            }
            _ => no(404, "Unbekannter Befehl. /help zeigt alle."),
        };
        if let Err(E(_, why)) = r {
            self.system(id, &why);
        }
    }
    pub fn admin_mute(&mut self, uid: u32, minutes: i64) -> R<Value> {
        self.sky_row(uid)?;
        let until = if minutes > 0 { self.now() + minutes.min(60 * 24 * 30) * 60_000 } else { 0 };
        self.db.execute("update sky set muted = ? where user = ?", params![until, uid])?;
        if minutes > 0 {
            self.system(uid, &format!("Du bist für {minutes} Minuten im Chat stummgeschaltet."));
        }
        Ok(json!({ "ok": true, "bis": until }))
    }

    // ---------- NPCs und Quests ----------
    pub fn talk(&mut self, id: u32, npc_id: u8) -> R<()> {
        let n = sb::npc(npc_id).ok_or(E(404, "Diese Person gibt es nicht.".into()))?;
        let p = self.players.get(&id).map(|p| p.p).ok_or(E(401, "Nicht verbunden.".into()))?;
        let at = sb::npc_pos(n);
        if (p[0] - at[0]).powi(2) + (p[2] - at[2]).powi(2) > 8.0 * 8.0 {
            return no(403, "Geh näher heran.");
        }
        self.sky_row(id)?;
        let mut gift = false;
        if npc_id == sb::DEKAN && self.sky_i64(id, "weber")? == 0 {
            self.tx(|g| {
                g.db.execute("update sky set weber = 1 where user = ?", [id])?;
                g.pay(id, sb::STARTER_TALER, "start:weber")?;
                g.give(id, sb::minion_item(0, 1), 1)?;
                g.give(id, sb::HAMMER, 1)?;
                g.fuel(id, sb::STARTER_FUEL_H * sb::HOUR_MS)
            })?;
            gift = true;
        }
        // Quest dieser Person beginnen, wenn gerade keine läuft und sie noch offen ist
        if let Some(q) = n.quest {
            let (cur, done): (i64, bool) = (self.sky_i64(id, "quest")?, self.db.query_row("select 1 from quests_done where user = ? and quest = ?", params![id, q], |_| Ok(())).optional()?.is_some());
            if cur == 0 && !done {
                self.db.execute("update sky set quest = ?, qstep = 0, qn = 0 where user = ?", params![q, id])?;
            }
        }
        self.quest_hit(id, |g| (g == Goal::Talk(npc_id)).then_some(1));
        self.hud(id, json!({ "t": "dialog", "npc": npc_id, "geschenk": gift }));
        Ok(())
    }
    /// Fortschritt melden: `f` sagt, um wie viel ein Ziel vorankommt.
    pub fn quest_hit(&mut self, uid: u32, f: impl Fn(Goal) -> Option<u32>) {
        self.book_hit(uid, &f); // Campus-Questbuch (island.rs) hört dieselben Ziele
        let Ok((q, step, n)) = self.db.query_row("select quest, qstep, qn from sky where user = ?", [uid], |r| Ok((r.get::<_, u8>(0)?, r.get::<_, usize>(1)?, r.get::<_, u32>(2)?))) else { return };
        let Some(quest) = sb::quest(q) else { return };
        let Some(s) = quest.steps.get(step) else { return };
        let Some(add) = f(s.goal) else { return };
        let need = s.goal.need();
        let n = n + add;
        if n < need {
            let _ = self.db.execute("update sky set qn = ? where user = ?", params![n, uid]);
            self.hud(uid, json!({ "t": "quest" }));
            return;
        }
        let _ = self.pay(uid, s.taler as i64, &format!("quest:{q}:{step}"));
        let next = step + 1;
        if next >= quest.steps.len() {
            let _ = self.db.execute("insert or ignore into quests_done (user, quest) values (?, ?)", params![uid, q]);
            let _ = self.db.execute("update sky set quest = 0, qstep = 0, qn = 0 where user = ?", [uid]);
            for &(_, item, k) in campus_shared::content::QUEST_REWARDS.iter().filter(|r| r.0 == q) {
                if let Ok(n) = self.give(uid, item, k as i64) {
                    self.send(uid, &S2C::Inv { item, n });
                }
            }
            self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("Quest „{}“ geschafft! +{} Taler", quest.name, s.taler) }));
        } else {
            let _ = self.db.execute("update sky set qstep = ?, qn = 0 where user = ?", params![next, uid]);
            self.hud(uid, json!({ "t": "toast", "art": "gut", "text": format!("Erledigt: {} (+{} Taler). Nächstes Ziel: {}", s.text, s.taler, quest.steps[next].text) }));
        }
        self.hud(uid, json!({ "t": "quest" }));
    }
    fn objective(&self, uid: u32) -> Option<(String, String, u32, u32)> {
        let (q, step, n): (u8, usize, u32) = self.db.query_row("select quest, qstep, qn from sky where user = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).ok()?;
        let quest = sb::quest(q)?;
        let s = quest.steps.get(step)?;
        Some((quest.name.into(), s.text.into(), n, s.goal.need()))
    }

    // ---------- Lernhelfer: Splitter, Kristalle, Lern-Energie, Fähigkeits-EP ----------
    pub fn fuel(&mut self, uid: u32, ms: i64) -> R<()> {
        let now = self.now();
        let (until, _) = sb::add_fuel(self.sky_i64(uid, "fuel_until")?, now, ms);
        self.db.execute("update sky set fuel_until = ? where user = ?", params![until, uid])?;
        Ok(())
    }
    /// Ein Lernereignis aus dem Lernhelfer (läuft innerhalb der Zahltag-Transaktion).
    pub fn learn(&mut self, uid: u32, e: &LhEvent, kind: Kind) -> R<Value> {
        let s = sb::subject_of(&e.fach);
        let lv = self.levels(uid);
        let skill = if s < 4 { s } else { sb::SK_FOKUS };
        let (shards, kristalle) = match kind {
            Kind::Focus => (e.menge / 5, e.menge / 15),
            Kind::Check => (3, 0),
            Kind::Card => (e.menge.div_ceil(2), 0),
            Kind::Exam => (30, 3),
            Kind::Challenge => (5, 1),
        };
        let shards = sb::with_yield(shards as u64, lv[skill as usize]) as u32;
        if shards > 0 {
            self.gain(uid, sb::SHARD + s as u16, shards)?;
        }
        if kristalle > 0 {
            self.give(uid, sb::FOKUS_KRISTALL, kristalle as i64)?;
        }
        self.add_xp(uid, skill, shards as u64 * 10)?;
        if kind == Kind::Focus {
            self.add_xp(uid, sb::SK_FOKUS, e.menge as u64 * 2)?;
        }
        let ms = sb::fuel_ms(&e.art, e.menge);
        if ms > 0 {
            self.fuel(uid, ms)?;
        }
        self.db.execute("update sky set last_study = ? where user = ?", params![self.now(), uid])?;
        self.quest_hit(uid, |g| (matches!(g, Goal::Learn(_)) || matches!(g, Goal::Check(_)) && kind == Kind::Check).then_some(1));
        if shards > 0 {
            let fach = campus_shared::items::item_name(sb::SHARD + s as u16);
            self.hud(uid, json!({ "t": "aktion", "text": format!("+{} {}-XP | +{shards} {fach}", shards * 10, sb::SKILLS[skill as usize]) }));
        }
        Ok(json!({ "fach": sb::FACH_KEYS[s as usize], "art": e.art, "splitter": shards, "kristalle": kristalle, "energie_ms": ms }))
    }

    /// Alles SkyBlock-Eigene für /me und die Anzeigetafel.
    pub fn sky_me(&mut self, uid: u32) -> R<Value> {
        self.bank_settle(uid)?;
        let now = self.now();
        let (fuel_until, bank): (i64, i64) = self.db.query_row("select fuel_until, bank from sky where user = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let lv = self.levels(uid);
        let mana_max = self.stats(uid).max_mana as u32;
        let mana = self.mana(uid, mana_max)?;
        let mut splitter = serde_json::Map::new();
        for (i, k) in sb::FACH_KEYS.iter().enumerate() {
            splitter.insert((*k).into(), json!(self.inv(uid, sb::SHARD + i as u16)));
        }
        let xp: Vec<u64> = (0..7u8).map(|s| self.db.query_row("select xp from skills where user = ? and skill = ?", params![uid, s], |r| r.get(0)).optional().ok().flatten().unwrap_or(0)).collect();
        let skills: Vec<Value> = (0..7).map(|i| json!({ "name": sb::SKILLS[i], "stufe": lv[i], "ep": xp[i], "naechste": sb::xp_for(lv[i] + 1) })).collect();
        let minions: i64 = self.db.query_row("select count(*) from minions where user = ?", [uid], |r| r.get(0))?;
        let obj = self.objective(uid);
        Ok(json!({
            "splitter": splitter, "kristalle": self.inv(uid, sb::FOKUS_KRISTALL), "treibstoff_h": ((fuel_until - now).max(0) as f64 / sb::HOUR_MS as f64 * 10.0).round() / 10.0,
            "mana": mana, "mana_max": mana_max, "bank": bank, "faehigkeiten": skills, "weber": self.sky_i64(uid, "weber")? != 0,
            "minions": { "belegt": minions, "plaetze": sb::minion_slots(&lv) },
            "quest": obj.as_ref().map(|o| json!({ "name": o.0, "ziel": o.1, "stand": o.2, "von": o.3 })),
            "ziel": obj.map(|o| if o.3 > 1 { format!("{} ({}/{})", o.1, o.2, o.3) } else { o.1 }),
            "buch": self.book_state(uid),
            "werte": self.stats_json(uid),
        }))
    }

    // ---------- Minions ----------
    fn minion_settle(&mut self, mid: i64) -> R<()> {
        let now = self.now();
        let (uid, kind, tier, last, acc, stored): (u32, u8, u8, i64, i64, u32) =
            self.db.query_row("select user, kind, tier, last, acc, stored from minions where id = ?", [mid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        let fuel = self.sky_i64(uid, "fuel_until")?;
        let (n, acc) = sb::minion_step(kind, tier, last, now, fuel, acc, stored);
        self.db.execute("update minions set last = ?, acc = ?, stored = stored + ? where id = ?", params![now, acc, n, mid])?;
        Ok(())
    }
    /// Minions einer Insel (für die Anzeige), frisch nachgerechnet.
    pub fn minions_on(&mut self, plot: u32) -> R<Value> {
        let Some(p) = self.plots.iter().find(|p| p.id == plot).cloned() else { return no(404, "Diese Insel gibt es nicht.") };
        let r = world::plot_rect(p.slot, p.size);
        let ids: Vec<i64> = self.db.prepare("select id from minions where x >= ? and z >= ? and x < ? and z < ?")?.query_map(params![r.0, r.1, r.0 + r.2, r.1 + r.2], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for &m in &ids {
            self.minion_settle(m)?;
        }
        let v: Vec<Value> = self
            .db
            .prepare("select id, user, kind, tier, x, y, z, stored from minions where x >= ? and z >= ? and x < ? and z < ?")?
            .query_map(params![r.0, r.1, r.0 + r.2, r.1 + r.2], |r| {
                let (kind, tier): (u8, u8) = (r.get(2)?, r.get(3)?);
                Ok(json!({ "id": r.get::<_, i64>(0)?, "user": r.get::<_, u32>(1)?, "art": kind, "stufe": tier, "pos": [r.get::<_, i32>(4)?, r.get::<_, i32>(5)?, r.get::<_, i32>(6)?],
                    "lager": r.get::<_, u32>(7)?, "max": sb::minion_cap(tier), "name": sb::name(sb::minion_item(kind, tier)) }))
            })?
            .collect::<Result<_, _>>()?;
        Ok(json!({ "minions": v }))
    }
    pub fn minion_place(&mut self, uid: u32, item: u16, x: i32, y: i32, z: i32) -> R<Value> {
        let (kind, tier) = sb::minion_of(item).ok_or(E(400, "Das ist kein Minion.".into()))?;
        let own = self.plots.iter().any(|p| p.owner == uid && world::in_rect(world::plot_rect(p.slot, p.size), x, z));
        if !own || !(1..CH - 2).contains(&y) {
            return no(403, "Minions stehen nur auf deiner eigenen Insel.");
        }
        if world::solid(self.block(x, y, z)) || !world::solid(self.block(x, y - 1, z)) {
            return no(409, "Stell den Minion auf einen freien Platz auf festem Boden.");
        }
        let used: u32 = self.db.query_row("select count(*) from minions where user = ?", [uid], |r| r.get(0))?;
        let slots = sb::minion_slots(&self.levels(uid));
        if used >= slots {
            return no(409, &format!("Alle {slots} Minion-Plätze belegt. Mehr gibt es mit höheren Fähigkeiten."));
        }
        if self.db.query_row("select 1 from minions where x = ? and y = ? and z = ?", params![x, y, z], |_| Ok(())).optional()?.is_some() {
            return no(409, "Da steht schon ein Minion.");
        }
        let now = self.now();
        self.tx(|g| {
            g.give(uid, item, -1)?;
            g.db.execute("insert into minions (user, kind, tier, x, y, z, last) values (?, ?, ?, ?, ?, ?, ?)", params![uid, kind, tier, x, y, z, now])?;
            Ok(())
        })?;
        self.quest_hit(uid, |g| (g == Goal::PlaceMinion).then_some(1));
        Ok(json!({ "ok": true }))
    }
    /// Lager leeren (`pickup`: Minion zurück ins Inventar).
    pub fn minion_collect(&mut self, uid: u32, mid: i64, pickup: bool) -> R<Value> {
        let owner: Option<u32> = self.db.query_row("select user from minions where id = ?", [mid], |r| r.get(0)).optional()?;
        if owner != Some(uid) {
            return no(404, "Das ist nicht dein Minion.");
        }
        self.minion_settle(mid)?;
        let (kind, tier, stored): (u8, u8, u32) = self.db.query_row("select kind, tier, stored from minions where id = ?", [mid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let product = sb::MINIONS[kind as usize].product;
        self.tx(|g| {
            if stored > 0 {
                g.gain(uid, product, stored)?;
            }
            if pickup {
                g.db.execute("delete from minions where id = ?", [mid])?;
                g.give(uid, sb::minion_item(kind, tier), 1)?;
            } else {
                g.db.execute("update minions set stored = 0 where id = ?", [mid])?;
            }
            Ok(())
        })?;
        self.add_xp(uid, sb::SK_BAU, stored as u64 / 4)?;
        Ok(json!({ "ok": true, "menge": stored, "item": product }))
    }

    // ---------- Crafting ----------
    pub fn craft(&mut self, uid: u32, out: u16, times: u32) -> R<Value> {
        let times = times.clamp(1, 64);
        let r = sb::RECIPES.iter().find(|r| r.out == out).ok_or(E(404, "Dafür gibt es kein Rezept.".into()))?;
        if let Some((item, tier)) = r.unlock {
            let n: u64 = self.db.query_row("select n from coll where user = ? and item = ?", params![uid, item], |r| r.get(0)).optional()?.unwrap_or(0);
            if sb::coll_tier(n) < tier {
                return no(403, &format!("Freigeschaltet ab Sammlung {} Stufe {tier}.", campus_shared::items::item_name(item)));
            }
        }
        self.tx(|g| {
            for &(item, n) in r.ins {
                g.give(uid, item, -((n * times) as i64)).map_err(|_| E(402, format!("Es fehlt: {} × {}", n * times, campus_shared::items::item_name(item))))?;
            }
            g.give(uid, out, (r.n * times) as i64)?;
            Ok(())
        })?;
        self.add_xp(uid, sb::SK_BAU, 5 * times as u64)?;
        self.quest_hit(uid, |g| (g == Goal::Craft(out)).then_some(1));
        if sb::is_compacted(out) {
            self.quest_hit(uid, |g| (g == Goal::Compact).then_some(1));
        }
        Ok(json!({ "ok": true, "item": out, "n": r.n * times }))
    }
    pub fn recipes(&self, uid: u32) -> R<Value> {
        let coll: std::collections::HashMap<u16, u64> = self.db.prepare("select item, n from coll where user = ?")?.query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let v: Vec<Value> = sb::RECIPES
            .iter()
            .map(|r| {
                let open = r.unlock.is_none_or(|(i, t)| sb::coll_tier(*coll.get(&i).unwrap_or(&0)) >= t);
                json!({ "out": r.out, "n": r.n, "ins": r.ins, "frei": open, "ab": r.unlock.map(|(i, t)| json!([i, t])) })
            })
            .collect();
        let c: Vec<Value> = coll.iter().map(|(i, n)| json!({ "item": i, "n": n, "stufe": sb::coll_tier(*n), "naechste": sb::COLL_TIERS.get(sb::coll_tier(*n) as usize) })).collect();
        Ok(json!({ "rezepte": v, "sammlungen": c }))
    }

    // ---------- Bazaar ----------
    /// Ein Handel: Käufer bekommt Ware, Verkäufer Taler abzüglich 1,5 % (vernichtet).
    fn fill(&mut self, buyer: u32, seller: u32, item: u16, q: u32, price: i64) -> R<()> {
        let gross = price * q as i64;
        let tax = sb::bazaar_tax(gross);
        self.give(buyer, item, q as i64)?;
        self.pay(seller, gross - tax, &format!("bazaar:verkauf:{item}"))?;
        self.db.execute("insert into ledger (user, delta, reason, ts) values (0, ?, 'bazaar:gebuehr', ?)", params![-tax, self.now()])?;
        for u in [buyer, seller] {
            self.add_xp(u, sb::SK_HANDEL, (gross / 10).max(1) as u64)?;
        }
        Ok(())
    }
    /// Auftrag (`instant`: sofort zum besten Preis, Rest verfällt). Kauf hinterlegt Taler, Verkauf hinterlegt Ware.
    pub fn order(&mut self, uid: u32, item: u16, buy: bool, price: i64, qty: u32, instant: bool) -> R<Value> {
        if !sb::bazaar_item(item) {
            return no(400, "Das wird im Bazaar nicht gehandelt.");
        }
        if !(1..=71_680).contains(&qty) || (!instant && !(1..=1_000_000).contains(&price)) {
            return no(400, "Menge oder Preis ungültig.");
        }
        let r = self.tx(|g| {
            let book: Vec<(i64, u32, i64, u32)> = g
                .db
                .prepare(if buy { "select id, user, price, open from orders where item = ? and buy = 0 and user != ? order by price asc, ts asc, id asc" } else { "select id, user, price, open from orders where item = ? and buy = 1 and user != ? order by price desc, ts asc, id asc" })?
                .query_map(params![item, uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<_, _>>()?;
            let mut left = qty;
            let mut spent = 0i64;
            if buy && !instant {
                g.pay(uid, -price * qty as i64, "bazaar:hinterlegt")?;
            }
            if !buy {
                g.give(uid, item, -(qty as i64)).map_err(|_| E(402, "So viel hast du nicht.".into()))?;
            }
            for (oid, other, p, open) in book {
                if left == 0 || (!instant && if buy { p > price } else { p < price }) {
                    break;
                }
                let q = left.min(open);
                if buy {
                    if instant {
                        g.pay(uid, -p * q as i64, "bazaar:kauf")?;
                    } else if price > p {
                        g.pay(uid, (price - p) * q as i64, "bazaar:rest")?; // zu viel hinterlegt
                    }
                    g.fill(uid, other, item, q, p)?;
                } else {
                    g.fill(other, uid, item, q, p)?;
                }
                spent += p * q as i64;
                g.db.execute("update orders set open = open - ? where id = ?", params![q, oid])?;
                left -= q;
            }
            g.db.execute("delete from orders where open = 0", [])?;
            if left > 0 {
                if instant {
                    if !buy {
                        g.give(uid, item, left as i64)?; // nicht verkauft: Ware zurück
                    }
                } else {
                    g.db.execute("insert into orders (user, item, buy, price, qty, open, ts) values (?, ?, ?, ?, ?, ?, ?)", params![uid, item, buy, price, qty, left, g.now()])?;
                }
            }
            if instant && left == qty {
                return no(409, "Gerade gibt es dafür keine Angebote.");
            }
            Ok(json!({ "ok": true, "gehandelt": qty - left, "offen": if instant { 0 } else { left }, "taler": spent }))
        })?;
        if r["gehandelt"].as_u64().unwrap_or(0) > 0 {
            self.quest_hit(uid, |g| (g == Goal::Bazaar).then_some(1));
        }
        Ok(r)
    }
    pub fn order_cancel(&mut self, uid: u32, oid: i64) -> R<Value> {
        self.tx(|g| {
            let (item, buy, price, open): (u16, bool, i64, u32) = g
                .db
                .query_row("select item, buy, price, open from orders where id = ? and user = ?", params![oid, uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .optional()?
                .ok_or(E(404, "Diesen Auftrag gibt es nicht.".into()))?;
            g.db.execute("delete from orders where id = ?", [oid])?;
            if buy {
                g.pay(uid, price * open as i64, "bazaar:storno")?;
            } else {
                g.give(uid, item, open as i64)?;
            }
            Ok(json!({ "ok": true }))
        })
    }
    pub fn bazaar(&self, uid: u32) -> R<Value> {
        let mut prod = Vec::new();
        let items: Vec<u16> = (0..256u16).chain(sb::all_items().map(|(i, _)| i)).filter(|&i| sb::bazaar_item(i)).collect();
        for item in items {
            let best = |buy: bool| -> Option<(i64, i64)> {
                self.db
                    .query_row(
                        if buy { "select price, sum(open) from orders where item = ? and buy = 1 group by price order by price desc limit 1" } else { "select price, sum(open) from orders where item = ? and buy = 0 group by price order by price asc limit 1" },
                        [item],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .ok()
                    .flatten()
            };
            prod.push(json!({ "item": item, "name": campus_shared::items::item_name(item), "kauf": best(false), "verkauf": best(true) }));
        }
        let mine: Vec<Value> = self
            .db
            .prepare("select id, item, buy, price, qty, open from orders where user = ? order by ts desc")?
            .query_map([uid], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "item": r.get::<_, u16>(1)?, "kauf": r.get::<_, bool>(2)?, "preis": r.get::<_, i64>(3)?, "menge": r.get::<_, u32>(4)?, "offen": r.get::<_, u32>(5)? })))?
            .collect::<Result<_, _>>()?;
        Ok(json!({ "waren": prod, "meine": mine, "steuer_bp": sb::BAZAAR_TAX_BP }))
    }

    // ---------- Bank ----------
    pub fn bank_settle(&mut self, uid: u32) -> R<i64> {
        self.sky_row(uid)?;
        let (bank, at, study): (i64, i64, i64) = self.db.query_row("select bank, bank_at, last_study from sky where user = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let (z, next) = sb::bank_interest(bank, at, self.now(), study);
        if z > 0 || next != at {
            self.db.execute("update sky set bank = bank + ?, bank_at = ? where user = ?", params![z, next, uid])?;
        }
        if z > 0 {
            self.db.execute("insert into ledger (user, delta, reason, ts) values (?, ?, 'bank:zins', ?)", params![uid, z, self.now()])?;
        }
        Ok(bank + z)
    }
    pub fn bank_move(&mut self, uid: u32, amount: i64) -> R<Value> {
        if amount == 0 {
            return no(400, "Betrag fehlt.");
        }
        self.bank_settle(uid)?;
        self.tx(|g| {
            g.pay(uid, -amount, if amount > 0 { "bank:einzahlung" } else { "bank:abhebung" })?;
            if g.db.execute("update sky set bank = bank + ?1 where user = ?2 and bank + ?1 >= 0", params![amount, uid])? != 1 {
                return no(402, "So viel liegt nicht auf der Bank.");
            }
            Ok(())
        })?;
        if amount > 0 {
            self.quest_hit(uid, |g| matches!(g, Goal::Deposit(_)).then_some(amount as u32));
        }
        Ok(json!({ "ok": true, "bank": self.sky_i64(uid, "bank")?, "taler": self.taler(uid) }))
    }

    // ---------- Truhen ----------
    /// Insel (Grundstücks-Nummer) der Truhe an dieser Stelle, wenn `uid` sie benutzen darf. Alle TRUHE-Blöcke einer Insel teilen eine Truhe.
    fn chest_plot(&self, uid: u32, x: i32, y: i32, z: i32) -> R<u32> {
        if self.block(x, y, z) != world::TRUHE {
            return no(404, "Hier steht keine Truhe.");
        }
        let grp = match self.players.get(&uid) {
            Some(p) => p.group,
            None => self.db.query_row("select grp from users where id = ?", [uid], |r| r.get(0)).optional()?.unwrap_or(0),
        };
        match world::slot_at(x, z).and_then(|s| self.plots.iter().find(|p| p.slot == s)) {
            Some(p) if crate::game::plot_may(p, uid, grp) => Ok(p.id),
            _ => no(403, "Diese Truhe gehört nicht dir."),
        }
    }
    fn chest_slots(&self, plot: u32) -> R<Value> {
        let v: Vec<[i64; 2]> = self.db.prepare_cached("select item, n from chest where plot = ? order by item")?.query_map([plot], |r| Ok([r.get(0)?, r.get(1)?]))?.collect::<Result<_, _>>()?;
        Ok(json!(v))
    }
    /// [E] auf TRUHE (Fenster öffnen) oder SCHATZ (einmal am Tag je Person und Truhe eine Belohnung).
    pub fn chest_open(&mut self, id: u32, x: i32, y: i32, z: i32) -> R<()> {
        let p = self.players.get(&id).map(|p| p.p).ok_or(E(401, "Nicht verbunden.".into()))?;
        let d = (x as f32 + 0.5 - p[0]).powi(2) + (y as f32 + 0.5 - p[1] - 1.6).powi(2) + (z as f32 + 0.5 - p[2]).powi(2);
        if d > campus_shared::economy::REACH * campus_shared::economy::REACH {
            return no(403, "Zu weit weg.");
        }
        match self.block(x, y, z) {
            world::TRUHE | world::GROSSE_TRUHE => self.chest_open_new(id, x, y, z),
            world::SCHATZ => {
                let (pos, day) = ((x as i64) << 32 | (y as i64) << 16 | z as i64, self.today());
                let roll = self.rng.next() as u32;
                let (taler, item, n) = sb::treasure(roll);
                let fresh = self.tx(|g| {
                    let k = g.db.execute("insert into schatz (user, pos, day) values (?1, ?2, ?3) on conflict (user, pos) do update set day = ?3 where day < ?3", params![id, pos, day])?;
                    if k == 1 {
                        g.pay(id, taler, "schatz")?;
                        g.gain(id, item, n)?;
                    }
                    Ok(k == 1)
                })?;
                if !fresh {
                    return no(409, "Diese Schatztruhe hast du heute schon geöffnet.");
                }
                let left = self.inv(id, item);
                self.send(id, &S2C::Inv { item, n: left });
                self.hud(id, json!({ "t": "toast", "art": "gut", "text": format!("Schatz! +{taler} Taler, +{n} {}", campus_shared::items::item_name(item)) }));
                Ok(())
            }
            b => self.use_block(id, x, y, z, b), // Altar, Prüfungs-Altar, Ständer, Schild (island.rs)
        }
    }
    /// Ein- (n > 0) oder auslagern (n < 0) über das Truhen-Fenster.
    pub fn chest_move(&mut self, uid: u32, x: i32, y: i32, z: i32, item: u16, n: i64) -> R<Value> {
        let plot = self.chest_plot(uid, x, y, z)?;
        if n == 0 {
            return Ok(json!({ "slots": self.chest_slots(plot)?, "max": CHEST_MAX }));
        }
        self.tx(|g| {
            let have: i64 = g.db.query_row("select n from chest where plot = ? and item = ?", params![plot, item], |r| r.get(0)).optional()?.unwrap_or(0);
            if n > 0 {
                let kinds: i64 = g.db.query_row("select count(*) from chest where plot = ?", [plot], |r| r.get(0))?;
                if have == 0 && kinds >= CHEST_MAX as i64 {
                    return no(409, "Die Truhe ist voll.");
                }
                g.give(uid, item, -n).map_err(|_| E(402, "So viel hast du nicht.".into()))?;
            } else {
                if have + n < 0 {
                    return no(402, "So viel liegt nicht in der Truhe.");
                }
                g.give(uid, item, -n)?;
            }
            if have + n == 0 {
                g.db.execute("delete from chest where plot = ? and item = ?", params![plot, item])?;
            } else {
                g.db.execute("insert into chest (plot, item, n) values (?1, ?2, ?3) on conflict (plot, item) do update set n = ?3", params![plot, item, have + n])?;
            }
            Ok(())
        })?;
        let left = self.inv(uid, item);
        self.send(uid, &S2C::Inv { item, n: left });
        Ok(json!({ "slots": self.chest_slots(plot)?, "max": CHEST_MAX }))
    }

    // ---------- Fokus-Mana und Werkzeuge ----------
    pub fn mana(&mut self, uid: u32, max: u32) -> R<u32> {
        let (m, at): (u32, i64) = self.db.query_row("select mana, mana_at from sky where user = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(sb::mana_now(m, at, self.now(), max))
    }
    pub fn spend_mana(&mut self, uid: u32, cost: u32) -> R<u32> {
        self.sky_row(uid)?;
        let st = self.stats(uid);
        let max = st.max_mana as u32;
        let cost = campus_shared::stats::mana_cost(cost, &st);
        let m = self.mana(uid, max)?;
        if m < cost {
            return no(402, &format!("Nicht genug Fokus-Mana ({m}/{cost})."));
        }
        self.db.execute("update sky set mana = ?, mana_at = ? where user = ?", params![m - cost, self.now(), uid])?;
        Ok(m - cost)
    }
    /// Werkzeug-Fähigkeit. `dir` = Blickrichtung; Ziel und Wirkung bestimmt allein der Server.
    pub fn ability(&mut self, id: u32, tool: u16, dir: [f32; 3]) -> R<()> {
        let t = sb::tool(tool).ok_or(E(400, "Unbekanntes Werkzeug.".into()))?;
        if tool == campus_shared::rpg::ERNTE_HACKE || tool == campus_shared::rpg::SEGEN_HACKE {
            return self.till(id, tool, dir); // Ackerboden anlegen (island.rs)
        }
        let now = self.now();
        let Some(pl) = self.players.get(&id) else { return no(401, "Nicht verbunden.") };
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !len.is_finite() || len < 0.5 {
            return no(400, "Richtung fehlt.");
        }
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let ti = sb::TOOLS.iter().position(|x| x.id == tool).unwrap_or(0);
        let (pos, grp, last) = (pl.p, pl.group, pl.tool_at[ti]);
        if self.inv(id, tool) == 0 {
            return no(403, "Dieses Werkzeug hast du nicht.");
        }
        if now - last < t.cooldown_ms {
            return no(429, "Noch nicht wieder bereit.");
        }
        let eye = [pos[0], pos[1] + 1.62, pos[2]];
        match tool {
            t if sb::wand_limit(t) > 0 => {
                let (hit, prev) = world::raycast(|x, y, z| self.block(x, y, z), eye, d, 7.0).ok_or(E(400, "Zielt auf keinen Block.".into()))?;
                let n = [prev[0] - hit[0], prev[1] - hit[1], prev[2] - hit[2]];
                let b = self.block(hit[0], hit[1], hit[2]);
                if b == 0 || world::FLAGS[b as usize] & (world::F_REGEN | world::F_LIQUID) != 0 {
                    return no(400, "Mit diesem Block geht das nicht.");
                }
                // Fläche gleicher Blöcke in der Ebene senkrecht zu n, deren Vorderseite frei ist
                let have = self.inv(id, b as u16) as usize;
                let limit = sb::wand_limit(t).min(have);
                if limit == 0 {
                    return no(402, "Davon hast du keine Blöcke mehr.");
                }
                let mut seen = vec![hit];
                let mut out: Vec<[i32; 3]> = Vec::new();
                let mut k = 0;
                while k < seen.len() && out.len() < limit {
                    let c = seen[k];
                    k += 1;
                    let f = [c[0] + n[0], c[1] + n[1], c[2] + n[2]];
                    if self.block(c[0], c[1], c[2]) != b || self.block(f[0], f[1], f[2]) != 0 || !(1..CH - 1).contains(&f[1]) || !self.can_build(id, grp, f[0], f[2]) {
                        continue;
                    }
                    if (f[0] as f32 + 0.5 - pos[0]).abs() < 0.9 && (f[2] as f32 + 0.5 - pos[2]).abs() < 0.9 && (f[1] as f32 - pos[1]) > -1.0 && (f[1] as f32 - pos[1]) < 2.0 {
                        continue; // nicht in den Spieler hinein
                    }
                    out.push(f);
                    for a in 0..3 {
                        if n[a] != 0 {
                            continue;
                        }
                        for s in [-1, 1] {
                            let mut q = c;
                            q[a] += s;
                            if !seen.contains(&q) && seen.len() < 512 {
                                seen.push(q);
                            }
                        }
                    }
                }
                if out.is_empty() {
                    return no(409, "Hier ist kein Platz zum Verlängern.");
                }
                self.spend_mana(id, t_mana(tool))?;
                let n_out = out.len();
                let left = self.give(id, b as u16, -(n_out as i64))?;
                for f in out {
                    self.set_block(f[0], f[1], f[2], b, id);
                }
                self.send(id, &S2C::Inv { item: b as u16, n: left });
                self.add_xp(id, sb::SK_BAU, n_out as u64)?;
            }
            sb::HAMMER => {
                let (hit, prev) = world::raycast(|x, y, z| self.block(x, y, z), eye, d, 7.0).ok_or(E(400, "Zielt auf keinen Block.".into()))?;
                // gleiche Regeln wie beim Abbauen von Hand (world::edit_allowed, keine unendliche Härte), 3×3 auf der Trefferfläche
                let n = [prev[0] - hit[0], prev[1] - hit[1], prev[2] - hit[2]];
                let (ua, va) = match (n[0] != 0, n[1] != 0) { (true, _) => (1, 2), (_, true) => (0, 2), _ => (0, 1) };
                let mut cells = Vec::new();
                for du in -1..=1 {
                    for dv in -1..=1 {
                        let mut c = hit;
                        c[ua] += du;
                        c[va] += dv;
                        let cur = self.block(c[0], c[1], c[2]);
                        if world::hardness(cur).is_finite() && (1..CH - 1).contains(&c[1]) && self.may_edit(id, grp, c[0], c[2], cur, false) {
                            cells.push((c, cur));
                        }
                    }
                }
                if cells.is_empty() {
                    return no(403, "Hier wirkt der Hammer nicht.");
                }
                self.spend_mana(id, t_mana(tool))?;
                for (c, cur) in cells {
                    self.take_block(id, c[0], c[1], c[2], cur)?;
                }
            }
            sb::HAKEN => {
                // Flug macht der Browser; der Server erlaubt dafür kurz höheres Tempo
                if let Some(p) = self.players.get_mut(&id) {
                    p.boost_until = now + 2500;
                }
            }
            sb::SPRUNGSTAB => {
                let free = |g: &Game, p: [f32; 3]| {
                    let (x, y, z) = (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
                    (0..WORLD).contains(&x) && (0..WORLD).contains(&z) && !world::solid(g.block(x, y, z)) && !world::solid(g.block(x, y + 1, z))
                };
                let mut to = pos;
                let mut s = 0.25;
                while s <= sb::BLINK {
                    let q = [pos[0] + d[0] * s, pos[1] + d[1] * s, pos[2] + d[2] * s];
                    if !free(self, q) {
                        break;
                    }
                    to = q;
                    s += 0.25;
                }
                self.spend_mana(id, t_mana(tool))?;
                self.teleport(id, to);
            }
            _ => return no(400, "Dieses Werkzeug hat keine Fähigkeit."),
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.tool_at[ti] = now;
        }
        self.quest_hit(id, |g| (g == Goal::Ability).then_some(1));
        self.quest_hit(id, |g| (g == Goal::UseTool(tool)).then_some(1));
        let max = self.stats(id).max_mana as u32;
        let m = self.mana(id, max)?;
        self.hud(id, json!({ "t": "mana", "mana": m, "max": max }));
        Ok(())
    }

    // ---------- HTTP ----------
    pub fn sky_get(&mut self, uid: u32, op: &str) -> Option<R<Value>> {
        if let Some(r) = self.rpg_get(uid, op) {
            return Some(r);
        }
        Some(match op {
            "sky" => self.sky_me(uid),
            "bazaar" => self.bazaar(uid),
            "recipes" => self.recipes(uid),
            o if o.starts_with("minions/") => match o[8..].parse() {
                Ok(p) => self.minions_on(p),
                Err(_) => no(400, "Insel fehlt."),
            },
            _ => return None,
        })
    }
    pub fn sky_post(&mut self, uid: u32, admin: bool, op: &str, b: &Value) -> Option<R<Value>> {
        let flag = |k: &str| b.get(k).and_then(Value::as_bool).unwrap_or(false);
        let item = |k: &str| int(b, k).and_then(|v| u16::try_from(v).map_err(|_| E(400, "Gegenstand ungültig.".into())));
        if let Some(r) = self.rpg_post(uid, op, b) {
            return Some(r);
        }
        Some(match op {
            "craft" => item("item").and_then(|i| self.craft(uid, i, int(b, "mal").unwrap_or(1) as u32)),
            "minion/place" => item("item").and_then(|i| self.minion_place(uid, i, int(b, "x")? as i32, int(b, "y")? as i32, int(b, "z")? as i32)),
            "minion/collect" => int(b, "id").and_then(|m| self.minion_collect(uid, m, flag("abholen"))),
            "bazaar/order" => item("item").and_then(|i| self.order(uid, i, flag("kauf"), int(b, "preis").unwrap_or(0), int(b, "menge")? as u32, flag("sofort"))),
            "bazaar/cancel" => int(b, "id").and_then(|o| self.order_cancel(uid, o)),
            "bank/deposit" => int(b, "taler").and_then(|t| self.bank_move(uid, t)),
            "bank/withdraw" => int(b, "taler").and_then(|t| self.bank_move(uid, -t)),
            "truhe" => self.truhe_post(uid, b),
            "fuel/use" => int(b, "n").and_then(|n| {
                let n = n.clamp(1, 12);
                self.tx(|g| {
                    g.give(uid, sb::ENERGIE, -n).map_err(|_| E(402, "Nicht genug Lern-Energie-Zellen.".into()))?;
                    g.fuel(uid, n * sb::ENERGIE_H * sb::HOUR_MS)
                })?;
                self.quest_hit(uid, |g| (g == Goal::Fuel).then_some(1));
                Ok(json!({ "ok": true }))
            }),
            "admin/mute" if admin => int(b, "user").and_then(|u| self.admin_mute(u as u32, int(b, "minuten").unwrap_or(0))),
            _ => return None,
        })
    }
}

fn t_mana(tool: u16) -> u32 {
    sb::tool(tool).map_or(0, |t| t.mana)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_saeubern() {
        assert_eq!(clean("  hallo <b>du</b>  "), "hallo ‹b›du‹/b›");
        assert_eq!(clean("Du Arschloch!"), "Du *********!");
        assert_eq!(clean(&"x".repeat(500)).len(), CHAT_MAX);
        assert_eq!(clean("a\u{0007}b\nc"), "abc");
        assert_eq!(clean("Grüße, Fuck"), "Grüße, ****");
    }
}
