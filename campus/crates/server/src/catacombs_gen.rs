//! Phase 24 (Besitz: Agent S-KATA): reine, prüfbare Rechnung der Prüfungs-Katakomben ohne Game – 3×3-Generator,
//! Spiegel-/Runen-/Weisen-Rätsel, Wertung und Beute. Vertrag: CONTRACT-21-25.md §S-KATA.
//! Lokale Zellen: x, z 0..48 (Raum i liegt bei ((i % 3)·16, (i / 3)·16)), y 0..24 (0 = Boden auf DUNGEON_Y).
//! Innenraum eines Raums = 14×14 Zellen (ix, iz 0..14) bei Raum-Ecke + 1.
use crate::dungeon_rooms_data::{RoomTpl, ROOMS, T_BOSS, T_KAMPF, T_SCHATZ, T_START};
use campus_shared::economy::Rng;
use campus_shared::{rpg, skyblock as sb, world};

pub const K_START: u8 = 0;
pub const K_KAMPF: u8 = 1;
pub const K_SPIEGEL: u8 = 2;
pub const K_RUNEN: u8 = 3;
pub const K_WEISEN: u8 = 4;
pub const K_SCHATZ: u8 = 5;
pub const K_BOSS: u8 = 6;
pub const KIND_NAMES: [&str; 7] = ["Start-Halle", "Kampf-Kammer", "Spiegel-Kammer", "Runen-Kammer", "Halle der drei Weisen", "Versteckte Schatz-Kammer", "Endboss-Saal"];
/// Kantenlänge eines Raums (mit Wänden), Decke (lokales y), Türhöhe.
pub const ROOM: i32 = 16;
pub const CEIL: i32 = 6;
pub const DOOR_H: i32 = 3;
/// Höhe des Spiegel-Strahls (Prismen, Strahler, Empfänger stehen auf einem Sockel darunter).
pub const BEAM_Y: i32 = 2;
/// Runen-Feld 3×3: Innenraum-Spalten/-Zeilen; Rune k liegt bei (RUNE_POS[k % 3], RUNE_POS[k / 3]).
pub const RUNE_POS: [i32; 3] = [3, 6, 9];
pub const RUNE_STEPS: usize = 5;
/// Halle der Weisen: Statue nr bei (SAGE_X[nr], STATUE_Z), ihre Truhe bei (SAGE_X[nr], CHEST_Z).
pub const SAGE_X: [i32; 3] = [3, 6, 9];
pub const STATUE_Z: i32 = 3;
pub const CHEST_Z: i32 = 6;
pub const GRADES: [&str; 5] = ["C", "B", "A", "S", "S+"];
/// Türfelder einer Vorlage (ix, iz): müssen frei bleiben.
pub const DOOR_FIELDS: [(i32, i32); 8] = [(6, 0), (7, 0), (6, 13), (7, 13), (0, 6), (0, 7), (13, 6), (13, 7)];

pub struct Room {
    pub kind: u8,
    /// Index in dungeon_rooms_data::ROOMS (Start, Kampf, Schatz, Boss); Rätsel-Räume werden erzeugt.
    pub tpl: Option<usize>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Mirror {
    /// Innenraum-Zellen der Spiegel-Kammer
    pub emitter: (i32, i32),
    pub dir: (i32, i32),
    pub receiver: (i32, i32),
    /// (Zelle, true = PRISMA_A „/“, false = PRISMA_B „\“) – aktueller Stand
    pub prisms: Vec<((i32, i32), bool)>,
    /// eine Lösung (gleiche Reihenfolge wie `prisms`)
    pub solution: Vec<bool>,
}

pub struct Layout {
    pub seed: u64,
    pub stufe: u8,
    /// 9 Räume, Index rz·3 + rx
    pub rooms: Vec<Room>,
    /// Türen zwischen Nachbarn (a < b)
    pub doors: Vec<(usize, usize)>,
    pub mirror: Mirror,
    /// Runen-Folge (Index 0..9 im 3×3-Feld), aufeinanderfolgende verschieden
    pub runes: [u8; RUNE_STEPS],
    /// Index in SAGES und die richtige Truhe
    pub sages: usize,
    pub sage_answer: u8,
}

// ---------- Geometrie ----------
pub fn room_origin(i: usize) -> (i32, i32) {
    ((i % 3) as i32 * ROOM, (i / 3) as i32 * ROOM)
}
/// Lokale Zelle (x, z) der Innenraum-Zelle (ix, iz) von Raum i.
pub fn interior(i: usize, ix: i32, iz: i32) -> (i32, i32) {
    let (x, z) = room_origin(i);
    (x + 1 + ix, z + 1 + iz)
}
/// Raum, in dessen Innenraum die lokale Zelle liegt (Wände und Türöffnungen: None).
pub fn room_at(lx: i32, lz: i32) -> Option<usize> {
    if !(0..3 * ROOM).contains(&lx) || !(0..3 * ROOM).contains(&lz) {
        return None;
    }
    let (dx, dz) = (lx % ROOM, lz % ROOM);
    if !(1..ROOM - 1).contains(&dx) || !(1..ROOM - 1).contains(&dz) {
        return None;
    }
    Some((lz / ROOM * 3 + lx / ROOM) as usize)
}
pub fn adjacent(a: usize, b: usize) -> bool {
    let (ax, az, bx, bz) = (a % 3, a / 3, b % 3, b / 3);
    a < 9 && b < 9 && ((az == bz && ax.abs_diff(bx) == 1) || (ax == bx && az.abs_diff(bz) == 1))
}
/// Die 4 Zellen (beide Wandschichten × 2 breit) der Tür zwischen den Nachbarn a und b (lokal x, z).
pub fn door_cells(a: usize, b: usize) -> [(i32, i32); 4] {
    let (a, b) = (a.min(b), a.max(b));
    let (x0, z0) = room_origin(a);
    if b == a + 1 {
        [(x0 + 15, z0 + 7), (x0 + 15, z0 + 8), (x0 + 16, z0 + 7), (x0 + 16, z0 + 8)]
    } else {
        [(x0 + 7, z0 + 15), (x0 + 8, z0 + 15), (x0 + 7, z0 + 16), (x0 + 8, z0 + 16)]
    }
}
/// Anfangs-Füllung einer Türöffnung: Blut-Portal vor dem Endboss-Saal, Gitter vor der Schatz-Kammer, sonst offen.
pub fn door_fill(lay: &Layout, a: usize, b: usize) -> u8 {
    let k = |i: usize| lay.rooms.get(i).map_or(K_START, |r| r.kind);
    if k(a) == K_BOSS || k(b) == K_BOSS {
        world::BLUT_PORTAL
    } else if k(a) == K_SCHATZ || k(b) == K_SCHATZ {
        world::KATA_TOR
    } else {
        0
    }
}
/// Erster Raum dieser Art (jede Art kommt vor).
pub fn find(lay: &Layout, kind: u8) -> usize {
    lay.rooms.iter().position(|r| r.kind == kind).unwrap_or(0)
}
/// Marken `ch` (b's', b'm', b't', b'B') eines Raums als Innenraum-Zellen; ohne Vorlage feste Ersatz-Stellen.
pub fn markers(lay: &Layout, i: usize, ch: u8) -> Vec<(i32, i32)> {
    let Some(room) = lay.rooms.get(i) else { return Vec::new() };
    if let Some(t) = room.tpl.and_then(|k| ROOMS.get(k)) {
        let v: Vec<(i32, i32)> = t.rows.iter().enumerate().flat_map(|(iz, r)| r.bytes().enumerate().filter(move |c| c.1 == ch).map(move |(ix, _)| (ix as i32, iz as i32))).collect();
        if !v.is_empty() {
            return v;
        }
    }
    match (room.kind, ch) {
        (K_START, b's') | (K_SCHATZ, b't') | (K_BOSS, b'B') => vec![(6, 6)],
        (K_KAMPF, b'm') => vec![(3, 3), (10, 3), (3, 10), (10, 10)],
        (K_BOSS, b'm') => vec![(3, 3), (10, 10)],
        _ => Vec::new(),
    }
}
pub fn rune_cell(k: usize) -> (i32, i32) {
    (RUNE_POS[k % 3], RUNE_POS[(k / 3).min(2)])
}
pub fn rune_at(ix: i32, iz: i32) -> Option<usize> {
    let a = RUNE_POS.iter().position(|&v| v == ix)?;
    let b = RUNE_POS.iter().position(|&v| v == iz)?;
    Some(b * 3 + a)
}

// ---------- Vorlagen ----------
/// Prüft eine Raum-Vorlage gegen die Regeln aus §H-RAEUME.
pub fn tpl_check(t: &RoomTpl) -> Result<(), String> {
    let g: Vec<&[u8]> = t.rows.iter().map(|r| r.as_bytes()).collect();
    for (z, r) in g.iter().enumerate() {
        if r.len() != 14 {
            return Err(format!("{}: Zeile {z} hat {} Zeichen", t.name, r.len()));
        }
        if let Some(&c) = r.iter().find(|&&c| !b".#oLmstB".contains(&c)) {
            return Err(format!("{}: Zeichen {:?} in Zeile {z}", t.name, c as char));
        }
    }
    let at = |x: i32, z: i32| g[z as usize][x as usize];
    if let Some(d) = DOOR_FIELDS.iter().find(|d| at(d.0, d.1) != b'.') {
        return Err(format!("{}: Türfeld {:?} ist nicht frei", t.name, d));
    }
    let walk = |c: u8| !matches!(c, b'#' | b'o' | b'L');
    // alle begehbaren Felder hängen zusammen (Start an einer Tür)
    let mut seen = [[false; 14]; 14];
    let mut stack = vec![(6i32, 0i32)];
    seen[0][6] = true;
    while let Some((x, z)) = stack.pop() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x + dx, z + dz);
            if (0..14).contains(&nx) && (0..14).contains(&nz) && !seen[nz as usize][nx as usize] && walk(at(nx, nz)) {
                seen[nz as usize][nx as usize] = true;
                stack.push((nx, nz));
            }
        }
    }
    for z in 0..14 {
        for x in 0..14 {
            if walk(at(x, z)) && !seen[z as usize][x as usize] {
                return Err(format!("{}: Feld ({x}, {z}) ist nicht erreichbar", t.name));
            }
        }
    }
    let cells = |ch: u8| -> Vec<(i32, i32)> { (0..14).flat_map(|z| (0..14).map(move |x| (x, z))).filter(|&(x, z)| at(x, z) == ch).collect() };
    let (s, m, tt, b) = (cells(b's'), cells(b'm'), cells(b't'), cells(b'B'));
    let need = |ok: bool, why: &str| if ok { Ok(()) } else { Err(format!("{}: {why}", t.name)) };
    need(t.kind == T_START || s.is_empty(), "„s“ nur in der Start-Halle")?;
    need(t.kind == T_SCHATZ || tt.is_empty(), "„t“ nur in der Schatz-Kammer")?;
    need(t.kind == T_BOSS || b.is_empty(), "„B“ nur im Endboss-Saal")?;
    match t.kind {
        T_START => need(s.len() == 1 && m.is_empty(), "Start-Halle braucht genau ein „s“ und kein „m“"),
        T_KAMPF => {
            need((4..=6).contains(&m.len()), "Kampf-Kammer braucht 4–6 „m“")?;
            let far = m.iter().all(|&(x, z)| DOOR_FIELDS.iter().all(|d| (x - d.0).abs() + (z - d.1).abs() >= 3));
            need(far, "„m“ zu nah an einer Tür")
        }
        T_SCHATZ => need(tt.len() == 1, "Schatz-Kammer braucht genau ein „t“"),
        T_BOSS => {
            need(b.len() == 1 && b.iter().all(|&(x, z)| (4..10).contains(&x) && (4..10).contains(&z)), "genau ein „B“ in der Mitte")?;
            need((2..=4).contains(&m.len()), "Endboss-Saal braucht 2–4 „m“")?;
            need(cells(b'#').len() + cells(b'o').len() <= 8, "höchstens 8 „#“/„o“")
        }
        k => Err(format!("{}: unbekannte Art {k}", t.name)),
    }
}

// ---------- Zufall ----------
fn below(rng: &mut Rng, n: usize) -> usize {
    rng.below(n.max(1) as u64) as usize
}
fn range(rng: &mut Rng, a: i32, b: i32) -> i32 {
    a + below(rng, (b - a + 1) as usize) as i32
}
fn shuffle<T>(v: &mut [T], rng: &mut Rng) {
    for i in (1..v.len()).rev() {
        let j = below(rng, i + 1);
        v.swap(i, j);
    }
}

// ---------- Generator ----------
/// Neuer Lauf: 3×3 Räume (genau die neun Arten), Türen, Rätsel. Gleicher Seed = gleiche Katakombe.
pub fn generate(seed: u64, stufe: u8) -> Layout {
    let mut rng = Rng(seed);
    let base = [K_START, K_KAMPF, K_KAMPF, K_KAMPF, K_SPIEGEL, K_RUNEN, K_WEISEN, K_SCHATZ, K_BOSS];
    let mut plan = None;
    for _ in 0..500 {
        let mut k = base;
        shuffle(&mut k, &mut rng);
        if let Some(d) = plan_doors(&k, &mut rng) {
            plan = Some((k, d));
            break;
        }
    }
    // Ersatz (praktisch nie nötig): fester, gültiger Plan
    let (kinds, doors) = plan.unwrap_or((
        [K_SCHATZ, K_RUNEN, K_KAMPF, K_KAMPF, K_START, K_SPIEGEL, K_BOSS, K_KAMPF, K_WEISEN],
        vec![(0, 1), (1, 2), (1, 4), (2, 5), (3, 4), (4, 5), (4, 7), (5, 8), (3, 6)],
    ));
    let mut tpls: Vec<Vec<usize>> = [T_START, T_KAMPF, T_SCHATZ, T_BOSS]
        .iter()
        .map(|&tk| {
            let mut v: Vec<usize> = ROOMS.iter().enumerate().filter(|(_, t)| t.kind == tk && tpl_check(t).is_ok()).map(|(i, _)| i).collect();
            shuffle(&mut v, &mut rng);
            v
        })
        .collect();
    let rooms = kinds
        .iter()
        .map(|&kind| {
            let slot = match kind {
                K_START => Some(0),
                K_KAMPF => Some(1),
                K_SCHATZ => Some(2),
                K_BOSS => Some(3),
                _ => None,
            };
            // Vorlagen der Reihe nach (bei weniger Vorlagen als Räumen wiederholt)
            let tpl = slot.and_then(|s| {
                let v = &mut tpls[s];
                let t = v.first().copied();
                if let Some(t) = t {
                    v.rotate_left(1);
                    Some(t)
                } else {
                    None
                }
            });
            let name = match tpl.and_then(|k| ROOMS.get(k)) {
                Some(t) => format!("{} – {}", KIND_NAMES[kind as usize], t.name),
                None => KIND_NAMES[kind as usize].to_string(),
            };
            Room { kind, tpl, name }
        })
        .collect();
    let mirror = gen_mirror(&mut rng);
    let mut runes = [0u8; RUNE_STEPS];
    for i in 0..RUNE_STEPS {
        loop {
            let r = below(&mut rng, 9) as u8;
            if i == 0 || r != runes[i - 1] {
                runes[i] = r;
                break;
            }
        }
    }
    let sages = below(&mut rng, SAGES.len());
    let sage_answer = sage_answer(&SAGES[sages]).unwrap_or(0);
    Layout { seed, stufe: stufe.clamp(1, 5), rooms, doors, mirror, runes, sages, sage_answer }
}

/// Türen zu einer Raum-Verteilung: Zufalls-Spannbaum über alle Räume außer Schatz und Boss (+ ein paar Schleifen),
/// Schatz-Kammer nur an der Runen-Kammer, Endboss-Saal an genau einem anderen Raum (nicht der Schatz-Kammer).
fn plan_doors(k: &[u8; 9], rng: &mut Rng) -> Option<Vec<(usize, usize)>> {
    let pos = |kind: u8| k.iter().position(|&x| x == kind);
    let (t, r, b, s) = (pos(K_SCHATZ)?, pos(K_RUNEN)?, pos(K_BOSS)?, pos(K_START)?);
    if !adjacent(t, r) {
        return None;
    }
    let core: Vec<usize> = (0..9).filter(|&i| i != t && i != b).collect();
    let mut tree = vec![s];
    let mut doors = Vec::new();
    while tree.len() < core.len() {
        let cand: Vec<(usize, usize)> = tree.iter().flat_map(|&u| core.iter().filter(move |&&v| adjacent(u, v)).map(move |&v| (u, v))).filter(|e| !tree.contains(&e.1)).collect();
        if cand.is_empty() {
            return None; // Kern hängt nicht zusammen
        }
        let (u, v) = cand[below(rng, cand.len())];
        tree.push(v);
        doors.push((u.min(v), u.max(v)));
    }
    for &u in &core {
        for &v in &core {
            if u < v && adjacent(u, v) && !doors.contains(&(u, v)) && below(rng, 4) == 0 {
                doors.push((u, v));
            }
        }
    }
    doors.push((t.min(r), t.max(r)));
    let nb: Vec<usize> = core.iter().copied().filter(|&v| adjacent(b, v)).collect();
    if nb.is_empty() {
        return None;
    }
    let v = nb[below(rng, nb.len())];
    doors.push((b.min(v), b.max(v)));
    Some(doors)
}

/// Alle neun Räume sind von der Start-Halle aus über Türen erreichbar.
pub fn connected(lay: &Layout) -> bool {
    reach(lay, None).iter().all(|&x| x) && lay.rooms.len() == 9
}
/// Von der Start-Halle über Türen erreichbare Räume; `skip` = so tun, als gäbe es diesen Raum nicht.
pub fn reach(lay: &Layout, skip: Option<usize>) -> [bool; 9] {
    let mut seen = [false; 9];
    let s = find(lay, K_START);
    seen[s] = true;
    let mut stack = vec![s];
    while let Some(u) = stack.pop() {
        for &(a, b) in &lay.doors {
            let v = if a == u { b } else if b == u { a } else { continue };
            if v < 9 && !seen[v] && Some(v) != skip {
                seen[v] = true;
                stack.push(v);
            }
        }
    }
    seen
}

// ---------- Spiegel ----------
fn slash(d: (i32, i32)) -> (i32, i32) {
    (-d.1, -d.0)
}
fn backslash(d: (i32, i32)) -> (i32, i32) {
    (d.1, d.0)
}
/// Stünde hier ein Block, wäre eine Tür halb zugestellt.
fn near_door(c: (i32, i32)) -> bool {
    let (x, z) = c;
    ((x == 6 || x == 7) && (z <= 1 || z >= 12)) || ((z == 6 || z == 7) && (x <= 1 || x >= 12))
}
/// Weg des Strahls (Zellen ohne Strahler/Prismen/Empfänger) und ob er den Empfänger trifft.
pub fn beam(m: &Mirror) -> (Vec<(i32, i32)>, bool) {
    let (mut x, mut z) = m.emitter;
    let mut d = m.dir;
    let mut path = Vec::new();
    for _ in 0..256 {
        x += d.0;
        z += d.1;
        if !(0..14).contains(&x) || !(0..14).contains(&z) || (x, z) == m.emitter {
            return (path, false);
        }
        if (x, z) == m.receiver {
            return (path, true);
        }
        if let Some(p) = m.prisms.iter().find(|p| p.0 == (x, z)) {
            d = if p.1 { slash(d) } else { backslash(d) };
            continue;
        }
        if !path.contains(&(x, z)) {
            path.push((x, z));
        }
    }
    (path, false)
}
/// Lösbares, anfangs ungelöstes Spiegel-Rätsel: Strahler am Westrand, drei Umlenkungen, zwei Köder-Prismen.
pub fn gen_mirror(rng: &mut Rng) -> Mirror {
    let sg = |v: i32| v.signum();
    for _ in 0..500 {
        let z0 = [2, 3, 4, 9, 10, 11][below(rng, 6)];
        let (x1, z1) = (range(rng, 4, 11), range(rng, 1, 12));
        let (x2, z2) = (range(rng, 2, 12), range(rng, 1, 12));
        if (z1 - z0).abs() < 2 || (x2 - x1).abs() < 2 || (z2 - z1).abs() < 2 {
            continue;
        }
        let cells = [(1, z0), (x1, z0), (x1, z1), (x2, z1), (x2, z2)];
        if cells.iter().any(|&c| near_door(c)) || (0..5).any(|i| (i + 1..5).any(|j| cells[i] == cells[j])) {
            continue;
        }
        let (d0, d1, d2, d3) = ((1, 0), (0, sg(z1 - z0)), (sg(x2 - x1), 0), (0, sg(z2 - z1)));
        let mut m = Mirror {
            emitter: cells[0],
            dir: d0,
            receiver: cells[4],
            prisms: vec![(cells[1], slash(d0) == d1), (cells[2], slash(d1) == d2), (cells[3], slash(d2) == d3)],
            solution: Vec::new(),
        };
        let (path, ok) = beam(&m);
        if !ok {
            continue;
        }
        for _ in 0..2 {
            for _ in 0..20 {
                let c = (range(rng, 2, 11), range(rng, 1, 12));
                if !near_door(c) && !path.contains(&c) && !cells.contains(&c) && !m.prisms.iter().any(|p| p.0 == c) {
                    m.prisms.push((c, below(rng, 2) == 0));
                    break;
                }
            }
        }
        m.solution = m.prisms.iter().map(|p| p.1).collect();
        if !beam(&m).1 {
            continue;
        }
        for k in 0..m.prisms.len() {
            if k == 0 || below(rng, 2) == 0 {
                m.prisms[k].1 = !m.prisms[k].1;
            }
        }
        if beam(&m).1 {
            continue;
        }
        return m;
    }
    // Ersatz: von Hand gerechnet (Lösung B, B, A; das erste Prisma steht falsch)
    Mirror { emitter: (1, 3), dir: (1, 0), receiver: (10, 4), prisms: vec![((5, 3), true), ((5, 9), false), ((10, 9), true)], solution: vec![false, false, true] }
}

// ---------- Drei Weise ----------
/// Ein Rätsel-Satz: so viele der drei Weisen sagen die Wahrheit; jeder sagt (true = „liegt in“, false = „liegt nicht in“, Truhe).
pub struct SageSet {
    pub truthful: u8,
    pub says: [(bool, u8); 3],
}
pub static SAGES: [SageSet; 5] = [
    SageSet { truthful: 1, says: [(true, 0), (false, 1), (false, 0)] },
    SageSet { truthful: 1, says: [(false, 2), (true, 2), (false, 0)] },
    SageSet { truthful: 2, says: [(true, 2), (false, 0), (true, 0)] },
    SageSet { truthful: 0, says: [(false, 1), (true, 0), (true, 2)] },
    SageSet { truthful: 3, says: [(false, 2), (false, 1), (true, 0)] },
];
const ORD: [&str; 3] = ["ersten", "zweiten", "dritten"];
const NOM: [&str; 3] = ["erste", "zweite", "dritte"];
const RULE: [&str; 4] = ["Wir drei lügen alle.", "Nur einer von uns sagt die Wahrheit.", "Genau zwei von uns sagen die Wahrheit.", "Wir alle sagen die Wahrheit."];
/// Die einzige Truhe, bei der genau `truthful` Aussagen stimmen (None, wenn es keine oder mehrere gibt).
pub fn sage_answer(s: &SageSet) -> Option<u8> {
    let ok: Vec<u8> = (0..3u8).filter(|&c| s.says.iter().filter(|&&(is, k)| (c == k) == is).count() == s.truthful as usize).collect();
    (ok.len() == 1).then(|| ok[0])
}
/// Ein Satz des Weisen `nr` (0..2) samt der gemeinsamen Regel.
pub fn sage_text(s: &SageSet, nr: usize) -> String {
    let nr = nr.min(2);
    let (is, k) = s.says[nr];
    let k = (k as usize).min(2);
    let satz = if is { format!("Der Segen liegt in der Truhe des {} Weisen.", ORD[k]) } else { format!("In der Truhe des {} Weisen liegt kein Segen.", ORD[k]) };
    format!("Der {} Weise spricht: „{satz}“ ({})", NOM[nr], RULE[(s.truthful as usize).min(3)])
}

// ---------- Aufbau ----------
fn floor_of(kind: u8, x: i32, z: i32) -> u8 {
    let h = (x.wrapping_mul(73_856_093) ^ z.wrapping_mul(19_349_663)) as u32 % 7;
    match (kind, h) {
        (K_BOSS, 0) => world::GLUT,
        (K_BOSS, _) => world::BASALT,
        (_, 0 | 1) => world::MOOS_ZIEGEL,
        _ => world::STEINZIEGEL,
    }
}
/// Schreibt die ganze Katakombe als lokale Zellen (x, z 0..48, y 0..24); Luft wird nur in Türöffnungen geschrieben.
pub fn build(lay: &Layout, mut put: impl FnMut(i32, i32, i32, u8)) {
    for (i, room) in lay.rooms.iter().enumerate().take(9) {
        let (x0, z0) = room_origin(i);
        for dz in 0..ROOM {
            for dx in 0..ROOM {
                let (x, z) = (x0 + dx, z0 + dz);
                if dx == 0 || dz == 0 || dx == ROOM - 1 || dz == ROOM - 1 {
                    for y in 0..=CEIL {
                        put(x, y, z, world::TIEFENFELS);
                    }
                } else {
                    put(x, 0, z, floor_of(room.kind, x, z));
                    put(x, CEIL, z, world::SCHIEFER);
                }
            }
        }
        for (ix, iz) in [(3, 3), (10, 3), (3, 10), (10, 10)] {
            let (x, z) = interior(i, ix, iz);
            put(x, CEIL, z, world::LATERNE);
        }
        if let Some(t) = room.tpl.and_then(|k| ROOMS.get(k)) {
            for (iz, row) in t.rows.iter().enumerate() {
                for (ix, c) in row.bytes().enumerate().take(14) {
                    let (x, z) = interior(i, ix as i32, iz as i32);
                    match c {
                        b'#' => {
                            for y in 1..CEIL {
                                put(x, y, z, world::BRUCHSTEIN);
                            }
                        }
                        b'o' => put(x, 1, z, world::MOOS_BRUCH),
                        b'L' => {
                            put(x, 1, z, world::BRUCHSTEIN);
                            put(x, 2, z, world::LATERNE);
                        }
                        _ => {}
                    }
                }
            }
        }
        let at = |ix: i32, iz: i32| interior(i, ix, iz);
        match room.kind {
            K_START => {
                for (ix, iz) in markers(lay, i, b's') {
                    let (x, z) = at(ix, iz);
                    put(x, 0, z, world::GOLDZIER);
                }
            }
            K_SCHATZ => {
                for (ix, iz) in markers(lay, i, b't') {
                    let (x, z) = at(ix, iz);
                    put(x, 1, z, world::SCHATZ);
                }
            }
            K_BOSS => {
                for (ix, iz) in markers(lay, i, b'B') {
                    let (x, z) = at(ix, iz);
                    put(x, 0, z, world::GLUT);
                }
            }
            K_SPIEGEL => {
                let m = &lay.mirror;
                let mut blocks = vec![(m.emitter, world::STRAHLER), (m.receiver, world::EMPFAENGER)];
                blocks.extend(m.prisms.iter().map(|p| (p.0, if p.1 { world::PRISMA_A } else { world::PRISMA_B })));
                for ((ix, iz), b) in blocks {
                    let (x, z) = at(ix, iz);
                    put(x, BEAM_Y - 1, z, world::BRUCHSTEIN);
                    put(x, BEAM_Y, z, b);
                }
                for (ix, iz) in beam(m).0 {
                    let (x, z) = at(ix, iz);
                    put(x, BEAM_Y, z, world::STRAHL);
                }
            }
            K_RUNEN => {
                for k in 0..9 {
                    let (ix, iz) = rune_cell(k);
                    let (x, z) = at(ix, iz);
                    put(x, 0, z, world::RUNE);
                }
            }
            K_WEISEN => {
                for &sx in &SAGE_X {
                    let (x, z) = at(sx, STATUE_Z);
                    put(x, 1, z, world::STATUE);
                    put(x, 2, z, world::STATUE);
                    let (x, z) = at(sx, CHEST_Z);
                    put(x, 1, z, world::WEISEN_TRUHE);
                }
            }
            _ => {}
        }
    }
    for &(a, b) in &lay.doors {
        let fill = door_fill(lay, a, b);
        for (x, z) in door_cells(a, b) {
            for y in 1..=DOOR_H {
                put(x, y, z, fill);
            }
        }
    }
}

// ---------- Wertung ----------
/// [Können, Tempo, Rätsel] je 0..100.
pub fn score(tode: u32, ms: i64, solved: u32) -> [u32; 3] {
    let koennen = 100u32.saturating_sub(25u32.saturating_mul(tode));
    let tempo = if ms <= 300_000 {
        100
    } else if ms >= 900_000 {
        0
    } else {
        ((900_000 - ms) * 100 / 600_000) as u32
    };
    [koennen, tempo, 100 * solved.min(3) / 3]
}
pub fn grade(p: [u32; 3]) -> &'static str {
    if p.iter().all(|&v| v >= 100) {
        return "S+";
    }
    let m = (p[0] + p[1] + p[2]) as f32 / 3.0;
    if m >= 90.0 {
        "S"
    } else if m >= 75.0 {
        "A"
    } else if m >= 55.0 {
        "B"
    } else {
        "C"
    }
}
/// 0 C … 4 S+ (so steht die Note in dungeon_best).
pub fn grade_rank(g: &str) -> u8 {
    GRADES.iter().position(|&x| x == g).unwrap_or(0) as u8
}

// ---------- Beute ----------
/// Beute-Altar (jede Note): Wissens-Splitter. Die Taler (60·Stufe) bucht der Server dazu.
pub fn altar_loot(stufe: u8) -> Vec<(u16, u32)> {
    vec![(sb::SHARD + 4, 2 + 2 * stufe.clamp(1, 5) as u32)]
}
/// Versteckte Schatz-Kammer (je Mitglied einmal).
pub fn treasure_loot(stufe: u8) -> Vec<(u16, u32)> {
    vec![(sb::SHARD + 4, 3 + stufe.clamp(1, 5) as u32), (sb::FOKUS_KRISTALL, 1)]
}
/// Smaragd- (ab „S“) bzw. Obsidian-Truhe („S+“): die einzigen Quellen der Katakomben-Artefakte; Chancen (Promille) steigen mit der Stufe.
pub fn chest_loot(obsidian: bool, stufe: u8, rng: &mut Rng) -> Vec<(u16, u32)> {
    let s = stufe.clamp(1, 5) as u64;
    let table: [(u16, u32, u64); 4] = if obsidian {
        [(rpg::FOLIANT_VI, 1, 150 + 100 * s), (rpg::RUNEN_HERZ, 1, 100 + 60 * s), (rpg::SCHATTEN_LATERNE, 1, 100 + 60 * s), (rpg::MEISTER_KERN, 1 + s as u32 / 2, 1000)]
    } else {
        [(rpg::MEISTER_KERN, 1, 250 + 100 * s), (rpg::VERZ_BUCH, 1, 400 + 100 * s), (rpg::SCHATTEN_LATERNE, 1, 40 + 30 * s), (rpg::RUNEN_HERZ, 1, 40 + 30 * s)]
    };
    let mut out: Vec<(u16, u32)> = table.iter().filter(|e| rng.below(1000) < e.2).map(|e| (e.0, e.1)).collect();
    if out.is_empty() {
        out.push((rpg::VERZ_BUCH, 1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grundriss_200_seeds() {
        for seed in 0..200u64 {
            let lay = generate(seed.wrapping_mul(0x9E37_79B9) ^ 77, 1 + (seed % 5) as u8);
            assert!(connected(&lay), "Seed {seed}: nicht zusammenhängend");
            let mut n = [0; 7];
            for r in &lay.rooms {
                n[r.kind as usize] += 1;
            }
            assert_eq!(n, [1, 3, 1, 1, 1, 1, 1], "Seed {seed}: Raumarten");
            let (t, r) = (find(&lay, K_SCHATZ), find(&lay, K_RUNEN));
            let at_t: Vec<_> = lay.doors.iter().filter(|d| d.0 == t || d.1 == t).collect();
            assert_eq!(at_t, vec![&(t.min(r), t.max(r))], "Seed {seed}: Schatz-Kammer nur über die Runen-Kammer");
            assert!(!reach(&lay, Some(r))[t], "Seed {seed}: Schatz ohne Runen-Kammer erreichbar");
            assert!(lay.doors.iter().all(|&(a, b)| a < b && adjacent(a, b)));
            let b = find(&lay, K_BOSS);
            assert_eq!(lay.doors.iter().filter(|d| d.0 == b || d.1 == b).count(), 1);
            // Spiegel: lösbar, anfangs ungelöst
            let m = &lay.mirror;
            assert!(!beam(m).1, "Seed {seed}: Spiegel schon gelöst");
            let mut sol = m.clone();
            for (p, &s) in sol.prisms.iter_mut().zip(&m.solution) {
                p.1 = s;
            }
            assert!(beam(&sol).1, "Seed {seed}: Spiegel unlösbar");
            // Runen-Folge
            assert!(lay.runes.iter().all(|&k| k < 9) && lay.runes.windows(2).all(|w| w[0] != w[1]));
            assert_eq!(Some(lay.sage_answer), sage_answer(&SAGES[lay.sages]));
            // Aufbau bleibt im Feld, Ankunft ist frei
            let mut grid = vec![0u8; 48 * 48 * 24];
            build(&lay, |x, y, z, b| {
                assert!((0..48).contains(&x) && (0..48).contains(&z) && (0..24).contains(&y));
                grid[((y * 48 + z) * 48 + x) as usize] = b;
            });
            let s = find(&lay, K_START);
            let (ix, iz) = markers(&lay, s, b's')[0];
            let (x, z) = interior(s, ix, iz);
            let g = |y: i32| grid[((y * 48 + z) * 48 + x) as usize];
            assert!(world::solid(g(0)) && !world::solid(g(1)) && !world::solid(g(2)));
        }
    }

    #[test]
    fn vorlagen_gueltig() {
        for t in ROOMS {
            assert_eq!(tpl_check(t), Ok(()));
        }
        for k in [T_START, T_KAMPF, T_SCHATZ, T_BOSS] {
            assert!(ROOMS.iter().any(|t| t.kind == k), "Art {k} fehlt");
        }
    }

    #[test]
    fn genau_eine_wahre_truhe() {
        for s in &SAGES {
            assert!(sage_answer(s).is_some());
        }
        assert!(SAGES.len() >= 4);
        assert!(sage_text(&SAGES[0], 1).contains("zweite"));
    }

    #[test]
    fn noten() {
        assert_eq!(grade(score(0, 240_000, 3)), "S+");
        assert_ne!(grade(score(0, 240_000, 2)), "S+");
        assert_eq!(grade(score(0, 240_000, 2)), "A");
        assert_eq!(score(5, 900_000, 0), [0, 0, 0]);
        assert_eq!(score(1, 600_000, 1), [75, 50, 33]);
        assert_eq!(grade(score(4, 1_000_000, 0)), "C");
        assert_eq!(grade_rank("S+"), 4);
        let mut rng = Rng(5);
        let o = chest_loot(true, 5, &mut rng);
        assert!(o.iter().any(|e| e.0 == rpg::MEISTER_KERN));
        assert!(!chest_loot(false, 1, &mut rng).is_empty());
    }
}
