//! Kampf- und Insel-Deko (Phasen 16–20): Wesen, Lebensbalken, Pflanzen, Beute, Ständer, Stoßwellen, Geschosse, Waffen.
//! Reine Zeichenfunktionen aus Quadern – keine Spiellogik, keine Allokation (alles landet in `out`, `push` achtet auf MAX_INST).
use crate::avatar::{armor, cube, mix, part, push, sparkles, tint, v};
use crate::gfx::Inst;
use campus_shared::proto::{MOB_SLAM, MOB_WINDUP};
use campus_shared::{items, rpg, skyblock as sb, world};
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::PI;

const EYE: [u8; 3] = [245, 245, 245];
const PUPIL: [u8; 3] = [20, 20, 28];
const METAL: [u8; 3] = [170, 175, 188];
const WOOD: [u8; 3] = [120, 86, 54];
const GOLD: [u8; 3] = [255, 210, 80];
const SPROUT: [u8; 3] = [96, 170, 70];
/// Stellen der Halme/Büschel im Block (relativ zur Mitte)
const SPOTS: [(f32, f32); 5] = [(-0.25, -0.2), (0.2, -0.25), (0.0, 0.05), (-0.2, 0.25), (0.25, 0.2)];

fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(v(x, y, z))
}
fn s(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_scale(v(x, y, z))
}

/// Zwei Augen auf der Vorderseite (vorn ist -Z): Weiß mit Pupille.
fn eyes(out: &mut Vec<Inst>, r: &Mat4, y: f32, z: f32, dx: f32, sz: f32) {
    for x in [-dx, dx] {
        cube(out, r, v(x, y, z), v(sz, sz * 1.2, 0.02), EYE, false);
        cube(out, r, v(x, y - sz * 0.15, z - 0.012), v(sz * 0.5, sz * 0.6, 0.02), PUPIL, false);
    }
}

/// Wesen der Zitadelle. `pos` = Füße, vorn ist -Z wie bei der Spielerfigur, Maßstab `rpg::mob(kind).size`.
pub fn mob(out: &mut Vec<Inst>, kind: u8, pos: Vec3, yaw: f32, walk: f32, hurt: f32, fl: u8, time: f32) {
    let Some(d) = rpg::mob(kind) else { return };
    let start = out.len();
    let root = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw) * Mat4::from_scale(Vec3::splat(d.size));
    let sw = walk.sin() * 0.7;
    match kind {
        0 => slime(out, &root, d.rgb, d.rgb2, walk, time),
        1 => golem(out, &root, d.rgb, d.rgb2, sw, time),
        // 5 Katakomben-Wächter, 7 Namenloser Prüfer: Gestalt der Zitadelle, Farben aus der eigenen Definition
        2 | 5 => guard(out, &root, d.rgb, d.rgb2, sw, fl & MOB_WINDUP != 0, time),
        3 | 7 => archon(out, &root, d.rgb, d.rgb2, fl & MOB_SLAM != 0, time),
        6 => scholar(out, &root, d.rgb, d.rgb2, time),
        _ => servant(out, &root, d.rgb, d.rgb2, sw, time),
    }
    tint(out, start, hurt);
}

/// Chaos-Schleim: hüpfender, gestauchter Gallertwürfel mit durchscheinendem Kern und Glanzlicht.
fn slime(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], walk: f32, time: f32) {
    let hop = (walk * 0.5 + time * 3.0).sin().abs();
    let k = (1.0 - hop).powi(3) * 0.3;
    let m = *r * t(0.0, hop * 0.45, 0.0) * s(1.0 + k * 0.8, 1.0 - k, 1.0 + k * 0.8);
    let m = &m;
    cube(out, m, v(0.0, 0.4, 0.0), v(0.86, 0.8, 0.86), mix(c, [255, 255, 255], 0.2), false);
    // Kern schimmert an den Seiten durch
    for (x, z, w, dp) in [(0.0f32, 0.432f32, 0.44f32, 0.02f32), (-0.432, 0.0, 0.02, 0.44), (0.432, 0.0, 0.02, 0.44)] {
        cube(out, m, v(x, 0.32, z), v(w, 0.34, dp), c2, false);
    }
    cube(out, m, v(-0.26, 0.72, -0.3), v(0.14, 0.06, 0.12), [235, 255, 235], true); // Glanzlicht
    eyes(out, m, 0.52, -0.432, 0.16, 0.14);
    cube(out, m, v(0.0, 0.3, -0.432), v(0.18, 0.05, 0.02), mix(c2, [0, 0, 0], 0.5), false); // Mund
    for (x, z) in [(-0.38f32, -0.2f32), (0.36, 0.3), (0.1, -0.4)] {
        cube(out, m, v(x, 0.04, z), v(0.14, 0.08, 0.14), c2, false); // Tropfen am Boden
    }
}

/// Tinten-Golem: klobig, lange schwere Arme, glühender Augenschlitz, tropfende Tinte.
fn golem(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], sw: f32, time: f32) {
    let dark = mix(c, [0, 0, 0], 0.35);
    for (x, q) in [(-0.22f32, Quat::from_rotation_x(sw)), (0.22, Quat::from_rotation_x(-sw))] {
        part(out, r, v(x, 0.6, 0.0), q, v(0.0, -0.3, 0.0), v(0.34, 0.6, 0.38), dark, false);
        part(out, r, v(x, 0.6, 0.0), q, v(0.0, -0.55, -0.04), v(0.4, 0.12, 0.46), c, false); // Fuß
    }
    let b = v(0.0, sw.abs() * 0.04, 0.0);
    cube(out, r, b + v(0.0, 1.05, 0.0), v(0.95, 0.8, 0.62), c, false); // Rumpf
    cube(out, r, b + v(0.0, 1.5, 0.02), v(1.05, 0.2, 0.66), dark, false); // Schultern
    cube(out, r, b + v(0.0, 1.66, -0.06), v(0.44, 0.38, 0.42), c, false); // Kopf, eingesunken
    cube(out, r, b + v(0.0, 1.7, -0.28), v(0.34, 0.07, 0.02), c2, true); // Augenschlitz
    cube(out, r, b + v(0.0, 1.1, -0.315), v(0.08, 0.36, 0.02), c2, true); // Rune auf der Brust
    cube(out, r, b + v(0.0, 1.18, -0.315), v(0.3, 0.06, 0.02), c2, true);
    for k in 0..3 {
        let f = (time * 0.7 + k as f32 * 0.33).fract();
        cube(out, r, b + v(-0.3 + k as f32 * 0.3, 0.9 - f * 0.8, -0.33), Vec3::splat(0.07 * (1.0 - f * 0.5)), c2, false); // Tintentropfen
    }
    for (x, q) in [(-0.6f32, Quat::from_rotation_x(-sw * 0.8)), (0.6, Quat::from_rotation_x(sw * 0.8))] {
        part(out, r, b + v(x, 1.45, 0.0), q, v(0.0, -0.42, 0.0), v(0.32, 0.84, 0.36), c, false);
        part(out, r, b + v(x, 1.45, 0.0), q, v(0.0, -0.9, 0.0), v(0.42, 0.3, 0.42), dark, false); // Faust
    }
}

/// Klausur-Wächter: Ritter mit Schild links und Lanze rechts; beim Ausholen lehnt er sich zurück, Visier und Spitze glühen.
fn guard(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], sw: f32, windup: bool, time: f32) {
    let dark = mix(c, [0, 0, 0], 0.35);
    let (lean, crouch) = if windup { (0.3, 0.12) } else { (0.0, 0.0) };
    for (x, q) in [(-0.13f32, Quat::from_rotation_x(sw)), (0.13, Quat::from_rotation_x(-sw))] {
        part(out, r, v(x, 0.75 - crouch, 0.0), q, v(0.0, -0.36, 0.0), v(0.22, 0.75 - crouch, 0.24), c, false);
        part(out, r, v(x, 0.75 - crouch, 0.0), q, v(0.0, -0.68, -0.03), v(0.25, 0.14, 0.3), dark, false);
    }
    let body = *r * t(0.0, 0.75 - crouch, 0.0) * Mat4::from_rotation_x(lean);
    let b = &body;
    cube(out, b, v(0.0, 0.33, 0.0), v(0.56, 0.66, 0.32), c, false); // Harnisch
    cube(out, b, v(0.0, 0.22, -0.17), v(0.3, 0.6, 0.02), c2, false); // Wappenrock
    cube(out, b, v(0.0, 0.04, 0.0), v(0.58, 0.08, 0.34), dark, false); // Gürtel
    for x in [-0.32f32, 0.32] {
        cube(out, b, v(x, 0.62, 0.0), v(0.2, 0.14, 0.36), dark, false); // Schulterstücke
    }
    cube(out, b, v(0.0, 0.9, 0.0), v(0.44, 0.46, 0.44), c, false); // Helm
    let visor = if windup { mix(c2, [255, 255, 255], 0.3) } else { PUPIL };
    cube(out, b, v(0.0, 0.92, -0.225), v(0.32, 0.05, 0.02), visor, windup);
    cube(out, b, v(0.0, 1.2, 0.05), v(0.08, 0.2, 0.42), c2, false); // Helmbusch
    // Schildarm links
    let ql = Quat::from_rotation_x(0.5 + sw * 0.2);
    part(out, b, v(-0.36, 0.6, 0.0), ql, v(0.0, -0.22, 0.0), v(0.16, 0.44, 0.2), c, false);
    part(out, b, v(-0.36, 0.6, 0.0), ql, v(-0.04, -0.36, -0.16), v(0.5, 0.62, 0.07), METAL, false);
    part(out, b, v(-0.36, 0.6, 0.0), ql, v(-0.04, -0.36, -0.2), v(0.14, 0.3, 0.02), c2, false); // Wappen
    // Lanzenarm rechts: Hand berechnet, Lanze zeigt stets nach vorn
    let th: f32 = if windup { -0.3 + (time * 30.0).sin() * 0.04 } else { 0.9 };
    part(out, b, v(0.36, 0.6, 0.0), Quat::from_rotation_x(th), v(0.0, -0.22, 0.0), v(0.16, 0.44, 0.2), c, false);
    let (hy, hz) = (0.6 - 0.45 * th.cos(), -0.45 * th.sin());
    cube(out, b, v(0.42, hy, hz - 0.5), v(0.07, 0.07, 1.9), WOOD, false);
    cube(out, b, v(0.42, hy, hz - 0.2), v(0.2, 0.2, 0.1), METAL, false); // Brechscheibe
    cube(out, b, v(0.42, hy, hz - 1.5), v(0.1, 0.1, 0.22), if windup { c2 } else { METAL }, windup);
    cube(out, b, v(0.42, hy, hz - 1.66), v(0.05, 0.05, 0.12), METAL, false);
}

/// Großer Prüfungs-Archon: schwebender Magier in Robe, Krone, kreisende Ringe; beim Stampfen reißt er die Arme hoch.
fn archon(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], slam: bool, time: f32) {
    let dark = mix(c, [0, 0, 0], 0.35);
    let fm = *r * t(0.0, 0.35 + (time * 1.6).sin() * 0.08, 0.0);
    let f = &fm;
    for k in 0..4 {
        let w = 0.7 - k as f32 * 0.08;
        cube(out, f, v(0.0, 0.15 + k as f32 * 0.22, 0.0), v(w, 0.24, w * 0.7), if k == 0 { dark } else { c }, false);
    }
    cube(out, f, v(0.0, 0.04, 0.0), v(0.74, 0.04, 0.52), c2, true); // leuchtender Saum
    cube(out, f, v(0.0, 0.75, -0.25), v(0.1, 1.2, 0.02), c2, false); // Goldborte
    cube(out, f, v(0.0, 1.15, 0.0), v(0.56, 0.5, 0.34), c, false);
    cube(out, f, v(0.0, 1.62, 0.02), v(0.48, 0.46, 0.46), dark, false); // Kapuze
    cube(out, f, v(0.0, 1.58, -0.215), v(0.32, 0.3, 0.02), [20, 10, 30], false); // Schatten-Gesicht
    for x in [-0.08f32, 0.08] {
        cube(out, f, v(x, 1.6, -0.23), v(0.07, 0.05, 0.02), c2, true);
    }
    cube(out, f, v(0.0, 1.88, 0.0), v(0.42, 0.08, 0.42), c2, false); // Krone
    for k in 0..5 {
        let a = k as f32 / 5.0 * PI * 2.0;
        cube(out, f, v(a.sin() * 0.17, 1.98, -a.cos() * 0.17), v(0.07, 0.14, 0.07), c2, false);
    }
    cube(out, f, v(0.0, 1.9, -0.215), Vec3::splat(0.06), [255, 80, 120], true); // Stirnjuwel
    for sg in [-1.0f32, 1.0] {
        let q = if slam { Quat::from_rotation_z(sg * 2.7) } else { Quat::from_rotation_z(sg * 0.5) * Quat::from_rotation_x(0.6 + (time * 1.3).sin() * 0.2) };
        part(out, f, v(sg * 0.34, 1.35, 0.0), q, v(0.0, -0.25, 0.0), v(0.22, 0.5, 0.24), c, false); // weiter Ärmel
        part(out, f, v(sg * 0.34, 1.35, 0.0), q, v(0.0, -0.56, 0.0), Vec3::splat(if slam { 0.2 } else { 0.13 }), c2, true); // Leuchtkugel
    }
    for (ri, tilt) in [(0usize, 0.35f32), (1, -0.5)] {
        let dirv: f32 = if ri == 0 { 0.8 } else { -1.1 };
        let spin = time * dirv * if slam { 3.0 } else { 1.0 };
        let ring = *f * t(0.0, 1.0, 0.0) * Mat4::from_rotation_x(tilt) * Mat4::from_rotation_y(spin);
        let rad = 0.85 + ri as f32 * 0.2;
        for k in 0..12 {
            let a = k as f32 / 12.0 * PI * 2.0;
            push(out, ring * t(a.cos() * rad, 0.0, a.sin() * rad) * Mat4::from_scale(Vec3::splat(0.07)), if k % 3 == 0 { GOLD } else { c2 }, true);
        }
    }
}

/// Diener des Archons: kleiner Kapuzenwicht mit leuchtenden Augen.
fn servant(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], sw: f32, time: f32) {
    let dark = mix(c, [0, 0, 0], 0.4);
    for (x, q) in [(-0.1f32, Quat::from_rotation_x(sw)), (0.1, Quat::from_rotation_x(-sw))] {
        part(out, r, v(x, 0.45, 0.0), q, v(0.0, -0.22, 0.0), v(0.16, 0.45, 0.18), dark, false);
    }
    let b = v(0.0, sw.abs() * 0.05, 0.0);
    cube(out, r, b + v(0.0, 0.75, 0.0), v(0.44, 0.6, 0.32), c, false);
    cube(out, r, b + v(0.0, 0.62, -0.165), v(0.08, 0.34, 0.02), c2, false);
    cube(out, r, b + v(0.0, 1.25, 0.0), v(0.46, 0.42, 0.42), c, false); // Kapuze
    part(out, r, b + v(0.0, 1.46, 0.12), Quat::from_rotation_x(0.5), v(0.0, 0.06, 0.0), v(0.14, 0.18, 0.14), c, false); // Zipfel
    cube(out, r, b + v(0.0, 1.22, -0.205), v(0.32, 0.26, 0.02), [20, 10, 30], false);
    for x in [-0.07f32, 0.07] {
        cube(out, r, b + v(x, 1.24, -0.215), v(0.07, 0.06, 0.02), c2, true);
    }
    for (x, q) in [(-0.27f32, Quat::from_rotation_x(-sw)), (0.27, Quat::from_rotation_x(sw))] {
        part(out, r, b + v(x, 0.95, 0.0), q, v(0.0, -0.18, 0.0), v(0.12, 0.36, 0.14), c, false);
    }
    cube(out, r, b + v(0.0, 1.65 + (time * 3.0).sin() * 0.05, 0.0), Vec3::splat(0.07), GOLD, true); // Funke über dem Kopf
}

/// Schatten-Gelehrter (Katakomben): schwebende, schmale Robe ohne Beine, Kapuze mit glühenden Augen, leuchtendes Buch.
fn scholar(out: &mut Vec<Inst>, r: &Mat4, c: [u8; 3], c2: [u8; 3], time: f32) {
    let dark = mix(c, [0, 0, 0], 0.35);
    let fm = *r * t(0.0, 0.3 + (time * 2.0).sin() * 0.08, 0.0);
    let f = &fm;
    for k in 0..4 {
        let w = 0.18 + k as f32 * 0.08; // Robe läuft nach unten spitz zu
        cube(out, f, v(0.0, 0.1 + k as f32 * 0.22, 0.0), v(w, 0.24, w * 0.8), if k == 0 { dark } else { c }, false);
    }
    cube(out, f, v(0.0, 1.15, 0.0), v(0.42, 0.5, 0.28), c, false); // Brust
    cube(out, f, v(0.0, 1.55, 0.02), v(0.4, 0.4, 0.4), dark, false); // Kapuze
    part(out, f, v(0.0, 1.75, 0.1), Quat::from_rotation_x(0.6), v(0.0, 0.08, 0.0), v(0.16, 0.22, 0.16), dark, false); // Zipfel
    cube(out, f, v(0.0, 1.52, -0.195), v(0.28, 0.26, 0.02), [12, 8, 20], false); // Gesicht im Schatten
    for x in [-0.07f32, 0.07] {
        cube(out, f, v(x, 1.55, -0.21), v(0.06, 0.04, 0.02), c2, true); // glühende Augen
    }
    for sg in [-1.0f32, 1.0] {
        part(out, f, v(sg * 0.25, 1.3, 0.0), Quat::from_rotation_x(0.9), v(0.0, -0.22, 0.0), v(0.12, 0.4, 0.14), c, false);
    }
    cube(out, f, v(0.0, 1.05, -0.38), v(0.34, 0.06, 0.26), [60, 40, 90], false); // Buch
    cube(out, f, v(0.0, 1.09, -0.38), v(0.3, 0.02, 0.22), mix(c2, [255, 255, 255], 0.5), true); // leuchtende Seiten
    for k in 0..3 {
        let ff = (time * 0.8 + k as f32 * 0.33).fract();
        cube(out, f, v(-0.2 + k as f32 * 0.2, 0.1 - ff * 0.3, 0.1), Vec3::splat(0.08 * (1.0 - ff)), c2, true); // Schattenfetzen
    }
}

/// Lebensbalken (in Welt-X ausgerichtet), Farbe von Grün über Gelb nach Rot.
pub fn health_bar(out: &mut Vec<Inst>, top: Vec3, frac: f32, width: f32) {
    let f = frac.clamp(0.0, 1.0);
    push(out, Mat4::from_translation(top) * s(width + 0.06, 0.12, 0.12), [30, 20, 24], false);
    if f > 0.0 {
        let col = if f > 0.5 { mix([240, 200, 60], [90, 220, 90], (f - 0.5) * 2.0) } else { mix([230, 50, 50], [240, 200, 60], f * 2.0) };
        push(out, Mat4::from_translation(top + v(-width * (1.0 - f) / 2.0, 0.0, 0.0)) * s(width * f, 0.14, 0.14), col, true);
    }
}

/// Pflanze je Stufe: 0 Keim, 1 jung, 2 fast reif, 3 reif mit Frucht (`rgb2`). `base` = Mitte der Blockunterseite.
pub fn crop(out: &mut Vec<Inst>, base: Vec3, kind: usize, stage: u8, time: f32) {
    let Some(c) = rpg::crop(kind) else { return };
    let st = stage.min(3);
    let g = (st as f32 + 1.0) / 4.0;
    let sway = (time * 1.6 + base.x * 0.7 + base.z * 1.3).sin() * 0.05;
    let root = Mat4::from_translation(base) * Mat4::from_rotation_z(sway);
    let r = &root;
    let ripe = st == 3;
    let stem = if ripe { c.rgb } else { mix(SPROUT, c.rgb, g) };
    if st == 0 {
        cube(out, r, v(0.0, 0.07, 0.0), v(0.04, 0.14, 0.04), stem, false);
        for sg in [-1.0f32, 1.0] {
            part(out, r, v(0.0, 0.13, 0.0), Quat::from_rotation_z(sg * 0.5), v(sg * 0.06, 0.0, 0.0), v(0.12, 0.03, 0.07), stem, false);
        }
        return;
    }
    let glow = ripe && (kind == 2 || kind == 4);
    match kind {
        0 => {
            // Weizen: Halme, ab Stufe 2 Ähren
            for (k, &(x, z)) in SPOTS.iter().enumerate() {
                let h = 0.9 * g * (0.85 + 0.07 * (k % 3) as f32);
                cube(out, r, v(x, h / 2.0, z), v(0.05, h, 0.05), stem, false);
                if st >= 2 {
                    cube(out, r, v(x, h + 0.08, z), v(0.09, 0.18, 0.09), if ripe { c.rgb2 } else { stem }, false);
                }
            }
        }
        1 => {
            // Karotte: Blattbüschel, reif schaut die Rübe heraus
            for &(x, z) in &SPOTS[..4] {
                for sg in [-1.0f32, 1.0] {
                    part(out, r, v(x * 0.8, 0.0, z * 0.8), Quat::from_rotation_z(sg * 0.35), v(0.0, 0.22 * g, 0.0), v(0.05, 0.44 * g, 0.12), stem, false);
                }
                if st >= 2 {
                    cube(out, r, v(x * 0.8, 0.03, z * 0.8), Vec3::splat(if ripe { 0.14 } else { 0.08 }), c.rgb2, false);
                }
            }
        }
        2 => {
            // Leucht-Beere: Busch, Beeren leuchten reif
            cube(out, r, v(0.0, 0.3 * g, 0.0), v(0.6 * g + 0.1, 0.6 * g, 0.6 * g + 0.1), stem, false);
            cube(out, r, v(0.0, 0.64 * g, 0.0), v(0.4 * g, 0.16 * g, 0.4 * g), mix(stem, [255, 255, 255], 0.15), false);
            if st >= 2 {
                let berry = if ripe { c.rgb2 } else { mix(c.rgb2, stem, 0.5) };
                for k in 0..if ripe { 8 } else { 4 } {
                    let a = k as f32 * 2.4;
                    let rad = 0.33 * g + 0.06;
                    cube(out, r, v(a.cos() * rad, 0.2 + (k % 3) as f32 * 0.13 * g, a.sin() * rad), Vec3::splat(0.09), berry, glow);
                }
            }
        }
        3 => {
            // Wissens-Kürbis: Blätter am Boden, dann die Frucht
            for k in 0..3 {
                part(out, r, Vec3::ZERO, Quat::from_rotation_y(k as f32 * 2.1), v(0.0, 0.03, 0.22 * g + 0.06), v(0.26 * g + 0.06, 0.05, 0.24 * g + 0.06), stem, false);
            }
            if st >= 2 {
                let sz = if ripe { 0.46 } else { 0.22 };
                cube(out, r, v(0.0, sz * 0.4, 0.0), v(sz, sz * 0.8, sz), if ripe { c.rgb2 } else { mix(c.rgb2, stem, 0.6) }, false);
                if ripe {
                    for x in [-0.12f32, 0.12] {
                        cube(out, r, v(x, sz * 0.4, 0.0), v(0.03, sz * 0.82, sz * 1.02), mix(c.rgb2, [0, 0, 0], 0.2), false); // Rippen
                    }
                    cube(out, r, v(0.0, sz * 0.8 + 0.05, 0.0), v(0.06, 0.1, 0.06), [90, 70, 40], false);
                }
            }
        }
        _ => {
            // Kristall-Blüte: Stiel, Blätter, kristalline Blütenblätter
            let h = 0.7 * g;
            cube(out, r, v(0.0, h / 2.0, 0.0), v(0.05, h, 0.05), stem, false);
            for sg in [-1.0f32, 1.0] {
                part(out, r, v(0.0, h * 0.4, 0.0), Quat::from_rotation_z(sg * 0.6), v(sg * 0.08, 0.0, 0.0), v(0.16, 0.03, 0.08), stem, false);
            }
            if st >= 2 {
                let n = if ripe { 6 } else { 4 };
                for k in 0..n {
                    let a = k as f32 / n as f32 * PI * 2.0 + time * 0.6;
                    part(out, r, v(0.0, h, 0.0), Quat::from_rotation_y(a) * Quat::from_rotation_x(-0.6), v(0.0, 0.12, 0.0), v(0.08, 0.24, 0.03), c.rgb2, glow);
                }
                if ripe {
                    part(out, r, v(0.0, h + 0.06, 0.0), Quat::from_rotation_y(time) * Quat::from_rotation_x(0.785), Vec3::ZERO, Vec3::splat(0.12), mix(c.rgb2, [255, 255, 255], 0.4), true);
                }
            }
        }
    }
}

/// Beute-Würfelchen (Waffen als kleines Modell), Seltenheit als Funke darüber.
pub fn drop_item(out: &mut Vec<Inst>, pos: Vec3, item: u16, spin: f32, scale: f32) {
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y(spin) * Mat4::from_scale(Vec3::splat(scale));
    let rar = rpg::rarity(item);
    if rpg::is_weapon(item) {
        weapon(out, m * Mat4::from_rotation_z(0.6) * t(0.0, -0.25, 0.0) * Mat4::from_scale(Vec3::splat(0.6)), item);
    } else {
        let glow = item < 256 && world::FLAGS[item as usize] & world::F_GLOW != 0;
        push(out, m * Mat4::from_scale(Vec3::splat(0.25)), item_rgb(item), glow);
    }
    if rar > 0 {
        let col = [[255, 255, 255], [90, 150, 255], [190, 110, 255], GOLD][rar.min(3) as usize];
        push(out, m * t(0.0, 0.24 + (spin * 2.0).sin() * 0.03, 0.0) * Mat4::from_scale(Vec3::splat(0.05)), col, true);
    }
}

/// Rüstungsständer: Holzgestell mit angelegter Rüstung (`items[0..4]`), Talismane kreisen. `pos` = Mitte der Blockunterseite.
pub fn stand(out: &mut Vec<Inst>, pos: Vec3, items: &[u16; 7], time: f32) {
    let root = Mat4::from_translation(pos);
    let r = &root;
    cube(out, r, v(0.0, 0.04, 0.0), v(0.8, 0.08, 0.5), mix(WOOD, [0, 0, 0], 0.25), false);
    cube(out, r, v(0.0, 0.95, 0.0), v(0.08, 1.75, 0.08), WOOD, false);
    cube(out, r, v(0.0, 1.38, 0.0), v(0.66, 0.08, 0.1), WOOD, false);
    cube(out, r, v(0.0, 0.8, 0.0), v(0.4, 0.06, 0.08), WOOD, false);
    cube(out, r, v(0.0, 1.62, 0.0), Vec3::splat(0.2), WOOD, false);
    armor(out, r, &items[..], [Quat::IDENTITY; 2], [Quat::IDENTITY; 2], false);
    sparkles(out, pos + Vec3::Y * 1.1, &items[..], time);
}

/// Stoßwelle des Archons: Ring aus glühenden Segmenten mit Radius `r` am Boden.
pub fn shock_ring(out: &mut Vec<Inst>, at: Vec3, r: f32) {
    const N: usize = 32;
    let len = 2.0 * PI * r.max(0.1) / N as f32 * 1.1;
    for k in 0..N {
        let a = k as f32 / N as f32 * PI * 2.0;
        let m = Mat4::from_translation(at + v(a.cos() * r, 0.15, a.sin() * r)) * Mat4::from_rotation_y(-a);
        push(out, m * s(0.15, 0.3, len), if k % 2 == 0 { [255, 150, 60] } else { GOLD }, true);
    }
}

/// Geschoss im Flug: `t` 0..1 von `from` nach `to`. kind 1 = Zauber (Stab), 2 = Schneeball (Duell, harmlos), sonst Pfeil (Bogen).
pub fn shot(out: &mut Vec<Inst>, from: Vec3, to: Vec3, t_: f32, kind: u8) {
    let d = to - from;
    let len = d.length();
    if len < 1e-3 {
        return;
    }
    let dir = d / len;
    let p = from + d * t_.clamp(0.0, 1.0);
    let m = Mat4::from_translation(p) * Mat4::from_quat(Quat::from_rotation_arc(Vec3::NEG_Z, dir));
    if kind == 1 {
        push(out, m * Mat4::from_rotation_z(t_ * 20.0) * Mat4::from_scale(Vec3::splat(0.22)), [200, 120, 255], true);
        for k in 1..5 {
            push(out, Mat4::from_translation(p - dir * k as f32 * 0.25) * Mat4::from_scale(Vec3::splat(0.14 - k as f32 * 0.025)), [150, 80, 230], true);
        }
    } else if kind == 2 {
        // leichter Bogen nach oben, damit er wie geworfen aussieht
        let p = p + Vec3::Y * (t_.clamp(0.0, 1.0) * PI).sin() * len * 0.08;
        push(out, Mat4::from_translation(p) * Mat4::from_rotation_y(t_ * 9.0) * Mat4::from_rotation_x(t_ * 7.0) * Mat4::from_scale(Vec3::splat(0.24)), [250, 252, 255], false);
        for k in 1..4 {
            push(out, Mat4::from_translation(p - dir * k as f32 * 0.22) * Mat4::from_scale(Vec3::splat(0.1 - k as f32 * 0.02)), [225, 235, 250], false);
        }
    } else {
        push(out, m * s(0.04, 0.04, 0.7), WOOD, false);
        push(out, m * t(0.0, 0.0, -0.38) * s(0.08, 0.08, 0.12), METAL, false);
        push(out, m * t(0.0, 0.0, 0.3) * s(0.16, 0.02, 0.14), EYE, false);
        push(out, m * t(0.0, 0.0, 0.3) * s(0.02, 0.16, 0.14), EYE, false);
        push(out, Mat4::from_translation(p - dir * 0.5) * Mat4::from_scale(Vec3::splat(0.06)), [120, 220, 200], true);
    }
}

/// Waffe für die Hand: Ursprung = Griff, Klinge/Stab entlang +Y, Bogensehne bei +Z.
pub fn weapon(out: &mut Vec<Inst>, m: Mat4, item: u16) {
    let Some(e) = rpg::equip(item) else { return };
    let c = e.rgb;
    match item {
        rpg::FOKUS_BOGEN => {
            push(out, m * s(0.06, 0.2, 0.07), WOOD, false);
            for sg in [-1.0f32, 1.0] {
                push(out, m * t(0.0, sg * 0.1, 0.0) * Mat4::from_rotation_x(sg * 0.35) * t(0.0, sg * 0.22, 0.0) * s(0.05, 0.44, 0.05), c, false);
            }
            push(out, m * t(0.0, 0.0, 0.15) * s(0.012, 1.0, 0.012), [235, 235, 235], false); // Sehne
        }
        rpg::ARKANER_STAB => {
            push(out, m * t(0.0, 0.25, 0.0) * s(0.06, 1.1, 0.06), [70, 50, 90], false);
            push(out, m * t(0.0, 0.86, 0.0) * s(0.14, 0.1, 0.14), GOLD, false);
            for sg in [-1.0f32, 1.0] {
                push(out, m * t(sg * 0.08, 0.95, 0.0) * s(0.03, 0.16, 0.03), GOLD, false);
            }
            push(out, m * t(0.0, 1.02, 0.0) * Mat4::from_rotation_y(0.785) * Mat4::from_scale(Vec3::splat(0.16)), c, true);
        }
        _ => {
            let (wood, legend) = (item == rpg::HOLZ_KLINGE, e.rarity >= 3);
            let fit = if legend { GOLD } else { METAL };
            push(out, m * t(0.0, -0.08, 0.0) * s(0.06, 0.2, 0.06), if wood { WOOD } else { [60, 44, 36] }, false); // Griff
            push(out, m * t(0.0, -0.2, 0.0) * s(0.08, 0.06, 0.08), fit, false); // Knauf
            push(out, m * t(0.0, 0.04, 0.0) * s(0.28, 0.05, 0.08), fit, false); // Parierstange
            push(out, m * t(0.0, 0.42, 0.0) * s(0.09, 0.72, 0.03), c, false); // Klinge
            push(out, m * t(0.0, 0.82, 0.0) * s(0.05, 0.1, 0.03), c, false); // Spitze
            if !wood {
                push(out, m * t(0.0, 0.4, 0.0) * s(0.025, 0.6, 0.035), mix(c, [255, 255, 255], 0.5), legend); // Hohlkehle
            }
        }
    }
}

/// Grundfarbe eines Gegenstands (Blöcke, Ausrüstung, Werkzeuge, Ernte, Beute, Cosmetics).
pub fn item_rgb(item: u16) -> [u8; 3] {
    if item < 256 {
        return world::block(item as u8).map_or([150, 150, 160], |b| b.rgb);
    }
    if let Some(e) = rpg::equip(item) {
        return e.rgb;
    }
    if let Some(tl) = sb::tool(item) {
        return tl.rgb;
    }
    if let Some(c) = rpg::crop(item.wrapping_sub(rpg::ERNTE) as usize).filter(|c| c.product == item) {
        return c.rgb2;
    }
    if let Some(c) = items::cosmetic(item) {
        return c.rgb;
    }
    match item {
        rpg::SCHLEIM => [110, 220, 90],
        rpg::TINTE => [40, 50, 110],
        rpg::WAECHTER_SIEGEL => [200, 40, 60],
        rpg::ZITADELLEN_FRAGMENT => GOLD,
        rpg::VERZ_BUCH => [180, 120, 255],
        rpg::ARCHON_KERN => [255, 120, 200],
        _ => [200, 200, 210],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn katakomben_wesen_und_schneeball() {
        for kind in 5..=7u8 {
            let mut out = Vec::new();
            mob(&mut out, kind, Vec3::new(5.0, 9.0, 5.0), 0.3, 1.0, 0.0, 0, 2.0);
            assert!(out.len() > 10, "Wesen {kind}");
        }
        let mut out = Vec::new();
        shot(&mut out, Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.5, 2);
        assert!(!out.is_empty() && out.iter().all(|i| i.col[0] > 200 && i.col[3] == 0), "weiß, leuchtet nicht");
    }
}
