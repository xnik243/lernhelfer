//! Greedy Meshing: benachbarte gleiche Flächen eines Stücks werden zu einem Rechteck.
//! Abschnitt 8: Teilblock-Modelle (F_SHAPE, shape.rs) laufen am Greedy-Verfahren vorbei und kommen als eigene Kästen/Kreuze dazu.
//! Eckpunkt = 3×u32 (VERT), Orte in 1/16 Block:
//!   [0] x16 | y16 << 16   [1] z16 | normale << 16 | ao << 20   [2] Farbe/Muster/Flags (world::PACKED des Stoffs)
//! Normalen: 0..5 = Achse d*2 + Seite (0 -x, 1 +x, 2 -y, 3 +y, 4 -z, 5 +z); 6..9 schräg (Kreuz-Pflanzen):
//!   6 (+1,0,-1)/√2, 7 (-1,0,+1)/√2, 8 (+1,0,+1)/√2, 9 (-1,0,-1)/√2. ao 0..3 (3 = hell).
//! Jedes Rechteck = 4 Eckpunkte, gegen den Uhrzeigersinn von der Seite gesehen, in die die Normale zeigt.
use crate::world::{opaque, FLAGS, F_CLEAR, PACKED};

pub type Vert = [u32; 3];

/// Eckpunkt packen: Ort in 1/16 Block (Weltkoordinaten × 16), Normale 0..9, ao 0..3, Stoff-Block (Farbe aus PACKED).
#[inline]
pub fn vert(x16: u32, y16: u32, z16: u32, n: u32, ao: u32, mat: u8) -> Vert {
    [x16 | y16 << 16, z16 | n << 16 | ao << 20, PACKED[mat as usize]]
}

pub const PX: usize = 18;
pub const PY: usize = 66;
pub const PAD: usize = PX * PX * PY;

/// Index im gepolsterten Feld; x,z in -1..=16, y in -1..=64.
#[inline]
pub fn pidx(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize * PX + (z + 1) as usize) * PX + (x + 1) as usize
}

#[derive(Default)]
pub struct MeshOut {
    pub solid: Vec<Vert>,
    pub clear: Vec<Vert>,
    mask: Vec<u32>,
}

const DIMS: [i32; 3] = [16, 64, 16];

#[inline]
fn visible(a: u8, b: u8) -> bool {
    a != b && !opaque(b)
}

/// `p`: Stück samt einem Block Rand (Nachbarn), `ox/oz`: Weltposition der Ecke. Die Ausgabe-Puffer werden wiederverwendet.
pub fn mesh(p: &[u8], ox: i32, oz: i32, out: &mut MeshOut) {
    out.solid.clear();
    out.clear.clear();
    let at = |c: [i32; 3]| p[pidx(c[0], c[1], c[2])];
    for d in 0..3usize {
        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
        let (du, dv) = (DIMS[u], DIMS[v]);
        for side in 0..2i32 {
            let nd = side * 2 - 1;
            for s in 0..DIMS[d] {
                out.mask.clear();
                out.mask.resize((du * dv) as usize, 0);
                let mut any = false;
                for b in 0..dv {
                    for a in 0..du {
                        let mut c = [0i32; 3];
                        c[d] = s;
                        c[u] = a;
                        c[v] = b;
                        let blk = at(c);
                        if blk == 0 || crate::world::FLAGS[blk as usize] & (crate::world::F_INST | crate::world::F_SHAPE) != 0 {
                            continue;
                        }
                        let mut n = c;
                        n[d] += nd;
                        if !visible(blk, at(n)) {
                            continue;
                        }
                        let occ = |da: i32, db: i32| -> u32 {
                            let mut q = n;
                            q[u] += da;
                            q[v] += db;
                            opaque(at(q)) as u32
                        };
                        let ao = |s1: u32, s2: u32, cr: u32| if s1 + s2 == 2 { 0 } else { 3 - (s1 + s2 + cr) };
                        let (l, r, dn, up) = (occ(-1, 0), occ(1, 0), occ(0, -1), occ(0, 1));
                        let a00 = ao(l, dn, occ(-1, -1));
                        let a10 = ao(r, dn, occ(1, -1));
                        let a11 = ao(r, up, occ(1, 1));
                        let a01 = ao(l, up, occ(-1, 1));
                        out.mask[(b * du + a) as usize] = blk as u32 | a00 << 8 | a10 << 10 | a11 << 12 | a01 << 14 | 1 << 16;
                        any = true;
                    }
                }
                if !any {
                    continue;
                }
                for b in 0..dv {
                    let mut a = 0;
                    while a < du {
                        let m = out.mask[(b * du + a) as usize];
                        if m == 0 {
                            a += 1;
                            continue;
                        }
                        let mut w = 1;
                        while a + w < du && out.mask[(b * du + a + w) as usize] == m {
                            w += 1;
                        }
                        let mut h = 1;
                        'o: while b + h < dv {
                            for k in 0..w {
                                if out.mask[((b + h) * du + a + k) as usize] != m {
                                    break 'o;
                                }
                            }
                            h += 1;
                        }
                        for hh in 0..h {
                            for k in 0..w {
                                out.mask[((b + hh) * du + a + k) as usize] = 0;
                            }
                        }
                        let blk = (m & 0xFF) as u8;
                        let aos = [(m >> 8) & 3, (m >> 10) & 3, (m >> 12) & 3, (m >> 14) & 3];
                        let uv = [(a, b), (a + w, b), (a + w, b + h), (a, b + h)];
                        // +d: Reihenfolge gegen den Uhrzeigersinn von außen gesehen; -d: umgedreht.
                        let mut ord = if side == 1 { [0usize, 1, 2, 3] } else { [0, 3, 2, 1] };
                        if aos[ord[0]] + aos[ord[2]] < aos[ord[1]] + aos[ord[3]] {
                            ord.rotate_left(1);
                        }
                        let dst = if FLAGS[blk as usize] & F_CLEAR != 0 { &mut out.clear } else { &mut out.solid };
                        for k in ord {
                            let mut c = [0i32; 3];
                            c[d] = s + side;
                            c[u] = uv[k].0;
                            c[v] = uv[k].1;
                            dst.push(vert(((c[0] + ox) * 16) as u32, (c[1] * 16) as u32, ((c[2] + oz) * 16) as u32, (d as u32) * 2 + side as u32, aos[k], blk));
                        }
                        a += w;
                    }
                }
            }
        }
    }
    shapes(p, ox, oz, out);
}

/// Teilblock-Modelle (F_SHAPE) des Stücks als eigene Kästen und Kreuze. Agent „mesher“ (CONTRACT-8.md §mesher).
fn shapes(p: &[u8], ox: i32, oz: i32, out: &mut MeshOut) {
    use crate::shape::{self, Conn, Part, DIRS};
    for y in 0..64i32 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let id = p[pidx(x, y, z)];
                if FLAGS[id as usize] & crate::world::F_SHAPE == 0 {
                    continue;
                }
                let Some((m, _)) = shape::def(id) else { continue };
                let conn = if m.conn == Conn::None { 0 } else { shape::conn_mask(id, DIRS.map(|(dx, dz)| p[pidx(x + dx, y, z + dz)])) };
                let base = [(x + ox) * 16, y * 16, (z + oz) * 16];
                shape::parts(id, conn, &mut |q: Part| {
                    let dst = if FLAGS[q.mat as usize] & F_CLEAR != 0 { &mut out.clear } else { &mut out.solid };
                    let lo = [q.b[0] as i32, q.b[1] as i32, q.b[2] as i32];
                    let hi = [q.b[3] as i32, q.b[4] as i32, q.b[5] as i32];
                    let mut quad = |c: [[i32; 3]; 4], n: u32| {
                        for k in c {
                            dst.push(vert((base[0] + k[0]) as u32, (base[1] + k[1]) as u32, (base[2] + k[2]) as u32, n, 3, q.mat));
                        }
                    };
                    if q.cross {
                        let (y0, y1) = (lo[1], hi[1]);
                        let (a0, a1) = ([lo[0], lo[2]], [hi[0], hi[2]]); // Ebene entlang (1,0,1)
                        let (b0, b1) = ([lo[0], hi[2]], [hi[0], lo[2]]); // Ebene entlang (1,0,-1)
                        for (s, e, front, back) in [(a0, a1, 7u32, 6u32), (b0, b1, 8, 9)] {
                            // (e - s) × y zeigt zu `front`
                            let c = [[s[0], y0, s[1]], [e[0], y0, e[1]], [e[0], y1, e[1]], [s[0], y1, s[1]]];
                            quad(c, front);
                            quad([c[0], c[3], c[2], c[1]], back);
                        }
                        return;
                    }
                    let cell = [x, y, z];
                    for d in 0..3usize {
                        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
                        for side in 0..2usize {
                            let plane = if side == 1 { hi[d] } else { lo[d] };
                            if plane == side as i32 * 16 {
                                let mut nb = cell;
                                nb[d] += side as i32 * 2 - 1;
                                if opaque(p[pidx(nb[0], nb[1], nb[2])]) {
                                    continue;
                                }
                            }
                            let uv = [(lo[u], lo[v]), (hi[u], lo[v]), (hi[u], hi[v]), (lo[u], hi[v])];
                            let ord = if side == 1 { [0usize, 1, 2, 3] } else { [0, 3, 2, 1] };
                            let c = ord.map(|k| {
                                let mut c = [0i32; 3];
                                c[d] = plane;
                                c[u] = uv[k].0;
                                c[v] = uv[k].1;
                                c
                            });
                            quad(c, (d * 2 + side) as u32);
                        }
                    }
                });
            }
        }
    }
}

/// Eckpunkt entpacken: (x16, y16, z16, normale, ao) – Orte in 1/16 Block.
pub fn unpack(v: Vert) -> (i32, i32, i32, u32, u32) {
    ((v[0] & 0xFFFF) as i32, (v[0] >> 16) as i32, (v[1] & 0xFFFF) as i32, (v[1] >> 16) & 15, (v[1] >> 20) & 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{FELS, GLAS, WASSER};

    fn feld(f: impl Fn(i32, i32, i32) -> u8) -> Vec<u8> {
        let mut p = vec![0u8; PAD];
        for y in 0..64 {
            for z in 0..16 {
                for x in 0..16 {
                    p[pidx(x, y, z)] = f(x, y, z);
                }
            }
        }
        p
    }

    #[test]
    fn ein_block_sechs_flaechen_nach_aussen() {
        let p = feld(|x, y, z| if (x, y, z) == (5, 9, 7) { FELS } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 32, 48, &mut o);
        assert_eq!(o.solid.len(), 24);
        for q in o.solid.chunks(4) {
            let v: Vec<_> = q.iter().map(|v| unpack(*v)).collect();
            let f = |i: usize| [v[i].0 as f32 / 16.0, v[i].1 as f32 / 16.0, v[i].2 as f32 / 16.0];
            let (a, b, c) = (f(0), f(1), f(3));
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let n = v[0].3 as usize;
            let want = if n % 2 == 1 { 1.0 } else { -1.0 };
            assert_eq!(cr[n / 2], want, "Fläche {n} zeigt nach außen");
            assert!(v.iter().all(|c| c.4 == 3), "frei stehend: kein Schatten");
            assert!(v.iter().all(|c| [37 * 16, 38 * 16].contains(&c.0) && [55 * 16, 56 * 16].contains(&c.2)));
        }
    }

    #[test]
    fn flaechen_verschmelzen() {
        // massiver Quader 16×4×16 = 1024 Blöcke, aber nur 6 Rechtecke
        let p = feld(|_, y, _| if y < 4 { FELS } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        assert_eq!(o.solid.len() / 4, 6);
        // Wasser neben Glas: getrennte Liste, keine Innenflächen im Wasser
        let p = feld(|x, y, _| if y == 0 { if x < 8 { WASSER } else { GLAS } } else { 0 });
        mesh(&p, 0, 0, &mut o);
        assert!(o.solid.is_empty());
        assert_eq!(o.clear.len() / 4, 2 * 6);
    }

    fn quads(v: &[Vert]) -> Vec<[(i32, i32, i32, u32, u32); 4]> {
        v.chunks(4).map(|q| [unpack(q[0]), unpack(q[1]), unpack(q[2]), unpack(q[3])]).collect()
    }

    #[test]
    fn teilbloecke_reissen_keine_loecher() {
        use crate::world::{MOHN, SCHEIBE, STUFE_STEIN, TISCH, TREPPE_EICHE_N, WEBTEPPICH_ROT};
        let teile = [(1, TISCH), (3, WEBTEPPICH_ROT), (5, TREPPE_EICHE_N), (7, SCHEIBE), (9, MOHN), (11, STUFE_STEIN)];
        let p = feld(|x, y, z| match (x, y, z) {
            (_, 0, _) => FELS,
            (7, 1, 2) => FELS, // Wand südlich der Scheibe
            (x, 1, 1) => teile.iter().find(|t| t.0 == x).map_or(0, |t| t.1),
            _ => 0,
        });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        let all = quads(&o.solid);
        let rng = |q: &[(i32, i32, i32, u32, u32); 4], f: fn(&(i32, i32, i32, u32, u32)) -> i32| {
            (q.iter().map(f).min().unwrap_or(0), q.iter().map(f).max().unwrap_or(0))
        };
        for (cx, id) in teile {
            let (c0, c1) = (cx * 16, cx * 16 + 16);
            let area: i32 = all
                .iter()
                .filter(|q| q.iter().all(|v| v.3 == 3 && v.1 == 16))
                .map(|q| {
                    let (x0, x1) = rng(q, |v| v.0);
                    let (z0, z1) = rng(q, |v| v.2);
                    (x1.min(c1) - x0.max(c0)).max(0) * (z1.min(32) - z0.max(16)).max(0)
                })
                .sum();
            assert_eq!(area, 256, "Boden unter Block {id} vollständig sichtbar");
        }
        let wand = all.iter().any(|q| {
            q.iter().all(|v| v.3 == 4 && v.2 == 32) && rng(q, |v| v.0).0 <= 112 && rng(q, |v| v.0).1 >= 128 && rng(q, |v| v.1).0 <= 16 && rng(q, |v| v.1).1 >= 32
        });
        assert!(wand, "Wand neben der Scheibe behält ihre Seitenfläche");
    }

    #[test]
    fn scheibe_glas_durchsichtig_rahmen_fest() {
        use crate::world::{DUNKELBRETT, SCHEIBE};
        let p = feld(|x, y, z| if (x, y, z) == (5, 5, 5) { SCHEIBE } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        assert_eq!(o.clear.len() / 4, 12, "2 Glasfelder × 6 Flächen");
        assert!(o.clear.iter().all(|v| v[2] == PACKED[GLAS as usize]));
        assert_eq!(o.solid.len() / 4, 18, "3 Rahmenteile × 6 Flächen");
        assert!(o.solid.iter().all(|v| v[2] == PACKED[DUNKELBRETT as usize]));
    }

    #[test]
    fn mohn_kreuz_gegen_den_uhrzeigersinn() {
        use crate::world::MOHN;
        let p = feld(|x, y, z| if (x, y, z) == (5, 5, 5) { MOHN } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        let mut all = quads(&o.solid);
        all.extend(quads(&o.clear));
        let kreuz: Vec<_> = all.iter().filter(|q| q[0].3 >= 6).collect();
        assert_eq!(kreuz.len(), 4);
        let mut ns: Vec<u32> = kreuz.iter().map(|q| q[0].3).collect();
        ns.sort();
        assert_eq!(ns, vec![6, 7, 8, 9]);
        let dir = [[1.0, 0.0, -1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, 1.0], [-1.0, 0.0, -1.0]];
        for q in kreuz {
            assert!(q.iter().all(|v| v.3 == q[0].3));
            let f = |i: usize| [q[i].0 as f32, q[i].1 as f32, q[i].2 as f32];
            let (a, b, c) = (f(0), f(1), f(3));
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let n = dir[q[0].3 as usize - 6];
            assert!(cr[0] * n[0] + cr[1] * n[1] + cr[2] * n[2] > 0.0, "Kreuzfläche {} zeigt zur Normale", q[0].3);
        }
        let bluete = all.iter().filter(|q| q[0].3 < 6).count();
        assert_eq!(bluete, 6, "Blütenkasten");
        assert_eq!(all.len(), 10);
    }

    #[test]
    fn tisch_ohne_bein_unterseiten() {
        use crate::world::TISCH;
        let p = feld(|x, y, z| if y == 0 { FELS } else if (x, y, z) == (5, 1, 5) { TISCH } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        let tisch: Vec<_> = quads(&o.solid).into_iter().zip(o.solid.chunks(4)).filter(|(_, r)| r[0][2] == PACKED[TISCH as usize]).map(|(q, _)| q).collect();
        assert_eq!(tisch.len(), 26, "5 Kästen × 6 − 4 Bein-Unterseiten");
        assert!(!tisch.iter().any(|q| q.iter().all(|v| v.3 == 2 && v.1 == 16)), "keine Fläche auf dem Boden");
    }

    #[test]
    fn schatten_in_der_ecke() {
        let p = feld(|x, y, _| if y == 0 || (x == 0 && y == 1) { FELS } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        assert!(o.solid.iter().any(|v| unpack(*v).4 < 3));
    }
}
