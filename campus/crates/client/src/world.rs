//! Welt im Browser: geladene Stücke, Gitter nur bei Änderung neu, Strahl für das Ziel-Feld.
use crate::gfx::{ChunkGpu, Gfx};
use campus_shared::mesh::{self, MeshOut, PAD};
use campus_shared::world::{self, WorldState, CH, VOL, WC, WORLD};
use glam::Vec3;

pub struct World {
    pub st: WorldState,
    blocks: Vec<Option<Vec<u8>>>, // Some(leer) = geladen, reine Luft
    pub gpu: Vec<ChunkGpu>,
    wait: Vec<u8>, // 0 = sauber, sonst Bilder bis zum Neuaufbau (wartet kurz auf Nachbarn)
    pad: Vec<u8>,
    out: MeshOut,
    pub loaded: u32,
    /// Leuchtende Blöcke mit freier Seite je Stück: x, y, z, Block (für farbige Punktlichter)
    pub glow: Vec<Vec<[i32; 4]>>,
}

fn ci(cx: i32, cz: i32) -> usize {
    (cz * WC + cx) as usize
}

impl World {
    pub fn new() -> World {
        let n = (WC * WC) as usize;
        World { st: WorldState::default(), blocks: (0..n).map(|_| None).collect(), gpu: (0..n).map(|_| ChunkGpu::default()).collect(), wait: vec![0; n], pad: vec![0; PAD], out: MeshOut::default(), loaded: 0, glow: (0..n).map(|_| Vec::new()).collect() }
    }

    pub fn has(&self, x: i32, z: i32) -> bool {
        (0..WORLD).contains(&x) && (0..WORLD).contains(&z) && self.blocks[ci(x >> 4, z >> 4)].is_some()
    }
    /// Unter y 0 ist Leere (kein unsichtbarer Boden).
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if !(0..CH).contains(&y) || !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) {
            return 0;
        }
        match &self.blocks[ci(x >> 4, z >> 4)] {
            Some(b) if !b.is_empty() => b[world::idx(x & 15, y, z & 15)],
            _ => 0,
        }
    }
    fn touch(&mut self, cx: i32, cz: i32, now: bool) {
        if (0..WC).contains(&cx) && (0..WC).contains(&cz) && self.blocks[ci(cx, cz)].is_some() {
            let w = &mut self.wait[ci(cx, cz)];
            *w = if now { 1 } else if *w == 0 { 20 } else { *w };
        }
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, b: u8) {
        if !(0..CH).contains(&y) || !self.has(x, z) {
            return;
        }
        let (cx, cz) = (x >> 4, z >> 4);
        if let Some(c) = &mut self.blocks[ci(cx, cz)] {
            if c.is_empty() {
                *c = vec![0u8; VOL];
            }
            c[world::idx(x & 15, y, z & 15)] = b;
        }
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            if (dx, dz) == (0, 0) || (dx == -1 && x & 15 == 0) || (dx == 1 && x & 15 == 15) || (dz == -1 && z & 15 == 0) || (dz == 1 && z & 15 == 15) {
                self.touch(cx + dx, cz + dz, true);
            }
        }
    }
    pub fn load(&mut self, cx: i32, cz: i32, rle: &[u8]) {
        if !(0..WC).contains(&cx) || !(0..WC).contains(&cz) {
            return;
        }
        if self.blocks[ci(cx, cz)].is_none() {
            self.loaded += 1;
        }
        let mut b = vec![0u8; VOL];
        world::unrle(rle, &mut b);
        // reine Luft (Leere zwischen den Inseln) kostet keinen Speicher
        self.blocks[ci(cx, cz)] = Some(if b.iter().all(|&v| v == 0) { Vec::new() } else { b });
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            self.touch(cx + dx, cz + dz, false);
        }
    }

    /// Baut höchstens `budget` Gitter je Bild neu, die nächsten zuerst. Ein Stück wartet kurz, bis seine Nachbarn da sind.
    pub fn remesh(&mut self, gfx: &Gfx, cam: Vec3, budget: usize) {
        for _ in 0..budget {
            let mut best: Option<(usize, i32)> = None;
            for cz in 0..WC {
                for cx in 0..WC {
                    let i = ci(cx, cz);
                    if self.wait[i] == 0 {
                        continue;
                    }
                    let all = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().all(|&(dx, dz)| !(0..WC).contains(&(cx + dx)) || !(0..WC).contains(&(cz + dz)) || self.blocks[ci(cx + dx, cz + dz)].is_some());
                    if self.wait[i] > 1 && !all {
                        continue;
                    }
                    let d = (cx * 16 + 8 - cam.x as i32).pow(2) + (cz * 16 + 8 - cam.z as i32).pow(2);
                    if best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((i, d));
                    }
                }
            }
            let Some((i, _)) = best else { break };
            self.wait[i] = 0;
            let (cx, cz) = (i as i32 % WC, i as i32 / WC);
            self.glow[i].clear();
            if self.blocks[i].as_ref().is_some_and(|b| b.is_empty()) {
                gfx.upload(&mut self.gpu[i], &[], &[]);
                continue;
            }
            for y in 0..CH {
                for z in -1..=16 {
                    for x in -1..=16 {
                        let v = self.get(cx * 16 + x, y, cz * 16 + z);
                        self.pad[mesh::pidx(x, y, z)] = v;
                    }
                }
            }
            for z in -1..=16 {
                for x in -1..=16 {
                    self.pad[mesh::pidx(x, -1, z)] = 0;
                    self.pad[mesh::pidx(x, CH, z)] = 0;
                }
            }
            mesh::mesh(&self.pad, cx * 16, cz * 16, &mut self.out);
            let (pad, glow) = (&self.pad, &mut self.glow[i]);
            for y in 0..CH {
                for z in 0..16 {
                    for x in 0..16 {
                        let b = pad[mesh::pidx(x, y, z)];
                        if world::FLAGS[b as usize] & world::F_GLOW != 0
                            && [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().any(|&(dx, dy, dz)| !world::opaque(pad[mesh::pidx(x + dx, y + dy, z + dz)]))
                        {
                            glow.push([cx * 16 + x, y, cz * 16 + z, b as i32]);
                        }
                    }
                }
            }
            gfx.upload(&mut self.gpu[i], &self.out.solid, &self.out.clear);
        }
        for w in &mut self.wait {
            if *w > 1 {
                *w -= 1;
            }
        }
    }

    /// Strahl durch das Blockgitter: (getroffener Block, freies Feld davor).
    pub fn raycast(&self, o: Vec3, d: Vec3, max: f32) -> Option<([i32; 3], [i32; 3])> {
        world::raycast(|x, y, z| self.get(x, y, z), o.to_array(), d.to_array(), max)
    }

    /// Stößt ein Körper (Füße bei p, 0,6 breit, 1,8 hoch) an feste Blöcke?
    pub fn collides(&self, p: Vec3) -> bool {
        let (x0, x1) = ((p.x - 0.3).floor() as i32, (p.x + 0.3).floor() as i32);
        let (y0, y1) = (p.y.floor() as i32, (p.y + 1.79).floor() as i32);
        let (z0, z1) = ((p.z - 0.3).floor() as i32, (p.z + 0.3).floor() as i32);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if world::solid(self.get(x, y, z)) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
