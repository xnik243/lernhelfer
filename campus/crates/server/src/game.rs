//! Spielzustand im Speicher: Welt, Spieler, Takt mit räumlichem Raster, Block-Regeln.
//! Alles liegt hinter EINER Sperre (lib.rs), die nie über ein await gehalten wird.
// ponytail: eine Sperre für Welt und Datenbank; bei deutlich über 50 Spielern in Welt-Sperre und DB-Thread trennen.
use crate::{no, E, R};
use axum::body::Bytes;
use campus_shared::economy::{self as eco, Rng};
use campus_shared::items::Look;
use campus_shared::proto::{self, PState, PlotInfo, Warp, C2S, S2C};
use campus_shared::skyblock::{self as sb, Goal};
use campus_shared::world::{self, SlotKind, WorldState, CH, WC, WORLD};
use serde_json::json;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

pub struct Chunk {
    /// leer = reine Luft (spart Speicher), sonst VOL Blöcke
    pub b: Vec<u8>,
    pub dirty: bool,
    msg: Option<Bytes>,
}

#[derive(Clone, Debug)]
pub struct Plot {
    pub id: u32,
    pub slot: (u8, u8),
    pub size: u8,
    pub owner: u32,
    pub group: u32,
    pub acl: Vec<u32>,
    pub name: String,
    pub visit: u8,
    pub owner_grp: u32,
}

pub struct Player {
    pub id: u32,
    pub name: String,
    pub look: Look,
    pub p: [f32; 3],
    pub yaw: f32,
    pub anim: u8,
    pub group: u32,
    tx: Sender<Bytes>,
    sent: Vec<u64>,
    pub session: u64,
    last_block: i64,
    pub(crate) last_chat: i64,
    last_move: i64,
    view: i32,
    pub(crate) tool_at: [i64; 16], // je Werkzeug (Index in skyblock::TOOLS)
    pub boost_until: i64,
    /// Abbaubeginn (C2S::Dig): Stelle und Zeit
    dig: Option<([i32; 3], i64)>,
    /// letzter Sturz ins Leere
    void_at: i64,
    // ---------- Phasen 16–20 ----------
    /// gewählter Schnellleisten-Platz 0..9 (C2S::Held)
    pub held: u8,
    /// Lebenspunkte (nur im Speicher; nach dem Neustart voll)
    pub hp: f32,
    pub shield_until: i64,
    pub swing_at: i64,
    pub shot_at: i64,
    pub hurt_at: i64,
    /// letzter Teleporter-/Sprungplatten-Auslöser (gegen Ping-Pong)
    pub pad_at: i64,
    /// Platz der Privatinsel, deren Inhalt (S2C::Island) zuletzt geschickt wurde
    pub island_sent: Option<(u8, u8)>,
    /// zuletzt gerechnete Werte (rpg::Game::refresh_stats)
    pub stats: campus_shared::stats::Stats,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub ticks: u64,
    pub last_us: u64,
    pub max_us: u64,
    pub avg_us: f64,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: u32,
    pub name: String,
    pub admin: bool,
}

pub struct Game {
    pub db: Connection,
    pub st: WorldState,
    pub chunks: Vec<Chunk>,
    pub players: HashMap<u32, Player>,
    pub plots: Vec<Plot>,
    pub rng: Rng,
    pub clock_offset: i64,
    pub stats: Stats,
    session: u64,
    grid: HashMap<(i32, i32), Vec<u32>>,
    states: Vec<PState>,
    ids: Vec<u32>,
    scratch: Vec<u8>,
    ring: Vec<(i32, i32)>,
    kick: Vec<u32>,
    pub report_at: HashMap<u32, i64>,
    /// beim Vorrechnen (Zahltag-Hinweis) keine Meldungen verschicken
    pub quiet: bool,
    /// abgebaute nachwachsende Blöcke: (fällig, x, y, z, Block)
    // ponytail: lineare Liste, reicht für einige hundert gleichzeitig; bei viel mehr nach Fälligkeit sortieren
    pub regen: Vec<(i64, i32, i32, i32, u8)>,
    /// Prüfungs-Zitadelle: Wesen, Boss, Schadenslisten (mobs.rs)
    pub mobs: crate::mobs::Mobs,
    /// laufende Handelsfenster (inv.rs)
    pub trades: crate::inv::Trades,
    /// Konten, deren Beutel (/stash) sich gerade gefüllt hat; give() ist &self, darum hier gesammelt, im Takt gemeldet
    pub stash_notes: std::cell::RefCell<Vec<u32>>,
}

const SCHEMA: &str = "
pragma journal_mode = WAL; pragma synchronous = NORMAL; pragma temp_store = MEMORY;
create table if not exists users (id integer primary key, name text not null, taler integer not null default 0 check (taler >= 0),
  funde integer not null default 0 check (funde >= 0), look blob, last_ev integer not null default 0, level integer not null default 1,
  streak integer not null default 0, best_streak integer not null default 0, banned integer not null default 0, grp integer not null default 0,
  created integer not null, x real, y real, z real);
create table if not exists inv (user integer not null, item integer not null, n integer not null check (n >= 0), primary key (user, item)) without rowid;
create table if not exists timed (user integer not null, item integer not null, until integer not null, primary key (user, item)) without rowid;
create table if not exists daily (user integer not null, day text not null, focus integer not null default 0, cards integer not null default 0, primary key (user, day)) without rowid;
create table if not exists ledger (id integer primary key, user integer not null, delta integer not null, reason text not null, ts integer not null);
create table if not exists stats (user integer not null, key text not null, n integer not null, primary key (user, key)) without rowid;
create table if not exists earn_week (user integer not null, week integer not null, taler integer not null, primary key (user, week)) without rowid;
create table if not exists chunks (cx integer not null, cz integer not null, data blob not null, primary key (cx, cz)) without rowid;
create table if not exists plots (id integer primary key, si integer not null, sj integer not null, size integer not null, owner integer not null, grp integer not null, unique (si, sj));
create table if not exists plot_acl (plot integer not null, user integer not null, primary key (plot, user)) without rowid;
create table if not exists auctions (id integer primary key, seller integer not null, item integer not null, qty integer not null, start integer not null,
  buyout integer, bid integer, bidder integer, ends integer not null, state integer not null default 0);
create table if not exists transfers (id integer primary key, from_u integer not null, to_u integer not null, amount integer not null, ts integer not null);
create table if not exists flags (id integer primary key, kind text not null, a integer not null, b integer not null, detail text not null, ts integer not null, done integer not null default 0);
create table if not exists reports (id integer primary key, reporter integer not null, user integer not null, plot integer not null, reason text not null, ts integer not null, done integer not null default 0);
create table if not exists groups (id integer primary key, name text not null unique collate nocase, treasury integer not null default 0 check (treasury >= 0), owner integer not null);
create table if not exists projects (key text primary key, funded integer not null default 0, done integer not null default 0);
create table if not exists project_gifts (key text not null, user integer not null, amount integer not null, primary key (key, user)) without rowid;
create table if not exists kv (k text primary key, v text not null);
";
/// Phasen 16–20: alle neuen Tabellen in einem Block (idempotent, WAL wie oben). Besitz: inv.rs (inv_layout, player_stash,
/// player_equipment, island_chests, trade_log, Ständer in island_functional_blocks.data), island.rs (item_ench, island_crops,
/// island_functional_blocks, coop_members, island_likes, questbook).
const RPG_SCHEMA: &str = "
create table if not exists inv_layout (user integer primary key, data blob not null);
create table if not exists player_stash (user integer not null, item integer not null, n integer not null check (n > 0), primary key (user, item)) without rowid;
create table if not exists player_equipment (user integer not null, slot integer not null, item integer not null, primary key (user, slot)) without rowid;
create table if not exists item_ench (user integer not null, item integer not null, ench integer not null, level integer not null, primary key (user, item, ench)) without rowid;
create table if not exists island_chests (x integer not null, y integer not null, z integer not null, plot integer not null, size integer not null, data blob not null, primary key (x, y, z)) without rowid;
create table if not exists island_crops (x integer not null, y integer not null, z integer not null, plot integer not null, kind integer not null, planted integer not null, fert integer not null default 1000, primary key (x, y, z)) without rowid;
create table if not exists island_functional_blocks (x integer not null, y integer not null, z integer not null, plot integer not null, kind integer not null, data text not null default '', primary key (x, y, z)) without rowid;
create table if not exists coop_members (plot integer not null, user integer not null, since integer not null, primary key (plot, user)) without rowid;
create table if not exists island_likes (plot integer not null, user integer not null, stamp integer not null default 0, ts integer not null, primary key (plot, user)) without rowid;
create table if not exists questbook (user integer primary key, chapter integer not null default 1, step integer not null default 0, n integer not null default 0);
create table if not exists trade_log (id integer primary key, a integer not null, b integer not null, detail text not null, ts integer not null);
create index if not exists crops_plot on island_crops (plot);
create index if not exists fblocks_plot on island_functional_blocks (plot);
create index if not exists chests_plot on island_chests (plot);
";
/// Spätere Spalten; schlagen sie fehl, gibt es sie schon.
const ALTER: &[&str] = &["alter table plots add column visit integer not null default 0"];
/// Weltfassung: steigt sie, sind gespeicherte Stücke einer alten Grundwelt wertlos und werden verworfen.
const WORLD_VERSION: &str = "3-archipel";

fn ci(cx: i32, cz: i32) -> usize {
    (cz * WC + cx) as usize
}

impl Game {
    pub fn open(path: &str) -> R<Game> {
        let db = Connection::open(path)?;
        db.execute_batch(SCHEMA)?;
        db.execute_batch(crate::sky::SCHEMA)?;
        db.execute_batch(RPG_SCHEMA)?;
        for a in ALTER {
            let _ = db.execute(a, []);
        }
        let mut seed = [0u8; 8];
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            let _ = std::io::Read::read_exact(&mut f, &mut seed);
        }
        let mut ring: Vec<(i32, i32)> = (-eco::MAX_VIEW_CHUNKS..=eco::MAX_VIEW_CHUNKS).flat_map(|z| (-eco::MAX_VIEW_CHUNKS..=eco::MAX_VIEW_CHUNKS).map(move |x| (x, z))).filter(|(x, z)| x * x + z * z <= eco::MAX_VIEW_CHUNKS * eco::MAX_VIEW_CHUNKS).collect();
        ring.sort_by_key(|(x, z)| x * x + z * z);
        let mut g = Game {
            db,
            st: WorldState::default(),
            chunks: Vec::new(),
            players: HashMap::new(),
            plots: Vec::new(),
            rng: Rng(u64::from_le_bytes(seed) ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)),
            clock_offset: 0,
            stats: Stats::default(),
            session: 0,
            grid: HashMap::new(),
            states: Vec::with_capacity(64),
            ids: Vec::with_capacity(64),
            scratch: Vec::with_capacity(4096),
            ring,
            kick: Vec::new(),
            report_at: HashMap::new(),
            quiet: false,
            regen: Vec::new(),
            mobs: Default::default(),
            trades: Default::default(),
            stash_notes: Default::default(),
        };
        g.st = WorldState { regions: g.kv("regions").and_then(|v| v.parse().ok()).unwrap_or(0), projects: g.kv("projects").and_then(|v| v.parse().ok()).unwrap_or(0) };
        for cz in 0..WC {
            for cx in 0..WC {
                g.chunks.push(Chunk { b: world::gen_chunk(cx, cz, g.st), dirty: false, msg: None });
            }
        }
        if g.kv("welt").as_deref() != Some(WORLD_VERSION) {
            g.migrate()?;
        }
        let rows: Vec<(i32, i32, Vec<u8>)> = g.db.prepare("select cx, cz, data from chunks")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        for (cx, cz, data) in rows {
            if !(0..WC).contains(&cx) || !(0..WC).contains(&cz) {
                continue;
            }
            let c = &mut g.chunks[ci(cx, cz)];
            if c.b.is_empty() {
                c.b = vec![0; world::VOL];
            }
            // nachwachsende Blöcke, die beim Beenden noch erschöpft waren, sind sofort wieder da
            if world::unrle(&data, &mut c.b) && c.b.contains(&world::ABGEBAUT) {
                let base = world::gen_chunk(cx, cz, g.st);
                for (k, v) in c.b.iter_mut().enumerate() {
                    if *v == world::ABGEBAUT {
                        *v = base.get(k).copied().unwrap_or(0);
                    }
                }
            }
        }
        g.load_plots()?;
        Ok(g)
    }

    /// Umzug auf eine neue Grundwelt: alte Stücke und Standorte verwerfen, Grundstücke neu verteilen
    /// (Reihenfolge der Anlage), aufgestellte Minions samt Lager zurück ins Inventar.
    fn migrate(&mut self) -> R<()> {
        self.db.execute("delete from chunks", [])?;
        self.db.execute("update users set x = null, y = null, z = null", [])?;
        let rows: Vec<(u32, u32)> = self.db.prepare("select id, grp from plots order by id")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let mut pers = world::slots_of(SlotKind::Personal, self.st).into_iter();
        let mut grp = world::slots_of(SlotKind::Group, self.st).into_iter();
        self.db.execute("update plots set si = -1 - id, sj = -1", [])?; // unique (si, sj) frei machen
        for (id, g) in rows {
            let slot = if g != 0 { grp.next() } else { pers.next() };
            match slot {
                Some(s) => self.db.execute("update plots set si = ?, sj = ? where id = ?", params![s.0, s.1, id])?,
                None => self.db.execute("delete from plots where id = ?", [id])?,
            };
        }
        let ms: Vec<(i64, u32)> = self.db.prepare("select id, user from minions")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        for (mid, uid) in ms {
            self.minion_collect(uid, mid, true)?;
        }
        self.kv_set("welt", WORLD_VERSION)?;
        Ok(())
    }

    pub fn now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64) + self.clock_offset
    }
    pub fn today(&self) -> i64 {
        self.now().div_euclid(campus_shared::time::DAY_MS)
    }
    pub fn week(&self) -> u32 {
        campus_shared::time::iso_week(self.today())
    }
    pub fn kv(&self, k: &str) -> Option<String> {
        self.db.query_row("select v from kv where k = ?", [k], |r| r.get(0)).optional().ok().flatten()
    }
    pub fn kv_set(&self, k: &str, v: &str) -> R<()> {
        self.db.execute("insert into kv (k, v) values (?, ?) on conflict (k) do update set v = excluded.v", [k, v])?;
        Ok(())
    }

    pub fn load_plots(&mut self) -> R<()> {
        let mut plots: Vec<Plot> = self
            .db
            .prepare("select p.id, p.si, p.sj, p.size, p.owner, p.grp, coalesce(u.name, g.name, ''), p.visit, coalesce(u.grp, 0) from plots p left join users u on u.id = p.owner left join groups g on g.id = p.grp")?
            .query_map([], |r| Ok(Plot { id: r.get(0)?, slot: (r.get(1)?, r.get(2)?), size: r.get(3)?, owner: r.get(4)?, group: r.get(5)?, acl: Vec::new(), name: r.get(6)?, visit: r.get(7)?, owner_grp: r.get(8)? }))?
            .collect::<Result<_, _>>()?;
        for p in &mut plots {
            // Mitbauer (alt) und Co-op-Partner (Phase 20) haben dieselben Rechte
            p.acl = self.db.prepare("select user from plot_acl where plot = ?1 union select user from coop_members where plot = ?1")?.query_map([p.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        }
        self.plots = plots;
        Ok(())
    }

    // ---------- Senden ----------
    fn enc(&mut self, m: &S2C) -> Bytes {
        self.scratch.clear();
        let _ = bincode::serialize_into(&mut self.scratch, m);
        Bytes::copy_from_slice(&self.scratch)
    }
    /// Wichtige Nachricht; wer nicht hinterherkommt, fliegt raus und meldet sich neu an.
    fn push(&mut self, id: u32, b: Bytes) {
        if let Some(p) = self.players.get(&id) {
            if p.tx.try_send(b).is_err() {
                self.kick.push(id);
            }
        }
    }
    pub fn send(&mut self, id: u32, m: &S2C) {
        if self.players.contains_key(&id) {
            let b = self.enc(m);
            self.push(id, b);
        }
    }
    pub fn broadcast(&mut self, m: &S2C) {
        let b = self.enc(m);
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            self.push(id, b.clone());
        }
    }
    pub fn hud(&mut self, id: u32, v: serde_json::Value) {
        if self.quiet {
            return;
        }
        self.send(id, &S2C::Hud(v.to_string()));
    }
    pub fn hud_all(&mut self, v: serde_json::Value) {
        self.broadcast(&S2C::Hud(v.to_string()));
    }

    pub fn plot_infos(&self, uid: u32, group: u32) -> Vec<PlotInfo> {
        self.plots.iter().map(|p| PlotInfo { id: p.id, slot: p.slot, size: p.size, owner: p.owner, group: p.group, name: p.name.clone(), may: plot_may(p, uid, group) }).collect()
    }
    pub fn send_plots(&mut self) {
        let who: Vec<(u32, u32)> = self.players.values().map(|p| (p.id, p.group)).collect();
        for (id, grp) in who {
            let m = S2C::Plots(self.plot_infos(id, grp));
            self.send(id, &m);
        }
    }

    // ---------- Welt ----------
    pub fn block(&self, x: i32, y: i32, z: i32) -> u8 {
        if !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) || !(0..CH).contains(&y) {
            return 0;
        }
        let c = &self.chunks[ci(x >> 4, z >> 4)];
        if c.b.is_empty() {
            0
        } else {
            c.b[world::idx(x & 15, y, z & 15)]
        }
    }
    fn set_raw(&mut self, x: i32, y: i32, z: i32, b: u8) {
        let c = &mut self.chunks[ci(x >> 4, z >> 4)];
        if c.b.is_empty() {
            if b == 0 {
                return;
            }
            c.b = vec![0; world::VOL];
        }
        c.b[world::idx(x & 15, y, z & 15)] = b;
        c.dirty = true;
        c.msg = None;
    }
    /// Setzt einen Block und meldet ihn sofort allen, die das Stück geladen haben.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, b: u8, by: u32) {
        self.set_raw(x, y, z, b);
        let i = ci(x >> 4, z >> 4);
        let m = self.enc(&S2C::Block { x: x as i16, y: y as i16, z: z as i16, b, by });
        let ids: Vec<u32> = self.players.values().filter(|p| p.id == by || p.sent[i / 64] >> (i % 64) & 1 == 1).map(|p| p.id).collect();
        for id in ids {
            self.push(id, m.clone());
        }
    }
    /// Hat der Spieler das Stück an dieser Stelle schon bekommen?
    pub fn has_chunk(&self, id: u32, x: i32, z: i32) -> bool {
        let i = ci(x >> 4, z >> 4);
        self.players.get(&id).is_some_and(|p| p.sent[i / 64] >> (i % 64) & 1 == 1)
    }
    /// Stück neu an alle schicken, die es schon haben (nach Rücksetzen oder Weltwandel).
    fn resend(&mut self, i: usize) {
        self.chunks[i].msg = None;
        self.chunks[i].dirty = true;
        for p in self.players.values_mut() {
            p.sent[i / 64] &= !(1 << (i % 64));
        }
    }

    /// Baurecht (world::edit_allowed) mit den Rechten am Grundstück dieses Platzes.
    pub fn may_edit(&self, uid: u32, group: u32, x: i32, z: i32, cur: u8, place: bool) -> bool {
        let (may, in_plot) = match world::slot_at(x, z).and_then(|s| self.plots.iter().find(|p| p.slot == s)) {
            Some(p) => (plot_may(p, uid, group), world::in_rect(world::plot_rect(p.slot, p.size), x, z)),
            None => (false, false),
        };
        world::edit_allowed(x, z, cur, place, may, in_plot)
    }
    /// Darf hier gesetzt werden?
    pub fn can_build(&self, uid: u32, group: u32, x: i32, z: i32) -> bool {
        self.may_edit(uid, group, x, z, 0, true)
    }

    /// Weltwandel (Gebiet frei, Großprojekt fertig): nur die Stellen ändern, an denen sich die Grundwelt ändert.
    // ponytail: rechnet die Hub-Stücke zweimal unter der Sperre (einmalig, selten); Großprojekte liegen nur im Hub.
    pub fn apply_state(&mut self, new: WorldState) -> R<()> {
        let old = self.st;
        if old == new {
            return Ok(());
        }
        let n = world::HUB_EDGE / 16;
        for cz in 0..n {
            for cx in 0..n {
                let (a, b) = (world::gen_chunk(cx, cz, old), world::gen_chunk(cx, cz, new));
                if a == b {
                    continue;
                }
                let i = ci(cx, cz);
                let c = &mut self.chunks[i].b;
                if c.is_empty() {
                    *c = vec![0; world::VOL];
                }
                for k in 0..world::VOL {
                    let (va, vb) = (a.get(k).copied().unwrap_or(0), b.get(k).copied().unwrap_or(0));
                    if va != vb {
                        c[k] = vb;
                    }
                }
                self.resend(i);
            }
        }
        self.st = new;
        self.kv_set("regions", &new.regions.to_string())?;
        self.kv_set("projects", &new.projects.to_string())?;
        self.broadcast(&S2C::World(new));
        Ok(())
    }

    /// Setzt die Fläche eines Grundstücks auf die Grundwelt zurück.
    pub fn reset_rect(&mut self, r: (i32, i32, i32)) {
        let mut cache: HashMap<(i32, i32), Vec<u8>> = HashMap::new();
        for z in r.1..r.1 + r.2 {
            for x in r.0..r.0 + r.2 {
                let (cx, cz) = (x >> 4, z >> 4);
                let st = self.st;
                let g = cache.entry((cx, cz)).or_insert_with(|| world::gen_chunk(cx, cz, st));
                let c = &mut self.chunks[ci(cx, cz)].b;
                if c.is_empty() {
                    if g.is_empty() {
                        continue;
                    }
                    *c = vec![0; world::VOL];
                }
                for y in 0..CH {
                    let k = world::idx(x & 15, y, z & 15);
                    c[k] = g.get(k).copied().unwrap_or(0);
                }
            }
        }
        for (cx, cz) in cache.into_keys() {
            self.resend(ci(cx, cz));
        }
    }

    /// Geänderte Stücke in einer einzigen Transaktion sichern.
    pub fn save(&mut self) -> R<usize> {
        let tx = self.db.unchecked_transaction()?;
        let mut n = 0;
        {
            let mut q = tx.prepare_cached("insert into chunks (cx, cz, data) values (?, ?, ?) on conflict (cx, cz) do update set data = excluded.data")?;
            for cz in 0..WC {
                for cx in 0..WC {
                    let c = &mut self.chunks[ci(cx, cz)];
                    if c.dirty && !c.b.is_empty() {
                        q.execute(params![cx, cz, world::rle(&c.b)])?;
                        c.dirty = false;
                        n += 1;
                    }
                }
            }
            let mut q = tx.prepare_cached("update users set x = ?, y = ?, z = ? where id = ?")?;
            for p in self.players.values() {
                q.execute(params![p.p[0], p.p[1], p.p[2], p.id])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    // ---------- Spieler ----------
    pub fn join(&mut self, u: &User, tx: Sender<Bytes>) -> R<u64> {
        self.ensure_user(u)?;
        self.assign_plot(u.id)?;
        let (look, grp, pos): (Option<Vec<u8>>, u32, Option<[f32; 3]>) = self.db.query_row("select look, grp, x, y, z from users where id = ?", [u.id], |r| {
            let x: Option<f32> = r.get(2)?;
            Ok((r.get(0)?, r.get(1)?, x.map(|x| [x, r.get(3).unwrap_or(0.0), r.get(4).unwrap_or(0.0)])))
        })?;
        let look: Look = look.and_then(|b| proto::dec(&b)).unwrap_or_default();
        let saved = pos.filter(|p| p[1] > 0.0);
        let pos = saved.unwrap_or(world::SPOTS[0].1);
        self.session += 1;
        let session = self.session;
        let old: Vec<(u32, String, Look)> = self.players.values().filter(|p| p.id != u.id).map(|p| (p.id, p.name.clone(), p.look)).collect();
        let sent = vec![0u64; ((WC * WC + 63) / 64) as usize];
        self.players.insert(u.id, Player { id: u.id, name: u.name.clone(), look, p: pos, yaw: 0.0, anim: 0, group: grp, tx, sent, session, last_block: 0, last_chat: 0, last_move: self.now(), view: eco::VIEW_CHUNKS, tool_at: [0; 16], boost_until: 0, dig: None, void_at: 0,
            held: 0, hp: campus_shared::stats::BASE_HP, shield_until: 0, swing_at: 0, shot_at: 0, hurt_at: 0, pad_at: 0, island_sent: None, stats: Default::default() });
        self.sky_row(u.id)?;
        self.send(u.id, &S2C::Welcome { id: u.id, pos, state: self.st });
        // gleich danach immer die Ankunft: gespeicherte Stelle, wenn Füße und Kopf frei sind und höchstens 2 darunter Boden ist;
        // sonst sicherer Boden an der eigenen Insel bzw. am Hub-Spawn (nicht die gespeicherte Spalte: dort läge das Dach)
        let free = |p: [f32; 3]| {
            let (x, y, z) = (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
            !world::solid(self.block(x, y, z)) && !world::solid(self.block(x, y + 1, z)) && (1..=2).any(|k| world::solid(self.block(x, y - k, z)))
        };
        match saved {
            Some(p) if free(p) => self.send(u.id, &S2C::Spawn { p, yaw: 0.0 }),
            Some(_) => {
                let (to, yaw) = self.plots.iter().find(|p| p.owner == u.id).map_or((world::SPOTS[0].1, world::SPOTS[0].2), |p| (world::plot_spawn(p.slot), world::PLOT_YAW));
                self.spawn_at(u.id, to, yaw)
            }
            None => self.spawn_at(u.id, world::SPOTS[0].1, world::SPOTS[0].2),
        }
        let m = S2C::Plots(self.plot_infos(u.id, grp));
        self.send(u.id, &m);
        for (id, name, look) in old {
            self.send(u.id, &S2C::Join { id, name, look });
        }
        self.broadcast(&S2C::Join { id: u.id, name: u.name.clone(), look });
        // Phasen 16–20: Inventar-Plätze angleichen, Werte/Leben/Ausrüstung, Insel-Inhalt
        self.inv_join(u.id);
        self.combat_join(u.id);
        self.island_join(u.id);
        Ok(session)
    }

    pub fn leave(&mut self, id: u32, session: u64) {
        if self.players.get(&id).is_some_and(|p| p.session == session) {
            self.drop_player(id);
        }
    }
    pub fn drop_player(&mut self, id: u32) {
        self.trade_abort(id, "Die andere Person hat den Campus verlassen.");
        if let Some(p) = self.players.remove(&id) {
            let _ = self.db.execute("update users set x = ?, y = ?, z = ? where id = ?", params![p.p[0], p.p[1], p.p[2], id]);
            self.broadcast(&S2C::Leave(id));
        }
    }
    /// Aussehen neu an alle melden (nach Anlegen eines Cosmetics).
    pub fn announce(&mut self, id: u32) {
        if let Some(p) = self.players.get(&id) {
            let m = S2C::Join { id, name: p.name.clone(), look: p.look };
            self.broadcast(&m);
        }
    }

    pub fn handle(&mut self, id: u32, m: C2S) {
        let now = self.now();
        match m {
            C2S::Move { p, yaw, anim } => {
                let Some(pl) = self.players.get_mut(&id) else { return };
                let ok = p.iter().all(|v| v.is_finite()) && yaw.is_finite() && (0.0..WORLD as f32).contains(&p[0]) && (0.0..WORLD as f32).contains(&p[2]) && (-64.0..200.0).contains(&p[1]);
                let dt = (now - pl.last_move).max(0) as f32 / 1000.0;
                let d = ((p[0] - pl.p[0]).powi(2) + (p[2] - pl.p[2]).powi(2)).sqrt();
                // Sprungpolster: von der letzten angenommenen Stelle aus darf der Bogen 6 s lang schnell sein
                if world::launch_pad(pl.p[0], pl.p[2], 4.0).is_some() {
                    pl.boost_until = pl.boost_until.max(now + 6000);
                }
                // Tempo-Grenze waagrecht; zu schnelle Sprünge werden verworfen, bis die Zeit sie deckt.
                let max = if now < pl.boost_until { world::LAUNCH_MAX_SPEED } else { eco::MAX_SPEED };
                let accepted = ok && d <= max * dt + 2.0;
                if accepted {
                    pl.p = p;
                    pl.yaw = yaw;
                    pl.anim = anim;
                    pl.last_move = now;
                }
                if ok && p[1] < world::VOID_Y && now - pl.void_at > 2000 {
                    pl.void_at = now;
                    self.void_fall(id);
                } else if accepted {
                    self.on_move(id, now); // Sprungplatte, Teleporter, Insel betreten (island.rs)
                }
            }
            C2S::Held(i) => self.set_held(id, i),
            C2S::Attack { mob } => {
                if let Err(E(_, why)) = self.attack(id, mob, now) {
                    self.hud(id, json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Shoot { dir } => {
                if let Err(E(_, why)) = self.shoot(id, dir, now) {
                    self.hud(id, json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Dig { x, y, z } => {
                if let Some(pl) = self.players.get_mut(&id) {
                    pl.dig = Some(([x as i32, y as i32, z as i32], now));
                }
            }
            C2S::Chest { x, y, z } => {
                if let Err(E(_, why)) = self.chest_open(id, x as i32, y as i32, z as i32) {
                    self.hud(id, json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Block { x, y, z, b } => {
                let (x, y, z) = (x as i32, y as i32, z as i32);
                if let Err(E(_, why)) = self.edit(id, x, y, z, b, now) {
                    let cur = self.block(x, y, z);
                    self.send(id, &S2C::Block { x: x as i16, y: y as i16, z: z as i16, b: cur, by: 0 });
                    self.hud(id, serde_json::json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Chat(n) => {
                let Some(pl) = self.players.get_mut(&id) else { return };
                if (n as usize) < proto::PHRASES.len() && now - pl.last_chat >= eco::CHAT_COOLDOWN_MS as i64 {
                    pl.last_chat = now;
                    self.broadcast(&S2C::Chat { id, n });
                }
            }
            C2S::Warp(w) => {
                if let Err(E(_, why)) = self.warp(id, w) {
                    self.hud(id, serde_json::json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Say { ch, text } => self.say(id, ch, &text),
            C2S::Talk(n) => {
                if let Err(E(_, why)) = self.talk(id, n) {
                    self.hud(id, serde_json::json!({ "t": "nein", "text": why }));
                }
            }
            C2S::Ability { tool, dir } => {
                if let Err(E(_, why)) = self.ability(id, tool, dir) {
                    self.hud(id, serde_json::json!({ "t": "nein", "text": why }));
                }
            }
            C2S::View(v) => {
                if let Some(pl) = self.players.get_mut(&id) {
                    pl.view = (v as i32).clamp(4, eco::MAX_VIEW_CHUNKS);
                }
            }
            C2S::Emote(n) => {
                let Some(pl) = self.players.get_mut(&id) else { return };
                if n >= proto::EMOTE_COUNT || now - pl.last_chat < eco::CHAT_COOLDOWN_MS as i64 {
                    return;
                }
                pl.last_chat = now;
                let item = campus_shared::items::EMOTE_WINKEN + n as u16;
                if n == 0 || self.owns(id, item) {
                    self.broadcast(&S2C::Emote { id, n });
                }
            }
        }
    }

    /// Darf `uid` die Insel besuchen? Bauende immer, sonst nach Einstellung des Besitzers.
    pub fn may_visit(&self, p: &Plot, uid: u32) -> bool {
        let grp = self.players.get(&uid).map_or(0, |pl| pl.group);
        plot_may(p, uid, grp) || p.visit == 0 || (p.visit == 1 && grp != 0 && (grp == p.owner_grp || grp == p.group))
    }
    /// Ankunft mit Blickrichtung: in der Spalte des Vorschlags sicheren Boden suchen (3×3×4 Luft), dorthin versetzen.
    pub fn spawn_at(&mut self, id: u32, hint: [f32; 3], yaw: f32) {
        let p = world::find_safe_spawn(|x, y, z| self.block(x, y, z), hint[0].floor() as i32, hint[2].floor() as i32).unwrap_or(hint);
        let now = self.now();
        if let Some(pl) = self.players.get_mut(&id) {
            pl.p = p;
            pl.yaw = yaw;
            pl.last_move = now;
        }
        self.send(id, &S2C::Spawn { p, yaw });
    }
    /// Sturz ins Leere: ein Viertel der ungesicherten Taler (Geldbörse, nicht Bank) ist weg, zurück zum Ankunftsort der Insel.
    fn void_fall(&mut self, id: u32) {
        let Some(pos) = self.players.get(&id).map(|p| p.p) else { return };
        let loss = self.taler(id) * world::VOID_LOSS_PCT / 100;
        if loss > 0 && self.pay(id, -loss, "leere").is_err() {
            return;
        }
        let (to, yaw) = match world::slot_at(pos[0] as i32, pos[2] as i32) {
            Some(s) => (world::plot_spawn(s), world::PLOT_YAW),
            None => {
                let s = world::SPOTS[world::nearest_spot(pos[0], pos[2]) as usize];
                (s.1, s.2)
            }
        };
        self.spawn_at(id, to, yaw);
        self.hud(id, json!({ "t": "leere", "verlust": loss }));
    }
    /// Spieler versetzen; der Server merkt sich die Stelle sofort, damit die Tempo-Grenze nicht greift.
    pub fn teleport(&mut self, id: u32, p: [f32; 3]) {
        let now = self.now();
        if let Some(pl) = self.players.get_mut(&id) {
            pl.p = p;
            pl.last_move = now;
        }
        self.send(id, &S2C::Teleport(p));
    }
    pub fn warp(&mut self, id: u32, w: Warp) -> R<()> {
        let (to, yaw) = match w {
            Warp::Spot(i) => world::SPOTS.get(i as usize).map(|s| (s.1, s.2)).ok_or(E(404, "Diesen Ort gibt es nicht.".into()))?,
            Warp::Home => self.plots.iter().find(|p| p.owner == id).map(|p| (world::plot_spawn(p.slot), world::PLOT_YAW)).ok_or(E(404, "Du hast noch keine Insel.".into()))?,
            Warp::Plot(pid) => {
                let p = self.plots.iter().find(|p| p.id == pid).ok_or(E(404, "Diese Insel gibt es nicht.".into()))?;
                if !self.may_visit(p, id) {
                    return no(403, "Diese Insel ist nicht für Besuch geöffnet.");
                }
                (world::plot_spawn(p.slot), world::PLOT_YAW)
            }
        };
        self.spawn_at(id, to, yaw);
        match w {
            Warp::Spot(s) => self.quest_hit(id, |g| (g == Goal::Warp(s)).then_some(1)),
            Warp::Plot(p) if self.plots.iter().any(|q| q.id == p && q.owner != id) => self.quest_hit(id, |g| (g == Goal::Visit).then_some(1)),
            _ => self.quest_hit(id, |g| (g == Goal::Home).then_some(1)),
        }
        Ok(())
    }

    /// Block setzen (b > 0) oder abbauen (b = 0): Abklingzeit, Reichweite, Baurecht (world::edit_allowed), Abbauzeit, Vorrat.
    fn edit(&mut self, id: u32, x: i32, y: i32, z: i32, b: u8, now: i64) -> R<()> {
        let Some(pl) = self.players.get_mut(&id) else { return no(401, "Nicht verbunden.") };
        if now - pl.last_block < eco::BLOCK_COOLDOWN_MS as i64 {
            return no(429, "Zu schnell.");
        }
        pl.last_block = now;
        let dig = if b == 0 { pl.dig.take() } else { None };
        let (pos, grp) = (pl.p, pl.group);
        if !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) || !(1..CH - 1).contains(&y) {
            return no(400, "Außerhalb der Welt.");
        }
        let d = ((x as f32 + 0.5 - pos[0]).powi(2) + (y as f32 + 0.5 - pos[1] - 1.6).powi(2) + (z as f32 + 0.5 - pos[2]).powi(2)).sqrt();
        if d > eco::REACH {
            return no(403, "Zu weit weg.");
        }
        let cur = self.block(x, y, z);
        if !self.may_edit(id, grp, x, z, cur, b > 0) {
            return no(403, "Hier darfst du nicht bauen.");
        }
        if world::anchor(x, y, z) {
            return no(403, "Der Ankerblock deiner Insel bleibt stehen.");
        }
        if b == 0 {
            if cur == 0 || cur == world::WASSER {
                return no(409, "Da ist nichts.");
            }
            if !world::hardness(cur).is_finite() {
                return no(409, "Das lässt sich nicht abbauen.");
            }
            // Abbauzeit: schnellstes Werkzeug, das man besitzt, und Baukunst-Stufe; der Browser hat mit Dig begonnen
            let bau = self.levels(id)[sb::SK_BAU as usize];
            // Abbau-Tempo aus den Werten (Rüstung, Verzauberung „Effizienz“ des gehaltenen Werkzeugs, Set-Bonus „Eile“)
            let speed = self.mine_speed(id, cur);
            let secs = [0, sb::HAMMER, sb::SPITZHACKE, sb::AXT].into_iter().filter(|&t| t == 0 || self.owns(id, t)).map(|t| world::mine_secs_speed(cur, t, bau, speed)).fold(f32::INFINITY, f32::min);
            if !dig.is_some_and(|(p, t)| p == [x, y, z] && world::mine_ok(secs, now - t)) {
                return no(429, "Zu schnell.");
            }
            // Pflanzen ernten, Truhen nur leer, Funktionsblöcke austragen (island.rs / inv.rs); true = schon erledigt
            if self.before_break(id, x, y, z, cur)? {
                return Ok(());
            }
            return self.take_block(id, x, y, z, cur);
        }
        if world::block(b).is_none() {
            return no(400, "Diesen Block gibt es nicht.");
        }
        if world::FLAGS[b as usize] & (world::F_LIQUID | world::F_REGEN) != 0 || !world::hardness(b).is_finite() {
            return no(400, "Diesen Block kannst du nicht setzen.");
        }
        if cur != 0 && cur != world::WASSER {
            return no(409, "Da ist schon ein Block.");
        }
        self.before_place(id, x, y, z, b)?; // Saat nur auf Ackerboden usw. (island.rs)
        let n = self.give(id, b as u16, -1).map_err(|_| E(402, format!("Kein Block „{}“ im Vorrat.", campus_shared::items::item_name(b as u16))))?;
        self.set_block(x, y, z, b, id);
        self.send(id, &S2C::Inv { item: b as u16, n });
        let _ = self.add_xp(id, sb::SK_BAU, 1);
        self.after_place(id, x, y, z, b); // Pflanze/Schild/Ständer/Truhe eintragen, Questbuch (island.rs)
        Ok(())
    }

    /// Block abbauen und gutschreiben: nachwachsend → Ertrag ins Lager, ABGEBAUT steht REGEN_MS lang da;
    /// sonst der Block selbst. Dazu Baukunst-EP, Aktionsleiste und Quest-Fortschritt.
    pub fn take_block(&mut self, id: u32, x: i32, y: i32, z: i32, cur: u8) -> R<()> {
        let roll = self.rng.next() as u32;
        let item = match world::regen_drop(cur, roll) {
            Some(drop) => {
                self.set_block(x, y, z, world::ABGEBAUT, id);
                let due = self.now() + world::REGEN_MS;
                self.regen.push((due, x, y, z, cur));
                drop
            }
            None => {
                self.set_block(x, y, z, 0, id);
                cur as u16
            }
        };
        // Glück (Werte): je 100 % ein sicheres Stück mehr, der Rest als Chance
        let fortune = self.players.get(&id).map_or(0.0, |p| p.stats.fortune).max(0.0);
        let k = 1 + (fortune / 100.0) as u32 + u32::from(((self.rng.next() % 100) as f32) < fortune % 100.0);
        let n = self.gain(id, item, k)?;
        self.send(id, &S2C::Inv { item, n });
        let xp = world::mine_xp(cur);
        let _ = self.add_xp(id, sb::SK_BAU, xp);
        self.hud(id, json!({ "t": "aktion", "text": format!("+{k} {} | +{xp} Baukunst-XP", campus_shared::items::item_name(item)) }));
        let minion = self.db.query_row("select 1 from minions where user = ?", [id], |_| Ok(())).is_ok();
        self.quest_hit(id, |g| match g {
            Goal::Mine(i, _) => (i == item).then_some(1),
            Goal::MineMinion(i, _) => (i == item && minion).then_some(1),
            _ => None,
        });
        Ok(())
    }

    // ---------- Takt (10 Hz) ----------
    pub fn tick(&mut self) {
        let t0 = Instant::now();
        // erschöpfte nachwachsende Blöcke zurückstellen
        let now = self.now();
        while let Some(k) = self.regen.iter().position(|r| r.0 <= now) {
            let (_, x, y, z, b) = self.regen.swap_remove(k);
            if self.block(x, y, z) == world::ABGEBAUT {
                self.set_block(x, y, z, b, 0);
            }
        }
        // Prüfungs-Zitadelle: steigt sofort aus, wenn niemand dort ist (mobs.rs, LAW 2)
        self.mob_tick(now);
        self.flush_notes();
        const CELL: f32 = eco::PLAYER_VIEW;
        for v in self.grid.values_mut() {
            v.clear();
        }
        self.ids.clear();
        for p in self.players.values() {
            self.grid.entry(((p.p[0] / CELL) as i32, (p.p[2] / CELL) as i32)).or_default().push(p.id);
            self.ids.push(p.id);
        }
        for k in 0..self.ids.len() {
            let id = self.ids[k];
            let (pos, tx) = {
                let p = &self.players[&id];
                (p.p, p.tx.clone())
            };
            // Nachbarn nur aus den 3×3 Zellen um den Spieler: O(1) je Spieler statt aller Paare
            let (gx, gz) = ((pos[0] / CELL) as i32, (pos[2] / CELL) as i32);
            self.states.clear();
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if let Some(v) = self.grid.get(&(gx + dx, gz + dz)) {
                        for &o in v {
                            if o != id {
                                let q = &self.players[&o];
                                self.states.push(PState { id: o, p: q.p, yaw: q.yaw, anim: q.anim });
                            }
                        }
                    }
                }
            }
            if !self.states.is_empty() {
                let m = S2C::Players(std::mem::take(&mut self.states));
                self.scratch.clear();
                let _ = bincode::serialize_into(&mut self.scratch, &m);
                if let S2C::Players(v) = m {
                    self.states = v;
                }
                let _ = tx.try_send(Bytes::copy_from_slice(&self.scratch)); // Positionen dürfen verloren gehen
            }
            // Stücke in Sichtweite nachladen, die nächsten zuerst
            let (pcx, pcz) = ((pos[0] as i32) >> 4, (pos[2] as i32) >> 4);
            let view = self.players[&id].view;
            let mut budget = 6;
            for r in 0..self.ring.len() {
                let (dx, dz) = self.ring[r];
                if dx * dx + dz * dz > view * view {
                    break; // Ring ist nach Abstand sortiert
                }
                let (cx, cz) = (pcx + dx, pcz + dz);
                if !(0..WC).contains(&cx) || !(0..WC).contains(&cz) {
                    continue;
                }
                let i = ci(cx, cz);
                if self.players[&id].sent[i / 64] >> (i % 64) & 1 == 1 {
                    continue;
                }
                if self.chunks[i].msg.is_none() {
                    let b = &self.chunks[i].b;
                    let m = S2C::Chunk { cx: cx as u8, cz: cz as u8, rle: if b.is_empty() { world::rle_air() } else { world::rle(b) } };
                    self.chunks[i].msg = Some(Bytes::from(proto::enc(&m)));
                }
                let Some(m) = self.chunks[i].msg.clone() else { continue };
                if tx.try_send(m).is_err() {
                    break;
                }
                if let Some(p) = self.players.get_mut(&id) {
                    p.sent[i / 64] |= 1 << (i % 64);
                }
                budget -= 1;
                if budget == 0 {
                    break;
                }
            }
        }
        while let Some(id) = self.kick.pop() {
            self.drop_player(id);
        }
        let us = t0.elapsed().as_micros() as u64;
        self.stats.ticks += 1;
        self.stats.last_us = us;
        self.stats.max_us = self.stats.max_us.max(us);
        self.stats.avg_us += (us as f64 - self.stats.avg_us) / self.stats.ticks.min(600) as f64;
    }

    // ---------- Vorrat ----------
    pub fn inv(&self, user: u32, item: u16) -> u32 {
        self.db.query_row("select n from inv where user = ? and item = ?", params![user, item], |r| r.get(0)).optional().ok().flatten().unwrap_or(0)
    }
    /// Vorrat ändern; schlägt fehl, wenn er unter 0 fiele. Liefert den neuen Stand.
    /// Einziger Weg für Gegenstände: läuft über InventoryTx (inv.rs) – Plätze, Überlauf in den Beutel (/stash).
    pub fn give(&self, user: u32, item: u16, delta: i64) -> R<u32> {
        crate::inv::InventoryTx::execute(self, user, item, delta)
    }
    pub fn owns(&self, user: u32, item: u16) -> bool {
        self.inv(user, item) > 0 || self.db.query_row("select 1 from timed where user = ? and item = ? and until > ?", params![user, item, self.now()], |_| Ok(())).optional().ok().flatten().is_some()
    }
}

pub fn plot_may(p: &Plot, uid: u32, group: u32) -> bool {
    (p.owner != 0 && p.owner == uid) || p.acl.contains(&uid) || (p.group != 0 && p.group == group)
}
