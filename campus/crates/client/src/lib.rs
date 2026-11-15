//! Campus im Browser: reiner Anzeiger und Eingabe-Sender. Der Server entscheidet alles; hier wird nur vorhergesagt und gezeichnet.
//! Die Oberfläche (hud.js) reicht Eingaben und WebSocket-Daten herein und bekommt Ereignisse über `campusHud.event`.
mod avatar;
mod deco;
mod fx;
mod gfx;
mod kino;
mod world;

use avatar::{Particles, Pose};
use campus_shared::combat as cb;
use campus_shared::farming;
use campus_shared::items::{self, Look};
use campus_shared::npc;
use campus_shared::proto::{self, CropState, NpcState, PlotInfo, SignState, StandState, Warp, C2S, MOB_DEAD, MOB_HURT, S2C};
use campus_shared::rpg;
use campus_shared::skyblock as sb;
use campus_shared::world as w;
use bytemuck::Zeroable;
use gfx::{Env, Gfx, Inst};
use glam::{Mat4, Vec3, Vec4};
use wasm_bindgen::prelude::*;
use world::World;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = campusHud, js_name = send)]
    fn js_send(b: &[u8]);
    #[wasm_bindgen(js_namespace = campusHud, js_name = event)]
    fn js_event(kind: &str, a: f64, b: f64, s: &str);
    #[wasm_bindgen(js_namespace = campusHud, js_name = plates)]
    fn js_plates(a: &[f32]);
    #[wasm_bindgen(js_namespace = campusHud, js_name = numbers)]
    fn js_numbers(a: &[f32]);
}

struct Remote {
    id: u32,
    look: Look,
    gear: [u16; 8],
    ip: fx::Interp,
    pos: Vec3,
    yaw: f32,
    anim: u8,
    walk: f32,
    moving: f32,
    emote: (u8, f32),
    pet: Vec3,
    trail: f32,
    seen: f32,
}

const EYE: f32 = 1.62;
/// Reichweite für Zielblock, Abbauen und Setzen
const REACH: f32 = 5.0;
/// NPCs ansprechen bis zu dieser Entfernung
const TALK: f32 = 4.0;
/// Halbe Kantenlänge der Schattenkarte in Blöcken
const SHADOW_R: f32 = 64.0;
/// Ein Tag-Nacht-Wechsel in Sekunden (20 Minuten)
const DAY_SECS: f64 = 1200.0;

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// Namensschild-Nummern der NPCs (über allen Konto-Nummern, in f32 noch exakt)
pub const NPC_BASE: u32 = 1_000_000;
/// Plaketten-Nummern der Schilder (k = Index aus dem Ereignis `schilder`)
pub const SIGN_BASE: u32 = 2_000_000;
/// Höchstzahl Namensschilder/Plaketten je Bild
const PLATES: usize = 96;

fn palette_index(rgb: [u8; 3]) -> u8 {
    let d = |p: &[u8; 3]| (0..3).map(|k| (p[k] as i32 - rgb[k] as i32).pow(2)).sum::<i32>();
    items::PALETTE.iter().enumerate().min_by_key(|(_, p)| d(p)).map_or(0, |(i, _)| i as u8)
}

/// Einstellungen aus dem Menü (nur Browser, nichts davon belastet den Server).
struct Opts {
    view: f32, // Sichtweite in Blöcken
    fov: f32,  // senkrecht, Bogenmaß
    ao: bool,
    clouds: bool,
    particles: bool,
}

#[wasm_bindgen]
pub struct Game {
    gfx: Gfx,
    world: World,
    parts: Particles,
    id: u32,
    ready: bool,
    pos: Vec3,
    vel: Vec3,
    spawn: Vec3,
    yaw: f32,
    pitch: f32,
    ground: bool,
    mx: f32,
    mz: f32,
    jump: bool,
    sprint: bool,
    sel: u8,
    view: u8,
    demo: bool,
    look: Look,
    walk: f32,
    moving: f32,
    emote: (u8, f32),
    pet: Vec3,
    trail: f32,
    step: f32,
    remotes: Vec<Remote>,
    plots: Vec<PlotInfo>,
    here: u32,
    target: Option<([i32; 3], [i32; 3])>,
    last_t: f64,
    time: f32,
    send_acc: f32,
    fps: f32,
    quads: u32,
    insts: Vec<Inst>,
    visible: Vec<u32>,
    plate_buf: Vec<f32>,
    opts: Opts,
    swing: f32,
    tool: u16,
    tool_at: f32,
    portal_at: f32,
    minions: Vec<(Vec3, u8, u8)>,
    npc_looks: Vec<Look>,
    folk: Vec<Look>,
    settled: bool,  // Boden nach der Ankunft geladen (oder lange genug gewartet)
    load_wait: f32,
    void_at: f32,   // Zeitpunkt des Sturzes ins Leere, < 0 = keiner
    flying: bool,   // Sprungpolster-Bogen bis zur Landung
    fov_boost: f32, // Sprint weitet das Sichtfeld
    mining: bool,   // linke Taste / Abbau-Knopf gehalten
    dig: Option<[i32; 3]>,
    dig_b: u8,
    dig_ok: bool,
    dig_p: f32, // Fortschritt 0..1
    dig_hit: f32,
    dig_q: i32,
    bau: u32, // Baukunst-Stufe
    nein_at: f32,
    zone: &'static str,
    prompt: (u8, u32),
    sky_mode: u8, // 0 dynamisch, 1 Tag, 2 Sonnenuntergang, 3 Nacht
    shadow_vis: Vec<u32>,
    env: Env,
    light_at: f32,
    // ---------- Phasen 16–20 ----------
    weapon: u16,     // gehaltene Waffe (0 = keine)
    gear: [u16; 8],  // eigene sichtbare Ausrüstung (S2C::Gear)
    mine_speed: f32, // Abbau-Tempo in Prozent (Hud `werte`)
    telekinese: bool,
    attacking: bool, // linke Taste im Kampf gehalten
    atk_at: f32,     // letzter Schlag/Schuss
    hurt: f32,       // Rot-Aufblitzen 0..1
    mobs: Vec<fx::Mob>,
    numbers: fx::Numbers,
    num_buf: Vec<f32>,
    drops: fx::Drops,
    waves: fx::Waves,
    isl_now: i64,  // Serverzeit (ms) bei Empfang von S2C::Island
    isl_recv: f32, // lokale Zeit bei Empfang
    crops: Vec<CropState>,
    signs: Vec<SignState>,
    stands: Vec<StandState>,
    // ---------- Phasen 21–25 ----------
    wx_next: u8,  // gewünschtes Wetter
    wx_kind: u8,  // gezeigtes Wetter (blendet über)
    wx_k: f32,    // Stärke 0..1
    filter: u8,   // Foto-Filter
    focal: f32,   // Brennweite in Blöcken, 0 = aus
    ride: Option<kino::Ride>,
    free: Option<kino::Free>,
    architect: bool,
    selection: Option<([i32; 3], [i32; 3])>, // min, max (einschließlich)
    speed: f32,
    npcs: Vec<NpcState>,
}

fn emote_len(n: u8) -> f32 {
    if n == 2 { 30.0 } else { 2.6 }
}
fn rgb(b: u8) -> [u8; 3] {
    w::block(b).map_or([200, 200, 200], |d| d.rgb)
}

#[wasm_bindgen]
impl Game {
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<Game, JsValue> {
        console_error_panic_hook::set_once();
        let (cw, ch) = (canvas.width().max(1), canvas.height().max(1));
        let gfx = Gfx::new(canvas, cw, ch).await.map_err(|e| JsValue::from_str(&e))?;
        let pos = Vec3::from(w::SPOTS[0].1);
        Ok(Game {
            gfx,
            world: World::new(),
            parts: Particles::new(),
            id: 0,
            ready: false,
            pos,
            vel: Vec3::ZERO,
            spawn: pos,
            yaw: w::SPOTS[0].2,
            pitch: -0.1,
            ground: false,
            mx: 0.0,
            mz: 0.0,
            jump: false,
            sprint: false,
            sel: 0,
            view: 0,
            demo: false,
            look: Look::default(),
            walk: 0.0,
            moving: 0.0,
            emote: (255, 0.0),
            pet: pos,
            trail: 0.0,
            step: 0.0,
            remotes: Vec::with_capacity(64),
            plots: Vec::new(),
            here: u32::MAX,
            target: None,
            last_t: 0.0,
            time: 0.0,
            send_acc: 0.0,
            fps: 60.0,
            quads: 0,
            insts: Vec::with_capacity(gfx::MAX_INST),
            visible: Vec::with_capacity((w::WC * w::WC) as usize),
            plate_buf: Vec::with_capacity(PLATES * 4),
            opts: Opts { view: 7.0 * 16.0, fov: 70f32.to_radians(), ao: true, clouds: true, particles: true },
            swing: 1.0,
            tool: 0,
            tool_at: -9.0,
            portal_at: -9.0,
            minions: Vec::new(),
            npc_looks: sb_npc_looks(),
            folk: folk_looks(),
            settled: false,
            load_wait: 0.0,
            void_at: -1.0,
            flying: false,
            fov_boost: 0.0,
            mining: false,
            dig: None,
            dig_b: 0,
            dig_ok: false,
            dig_p: 0.0,
            dig_hit: 0.0,
            dig_q: 0,
            bau: 0,
            nein_at: -9.0,
            zone: "",
            prompt: (0, 0),
            sky_mode: 0,
            shadow_vis: Vec::with_capacity(256),
            env: Env::zeroed(),
            light_at: -1.0,
            weapon: 0,
            gear: [0; 8],
            mine_speed: 0.0,
            telekinese: false,
            attacking: false,
            atk_at: -9.0,
            hurt: 0.0,
            mobs: Vec::with_capacity(32),
            numbers: fx::Numbers::new(),
            num_buf: Vec::with_capacity(64 * 5),
            drops: fx::Drops::new(),
            waves: fx::Waves::new(),
            isl_now: 0,
            isl_recv: 0.0,
            crops: Vec::new(),
            signs: Vec::new(),
            stands: Vec::new(),
            wx_next: 0,
            wx_kind: 0,
            wx_k: 0.0,
            filter: 0,
            focal: 0.0,
            ride: None,
            free: None,
            architect: false,
            selection: None,
            speed: 1.0,
            npcs: Vec::new(),
        })
    }

    /// „WebGPU“ oder „WebGL 2“.
    pub fn backend(&self) -> String {
        if self.gfx.webgpu { "WebGPU".into() } else { "WebGL 2".into() }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gfx.resize(width, height);
    }
    /// Bewegung: x seitwärts, z vorwärts, je -1..1.
    pub fn input(&mut self, x: f32, z: f32, jump: bool, sprint: bool) {
        if self.ride.is_some() {
            return; // während der Fahrt ruhen die Eingaben
        }
        if let Some(f) = self.free.as_mut() {
            // Freikamera: die Figur bleibt stehen
            f.mx = x.clamp(-1.0, 1.0);
            f.mz = z.clamp(-1.0, 1.0);
            f.up = jump;
            f.fast = sprint;
            return;
        }
        self.mx = x.clamp(-1.0, 1.0);
        self.mz = z.clamp(-1.0, 1.0);
        self.jump = jump;
        self.sprint = sprint;
    }
    pub fn turn(&mut self, dx: f32, dy: f32) {
        if self.ride.is_some() {
            return;
        }
        if let Some(f) = self.free.as_mut() {
            return f.turn(dx, dy);
        }
        self.yaw -= dx;
        self.pitch = (self.pitch - dy).clamp(-1.55, 1.55);
    }

    // ---------- Phasen 21–25 ----------
    /// 0 Klar, 1 Sommerregen, 2 Schneefall, 3 Kirschblüten; blendet je 1 s aus/ein.
    pub fn set_weather(&mut self, kind: u8) {
        self.wx_next = kind.min(3);
    }
    /// F8-Freikamera an/aus.
    pub fn set_photo(&mut self, on: bool) {
        if on && self.free.is_none() {
            self.skip_cutscene();
            self.free = Some(kino::Free::new(self.pos + Vec3::Y * EYE, self.yaw, self.pitch));
            self.halt();
        } else if !on {
            self.free = None;
        }
    }
    /// 0 Standard, 1 Retro-Pixel, 2 Goldene Stunde, 3 Kino-Kontrast; `focal` in Blöcken, 0 = aus.
    pub fn set_filter(&mut self, filter: u8, focal: f32) {
        self.filter = filter.min(3);
        self.focal = if focal.is_finite() { focal.clamp(0.0, 64.0) } else { 0.0 };
    }
    /// Kamerafahrt 0 Hub-Marktplatz, 1 Zitadelle, 2 Katakomben, 3 Arena; false = gerade nicht möglich.
    pub fn cutscene(&mut self, id: u8) -> bool {
        if id > 3 || !self.ready || !self.settled || self.ride.is_some() || self.free.is_some() {
            return false;
        }
        let (mut pts, target) = kino::route(id, self.pos, self.yaw);
        for p in pts.iter_mut() {
            *p = self.ride_point(*p);
        }
        self.ride = Some(kino::Ride { id, t: 0.0, pts, target });
        self.halt();
        js_event("kino", 1.0, id as f64, "");
        true
    }
    pub fn skip_cutscene(&mut self) {
        if let Some(r) = self.ride.take() {
            js_event("kino", 0.0, r.id as f64, "");
        }
    }
    /// Architekten-Stab aktiv: linke Taste setzt Pos1, rechte Pos2 (C2S::Arch).
    pub fn set_architect(&mut self, on: bool) {
        self.architect = on;
        if on {
            self.mining = false;
            self.attacking = false;
            self.dig_stop();
        }
    }
    /// Auswahl-Quader (Ecken beliebig, einschließlich) mit leuchtenden Kanten.
    pub fn set_selection(&mut self, on: bool, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        self.selection = on.then(|| ([x0.min(x1), y0.min(y1), z0.min(z1)], [x0.max(x1), y0.max(y1), z0.max(z1)]));
    }
    /// Lauf-Tempo-Faktor (Katakomben-Segen), 1.0..=1.5.
    pub fn set_speed(&mut self, f: f32) {
        self.speed = if f.is_finite() { f.clamp(1.0, 1.5) } else { 1.0 };
    }
    pub fn select(&mut self, block: u8) {
        self.sel = block;
        self.tool = 0;
        self.weapon = 0;
    }
    /// Werkzeug in die Hand nehmen (0 = keins).
    pub fn select_tool(&mut self, item: u16) {
        self.tool = if sb::tool(item).is_some() { item } else { 0 };
        if self.tool != 0 {
            self.sel = 0;
            self.weapon = 0;
        }
    }
    /// Leisten-Wahl: meldet den Platz dem Server und nimmt Block (< 256), Werkzeug oder Waffe in die Hand.
    pub fn held(&mut self, slot: u8, item: u16) {
        js_send(&proto::enc(&C2S::Held(slot)));
        if item != 0 && item < 256 {
            self.select(item as u8);
        } else if sb::tool(item).is_some() {
            self.select_tool(item);
        } else {
            self.select(0);
            if rpg::is_weapon(item) {
                self.weapon = item;
            }
        }
        if self.weapon == 0 {
            self.attacking = false;
        }
    }
    /// Werte aus dem Hud (`werte`): Abbau-Tempo in Prozent, Telekinese (Beute kommt ohne Würfelchen).
    pub fn set_stats(&mut self, mining_speed: f32, telekinese: bool) {
        self.mine_speed = mining_speed.max(0.0);
        self.telekinese = telekinese;
    }
    /// Textur-Auflösung 16, 64 oder 128.
    pub fn set_texture(&mut self, res: u32) {
        self.gfx.set_tex_res(res as f32);
    }
    /// Eigenes Textur-Paket (RGBA, Atlas 4×4 Kacheln).
    pub fn set_texture_pack(&mut self, rgba: &[u8], w: u32, h: u32) {
        self.gfx.set_texture_pack(rgba, w, h);
    }
    pub fn clear_texture_pack(&mut self) {
        self.gfx.clear_texture_pack();
    }
    /// Freies Feld vor dem anvisierten Block (zum Aufstellen), sonst leer.
    pub fn target_prev(&self) -> Vec<i32> {
        self.target.map_or(Vec::new(), |(_, p)| p.to_vec())
    }
    /// Chatzeile oder Befehl senden.
    pub fn say(&mut self, ch: u8, text: &str) {
        js_send(&proto::enc(&C2S::Say { ch, text: text.chars().take(160).collect() }));
    }
    /// Minions der aktuellen Insel: je fünf Zahlen x, y, z, Art, Stufe.
    pub fn set_minions(&mut self, a: &[f32]) {
        self.minions = a.chunks_exact(5).map(|c| (Vec3::new(c[0] + 0.5, c[1], c[2] + 0.5), c[3] as u8, c[4] as u8)).collect();
    }
    /// Werkzeug benutzen (rechte Maustaste mit Werkzeug in der Hand).
    pub fn use_tool(&mut self) {
        let t = self.tool;
        let Some(def) = sb::tool(t) else { return };
        if t == w::SPITZHACKE || t == w::AXT {
            return; // reine Abbau-Werkzeuge, keine Fähigkeit
        }
        if self.time - self.tool_at < def.cooldown_ms as f32 / 1000.0 {
            return;
        }
        self.tool_at = self.time;
        self.swing = 0.0;
        let dir = self.look_dir();
        if t == sb::KOMPASS {
            return js_event("kompass", 0.0, 0.0, "");
        }
        if t == sb::HAKEN {
            self.vel = dir * sb::GRAPPLE_SPEED + Vec3::Y * 4.0;
            js_event("sfx", 10.0, 1.0, "");
        }
        js_send(&proto::enc(&C2S::Ability { tool: t, dir: dir.to_array() }));
    }
    /// 0 = erste Person, 1 = von hinten, 2 = von vorn (Umkleide).
    pub fn set_view(&mut self, v: u8) {
        self.view = v % 3;
    }
    /// Grafik-Einstellungen: Sichtweite in Stücken (4..12), Blickwinkel in Grad (60..110), Eckenschatten, Wolken, Partikel.
    pub fn set_options(&mut self, chunks: u8, fov_deg: f32, ao: bool, clouds: bool, particles: bool) {
        let chunks = chunks.clamp(4, 12);
        self.opts = Opts { view: chunks as f32 * 16.0, fov: fov_deg.clamp(60.0, 110.0).to_radians(), ao, clouds, particles };
        js_send(&proto::enc(&C2S::View(chunks)));
    }
    /// Shader-Stufe 0 Eco, 1 Hoch, 2 Ultra, 3 Extrem; Himmel 0 dynamisch, 1 Tag, 2 Sonnenuntergang, 3 Nacht. Wirkt sofort.
    pub fn set_graphics(&mut self, tier: u8, sky: u8) {
        self.gfx.set_tier(tier);
        self.sky_mode = sky.min(3);
        self.light_at = -1.0;
    }
    /// Grafikkarte, wie der Browser sie nennt (für die automatische Stufe).
    pub fn gpu_info(&self) -> String {
        self.gfx.adapter_name.clone()
    }
    /// Reise: 0 eigene Insel, 1 Insel mit Nummer `n`, 2 Hub-Ort `n`.
    pub fn warp(&mut self, kind: u8, n: u32) {
        let w = match kind { 0 => Warp::Home, 1 => Warp::Plot(n), _ => Warp::Spot(n as u8) };
        js_send(&proto::enc(&C2S::Warp(w)));
    }
    /// Rundflug für die Leistungsmessung.
    pub fn set_demo(&mut self, on: bool) {
        self.demo = on;
    }
    /// 0 Bilder/s, 1 sichtbare Stücke, 2 Rechtecke, 3 Instanzen, 4 geladene Stücke, 5–7 Position.
    pub fn info(&self, i: u8) -> f32 {
        match i {
            0 => self.fps,
            1 => self.visible.len() as f32,
            2 => self.quads as f32,
            3 => self.insts.len() as f32,
            4 => self.world.loaded as f32,
            5 => self.pos.x,
            6 => self.pos.y,
            _ => self.pos.z,
        }
    }
    pub fn chat(&mut self, n: u8) {
        js_send(&proto::enc(&C2S::Chat(n)));
    }
    pub fn emote(&mut self, n: u8) {
        js_send(&proto::enc(&C2S::Emote(n)));
    }

    /// Darf ich hier setzen (`place`) bzw. abbauen? Gleiche Regel wie der Server (world::edit_allowed).
    fn may_edit(&self, x: i32, z: i32, cur: u8, place: bool) -> bool {
        let slot = w::slot_at(x, z);
        let (mut may, mut in_plot) = (false, false);
        for p in self.plots.iter().filter(|p| p.may && Some(p.slot) == slot) {
            may = true;
            in_plot |= w::in_rect(w::plot_rect(p.slot, p.size), x, z);
        }
        w::edit_allowed(x, z, cur, place, may, in_plot)
    }

    fn talk(&mut self, n: u8) {
        self.swing = 0.0;
        js_send(&proto::enc(&C2S::Talk(n)));
        js_event("npc", n as f64, 0.0, "");
    }

    /// 0 = NPC ansprechen (Abbauen läuft über `mine`), 1 = setzen bzw. Werkzeug. Setzen wird sofort angezeigt;
    /// lehnt der Server ab, stellt seine Antwort den Block zurück.
    pub fn act(&mut self, kind: u8) {
        if self.cam_busy() {
            return;
        }
        if kind == 1 && self.architect {
            return self.arch_click(2);
        }
        // Personen ansprechen hat Vorrang vor Blöcken
        if let Some(n) = self.npc_in_sight() {
            return self.talk(n);
        }
        if kind == 0 {
            return;
        }
        if self.tool != 0 {
            return self.use_tool();
        }
        let Some((_, c)) = self.target else { return };
        if !(1..w::CH - 1).contains(&c[1]) {
            return;
        }
        let b = self.sel;
        if b == 0 {
            return js_event("nein", 0.0, 0.0, "Wähle zuerst einen Block aus der Leiste.");
        }
        if !self.may_edit(c[0], c[2], self.world.get(c[0], c[1], c[2]), true) {
            return js_event("nein", 0.0, 0.0, "Hier darfst du nicht bauen. Bauen kannst du auf deiner eigenen Insel.");
        }
        let (lo, hi) = (self.pos - Vec3::new(0.3, 0.0, 0.3), self.pos + Vec3::new(0.3, 1.8, 0.3));
        let (bl, bh) = (Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32), Vec3::new(c[0] as f32 + 1.0, c[1] as f32 + 1.0, c[2] as f32 + 1.0));
        if lo.cmplt(bh).all() && hi.cmpgt(bl).all() {
            return; // nicht in sich selbst bauen
        }
        self.swing = 0.0;
        self.world.set(c[0], c[1], c[2], b);
        js_send(&proto::enc(&C2S::Block { x: c[0] as i16, y: c[1] as i16, z: c[2] as i16, b }));
    }

    /// Linke Taste bzw. Abbau-Knopf gedrückt (`true`) oder losgelassen (`false`, auch bei Fokusverlust).
    pub fn mine(&mut self, on: bool) {
        if on && (self.cam_busy() || self.architect) {
            if !self.cam_busy() {
                self.arch_click(1);
            }
            self.attacking = false;
            self.mining = false;
            return self.dig_stop();
        }
        if on {
            if let Some(n) = self.npc_in_sight() {
                return self.talk(n);
            }
            // Kampf: Waffe in der Hand oder ein Wesen in Reichweite im Fadenkreuz
            if self.weapon != 0 || self.mob_in_sight().is_some() {
                self.attacking = true;
                self.mining = false;
                self.dig_stop();
                return self.strike();
            }
            self.swing = 0.0;
        }
        self.attacking = false;
        self.mining = on;
        if !on {
            self.dig_stop();
        }
    }

    /// Taste E: NPC ansprechen, Truhe öffnen, Werkbank oder Questtafel benutzen.
    pub fn interact(&mut self) {
        if self.cam_busy() {
            return;
        }
        match self.prompt_now() {
            (1, n, _) => self.talk(n as u8),
            (2, b, c) => match b as u8 {
                w::TRUHE | w::SCHATZ | w::GROSSE_TRUHE | w::ALTAR | w::BOSS_ALTAR | w::STAENDER | w::SCHILD => js_send(&proto::enc(&C2S::Chest { x: c[0] as i16, y: c[1] as i16, z: c[2] as i16 })),
                b if w::USE_NEW.contains(&b) => js_send(&proto::enc(&C2S::Chest { x: c[0] as i16, y: c[1] as i16, z: c[2] as i16 })),
                w::WERKBANK => js_event("werkbank", 0.0, 0.0, ""),
                _ => js_event("questbrett", 0.0, 0.0, ""),
            },
            (3, ..) => js_event("trainer", 0.0, 0.0, ""),
            _ => {}
        }
    }

    /// Baukunst-Stufe (beschleunigt das Abbauen wie auf dem Server).
    pub fn set_bau_level(&mut self, level: u32) {
        self.bau = level;
    }

    /// Nachricht vom Server (bincode).
    pub fn message(&mut self, data: &[u8]) {
        let Some(m) = proto::dec::<S2C>(data) else { return };
        match m {
            S2C::Welcome { id, pos, state } => {
                self.id = id;
                self.arrive(Vec3::from(pos));
                self.spawn = Vec3::from(w::SPOTS[0].1);
                self.world.st = state;
                self.ready = true;
                js_event("welcome", id as f64, 0.0, "");
            }
            S2C::Chunk { cx, cz, rle } => self.world.load(cx as i32, cz as i32, &rle),
            S2C::Block { x, y, z, b, by } => {
                let (x, y, z) = (x as i32, y as i32, z as i32);
                let old = self.world.get(x, y, z);
                self.world.set(x, y, z, b);
                // eigenes Abbauen hat seine Splitter und Töne schon beim Zerbrechen bekommen
                let own_break = by == self.id && (b == 0 || b == w::ABGEBAUT);
                if by != 0 && !own_break {
                    let at = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    let look = if by == self.id { Some(self.look) } else { self.remotes.iter().find(|r| r.id == by).map(|r| r.look) };
                    let fx = look.and_then(|l| items::cosmetic(l.slots[items::BUILD_FX as usize]));
                    let base = rgb(if b != 0 { b } else { old });
                    match fx {
                        Some(c) if b != 0 => self.parts.burst(c.shape, at, c.rgb, c.rgb2),
                        _ => self.parts.burst(255, at, base, base),
                    }
                    let vol = (1.0 - at.distance(self.pos) / 28.0).max(0.0) as f64;
                    let kind = if b == 0 { 1.0 } else if fx.is_some_and(|c| c.shape == 3) { 5.0 } else { 0.0 };
                    js_event("sfx", kind, vol, "");
                }
            }
            S2C::Players(v) => {
                for s in v {
                    if let Some(r) = self.remotes.iter_mut().find(|r| r.id == s.id) {
                        let p = Vec3::from(s.p);
                        r.anim = s.anim;
                        if r.seen > 1.0 {
                            r.ip.reset(self.time, p, s.yaw);
                            r.pos = p;
                            r.pet = p;
                            r.yaw = s.yaw;
                        } else {
                            r.ip.push(self.time, p, s.yaw);
                        }
                        r.seen = 0.0;
                    }
                }
            }
            S2C::Join { id, name, look } => {
                if id == self.id {
                    self.look = look;
                } else if let Some(r) = self.remotes.iter_mut().find(|r| r.id == id) {
                    r.look = look;
                } else {
                    let far = Vec3::new(0.0, -100.0, 0.0);
                    self.remotes.push(Remote { id, look, gear: [0; 8], ip: fx::Interp::new(), pos: far, yaw: 0.0, anim: 0, walk: 0.0, moving: 0.0, emote: (255, 0.0), pet: far, trail: 0.0, seen: 9.0 });
                }
                js_event("join", id as f64, look.slots[items::NAMEPLATE as usize] as f64, &name);
            }
            S2C::Leave(id) => {
                self.remotes.retain(|r| r.id != id);
                js_event("leave", id as f64, 0.0, "");
            }
            S2C::Chat { id, n } => js_event("chat", id as f64, n as f64, ""),
            S2C::Emote { id, n } => {
                if id == self.id {
                    self.emote = (n, 0.0);
                } else if let Some(r) = self.remotes.iter_mut().find(|r| r.id == id) {
                    r.emote = (n, 0.0);
                }
                js_event("emote", id as f64, n as f64, "");
            }
            S2C::Inv { item, n } => js_event("inv", item as f64, n as f64, ""),
            S2C::Plots(v) => {
                self.plots = v;
                self.here = u32::MAX;
            }
            S2C::World(st) => {
                self.world.st = st;
                js_event("world", st.regions as f64, st.projects as f64, "");
            }
            S2C::Hud(s) => js_event("hud", 0.0, 0.0, &s),
            S2C::Kick(s) => js_event("kick", 0.0, 0.0, &s),
            S2C::Said { id, ch, name, text } => js_event("said", id as f64, ch as f64, &format!("{name}\u{1f}{text}")),
            S2C::Teleport(p) => {
                self.arrive(Vec3::from(p));
                js_event("teleport", p[0] as f64, p[2] as f64, "");
            }
            S2C::Spawn { p, yaw } => {
                self.arrive(Vec3::from(p));
                self.spawn = Vec3::from(p); // nur sichere Ankunftspunkte (nicht Sprungstab-Ziele) für den eigenen Leere-Rückweg
                self.yaw = yaw;
                self.pitch = 0.0;
                js_event("teleport", p[0] as f64, p[2] as f64, "");
            }
            S2C::Npcs(v) => {
                let mut j = String::from("[");
                for (k, n) in v.iter().enumerate() {
                    if k > 0 {
                        j.push(',');
                    }
                    j.push_str(&format!("[{},", n.id));
                    fx::json_str(&mut j, &n.name);
                    j.push(']');
                }
                j.push(']');
                js_event("npcs", v.len() as f64, 0.0, &j);
                self.npcs = v;
            }
            m => self.rpg_message(m),
        }
    }

    fn physics(&mut self, dt: f32) {
        let (bx, bz) = (self.pos.x.floor() as i32, self.pos.z.floor() as i32);
        if !self.settled {
            // nach der Ankunft warten, bis der Boden geladen ist; über der Leere kommt evtl. nie etwas, daher höchstens 3 s
            self.load_wait += dt;
            if !self.world.has(bx, bz) && self.load_wait < 3.0 {
                return;
            }
            self.settled = true;
        }
        if self.demo {
            self.yaw += dt * 0.35;
            self.mz = 1.0;
            self.mx = 0.0;
        }
        let (sy, cy) = self.yaw.sin_cos();
        let mut wish = Vec3::new(-sy, 0.0, -cy) * self.mz + Vec3::new(cy, 0.0, -sy) * self.mx;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let below_b = self.world.get(bx, (self.pos.y - 0.05).floor() as i32, bz);
        let below = w::FLAGS[below_b as usize];
        let wet = w::FLAGS[self.world.get(bx, (self.pos.y + 0.4).floor() as i32, bz) as usize] & w::F_LIQUID != 0;
        let mut speed = (if self.sprint { 7.0 } else { 4.6 }) * self.speed;
        if below & w::F_SPEED != 0 {
            speed *= 1.8;
        }
        if wet {
            speed *= 0.55;
        }
        // im Sprungpolster-Bogen keine Luftsteuerung
        let k = if self.flying { 0.0 } else { (if self.ground || wet { 14.0 } else { 5.0 } * dt).min(1.0) };
        self.vel.x += (wish.x * speed - self.vel.x) * k;
        self.vel.z += (wish.z * speed - self.vel.z) * k;
        if wet {
            self.vel.y = (self.vel.y - 6.0 * dt).max(-2.5);
            if self.jump {
                self.vel.y = 3.2;
            }
        } else {
            self.vel.y -= w::GRAVITY * dt;
            if !self.flying {
                self.vel.y = self.vel.y.max(-45.0);
            }
            let pad = if self.ground && !self.flying && below_b == w::SPRUNGPAD { w::launch_pad(self.pos.x, self.pos.z, 2.0) } else { None };
            if let Some(to) = pad {
                self.vel = Vec3::from(w::launch_velocity(self.pos.to_array(), to));
                self.flying = true;
                js_event("launch", 0.0, 0.0, w::zone_name(to[0] as i32, to[1] as i32, to[2] as i32));
            } else if self.ground && !self.flying && below_b == w::SPRUNGPLATTE {
                // Sprungplatte der Insel: in Blickrichtung weit nach vorn und hoch
                self.vel = Vec3::new(-sy, 0.0, -cy) * 12.0 + Vec3::Y * 16.0;
                self.flying = true;
                js_event("sprung", 0.0, 0.0, "");
            } else if self.ground && below & w::F_BOUNCE != 0 {
                self.vel.y = 15.0;
                js_event("sfx", 3.0, 1.0, "");
            } else if self.ground && self.jump {
                self.vel.y = 8.7;
                js_event("sfx", 2.0, 1.0, "");
            }
        }
        let was_ground = self.ground;
        self.ground = false;
        let d = self.vel * dt;
        let steps = (d.abs().max_element() / 0.35).ceil().max(1.0);
        let s = d / steps;
        let mut blocked = false;
        for _ in 0..steps as i32 {
            for axis in [1usize, 0, 2] {
                let mut p = self.pos;
                p[axis] += s[axis];
                if self.world.collides(p) {
                    if axis == 1 {
                        self.ground |= s.y < 0.0;
                        self.vel.y = 0.0;
                    } else {
                        blocked = true;
                    }
                } else {
                    self.pos = p;
                }
            }
        }
        if self.ground || wet {
            self.flying = false;
        }
        // eine Stufe hoch geht von selbst (wichtig am iPad)
        if blocked && was_ground && !wet && !self.flying && wish.length_squared() > 0.05 && !self.world.collides(self.pos + Vec3::Y * 1.05 + wish.normalize() * 0.35) {
            self.vel.y = 8.7;
        }
        self.pos.x = self.pos.x.clamp(0.5, w::WORLD as f32 - 0.5);
        self.pos.z = self.pos.z.clamp(0.5, w::WORLD as f32 - 0.5);
        // ins Leere gefallen: Wind, weiter Move senden (der Server zieht Taler ab und schickt Spawn), unten anhalten
        if self.pos.y < w::VOID_Y {
            if self.void_at < 0.0 {
                self.void_at = self.time;
                self.flying = false;
                js_event("void", 0.0, 0.0, "");
            }
            if self.pos.y < w::VOID_Y - 15.0 {
                self.pos.y = w::VOID_Y - 15.0;
                self.vel = Vec3::ZERO;
            }
            if self.time - self.void_at > 3.0 {
                let p = self.spawn; // Server hat nicht geantwortet: selbst zurück
                self.arrive(p);
            }
        }
        let sp = Vec3::new(self.vel.x, 0.0, self.vel.z).length();
        self.moving += ((sp / 4.6).min(1.0) - self.moving) * (dt * 10.0).min(1.0);
        self.walk += sp * dt * 2.2;
        if self.ground && sp > 1.0 {
            self.step += sp * dt;
            if self.step > 2.3 {
                self.step = 0.0;
                js_event("step", w::material(below_b) as f64, if self.sprint { 0.6 } else { 0.4 }, "");
            }
        }
        let behind = self.pos + Vec3::new(sy + cy, 0.0, cy - sy) * 0.7;
        self.pet += (behind - self.pet) * (dt * 4.0).min(1.0);
        if self.emote.0 != 255 {
            self.emote.1 += dt;
            if self.emote.1 > emote_len(self.emote.0) || (self.emote.0 == 2 && self.moving > 0.3) {
                self.emote.0 = 255;
            }
        }
        if let Some(c) = items::cosmetic(self.look.slots[items::TRAIL as usize]) {
            self.trail += sp * dt;
            if self.trail > 0.45 && self.view != 0 {
                self.trail = 0.0;
                self.parts.trail(c, self.pos);
            }
        }
        // Warp-Portal: hineinlaufen (Füße oder Brust im Portal) reist zum Ziel aus world::PORTALS
        let (bx, bz) = (self.pos.x.floor() as i32, self.pos.z.floor() as i32);
        let inside = [0.1f32, 1.2].iter().any(|&h| self.world.get(bx, (self.pos.y + h).floor() as i32, bz) == w::PORTAL);
        if inside && self.time - self.portal_at > 3.0 {
            self.portal_at = self.time;
            if let Some(d) = w::portal_dest(bx, bz) {
                let (to, name) = if d == w::PORTAL_HOME { (Warp::Home, "Deine Insel") } else { (Warp::Spot(d), w::SPOTS.get(d as usize).map_or("Campus-Hub", |s| s.0)) };
                js_send(&proto::enc(&C2S::Warp(to)));
                js_event("portal", d as f64, 0.0, name);
            }
        }
        let zone = w::zone_name(bx, self.pos.y.floor() as i32, bz);
        if zone != self.zone {
            self.zone = zone;
            js_event("zone", 0.0, 0.0, zone);
        }
        self.send_acc += dt;
        if self.send_acc >= 0.1 {
            self.send_acc = 0.0;
            js_send(&proto::enc(&C2S::Move { p: self.pos.to_array(), yaw: self.yaw, anim: (self.moving > 0.2) as u8 }));
        }
    }

    /// Fernspieler und Wesen: aus den Schnappschüssen bei jetzt − 100 ms, Laufzyklus aus der Geschwindigkeit.
    fn remotes_step(&mut self, dt: f32) {
        let t = self.time - fx::DELAY;
        for m in &mut self.mobs {
            let (p, yaw, sp) = m.ip.at(t);
            m.pos = p;
            m.yaw = yaw;
            m.walk += sp * dt * 2.2;
            m.hurt = (m.hurt - dt / (cb::FLASH_MS as f32 / 1000.0)).max(0.0);
        }
        for r in &mut self.remotes {
            r.seen += dt;
            let (p, yaw, sp) = r.ip.at(t);
            if r.seen < 1.0 {
                r.pos = p;
                r.yaw = yaw;
            }
            r.moving += ((sp / 4.6).min(1.0).max(r.anim as f32 * 0.5) - r.moving) * (dt * 10.0).min(1.0);
            r.walk += sp.max(r.anim as f32 * 3.0) * dt * 2.2;
            let (sy, cy) = r.yaw.sin_cos();
            let behind = r.pos + Vec3::new(sy + cy, 0.0, cy - sy) * 0.7;
            r.pet += (behind - r.pet) * (dt * 4.0).min(1.0);
            if r.emote.0 != 255 {
                r.emote.1 += dt;
                if r.emote.1 > emote_len(r.emote.0) || (r.emote.0 == 2 && r.moving > 0.3) {
                    r.emote.0 = 255;
                }
            }
            if r.seen < 1.0 {
                if let Some(c) = items::cosmetic(r.look.slots[items::TRAIL as usize]) {
                    r.trail += sp * dt;
                    if r.trail > 0.45 {
                        r.trail = 0.0;
                        self.parts.trail(c, r.pos);
                    }
                }
            }
        }
    }

    /// Ein Bild: Spielschritt, sichtbare Stücke bestimmen, zeichnen. Alle Listen sind vorab angelegt und werden nur geleert.
    pub fn frame(&mut self, t_ms: f64) {
        let raw = ((t_ms - self.last_t) / 1000.0) as f32;
        self.last_t = t_ms;
        let dt = raw.clamp(0.0, 0.05);
        if raw > 0.0 && raw < 1.0 {
            self.fps += (1.0 / raw - self.fps) * 0.05;
        }
        self.time += dt;
        self.swing = (self.swing + dt * 4.0).min(1.0);
        let boost = if self.sprint && self.moving > 0.3 { 8f32.to_radians() } else { 0.0 } + if self.flying { 6f32.to_radians() } else { 0.0 };
        self.fov_boost += (boost - self.fov_boost) * (dt * 6.0).min(1.0);
        if self.ready {
            self.physics(dt);
        }
        self.remotes_step(dt);
        self.parts.update(dt);
        self.fx_step(dt);
        self.world.remesh(&self.gfx, self.pos, 3);
        (self.wx_kind, self.wx_k) = kino::fade(self.wx_kind, self.wx_next, self.wx_k, dt);

        // Kamera
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let dir = Vec3::new(-sy * cp, sp, -cy * cp);
        let head = self.pos + Vec3::Y * EYE;
        // Kamerafahrt oder Freikamera übersteuern die normale Kamera
        let mut cam = None;
        if let Some(r) = self.ride.as_mut() {
            let id = r.id;
            match r.step(dt) {
                Some(c) => cam = Some(c),
                None => {
                    self.ride = None;
                    js_event("kino", 0.0, id as f64, "");
                }
            }
        } else if let Some(f) = self.free.as_mut() {
            cam = Some(f.step(dt, head));
        }
        let busy = cam.is_some();
        // F5: 0 erste Person, 1 von hinten, 2 von vorn; die Kamera rückt vor Wänden heran
        let (eye, look) = if let Some(c) = cam {
            c
        } else if self.view == 0 {
            (head, dir)
        } else {
            let f = Vec3::new(-sy, 0.0, -cy);
            let (from, back, look, max) = if self.view == 1 { (head, -dir, dir, 4.0) } else { (self.pos + Vec3::Y * 1.25, f, -f, 3.0) };
            let mut d = max;
            let mut t = 0.5;
            while t < max {
                let q = from + back * t;
                if w::solid(self.world.get(q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32)) {
                    d = t - 0.3;
                    break;
                }
                t += 0.25;
            }
            (from + back * d, look)
        };
        let fov = (self.opts.fov + self.fov_boost).min(2.2);
        let vp = Mat4::perspective_rh(fov, self.gfx.aspect(), 0.05, 400.0) * Mat4::look_to_rh(eye, look, Vec3::Y);
        let under = w::FLAGS[self.world.get(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32) as usize] & w::F_LIQUID != 0;
        let far = if under { 26.0 } else { self.opts.view };
        let view = self.opts.view;

        // Sichtkegel: sechs Ebenen aus der Matrix, Stücke außerhalb werden nicht gezeichnet
        let (r0, r1, r2, r3) = (vp.row(0), vp.row(1), vp.row(2), vp.row(3));
        let planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        self.visible.clear();
        for i in 0..(w::WC * w::WC) as u32 {
            let c = &self.world.gpu[i as usize];
            if c.solid + c.clear == 0 {
                continue;
            }
            let lo = Vec3::new((i as i32 % w::WC * 16) as f32, 0.0, (i as i32 / w::WC * 16) as f32);
            let hi = lo + Vec3::new(16.0, w::CH as f32, 16.0);
            let (dx, dz) = (lo.x + 8.0 - eye.x, lo.z + 8.0 - eye.z);
            if dx * dx + dz * dz > (far + 12.0) * (far + 12.0) {
                continue;
            }
            let inside = planes.iter().all(|p: &Vec4| {
                let v = Vec3::new(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
                p.x * v.x + p.y * v.y + p.z * v.z + p.w >= 0.0
            });
            if inside {
                self.visible.push(i);
            }
        }
        self.visible.sort_unstable_by_key(|&i| {
            let (dx, dz) = ((i as i32 % w::WC * 16 + 8) as f32 - eye.x, (i as i32 / w::WC * 16 + 8) as f32 - eye.z);
            (dx * dx + dz * dz) as i32
        });
        // Stücke, die in die Schattenkarte fallen (Umkreis um die Figur)
        self.shadow_vis.clear();
        if self.gfx.tier() > 0 {
            let r = SHADOW_R + 24.0;
            for i in 0..(w::WC * w::WC) as u32 {
                let (dx, dz) = ((i as i32 % w::WC * 16 + 8) as f32 - self.pos.x, (i as i32 / w::WC * 16 + 8) as f32 - self.pos.z);
                if self.world.gpu[i as usize].solid > 0 && dx * dx + dz * dz < r * r {
                    self.shadow_vis.push(i);
                }
            }
        }

        // Ziel-Feld
        self.target = if self.ready && !busy { self.world.raycast(head, dir, REACH) } else { None };
        self.mine_step(dt);
        let (pk, pid, _) = if busy { (0, 0, [0; 3]) } else { self.prompt_now() };
        if (pk, pid) != self.prompt {
            self.prompt = (pk, pid);
            let name = match pk {
                1 => self.npcs.iter().find(|n| n.id as u32 == pid).map(|n| n.name.as_str()).or_else(|| sb::npc(pid as u8).map(|n| n.name)).unwrap_or(""),
                2 => w::block(pid as u8).map_or("", |d| d.name),
                3 => "Fach-Trainer",
                _ => "",
            };
            js_event("prompt", pk as f64, pid as f64, name);
        }

        // Würfel-Instanzen: Figuren, Begleiter, Partikel, Rahmen
        self.insts.clear();
        if (self.view != 0 || busy) && self.ready {
            let held = if self.tool != 0 {
                Some(sb::tool(self.tool).map_or(([150, 150, 160], false, true), |t| (t.rgb, t.mana > 0, true)))
            } else {
                (self.sel != 0).then(|| (rgb(self.sel), w::FLAGS[self.sel as usize] & w::F_GLOW != 0, false))
            };
            let mut gear = self.gear;
            gear[7] = self.weapon;
            avatar::avatar(&mut self.insts, &self.look, &Pose { pos: self.pos, yaw: self.yaw, walk: self.walk, moving: self.moving, emote: self.emote.0, emote_t: self.emote.1, time: self.time, pet: self.pet, held, gear, hurt: self.hurt });
        }
        self.plate_buf.clear();
        for r in &self.remotes {
            if r.seen > 1.0 || r.pos.distance_squared(eye) > view * view {
                continue;
            }
            avatar::avatar(&mut self.insts, &r.look, &Pose { pos: r.pos, yaw: r.yaw, walk: r.walk, moving: r.moving, emote: r.emote.0, emote_t: r.emote.1, time: self.time, pet: r.pet, held: None, gear: r.gear, hurt: 0.0 });
            let c = vp * (r.pos + Vec3::Y * 2.2).extend(1.0);
            if c.w > 0.2 && self.plate_buf.len() < PLATES * 4 {
                let (x, y) = (c.x / c.w, c.y / c.w);
                if x.abs() < 1.05 && y.abs() < 1.05 {
                    self.plate_buf.extend_from_slice(&[r.id as f32, x * 0.5 + 0.5, 0.5 - y * 0.5, c.w]);
                }
            }
        }
        if self.opts.particles {
            self.parts.draw(&mut self.insts);
        }
        // NPCs: Figuren mit Namensschild, leicht wippend
        for (k, n) in campus_shared::content::NPCS.iter().enumerate() {
            let p = Vec3::from(sb::npc_pos(n));
            if p.distance_squared(eye) > view * view {
                continue;
            }
            avatar::avatar(&mut self.insts, &self.npc_looks[k], &Pose { pos: p, yaw: n.yaw, walk: 0.0, moving: 0.0, emote: 255, emote_t: 0.0, time: self.time, pet: p, held: None, gear: [0; 8], hurt: 0.0 });
            avatar::npc_extra(&mut self.insts, n.id, p, n.yaw, self.time);
            let c = vp * (p + Vec3::Y * 2.3).extend(1.0);
            if c.w > 0.2 && self.plate_buf.len() < PLATES * 4 {
                let (x, y) = (c.x / c.w, c.y / c.w);
                if x.abs() < 1.05 && y.abs() < 1.05 {
                    self.plate_buf.extend_from_slice(&[(NPC_BASE + n.id as u32) as f32, x * 0.5 + 0.5, 0.5 - y * 0.5, c.w]);
                }
            }
        }
        // frei gesetzte NPCs der Verwaltung (Plakette NPC_BASE + 200 + n)
        for n in &self.npcs {
            let p = Vec3::from(n.p);
            if p.distance_squared(eye) > view * view {
                continue;
            }
            avatar::avatar(&mut self.insts, &npc::preset(n.look), &Pose { pos: p, yaw: n.yaw, walk: 0.0, moving: 0.0, emote: 255, emote_t: 0.0, time: self.time, pet: p, held: None, gear: [0; 8], hurt: 0.0 });
            let c = vp * (p + Vec3::Y * 2.3).extend(1.0);
            if c.w > 0.2 && self.plate_buf.len() < PLATES * 4 {
                let (x, y) = (c.x / c.w, c.y / c.w);
                if x.abs() < 1.05 && y.abs() < 1.05 {
                    self.plate_buf.extend_from_slice(&[(NPC_BASE + n.id as u32) as f32, x * 0.5 + 0.5, 0.5 - y * 0.5, c.w]);
                }
            }
        }
        // Marktbewohner: Deko-Figuren auf festen Runden um den Kristallbrunnen
        for k in 0..self.folk.len() {
            let dirn = if k % 2 == 0 { 1.0 } else { -1.0 };
            let a = self.time * 0.07 * dirn + k as f32 * 1.047;
            let r = 10.0 + (k % 3) as f32 * 3.5;
            let (x, z) = (w::MID as f32 + 0.5 + r * a.cos(), 230.5 + r * a.sin());
            if Vec3::new(x, 29.0, z).distance_squared(eye) > view * view || !self.world.has(x as i32, z as i32) {
                continue;
            }
            let y = (20..44).rev().find(|&y| w::solid(self.world.get(x as i32, y, z as i32))).map_or(29.0, |y| y as f32 + 1.0);
            let yaw = (a.cos() * dirn).atan2(a.sin() * dirn); // Blick entlang der Runde
            let p = Vec3::new(x, y, z);
            avatar::avatar(&mut self.insts, &self.folk[k], &Pose { pos: p, yaw, walk: self.time * 2.4 + k as f32, moving: 0.8, emote: 255, emote_t: 0.0, time: self.time, pet: p, held: None, gear: [0; 8], hurt: 0.0 });
        }
        self.rpg_draw(vp, eye);
        // Lern-Minions der Insel und der Schaukasten in der Minion-Werkstatt (Stufe I–XI)
        for &(p, kind, tier) in &self.minions {
            if p.distance_squared(eye) > view * view {
                continue;
            }
            let col = sb::MINIONS.get(kind as usize).map_or([150, 150, 150], |m| m.rgb);
            avatar::minion(&mut self.insts, p, col, tier, self.time);
        }
        let [shx, shy, shz] = w::SHOWCASE;
        for k in 0..11u8 {
            let p = Vec3::new((shx + 2 * k as i32) as f32 + 0.5, (shy + 1) as f32, shz as f32 + 0.5);
            if p.distance_squared(eye) < view * view {
                avatar::minion(&mut self.insts, p, sb::MINIONS[0].rgb, k + 1, self.time);
            }
        }
        if self.view == 0 && self.ready && !busy {
            self.hand(eye, dir);
        }
        // Architekten-Auswahl: 12 leuchtende Kanten um min..=max
        if let Some((a, b)) = self.selection {
            let lo = Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
            let hi = Vec3::new(b[0] as f32 + 1.0, b[1] as f32 + 1.0, b[2] as f32 + 1.0);
            let (c, h) = ((lo + hi) * 0.5, (hi - lo) * 0.5);
            const T: f32 = 0.06;
            const CYAN: [u8; 3] = [80, 235, 255];
            for (u, v) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                avatar::world_box(&mut self.insts, c + Vec3::new(0.0, u * h.y, v * h.z), Vec3::new(2.0 * h.x + T, T, T), CYAN, true);
                avatar::world_box(&mut self.insts, c + Vec3::new(u * h.x, 0.0, v * h.z), Vec3::new(T, 2.0 * h.y + T, T), CYAN, true);
                avatar::world_box(&mut self.insts, c + Vec3::new(u * h.x, v * h.y, 0.0), Vec3::new(T, T, 2.0 * h.z + T), CYAN, true);
            }
        }
        // Zielblock: schwarzer Drahtrahmen, beim Abbauen dazu der Pixel-Riss (beides im Shader, je eine Instanz)
        if let Some((hit, _)) = self.target {
            let c = Vec3::new(hit[0] as f32 + 0.5, hit[1] as f32 + 0.5, hit[2] as f32 + 0.5);
            avatar::push_a(&mut self.insts, Mat4::from_translation(c) * Mat4::from_scale(Vec3::splat(1.006)), [0, 0, 0], 9);
            if self.dig == Some(hit) && self.dig_p > 0.0 {
                let stage = ((self.dig_p * 8.0) as u8).min(7) + 1;
                avatar::push_a(&mut self.insts, Mat4::from_translation(c) * Mat4::from_scale(Vec3::splat(1.004)), [0, 0, 0], stage);
            }
        }
        let mut here = 0u32;
        for p in &self.plots {
            let r = w::plot_rect(p.slot, p.size);
            let (x0, z0, n) = (r.0 as f32, r.1 as f32, r.2 as f32);
            if w::in_rect(r, self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
                here = p.id;
            }
            let mid = Vec3::new(x0 + n / 2.0, w::GROUND as f32 + 1.0, z0 + n / 2.0);
            if mid.distance_squared(self.pos) > 90.0 * 90.0 {
                continue;
            }
            let col = if p.may { [120, 230, 140] } else { [235, 235, 240] };
            let y = w::GROUND as f32 + 1.04;
            for (cx, cz, sx, sz) in [(mid.x, z0, n, 0.08), (mid.x, z0 + n, n, 0.08), (x0, mid.z, 0.08, n), (x0 + n, mid.z, 0.08, n)] {
                avatar::world_box(&mut self.insts, Vec3::new(cx, y, cz), Vec3::new(sx, 0.05, sz), col, true);
            }
            for (cx, cz) in [(x0, z0), (x0 + n, z0), (x0, z0 + n), (x0 + n, z0 + n)] {
                avatar::world_box(&mut self.insts, Vec3::new(cx, y + 0.6, cz), Vec3::new(0.14, 1.2, 0.14), col, true);
            }
        }
        if self.opts.clouds && !under {
            self.clouds(eye);
        }
        if here != self.here {
            self.here = here;
            match self.plots.iter().find(|p| p.id == here) {
                Some(p) => js_event("here", p.id as f64, p.may as u8 as f64, &p.name),
                None => js_event("here", 0.0, 0.0, ""),
            }
        }
        js_plates(&self.plate_buf);
        js_numbers(&self.num_buf);

        self.fill_env(vp, eye, under, far);
        self.quads = self.gfx.render(&mut self.env, &self.world.gpu, &self.visible, &self.shadow_vis, &self.insts);
    }
}

impl Game {
    /// Tageszeit, Himmel, Licht, Schattenmatrix und Punktlichter für die Uniform.
    fn fill_env(&mut self, vp: Mat4, eye: Vec3, under: bool, far: f32) {
        // 20 Minuten je Tag, aus der Uhrzeit (alle sehen dasselbe) oder fest gewählt
        let phase = match self.sky_mode {
            1 => 0.14,
            2 => 0.485,
            3 => 0.75,
            _ => (js_sys::Date::now() / 1000.0 % DAY_SECS / DAY_SECS) as f32,
        };
        let a = phase * std::f32::consts::TAU;
        let sun = Vec3::new(a.cos(), a.sin(), 0.35).normalize();
        let e = sun.y;
        let day = smooth(-0.10, 0.22, e);
        let dusk = (-((e - 0.06) / 0.13).powi(2)).exp();
        let lin = |c: [f32; 3]| Vec3::new(c[0].powf(2.2), c[1].powf(2.2), c[2].powf(2.2));
        let mut zen = lin([0.02, 0.03, 0.08]).lerp(lin([0.27, 0.50, 0.92]), day).lerp(lin([0.30, 0.30, 0.58]), dusk * 0.55);
        let mut hor = lin([0.07, 0.09, 0.18]).lerp(lin([0.70, 0.84, 0.97]), day).lerp(lin([1.0, 0.58, 0.32]), dusk * 0.8);
        let amb = Vec3::new(0.032, 0.04, 0.08).lerp(Vec3::new(0.14, 0.175, 0.24), day).lerp(Vec3::new(0.14, 0.10, 0.10), dusk * 0.5);
        let gnd = amb * Vec3::new(0.5, 0.45, 0.36);
        // Hauptlicht: Sonne über dem Horizont, sonst der Mond gegenüber
        let (ldir, lcol, strength) = if e > -0.04 {
            let c = Vec3::new(1.0, 0.42, 0.13).lerp(Vec3::new(1.0, 0.95, 0.86), smooth(0.0, 0.35, e));
            (sun, c * 0.9 * smooth(-0.04, 0.08, e), 1.0)
        } else {
            (-sun, Vec3::new(0.32, 0.42, 0.75) * 0.32 * smooth(0.04, 0.2, -e), 0.7)
        };
        if under {
            hor = lin([0.16, 0.36, 0.56]);
            zen = hor;
        }
        // Schattenkarte: ortho um die Figur, auf Texel gerastert (kein Flimmern beim Gehen)
        let up = if ldir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let lv = Mat4::look_to_rh(Vec3::ZERO, -ldir, up);
        let c = lv.transform_point3(self.pos);
        let texel = 2.0 * SHADOW_R / if self.gfx.tier() <= 1 { 1024.0 } else { 2048.0 };
        let (cx, cy) = ((c.x / texel).round() * texel, (c.y / texel).round() * texel);
        let lvp = Mat4::orthographic_rh(cx - SHADOW_R, cx + SHADOW_R, cy - SHADOW_R, cy + SHADOW_R, -c.z - 200.0, -c.z + 200.0) * lv;
        if self.time - self.light_at > 0.15 || self.light_at < 0.0 {
            self.light_at = self.time;
            self.gather_lights(eye);
        }
        let g = &mut self.env;
        g.vp = vp.to_cols_array_2d();
        g.ivp = vp.inverse().to_cols_array_2d();
        g.lvp = lvp.to_cols_array_2d();
        g.cam = [eye.x, eye.y, eye.z, self.time];
        g.sun = [ldir.x, ldir.y, ldir.z, strength];
        g.sun_col = [lcol.x, lcol.y, lcol.z, 1.0 + 1.2 * (1.0 - day)];
        g.sky_dir = [sun.x, sun.y, sun.z, day];
        g.zen = [zen.x, zen.y, zen.z, smooth(0.05, -0.2, e)];
        g.hor = [hor.x, hor.y, hor.z, far];
        g.amb = [amb.x, amb.y, amb.z, dusk];
        g.gnd = [gnd.x, gnd.y, gnd.z, g.gnd[3]];
        g.par = [0.0, self.opts.ao as u8 as f32, under as u8 as f32, 0.0];
        g.wx = [self.wx_kind as f32, if self.opts.particles { self.wx_k } else { 0.0 }, 0.0, 0.0];
        g.post = [self.filter as f32, self.focal, 0.0, 0.0];
    }

    /// Farbige Punktlichter: leuchtende Blöcke in der Nähe, nahe beieinander zu einem Licht zusammengefasst.
    fn gather_lights(&mut self, eye: Vec3) {
        let max = match self.gfx.tier() {
            0 => 8,
            1 => 16,
            _ => gfx::MAX_LIGHTS,
        };
        let mut cand: Vec<(f32, Vec3, u8)> = Vec::new();
        let (ecx, ecz) = (eye.x.floor() as i32 >> 4, eye.z.floor() as i32 >> 4);
        for cz in (ecz - 3).max(0)..=(ecz + 3).min(w::WC - 1) {
            for cx in (ecx - 3).max(0)..=(ecx + 3).min(w::WC - 1) {
                for &[x, y, z, b] in &self.world.glow[(cz * w::WC + cx) as usize] {
                    let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    let d2 = p.distance_squared(eye);
                    if d2 < 40.0 * 40.0 {
                        cand.push((d2, p, b as u8));
                    }
                }
            }
        }
        cand.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        // je Licht: Summe der Orte, Anzahl, Summe der Farben, Reichweite
        let mut ls: Vec<(Vec3, f32, Vec3, f32)> = Vec::with_capacity(max);
        for (_, p, b) in cand {
            let (col, rad) = match b {
                w::LATERNE => (Vec3::new(1.4, 0.85, 0.3), 9.0),
                w::PORTAL => (Vec3::new(0.9, 0.25, 1.6), 8.0),
                w::LAVA | w::GLUT => (Vec3::new(1.5, 0.55, 0.15), 7.0),
                _ => {
                    let c = rgb(b);
                    (Vec3::new((c[0] as f32 / 255.0).powf(2.2), (c[1] as f32 / 255.0).powf(2.2), (c[2] as f32 / 255.0).powf(2.2)) * 1.2, 6.5)
                }
            };
            if let Some(l) = ls.iter_mut().find(|l| (l.0 / l.1).distance_squared(p) < 2.5 * 2.5) {
                l.0 += p;
                l.1 += 1.0;
                l.2 += col;
                l.3 = l.3.max(rad);
            } else if ls.len() < max {
                ls.push((p, 1.0, col, rad));
            }
        }
        let g = &mut self.env;
        for (k, l) in ls.iter().enumerate() {
            let p = l.0 / l.1;
            let c = l.2 / l.1 * l.1.sqrt().min(2.5);
            g.lpos[k] = [p.x, p.y, p.z, l.3 + l.1.sqrt().min(3.0)];
            g.lcol[k] = [c.x, c.y, c.z, 0.0];
        }
        g.gnd[3] = ls.len() as f32;
    }

    /// Flache Würfelwolken, langsam treibend: dichte Schicht unter den Inselbäuchen (y -12 bis -6), wenige darüber.
    fn clouds(&mut self, eye: Vec3) {
        const CELL: f32 = 12.0;
        let r = (self.opts.view / CELL) as i32 + 2;
        for (y, keep, salt, drift, col) in [(-12.0f32, 40u32, 0u32, self.time * 0.8, [236, 240, 248]), (w::GROUND as f32 + 28.0, 15, 7, self.time * 1.2, [250, 251, 255])] {
            let (ci, cj) = (((eye.x - drift) / CELL).floor() as i32, (eye.z / CELL).floor() as i32);
            for j in cj - r..=cj + r {
                for i in ci - r..=ci + r {
                    let h = (i as u32).wrapping_mul(0x9E37_79B1) ^ (j as u32).wrapping_mul(0x85EB_CA77) ^ salt.wrapping_mul(0xC2B2_AE3D);
                    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
                    if h % 100 >= keep || self.insts.len() >= gfx::MAX_INST - 64 {
                        continue;
                    }
                    let sx = CELL * (0.7 + (h >> 8 & 7) as f32 * 0.06);
                    let sz = CELL * (0.6 + (h >> 12 & 7) as f32 * 0.07);
                    let at = Vec3::new(i as f32 * CELL + drift, y + (h >> 16 & 3) as f32 * 2.0, j as f32 * CELL);
                    avatar::world_box(&mut self.insts, at, Vec3::new(sx, 2.0, sz), col, false);
                }
            }
        }
    }

    /// Rechter Arm mit gehaltenem Block in der ersten Person: wippt beim Laufen, schwingt beim Bauen.
    fn hand(&mut self, eye: Vec3, dir: Vec3) {
        let right = dir.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(dir);
        let bob = (self.walk * 2.0).sin() * 0.03 * self.moving;
        let sway = self.walk.sin() * 0.02 * self.moving;
        let sw = (self.swing * std::f32::consts::PI).sin();
        let base = eye + dir * (0.55 - sw * 0.12) + right * (0.36 + sway - sw * 0.1) - up * (0.34 - bob.abs() - sw * 0.08);
        let basis = Mat4::from_cols(right.extend(0.0), up.extend(0.0), (-dir).extend(0.0), base.extend(1.0));
        let arm = basis * Mat4::from_rotation_x(-0.9 - sw * 0.9) * Mat4::from_rotation_y(0.25);
        // getroffen: Arm blitzt rot auf
        let k = self.hurt * 0.7;
        let red = |c: [u8; 3]| [(c[0] as f32 + (235.0 - c[0] as f32) * k) as u8, (c[1] as f32 * (1.0 - k)) as u8, (c[2] as f32 * (1.0 - k)) as u8];
        let skin = red(items::SKINS[(self.look.skin as usize) % items::SKINS.len()]);
        let sleeve = red(items::PALETTE[(self.look.col_a as usize) % items::PALETTE.len()]);
        let at = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
        let size = |x: f32, y: f32, z: f32| Mat4::from_scale(Vec3::new(x, y, z));
        // Ärmel in Jackenfarbe, dann die Hand
        avatar::push(&mut self.insts, arm * at(0.0, 0.0, -0.13) * size(0.15, 0.15, 0.32), sleeve, false);
        avatar::push(&mut self.insts, arm * at(0.0, 0.0, -0.38) * size(0.13, 0.13, 0.18), skin, false);
        if self.weapon != 0 {
            // Waffe: Griff in der Hand, beim Schlag ein Bogen von rechts oben nach links unten
            let grip = arm * at(0.0, 0.05, -0.46) * Mat4::from_rotation_z(sw * 1.2) * Mat4::from_rotation_x(0.9 - sw * 0.7);
            deco::weapon(&mut self.insts, grip, self.weapon);
        } else if self.tool != 0 {
            // Werkzeug: Stiel, Kopf je nach Art in der Werkzeugfarbe
            let (col, glow) = sb::tool(self.tool).map_or(([150, 150, 160], false), |t| (t.rgb, t.mana > 0));
            let grip = arm * at(0.0, 0.05, -0.48) * Mat4::from_rotation_x(0.9);
            avatar::push(&mut self.insts, grip * at(0.0, 0.18, 0.0) * size(0.05, 0.55, 0.05), [120, 86, 54], false);
            let top = grip * at(0.0, 0.44, 0.0);
            match self.tool {
                w::SPITZHACKE => {
                    avatar::push(&mut self.insts, top * size(0.1, 0.1, 0.1), col, false);
                    for s in [-1.0f32, 1.0] {
                        avatar::push(&mut self.insts, top * at(s * 0.15, -0.03, 0.0) * Mat4::from_rotation_z(s * 0.3) * size(0.22, 0.06, 0.07), col, false);
                    }
                }
                w::AXT => avatar::push(&mut self.insts, top * at(-0.09, -0.04, 0.0) * size(0.14, 0.2, 0.04), col, false),
                sb::HAMMER => avatar::push(&mut self.insts, top * size(0.28, 0.12, 0.12), col, false),
                _ => avatar::push(&mut self.insts, top * size(0.1, 0.1, 0.1), col, glow),
            }
        } else if self.sel != 0 {
            avatar::push(&mut self.insts, arm * Mat4::from_translation(Vec3::new(0.0, 0.1, -0.5)) * Mat4::from_scale(Vec3::splat(0.2)), rgb(self.sel), w::FLAGS[self.sel as usize] & w::F_GLOW != 0);
        }
    }
}

fn sb_npc_looks() -> Vec<Look> {
    // Kleidung je Rolle: Robe (Dekan, Bibliothekarin), Weste (Bankier, Händler, Auktionator), Kapuze (Förster), sonst Hemd
    const OUTFIT: [u8; 11] = [0, 2, 3, 2, 0, 3, 0, 0, 3, 1, 0];
    campus_shared::content::NPCS
        .iter()
        .map(|n| Look {
            skin: n.id % 6,
            hair: n.id % 4,
            hair_col: n.id % 5,
            outfit: OUTFIT.get(n.id as usize).copied().unwrap_or(0),
            col_a: palette_index(n.rgb[0]),
            col_b: palette_index(n.rgb[1]),
            ..Look::default()
        })
        .collect()
}
/// Sechs Marktbewohner (nur Deko, ohne Server).
fn folk_looks() -> Vec<Look> {
    let n = items::PALETTE.len() as u8;
    (0..6u8).map(|k| Look { skin: (k * 5 + 1) % 6, hair: k % 4, hair_col: (k * 3 + 1) % n, outfit: k % 4, col_a: (k * 5 + 2) % n, col_b: (k * 7 + 4) % n, ..Look::default() }).collect()
}

impl Game {
    fn look_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }
    /// Was [E] gerade täte: (0 nichts | 1 NPC | 2 Block, NPC- bzw. Block-Id, Blockstelle).
    fn prompt_now(&self) -> (u8, u32, [i32; 3]) {
        if let Some(n) = self.npc_in_sight() {
            return (1, n as u32, [0; 3]);
        }
        if let Some((hit, _)) = self.target {
            let b = self.world.get(hit[0], hit[1], hit[2]);
            if matches!(b, w::TRUHE | w::SCHATZ | w::WERKBANK | w::QUESTBRETT | w::GROSSE_TRUHE | w::ALTAR | w::BOSS_ALTAR | w::STAENDER | w::SCHILD) || w::USE_NEW.contains(&b) {
                return (2, b as u32, hit);
            }
            // Fach-Trainer im Bibliotheks-Turm: Sockel (oder der Block darauf) im Ring um die Turmmitte 393/240
            let s = if b == w::SOCKEL { hit } else { [hit[0], hit[1] - 1, hit[2]] };
            if self.world.get(s[0], s[1], s[2]) == w::SOCKEL && (s[0] - 393).pow(2) + (s[2] - 240).pow(2) <= 121 {
                return (3, 0, s);
            }
        }
        (0, 0, [0; 3])
    }

    /// Abbauen, solange die Taste gehalten wird: Fortschritt nach world::mine_secs, Schläge im Takt, bei 100 % zerbricht der Block.
    fn mine_step(&mut self, dt: f32) {
        let tgt = if self.mining && self.ready { self.target } else { None };
        let Some((hit, prev)) = tgt else { return self.dig_stop() };
        let b = self.world.get(hit[0], hit[1], hit[2]);
        if self.dig != Some(hit) || b != self.dig_b {
            self.dig_stop();
            self.dig = Some(hit);
            self.dig_b = b;
            self.dig_hit = 0.0;
            self.dig_ok = false;
            if !w::hardness(b).is_finite() {
                return;
            }
            if !self.may_edit(hit[0], hit[2], b, false) {
                if self.time - self.nein_at < 2.0 {
                    return; // nicht bei jedem Block neu meckern
                }
                self.nein_at = self.time;
                let why = if w::in_hub(hit[0], hit[2]) { "Im Hub kannst du nur Erze, Kristalladern und Holzknoten abbauen." } else { "Hier darfst du nicht abbauen." };
                return js_event("nein", 0.0, 0.0, why);
            }
            self.dig_ok = true;
            js_send(&proto::enc(&C2S::Dig { x: hit[0] as i16, y: hit[1] as i16, z: hit[2] as i16 }));
        }
        if !self.dig_ok {
            return;
        }
        self.dig_p += dt / w::mine_secs_speed(b, self.tool, self.bau, self.mine_speed);
        let m = w::material(b);
        let center = Vec3::new(hit[0] as f32 + 0.5, hit[1] as f32 + 0.5, hit[2] as f32 + 0.5);
        let n = Vec3::new((prev[0] - hit[0]) as f32, (prev[1] - hit[1]) as f32, (prev[2] - hit[2]) as f32);
        self.dig_hit -= dt;
        if self.dig_hit <= 0.0 && self.dig_p < 1.0 {
            self.dig_hit = 0.25;
            self.swing = 0.0;
            js_event("hit", ((self.dig_p * 8.0) as i32).min(7) as f64, m as f64, "");
            if self.opts.particles {
                self.parts.chips(center + n * 0.5, n, rgb(b), 3);
            }
        }
        let q = (self.dig_p * 16.0) as i32;
        if q != self.dig_q {
            self.dig_q = q;
            js_event("mine", self.dig_p.min(1.0) as f64, b as f64, "");
        }
        if self.dig_p >= 1.0 {
            // vorhersagen: nachwachsende Blöcke werden zu ABGEBAUT, alles andere verschwindet
            let new = if w::FLAGS[b as usize] & w::F_REGEN != 0 { w::ABGEBAUT } else { 0 };
            self.world.set(hit[0], hit[1], hit[2], new);
            js_send(&proto::enc(&C2S::Block { x: hit[0] as i16, y: hit[1] as i16, z: hit[2] as i16, b: 0 }));
            js_event("break", m as f64, b as f64, "");
            self.parts.burst(255, center, rgb(b), rgb(b));
            if self.opts.particles {
                self.parts.chips(center + n * 0.5, n, rgb(b), 10);
            }
            // Beute-Würfelchen fliegen zur Figur (mit Telekinese landet alles direkt im Inventar)
            if !self.telekinese && b != w::ABGEBAUT {
                let item = match farming::crop_of(b) {
                    Some(c) if self.crop_stage(hit) == Some(3) => rpg::crop(c).map_or(b as u16, |d| d.product),
                    _ => w::regen_drop(b, 1).unwrap_or(b as u16),
                };
                let parts = &mut self.parts;
                self.drops.spawn(center, item, &mut || parts.rnd());
            }
            self.swing = 0.0;
            self.dig = None;
            self.dig_p = 0.0;
            self.dig_q = 0;
        }
    }
    fn dig_stop(&mut self) {
        if self.dig.is_some() && self.dig_p > 0.0 {
            js_event("mine", -1.0, self.dig_b as f64, "");
        }
        self.dig = None;
        self.dig_p = 0.0;
        self.dig_q = 0;
    }

    /// Ankunft (Anmeldung, Warp, Portal, Leere, Sprungstab): Stand, Ruhe, Abbau und Fall zurücksetzen.
    fn arrive(&mut self, p: Vec3) {
        self.pos = p;
        self.vel = Vec3::ZERO;
        self.pet = p;
        self.void_at = -1.0;
        self.flying = false;
        self.settled = false;
        self.load_wait = 0.0;
        self.dig_stop();
    }

    /// Kamerafahrt oder Freikamera läuft: Spiel-Eingaben ruhen.
    fn cam_busy(&self) -> bool {
        self.ride.is_some() || self.free.is_some()
    }
    /// Figur anhalten (Fahrt/Freikamera beginnt).
    fn halt(&mut self) {
        self.mx = 0.0;
        self.mz = 0.0;
        self.jump = false;
        self.sprint = false;
        self.mining = false;
        self.attacking = false;
        self.dig_stop();
    }
    /// Stützpunkt der Fahrt: nur über geladenen Stücken (sonst zur Figur heranziehen) und nicht in der Wand (sonst anheben).
    fn ride_point(&self, p: Vec3) -> Vec3 {
        let me = self.pos + Vec3::Y * EYE;
        let mut q = p;
        for k in 1..=4 {
            if self.world.has(q.x.floor() as i32, q.z.floor() as i32) {
                break;
            }
            q = p.lerp(me, k as f32 * 0.25);
        }
        for _ in 0..12 {
            if !w::solid(self.world.get(q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32)) {
                break;
            }
            q.y += 1.0;
        }
        q
    }
    /// Architekten-Stab: angeklickten Block als Pos1 (op 1) bzw. Pos2 (op 2) melden.
    fn arch_click(&mut self, op: u8) {
        if let Some((hit, _)) = self.target {
            self.swing = 0.0;
            js_send(&proto::enc(&C2S::Arch { op, x: hit[0] as i16, y: hit[1] as i16, z: hit[2] as i16 }));
        }
    }

    /// NPC im Fadenkreuz (bis TALK Blöcke, Abstand Strahl–Körperachse unter 0,7); feste und frei gesetzte (id ≥ 200).
    fn npc_in_sight(&self) -> Option<u8> {
        let (eye, d) = (self.pos + Vec3::Y * EYE, self.look_dir());
        let block = self.target.map_or(f32::MAX, |(h, _)| Vec3::new(h[0] as f32 + 0.5, h[1] as f32 + 0.5, h[2] as f32 + 0.5).distance(eye));
        let fixed = campus_shared::content::NPCS.iter().map(|n| (Vec3::from(sb::npc_pos(n)), n.id));
        let free = self.npcs.iter().map(|n| (Vec3::from(n.p), n.id));
        fixed
            .chain(free)
            .filter_map(|(p, id)| {
                let c = p + Vec3::Y;
                let t = (c - eye).dot(d);
                let miss = (eye + d * t - c).length();
                (t > 0.0 && t < TALK && t < block && miss < 0.7).then_some((t, id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1)
    }
}

/// Phasen 16–20: Kampf, Wesen, Insel-Inhalt, Beute.
impl Game {
    fn rpg_message(&mut self, m: S2C) {
        match m {
            S2C::Mobs(v) => {
                self.mobs.retain(|m| v.iter().any(|s| s.id == m.st.id));
                for s in v {
                    let p = Vec3::from(s.p);
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.st.id == s.id) {
                        m.ip.push(self.time, p, s.yaw);
                        m.st = s;
                    } else {
                        self.mobs.push(fx::Mob::new(s, self.time));
                    }
                }
                for m in &mut self.mobs {
                    if m.st.fl & MOB_HURT != 0 {
                        m.hurt = 1.0;
                    }
                    if m.st.fl & MOB_DEAD != 0 && !m.dead {
                        m.dead = true;
                        let c = rpg::mob(m.st.kind).map_or([200, 200, 200], |d| d.rgb);
                        self.parts.burst(255, m.pos + Vec3::Y * 0.6, c, c);
                    }
                }
            }
            S2C::Damage { at, n, crit, mob } => {
                self.numbers.add(Vec3::from(at), n as f32, crit);
                if mob == 0 {
                    self.hurt = 1.0;
                    js_event("treffer", crit as u8 as f64, 1.0, "");
                } else if self.time - self.atk_at < 1.0 {
                    js_event("treffer", crit as u8 as f64, 0.0, "");
                }
            }
            S2C::Gear { id, items } => {
                if id == self.id {
                    self.gear = items;
                } else if let Some(r) = self.remotes.iter_mut().find(|r| r.id == id) {
                    r.gear = items;
                }
            }
            S2C::Island { now, crops, signs, stands } => {
                self.isl_now = now;
                self.isl_recv = self.time;
                let mut j = String::from("[");
                for (k, s) in signs.iter().enumerate() {
                    if k > 0 {
                        j.push(',');
                    }
                    j.push_str(&format!("[{},{},{},", s.x, s.y, s.z));
                    fx::json_str(&mut j, &s.lines.join("\n"));
                    j.push(']');
                }
                j.push(']');
                js_event("schilder", signs.len() as f64, 0.0, &j);
                self.crops = crops;
                self.signs = signs;
                self.stands = stands;
            }
            S2C::Health { hp, max, shield_ms } => js_event("hp", hp as f64, max as f64, &shield_ms.to_string()),
            S2C::Shock { at, r } => {
                self.waves.shock(Vec3::from(at), r);
                js_event("schock", 0.0, 0.0, "");
            }
            S2C::Shot { from, to, kind } => self.waves.shot(Vec3::from(from), Vec3::from(to), kind),
            S2C::Drops { at, items } => {
                let at = Vec3::from(at);
                let parts = &mut self.parts;
                for item in items.into_iter().take(16) {
                    self.drops.spawn(at, item, &mut || parts.rnd());
                }
            }
            _ => {}
        }
    }

    /// Effekte weiterrechnen, gehaltenen Angriff wiederholen.
    fn fx_step(&mut self, dt: f32) {
        self.hurt = (self.hurt - dt / (cb::FLASH_MS as f32 / 1000.0)).max(0.0);
        self.numbers.update(dt);
        self.waves.update(dt);
        let world = &self.world;
        let got = self.drops.update(dt, self.pos, |p| w::solid(world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)));
        if got > 0 {
            js_event("pop", 0.0, 0.0, "");
        }
        if self.attacking && self.ready {
            self.strike();
        }
    }

    /// Schlag (Nahkampf) oder Schuss (Fernwaffe), höchstens im Takt der Abklingzeit.
    fn strike(&mut self) {
        let ranged = rpg::is_ranged(self.weapon);
        // kleiner Spielraum, damit der Server den Takt trotz Netz-Schwankung annimmt
        let cd_ms = if ranged { cb::SHOT_CD_MS } else { cb::SWING_CD_MS };
        if self.time - self.atk_at < (cd_ms + 30) as f32 / 1000.0 {
            return;
        }
        self.atk_at = self.time;
        self.swing = 0.0;
        js_event("schwung", 0.0, 0.0, "");
        if ranged {
            js_send(&proto::enc(&C2S::Shoot { dir: self.look_dir().to_array() }));
        } else if let Some(mob) = self.mob_in_sight() {
            js_send(&proto::enc(&C2S::Attack { mob }));
        }
    }

    /// Wesen im Fadenkreuz bis MELEE_REACH (+ halbe Größe), vor dem anvisierten Block.
    fn mob_in_sight(&self) -> Option<u32> {
        let (eye, d) = (self.pos + Vec3::Y * EYE, self.look_dir());
        let block = self.target.map_or(f32::MAX, |(h, _)| Vec3::new(h[0] as f32 + 0.5, h[1] as f32 + 0.5, h[2] as f32 + 0.5).distance(eye));
        self.mobs
            .iter()
            .filter(|m| !m.dead)
            .filter_map(|m| {
                let size = rpg::mob(m.st.kind).map_or(1.0, |d| d.size);
                let c = m.pos + Vec3::Y * 0.9 * size;
                let t = cb::ray_sphere(eye.to_array(), d.to_array(), c.to_array(), 0.6 * size, cb::MELEE_REACH + size / 2.0)?;
                (t < block).then_some((t, m.st.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1)
    }

    /// Serverzeit in ms (aus S2C::Island plus vergangener lokaler Zeit).
    fn server_now(&self) -> i64 {
        self.isl_now + ((self.time - self.isl_recv) * 1000.0) as i64
    }
    /// Pflanzen-Index (in CROPS) einer CropState; der Server schickt Index oder Saat-Block.
    fn crop_index(kind: u8) -> Option<usize> {
        if (kind as usize) < w::SAATEN.len() { Some(kind as usize) } else { farming::crop_of(kind) }
    }
    /// Wachstumsstufe der Pflanze an dieser Stelle.
    fn crop_stage(&self, at: [i32; 3]) -> Option<u8> {
        let now = self.server_now();
        self.crops
            .iter()
            .find(|c| [c.x as i32, c.y as i32, c.z as i32] == at)
            .map(|c| farming::stage(c.planted, now, c.grow_ms as i64))
    }

    /// Wesen, Lebensbalken, Pflanzen, Ständer, Schilder-Plaketten, Beute, Stoßwellen, Geschosse, Schadenszahlen.
    fn rpg_draw(&mut self, vp: Mat4, eye: Vec3) {
        let view = self.opts.view;
        for m in &self.mobs {
            if m.pos.distance_squared(eye) > view * view {
                continue;
            }
            let Some(def) = rpg::mob(m.st.kind) else { continue };
            deco::mob(&mut self.insts, m.st.kind, m.pos, m.yaw, m.walk, m.hurt, m.st.fl, self.time);
            let boss = def.ring == 2 || m.st.kind == 7; // Archon und Namenloser Prüfer
            if !m.dead && (boss || m.st.hp < 1000) {
                let top = m.pos + Vec3::Y * (1.9 * def.size + 0.35);
                deco::health_bar(&mut self.insts, top, m.st.hp as f32 / 1000.0, 0.9 * def.size.max(1.0));
            }
        }
        let now = self.server_now();
        for c in &self.crops {
            let (x, y, z) = (c.x as i32, c.y as i32, c.z as i32);
            let base = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
            if base.distance_squared(eye) > view * view || farming::crop_of(self.world.get(x, y, z)).is_none() {
                continue; // weit weg oder eben abgeerntet
            }
            let Some(kind) = Self::crop_index(c.kind) else { continue };
            deco::crop(&mut self.insts, base, kind, farming::stage(c.planted, now, c.grow_ms as i64), self.time);
        }
        for s in &self.stands {
            let p = Vec3::new(s.x as f32 + 0.5, s.y as f32 + 1.0, s.z as f32 + 0.5);
            if p.distance_squared(eye) < view * view {
                deco::stand(&mut self.insts, p, &s.items, self.time);
            }
        }
        // Schilder: Plakette über dem Schild, nur bis 10 Blöcke
        for (k, s) in self.signs.iter().enumerate() {
            let p = Vec3::new(s.x as f32 + 0.5, s.y as f32 + 1.3, s.z as f32 + 0.5);
            if p.distance_squared(self.pos) > 10.0 * 10.0 || self.plate_buf.len() >= PLATES * 4 {
                continue;
            }
            let c = vp * p.extend(1.0);
            if c.w > 0.2 {
                let (x, y) = (c.x / c.w, c.y / c.w);
                if x.abs() < 1.05 && y.abs() < 1.05 {
                    self.plate_buf.extend_from_slice(&[(SIGN_BASE + k as u32) as f32, x * 0.5 + 0.5, 0.5 - y * 0.5, c.w]);
                }
            }
        }
        self.drops.draw(&mut self.insts, self.time);
        self.waves.draw(&mut self.insts);
        self.numbers.project(vp, &mut self.num_buf);
    }
}
