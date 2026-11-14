//! Verzaubern am Altar im Bibliotheks-Turm (Phase 17). Stufen I–V, bezahlt mit Wissens-Splittern und Fokus-Kristallen.
//! Verzauberungen hängen an (Spieler, Gegenstands-Art): „deine Spitzhacke hat Effizienz III“.
// ponytail: je Gegenstands-Art statt je Einzelstück, weil das Inventar Zählungen hält; Einzelstück-Verzauberung bräuchte Stück-Nummern.
use crate::enchants_data::ENCHANTS;
use crate::rpg::{self, EnchDef};

pub const N_ENCH: usize = 6;
pub const EFFIZIENZ: u8 = 0;
pub const GLUECK: u8 = 1;
pub const SCHAERFE: u8 = 2;
pub const SCHUTZ: u8 = 3;
pub const WEISHEIT: u8 = 4;
pub const TELEKINESE: u8 = 5;

pub const K_TOOL: u8 = 1;
pub const K_WEAPON: u8 = 2;
pub const K_ARMOR: u8 = 4;
pub const K_TALISMAN: u8 = 8;

pub type Levels = [u8; N_ENCH];

pub fn def(id: u8) -> Option<&'static EnchDef> {
    ENCHANTS.iter().find(|e| e.id == id)
}
/// Art des Gegenstands als Bitmaske (K_*), 0 = nicht verzauberbar.
pub fn kind(item: u16) -> u8 {
    use crate::skyblock::{AXT, HAMMER, SPITZHACKE};
    if [SPITZHACKE, AXT, HAMMER, rpg::ERNTE_HACKE, rpg::SEGEN_HACKE].contains(&item) {
        return K_TOOL;
    }
    match rpg::equip(item).map(|e| e.slot) {
        Some(rpg::SL_NAH) | Some(rpg::SL_FERN) => K_WEAPON,
        Some(rpg::SL_TALISMAN) => K_TALISMAN,
        Some(_) => K_ARMOR,
        None => 0,
    }
}
pub fn applies(ench: u8, item: u16) -> bool {
    def(ench).is_some_and(|d| d.kinds & kind(item) != 0)
}
/// Kosten für Stufe `level` (1..=max): (Splitter-Fach, Splitter, Fokus-Kristalle). Steigt quadratisch.
pub fn cost(ench: u8, level: u8) -> Option<(u8, u32, u32)> {
    let d = def(ench)?;
    (1..=d.max).contains(&level).then(|| (d.fach, 8 * (level as u32).pow(2), level as u32))
}
/// Wirkung einer Stufe in der Einheit der Verzauberung (Prozent bzw. Punkte), siehe EnchDef.per_level.
pub fn effect(ench: u8, level: u8) -> f32 {
    def(ench).map_or(0.0, |d| d.per_level * level as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stufen_und_kosten() {
        assert_eq!(ENCHANTS.len(), N_ENCH);
        for (i, e) in ENCHANTS.iter().enumerate() {
            assert_eq!(e.id as usize, i);
            assert!(e.max >= 1 && e.max <= 5 && e.fach < 5 && e.kinds != 0);
        }
        assert_eq!(def(TELEKINESE).unwrap().max, 1);
        assert!(applies(EFFIZIENZ, crate::skyblock::SPITZHACKE) && !applies(EFFIZIENZ, rpg::HOLZ_KLINGE));
        assert!(applies(SCHAERFE, rpg::HOLZ_KLINGE) && applies(SCHUTZ, rpg::armor_item(0, 1)) && !applies(SCHUTZ, crate::world::BRUCHSTEIN as u16));
        assert!(cost(EFFIZIENZ, 5).unwrap().1 > cost(EFFIZIENZ, 1).unwrap().1);
        assert!(cost(EFFIZIENZ, 6).is_none() && cost(TELEKINESE, 2).is_none());
    }
}
