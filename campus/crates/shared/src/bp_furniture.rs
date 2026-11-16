//! Abschnitt 8: Bau-Helfer für Möbel, Fenster, Dächer und Außenflächen. Eigentum: Architekt (Änderungen nur über CONTRACT-8.md).
//! Regeln: alles schreibt über Put (auf das Stück beschnitten). Entscheidungen nur aus Weltkoordinaten, `surf()`, `hash()` und
//! `p.get` derselben Spalte – so wird jede Stelle in jedem Stück gleich gebaut, ohne Nähte an Stückgrenzen.
#![allow(dead_code)]
use crate::blueprints::{keep_free, surf, Put};
use crate::shape::facing;
use crate::world::*;

/// Blickrichtung ausgerichteter Teilblöcke (wie shape: 0 Nord -z, 1 Ost +x, 2 Süd +z, 3 West -x).
/// Achtung: NICHT die Tür-Konstanten aus blueprints (NORD 0, SUED 1, WEST 2, OST 3)!
pub(crate) const ZU_N: u8 = 0;
pub(crate) const ZU_O: u8 = 1;
pub(crate) const ZU_S: u8 = 2;
pub(crate) const ZU_W: u8 = 3;

/// Stuhl bei (x, y, z); `zu` = Blickrichtung des Sitzenden (Lehne gegenüber).
pub(crate) fn chair(p: &mut Put, x: i32, y: i32, z: i32, zu: u8) {
    p.set(x, y, z, facing(STUHL_N, zu));
}

/// Tisch bei (x, y, z) mit Stühlen an den Seiten `sides` (Bits shape::N/O/S/W), alle mit Blick zum Tisch;
/// `top` (≠ 0) steht obendrauf (z. B. BUECHERSTAPEL).
pub(crate) fn table_set(p: &mut Put, x: i32, y: i32, z: i32, sides: u8, top: u8) {
    p.set(x, y, z, TISCH);
    for (bit, dx, dz, zu) in [(crate::shape::N, 0, -1, ZU_S), (crate::shape::O, 1, 0, ZU_W), (crate::shape::S, 0, 1, ZU_N), (crate::shape::W, -1, 0, ZU_O)] {
        if sides & bit != 0 {
            chair(p, x + dx, y, z + dz, zu);
        }
    }
    if top != 0 {
        p.set(x, y + 1, z, top);
    }
}

/// Sitzbank-Reihe (Gerade oder Rechteck) x0..=x1 × z0..=z1 auf Höhe y, Blick `zu`.
pub(crate) fn bench(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, y: i32, zu: u8) {
    p.fill(x0, y, z0, x1, y, z1, facing(SITZBANK_N, zu));
}

/// Teppich x0..=x1 × z0..=z1 in Höhe y (= erste Luftschicht über dem Fußboden), nur in Luft. Bordüre entsteht von selbst am Rand.
pub(crate) fn rug(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, y: i32, v: u8) {
    for z in z0.min(z1)..=z0.max(z1) {
        for x in x0.min(x1)..=x0.max(x1) {
            p.add(x, y, z, v);
        }
    }
}

/// Zaun/Geländer (ZAUN, GELAENDER) x0..=x1 × z0..=z1 auf Höhe y, nur in Luft. Riegel verbinden sich von selbst.
pub(crate) fn fence_line(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, y: i32, v: u8) {
    rug(p, x0, z0, x1, z1, y, v);
}

/// Laternenmast, 3 hoch: zwei Zaunpfosten, oben LATERNENMAST (Licht). y = erste Luftschicht über dem Boden.
pub(crate) fn lamp_post(p: &mut Put, x: i32, y: i32, z: i32) {
    p.fill(x, y, z, x, y + 1, z, ZAUN);
    p.set(x, y + 2, z, LATERNENMAST);
}

/// Fenster: Scheiben (SCHEIBE) von y bis y + h - 1 in der Spalte (x, z). Die Scheibe richtet sich nach den Wänden daneben.
pub(crate) fn window(p: &mut Put, x: i32, y: i32, z: i32, h: i32) {
    p.fill(x, y, z, x, y + h - 1, z, SCHEIBE);
}

/// Satteldach aus Treppen über x0..=x1 × z0..=z1 (Überstand eingerechnet), Traufe auf Höhe y, First entlang x (`along_x`)
/// bzw. entlang z. `north`: Nord-Block der Treppen (TREPPE_DACH_N, TREPPE_EICHE_N, TREPPE_STEIN_N), `ridge`: Firstblock
/// (STUFE_DACH, …), bei gerader Breite liegt der First auf den beiden obersten Treppenreihen. Liefert die Höhe des Firsts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stair_roof(p: &mut Put, x0: i32, z0: i32, x1: i32, z1: i32, y: i32, along_x: bool, north: u8, ridge: u8) -> i32 {
    let (x0, x1, z0, z1) = (x0.min(x1), x0.max(x1), z0.min(z1), z0.max(z1));
    let (s0, s1) = if along_x { (z0, z1) } else { (x0, x1) };
    let mut k = 0;
    loop {
        let (a, b) = (s0 + k, s1 - k);
        let yr = y + k;
        if a > b {
            return yr - 1;
        }
        if a == b {
            if along_x {
                p.fill(x0, yr, a, x1, yr, a, ridge);
            } else {
                p.fill(a, yr, z0, a, yr, z1, ridge);
            }
            return yr;
        }
        // untere Seite steigt zum First hin: Nordreihe blickt nach Süden, Südreihe nach Norden (bzw. West → Ost, Ost → West)
        let (up_a, up_b) = if along_x { (ZU_S, ZU_N) } else { (ZU_O, ZU_W) };
        if along_x {
            p.fill(x0, yr, a, x1, yr, a, facing(north, up_a));
            p.fill(x0, yr, b, x1, yr, b, facing(north, up_b));
        } else {
            p.fill(a, yr, z0, a, yr, z1, facing(north, up_a));
            p.fill(b, yr, z0, b, yr, z1, facing(north, up_b));
        }
        k += 1;
    }
}

/// Wildblumen (MOHN, SONNENBLUME, KORNBLUME, LEUCHTKLEE) und HOHES_GRAS auf Wiese: in r = [x0, z0, x1, z1] (eingeschlossen),
/// wo bei y WIESE liegt und darüber Luft ist, auf `pct` Prozent der Felder (Hälfte Gras); nie an keep_free-Stellen des Hubs.
pub(crate) fn meadow(p: &mut Put, r: [i32; 4], y: i32, pct: u32, seed: u32) {
    if !p.hits(r[0], r[1], r[2], r[3]) {
        return;
    }
    for z in r[1].max(p.oz)..=r[3].min(p.oz + 15) {
        for x in r[0].max(p.ox)..=r[2].min(p.ox + 15) {
            let h = hash(x, z, seed);
            if h % 100 >= pct || p.get(x, y, z) != WIESE || p.get(x, y + 1, z) != 0 || (in_hub(x, z) && keep_free(x, z)) {
                continue;
            }
            let v = if h / 100 % 2 == 0 { HOHES_GRAS } else { BLUMEN[(h / 200 % 4) as usize] };
            p.set(x, y + 1, z, v);
        }
    }
}

/// Zaun an Klippen: jede Inselspalte in r = [x0, z0, x1, z1], neben der (4er-Nachbarschaft) Leere ist, bekommt über ihrer
/// Oberfläche einen Zaun `v` (nur in Luft, nicht auf Wasser/Lava, nie an keep_free-Stellen des Hubs, nie in `skip`-Rechtecken).
pub(crate) fn cliff_fence(p: &mut Put, st: WorldState, r: [i32; 4], v: u8, skip: &[[i32; 4]]) {
    if !p.hits(r[0], r[1], r[2], r[3]) {
        return;
    }
    for z in r[1].max(p.oz)..=r[3].min(p.oz + 15) {
        for x in r[0].max(p.ox)..=r[2].min(p.ox + 15) {
            let Some(s) = surf(x, z, st) else { continue };
            // 8er-Nachbarschaft: an runden Rändern schließt die Kette auch über Ecken (sonst Lücke zwischen Diagonal-Pfosten)
            let edge = (-1..=1).any(|dz| (-1..=1).any(|dx| surf(x + dx, z + dz, st).is_none()));
            let skipped = skip.iter().any(|q| x >= q[0] && x <= q[2] && z >= q[1] && z <= q[3]) || (in_hub(x, z) && keep_free(x, z));
            if edge && !skipped && FLAGS[p.get(x, s.h, z) as usize] & F_LIQUID == 0 {
                p.add(x, s.h + 1, z, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helfer() {
        let mut b = vec![0u8; VOL];
        let p = &mut Put { b: &mut b, ox: 0, oz: 0 };
        p.fill(0, 9, 0, 15, 9, 15, WIESE);
        table_set(p, 5, 10, 5, crate::shape::N | crate::shape::S, BUECHERSTAPEL);
        assert_eq!((p.get(5, 10, 5), p.get(5, 10, 4), p.get(5, 10, 6), p.get(5, 11, 5)), (TISCH, STUHL_S, STUHL_N, BUECHERSTAPEL));
        // Dach 5 breit (z 0..4): Reihen auf 20, 21, First auf 22
        assert_eq!(stair_roof(p, 0, 0, 3, 4, 20, true, TREPPE_DACH_N, STUFE_DACH), 22);
        assert_eq!((p.get(1, 20, 0), p.get(1, 20, 4), p.get(1, 21, 1), p.get(1, 22, 2)), (TREPPE_DACH_S, TREPPE_DACH_N, TREPPE_DACH_S, STUFE_DACH));
        // 4 breit: zwei Treppenreihen oben nebeneinander, kein First-Block
        assert_eq!(stair_roof(p, 8, 0, 11, 3, 30, false, TREPPE_STEIN_N, STUFE_STEIN), 31);
        assert_eq!((p.get(9, 31, 1), p.get(10, 31, 1)), (TREPPE_STEIN_O, TREPPE_STEIN_W));
        meadow(p, [0, 0, 15, 15], 9, 100, 3);
        assert!(p.get(1, 10, 12) != 0 && FLAGS[p.get(1, 10, 12) as usize] & F_SHAPE != 0 && !solid(p.get(1, 10, 12)));
        assert_eq!(p.get(5, 11, 5), BUECHERSTAPEL, "Wiese nur dort, wo Wiese liegt");
        lamp_post(p, 14, 20, 14);
        assert_eq!((p.get(14, 21, 14), p.get(14, 22, 14)), (ZAUN, LATERNENMAST));
    }
}
