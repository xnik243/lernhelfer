//! Abschnitt 8: Teilblock-Modelle (Möbel, Fenster, Lampen, Flora). EINE Tabelle für den Mesher im Browser und für die
//! Kollision – beide lesen dieselben Kästen, damit Sehen und Anstoßen übereinstimmen.
//! Maße in 1/16 Block. Ausrichtung: Modelle sind für Blickrichtung Nord (-z) beschrieben; `_O/_S/_W`-Blöcke drehen sie
//! um 90°/180°/270° im Uhrzeigersinn (von oben gesehen: Nord → Ost → Süd → West).
use crate::world::*;

/// Ein Kasten [x0, y0, z0, x1, y1, z1] in 1/16 Block (x0 < x1 …, alles 0..=16). `mat`: Block, dessen Farbe/Muster/Flags
/// der Kasten trägt (aus `parts` nie 0; F_CLEAR des Stoffs = durchsichtige Liste). `cross`: statt Kasten zwei senkrechte,
/// beidseitige Flächen über die beiden Diagonalen der Grundfläche x0..x1 × z0..z1 (Pflanzen); stoßen nie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub b: [u8; 6],
    pub mat: u8,
    pub cross: bool,
}
const fn p(b: [u8; 6], mat: u8) -> Part {
    Part { b, mat, cross: false }
}
const fn x(b: [u8; 6], mat: u8) -> Part {
    Part { b, mat, cross: true }
}

/// Wie das Modell an Nachbarn anschließt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conn {
    None,
    /// Zaun/Geländer: Pfosten, Riegel zu jedem Zaun-Nachbarn oder deckenden Vollwürfel.
    Fence,
    /// Scheibe/Gitter: liegt entlang x, außer die Nachbarn sitzen nur in z-Richtung (dann entlang z).
    Pane,
    /// Teppich: Bordüre nur an Seiten ohne gleichen Teppich daneben.
    Rug,
}

pub struct Model {
    /// Nord-Ausrichtung; `mat` 0 = der Block selbst.
    pub parts: &'static [Part],
    pub conn: Conn,
    /// Stufe: Kanten bis STEP_HEIGHT über den Füßen ersteigt man im Gehen (Treppe, Stufe, Tresen, Goldstapel).
    pub step: bool,
}

/// Maske der Nachbarn: Bit 0 Nord (-z), 1 Ost (+x), 2 Süd (+z), 3 West (-x) – dieselbe Reihenfolge wie das Feld `n` in conn_mask.
pub const N: u8 = 1;
pub const O: u8 = 2;
pub const S: u8 = 4;
pub const W: u8 = 8;
/// Versatz (dx, dz) der Nachbarn in der Reihenfolge N, O, S, W.
pub const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

pub const STEP_HEIGHT: f32 = 0.55;
/// Körper der Figur: halbe Breite, Höhe (wie bisher im Browser).
pub const HALF_W: f32 = 0.3;
pub const BODY_H: f32 = 1.79;

const NONE: Conn = Conn::None;
pub static STAIRS: Model = Model { parts: &[p([0, 0, 0, 16, 8, 16], 0), p([0, 8, 0, 16, 16, 8], 0)], conn: NONE, step: true };
pub static SLAB: Model = Model { parts: &[p([0, 0, 0, 16, 8, 16], 0)], conn: NONE, step: true };
pub static COUNTER: Model = Model { parts: &[p([1, 0, 1, 15, 7, 15], DUNKELBRETT), p([0, 7, 0, 16, 8, 16], 0)], conn: NONE, step: true };
pub static TABLE: Model = Model {
    parts: &[p([0, 14, 0, 16, 16, 16], 0), p([1, 0, 1, 3, 14, 3], 0), p([13, 0, 1, 15, 14, 3], 0), p([1, 0, 13, 3, 14, 15], 0), p([13, 0, 13, 15, 14, 15], 0)],
    conn: NONE,
    step: false,
};
/// Stuhl mit Blick nach Norden: Lehne im Süden, Sitz auf 7..9 (0,45 Block).
pub static CHAIR: Model = Model {
    parts: &[
        p([2, 0, 2, 4, 7, 4], 0),
        p([12, 0, 2, 14, 7, 4], 0),
        p([2, 0, 12, 4, 7, 14], 0),
        p([12, 0, 12, 14, 7, 14], 0),
        p([2, 7, 2, 14, 9, 14], 0),
        p([2, 9, 12, 14, 16, 14], 0),
    ],
    conn: NONE,
    step: false,
};
/// Sitzbank mit Blick nach Norden: nebeneinander gestellt eine durchgehende Bank.
pub static BENCH: Model = Model { parts: &[p([0, 7, 3, 16, 9, 13], 0), p([1, 0, 4, 3, 7, 12], 0), p([13, 0, 4, 15, 7, 12], 0), p([0, 9, 11, 16, 15, 13], 0)], conn: NONE, step: false };
/// Teppich: Mitte im eigenen Stoff; die Bordüre fügt `parts` an offenen Seiten hinzu.
pub static RUG: Model = Model { parts: &[], conn: Conn::Rug, step: false };
pub static CHANDELIER: Model = Model {
    parts: &[
        p([7, 10, 7, 9, 16, 9], 0),
        p([2, 8, 2, 14, 10, 14], 0),
        p([3, 10, 3, 5, 13, 5], LAMPENGLAS),
        p([11, 10, 3, 13, 13, 5], LAMPENGLAS),
        p([3, 10, 11, 5, 13, 13], LAMPENGLAS),
        p([11, 10, 11, 13, 13, 13], LAMPENGLAS),
        p([6, 4, 6, 10, 8, 10], LAMPENGLAS),
    ],
    conn: NONE,
    step: false,
};
pub static LAMP_POST: Model = Model {
    parts: &[p([6, 0, 6, 10, 2, 10], 0), p([7, 2, 7, 9, 11, 9], 0), p([5, 11, 5, 11, 12, 11], 0), p([6, 12, 6, 10, 15, 10], LAMPENGLAS), p([5, 15, 5, 11, 16, 11], 0)],
    conn: NONE,
    step: false,
};
/// Hängt am Block darüber (Decke, Ast).
pub static WALL_LAMP: Model = Model {
    parts: &[p([7, 12, 7, 9, 16, 9], 0), p([5, 11, 5, 11, 12, 11], 0), p([6, 5, 6, 10, 11, 10], LAMPENGLAS), p([5, 4, 5, 11, 5, 11], 0)],
    conn: NONE,
    step: false,
};
/// Pfosten; die Riegel (FENCE_ARM, nach Norden) dreht `parts` zu jedem verbundenen Nachbarn.
pub static FENCE: Model = Model { parts: &[p([6, 0, 6, 10, 16, 10], 0)], conn: Conn::Fence, step: false };
static FENCE_ARM: [Part; 2] = [p([7, 6, 0, 9, 9, 6], 0), p([7, 12, 0, 9, 15, 6], 0)];
/// Scheibe entlang x (Ebene z = 8, 2/16 = 0,125 dick) mit Holzrahmen und Mittelsprosse.
pub static PANE: Model = Model {
    parts: &[
        p([0, 0, 6, 16, 1, 10], DUNKELBRETT),
        p([0, 15, 6, 16, 16, 10], DUNKELBRETT),
        p([7, 1, 6, 9, 15, 10], DUNKELBRETT),
        p([0, 1, 7, 7, 15, 9], GLAS),
        p([9, 1, 7, 16, 15, 9], GLAS),
    ],
    conn: Conn::Pane,
    step: false,
};
pub static BARS: Model = Model {
    parts: &[p([1, 0, 7, 3, 16, 9], 0), p([5, 0, 7, 7, 16, 9], 0), p([9, 0, 7, 11, 16, 9], 0), p([13, 0, 7, 15, 16, 9], 0), p([0, 2, 7, 16, 3, 9], 0), p([0, 13, 7, 16, 14, 9], 0)],
    conn: Conn::Pane,
    step: false,
};
/// Markise mit Blick nach Norden: hinten (Süden) hoch, vorn tief, vier Stufen, Streifen im eigenen Stoff und Weiß.
pub static AWNING: Model = Model {
    parts: &[
        p([0, 13, 12, 4, 15, 16], 0),
        p([4, 13, 12, 8, 15, 16], STOFF_WEISS),
        p([8, 13, 12, 12, 15, 16], 0),
        p([12, 13, 12, 16, 15, 16], STOFF_WEISS),
        p([0, 10, 8, 4, 12, 12], 0),
        p([4, 10, 8, 8, 12, 12], STOFF_WEISS),
        p([8, 10, 8, 12, 12, 12], 0),
        p([12, 10, 8, 16, 12, 12], STOFF_WEISS),
        p([0, 7, 4, 4, 9, 8], 0),
        p([4, 7, 4, 8, 9, 8], STOFF_WEISS),
        p([8, 7, 4, 12, 9, 8], 0),
        p([12, 7, 4, 16, 9, 8], STOFF_WEISS),
        p([0, 4, 0, 4, 6, 4], 0),
        p([4, 4, 0, 8, 6, 4], STOFF_WEISS),
        p([8, 4, 0, 12, 6, 4], 0),
        p([12, 4, 0, 16, 6, 4], STOFF_WEISS),
    ],
    conn: NONE,
    step: false,
};
pub static BARREL: Model = Model { parts: &[p([3, 0, 3, 13, 14, 13], 0), p([2, 2, 2, 14, 3, 14], KOHLE), p([2, 11, 2, 14, 12, 14], KOHLE)], conn: NONE, step: false };
pub static BOOKS: Model = Model { parts: &[p([3, 0, 4, 13, 2, 12], TEPPICH_ROT), p([4, 2, 3, 12, 4, 12], STOFF_BLAU), p([5, 4, 5, 11, 6, 11], STOFF_GRUEN)], conn: NONE, step: false };
pub static GOLD_STACK: Model = Model { parts: &[p([2, 0, 3, 14, 3, 13], 0), p([4, 3, 5, 12, 6, 11], 0)], conn: NONE, step: true };
pub static POPPY: Model = Model { parts: &[x([4, 0, 4, 12, 10, 12], HOHES_GRAS), p([6, 9, 6, 10, 12, 10], 0)], conn: NONE, step: false };
pub static SUNFLOWER: Model = Model { parts: &[x([5, 0, 5, 11, 14, 11], HOHES_GRAS), p([4, 12, 4, 12, 15, 12], 0)], conn: NONE, step: false };
pub static CORNFLOWER: Model = Model { parts: &[x([5, 0, 5, 11, 9, 11], HOHES_GRAS), p([6, 8, 6, 10, 11, 10], 0)], conn: NONE, step: false };
pub static CLOVER: Model = Model { parts: &[x([2, 0, 2, 14, 5, 14], 0)], conn: NONE, step: false };
pub static GRASS: Model = Model { parts: &[x([1, 0, 1, 15, 13, 15], 0)], conn: NONE, step: false };

/// Modell und Drehung (0 N, 1 O, 2 S, 3 W) eines Teilblocks; None für gewöhnliche Würfel.
pub fn def(id: u8) -> Option<(&'static Model, u8)> {
    Some(match id {
        SCHEIBE => (&PANE, 0),
        GITTERSTAB => (&BARS, 0),
        TREPPE_EICHE_N..=TREPPE_DACH_W => (&STAIRS, (id - TREPPE_EICHE_N) % 4),
        STUFE_EICHE..=STUFE_DACH => (&SLAB, 0),
        TRESEN => (&COUNTER, 0),
        TISCH => (&TABLE, 0),
        STUHL_N..=STUHL_W => (&CHAIR, id - STUHL_N),
        SITZBANK_N..=SITZBANK_W => (&BENCH, id - SITZBANK_N),
        WEBTEPPICH_ROT..=WEBTEPPICH_SMARAGD => (&RUG, 0),
        KRONLEUCHTER => (&CHANDELIER, 0),
        LATERNENMAST => (&LAMP_POST, 0),
        WANDLATERNE => (&WALL_LAMP, 0),
        ZAUN | GELAENDER => (&FENCE, 0),
        MARKISE_ROT_N..=MARKISE_BLAU_W => (&AWNING, (id - MARKISE_ROT_N) % 4),
        DEKOFASS => (&BARREL, 0),
        BUECHERSTAPEL => (&BOOKS, 0),
        GOLDSTAPEL => (&GOLD_STACK, 0),
        MOHN => (&POPPY, 0),
        SONNENBLUME => (&SUNFLOWER, 0),
        KORNBLUME => (&CORNFLOWER, 0),
        LEUCHTKLEE => (&CLOVER, 0),
        HOHES_GRAS => (&GRASS, 0),
        _ => return None,
    })
}

/// Block mit Blickrichtung `facing` (0 N, 1 O, 2 S, 3 W) aus dem Nord-Block einer Vierer-Reihe (TREPPE_*_N, STUHL_N, SITZBANK_N, MARKISE_*_N).
pub const fn facing(north: u8, facing: u8) -> u8 {
    north + facing % 4
}

/// Kasten um 90° im Uhrzeigersinn (von oben) drehen, `k` mal: Nord-Hälfte (z 0..8) → Ost-Hälfte (x 8..16).
pub fn rot(b: [u8; 6], k: u8) -> [u8; 6] {
    let mut b = b;
    for _ in 0..k % 4 {
        b = [16 - b[5], b[1], b[0], 16 - b[2], b[4], b[3]];
    }
    b
}

fn fence_family(b: u8) -> bool {
    matches!(b, ZAUN | GELAENDER)
}
fn pane_family(b: u8) -> bool {
    matches!(b, SCHEIBE | GITTERSTAB)
}

/// Anschluss-Maske aus den vier Nachbarn derselben Höhe in der Reihenfolge N, O, S, W (siehe DIRS).
pub fn conn_mask(id: u8, n: [u8; 4]) -> u8 {
    let Some((m, _)) = def(id) else { return 0 };
    let hit = |b: u8| match m.conn {
        Conn::None => false,
        Conn::Fence => fence_family(b) || opaque(b),
        Conn::Pane => pane_family(b) || opaque(b),
        Conn::Rug => b == id,
    };
    (0..4).filter(|&i| hit(n[i])).fold(0, |a, i| a | 1 << i)
}

/// Alle Teile des Blocks `id` mit Anschluss-Maske `conn`, fertig gedreht, `mat` aufgelöst (nie 0). Nichts für Würfel.
pub fn parts(id: u8, conn: u8, out: &mut impl FnMut(Part)) {
    let Some((m, r)) = def(id) else { return };
    let own = |q: &Part, k: u8| Part { b: rot(q.b, k), mat: if q.mat == 0 { id } else { q.mat }, cross: q.cross };
    match m.conn {
        Conn::None => m.parts.iter().for_each(|q| out(own(q, r))),
        Conn::Fence => {
            m.parts.iter().for_each(|q| out(own(q, 0)));
            for k in 0..4u8 {
                if conn >> k & 1 == 1 {
                    FENCE_ARM.iter().for_each(|q| out(own(q, k)));
                }
            }
        }
        Conn::Pane => {
            let along_x = conn & (O | W) != 0 || conn & (N | S) == 0;
            m.parts.iter().for_each(|q| out(own(q, if along_x { 0 } else { 1 })));
        }
        Conn::Rug => {
            let open = |bit: u8| conn & bit == 0;
            let (x0, x1, z0, z1) = (if open(W) { 2 } else { 0 }, if open(O) { 14 } else { 16 }, if open(N) { 2 } else { 0 }, if open(S) { 14 } else { 16 });
            out(Part { b: [x0, 0, z0, x1, 1, z1], mat: id, cross: false });
            let border = if id == WEBTEPPICH_GOLD { TEPPICH_ROT } else { GELB };
            for (bit, b) in [(N, [0, 0, 0, 16, 1, 2]), (O, [14, 0, 0, 16, 1, 16]), (S, [0, 0, 14, 16, 1, 16]), (W, [0, 0, 0, 2, 1, 16])] {
                if open(bit) {
                    out(Part { b, mat: border, cross: false });
                }
            }
        }
    }
}

/// Anschluss-Maske an einer Weltstelle.
pub fn conn_at(get: &impl Fn(i32, i32, i32) -> u8, id: u8, x: i32, y: i32, z: i32) -> u8 {
    conn_mask(id, DIRS.map(|(dx, dz)| get(x + dx, y, z + dz)))
}

/// Kollisions-Kästen des Blocks an (x, y, z) in Weltkoordinaten: (Kasten [x0, y0, z0, x1, y1, z1], ersteigbar).
/// Feste Würfel: ein Vollwürfel. Teilblöcke: ihre Kästen (ohne Kreuz-Teile), nicht feste Teilblöcke: nichts.
pub fn coll_boxes(get: &impl Fn(i32, i32, i32) -> u8, x: i32, y: i32, z: i32, out: &mut impl FnMut([f32; 6], bool)) {
    let id = get(x, y, z);
    if !solid(id) {
        return;
    }
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let Some((m, _)) = def(id) else {
        out([fx, fy, fz, fx + 1.0, fy + 1.0, fz + 1.0], false);
        return;
    };
    let conn = if m.conn == Conn::None { 0 } else { conn_at(get, id, x, y, z) };
    parts(id, conn, &mut |q: Part| {
        if !q.cross {
            let s = |i: usize| q.b[i] as f32 / 16.0;
            out([fx + s(0), fy + s(1), fz + s(2), fx + s(3), fy + s(4), fz + s(5)], m.step);
        }
    });
}

/// Alle Kollisions-Kästen, die den Körper (Füße p) schneiden (Berühren zählt nicht).
fn body_boxes(get: &impl Fn(i32, i32, i32) -> u8, p: [f32; 3], out: &mut impl FnMut([f32; 6], bool)) {
    let lo = [p[0] - HALF_W, p[1], p[2] - HALF_W];
    let hi = [p[0] + HALF_W, p[1] + BODY_H, p[2] + HALF_W];
    for y in lo[1].floor() as i32..=hi[1].floor() as i32 {
        for z in lo[2].floor() as i32..=hi[2].floor() as i32 {
            for x in lo[0].floor() as i32..=hi[0].floor() as i32 {
                coll_boxes(get, x, y, z, &mut |b, step| {
                    if b[0] < hi[0] && b[3] > lo[0] && b[1] < hi[1] && b[4] > lo[1] && b[2] < hi[2] && b[5] > lo[2] {
                        out(b, step);
                    }
                });
            }
        }
    }
}

/// Stößt ein Körper (Füße p, halbe Breite HALF_W, Höhe BODY_H) an? Für Würfel genau wie die alte Prüfung im Browser.
pub fn body_hits(get: impl Fn(i32, i32, i32) -> u8, p: [f32; 3]) -> bool {
    let mut hit = false;
    body_boxes(&get, p, &mut |_, _| hit = true);
    hit
}

/// Ersteigen im Gehen: Steht der Körper bei p in Stufen-Kästen (Treppe, Stufe, Tresen, Goldstapel), deren Oberkante höchstens
/// STEP_HEIGHT über den Füßen liegt, und ist es darüber frei? Some(h) = um h anheben (0 = gar nicht blockiert), None = Wand.
pub fn step_lift(get: impl Fn(i32, i32, i32) -> u8, p: [f32; 3]) -> Option<f32> {
    let (mut lift, mut wall) = (0.0f32, false);
    body_boxes(&get, p, &mut |b, step| {
        let h = b[4] - p[1];
        if !step || h > STEP_HEIGHT {
            wall = true;
        } else {
            lift = lift.max(h);
        }
    });
    if wall {
        return None;
    }
    if lift <= 0.0 {
        return Some(0.0);
    }
    let h = lift + 0.001;
    (!body_hits(&get, [p[0], p[1] + h, p[2]])).then_some(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabelle_und_drehung() {
        for b in BLOCKS {
            let f = FLAGS[b.id as usize];
            assert_eq!(def(b.id).is_some(), f & F_SHAPE != 0, "{}: F_SHAPE und Tabelle passen zusammen", b.name);
            if f & F_SHAPE != 0 {
                assert!(!opaque(b.id), "{} ist nie ein deckender Vollwürfel", b.name);
                let mut n = 0;
                for c in 0..16 {
                    parts(b.id, c, &mut |q: Part| {
                        n += 1;
                        assert!(q.mat != 0 && q.b[0] < q.b[3] && q.b[1] < q.b[4] && q.b[2] < q.b[5] && q.b[3] <= 16 && q.b[4] <= 16 && q.b[5] <= 16, "{}: {q:?}", b.name);
                    });
                }
                assert!(n > 0, "{} hat Teile", b.name);
            }
        }
        assert_eq!(rot([0, 8, 0, 16, 16, 8], 1), [8, 8, 0, 16, 16, 16], "Nord-Hälfte → Ost-Hälfte");
        assert_eq!(rot([0, 8, 0, 16, 16, 8], 2), [0, 8, 8, 16, 16, 16], "→ Süd-Hälfte");
        assert_eq!(rot([0, 8, 0, 16, 16, 8], 4), [0, 8, 0, 16, 16, 8]);
        assert_eq!(facing(TREPPE_STEIN_N, 3), TREPPE_STEIN_W);
        assert_eq!(facing(MARKISE_BLAU_N, 2), MARKISE_BLAU_S);
        // Scheibe zwischen zwei Wänden in x-Richtung liegt entlang x; Nachbarn nur in z → entlang z
        assert_eq!(conn_mask(SCHEIBE, [0, FELS, 0, FELS]), O | W);
        let mut zs = vec![];
        parts(SCHEIBE, N | S, &mut |q: Part| zs.push(q.b[2]));
        assert!(zs.contains(&0), "entlang z reicht bis z 0");
        assert_eq!(conn_mask(ZAUN, [GELAENDER, 0, FELS, MOHN]), N | S);
        assert_eq!(conn_mask(WEBTEPPICH_ROT, [WEBTEPPICH_ROT, WEBTEPPICH_BLAU, 0, 0]), N);
    }

    #[test]
    fn kollision() {
        // Boden y 9, darauf bei x 5: Blume (frei), x 6: Teppich (frei), x 7: Stufe, x 8: Treppe nach Osten, x 11: Tisch, x 13/14: Leuchter
        let get = |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 9, _) => FELS,
            (5, 10, 0) => MOHN,
            (6, 10, 0) => WEBTEPPICH_ROT,
            (7, 10, 0) => STUFE_STEIN,
            (8, 10, 0) => TREPPE_EICHE_O,
            (11, 10, 0) => TISCH,
            (13, 10, 0) | (14, 10, 0) => KRONLEUCHTER,
            _ => 0,
        };
        let at = |x: f32| [x, 10.0, 0.5];
        assert!(!body_hits(get, at(5.5)) && !body_hits(get, at(6.5)) && !body_hits(get, at(13.5)), "Blume, Teppich, Leuchter: durchlaufen");
        assert!(body_hits(get, at(7.5)) && body_hits(get, at(11.5)), "Stufe und Tisch stoßen");
        assert_eq!(step_lift(get, at(6.5)), Some(0.0));
        let h = step_lift(get, at(7.5)).expect("Stufe ersteigbar");
        assert!((h - 0.5).abs() < 0.01);
        assert_eq!(step_lift(get, at(11.5)), None, "Tisch ist keine Stufe");
        // Treppe nach Osten: auf der unteren Stufe (y 10,5) im Westteil, weiter nach Osten die obere Stufe
        assert!(!body_hits(get, [8.15, 10.5, 0.5]), "untere Stufe begehbar");
        let h = step_lift(get, [8.75, 10.5, 0.5]).expect("obere Stufe ersteigbar");
        assert!((h - 0.5).abs() < 0.01);
        assert_eq!(step_lift(get, [8.75, 10.0, 0.5]), None, "von unten direkt in die volle Höhe: Wand");
        assert!(!body_hits(get, [8.5, 11.0, 0.5]), "oben auf der Treppe frei");
        // Würfel wie bisher: Füße genau auf der Oberkante stoßen nicht
        assert!(!body_hits(get, [3.5, 10.0, 0.5]) && body_hits(get, [3.5, 9.99, 0.5]));
    }
}
