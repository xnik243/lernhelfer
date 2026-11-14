//! Campus-Questbuch (Phase 20): 12 Kapitel vom ersten Schritt bis zum Boss. Reine Daten.
use crate::rpg::{Chapter, ERNTE_HACKE};
use crate::skyblock::{Goal, BAUSTAB, HAKEN, DEKAN};
use crate::world::BRUCHSTEIN;

pub static CHAPTERS: [Chapter; 12] = [
    Chapter {
        id: 1,
        name: "Ankunft auf dem Campus",
        text: "Willkommen auf dem Campus. Der Dekan erwartet dich schon am Eingang.",
        steps: &[(Goal::Talk(DEKAN), "Sprich mit Dekan Weber und hol dein Starter-Paket")],
        taler: 25,
        shards: 2,
    },
    Chapter {
        id: 2,
        name: "Die eigene Insel",
        text: "Deine Insel wartet auf dich. Mach sie groß und bau sie aus.",
        steps: &[
            (Goal::Home, "Reise zur eigenen Insel (Portal oder /is)"),
            (Goal::Mine(BRUCHSTEIN as u16, 32), "Baue 32 Bruchstein am Insel-Generator ab"),
            (Goal::Place(16), "Erweitere deine Insel um 16 Blöcke"),
        ],
        taler: 40,
        shards: 3,
    },
    Chapter {
        id: 3,
        name: "Minion-Meister",
        text: "Lern-Minions arbeiten für dich, auch wenn du nicht hinschaust. Sie brauchen aber Treibstoff.",
        steps: &[
            (Goal::PlaceMinion, "Stelle deinen ersten Lern-Minion auf"),
            (Goal::Fuel, "Tanke deinen Minion mit Lern-Energie auf"),
        ],
        taler: 50,
        shards: 3,
    },
    Chapter {
        id: 4,
        name: "Truhen & Rüstung",
        text: "Sammle deine Funde sicher ein und lege dich für den Kampf in Schale.",
        steps: &[
            (Goal::Chest, "Stelle eine Lager-Truhe auf deine Insel"),
            (Goal::Equip(4), "Trage ein komplettes Rüstungsset (4/4)"),
        ],
        taler: 80,
        shards: 4,
    },
    Chapter {
        id: 5,
        name: "Erste Ernte",
        text: "Auf deiner Insel wächst etwas. Bearbeite den Boden und ernte deine erste Pflanze.",
        steps: &[
            (Goal::Craft(ERNTE_HACKE), "Stelle eine Ernte-Hacke her"),
            (Goal::Plant(1), "Pflanze 1 Saat auf deiner Insel"),
            (Goal::Harvest(1), "Ernte eine reife Pflanze"),
        ],
        taler: 60,
        shards: 4,
    },
    Chapter {
        id: 6,
        name: "Handel & Verdichten",
        text: "Aus vielen kleinen Stücken wird etwas Großes. Verdichte und verkaufe klug.",
        steps: &[
            (Goal::Compact, "Verdichte 160 Stück zu einem verzauberten Material"),
            (Goal::Bazaar, "Handle einmal im Bazaar"),
        ],
        taler: 80,
        shards: 5,
    },
    Chapter {
        id: 7,
        name: "Die Bank",
        text: "Geld liegt am besten sicher. Wer regelmäßig lernt, bekommt Zinsen.",
        steps: &[(Goal::Deposit(100), "Zahle 100 Taler beim Bankier ein")],
        taler: 60,
        shards: 5,
    },
    Chapter {
        id: 8,
        name: "Verzauberung",
        text: "Magie folgt Regeln. Der Altar im Bibliotheks-Turm verändert deine Werkzeuge.",
        steps: &[
            (Goal::Warp(2), "Reise zum Bibliotheks-Turm"),
            (Goal::Enchant, "Verzaubere ein Werkzeug am Altar"),
        ],
        taler: 100,
        shards: 6,
    },
    Chapter {
        id: 9,
        name: "Werkzeug-Meister",
        text: "Jedes Werkzeug hat eine eigene Fähigkeit. Probiere sie alle aus.",
        steps: &[
            (Goal::UseTool(BAUSTAB[0]), "Baue mit dem Bau-Stab"),
            (Goal::UseTool(HAKEN), "Schwinge dich mit dem Enterhaken"),
        ],
        taler: 100,
        shards: 6,
    },
    Chapter {
        id: 10,
        name: "Inselbesuch",
        text: "Der Campus ist größer als deine Insel. Besuche andere und zeige ihnen Wertschätzung.",
        steps: &[
            (Goal::Visit, "Besuche die Insel einer anderen Person"),
            (Goal::Like, "Vergib ein Herz für eine fremde Insel"),
        ],
        taler: 80,
        shards: 6,
    },
    Chapter {
        id: 11,
        name: "Die Prüfungs-Zitadelle",
        text: "Hinter den Mauern der Zitadelle warten die Klausur-Wächter. Nur gut Vorbereitete kommen durch.",
        steps: &[
            (Goal::Warp(4), "Reise zur Prüfungs-Zitadelle"),
            (Goal::Kill(2, 3), "Besiege 3 Klausur-Wächter"),
        ],
        taler: 200,
        shards: 10,
    },
    Chapter {
        id: 12,
        name: "Der Große Prüfungs-Archon",
        text: "Im Herzen der Zitadelle wartet der Archon, der letzte Prüfer des Campus. Zeig, was du gelernt hast.",
        steps: &[(Goal::Kill(3, 1), "Besiege den Großen Prüfungs-Archon")],
        taler: 500,
        shards: 25,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kapitel() {
        assert_eq!(CHAPTERS.len(), 12);
        for (i, c) in CHAPTERS.iter().enumerate() {
            assert_eq!(c.id as usize, i + 1, "Kapitel-Ids müssen 1..=12 in Reihenfolge sein");
            assert!(!c.steps.is_empty(), "Kapitel {} hat keine Schritte", c.id);
            for (_, t) in c.steps.iter() {
                assert!(t.chars().count() <= 60, "Schritt zu lang: {}", t);
            }
        }
    }
}
