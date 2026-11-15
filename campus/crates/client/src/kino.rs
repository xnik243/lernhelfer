//! Phase 21: Kamerafahrten (Catmull-Rom), F8-Freikamera und das Ein-/Ausblenden des Wetters.
//! Reine Rechnung ohne Welt und ohne Browser – die Wand-Prüfung und die Ereignisse macht lib.rs.
use glam::Vec3;

/// Dauer einer Fahrt in Sekunden.
pub const DUR: f32 = 4.5;
/// Weitester Abstand der Fahrt von der Figur.
pub const R_MAX: f32 = 40.0;
/// Weitester Abstand der Freikamera von der Figur.
pub const FREE_R: f32 = 16.0;
/// Stützpunkte je Fahrt.
pub const N: usize = 6;

/// Catmull-Rom durch alle Punkte, `t` 0..1 über die ganze Strecke; die Enden zählen doppelt.
pub fn catmull(pts: &[Vec3], t: f32) -> Vec3 {
    let n = pts.len();
    if n < 2 {
        return pts.first().copied().unwrap_or(Vec3::ZERO);
    }
    let f = t.clamp(0.0, 1.0) * (n - 1) as f32;
    let i = (f as usize).min(n - 2);
    let u = f - i as f32;
    let p = |k: isize| pts[k.clamp(0, n as isize - 1) as usize];
    let i = i as isize;
    let (p0, p1, p2, p3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
    (p1 * 2.0 + (p2 - p0) * u + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * (u * u) + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * (u * u * u)) * 0.5
}

/// Punkt höchstens `r` von `c` entfernt.
pub fn clamp_r(p: Vec3, c: Vec3, r: f32) -> Vec3 {
    let d = p - c;
    let l = d.length();
    if l > r { c + d * (r / l) } else { p }
}

/// Stützpunkte und Blickziel der Fahrt `id` (0 Hub-Marktplatz, 1 Zitadelle, 2 Katakomben, 3 Arena) um die Figur
/// (`me` = Füße, vorn ist -Z): ein Bogen, der hinter der Figur endet.
pub fn route(id: u8, me: Vec3, yaw: f32) -> ([Vec3; N], Vec3) {
    // Radius Anfang/Ende, Höhe Anfang/Ende, Schwenk im Bogenmaß, Blickziel so weit vor der Figur
    let (r0, r1, h0, h1, sweep, ahead) = match id {
        0 => (30.0, 5.0, 20.0, 3.0, 2.4, 6.0),
        1 => (24.0, 5.0, 4.0, 3.0, -1.6, 14.0),
        2 => (5.0, 3.0, 5.0, 2.5, 3.0, 3.0),
        _ => (18.0, 5.0, 12.0, 3.0, -3.1, 4.0),
    };
    let mut pts = [me; N];
    for (k, p) in pts.iter_mut().enumerate() {
        let f = k as f32 / (N - 1) as f32;
        let a = yaw + sweep * (1.0 - f);
        let (r, h): (f32, f32) = (r0 + (r1 - r0) * f, h0 + (h1 - h0) * f);
        *p = clamp_r(me + Vec3::new(a.sin() * r, h, a.cos() * r), me, R_MAX);
    }
    (pts, me + Vec3::new(-yaw.sin() * ahead, 1.2, -yaw.cos() * ahead))
}

/// Laufende Fahrt.
pub struct Ride {
    pub id: u8,
    pub t: f32,
    pub pts: [Vec3; N],
    pub target: Vec3,
}
impl Ride {
    /// Nächstes Bild: (Auge, Blickrichtung) oder None, wenn die Fahrt zu Ende ist.
    pub fn step(&mut self, dt: f32) -> Option<(Vec3, Vec3)> {
        self.t += dt;
        if self.t >= DUR {
            return None;
        }
        let f = self.t / DUR;
        let eye = catmull(&self.pts, f * f * (3.0 - 2.0 * f)); // weich anfahren und ausrollen
        Some((eye, (self.target - eye).try_normalize().unwrap_or(Vec3::NEG_Z)))
    }
}

/// F8-Freikamera: eigene Stelle und Blickrichtung, Eingaben wie beim Laufen.
pub struct Free {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub mx: f32,
    pub mz: f32,
    pub up: bool,
    pub fast: bool,
}
impl Free {
    pub fn new(pos: Vec3, yaw: f32, pitch: f32) -> Free {
        Free { pos, vel: Vec3::ZERO, yaw, pitch, mx: 0.0, mz: 0.0, up: false, fast: false }
    }
    pub fn turn(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx;
        self.pitch = (self.pitch - dy).clamp(-1.55, 1.55);
    }
    /// Bewegt die Kamera (vorwärts entlang des Blicks, Springen = hoch) und hält sie bei `anchor` (Kopf der Figur).
    pub fn step(&mut self, dt: f32, anchor: Vec3) -> (Vec3, Vec3) {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let look = Vec3::new(-sy * cp, sp, -cy * cp);
        let wish = (look * self.mz + Vec3::new(cy, 0.0, -sy) * self.mx + Vec3::Y * self.up as u8 as f32) * if self.fast { 14.0 } else { 6.0 };
        self.vel += (wish - self.vel) * (dt * 8.0).min(1.0);
        self.pos = clamp_r(self.pos + self.vel * dt, anchor, FREE_R);
        (self.pos, look)
    }
}

/// Wetter-Überblendung: (gezeigte Art, Stärke 0..1) einen Schritt weiter – erst 1 s aus-, dann 1 s einblenden.
pub fn fade(kind: u8, next: u8, k: f32, dt: f32) -> (u8, f32) {
    if kind != next {
        let k = k - dt;
        if k <= 0.0 { (next, 0.0) } else { (kind, k) }
    } else if kind == 0 {
        (0, 0.0)
    } else {
        (kind, (k + dt).min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catmull_geht_durch_alle_punkte() {
        let pts = [Vec3::ZERO, Vec3::new(4.0, 1.0, 0.0), Vec3::new(8.0, 0.0, 3.0), Vec3::new(9.0, 5.0, 9.0), Vec3::new(0.0, 2.0, 12.0)];
        for (k, p) in pts.iter().enumerate() {
            assert!(catmull(&pts, k as f32 / 4.0).distance(*p) < 1e-4, "Knoten {k}");
        }
        assert_eq!(catmull(&pts, -1.0), pts[0]);
        assert!(catmull(&pts, 2.0).distance(pts[4]) < 1e-4);
        assert_eq!(catmull(&[], 0.5), Vec3::ZERO);
        assert_eq!(catmull(&pts[..1], 0.5), pts[0]);
        // zwischen den Knoten stetig: kleine Schritte machen kleine Wege
        let mut last = pts[0];
        for k in 1..=400 {
            let p = catmull(&pts, k as f32 / 400.0);
            assert!(p.distance(last) < 0.25);
            last = p;
        }
    }

    #[test]
    fn fahrt_bleibt_im_umkreis_und_endet() {
        let me = Vec3::new(240.5, 33.0, 296.5);
        for id in 0..4u8 {
            let (pts, target) = route(id, me, 0.7);
            assert!(pts.len() >= 5);
            assert!(target.distance(me) <= R_MAX);
            let mut r = Ride { id, t: 0.0, pts, target };
            let mut n = 0;
            while let Some((eye, look)) = r.step(0.05) {
                assert!(eye.distance(me) <= R_MAX + 0.5, "Fahrt {id}");
                assert!((look.length() - 1.0).abs() < 1e-3);
                n += 1;
            }
            assert!((88..=91).contains(&n), "4,5 s bei 20 Bildern/s, waren {n}");
        }
    }

    #[test]
    fn freikamera_bleibt_bei_der_figur() {
        let a = Vec3::new(10.0, 20.0, 30.0);
        let mut f = Free::new(a, 0.0, 0.0);
        f.mz = 1.0;
        f.up = true;
        f.fast = true;
        for _ in 0..600 {
            let (eye, _) = f.step(0.05, a);
            assert!(eye.distance(a) <= FREE_R + 1e-3);
        }
        assert!(f.pos.distance(a) > FREE_R - 0.1);
        f.turn(0.0, 9.0);
        assert!(f.pitch >= -1.55);
    }

    #[test]
    fn wetter_blendet_aus_und_ein() {
        let (mut kind, mut k) = (0u8, 0.0f32);
        for _ in 0..25 {
            (kind, k) = fade(kind, 1, k, 0.05);
        }
        assert_eq!((kind, k), (1, 1.0));
        (kind, k) = fade(kind, 2, k, 0.5);
        assert_eq!(kind, 1);
        assert!(k < 1.0 && k > 0.0);
        (kind, k) = fade(kind, 2, k, 0.6);
        assert_eq!((kind, k), (2, 0.0));
        for _ in 0..25 {
            (kind, k) = fade(kind, 0, k, 0.05);
        }
        assert_eq!((kind, k), (0, 0.0));
    }
}
