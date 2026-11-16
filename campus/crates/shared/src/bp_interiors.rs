//! Innenräume des Hubs: Bank (Marktplatz West), Markthalle (= Bazaar-Halle mit Ständen), Minion-Werkstatt.
//! Eigentum: Agent „interiors“ (CONTRACT-8.md). Hülle der Bank baut `house` in blueprints::market vorher.
use crate::blueprints::*;
use crate::bp_furniture::*;
use crate::world::*;

/// Bank: Haus 206..221 × 218..235, Boden y 28, Tür im Osten (x 221, z 226..228); Bankier Goldbach steht bei (212.5, 29, 227.5).
/// Innen x 207..220 × z 219..234, Erdgeschoss y 29..31, Zwischenboden y 32.
pub(crate) fn bank(p: &mut Put) {
    if !p.hits(205, 217, 222, 236) {
        return;
    }
    // Tresor hinten (x 207..210) hinter Gitterstäben auf x 211
    p.fill(211, 29, 219, 211, 31, 234, GITTERSTAB);
    p.fill(207, 29, 219, 207, 30, 221, GOLD);
    p.fill(207, 29, 232, 207, 30, 234, GOLD);
    p.fill(207, 29, 222, 207, 29, 231, GOLDSTAPEL);
    for z in [219, 220, 221, 232, 233, 234] {
        p.set(209, 29, z, GOLDSTAPEL);
    }
    p.set(210, 29, 219, KISTE);
    p.set(210, 29, 234, KISTE);
    // Schalter-Reihe vor dem Bankier (Goldbach bleibt frei auf x 212, Tresen außerhalb seines keep_free-Kreises)
    p.fill(215, 29, 221, 215, 29, 233, TRESEN);
    p.fill(212, 29, 227, 212, 30, 227, 0);
    // Läufer von der Tür bis zum Schalter: rot in der Mitte, gold daneben
    rug(p, 216, 226, 220, 228, 29, WEBTEPPICH_ROT);
    rug(p, 216, 225, 220, 225, 29, WEBTEPPICH_GOLD);
    rug(p, 216, 229, 220, 229, 29, WEBTEPPICH_GOLD);
    // Sitzbänke für Wartende an den Längswänden, Blick zum Läufer
    bench(p, 217, 219, 219, 219, 29, ZU_S);
    bench(p, 217, 234, 219, 234, 29, ZU_N);
    // Kronleuchter unter dem Zwischenboden (y 32); die Mitte (214, 31, 227) setzt house
    for (x, z) in [(218, 222), (218, 231), (209, 227)] {
        if solid(p.get(x, 32, z)) {
            p.set(x, 31, z, KRONLEUCHTER);
        }
    }
}

/// Markthalle (Bazaar-Halle): offenes Dach auf Pfosten 222..258 × 199..213, Boden y 28, darunter 8 Stände.
pub(crate) fn markthalle(p: &mut Put) {
    // Bazaar-Halle: offenes Dach auf Pfosten, darunter 8 gestreifte Stände
    if !p.hits(220, 197, 260, 215) {
        return;
    }
    for x in (222..=258).step_by(6) {
        for z in [199, 213] {
            p.fill(x, 29, z, x, 33, z, DUNKELHOLZ);
        }
    }
    for (a, b, c, e) in [(222, 199, 258, 199), (222, 213, 258, 213), (222, 199, 222, 213), (258, 199, 258, 213)] {
        p.fill(a, 34, b, c, 34, e, DUNKELHOLZ);
    }
    // Querbalken über den inneren Pfosten, daran hängen die Laternen
    for x in (228..=252).step_by(6) {
        p.fill(x, 34, 200, x, 34, 212, DUNKELHOLZ);
    }
    for k in 0..=8 {
        p.fill(221, 35 + k, 198 + k, 259, 35 + k, 198 + k, DACH);
        p.fill(221, 35 + k, 214 - k, 259, 35 + k, 214 - k, DACH);
    }
    let cols = [
        (STOFF_ROT, STOFF_WEISS),
        (STOFF_BLAU, STOFF_WEISS),
        (STOFF_GRUEN, STOFF_GELB),
        (STOFF_GELB, STOFF_ROT),
        (STOFF_BLAU, STOFF_GELB),
        (STOFF_ROT, STOFF_GRUEN),
        (STOFF_WEISS, STOFF_BLAU),
        (STOFF_GRUEN, STOFF_WEISS),
    ];
    let goods = [SAND, BRETTER, GLAS, KUPFER, ZIEGEL, STAMM, LAUB, BRUCHSTEIN];
    for i in 0..4 {
        let x0 = 225 + i as i32 * 8;
        stall(p, x0, 200, 28, cols[i], goods[i], true);
        stall(p, x0, 208, 28, cols[i + 4], goods[i + 4], false);
    }
    // Läufer im Mittelgang (Händler Brandt steht draußen bei z 215)
    rug(p, 223, 205, 257, 206, 29, WEBTEPPICH_ROT);
    for x in (228..=252).step_by(6) {
        p.add(x, 33, 206, WANDLATERNE);
    }
    // Sitzbänke an den Hallenenden, Deko-Fässer zwischen den Ständen
    for z in [201, 202, 209, 210] {
        p.add(223, 29, z, SITZBANK_O);
        p.add(257, 29, z, SITZBANK_W);
    }
    for x in [231, 239, 247] {
        p.add(x, 29, 201, DEKOFASS);
        p.add(x, 29, 211, DEKOFASS);
    }
}

/// Minion-Werkstatt: offene Halle 164..192 × 240..250, Boden y 26, Sockel für Stufe I–XI (SHOWCASE).
pub(crate) fn werkstatt(p: &mut Put) {
    if !p.hits(162, 236, 194, 252) {
        return;
    }
    p.fill(164, 26, 240, 192, 26, 250, PFLASTER);
    p.fill(164, 27, 250, 192, 31, 250, PUTZ);
    p.fill(164, 27, 240, 164, 31, 250, PUTZ);
    p.fill(192, 27, 240, 192, 31, 250, PUTZ);
    p.fill(165, 27, 241, 191, 31, 249, 0);
    for x in (164..=192).step_by(4) {
        p.fill(x, 27, 240, x, 31, 240, DUNKELHOLZ);
        p.fill(x, 27, 250, x, 31, 250, DUNKELHOLZ);
    }
    for (a, b, c, e) in [(164, 240, 192, 240), (164, 250, 192, 250), (164, 240, 164, 250), (192, 240, 192, 250)] {
        p.fill(a, 32, b, c, 32, e, DUNKELHOLZ);
    }
    for k in 0..=6 {
        p.fill(163, 33 + k, 239 + k, 193, 33 + k, 239 + k, DACH);
        p.fill(163, 33 + k, 251 - k, 193, 33 + k, 251 - k, DACH);
        for x in [164, 192] {
            p.fill(x, 33 + k, 240 + k, x, 33 + k, 250 - k, PUTZ);
        }
    }
    // Fenster in den Putzwänden (zwischen den Pfosten)
    for x in (166..=190).step_by(4) {
        window(p, x, 28, 250, 3);
    }
    for x in [164, 192] {
        for z in 244..=246 {
            window(p, x, 28, z, 3);
        }
    }
    for k in 0..11 {
        p.set(SHOWCASE[0] + 2 * k, SHOWCASE[1], SHOWCASE[2], SOCKEL);
    }
    // Läufer vor den Sockeln (z 244), Querbalken mit Kronleuchtern
    rug(p, 166, 242, 190, 243, 27, WEBTEPPICH_ROT);
    for x in [170, 178, 186] {
        p.fill(x, 32, 241, x, 32, 249, DUNKELHOLZ);
        p.set(x, 31, 246, KRONLEUCHTER);
    }
    // Arbeitsplätze an der Rückwand
    for x in [172, 180, 189] {
        p.set(x, 27, 249, WERKBANK);
    }
    for x in [176, 184] {
        p.set(x, 27, 248, AMBOSS);
    }
    p.fill(166, 27, 248, 167, 28, 248, KISTE);
    for (x, z) in [(165, 249), (191, 249), (191, 247)] {
        p.set(x, 27, z, DEKOFASS);
    }
    bench(p, 169, 247, 171, 247, 27, ZU_N);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct W(HashMap<(i32, i32), Vec<u8>>);
    impl W {
        fn get(&mut self, x: i32, y: i32, z: i32) -> u8 {
            let c = self.0.entry((x.div_euclid(16), z.div_euclid(16))).or_insert_with(|| gen_chunk(x.div_euclid(16), z.div_euclid(16), WorldState::default()));
            c.get(idx(x.rem_euclid(16), y, z.rem_euclid(16))).copied().unwrap_or(0)
        }
        #[allow(clippy::too_many_arguments)]
        fn has(&mut self, v: u8, x0: i32, x1: i32, y0: i32, y1: i32, z0: i32, z1: i32) -> bool {
            (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| self.get(x, y, z) == v)))
        }
    }

    #[test]
    fn innenraeume() {
        let mut w = W(HashMap::new());
        for v in [TRESEN, WEBTEPPICH_ROT, SCHEIBE, KRONLEUCHTER, GITTERSTAB, GOLDSTAPEL] {
            assert!(w.has(v, 206, 221, 29, 40, 218, 235), "Bank enthält Block {v}");
        }
        // Goldbach: Füße und Kopf frei, fester Boden, der Schalter in Reichweite (≤ 8 Blöcke) vom Eingang her
        assert!(solid(w.get(212, 28, 227)) && w.get(212, 29, 227) == 0 && w.get(212, 30, 227) == 0);
        assert!(!solid(w.get(216, 29, 227)) && !solid(w.get(216, 30, 227)), "Kundenplatz vor dem Schalter frei");
        // Werkstatt: Sockel unverändert, 2 Luft darüber
        for k in 0..11 {
            let x = SHOWCASE[0] + 2 * k;
            assert_eq!(w.get(x, SHOWCASE[1], SHOWCASE[2]), SOCKEL, "Sockel {k}");
            assert_eq!((w.get(x, SHOWCASE[1] + 1, SHOWCASE[2]), w.get(x, SHOWCASE[1] + 2, SHOWCASE[2])), (0, 0), "Luft über Sockel {k}");
        }
        for v in [WERKBANK, AMBOSS, SCHEIBE, KRONLEUCHTER, WEBTEPPICH_ROT] {
            assert!(w.has(v, 164, 192, 27, 31, 240, 250), "Werkstatt enthält Block {v}");
        }
        // Lampen hängen unter festen Blöcken
        for x in [170, 178, 186] {
            assert!(w.get(x, 31, 246) == KRONLEUCHTER && solid(w.get(x, 32, 246)));
        }
        for x in (228..=252).step_by(6) {
            assert!(w.get(x, 33, 206) == WANDLATERNE && solid(w.get(x, 34, 206)), "Hängelampe Markthalle x {x}");
        }
        // Händler Brandt frei
        assert!(!solid(w.get(240, 29, 215)) && !solid(w.get(240, 30, 215)));
        assert!(w.has(WEBTEPPICH_ROT, 223, 257, 29, 29, 205, 206) && w.has(DEKOFASS, 222, 258, 29, 29, 199, 213));
    }
}
