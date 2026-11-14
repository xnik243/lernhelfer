//! Figuren, Cosmetics, Begleiter und Partikel – alles aus Quadern zusammengesetzt (ein Instanz-Aufruf für alle).
//! Keine Modelle, keine Texturen: jede Form entsteht aus wenigen Parametern (Form-Nummer, zwei Farben).
use crate::deco;
use crate::gfx::{Inst, MAX_INST};
use campus_shared::items::{self, Look, PALETTE, SKINS};
use campus_shared::{rpg, stats};
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::PI;

pub struct Pose {
    pub pos: Vec3,
    pub yaw: f32,
    pub walk: f32,   // Schrittphase
    pub moving: f32, // 0..1
    pub emote: u8,   // 255 = keines
    pub emote_t: f32,
    pub time: f32,
    pub pet: Vec3,
    /// in der rechten Hand: Farbe, leuchtet, Werkzeug (sonst Block); nur die eigene Figur
    pub held: Option<([u8; 3], bool, bool)>,
    /// Ausrüstung: 0..4 Rüstung, 4..7 Talismane, 7 gehalten (Waffe wird gezeichnet, wenn `held` None)
    pub gear: [u16; 8],
    /// Rot-Anteil beim Treffer 0..1
    pub hurt: f32,
}

pub fn push(out: &mut Vec<Inst>, m: Mat4, rgb: [u8; 3], glow: bool) {
    push_a(out, m, rgb, if glow { 255 } else { 0 });
}
/// Wie `push`, aber mit freiem Alpha-Code (1..8 Riss-Stufe, 9 Drahtrahmen; siehe Shader `fsi`).
pub fn push_a(out: &mut Vec<Inst>, m: Mat4, rgb: [u8; 3], a: u8) {
    if out.len() < MAX_INST {
        let t = m.transpose();
        out.push(Inst { m: [t.x_axis.to_array(), t.y_axis.to_array(), t.z_axis.to_array()], col: [rgb[0], rgb[1], rgb[2], a] });
    }
}
/// Quader an einem Gelenk: `pivot` im Körper, Drehung, Versatz der Mitte, Größe.
pub(crate) fn part(out: &mut Vec<Inst>, root: &Mat4, pivot: Vec3, rot: Quat, off: Vec3, size: Vec3, rgb: [u8; 3], glow: bool) {
    push(out, *root * Mat4::from_translation(pivot) * Mat4::from_quat(rot) * Mat4::from_translation(off) * Mat4::from_scale(size), rgb, glow);
}
pub(crate) fn cube(out: &mut Vec<Inst>, root: &Mat4, at: Vec3, size: Vec3, rgb: [u8; 3], glow: bool) {
    part(out, root, at, Quat::IDENTITY, Vec3::ZERO, size, rgb, glow);
}
pub fn world_box(out: &mut Vec<Inst>, at: Vec3, size: Vec3, rgb: [u8; 3], glow: bool) {
    push(out, Mat4::from_translation(at) * Mat4::from_scale(size), rgb, glow);
}
pub(crate) fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)) as u8)
}
pub(crate) fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}
const SHOE: [u8; 3] = [54, 40, 32];
const BELT: [u8; 3] = [66, 46, 32];
const GOLD: [u8; 3] = [255, 210, 80];

/// Färbt alle seit `from` gezeichneten, nicht leuchtenden Quader rötlich (Treffer-Aufblitzen).
pub(crate) fn tint(out: &mut [Inst], from: usize, hurt: f32) {
    if hurt <= 0.0 {
        return;
    }
    for i in out.iter_mut().skip(from) {
        if i.col[3] <= 127 {
            let c = mix([i.col[0], i.col[1], i.col[2]], [235, 40, 40], hurt * 0.7);
            i.col[..3].copy_from_slice(&c);
        }
    }
}

/// Rüstung als Überzug, leicht größer als die Körperteile; folgt denselben Gelenken. `gear[0..4]` = Helm, Brust, Beine, Stiefel.
pub(crate) fn armor(out: &mut Vec<Inst>, r: &Mat4, gear: &[u16], legs: [Quat; 2], arms: [Quat; 2], robe: bool) {
    let col = |i: usize| gear.get(i).and_then(|&g| rpg::equip(g)).filter(|e| e.slot as usize == i).map(|e| e.rgb);
    let dark = |c: [u8; 3]| mix(c, [0, 0, 0], 0.3);
    if let Some(c) = col(0) {
        cube(out, r, v(0.0, 1.845, 0.0), v(0.48, 0.09, 0.48), c, false);
        cube(out, r, v(0.0, 1.62, 0.23), v(0.48, 0.44, 0.07), c, false);
        for x in [-0.225f32, 0.225] {
            cube(out, r, v(x, 1.66, 0.0), v(0.05, 0.36, 0.46), c, false);
        }
        cube(out, r, v(0.0, 1.765, -0.225), v(0.48, 0.07, 0.05), dark(c), false); // Stirnband
        cube(out, r, v(0.0, 1.93, 0.02), v(0.06, 0.08, 0.32), dark(c), false); // Kamm
    }
    if let Some(c) = col(1) {
        cube(out, r, v(0.0, 1.1, 0.0), v(0.56, 0.6, 0.32), c, false);
        cube(out, r, v(0.0, 1.13, -0.162), v(0.3, 0.36, 0.02), mix(c, [255, 255, 255], 0.25), false); // Brustplatte
        for (x, q) in [(-0.33f32, arms[0]), (0.33, arms[1])] {
            part(out, r, v(x, 1.36, 0.0), q, v(x.signum() * 0.02, -0.08, 0.0), v(0.21, 0.18, 0.24), dark(c), false); // Schulterstück
        }
    }
    if robe {
        return;
    }
    for (x, q) in [(-0.12f32, legs[0]), (0.12, legs[1])] {
        if let Some(c) = col(2) {
            part(out, r, v(x, 0.75, 0.0), q, v(0.0, -0.25, 0.0), v(0.24, 0.46, 0.26), c, false);
            part(out, r, v(x, 0.75, 0.0), q, v(0.0, -0.36, -0.13), v(0.14, 0.1, 0.02), dark(c), false); // Knieschutz
        }
        if let Some(c) = col(3) {
            part(out, r, v(x, 0.75, 0.0), q, v(0.0, -0.64, -0.02), v(0.25, 0.2, 0.3), c, false);
        }
    }
}

/// Talismane (`gear[4..7]`) kreisen als leuchtende Funken um `center`.
pub(crate) fn sparkles(out: &mut Vec<Inst>, center: Vec3, gear: &[u16], time: f32) {
    for k in 4..7usize {
        if let Some(e) = gear.get(k).and_then(|&g| rpg::equip(g)).filter(|e| e.slot == rpg::SL_TALISMAN) {
            let a = time * 1.5 + k as f32 * 2.1;
            world_box(out, center + v(a.cos() * 0.45, (time * 2.0 + k as f32).sin() * 0.15, a.sin() * 0.45), Vec3::splat(0.05), e.rgb, true);
        }
    }
}

/// Haar als Kappe: oben, hinten, an den Seiten und ein Pony über der Stirn.
fn hair_cap(out: &mut Vec<Inst>, r: &Mat4, hair: [u8; 3]) {
    cube(out, r, v(0.0, 1.85, 0.01), v(0.45, 0.1, 0.45), hair, false);
    cube(out, r, v(0.0, 1.69, 0.2), v(0.45, 0.3, 0.06), hair, false);
    for x in [-0.2f32, 0.2] {
        cube(out, r, v(x, 1.74, 0.04), v(0.06, 0.2, 0.36), hair, false);
    }
    cube(out, r, v(0.0, 1.79, -0.2), v(0.44, 0.06, 0.06), hair, false);
}

/// Zeichnet eine Figur samt angelegter Cosmetics. Vorn ist -Z.
pub fn avatar(out: &mut Vec<Inst>, look: &Look, p: &Pose) {
    let (skin, hair, ca, cb) = (SKINS[look.skin as usize % SKINS.len()], PALETTE[look.hair_col as usize % PALETTE.len()], PALETTE[look.col_a as usize % PALETTE.len()], PALETTE[look.col_b as usize % PALETTE.len()]);
    let (e, t) = (p.emote, p.emote_t);
    let mut base = p.pos;
    let mut yaw = p.yaw;
    let swing = p.walk.sin() * 0.8 * p.moving;
    let (mut leg_l, mut leg_r) = (Quat::from_rotation_x(swing), Quat::from_rotation_x(-swing));
    let (mut arm_l, mut arm_r) = (Quat::from_rotation_x(-swing * 0.75), Quat::from_rotation_x(swing * 0.75));
    match e {
        0 => arm_r = Quat::from_rotation_z(2.6 + (t * 10.0).sin() * 0.4), // Winken
        1 => {
            // Jubeln
            base.y += (t * 8.0).sin().abs() * 0.25;
            arm_l = Quat::from_rotation_z(-2.8);
            arm_r = Quat::from_rotation_z(2.8);
        }
        2 => {
            // Hinsetzen
            base.y -= 0.42;
            leg_l = Quat::from_rotation_x(PI / 2.0);
            leg_r = leg_l;
        }
        3 => {
            // Tanz
            yaw += (t * 6.0).sin() * 0.5;
            base.y += (t * 12.0).sin().abs() * 0.12;
            arm_l = Quat::from_rotation_z(-1.2 - (t * 12.0).sin());
            arm_r = Quat::from_rotation_z(1.2 - (t * 12.0).sin());
        }
        _ => {}
    }
    let start = out.len();
    let helm = rpg::equip(p.gear[0]).is_some_and(|e| e.slot == rpg::SL_HELM);
    let root = Mat4::from_translation(base) * Mat4::from_rotation_y(yaw);
    let r = &root;
    // Beine, Rumpf, Arme, Kopf
    if look.outfit == 2 {
        cube(out, r, v(0.0, 0.42, 0.0), v(0.5, 0.84, 0.3), ca, false); // Robe bis zu den Füßen
        cube(out, r, v(0.0, 0.04, 0.0), v(0.34, 0.08, 0.24), cb, false);
    } else {
        for (x, q) in [(-0.12, leg_l), (0.12, leg_r)] {
            part(out, r, v(x, 0.75, 0.0), q, v(0.0, -0.375, 0.0), v(0.2, 0.75, 0.22), cb, false);
            part(out, r, v(x, 0.75, 0.0), q, v(0.0, -0.7, -0.02), v(0.22, 0.1, 0.27), SHOE, false); // Schuh
        }
    }
    // Jacke mit Schattierung: dunkler Kragen und Saum, Gürtel mit Schnalle, helles Hemd in der offenen Front
    let dark = mix(ca, [0, 0, 0], 0.3);
    cube(out, r, v(0.0, 1.075, 0.0), v(0.5, 0.65, 0.26), ca, false);
    cube(out, r, v(0.0, 1.38, 0.0), v(0.36, 0.05, 0.28), dark, false);
    if look.outfit != 2 {
        cube(out, r, v(0.0, 0.79, 0.0), v(0.52, 0.08, 0.28), BELT, false);
        cube(out, r, v(0.0, 0.79, -0.142), v(0.08, 0.06, 0.02), [222, 186, 84], false);
        cube(out, r, v(0.0, 0.88, 0.0), v(0.51, 0.1, 0.27), dark, false);
    }
    if look.outfit == 0 {
        cube(out, r, v(0.0, 1.12, -0.135), v(0.12, 0.46, 0.02), mix(cb, [255, 255, 255], 0.55), false);
    }
    if look.outfit == 3 {
        cube(out, r, v(0.0, 1.075, -0.135), v(0.14, 0.6, 0.02), cb, false); // Weste: Knopfleiste
    }
    if look.outfit == 1 {
        cube(out, r, v(0.0, 1.45, 0.17), v(0.4, 0.26, 0.12), ca, false); // Kapuze
    }
    for (x, q) in [(-0.33, arm_l), (0.33, arm_r)] {
        part(out, r, v(x, 1.36, 0.0), q, v(0.0, -0.2, 0.0), v(0.15, 0.4, 0.18), ca, false);
        part(out, r, v(x, 1.36, 0.0), q, v(0.0, -0.39, 0.0), v(0.16, 0.04, 0.19), dark, false); // Ärmelbund
        part(out, r, v(x, 1.36, 0.0), q, v(0.0, -0.5, 0.0), v(0.14, 0.22, 0.17), skin, false);
    }
    match p.held {
        Some((c, g, true)) => {
            part(out, r, v(0.33, 1.36, 0.0), arm_r, v(0.0, -0.58, -0.2), v(0.05, 0.05, 0.42), [120, 86, 54], false); // Stiel nach vorn
            part(out, r, v(0.33, 1.36, 0.0), arm_r, v(0.0, -0.58, -0.42), v(0.1, 0.18, 0.1), c, g);
        }
        Some((c, g, false)) => part(out, r, v(0.33, 1.36, 0.0), arm_r, v(0.0, -0.6, -0.14), v(0.2, 0.2, 0.2), c, g),
        None if rpg::is_weapon(p.gear[7]) => {
            // Nahkampf schräg nach vorn, Bogen/Stab senkrecht vor dem Arm
            let hand = *r * Mat4::from_translation(v(0.33, 1.36, 0.0)) * Mat4::from_quat(arm_r);
            let m = if rpg::is_ranged(p.gear[7]) { hand * Mat4::from_translation(v(0.0, -0.58, -0.15)) } else { hand * Mat4::from_translation(v(0.0, -0.58, -0.04)) * Mat4::from_rotation_x(-1.2) };
            deco::weapon(out, m, p.gear[7]);
        }
        None => {}
    }
    armor(out, r, &p.gear, [leg_l, leg_r], [arm_l, arm_r], look.outfit == 2);
    // Kopf mit Gesicht: Augenweiß, Pupillen, Brauen, Mund
    cube(out, r, v(0.0, 1.61, 0.0), v(0.42, 0.42, 0.42), skin, false);
    let brow = mix(hair, [0, 0, 0], 0.25);
    for x in [-0.1f32, 0.1] {
        cube(out, r, v(x, 1.64, -0.212), v(0.09, 0.07, 0.02), [245, 245, 245], false);
        cube(out, r, v(x - x.signum() * 0.02, 1.64, -0.218), v(0.045, 0.07, 0.02), [40, 52, 96], false);
        cube(out, r, v(x, 1.705, -0.212), v(0.1, 0.025, 0.02), brow, false);
    }
    cube(out, r, v(0.0, 1.51, -0.212), v(0.13, 0.03, 0.02), [150, 72, 72], false);
    let hair_style = if helm { 255 } else { look.hair };
    match hair_style {
        0 => hair_cap(out, r, hair),
        1 => {
            hair_cap(out, r, hair);
            cube(out, r, v(0.0, 1.55, 0.225), v(0.45, 0.6, 0.06), hair, false);
        }
        2 => {
            hair_cap(out, r, hair);
            part(out, r, v(0.0, 1.78, 0.24), Quat::from_rotation_x(0.5 + swing * 0.3), v(0.0, -0.2, 0.0), v(0.12, 0.4, 0.12), hair, false);
        }
        3 => {
            for (x, z) in [(-0.16, -0.12), (0.16, -0.12), (-0.16, 0.14), (0.16, 0.14), (0.0, 0.0)] {
                cube(out, r, v(x, 1.87, z), v(0.2, 0.16, 0.2), hair, false);
            }
        }
        _ => {}
    }
    let c = |slot: u8| items::cosmetic(look.slots[slot as usize]).filter(|c| c.slot == slot);
    // 1 Umhang: fünf Glieder, die beim Laufen wehen
    if let Some(c) = c(items::CAPE) {
        let mut m = *r * Mat4::from_translation(v(0.0, 1.38, 0.14));
        for k in 0..5 {
            let ang = 0.12 + p.moving * 0.22 + (p.time * 3.0 + k as f32 * 0.7).sin() * (0.03 + p.moving * 0.05);
            m = m * Mat4::from_rotation_x(ang);
            let col = match c.shape {
                1 if k == 4 => c.rgb2,
                2 if k % 2 == 1 => c.rgb2,
                3 => mix(c.rgb, c.rgb2, 0.5 + 0.5 * (p.time * 2.0 + k as f32).sin()),
                _ => c.rgb,
            };
            push(out, m * Mat4::from_translation(v(0.0, -0.11, 0.0)) * Mat4::from_scale(v(0.46 + k as f32 * 0.02, 0.22, 0.03)), col, false);
            if c.shape == 3 {
                push(out, m * Mat4::from_translation(v((k as f32 * 1.7).sin() * 0.16, -0.1, 0.02)) * Mat4::from_scale(Vec3::splat(0.035)), c.rgb2, true);
            }
            m = m * Mat4::from_translation(v(0.0, -0.22, 0.0));
        }
    }
    // 2 Kopf
    if let Some(c) = c(items::HEAD) {
        let top = 1.9;
        match c.shape {
            0 => {
                cube(out, r, v(0.0, top, 0.0), v(0.66, 0.04, 0.66), c.rgb, false);
                cube(out, r, v(0.0, top + 0.1, 0.0), v(0.38, 0.16, 0.38), c.rgb, false);
                cube(out, r, v(0.0, top + 0.05, 0.0), v(0.4, 0.05, 0.4), c.rgb2, false);
            }
            1 => {
                cube(out, r, v(0.0, top + 0.03, 0.0), v(0.4, 0.1, 0.4), c.rgb, false);
                part(out, r, v(0.0, top + 0.1, 0.0), Quat::from_rotation_y(PI / 4.0), Vec3::ZERO, v(0.6, 0.03, 0.6), c.rgb, false);
                part(out, r, v(0.26, top + 0.1, 0.0), Quat::from_rotation_z((p.time * 2.0).sin() * 0.2 + swing * 0.3), v(0.0, -0.1, 0.0), v(0.03, 0.2, 0.03), c.rgb2, false);
            }
            2 => {
                cube(out, r, v(0.0, top - 0.03, 0.0), v(0.5, 0.05, 0.08), c.rgb, false);
                for x in [-0.24, 0.24] {
                    cube(out, r, v(x, 1.62, 0.0), v(0.08, 0.2, 0.2), c.rgb, false);
                    cube(out, r, v(x * 1.2, 1.62, 0.0), v(0.03, 0.12, 0.12), c.rgb2, true);
                }
            }
            3 | 6 => {
                // Heiligenschein (schwebt, dreht sich, leuchtet) oder Blumenkranz (liegt auf)
                let (n, y, rad, glow) = if c.shape == 3 { (10, top + 0.22 + (p.time * 2.0).sin() * 0.02, 0.25, true) } else { (8, top - 0.04, 0.24, false) };
                for k in 0..n {
                    let a = k as f32 / n as f32 * PI * 2.0 + if glow { p.time } else { 0.0 };
                    cube(out, r, v(a.cos() * rad, y, a.sin() * rad), Vec3::splat(if glow { 0.07 } else { 0.1 }), if k % 2 == 0 { c.rgb } else { c.rgb2 }, glow);
                }
            }
            4 => {
                cube(out, r, v(0.0, top, 0.0), v(0.44, 0.08, 0.44), c.rgb, false);
                for k in 0..6 {
                    let a = k as f32 / 6.0 * PI * 2.0;
                    cube(out, r, v(a.cos() * 0.19, top + 0.1, a.sin() * 0.19), v(0.07, 0.14, 0.07), c.rgb, false);
                }
                cube(out, r, v(0.0, top + 0.02, -0.225), Vec3::splat(0.07), c.rgb2, true);
            }
            _ => {
                cube(out, r, v(0.0, top, 0.0), v(0.6, 0.04, 0.6), c.rgb, false);
                cube(out, r, v(0.0, top + 0.2, 0.0), v(0.36, 0.38, 0.36), c.rgb, false);
                cube(out, r, v(0.0, top + 0.06, 0.0), v(0.38, 0.07, 0.38), c.rgb2, false);
            }
        }
    }
    // 3 Rücken
    if let Some(c) = c(items::BACK) {
        match c.shape {
            0 => {
                cube(out, r, v(0.0, 1.1, 0.22), v(0.36, 0.44, 0.18), c.rgb, false);
                cube(out, r, v(0.0, 1.0, 0.32), v(0.22, 0.16, 0.04), c.rgb2, false);
            }
            1 => {
                for k in 0..3 {
                    cube(out, r, v(((k * 7) % 3) as f32 * 0.02 - 0.02, 0.94 + k as f32 * 0.14, 0.24), v(0.34, 0.12, 0.2), if k % 2 == 0 { c.rgb } else { c.rgb2 }, false);
                }
                cube(out, r, v(0.0, 1.1, 0.24), v(0.04, 0.46, 0.22), [60, 44, 30], false);
            }
            _ => {
                // Flügel: Fächer aus Federn, schlagen im Takt; Libelle: vier schmale, schnelle, leuchtende Blätter
                let fast = c.shape == 3;
                let flap = (p.time * if fast { 22.0 } else { 4.0 }).sin() * if fast { 0.35 } else { 0.3 } + 0.3;
                for side in [-1.0f32, 1.0] {
                    for k in 0..if fast { 2 } else { 4 } {
                        let q = Quat::from_rotation_y(side * (0.5 + flap)) * Quat::from_rotation_z(side * (-0.3 - k as f32 * if fast { 0.7 } else { 0.32 }));
                        let len = if fast { 0.8 } else { 0.7 - k as f32 * 0.1 };
                        part(out, r, v(side * 0.1, 1.32, 0.16), q, v(side * len / 2.0, 0.0, 0.0), v(len, if fast { 0.1 } else { 0.16 }, 0.02), if k % 2 == 0 { c.rgb } else { c.rgb2 }, fast);
                    }
                }
            }
        }
    }
    // 4 Begleiter: folgt weich hinterher
    if let Some(c) = c(items::COMPANION) {
        let bob = (p.time * 3.0).sin() * 0.06;
        let dir = p.pos - p.pet;
        let pr = Mat4::from_translation(p.pet) * Mat4::from_rotation_y((-dir.x).atan2(-dir.z));
        match c.shape {
            0 => {
                let pr = pr * Mat4::from_translation(v(0.0, 1.5 + bob, 0.0));
                cube(out, &pr, v(0.0, 0.0, 0.0), v(0.22, 0.26, 0.2), c.rgb, false);
                cube(out, &pr, v(0.0, 0.2, 0.0), v(0.24, 0.18, 0.2), c.rgb, false);
                for x in [-0.06, 0.06] {
                    cube(out, &pr, v(x, 0.22, -0.105), Vec3::splat(0.06), c.rgb2, true);
                    part(out, &pr, v(x * 2.0, 0.08, 0.0), Quat::from_rotation_z(x.signum() * (0.6 + (p.time * 12.0).sin() * 0.6)), v(x.signum() * 0.12, 0.0, 0.0), v(0.24, 0.04, 0.16), c.rgb, false);
                }
            }
            1 => {
                let pr = pr * Mat4::from_translation(v(0.0, 1.6 + bob * 2.0, 0.0));
                part(out, &pr, Vec3::ZERO, Quat::from_euler(glam::EulerRot::YXZ, p.time * 1.3, p.time, 0.0), Vec3::ZERO, Vec3::splat(0.2), c.rgb, true);
                part(out, &pr, Vec3::ZERO, Quat::from_euler(glam::EulerRot::YXZ, -p.time, 0.7, p.time * 0.8), Vec3::ZERO, Vec3::splat(0.14), c.rgb2, true);
                for k in 0..3 {
                    let a = p.time * 2.0 + k as f32 * 2.1;
                    cube(out, &pr, v(a.cos() * 0.26, (a * 1.3).sin() * 0.08, a.sin() * 0.26), Vec3::splat(0.04), c.rgb2, true);
                }
            }
            2 => {
                let trot = (p.walk * 1.5).sin() * 0.6 * p.moving;
                cube(out, &pr, v(0.0, 0.26, 0.0), v(0.18, 0.18, 0.44), c.rgb, false);
                cube(out, &pr, v(0.0, 0.36, -0.28), v(0.2, 0.18, 0.2), c.rgb, false);
                cube(out, &pr, v(0.0, 0.32, -0.4), v(0.1, 0.08, 0.1), c.rgb2, false);
                for x in [-0.07, 0.07] {
                    cube(out, &pr, v(x, 0.49, -0.26), v(0.06, 0.1, 0.05), c.rgb, false);
                    part(out, &pr, v(x, 0.18, -0.15), Quat::from_rotation_x(trot * x.signum()), v(0.0, -0.09, 0.0), v(0.06, 0.18, 0.06), c.rgb, false);
                    part(out, &pr, v(x, 0.18, 0.16), Quat::from_rotation_x(-trot * x.signum()), v(0.0, -0.09, 0.0), v(0.06, 0.18, 0.06), c.rgb, false);
                }
                part(out, &pr, v(0.0, 0.3, 0.22), Quat::from_rotation_x(-0.7 + (p.time * 5.0).sin() * 0.15), v(0.0, 0.0, 0.16), v(0.1, 0.1, 0.32), c.rgb, false);
                part(out, &pr, v(0.0, 0.3, 0.22), Quat::from_rotation_x(-0.7 + (p.time * 5.0).sin() * 0.15), v(0.0, 0.0, 0.36), v(0.11, 0.11, 0.1), c.rgb2, false);
            }
            _ => {
                let s = 1.0 + (p.time * 4.0).sin() * 0.18;
                cube(out, &pr, v(0.0, 0.14 * s, 0.0), v(0.34 / s, 0.28 * s, 0.34 / s), c.rgb, false);
                cube(out, &pr, v(0.0, 0.3 * s, 0.0), v(0.2 / s, 0.14 * s, 0.2 / s), c.rgb2, false);
                for x in [-0.06, 0.06] {
                    cube(out, &pr, v(x, 0.2 * s, -0.17 / s), Vec3::splat(0.05), [245, 245, 255], true);
                }
            }
        }
    }
    sparkles(out, base + Vec3::Y * 1.1, &p.gear, p.time);
    // Zitadellen-Set: goldene Glüh-Würfelchen steigen spiralförmig auf
    let mut g = [0u16; 7];
    g.copy_from_slice(&p.gear[..7]);
    if stats::full_set(&g) == Some(stats::SET_ZITADELLE) {
        for k in 0..10 {
            let ph = (p.time * 0.5 + k as f32 * 0.1).fract();
            let a = k as f32 * 0.63 + p.time * 1.2;
            world_box(out, base + v(a.cos() * 0.5, ph * 2.1, a.sin() * 0.5), Vec3::splat(0.03 + 0.05 * (1.0 - ph)), GOLD, true);
        }
    }
    tint(out, start, p.hurt);
}

/// Erkennungszeichen der NPCs: Hut, Werkzeug, Schürze – je Rolle anders.
pub fn npc_extra(out: &mut Vec<Inst>, id: u8, pos: Vec3, yaw: f32, time: f32) {
    let root = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
    let r = &root;
    let top = 1.9;
    let wood = [120, 86, 54];
    let metal = [168, 172, 182];
    let apron = |out: &mut Vec<Inst>, col: [u8; 3]| cube(out, r, v(0.0, 0.95, -0.16), v(0.42, 0.62, 0.02), col, false);
    match id {
        1 => {
            // Doktorhut mit Quaste
            cube(out, r, v(0.0, top, 0.0), v(0.4, 0.1, 0.4), [30, 30, 36], false);
            cube(out, r, v(0.0, top + 0.07, 0.0), v(0.62, 0.03, 0.62), [30, 30, 36], false);
            part(out, r, v(0.24, top + 0.08, -0.24), Quat::from_rotation_z((time * 1.5).sin() * 0.15), v(0.0, -0.1, 0.0), v(0.03, 0.2, 0.03), [230, 190, 70], false);
        }
        2 | 8 => {
            // Zylinder; der Bankier trägt ein Monokel, der Auktionator einen Hammer
            cube(out, r, v(0.0, top - 0.02, 0.0), v(0.56, 0.03, 0.56), [36, 32, 40], false);
            cube(out, r, v(0.0, top + 0.16, 0.0), v(0.36, 0.34, 0.36), [36, 32, 40], false);
            cube(out, r, v(0.0, top + 0.04, 0.0), v(0.37, 0.06, 0.37), if id == 2 { [228, 182, 62] } else { [170, 40, 50] }, false);
            if id == 2 {
                cube(out, r, v(0.1, 1.64, -0.226), v(0.12, 0.12, 0.01), [228, 182, 62], true);
            } else {
                let q = Quat::from_rotation_x(-0.6 + (time * 2.0).sin().max(0.0) * 0.5);
                part(out, r, v(0.33, 0.95, -0.05), q, v(0.0, 0.0, -0.15), v(0.04, 0.04, 0.3), wood, false);
                part(out, r, v(0.33, 0.95, -0.05), q, v(0.0, 0.0, -0.3), v(0.16, 0.09, 0.09), [90, 60, 40], false);
            }
        }
        3 => {
            // Bibliothekarin: Buch vor der Brust, Brille
            cube(out, r, v(0.0, 1.05, -0.2), v(0.3, 0.36, 0.08), [120, 40, 40], false);
            cube(out, r, v(0.155, 1.05, -0.2), v(0.02, 0.32, 0.07), [240, 232, 210], false);
            cube(out, r, v(0.0, 1.64, -0.228), v(0.32, 0.025, 0.01), [40, 40, 46], false);
        }
        4 => {
            // Minion-Meisterin: Schutzbrille auf der Stirn, Schürze
            cube(out, r, v(0.0, 1.76, -0.215), v(0.44, 0.07, 0.03), [60, 60, 70], false);
            for x in [-0.1f32, 0.1] {
                cube(out, r, v(x, 1.76, -0.233), v(0.1, 0.08, 0.01), [120, 220, 255], true);
            }
            apron(out, [110, 80, 50]);
        }
        5 => {
            // Händler: breiter Hut, grüne Schürze
            cube(out, r, v(0.0, top - 0.02, 0.0), v(0.72, 0.03, 0.72), [150, 110, 60], false);
            cube(out, r, v(0.0, top + 0.07, 0.0), v(0.4, 0.16, 0.4), [150, 110, 60], false);
            apron(out, [60, 120, 70]);
        }
        6 => {
            // Torwächterin: Helm mit Kamm, Speer
            cube(out, r, v(0.0, 1.8, 0.0), v(0.47, 0.22, 0.47), metal, false);
            cube(out, r, v(0.0, top + 0.06, 0.0), v(0.06, 0.14, 0.38), [180, 40, 40], false);
            cube(out, r, v(0.42, 1.1, -0.1), v(0.04, 2.0, 0.04), wood, false);
            cube(out, r, v(0.42, 2.15, -0.1), v(0.08, 0.18, 0.08), metal, false);
        }
        7 => {
            // Schmied: Lederschürze, Hammer in der Hand
            apron(out, [90, 60, 40]);
            cube(out, r, v(0.4, 0.75, -0.05), v(0.04, 0.36, 0.04), wood, false);
            cube(out, r, v(0.4, 0.95, -0.05), v(0.16, 0.1, 0.1), [70, 70, 76], false);
        }
        9 => {
            // Förster: grüne Mütze, Axt auf dem Rücken
            cube(out, r, v(0.0, top - 0.03, 0.0), v(0.46, 0.12, 0.46), [60, 110, 60], false);
            cube(out, r, v(0.0, top - 0.06, -0.27), v(0.4, 0.03, 0.14), [50, 92, 50], false);
            cube(out, r, v(0.0, 1.1, 0.17), v(0.04, 0.7, 0.04), wood, false);
            cube(out, r, v(0.08, 1.38, 0.17), v(0.14, 0.16, 0.03), metal, false);
        }
        10 => {
            // Gärtnerin: Strohhut mit Blumen
            cube(out, r, v(0.0, top - 0.02, 0.0), v(0.68, 0.03, 0.68), [226, 196, 120], false);
            cube(out, r, v(0.0, top + 0.06, 0.0), v(0.38, 0.14, 0.38), [226, 196, 120], false);
            for (k, col) in [[230u8, 90, 120], [250, 220, 90], [150, 110, 230]].into_iter().enumerate() {
                cube(out, r, v(0.2, top + 0.04, -0.1 + k as f32 * 0.1), Vec3::splat(0.06), col, false);
            }
        }
        _ => {}
    }
}

/// Lern-Minion: kleiner Steingolem, wippt und dreht sich; je Stufe ein Leuchtpunkt in der Krone.
pub fn minion(out: &mut Vec<Inst>, p: Vec3, col: [u8; 3], tier: u8, time: f32) {
    let bob = (time * 2.2 + p.x).sin() * 0.06;
    let root = Mat4::from_translation(p + Vec3::Y * (0.32 + bob)) * Mat4::from_rotation_y(time * 0.6 + p.z);
    push(out, root * Mat4::from_scale(Vec3::new(0.42, 0.44, 0.34)), col, false);
    push(out, root * Mat4::from_translation(Vec3::new(0.0, 0.38, 0.0)) * Mat4::from_scale(Vec3::splat(0.32)), col, false);
    for sx in [-0.08f32, 0.08] {
        push(out, root * Mat4::from_translation(Vec3::new(sx, 0.4, -0.17)) * Mat4::from_scale(Vec3::splat(0.05)), [120, 220, 255], true);
    }
    for t in 0..tier.min(11) {
        let a = t as f32 / tier as f32 * PI * 2.0 + time;
        push(out, root * Mat4::from_translation(Vec3::new(a.cos() * 0.2, 0.62, a.sin() * 0.2)) * Mat4::from_scale(Vec3::splat(0.04)), [255, 214, 120], true);
    }
}

#[derive(Clone, Copy, Default)]
pub struct Particle {
    p: Vec3,
    v: Vec3,
    life: f32,
    max: f32,
    size: f32,
    rgb: [u8; 3],
    glow: bool,
    grav: f32,
}

/// Fester Ring aus Partikeln: nie wächst etwas, das älteste wird überschrieben.
pub struct Particles {
    v: Vec<Particle>,
    next: usize,
    seed: u32,
}

impl Particles {
    pub fn new() -> Particles {
        Particles { v: vec![Particle::default(); 1024], next: 0, seed: 0x1234_5678 }
    }
    pub fn rnd(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed & 0xFFFF) as f32 / 65535.0
    }
    fn dir(&mut self, spread: f32, up: f32) -> Vec3 {
        v((self.rnd() - 0.5) * spread, self.rnd() * up, (self.rnd() - 0.5) * spread)
    }
    fn spawn(&mut self, p: Vec3, vel: Vec3, life: f32, size: f32, rgb: [u8; 3], glow: bool, grav: f32) {
        self.v[self.next] = Particle { p, v: vel, life, max: life, size, rgb, glow, grav };
        self.next = (self.next + 1) % self.v.len();
    }
    pub fn update(&mut self, dt: f32) {
        for q in &mut self.v {
            if q.life > 0.0 {
                q.life -= dt;
                q.v.y -= q.grav * dt;
                q.p += q.v * dt;
            }
        }
    }
    pub fn draw(&self, out: &mut Vec<Inst>) {
        for q in &self.v {
            if q.life > 0.0 {
                let s = q.size * (q.life / q.max).min(1.0);
                push(out, Mat4::from_translation(q.p) * Mat4::from_rotation_y(q.life * 6.0) * Mat4::from_scale(Vec3::splat(s)), q.rgb, q.glow);
            }
        }
    }
    /// Bau-Effekt am gesetzten Block: 0 Konfetti, 1 Kreidestaub, 2 Sternenstaub, 3 Glocke, sonst ein kleiner Puff in Blockfarbe.
    pub fn burst(&mut self, kind: u8, at: Vec3, c1: [u8; 3], c2: [u8; 3]) {
        let n = match kind { 0 => 26, 1 => 16, 2 => 20, 3 => 12, _ => 8 };
        for k in 0..n {
            let d = self.dir(1.0, 1.0);
            let col = match kind {
                0 => PALETTE[2 + (k * 5) % 8],
                _ => if k % 2 == 0 { c1 } else { c2 },
            };
            match kind {
                0 => self.spawn(at + d * 0.4, d * v(4.0, 6.0, 4.0), 1.2, 0.09, col, false, 9.0),
                1 => self.spawn(at + d * 0.5, d * v(1.2, 0.8, 1.2), 0.9, 0.16, col, false, -0.6),
                2 => self.spawn(at + d * 0.6, d * v(1.5, 2.5, 1.5), 1.4, 0.07, col, true, 1.0),
                3 => self.spawn(at + d * 0.3, v(d.x * 2.0, 2.0 + d.y, d.z * 2.0), 1.0, 0.08, col, true, 3.0),
                _ => self.spawn(at + d * 0.5, d * v(2.0, 3.0, 2.0), 0.5, 0.1, col, false, 9.0),
            }
        }
    }
    /// Splitter beim Abbauen: fliegen von der getroffenen Seite `n` weg.
    pub fn chips(&mut self, at: Vec3, n: Vec3, rgb: [u8; 3], k: usize) {
        for _ in 0..k {
            let d = self.dir(1.0, 1.0);
            let shade = 0.7 + self.rnd() * 0.4;
            let col = rgb.map(|c| (c as f32 * shade).min(255.0) as u8);
            let j = v(d.x, d.y - 0.5, d.z) * 0.8 * (Vec3::ONE - n.abs());
            self.spawn(at + n * 0.05 + j, n * 2.0 + v(d.x * 2.0, 1.5 + d.y * 1.5, d.z * 2.0), 0.55, 0.07, col, false, 14.0);
        }
    }
    /// Spur beim Laufen: 0 Flamme (Stufe = mehr), 1 Funken, 2 Blätter, 3 Tinte.
    pub fn trail(&mut self, c: &items::Cosmetic, at: Vec3) {
        let tier = if c.shape == 0 { 1 + (c.id - items::TRAIL_FLAMME_1) as usize } else { 1 };
        for k in 0..tier {
            let d = self.dir(0.5, 1.0);
            let col = if k % 2 == 0 { c.rgb } else { c.rgb2 };
            match c.shape {
                0 => self.spawn(at + d * v(1.0, 0.2, 1.0), v(d.x, 1.2 + d.y, d.z), 0.6 + 0.2 * tier as f32, 0.14, col, true, -1.0),
                1 => self.spawn(at + d * v(1.0, 0.6, 1.0), d * v(3.0, 2.5, 3.0), 0.5, 0.05, col, true, 6.0),
                2 => self.spawn(at + v(d.x, 0.9 + d.y * 0.4, d.z), v(d.x, -0.3, d.z), 1.4, 0.1, col, false, 0.8),
                _ => self.spawn(at + v(d.x * 0.6, 0.03, d.z * 0.6), Vec3::ZERO, 1.6, 0.16, col, false, 0.0),
            }
        }
    }
}
