//! Pflanzen (Phase 19). Reine Daten; Regeln in farming.rs.
use crate::rpg::{CropDef, ERNTE};
use crate::world::{SAAT_WEIZEN, SAAT_KAROTTE, SAAT_BEERE, SAAT_KUERBIS, SAAT_KRISTALL};

pub static CROPS: [CropDef; 5] = [
    CropDef { seed: SAAT_WEIZEN, name: "Weizen", product: ERNTE + 0, grow_ms: 600_000, yield_min: 2, yield_max: 3, xp: 4, rgb: [200, 180, 70], rgb2: [235, 210, 100] },
    CropDef { seed: SAAT_KAROTTE, name: "Karotte", product: ERNTE + 1, grow_ms: 900_000, yield_min: 2, yield_max: 4, xp: 6, rgb: [80, 160, 60], rgb2: [240, 130, 40] },
    CropDef { seed: SAAT_BEERE, name: "Leucht-Beere", product: ERNTE + 2, grow_ms: 1_200_000, yield_min: 2, yield_max: 3, xp: 9, rgb: [60, 140, 90], rgb2: [120, 220, 255] },
    CropDef { seed: SAAT_KUERBIS, name: "Wissens-Kürbis", product: ERNTE + 3, grow_ms: 1_800_000, yield_min: 1, yield_max: 2, xp: 14, rgb: [70, 130, 60], rgb2: [235, 150, 40] },
    CropDef { seed: SAAT_KRISTALL, name: "Kristall-Blüte", product: ERNTE + 4, grow_ms: 3_600_000, yield_min: 1, yield_max: 1, xp: 25, rgb: [120, 90, 200], rgb2: [210, 170, 255] },
];
