//! Privatinsel-Vorlage (Starthütte, Garten, Rohstoffinsel). Eigentum: Agent „private“ (CONTRACT-8.md).
//! Invarianten (Tests, Server, Migration) stehen in CONTRACT-8.md §private.
use crate::blueprints::*;
use crate::bp_furniture::{bench, cliff_fence, lamp_post, meadow, rug, stair_roof, table_set, window, ZU_O};
use crate::world::*;

/// Naturstein-Wege (relativ zur Platz-Ecke, [x0, z0, x1, z1] eingeschlossen): Ankunftsplatz, zur Hüttentür, zum Portal,
/// zur Hängebrücke (→ Generatoren), Fläche um den Minion-Sockel.
const WEGE: [[i32; 4]; 5] = [[46, 50, 50, 54], [47, 39, 49, 49], [47, 55, 49, 61], [50, 47, 65, 49], [50, 45, 52, 46]];

/// 38×38-Hauptinsel mit möblierter Starthütte (Satteldach aus Treppen, Scheiben, Tisch, Stühle, Teppich, Bett, Regal,
/// Truhe, Werkbank, Kronleuchter), Naturstein-Gartenweg, Minion-Sockel, Eiche mit Wandlaterne, Zaun am Rand, Wildblumen,
/// Portal zum Hub; Hängebrücke zur 16×16-Rohstoffinsel mit Bruchstein- und Splitter-Generator, Holzknoten-Eichen.
pub(crate) fn private(p: &mut Put, st: WorldState) {
    let (a, c) = (p.ox / SLOT * SLOT, p.oz / SLOT * SLOT);
    if !p.hits(a + 20, c + 24, a + 94, c + 72) {
        return;
    }
    let g = GROUND;
    // Gartenweg: erst alle Steine (nur auf Inselboden), dann Kiesrand (WEG) nur auf Wiese daneben
    for r in WEGE {
        for z in c + r[1]..=c + r[3] {
            for x in a + r[0]..=a + r[2] {
                if p.get(x, g, z) != 0 {
                    p.set(x, g, z, if hash(x, z, 71) % 4 == 0 { MOOS_BRUCH } else { BRUCHSTEIN });
                }
            }
        }
    }
    for r in WEGE {
        for z in c + r[1] - 1..=c + r[3] + 1 {
            for x in a + r[0] - 1..=a + r[2] + 1 {
                if p.get(x, g, z) == WIESE {
                    p.set(x, g, z, WEG);
                }
            }
        }
    }
    hut(p, a, c, g);
    p.set(a + 51, g + 1, c + 46, SOCKEL);
    arch(p, a + 48, c + 62, g, false);
    for (x, z) in [(45, 40), (51, 40), (64, 46), (64, 50)] {
        lamp_post(p, a + x, g + 1, c + z);
    }
    bench(p, a + 44, c + 48, a + 44, c + 49, g + 1, ZU_O);
    for (x, z) in [(34, 44), (62, 36), (35, 59), (62, 58)] {
        tree(p, a + x, c + z, g, 5 + (hash(a + x, c + z, 66) % 2) as i32, STAMM, LAUB, 69);
    }
    // Garten-Eiche: Ast nach Osten, darunter eine Wandlaterne (vor dem Laub setzen, Laub füllt nur Luft)
    let (ex, ez) = (a + 41, c + 45);
    p.fill(ex, g + 1, ez, ex, g + 6, ez, STAMM);
    p.fill(ex + 1, g + 5, ez, ex + 3, g + 5, ez, STAMM);
    p.set(ex + 3, g + 4, ez, WANDLATERNE);
    blob(p, ex, g + 7, ez, 3.0, LAUB);
    blob(p, ex + 2, g + 7, ez, 2.3, LAUB);
    // Hängebrücke
    p.fill(a + 66, g, c + 47, a + 73, g, c + 49, BRETTER);
    for x in [66, 72] {
        for z in [46, 50] {
            p.fill(a + x, g, c + z, a + x, g + 3, c + z, DUNKELHOLZ);
        }
    }
    for z in [46, 50] {
        p.fill(a + 67, g + 2, c + z, a + 71, g + 2, c + z, DUNKELBRETT);
    }
    // Rohstoffinsel: Becken mit Wasser | Generatoren | Lava
    p.fill(a + 76, g, c + 44, a + 84, g, c + 52, STEINZIEGEL);
    for z in 44..=52 {
        for x in 76..=84 {
            let v = if x == 76 || x == 84 || z == 44 || z == 52 {
                STEINZIEGEL
            } else {
                match x {
                    77 | 78 => WASSER,
                    79 | 81 => POLIERT,
                    80 => match z {
                        46 | 47 => GENERATOR,
                        48 => KRISTALL,
                        49 | 50 => SPLITTER_GEN,
                        _ => POLIERT,
                    },
                    _ => LAVA,
                }
            };
            p.set(a + x, g + 1, c + z, v);
        }
    }
    for (x, z) in [(86, 42), (86, 54)] {
        p.fill(a + x, g + 1, c + z, a + x, g + 4, c + z, KNOTEN_EICHE);
        blob(p, a + x, g + 5, c + z, 2.2, LAUB);
    }
    // Laternen-Würfel am Generator (wie bisher lantern(), aber unabhängig davon, falls lantern umgestellt wird)
    p.fill(a + 75, g + 1, c + 43, a + 75, g + 2, c + 43, DUNKELHOLZ);
    p.set(a + 75, g + 3, c + 43, LATERNE);
    fall(p, st, a + 24, c + 60, a + 31, c + 61, g, 6);
    // Holzzaun am Inselrand, frei an Brückenköpfen, hinter dem Portal und am Wasserfall
    let skip = [[a + 62, c + 45, a + 76, c + 51], [a + 44, c + 58, a + 52, c + 70], [a + 22, c + 58, a + 33, c + 63]];
    cliff_fence(p, st, [a + 24, c + 24, a + 94, c + 72], ZAUN, &skip);
    meadow(p, [a + 24, c + 24, a + 94, c + 72], g, 30, 811);
}

/// Starthütte x a+41..a+54 × z c+31..c+38, Boden g, Tür Süd (x a+47..a+49). Truhe/Werkbank/Bett/Regal an den alten Stellen.
fn hut(p: &mut Put, a: i32, c: i32, g: i32) {
    let (x0, z0, x1, z1) = (a + 41, c + 31, a + 54, c + 38);
    if !p.hits(x0 - 2, z0 - 2, x1 + 2, z1 + 2) {
        return;
    }
    let top = g + 4;
    p.fill(x0, g + 1, z0, x1, g + 9, z1, 0);
    p.fill(x0, g - 3, z0, x1, g, z1, STEINZIEGEL);
    p.fill(x0 + 1, g, z0 + 1, x1 - 1, g, z1 - 1, BRETTER);
    // Wände, Fachwerk-Pfosten, Rähm, Decke
    p.fill(x0, g + 1, z0, x1, g + 3, z1, PUTZ);
    p.fill(x0 + 1, g + 1, z0 + 1, x1 - 1, g + 3, z1 - 1, 0);
    for dx in [0, 5, 9, 13] {
        p.fill(x0 + dx, g + 1, z0, x0 + dx, g + 3, z0, DUNKELHOLZ);
        p.fill(x0 + dx, g + 1, z1, x0 + dx, g + 3, z1, DUNKELHOLZ);
    }
    p.fill(x0, g + 1, z0 + 3, x0, g + 3, z0 + 3, DUNKELHOLZ);
    p.fill(x1, g + 1, z0 + 3, x1, g + 3, z0 + 3, DUNKELHOLZ);
    p.fill(x0, top, z0, x1, top, z1, DUNKELHOLZ);
    p.fill(x0 + 1, top, z0 + 1, x1 - 1, top, z1 - 1, DUNKELBRETT);
    // Satteldach mit Überstand (First entlang x), Giebelwände, Schornstein
    stair_roof(p, x0 - 1, z0 - 1, x1 + 1, z1 + 1, top, true, TREPPE_DACH_N, STUFE_DACH);
    for k in 1..=3 {
        for gx in [x0, x1] {
            p.fill(gx, top + k, z0 + k, gx, top + k, z1 - k, PUTZ);
        }
    }
    p.fill(x1 - 2, top, z0 + 2, x1 - 2, top + 5, z0 + 2, BRUCHSTEIN);
    // Fenster (Scheiben, 2 hoch), Giebelfenster, Blumenkästen unter den Südfenstern
    for dx in [2, 3, 7, 11] {
        window(p, x0 + dx, g + 2, z0, 2);
    }
    for dx in [2, 3, 11, 12] {
        window(p, x0 + dx, g + 2, z1, 2);
        p.set(x0 + dx, g + 1, z1 + 1, DUNKELBRETT);
        p.set(x0 + dx, g + 2, z1 + 1, BLUMEN[(dx % 4) as usize]);
    }
    for dz in [1, 2, 5, 6] {
        window(p, x0, g + 2, z0 + dz, 2);
    }
    for dz in [5, 6] {
        window(p, x1, g + 2, z0 + dz, 2);
    }
    for gx in [x0, x1] {
        window(p, gx, top + 2, z0 + 3, 1);
        window(p, gx, top + 2, z0 + 4, 1);
    }
    // Tür (3 breit), daneben Wandlaternen unter der Traufe
    p.fill(a + 47, g + 1, z1, a + 49, g + 3, z1, 0);
    p.fill(a + 47, g, z1, a + 49, g, z1, BRETTER);
    p.set(a + 46, g + 3, z1 + 1, WANDLATERNE);
    p.set(a + 50, g + 3, z1 + 1, WANDLATERNE);
    // Einrichtung
    p.fill(a + 42, g + 1, c + 32, a + 43, g + 1, c + 32, BETT);
    p.fill(a + 53, g + 1, c + 32, a + 53, g + 3, c + 34, REGAL);
    p.set(a + 51, g + 1, c + 32, TRUHE);
    p.set(a + 50, g + 1, c + 32, WERKBANK);
    p.set(a + 53, g + 1, c + 37, DEKOFASS);
    p.set(a + 52, g + 1, c + 37, KISTE);
    table_set(p, a + 45, g + 1, c + 35, crate::shape::N | crate::shape::O | crate::shape::S | crate::shape::W, BUECHERSTAPEL);
    rug(p, a + 46, c + 33, a + 50, c + 36, g + 1, WEBTEPPICH_ROT);
    p.set(a + 48, g + 3, c + 35, KRONLEUCHTER);
    p.set(a + 43, g + 3, c + 33, WANDLATERNE);
}

#[cfg(test)]
mod tests {
    use crate::world::*;
    use std::collections::HashMap;

    #[test]
    fn vorlage_starthuette() {
        let st = WorldState::default();
        let (slot, g) = ((5u8, 0u8), GROUND);
        let (a, c) = (slot.0 as i32 * SLOT, slot.1 as i32 * SLOT);
        let mut m: HashMap<(i32, i32), Vec<u8>> = HashMap::new();
        for cz in (c + 16) >> 4..=(c + 79) >> 4 {
            for cx in (a + 16) >> 4..=(a + 95) >> 4 {
                m.insert((cx, cz), gen_chunk(cx, cz, st));
            }
        }
        let get = |x: i32, y: i32, z: i32| -> u8 {
            if !(0..CH).contains(&y) {
                return 0;
            }
            m.get(&(x >> 4, z >> 4)).filter(|v| !v.is_empty()).map_or(0, |v| v[idx(x & 15, y, z & 15)])
        };
        let has = |f: &dyn Fn(u8) -> bool, y1: i32| (a + 20..=a + 94).any(|x| (c + 24..=c + 72).any(|z| (g..y1).any(|y| f(get(x, y, z)))));
        // neue Ausstattung
        let checks: [(&str, fn(u8) -> bool); 9] = [
            ("SCHEIBE", |b| b == SCHEIBE),
            ("TISCH", |b| b == TISCH),
            ("STUHL_*", |b| (STUHL_N..STUHL_N + 4).contains(&b)),
            ("WEBTEPPICH_*", |b| TEPPICHE.contains(&b)),
            ("TREPPE_*", |b| (TREPPE_EICHE_N..TREPPE_DACH_N + 4).contains(&b)),
            ("BLUMEN", |b| BLUMEN.contains(&b)),
            ("KRONLEUCHTER", |b| b == KRONLEUCHTER),
            ("WANDLATERNE", |b| b == WANDLATERNE),
            ("ZAUN", |b| b == ZAUN),
        ];
        for (name, f) in checks {
            assert!(has(&f, g + 12), "{name} fehlt in der Vorlage");
        }
        // Pflichtblöcke auf GROUND..GROUND+6
        for b in [PORTAL, TRUHE, WERKBANK, SOCKEL, GENERATOR, SPLITTER_GEN, BETT, REGAL, LATERNE, KNOTEN_EICHE] {
            assert!(has(&|v| v == b, g + 6), "Block {b} fehlt");
        }
        // feste Stellen (Migration hält Truhen fest)
        assert_eq!((get(a + 51, g + 1, c + 32), get(a + 50, g + 1, c + 32), get(a + 51, g + 1, c + 46)), (TRUHE, WERKBANK, SOCKEL));
        // Ankunft
        assert!(solid(get(a + 48, g, c + 52)), "Ankerblock");
        assert!((a + 47..=a + 49).all(|x| (c + 51..=c + 53).all(|z| (g + 1..CH).all(|y| !solid(get(x, y, z))))), "Himmel über der Ankunft");
        assert!((a + 46..=a + 50).all(|x| (c + 50..=c + 54).all(|z| (g + 1..=g + 4).all(|y| get(x, y, z) == 0))), "Luft um die Ankunft");
        assert_eq!(find_safe_spawn(|x, y, z| get(x, y, z), a + 48, c + 52), Some(plot_spawn(slot)));
    }
}
