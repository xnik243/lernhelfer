//! Werte-Rechnung (Phase 17): Grundwerte + Fähigkeits-Stufen + 4 Rüstungsteile + 3 Talismane + gehaltene Waffe/Werkzeug
//! + Verzauberungen. Server und Browser rechnen dieselbe Funktion; entscheiden tut der Server.
use crate::enchanting::{self as en, Levels};
use crate::inventory::GEAR_SLOTS;
use crate::rpg::{self, B0};
use crate::skyblock as sb;
use serde::{Deserialize, Serialize};

pub const BASE_HP: f32 = 100.0;
pub const BASE_CRIT: f32 = 10.0;
pub const BASE_CRIT_DMG: f32 = 50.0;
pub const FIST_DMG: f32 = 5.0;
pub const NO_SET: u8 = 255;
// Set-Nummern (equipment_data::SETS), deren 4/4-Bonus auch Verhalten ändert
pub const SET_BERGARBEITER: u8 = 0; // Eile auf Stein/Erz (+ Abbau-Tempo im Bonus)
pub const SET_BAUERN: u8 = 1; // doppeltes Wachstum auf eigener Insel
pub const SET_MATHE: u8 = 2;
pub const SET_SPRACH: u8 = 3;
pub const SET_NATUR: u8 = 4; // Lebens-Regeneration
pub const SET_ZITADELLE: u8 = 5; // goldene Aura

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub max_hp: f32,
    pub defense: f32,
    pub strength: f32,
    pub crit_chance: f32,
    pub crit_damage: f32,
    pub mining_speed: f32,
    pub fortune: f32,
    pub crop_fortune: f32,
    pub max_mana: f32,
    pub mana_regen: f32,
    pub hp_regen: f32,
    pub taler_bonus: f32,
    pub mana_cost: f32,
    /// Grundschaden der gehaltenen Waffe (0 = Faust), Schärfe schon eingerechnet
    pub weapon_damage: f32,
    pub telekinese: bool,
    /// volles Rüstungsset (0..6) oder NO_SET
    pub set: u8,
}

/// Schadensminderung aus Verteidigung: D / (D + 100).
pub fn reduction(def: f32) -> f32 {
    let d = def.max(0.0);
    d / (d + 100.0)
}
/// Volles Set (alle 4 Rüstungsplätze aus demselben Set)?
pub fn full_set(gear: &[u16; GEAR_SLOTS]) -> Option<u8> {
    let s = rpg::equip(gear[0])?.set;
    (s != NO_SET && gear[..4].iter().all(|&g| rpg::equip(g).is_some_and(|e| e.set == s))).then_some(s)
}

/// Alle Werte. `ench(item)` liefert die Verzauberungs-Stufen, die der Spieler auf dieser Gegenstands-Art hat.
pub fn compute(levels: &[u32; 7], gear: &[u16; GEAR_SLOTS], held: u16, ench: &dyn Fn(u16) -> Levels) -> Stats {
    let l = |i: usize| levels[i].max(1) as f32 - 1.0;
    let mut b = B0;
    // getragene Rüstung/Talismane und die gehaltene Waffe (gehaltene Rüstung zählt nicht)
    let worn = gear.iter().copied().filter(|&g| !rpg::is_weapon(g));
    for g in worn.chain(std::iter::once(held).filter(|&h| rpg::is_weapon(h))) {
        if let Some(e) = rpg::equip(g) {
            b.add(&e.bonus);
        }
    }
    let set = full_set(gear);
    if let Some(s) = set.and_then(rpg::set_def) {
        b.add(&s.full);
    }
    // Verzauberungen: Rüstung/Talismane getragen, Werkzeug/Waffe gehalten
    let mut tele = false;
    for &g in gear.iter().filter(|&&g| g != 0) {
        let e = ench(g);
        b.def += en::effect(en::SCHUTZ, e[en::SCHUTZ as usize]);
        b.mana += en::effect(en::WEISHEIT, e[en::WEISHEIT as usize]);
    }
    let mut weapon = b.dmg;
    if held != 0 {
        let e = ench(held);
        b.mining += en::effect(en::EFFIZIENZ, e[en::EFFIZIENZ as usize]);
        b.fortune += en::effect(en::GLUECK, e[en::GLUECK as usize]);
        weapon *= 1.0 + en::effect(en::SCHAERFE, e[en::SCHAERFE as usize]) / 100.0;
        tele = e[en::TELEKINESE as usize] > 0;
    }
    Stats {
        max_hp: BASE_HP + 2.0 * l(2) + b.hp,
        defense: l(3) + b.def,
        strength: 0.5 * l(1) + b.str_,
        crit_chance: (BASE_CRIT + b.crit).min(100.0),
        crit_damage: BASE_CRIT_DMG + 0.5 * l(1),
        mining_speed: b.mining,
        fortune: b.fortune,
        crop_fortune: b.crop + l(2) * 0.5,
        max_mana: sb::max_mana(levels) as f32 + b.mana,
        mana_regen: sb::MANA_REGEN_PER_S as f32 + b.regen,
        hp_regen: 0.5 + b.hp_regen,
        taler_bonus: b.taler,
        mana_cost: b.mana_cost.min(80.0),
        weapon_damage: weapon,
        telekinese: tele,
        set: set.unwrap_or(NO_SET),
    }
}

/// Mana-Kosten eines Werkzeugs nach Rabatt (Mathe-Saphir-Robe).
pub fn mana_cost(base: u32, s: &Stats) -> u32 {
    (base as f32 * (1.0 - s.mana_cost / 100.0)).round().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpg::armor_item;

    #[test]
    fn set_bonus_und_verzauberung() {
        let lv = [1u32; 7];
        let none = |_: u16| [0u8; en::N_ENCH];
        let bare = compute(&lv, &[0; GEAR_SLOTS], 0, &none);
        assert_eq!((bare.max_hp, bare.defense, bare.set), (BASE_HP, 0.0, NO_SET));
        // 3 von 4 Teilen: kein Set-Bonus; 4/4: Set-Bonus aktiv
        let mut g = [armor_item(4, 0), armor_item(4, 1), armor_item(4, 2), 0, 0, 0, 0];
        let three = compute(&lv, &g, 0, &none);
        assert_eq!(three.set, NO_SET);
        g[3] = armor_item(4, 3);
        let four = compute(&lv, &g, 0, &none);
        assert_eq!(four.set, SET_NATUR);
        let sd = rpg::set_def(SET_NATUR).unwrap();
        let pieces: f32 = (0..4).map(|p| rpg::equip(armor_item(4, p)).unwrap().bonus.hp).sum();
        assert!((four.max_hp - (BASE_HP + pieces + sd.full.hp)).abs() < 0.01, "Teile + Set-Bonus");
        assert!(four.max_hp > three.max_hp && four.defense > three.defense);
        // gemischtes Set zählt nicht
        g[3] = armor_item(0, 3);
        assert_eq!(compute(&lv, &g, 0, &none).set, NO_SET);
        // Schutz III auf jedem Teil: +3 × 4 × je-Stufe Verteidigung
        g[3] = armor_item(4, 3);
        let schutz = |_: u16| {
            let mut e = [0u8; en::N_ENCH];
            e[en::SCHUTZ as usize] = 3;
            e
        };
        let s3 = compute(&lv, &g, 0, &schutz);
        assert!((s3.defense - four.defense - 4.0 * en::effect(en::SCHUTZ, 3)).abs() < 0.01);
        // Effizienz V auf der gehaltenen Spitzhacke beschleunigt nur, wenn gehalten
        let eff = |i: u16| {
            let mut e = [0u8; en::N_ENCH];
            if i == sb::SPITZHACKE {
                e[en::EFFIZIENZ as usize] = 5;
            }
            e
        };
        assert!(compute(&lv, &[0; 7], sb::SPITZHACKE, &eff).mining_speed > compute(&lv, &[0; 7], 0, &eff).mining_speed);
        // Schärfe skaliert den Waffenschaden
        let sh = |_: u16| {
            let mut e = [0u8; en::N_ENCH];
            e[en::SCHAERFE as usize] = 5;
            e
        };
        assert!(compute(&lv, &[0; 7], rpg::HOLZ_KLINGE, &sh).weapon_damage > compute(&lv, &[0; 7], rpg::HOLZ_KLINGE, &none).weapon_damage);
        assert!((reduction(100.0) - 0.5).abs() < 1e-6 && reduction(0.0) == 0.0);
    }
}
