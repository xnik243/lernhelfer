//! Eingefrorene Privatinsel-Vorlage der Weltfassung 2 (Stand vor Abschnitt 8), NUR für die Migration auf WORLD_VERSION 3:
//! Der Server vergleicht gespeicherte Privatinsel-Stücke mit dieser alten Grundwelt, um Spieler-Bauten zu erkennen.
//! NIE ändern – jede Änderung verfälscht die Erkennung (der Test `fingerabdruck` wacht darüber).
use crate::blueprints::Put;
use crate::world::*;
use std::f32::consts::PI;

/// Stück der alten Privatinsel-Grundwelt (Gelände + Vorlage). Hub-Stücke und reine Luft: leerer Vec.
pub fn gen_chunk(cx: i32, cz: i32) -> Vec<u8> {
    let (ox, oz) = (cx * 16, cz * 16);
    if in_hub(ox, oz) || !touches(ox, oz) {
        return Vec::new();
    }
    let mut b = vec![0u8; VOL];
    let p = &mut Put { b: &mut b, ox, oz };
    for z in oz..oz + 16 {
        for x in ox..ox + 16 {
            if let Some(s) = surf(x, z) {
                ground(p, x, z, s);
            }
        }
    }
    private(p);
    if b.iter().all(|&v| v == 0) {
        return Vec::new();
    }
    b
}

/// Migrations-Regel je Zelle (Privatinsel-Stück, WORLD_VERSION 2 → 3): `saved` = gespeichert, `old` = alte Grundwelt
/// (gen_chunk hier), `new` = neue Grundwelt (world::gen_chunk). Leere Vecs = reine Luft.
/// Eine Zelle gilt als unberührt, wenn saved == old; nur dann kommt die neue Vorlage hin. Alles, was Spieler gesetzt,
/// abgebaut oder angebaut haben (saved ≠ old), bleibt – ebenso Truhen, Rüstungsständer und Minion-Sockel (an ihnen hängen
/// Datenbank-Zeilen). Ein zweiter Lauf ändert nichts mehr (idempotent), solange `new` gleich bleibt.
pub fn merge(saved: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    let at = |v: &[u8], k: usize| v.get(k).copied().unwrap_or(0);
    (0..VOL)
        .map(|k| {
            let s = at(saved, k);
            if s != at(old, k) || matches!(s, TRUHE | GROSSE_TRUHE | STAENDER | SOCKEL) {
                s
            } else {
                at(new, k)
            }
        })
        .collect()
}

fn touches(ox: i32, oz: i32) -> bool {
    let (a, c) = (ox / SLOT * SLOT, oz / SLOT * SLOT);
    a + 20 <= ox + 15 && a + 94 >= ox && c + 24 <= oz + 15 && c + 72 >= oz
}

#[derive(Clone, Copy)]
struct Surf {
    h: i32,
    top: u8,
    d: f32,
}
fn rr(x: i32, z: i32, r: [i32; 4], rad: f32) -> f32 {
    let (px, pz) = (x as f32 + 0.5, z as f32 + 0.5);
    let (mx, mz) = ((r[0] + r[2]) as f32 / 2.0, (r[1] + r[3]) as f32 / 2.0);
    let (hx, hz) = ((r[2] - r[0]) as f32 / 2.0, (r[3] - r[1]) as f32 / 2.0);
    let (qx, qz) = ((px - mx).abs() - (hx - rad), (pz - mz).abs() - (hz - rad));
    -(qx.max(0.0).hypot(qz.max(0.0)) + qx.max(qz).min(0.0) - rad)
}
fn wob(x: i32, z: i32, s: u32) -> f32 {
    (noise(x as f32 / 7.0, z as f32 / 7.0, s) - 0.4) * 3.0
}
fn surf(x: i32, z: i32) -> Option<Surf> {
    if !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) || in_hub(x, z) {
        return None;
    }
    let (lx, lz) = (x % SLOT, z % SLOT);
    let d = rr(lx, lz, [29, 29, 67, 67], 5.0) + wob(x, z, 31);
    if d > 0.0 {
        return Some(Surf { h: GROUND, top: WIESE, d });
    }
    let d = rr(lx, lz, [73, 40, 89, 56], 3.0) + wob(x, z, 32) * 0.5;
    (d > 0.0).then_some(Surf { h: GROUND, top: WIESE, d })
}
fn ground(p: &mut Put, x: i32, z: i32, s: Surf) {
    let drip = if hash(x, z, 23) % 11 == 0 { 2 + (hash(x, z, 24) % 6) as i32 } else { 0 };
    let d = s.d;
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

const SUED: u8 = 1;
const WEST: u8 = 2;
fn lantern(p: &mut Put, x: i32, y: i32, z: i32) {
    p.fill(x, y, z, x, y + 1, z, DUNKELHOLZ);
    p.set(x, y + 2, z, LATERNE);
}
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
/// Fachwerk-Haus, Tür im Süden (nur was die alte Starthütte brauchte; gleiche Reihenfolge wie damals).
#[allow(clippy::too_many_arguments)]
fn house(p: &mut Put, x0: i32, z0: i32, w: i32, d: i32, y: i32, floors: i32, door: u8) {
    let (wall, frame, roof, base, floor) = (PUTZ, DUNKELHOLZ, DACH, STEINZIEGEL, BRETTER);
    let (x1, z1) = (x0 + w - 1, z0 + d - 1);
    if !p.hits(x0 - 2, z0 - 2, x1 + 2, z1 + 2) {
        return;
    }
    let top = y + 4 * floors;
    let along_x = w >= d;
    let span = if along_x { d } else { w };
    let ridge = top + 1 + (span + 1) / 2;
    p.fill(x0, y - 6, z0, x1, y, z1, base);
    p.fill(x0 + 1, y, z0 + 1, x1 - 1, y, z1 - 1, floor);
    p.fill(x0, y + 1, z0, x1, top, z1, wall);
    p.fill(x0 + 1, y + 1, z0 + 1, x1 - 1, ridge, z1 - 1, 0);
    for f in 1..=floors {
        let yb = y + 4 * f;
        for (a, b, c, e) in [(x0, z0, x1, z0), (x0, z1, x1, z1), (x0, z0, x0, z1), (x1, z0, x1, z1)] {
            p.fill(a, yb, b, c, yb, e, frame);
        }
        if f < floors {
            p.fill(x0 + 1, yb, z0 + 1, x1 - 1, yb, z1 - 1, floor);
        }
    }
    for x in x0..=x1 {
        if (x - x0) % 3 == 0 || x == x1 {
            p.fill(x, y + 1, z0, x, top, z0, frame);
            p.fill(x, y + 1, z1, x, top, z1, frame);
        }
    }
    for z in z0..=z1 {
        if (z - z0) % 3 == 0 || z == z1 {
            p.fill(x0, y + 1, z, x0, top, z, frame);
            p.fill(x1, y + 1, z, x1, top, z, frame);
        }
    }
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
    let dw = if (if door < WEST { w } else { d }) >= 14 { 3 } else { 2 };
    let (dx0, dz0, ox, oz) = match door {
        0 => (x0 + w / 2 - 1, z0, 0, -1),
        SUED => (x0 + w / 2 - 1, z1, 0, 1),
        WEST => (x0, z0 + d / 2 - 1, -1, 0),
        _ => (x1, z0 + d / 2 - 1, 1, 0),
    };
    let (dx1, dz1) = if door < WEST { (dx0 + dw - 1, dz0) } else { (dx0, dz0 + dw - 1) };
    p.fill(dx0, y + 1, dz0, dx1, y + 3, dz1, 0);
    p.fill(dx0, y, dz0, dx1, y, dz1, floor);
    p.fill(dx0 + ox, y + 1, dz0 + oz, dx1 + ox, y + 3, dz1 + oz, 0);
    if door < WEST {
        p.set(dx0 - 1, y + 3, dz0 + oz, LATERNE);
        p.set(dx1 + 1, y + 3, dz0 + oz, SCHILD);
    } else {
        p.set(dx0 + ox, y + 3, dz0 - 1, LATERNE);
        p.set(dx0 + ox, y + 3, dz1 + 1, SCHILD);
    }
    let (l0, l1, s0, s1) = if along_x { (x0 - 1, x1 + 1, z0 - 1, z1 + 1) } else { (z0 - 1, z1 + 1, x0 - 1, x1 + 1) };
    let gables = if along_x { [x0, x1] } else { [z0, z1] };
    let mut k = 0;
    while s0 + k <= s1 - k {
        let yr = top + 1 + k;
        for l in l0..=l1 {
            for sv in [s0 + k, s1 - k] {
                let (x, z) = if along_x { (l, sv) } else { (sv, l) };
                p.set(x, yr, z, roof);
            }
        }
        for g in gables {
            for sv in s0 + k + 1..s1 - k {
                let (x, z) = if along_x { (g, sv) } else { (sv, g) };
                p.set(x, yr, z, wall);
            }
        }
        k += 1;
    }
    p.fill(x1 - 1, y + 1, z0 + 1, x1 - 1, ridge + 2, z0 + 1, BRUCHSTEIN);
}
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
#[allow(clippy::too_many_arguments)]
fn fall(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, top: i32, bot: i32) {
    if !p.hits(x0, z0, x1, z1) {
        return;
    }
    for z in z0.max(p.oz)..=z1.min(p.oz + 15) {
        for x in x0.max(p.ox)..=x1.min(p.ox + 15) {
            match surf(x, z) {
                None => p.fill(x, bot, z, x, top, z, WASSER),
                Some(s) => p.set(x, s.h, z, WASSER),
            }
        }
    }
}
fn private(p: &mut Put) {
    let (a, c) = (p.ox / SLOT * SLOT, p.oz / SLOT * SLOT);
    if !p.hits(a + 20, c + 24, a + 94, c + 72) {
        return;
    }
    let g = GROUND;
    p.fill(a + 47, g, c + 39, a + 48, g, c + 51, WEG);
    p.fill(a + 47, g, c + 53, a + 49, g, c + 60, WEG);
    p.fill(a + 50, g, c + 48, a + 66, g, c + 48, WEG);
    house(p, a + 41, c + 31, 14, 8, g, 1, SUED);
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
    p.fill(a + 66, g, c + 47, a + 73, g, c + 49, BRETTER);
    for x in [66, 72] {
        for z in [46, 50] {
            p.fill(a + x, g, c + z, a + x, g + 3, c + z, DUNKELHOLZ);
        }
    }
    for z in [46, 50] {
        p.fill(a + 67, g + 2, c + z, a + 71, g + 2, c + z, DUNKELBRETT);
    }
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
    fall(p, a + 24, c + 60, a + 31, c + 61, g, 6);
}

#[cfg(test)]
mod tests {
    /// FNV-1a über alle Stücke des Platzes (5, 0); festgehalten, als die Vorlage noch die heutige Grundwelt war.
    fn fp() -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for cz in 0..6 {
            for cx in 30..36 {
                for &b in super::gen_chunk(cx, cz).iter() {
                    h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
                }
            }
        }
        h
    }
    #[test]
    fn fingerabdruck() {
        let h = fp();
        eprintln!("V2-FINGERABDRUCK: {h:#x}");
        assert_eq!(h, FP, "blueprints_v2 darf sich nie ändern");
    }
    const FP: u64 = 0x9f6a_4e90_8fa7_47b1;
    #[test]
    fn merge_regel() {
        use crate::world::*;
        let mut old = vec![WIESE; VOL];
        old[3] = TRUHE;
        let mut new = vec![0u8; VOL];
        new[4] = BRETTER;
        let mut saved = old.clone();
        saved[0] = FELS; // gesetzt
        saved[1] = 0; // abgebaut
        let m = super::merge(&saved, &old, &new);
        assert_eq!(&m[..6], &[FELS, 0, 0, TRUHE, BRETTER, 0]);
        assert_eq!(super::merge(&m, &old, &new), m, "idempotent");
        assert_eq!(super::merge(&[], &[], &new), new, "unberührte Luft bekommt die neue Vorlage");
    }
}
