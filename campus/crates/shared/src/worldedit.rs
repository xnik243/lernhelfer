//! Phase 22: Voxel-Mathematik des Architekten-Modus (`/architect`, `//set` …). Reine Funktionen ohne Welt-Zugriff:
//! der Server (architect.rs) prüft Rechte, liest alte Blöcke und schreibt; hier entstehen nur Zell-Listen.
use crate::world::{self, CH, WORLD};

/// Höchstzahl Blöcke je Befehl.
pub const MAX_BLOCKS: usize = 32_768;
/// Rückgängig-Schritte je Verwalter.
pub const UNDO_MAX: usize = 10;
pub type Cell = [i32; 3];

/// Quader zwischen zwei Ecken (beide einschließlich).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub min: Cell,
    pub max: Cell,
}
impl Region {
    pub fn new(a: Cell, b: Cell) -> Region {
        Region { min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])] }
    }
    /// Kantenlängen X × Y × Z.
    pub fn dims(&self) -> [i32; 3] {
        [self.max[0] - self.min[0] + 1, self.max[1] - self.min[1] + 1, self.max[2] - self.min[2] + 1]
    }
    pub fn volume(&self) -> usize {
        let d = self.dims();
        d[0] as usize * d[1] as usize * d[2] as usize
    }
    pub fn contains(&self, c: Cell) -> bool {
        (0..3).all(|k| c[k] >= self.min[k] && c[k] <= self.max[k])
    }
    /// Alle Zellen, Reihenfolge y → z → x (wie `Clip::b`).
    pub fn cells(&self) -> impl Iterator<Item = Cell> {
        let (a, b) = (self.min, self.max);
        (a[1]..=b[1]).flat_map(move |y| (a[2]..=b[2]).flat_map(move |z| (a[0]..=b[0]).map(move |x| [x, y, z])))
    }
}

/// Liegt die Zelle im bebaubaren Teil der Welt (x, z in 0..WORLD, y in 1..CH-1)?
pub fn in_world(c: Cell) -> bool {
    (0..WORLD).contains(&c[0]) && (0..WORLD).contains(&c[2]) && (1..CH - 1).contains(&c[1])
}
/// Blöcke mit eigenen Tabellenzeilen (Truhen, Ständer, Schilder, Saat, Teleporter, Sprungplatte) oder festem Zweck:
/// der Architekten-Modus fasst sie weder als alten noch als neuen Block an.
pub fn protected(b: u8) -> bool {
    matches!(b, world::TRUHE | world::GROSSE_TRUHE | world::STAENDER | world::SCHILD | world::SPRUNGPLATTE | world::ABGEBAUT) || world::SAATEN.contains(&b) || world::TELEPORTER.contains(&b)
}

/// Die vier senkrechten Seiten des Quaders (alle Höhen), ohne Deckel und Boden.
pub fn walls(r: Region) -> Vec<Cell> {
    r.cells().filter(|c| c[0] == r.min[0] || c[0] == r.max[0] || c[2] == r.min[2] || c[2] == r.max[2]).collect()
}
/// Volle Kugel um `c` (Radius 1..=20, sonst leer).
pub fn sphere(c: Cell, r: i32) -> Vec<Cell> {
    if !(1..=20).contains(&r) {
        return Vec::new();
    }
    let lim = (r as f32 + 0.5).powi(2);
    Region::new([c[0] - r, c[1] - r, c[2] - r], [c[0] + r, c[1] + r, c[2] + r])
        .cells()
        .filter(|q| ((q[0] - c[0]).pow(2) + (q[1] - c[1]).pow(2) + (q[2] - c[2]).pow(2)) as f32 <= lim)
        .collect()
}
/// Voller stehender Zylinder: Grundfläche um `c`, `h` Schichten nach oben (Radius 1..=30, Höhe 1..=62, sonst leer).
pub fn cyl(c: Cell, r: i32, h: i32) -> Vec<Cell> {
    if !(1..=30).contains(&r) || !(1..=62).contains(&h) {
        return Vec::new();
    }
    let lim = (r as f32 + 0.5).powi(2);
    Region::new([c[0] - r, c[1], c[2] - r], [c[0] + r, c[1] + h - 1, c[2] + r]).cells().filter(|q| ((q[0] - c[0]).pow(2) + (q[2] - c[2]).pow(2)) as f32 <= lim).collect()
}

/// Zwischenablage (`//copy`): Blöcke eines Quaders und seine Lage relativ zum Standpunkt beim Kopieren.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    /// Kantenlängen X, Y, Z
    pub size: [i32; 3],
    /// kleinste Ecke minus Standpunkt (Block, in dem die Füße stehen)
    pub off: [i32; 3],
    /// Blöcke, Index `(y * size[2] + z) * size[0] + x`
    pub b: Vec<u8>,
}
impl Clip {
    pub fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((y * self.size[2] + z) * self.size[0] + x) as usize
    }
    /// Liest den Quader über `get` (Weltkoordinaten) aus.
    pub fn copy(r: Region, stand: Cell, get: impl Fn(i32, i32, i32) -> u8) -> Clip {
        Clip { size: r.dims(), off: [r.min[0] - stand[0], r.min[1] - stand[1], r.min[2] - stand[2]], b: r.cells().map(|c| get(c[0], c[1], c[2])).collect() }
    }
    /// 90° um die senkrechte Achse (von oben gesehen im Uhrzeigersinn: +x wird +z). Vier Drehungen ergeben wieder das Original.
    pub fn rotated(&self) -> Clip {
        let [sx, sy, sz] = self.size;
        let mut out = Clip { size: [sz, sy, sx], off: [-(self.off[2] + sz - 1), self.off[1], self.off[0]], b: vec![0; self.b.len()] };
        for y in 0..sy {
            for z in 0..sz {
                for x in 0..sx {
                    let k = out.idx(sz - 1 - z, y, x);
                    out.b[k] = self.b[self.idx(x, y, z)];
                }
            }
        }
        out
    }
    /// Zellen beim Einfügen am Standpunkt `at` (Luft wird mitkopiert: der Ausschnitt ersetzt das Ziel vollständig).
    pub fn cells(&self, at: Cell) -> impl Iterator<Item = (Cell, u8)> + '_ {
        let o = [at[0] + self.off[0], at[1] + self.off[1], at[2] + self.off[2]];
        let s = self.size;
        Region { min: o, max: [o[0] + s[0] - 1, o[1] + s[1] - 1, o[2] + s[2] - 1] }.cells().zip(self.b.iter().copied())
    }
}

/// „Als Rust-Blueprint kopieren“: Funktion im Stil von blueprints.rs (dort einfügen und aus `hub()` aufrufen).
/// Läufe gleicher Blöcke entlang x werden zu `p.fill`, Luft wird ausgelassen. `name` wird zu einem gültigen Bezeichner.
pub fn rust_blueprint(name: &str, clip: &Clip) -> String {
    let mut ident: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    let [sx, sy, sz] = clip.size;
    let key = |b: u8| world::block(b).map_or_else(|| b.to_string(), |d| d.key.to_string());
    let mut s = format!("/// Export aus dem Architekten-Modus: {sx} × {sy} × {sz}. Ecke (x, y, z) = kleinste Koordinaten.\nfn {ident}(p: &mut Put, x: i32, y: i32, z: i32) {{\n    if !p.hits(x, z, x + {}, z + {}) {{\n        return;\n    }}\n", sx - 1, sz - 1);
    for y in 0..sy {
        for z in 0..sz {
            let mut x = 0;
            while x < sx {
                let b = clip.b[clip.idx(x, y, z)];
                let mut e = x;
                while e + 1 < sx && clip.b[clip.idx(e + 1, y, z)] == b {
                    e += 1;
                }
                if b != 0 {
                    if e > x {
                        s += &format!("    p.fill(x + {x}, y + {y}, z + {z}, x + {e}, y + {y}, z + {z}, {});\n", key(b));
                    } else {
                        s += &format!("    p.set(x + {x}, y + {y}, z + {z}, {});\n", key(b));
                    }
                }
                x = e + 1;
            }
        }
    }
    s += "}\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quader_waende_kugel_zylinder() {
        let r = Region::new([5, 9, 3], [2, 4, 3]);
        assert_eq!((r.min, r.max, r.dims(), r.volume()), ([2, 4, 3], [5, 9, 3], [4, 6, 1], 24));
        assert_eq!(r.cells().count(), 24);
        assert!(r.contains([3, 5, 3]) && !r.contains([3, 5, 4]));
        let w = walls(Region::new([0, 1, 0], [4, 3, 4]));
        assert_eq!(w.len(), 16 * 3, "Rand von 5×5 = 16 Spalten, 3 hoch");
        assert!(!w.contains(&[2, 2, 2]) && w.contains(&[0, 2, 2]));
        let k = sphere([10, 10, 10], 3);
        assert!(k.contains(&[13, 10, 10]) && !k.contains(&[13, 13, 13]) && k.len() > 100 && k.len() < 7 * 7 * 7);
        assert!(sphere([0, 0, 0], 0).is_empty() && sphere([0, 0, 0], 21).is_empty());
        let z = cyl([10, 5, 10], 2, 4);
        assert!(z.iter().all(|c| (5..9).contains(&c[1])) && z.contains(&[12, 8, 10]) && !z.contains(&[12, 5, 12]));
        assert_eq!(z.len() % 4, 0);
        assert!(in_world([0, 1, 0]) && !in_world([0, 0, 0]) && !in_world([WORLD, 5, 0]) && !in_world([5, CH - 1, 5]));
        assert!(protected(world::TRUHE) && protected(world::SAAT_WEIZEN) && !protected(world::FELS) && !protected(0));
        assert!(sphere([0, 0, 0], 20).len() > MAX_BLOCKS, "größte Kugel muss der Server ablehnen");
    }

    #[test]
    fn zwischenablage_dreht_und_exportiert() {
        // 3×1×2-Ausschnitt mit lauter verschiedenen Blöcken, Standpunkt eine Zelle westlich davon
        let get = |x: i32, _y: i32, z: i32| (1 + x + z * 3) as u8;
        let c = Clip::copy(Region::new([0, 5, 0], [2, 5, 1]), [-1, 5, 0], get);
        assert_eq!((c.size, c.off, c.b.clone()), ([3, 1, 2], [1, 0, 0], vec![1, 2, 3, 4, 5, 6]));
        let r = c.rotated();
        assert_eq!(r.size, [2, 1, 3]);
        // +x wird +z: der Block bei (2, 0) liegt nach der Drehung bei z = 2
        let at = |cl: &Clip, x: i32, z: i32| cl.b[cl.idx(x, 0, z)];
        assert_eq!((at(&r, 1, 0), at(&r, 1, 2), at(&r, 0, 0)), (1, 3, 4));
        assert_eq!(c.rotated().rotated().rotated().rotated(), c, "vier Drehungen = Original");
        let cells: Vec<(Cell, u8)> = c.cells([10, 20, 30]).collect();
        assert_eq!(cells[0], ([11, 20, 30], 1));
        assert_eq!(cells[5], ([13, 20, 31], 6));
        let mut flat = Clip { size: [4, 1, 1], off: [0; 3], b: vec![world::FELS, world::FELS, 0, world::GLAS] };
        let code = rust_blueprint("Mein Turm!", &flat);
        assert!(code.contains("fn mein_turm_(p: &mut Put, x: i32, y: i32, z: i32)"), "{code}");
        assert!(code.contains("p.fill(x + 0, y + 0, z + 0, x + 1, y + 0, z + 0, FELS);") && code.contains("p.set(x + 3, y + 0, z + 0, GLAS);"), "{code}");
        flat.b = vec![0; 4];
        assert!(!rust_blueprint("9", &flat).contains("p.set") && rust_blueprint("9", &flat).contains("fn _9("));
    }
}
