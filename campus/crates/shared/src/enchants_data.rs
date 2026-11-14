//! Verzauberungen (Phase 17). Reine Daten; Regeln in enchanting.rs.
use crate::rpg::EnchDef;

pub static ENCHANTS: [EnchDef; 6] = [
    EnchDef { id: 0, name: "Effizienz", max: 5, kinds: 1, per_level: 12.0, fach: 0, desc: "+12 % Abbau-Tempo je Stufe (gehaltenes Werkzeug)" },
    EnchDef { id: 1, name: "Glück", max: 5, kinds: 1, per_level: 8.0, fach: 4, desc: "+8 % Chance auf doppelte Ausbeute je Stufe" },
    EnchDef { id: 2, name: "Schärfe", max: 5, kinds: 2, per_level: 8.0, fach: 3, desc: "+8 % Waffenschaden je Stufe" },
    EnchDef { id: 3, name: "Schutz", max: 5, kinds: 4, per_level: 6.0, fach: 2, desc: "+6 Verteidigung je Stufe und Rüstungsteil" },
    EnchDef { id: 4, name: "Weisheit", max: 5, kinds: 12, per_level: 15.0, fach: 0, desc: "+15 Fokus-Mana je Stufe und Teil" },
    EnchDef { id: 5, name: "Telekinese", max: 1, kinds: 3, per_level: 1.0, fach: 1, desc: "Beute landet ohne Umweg direkt im Inventar" },
];
