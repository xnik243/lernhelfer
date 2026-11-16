//! Welt-Mathematik: Stücke 16×16×64, Block-Register, schwebende Inseln (Kegel-Unterseite), Bauplan-Stempel.
use serde::{Deserialize, Serialize};

pub const CS: i32 = 16;
pub const CH: i32 = 64;
pub const WC: i32 = 60; // Stücke je Achse (Eckpunkt-Packung in mesh.rs erlaubt höchstens 64)
pub const WORLD: i32 = WC * CS; // 960 Blöcke
pub const VOL: usize = 16 * 16 * 64;
pub const GROUND: i32 = 30; // Oberfläche der Privatinseln und Satelliten; darunter hängt der Felskegel
pub const SLOT: i32 = 96; // Raster der Privatinseln
pub const SLOTS: i32 = WORLD / SLOT; // 10
/// Plätze (i, j) mit i < HUB_SLOTS und j < HUB_SLOTS gehören dem Hub-Archipel (Blöcke 0..HUB_EDGE).
pub const HUB_SLOTS: i32 = 5;
pub const HUB_EDGE: i32 = HUB_SLOTS * SLOT; // 480
pub const MID: i32 = 240; // Mitte der Hub-Hauptinsel (x und z)
/// Hub-Hauptinsel 180×180: x0, z0, x1, z1 (x1/z1 exklusiv).
pub const HUB_MAIN: [i32; 4] = [150, 150, 330, 330];
/// Darunter liegt auf der Hauptinsel das Bergwerk (Kristallminen).
pub const MINE_Y: i32 = 24;
/// Unter dieser Höhe gilt man als ins Leere gefallen.
pub const VOID_Y: f32 = -25.0;
pub const VOID_LOSS_PCT: i64 = 25; // so viel der ungesicherten Taler (nicht Bank) gehen verloren
/// Nachwachsende Blöcke stehen so lange als ABGEBAUT da.
pub const REGEN_MS: i64 = 2500;
/// Schwerkraft im Browser (Blöcke/s²), auch für die Sprungpolster-Bahn.
pub const GRAVITY: f32 = 28.0;

/// Schwebende Satelliten-Insel des Hubs.
pub struct Isle {
    pub name: &'static str,
    pub x0: i32,
    pub z0: i32,
    pub size: i32,
    pub y: i32, // Oberfläche
}
pub const SATS: [Isle; 3] = [
    Isle { name: "Bibliotheks-Turm", x0: 370, z0: 218, size: 45, y: GROUND },
    Isle { name: "Prüfungs-Zitadelle", x0: 215, z0: 60, size: 50, y: GROUND },
    Isle { name: "Himmels-Garten", x0: 70, z0: 220, size: 40, y: GROUND },
];
/// Viertel der Hauptinsel (x0, z0, x1, z1), der erste Treffer zählt.
pub const DISTRICTS: [(&str, [i32; 4]); 6] = [
    ("Himmelsruinen", [150, 150, 205, 192]),
    ("Spawn-Zitadelle", [200, 255, 280, 330]),
    ("Marktplatz", [210, 200, 270, 255]),
    ("Schmiedeviertel", [150, 192, 210, 330]),
    ("Flüsterwald", [270, 150, 330, 330]),
    ("Kristallminen", [205, 150, 270, 200]),
];

/// Warp-Ziele (Insel-Kompass, /hub, Portale): Name, Standpunkt-Vorschlag (der Server sucht mit find_safe_spawn
/// in dieser Spalte den sicheren Boden), Blickrichtung yaw (0 = Norden/-z, PI/2 = Westen, -PI/2 = Osten, PI = Süden).
/// Die Reihenfolge 0..=4 ist fest (Quests nutzen Goal::Warp(2)).
pub const SPOTS: [(&str, [f32; 3], f32); 8] = [
    ("Campus-Hub", [240.5, 33.05, 296.5], 0.0),
    ("Bazaar", [240.5, 29.05, 247.5], 0.0),
    ("Bibliotheks-Turm", [378.5, 31.05, 240.5], -std::f32::consts::FRAC_PI_2),
    ("Minion-Werkstatt", [184.5, 27.05, 232.5], std::f32::consts::FRAC_PI_2),
    ("Prüfungs-Zitadelle", [240.5, 31.05, 104.5], 0.0),
    ("Himmels-Garten", [102.5, 31.05, 240.5], std::f32::consts::FRAC_PI_2),
    ("Himmelsruinen", [192.5, 36.05, 184.5], std::f32::consts::FRAC_PI_4),
    ("Kristallminen", [240.5, 29.05, 172.5], 0.0),
];
/// Blickrichtung bei Ankunft auf einer Privatinsel (zur Starthütte).
pub const PLOT_YAW: f32 = 0.0;
/// Portal-Ziel „eigene Insel“ (sonst Index in SPOTS).
pub const PORTAL_HOME: u8 = 255;
/// Portale im Hub-Archipel: Mitte (x, z) und Ziel. Auf Privatinseln führt jedes Portal zum Hub (SPOTS[0]).
pub const PORTALS: [(i32, i32, u8); 7] = [
    (231, 296, PORTAL_HOME), // Spawn-Zitadelle → eigene Insel
    (326, 235, 2),           // Ostrand → Bibliotheks-Turm (Mitte des Torbogens, blueprints::arch)
    (235, 154, 4),           // Nordrand → Prüfungs-Zitadelle
    (154, 235, 5),           // Westrand → Himmels-Garten
    (374, 235, 0),           // Satelliten zurück zum Hub
    (235, 106, 0),
    (106, 235, 0),
];
/// Sprungpolster (Mitte x, z) in den Himmelsruinen und ihr Landepunkt.
pub const LAUNCH_PADS: [(i32, i32, [f32; 3]); 3] = [
    (160, 186, [102.5, 31.0, 240.5]), // → Himmels-Garten
    (176, 156, [240.5, 31.0, 100.5]), // → Prüfungs-Zitadelle
    (196, 168, [378.5, 31.0, 240.5]), // → Bibliotheks-Turm
];
/// Minion-Werkstatt: Sockel (SOCKEL-Block) für Stufe I steht hier, Stufe k bei x + 2·(k-1); der Browser zeichnet die Figur darauf.
pub const SHOWCASE: [i32; 3] = [168, 27, 244];
pub const LAUNCH_SPEED: f32 = 30.0; // waagrecht, Blöcke/s
pub const LAUNCH_MAX_SPEED: f32 = 60.0; // Tempo-Grenze des Servers während eines Sprungs

pub fn in_hub(x: i32, z: i32) -> bool {
    x >= 0 && z >= 0 && x < HUB_EDGE && z < HUB_EDGE
}
/// Platz der Privatinsel an dieser Stelle (None im Hub-Archipel oder außerhalb der Welt).
pub fn slot_at(x: i32, z: i32) -> Option<(u8, u8)> {
    ((0..WORLD).contains(&x) && (0..WORLD).contains(&z) && !in_hub(x, z)).then(|| ((x / SLOT) as u8, (z / SLOT) as u8))
}
/// Name der Zone für die Anzeigetafel.
pub fn zone_name(x: i32, y: i32, z: i32) -> &'static str {
    // Instanz-Felder (Katakomben, Arena) liegen in der Leere im Nordosten des Hub-Archipels
    if let Some(k) = crate::instance::kind_at(x, z) {
        return crate::instance::ZONE[k as usize];
    }
    if !in_hub(x, z) {
        return if slot_at(x, z).is_some() { "Privatinsel" } else { "Leere" };
    }
    if let Some(s) = SATS.iter().find(|s| x >= s.x0 - 4 && z >= s.z0 - 4 && x < s.x0 + s.size + 4 && z < s.z0 + s.size + 4) {
        return s.name;
    }
    let [x0, z0, x1, z1] = HUB_MAIN;
    if !(x >= x0 && z >= z0 && x < x1 && z < z1) {
        return "Himmelsbrücken";
    }
    if y < MINE_Y {
        return "Kristallminen";
    }
    DISTRICTS.iter().find(|(_, r)| x >= r[0] && z >= r[1] && x < r[2] && z < r[3]).map_or("Campus-Hub", |d| d.0)
}
/// Nächster Warp-Ort (Rückkehr nach einem Sturz im Hub-Archipel).
pub fn nearest_spot(x: f32, z: f32) -> u8 {
    let d = |p: &[f32; 3]| (p[0] - x).powi(2) + (p[2] - z).powi(2);
    (0..SPOTS.len()).min_by(|&a, &b| d(&SPOTS[a].1).total_cmp(&d(&SPOTS[b].1))).unwrap_or(0) as u8
}
/// Wohin führt das Portal an dieser Stelle? PORTAL_HOME oder Index in SPOTS.
pub fn portal_dest(x: i32, z: i32) -> Option<u8> {
    if !in_hub(x, z) {
        return slot_at(x, z).map(|_| 0);
    }
    let d = |p: &(i32, i32, u8)| (p.0 - x).pow(2) + (p.1 - z).pow(2);
    PORTALS.iter().filter(|p| d(*p) <= 36).min_by_key(|p| d(*p)).map(|p| p.2)
}
/// Landepunkt des Sprungpolsters, dessen Mitte höchstens `r` entfernt ist.
pub fn launch_pad(x: f32, z: f32, r: f32) -> Option<[f32; 3]> {
    LAUNCH_PADS.iter().find(|p| (p.0 as f32 + 0.5 - x).powi(2) + (p.1 as f32 + 0.5 - z).powi(2) <= r * r).map(|p| p.2)
}
/// Startgeschwindigkeit für einen Bogen von `from` nach `to` (Flugzeit 1,5..4 s, Schwerkraft GRAVITY).
pub fn launch_velocity(from: [f32; 3], to: [f32; 3]) -> [f32; 3] {
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let d = (dx * dx + dz * dz).sqrt().max(0.01);
    let t = (d / LAUNCH_SPEED).clamp(1.5, 4.0);
    [dx / t, (to[1] - from[1] + 0.5 * GRAVITY * t * t) / t, dz / t]
}
/// Sicherer Ankunftspunkt: von oben die höchste feste Fläche suchen, darüber 3×3×4 Luft (kein Portal, keine Flüssigkeit).
/// Sucht in Ringen bis 4 Blöcke um (x, z). Liefert die Füße bei (x + 0.5, oben + 1.05, z + 0.5).
pub fn find_safe_spawn(get: impl Fn(i32, i32, i32) -> u8, x: i32, z: i32) -> Option<[f32; 3]> {
    let free = |v: u8| !solid(v) && FLAGS[v as usize] & F_LIQUID == 0 && v != PORTAL;
    for r in 0..=4i32 {
        for dz in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dz.abs()) != r {
                    continue;
                }
                let (cx, cz) = (x + dx, z + dz);
                let Some(top) = (1..CH).rev().find(|&y| solid(get(cx, y, cz))) else { continue };
                if get(cx, top, cz) == SPRUNGPAD {
                    continue;
                }
                if (top + 1..=top + 4).all(|y| (-1..=1).all(|a| (-1..=1).all(|b| free(get(cx + a, y, cz + b))))) {
                    return Some([cx as f32 + 0.5, top as f32 + 1.05, cz as f32 + 0.5]);
                }
            }
        }
    }
    None
}
/// Ankunftspunkt auf einer Insel: freier Platz unter offenem Himmel vor der Starthütte (Blick nach Norden auf die Tür).
/// Genau der Punkt, den find_safe_spawn auf der unberührten Insel liefert.
pub fn plot_spawn(slot: (u8, u8)) -> [f32; 3] {
    [(slot.0 as i32 * SLOT + 48) as f32 + 0.5, GROUND as f32 + 1.05, (slot.1 as i32 * SLOT + 52) as f32 + 0.5]
}

#[inline]
pub fn idx(x: i32, y: i32, z: i32) -> usize {
    ((y * 16 + z) * 16 + x) as usize
}

pub const F_SOLID: u16 = 1; // man stößt an
pub const F_CLEAR: u16 = 2; // durchsichtig
pub const F_LIQUID: u16 = 4;
pub const F_GLOW: u16 = 8;
pub const F_RARE: u16 = 16; // nur aus Funden, handelbar
pub const F_WEEKLY: u16 = 32; // Wochenblock
pub const F_BOUNCE: u16 = 64;
pub const F_SPEED: u16 = 128;
pub const F_SEMESTER: u16 = 256;
pub const F_REGEN: u16 = 512; // abbauen gibt Ertrag, der Block bleibt stehen
/// Nicht im Gitter gezeichnet (Browser zeichnet Würfel-Instanzen, z. B. Pflanzen je Wachstumsstufe), nicht fest, aber anvisierbar.
pub const F_INST: u16 = 1024;
/// Teilblock-Modell (shape.rs): nie ein deckender Würfel, Nachbarn werden nie weggelassen; anvisierbar, auch wenn nicht fest.
pub const F_SHAPE: u16 = 2048;
/// Warmes Lampenlicht im Shader (PACKED-Bit 30, Leuchten vec3(3.2, 2.1, 0.9)); Stoff der Lampen-Teile.
pub const F_LAMP: u16 = 4096;
const S: u16 = F_SOLID;
const M: u16 = F_SOLID | F_SHAPE; // festes Teilblock-Modell
const D: u16 = F_SHAPE; // begehbares Teilblock-Modell (Blumen, Teppich, Leuchter)
const R: u16 = F_SOLID | F_RARE;

pub struct BlockDef {
    pub id: u8,
    pub name: &'static str,
    pub rgb: [u8; 3],
    pub pat: u8, // Muster im Shader
    pub price: u32, // 0 = nicht im Laden
    pub flags: u16,
    /// Name der Konstante (für den Rust-Blueprint-Export von /architect)
    pub key: &'static str,
}

macro_rules! blocks {
    ($($c:ident = $id:literal, $name:literal, [$r:literal, $g:literal, $b:literal], $pat:literal, $price:literal, $fl:expr;)*) => {
        $(pub const $c: u8 = $id;)*
        pub const BLOCKS: &[BlockDef] = &[$(BlockDef { id: $id, name: $name, rgb: [$r, $g, $b], pat: $pat, price: $price, flags: $fl, key: stringify!($c) }),*];
    };
}

// Muster: 0 glatt, 1 Wiese, 2 Ziegel, 3 Bretter, 4 Körnung, 5 Fliesen, 6 Leuchten, 7 Rinde, 8 Wellen, 9 Laub, 10 Schimmer, 11 Regal, 12 Glas,
// 13 Bruchstein, 14 Steinziegel, 15 Kristallader
blocks! {
    WIESE = 1, "Wiese", [104, 168, 84], 1, 1, S;
    ERDE = 2, "Erde", [134, 98, 66], 4, 1, S;
    FELS = 3, "Fels", [128, 132, 140], 4, 1, S;
    SAND = 4, "Sand", [226, 208, 158], 4, 1, S;
    WASSER = 5, "Wasser", [58, 128, 196], 8, 0, F_CLEAR | F_LIQUID;
    STAMM = 6, "Stamm", [112, 82, 54], 7, 2, S;
    LAUB = 7, "Laub", [70, 138, 72], 9, 1, S;
    BRETTER = 10, "Bretter", [196, 152, 96], 3, 1, S;
    ZIEGEL = 11, "Ziegel", [176, 84, 66], 2, 2, S;
    PFLASTER = 12, "Pflaster", [168, 164, 156], 5, 1, S;
    PUTZ = 13, "Kalkputz", [236, 232, 220], 0, 2, S;
    SCHIEFER = 14, "Schiefer", [72, 80, 96], 4, 2, S;
    GLAS = 15, "Glas", [190, 226, 236], 12, 3, F_SOLID | F_CLEAR;
    TAFEL = 16, "Tafelgrün", [44, 84, 70], 0, 3, S;
    KUPFER = 17, "Kupfer", [198, 120, 72], 0, 4, S;
    TEPPICH_ROT = 18, "Teppich rot", [176, 52, 60], 4, 2, S;
    TEPPICH_BLAU = 19, "Teppich blau", [48, 84, 172], 4, 2, S;
    GELB = 20, "Farbstein gelb", [238, 196, 62], 0, 2, S;
    GRUEN = 21, "Farbstein grün", [72, 164, 104], 0, 2, S;
    LILA = 22, "Farbstein lila", [140, 96, 188], 0, 2, S;
    SCHWARZ = 23, "Farbstein schwarz", [38, 40, 48], 0, 2, S;
    FLIESE = 24, "Fliese", [214, 206, 190], 5, 3, S;
    REGAL = 25, "Bücherregal", [150, 104, 64], 11, 5, S;
    DACH = 26, "Dachziegel", [156, 62, 48], 2, 3, S;
    MARMOR = 30, "Marmor", [232, 230, 236], 4, 5, F_SOLID | F_WEEKLY;
    MOSAIK = 31, "Mosaik", [64, 150, 168], 5, 5, F_SOLID | F_WEEKLY;
    TERRAZZO = 32, "Terrazzo", [206, 178, 168], 4, 4, F_SOLID | F_WEEKLY;
    BAMBUS = 33, "Bambus", [168, 190, 96], 3, 4, F_SOLID | F_WEEKLY;
    BASALT = 34, "Basalt", [56, 58, 66], 2, 4, F_SOLID | F_WEEKLY;
    BERNSTEIN = 35, "Bernstein", [226, 150, 44], 0, 5, F_SOLID | F_WEEKLY;
    SCHACH = 36, "Schachbrett", [228, 228, 228], 5, 5, F_SOLID | F_WEEKLY;
    KORALLE = 37, "Korallenstein", [236, 122, 120], 4, 5, F_SOLID | F_WEEKLY;
    GLUEH = 40, "Glühstein", [255, 214, 130], 6, 0, R | F_GLOW;
    NEON_CYAN = 41, "Neon türkis", [90, 240, 232], 6, 0, R | F_GLOW;
    NEON_PINK = 42, "Neon pink", [255, 110, 190], 6, 0, R | F_GLOW;
    FEDER = 43, "Sprungfeder", [120, 220, 120], 5, 0, R | F_BOUNCE;
    LAUFBAND = 44, "Laufband", [90, 100, 120], 3, 0, R | F_SPEED;
    AURORA = 45, "Aurora", [120, 200, 220], 10, 0, R;
    GOLD = 46, "Goldblock", [244, 200, 60], 10, 0, R;
    KRISTALL = 47, "Kristall", [200, 170, 250], 6, 0, R | F_CLEAR | F_GLOW;
    SEMESTERSTEIN = 48, "Semesterstein WS 26", [60, 90, 200], 10, 0, R | F_SEMESTER;
    BRUCHSTEIN = 50, "Bruchstein", [124, 124, 128], 13, 1, S;
    STEINZIEGEL = 51, "Steinziegel", [140, 140, 146], 14, 2, S;
    SAPHIR = 52, "Mathe-Saphir", [70, 110, 230], 15, 0, R | F_GLOW;
    SPRACHGOLD = 53, "Sprach-Gold", [236, 190, 60], 15, 0, R | F_GLOW;
    SMARAGD = 54, "Natur-Smaragd", [60, 200, 120], 15, 0, R | F_GLOW;
    LATERNE = 55, "Laterne", [255, 196, 110], 6, 4, F_SOLID | F_GLOW;
    PORTAL = 56, "Warp-Portal", [150, 90, 240], 10, 0, F_CLEAR | F_GLOW;
    GENERATOR = 57, "Bruchstein-Generator", [96, 98, 108], 13, 0, F_SOLID | F_REGEN;
    MOOS_BRUCH = 58, "Moos-Bruchstein", [104, 124, 96], 13, 1, S;
    TIEFENFELS = 59, "Tiefenfels", [64, 64, 74], 4, 1, S;
    POLIERT = 60, "Polierter Stein", [178, 180, 186], 0, 2, S;
    MOOS_ZIEGEL = 61, "Moos-Steinziegel", [116, 132, 108], 14, 2, S;
    DUNKELHOLZ = 62, "Dunkler Eichenstamm", [72, 52, 36], 7, 2, S;
    DUNKELBRETT = 63, "Dunkle Bretter", [96, 66, 42], 3, 1, S;
    BIRKE = 64, "Birkenstamm", [216, 212, 198], 7, 2, S;
    BIRKENLAUB = 65, "Birkenlaub", [118, 166, 84], 9, 1, S;
    KIRSCHLAUB = 66, "Kirschblüten", [240, 172, 204], 9, 2, S;
    GLUEHLAUB = 67, "Glühlaub", [130, 230, 170], 9, 3, F_SOLID | F_GLOW;
    STOFF_ROT = 68, "Markise rot", [196, 54, 52], 4, 2, S;
    STOFF_BLAU = 69, "Markise blau", [52, 92, 186], 4, 2, S;
    STOFF_GRUEN = 70, "Markise grün", [62, 150, 84], 4, 2, S;
    STOFF_GELB = 71, "Markise gelb", [232, 190, 64], 4, 2, S;
    STOFF_WEISS = 72, "Markise weiß", [238, 234, 226], 4, 2, S;
    FASS = 73, "Fass", [124, 86, 52], 3, 2, S;
    KISTE = 74, "Kiste", [174, 134, 84], 3, 2, S;
    TRUHE = 75, "Truhe", [152, 106, 52], 3, 5, S;
    SCHATZ = 76, "Schatztruhe", [226, 176, 64], 10, 0, F_SOLID | F_GLOW;
    WERKBANK = 77, "Werkbank", [164, 114, 72], 3, 3, S;
    BETT = 78, "Bett", [180, 56, 64], 4, 3, S;
    ABGEBAUT = 79, "Erschöpfter Fels", [54, 52, 58], 13, 0, S;
    SPRUNGPAD = 80, "Sprungpolster", [80, 224, 204], 6, 0, F_SOLID | F_GLOW;
    LAVA = 81, "Lava", [244, 112, 32], 8, 0, F_CLEAR | F_LIQUID | F_GLOW;
    GLUT = 82, "Glutstein", [206, 84, 44], 6, 0, F_SOLID | F_GLOW;
    ERZ_KOHLE = 83, "Kohle-Ader", [34, 34, 38], 15, 0, F_SOLID | F_REGEN;
    ERZ_EISEN = 84, "Eisen-Ader", [214, 176, 150], 15, 0, F_SOLID | F_REGEN;
    ERZ_GOLD = 85, "Gold-Ader", [250, 208, 70], 15, 0, F_SOLID | F_REGEN;
    ADER_SAPHIR = 86, "Saphir-Ader", [80, 120, 255], 15, 0, F_SOLID | F_REGEN;
    ADER_GOLD = 87, "Sprachgold-Ader", [255, 200, 70], 15, 0, F_SOLID | F_REGEN;
    ADER_SMARAGD = 88, "Smaragd-Ader", [70, 230, 140], 15, 0, F_SOLID | F_REGEN;
    KOHLE = 89, "Kohle", [44, 44, 48], 4, 2, S;
    EISEN = 90, "Eisen", [204, 204, 210], 0, 3, S;
    ROHGOLD = 91, "Rohgold", [232, 184, 72], 4, 5, S;
    KNOTEN_EICHE = 92, "Eichen-Holzknoten", [118, 86, 56], 7, 0, F_SOLID | F_REGEN;
    KNOTEN_BIRKE = 93, "Birken-Holzknoten", [222, 218, 204], 7, 0, F_SOLID | F_REGEN;
    SPLITTER_GEN = 94, "Splitter-Generator", [150, 110, 230], 15, 0, F_SOLID | F_REGEN;
    SPLITTER_ADER = 95, "Splitter-Kristall", [210, 170, 255], 15, 0, F_SOLID | F_REGEN;
    BLUMEN_ROT = 96, "Blumenbeet rot", [204, 72, 84], 9, 1, S;
    BLUMEN_GELB = 97, "Blumenbeet gelb", [226, 196, 72], 9, 1, S;
    HECKE = 98, "Hecke", [58, 112, 60], 9, 1, S;
    PILZHUT = 99, "Pilzhut", [188, 62, 52], 4, 1, S;
    PILZSTIEL = 100, "Pilzstiel", [224, 216, 198], 0, 1, S;
    WEG = 101, "Trampelpfad", [152, 122, 82], 4, 1, S;
    SCHIENE = 102, "Schienenbett", [112, 98, 86], 3, 0, S;
    QUESTBRETT = 103, "Questtafel", [156, 114, 62], 3, 0, S;
    AMBOSS = 104, "Amboss", [62, 62, 68], 0, 0, S;
    SOCKEL = 105, "Minion-Sockel", [184, 184, 194], 5, 2, S;
    GOLDZIER = 106, "Goldzier", [228, 182, 62], 14, 0, S;
    SCHILD = 107, "Holzschild", [176, 132, 78], 3, 0, S;
    // ---------- Phasen 16–20 ----------
    GROSSE_TRUHE = 108, "Große Wissens-Truhe", [120, 92, 170], 10, 0, S;
    ACKERBODEN = 109, "Ackerboden", [96, 66, 44], 4, 0, S;
    SAAT_WEIZEN = 110, "Weizen-Saat", [222, 196, 96], 9, 1, F_INST;
    SAAT_KAROTTE = 111, "Karotten-Saat", [236, 128, 48], 9, 1, F_INST;
    SAAT_BEERE = 112, "Leucht-Beeren-Saat", [120, 220, 255], 9, 2, F_INST | F_GLOW;
    SAAT_KUERBIS = 113, "Wissens-Kürbis-Saat", [230, 150, 40], 9, 3, F_INST;
    SAAT_KRISTALL = 114, "Kristall-Blumen-Saat", [200, 160, 255], 9, 0, F_INST | F_GLOW;
    SPRUNGPLATTE = 115, "Sprungplatte", [110, 230, 120], 6, 0, F_SOLID | F_GLOW;
    TELE_ROT = 116, "Teleporter rot", [220, 60, 60], 10, 0, F_SOLID | F_GLOW;
    TELE_BLAU = 117, "Teleporter blau", [60, 110, 230], 10, 0, F_SOLID | F_GLOW;
    TELE_GRUEN = 118, "Teleporter grün", [60, 200, 100], 10, 0, F_SOLID | F_GLOW;
    TELE_GOLD = 119, "Teleporter gold", [240, 196, 60], 10, 0, F_SOLID | F_GLOW;
    STAENDER = 120, "Rüstungsständer", [168, 132, 92], 5, 0, S;
    ALTAR = 121, "Verzauberungs-Altar", [110, 70, 200], 15, 0, F_SOLID | F_GLOW;
    BOSS_ALTAR = 122, "Prüfungs-Altar", [200, 40, 60], 10, 0, F_SOLID | F_GLOW;
    // ---------- Phasen 21–25 (alle unzerstörbar, nie im Laden; stehen nur in Instanzen bzw. als feste Portale) ----------
    KATA_PORTAL = 123, "Katakomben-Portal", [170, 30, 50], 10, 0, F_SOLID | F_GLOW;
    ARENA_PORTAL = 124, "Arena-Portal", [60, 220, 255], 10, 0, F_SOLID | F_GLOW;
    QUAD_A = 125, "Antwort-Feld A", [60, 110, 230], 6, 0, F_SOLID | F_GLOW;
    QUAD_B = 126, "Antwort-Feld B", [240, 196, 60], 6, 0, F_SOLID | F_GLOW;
    QUAD_C = 127, "Antwort-Feld C", [60, 200, 120], 6, 0, F_SOLID | F_GLOW;
    QUAD_D = 128, "Antwort-Feld D", [150, 80, 220], 6, 0, F_SOLID | F_GLOW;
    QUAD_FALSCH = 129, "Falsches Feld", [235, 50, 50], 6, 0, F_SOLID | F_GLOW;
    QUAD_RICHTIG = 130, "Richtiges Feld", [90, 255, 120], 6, 0, F_SOLID | F_GLOW;
    PRISMA_A = 131, "Spiegel-Prisma (steigend)", [150, 230, 255], 12, 0, F_SOLID | F_CLEAR | F_GLOW;
    PRISMA_B = 132, "Spiegel-Prisma (fallend)", [255, 190, 240], 12, 0, F_SOLID | F_CLEAR | F_GLOW;
    STRAHL = 133, "Lichtstrahl", [255, 70, 70], 6, 0, F_CLEAR | F_GLOW;
    STRAHLER = 134, "Licht-Emitter", [255, 120, 60], 15, 0, F_SOLID | F_GLOW;
    EMPFAENGER = 135, "Kristall-Empfänger", [90, 90, 120], 15, 0, F_SOLID;
    EMPFAENGER_AN = 136, "Kristall-Empfänger (leuchtet)", [140, 255, 220], 15, 0, F_SOLID | F_GLOW;
    RUNE = 137, "Runen-Platte", [70, 70, 96], 5, 0, F_SOLID;
    RUNE_AN = 138, "Leuchtende Rune", [120, 220, 255], 6, 0, F_SOLID | F_GLOW;
    STATUE = 139, "Weisen-Statue", [190, 186, 176], 14, 0, F_SOLID;
    WEISEN_TRUHE = 140, "Truhe der Weisen", [150, 110, 60], 3, 0, F_SOLID;
    BEUTE_ALTAR = 141, "Beute-Altar", [255, 215, 90], 10, 0, F_SOLID | F_GLOW;
    SMARAGD_TRUHE = 142, "Smaragd-Beutetruhe", [50, 210, 130], 10, 0, F_SOLID | F_GLOW;
    OBSIDIAN_TRUHE = 143, "Obsidian-Beutetruhe", [60, 40, 90], 10, 0, F_SOLID | F_GLOW;
    KATA_TOR = 144, "Katakomben-Gitter", [80, 84, 96], 2, 0, F_SOLID;
    BLUT_PORTAL = 145, "Blut-Portal", [200, 20, 40], 10, 0, F_SOLID | F_GLOW;
    // ---------- Abschnitt 8: Teilblock-Modelle (Form in shape.rs; _N/_O/_S/_W = Blickrichtung, siehe shape::facing) ----------
    SCHEIBE = 146, "Fensterscheibe", [190, 226, 236], 12, 3, M;
    GITTERSTAB = 147, "Tresorgitter", [70, 72, 80], 0, 3, M;
    TREPPE_EICHE_N = 148, "Eichentreppe", [196, 152, 96], 3, 2, M;
    TREPPE_EICHE_O = 149, "Eichentreppe", [196, 152, 96], 3, 2, M;
    TREPPE_EICHE_S = 150, "Eichentreppe", [196, 152, 96], 3, 2, M;
    TREPPE_EICHE_W = 151, "Eichentreppe", [196, 152, 96], 3, 2, M;
    TREPPE_STEIN_N = 152, "Steintreppe", [140, 140, 146], 14, 2, M;
    TREPPE_STEIN_O = 153, "Steintreppe", [140, 140, 146], 14, 2, M;
    TREPPE_STEIN_S = 154, "Steintreppe", [140, 140, 146], 14, 2, M;
    TREPPE_STEIN_W = 155, "Steintreppe", [140, 140, 146], 14, 2, M;
    TREPPE_DACH_N = 156, "Dachtreppe", [156, 62, 48], 2, 3, M;
    TREPPE_DACH_O = 157, "Dachtreppe", [156, 62, 48], 2, 3, M;
    TREPPE_DACH_S = 158, "Dachtreppe", [156, 62, 48], 2, 3, M;
    TREPPE_DACH_W = 159, "Dachtreppe", [156, 62, 48], 2, 3, M;
    STUFE_EICHE = 160, "Eichenstufe", [196, 152, 96], 3, 1, M;
    STUFE_STEIN = 161, "Steinstufe", [140, 140, 146], 14, 1, M;
    STUFE_DACH = 162, "Dachstufe", [156, 62, 48], 2, 2, M;
    TRESEN = 163, "Tresen", [170, 118, 70], 3, 3, M;
    TISCH = 164, "Tisch", [176, 128, 80], 3, 3, M;
    STUHL_N = 165, "Stuhl", [150, 104, 64], 3, 2, M;
    STUHL_O = 166, "Stuhl", [150, 104, 64], 3, 2, M;
    STUHL_S = 167, "Stuhl", [150, 104, 64], 3, 2, M;
    STUHL_W = 168, "Stuhl", [150, 104, 64], 3, 2, M;
    SITZBANK_N = 169, "Sitzbank", [124, 86, 52], 3, 2, M;
    SITZBANK_O = 170, "Sitzbank", [124, 86, 52], 3, 2, M;
    SITZBANK_S = 171, "Sitzbank", [124, 86, 52], 3, 2, M;
    SITZBANK_W = 172, "Sitzbank", [124, 86, 52], 3, 2, M;
    WEBTEPPICH_ROT = 173, "Webteppich rot", [176, 52, 60], 4, 2, D;
    WEBTEPPICH_GOLD = 174, "Webteppich gold", [214, 168, 60], 4, 2, D;
    WEBTEPPICH_BLAU = 175, "Webteppich blau", [48, 84, 172], 4, 2, D;
    WEBTEPPICH_SMARAGD = 176, "Webteppich smaragd", [40, 150, 100], 4, 2, D;
    KRONLEUCHTER = 177, "Kronleuchter", [58, 58, 64], 0, 5, D | F_GLOW;
    LATERNENMAST = 178, "Laternenmast", [52, 52, 58], 0, 4, M | F_GLOW;
    WANDLATERNE = 179, "Wandlaterne", [52, 52, 58], 0, 3, D | F_GLOW;
    ZAUN = 180, "Holzzaun", [124, 86, 52], 3, 1, M;
    GELAENDER = 181, "Geländer", [72, 52, 36], 3, 2, M;
    MARKISE_ROT_N = 182, "Markise rot-weiß", [196, 54, 52], 4, 2, M;
    MARKISE_ROT_O = 183, "Markise rot-weiß", [196, 54, 52], 4, 2, M;
    MARKISE_ROT_S = 184, "Markise rot-weiß", [196, 54, 52], 4, 2, M;
    MARKISE_ROT_W = 185, "Markise rot-weiß", [196, 54, 52], 4, 2, M;
    MARKISE_BLAU_N = 186, "Markise blau-weiß", [52, 92, 186], 4, 2, M;
    MARKISE_BLAU_O = 187, "Markise blau-weiß", [52, 92, 186], 4, 2, M;
    MARKISE_BLAU_S = 188, "Markise blau-weiß", [52, 92, 186], 4, 2, M;
    MARKISE_BLAU_W = 189, "Markise blau-weiß", [52, 92, 186], 4, 2, M;
    DEKOFASS = 190, "Deko-Fass", [124, 86, 52], 3, 2, M;
    BUECHERSTAPEL = 191, "Bücherstapel", [150, 60, 50], 4, 2, D;
    GOLDSTAPEL = 192, "Goldbarren-Stapel", [244, 200, 60], 10, 5, M;
    MOHN = 193, "Mohnblume", [210, 48, 44], 9, 1, D;
    SONNENBLUME = 194, "Sonnenblume", [240, 196, 40], 9, 1, D;
    KORNBLUME = 195, "Kornblume", [80, 110, 220], 9, 1, D;
    LEUCHTKLEE = 196, "Leucht-Klee", [120, 230, 170], 9, 2, D | F_GLOW;
    HOHES_GRAS = 197, "Hohes Gras", [96, 160, 72], 9, 1, D;
    LAMPENGLAS = 198, "Lampenglas", [255, 214, 140], 6, 0, F_SOLID | F_GLOW | F_LAMP;
}
/// Abschnitt 8: Wildblumen (Kreuz-Modelle) für Wiesen-Helfer.
pub const BLUMEN: [u8; 4] = [MOHN, SONNENBLUME, KORNBLUME, LEUCHTKLEE];
/// Webteppiche in der Reihenfolge Rot, Gold, Blau, Smaragd.
pub const TEPPICHE: [u8; 4] = [WEBTEPPICH_ROT, WEBTEPPICH_GOLD, WEBTEPPICH_BLAU, WEBTEPPICH_SMARAGD];
/// Saat-Blöcke (= Saat-Gegenstände) der Reihe nach, Index = Pflanze in farming_data::CROPS.
pub const SAATEN: [u8; 5] = [SAAT_WEIZEN, SAAT_KAROTTE, SAAT_BEERE, SAAT_KUERBIS, SAAT_KRISTALL];
pub const TELEPORTER: [u8; 4] = [TELE_ROT, TELE_BLAU, TELE_GRUEN, TELE_GOLD];
/// Phasen 21–25: Blöcke, die man mit [E] benutzt (Browser: interact/prompt, Server: C2S::Chest → use_block bzw. inst_use).
pub const USE_NEW: [u8; 10] = [KATA_PORTAL, ARENA_PORTAL, PRISMA_A, PRISMA_B, STATUE, WEISEN_TRUHE, BEUTE_ALTAR, SMARAGD_TRUHE, OBSIDIAN_TRUHE, BLUT_PORTAL];
/// Antwort-Felder der Lern-Duell-Arena in der Reihenfolge A, B, C, D.
pub const QUADS: [u8; 4] = [QUAD_A, QUAD_B, QUAD_C, QUAD_D];
/// Prüfungs-Zitadelle: Kampfzone im Mauerhof (x0, z0, x1, z1), Füße auf CITADEL_Y. Innenhalle = Bergfried, Arena = Südhof.
pub const CITADEL: [i32; 4] = [226, 70, 255, 95];
pub const CITADEL_HALL: [i32; 4] = [235, 73, 246, 82];
pub const CITADEL_ARENA: [i32; 4] = [232, 84, 249, 95];
pub const CITADEL_Y: f32 = 31.0;
/// Prüfungs-Altar (Boss rufen) im Südhof, Verzauberungs-Altar im Erdgeschoss des Bibliotheks-Turms (blueprints setzt beide).
pub const BOSS_ALTAR_AT: [i32; 3] = [240, 31, 92];
/// Abschnitt 8: Altar auf dem Podest (y 31) in der Mitte des Bibliotheks-Atriums.
pub const ALTAR_AT: [i32; 3] = [393, 32, 240];
/// Phasen 23/24: Portal-Blöcke (2 hoch, [E]): Katakomben neben der Ankunft der Zitadelle, Arena neben dem Hub-Spawn.
pub const KATA_PORTAL_AT: [i32; 3] = [245, 31, 104];
pub const ARENA_PORTAL_AT: [i32; 3] = [249, 33, 296];
pub fn in_citadel(x: f32, y: f32, z: f32) -> bool {
    let [x0, z0, x1, z1] = CITADEL;
    x >= x0 as f32 && z >= z0 as f32 && x < x1 as f32 && z < z1 as f32 && (28.0..50.0).contains(&y)
}

// ---------- Abbauen (Überlebens-Modus) ----------
/// Werkzeuge, die das Abbauen beschleunigen (Gegenstands-Nummern; skyblock nimmt sie in TOOLS auf).
pub const SPITZHACKE: u16 = 1014;
pub const AXT: u16 = 1015;
/// Werkstoff-Klassen: Geräusch beim Laufen/Abbauen und Abbau-Zeit.
pub const M_STEIN: u8 = 0;
pub const M_ERDE: u8 = 1;
pub const M_HOLZ: u8 = 2;
pub const M_PFLANZE: u8 = 3;
pub const M_KRISTALL: u8 = 4; // auch Glas
pub const M_STOFF: u8 = 5;
pub const M_METALL: u8 = 6;
pub const M_FLUESSIG: u8 = 7;

pub fn material(b: u8) -> u8 {
    match b {
        LAUB | BIRKENLAUB | KIRSCHLAUB | GLUEHLAUB | HECKE | BLUMEN_ROT | BLUMEN_GELB | PILZHUT | PILZSTIEL | SAAT_WEIZEN | SAAT_KAROTTE
        | SAAT_BEERE | SAAT_KUERBIS | SAAT_KRISTALL | MOHN..=HOHES_GRAS => M_PFLANZE,
        TREPPE_EICHE_N..=TREPPE_EICHE_W | STUFE_EICHE | TRESEN | TISCH | STUHL_N..=SITZBANK_W | ZAUN | GELAENDER | DEKOFASS | BUECHERSTAPEL => M_HOLZ,
        WEBTEPPICH_ROT..=WEBTEPPICH_SMARAGD | MARKISE_ROT_N..=MARKISE_BLAU_W => M_STOFF,
        SCHEIBE | LAMPENGLAS => M_KRISTALL,
        GITTERSTAB | KRONLEUCHTER | LATERNENMAST | WANDLATERNE | GOLDSTAPEL => M_METALL,
        WIESE | ERDE | SAND | WEG | ACKERBODEN => M_ERDE,
        STAMM | BRETTER | REGAL | BAMBUS | DUNKELHOLZ | DUNKELBRETT | BIRKE | FASS | KISTE | TRUHE | SCHATZ | WERKBANK | QUESTBRETT | SCHILD | SCHIENE
        | KNOTEN_EICHE | KNOTEN_BIRKE | GROSSE_TRUHE | STAENDER => M_HOLZ,
        TEPPICH_ROT | TEPPICH_BLAU | STOFF_ROT | STOFF_BLAU | STOFF_GRUEN | STOFF_GELB | STOFF_WEISS | BETT => M_STOFF,
        GLAS | KRISTALL | SAPHIR | SPRACHGOLD | SMARAGD | ADER_SAPHIR | ADER_GOLD | ADER_SMARAGD | SPLITTER_GEN | SPLITTER_ADER | NEON_CYAN
        | NEON_PINK | GLUEH | AURORA | BERNSTEIN | PORTAL | TELE_ROT | TELE_BLAU | TELE_GRUEN | TELE_GOLD | SPRUNGPLATTE | KATA_PORTAL..=EMPFAENGER_AN
        | RUNE_AN | BEUTE_ALTAR | BLUT_PORTAL => M_KRISTALL,
        KUPFER | GOLD | GOLDZIER | EISEN | ROHGOLD | AMBOSS | LATERNE => M_METALL,
        WASSER | LAVA => M_FLUESSIG,
        _ => M_STEIN,
    }
}
/// Grund-Abbauzeit in Sekunden ohne Werkzeug; unendlich = nicht abbaubar.
pub fn hardness(b: u8) -> f32 {
    match b {
        0 | WASSER | LAVA | PORTAL | ABGEBAUT | SCHATZ | SPRUNGPAD | ALTAR | BOSS_ALTAR | KATA_PORTAL..=BLUT_PORTAL => f32::INFINITY,
        SAAT_WEIZEN | SAAT_KAROTTE | SAAT_BEERE | SAAT_KUERBIS | SAAT_KRISTALL => 0.1,
        GLAS => 0.3,
        ERZ_KOHLE | ERZ_EISEN | ERZ_GOLD => 1.4,
        _ => match material(b) {
            M_PFLANZE => 0.25,
            M_ERDE => 0.4,
            M_HOLZ => 0.7,
            M_STOFF => 0.3,
            M_KRISTALL => 1.6,
            M_METALL => 1.4,
            _ => 1.1,
        },
    }
}
/// Abbauzeit mit Werkzeug in der Hand (0 = Hand) und Baukunst-Stufe (je Stufe 2 % schneller, Stufe 50 = halbe Zeit).
pub fn mine_secs(b: u8, tool: u16, bau_level: u32) -> f32 {
    let h = hardness(b);
    if !h.is_finite() {
        return h;
    }
    let m = material(b);
    let f = match tool {
        SPITZHACKE if matches!(m, M_STEIN | M_KRISTALL | M_METALL) => 0.4,
        AXT if m == M_HOLZ => 0.4,
        crate::skyblock::HAMMER if matches!(m, M_STEIN | M_KRISTALL | M_METALL | M_HOLZ) => 0.6,
        _ => 1.0,
    };
    (h * f / (1.0 + bau_level.min(50) as f32 * 0.02)).max(0.05)
}
/// Wie mine_secs, dazu der Abbau-Tempo-Wert aus stats (Prozent, Rüstung/Verzauberung): Zeit / (1 + speed/100).
pub fn mine_secs_speed(b: u8, tool: u16, bau_level: u32, speed_pct: f32) -> f32 {
    (mine_secs(b, tool, bau_level) / (1.0 + speed_pct.max(0.0) / 100.0)).max(0.05)
}
/// Server-Prüfung: hat das Abbauen lange genug gedauert? (30 % Spielraum plus 150 ms für Netz-Schwankung)
pub fn mine_ok(secs: f32, elapsed_ms: i64) -> bool {
    secs.is_finite() && elapsed_ms as f32 + 150.0 >= secs * 700.0
}
/// Baukunst-EP für einen abgebauten Block.
pub fn mine_xp(b: u8) -> u64 {
    match material(b) {
        M_HOLZ => 2,
        M_STEIN | M_METALL => 3,
        M_KRISTALL => 5,
        _ => 1,
    }
}
/// Ertrag beim Abbauen eines nachwachsenden Blocks (Gegenstands-Nummer); `roll` ist eine Zufallszahl des Servers.
/// Danach steht REGEN_MS lang ABGEBAUT an der Stelle, dann wieder der Block.
// ponytail: Splitter nur mit kleiner Wahrscheinlichkeit, damit Abbauen das Lernen im Lernhelfer nicht ersetzt; Quoten bei Bedarf anpassen.
pub fn regen_drop(b: u8, roll: u32) -> Option<u16> {
    if FLAGS[b as usize] & F_REGEN == 0 {
        return None;
    }
    let one_in = |n: u32, hit: u16, miss: u8| if roll % n == 0 { hit } else { miss as u16 };
    Some(match b {
        GENERATOR => BRUCHSTEIN as u16,
        SPLITTER_GEN => one_in(25, crate::skyblock::SHARD + 4, BRUCHSTEIN),
        SPLITTER_ADER => one_in(10, crate::skyblock::SHARD + (roll / 16 % 5) as u16, TIEFENFELS),
        ADER_SAPHIR => one_in(8, SAPHIR as u16, TIEFENFELS),
        ADER_GOLD => one_in(8, SPRACHGOLD as u16, TIEFENFELS),
        ADER_SMARAGD => one_in(8, SMARAGD as u16, TIEFENFELS),
        ERZ_KOHLE => KOHLE as u16,
        ERZ_EISEN => EISEN as u16,
        ERZ_GOLD => ROHGOLD as u16,
        KNOTEN_EICHE => STAMM as u16,
        KNOTEN_BIRKE => BIRKE as u16,
        _ => b as u16,
    })
}
/// Darf hier gesetzt (`place`) oder abgebaut werden? Hub-Archipel: nur nachwachsende Blöcke abbauen.
/// Privatinsel: `may` = Rechte am Grundstück dieses Platzes (Besitz, Mitbauer/Co-op, Gruppe) → überall auf dem ganzen Platz
/// setzen und abbauen (kein 16×16-Rechteck mehr). `_in_plot` bleibt nur für alte Aufrufer. Den Ankerblock unter dem
/// Ankunftspunkt schützt `anchor` (Aufrufer mit y).
pub fn edit_allowed(x: i32, z: i32, cur: u8, place: bool, may: bool, _in_plot: bool) -> bool {
    if in_hub(x, z) {
        return !place && FLAGS[cur as usize] & F_REGEN != 0;
    }
    may && slot_at(x, z).is_some()
}
/// Geschützter Ankerblock einer Privatinsel: der Boden unter dem Ankunftspunkt (plot_spawn) – nie abbauen/ersetzen.
pub fn anchor(x: i32, y: i32, z: i32) -> bool {
    slot_at(x, z).is_some_and(|s| {
        let p = plot_spawn(s);
        x == p[0].floor() as i32 && z == p[2].floor() as i32 && y == GROUND
    })
}

pub const FLAGS: [u16; 256] = {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < BLOCKS.len() {
        t[BLOCKS[i].id as usize] = BLOCKS[i].flags;
        i += 1;
    }
    t
};
/// Farbe, Muster und Leuchten je Block, fertig gepackt für den Shader: r | g<<8 | b<<16 | muster<<24 | leuchtet<<28.
pub const PACKED: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < BLOCKS.len() {
        let b = &BLOCKS[i];
        t[b.id as usize] = b.rgb[0] as u32
            | (b.rgb[1] as u32) << 8
            | (b.rgb[2] as u32) << 16
            | (b.pat as u32) << 24
            | (((b.flags & F_GLOW) != 0) as u32) << 28
            | (((b.flags & F_CLEAR) != 0) as u32) << 29
            | (((b.flags & F_LAMP) != 0) as u32) << 30;
        i += 1;
    }
    t
};

pub fn block(id: u8) -> Option<&'static BlockDef> {
    BLOCKS.iter().find(|b| b.id == id)
}
#[inline]
pub fn solid(id: u8) -> bool {
    FLAGS[id as usize] & F_SOLID != 0
}
/// Deckender Vollwürfel (`is_opaque_full_cube`): nur dann darf der Mesher die Nachbarfläche weglassen.
/// Teilblock-Modelle (F_SHAPE) sind es nie – sonst entstehen Löcher unter Tischen, Teppichen, Treppen, Scheiben und Blumen.
#[inline]
pub fn opaque(id: u8) -> bool {
    FLAGS[id as usize] & (F_SOLID | F_CLEAR | F_SHAPE) == F_SOLID
}
#[inline]
pub fn is_opaque_full_cube(id: u8) -> bool {
    opaque(id)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    pub regions: u8,  // 0..=4 freigeschaltete Gebiete
    pub projects: u8, // Bitmaske fertiger Großprojekte
}
pub const P_BRIDGE: u8 = 1;
pub const P_TOWER: u8 = 2;
pub const P_ISLAND: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Sea,
    Commons,
    Group,
    Personal,
}

/// Art eines Platzes im 10×10-Raster: der Hub-Archipel (i, j < 5) gehört allen, die letzte Reihe den Lerngruppen,
/// alle übrigen sind Privatinseln (65). Alle Inseln schweben von Anfang an; der Weltzustand ändert daran nichts.
pub fn slot_kind(i: i32, j: i32, _st: WorldState) -> SlotKind {
    if !(0..SLOTS).contains(&i) || !(0..SLOTS).contains(&j) {
        SlotKind::Sea
    } else if i < HUB_SLOTS && j < HUB_SLOTS {
        SlotKind::Commons
    } else if j == SLOTS - 1 {
        SlotKind::Group
    } else {
        SlotKind::Personal
    }
}

/// Alle Plätze einer Art in fester Reihenfolge (Vergabe der Grundstücke): die dem Hub nächsten zuerst.
pub fn slots_of(kind: SlotKind, st: WorldState) -> Vec<(u8, u8)> {
    let mut v: Vec<(u8, u8)> = (0..SLOTS).flat_map(|j| (0..SLOTS).map(move |i| (i, j))).filter(|&(i, j)| slot_kind(i, j, st) == kind).map(|(i, j)| (i as u8, j as u8)).collect();
    v.sort_by_key(|&(i, j)| (i.max(j), j, i));
    v
}

/// Rechteck eines Grundstücks: (x0, z0, Kantenlänge), mittig im Platz (liegt auf der 38×38-Hauptinsel).
pub fn plot_rect(slot: (u8, u8), size: u8) -> (i32, i32, i32) {
    let o = (SLOT - size as i32) / 2;
    (slot.0 as i32 * SLOT + o, slot.1 as i32 * SLOT + o, size as i32)
}
pub fn in_rect(r: (i32, i32, i32), x: i32, z: i32) -> bool {
    x >= r.0 && z >= r.1 && x < r.0 + r.2 && z < r.1 + r.2
}
/// Gemeinschaftsfläche = Hub-Archipel.
pub fn is_commons(x: i32, z: i32, _st: WorldState) -> bool {
    in_hub(x, z)
}

pub(crate) fn hash(x: i32, z: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77) ^ s.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}
pub(crate) fn rnd(x: i32, z: i32, s: u32) -> f32 {
    (hash(x, z, s) & 0xFFFF) as f32 / 65535.0
}
pub(crate) fn noise(x: f32, z: f32, s: u32) -> f32 {
    let (xi, zi) = (x.floor(), z.floor());
    let (fx, fz) = (x - xi, z - zi);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (xi, zi) = (xi as i32, zi as i32);
    let (a, b, c, d) = (rnd(xi, zi, s), rnd(xi + 1, zi, s), rnd(xi, zi + 1, s), rnd(xi + 1, zi + 1, s));
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Höhe und Deckblock der Spalte; (-1, 0) ist Leere.
pub fn column(x: i32, z: i32, st: WorldState) -> (i32, u8) {
    crate::blueprints::surf(x, z, st).map_or((-1, 0), |s| (s.h, s.top))
}

/// Erzeugt ein Stück vollständig aus dem Weltzustand (ohne Spieler-Bauten). Reine Luft ist ein leerer Vec (spart Speicher).
pub fn gen_chunk(cx: i32, cz: i32, st: WorldState) -> Vec<u8> {
    let (ox, oz) = (cx * 16, cz * 16);
    if !crate::blueprints::touches(ox, oz) {
        return Vec::new();
    }
    let mut b = vec![0u8; VOL];
    crate::blueprints::build(&mut crate::blueprints::Put { b: &mut b, ox, oz }, st);
    if b.iter().all(|&v| v == 0) {
        return Vec::new();
    }
    b
}

/// Gepacktes reines Luft-Stück (zum Senden leerer Stücke).
pub fn rle_air() -> Vec<u8> {
    rle(&[0u8; VOL])
}

/// Strahl durch das Blockgitter (DDA): erster feste Block und das Feld davor.
/// Wie raycast, aber nur feste Blöcke (und F_INST) halten den Strahl auf: Blumen, Gras, Teppich, Kronleuchter nicht (Sichtlinie für Schüsse).
pub fn raycast_solid(get: impl Fn(i32, i32, i32) -> u8, o: [f32; 3], d: [f32; 3], max: f32) -> Option<([i32; 3], [i32; 3])> {
    raycast(
        |x, y, z| {
            let v = get(x, y, z);
            if solid(v) || FLAGS[v as usize] & F_INST != 0 { v } else { 0 }
        },
        o,
        d,
        max,
    )
}

pub fn raycast(get: impl Fn(i32, i32, i32) -> u8, o: [f32; 3], d: [f32; 3], max: f32) -> Option<([i32; 3], [i32; 3])> {
    let mut c = [o[0].floor() as i32, o[1].floor() as i32, o[2].floor() as i32];
    let mut step = [0i32; 3];
    let (mut tmax, mut tdelta) = ([f32::MAX; 3], [f32::MAX; 3]);
    for k in 0..3 {
        if d[k] > 1e-6 {
            step[k] = 1;
            tmax[k] = (c[k] as f32 + 1.0 - o[k]) / d[k];
            tdelta[k] = 1.0 / d[k];
        } else if d[k] < -1e-6 {
            step[k] = -1;
            tmax[k] = (c[k] as f32 - o[k]) / d[k];
            tdelta[k] = -1.0 / d[k];
        }
    }
    let mut prev = c;
    loop {
        let v = get(c[0], c[1], c[2]);
        if (solid(v) || FLAGS[v as usize] & (F_INST | F_SHAPE) != 0) && c[1] >= 0 {
            return Some((c, prev));
        }
        let k = if tmax[0] < tmax[1] && tmax[0] < tmax[2] { 0 } else if tmax[1] < tmax[2] { 1 } else { 2 };
        if tmax[k] > max {
            return None;
        }
        prev = c;
        c[k] += step[k];
        tmax[k] += tdelta[k];
    }
}

/// Lauflängen-Kodierung: (Block, Länge u16 LE).
pub fn rle(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    let mut i = 0;
    while i < b.len() {
        let v = b[i];
        let mut n = 1usize;
        while i + n < b.len() && b[i + n] == v && n < 65535 {
            n += 1;
        }
        out.push(v);
        out.extend_from_slice(&(n as u16).to_le_bytes());
        i += n;
    }
    out
}
pub fn unrle(r: &[u8], out: &mut [u8]) -> bool {
    let mut o = 0;
    for c in r.chunks_exact(3) {
        let n = u16::from_le_bytes([c[1], c[2]]) as usize;
        if o + n > out.len() {
            return false;
        }
        out[o..o + n].fill(c[0]);
        o += n;
    }
    o == out.len() && r.len() % 3 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Welt-Abfrage über gecachte Stücke (wie der Server, nur ohne Spieler-Bauten).
    struct Welt(RefCell<HashMap<(i32, i32), Vec<u8>>>, WorldState);
    impl Welt {
        fn new(st: WorldState) -> Welt {
            Welt(RefCell::new(HashMap::new()), st)
        }
        fn get(&self, x: i32, y: i32, z: i32) -> u8 {
            if !(0..WORLD).contains(&x) || !(0..WORLD).contains(&z) || !(0..CH).contains(&y) {
                return 0;
            }
            let st = self.1;
            let mut m = self.0.borrow_mut();
            let c = m.entry((x >> 4, z >> 4)).or_insert_with(|| gen_chunk(x >> 4, z >> 4, st));
            if c.is_empty() {
                0
            } else {
                c[idx(x & 15, y, z & 15)]
            }
        }
        /// höchster Block der Spalte (nicht Luft), sonst -1
        fn top(&self, x: i32, z: i32) -> i32 {
            (0..CH).rev().find(|&y| self.get(x, y, z) != 0).unwrap_or(-1)
        }
    }

    #[test]
    fn rle_roundtrip() {
        let c = gen_chunk(38, 38, WorldState::default());
        assert_eq!(c.len(), VOL, "Privatinsel (6, 6)");
        let r = rle(&c);
        assert!(r.len() < VOL, "Stück wird kleiner: {}", r.len());
        let mut back = vec![0u8; VOL];
        assert!(unrle(&r, &mut back));
        assert_eq!(c, back);
        assert!(!unrle(&r[..r.len() - 3], &mut back));
        assert!(unrle(&rle_air(), &mut back) && back.iter().all(|&b| b == 0));
    }

    #[test]
    fn insel_und_plaetze() {
        let st = WorldState::default();
        assert_eq!(slots_of(SlotKind::Personal, st).len(), 65);
        assert_eq!(slots_of(SlotKind::Group, st).len(), 10);
        assert_eq!(slots_of(SlotKind::Personal, st)[0], (5, 0), "die dem Hub nächste zuerst");
        // jedes Grundstück liegt auch in voller Größe ganz auf der Hauptinsel seiner Privatinsel
        for s in slots_of(SlotKind::Personal, st).into_iter().chain(slots_of(SlotKind::Group, st)) {
            let r = plot_rect(s, 32);
            for z in r.1..r.1 + 32 {
                for x in r.0..r.0 + 32 {
                    assert_eq!(column(x, z, st).0, GROUND, "Platz {s:?} bei {x},{z}");
                }
            }
        }
        assert_eq!(column(1, 1, st), (-1, 0), "außen ist Leere");
        assert!(gen_chunk(0, 0, st).is_empty(), "reine Luft kostet keinen Speicher");
        assert!(gen_chunk(WC - 1, 0, st).is_empty(), "Ecke eines Inselplatzes ist Leere");
        // zwischen Hauptinsel und Satellit: Leere, darüber die Himmelsbrücke
        assert_eq!(column(350, 240, st).0, -1);
        assert_eq!(gen_chunk(350 / 16, 240 / 16, st)[idx(350 & 15, GROUND, 240 & 15)], BRETTER);
        // Privatinsel: Hängebrücke über die Leere zur Rohstoffinsel
        let (a, c) = (5 * SLOT, 0);
        let w = Welt::new(st);
        assert_eq!(column(a + 70, c + 48, st).0, -1);
        assert_eq!(w.get(a + 70, GROUND, c + 48), BRETTER);
        assert_eq!(column(a + 81, c + 48, st).0, GROUND, "Rohstoffinsel");
        // Inseln hängen: unter der Oberfläche ein tiefer Felskegel, ganz unten und daneben Luft
        assert!(w.get(a + 48, GROUND - 6, c + 48) != 0 && w.get(a + 48, 0, c + 48) == 0);
        let tiefe = (0..GROUND).rev().take_while(|&y| w.get(a + 48, y, c + 48) != 0).count();
        assert!(tiefe >= 18, "Kegel unter der Inselmitte nur {tiefe} tief");
        assert!((0..CH).all(|y| w.get(a + 10, y, c + 10) == 0));
        assert!(is_commons(MID, MID, st) && !is_commons(600, 150, st));
        // Plätze des Hubs sind keine Privatinseln
        assert!(slots_of(SlotKind::Personal, st).iter().all(|s| slot_at(s.0 as i32 * SLOT, s.1 as i32 * SLOT) == Some(*s)));
    }

    #[test]
    fn projekte_aendern_nur_ihre_stellen() {
        let (a, b) = (WorldState::default(), WorldState { regions: 0, projects: P_TOWER });
        assert_eq!(gen_chunk(40, 40, a), gen_chunk(40, 40, b), "Privatinseln bleiben");
        assert_eq!(gen_chunk(MID / 16, MID / 16, a), gen_chunk(MID / 16, MID / 16, b));
        assert_ne!(gen_chunk(262 / 16, 186 / 16, a), gen_chunk(262 / 16, 186 / 16, b), "der Turm");
        assert!(gen_chunk(240 / 16, 384 / 16, a).is_empty());
        assert_ne!(gen_chunk(240 / 16, 384 / 16, a), gen_chunk(240 / 16, 384 / 16, WorldState { regions: 0, projects: P_ISLAND }), "die Insel");
        assert!(gen_chunk(240 / 16, 350 / 16, a).is_empty());
        assert!(!gen_chunk(240 / 16, 350 / 16, WorldState { regions: 0, projects: P_BRIDGE }).is_empty(), "der Steg");
    }

    #[test]
    fn abbauen_und_ankunft() {
        assert_eq!((hardness(LAUB), hardness(WIESE), hardness(STAMM), hardness(BRUCHSTEIN), hardness(ADER_SAPHIR)), (0.25, 0.4, 0.7, 1.1, 1.6));
        assert!(!hardness(ABGEBAUT).is_finite() && !hardness(0).is_finite() && !mine_ok(hardness(PORTAL), 99_999));
        assert!(mine_secs(FELS, SPITZHACKE, 0) < mine_secs(FELS, crate::skyblock::HAMMER, 0) && mine_secs(FELS, crate::skyblock::HAMMER, 0) < mine_secs(FELS, 0, 0));
        assert!(mine_secs(STAMM, AXT, 50) < mine_secs(STAMM, AXT, 0) && mine_secs(STAMM, SPITZHACKE, 0) == mine_secs(STAMM, 0, 0));
        assert!(mine_ok(1.1, 800) && !mine_ok(1.1, 300));
        assert_eq!(regen_drop(GENERATOR, 7), Some(BRUCHSTEIN as u16));
        assert_eq!(regen_drop(SPLITTER_GEN, 25), Some(crate::skyblock::SHARD + 4));
        assert_eq!(regen_drop(FELS, 0), None);
        assert!(edit_allowed(MID, MID, ERZ_KOHLE, false, false, false) && !edit_allowed(MID, MID, FELS, false, true, true) && !edit_allowed(MID, MID, 0, true, true, true));
        assert!(edit_allowed(600, 600, FELS, false, true, true) && !edit_allowed(600, 600, FELS, false, false, true) && edit_allowed(600, 600, GENERATOR, false, true, false));
        assert!(edit_allowed(600, 600, STAMM, false, true, false) && edit_allowed(600, 600, 0, true, true, false), "ganze Insel bebaubar");
        assert!(!edit_allowed(600, 600, 0, true, false, false), "fremde Insel bleibt gesperrt");
        let sp = plot_spawn((6, 6));
        assert!(anchor(sp[0] as i32, GROUND, sp[2] as i32) && !anchor(sp[0] as i32 + 1, GROUND, sp[2] as i32));
        assert!(in_citadel(240.0, 31.0, 88.0) && !in_citadel(240.0, 31.0, 104.0));
        assert!(raycast(|x, y, z| if (x, y, z) == (3, 0, 0) { SAAT_WEIZEN } else { 0 }, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 5.0).is_some(), "Saat anvisierbar");
        assert!(!solid(SAAT_WEIZEN) && mine_secs_speed(FELS, 0, 0, 100.0) < mine_secs(FELS, 0, 0));
        // Ankunft: flacher Boden auf y = 10, Portal in der Mitte → erst zwei Felder daneben ist 3×3×4 frei
        let get = |x: i32, y: i32, z: i32| if y == 10 { FELS } else if (x, y, z) == (50, 11, 50) { PORTAL } else { 0 };
        assert_eq!(find_safe_spawn(get, 70, 70), Some([70.5, 11.05, 70.5]));
        let p = find_safe_spawn(get, 50, 50).unwrap();
        assert!(p[1] == 11.05 && ((p[0] - 50.5).abs() >= 2.0 || (p[2] - 50.5).abs() >= 2.0));
        assert_eq!(find_safe_spawn(|_, _, _| 0, 5, 5), None);
        // Zonen, Portale, Sprungpolster
        assert_eq!(zone_name(MID, 30, 230), "Marktplatz");
        assert_eq!(zone_name(MID, 10, 230), "Kristallminen");
        assert_eq!(zone_name(390, 30, 240), "Bibliotheks-Turm");
        assert_eq!(zone_name(700, 30, 700), "Privatinsel");
        assert_eq!(portal_dest(231, 297), Some(PORTAL_HOME));
        assert_eq!(portal_dest(700, 700), Some(0));
        assert_eq!(portal_dest(MID, MID), None);
        assert_eq!(slot_at(700, 100), Some((7, 1)));
        assert_eq!(slot_at(100, 100), None);
        assert_eq!(nearest_spot(240.0, 296.0), 0);
        for p in LAUNCH_PADS {
            let v = launch_velocity([p.0 as f32 + 0.5, 36.0, p.1 as f32 + 0.5], p.2);
            assert!(v[0].hypot(v[2]) <= LAUNCH_MAX_SPEED && v[1] > 0.0, "{v:?}");
            assert_eq!(launch_pad(p.0 as f32 + 0.5, p.1 as f32 + 0.5, 2.0), Some(p.2));
        }
        for s in SPOTS {
            assert!(in_hub(s.1[0] as i32, s.1[2] as i32), "{}", s.0);
        }
    }

    #[test]
    fn ankunft_auf_festem_boden() {
        let st = WorldState::default();
        let w = Welt::new(st);
        for (name, p, _) in SPOTS {
            let s = find_safe_spawn(|x, y, z| w.get(x, y, z), p[0] as i32, p[2] as i32).unwrap_or_else(|| panic!("{name}: kein sicherer Platz"));
            assert!((s[1] - p[1]).abs() <= 3.0, "{name}: {s:?} statt {p:?}");
            assert!(solid(w.get(s[0] as i32, s[1] as i32 - 1, s[2] as i32)), "{name} steht auf festem Boden");
            assert!((0..4).all(|k| w.get(s[0] as i32, s[1] as i32 + k, s[2] as i32) == 0), "{name}: Luft darüber");
        }
        for slot in [(5u8, 0u8), (9, 9), (0, 5), (7, 3)] {
            let p = plot_spawn(slot);
            assert_eq!(find_safe_spawn(|x, y, z| w.get(x, y, z), p[0] as i32, p[2] as i32), Some(p), "Insel {slot:?}");
        }
        for (x, z, _) in PORTALS {
            let found = (x - 1..=x + 1).any(|a| (z - 1..=z + 1).any(|b| (a - x).pow(2) + (b - z).pow(2) <= 1 && (20..50).any(|y| w.get(a, y, b) == PORTAL)));
            assert!(found, "Portal bei {x},{z}");
        }
        // Privatinsel: alles da, was die Hütte und die Rohstoffinsel brauchen
        let (a, c) = (5 * SLOT, 0);
        let has = |b: u8| (a..a + SLOT).any(|x| (c..c + SLOT).any(|z| (GROUND..GROUND + 6).any(|y| w.get(x, y, z) == b)));
        for b in [PORTAL, TRUHE, WERKBANK, SOCKEL, GENERATOR, SPLITTER_GEN, BETT, REGAL, LATERNE, KNOTEN_EICHE] {
            assert!(has(b), "Block {b} fehlt auf der Privatinsel");
        }
        // Härte: Pflanze < Erde < Holz < Stein < Erz < Kristall
        assert!(hardness(LAUB) < hardness(ERDE) && hardness(ERDE) < hardness(STAMM) && hardness(STAMM) < hardness(FELS));
        assert!(hardness(FELS) < hardness(ERZ_KOHLE) && hardness(ERZ_KOHLE) < hardness(ADER_SAPHIR));
        // außerhalb jeder Insel: Luft bis ganz unten
        assert!((0..CH).all(|y| w.get(140, y, 200) == 0 && w.get(600, y, 10) == 0 && w.get(350, y, 300) == 0));
    }

    /// Abschnitt 8: Möbel und Flora dürfen keine NPCs einmauern, Altäre und Portale bleiben, wo der Server sie sucht.
    #[test]
    fn npcs_altaere_portale() {
        let w = Welt::new(WorldState::default());
        for n in crate::content::NPCS {
            let [x, y, z] = n.at.map(|v| v.floor() as i32);
            assert!(solid(w.get(x, y - 1, z)), "{} steht auf festem Boden", n.name);
            assert!(!solid(w.get(x, y, z)) && !solid(w.get(x, y + 1, z)), "{}: Füße und Kopf frei", n.name);
        }
        let at = |p: [i32; 3]| w.get(p[0], p[1], p[2]);
        assert_eq!((at(ALTAR_AT), at(BOSS_ALTAR_AT), at(KATA_PORTAL_AT), at(ARENA_PORTAL_AT)), (ALTAR, BOSS_ALTAR, KATA_PORTAL, ARENA_PORTAL));
        assert!(solid(at([ALTAR_AT[0], ALTAR_AT[1] - 1, ALTAR_AT[2]])), "Altar steht auf dem Podest");
        for k in 0..11 {
            assert_eq!(w.get(SHOWCASE[0] + 2 * k, SHOWCASE[1], SHOWCASE[2]), SOCKEL, "Minion-Sockel {k}");
        }
    }

    /// Dichte-Prüfung des Hub-Archipels (Pflicht): genug Masse, genug Bauwerke, Höhlen im Inselbauch, kaum nackte Wiese.
    #[test]
    fn hub_dichte() {
        let st = WorldState::default();
        let w = Welt::new(st);
        let n = HUB_EDGE / 16;
        let mut fest = 0usize;
        for cz in 0..n {
            for cx in 0..n {
                let c = gen_chunk(cx, cz, st);
                fest += c.iter().filter(|&&b| solid(b)).count();
                w.0.borrow_mut().insert((cx, cz), c);
            }
        }
        let [x0, z0, x1, z1] = HUB_MAIN;
        let (wd, dd) = (x1 - x0 + 2, z1 - z0 + 2);
        let tops: Vec<i32> = (z0 - 1..=z1).flat_map(|z| (x0 - 1..=x1).map(move |x| (x, z))).map(|(x, z)| w.top(x, z)).collect();
        let top = |x: i32, z: i32| tops[((z - z0 + 1) * wd + (x - x0 + 1)) as usize];
        assert_eq!(tops.len() as i32, wd * dd);
        let (mut hohl, mut wiese, mut spalten) = (0usize, 0usize, 0usize);
        for z in z0..z1 {
            for x in x0..x1 {
                let t = top(x, z);
                if t < 0 || crate::blueprints::surf(x, z, st).is_none() {
                    continue;
                }
                spalten += 1;
                if w.get(x, t, z) == WIESE && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dz)| top(x + dx, z + dz) == t) {
                    wiese += 1;
                }
                let col: Vec<u8> = (0..CH).map(|y| w.get(x, y, z)).collect();
                let lowest = col.iter().position(|&b| solid(b)).unwrap_or(col.len()) as i32;
                hohl += (lowest + 1..22).filter(|&y| col[y as usize] == 0 && (y + 1..=t).any(|u| solid(col[u as usize]))).count();
            }
        }
        let marks = crate::blueprints::hub_landmarks();
        for &(name, [x, y, z]) in marks {
            let k = (x - 4..=x + 4).flat_map(|a| (z - 4..=z + 4).flat_map(move |b| (y + 1..=y + 8).map(move |c| (a, c, b)))).filter(|&(a, c, b)| solid(w.get(a, c, b))).count();
            assert!(k >= 12, "{name}: nur {k} feste Blöcke");
        }
        eprintln!(
            "HUB-DICHTE: {fest} feste Blöcke, {} Landmarken, {hohl} Höhlenluft-Blöcke unter y 22, nackte flache Wiese {wiese}/{spalten} = {:.1} %",
            marks.len(),
            100.0 * wiese as f64 / spalten.max(1) as f64
        );
        assert!(fest >= 35_000, "zu wenig Masse: {fest}");
        assert!(marks.len() >= 15);
        assert!(hohl >= 500, "Höhlen fehlen: {hohl}");
        assert!(spalten > 20_000 && wiese * 100 < spalten * 12, "zu viel nackte Wiese: {wiese}/{spalten}");
    }
}
