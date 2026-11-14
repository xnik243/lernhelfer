//! Alle Zahlen der Wirtschaft an EINER Stelle. Server und Browser benutzen dieselbe Datei.
use crate::items::{self, Path, BLUEPRINT, COSMETICS};
use crate::world::{self, BLOCKS, F_BOUNCE, F_GLOW, F_RARE, F_SEMESTER, F_SPEED, F_WEEKLY};
use serde::{Deserialize, Serialize};

// --- Verdienen (nur vom Lernhelfer-Server bestätigt) ---
pub const FOCUS_TALER: u64 = 1; // je Fokus-Minute
pub const FOCUS_DAILY_CAP: u32 = 240;
pub const CHECK_TALER: u64 = 15;
pub const CHECK_FUNDE: u32 = 1;
pub const CARD_TALER: u64 = 2;
pub const CARD_DAILY_CAP: u32 = 60;
pub const EXAM_TALER: u64 = 100;
pub const EXAM_BLUEPRINTS: u32 = 1;
pub const CHALLENGE_TALER: u64 = 25;
pub const STREAK_PCT_PER_DAY: u64 = 5;
pub const STREAK_PCT_CAP: u64 = 50;
pub const EXAM_PHASE_DAYS: i32 = 14;
pub const EXAM_PHASE_MULT: u64 = 2;

// --- Ausgeben ---
pub const AUCTION_FEE_PCT: u64 = 5;
pub const AUCTION_HOURS: [u32; 2] = [24, 72];
pub const BID_STEP_PCT: u64 = 5;
pub const AUCTION_MAX_PRICE: u64 = 1_000_000;
pub const PLOT_SIZES: [u8; 3] = [16, 24, 32];
pub const PLOT_COST: [u64; 3] = [0, 300, 900];
pub const PLOT_LEVEL: [u32; 3] = [1, 5, 10];
pub const PLOT_BLUEPRINTS: [u32; 3] = [0, 0, 1];
pub const GROUP_COST: u64 = 200;
pub const GROUP_MAX: usize = 12;
pub const STARTER_KIT: [(u8, u32); 3] = [(world::BRETTER, 48), (world::ZIEGEL, 24), (world::GLAS, 8)];
/// (Schlüssel, Name, Ziel in Talern, Bit im Weltzustand)
pub const PROJECTS: [(&str, &str, u64, u8); 3] = [
    ("bruecke", "Brücke zum Felsriff", 3000, world::P_BRIDGE),
    ("turm", "Campanile", 5000, world::P_TOWER),
    ("insel", "Neue Insel", 8000, world::P_ISLAND),
];
pub const WORLD_GOAL_WEEKLY: u64 = 2000; // gemeinsam erlernte Taler je Woche
pub const REGIONS: [&str; 4] = ["Nordufer", "Südwiesen", "Westhain", "Ostkap"];
pub const CROWN_DAYS: i64 = 7;
pub const SUBJECT_CAPE_CHECKS: u32 = 100;
/// Letzter Tag, an dem Semester-Stücke in Funden liegen (28.02.2027).
pub const SEMESTER_END_DAY: i64 = 20_877;

// --- Server-Grenzen ---
pub const REACH: f32 = 7.0;
pub const BLOCK_COOLDOWN_MS: u64 = 60;
pub const CHAT_COOLDOWN_MS: u64 = 1500;
pub const MAX_SPEED: f32 = 14.0; // Blöcke je Sekunde, großzügig (Sprungfeder, Laufband)
pub const VIEW_CHUNKS: i32 = 7; // Vorgabe; Spieler wählen 4..=MAX_VIEW_CHUNKS
pub const MAX_VIEW_CHUNKS: i32 = 12;
pub const PLAYER_VIEW: f32 = 96.0;

// --- Auffälligkeiten im Handel ---
pub const FLAG_PRICE_FACTOR: u64 = 10; // Preis über dem Zehnfachen des Richtwerts …
pub const FLAG_MIN_PRICE: u64 = 200; // … und mindestens so viele Taler
pub const FLAG_PAIR_DAYS: i64 = 7;
pub const FLAG_PAIR_SUM: u64 = 600; // so viel fließt in 7 Tagen von A nach B

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Focus,
    Check,
    Card,
    Exam,
    Challenge,
}

impl Kind {
    pub fn from_key(s: &str) -> Option<Kind> {
        Some(match s {
            "fokus" => Kind::Focus,
            "check" => Kind::Check,
            "karte" => Kind::Card,
            "klausur" => Kind::Exam,
            "challenge" => Kind::Challenge,
            _ => return None,
        })
    }
}

/// Ein vom Lernhelfer gemeldetes Lern-Ereignis.
#[derive(Clone, Copy, Debug)]
pub struct Earn {
    pub kind: Kind,
    pub amount: u32,          // Minuten, Karten oder Stück
    pub streak: u32,          // Lernserie in Tagen zum Zeitpunkt
    pub exam_in: Option<i32>, // Tage bis zur Prüfung im Fach des Ereignisses
    pub grade: Option<u8>,    // Note der Probeklausur
}

/// Was an einem Tag schon gezählt wurde.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DayUse {
    pub focus: u32,
    pub cards: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Payout {
    pub taler: u64,
    pub funde: u32,
    pub blueprints: u32,
    pub gold_frame: bool,
    pub counted: u32,
}

pub fn streak_pct(days: u32) -> u64 {
    (days as u64 * STREAK_PCT_PER_DAY).min(STREAK_PCT_CAP)
}
pub fn exam_phase(exam_in: Option<i32>) -> bool {
    exam_in.is_some_and(|d| (0..=EXAM_PHASE_DAYS).contains(&d))
}

/// Taler für ein Ereignis: Grundwert × (100 % + Serie) × Prüfungszeit, abgerundet. Tagesgrenzen werden in `used` fortgeschrieben.
pub fn payout(e: &Earn, used: &mut DayUse) -> Payout {
    let (counted, rate) = match e.kind {
        Kind::Focus => {
            let c = e.amount.min(FOCUS_DAILY_CAP.saturating_sub(used.focus));
            used.focus += c;
            (c, FOCUS_TALER)
        }
        Kind::Card => {
            let c = e.amount.min(CARD_DAILY_CAP.saturating_sub(used.cards));
            used.cards += c;
            (c, CARD_TALER)
        }
        Kind::Check => (e.amount, CHECK_TALER),
        Kind::Exam => (e.amount, EXAM_TALER),
        Kind::Challenge => (e.amount, CHALLENGE_TALER),
    };
    let mult = if exam_phase(e.exam_in) { EXAM_PHASE_MULT } else { 1 };
    Payout {
        taler: counted as u64 * rate * (100 + streak_pct(e.streak)) * mult / 100,
        funde: if e.kind == Kind::Check { counted * CHECK_FUNDE } else { 0 },
        blueprints: if e.kind == Kind::Exam { counted * EXAM_BLUEPRINTS } else { 0 },
        gold_frame: e.kind == Kind::Exam && counted > 0 && e.grade == Some(1),
        counted,
    }
}

/// Stufe wie in der Arena des Lernhelfers.
pub fn level(xp: u64) -> u32 {
    ((xp / 25) as f64).sqrt().floor() as u32 + 1
}

/// Gebühr des Auktionshauses, aufgerundet; verschwindet aus dem Spiel.
pub fn auction_fee(price: u64) -> u64 {
    (price * AUCTION_FEE_PCT).div_ceil(100)
}
pub fn min_bid(start: u64, current: Option<u64>) -> u64 {
    match current {
        None => start.max(1),
        Some(c) => c + (c * BID_STEP_PCT / 100).max(1),
    }
}
/// Verdächtig hoher Preis für den Gegenstand?
pub fn overpriced(item: u16, qty: u32, price: u64) -> bool {
    price >= FLAG_MIN_PRICE && price > items::item_value(item) * qty as u64 * FLAG_PRICE_FACTOR
}

/// Erlernte Stücke aus Stufe und bester Serie.
pub fn milestones(level: u32, best_streak: u32) -> Vec<u16> {
    let mut v = Vec::new();
    for (l, c) in [(5, items::CAPE_L5), (10, items::CAPE_L10), (20, items::CAPE_L20), (30, items::CAPE_L30)] {
        if level >= l {
            v.push(c);
        }
    }
    for (s, c) in [(7, items::TRAIL_FLAMME_1), (30, items::TRAIL_FLAMME_2), (100, items::TRAIL_FLAMME_3)] {
        if best_streak >= s {
            v.push(c);
        }
    }
    v
}
/// Fach-Umhang: eine von vier Farben, fest je Fachname.
pub fn subject_cape(fach: &str) -> u16 {
    items::CAPE_FACH_A + (fach.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 4) as u16
}

/// Die zwei Wochenblöcke, fest aus der ISO-Woche (Jahr*100 + Woche).
pub fn weekly_blocks(week: u32) -> [u8; 2] {
    let pool: Vec<u8> = BLOCKS.iter().filter(|b| b.flags & F_WEEKLY != 0).map(|b| b.id).collect();
    let n = pool.len() as u32;
    let h = week.wrapping_mul(2_654_435_761).rotate_left(13) ^ week;
    let a = h % n;
    let b = (a + 1 + (h / n) % (n - 1)) % n;
    [pool[a as usize], pool[b as usize]]
}

/// Preis im Laden; None = dort nicht zu haben.
pub fn shop_price(item: u16, week: u32) -> Option<u64> {
    if item < 256 {
        let b = world::block(item as u8)?;
        if b.price == 0 || (b.flags & F_WEEKLY != 0 && !weekly_blocks(week).contains(&b.id)) {
            return None;
        }
        return Some(b.price as u64);
    }
    let c = items::cosmetic(item)?;
    (c.path == Path::Shop && c.price > 0).then_some(c.price as u64)
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loot {
    pub item: u16,
    pub qty: u32,
    pub rare: bool,
}

/// Gewichte der Fund-Kiste in Promille: (Grenze, Art).
pub const LOOT_WEIGHTS: [u64; 7] = [600, 200, 80, 40, 40, 30, 10];

/// Öffnet einen Fund. Meist Baumaterial, selten Licht, Mechanik, Sonderfarben, Cosmetics oder Semester-Stücke.
pub fn open_fund(rng: &mut Rng, day: i64) -> Loot {
    let blocks_with = |f: u16, not: u16| -> Vec<u16> { BLOCKS.iter().filter(|b| b.flags & F_RARE != 0 && b.flags & f != 0 && b.flags & not == 0).map(|b| b.id as u16).collect() };
    let drops = |p: Path| -> Vec<u16> { COSMETICS.iter().filter(|c| c.path == p).map(|c| c.id).collect() };
    let mut r = rng.below(1000);
    let mut class = 0;
    for (i, w) in LOOT_WEIGHTS.iter().enumerate() {
        if r < *w {
            class = i;
            break;
        }
        r -= w;
    }
    let (pool, qty): (Vec<u16>, u32) = match class {
        0 => (items::standard_blocks().into_iter().map(u16::from).collect(), 16),
        1 => (items::standard_blocks().into_iter().map(u16::from).collect(), 32),
        2 => (blocks_with(F_GLOW, F_SEMESTER), 4),
        3 => (blocks_with(F_BOUNCE | F_SPEED, 0), 2),
        4 => (BLOCKS.iter().filter(|b| b.flags & F_RARE != 0 && b.flags & (F_GLOW | F_BOUNCE | F_SPEED | F_SEMESTER) == 0).map(|b| b.id as u16).collect(), 4),
        5 => (drops(Path::Drop), 1),
        _ if day <= SEMESTER_END_DAY => {
            let mut p = drops(Path::Semester);
            p.push(world::SEMESTERSTEIN as u16);
            (p, 1)
        }
        _ => (drops(Path::Drop), 1),
    };
    let item = pool[rng.below(pool.len() as u64) as usize];
    let qty = if item == world::SEMESTERSTEIN as u16 { 8 } else { qty };
    Loot { item, qty, rare: class >= 2 }
}

/// Nächste Ausbaustufe des Grundstücks: (neue Größe, Taler, Baupläne, nötige Stufe).
pub fn plot_upgrade(size: u8) -> Option<(u8, u64, u32, u32)> {
    let i = PLOT_SIZES.iter().position(|&s| s == size)? + 1;
    (i < PLOT_SIZES.len()).then(|| (PLOT_SIZES[i], PLOT_COST[i], PLOT_BLUEPRINTS[i], PLOT_LEVEL[i]))
}

pub const _BLUEPRINT_ITEM: u16 = BLUEPRINT;

#[cfg(test)]
mod tests {
    use super::*;

    fn e(kind: Kind, amount: u32, streak: u32, exam_in: Option<i32>) -> Earn {
        Earn { kind, amount, streak, exam_in, grade: None }
    }

    #[test]
    fn taler_genau() {
        let mut u = DayUse::default();
        assert_eq!(payout(&e(Kind::Focus, 25, 0, None), &mut u).taler, 25);
        assert_eq!(payout(&e(Kind::Check, 1, 0, None), &mut u), Payout { taler: 15, funde: 1, blueprints: 0, gold_frame: false, counted: 1 });
        assert_eq!(payout(&e(Kind::Card, 10, 0, None), &mut u).taler, 20);
        assert_eq!(payout(&e(Kind::Challenge, 1, 0, None), &mut u).taler, 25);
        let p = payout(&Earn { kind: Kind::Exam, amount: 1, streak: 0, exam_in: None, grade: Some(1) }, &mut u);
        assert_eq!((p.taler, p.blueprints, p.gold_frame), (100, 1, true));
        assert!(!payout(&Earn { kind: Kind::Exam, amount: 1, streak: 0, exam_in: None, grade: Some(2) }, &mut u).gold_frame);
    }

    #[test]
    fn tagesgrenzen() {
        let mut u = DayUse::default();
        assert_eq!(payout(&e(Kind::Focus, 200, 0, None), &mut u).taler, 200);
        assert_eq!(payout(&e(Kind::Focus, 100, 0, None), &mut u).taler, 40, "nur noch 40 bis 240");
        assert_eq!(payout(&e(Kind::Focus, 45, 0, None), &mut u).taler, 0);
        assert_eq!(u.focus, 240);
        assert_eq!(payout(&e(Kind::Card, 59, 0, None), &mut u).taler, 118);
        assert_eq!(payout(&e(Kind::Card, 5, 0, None), &mut u), Payout { taler: 2, counted: 1, ..Default::default() });
        assert_eq!(payout(&e(Kind::Card, 1, 0, None), &mut u).taler, 0);
        // neuer Tag, neue Grenze
        assert_eq!(payout(&e(Kind::Card, 60, 0, None), &mut DayUse::default()).taler, 120);
    }

    #[test]
    fn serie_und_pruefungszeit() {
        assert_eq!((streak_pct(0), streak_pct(1), streak_pct(7), streak_pct(10), streak_pct(300)), (0, 5, 35, 50, 50));
        let mut u = DayUse::default();
        assert_eq!(payout(&e(Kind::Check, 1, 3, None), &mut u).taler, 17, "15 × 1,15 = 17,25 → 17");
        assert_eq!(payout(&e(Kind::Check, 1, 10, None), &mut u).taler, 22, "15 × 1,5 = 22,5 → 22");
        assert_eq!(payout(&e(Kind::Check, 1, 10, Some(14)), &mut u).taler, 45, "15 × 1,5 × 2");
        assert_eq!(payout(&e(Kind::Check, 1, 0, Some(0)), &mut u).taler, 30);
        assert_eq!(payout(&e(Kind::Check, 1, 0, Some(15)), &mut u).taler, 15, "15 Tage vorher zählt noch nicht");
        assert_eq!(payout(&e(Kind::Check, 1, 0, Some(-1)), &mut u).taler, 15, "nach der Prüfung nicht mehr");
        assert_eq!(payout(&e(Kind::Focus, 240, 99, Some(3)), &mut DayUse::default()).taler, 720);
        assert_eq!(payout(&e(Kind::Card, 7, 1, None), &mut DayUse::default()).taler, 14, "14 × 1,05 = 14,7 → 14");
    }

    #[test]
    fn stufe_und_meilensteine() {
        assert_eq!((level(0), level(24), level(25), level(100), level(2025), level(21025)), (1, 1, 2, 3, 10, 30));
        assert_eq!(milestones(4, 6), vec![]);
        assert_eq!(milestones(10, 30), vec![items::CAPE_L5, items::CAPE_L10, items::TRAIL_FLAMME_1, items::TRAIL_FLAMME_2]);
        assert_eq!(milestones(30, 100).len(), 7);
        assert!((items::CAPE_FACH_A..=items::CAPE_FACH_D).contains(&subject_cape("Mathematik")));
        assert_eq!(subject_cape("VWL"), subject_cape("VWL"));
    }

    #[test]
    fn auktion_gebuehr_und_gebote() {
        assert_eq!((auction_fee(100), auction_fee(1), auction_fee(19), auction_fee(20), auction_fee(21), auction_fee(999)), (5, 1, 1, 1, 2, 50));
        assert_eq!((min_bid(10, None), min_bid(0, None), min_bid(10, Some(10)), min_bid(10, Some(100)), min_bid(1, Some(1000))), (10, 1, 11, 105, 1050));
        assert!(overpriced(world::GLUEH as u16, 1, 201) && !overpriced(world::GLUEH as u16, 1, 199) && !overpriced(world::GLUEH as u16, 4, 800));
    }

    #[test]
    fn laden_und_wochenbloecke() {
        let mut seen = std::collections::HashSet::new();
        for w in 202601..202653 {
            let [a, b] = weekly_blocks(w);
            assert_ne!(a, b);
            assert_eq!(weekly_blocks(w), [a, b], "fest je Woche");
            assert!(world::block(a).unwrap().flags & F_WEEKLY != 0 && world::block(b).unwrap().flags & F_WEEKLY != 0);
            seen.insert(a);
            seen.insert(b);
            assert_eq!(shop_price(a as u16, w), Some(world::block(a).unwrap().price as u64));
        }
        assert_eq!(seen.len(), 8, "übers Jahr kommt jeder Wochenblock dran");
        let w = 202646;
        let other = BLOCKS.iter().find(|b| b.flags & F_WEEKLY != 0 && !weekly_blocks(w).contains(&b.id)).unwrap();
        assert_eq!(shop_price(other.id as u16, w), None);
        assert_eq!(shop_price(world::BRETTER as u16, w), Some(1));
        assert_eq!(shop_price(world::GLUEH as u16, w), None, "Seltenes gibt es nie im Laden");
        assert_eq!(shop_price(items::HEAD_DOKTOR, w), Some(150));
        assert_eq!(shop_price(items::HEAD_HALO, w), None);
        assert_eq!(shop_price(items::CAPE_L5, w), None);
        assert_eq!(shop_price(BLUEPRINT, w), None);
    }

    #[test]
    fn funde() {
        assert_eq!(LOOT_WEIGHTS.iter().sum::<u64>(), 1000);
        let mut rng = Rng(42);
        let (mut rare, mut sem, mut cos) = (0, 0, 0);
        for _ in 0..20_000 {
            let l = open_fund(&mut rng, 20_000);
            assert!(items::exists(l.item) && l.qty > 0);
            assert!(l.rare == items::tradeable(l.item), "selten = handelbar: {}", l.item);
            rare += l.rare as u32;
            sem += (l.item == world::SEMESTERSTEIN as u16 || items::cosmetic(l.item).is_some_and(|c| c.path == Path::Semester)) as u32;
            cos += (l.item >= 300) as u32;
            assert!(items::cosmetic(l.item).is_none_or(|c| c.path != Path::Earned && c.path != Path::Shop));
        }
        assert!((3600..4400).contains(&rare), "rund 20 % selten: {rare}");
        assert!((120..290).contains(&sem), "rund 1 % Semester: {sem}");
        assert!(cos > 500);
        // nach Semesterende gibt es die Semester-Stücke nicht mehr
        let mut rng = Rng(42);
        for _ in 0..20_000 {
            let l = open_fund(&mut rng, SEMESTER_END_DAY + 1);
            assert!(l.item != world::SEMESTERSTEIN as u16 && items::cosmetic(l.item).is_none_or(|c| c.path != Path::Semester));
        }
        assert_eq!(SEMESTER_END_DAY, crate::time::days_from_civil(2027, 2, 28));
    }

    #[test]
    fn grundstueck() {
        assert_eq!(plot_upgrade(16), Some((24, 300, 0, 5)));
        assert_eq!(plot_upgrade(24), Some((32, 900, 1, 10)));
        assert_eq!(plot_upgrade(32), None);
        assert_eq!(plot_upgrade(17), None);
    }
}
