//! Nachrichten zwischen Browser und Spielserver (bincode über WebSocket).
use crate::items::Look;
use crate::world::WorldState;
use serde::{Deserialize, Serialize};

pub const PHRASES: [&str; 12] = [
    "Hallo!", "Danke!", "Gut gebaut!", "Komm mit!", "Brauchst du Hilfe?", "Ja", "Nein", "Viel Erfolg bei der Prüfung!",
    "Lernpause?", "Schau dir mein Grundstück an!", "Wer zahlt mit ins Großprojekt ein?", "Bis später!",
];
pub const EMOTE_COUNT: u8 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum C2S {
    Move { p: [f32; 3], yaw: f32, anim: u8 },
    Block { x: i16, y: i16, z: i16, b: u8 },
    Chat(u8),
    Emote(u8),
    /// Reise per Insel-Kompass, Portal oder Befehl
    Warp(Warp),
    /// Sichtweite in Stücken (4..=12), nur für das Nachladen
    View(u8),
    /// Freitext im Kanal 0 Hub, 1 Insel, 2 Gruppe; beginnt er mit „/“, ist es ein Befehl
    Say { ch: u8, text: String },
    /// mit einer Person (NPC) sprechen
    Talk(u8),
    /// Werkzeug-Fähigkeit in Blickrichtung
    Ability { tool: u16, dir: [f32; 3] },
    /// Beginnt, diesen Block abzubauen (linke Taste gedrückt). Der Server merkt sich Stelle und Zeit;
    /// das folgende Block{b: 0} gilt nur, wenn world::mine_ok(Abbauzeit, vergangene ms) stimmt.
    Dig { x: i16, y: i16, z: i16 },
    /// Truhe öffnen (Truhe auf der Insel) oder Schatztruhe einmal am Tag plündern ([E] auf den Block).
    /// Phase 16–20: auch Große Wissens-Truhe, Verzauberungs-Altar, Prüfungs-Altar, Rüstungsständer, Schild ([E] = benutzen).
    Chest { x: i16, y: i16, z: i16 },
    // ---------- Phasen 16–20 (angehängt) ----------
    /// Gewählter Schnellleisten-Platz 0..9: der Server liest daraus die gehaltene Waffe / das Werkzeug.
    Held(u8),
    /// Nahkampf-Schlag auf ein Wesen (Server prüft Reichweite combat::MELEE_REACH und Abklingzeit SWING_CD_MS).
    Attack { mob: u32 },
    /// Fernkampf mit gehaltenem Bogen/Stab in Blickrichtung (Stab kostet Fokus-Mana).
    Shoot { dir: [f32; 3] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Warp {
    /// eigene Insel
    Home,
    /// Insel eines Grundstücks
    Plot(u32),
    /// Ort im Hub, Index in world::SPOTS
    Spot(u8),
}

/// Wer darf eine Insel besuchen: 0 alle, 1 Lerngruppe und Mitbauer, 2 nur Mitbauer.
pub const VISIT_NAMES: [&str; 3] = ["Öffentlich", "Gruppe", "Privat"];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PState {
    pub id: u32,
    pub p: [f32; 3],
    pub yaw: f32,
    pub anim: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlotInfo {
    pub id: u32,
    pub slot: (u8, u8),
    pub size: u8,
    pub owner: u32,   // Konto oder 0 bei Gruppen
    pub group: u32,   // Gruppe oder 0
    pub name: String, // Schild am Grundstück
    pub may: bool,    // darf der Empfänger hier bauen?
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum S2C {
    Welcome { id: u32, pos: [f32; 3], state: WorldState },
    Chunk { cx: u8, cz: u8, rle: Vec<u8> },
    Block { x: i16, y: i16, z: i16, b: u8, by: u32 },
    Players(Vec<PState>),
    Join { id: u32, name: String, look: Look },
    Leave(u32),
    Chat { id: u32, n: u8 },
    Emote { id: u32, n: u8 },
    Inv { item: u16, n: u32 },
    Plots(Vec<PlotInfo>),
    World(WorldState),
    /// JSON für die Bedienoberfläche (Kontostand, Projekte, Gebote …)
    Hud(String),
    Kick(String),
    /// Server versetzt den Spieler (Warp, Portal)
    Teleport([f32; 3]),
    /// Chatzeile: Kanal 0 Hub, 1 Insel, 2 Gruppe, 3 privat, 4 System (id 0)
    Said { id: u32, ch: u8, name: String, text: String },
    /// Wie Teleport, aber mit Blickrichtung (Warp, Portal, Sturz ins Leere, Anmeldung)
    Spawn { p: [f32; 3], yaw: f32 },
    // ---------- Phasen 16–20 (angehängt) ----------
    /// Wesen in der Nähe (10 Hz, nur an Spieler in/um die Prüfungs-Zitadelle); leere Liste = keine mehr.
    Mobs(Vec<MobState>),
    /// Schwebende Schadenszahl: an `at`, Höhe `n`, kritisch; `mob` = getroffenes Wesen (0 = du selbst wurdest getroffen).
    Damage { at: [f32; 3], n: u32, crit: bool, mob: u32 },
    /// Sichtbare Ausrüstung einer Figur: 0..4 Rüstung, 4..7 Talismane, 7 gehaltene Waffe/Werkzeug (an alle).
    Gear { id: u32, items: [u16; 8] },
    /// Insel-Inhalt beim Betreten/Ändern: Pflanzen (Stufe rechnet der Browser aus `now`), Schilder, Rüstungsständer.
    Island { now: i64, crops: Vec<CropState>, signs: Vec<SignState>, stands: Vec<StandState> },
    /// Eigene Lebenspunkte; `shield_ms` = Schutzschild nach der Wiederbelebung.
    Health { hp: f32, max: f32, shield_ms: u32 },
    /// Stoßwelle des Bosses (wachsender Ring ab `at` bis Radius `r`).
    Shock { at: [f32; 3], r: f32 },
    /// Geschoss (Bogen/Stab) zum Zeichnen: von → nach, Art 0 Pfeil, 1 Arkan-Kugel.
    Shot { from: [f32; 3], to: [f32; 3], kind: u8 },
    /// Beute fliegt sichtbar zur Figur (Magnet); die Gegenstände sind schon gutgeschrieben.
    Drops { at: [f32; 3], items: Vec<u16> },
}

/// Ein Wesen: Lebens-Anteil in Promille, `fl` Bits: 1 getroffen (rot aufblitzen), 2 holt aus (Ansturm), 4 Stoßwelle lädt, 8 besiegt.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MobState {
    pub id: u32,
    pub kind: u8,
    pub p: [f32; 3],
    pub yaw: f32,
    pub hp: u16,
    pub fl: u8,
}
pub const MOB_HURT: u8 = 1;
pub const MOB_WINDUP: u8 = 2;
pub const MOB_SLAM: u8 = 4;
pub const MOB_DEAD: u8 = 8;

/// Pflanze: `grow_ms` = Dauer bis reif (Faktor schon eingerechnet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropState {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub kind: u8,
    pub planted: i64,
    pub grow_ms: u32,
}
/// Beschriftetes Schild: bis zu 4 Zeilen (gesäubert).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignState {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub lines: Vec<String>,
}
/// Rüstungsständer: 0..4 Rüstung, 4..7 Talismane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandState {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub items: [u16; 7],
}

pub fn enc<T: Serialize>(m: &T) -> Vec<u8> {
    bincode::serialize(m).unwrap_or_default()
}
pub fn dec<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    bincode::deserialize(b).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hin_und_zurueck() {
        let m = C2S::Block { x: 190, y: 15, z: -3, b: 11 };
        assert!(matches!(dec::<C2S>(&enc(&m)), Some(C2S::Block { x: 190, y: 15, z: -3, b: 11 })));
        let s = S2C::Players(vec![PState { id: 7, p: [1.0, 2.0, 3.0], yaw: 0.5, anim: 1 }; 50]);
        let b = enc(&s);
        assert!(b.len() < 1200, "50 Spieler bleiben klein: {}", b.len());
        assert!(matches!(dec::<S2C>(&b), Some(S2C::Players(v)) if v.len() == 50));
        assert!(dec::<C2S>(&[9, 9, 9]).is_none());
        assert!(matches!(dec::<C2S>(&enc(&C2S::Dig { x: 5, y: 6, z: 7 })), Some(C2S::Dig { x: 5, y: 6, z: 7 })));
        assert!(matches!(dec::<S2C>(&enc(&S2C::Spawn { p: [1.0, 2.0, 3.0], yaw: 0.5 })), Some(S2C::Spawn { yaw, .. }) if yaw == 0.5));
        // angehängte Nachrichten verschieben die alten Nummern nicht (Browser im Cache bleibt lesbar)
        assert_eq!(enc(&C2S::Chest { x: 1, y: 2, z: 3 })[0..4], [10, 0, 0, 0]);
        assert_eq!(enc(&S2C::Spawn { p: [0.0; 3], yaw: 0.0 })[0..4], [15, 0, 0, 0]);
        let m = S2C::Mobs(vec![MobState { id: 1, kind: 2, p: [1.0, 2.0, 3.0], yaw: 0.0, hp: 500, fl: MOB_HURT }; 30]);
        assert!(enc(&m).len() < 1100, "30 Wesen bleiben klein");
        assert!(matches!(dec::<C2S>(&enc(&C2S::Attack { mob: 9 })), Some(C2S::Attack { mob: 9 })));
    }
}
