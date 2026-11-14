//! SkyBlock-Schicht: Fach-Splitter, Fähigkeiten, Sammlungen, Lern-Minions, Verdichten, Bazaar, Bank, Fokus-Mana, Werkzeuge,
//! NPCs und Quests. Nur Regeln und Formeln; die Daten (Rezepte, Dialoge, Quests) liegen in `content.rs`.
//! Alles Zeitabhängige wird faul aus Zeitstempeln gerechnet: der Server tickt dafür nie.
use crate::world;
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------- Gegenstände (u16, ab 1000) ----------
pub const BAUSTAB: [u16; 3] = [1000, 1001, 1002]; // 9 / 25 / 64 Blöcke
pub const HAMMER: u16 = 1010;
pub const HAKEN: u16 = 1011;
pub const SPRUNGSTAB: u16 = 1012;
pub const KOMPASS: u16 = 1013;
pub use crate::world::{AXT, SPITZHACKE}; // 1014, 1015: beschleunigen das Abbauen (world::mine_secs)
pub const SHARD: u16 = 1100; // + Fach (0..5)
pub const FOKUS_KRISTALL: u16 = 1110; // Fokus-Kristall
pub const ENERGIE: u16 = 1120; // Lern-Energie-Zelle = ENERGIE_H Stunden Treibstoff
pub const MINION: u16 = 1200; // + Art * 16 + Stufe (1..=11)
pub const VERDICHTET: u16 = 1400; // + Index in VERDICHTBAR
pub const SUPER: u16 = 1450; // + Index in VERDICHTBAR
pub const STACK: u32 = 160; // so viele ergeben eine Stufe höher

pub const FAECHER: [&str; 5] = ["Mathematik", "Sprachen", "Naturwissenschaften", "Gesellschaft", "Allgemein"];
pub const FACH_KEYS: [&str; 5] = ["math", "languages", "science", "humanities", "general"];

/// Fach aus dem Freitext des Lernhelfers (Kursname) ableiten.
pub fn subject_of(fach: &str) -> u8 {
    let f = fach.to_lowercase();
    if let Some(i) = FACH_KEYS.iter().position(|k| f == *k) {
        return i as u8;
    }
    const W: [&[&str]; 4] = [
        &["mathe", "statist", "rechnungswesen", "buchhalt", "finanz", "informatik", "analysis", "algebra", "ökonometr", "oekonometr", "kosten"],
        &["sprach", "englisch", "english", "deutsch", "französ", "franzoes", "spanisch", "italien", "latein", "business english", "kommunikation"],
        &["physik", "chemie", "bio", "natur", "medizin", "geo", "umwelt", "technik"],
        &["recht", "gesch", "politik", "sozio", "philos", "wirtschaft", "bwl", "vwl", "management", "marketing", "organisation", "ethik", "psycho"],
    ];
    W.iter().position(|ws| ws.iter().any(|w| f.contains(w))).map_or(4, |i| i as u8)
}

// ---------- Fähigkeiten ----------
pub const SKILLS: [&str; 7] = ["Mathematik", "Sprachen", "Naturwissenschaften", "Gesellschaft", "Fokus", "Baukunst", "Handel"];
pub const SK_FOKUS: u8 = 4;
pub const SK_BAU: u8 = 5;
pub const SK_HANDEL: u8 = 6;
pub const MAX_LEVEL: u32 = 50;

/// Gesamt-EP, um Stufe `l` zu erreichen (Stufe 1 = 0 EP).
pub fn xp_for(l: u32) -> u64 {
    let l = l.clamp(1, MAX_LEVEL) as u64;
    25 * (l - 1) * l
}
pub fn level_of(xp: u64) -> u32 {
    (1..=MAX_LEVEL).rev().find(|&l| xp_for(l) <= xp).unwrap_or(1)
}
/// +0,5 % Ertrag je Stufe über 1, in Tausendsteln.
pub fn yield_permille(level: u32) -> u64 {
    1000 + 5 * (level.max(1) as u64 - 1)
}
pub fn with_yield(n: u64, level: u32) -> u64 {
    n * yield_permille(level) / 1000
}
/// Höchstes Fokus-Mana: 100 + 1 je Stufe über 1 in allen Fähigkeiten, Fokus zählt dreifach.
pub fn max_mana(levels: &[u32; 7]) -> u32 {
    100 + levels.iter().map(|l| l - 1).sum::<u32>() + 2 * (levels[SK_FOKUS as usize] - 1)
}
/// Minion-Plätze: 2 zu Beginn, je 15 Fähigkeitsstufen einer mehr, höchstens 10.
pub fn minion_slots(levels: &[u32; 7]) -> u32 {
    (2 + (levels.iter().sum::<u32>() - 7) / 15).min(10)
}

// ---------- Sammlungen ----------
pub const COLL_TIERS: [u64; 9] = [50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000];
pub fn coll_tier(n: u64) -> u8 {
    COLL_TIERS.iter().filter(|&&t| n >= t).count() as u8
}

// ---------- Verdichten ----------
/// Grundgegenstand, Name der verdichteten und der super-verdichteten Stufe.
pub const VERDICHTBAR: [(u16, &str, &str); 14] = [
    (world::BRUCHSTEIN as u16, "Verdichteter Bruchstein", "Super-verdichteter Bruchstein"),
    (world::STAMM as u16, "Verdichtetes Holz", "Super-verdichtetes Holz"),
    (world::SAND as u16, "Verdichteter Sand", "Super-verdichteter Sand"),
    (world::LAUB as u16, "Verdichtetes Laub", "Super-verdichtetes Laub"),
    (world::ERDE as u16, "Verdichtete Erde", "Super-verdichtete Erde"),
    (world::FELS as u16, "Verdichteter Fels", "Super-verdichteter Fels"),
    (world::SAPHIR as u16, "Verdichteter Mathe-Saphir", "Super-verdichteter Mathe-Saphir"),
    (world::SPRACHGOLD as u16, "Verdichtetes Sprach-Gold", "Super-verdichtetes Sprach-Gold"),
    (world::SMARAGD as u16, "Verdichteter Natur-Smaragd", "Super-verdichteter Natur-Smaragd"),
    (SHARD, "Verdichteter Mathe-Splitter", "Super-verdichteter Mathe-Splitter"),
    (SHARD + 1, "Verdichteter Sprach-Splitter", "Super-verdichteter Sprach-Splitter"),
    (SHARD + 2, "Verdichteter Natur-Splitter", "Super-verdichteter Natur-Splitter"),
    (SHARD + 3, "Verdichteter Gesellschafts-Splitter", "Super-verdichteter Gesellschafts-Splitter"),
    (SHARD + 4, "Verdichteter Wissens-Splitter", "Super-verdichteter Wissens-Splitter"),
];
/// Was entsteht aus 160 Stück von `item`?
pub fn compact_of(item: u16) -> Option<u16> {
    if let Some(i) = VERDICHTBAR.iter().position(|v| v.0 == item) {
        return Some(VERDICHTET + i as u16);
    }
    (VERDICHTET..VERDICHTET + VERDICHTBAR.len() as u16).contains(&item).then(|| item - VERDICHTET + SUPER)
}
pub fn is_compacted(item: u16) -> bool {
    (VERDICHTET..VERDICHTET + VERDICHTBAR.len() as u16).contains(&item) || (SUPER..SUPER + VERDICHTBAR.len() as u16).contains(&item)
}

/// Rezept: aus `ins` wird `n` mal `out`; `unlock` = (Sammlung, Stufe), die es freischaltet.
pub struct Recipe {
    pub out: u16,
    pub n: u32,
    pub ins: &'static [(u16, u32)],
    pub unlock: Option<(u16, u8)>,
}
/// Alle Rezepte: Verdichtungen werden erzeugt, der Rest kommt aus den Inhaltsdaten.
pub static RECIPES: LazyLock<Vec<Recipe>> = LazyLock::new(|| {
    let mut v: Vec<Recipe> = Vec::new();
    for (i, (base, _, _)) in VERDICHTBAR.iter().enumerate() {
        let c = VERDICHTET + i as u16;
        v.push(Recipe { out: c, n: 1, ins: Box::leak(Box::new([(*base, STACK)])), unlock: Some((*base, 1)) });
        v.push(Recipe { out: SUPER + i as u16, n: 1, ins: Box::leak(Box::new([(c, STACK)])), unlock: Some((*base, 5)) });
    }
    v.extend(crate::content::RECIPES.iter().map(|r| Recipe { out: r.out, n: r.n, ins: r.ins, unlock: r.unlock }));
    v.extend(crate::equipment_data::EQUIP_RECIPES.iter().map(|r| Recipe { out: r.out, n: r.n, ins: r.ins, unlock: r.unlock }));
    v
});

// ---------- Lern-Minions ----------
pub struct MinionKind {
    pub name: &'static str,
    pub product: u16,
    pub secs: u32, // Sekunden je Stück auf Stufe I mit Treibstoff
    pub rgb: [u8; 3],
}
pub const MINIONS: [MinionKind; 4] = [
    MinionKind { name: "Stein-Minion", product: world::BRUCHSTEIN as u16, secs: 28, rgb: [150, 150, 158] },
    MinionKind { name: "Holz-Minion", product: world::STAMM as u16, secs: 40, rgb: [150, 110, 70] },
    MinionKind { name: "Sand-Minion", product: world::SAND as u16, secs: 32, rgb: [226, 208, 158] },
    MinionKind { name: "Laub-Minion", product: world::LAUB as u16, secs: 36, rgb: [80, 160, 84] },
];
pub const MINION_TIERS: u8 = 11;
pub const UNFUELED_PCT: i64 = 20; // ohne Lern-Energie 20 % Tempo, mit Treibstoff 100 % (5×)
pub const ENERGIE_H: i64 = 6;
pub const FUEL_CAP_H: i64 = 72;
pub const HOUR_MS: i64 = 3_600_000;

pub fn minion_item(kind: u8, tier: u8) -> u16 {
    MINION + kind as u16 * 16 + tier.clamp(1, MINION_TIERS) as u16
}
/// (Art, Stufe) eines Minion-Gegenstands.
pub fn minion_of(item: u16) -> Option<(u8, u8)> {
    let k = item.checked_sub(MINION)?;
    let (kind, tier) = ((k / 16) as u8, (k % 16) as u8);
    ((kind as usize) < MINIONS.len() && (1..=MINION_TIERS).contains(&tier)).then_some((kind, tier))
}
/// Millisekunden je Stück bei voller Fahrt: jede Stufe 6 % schneller (Stufe XI: 40 %).
pub fn period_ms(kind: u8, tier: u8) -> i64 {
    MINIONS[kind as usize].secs as i64 * 1000 * (100 - 6 * (tier.clamp(1, MINION_TIERS) as i64 - 1)) / 100
}
pub fn minion_cap(tier: u8) -> u32 {
    64 * (tier as u32 + 1)
}

/// Faule Minion-Rechnung zwischen zwei Besuchen. `acc` = angesammelte volle-Fahrt-Millisekunden, die noch kein Stück ergaben.
/// Liefert (neue Stücke, neues acc). Mit Treibstoff zählt die Zeit voll, ohne zu 20 %.
pub fn minion_step(kind: u8, tier: u8, last: i64, now: i64, fuel_until: i64, acc: i64, stored: u32) -> (u32, i64) {
    if now <= last {
        return (0, acc);
    }
    let fueled = (fuel_until.min(now) - last).max(0);
    let eff = acc + fueled + (now - last - fueled) * UNFUELED_PCT / 100;
    let p = period_ms(kind, tier);
    let room = minion_cap(tier).saturating_sub(stored) as i64;
    let n = (eff / p).min(room);
    (n as u32, if n == room { 0 } else { eff - n * p })
}
/// Neuer Treibstoff-Endzeitpunkt nach `ms` Lern-Energie; mehr als 72 h Vorrat verfällt. Liefert auch die angerechnete Zeit.
pub fn add_fuel(fuel_until: i64, now: i64, ms: i64) -> (i64, i64) {
    let start = fuel_until.max(now);
    let end = (start + ms.max(0)).min(now + FUEL_CAP_H * HOUR_MS).max(start);
    (end, end - start)
}
/// Lern-Energie aus dem Lernhelfer in Millisekunden: 15 Fokus-Minuten oder 20 Karten = 24 h.
pub fn fuel_ms(kind: &str, amount: u32) -> i64 {
    match kind {
        "fokus" => amount as i64 * 24 * HOUR_MS / 15,
        "karte" => amount as i64 * 24 * HOUR_MS / 20,
        _ => 0,
    }
}

// ---------- Bazaar ----------
pub const BAZAAR_TAX_BP: i64 = 150; // 1,5 % vom Erlös werden vernichtet
pub fn bazaar_tax(gross: i64) -> i64 {
    (gross * BAZAAR_TAX_BP + 9_999) / 10_000
}
pub fn bazaar_item(item: u16) -> bool {
    (item < 256 && world::block(item as u8).is_some_and(|b| b.price > 0 || b.flags & world::F_RARE != 0) && item as u8 != world::WASSER)
        || (SHARD..SHARD + 5).contains(&item)
        || item == FOKUS_KRISTALL
        || item == ENERGIE
        || is_compacted(item)
}

// ---------- Bank ----------
pub const BANK_RATE_BP: i64 = 100; // 1 % je Tag
pub const BANK_INTEREST_CAP: i64 = 20_000; // ponytail: Zinsen nur auf die ersten 20 000 Taler, gegen Inflation; bei Bedarf Staffel einführen
pub const BANK_ACTIVE_MS: i64 = 48 * HOUR_MS;
/// Zinsen seit `from` (ganze Tage), gezählt nur, solange zuletzt innerhalb von 48 h gelernt wurde. Liefert (Zinsen, neues from).
pub fn bank_interest(balance: i64, from: i64, now: i64, last_study: i64) -> (i64, i64) {
    let day = 24 * HOUR_MS;
    let until = now.min(last_study + BANK_ACTIVE_MS);
    let days = ((until - from).max(0)) / day;
    let mut b = balance;
    for _ in 0..days.min(365) {
        b += b.min(BANK_INTEREST_CAP) * BANK_RATE_BP / 10_000;
    }
    // ungenutzte Zeit verfällt, angebrochene Tage zählen beim nächsten Mal weiter
    let next = if until < now { now.max(from) } else { from + days * day };
    (b - balance, next)
}

// ---------- Fokus-Mana ----------
pub const MANA_REGEN_PER_S: i64 = 2;
pub fn mana_now(stored: u32, at: i64, now: i64, max: u32) -> u32 {
    (stored as i64 + (now - at).max(0) * MANA_REGEN_PER_S / 1000).min(max as i64) as u32
}

// ---------- Werkzeuge ----------
pub struct Tool {
    pub id: u16,
    pub name: &'static str,
    pub mana: u32,
    pub cooldown_ms: i64,
    pub rgb: [u8; 3],
}
pub const TOOLS: [Tool; 11] = [
    Tool { id: BAUSTAB[0], name: "Bau-Stab I", mana: 10, cooldown_ms: 300, rgb: [214, 170, 90] },
    Tool { id: BAUSTAB[1], name: "Bau-Stab II", mana: 10, cooldown_ms: 300, rgb: [120, 200, 240] },
    Tool { id: BAUSTAB[2], name: "Bau-Stab III", mana: 10, cooldown_ms: 300, rgb: [220, 120, 255] },
    Tool { id: HAMMER, name: "Abbau-Hammer", mana: 5, cooldown_ms: 250, rgb: [150, 150, 160] },
    Tool { id: HAKEN, name: "Enterhaken", mana: 0, cooldown_ms: 2000, rgb: [180, 140, 90] },
    Tool { id: SPRUNGSTAB, name: "Vektor-Sprungstab", mana: 15, cooldown_ms: 400, rgb: [110, 90, 230] },
    Tool { id: KOMPASS, name: "Insel-Kompass", mana: 0, cooldown_ms: 0, rgb: [230, 200, 90] },
    Tool { id: SPITZHACKE, name: "Spitzhacke", mana: 0, cooldown_ms: 0, rgb: [140, 150, 170] },
    Tool { id: AXT, name: "Axt", mana: 0, cooldown_ms: 0, rgb: [170, 120, 80] },
    Tool { id: crate::rpg::ERNTE_HACKE, name: "Ernte-Hacke", mana: 0, cooldown_ms: 150, rgb: [150, 120, 70] },
    Tool { id: crate::rpg::SEGEN_HACKE, name: "Segens-Hacke", mana: 0, cooldown_ms: 150, rgb: [120, 220, 140] },
];
pub fn tool(id: u16) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id == id)
}
pub fn wand_limit(id: u16) -> usize {
    match id {
        x if x == BAUSTAB[0] => 9,
        x if x == BAUSTAB[1] => 25,
        x if x == BAUSTAB[2] => 64,
        _ => 0,
    }
}
pub const BLINK: f32 = 6.0;
pub const GRAPPLE_SPEED: f32 = 22.0;

// ---------- NPCs und Quests ----------
pub struct Npc {
    pub id: u8,
    pub name: &'static str,
    /// Standpunkt (Füße) in Weltkoordinaten
    pub at: [f32; 3],
    pub yaw: f32,
    pub rgb: [[u8; 3]; 2], // Kleidung, Akzent
    pub lines: &'static [&'static str],
    pub quest: Option<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Talk(u8),
    Mine(u16, u32),
    /// abbauen zählt erst, wenn ein eigener Minion aufgestellt ist
    MineMinion(u16, u32),
    Craft(u16),
    Warp(u8),
    Visit,
    Learn(u32),
    /// nur Check-Fragen aus dem Lernhelfer
    Check(u32),
    PlaceMinion,
    Deposit(u32),
    Bazaar,
    Ability,
    /// zur eigenen Insel reisen (/is, Portal)
    Home,
    // ---------- Campus-Questbuch (Phasen 16–20) ----------
    /// so viele Blöcke auf der eigenen Insel setzen
    Place(u32),
    /// Lern-Energie in die Minions füllen (fuel/use)
    Fuel,
    /// so viele Rüstungsteile gleichzeitig tragen (der Treffer meldet gleich `need`)
    Equip(u32),
    /// Saaten pflanzen
    Plant(u32),
    /// reife Pflanzen ernten
    Harvest(u32),
    /// etwas 160× verdichten
    Compact,
    /// am Altar verzaubern
    Enchant,
    /// Fähigkeit dieses Werkzeugs benutzen (Bau-Stab, Enterhaken …)
    UseTool(u16),
    /// eine fremde Insel liken
    Like,
    /// Wesen der Art (rpg::MobDef.id) besiegen
    Kill(u8, u32),
    /// eine Truhe auf die eigene Insel stellen
    Chest,
}
impl Goal {
    /// Wie oft muss das Ziel getroffen werden?
    pub fn need(self) -> u32 {
        match self {
            Goal::Mine(_, k) | Goal::MineMinion(_, k) | Goal::Learn(k) | Goal::Check(k) | Goal::Deposit(k) | Goal::Place(k) | Goal::Equip(k)
            | Goal::Plant(k) | Goal::Harvest(k) | Goal::Kill(_, k) => k.max(1),
            _ => 1,
        }
    }
}
pub struct Step {
    pub text: &'static str,
    pub goal: Goal,
    pub taler: u32,
}
pub struct Quest {
    pub id: u8,
    pub name: &'static str,
    pub steps: &'static [Step],
}
pub fn npc(id: u8) -> Option<&'static Npc> {
    crate::content::NPCS.iter().find(|n| n.id == id)
}
pub fn npc_pos(n: &Npc) -> [f32; 3] {
    n.at
}
pub fn quest(id: u8) -> Option<&'static Quest> {
    crate::content::QUESTS.iter().find(|q| q.id == id)
}
/// Dekan Weber: Startgeschenk
pub const DEKAN: u8 = 1;
pub const STARTER_TALER: i64 = 50;
pub const STARTER_FUEL_H: i64 = 6;

/// Schatztruhe (je Truhe und Person einmal am Tag): (Taler, Gegenstand, Menge) aus einer Zufallszahl des Servers.
pub fn treasure(roll: u32) -> (i64, u16, u32) {
    let taler = 10 + (roll % 31) as i64;
    let (item, n) = match roll / 31 % 4 {
        0 => (SHARD + (roll / 124 % 5) as u16, 1 + roll / 620 % 3),
        1 => (FOKUS_KRISTALL, 1),
        2 => (ENERGIE, 1),
        _ => (world::BRUCHSTEIN as u16, 32),
    };
    (taler, item, n)
}

// ---------- Namen ----------
static NAMES: LazyLock<HashMap<u16, String>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for t in &TOOLS {
        m.insert(t.id, t.name.to_string());
    }
    for (i, f) in ["Mathe-Splitter", "Sprach-Splitter", "Natur-Splitter", "Gesellschafts-Splitter", "Wissens-Splitter"].iter().enumerate() {
        m.insert(SHARD + i as u16, f.to_string());
    }
    m.insert(FOKUS_KRISTALL, "Fokus-Kristall".into());
    m.insert(ENERGIE, "Lern-Energie-Zelle".into());
    const ROM: [&str; 11] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI"];
    for (k, mk) in MINIONS.iter().enumerate() {
        for t in 1..=MINION_TIERS {
            m.insert(minion_item(k as u8, t), format!("{} {}", mk.name, ROM[t as usize - 1]));
        }
    }
    for (i, v) in VERDICHTBAR.iter().enumerate() {
        m.insert(VERDICHTET + i as u16, v.1.to_string());
        m.insert(SUPER + i as u16, v.2.to_string());
    }
    for (id, n) in crate::rpg::extra_names() {
        m.insert(id, n.to_string());
    }
    m
});
pub fn name(item: u16) -> Option<&'static str> {
    NAMES.get(&item).map(|s| s.as_str())
}
pub fn all_items() -> impl Iterator<Item = (u16, &'static str)> {
    NAMES.iter().map(|(k, v)| (*k, v.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stufen_und_boni() {
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(xp_for(10)), 10);
        assert_eq!(level_of(xp_for(10) - 1), 9);
        assert_eq!(level_of(u64::MAX / 2), 50);
        let mut l = [1u32; 7];
        assert_eq!((max_mana(&l), minion_slots(&l)), (100, 2));
        l = [50; 7];
        assert_eq!(minion_slots(&l), 10);
        assert_eq!(with_yield(1000, 11), 1050);
        assert_eq!(coll_tier(49), 0);
        assert_eq!(coll_tier(25_000), 9);
    }

    #[test]
    fn faecher() {
        assert_eq!(subject_of("Mathematik für WiWi"), 0);
        assert_eq!(subject_of("Business English"), 1);
        assert_eq!(subject_of("Einführung in die BWL"), 3);
        assert_eq!(subject_of("science"), 2);
        assert_eq!(subject_of("Irgendwas"), 4);
    }

    #[test]
    fn minion_zwoelf_stunden_faul() {
        let h = HOUR_MS;
        let p = period_ms(0, 1);
        // 12 h voll betankt = 5 × so viel wie 12 h ohne Treibstoff
        let (voll, _) = minion_step(0, 1, 0, 12 * h, 99 * h, 0, 0);
        let (leer, _) = minion_step(0, 1, 0, 12 * h, 0, 0, 0);
        assert_eq!(voll, minion_cap(1), "voll betankt läuft das Lager in 12 h voll");
        assert_eq!(leer as i64, ((12 * h / 5) / p).min(minion_cap(1) as i64));
        // halb betankt: 1 h voll + 1 h zu 20 %
        let (n, acc) = minion_step(0, 11, 0, 2 * h, h, 0, 0);
        assert_eq!(n as i64 * period_ms(0, 11) + acc, h + h / 5);
        // in Teilen gerechnet = in einem Stück gerechnet (nichts geht verloren, nichts doppelt)
        let (mut sum, mut acc, mut last) = (0u32, 0i64, 0i64);
        for k in 1..=12 {
            let (n, a) = minion_step(1, 4, last, k * h, 5 * h, acc, sum);
            sum += n;
            acc = a;
            last = k * h;
        }
        assert_eq!(sum, minion_step(1, 4, 0, 12 * h, 5 * h, 0, 0).0);
        assert_eq!(minion_step(0, 1, 5, 5, 0, 0, 0), (0, 0));
        assert!(period_ms(0, 11) < period_ms(0, 1));
        assert_eq!(minion_of(minion_item(3, 11)), Some((3, 11)));
        assert_eq!(minion_of(MINION + 4 * 16 + 1), None);
    }

    #[test]
    fn treibstoff_bank_mana_steuer() {
        let h = HOUR_MS;
        assert_eq!(add_fuel(0, 100 * h, 24 * h), (124 * h, 24 * h));
        assert_eq!(add_fuel(150 * h, 100 * h, 24 * h), (172 * h, 22 * h), "höchstens 72 h Vorrat");
        assert_eq!(fuel_ms("fokus", 15), 24 * h);
        assert_eq!(fuel_ms("karte", 20), 24 * h);
        assert_eq!(fuel_ms("karte", 1) * 20, 24 * h, "auch einzelne Karten zählen");
        let d = 24 * h;
        assert_eq!(bank_interest(1000, 0, 3 * d, 3 * d), (30, 3 * d));
        assert_eq!(bank_interest(1000, 0, 10 * d, 0), (20, 10 * d), "nur 48 h nach dem letzten Lernen");
        assert_eq!(bank_interest(1000, 0, d / 2, d).0, 0);
        assert_eq!(bank_interest(1_000_000, 0, d, d).0, 200, "Deckel");
        assert_eq!(mana_now(10, 0, 5000, 100), 20);
        assert_eq!(mana_now(90, 0, 60_000, 100), 100);
        assert_eq!(bazaar_tax(1000), 15);
        assert_eq!(bazaar_tax(1), 1);
        assert_eq!(bazaar_tax(0), 0);
    }

    #[test]
    fn verdichten_und_namen() {
        let c = compact_of(world::BRUCHSTEIN as u16).unwrap();
        assert_eq!(name(c), Some("Verdichteter Bruchstein"));
        assert_eq!(name(compact_of(c).unwrap()), Some("Super-verdichteter Bruchstein"));
        assert_eq!(compact_of(compact_of(c).unwrap()), None);
        assert_eq!(name(minion_item(0, 1)), Some("Stein-Minion I"));
        assert!(RECIPES.iter().all(|r| name(r.out).is_some() || r.out < 256), "jedes Rezept hat einen Namen");
        assert!(RECIPES.iter().all(|r| r.ins.iter().all(|i| name(i.0).is_some() || (i.0 < 256 && crate::world::block(i.0 as u8).is_some()))), "jede Zutat existiert");
        for e in crate::equipment_data::EQUIP {
            assert!(RECIPES.iter().any(|r| r.out == e.id), "Rezept für {}", e.name);
        }
        assert!(RECIPES.iter().any(|r| r.out == HAMMER));
        assert!(bazaar_item(world::BRUCHSTEIN as u16) && bazaar_item(c) && !bazaar_item(HAMMER));
        assert_eq!((name(SPITZHACKE), name(AXT)), (Some("Spitzhacke"), Some("Axt")));
        for r in 0..2000 {
            let (t, item, n) = treasure(r);
            assert!((10..=40).contains(&t) && n >= 1 && name(item).or(crate::world::block(item as u8).map(|b| b.name)).is_some());
        }
    }
}
