//! Phasen 23/24: Instanz-Felder für die Prüfungs-Katakomben und die Lern-Duell-Arena.
//! Eine Instanz belegt 3×3 Stücke (48×48 Blöcke) in der Leere im Nordosten des Hub-Archipels. Der Server hält ihre
//! Blöcke nur im Speicher und schickt sie ausschließlich ihren Mitgliedern als gewöhnliche S2C::Chunk an diesen
//! Koordinaten (Überlagerung); die echte Welt bleibt dort reine Luft. Der Browser braucht dafür keine Sonderfälle.

/// Art einer Instanz (Index in `ZONE` und `Z0`).
pub const KIND_DUNGEON: u8 = 0;
pub const KIND_ARENA: u8 = 1;
/// Kantenlänge in Blöcken und in Stücken.
pub const SIZE: i32 = 48;
pub const CHUNKS: i32 = 3;
/// Felder je Art: 3 nebeneinander (x), 2 Reihen (z).
pub const COLS: i32 = 3;
pub const ROWS: i32 = 2;
pub const SLOTS: u8 = (COLS * ROWS) as u8;
/// Westkante aller Felder: östlich der Hauptinsel (endet bei x = 330) und weiter als 64 Blöcke von der Zitadelle (keine Zitadellen-Wesen im Bild).
pub const X0: i32 = 336;
/// Nordkante je Art: Katakomben z 0..96, Arena z 96..192.
pub const Z0: [i32; 2] = [0, 96];
/// Zonen-Name je Art (world::zone_name → Anzeigetafel, Musik-Stimmung, Zwischensequenz).
pub const ZONE: [&str; 2] = ["Prüfungs-Katakomben", "Lern-Duell-Arena"];
/// Katakomben: unterste Schicht (Boden), Füße stehen auf DUNGEON_Y + 1; Höhe 24 Blöcke (y 8..32).
pub const DUNGEON_Y: i32 = 8;
pub const DUNGEON_H: i32 = 24;
/// Arena: Oberkante der Plattformen (Füße auf ARENA_Y + 1); wer unter ARENA_FALL_Y fällt, ist abgestürzt.
pub const ARENA_Y: i32 = 40;
pub const ARENA_FALL_Y: f32 = 30.0;

/// Nordwest-Ecke (x, z) des Feldes `slot` (0..SLOTS) der Art `kind`.
pub const fn origin(kind: u8, slot: u8) -> (i32, i32) {
    let s = slot as i32;
    (X0 + (s % COLS) * SIZE, Z0[kind as usize] + (s / COLS) * SIZE)
}
/// Art des Instanz-Feldes an dieser Stelle (None außerhalb).
pub fn kind_at(x: i32, z: i32) -> Option<u8> {
    if x < X0 || x >= X0 + COLS * SIZE || z < 0 {
        return None;
    }
    let k = z / (ROWS * SIZE);
    (k < 2).then_some(k as u8)
}
/// (Art, Feld) an dieser Stelle.
pub fn slot_at(x: i32, z: i32) -> Option<(u8, u8)> {
    let k = kind_at(x, z)?;
    let (dx, dz) = ((x - X0) / SIZE, (z - Z0[k as usize]) / SIZE);
    Some((k, (dz * COLS + dx) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world;

    #[test]
    fn felder_liegen_in_der_leere() {
        let st = world::WorldState::default();
        for k in [KIND_DUNGEON, KIND_ARENA] {
            for s in 0..SLOTS {
                let (x, z) = origin(k, s);
                assert_eq!(slot_at(x, z), Some((k, s)));
                assert_eq!(slot_at(x + SIZE - 1, z + SIZE - 1), Some((k, s)));
                assert!(x % 16 == 0 && z % 16 == 0 && world::in_hub(x + SIZE - 1, z + SIZE - 1));
                for cz in 0..CHUNKS {
                    for cx in 0..CHUNKS {
                        assert!(world::gen_chunk(x / 16 + cx, z / 16 + cz, st).is_empty(), "Feld {k}/{s} liegt nicht in der Leere");
                    }
                }
            }
        }
        assert_eq!(kind_at(X0 - 1, 10), None);
        assert_eq!(kind_at(400, 192), None);
        assert_eq!(world::zone_name(340, 10, 20), ZONE[0]);
        assert_eq!(world::zone_name(340, 41, 100), ZONE[1]);
        // weit genug von der Zitadelle: deren Wesen-Liste (64 Blöcke um das Rechteck) erreicht die Felder nicht
        assert!(X0 - world::CITADEL[2] > 64);
    }
}
