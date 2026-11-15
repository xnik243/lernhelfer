//! Phase 22: frei gesetzte NPCs der Verwaltung (`/architect npc …`). Aussehen als fester Satz von Vorlagen,
//! Aktion als kurzer Text in der Tabelle `custom_npcs` (hier nur gelesen und geprüft).
use crate::items::Look;

/// C2S::Talk(id): Nummern ab hier sind freie NPCs (feste NPCs aus content::NPCS liegen darunter).
pub const CUSTOM_NPC_BASE: u8 = 200;
pub const CUSTOM_NPC_MAX: usize = 50;
pub const NAME_MAX: usize = 24;
pub const TEXT_MAX: usize = 160;

const fn look(skin: u8, hair: u8, hair_col: u8, outfit: u8, col_a: u8, col_b: u8) -> Look {
    Look { skin, hair, hair_col, outfit, col_a, col_b, slots: [0; 8] }
}
/// Aussehens-Vorlagen (Name für die Auswahl, Aussehen). Index = `custom_npcs.look` = `NpcState.look`.
pub const NPC_PRESETS: [(&str, Look); 8] = [
    ("Studentin", look(1, 1, 10, 1, 7, 1)),
    ("Student", look(2, 0, 0, 0, 5, 0)),
    ("Professorin", look(0, 2, 11, 2, 8, 4)),
    ("Professor", look(3, 4, 0, 2, 0, 4)),
    ("Händlerin", look(4, 3, 3, 3, 3, 10)),
    ("Wächter", look(2, 0, 0, 3, 11, 2)),
    ("Gärtner", look(1, 0, 10, 0, 5, 10)),
    ("Festgast", look(5, 3, 9, 1, 9, 4)),
];
pub fn preset(i: u8) -> Look {
    NPC_PRESETS[i as usize % NPC_PRESETS.len()].1
}

/// Was passiert nach dem Dialog? Text-Form in der Tabelle: "" | "warp:<SPOTS-Index>" | "taler:<n>" | "item:<nr>:<menge>".
/// Belohnungen gibt es je Konto und NPC höchstens einmal am Tag (Server).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcAction {
    None,
    Warp(u8),
    Taler(u32),
    Item(u16, u32),
}
/// Grenzen: Warp nur zu vorhandenen Orten, höchstens 500 Taler bzw. 64 Stück eines vorhandenen Gegenstands.
pub fn parse_action(s: &str) -> Option<NpcAction> {
    let mut it = s.trim().split(':');
    let a = match (it.next()?, it.next(), it.next(), it.next()) {
        ("", None, None, None) => NpcAction::None,
        ("warp", Some(i), None, None) => NpcAction::Warp(i.parse().ok().filter(|&i: &u8| (i as usize) < crate::world::SPOTS.len())?),
        ("taler", Some(n), None, None) => NpcAction::Taler(n.parse().ok().filter(|n| (1..=500).contains(n))?),
        ("item", Some(i), Some(n), None) => {
            let (i, n): (u16, u32) = (i.parse().ok()?, n.parse().ok()?);
            if !crate::items::exists(i) || !crate::inventory::in_slots(i) || !(1..=64).contains(&n) {
                return None;
            }
            NpcAction::Item(i, n)
        }
        _ => return None,
    };
    Some(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aktionen_und_vorlagen() {
        assert_eq!(parse_action(""), Some(NpcAction::None));
        assert_eq!(parse_action("warp:4"), Some(NpcAction::Warp(4)));
        assert_eq!(parse_action("warp:99"), None);
        assert_eq!(parse_action("taler:25"), Some(NpcAction::Taler(25)));
        assert_eq!(parse_action("taler:99999"), None);
        assert_eq!(parse_action("item:1110:2"), Some(NpcAction::Item(1110, 2)));
        assert_eq!(parse_action("item:65000:1"), None);
        assert_eq!(parse_action("quatsch"), None);
        for (name, l) in NPC_PRESETS {
            assert_eq!(l, l.clamped(), "{name}");
        }
        assert_eq!(preset(9), NPC_PRESETS[1].1);
    }
}
