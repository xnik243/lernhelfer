//! RPG-Schicht (Phasen 16–20): Gegenstands-Nummern, Datentypen der Inhaltstabellen und kleine Helfer.
//! Die Tabellen selbst stehen in den *_data.rs-Dateien (reine Daten), die Regeln in inventory/stats/enchanting/farming/combat.
use crate::skyblock::Goal;

// ---------- Gegenstands-Nummern (u16) ----------
/// Rüstung: ARMOR + Set·4 + Teil (0 Helm, 1 Brustpanzer, 2 Beinschutz, 3 Stiefel), Sets 0..6.
pub const ARMOR: u16 = 2000;
pub const ARMOR_SETS: u8 = 6;
/// Talismane (Artefakte) 2040..2048.
pub const ARTIFACT: u16 = 2040;
pub const ARTIFACTS: u16 = 8;
pub const HOLZ_KLINGE: u16 = 2100;
pub const SAPHIR_KLINGE: u16 = 2101;
pub const ZITADELLEN_SCHWERT: u16 = 2102;
pub const FOKUS_BOGEN: u16 = 2103;
pub const ARKANER_STAB: u16 = 2104;
/// Werkzeuge der Landwirtschaft (stehen auch in skyblock::TOOLS): Hacke macht Ackerboden, Segen pflanzt nach der Ernte neu.
pub const ERNTE_HACKE: u16 = 2105;
pub const SEGEN_HACKE: u16 = 2106;
/// Ernte: ERNTE + Pflanze (Index in world::SAATEN / farming_data::CROPS).
pub const ERNTE: u16 = 2300;
pub const ZITADELLEN_FRAGMENT: u16 = 2400;
pub const VERZ_BUCH: u16 = 2401;
pub const ARCHON_KERN: u16 = 2402;
pub const SCHLEIM: u16 = 2410;
pub const TINTE: u16 = 2411;
pub const WAECHTER_SIEGEL: u16 = 2412;

pub const fn armor_item(set: u8, piece: u8) -> u16 {
    ARMOR + set as u16 * 4 + piece as u16
}

/// Ausrüstungs-Platz eines Gegenstands (EquipDef.slot).
pub const SL_HELM: u8 = 0;
pub const SL_BRUST: u8 = 1;
pub const SL_BEINE: u8 = 2;
pub const SL_STIEFEL: u8 = 3;
pub const SL_TALISMAN: u8 = 4;
pub const SL_NAH: u8 = 5;
pub const SL_FERN: u8 = 6;
pub const RARITY_NAMES: [&str; 4] = ["Gewöhnlich", "Selten", "Episch", "Legendär"];

/// Feste Werte-Boni (alle additiv). `dmg` = Grundschaden einer Waffe.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bonus {
    pub hp: f32,
    pub def: f32,
    pub str_: f32,
    pub crit: f32,     // Prozentpunkte Krit-Chance
    pub mining: f32,   // Prozent Abbau-Tempo
    pub fortune: f32,  // Prozent Glück (Blöcke, Splitter)
    pub crop: f32,     // Prozent Ernte-Glück
    pub mana: f32,     // höchstes Fokus-Mana
    pub regen: f32,    // Mana je Sekunde
    pub hp_regen: f32, // Lebenspunkte je Sekunde
    pub taler: f32,    // Prozent mehr Taler (Bazaar-Verkauf)
    pub mana_cost: f32, // Prozent weniger Mana je Werkzeug
    pub dmg: f32,
}
pub const B0: Bonus = Bonus { hp: 0.0, def: 0.0, str_: 0.0, crit: 0.0, mining: 0.0, fortune: 0.0, crop: 0.0, mana: 0.0, regen: 0.0, hp_regen: 0.0, taler: 0.0, mana_cost: 0.0, dmg: 0.0 };
impl Bonus {
    pub fn add(&mut self, o: &Bonus) {
        self.hp += o.hp;
        self.def += o.def;
        self.str_ += o.str_;
        self.crit += o.crit;
        self.mining += o.mining;
        self.fortune += o.fortune;
        self.crop += o.crop;
        self.mana += o.mana;
        self.regen += o.regen;
        self.hp_regen += o.hp_regen;
        self.taler += o.taler;
        self.mana_cost += o.mana_cost;
        self.dmg += o.dmg;
    }
}

/// Rüstungsteil, Talisman oder Waffe.
pub struct EquipDef {
    pub id: u16,
    pub name: &'static str,
    pub slot: u8,   // SL_*
    pub set: u8,    // Rüstungsset 0..6, 255 = keins
    pub rarity: u8, // 0..4 (RARITY_NAMES)
    pub rgb: [u8; 3],
    pub bonus: Bonus,
    pub lore: &'static str,
    pub value: u32, // Verkaufswert in Talern
}
/// Rüstungsset: Name, Text des 4/4-Bonus und sein Wert.
pub struct SetDef {
    pub id: u8,
    pub name: &'static str,
    pub bonus_text: &'static str,
    pub full: Bonus,
}
/// Gegner der Prüfungs-Zitadelle. `ring`: 0 Außenring, 1 Innenhalle, 2 Boss, 3 Diener des Bosses.
pub struct MobDef {
    pub id: u8,
    pub name: &'static str,
    pub level: u8,
    pub hp: f32,
    pub dmg: f32,
    pub speed: f32,  // Blöcke/s
    pub reach: f32,  // Angriffsreichweite in Blöcken
    pub size: f32,   // Maßstab (1 = Spielergröße, Boss 3)
    pub rgb: [u8; 3],
    pub rgb2: [u8; 3],
    pub xp: u64,     // Kampf-EP (Fähigkeit Gesellschaft)
    pub taler: u32,
    /// (Gegenstand, Menge, Chance in Promille)
    pub drops: &'static [(u16, u32, u32)],
    pub ring: u8,
}
/// Verzauberung. `kinds`: Bitmaske 1 Werkzeug, 2 Waffe, 4 Rüstung, 8 Talisman. `fach` = Splitter-Fach der Kosten (0..5).
pub struct EnchDef {
    pub id: u8,
    pub name: &'static str,
    pub max: u8,
    pub kinds: u8,
    pub per_level: f32,
    pub fach: u8,
    pub desc: &'static str,
}
/// Pflanze. `seed` = Saat-Block (world::SAATEN), `product` = ERNTE + Index.
pub struct CropDef {
    pub seed: u8,
    pub name: &'static str,
    pub product: u16,
    pub grow_ms: i64,
    pub yield_min: u32,
    pub yield_max: u32,
    pub xp: u64, // Naturwissenschaften-EP je Ernte
    pub rgb: [u8; 3],  // Pflanze
    pub rgb2: [u8; 3], // Frucht
}
/// Kapitel des Campus-Questbuchs: Schritte (Ziel, Text ≤ 60 Zeichen), Belohnung.
pub struct Chapter {
    pub id: u8,
    pub name: &'static str,
    pub text: &'static str,
    pub steps: &'static [(Goal, &'static str)],
    pub taler: u32,
    pub shards: u32, // Wissens-Splitter (SHARD + 4)
}

// ---------- Nachschlagen ----------
pub fn equip(id: u16) -> Option<&'static EquipDef> {
    crate::equipment_data::EQUIP.iter().find(|e| e.id == id)
}
pub fn set_def(id: u8) -> Option<&'static SetDef> {
    crate::equipment_data::SETS.iter().find(|s| s.id == id)
}
pub fn mob(id: u8) -> Option<&'static MobDef> {
    crate::mobs_data::MOBS.iter().find(|m| m.id == id)
}
pub fn crop(i: usize) -> Option<&'static CropDef> {
    crate::farming_data::CROPS.get(i)
}
pub fn is_weapon(item: u16) -> bool {
    equip(item).is_some_and(|e| e.slot == SL_NAH || e.slot == SL_FERN)
}
pub fn is_ranged(item: u16) -> bool {
    equip(item).is_some_and(|e| e.slot == SL_FERN)
}

/// Seltenheit 0..3 für Rahmenfarbe und Tooltip.
pub fn rarity(item: u16) -> u8 {
    if let Some(e) = equip(item) {
        return e.rarity;
    }
    match item {
        ZITADELLEN_FRAGMENT | VERZ_BUCH | WAECHTER_SIEGEL => 2,
        ARCHON_KERN => 3,
        SEGEN_HACKE => 2,
        i if (crate::skyblock::SUPER..crate::skyblock::SUPER + 50).contains(&i) => 2,
        i if (crate::skyblock::VERDICHTET..crate::skyblock::VERDICHTET + 50).contains(&i) => 1,
        i if i < 256 && crate::world::FLAGS[i as usize] & crate::world::F_RARE != 0 => 1,
        i if i == ERNTE + 4 || i == crate::skyblock::FOKUS_KRISTALL => 1,
        _ => 0,
    }
}

/// Verkaufswert (Tooltip, NPC-Ankauf gibt es nicht – nur Richtwert).
pub fn sell_value(item: u16) -> u32 {
    if let Some(e) = equip(item) {
        return e.value;
    }
    match item {
        i if (ERNTE..ERNTE + 5).contains(&i) => [2, 3, 6, 12, 20][(i - ERNTE) as usize],
        SCHLEIM | TINTE => 4,
        WAECHTER_SIEGEL => 40,
        ZITADELLEN_FRAGMENT => 120,
        VERZ_BUCH => 200,
        ARCHON_KERN => 1500,
        i => crate::items::item_value(i).min(u32::MAX as u64) as u32,
    }
}

/// Namen der Phasen-16–20-Gegenstände (ohne Werkzeuge, die stehen in skyblock::TOOLS).
pub fn extra_names() -> Vec<(u16, &'static str)> {
    let mut v: Vec<(u16, &'static str)> = crate::equipment_data::EQUIP.iter().map(|e| (e.id, e.name)).collect();
    v.extend(crate::farming_data::CROPS.iter().map(|c| (c.product, c.name)));
    v.extend([
        (ZITADELLEN_FRAGMENT, "Zitadellen-Fragment"),
        (VERZ_BUCH, "Verzaubertes Buch"),
        (ARCHON_KERN, "Archon-Kern"),
        (SCHLEIM, "Chaos-Schleim"),
        (TINTE, "Tintenklumpen"),
        (WAECHTER_SIEGEL, "Wächter-Siegel"),
    ]);
    v
}
/// Kurzbeschreibung für den Tooltip (Lore) aller RPG-Gegenstände.
pub fn lore(item: u16) -> &'static str {
    if let Some(e) = equip(item) {
        return e.lore;
    }
    match item {
        ZITADELLEN_FRAGMENT => "Splitter der Prüfungs-Zitadelle. Grundstoff der Meisterrüstung.",
        VERZ_BUCH => "Gibt am Verzauberungs-Altar eine Stufe ohne Splitter-Kosten.",
        ARCHON_KERN => "Pulsierendes Herz des Großen Prüfungs-Archons.",
        SCHLEIM => "Klebt. Riecht nach durchgelernter Nacht.",
        TINTE => "Tropft von Tinten-Golems. Gut für Bücher.",
        WAECHTER_SIEGEL => "Siegel eines Klausur-Wächters.",
        ERNTE_HACKE => "Rechtsklick auf Wiese/Erde: Ackerboden.",
        SEGEN_HACKE => "Ernte-Segen: pflanzt nach der Ernte automatisch neu.",
        _ => "",
    }
}
