//! Außen-Ausstattung des Hub-Archipels (Hauptinsel, Himmels-Garten, Zitadelle, Brücken): Wildblumen-Wiesen, Bänke unter Bäumen,
//! Laternenmasten, Zäune an Klippen. Läuft als letzter Schritt von blueprints::hub. Eigentum: Agent „flora“ (CONTRACT-8.md).
#[allow(unused_imports)]
use crate::blueprints::*;
#[allow(unused_imports)]
use crate::bp_furniture::*;
use crate::world::*;

/// Pfadbeläge: dort kommen keine Bänke und Laternen hin.
const PFADE: [u8; 7] = [WEG, PFLASTER, POLIERT, STEINZIEGEL, MOOS_ZIEGEL, MARMOR, BRUCHSTEIN];
/// Zaun-Ausnahmen: Bogenbrücke der Himmelsruinen, Brückenenden West (Garten) und Nord (Zitadelle), Aussichtssteg im Süden.
const SKIP: [[i32; 4]; 4] = [[196, 172, 208, 182], [104, 236, 114, 244], [236, 104, 244, 114], [236, 325, 244, 334]];

/// Frei für Möbel über dem Gelände: fester Boden (kein Pfad) bei y-1, Luft bei y..y+2, nie keep_free.
fn frei(p: &Put<'_>, st: WorldState, x: i32, y: i32, z: i32) -> bool {
    let boden = p.get(x, y - 1, z);
    solid(boden)
        && !PFADE.contains(&boden)
        && surf(x, z, st).is_some_and(|s| s.h == y - 1)
        && (0..3).all(|k| p.get(x, y + k, z) == 0)
        && !(in_hub(x, z) && keep_free(x, z))
}

/// Sitzbank-Reihe, Zelle für Zelle, jede auf der Geländeoberfläche (nur wo `frei`).
fn sitz(p: &mut Put<'_>, st: WorldState, x0: i32, z0: i32, x1: i32, z1: i32, zu: u8) {
    for z in z0..=z1 {
        for x in x0..=x1 {
            if let Some(s) = surf(x, z, st) {
                let y = s.h + 1;
                if frei(p, st, x, y, z) {
                    bench(p, x, z, x, z, y, zu);
                }
            }
        }
    }
}

/// Laternenmast auf der Geländeoberfläche, nur wo `frei`.
fn laterne(p: &mut Put<'_>, st: WorldState, x: i32, z: i32) {
    if let Some(s) = surf(x, z, st) {
        let y = s.h + 1;
        if frei(p, st, x, y, z) {
            lamp_post(p, x, y, z);
        }
    }
}

/// Nur mit den Helfern aus bp_furniture; schreibt nur in Luft (p.add bzw. Helfer, die Luft prüfen).
pub(crate) fn dress(p: &mut Put, st: WorldState) {
    // Wiesen: meadow setzt nur, wo bei der jeweiligen Höhe WIESE liegt – also je Höhenstufe aufrufen.
    for h in 28..=38 {
        meadow(p, [200, 255, 280, 330], h, 30, 17); // Spawn-Viertel
        meadow(p, [270, 150, 330, 330], h, 35, 19); // Flüsterwald
    }
    meadow(p, [62, 212, 118, 268], 30, 30, 11); // Himmels-Garten
    meadow(p, [207, 52, 273, 118], 30, 30, 13); // Zitadellen-Insel
    meadow(p, ISLET, 32, 30, 23); // Kleine Insel (nur mit Projekt)

    // Sitzbänke: Himmels-Garten (Blick zum Weg), Spawn-Terrassen (Portal-Umkreis bleibt frei).
    sitz(p, st, 76, 226, 78, 226, ZU_S);
    sitz(p, st, 100, 254, 102, 254, ZU_N);
    sitz(p, st, 204, 262, 206, 262, ZU_S);
    sitz(p, st, 274, 262, 276, 262, ZU_S);

    // Laternen alle ~12 Blöcke neben den Gartenwegen (Wege x 89..91 und z 239..241, Rand x 88/92, z 238/242).
    for z in (222..=258).step_by(12) {
        laterne(p, st, 93, z);
    }
    for x in (72..=108).step_by(12) {
        laterne(p, st, x, 243);
    }

    // Zäune um die Inseln, Geländer an den Rändern von Markt und Spawn.
    cliff_fence(p, st, [62, 212, 118, 268], ZAUN, &SKIP);
    cliff_fence(p, st, [207, 52, 273, 118], ZAUN, &SKIP);
    cliff_fence(p, st, [210, 200, 270, 255], GELAENDER, &SKIP);
    cliff_fence(p, st, [200, 255, 280, 330], GELAENDER, &SKIP);
}
