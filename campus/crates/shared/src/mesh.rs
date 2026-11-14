//! Greedy Meshing: benachbarte gleiche Flächen eines Stücks werden zu einem Rechteck.
//! Eckpunkt = 2×u32: [x | y<<10 | z<<17 | normale<<27 | ao<<30, farbe/muster (world::PACKED)].
use crate::world::{opaque, FLAGS, F_CLEAR, PACKED};

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
    pub solid: Vec<[u32; 2]>,
    pub clear: Vec<[u32; 2]>,
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
                        if blk == 0 || crate::world::FLAGS[blk as usize] & crate::world::F_INST != 0 {
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
                            let pos = (c[0] + ox) as u32 | (c[1] as u32) << 10 | ((c[2] + oz) as u32) << 17;
                            dst.push([pos | ((d as u32) * 2 + side as u32) << 27 | aos[k] << 30, PACKED[blk as usize]]);
                        }
                        a += w;
                    }
                }
            }
        }
    }
}

/// Eckpunkt entpacken: (x, y, z, normale, ao).
pub fn unpack(v: u32) -> (i32, i32, i32, u32, u32) {
    ((v & 1023) as i32, ((v >> 10) & 127) as i32, ((v >> 17) & 1023) as i32, (v >> 27) & 7, v >> 30)
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
            let v: Vec<_> = q.iter().map(|v| unpack(v[0])).collect();
            let f = |i: usize| [v[i].0 as f32, v[i].1 as f32, v[i].2 as f32];
            let (a, b, c) = (f(0), f(1), f(3));
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let n = v[0].3 as usize;
            let want = if n % 2 == 1 { 1.0 } else { -1.0 };
            assert_eq!(cr[n / 2], want, "Fläche {n} zeigt nach außen");
            assert!(v.iter().all(|c| c.4 == 3), "frei stehend: kein Schatten");
            assert!(v.iter().all(|c| (37..=38).contains(&c.0) && (55..=56).contains(&c.2)));
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

    #[test]
    fn schatten_in_der_ecke() {
        let p = feld(|x, y, _| if y == 0 || (x == 0 && y == 1) { FELS } else { 0 });
        let mut o = MeshOut::default();
        mesh(&p, 0, 0, &mut o);
        assert!(o.solid.iter().any(|v| unpack(v[0]).4 < 3));
    }
}
