//! Kleine, eigenständige Effekte für lib.rs: Schnappschuss-Interpolation, Schadenszahlen, Beute-Magnet, Stoßwellen, Geschosse.
//! Alles in festen Listen, die nur geleert und wieder gefüllt werden (kein Anlegen im Bild).
use crate::deco;
use crate::gfx::Inst;
use campus_shared::combat::SHOCK_SPEED;
use campus_shared::proto::MobState;
use glam::{Mat4, Vec3};
use std::f32::consts::{PI, TAU};

/// So weit hinter der Gegenwart wird gezeichnet (Sekunden).
pub const DELAY: f32 = 0.1;
const SNAPS: usize = 8;

/// Kürzester Winkel von a nach b (−π..π).
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    d
}

#[derive(Clone, Copy, Default)]
struct Snap {
    t: f32,
    p: Vec3,
    yaw: f32,
}

/// Ringpuffer der letzten Schnappschüsse (Zeit = lokale Ankunft) einer Figur oder eines Wesens.
#[derive(Clone)]
pub struct Interp {
    s: [Snap; SNAPS],
    n: usize,
}

impl Interp {
    pub fn new() -> Interp {
        Interp { s: [Snap::default(); SNAPS], n: 0 }
    }
    /// Neuer Schnappschuss; ein Sprung über 10 Blöcke (Warp) beginnt den Puffer neu.
    pub fn push(&mut self, t: f32, p: Vec3, yaw: f32) {
        if self.n > 0 && self.s[self.n - 1].p.distance_squared(p) > 100.0 {
            self.n = 0;
        }
        if self.n == SNAPS {
            self.s.copy_within(1.., 0);
            self.n -= 1;
        }
        self.s[self.n] = Snap { t, p, yaw };
        self.n += 1;
    }
    pub fn reset(&mut self, t: f32, p: Vec3, yaw: f32) {
        self.n = 0;
        self.push(t, p, yaw);
    }
    /// Lage zur Zeit t: (Position linear, Gier über den kürzesten Winkel, waagrechte Geschwindigkeit). Nach dem letzten
    /// Schnappschuss bleibt die Figur stehen (keine Vorhersage).
    pub fn at(&self, t: f32) -> (Vec3, f32, f32) {
        if self.n == 0 {
            return (Vec3::ZERO, 0.0, 0.0);
        }
        let (first, last) = (self.s[0], self.s[self.n - 1]);
        if t <= first.t {
            return (first.p, first.yaw, 0.0);
        }
        if t >= last.t {
            return (last.p, last.yaw, 0.0);
        }
        for i in 1..self.n {
            let (a, b) = (self.s[i - 1], self.s[i]);
            if t <= b.t {
                let span = (b.t - a.t).max(1e-3);
                let k = ((t - a.t) / span).clamp(0.0, 1.0);
                let d = b.p - a.p;
                let sp = Vec3::new(d.x, 0.0, d.z).length() / span;
                return (a.p.lerp(b.p, k), a.yaw + angle_diff(a.yaw, b.yaw) * k, sp);
            }
        }
        (last.p, last.yaw, 0.0)
    }
}

/// Ein Wesen der Zitadelle, wie es gezeichnet wird.
pub struct Mob {
    pub st: MobState,
    pub ip: Interp,
    pub pos: Vec3,
    pub yaw: f32,
    pub walk: f32,
    pub hurt: f32,
    pub dead: bool,
}

impl Mob {
    pub fn new(st: MobState, t: f32) -> Mob {
        let p = Vec3::from(st.p);
        let mut ip = Interp::new();
        ip.push(t, p, st.yaw);
        Mob { st, ip, pos: p, yaw: st.yaw, walk: 0.0, hurt: 0.0, dead: false }
    }
}

/// Schwebende Schadenszahlen (0,9 s).
pub struct Numbers {
    v: Vec<(Vec3, f32, bool, f32)>, // Ort, Wert, krit, Alter 0..1
}

impl Numbers {
    pub fn new() -> Numbers {
        Numbers { v: Vec::with_capacity(64) }
    }
    pub fn add(&mut self, at: Vec3, n: f32, crit: bool) {
        if self.v.len() >= 64 {
            self.v.remove(0);
        }
        self.v.push((at, n, crit, 0.0));
    }
    pub fn update(&mut self, dt: f32) {
        for q in &mut self.v {
            q.3 += dt / 0.9;
        }
        self.v.retain(|q| q.3 < 1.0);
    }
    /// Je Zahl [x 0..1, y 0..1, Wert, krit, Alter] in `out` (vorher geleert).
    pub fn project(&self, vp: Mat4, out: &mut Vec<f32>) {
        out.clear();
        for &(p, n, crit, age) in &self.v {
            let c = vp * (p + Vec3::Y * (0.4 + age * 1.2)).extend(1.0);
            if c.w > 0.2 {
                let (x, y) = (c.x / c.w, c.y / c.w);
                if x.abs() < 1.1 && y.abs() < 1.1 {
                    out.extend_from_slice(&[x * 0.5 + 0.5, 0.5 - y * 0.5, n, crit as u8 as f32, age]);
                }
            }
        }
    }
}

struct Loot {
    p: Vec3,
    v: Vec3,
    item: u16,
    age: f32,
}

/// Beute-Würfelchen: fliegen im Bogen heraus, ab 3,5 Blöcken (oder nach 1,5 s) zieht die Figur sie an.
pub struct Drops {
    v: Vec<Loot>,
}

impl Drops {
    pub fn new() -> Drops {
        Drops { v: Vec::with_capacity(64) }
    }
    /// `r` liefert Zufallszahlen 0..1 (Partikel-Zufall).
    pub fn spawn(&mut self, at: Vec3, item: u16, r: &mut impl FnMut() -> f32) {
        if self.v.len() >= 64 {
            return;
        }
        let a = r() * TAU;
        let v = Vec3::new(a.cos() * 1.8, 4.5 + r() * 1.5, a.sin() * 1.8);
        self.v.push(Loot { p: at, v, item, age: 0.0 });
    }
    /// Bewegt alles; `solid` prüft den Boden. Liefert die Zahl der eben eingesammelten Würfelchen.
    pub fn update(&mut self, dt: f32, me: Vec3, solid: impl Fn(Vec3) -> bool) -> u32 {
        let target = me + Vec3::Y * 0.9;
        let mut got = 0;
        for d in &mut self.v {
            d.age += dt;
            let to = target - d.p;
            if d.age > 0.3 && (to.length_squared() < 3.5 * 3.5 || d.age > 1.5) {
                d.v = to.normalize_or_zero() * (6.0 + d.age * 6.0);
                d.p += d.v * dt;
                if to.length_squared() < 0.5 * 0.5 {
                    d.age = -1.0;
                    got += 1;
                }
                continue;
            }
            d.v.y -= 18.0 * dt;
            let n = d.p + d.v * dt;
            if solid(n) {
                d.v = Vec3::new(d.v.x * 0.5, 0.0, d.v.z * 0.5);
            } else {
                d.p = n;
            }
        }
        self.v.retain(|d| d.age >= 0.0 && d.age < 8.0);
        got
    }
    pub fn draw(&self, out: &mut Vec<Inst>, time: f32) {
        for d in &self.v {
            let bob = (time * 3.0 + d.age).sin() * 0.06;
            deco::drop_item(out, d.p + Vec3::Y * bob, d.item, time * 2.5 + d.age, 0.25);
        }
    }
}

/// Stoßwellen-Ringe und Geschosse.
pub struct Waves {
    shocks: Vec<(Vec3, f32, f32)>,     // Mitte, Endradius, Alter
    shots: Vec<(Vec3, Vec3, f32, f32, u8)>, // von, nach, Alter, Dauer, Art
}

impl Waves {
    pub fn new() -> Waves {
        Waves { shocks: Vec::with_capacity(8), shots: Vec::with_capacity(16) }
    }
    pub fn shock(&mut self, at: Vec3, r: f32) {
        if self.shocks.len() < 8 {
            self.shocks.push((at, r, 0.0));
        }
    }
    pub fn shot(&mut self, from: Vec3, to: Vec3, kind: u8) {
        if self.shots.len() < 16 {
            let dur = (from.distance(to) / 40.0).max(0.08);
            self.shots.push((from, to, 0.0, dur, kind));
        }
    }
    pub fn update(&mut self, dt: f32) {
        for s in &mut self.shocks {
            s.2 += dt;
        }
        self.shocks.retain(|s| s.2 * SHOCK_SPEED < s.1);
        for s in &mut self.shots {
            s.2 += dt;
        }
        self.shots.retain(|s| s.2 < s.3);
    }
    pub fn draw(&self, out: &mut Vec<Inst>) {
        for &(at, _, age) in &self.shocks {
            deco::shock_ring(out, at, age * SHOCK_SPEED);
        }
        for &(from, to, age, dur, kind) in &self.shots {
            deco::shot(out, from, to, age / dur, kind);
        }
    }
}

/// Text als JSON-Zeichenkette (mit Anführungszeichen) anhängen.
pub fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation() {
        let mut ip = Interp::new();
        ip.push(0.0, Vec3::ZERO, 3.0);
        ip.push(0.1, Vec3::new(1.0, 0.0, 0.0), -3.0);
        let (p, yaw, sp) = ip.at(0.05);
        assert!((p.x - 0.5).abs() < 1e-4 && (sp - 10.0).abs() < 1e-3);
        // 3 → −3 über ±π (kurzer Weg), nicht über 0
        assert!(yaw.abs() > 3.0);
        ip.push(0.2, Vec3::new(50.0, 0.0, 0.0), 0.0);
        assert_eq!(ip.at(0.05).0, Vec3::new(50.0, 0.0, 0.0), "Warp beginnt neu");
        let mut s = String::new();
        json_str(&mut s, "a\"b\nc");
        assert_eq!(s, "\"a\\\"b\\nc\"");
    }
}
