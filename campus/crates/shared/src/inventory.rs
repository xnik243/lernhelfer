//! Inventar-Plätze (Phase 16): 36 Plätze (0..9 Schnellleiste, 9..36 Rucksack), Truhen 27/54, Ausrüstung 7.
//! Reine Rechnung ohne Datenbank. Der Server hält die Zählung `inv(user, item, n)` als Wahrheit; die Plätze sind die
//! Anordnung dieser Zählung (je Gegenstand: Summe der Plätze == Zählung). Jeder Zug hier erhält die Summen exakt
//! (Test `nichts_entsteht_nichts_verschwindet`), also kann es durch Klicken weder Kopien noch Verluste geben.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::Range;

pub const INV_SLOTS: usize = 36;
pub const HOTBAR: usize = 9;
pub const CHEST_SMALL: usize = 27;
pub const CHEST_LARGE: usize = 54;
pub const GEAR_SLOTS: usize = 7;
pub const STACK_MAX: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub item: u16,
    pub n: u32,
}
impl Stack {
    pub const EMPTY: Stack = Stack { item: 0, n: 0 };
    pub fn is_empty(&self) -> bool {
        self.n == 0 || self.item == 0
    }
}

/// Liegt der Gegenstand in Plätzen? Cosmetics (300..400) sind Garderobe, nicht Inventar.
pub fn in_slots(item: u16) -> bool {
    item != 0 && !(300..400).contains(&item)
}
/// Stapelgröße: Werkzeuge, Waffen, Rüstung, Talismane 1, sonst 64.
pub fn stack_max(item: u16) -> u32 {
    if crate::skyblock::tool(item).is_some() || crate::rpg::equip(item).is_some() {
        1
    } else {
        STACK_MAX
    }
}
/// Darf `item` in Ausrüstungs-Platz `slot` (0..4 Rüstung Helm/Brust/Beine/Stiefel, 4..7 Talismane)?
pub fn gear_fits(slot: usize, item: u16) -> bool {
    crate::rpg::equip(item).is_some_and(|e| if slot < 4 { e.slot as usize == slot } else { slot < GEAR_SLOTS && e.slot == crate::rpg::SL_TALISMAN })
}

pub fn count(s: &[Stack], item: u16) -> u64 {
    s.iter().filter(|x| x.item == item && x.n > 0).map(|x| x.n as u64).sum()
}
pub fn totals(s: &[Stack]) -> BTreeMap<u16, u64> {
    let mut m = BTreeMap::new();
    for x in s.iter().filter(|x| !x.is_empty()) {
        *m.entry(x.item).or_insert(0) += x.n as u64;
    }
    m
}
/// So viel von `item` passt noch hinein (in `range`).
pub fn room(s: &[Stack], item: u16, range: Range<usize>) -> u64 {
    let m = stack_max(item);
    s[range].iter().map(|x| if x.is_empty() { m } else if x.item == item { m.saturating_sub(x.n) } else { 0 } as u64).sum()
}

/// Legt `n` Stück ab: erst auf passende Stapel, dann in leere Plätze (in `range`-Reihenfolge). Liefert den Rest, der nicht passte.
pub fn add(s: &mut [Stack], item: u16, mut n: u32, range: Range<usize>) -> u32 {
    let m = stack_max(item);
    for x in s[range.clone()].iter_mut() {
        if n == 0 {
            break;
        }
        if x.item == item && x.n > 0 && x.n < m {
            let k = (m - x.n).min(n);
            x.n += k;
            n -= k;
        }
    }
    for x in s[range].iter_mut() {
        if n == 0 {
            break;
        }
        if x.is_empty() {
            let k = m.min(n);
            *x = Stack { item, n: k };
            n -= k;
        }
    }
    n
}
/// Nimmt `n` Stück heraus, zuerst aus Platz `prefer`, dann von hinten nach vorn. Fehler, wenn nicht genug da ist (dann unverändert).
pub fn remove(s: &mut [Stack], item: u16, mut n: u32, prefer: Option<usize>) -> Result<(), &'static str> {
    if count(s, item) < n as u64 {
        return Err("Nicht genug im Inventar.");
    }
    let order = prefer.into_iter().chain((0..s.len()).rev());
    for i in order {
        if n == 0 {
            break;
        }
        let x = &mut s[i];
        if x.item == item && x.n > 0 {
            let k = x.n.min(n);
            x.n -= k;
            n -= k;
            if x.n == 0 {
                *x = Stack::EMPTY;
            }
        }
    }
    Ok(())
}

/// Plätze an die Zählung angleichen (Wahrheit = `counts`). Fehlendes wird abgelegt, Überzähliges entfernt.
/// Liefert, was nicht mehr hineinpasste (→ Beutel /stash).
pub fn reconcile(s: &mut Vec<Stack>, counts: &BTreeMap<u16, u64>) -> Vec<(u16, u64)> {
    s.resize(INV_SLOTS, Stack::EMPTY);
    for x in s.iter_mut() {
        if x.is_empty() || !in_slots(x.item) {
            *x = Stack::EMPTY;
        }
    }
    let have = totals(s);
    for (&item, &n) in &have {
        let want = counts.get(&item).copied().unwrap_or(0);
        if n > want {
            let _ = remove(s, item, (n - want) as u32, None);
        }
    }
    let mut over = Vec::new();
    for (&item, &want) in counts.iter().filter(|(i, _)| in_slots(**i)) {
        let n = count(s, item);
        if want > n {
            let rest = add(s, item, (want - n).min(u32::MAX as u64) as u32, 0..INV_SLOTS) as u64 + (want - n).saturating_sub(u32::MAX as u64);
            if rest > 0 {
                over.push((item, rest));
            }
        }
    }
    over
}

fn two<'a>(s: &'a mut [Stack], a: usize, b: usize) -> (&'a mut Stack, &'a mut Stack) {
    assert!(a != b);
    if a < b {
        let (l, r) = s.split_at_mut(b);
        (&mut l[a], &mut r[0])
    } else {
        let (l, r) = s.split_at_mut(a);
        (&mut r[0], &mut l[b])
    }
}
/// Ein Zug zwischen zwei Plätzen: `n` Stück von `a` nach `b` (n = 0 oder ≥ Stapel heißt ganzer Stapel).
/// Leeres Ziel → ablegen; gleicher Gegenstand → bis zur Stapelgröße zusammenlegen; anderer Gegenstand und ganzer Stapel → tauschen.
/// `fits(item)` prüft, ob der Gegenstand in `b` darf (Ausrüstung), `fits_a` dasselbe für den Rücktausch nach `a`.
pub fn xfer(a: &mut Stack, b: &mut Stack, n: u32, fits_b: impl Fn(u16) -> bool, fits_a: impl Fn(u16) -> bool) -> Result<(), &'static str> {
    if a.is_empty() {
        return Err("Da liegt nichts.");
    }
    if !fits_b(a.item) {
        return Err("Das passt dort nicht hinein.");
    }
    let n = if n == 0 || n >= a.n { a.n } else { n };
    if b.is_empty() {
        let m = stack_max(a.item).min(n);
        *b = Stack { item: a.item, n: m };
        a.n -= m;
    } else if b.item == a.item {
        let k = stack_max(a.item).saturating_sub(b.n).min(n);
        if k == 0 {
            return Err("Der Stapel ist voll.");
        }
        b.n += k;
        a.n -= k;
    } else if n == a.n {
        if !fits_a(b.item) {
            return Err("Das passt dort nicht hinein.");
        }
        std::mem::swap(a, b);
        return Ok(());
    } else {
        return Err("Der Platz ist belegt.");
    }
    if a.n == 0 {
        *a = Stack::EMPTY;
    }
    Ok(())
}
/// Zug innerhalb eines Behälters.
pub fn move_in(s: &mut [Stack], a: usize, b: usize, n: u32) -> Result<(), &'static str> {
    if a >= s.len() || b >= s.len() {
        return Err("Diesen Platz gibt es nicht.");
    }
    if a == b {
        return Ok(());
    }
    let (x, y) = two(s, a, b);
    xfer(x, y, n, |_| true, |_| true)
}
/// Ganzen Stapel `a` in den Bereich `to` legen (Umschalt-Klick). Liefert, wie viele bewegt wurden.
pub fn quick_in(s: &mut [Stack], a: usize, to: Range<usize>) -> u32 {
    let st = s[a];
    if st.is_empty() {
        return 0;
    }
    s[a] = Stack::EMPTY;
    let rest = add(s, st.item, st.n, to);
    if rest > 0 {
        // was nicht passte, bleibt an seinem Platz (der ist leer, weil wir ihn eben geleert haben)
        s[a] = Stack { item: st.item, n: rest };
    }
    st.n - rest
}
/// Ganzen Stapel `a` aus `src` in den Behälter `dst` legen (Umschalt-Klick zwischen Inventar und Truhe).
pub fn quick_between(src: &mut [Stack], a: usize, dst: &mut [Stack]) -> u32 {
    let st = src[a];
    if st.is_empty() {
        return 0;
    }
    let len = dst.len();
    let rest = add(dst, st.item, st.n, 0..len);
    src[a] = if rest > 0 { Stack { item: st.item, n: rest } } else { Stack::EMPTY };
    st.n - rest
}
/// Sortieren: gleiche Gegenstände zusammenlegen, dann nach Art (Blöcke, Werkzeuge, Ausrüstung, Rest) und Nummer.
pub fn sort(s: &mut [Stack], range: Range<usize>) {
    let key = |i: u16| -> (u8, u16) {
        let k = if i < 256 { 0 } else if crate::skyblock::tool(i).is_some() { 1 } else if crate::rpg::equip(i).is_some() { 2 } else { 3 };
        (k, i)
    };
    let mut t: BTreeMap<(u8, u16), u64> = BTreeMap::new();
    for x in s[range.clone()].iter().filter(|x| !x.is_empty()) {
        *t.entry(key(x.item)).or_insert(0) += x.n as u64;
    }
    for x in s[range.clone()].iter_mut() {
        *x = Stack::EMPTY;
    }
    let mut i = range.start;
    for ((_, item), mut n) in t {
        let m = stack_max(item) as u64;
        while n > 0 && i < range.end {
            let k = n.min(m);
            s[i] = Stack { item, n: k as u32 };
            n -= k;
            i += 1;
        }
    }
}
/// „Alles einlagern“: Rucksack-Stapel (nicht die Schnellleiste), deren Gegenstand schon in der Truhe liegt, wandern hinein.
pub fn quick_stack(inv: &mut [Stack], chest: &mut [Stack]) -> u32 {
    let mut moved = 0;
    for a in HOTBAR..inv.len() {
        if !inv[a].is_empty() && chest.iter().any(|c| c.item == inv[a].item && c.n > 0) {
            moved += quick_between(inv, a, chest);
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::*;

    fn sum(a: &[Stack], b: &[Stack]) -> BTreeMap<u16, u64> {
        let mut m = totals(a);
        for (k, v) in totals(b) {
            *m.entry(k).or_insert(0) += v;
        }
        m
    }

    #[test]
    fn stapel_teilen_zusammenlegen_tauschen() {
        let mut s = vec![Stack::EMPTY; INV_SLOTS];
        assert_eq!(add(&mut s, BRUCHSTEIN as u16, 100, 0..INV_SLOTS), 0);
        assert_eq!((s[0].n, s[1].n), (64, 36));
        move_in(&mut s, 0, 5, 32).unwrap(); // Rechtsklick: Hälfte
        assert_eq!((s[0].n, s[5].n), (32, 32));
        move_in(&mut s, 5, 1, 0).unwrap(); // zusammenlegen bis 64
        assert_eq!((s[1].n, s[5].n), (64, 4));
        add(&mut s, crate::skyblock::SPITZHACKE, 1, 0..INV_SLOTS);
        let h = s.iter().position(|x| x.item == crate::skyblock::SPITZHACKE).unwrap();
        move_in(&mut s, h, 1, 0).unwrap(); // ganzer Stapel auf anderen Gegenstand = tauschen
        assert_eq!(s[1].item, crate::skyblock::SPITZHACKE);
        assert!(move_in(&mut s, 0, 1, 1).is_err(), "halber Stapel auf belegten Platz geht nicht");
        assert_eq!(count(&s, BRUCHSTEIN as u16), 100);
        assert_eq!(stack_max(crate::skyblock::SPITZHACKE), 1);
        assert!(remove(&mut s, BRUCHSTEIN as u16, 101, None).is_err());
        remove(&mut s, BRUCHSTEIN as u16, 99, Some(0)).unwrap();
        assert_eq!(count(&s, BRUCHSTEIN as u16), 1);
    }

    #[test]
    fn angleichen_und_ueberlauf() {
        let mut s = Vec::new();
        let mut c = BTreeMap::new();
        for b in 1..=40u16 {
            c.insert(b, 70);
        }
        c.insert(310, 1); // Cosmetic: Garderobe, kein Platz
        let over = reconcile(&mut s, &c);
        assert_eq!(s.len(), INV_SLOTS);
        let in_s: u64 = totals(&s).values().sum();
        let out: u64 = over.iter().map(|o| o.1).sum();
        assert_eq!(in_s + out, 40 * 70, "nichts verschwindet: Plätze + Beutel = Zählung");
        assert!(totals(&s).get(&310).is_none());
        // erneut angleichen ändert nichts mehr, wenn die Zählung die Plätze ist
        let now: BTreeMap<u16, u64> = totals(&s);
        let before = s.clone();
        assert!(reconcile(&mut s, &now).is_empty());
        assert_eq!(totals(&s), totals(&before));
    }

    #[test]
    fn nichts_entsteht_nichts_verschwindet() {
        // 100 Grenzfälle mit fester Zufallsfolge: Züge im Inventar, zwischen Inventar und Truhe, Umschalt-Klick, Sortieren, Einlagern
        let mut r = 0x1234_5678_u64;
        let mut rnd = move |n: u64| {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            r % n
        };
        let items = [BRUCHSTEIN as u16, STAMM as u16, crate::skyblock::SPITZHACKE, crate::rpg::armor_item(0, 0), SAND as u16];
        for case in 0..100 {
            let mut inv = vec![Stack::EMPTY; INV_SLOTS];
            let mut chest = vec![Stack::EMPTY; if case % 2 == 0 { CHEST_SMALL } else { CHEST_LARGE }];
            for _ in 0..12 {
                let it = items[rnd(items.len() as u64) as usize];
                add(&mut inv, it, 1 + rnd(90) as u32, 0..INV_SLOTS);
                let cl = chest.len();
                add(&mut chest, it, rnd(40) as u32, 0..cl);
            }
            let before = sum(&inv, &chest);
            for _ in 0..40 {
                let (a, b) = (rnd(INV_SLOTS as u64) as usize, rnd(chest.len() as u64) as usize);
                let n = rnd(70) as u32;
                let _ = match rnd(7) {
                    0 => move_in(&mut inv, a, rnd(INV_SLOTS as u64) as usize, n),
                    1 => xfer(&mut inv[a], &mut chest[b], n, |_| true, |_| true),
                    2 => xfer(&mut chest[b], &mut inv[a], n, |_| true, |_| true),
                    3 => { quick_between(&mut inv, a, &mut chest); Ok(()) }
                    4 => { quick_in(&mut inv, a, if a < HOTBAR { HOTBAR..INV_SLOTS } else { 0..HOTBAR }); Ok(()) }
                    5 => { let cl = CHEST_SMALL.min(chest.len()); sort(&mut chest, 0..cl); Ok(()) }
                    _ => { quick_stack(&mut inv, &mut chest); Ok(()) }
                };
                assert_eq!(sum(&inv, &chest), before, "Fall {case}");
                assert!(inv.iter().chain(&chest).all(|x| x.is_empty() || x.n <= stack_max(x.item)), "Stapelgrenze Fall {case}");
            }
        }
    }

    #[test]
    fn ausruestung_passt() {
        assert!(gear_fits(0, crate::rpg::armor_item(2, 0)) && !gear_fits(1, crate::rpg::armor_item(2, 0)));
        assert!(gear_fits(5, crate::rpg::ARTIFACT) && !gear_fits(5, crate::rpg::armor_item(0, 3)) && !gear_fits(3, BRUCHSTEIN as u16));
    }
}
