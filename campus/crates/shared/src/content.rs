//! Inhaltsdaten: Rezepte, NPCs mit Dialogen, Quests (reine Daten, Regeln in skyblock.rs).
use crate::skyblock::*;
use crate::world::*;

// Hinweis: FOKUS_KRISTALL gibt es doppelt (Block 47 in world, Gegenstand 1110 in skyblock). Der Gegenstand ist voll qualifiziert.

pub static RECIPES: &[Recipe] = &[
    // ---------- Werkzeuge ----------
    Recipe { out: BAUSTAB[0], n: 1, ins: &[(BRETTER as u16, 8), (KUPFER as u16, 2), (VERDICHTET + 0, 1)], unlock: Some((STAMM as u16, 2)) },
    Recipe { out: BAUSTAB[1], n: 1, ins: &[(BAUSTAB[0], 1), (VERDICHTET + 1, 8), (SAPHIR as u16, 1)], unlock: Some((STAMM as u16, 5)) },
    Recipe { out: BAUSTAB[2], n: 1, ins: &[(BAUSTAB[1], 1), (SUPER + 1, 1), (SHARD, 16), (SHARD + 4, 16)], unlock: Some((STAMM as u16, 8)) },
    Recipe { out: HAMMER, n: 1, ins: &[(FELS as u16, 8), (BRUCHSTEIN as u16, 8), (STAMM as u16, 4)], unlock: Some((BRUCHSTEIN as u16, 1)) },
    Recipe { out: HAKEN, n: 1, ins: &[(STAMM as u16, 6), (LAUB as u16, 6), (KUPFER as u16, 2)], unlock: Some((LAUB as u16, 2)) },
    Recipe { out: SPRUNGSTAB, n: 1, ins: &[(SHARD, 32), (crate::skyblock::FOKUS_KRISTALL, 4), (VERDICHTET + 6, 1)], unlock: Some((SHARD, 3)) },
    Recipe { out: KOMPASS, n: 1, ins: &[(KUPFER as u16, 4), (GLAS as u16, 1)], unlock: None },
    Recipe { out: SPITZHACKE, n: 1, ins: &[(BRUCHSTEIN as u16, 12), (STAMM as u16, 4), (EISEN as u16, 2)], unlock: Some((BRUCHSTEIN as u16, 1)) },
    Recipe { out: AXT, n: 1, ins: &[(STAMM as u16, 10), (BRUCHSTEIN as u16, 4), (EISEN as u16, 1)], unlock: Some((STAMM as u16, 1)) },

    // ---------- Lern-Minions Stufe I ----------
    Recipe { out: 1201, n: 1, ins: &[(BRUCHSTEIN as u16, 80), (crate::skyblock::FOKUS_KRISTALL, 1)], unlock: None },
    Recipe { out: 1217, n: 1, ins: &[(STAMM as u16, 80), (crate::skyblock::FOKUS_KRISTALL, 1)], unlock: Some((STAMM as u16, 1)) },
    Recipe { out: 1233, n: 1, ins: &[(SAND as u16, 80), (crate::skyblock::FOKUS_KRISTALL, 1)], unlock: Some((SAND as u16, 1)) },
    Recipe { out: 1249, n: 1, ins: &[(LAUB as u16, 80), (crate::skyblock::FOKUS_KRISTALL, 1)], unlock: Some((LAUB as u16, 1)) },

    // ---------- Lern-Minions: Stufen 1 bis 11 (Stein) ----------
    Recipe { out: 1202, n: 1, ins: &[(1200 + 1, 1), (BRUCHSTEIN as u16, 64)], unlock: Some((BRUCHSTEIN as u16, 1)) },
    Recipe { out: 1203, n: 1, ins: &[(1200 + 2, 1), (BRUCHSTEIN as u16, 128)], unlock: Some((BRUCHSTEIN as u16, 2)) },
    Recipe { out: 1204, n: 1, ins: &[(1200 + 3, 1), (BRUCHSTEIN as u16, 256)], unlock: Some((BRUCHSTEIN as u16, 3)) },
    Recipe { out: 1205, n: 1, ins: &[(1200 + 4, 1), (BRUCHSTEIN as u16, 512)], unlock: Some((BRUCHSTEIN as u16, 4)) },
    Recipe { out: 1206, n: 1, ins: &[(1200 + 5, 1), (1400, 8)], unlock: Some((BRUCHSTEIN as u16, 5)) },
    Recipe { out: 1207, n: 1, ins: &[(1200 + 6, 1), (1400, 16)], unlock: Some((BRUCHSTEIN as u16, 6)) },
    Recipe { out: 1208, n: 1, ins: &[(1200 + 7, 1), (1400, 32)], unlock: Some((BRUCHSTEIN as u16, 7)) },
    Recipe { out: 1209, n: 1, ins: &[(1200 + 8, 1), (1400, 64)], unlock: Some((BRUCHSTEIN as u16, 8)) },
    Recipe { out: 1210, n: 1, ins: &[(1200 + 9, 1), (1400, 96)], unlock: Some((BRUCHSTEIN as u16, 9)) },
    Recipe { out: 1211, n: 1, ins: &[(1200 + 10, 1), (1400, 128)], unlock: Some((BRUCHSTEIN as u16, 10)) },

    // ---------- Lern-Minions: Stufen 1 bis 11 (Holz) ----------
    Recipe { out: 1218, n: 1, ins: &[(1216 + 1, 1), (STAMM as u16, 64)], unlock: Some((STAMM as u16, 1)) },
    Recipe { out: 1219, n: 1, ins: &[(1216 + 2, 1), (STAMM as u16, 128)], unlock: Some((STAMM as u16, 2)) },
    Recipe { out: 1220, n: 1, ins: &[(1216 + 3, 1), (STAMM as u16, 256)], unlock: Some((STAMM as u16, 3)) },
    Recipe { out: 1221, n: 1, ins: &[(1216 + 4, 1), (STAMM as u16, 512)], unlock: Some((STAMM as u16, 4)) },
    Recipe { out: 1222, n: 1, ins: &[(1216 + 5, 1), (1401, 8)], unlock: Some((STAMM as u16, 5)) },
    Recipe { out: 1223, n: 1, ins: &[(1216 + 6, 1), (1401, 16)], unlock: Some((STAMM as u16, 6)) },
    Recipe { out: 1224, n: 1, ins: &[(1216 + 7, 1), (1401, 32)], unlock: Some((STAMM as u16, 7)) },
    Recipe { out: 1225, n: 1, ins: &[(1216 + 8, 1), (1401, 64)], unlock: Some((STAMM as u16, 8)) },
    Recipe { out: 1226, n: 1, ins: &[(1216 + 9, 1), (1401, 96)], unlock: Some((STAMM as u16, 9)) },
    Recipe { out: 1227, n: 1, ins: &[(1216 + 10, 1), (1401, 128)], unlock: Some((STAMM as u16, 10)) },

    // ---------- Lern-Minions: Stufen 1 bis 11 (Sand) ----------
    Recipe { out: 1234, n: 1, ins: &[(1232 + 1, 1), (SAND as u16, 64)], unlock: Some((SAND as u16, 1)) },
    Recipe { out: 1235, n: 1, ins: &[(1232 + 2, 1), (SAND as u16, 128)], unlock: Some((SAND as u16, 2)) },
    Recipe { out: 1236, n: 1, ins: &[(1232 + 3, 1), (SAND as u16, 256)], unlock: Some((SAND as u16, 3)) },
    Recipe { out: 1237, n: 1, ins: &[(1232 + 4, 1), (SAND as u16, 512)], unlock: Some((SAND as u16, 4)) },
    Recipe { out: 1238, n: 1, ins: &[(1232 + 5, 1), (1402, 8)], unlock: Some((SAND as u16, 5)) },
    Recipe { out: 1239, n: 1, ins: &[(1232 + 6, 1), (1402, 16)], unlock: Some((SAND as u16, 6)) },
    Recipe { out: 1240, n: 1, ins: &[(1232 + 7, 1), (1402, 32)], unlock: Some((SAND as u16, 7)) },
    Recipe { out: 1241, n: 1, ins: &[(1232 + 8, 1), (1402, 64)], unlock: Some((SAND as u16, 8)) },
    Recipe { out: 1242, n: 1, ins: &[(1232 + 9, 1), (1402, 96)], unlock: Some((SAND as u16, 9)) },
    Recipe { out: 1243, n: 1, ins: &[(1232 + 10, 1), (1402, 128)], unlock: Some((SAND as u16, 10)) },

    // ---------- Lern-Minions: Stufen 1 bis 11 (Laub) ----------
    Recipe { out: 1250, n: 1, ins: &[(1248 + 1, 1), (LAUB as u16, 64)], unlock: Some((LAUB as u16, 1)) },
    Recipe { out: 1251, n: 1, ins: &[(1248 + 2, 1), (LAUB as u16, 128)], unlock: Some((LAUB as u16, 2)) },
    Recipe { out: 1252, n: 1, ins: &[(1248 + 3, 1), (LAUB as u16, 256)], unlock: Some((LAUB as u16, 3)) },
    Recipe { out: 1253, n: 1, ins: &[(1248 + 4, 1), (LAUB as u16, 512)], unlock: Some((LAUB as u16, 4)) },
    Recipe { out: 1254, n: 1, ins: &[(1248 + 5, 1), (1403, 8)], unlock: Some((LAUB as u16, 5)) },
    Recipe { out: 1255, n: 1, ins: &[(1248 + 6, 1), (1403, 16)], unlock: Some((LAUB as u16, 6)) },
    Recipe { out: 1256, n: 1, ins: &[(1248 + 7, 1), (1403, 32)], unlock: Some((LAUB as u16, 7)) },
    Recipe { out: 1257, n: 1, ins: &[(1248 + 8, 1), (1403, 64)], unlock: Some((LAUB as u16, 8)) },
    Recipe { out: 1258, n: 1, ins: &[(1248 + 9, 1), (1403, 96)], unlock: Some((LAUB as u16, 9)) },
    Recipe { out: 1259, n: 1, ins: &[(1248 + 10, 1), (1403, 128)], unlock: Some((LAUB as u16, 10)) },

    // ---------- Treibstoff ----------
    Recipe { out: ENERGIE, n: 1, ins: &[(crate::skyblock::FOKUS_KRISTALL, 2)], unlock: None },
];

pub static NPCS: &[Npc] = &[
    Npc {
        id: 1,
        name: "Dekan Weber",
        at: [240.5, 33.0, 292.5],
        yaw: std::f32::consts::PI,
        rgb: [[40, 70, 150], [230, 190, 70]],
        lines: &[
            "Willkommen auf dem Campus, du Neuling!",
            "Der Campus schwebt in der Luft. Jede Insel ist dein eigenes Grundstück.",
            "Die Mitte gehört allen: Marktplatz, Bazaar, Bibliothek und Werkstatt.",
            "Verbinde dich mit dem Lernhelfer. Jede Lernrunde wird dann zu Fortschritt auf dem Campus.",
            "Hier dein Startpaket: 50 Taler, ein Stein-Minion I, 6 Stunden Lern-Energie und ein Abbau-Hammer.",
            "Das Portal am Westrand des Platzes bringt dich auf deine eigene Insel. Die Questtafel zeigt dein nächstes Ziel.",
        ],
        quest: Some(1),
    },
    Npc {
        id: 2,
        name: "Bankier Goldbach",
        at: [212.5, 29.0, 227.5],
        yaw: -std::f32::consts::FRAC_PI_2,
        rgb: [[60, 60, 60], [200, 160, 40]],
        lines: &[
            "Guten Tag! Ich bin Goldbach, der Bankier des Campus.",
            "Bei mir kannst du Taler einzahlen und wieder abheben. Sag einfach, wie viel.",
            "Auf dein Guthaben gibt es 1 % Zinsen pro Tag.",
            "Die Zinsen laufen nur, solange du in den letzten 48 Stunden im Lernhelfer gelernt hast.",
            "Und: Wer ins Leere fällt, verliert ein Viertel der Taler in der Geldbörse. Was hier liegt, bleibt sicher.",
        ],
        quest: Some(3),
    },
    Npc {
        id: 3,
        name: "Bibliothekarin Lux",
        at: [384.5, 31.0, 240.5],
        yaw: std::f32::consts::FRAC_PI_2,
        rgb: [[120, 60, 140], [240, 220, 140]],
        lines: &[
            "Schhh ... willkommen im Bibliotheks-Turm!",
            "Jede Lernrunde im Lernhelfer bringt dir Wissens-Splitter, passend zum Fach.",
            "Mathematik gibt Mathe-Splitter, Sprachen Sprach-Splitter, Naturwissenschaften Natur-Splitter.",
            "Gesellschaft gibt Gesellschafts-Splitter. Alle anderen Fächer landen beim Wissens-Splitter.",
            "Aus 160 Splittern wird ein verdichteter Splitter. Das lohnt sich!",
        ],
        quest: Some(2),
    },
    Npc {
        id: 4,
        name: "Meisterin Kolbe",
        at: [176.5, 27.0, 232.5],
        yaw: -std::f32::consts::FRAC_PI_2,
        rgb: [[180, 90, 40], [90, 140, 200]],
        lines: &[
            "Hallo, ich bin Kolbe, Meisterin der Minion-Werkstatt!",
            "Minions sammeln für dich Rohstoffe, auch wenn du gerade offline bist.",
            "Ohne Lern-Energie arbeitet ein Minion nur mit 20 % Tempo.",
            "Lern-Energie gibt es im Lernhelfer: 15 Fokus-Minuten oder 20 Karteikarten ergeben 24 Stunden.",
            "Jede Stufe macht deinen Minion schneller. Sammle Stücke, dann steigt er auf.",
        ],
        quest: Some(4),
    },
    Npc {
        id: 5,
        name: "Händler Brandt",
        at: [240.5, 29.0, 215.5],
        yaw: std::f32::consts::PI,
        rgb: [[50, 120, 70], [230, 120, 60]],
        lines: &[
            "Na, auf Schnäppchen aus? Ich bin Brandt!",
            "Im Bazaar handelst du mit anderen Spielern. Du stellst Kauf- und Verkaufsaufträge ein.",
            "Passen Preis und Angebot zusammen, wird gehandelt.",
            "Bei jedem Verkauf gehen 1,5 % Gebühr verloren. Die werden vernichtet, nicht an mich gezahlt.",
            "Viel Erfolg beim Handeln!",
        ],
        quest: Some(5),
    },
    Npc {
        id: 6,
        name: "Torwächterin Vega",
        at: [244.5, 31.0, 101.5],
        yaw: std::f32::consts::PI,
        rgb: [[40, 40, 80], [150, 90, 240]],
        lines: &[
            "Halt! Hier beginnt die Prüfungs-Zitadelle.",
            "Durch dieses Portal gelangst du zu den Prüfungen. Dort wird dein Wissen auf die Probe gestellt.",
            "In der Prüfungsphase zählt jeder Fokus-Moment. Lerne gezielt, dann bist du vorbereitet.",
            "Mit Fokus-Mana setzt du deine Werkzeuge ein. Es lädt sich von selbst wieder auf.",
            "Komm wieder, wenn du bereit bist. Dann öffne ich dir das Tor.",
        ],
        quest: Some(6),
    },
    Npc {
        id: 7,
        name: "Schmied Hammer",
        at: [177.5, 27.0, 213.5],
        yaw: std::f32::consts::PI,
        rgb: [[90, 90, 100], [230, 110, 50]],
        lines: &[
            "Hammer, Zange, Amboss: Ich bin Hammer, der Schmied.",
            "Aus 160 Stücken eines Rohstoffs machst du einen verdichteten Block. Das spart Platz.",
            "Bei mir gibt es Werkzeuge: Bau-Stab, Abbau-Hammer, Enterhaken, Vektor-Sprungstab, Spitzhacke und Axt.",
            "Mit Spitzhacke und Axt baust du Stein und Holz viel schneller ab. Die Werkbank steht hier im Hof.",
            "Der Bau-Stab platziert Blöcke, der Enterhaken zieht dich zu Stellen, der Sprungstab teleportiert dich kurz nach vorn.",
            "Bring mir Material, dann schmiede ich dir etwas.",
        ],
        quest: None,
    },
    Npc {
        id: 8,
        name: "Auktionator Pfeiffer",
        at: [261.5, 29.0, 226.5],
        yaw: std::f32::consts::FRAC_PI_2,
        rgb: [[110, 30, 40], [230, 200, 120]],
        lines: &[
            "Zum Ersten, zum Zweiten ... willkommen im Auktionshaus!",
            "Seltene Funde stellst du hier zur Versteigerung ein: 24 oder 72 Stunden.",
            "Wer am Ende am meisten bietet, gewinnt. Mit einem Sofortpreis geht es schneller.",
            "Vom Verkaufspreis gehen 5 % Gebühr ab. Die verschwinden aus dem Spiel.",
        ],
        quest: None,
    },
    Npc {
        id: 9,
        name: "Förster Eichholz",
        at: [301.5, 31.0, 259.5],
        yaw: 0.0,
        rgb: [[60, 100, 50], [150, 100, 60]],
        lines: &[
            "Psst, nicht so laut, hier im Flüsterwald hören die Bäume mit.",
            "Die Baumstümpfe rund um meine Hütte wachsen nach. Hau ruhig drauf!",
            "Mit einer Axt geht das Holzhacken viel schneller.",
            "Aus Holz baust du Bretter, Werkzeuge und deinen ersten Holz-Minion.",
        ],
        quest: None,
    },
    Npc {
        id: 10,
        name: "Gärtnerin Flora",
        at: [96.5, 31.0, 244.5],
        yaw: std::f32::consts::FRAC_PI_2,
        rgb: [[200, 110, 150], [120, 190, 90]],
        lines: &[
            "Willkommen im Himmels-Garten und in der Sternwarte!",
            "Hier blühen Kirschen und Glühlaub, mitten über den Wolken.",
            "Man munkelt, im Garten sind Schatztruhen versteckt. Eine je Tag und Truhe darfst du öffnen.",
            "Schau hinter die Hecken, unter die Bäume und in die Sternwarte.",
        ],
        quest: None,
    },
];

/// Zusätzliche Belohnung beim Abschluss einer Quest: (Quest, Gegenstand, Menge).
pub static QUEST_REWARDS: &[(u8, u16, u32)] = &[(1, ENERGIE, 5)]; // 5 Lern-Energie-Zellen = „5× Minion-Turbo“

pub static QUESTS: &[Quest] = &[
    Quest {
        id: 1,
        name: "Willkommen auf dem Campus",
        steps: &[
            Step { text: "Hole dein Starter-Paket bei Dekan Weber", goal: Goal::Talk(1), taler: 5 },
            Step { text: "Reise durch das Portal auf deine Privatinsel", goal: Goal::Home, taler: 10 },
            Step { text: "Stelle deinen Stein-Minion auf & baue 10 Bruchstein ab", goal: Goal::MineMinion(BRUCHSTEIN as u16, 10), taler: 15 },
            Step { text: "Verkaufe oder kaufe etwas im Bazaar", goal: Goal::Bazaar, taler: 20 },
            Step { text: "Löse 1 Check-Frage im Lernhelfer (5× Minion-Turbo)", goal: Goal::Check(1), taler: 40 },
        ],
    },
    Quest {
        id: 2,
        name: "Wissen sammeln",
        steps: &[
            Step { text: "Sprich mit Bibliothekarin Lux", goal: Goal::Talk(3), taler: 5 },
            Step { text: "Lerne dreimal im Lernhelfer", goal: Goal::Learn(3), taler: 15 },
            Step { text: "Stelle einen Insel-Kompass her", goal: Goal::Craft(KOMPASS), taler: 30 },
        ],
    },
    Quest {
        id: 3,
        name: "Sparen mit Zinsen",
        steps: &[
            Step { text: "Sprich mit Bankier Goldbach", goal: Goal::Talk(2), taler: 5 },
            Step { text: "Lege 25 Taler bei Goldbach ein", goal: Goal::Deposit(25), taler: 15 },
            Step { text: "Lerne im Lernhelfer, damit Zinsen laufen", goal: Goal::Learn(1), taler: 30 },
        ],
    },
    Quest {
        id: 4,
        name: "Die Minion-Werkstatt",
        steps: &[
            Step { text: "Sprich mit Meisterin Kolbe", goal: Goal::Talk(4), taler: 5 },
            Step { text: "Stelle einen Minion auf deiner Insel auf", goal: Goal::PlaceMinion, taler: 15 },
            Step { text: "Stelle eine Lern-Energie-Zelle her", goal: Goal::Craft(ENERGIE), taler: 25 },
            Step { text: "Baue 32 Stamm ab", goal: Goal::Mine(STAMM as u16, 32), taler: 35 },
        ],
    },
    Quest {
        id: 5,
        name: "Auf dem Bazaar",
        steps: &[
            Step { text: "Sprich mit Händler Brandt", goal: Goal::Talk(5), taler: 5 },
            Step { text: "Handle einmal im Bazaar", goal: Goal::Bazaar, taler: 15 },
            Step { text: "Stelle einen Abbau-Hammer her", goal: Goal::Craft(HAMMER), taler: 25 },
            Step { text: "Nutze einmal die Fähigkeit eines Werkzeugs", goal: Goal::Ability, taler: 40 },
        ],
    },
    Quest {
        id: 6,
        name: "Die Prüfungs-Zitadelle",
        steps: &[
            Step { text: "Sprich mit Torwächterin Vega", goal: Goal::Talk(6), taler: 5 },
            Step { text: "Lerne fünfmal im Lernhelfer", goal: Goal::Learn(5), taler: 15 },
            Step { text: "Besuche die Insel eines Mitspielers", goal: Goal::Visit, taler: 25 },
            Step { text: "Reise zum Bibliotheks-Turm", goal: Goal::Warp(2), taler: 40 },
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daten_passen() {
        for q in QUESTS { assert!(!q.steps.is_empty()); for s in q.steps { assert!(s.text.chars().count() <= 60, "{}", s.text); } }
        for n in NPCS { assert!(n.quest.is_none_or(|q| QUESTS.iter().any(|x| x.id == q)), "{}", n.name); assert!(!n.lines.is_empty()); }
        assert!(NPCS.iter().any(|n| n.id == DEKAN));
        for r in RECIPES { assert!(!r.ins.is_empty() && r.n > 0); }
        for (i, n) in NPCS.iter().enumerate() { assert!(NPCS[..i].iter().all(|m| m.id != n.id), "{}", n.name); }
        assert!(QUEST_REWARDS.iter().all(|r| QUESTS.iter().any(|q| q.id == r.0)));
    }
}
