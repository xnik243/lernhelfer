//! Ackerbau ohne Takt (Phase 19): eine Pflanze speichert nur (Art, Pflanzzeit, Wachstums-Faktor). Die Stufe 0..3 rechnen
//! Server und Browser aus der Zeit – der Server tut dafür im Hintergrund nichts.
use crate::farming_data::CROPS;
use crate::world::SAATEN;

pub const STAGES: u8 = 4;
/// Wachstums-Faktor in Promille: 1000 normal, 2000 mit voller Bauern-Tracht auf der eigenen Insel.
pub const FERT_BASE: u32 = 1000;
pub const FERT_BAUERN: u32 = 2000;

/// Pflanzen-Index (in CROPS) eines Saat-Blocks.
pub fn crop_of(block: u8) -> Option<usize> {
    SAATEN.iter().position(|&s| s == block)
}
/// Wachstumsdauer bis Stufe 3 in ms nach Faktor.
pub fn grow_ms(crop: usize, fert: u32) -> i64 {
    CROPS.get(crop).map_or(i64::MAX, |c| c.grow_ms * 1000 / fert.max(1) as i64)
}
/// Stufe 0..=3 aus der Zeit: Anteil = (jetzt - gepflanzt) / Dauer; Stufe 3 erst bei 100 %.
pub fn stage(planted: i64, now: i64, grow: i64) -> u8 {
    if grow <= 0 {
        return STAGES - 1;
    }
    let el = (now - planted).max(0);
    if el >= grow {
        return STAGES - 1;
    }
    ((el * 3) / grow).min(2) as u8
}
/// Erntemenge bei Stufe 3: zufällig zwischen min und max, dazu Ernte-Glück (je 100 % ein sicheres Stück mehr, Rest als Chance).
pub fn harvest(crop: usize, crop_fortune: f32, roll: u32) -> u32 {
    let Some(c) = CROPS.get(crop) else { return 0 };
    let span = c.yield_max.saturating_sub(c.yield_min) + 1;
    let base = c.yield_min + roll % span;
    let f = crop_fortune.max(0.0);
    let chance = ((roll / span) % 100) as f32;
    let extra = (f / 100.0).floor() as u32 + u32::from(chance < f % 100.0);
    base + extra
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stufen_aus_der_zeit() {
        assert_eq!(CROPS.len(), SAATEN.len());
        for (i, c) in CROPS.iter().enumerate() {
            assert_eq!(c.seed, SAATEN[i]);
            assert_eq!(c.product, crate::rpg::ERNTE + i as u16);
            assert!(c.yield_min >= 1 && c.yield_max >= c.yield_min && c.grow_ms >= 60_000);
        }
        // Weizen: 600 s bis reif. Ohne einen einzigen Takt: nur Zeitstempel.
        let g = grow_ms(0, FERT_BASE);
        assert_eq!(g, 600_000);
        let t0 = 1_000_000;
        assert_eq!(stage(t0, t0, g), 0);
        assert_eq!(stage(t0, t0 + 200_000, g), 1);
        assert_eq!(stage(t0, t0 + 400_000, g), 2);
        assert_eq!(stage(t0, t0 + 599_999, g), 2);
        assert_eq!(stage(t0, t0 + 600_000, g), 3);
        assert_eq!(stage(t0, t0 + 10 * g, g), 3);
        assert_eq!(stage(t0, t0 - 50, g), 0, "Uhr rückwärts: nie negativ");
        // Bauern-Tracht: doppelt so schnell
        assert_eq!(grow_ms(0, FERT_BAUERN), 300_000);
        assert_eq!(stage(t0, t0 + 300_000, grow_ms(0, FERT_BAUERN)), 3);
        assert_eq!(crop_of(SAATEN[2]), Some(2));
        let c = &CROPS[0];
        for r in 0..500 {
            let n = harvest(0, 0.0, r);
            assert!((c.yield_min..=c.yield_max).contains(&n));
            assert!(harvest(0, 250.0, r) >= n + 2);
        }
    }
}
