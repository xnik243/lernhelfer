//! Phasen 16–20, gemeinsame Klammer: Werte-Rechnung (stats), HTTP-Weiche zu inv.rs / island.rs / mobs.rs und das
//! RPG-Register für die Oberfläche. Alles Fachliche steht in den drei Teil-Dateien.
use crate::game::Game;
use crate::R;
use campus_shared::stats::{self, Stats};
use campus_shared::{enchants_data, equipment_data, farming_data, inventory, items, mobs_data, questline_data, rpg, skyblock as sb, world};
use serde_json::{json, Value};

impl Game {
    /// Werte aus Fähigkeiten, getragener Ausrüstung, gehaltenem Gegenstand und Verzauberungen (frisch gerechnet).
    pub fn stats(&self, uid: u32) -> Stats {
        let lv = self.levels(uid);
        let gear = self.gear(uid);
        let held = self.held_item(uid);
        stats::compute(&lv, &gear, held, &|i| self.ench(uid, i))
    }
    /// Werte neu rechnen, im Spieler ablegen, an den Browser melden (Hud `werte`) und Leben anpassen.
    /// Aufrufen nach: Anmeldung, Schnellleisten-Wahl, An-/Ablegen, Verzaubern, Stufenaufstieg.
    pub fn refresh_stats(&mut self, id: u32) {
        let s = self.stats(id);
        let Some(p) = self.players.get_mut(&id) else { return };
        let (old_max, old) = (p.stats.max_hp, p.stats);
        p.stats = s;
        p.hp = if old_max <= 0.0 { s.max_hp } else { p.hp.min(s.max_hp) };
        let (hp, shield) = (p.hp, (p.shield_until - self.now()).max(0) as u32);
        if old != s {
            let v = self.stats_json(id);
            self.hud(id, json!({ "t": "werte", "werte": v }));
            self.send(id, &campus_shared::proto::S2C::Health { hp, max: s.max_hp, shield_ms: shield });
        }
    }
    pub fn stats_json(&self, uid: u32) -> Value {
        let s = self.players.get(&uid).map(|p| p.stats).filter(|s| s.max_hp > 0.0).unwrap_or_else(|| self.stats(uid));
        json!({
            "leben": s.max_hp, "verteidigung": s.defense, "staerke": s.strength, "krit": s.crit_chance, "kritSchaden": s.crit_damage,
            "abbau": s.mining_speed, "glueck": s.fortune, "ernteGlueck": s.crop_fortune, "mana": s.max_mana, "manaRegen": s.mana_regen,
            "regen": s.hp_regen, "taler": s.taler_bonus, "manaKosten": s.mana_cost, "waffe": s.weapon_damage, "telekinese": s.telekinese,
            "set": if s.set == stats::NO_SET { Value::Null } else { json!(s.set) }, "minderung": stats::reduction(s.defense),
        })
    }
    /// Abbau-Tempo (Prozent) für diesen Block: Werte, die „Eile“ der Bergarbeiter-Rüstung gilt nur für Stein/Erz/Kristall/Metall.
    pub fn mine_speed(&self, id: u32, block: u8) -> f32 {
        let s = self.players.get(&id).map(|p| p.stats).unwrap_or_default();
        let stony = matches!(world::material(block), world::M_STEIN | world::M_KRISTALL | world::M_METALL);
        let haste = rpg::set_def(stats::SET_BERGARBEITER).map_or(0.0, |d| d.full.mining);
        if s.set == stats::SET_BERGARBEITER && !stony {
            s.mining_speed - haste
        } else {
            s.mining_speed
        }
    }
    /// GET-Weiche: inv.rs, island.rs, mobs.rs (jede liefert None für fremde Ops).
    pub fn rpg_get(&mut self, uid: u32, op: &str) -> Option<R<Value>> {
        if let Some(r) = self.inv_get(uid, op) {
            return Some(r);
        }
        if let Some(r) = self.island_get(uid, op) {
            return Some(r);
        }
        self.mobs_get(uid, op)
    }
    /// POST-Weiche wie oben.
    pub fn rpg_post(&mut self, uid: u32, op: &str, b: &Value) -> Option<R<Value>> {
        if let Some(r) = self.inv_post(uid, op, b) {
            return Some(r);
        }
        self.island_post(uid, op, b)
    }
}

fn bonus(b: &rpg::Bonus) -> Value {
    json!({ "leben": b.hp, "verteidigung": b.def, "staerke": b.str_, "krit": b.crit, "abbau": b.mining, "glueck": b.fortune, "ernteGlueck": b.crop,
        "mana": b.mana, "manaRegen": b.regen, "regen": b.hp_regen, "taler": b.taler, "manaKosten": b.mana_cost, "schaden": b.dmg })
}

/// Alles, was die Oberfläche über Ausrüstung, Sets, Wesen, Verzauberungen, Pflanzen, Kapitel und Seltenheit wissen muss.
pub fn registry_rpg() -> Value {
    let equip: Vec<Value> = equipment_data::EQUIP.iter().map(|e| json!({ "id": e.id, "name": e.name, "platz": e.slot, "set": if e.set == 255 { Value::Null } else { json!(e.set) },
        "seltenheit": e.rarity, "rgb": e.rgb, "bonus": bonus(&e.bonus), "lore": e.lore, "wert": e.value })).collect();
    let sets: Vec<Value> = equipment_data::SETS.iter().map(|s| json!({ "id": s.id, "name": s.name, "text": s.bonus_text, "bonus": bonus(&s.full) })).collect();
    let mobs: Vec<Value> = mobs_data::MOBS.iter().map(|m| json!({ "id": m.id, "name": m.name, "stufe": m.level, "leben": m.hp, "ring": m.ring, "rgb": m.rgb, "rgb2": m.rgb2, "groesse": m.size })).collect();
    let ench: Vec<Value> = enchants_data::ENCHANTS.iter().map(|e| json!({ "id": e.id, "name": e.name, "max": e.max, "arten": e.kinds, "je": e.per_level, "fach": e.fach, "text": e.desc,
        "kosten": (1..=e.max).map(|l| campus_shared::enchanting::cost(e.id, l).map(|c| json!({ "splitter": c.1, "kristalle": c.2 }))).collect::<Vec<_>>() })).collect();
    let crops: Vec<Value> = farming_data::CROPS.iter().map(|c| json!({ "saat": c.seed, "name": c.name, "ernte": c.product, "ms": c.grow_ms, "rgb": c.rgb, "rgb2": c.rgb2 })).collect();
    let chapters: Vec<Value> = questline_data::CHAPTERS.iter().map(|c| json!({ "id": c.id, "name": c.name, "text": c.text, "schritte": c.steps.iter().map(|s| s.1).collect::<Vec<_>>(), "taler": c.taler, "splitter": c.shards })).collect();
    // Tooltip-Grunddaten für jeden bekannten Gegenstand (Blöcke, SkyBlock, RPG)
    let ids = world::BLOCKS.iter().map(|b| b.id as u16).chain(sb::all_items().map(|(i, _)| i)).chain([items::BLUEPRINT]);
    let all: Vec<Value> = ids
        .filter(|&i| i != 0)
        .map(|i| json!({ "id": i, "name": items::item_name(i), "seltenheit": rpg::rarity(i), "wert": rpg::sell_value(i), "lore": rpg::lore(i), "stapel": inventory::stack_max(i),
            "verzauberbar": campus_shared::enchanting::kind(i) }))
        .collect();
    json!({
        "ausruestung": equip, "sets": sets, "wesen": mobs, "verzauberungen": ench, "pflanzen": crops, "kapitel": chapters, "gegenstaende": all,
        "seltenheiten": rpg::RARITY_NAMES, "plaetze": { "inventar": inventory::INV_SLOTS, "leiste": inventory::HOTBAR, "ausruestung": inventory::GEAR_SLOTS,
            "truhe": inventory::CHEST_SMALL, "grosseTruhe": inventory::CHEST_LARGE },
        "bloecke": { "truhe": world::TRUHE, "grosseTruhe": world::GROSSE_TRUHE, "acker": world::ACKERBODEN, "saaten": world::SAATEN, "sprungplatte": world::SPRUNGPLATTE,
            "teleporter": world::TELEPORTER, "staender": world::STAENDER, "schild": world::SCHILD, "altar": world::ALTAR, "bossAltar": world::BOSS_ALTAR },
        "waffen": [rpg::HOLZ_KLINGE, rpg::SAPHIR_KLINGE, rpg::ZITADELLEN_SCHWERT, rpg::FOKUS_BOGEN, rpg::ARKANER_STAB],
    })
}
