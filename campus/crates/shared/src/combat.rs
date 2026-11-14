//! Kampf-Regeln (Phase 18): Reichweiten, Abklingzeiten, Schaden, Krit, Rückstoß, Boss-Phasen, Beute-Anteil.
//! Der Server rechnet alles; der Browser nutzt nur die Konstanten für Vorschau und Animation.
use crate::stats::{reduction, Stats, FIST_DMG};

pub const MELEE_REACH: f32 = 4.0;
pub const RANGED_REACH: f32 = 28.0;
pub const SWING_CD_MS: i64 = 450;
pub const SHOT_CD_MS: i64 = 600;
pub const STAFF_MANA: u32 = 12;
pub const KNOCKBACK: f32 = 7.0; // Blöcke/s waagrecht
pub const FLASH_MS: i64 = 150;
pub const SHIELD_MS: i64 = 5000;
pub const MOB_HIT_CD_MS: i64 = 900; // ein Wesen trifft höchstens so oft
pub const BOSS_EVERY_MS: i64 = 300_000;
/// Wer mindestens so viel Prozent des Boss-Lebens abgezogen hat, bekommt eine eigene Beute-Würfelrunde.
pub const LOOT_SHARE_PCT: f32 = 10.0;
/// Wellen: so viele Wesen je Ring, solange jemand in der Zitadelle ist.
pub const OUTER_MOBS: usize = 8;
pub const HALL_MOBS: usize = 3;
pub const BOSS_MINIONS: usize = 4;
/// Stoßwelle des Bosses: Ring wächst mit SHOCK_SPEED; wer am Boden steht, wenn sie vorbeizieht, nimmt Schaden (Springen weicht aus).
pub const SHOCK_SPEED: f32 = 9.0;
pub const SHOCK_MAX_R: f32 = 14.0;
pub const SHOCK_BAND: f32 = 1.2;
pub const SHOCK_DMG: f32 = 35.0;
pub const SHOCK_JUMP_Y: f32 = 0.6; // so hoch über dem Boden gilt als gesprungen
pub const TELEGRAPH_MS: i64 = 700; // Wächter holt aus, bevor er anstürmt
pub const CHARGE_SPEED: f32 = 11.0;

/// Schaden eines Spielerschlags und ob er kritisch war. `roll` = Zufallszahl des Servers.
pub fn player_hit(s: &Stats, roll: u32) -> (f32, bool) {
    let base = (FIST_DMG + s.weapon_damage) * (1.0 + s.strength / 100.0);
    let crit = (roll % 10_000) as f32 / 100.0 < s.crit_chance;
    (if crit { base * (1.0 + s.crit_damage / 100.0) } else { base }, crit)
}
/// Schaden, den ein Spieler mit Verteidigung `def` von einem Treffer `raw` nimmt.
pub fn taken(raw: f32, def: f32) -> f32 {
    raw * (1.0 - reduction(def))
}
/// Boss-Phase aus dem Lebens-Anteil: 0 (> 66 %), 1 (> 33 %), 2 (Rest). Ab Phase 1 ruft er Diener, ab 2 schlägt er schneller.
pub fn boss_phase(hp_frac: f32) -> u8 {
    if hp_frac > 0.66 {
        0
    } else if hp_frac > 0.33 {
        1
    } else {
        2
    }
}
/// Abstand des Strahls o + t·d (d normiert) zur Kugel (c, r): erstes t im Bereich 0..max oder None.
pub fn ray_sphere(o: [f32; 3], d: [f32; 3], c: [f32; 3], r: f32, max: f32) -> Option<f32> {
    let oc = [c[0] - o[0], c[1] - o[1], c[2] - o[2]];
    let t = oc[0] * d[0] + oc[1] * d[1] + oc[2] * d[2];
    if t < 0.0 || t > max + r {
        return None;
    }
    let q = [o[0] + d[0] * t - c[0], o[1] + d[1] * t - c[1], o[2] + d[2] * t - c[2]];
    let d2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
    (d2 <= r * r).then(|| (t - (r * r - d2).sqrt()).max(0.0)).filter(|&t| t <= max)
}
/// Wer bekommt Beute? Alle mit Anteil ≥ LOOT_SHARE_PCT am Gesamtschaden.
pub fn loot_share(damage: &[(u32, f32)]) -> Vec<u32> {
    let total: f32 = damage.iter().map(|d| d.1).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    damage.iter().filter(|d| d.1 / total * 100.0 >= LOOT_SHARE_PCT).map(|d| d.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schaden_und_beute() {
        let s = Stats { crit_chance: 0.0, crit_damage: 50.0, ..Default::default() };
        let (d, c) = player_hit(&s, 5);
        assert!(!c && (d - FIST_DMG).abs() < 1e-6);
        let s = Stats { crit_chance: 100.0, crit_damage: 50.0, weapon_damage: 15.0, strength: 100.0, ..Default::default() };
        let (d, c) = player_hit(&s, 9999);
        assert!(c && (d - 20.0 * 2.0 * 1.5).abs() < 1e-3);
        assert!((taken(100.0, 100.0) - 50.0).abs() < 1e-4);
        assert_eq!((boss_phase(1.0), boss_phase(0.5), boss_phase(0.1)), (0, 1, 2));
        assert!(ray_sphere([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [5.0, 0.2, 0.0], 0.5, 10.0).is_some());
        assert!(ray_sphere([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [5.0, 2.0, 0.0], 0.5, 10.0).is_none());
        assert!(ray_sphere([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [-5.0, 0.0, 0.0], 0.5, 10.0).is_none());
        let who = loot_share(&[(1, 900.0), (2, 95.0), (3, 5.0)]);
        assert_eq!(who, vec![1], "2 hat 9,5 % – knapp zu wenig");
        assert_eq!(loot_share(&[(1, 50.0), (2, 50.0)]), vec![1, 2]);
    }
}
