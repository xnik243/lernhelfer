//! Ausrüstungs-Daten (Phase 17): 6 Rüstungssets, 8 Talismane, 5 Waffen und Rezepte. Reine Daten.
use crate::rpg::*;
use crate::skyblock::{Recipe, FOKUS_KRISTALL, SHARD, SUPER, VERDICHTET};
use crate::world::{
    BRETTER, BRUCHSTEIN, EISEN, GLAS, GROSSE_TRUHE, KOHLE, KUPFER, LAUB, ROHGOLD, SAAT_BEERE, SAAT_KAROTTE, SAAT_KRISTALL,
    SAAT_KUERBIS, SAAT_WEIZEN, SAPHIR, SCHILD, SPRUNGPLATTE, STAENDER, STAMM, TELE_BLAU, TELE_GOLD, TELE_GRUEN, TELE_ROT, TRUHE,
};

pub static SETS: [SetDef; 6] = [
    SetDef { id: 0, name: "Bergarbeiter-Rüstung", bonus_text: "Eile: +25 % Abbau-Tempo auf Stein und Erz", full: Bonus { mining: 25.0, ..B0 } },
    SetDef { id: 1, name: "Bauern-Tracht", bonus_text: "Doppeltes Pflanzenwachstum auf der eigenen Insel", full: Bonus { crop: 10.0, ..B0 } },
    SetDef { id: 2, name: "Mathe-Saphir-Robe", bonus_text: "-20 % Mana-Kosten aller Werkzeuge", full: Bonus { mana_cost: 20.0, ..B0 } },
    SetDef { id: 3, name: "Sprach-Gold-Rüstung", bonus_text: "+10 % Krit-Chance, +5 % Taler im Bazaar", full: Bonus { crit: 10.0, taler: 5.0, ..B0 } },
    SetDef { id: 4, name: "Natur-Smaragd-Panzer", bonus_text: "Regeneration: +3 Leben je Sekunde", full: Bonus { hp_regen: 3.0, hp: 40.0, ..B0 } },
    SetDef { id: 5, name: "Zitadellen-Meisterrüstung", bonus_text: "Goldene Aura: +50 Stärke, +100 Leben", full: Bonus { str_: 50.0, hp: 100.0, ..B0 } },
];

pub static EQUIP: &[EquipDef] = &[
    // ---------- Bergarbeiter (Set 0) ----------
    EquipDef { id: armor_item(0, 0), name: "Bergarbeiter-Helm", slot: SL_HELM, set: 0, rarity: 0, rgb: [120, 100, 80], bonus: Bonus { mining: 8.75, def: 5.0, ..B0 }, lore: "Beule vom Stollen-Balken. Hält den Kopf im Schacht frei.", value: 40 },
    EquipDef { id: armor_item(0, 1), name: "Bergarbeiter-Brustpanzer", slot: SL_BRUST, set: 0, rarity: 0, rgb: [120, 100, 80], bonus: Bonus { mining: 8.75, def: 5.0, ..B0 }, lore: "Dicker Stoff, verstärkt mit Bruchstein-Platten.", value: 40 },
    EquipDef { id: armor_item(0, 2), name: "Bergarbeiter-Beinschutz", slot: SL_BEINE, set: 0, rarity: 0, rgb: [120, 100, 80], bonus: Bonus { mining: 8.75, def: 5.0, ..B0 }, lore: "Schützt die Knie beim Kriechen durch enge Gänge.", value: 40 },
    EquipDef { id: armor_item(0, 3), name: "Bergarbeiter-Stiefel", slot: SL_STIEFEL, set: 0, rarity: 0, rgb: [120, 100, 80], bonus: Bonus { mining: 8.75, def: 5.0, ..B0 }, lore: "Schwere Sohlen für rutschige Stollen.", value: 40 },
    // ---------- Bauern (Set 1) ----------
    EquipDef { id: armor_item(1, 0), name: "Bauern-Helm", slot: SL_HELM, set: 1, rarity: 0, rgb: [200, 170, 90], bonus: Bonus { crop: 6.25, hp: 5.0, ..B0 }, lore: "Strohhut mit Wachstumsschutz über dem Beet.", value: 40 },
    EquipDef { id: armor_item(1, 1), name: "Bauern-Brustpanzer", slot: SL_BRUST, set: 1, rarity: 0, rgb: [200, 170, 90], bonus: Bonus { crop: 6.25, hp: 5.0, ..B0 }, lore: "Leinen, das nach frischer Erde riecht.", value: 40 },
    EquipDef { id: armor_item(1, 2), name: "Bauern-Beinschutz", slot: SL_BEINE, set: 1, rarity: 0, rgb: [200, 170, 90], bonus: Bonus { crop: 6.25, hp: 5.0, ..B0 }, lore: "Gummierte Hosen für lange Tage auf dem Feld.", value: 40 },
    EquipDef { id: armor_item(1, 3), name: "Bauern-Stiefel", slot: SL_STIEFEL, set: 1, rarity: 0, rgb: [200, 170, 90], bonus: Bonus { crop: 6.25, hp: 5.0, ..B0 }, lore: "Gummistiefel, die jede Furche überstehen.", value: 40 },
    // ---------- Mathe-Saphir (Set 2) ----------
    EquipDef { id: armor_item(2, 0), name: "Mathe-Saphir-Helm", slot: SL_HELM, set: 2, rarity: 1, rgb: [70, 110, 230], bonus: Bonus { mana: 30.0, def: 4.0, ..B0 }, lore: "Ein Reif aus Saphir, der Zahlen flüstert.", value: 150 },
    EquipDef { id: armor_item(2, 1), name: "Mathe-Saphir-Brustpanzer", slot: SL_BRUST, set: 2, rarity: 1, rgb: [70, 110, 230], bonus: Bonus { mana: 30.0, def: 4.0, ..B0 }, lore: "Robe mit eingewebten Formeln in Blau.", value: 150 },
    EquipDef { id: armor_item(2, 2), name: "Mathe-Saphir-Beinschutz", slot: SL_BEINE, set: 2, rarity: 1, rgb: [70, 110, 230], bonus: Bonus { mana: 30.0, def: 4.0, ..B0 }, lore: "Schlichte Hose, die Zahlen nicht stört.", value: 150 },
    EquipDef { id: armor_item(2, 3), name: "Mathe-Saphir-Stiefel", slot: SL_STIEFEL, set: 2, rarity: 1, rgb: [70, 110, 230], bonus: Bonus { mana: 30.0, def: 4.0, ..B0 }, lore: "Leise Sohlen für lange Nächte am Rechenblatt.", value: 150 },
    // ---------- Sprach-Gold (Set 3) ----------
    EquipDef { id: armor_item(3, 0), name: "Sprach-Gold-Helm", slot: SL_HELM, set: 3, rarity: 1, rgb: [236, 190, 60], bonus: Bonus { taler: 3.75, hp: 20.0, def: 12.5, ..B0 }, lore: "Goldener Reif mit Wörtern im Muster.", value: 150 },
    EquipDef { id: armor_item(3, 1), name: "Sprach-Gold-Brustpanzer", slot: SL_BRUST, set: 3, rarity: 1, rgb: [236, 190, 60], bonus: Bonus { taler: 3.75, hp: 20.0, def: 12.5, ..B0 }, lore: "Brustplatte aus gehämmertem Sprachgold.", value: 150 },
    EquipDef { id: armor_item(3, 2), name: "Sprach-Gold-Beinschutz", slot: SL_BEINE, set: 3, rarity: 1, rgb: [236, 190, 60], bonus: Bonus { taler: 3.75, hp: 20.0, def: 12.5, ..B0 }, lore: "Beinschienen mit Vokabeln als Gravur.", value: 150 },
    EquipDef { id: armor_item(3, 3), name: "Sprach-Gold-Stiefel", slot: SL_STIEFEL, set: 3, rarity: 1, rgb: [236, 190, 60], bonus: Bonus { taler: 3.75, hp: 20.0, def: 12.5, ..B0 }, lore: "Stiefel, die bei jedem Satz ein wenig glänzen.", value: 150 },
    // ---------- Natur-Smaragd (Set 4) ----------
    EquipDef { id: armor_item(4, 0), name: "Natur-Smaragd-Helm", slot: SL_HELM, set: 4, rarity: 2, rgb: [60, 200, 120], bonus: Bonus { hp: 45.0, def: 30.0, ..B0 }, lore: "Helm mit Blättern aus grünem Glas.", value: 400 },
    EquipDef { id: armor_item(4, 1), name: "Natur-Smaragd-Brustpanzer", slot: SL_BRUST, set: 4, rarity: 2, rgb: [60, 200, 120], bonus: Bonus { hp: 45.0, def: 30.0, ..B0 }, lore: "Panzer, in dem der Wald weiterwächst.", value: 400 },
    EquipDef { id: armor_item(4, 2), name: "Natur-Smaragd-Beinschutz", slot: SL_BEINE, set: 4, rarity: 2, rgb: [60, 200, 120], bonus: Bonus { hp: 45.0, def: 30.0, ..B0 }, lore: "Beinschutz aus Moos und Smaragd.", value: 400 },
    EquipDef { id: armor_item(4, 3), name: "Natur-Smaragd-Stiefel", slot: SL_STIEFEL, set: 4, rarity: 2, rgb: [60, 200, 120], bonus: Bonus { hp: 45.0, def: 30.0, ..B0 }, lore: "Stiefel mit Wurzeln, die den Boden spüren.", value: 400 },
    // ---------- Zitadellen-Meister (Set 5) ----------
    EquipDef { id: armor_item(5, 0), name: "Zitadellen-Meister-Helm", slot: SL_HELM, set: 5, rarity: 3, rgb: [250, 215, 90], bonus: Bonus { hp: 100.0, def: 75.0, str_: 15.0, crit: 4.0, mining: 5.0, mana: 25.0, ..B0 }, lore: "Krone aus Fragmenten der Prüfungs-Zitadelle.", value: 3000 },
    EquipDef { id: armor_item(5, 1), name: "Zitadellen-Meister-Brustpanzer", slot: SL_BRUST, set: 5, rarity: 3, rgb: [250, 215, 90], bonus: Bonus { hp: 100.0, def: 75.0, str_: 15.0, crit: 4.0, mining: 5.0, mana: 25.0, ..B0 }, lore: "Panzer, geschmiedet im Herzen der Zitadelle.", value: 3000 },
    EquipDef { id: armor_item(5, 2), name: "Zitadellen-Meister-Beinschutz", slot: SL_BEINE, set: 5, rarity: 3, rgb: [250, 215, 90], bonus: Bonus { hp: 100.0, def: 75.0, str_: 15.0, crit: 4.0, mining: 5.0, mana: 25.0, ..B0 }, lore: "Schienen aus gehärtetem Zitadellenstahl.", value: 3000 },
    EquipDef { id: armor_item(5, 3), name: "Zitadellen-Meister-Stiefel", slot: SL_STIEFEL, set: 5, rarity: 3, rgb: [250, 215, 90], bonus: Bonus { hp: 100.0, def: 75.0, str_: 15.0, crit: 4.0, mining: 5.0, mana: 25.0, ..B0 }, lore: "Schritte, die im Archon-Saal widerhallen.", value: 3000 },
    // ---------- Talismane ----------
    EquipDef { id: ARTIFACT, name: "Talisman der Ausdauer", slot: SL_TALISMAN, set: 255, rarity: 0, rgb: [230, 80, 80], bonus: Bonus { hp: 20.0, ..B0 }, lore: "Hält den Atem länger, auch nach der dritten Prüfung.", value: 30 },
    EquipDef { id: ARTIFACT + 1, name: "Talisman des Schutzes", slot: SL_TALISMAN, set: 255, rarity: 0, rgb: [120, 160, 220], bonus: Bonus { def: 15.0, ..B0 }, lore: "Ein Stein, der bei jedem Stoß ein bisschen mehr hält.", value: 60 },
    EquipDef { id: ARTIFACT + 2, name: "Talisman der Stärke", slot: SL_TALISMAN, set: 255, rarity: 1, rgb: [240, 120, 60], bonus: Bonus { str_: 8.0, ..B0 }, lore: "Macht die Hände fester beim Tragen schwerer Bücher.", value: 120 },
    EquipDef { id: ARTIFACT + 3, name: "Glücksklee-Talisman", slot: SL_TALISMAN, set: 255, rarity: 1, rgb: [90, 200, 90], bonus: Bonus { fortune: 10.0, ..B0 }, lore: "Vier Blätter, eines davon ist für dich.", value: 150 },
    EquipDef { id: ARTIFACT + 4, name: "Talisman des Gärtners", slot: SL_TALISMAN, set: 255, rarity: 1, rgb: [150, 210, 80], bonus: Bonus { crop: 10.0, ..B0 }, lore: "Ein Samen, der nie ganz verdorrt.", value: 200 },
    EquipDef { id: ARTIFACT + 5, name: "Fokus-Talisman", slot: SL_TALISMAN, set: 255, rarity: 1, rgb: [180, 120, 255], bonus: Bonus { mana: 40.0, regen: 1.0, ..B0 }, lore: "Ein Kristall, der die Gedanken sammelt.", value: 300 },
    EquipDef { id: ARTIFACT + 6, name: "Krit-Talisman", slot: SL_TALISMAN, set: 255, rarity: 2, rgb: [255, 90, 160], bonus: Bonus { crit: 5.0, ..B0 }, lore: "Der Funke vor dem Schlag.", value: 450 },
    EquipDef { id: ARTIFACT + 7, name: "Archon-Siegel", slot: SL_TALISMAN, set: 255, rarity: 3, rgb: [255, 215, 0], bonus: Bonus { hp: 50.0, def: 30.0, str_: 15.0, ..B0 }, lore: "Das Siegel des Archons, unbeugsam und rein.", value: 800 },
    // ---------- Katakomben-Artefakte (Phase 24, nur als Beute) ----------
    EquipDef { id: SCHATTEN_LATERNE, name: "Schatten-Laterne", slot: SL_TALISMAN, set: 255, rarity: 2, rgb: [150, 90, 230], bonus: Bonus { crit: 4.0, mana: 30.0, ..B0 }, lore: "Leuchtet nur dort, wo es wirklich dunkel ist.", value: 900 },
    EquipDef { id: RUNEN_HERZ, name: "Runen-Herz", slot: SL_TALISMAN, set: 255, rarity: 3, rgb: [120, 220, 255], bonus: Bonus { hp: 60.0, str_: 20.0, hp_regen: 1.0, ..B0 }, lore: "Schlägt im Takt der fünf Runen.", value: 1600 },
    // ---------- Waffen ----------
    EquipDef { id: HOLZ_KLINGE, name: "Holz-Klinge", slot: SL_NAH, set: 255, rarity: 0, rgb: [150, 110, 70], bonus: Bonus { dmg: 20.0, ..B0 }, lore: "Eine Klinge aus Brettern. Besser als nichts.", value: 15 },
    EquipDef { id: SAPHIR_KLINGE, name: "Saphir-Klinge", slot: SL_NAH, set: 255, rarity: 1, rgb: [80, 140, 240], bonus: Bonus { dmg: 45.0, str_: 10.0, ..B0 }, lore: "Blau geschliffen, scharf wie eine Formel.", value: 150 },
    EquipDef { id: ZITADELLEN_SCHWERT, name: "Zitadellen-Schwert", slot: SL_NAH, set: 255, rarity: 3, rgb: [250, 215, 90], bonus: Bonus { dmg: 110.0, str_: 40.0, crit: 10.0, ..B0 }, lore: "Geschmiedet aus Fragmenten der Zitadelle.", value: 4000 },
    EquipDef { id: FOKUS_BOGEN, name: "Fokus-Bogen", slot: SL_FERN, set: 255, rarity: 1, rgb: [120, 220, 200], bonus: Bonus { dmg: 30.0, ..B0 }, lore: "Pfeile, die dem Blick folgen.", value: 120 },
    EquipDef { id: ARKANER_STAB, name: "Arkaner Prüfungs-Stab", slot: SL_FERN, set: 255, rarity: 2, rgb: [200, 120, 255], bonus: Bonus { dmg: 70.0, mana: 50.0, ..B0 }, lore: "Für die letzte Prüfung. Summt leise.", value: 900 },
];

pub static EQUIP_RECIPES: &[Recipe] = &[
    // ---------- Rüstung ----------
    Recipe { out: armor_item(0, 0), n: 1, ins: &[(BRUCHSTEIN as u16, 32), (EISEN as u16, 3), (KOHLE as u16, 4)], unlock: None },
    Recipe { out: armor_item(0, 1), n: 1, ins: &[(BRUCHSTEIN as u16, 32), (EISEN as u16, 3), (KOHLE as u16, 4)], unlock: None },
    Recipe { out: armor_item(0, 2), n: 1, ins: &[(BRUCHSTEIN as u16, 32), (EISEN as u16, 3), (KOHLE as u16, 4)], unlock: None },
    Recipe { out: armor_item(0, 3), n: 1, ins: &[(BRUCHSTEIN as u16, 32), (EISEN as u16, 3), (KOHLE as u16, 4)], unlock: None },
    Recipe { out: armor_item(1, 0), n: 1, ins: &[(ERNTE + 0, 16), (LAUB as u16, 12), (STAMM as u16, 8)], unlock: None },
    Recipe { out: armor_item(1, 1), n: 1, ins: &[(ERNTE + 0, 16), (LAUB as u16, 12), (STAMM as u16, 8)], unlock: None },
    Recipe { out: armor_item(1, 2), n: 1, ins: &[(ERNTE + 0, 16), (LAUB as u16, 12), (STAMM as u16, 8)], unlock: None },
    Recipe { out: armor_item(1, 3), n: 1, ins: &[(ERNTE + 0, 16), (LAUB as u16, 12), (STAMM as u16, 8)], unlock: None },
    Recipe { out: armor_item(2, 0), n: 1, ins: &[(VERDICHTET + 6, 3), (SHARD + 0, 20), (FOKUS_KRISTALL, 2)], unlock: None },
    Recipe { out: armor_item(2, 1), n: 1, ins: &[(VERDICHTET + 6, 3), (SHARD + 0, 20), (FOKUS_KRISTALL, 2)], unlock: None },
    Recipe { out: armor_item(2, 2), n: 1, ins: &[(VERDICHTET + 6, 3), (SHARD + 0, 20), (FOKUS_KRISTALL, 2)], unlock: None },
    Recipe { out: armor_item(2, 3), n: 1, ins: &[(VERDICHTET + 6, 3), (SHARD + 0, 20), (FOKUS_KRISTALL, 2)], unlock: None },
    Recipe { out: armor_item(3, 0), n: 1, ins: &[(VERDICHTET + 7, 3), (SHARD + 1, 20), (ROHGOLD as u16, 8)], unlock: None },
    Recipe { out: armor_item(3, 1), n: 1, ins: &[(VERDICHTET + 7, 3), (SHARD + 1, 20), (ROHGOLD as u16, 8)], unlock: None },
    Recipe { out: armor_item(3, 2), n: 1, ins: &[(VERDICHTET + 7, 3), (SHARD + 1, 20), (ROHGOLD as u16, 8)], unlock: None },
    Recipe { out: armor_item(3, 3), n: 1, ins: &[(VERDICHTET + 7, 3), (SHARD + 1, 20), (ROHGOLD as u16, 8)], unlock: None },
    Recipe { out: armor_item(4, 0), n: 1, ins: &[(VERDICHTET + 8, 5), (SHARD + 2, 32), (FOKUS_KRISTALL, 4)], unlock: None },
    Recipe { out: armor_item(4, 1), n: 1, ins: &[(VERDICHTET + 8, 5), (SHARD + 2, 32), (FOKUS_KRISTALL, 4)], unlock: None },
    Recipe { out: armor_item(4, 2), n: 1, ins: &[(VERDICHTET + 8, 5), (SHARD + 2, 32), (FOKUS_KRISTALL, 4)], unlock: None },
    Recipe { out: armor_item(4, 3), n: 1, ins: &[(VERDICHTET + 8, 5), (SHARD + 2, 32), (FOKUS_KRISTALL, 4)], unlock: None },
    Recipe { out: armor_item(5, 0), n: 1, ins: &[(ZITADELLEN_FRAGMENT, 12), (ARCHON_KERN, 1), (SUPER + 9, 1), (SUPER + 13, 1)], unlock: None },
    Recipe { out: armor_item(5, 1), n: 1, ins: &[(ZITADELLEN_FRAGMENT, 12), (ARCHON_KERN, 1), (SUPER + 9, 1), (SUPER + 13, 1)], unlock: None },
    Recipe { out: armor_item(5, 2), n: 1, ins: &[(ZITADELLEN_FRAGMENT, 12), (ARCHON_KERN, 1), (SUPER + 9, 1), (SUPER + 13, 1)], unlock: None },
    Recipe { out: armor_item(5, 3), n: 1, ins: &[(ZITADELLEN_FRAGMENT, 12), (ARCHON_KERN, 1), (SUPER + 9, 1), (SUPER + 13, 1)], unlock: None },
    // ---------- Talismane ----------
    Recipe { out: ARTIFACT + 0, n: 1, ins: &[(SHARD + 0, 16)], unlock: None },
    Recipe { out: ARTIFACT + 1, n: 1, ins: &[(SHARD + 1, 12)], unlock: None },
    Recipe { out: ARTIFACT + 2, n: 1, ins: &[(SHARD + 2, 16)], unlock: None },
    Recipe { out: ARTIFACT + 3, n: 1, ins: &[(ERNTE + 2, 6), (SHARD + 3, 8)], unlock: None },
    Recipe { out: ARTIFACT + 4, n: 1, ins: &[(ERNTE + 1, 8), (SHARD + 2, 8)], unlock: None },
    Recipe { out: ARTIFACT + 5, n: 1, ins: &[(FOKUS_KRISTALL, 2), (SHARD + 4, 16)], unlock: None },
    Recipe { out: ARTIFACT + 6, n: 1, ins: &[(FOKUS_KRISTALL, 2), (SHARD + 3, 24)], unlock: None },
    Recipe { out: ARTIFACT + 7, n: 1, ins: &[(ARCHON_KERN, 1), (WAECHTER_SIEGEL, 8)], unlock: None },
    // Katakomben-Artefakte: Beute der Beutetruhen oder aus Meister-Kernen geschmiedet
    Recipe { out: SCHATTEN_LATERNE, n: 1, ins: &[(MEISTER_KERN, 1), (SHARD + 4, 32)], unlock: None },
    Recipe { out: RUNEN_HERZ, n: 1, ins: &[(MEISTER_KERN, 3), (ARCHON_KERN, 1)], unlock: None },
    // ---------- Waffen ----------
    Recipe { out: HOLZ_KLINGE, n: 1, ins: &[(STAMM as u16, 6), (BRETTER as u16, 4)], unlock: None },
    Recipe { out: SAPHIR_KLINGE, n: 1, ins: &[(HOLZ_KLINGE, 1), (SAPHIR as u16, 8), (EISEN as u16, 4)], unlock: None },
    Recipe { out: ZITADELLEN_SCHWERT, n: 1, ins: &[(SAPHIR_KLINGE, 1), (ZITADELLEN_FRAGMENT, 12), (ARCHON_KERN, 1)], unlock: None },
    Recipe { out: FOKUS_BOGEN, n: 1, ins: &[(STAMM as u16, 6), (SCHLEIM, 6), (FOKUS_KRISTALL, 1)], unlock: None },
    Recipe { out: ARKANER_STAB, n: 1, ins: &[(FOKUS_BOGEN, 1), (WAECHTER_SIEGEL, 6), (SHARD + 4, 32)], unlock: None },
    // ---------- Extras: Truhen, Plattformen, Teleporter, Ständer, Saaten, Werkzeuge ----------
    Recipe { out: TRUHE as u16, n: 1, ins: &[(BRETTER as u16, 8)], unlock: None },
    Recipe { out: GROSSE_TRUHE as u16, n: 1, ins: &[(TRUHE as u16, 2), (EISEN as u16, 4), (VERDICHTET + 1, 1)], unlock: None },
    Recipe { out: SPRUNGPLATTE as u16, n: 1, ins: &[(SCHLEIM, 8), (EISEN as u16, 2), (KUPFER as u16, 4)], unlock: None },
    Recipe { out: TELE_ROT as u16, n: 2, ins: &[(FOKUS_KRISTALL, 2), (EISEN as u16, 4), (GLAS as u16, 2)], unlock: None },
    Recipe { out: TELE_BLAU as u16, n: 2, ins: &[(FOKUS_KRISTALL, 2), (EISEN as u16, 4), (GLAS as u16, 2)], unlock: None },
    Recipe { out: TELE_GRUEN as u16, n: 2, ins: &[(FOKUS_KRISTALL, 2), (EISEN as u16, 4), (GLAS as u16, 2)], unlock: None },
    Recipe { out: TELE_GOLD as u16, n: 2, ins: &[(FOKUS_KRISTALL, 2), (EISEN as u16, 4), (GLAS as u16, 2)], unlock: None },
    Recipe { out: STAENDER as u16, n: 1, ins: &[(BRETTER as u16, 6), (EISEN as u16, 1)], unlock: None },
    Recipe { out: SCHILD as u16, n: 3, ins: &[(BRETTER as u16, 6)], unlock: None },
    Recipe { out: SAAT_WEIZEN as u16, n: 4, ins: &[(LAUB as u16, 8)], unlock: None },
    Recipe { out: SAAT_KAROTTE as u16, n: 2, ins: &[(ERNTE + 0, 4)], unlock: None },
    Recipe { out: SAAT_BEERE as u16, n: 2, ins: &[(ERNTE + 1, 4), (SHARD + 2, 2)], unlock: None },
    Recipe { out: SAAT_KUERBIS as u16, n: 1, ins: &[(ERNTE + 2, 6), (SHARD + 4, 4)], unlock: None },
    Recipe { out: SAAT_KRISTALL as u16, n: 1, ins: &[(ERNTE + 3, 4), (FOKUS_KRISTALL, 1)], unlock: None },
    Recipe { out: ERNTE_HACKE, n: 1, ins: &[(STAMM as u16, 4), (EISEN as u16, 2)], unlock: None },
    Recipe { out: SEGEN_HACKE, n: 1, ins: &[(ERNTE_HACKE, 1), (ERNTE + 4, 8), (SHARD + 2, 32)], unlock: None },
];
