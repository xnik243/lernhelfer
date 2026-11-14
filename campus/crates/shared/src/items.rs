//! Gegenstände: Blöcke (0..=255), Cosmetics (300..), Bauplan (900). Aussehen der Figur.
use crate::world::{block, BLOCKS, F_RARE};
use serde::{Deserialize, Serialize};

pub const BLUEPRINT: u16 = 900;

pub const SLOT_NAMES: [&str; 8] = ["Umhang", "Kopf", "Rücken", "Begleiter", "Spur", "Emote", "Namensschild", "Bau-Effekt"];
pub const CAPE: u8 = 0;
pub const HEAD: u8 = 1;
pub const BACK: u8 = 2;
pub const COMPANION: u8 = 3;
pub const TRAIL: u8 = 4;
pub const EMOTE: u8 = 5;
pub const NAMEPLATE: u8 = 6;
pub const BUILD_FX: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Earned,   // erlernt, nicht handelbar
    Shop,     // mit Talern gekauft, nicht handelbar
    Drop,     // aus Funden, handelbar
    Semester, // nur dieses Semester in Funden, handelbar
}

pub struct Cosmetic {
    pub id: u16,
    pub slot: u8,
    pub name: &'static str,
    pub path: Path,
    pub price: u32,
    pub shape: u8, // Form-Variante für den Baukasten im Client
    pub rgb: [u8; 3],
    pub rgb2: [u8; 3],
}

macro_rules! cosmetics {
    ($($c:ident = $id:literal, $slot:ident, $name:literal, $path:ident, $price:literal, $shape:literal, [$r:literal, $g:literal, $b:literal], [$r2:literal, $g2:literal, $b2:literal];)*) => {
        $(pub const $c: u16 = $id;)*
        pub const COSMETICS: &[Cosmetic] = &[$(Cosmetic { id: $id, slot: $slot, name: $name, path: Path::$path, price: $price, shape: $shape, rgb: [$r, $g, $b], rgb2: [$r2, $g2, $b2] }),*];
    };
}

// Formen je Platz – Umhang: 0 schlicht, 1 Saum, 2 Muster, 3 animiert. Kopf: 0 Hut, 1 Doktorhut, 2 Kopfhörer, 3 Heiligenschein, 4 Krone, 5 Zylinder, 6 Kranz.
// Rücken: 0 Rucksack, 1 Bücherstapel, 2 Flügel, 3 Libellenflügel. Begleiter: 0 Eule, 1 Kugel, 2 Fuchs, 3 Klecks.
// Spur: 0 Flamme, 1 Funken, 2 Blätter, 3 Tinte. Emote: 0 Winken, 1 Jubeln, 2 Sitzen, 3 Tanz. Schild: 0 Titel, 1 Rahmen. Bau-Effekt: 0 Konfetti, 1 Staub, 2 Sterne, 3 Glocke.
cosmetics! {
    CAPE_L5 = 300, CAPE, "Umhang der Stufe 5", Earned, 0, 0, [92, 110, 150], [92, 110, 150];
    CAPE_L10 = 301, CAPE, "Umhang der Stufe 10", Earned, 0, 1, [60, 90, 180], [230, 230, 240];
    CAPE_L20 = 302, CAPE, "Umhang der Stufe 20", Earned, 0, 2, [120, 60, 170], [240, 200, 80];
    CAPE_L30 = 303, CAPE, "Umhang der Stufe 30", Earned, 0, 3, [30, 30, 50], [250, 210, 90];
    CAPE_FACH_A = 304, CAPE, "Fach-Umhang Zinnober", Earned, 0, 2, [200, 70, 50], [250, 240, 230];
    CAPE_FACH_B = 305, CAPE, "Fach-Umhang Petrol", Earned, 0, 2, [30, 120, 130], [250, 240, 230];
    CAPE_FACH_C = 306, CAPE, "Fach-Umhang Ocker", Earned, 0, 2, [200, 150, 50], [60, 40, 30];
    CAPE_FACH_D = 307, CAPE, "Fach-Umhang Flieder", Earned, 0, 2, [150, 120, 200], [250, 240, 230];
    CAPE_ROT = 310, CAPE, "Umhang Mohnrot", Shop, 120, 0, [190, 50, 50], [190, 50, 50];
    CAPE_NACHT = 311, CAPE, "Umhang Nachtblau", Shop, 120, 1, [30, 40, 90], [200, 210, 240];
    CAPE_SEMESTER = 312, CAPE, "Semester-Umhang WS 26", Semester, 0, 2, [60, 90, 200], [255, 255, 255];
    CAPE_STERNE = 313, CAPE, "Sternenumhang", Drop, 0, 3, [20, 24, 60], [255, 240, 160];
    HEAD_STROH = 320, HEAD, "Strohhut", Shop, 60, 0, [226, 200, 120], [150, 60, 50];
    HEAD_DOKTOR = 321, HEAD, "Doktorhut", Shop, 150, 1, [30, 30, 36], [240, 200, 70];
    HEAD_HOERER = 322, HEAD, "Kopfhörer", Shop, 90, 2, [50, 54, 64], [90, 200, 220];
    HEAD_HALO = 323, HEAD, "Heiligenschein", Drop, 0, 3, [255, 236, 150], [255, 236, 150];
    HEAD_KRONE = 324, HEAD, "Krone der Woche", Earned, 0, 4, [246, 204, 70], [220, 60, 80];
    HEAD_ZYLINDER = 325, HEAD, "Zylinder", Drop, 0, 5, [34, 34, 40], [170, 40, 60];
    HEAD_KRANZ = 326, HEAD, "Blumenkranz", Shop, 70, 6, [240, 130, 160], [90, 160, 90];
    BACK_RUCKSACK = 330, BACK, "Rucksack", Shop, 80, 0, [70, 120, 90], [220, 180, 90];
    BACK_BUECHER = 331, BACK, "Bücherstapel", Shop, 100, 1, [170, 70, 60], [70, 100, 170];
    BACK_FLUEGEL = 332, BACK, "Federflügel", Drop, 0, 2, [244, 244, 250], [210, 220, 240];
    BACK_LIBELLE = 333, BACK, "Libellenflügel", Drop, 0, 3, [140, 230, 220], [200, 160, 250];
    BACK_TORNISTER = 334, BACK, "Semester-Tornister WS 26", Semester, 0, 0, [60, 90, 200], [255, 255, 255];
    PET_EULE = 340, COMPANION, "Eule", Shop, 200, 0, [150, 120, 90], [250, 220, 120];
    PET_KUGEL = 341, COMPANION, "Schwebekugel", Shop, 150, 1, [120, 220, 250], [255, 255, 255];
    PET_FUCHS = 342, COMPANION, "Fuchs", Drop, 0, 2, [226, 120, 50], [250, 240, 230];
    PET_KLECKS = 343, COMPANION, "Tintenklecks", Drop, 0, 3, [40, 50, 120], [120, 140, 250];
    TRAIL_FLAMME_1 = 350, TRAIL, "Flammenspur I", Earned, 0, 0, [250, 160, 60], [250, 220, 120];
    TRAIL_FLAMME_2 = 351, TRAIL, "Flammenspur II", Earned, 0, 0, [250, 110, 50], [255, 230, 140];
    TRAIL_FLAMME_3 = 352, TRAIL, "Flammenspur III", Earned, 0, 0, [120, 180, 255], [240, 250, 255];
    TRAIL_FUNKEN = 353, TRAIL, "Funken", Shop, 90, 1, [255, 240, 150], [255, 255, 255];
    TRAIL_BLAETTER = 354, TRAIL, "Blätter", Shop, 90, 2, [110, 170, 80], [220, 170, 70];
    TRAIL_TINTE = 355, TRAIL, "Tinte", Drop, 0, 3, [30, 40, 110], [80, 100, 220];
    EMOTE_WINKEN = 360, EMOTE, "Winken", Shop, 0, 0, [0, 0, 0], [0, 0, 0];
    EMOTE_JUBELN = 361, EMOTE, "Jubeln", Shop, 30, 1, [0, 0, 0], [0, 0, 0];
    EMOTE_SITZEN = 362, EMOTE, "Hinsetzen", Shop, 30, 2, [0, 0, 0], [0, 0, 0];
    EMOTE_TANZ = 363, EMOTE, "Tanz", Drop, 0, 3, [0, 0, 0], [0, 0, 0];
    PLATE_ERSTI = 370, NAMEPLATE, "Titel „Erstsemester“", Shop, 20, 0, [90, 160, 110], [255, 255, 255];
    PLATE_WURM = 371, NAMEPLATE, "Titel „Bücherwurm“", Shop, 80, 0, [150, 100, 60], [255, 255, 255];
    PLATE_GOLD = 372, NAMEPLATE, "Goldener Rahmen", Earned, 0, 1, [246, 204, 70], [60, 44, 10];
    PLATE_SILBER = 373, NAMEPLATE, "Silberner Rahmen", Shop, 100, 1, [200, 206, 216], [40, 44, 54];
    PLATE_EULE = 374, NAMEPLATE, "Titel „Nachteule“", Drop, 0, 0, [50, 50, 110], [230, 230, 255];
    PLATE_TUERKIS = 375, NAMEPLATE, "Schild Türkis", Shop, 50, 0, [40, 170, 170], [255, 255, 255];
    FX_KONFETTI = 380, BUILD_FX, "Konfetti", Shop, 80, 0, [250, 120, 160], [120, 200, 250];
    FX_KREIDE = 381, BUILD_FX, "Kreidestaub", Shop, 60, 1, [240, 240, 236], [200, 200, 196];
    FX_STERNE = 382, BUILD_FX, "Sternenstaub", Drop, 0, 2, [255, 240, 160], [160, 200, 255];
    FX_GLOCKE = 383, BUILD_FX, "Glockenton", Drop, 0, 3, [240, 200, 100], [255, 255, 255];
}

/// Titeltexte der Namensschilder (Form 0).
pub fn plate_title(id: u16) -> &'static str {
    match id {
        PLATE_ERSTI => "Erstsemester",
        PLATE_WURM => "Bücherwurm",
        PLATE_EULE => "Nachteule",
        _ => "",
    }
}

pub fn cosmetic(id: u16) -> Option<&'static Cosmetic> {
    COSMETICS.iter().find(|c| c.id == id)
}

/// Darf ins Auktionshaus: seltene Blöcke, Baupläne, gefundene Cosmetics.
pub fn tradeable(item: u16) -> bool {
    if crate::skyblock::name(item).is_some() {
        return true;
    }
    if item == BLUEPRINT {
        return true;
    }
    if item < 256 {
        return block(item as u8).is_some_and(|b| b.flags & F_RARE != 0);
    }
    cosmetic(item).is_some_and(|c| matches!(c.path, Path::Drop | Path::Semester))
}

pub fn exists(item: u16) -> bool {
    item == BLUEPRINT || (item < 256 && item != 0 && block(item as u8).is_some()) || cosmetic(item).is_some() || crate::skyblock::name(item).is_some()
}

pub fn item_name(item: u16) -> &'static str {
    if item == BLUEPRINT {
        return "Bauplan";
    }
    if let Some(n) = crate::skyblock::name(item) {
        return n;
    }
    if item < 256 {
        return block(item as u8).map_or("?", |b| b.name);
    }
    cosmetic(item).map_or("?", |c| c.name)
}

/// Richtwert in Talern (für den Laden-Vergleich und die Auffälligkeits-Prüfung).
pub fn item_value(item: u16) -> u64 {
    if item == BLUEPRINT {
        return 100;
    }
    if item < 256 {
        return block(item as u8).map_or(1, |b| if b.flags & F_RARE != 0 { 20 } else { b.price.max(1) as u64 });
    }
    cosmetic(item).map_or(1, |c| match c.path {
        Path::Shop => c.price.max(1) as u64,
        Path::Drop => 250,
        Path::Semester => 400,
        Path::Earned => 0,
    })
}

pub fn standard_blocks() -> Vec<u8> {
    BLOCKS.iter().filter(|b| b.price > 0 && b.flags & crate::world::F_WEEKLY == 0).map(|b| b.id).collect()
}

pub const SKINS: [[u8; 3]; 6] = [[255, 224, 196], [240, 200, 160], [214, 164, 120], [176, 122, 84], [128, 84, 58], [88, 58, 42]];
pub const PALETTE: [[u8; 3]; 12] = [
    [40, 44, 56], [236, 236, 230], [190, 60, 60], [226, 140, 50], [238, 200, 70], [90, 170, 90],
    [50, 150, 160], [60, 100, 200], [130, 90, 190], [220, 120, 170], [130, 96, 66], [120, 128, 140],
];
pub const HAIR_NAMES: [&str; 5] = ["Kurz", "Lang", "Zopf", "Locken", "Glatze"];
pub const OUTFIT_NAMES: [&str; 4] = ["Shirt", "Kapuzenpulli", "Robe", "Weste"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    pub skin: u8,
    pub hair: u8,
    pub hair_col: u8,
    pub outfit: u8,
    pub col_a: u8,
    pub col_b: u8,
    pub slots: [u16; 8],
}

impl Look {
    /// Begrenzt alle Werte auf gültige Bereiche (Eingabe vom Client).
    pub fn clamped(mut self) -> Self {
        self.skin %= SKINS.len() as u8;
        self.hair %= HAIR_NAMES.len() as u8;
        self.hair_col %= PALETTE.len() as u8;
        self.outfit %= OUTFIT_NAMES.len() as u8;
        self.col_a %= PALETTE.len() as u8;
        self.col_b %= PALETTE.len() as u8;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::*;

    #[test]
    fn register() {
        let mut ids: Vec<u16> = COSMETICS.iter().map(|c| c.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), COSMETICS.len());
        for s in 0..8 {
            assert!(COSMETICS.iter().any(|c| c.slot == s), "Platz {s} hat Stücke");
        }
        assert!(tradeable(GLUEH as u16) && tradeable(BLUEPRINT) && tradeable(HEAD_HALO) && tradeable(CAPE_SEMESTER));
        assert!(!tradeable(BRETTER as u16) && !tradeable(CAPE_L5) && !tradeable(HEAD_STROH) && !tradeable(0) && !tradeable(299));
        assert!(standard_blocks().iter().all(|&b| (1..=5).contains(&block(b).unwrap().price)));
        assert_eq!(BLOCKS.iter().filter(|b| b.flags & F_WEEKLY != 0).count(), 8);
    }
}
