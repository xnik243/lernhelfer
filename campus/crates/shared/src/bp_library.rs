//! Bibliotheks-Turm (Satellit, Boden y 30, Mitte 393/240): Insel-Außen und Atrium innen. Eigentum: Agent „library“ (CONTRACT-8.md).
//! Innen (Versatz dx/dz zur Mitte, Tschebyschow-Abstand `cheb`): Hülle (r > 5,2 oder cheb ≥ 5) = Steinziegel außen, Regale innen
//! bündig an der Wand; Galerien auf y 38 und y 46 über cheb ≥ 2 (Geländer auf dem Ring cheb 2, Mitte cheb ≤ 1 offen);
//! Wendeltreppe auf dem Ring cheb 4 (Ecken über (±3, ±3)), 16 Stufen von y 31 bis y 46, je Stufe 3 Luft darüber.
use crate::blueprints::*;
use crate::bp_furniture::{cliff_fence, lamp_post, meadow, rug, table_set, window, ZU_N, ZU_O, ZU_S, ZU_W};
use crate::shape::facing;
use crate::world::*;
use std::f32::consts::PI;

/// Gallerie-Böden (Oberkante = Gehhöhe + 1).
const GALERIEN: [i32; 2] = [38, 46];
/// Stufen der Wendeltreppe: Stufe k liegt auf y 31 + k in Zelle SPIRAL[k].
const STUFEN: usize = 16;

/// Ring der Wendeltreppe (Versatz zur Mitte), 32 Zellen im Uhrzeigersinn (von oben): Nordseite (−3..3, −4) nach Osten,
/// Ecke (3, −3), Ostseite (4, −3..3) nach Süden, Ecke (3, 3), … – je zwei Nachbarn teilen eine Seite.
fn spiral() -> [(i32, i32); 32] {
    let mut r = [(0, 0); 32];
    for s in 0..4 {
        for t in 0..8 {
            let c = if t < 7 { (t as i32 - 3, -4) } else { (3, -3) };
            r[s * 8 + t] = (0..s).fold(c, |(x, z), _| (-z, x));
        }
    }
    r
}

/// Blickrichtung (ZU_*) eines Schritts (a, b) zwischen Nachbarzellen.
fn dir(a: i32, b: i32) -> u8 {
    match (a, b) {
        (0, -1) => ZU_N,
        (1, 0) => ZU_O,
        (0, 1) => ZU_S,
        _ => ZU_W,
    }
}

// ---------- Satelliten ----------
pub(crate) fn library(p: &mut Put, st: WorldState) {
    if !p.hits(362, 210, 423, 271) {
        return;
    }
    let (cx, cz) = (393, 240);
    disc(p, cx, 30, cz, 0.0, 11.0, POLIERT);
    for k in 0..8 {
        if k != 4 {
            let a = k as f32 * PI / 4.0;
            lamp_post(p, cx + (11.5 * a.cos()).round() as i32, 31, cz + (11.5 * a.sin()).round() as i32);
        }
    }
    // Turm: Hülle (Wand außen, Regale innen), Galerie-Böden, Podest, Decke
    for y in 31..=56 {
        for dz in -7..=7 {
            for dx in -7..=7 {
                let r = dist(dx, dz);
                if r > 6.4 {
                    continue;
                }
                let cheb = dx.abs().max(dz.abs());
                let v = if y == 56 {
                    POLIERT
                } else if r > 5.2 || cheb >= 5 {
                    let outer = crate::shape::DIRS.iter().any(|&(a, b)| dist(dx + a, dz + b) > 6.4);
                    if GALERIEN.contains(&y) {
                        POLIERT
                    } else if outer {
                        STEINZIEGEL
                    } else {
                        REGAL
                    }
                } else if GALERIEN.contains(&y) && cheb >= 2 {
                    if cheb == 2 {
                        DUNKELBRETT
                    } else {
                        BRETTER
                    }
                } else if y == 31 && r <= 2.0 {
                    if r > 1.5 {
                        GOLDZIER
                    } else {
                        POLIERT
                    }
                } else if y == 31 && r <= 3.2 {
                    STUFE_STEIN
                } else {
                    0
                };
                p.set(cx + dx, y, cz + dz, v);
            }
        }
    }
    // Fenster: je Himmelsrichtung 3 breit auf jeder Etage (West unten = Eingang), Regal dahinter frei für das Licht
    for y0 in [34, 42, 50] {
        for t in -1..=1 {
            for (dx, dz) in [(6, t), (-6, t), (t, 6), (t, -6)] {
                if dx == -6 && y0 == 34 {
                    p.set(cx + dx, y0, cz + dz, GOLDZIER);
                    continue;
                }
                window(p, cx + dx, y0, cz + dz, 3);
                let (ix, iz) = if dx.abs() == 6 { (dx - dx.signum(), dz) } else { (dx, dz - dz.signum()) };
                p.fill(cx + ix, y0, cz + iz, cx + ix, y0 + 2, cz + iz, 0);
            }
        }
    }
    // Galerie-Geländer am offenen Lichthof
    for y in GALERIEN {
        for dz in -2..=2i32 {
            for dx in -2..=2i32 {
                if dx.abs().max(dz.abs()) == 2 {
                    p.set(cx + dx, y + 1, cz + dz, GELAENDER);
                }
            }
        }
    }
    // Wendeltreppe: Stufe k auf y 31 + k, Blickrichtung = Laufrichtung (tangential nach oben), 3 Luft darüber (öffnet die Galerie)
    let ring = spiral();
    for k in 0..STUFEN {
        let ((dx, dz), (px, pz)) = (ring[k], ring[(k + 31) % 32]);
        let y = 31 + k as i32;
        p.set(cx + dx, y, cz + dz, facing(TREPPE_EICHE_N, dir(dx - px, dz - pz)));
        p.fill(cx + dx, y + 1, cz + dz, cx + dx, y + 3, cz + dz, 0);
    }
    // Geländer an der offenen Innenseite, wo es darunter mindestens 3 Blöcke frei hinabgeht
    for (k, &(dx, dz)) in ring.iter().enumerate().take(STUFEN) {
        let (ix, iz) = if dx.abs() == 4 {
            (dx - dx.signum(), dz)
        } else if dz.abs() == 4 {
            (dx, dz - dz.signum())
        } else {
            continue;
        };
        if ring.contains(&(ix, iz)) {
            continue;
        }
        let (x, z, y) = (cx + ix, cz + iz, 31 + k as i32);
        if (y - 2..=y + 2).all(|u| p.get(x, u, z) == 0) {
            p.add(x, y + 1, z, GELAENDER);
        }
    }
    // Kronleuchter: unter jeder Galerie und unter der Decke
    for (sx, sz) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        for y in [37, 45, 55] {
            p.set(cx + sx, y, cz + sz, KRONLEUCHTER);
        }
    }
    p.set(cx, 55, cz, KRONLEUCHTER);
    // Lese-Ecken: Tische mit Bücherstapeln, Stühle, Webteppiche (nie auf der Treppe oder ihren Öffnungen)
    let (ns, ow) = (crate::shape::N | crate::shape::S, crate::shape::O | crate::shape::W);
    table_set(p, cx, 31, cz + 4, ow, BUECHERSTAPEL);
    rug(p, cx - 3, cz + 4, cx + 3, cz + 4, 31, WEBTEPPICH_GOLD);
    table_set(p, cx - 4, 39, cz, ns, BUECHERSTAPEL);
    table_set(p, cx, 39, cz + 4, ow, BUECHERSTAPEL);
    rug(p, cx - 3, cz - 2, cx - 3, cz + 2, 39, WEBTEPPICH_ROT);
    rug(p, cx - 2, cz + 3, cx + 2, cz + 3, 39, WEBTEPPICH_GOLD);
    table_set(p, cx - 4, 47, cz, ns, BUECHERSTAPEL);
    table_set(p, cx, 47, cz + 4, ow, BUECHERSTAPEL);
    table_set(p, cx, 47, cz - 4, ow, BUECHERSTAPEL);
    rug(p, cx - 3, cz - 2, cx - 3, cz + 2, 47, WEBTEPPICH_GOLD);
    rug(p, cx - 2, cz + 3, cx + 2, cz + 3, 47, WEBTEPPICH_ROT);
    rug(p, cx - 2, cz - 3, cx + 2, cz - 3, 47, WEBTEPPICH_ROT);
    // Eingang West mit rotem Läufer
    p.fill(cx - 7, 31, cz - 1, cx - 4, 33, cz + 1, 0);
    rug(p, cx - 7, cz - 1, cx - 4, cz + 1, 31, WEBTEPPICH_ROT);
    // Dach
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
    // Blumenwiese auf der Insel, nicht vor Eingang und Portal (Streifen z 233..247 westlich des Turms bleibt frei)
    meadow(p, [362, 210, 423, 232], 30, 30, 81);
    meadow(p, [362, 248, 423, 271], 30, 30, 82);
    meadow(p, [400, 233, 423, 247], 30, 30, 83);
    // Zaun an der Klippe; Brückenende im Westen bleibt offen, der Wasserfall ist flüssig und wird übersprungen
    cliff_fence(p, st, [362, 210, 423, 271], ZAUN, &[[366, 236, 376, 244]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{body_hits, step_lift};
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Schwerkraft: absinken, bis der Körper aufsetzt.
    fn settle(get: &impl Fn(i32, i32, i32) -> u8, p: &mut [f32; 3]) {
        for _ in 0..4000 {
            let q = [p[0], p[1] - 0.01, p[2]];
            if body_hits(get, q) {
                return;
            }
            *p = q;
        }
        panic!("fällt ins Leere bei {p:?}");
    }

    /// Gehen in kleinen Schritten zum Ziel (x, z): blockiert nur Stufen-Kanten → hinauf (wie der Browser), sonst Wand = Fehler.
    fn walk(get: &impl Fn(i32, i32, i32) -> u8, p: &mut [f32; 3], to: (f32, f32)) {
        for _ in 0..400 {
            let (ddx, ddz) = (to.0 - p[0], to.1 - p[2]);
            let len = (ddx * ddx + ddz * ddz).sqrt();
            if len < 1e-3 {
                return;
            }
            let s = len.min(0.05) / len;
            let q = [p[0] + ddx * s, p[1], p[2] + ddz * s];
            if body_hits(get, q) {
                match step_lift(get, q) {
                    Some(h) if h > 0.0 => *p = [q[0], q[1] + h, q[2]],
                    _ => panic!("Wand bei {q:?} auf dem Weg nach {to:?}"),
                }
            } else {
                *p = q;
            }
            settle(get, p);
            assert!(!body_hits(get, *p), "Kopf frei bei {p:?}");
        }
        panic!("Ziel {to:?} nicht erreicht, stehe bei {p:?}");
    }

    #[test]
    fn wendeltreppe_begehbar() {
        let st = WorldState::default();
        let cache = RefCell::new(HashMap::<(i32, i32), Vec<u8>>::new());
        let get = |x: i32, y: i32, z: i32| -> u8 {
            if !(0..CH).contains(&y) || x < 0 || z < 0 {
                return 0;
            }
            let mut m = cache.borrow_mut();
            let c = m.entry((x >> 4, z >> 4)).or_insert_with(|| gen_chunk(x >> 4, z >> 4, st));
            if c.is_empty() {
                0
            } else {
                c[idx(x & 15, y, z & 15)]
            }
        };
        let (cx, cz) = (393, 240);
        let ring = spiral();
        let mid = |(dx, dz): (i32, i32)| ((cx + dx) as f32 + 0.5, (cz + dz) as f32 + 0.5);
        let at = |(dx, dz): (i32, i32), y: i32| get(cx + dx, y, cz + dz);

        assert_eq!(get(ALTAR_AT[0], ALTAR_AT[1], ALTAR_AT[2]), ALTAR);
        assert!(solid(get(cx, 31, cz + 1)) && solid(get(cx + 1, 31, cz)), "Podest");
        assert!((31..=33).all(|y| !solid(get(cx - 6, y, cz)) && !solid(get(cx - 5, y, cz + 1))), "Eingang West frei");
        assert_eq!((get(cx + 2, 37, cz + 2), get(cx, 55, cz)), (KRONLEUCHTER, KRONLEUCHTER));
        assert_eq!(get(cx + 6, 34, cz), SCHEIBE);
        assert_eq!((get(cx - 4, 39, cz), get(cx - 4, 40, cz)), (TISCH, BUECHERSTAPEL));
        for (k, &c) in ring.iter().enumerate().take(STUFEN) {
            assert!(matches!(at(c, 31 + k as i32), TREPPE_EICHE_N..=TREPPE_EICHE_W), "Stufe {k} ist eine Eichentreppe");
        }
        assert!((34..=45).any(|y| (-3..=3).any(|t| get(cx + t, y, cz - 3) == GELAENDER || get(cx + 3, y, cz + t) == GELAENDER)), "Geländer");

        // vom Erdgeschoss (Ringzelle vor der ersten Stufe) Stufe für Stufe hinauf
        let s = mid(ring[31]);
        let mut p = [s.0, 31.0, s.1];
        assert!(!body_hits(&get, p), "Startfeld frei");
        settle(&get, &mut p);
        assert!((p[1] - 31.0).abs() < 0.05, "Start auf dem Boden: {p:?}");
        for (k, &c) in ring.iter().enumerate().take(STUFEN) {
            walk(&get, &mut p, mid(c));
            let top = 32.0 + k as f32;
            assert!((p[1] - top).abs() < 0.05, "Stufe {k}: Füße {} statt {top}", p[1]);
            if k == 7 {
                // Abstecher auf die erste Galerie und zurück
                walk(&get, &mut p, mid((2, -3)));
                assert!((p[1] - 39.0).abs() < 0.05 && solid(get(cx + 2, 38, cz - 3)), "Galerie 1: {p:?}");
                walk(&get, &mut p, mid(c));
            }
        }
        // oben auf die zweite Galerie
        walk(&get, &mut p, mid((3, 2)));
        assert!((p[1] - 47.0).abs() < 0.05 && solid(get(cx + 3, 46, cz + 2)), "Galerie 2: {p:?}");
    }
}
