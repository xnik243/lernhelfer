//! Prozedurale Welt: Gelände der schwebenden Inseln (Oberfläche + Felskegel) und alle Bauten –
//! Hub-Archipel (Hauptinsel mit sechs Vierteln, drei Satelliten, Himmelsbrücken, Portale) und Privatinseln.
//! Jedes Stück entsteht für sich: alle Bauten schreiben über `Put`, das alles außerhalb des Stücks verwirft.
use crate::world::*;
use std::f32::consts::PI;

pub struct Put<'a> {
    pub b: &'a mut [u8],
    pub ox: i32,
    pub oz: i32,
}
impl Put<'_> {
    #[inline]
    fn at(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (lx, lz) = (x - self.ox, z - self.oz);
        ((0..16).contains(&lx) && (0..16).contains(&lz) && (0..CH).contains(&y)).then(|| idx(lx, y, lz))
    }
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.at(x, y, z).map_or(0, |i| self.b[i])
    }
    fn set(&mut self, x: i32, y: i32, z: i32, v: u8) {
        if let Some(i) = self.at(x, y, z) {
            self.b[i] = v;
        }
    }
    /// nur in Luft (Laub, Deko)
    fn add(&mut self, x: i32, y: i32, z: i32, v: u8) {
        if let Some(i) = self.at(x, y, z) {
            if self.b[i] == 0 {
                self.b[i] = v;
            }
        }
    }
    /// Quader mit eingeschlossenen Grenzen, auf das Stück beschnitten.
    #[allow(clippy::too_many_arguments)]
    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, v: u8) {
        let (xa, xb) = (x0.min(x1).max(self.ox), x0.max(x1).min(self.ox + 15));
        let (za, zb) = (z0.min(z1).max(self.oz), z0.max(z1).min(self.oz + 15));
        let (ya, yb) = (y0.min(y1).max(0), y0.max(y1).min(CH - 1));
        for y in ya..=yb {
            for z in za..=zb {
                for x in xa..=xb {
                    self.b[idx(x - self.ox, y, z - self.oz)] = v;
                }
            }
        }
    }
    /// Berührt das Rechteck (Grenzen eingeschlossen) dieses Stück?
    fn hits(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        x0.min(x1) <= self.ox + 15 && x0.max(x1) >= self.ox && z0.min(z1) <= self.oz + 15 && z0.max(z1) >= self.oz
    }
}

// ================= Gelände =================

/// Oberfläche einer Inselspalte: Höhe, Deckblock, Abstand zum Inselrand (bestimmt die Tiefe des Felskegels).
#[derive(Clone, Copy, Debug)]
pub struct Surf {
    pub h: i32,
    pub top: u8,
    pub d: f32,
}

/// Abstand von (x, z) zum Rand des abgerundeten Rechtecks [x0, z0, x1, z1) nach innen (negativ = draußen).
fn rr(x: i32, z: i32, r: [i32; 4], rad: f32) -> f32 {
    let (px, pz) = (x as f32 + 0.5, z as f32 + 0.5);
    let (mx, mz) = ((r[0] + r[2]) as f32 / 2.0, (r[1] + r[3]) as f32 / 2.0);
    let (hx, hz) = ((r[2] - r[0]) as f32 / 2.0, (r[3] - r[1]) as f32 / 2.0);
    let (qx, qz) = ((px - mx).abs() - (hx - rad), (pz - mz).abs() - (hz - rad));
    -(qx.max(0.0).hypot(qz.max(0.0)) + qx.max(qz).min(0.0) - rad)
}
/// Ausgefranster Inselrand: -1,2 .. +1,8 Blöcke.
fn wob(x: i32, z: i32, s: u32) -> f32 {
    (noise(x as f32 / 7.0, z as f32 / 7.0, s) - 0.4) * 3.0
}

/// Kleine Insel aus dem Großprojekt „Insel“ (südlich hinter dem Tempel).
pub const ISLET: [i32; 4] = [228, 372, 252, 396];

/// Oberfläche der Spalte oder None (Leere: die ganze Spalte ist Luft).
pub fn surf(x: i32, z: i32, st: WorldState) -> Option<Surf> {
    if !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) {
        return None;
    }
    if in_hub(x, z) {
        return hub_surf(x, z, st);
    }
    let (lx, lz) = (x % SLOT, z % SLOT);
    let d = rr(lx, lz, [29, 29, 67, 67], 5.0) + wob(x, z, 31);
    if d > 0.0 {
        return Some(Surf { h: GROUND, top: WIESE, d });
    }
    let d = rr(lx, lz, [73, 40, 89, 56], 3.0) + wob(x, z, 32) * 0.5;
    (d > 0.0).then_some(Surf { h: GROUND, top: WIESE, d })
}

fn hub_surf(x: i32, z: i32, st: WorldState) -> Option<Surf> {
    let d = rr(x, z, HUB_MAIN, 16.0) + wob(x, z, 1);
    if d > 0.0 {
        if gorge(x, z) {
            return None;
        }
        let (h, top) = main_top(x, z);
        return Some(Surf { h, top, d });
    }
    for (k, s) in SATS.iter().enumerate() {
        let d = rr(x, z, [s.x0, s.z0, s.x0 + s.size, s.z0 + s.size], 9.0) + wob(x, z, 2 + k as u32);
        if d > 0.0 {
            let r = hash(x, z, 79);
            return Some(Surf { h: s.y, top: if r % 9 == 0 { BLUMEN_ROT } else if r % 13 == 0 { BLUMEN_GELB } else { WIESE }, d });
        }
    }
    if st.projects & P_ISLAND != 0 {
        let d = rr(x, z, ISLET, 6.0) + wob(x, z, 9);
        if d > 0.0 {
            return Some(Surf { h: 32, top: WIESE, d });
        }
    }
    None
}

/// Wasserfall-Schlucht, die die Himmelsruinen (Nordwest-Klippe) vom Rest der Hauptinsel trennt.
fn gorge(x: i32, z: i32) -> bool {
    ((199..207).contains(&x) && z < 190) || ((189..195).contains(&z) && x < 207)
}

const RUINEN: usize = 0;
const SPAWN: usize = 1;
const MARKT: usize = 2;
const SCHMIEDE: usize = 3;
const WALD: usize = 4;

/// Viertel der Hauptinsel (Index in DISTRICTS); Randspalten zählen zum nächsten Viertel.
fn district(x: i32, z: i32) -> usize {
    let (x, z) = (x.clamp(HUB_MAIN[0], HUB_MAIN[2] - 1), z.clamp(HUB_MAIN[1], HUB_MAIN[3] - 1));
    DISTRICTS.iter().position(|(_, r)| x >= r[0] && z >= r[1] && x < r[2] && z < r[3]).unwrap_or(MARKT)
}
fn trail_x(z: i32) -> i32 {
    296 + (7.0 * (z as f32 / 14.0).sin()).round() as i32
}
const CABIN: (i32, i32) = (301, 267);

/// Oberfläche der Hauptinsel: Höhe je Viertel und Deckblock (gemischt, damit kaum nackte Wiese bleibt).
fn main_top(x: i32, z: i32) -> (i32, u8) {
    let r = hash(x, z, 77) % 100;
    let pick = |t: &[(u32, u8)]| t.iter().find(|e| r < e.0).map_or(t[t.len() - 1].1, |e| e.1);
    match district(x, z) {
        RUINEN => (35, pick(&[(15, WIESE), (40, MOOS_BRUCH), (60, MOOS_ZIEGEL), (85, ERDE), (100, HECKE)])),
        SPAWN => {
            let h = if z >= 267 { 32 } else { 28 + (z - 255).max(0) / 3 };
            let top = if z < 267 && z > 255 && (z - 255) % 3 == 0 {
                STEINZIEGEL // Kante der Stützmauer
            } else if x.rem_euclid(6) == 0 || z.rem_euclid(6) == 0 {
                MOOS_ZIEGEL // Gartenwege
            } else {
                pick(&[(20, WIESE), (40, ERDE), (60, BLUMEN_ROT), (80, BLUMEN_GELB), (100, HECKE)])
            };
            (h, top)
        }
        MARKT => (28, if (x + z) % 4 == 0 { POLIERT } else { PFLASTER }),
        SCHMIEDE => {
            let street = (252..=256).contains(&z) || (193..=197).contains(&x) || (284..=287).contains(&z);
            (26, if street { PFLASTER } else { pick(&[(8, WIESE), (35, ERDE), (65, BRUCHSTEIN), (100, PFLASTER)]) })
        }
        WALD => {
            let (dx, dz) = (x - CABIN.0, z - CABIN.1);
            let flat = dx * dx + dz * dz < 144 || (x >= 312 && (z - 240).abs() <= 10);
            let ramp = ((x - 274) as f32 / 10.0).clamp(0.0, 1.0);
            let h = if flat { 30 } else { (28 + (noise(x as f32 / 16.0, z as f32 / 16.0, 5) * 12.0 * ramp) as i32).min(38) };
            let top = if (x - trail_x(z)).abs() <= 1 { WEG } else { pick(&[(15, WIESE), (40, LAUB), (55, BIRKENLAUB), (78, ERDE), (90, MOOS_BRUCH), (100, BLUMEN_GELB)]) };
            (h, top)
        }
        _ => (28, pick(&[(10, WIESE), (35, ERDE), (65, FELS), (85, BRUCHSTEIN), (100, MOOS_BRUCH)])),
    }
}

/// Inselspalte: Deckblock, Erde, geschichteter Fels, Moos-Bruchstein am Rand, Kristalladern, Tiefenfels-Tropfsteine unten.
/// Die Tiefe wächst mit dem Abstand zum Rand: ein umgedrehter Kegel.
fn ground(p: &mut Put, x: i32, z: i32, s: Surf) {
    let drip = if hash(x, z, 23) % 11 == 0 { 2 + (hash(x, z, 24) % 6) as i32 } else { 0 };
    // Hub-Hauptinsel: ab d 12 flacher weiter, damit der Bauch ein Kegel bleibt statt einer Platte auf y 1;
    // unter den Kristallminen (y 6–24) bleibt der volle Fels, mit 12 Blöcken Übergang.
    let mut d = s.d;
    let [x0, z0, x1, z1] = HUB_MAIN;
    if (x0..x1).contains(&x) && (z0..z1).contains(&z) && d > 12.0 {
        let m = (205 - x).max(x - 272).max(144 - z).max(z - 202).max(0) as f32;
        d += (12.0 + (d - 12.0) * 0.12 - d) * (m / 12.0).min(1.0);
    }
    let depth = (4.0 + d * 1.1 + noise(x as f32 / 6.0, z as f32 / 6.0, 7) * 5.0) as i32 + drip;
    let bot = (s.h - depth).max(1);
    let band = (noise(x as f32 / 9.0, z as f32 / 9.0, 43) * 4.0) as i32;
    for y in bot..=s.h {
        let v = if y == s.h {
            s.top
        } else if s.h - y <= 2 {
            ERDE
        } else if y - bot < 2 + drip.min(3) {
            TIEFENFELS
        } else {
            let r = hash(x * 13 + y, z, 41);
            if s.d < 3.0 && r % 19 == 0 {
                [SAPHIR, SPRACHGOLD, SMARAGD][(r / 19 % 3) as usize]
            } else if s.d < 2.5 && r % 5 < 2 {
                MOOS_BRUCH
            } else {
                match (y + band).rem_euclid(8) {
                    _ if y < 12 => TIEFENFELS,
                    0 => TIEFENFELS,
                    4 => BRUCHSTEIN,
                    _ => FELS,
                }
            }
        };
        p.set(x, y, z, v);
    }
}

/// Grobe Kästen, in denen überhaupt etwas steht; Stücke außerhalb bleiben reine Luft (leerer Vec).
const HUB_BOXES: [[i32; 4]; 8] = [
    [140, 140, 340, 340],
    [362, 210, 423, 271],
    [207, 52, 273, 118],
    [62, 212, 118, 268],
    [312, 234, 374, 246],
    [234, 104, 246, 156],
    [104, 234, 156, 246],
    [222, 320, 258, 402],
];
pub fn touches(ox: i32, oz: i32) -> bool {
    let hit = |b: &[i32; 4]| b[0] <= ox + 15 && b[2] >= ox && b[1] <= oz + 15 && b[3] >= oz;
    if in_hub(ox, oz) {
        return HUB_BOXES.iter().any(hit);
    }
    let (a, c) = (ox / SLOT * SLOT, oz / SLOT * SLOT);
    hit(&[a + 20, c + 24, a + 94, c + 72])
}

/// Ein Stück: erst das Gelände Spalte für Spalte, dann die Bauten.
pub fn build(p: &mut Put, st: WorldState) {
    for z in p.oz..p.oz + 16 {
        for x in p.ox..p.ox + 16 {
            if let Some(s) = surf(x, z, st) {
                ground(p, x, z, s);
            }
        }
    }
    if in_hub(p.ox, p.oz) {
        hub(p, st);
    } else {
        private(p, st);
    }
}

// ================= Bausteine =================

const NORD: u8 = 0;
const SUED: u8 = 1;
const WEST: u8 = 2;
const OST: u8 = 3;

struct Style {
    wall: u8,
    frame: u8,
    roof: u8,
    base: u8,
    floor: u8,
}
const FACHWERK: Style = Style { wall: PUTZ, frame: DUNKELHOLZ, roof: DACH, base: STEINZIEGEL, floor: BRETTER };
const STEINHAUS: Style = Style { wall: STEINZIEGEL, frame: DUNKELHOLZ, roof: SCHIEFER, base: BRUCHSTEIN, floor: FLIESE };
const BANKHAUS: Style = Style { wall: POLIERT, frame: GOLDZIER, roof: SCHIEFER, base: STEINZIEGEL, floor: TEPPICH_ROT };
const BLOCKHAUS: Style = Style { wall: STAMM, frame: DUNKELHOLZ, roof: DUNKELBRETT, base: BRUCHSTEIN, floor: BRETTER };
const SCHMIEDE_STIL: Style = Style { wall: BRUCHSTEIN, frame: DUNKELHOLZ, roof: SCHIEFER, base: BRUCHSTEIN, floor: PFLASTER };

fn dist(dx: i32, dz: i32) -> f32 {
    ((dx * dx + dz * dz) as f32).sqrt()
}
/// Laternenpfahl: zwei Pfosten, Laterne obendrauf; y = erster Block über dem Boden.
fn lantern(p: &mut Put, x: i32, y: i32, z: i32) {
    p.fill(x, y, z, x, y + 1, z, DUNKELHOLZ);
    p.set(x, y + 2, z, LATERNE);
}
/// Kugel nur in Luft, mit kleinen Lücken (Baumkronen, Büsche).
fn blob(p: &mut Put, cx: i32, cy: i32, cz: i32, r: f32, v: u8) {
    let k = r.ceil() as i32;
    if !p.hits(cx - k, cz - k, cx + k, cz + k) {
        return;
    }
    for dy in -k..=k {
        for dz in -k..=k {
            for dx in -k..=k {
                if ((dx * dx + dy * dy + dz * dz) as f32) <= r * r && hash(cx + dx, cz + dz, (cy + dy) as u32) % 7 != 0 {
                    p.add(cx + dx, cy + dy, cz + dz, v);
                }
            }
        }
    }
}
/// Kreisring (r0..r1) bzw. Scheibe (r0 = 0) auf Höhe y.
fn disc(p: &mut Put, cx: i32, y: i32, cz: i32, r0: f32, r1: f32, v: u8) {
    let k = r1.ceil() as i32;
    if !p.hits(cx - k, cz - k, cx + k, cz + k) {
        return;
    }
    for dz in -k..=k {
        for dx in -k..=k {
            let d = dist(dx, dz);
            if d >= r0 && d <= r1 {
                p.set(cx + dx, y, cz + dz, v);
            }
        }
    }
}
/// Halbkugel-Schale ab Höhe cy; Rippen und Ringe aus `band`.
fn dome(p: &mut Put, cx: i32, cy: i32, cz: i32, r: f32, shell: u8, band: u8) {
    let k = r.ceil() as i32;
    if !p.hits(cx - k, cz - k, cx + k, cz + k) {
        return;
    }
    for dy in 0..=k {
        for dz in -k..=k {
            for dx in -k..=k {
                let q = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                if q <= r && q > r - 1.3 {
                    p.set(cx + dx, cy + dy, cz + dz, if dy % 4 == 2 || dx == 0 || dz == 0 { band } else { shell });
                }
            }
        }
    }
}

/// Giebelhaus mit Fachwerk: Grundfläche x0..x0+w-1 × z0..z0+d-1, Boden auf y, `floors` Stockwerke à 4 Blöcke,
/// Tür (3 hoch) mitten in der Wand `door`, Fenster mit Blumenkasten, Dachüberstand, Schornstein, Laterne innen.
#[allow(clippy::too_many_arguments)]
fn house(p: &mut Put, x0: i32, z0: i32, w: i32, d: i32, y: i32, floors: i32, s: &Style, door: u8) {
    let (x1, z1) = (x0 + w - 1, z0 + d - 1);
    if !p.hits(x0 - 2, z0 - 2, x1 + 2, z1 + 2) {
        return;
    }
    let top = y + 4 * floors;
    let along_x = w >= d;
    let span = if along_x { d } else { w };
    let ridge = top + 1 + (span + 1) / 2;
    p.fill(x0, y - 6, z0, x1, y, z1, s.base);
    p.fill(x0 + 1, y, z0 + 1, x1 - 1, y, z1 - 1, s.floor);
    p.fill(x0, y + 1, z0, x1, top, z1, s.wall);
    p.fill(x0 + 1, y + 1, z0 + 1, x1 - 1, ridge, z1 - 1, 0);
    for f in 1..=floors {
        let yb = y + 4 * f;
        for (a, b, c, e) in [(x0, z0, x1, z0), (x0, z1, x1, z1), (x0, z0, x0, z1), (x1, z0, x1, z1)] {
            p.fill(a, yb, b, c, yb, e, s.frame);
        }
        if f < floors {
            p.fill(x0 + 1, yb, z0 + 1, x1 - 1, yb, z1 - 1, s.floor);
        }
    }
    for x in x0..=x1 {
        if (x - x0) % 3 == 0 || x == x1 {
            p.fill(x, y + 1, z0, x, top, z0, s.frame);
            p.fill(x, y + 1, z1, x, top, z1, s.frame);
        }
    }
    for z in z0..=z1 {
        if (z - z0) % 3 == 0 || z == z1 {
            p.fill(x0, y + 1, z, x0, top, z, s.frame);
            p.fill(x1, y + 1, z, x1, top, z, s.frame);
        }
    }
    // Fenster (2 hoch) in jedem zweiten Feld, einen Block nach innen versetzt (Laibung), darunter ein Blumenkasten außen
    for f in 0..floors {
        let wy = y + 4 * f + 2;
        let flower = if f % 2 == 0 { BLUMEN_ROT } else { BLUMEN_GELB };
        for x in x0 + 1..x1 {
            if (x - x0) % 6 == 1 || (x - x0) % 6 == 2 {
                for (z, o) in [(z0, -1), (z1, 1)] {
                    p.fill(x, wy, z, x, wy + 1, z, 0);
                    p.fill(x, wy, z - o, x, wy + 1, z - o, GLAS);
                    p.set(x, wy - 1, z + o, flower);
                }
            }
        }
        for z in z0 + 1..z1 {
            if (z - z0) % 6 == 1 || (z - z0) % 6 == 2 {
                for (x, o) in [(x0, -1), (x1, 1)] {
                    p.fill(x, wy, z, x, wy + 1, z, 0);
                    p.fill(x - o, wy, z, x - o, wy + 1, z, GLAS);
                    p.set(x + o, wy - 1, z, flower);
                }
            }
        }
        p.set(x0 + w / 2, y + 4 * f + 3, z0 + d / 2, LATERNE);
    }
    // Tür, davor frei, daneben eine Laterne
    let dw = if (if door < WEST { w } else { d }) >= 14 { 3 } else { 2 };
    let (dx0, dz0, ox, oz) = match door {
        NORD => (x0 + w / 2 - 1, z0, 0, -1),
        SUED => (x0 + w / 2 - 1, z1, 0, 1),
        WEST => (x0, z0 + d / 2 - 1, -1, 0),
        _ => (x1, z0 + d / 2 - 1, 1, 0),
    };
    let (dx1, dz1) = if door < WEST { (dx0 + dw - 1, dz0) } else { (dx0, dz0 + dw - 1) };
    p.fill(dx0, y + 1, dz0, dx1, y + 3, dz1, 0);
    p.fill(dx0, y, dz0, dx1, y, dz1, s.floor);
    p.fill(dx0 + ox, y + 1, dz0 + oz, dx1 + ox, y + 3, dz1 + oz, 0);
    // Laterne links, Schild rechts neben der Tür
    if door < WEST {
        p.set(dx0 - 1, y + 3, dz0 + oz, LATERNE);
        p.set(dx1 + 1, y + 3, dz0 + oz, SCHILD);
    } else {
        p.set(dx0 + ox, y + 3, dz0 - 1, LATERNE);
        p.set(dx0 + ox, y + 3, dz1 + 1, SCHILD);
    }
    // Satteldach mit Überstand, Giebelwände
    let (l0, l1, s0, s1) = if along_x { (x0 - 1, x1 + 1, z0 - 1, z1 + 1) } else { (z0 - 1, z1 + 1, x0 - 1, x1 + 1) };
    let gables = if along_x { [x0, x1] } else { [z0, z1] };
    let mut k = 0;
    while s0 + k <= s1 - k {
        let yr = top + 1 + k;
        for l in l0..=l1 {
            for sv in [s0 + k, s1 - k] {
                let (x, z) = if along_x { (l, sv) } else { (sv, l) };
                p.set(x, yr, z, s.roof);
            }
        }
        for g in gables {
            for sv in s0 + k + 1..s1 - k {
                let (x, z) = if along_x { (g, sv) } else { (sv, g) };
                p.set(x, yr, z, s.wall);
            }
        }
        k += 1;
    }
    p.fill(x1 - 1, y + 1, z0 + 1, x1 - 1, ridge + 2, z0 + 1, BRUCHSTEIN); // Schornstein
}

/// Marktstand 5×4 mit gestreifter Markise; `south`: Theke vorn im Süden (sonst im Norden).
fn stall(p: &mut Put, x0: i32, z0: i32, y: i32, c: (u8, u8), goods: u8, south: bool) {
    let (x1, z1) = (x0 + 4, z0 + 3);
    if !p.hits(x0 - 1, z0 - 2, x1 + 1, z1 + 2) {
        return;
    }
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        p.fill(x, y + 1, z, x, y + 3, z, DUNKELHOLZ);
    }
    let (front, back, over) = if south { (z1, z0, z1 + 1) } else { (z0, z1, z0 - 1) };
    p.fill(x0 + 1, y + 1, front, x1 - 1, y + 1, front, KISTE);
    for x in [x0 + 1, x1 - 1] {
        p.set(x, y + 2, front, goods);
    }
    p.set(x0 + 1, y + 1, back, FASS);
    p.set(x1 - 1, y + 1, back, FASS);
    p.set(x1 - 1, y + 2, back, KISTE);
    for x in x0..=x1 {
        let v = if (x - x0) % 2 == 0 { c.0 } else { c.1 };
        p.fill(x, y + 4, z0, x, y + 4, z1, v);
        p.set(x, y + 3, over, v);
    }
}

/// Torbogen mit Warp-Portal (3 breit, 4 hoch), Mitte (cx, cz) auf Boden y; Ebene x = cx (`plane_x`) oder z = cz.
fn arch(p: &mut Put, cx: i32, cz: i32, y: i32, plane_x: bool) {
    if !p.hits(cx - 3, cz - 3, cx + 3, cz + 3) {
        return;
    }
    for k in -2..=2 {
        let (x, z) = if plane_x { (cx, cz + k) } else { (cx + k, cz) };
        p.set(x, y, z, POLIERT);
        for h in 1..=5 {
            p.set(x, y + h, z, if k.abs() == 2 || h == 5 { STEINZIEGEL } else { PORTAL });
        }
        if k.abs() == 2 {
            p.set(x, y + 6, z, LATERNE);
        }
    }
    p.set(cx, y + 6, cz, GOLDZIER);
}

/// Baum mit Wurzelansätzen; ab 9 Blöcken Höhe mit 2–4 Ästen und eigenen Laubbüscheln.
#[allow(clippy::too_many_arguments)]
fn tree(p: &mut Put, x: i32, z: i32, y: i32, h: i32, wood: u8, leaf: u8, seed: u32) {
    if !p.hits(x - 9, z - 9, x + 9, z + 9) {
        return;
    }
    p.fill(x, y + 1, z, x, y + h, z, wood);
    for (k, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
        if hash(x, z, seed + k as u32) % 3 != 0 {
            p.set(x + dx, y + 1, z + dz, wood);
        }
    }
    let n = if h >= 9 { 2 + (hash(x, z, seed + 7) % 3) as i32 } else { 0 };
    let len = 2 + h / 5;
    for i in 0..n {
        let a = (i as f32 + rnd(x + i, z, seed + 8)) * 2.0 * PI / n as f32;
        let (cx, cz) = (a.cos(), a.sin());
        let by = y + h / 2 + 1 + i * (h / 2 - 1).max(1) / n;
        for k in 1..=len {
            p.set(x + (cx * k as f32).round() as i32, by + k / 2, z + (cz * k as f32).round() as i32, wood);
        }
        blob(p, x + (cx * len as f32).round() as i32, by + len / 2 + 1, z + (cz * len as f32).round() as i32, 2.3, leaf);
    }
    blob(p, x, y + h, z, if h >= 9 { 3.4 } else { 2.6 }, leaf);
}

fn mushroom(p: &mut Put, x: i32, z: i32, y: i32, h: i32) {
    if !p.hits(x - 3, z - 3, x + 3, z + 3) {
        return;
    }
    p.fill(x, y + 1, z, x, y + h, z, PILZSTIEL);
    disc(p, x, y + h + 1, z, 0.0, 2.3, PILZHUT);
    disc(p, x, y + h, z, 1.6, 2.5, PILZHUT);
}

/// Bach bis zur Kante und Wasserfall in die Leere: auf Land wird die Oberfläche zu Wasser, in der Leere hängt eine Wassersäule.
#[allow(clippy::too_many_arguments)]
fn fall(p: &mut Put, st: WorldState, x0: i32, z0: i32, x1: i32, z1: i32, top: i32, bot: i32) {
    if !p.hits(x0, z0, x1, z1) {
        return;
    }
    for z in z0.max(p.oz)..=z1.min(p.oz + 15) {
        for x in x0.max(p.ox)..=x1.min(p.ox + 15) {
            match surf(x, z, st) {
                None => p.fill(x, bot, z, x, top, z, WASSER),
                Some(s) => p.set(x, s.h, z, WASSER),
            }
        }
    }
}

fn rocky(b: u8) -> bool {
    matches!(b, FELS | TIEFENFELS | BRUCHSTEIN | MOOS_BRUCH | ERDE | SCHIEFER)
}
/// Erz je Bergwerks-Ebene (0 oben … 2 unten).
fn ore(lvl: usize, r: u32) -> u8 {
    let t: &[u8] = match lvl {
        0 => &[ERZ_KOHLE, ERZ_KOHLE, ERZ_KOHLE, ERZ_EISEN, ERZ_EISEN, ADER_SAPHIR],
        1 => &[ERZ_EISEN, ERZ_EISEN, ERZ_GOLD, ERZ_GOLD, ADER_GOLD, ADER_SMARAGD],
        _ => &[ERZ_GOLD, ADER_SAPHIR, ADER_GOLD, ADER_SMARAGD, SPLITTER_ADER, ADER_SAPHIR],
    };
    t[(r % t.len() as u32) as usize]
}

// ================= Hub-Archipel =================

/// Bekannte Bauwerke des Hubs (Name, Boden-Stelle) – für die Dichte-Prüfung und Wegweiser.
pub fn hub_landmarks() -> &'static [(&'static str, [i32; 3])] {
    &[
        ("Spawn-Tempel", [240, 34, 316]),
        ("Kristallbrunnen", [240, 28, 230]),
        ("Bazaar-Halle", [240, 28, 206]),
        ("Bank", [214, 28, 227]),
        ("Auktionshaus", [265, 28, 226]),
        ("Kontor", [215, 28, 245]),
        ("Bäckerei", [267, 28, 245]),
        ("Gasthaus", [221, 32, 272]),
        ("Dekanat", [259, 32, 272]),
        ("Schmiede", [176, 26, 203]),
        ("Minion-Werkstatt", [178, 26, 247]),
        ("Wassermühle", [160, 26, 266]),
        ("Holzfällerhütte", [301, 30, 266]),
        ("Förderturm", [247, 28, 183]),
        ("Kristallhöhle", [240, 6, 178]),
        ("Parkour-Turm", [170, 35, 165]),
        ("Ruinen-Tempel", [182, 35, 176]),
        ("Bogenbrücke", [202, 30, 177]),
        ("Bibliotheks-Turm", [393, 30, 240]),
        ("Prüfungs-Zitadelle", [240, 30, 78]),
        ("Sternwarte", [84, 30, 236]),
    ]
}

fn hub(p: &mut Put, st: WorldState) {
    mines(p);
    spawn(p);
    avenue(p);
    market(p);
    forge(p, st);
    forest(p);
    ruins(p, st);
    library(p, st);
    zitadelle(p, st);
    garden(p, st);
    bridges(p);
    projects(p, st);
    scatter(p);
}

/// Stellen, an denen keine Zufalls-Deko stehen darf (Ankunft, NPCs, Portale, Sprungpolster, Turm-Bauplatz).
fn keep_free(x: i32, z: i32) -> bool {
    let near = |px: f32, pz: f32, r: f32| (px - x as f32 - 0.5).powi(2) + (pz - z as f32 - 0.5).powi(2) <= r * r;
    SPOTS.iter().any(|s| near(s.1[0], s.1[2], 3.0))
        || crate::content::NPCS.iter().any(|n| {
            let q = crate::skyblock::npc_pos(n);
            near(q[0], q[2], 2.5)
        })
        || PORTALS.iter().any(|q| near(q.0 as f32, q.1 as f32, 6.5))
        || LAUNCH_PADS.iter().any(|q| near(q.0 as f32 + 0.5, q.1 as f32 + 0.5, 3.5))
        || ((258..=266).contains(&x) && (182..=190).contains(&z))
}

/// Jede 10×10-Fläche der Hauptinsel bekommt Kleinkram: Fässer, Kisten, Pilze, Büsche, Felsbrocken, Mauerreste.
fn scatter(p: &mut Put) {
    let [x0, z0, x1, z1] = HUB_MAIN;
    for cz in (z0..z1).step_by(10) {
        for cx in (x0..x1).step_by(10) {
            if !p.hits(cx, cz, cx + 9, cz + 9) {
                continue;
            }
            for k in 0..2u32 {
                let r = hash(cx, cz, 95 + k);
                let (x, z) = (cx + 1 + (r % 8) as i32, cz + 1 + (r / 8 % 8) as i32);
                if gorge(x, z) || keep_free(x, z) {
                    continue;
                }
                let (h, top) = main_top(x, z);
                if top == WEG || p.get(x, h, z) != top || p.get(x, h + 1, z) != 0 || p.get(x, h + 2, z) != 0 {
                    continue;
                }
                let v = r / 64;
                match district(x, z) {
                    RUINEN => p.fill(x, h + 1, z, x, h + 1 + (v % 3) as i32, z, if v % 2 == 0 { MOOS_ZIEGEL } else { MOOS_BRUCH }),
                    SPAWN => p.fill(x, h + 1, z, x, h + 1 + (v % 2) as i32, z, HECKE),
                    MARKT | SCHMIEDE => {
                        p.set(x, h + 1, z, if v % 2 == 0 { FASS } else { KISTE });
                        if v % 3 == 0 {
                            p.set(x, h + 2, z, KISTE);
                        }
                    }
                    WALD => {
                        if v % 2 == 0 {
                            p.set(x, h + 1, z, PILZSTIEL);
                            p.set(x, h + 2, z, PILZHUT);
                        } else {
                            p.set(x, h + 1, z, LAUB); // einzelner Busch: ein Block bleibt im eigenen Stück (keine halben an Stückgrenzen)
                        }
                    }
                    _ => {
                        p.set(x, h + 1, z, FELS);
                        p.set(x, h + 2, z, if v % 2 == 0 { BRUCHSTEIN } else { FELS });
                    }
                }
            }
        }
    }
}

// ---------- Spawn-Zitadelle (y 32) ----------
fn spawn(p: &mut Put) {
    if !p.hits(198, 253, 282, 332) {
        return;
    }
    // Terrassen: Hecken, Blumen und Laternen an den Stützmauern
    for z in [257, 260, 263, 266] {
        for x in 200..280 {
            if (233..=247).contains(&x) {
                continue;
            }
            let h = main_top(x, z).0;
            if x % 8 == 4 {
                if p.get(x, h + 1, z) == 0 {
                    lantern(p, x, h + 1, z);
                }
            } else {
                p.add(x, h + 1, z, if x % 8 < 4 { HECKE } else { BLUMEN_ROT });
            }
        }
    }
    // Ankunftsplatz vor dem Tempel
    for z in 284..=301i32 {
        for x in 224..=256i32 {
            let v = if x == 224 || x == 256 || z == 284 || z == 301 {
                STEINZIEGEL
            } else if ((x - 240).abs() / 2 + (z - 293).abs() / 2) % 2 == 0 {
                POLIERT
            } else {
                MARMOR
            };
            p.set(x, 32, z, v);
        }
    }
    for (x, z) in [(224, 286), (224, 300), (256, 286), (256, 293), (256, 300), (232, 284), (248, 284)] {
        lantern(p, x, 33, z);
    }
    arch(p, 231, 296, 32, true);
    let [ax, ay, az] = ARENA_PORTAL_AT;
    p.fill(ax, ay, az, ax, ay + 1, az, ARENA_PORTAL);
    p.fill(243, 33, 292, 243, 34, 292, QUESTBRETT);
    p.set(243, 35, 292, LATERNE);
    for (cx, cz) in [(233, 288), (247, 288)] {
        for dz in -2..=2 {
            for dx in -2..=2 {
                p.set(cx + dx, 33, cz + dz, if dx.abs() == 2 || dz.abs() == 2 { STEINZIEGEL } else { WASSER });
            }
        }
        p.fill(cx, 33, cz, cx, 34, cz, POLIERT);
        p.set(cx, 35, cz, WASSER);
    }
    // Spawn-Tempel: Marmor-Stufen, Ring aus Kristallsäulen, Kuppel
    if p.hits(222, 300, 258, 330) {
        p.fill(224, 27, 302, 256, 33, 328, STEINZIEGEL);
        p.fill(225, 33, 303, 255, 33, 327, POLIERT);
        p.fill(228, 34, 306, 252, 34, 326, MARMOR);
        p.fill(229, 35, 307, 251, 43, 325, 0);
        for i in 0..12 {
            let a = i as f32 * PI / 6.0;
            let (x, z) = (240 + (10.0 * a.cos()).round() as i32, 316 + (10.0 * a.sin()).round() as i32);
            p.fill(x, 35, z, x, 42, z, MARMOR);
            p.fill(x, 38, z, x, 39, z, KRISTALL);
        }
        disc(p, 240, 43, 316, 9.0, 11.6, STEINZIEGEL);
        dome(p, 240, 44, 316, 11.0, MARMOR, GLAS);
        p.fill(240, 55, 316, 240, 56, 316, KRISTALL);
        p.fill(238, 35, 314, 242, 35, 318, POLIERT);
        p.fill(239, 36, 315, 241, 36, 317, GOLDZIER);
        p.fill(240, 37, 316, 240, 38, 316, KRISTALL);
    }
    house(p, 214, 266, 15, 13, 32, 2, &FACHWERK, OST); // Gasthaus
    if p.hits(214, 266, 228, 278) {
        for (x, z) in [(218, 270), (222, 270), (218, 275), (222, 275)] {
            p.set(x, 33, z, FASS);
        }
        p.fill(216, 33, 268, 216, 33, 276, DUNKELBRETT);
    }
    house(p, 252, 266, 15, 13, 32, 2, &STEINHAUS, WEST); // Dekanat
    if p.hits(252, 266, 266, 278) {
        p.fill(265, 33, 268, 265, 35, 276, REGAL);
        p.fill(259, 33, 271, 261, 33, 273, DUNKELBRETT);
        p.set(262, 33, 272, TAFEL);
    }
    house(p, 203, 302, 14, 12, 32, 1, &FACHWERK, OST); // Kanzlei
    house(p, 262, 302, 13, 12, 32, 1, &FACHWERK, WEST); // Wohnhaus
    if p.hits(262, 302, 275, 314) {
        p.fill(272, 33, 304, 272, 33, 305, BETT);
        p.set(270, 33, 311, KISTE);
    }
}

// ---------- Königsallee (9 breit, von der Zitadelle hinab zum Markt) ----------
fn avenue(p: &mut Put) {
    if !p.hits(231, 253, 249, 284) {
        return;
    }
    for z in 253..284 {
        let h = main_top(240, z).0;
        for x in 236..=244 {
            p.set(x, h, z, if x == 236 || x == 244 { STEINZIEGEL } else if hash(x, z, 90) % 3 == 0 { MOOS_ZIEGEL } else { BRUCHSTEIN });
        }
        for x in [234, 235, 245, 246] {
            p.set(x, h, z, ERDE);
        }
        let gap = z % 24 == 12; // Durchgang zu den Terrassen
        for x in [235, 245] {
            if z % 6 == 0 {
                lantern(p, x, h + 1, z);
            } else if !gap {
                p.set(x, h + 1, z, if z % 12 < 6 { HECKE } else { BLUMEN_ROT });
            }
        }
        if !gap {
            for x in [234, 246] {
                p.set(x, h + 1, z, if z % 2 == 0 { BLUMEN_GELB } else { BLUMEN_ROT });
            }
        }
        if z % 12 == 3 {
            for (x, c) in [(233, STOFF_ROT), (247, STOFF_BLAU)] {
                p.fill(x, h + 1, z, x, h + 4, z, DUNKELHOLZ);
                p.fill(x, h + 2, z + 1, x, h + 4, z + 1, c);
            }
        }
        if z % 12 == 9 {
            for x in [233, 247] {
                p.fill(x, h + 1, z, x, h + 1, z + 1, DUNKELBRETT);
            }
        }
    }
}

// ---------- Marktplatz (y 28): Kristallbrunnen, Bazaar-Halle, Bank, Auktionshaus, Stände ----------
fn market(p: &mut Put) {
    if !p.hits(204, 196, 276, 256) {
        return;
    }
    for z in 219..=241 {
        for x in 229..=251 {
            let r = dist(x - 240, z - 230);
            if r <= 10.5 {
                p.set(x, 29, z, if r > 9.5 { STEINZIEGEL } else { POLIERT });
            }
        }
    }
    for z in 225..=235 {
        for x in 235..=245 {
            let r = dist(x - 240, z - 230);
            if r <= 4.6 {
                p.set(x, 30, z, if r > 3.6 { STEINZIEGEL } else { WASSER });
            }
        }
    }
    p.fill(239, 30, 229, 241, 33, 231, POLIERT);
    p.fill(238, 34, 228, 242, 34, 232, POLIERT);
    p.fill(238, 35, 228, 242, 35, 232, STEINZIEGEL);
    p.fill(239, 35, 229, 241, 35, 231, WASSER);
    p.fill(240, 35, 230, 240, 38, 230, MARMOR);
    p.fill(240, 39, 230, 240, 40, 230, KRISTALL);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        p.set(240 + dx, 39, 230 + dz, KRISTALL);
        p.fill(240 + 3 * dx, 31, 230 + 3 * dz, 240 + 3 * dx, 34, 230 + 3 * dz, WASSER);
    }
    // Bazaar-Halle: offenes Dach auf Pfosten, darunter 8 gestreifte Stände
    if p.hits(220, 197, 260, 215) {
        for x in (222..=258).step_by(6) {
            for z in [199, 213] {
                p.fill(x, 29, z, x, 33, z, DUNKELHOLZ);
            }
        }
        for (a, b, c, e) in [(222, 199, 258, 199), (222, 213, 258, 213), (222, 199, 222, 213), (258, 199, 258, 213)] {
            p.fill(a, 34, b, c, 34, e, DUNKELHOLZ);
        }
        for k in 0..=8 {
            p.fill(221, 35 + k, 198 + k, 259, 35 + k, 198 + k, DACH);
            p.fill(221, 35 + k, 214 - k, 259, 35 + k, 214 - k, DACH);
        }
        let cols = [
            (STOFF_ROT, STOFF_WEISS),
            (STOFF_BLAU, STOFF_WEISS),
            (STOFF_GRUEN, STOFF_GELB),
            (STOFF_GELB, STOFF_ROT),
            (STOFF_BLAU, STOFF_GELB),
            (STOFF_ROT, STOFF_GRUEN),
            (STOFF_WEISS, STOFF_BLAU),
            (STOFF_GRUEN, STOFF_WEISS),
        ];
        let goods = [SAND, BRETTER, GLAS, KUPFER, ZIEGEL, STAMM, LAUB, BRUCHSTEIN];
        for i in 0..4 {
            let x0 = 225 + i as i32 * 8;
            stall(p, x0, 200, 28, cols[i], goods[i], true);
            stall(p, x0, 208, 28, cols[i + 4], goods[i + 4], false);
        }
        for x in [231, 239, 247] {
            p.set(x, 33, 206, LATERNE);
        }
    }
    // Bank: zwei Stockwerke mit Goldzier, Schalter und begehbarem Tresor
    house(p, 206, 218, 16, 18, 28, 2, &BANKHAUS, OST);
    if p.hits(205, 217, 222, 236) {
        p.fill(211, 29, 219, 211, 31, 234, EISEN);
        p.fill(211, 29, 225, 211, 31, 227, 0);
        p.fill(207, 29, 219, 208, 30, 222, GOLD);
        p.fill(207, 29, 231, 208, 29, 234, KISTE);
        p.fill(209, 29, 219, 210, 29, 219, ROHGOLD);
        p.fill(214, 29, 220, 214, 29, 234, POLIERT);
        p.fill(214, 29, 226, 214, 29, 228, 0);
        p.fill(214, 30, 220, 214, 30, 224, GOLDZIER);
        p.fill(214, 30, 230, 214, 30, 234, GOLDZIER);
    }
    // Auktionshaus: drei Stockwerke, Turm, Schausockel
    house(p, 258, 216, 16, 20, 28, 3, &STEINHAUS, WEST);
    if p.hits(256, 214, 276, 238) {
        let items = [GOLD, KRISTALL, AURORA, BERNSTEIN, NEON_CYAN, SAPHIR, SPRACHGOLD, SMARAGD];
        for (i, (x, z)) in [(264, 219), (268, 219), (264, 223), (268, 223), (264, 229), (268, 229), (264, 233), (268, 233)].into_iter().enumerate() {
            p.set(x, 29, z, SOCKEL);
            p.set(x, 30, z, items[i]);
        }
        for y in 41..=52 {
            for z in 223..=228 {
                for x in 263..=268 {
                    let edge = x == 263 || x == 268 || z == 223 || z == 228;
                    let win = y % 4 == 2 && (x == 265 || x == 266 || z == 225 || z == 226);
                    p.set(x, y, z, if !edge { 0 } else if win { GLAS } else { STEINZIEGEL });
                }
            }
        }
        for k in 0..=3 {
            p.fill(262 + k, 53 + k, 222 + k, 269 - k, 53 + k, 229 - k, SCHIEFER);
        }
        p.set(265, 57, 225, GOLDZIER);
    }
    house(p, 210, 240, 12, 12, 28, 2, &FACHWERK, OST); // Kontor
    house(p, 262, 240, 12, 12, 28, 1, &FACHWERK, WEST); // Bäckerei
    if p.hits(262, 240, 274, 252) {
        p.fill(270, 29, 242, 272, 30, 243, BRUCHSTEIN);
        p.set(271, 29, 243, GLUT);
        p.fill(264, 29, 249, 266, 29, 249, KISTE);
        p.set(268, 29, 249, FASS);
    }
    let tents = [
        (223, 214, (STOFF_ROT, STOFF_WEISS), BRETTER),
        (223, 237, (STOFF_BLAU, STOFF_GELB), GLAS),
        (252, 214, (STOFF_GRUEN, STOFF_WEISS), SAND),
        (252, 237, (STOFF_GELB, STOFF_ROT), KUPFER),
        (226, 245, (STOFF_WEISS, STOFF_BLAU), ZIEGEL),
        (250, 245, (STOFF_ROT, STOFF_GELB), LAUB),
    ];
    for (x, z, c, g) in tents {
        stall(p, x, z, 28, c, g, true);
    }
    for (x, z) in [(219, 210), (261, 210), (219, 252), (261, 252), (232, 252), (248, 252)] {
        lantern(p, x, 29, z);
    }
}

// ---------- Schmiede- und Minion-Viertel (Westflügel, y 26) ----------
fn forge(p: &mut Put, st: WorldState) {
    if !p.hits(136, 192, 212, 332) {
        return;
    }
    house(p, 166, 196, 20, 14, 26, 2, &SCHMIEDE_STIL, SUED);
    if p.hits(164, 194, 188, 224) {
        // oben dunkles Holz, unten Stein; Esse mit Lava und Glut
        for y in 31..=34 {
            for x in 166..=185 {
                for z in [196, 209] {
                    if p.get(x, y, z) == BRUCHSTEIN {
                        p.set(x, y, z, DUNKELBRETT);
                    }
                }
            }
            for z in 196..=209 {
                for x in [166, 185] {
                    if p.get(x, y, z) == BRUCHSTEIN {
                        p.set(x, y, z, DUNKELBRETT);
                    }
                }
            }
        }
        p.fill(170, 27, 197, 182, 29, 199, BRUCHSTEIN);
        p.fill(172, 27, 198, 180, 27, 198, LAVA);
        p.fill(172, 28, 198, 180, 28, 198, GLUT);
        p.fill(172, 27, 199, 180, 28, 199, 0);
        p.fill(176, 30, 198, 176, 46, 198, BRUCHSTEIN);
        for x in [170, 176, 182] {
            p.set(x, 27, 204, AMBOSS);
        }
        p.fill(167, 27, 206, 168, 28, 208, FASS);
        // Amboss-Hof
        p.fill(166, 26, 211, 185, 26, 221, BRUCHSTEIN);
        for x in 166..=185 {
            for z in [211, 221] {
                if !(175..=177).contains(&x) {
                    p.set(x, 27, z, MOOS_ZIEGEL);
                }
            }
        }
        for z in 211..=221 {
            p.set(166, 27, z, MOOS_ZIEGEL);
            p.set(185, 27, z, MOOS_ZIEGEL);
        }
        for (x, z) in [(166, 211), (185, 211), (166, 221), (185, 221)] {
            lantern(p, x, 27, z);
        }
        p.set(170, 27, 215, AMBOSS);
        p.set(181, 27, 215, AMBOSS);
        p.set(176, 27, 218, WERKBANK);
        p.fill(168, 27, 218, 169, 27, 219, KISTE);
        p.fill(183, 27, 218, 183, 28, 218, FASS);
    }
    house(p, 152, 212, 12, 14, 26, 1, &FACHWERK, OST); // Lagerhaus
    // Minion-Werkstatt: offene Halle, Sockel für Stufe I–XI (SHOWCASE)
    if p.hits(162, 236, 194, 252) {
        p.fill(164, 26, 240, 192, 26, 250, PFLASTER);
        p.fill(164, 27, 250, 192, 31, 250, PUTZ);
        p.fill(164, 27, 240, 164, 31, 250, PUTZ);
        p.fill(192, 27, 240, 192, 31, 250, PUTZ);
        p.fill(165, 27, 241, 191, 31, 249, 0);
        for x in (164..=192).step_by(4) {
            p.fill(x, 27, 240, x, 31, 240, DUNKELHOLZ);
            p.fill(x, 27, 250, x, 31, 250, DUNKELHOLZ);
        }
        for (a, b, c, e) in [(164, 240, 192, 240), (164, 250, 192, 250), (164, 240, 164, 250), (192, 240, 192, 250)] {
            p.fill(a, 32, b, c, 32, e, DUNKELHOLZ);
        }
        for k in 0..=6 {
            p.fill(163, 33 + k, 239 + k, 193, 33 + k, 239 + k, DACH);
            p.fill(163, 33 + k, 251 - k, 193, 33 + k, 251 - k, DACH);
            for x in [164, 192] {
                p.fill(x, 33 + k, 240 + k, x, 33 + k, 250 - k, PUTZ);
            }
        }
        for k in 0..11 {
            p.set(SHOWCASE[0] + 2 * k, SHOWCASE[1], SHOWCASE[2], SOCKEL);
        }
        for x in [170, 178, 186] {
            p.set(x, 31, 247, LATERNE);
        }
        p.set(189, 27, 248, WERKBANK);
        p.fill(166, 27, 248, 167, 28, 248, KISTE);
    }
    // Mühle mit Wasserrad, Bach und Wasserfall über die Westkante
    fall(p, st, 138, 260, 179, 262, 26, 4);
    if p.hits(150, 254, 183, 268) {
        p.fill(176, 25, 256, 181, 26, 266, WASSER);
        for dy in -4..=4 {
            for dx in -4..=4 {
                let r = dist(dx, dy);
                if r <= 4.4 && (r > 3.3 || dx == 0 || dy == 0 || dx == dy || dx == -dy) {
                    p.set(158 + dx, 27 + dy, 261, DUNKELBRETT);
                }
            }
        }
        p.fill(158, 27, 258, 158, 27, 264, STAMM);
    }
    house(p, 160, 265, 12, 11, 26, 2, &FACHWERK, NORD); // Mühle
    house(p, 176, 268, 14, 12, 26, 1, &FACHWERK, SUED); // Glasbläserei
    house(p, 160, 290, 13, 12, 26, 1, &FACHWERK, NORD); // Gerberei
    house(p, 176, 296, 14, 13, 26, 2, &FACHWERK, NORD); // Seilerei
    house(p, 166, 314, 12, 10, 26, 1, &BLOCKHAUS, NORD); // Köhlerei
    // Brückenkopf West mit Portal zum Himmels-Garten
    if p.hits(148, 230, 164, 250) {
        p.fill(150, 26, 232, 159, 29, 248, BRUCHSTEIN);
        p.fill(150, 30, 232, 159, 30, 248, POLIERT);
        for k in 0..3 {
            p.fill(160 + k, 26, 237, 160 + k, 29 - k, 243, STEINZIEGEL);
        }
        arch(p, 154, 235, 30, false);
        for (x, z) in [(151, 232), (159, 232), (151, 248), (159, 248)] {
            lantern(p, x, 31, z);
        }
    }
    // Straßenlaternen alle 8 Blöcke
    for x in (152..=208).step_by(8) {
        lantern(p, x, 27, 251);
    }
    for z in (200..=328).step_by(8) {
        lantern(p, 198, 27, z);
    }
}

// ---------- Flüsterwald (Ostflügel, y 27–38) ----------
fn tree_ok(x: i32, z: i32) -> bool {
    let (dx, dz) = (x - CABIN.0, z - CABIN.1);
    district(x, z) == WALD && (154..=326).contains(&z) && x <= 325 && dx * dx + dz * dz > 14 * 14 && (x - trail_x(z)).abs() > 3 && !(x >= 309 && (z - 240).abs() < 12)
}
fn forest(p: &mut Put) {
    if !p.hits(262, 144, 336, 336) {
        return;
    }
    for gz in 0..20 {
        for gx in 0..6 {
            let (x, z) = (275 + gx * 9 + (hash(gx, gz, 61) % 5) as i32, 152 + gz * 9 + (hash(gx, gz, 62) % 5) as i32);
            if !tree_ok(x, z) || !p.hits(x - 9, z - 9, x + 9, z + 9) {
                continue;
            }
            let h = 10 + (hash(x, z, 63) % 9) as i32;
            let birch = hash(x, z, 64) % 5 < 2;
            tree(p, x, z, main_top(x, z).0, h, if birch { BIRKE } else { STAMM }, if birch { BIRKENLAUB } else { LAUB }, hash(x, z, 65));
        }
    }
    // Holzfällerhütte auf der Lichtung, nachwachsende Holzknoten (Baumstümpfe)
    house(p, 296, 262, 11, 10, 30, 1, &BLOCKHAUS, NORD);
    if p.hits(286, 252, 316, 284) {
        p.fill(297, 31, 269, 298, 31, 269, BETT);
        p.set(304, 31, 269, WERKBANK);
        p.fill(304, 31, 263, 305, 32, 263, FASS);
        p.fill(308, 31, 264, 311, 32, 266, STAMM);
        for (i, (x, z)) in [(292, 258), (310, 257), (290, 272), (313, 274), (296, 280), (306, 280)].into_iter().enumerate() {
            let y = main_top(x, z).0;
            p.fill(x, y + 1, z, x, y + 2, z, if i % 2 == 0 { KNOTEN_EICHE } else { KNOTEN_BIRKE });
        }
        lantern(p, 298, 31, 259);
    }
    for (x, z, along_x, len) in [(282, 170, true, 5), (318, 190, false, 6), (285, 302, true, 4), (312, 314, false, 5), (300, 212, true, 6)] {
        if !p.hits(x - 1, z - 1, x + len + 1, z + len + 1) {
            continue;
        }
        let y = main_top(x, z).0 + 1;
        if along_x {
            p.fill(x, y, z, x + len, y, z, STAMM);
        } else {
            p.fill(x, y, z, x, y, z + len, STAMM);
        }
        p.add(x, y + 1, z, LAUB);
    }
    for (x, z, h) in [(288, 185, 4), (305, 230, 5), (280, 250, 3), (316, 296, 4), (290, 320, 5), (310, 160, 4)] {
        mushroom(p, x, z, main_top(x, z).0, h);
    }
    // Brückenkopf Ost mit Portal zum Bibliotheks-Turm
    if p.hits(310, 228, 332, 252) {
        p.fill(312, 30, 230, 330, 30, 250, PFLASTER);
        arch(p, 326, 235, 30, false);
        for (x, z) in [(313, 231), (313, 249), (321, 231)] {
            lantern(p, x, 31, z);
        }
    }
}

// ---------- Kristallminen (Inselbauch, y 6–24) ----------
/// Stollen: x0, z0, x1, z1 (eingeschlossen), Boden. Luft von Boden+1 bis Boden+4.
const TUNNELS: [[i32; 5]; 11] = [
    [212, 170, 266, 172, 20],
    [224, 152, 226, 198, 20],
    [246, 172, 248, 178, 20],
    [256, 172, 258, 182, 20],
    [214, 182, 262, 184, 14],
    [256, 158, 258, 194, 14],
    [230, 158, 256, 160, 14],
    [256, 178, 264, 180, 14],
    [214, 160, 216, 196, 8],
    [214, 160, 250, 162, 8],
    [238, 160, 240, 170, 8],
];
/// Treppen: x0, x1, z am Anfang, z am Ende, Boden am Anfang (je Schritt einen Block tiefer).
const STAIRS: [[i32; 5]; 3] = [[246, 248, 187, 179, 28], [262, 264, 173, 178, 19], [217, 219, 181, 176, 13]];
/// Höhlen: Mitte, Halbachsen.
const CAVES: [([i32; 3], [i32; 3]); 3] = [([258, 22, 188], [8, 4, 7]), ([232, 16, 166], [9, 3, 6]), ([240, 11, 178], [11, 5, 10])];

fn mines(p: &mut Put) {
    if !p.hits(205, 144, 272, 202) {
        return;
    }
    for t in TUNNELS {
        p.fill(t[0], t[4] + 1, t[1], t[2], t[4] + 4, t[3], 0);
    }
    for s in STAIRS {
        let dir = (s[3] - s[2]).signum();
        for i in 0..=(s[3] - s[2]).abs() {
            let (z, f) = (s[2] + i * dir, s[4] - i);
            p.fill(s[0], f + 1, z, s[1], f + 4, z, 0);
        }
    }
    p.fill(222, 21, 144, 228, 26, 156, 0); // Höhlenmund in der Nordklippe
    for (i, (c, r)) in CAVES.into_iter().enumerate() {
        cave(p, c, r, i);
    }
    for t in TUNNELS {
        tunnel_deco(p, t);
    }
    // Förderturm über der Treppe, Schienen und Erzkarren
    if p.hits(242, 176, 252, 200) {
        for (x, z) in [(244, 178), (250, 178), (244, 188), (250, 188)] {
            p.fill(x, 29, z, x, 40, z, DUNKELHOLZ);
        }
        for y in [34, 40] {
            for (a, b, c, e) in [(244, 178, 250, 178), (244, 188, 250, 188), (244, 178, 244, 188), (250, 178, 250, 188)] {
                p.fill(a, y, b, c, y, e, DUNKELHOLZ);
            }
        }
        p.fill(244, 41, 178, 250, 41, 188, DUNKELBRETT);
        for dy in -3..=3 {
            for dz in -3..=3 {
                let r = dist(dz, dy);
                if r <= 3.2 && (r > 2.2 || dz == 0 || dy == 0) {
                    p.set(247, 45 + dy, 183 + dz, DUNKELBRETT);
                }
            }
        }
        p.fill(246, 45, 183, 248, 45, 183, STAMM);
        p.fill(247, 28, 188, 247, 28, 198, SCHIENE);
        p.set(247, 29, 193, KISTE);
        p.set(246, 29, 196, FASS);
        for (x, z) in [(243, 186), (251, 186)] {
            lantern(p, x, 29, z);
        }
    }
    // Brückenkopf Nord mit Portal zur Prüfungs-Zitadelle
    if p.hits(228, 148, 252, 164) {
        p.fill(230, 28, 151, 250, 29, 160, BRUCHSTEIN);
        p.fill(230, 30, 151, 250, 30, 160, POLIERT);
        p.fill(236, 28, 161, 244, 29, 161, STEINZIEGEL);
        arch(p, 235, 154, 30, true);
        for (x, z) in [(231, 151), (249, 151), (231, 160), (249, 160)] {
            lantern(p, x, 31, z);
        }
    }
    for (x, z) in [(212, 158), (262, 156), (214, 192), (266, 198), (230, 190)] {
        blob(p, x, 29, z, 1.7, FELS);
    }
}

/// Schienen in der Mitte, Stützbalken alle 6, Laternen alle 8, Erzadern in den Wänden.
fn tunnel_deco(p: &mut Put, t: [i32; 5]) {
    let [x0, z0, x1, z1, f] = t;
    if !p.hits(x0 - 1, z0 - 1, x1 + 1, z1 + 1) {
        return;
    }
    let lvl = match f {
        20 => 0,
        14 => 1,
        _ => 2,
    };
    let along_x = x1 - x0 > z1 - z0;
    let (a0, a1, s0, s1) = if along_x { (x0, x1, z0, z1) } else { (z0, z1, x0, x1) };
    let at = |a: i32, s: i32| if along_x { (a, s) } else { (s, a) };
    let mid = (s0 + s1) / 2;
    for a in a0..=a1 {
        let (x, z) = at(a, mid);
        p.set(x, f, z, SCHIENE);
        for y in f + 1..=f + 4 {
            for s in [s0 - 1, s1 + 1] {
                let (x, z) = at(a, s);
                let r = hash(x * 3 + y, z, 70 + lvl as u32);
                if !rocky(p.get(x, y, z)) {
                    continue;
                }
                if a % 6 == 0 {
                    p.set(x, y, z, DUNKELHOLZ);
                } else if r % 5 == 0 {
                    p.set(x, y, z, ore(lvl, r / 5));
                }
            }
        }
        if a % 6 == 0 {
            for s in s0 - 1..=s1 + 1 {
                let (x, z) = at(a, s);
                if rocky(p.get(x, f + 5, z)) {
                    p.set(x, f + 5, z, DUNKELBRETT);
                }
            }
        }
        if a % 8 == 4 {
            let (x, z) = at(a, mid);
            if rocky(p.get(x, f + 5, z)) {
                p.set(x, f + 5, z, LATERNE);
            }
        }
    }
}

/// Höhle als verrauschtes Ellipsoid: unten ein See, an der Decke Tropfsteine, in der Wand Erze.
fn cave(p: &mut Put, c: [i32; 3], r: [i32; 3], lvl: usize) {
    let (rx, ry, rz) = (r[0] + 2, r[1] + 2, r[2] + 2);
    if !p.hits(c[0] - rx, c[2] - rz, c[0] + rx, c[2] + rz) {
        return;
    }
    let water = c[1] - r[1] + 1;
    for dy in -ry..=ry {
        for dz in -rz..=rz {
            for dx in -rx..=rx {
                let (x, y, z) = (c[0] + dx, c[1] + dy, c[2] + dz);
                let n = (dx * dx) as f32 / (r[0] * r[0]) as f32 + (dy * dy) as f32 / (r[1] * r[1]) as f32 + (dz * dz) as f32 / (r[2] * r[2]) as f32;
                let w = 1.0 + (noise(x as f32 / 4.0, z as f32 / 4.0 + y as f32 * 0.37, 71) - 0.5) * 0.5;
                if n <= w {
                    p.set(x, y, z, if y <= water { WASSER } else { 0 });
                } else if n <= w * 1.35 {
                    let h = hash(x * 5 + y, z, 72);
                    if h % 4 == 0 && rocky(p.get(x, y, z)) {
                        p.set(x, y, z, ore(lvl, h / 4));
                    }
                }
            }
        }
    }
    for dz in -r[2]..=r[2] {
        for dx in -r[0]..=r[0] {
            let (x, z) = (c[0] + dx, c[2] + dz);
            let h = hash(x, z, 73);
            if h % 5 != 0 {
                continue;
            }
            if let Some(t) = (water + 2..=c[1] + r[1] + 1).rev().find(|&y| p.get(x, y, z) == 0) {
                for k in 0..1 + (h / 5 % 3) as i32 {
                    if t - k > water + 1 {
                        p.set(x, t - k, z, TIEFENFELS);
                    }
                }
            }
        }
    }
    if lvl == 2 {
        for (dx, dz) in [(-4, 2), (3, -3), (5, 4), (-2, -5)] {
            p.fill(c[0] + dx, water + 1, c[2] + dz, c[0] + dx, water + 2, c[2] + dz, KRISTALL);
        }
    }
    p.set(c[0], water + 1, c[2] + r[2] - 2, LATERNE);
}

// ---------- Himmelsruinen und Parkour-Turm (Nordwest-Klippe, y 35–60) ----------
fn ruins(p: &mut Put, st: WorldState) {
    if !p.hits(146, 146, 216, 196) {
        return;
    }
    // Parkour-Turm: Säule, Spirale aus Trittsteinen, Schatztruhe auf y 61
    p.fill(169, 36, 164, 171, 59, 166, MOOS_ZIEGEL);
    for y in (38..59).step_by(5) {
        p.fill(169, y, 164, 171, y, 166, STEINZIEGEL);
    }
    for k in 0..24 {
        let a = k as f32 * 0.5;
        let (x, z) = (170 + (4.0 * a.cos()).round() as i32, 165 + (4.0 * a.sin()).round() as i32);
        p.set(x, 36 + k, z, if k % 6 == 5 { GLUEH } else { STEINZIEGEL });
    }
    p.fill(167, 60, 162, 173, 60, 168, POLIERT);
    p.set(170, 61, 165, SCHATZ);
    for (x, z) in [(167, 162), (173, 162), (167, 168), (173, 168)] {
        p.set(x, 61, z, LATERNE);
    }
    // Ruinen-Tempel: zerfallene Mauern, Säulenstümpfe, Bewuchs
    for z in 170..=182 {
        for x in 176..=188 {
            if x == 176 || x == 188 || z == 170 || z == 182 {
                let hgt = (hash(x, z, 80) % 5) as i32;
                if hgt > 0 {
                    p.fill(x, 36, z, x, 35 + hgt, z, if hash(x, z, 81) % 3 == 0 { MOOS_BRUCH } else { MOOS_ZIEGEL });
                    if hash(x, z, 82) % 4 == 0 {
                        p.set(x, 36 + hgt, z, LAUB);
                    }
                }
            } else {
                p.set(x, 35, z, if hash(x, z, 83) % 4 == 0 { MOOS_ZIEGEL } else { POLIERT });
            }
        }
    }
    for (x, z) in [(179, 173), (185, 173), (179, 179), (185, 179)] {
        let hgt = 4 + (hash(x, z, 84) % 5) as i32;
        p.fill(x, 36, z, x, 35 + hgt, z, STEINZIEGEL);
        p.add(x, 36 + hgt, z, LAUB);
    }
    for (x0, z0, x1, z1) in [(154, 176, 164, 176), (190, 154, 190, 162), (156, 154, 162, 154)] {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let hgt = (hash(x, z, 85) % 4) as i32;
                p.fill(x, 36, z, x, 35 + hgt, z, MOOS_ZIEGEL);
            }
        }
    }
    // schwebende Kristallsplitter
    for (x, y, z) in [(160, 48, 158), (186, 52, 160), (176, 45, 186), (158, 44, 178), (193, 55, 172)] {
        p.fill(x, y - 1, z, x, y + 1, z, KRISTALL);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            p.set(x + dx, y, z + dz, KRISTALL);
        }
    }
    // Sprungpolster zu den Satelliten
    for (x, z, _) in LAUNCH_PADS {
        disc(p, x, 35, z, 0.0, 2.3, POLIERT);
        p.fill(x - 1, 35, z - 1, x + 1, 35, z + 1, SPRUNGPAD);
    }
    // Bogenbrücke über die Schlucht, Treppe hinab zu den Minen
    p.fill(197, 35, 176, 208, 35, 178, STEINZIEGEL);
    for z in [175, 179] {
        p.fill(197, 36, z, 208, 36, z, MOOS_ZIEGEL);
        p.set(197, 37, z, LATERNE);
        p.set(208, 37, z, LATERNE);
    }
    for x in 197..=208 {
        let t = 1 + ((x as f32 - 202.5).powi(2) / 4.0) as i32;
        p.fill(x, (35 - t).max(22), 176, x, 34, 178, STEINZIEGEL);
    }
    for k in 1..=7 {
        p.fill(208 + k, 28, 176, 208 + k, 35 - k, 178, BRUCHSTEIN);
    }
    // Wasserfälle in die Schlucht
    fall(p, st, 197, 158, 200, 163, 35, 2);
    fall(p, st, 168, 187, 172, 190, 35, 2);
}

// ---------- Satelliten ----------
fn library(p: &mut Put, st: WorldState) {
    if !p.hits(362, 210, 423, 271) {
        return;
    }
    let (cx, cz) = (393, 240);
    disc(p, cx, 30, cz, 0.0, 11.0, POLIERT);
    for k in 0..8 {
        if k != 4 {
            let a = k as f32 * PI / 4.0;
            lantern(p, cx + (11.5 * a.cos()).round() as i32, 31, cz + (11.5 * a.sin()).round() as i32);
        }
    }
    // Turm: Wand mit Fenstern, Regale innen, Galerien, Wendeltreppe um die Mittelsäule
    for y in 31..=56 {
        for dz in -7..=7 {
            for dx in -7..=7 {
                let r = dist(dx, dz);
                let (x, z) = (cx + dx, cz + dz);
                let v = if r > 6.4 {
                    continue
                } else if r > 5.2 {
                    if (y % 8 == 3 || y % 8 == 4) && (dx == 0 || dz == 0) {
                        GLAS
                    } else if y % 8 == 0 {
                        POLIERT
                    } else {
                        STEINZIEGEL
                    }
                } else if y == 56 {
                    POLIERT
                } else if r > 4.2 {
                    if y % 8 == 6 {
                        BRETTER
                    } else if hash(x, z, y as u32) % 4 != 0 {
                        REGAL
                    } else {
                        0
                    }
                } else if r > 3.6 && y % 8 == 6 {
                    BRETTER
                } else {
                    0
                };
                p.set(x, y, z, v);
            }
        }
        if y < 56 {
            p.set(cx, y, cz, STAMM);
            let a = (y - 31) as f32 * 0.55;
            for rad in [2.0f32, 3.0] {
                p.set(cx + (rad * a.cos()).round() as i32, y, cz + (rad * a.sin()).round() as i32, BRETTER);
            }
        }
    }
    p.fill(cx - 7, 31, cz - 1, cx - 4, 33, cz + 1, 0);
    for y in 57..=62 {
        let ry = 7.5 - (y - 57) as f32 * 1.25;
        for dz in -8..=8 {
            for dx in -8..=8 {
                let r = dist(dx, dz);
                if r <= ry && r > ry - 1.4 {
                    p.set(cx + dx, y, cz + dz, if (y + dx + dz).rem_euclid(3) == 0 { LILA } else { SCHIEFER });
                }
            }
        }
    }
    p.fill(cx, 61, cz, cx, 63, cz, KRISTALL);
    // schwebende Bücherregale und Fach-Trainer
    for k in 0..8i32 {
        if k == 4 {
            continue;
        }
        let a = k as f32 * PI / 4.0 + 0.3;
        let (x, z) = (cx + (13.0 * a.cos()).round() as i32, cz + (13.0 * a.sin()).round() as i32);
        let y = 38 + (k % 3) * 4;
        p.fill(x, y, z, x, y + 1, z, REGAL);
        p.set(x, y - 1, z, GLUEH);
    }
    for (i, b) in [SAPHIR, SPRACHGOLD, SMARAGD, KUPFER, GLUEH, BRETTER, GOLD].into_iter().enumerate() {
        let a = -1.2 + i as f32 * 0.4;
        let (x, z) = (cx + (9.0 * a.cos()).round() as i32, cz + (9.0 * a.sin()).round() as i32);
        p.set(x, 31, z, SOCKEL);
        p.set(x, 32, z, b);
    }
    arch(p, 374, 235, 30, false);
    let [ax, ay, az] = ALTAR_AT;
    p.set(ax, ay, az, ALTAR);
    fall(p, st, 397, 254, 400, 268, 30, 6);
}

fn zitadelle(p: &mut Put, st: WorldState) {
    if !p.hits(207, 52, 273, 118) {
        return;
    }
    // Graben aus Lava über Kristallgrund, Zugbrücke im Süden
    for z in 64..=100 {
        for x in 220..=260 {
            if (222..=258).contains(&x) && (66..=98).contains(&z) {
                continue;
            }
            p.set(x, 28, z, KRISTALL);
            p.set(x, 29, z, LAVA);
            p.set(x, 30, z, if (238..=242).contains(&x) && z >= 99 { DUNKELBRETT } else { LAVA });
        }
    }
    // Mauern mit Zinnen, Hof
    for z in 68..=96 {
        for x in 224..=256 {
            if x <= 225 || x >= 255 || z <= 69 || z >= 95 {
                p.fill(x, 31, z, x, 38, z, STEINZIEGEL);
                p.set(x, 34, z, TIEFENFELS);
                if (x == 224 || x == 256 || z == 68 || z == 96) && (x + z) % 2 == 0 {
                    p.set(x, 39, z, STEINZIEGEL);
                }
            } else {
                p.set(x, 30, z, PFLASTER);
            }
        }
    }
    p.fill(238, 31, 95, 242, 34, 96, 0);
    p.set(240, 36, 96, GOLDZIER);
    for (tx, tz) in [(224, 68), (256, 68), (224, 96), (256, 96)] {
        p.fill(tx - 2, 31, tz - 2, tx + 2, 44, tz + 2, STEINZIEGEL);
        for dz in -2..=2i32 {
            for dx in -2..=2i32 {
                if (dx + dz) % 2 == 0 && (dx.abs() == 2 || dz.abs() == 2) {
                    p.set(tx + dx, 45, tz + dz, STEINZIEGEL);
                }
            }
        }
        p.set(tx, 45, tz, LATERNE);
    }
    // Bergfried
    p.fill(234, 31, 72, 246, 42, 82, STEINZIEGEL);
    p.fill(235, 31, 73, 245, 41, 81, 0);
    p.fill(239, 31, 82, 241, 33, 82, 0);
    for x in (234..=246).step_by(2) {
        p.set(x, 43, 72, STEINZIEGEL);
        p.set(x, 43, 82, STEINZIEGEL);
    }
    for z in [76, 78] {
        p.fill(234, 36, z, 234, 37, z, GLAS);
        p.fill(246, 36, z, 246, 37, z, GLAS);
    }
    p.fill(240, 43, 77, 240, 46, 77, DUNKELHOLZ);
    p.fill(241, 45, 77, 242, 46, 77, LILA);
    p.set(240, 40, 77, LATERNE);
    // Parkour-Arena (Ost): Säulen, jede einen Block höher
    for k in 0..8 {
        let (x, z) = (250 + k % 2, 72 + 2 * k);
        p.fill(x, 31, z, x, 31 + k, z, POLIERT);
        p.set(x, 32 + k, z, GLUEH);
    }
    // Splitter-Kristalle (nachwachsend) im Westhof
    for (x, z) in [(229, 86), (231, 88), (229, 90), (232, 92), (230, 84)] {
        p.set(x, 31, z, SPLITTER_ADER);
    }
    for (x, z) in [(228, 72), (252, 92), (228, 92)] {
        lantern(p, x, 31, z);
    }
    let [bx, by, bz] = BOSS_ALTAR_AT;
    p.set(bx, by, bz, BOSS_ALTAR);
    arch(p, 235, 106, 30, true);
    let [kx, ky, kz] = KATA_PORTAL_AT;
    p.fill(kx, ky, kz, kx, ky + 1, kz, KATA_PORTAL);
    fall(p, st, 238, 52, 241, 63, 30, 6);
}

fn garden(p: &mut Put, st: WorldState) {
    if !p.hits(62, 212, 118, 268) {
        return;
    }
    let (ox, oz) = (84, 236);
    // Blumenbeete (vor den Bäumen, damit Stämme durchgehen)
    for z in (247..=256).step_by(3) {
        for x in 84..=100 {
            p.set(x, 31, z, if (x + z) % 2 == 0 { BLUMEN_ROT } else { BLUMEN_GELB });
        }
    }
    // Sternwarte unter der Glaskuppel
    disc(p, ox, 30, oz, 0.0, 8.5, POLIERT);
    disc(p, ox, 31, oz, 7.0, 8.0, STEINZIEGEL);
    dome(p, ox, 31, oz, 7.5, GLAS, POLIERT);
    p.fill(ox + 6, 31, oz - 1, ox + 9, 33, oz + 1, 0);
    p.set(ox, 31, oz, SOCKEL);
    for k in 0..4 {
        p.set(ox + k, 32 + k, oz - k, KUPFER);
    }
    p.set(ox - 3, 31, oz + 3, REGAL);
    p.set(ox, 37, oz, LATERNE);
    // Kirschbäume und Glühlaub-Bäume
    for (x, z) in [(76, 225), (98, 226), (76, 254), (96, 256)] {
        tree(p, x, z, 30, 6 + (hash(x, z, 66) % 3) as i32, STAMM, KIRSCHLAUB, 67);
    }
    for (x, z) in [(102, 249), (80, 248), (93, 224)] {
        tree(p, x, z, 30, 7, BIRKE, GLUEHLAUB, 68);
    }
    // versteckte Schatztruhen: hinter der Hecke, in der Sternwarte, unter dem Kirschbaum
    p.fill(72, 31, 229, 78, 32, 229, HECKE);
    p.fill(78, 31, 226, 78, 32, 229, HECKE);
    p.set(75, 31, 227, SCHATZ);
    p.set(ox - 5, 31, oz, SCHATZ);
    p.set(97, 31, 257, SCHATZ);
    for (x, z) in [(88, 244), (100, 232)] {
        lantern(p, x, 31, z);
    }
    arch(p, 106, 235, 30, false);
    fall(p, st, 62, 250, 73, 252, 30, 8);
}

// ---------- Himmelsbrücken ----------
fn bridges(p: &mut Put) {
    deck(p, 316, 239, 371, 241, true);
    deck(p, 239, 109, 241, 152, false);
    deck(p, 109, 239, 152, 241, true);
}
fn deck(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, along_x: bool) {
    if !p.hits(x0 - 1, z0 - 1, x1 + 1, z1 + 1) {
        return;
    }
    p.fill(x0, 30, z0, x1, 30, z1, BRETTER);
    p.fill(x0, 31, z0, x1, 33, z1, 0);
    let (a0, a1) = if along_x { (x0, x1) } else { (z0, z1) };
    let sides = if along_x { [z0 - 1, z1 + 1] } else { [x0 - 1, x1 + 1] };
    for a in (a0..=a1).step_by(5) {
        for side in sides {
            let (x, z) = if along_x { (a, side) } else { (side, a) };
            p.fill(x, 29, z, x, 32, z, DUNKELHOLZ);
            if (a - a0) % 10 == 0 {
                p.set(x, 33, z, LATERNE);
            }
        }
        if along_x {
            p.fill(a, 29, z0, a, 29, z1, DUNKELBRETT);
        } else {
            p.fill(x0, 29, a, x1, 29, a, DUNKELBRETT);
        }
    }
}

// ---------- Großprojekte (Weltzustand) ----------
fn projects(p: &mut Put, st: WorldState) {
    if st.projects & P_BRIDGE != 0 && p.hits(236, 326, 244, 374) {
        // Aussichtssteg hinter dem Tempel nach Süden
        p.fill(239, 32, 329, 241, 32, 373, BRETTER);
        for z in (329..=373).step_by(6) {
            for x in [238, 242] {
                p.fill(x, 31, z, x, 33, z, DUNKELHOLZ);
                if (z - 329) % 12 == 0 {
                    p.set(x, 34, z, LATERNE);
                }
            }
        }
    }
    if st.projects & P_TOWER != 0 && p.hits(256, 180, 268, 192) {
        // Ziegelturm mit Wendeltreppe im Minenviertel
        let (mx, mz, top) = (262, 186, 50);
        for y in 29..=top {
            for z in mz - 3..=mz + 3 {
                for x in mx - 3..=mx + 3 {
                    let wall = x == mx - 3 || x == mx + 3 || z == mz - 3 || z == mz + 3;
                    let v = if !wall {
                        0
                    } else if (y % 6 == 3 || y % 6 == 4) && (x == mx || z == mz) {
                        GLAS
                    } else {
                        ZIEGEL
                    };
                    p.set(x, y, z, v);
                }
            }
            let k = (y - 29) % 16;
            let (sx, sz) = match k {
                0..=3 => (-2 + k, -2),
                4..=7 => (2, -2 + (k - 4)),
                8..=11 => (2 - (k - 8), 2),
                _ => (-2, 2 - (k - 12)),
            };
            p.set(mx + sx, y, mz + sz, BRETTER);
        }
        p.fill(mx, 29, mz - 3, mx, 30, mz - 3, 0);
        for z in mz - 4..=mz + 4 {
            for x in mx - 4..=mx + 4 {
                if (x - mx).abs() > 2 || (z - mz).abs() > 2 {
                    p.set(x, top + 1, z, SCHIEFER);
                }
                if (x - mx).abs() == 4 && (z - mz).abs() == 4 {
                    p.set(x, top + 2, z, GLUEH);
                }
            }
        }
    }
    if st.projects & P_ISLAND != 0 && p.hits(226, 370, 254, 398) {
        p.fill(240, 33, 384, 240, 35, 384, FELS);
        p.set(240, 36, 384, GLUEH);
    }
}

// ================= Privatinsel =================
/// 38×38-Hauptinsel mit Fachwerk-Starthütte (Bett, Regal, Laterne, Truhe, Werkbank), Minion-Sockel, Eichen, Portal zum Hub;
/// Hängebrücke zur 16×16-Rohstoffinsel mit Bruchstein- und Splitter-Generator zwischen Wasser und Lava, Holzknoten-Eichen.
fn private(p: &mut Put, st: WorldState) {
    let (a, c) = (p.ox / SLOT * SLOT, p.oz / SLOT * SLOT);
    if !p.hits(a + 20, c + 24, a + 94, c + 72) {
        return;
    }
    let g = GROUND;
    p.fill(a + 47, g, c + 39, a + 48, g, c + 51, WEG);
    p.fill(a + 47, g, c + 53, a + 49, g, c + 60, WEG);
    p.fill(a + 50, g, c + 48, a + 66, g, c + 48, WEG);
    house(p, a + 41, c + 31, 14, 8, g, 1, &FACHWERK, SUED);
    p.fill(a + 42, g + 1, c + 32, a + 43, g + 1, c + 32, BETT);
    p.fill(a + 53, g + 1, c + 32, a + 53, g + 3, c + 34, REGAL);
    p.set(a + 51, g + 1, c + 32, TRUHE);
    p.set(a + 50, g + 1, c + 32, WERKBANK);
    p.set(a + 44, g + 1, c + 35, FASS);
    p.fill(a + 45, g, c + 34, a + 50, g, c + 36, TEPPICH_ROT);
    p.set(a + 51, g + 1, c + 46, SOCKEL);
    arch(p, a + 48, c + 62, g, false);
    for (x, z) in [(45, 40), (51, 40), (64, 46), (64, 50)] {
        lantern(p, a + x, g + 1, c + z);
    }
    for x in [41, 42, 43, 52, 53, 54] {
        p.set(a + x, g, c + 39, if x % 2 == 0 { BLUMEN_ROT } else { BLUMEN_GELB });
    }
    for (x, z) in [(34, 44), (62, 36), (35, 59), (62, 58)] {
        tree(p, a + x, c + z, g, 5 + (hash(a + x, c + z, 66) % 2) as i32, STAMM, LAUB, 69);
    }
    // Hängebrücke
    p.fill(a + 66, g, c + 47, a + 73, g, c + 49, BRETTER);
    for x in [66, 72] {
        for z in [46, 50] {
            p.fill(a + x, g, c + z, a + x, g + 3, c + z, DUNKELHOLZ);
        }
    }
    for z in [46, 50] {
        p.fill(a + 67, g + 2, c + z, a + 71, g + 2, c + z, DUNKELBRETT);
    }
    // Rohstoffinsel: Becken mit Wasser | Generatoren | Lava
    p.fill(a + 76, g, c + 44, a + 84, g, c + 52, STEINZIEGEL);
    for z in 44..=52 {
        for x in 76..=84 {
            let v = if x == 76 || x == 84 || z == 44 || z == 52 {
                STEINZIEGEL
            } else {
                match x {
                    77 | 78 => WASSER,
                    79 | 81 => POLIERT,
                    80 => match z {
                        46 | 47 => GENERATOR,
                        48 => KRISTALL,
                        49 | 50 => SPLITTER_GEN,
                        _ => POLIERT,
                    },
                    _ => LAVA,
                }
            };
            p.set(a + x, g + 1, c + z, v);
        }
    }
    for (x, z) in [(86, 42), (86, 54)] {
        p.fill(a + x, g + 1, c + z, a + x, g + 4, c + z, KNOTEN_EICHE);
        blob(p, a + x, g + 5, c + z, 2.2, LAUB);
    }
    lantern(p, a + 75, g + 1, c + 43);
    fall(p, st, a + 24, c + 60, a + 31, c + 61, g, 6);
}
