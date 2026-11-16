//! Phase 23/25 (Besitz: Agent S-KI, Vertrag CONTRACT-21-25.md §S-KI): OpenRouter-Fragen, Cache, Fallback, Verwaltung.
//! LAW 1: nie unter der Spiel-Sperre warten – `fetch` läuft nur in Hintergrund-Aufgaben, das Ergebnis kommt über `jobs_tx`.
//! Der Schlüssel steht nur in der curl-Konfiguration (stdin), nie in argv, Fehlertexten oder Antworten.
use crate::duel_fallback_data::FALLBACK;
use crate::game::Game;
use crate::sky::clean;
use crate::{no, R};
use rusqlite::params;
use serde_json::{json, Value};

pub const DEFAULT_MODEL: &str = "google/gemini-2.5-flash";
pub const DEFAULT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Harte Grenze für einen OpenRouter-Aufruf (curl bricht selbst nach 6 s ab).
pub const TIMEOUT_MS: u64 = 6500;
/// Fragen je Duell.
pub const ROUNDS: usize = 8;

/// Vorschläge für die Verwaltung (DEFAULT_MODEL zuerst).
const MODELLE: [&str; 5] = [DEFAULT_MODEL, "openai/gpt-4o-mini", "google/gemini-2.5-flash-lite", "anthropic/claude-haiku-4.5", "mistralai/mistral-small-3.2-24b-instruct"];

/// Eine Quizfrage. `options` ohne „A) “-Vorsatz, `correct` 0..=3.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub q: String,
    pub options: [String; 4],
    pub correct: u8,
    pub explanation: String,
}

/// OpenRouter-Einstellungen. `key` leer = keine KI (nur Cache/Fallback).
#[derive(Clone, Debug, Default)]
pub struct KiCfg {
    pub key: String,
    pub model: String,
    pub url: String,
}

/// Ergebnis eines Hintergrund-Auftrags; kommt über Game.jobs_tx zurück und wird im Takt abgeholt (job_done).
pub enum Job {
    Fragen { duel: u32, thema: String, kontext: String, res: Result<Vec<Question>, String> },
}

/// Thema vereinheitlichen (Cache-Schlüssel): säubern (sky::clean), Kleinbuchstaben, Leerraum zusammenfassen, höchstens 60 Zeichen.
pub fn normalize_topic(s: &str) -> String {
    crate::sky::clean(s).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect()
}

fn cut(s: &str, n: usize) -> String {
    clean(s).chars().take(n).collect::<String>().trim().to_string()
}
/// Option säubern; einen Vorsatz „A) “ entfernen, wenn der Buchstabe zur Stelle passt.
fn option(i: usize, s: &str) -> String {
    let t = clean(s);
    let mut c = t.chars();
    let rest = match (c.next(), c.next()) {
        (Some(a), Some(')')) if a.to_ascii_uppercase() == (b'A' + i as u8) as char => c.as_str(),
        _ => t.as_str(),
    };
    cut(rest, 60)
}
fn question(v: &Value) -> Option<Question> {
    let q = cut(v.get("q")?.as_str()?, 110);
    let o = v.get("options")?.as_array()?;
    let correct = v.get("correct")?.as_u64()?;
    if q.is_empty() || o.len() != 4 || correct > 3 {
        return None;
    }
    let mut options: [String; 4] = Default::default();
    for (i, x) in o.iter().enumerate() {
        options[i] = option(i, x.as_str()?);
        if options[i].is_empty() {
            return None;
        }
    }
    Some(Question { q, options, correct: correct as u8, explanation: cut(v.get("explanation").and_then(Value::as_str).unwrap_or(""), 90) })
}
/// Antwort-JSON `{"questions":[…]}` prüfen: je Frage genau 4 Optionen, correct 0..=3, Längen gekappt, mindestens eine Frage.
pub fn parse_questions(json: &str) -> Result<Vec<Question>, String> {
    let v: Value = serde_json::from_str(json.trim()).map_err(|_| "Die KI-Antwort ist kein gültiges JSON.".to_string())?;
    let arr = v.get("questions").and_then(Value::as_array).ok_or_else(|| "Der KI-Antwort fehlt das Feld „questions“.".to_string())?;
    let qs: Vec<Question> = arr.iter().filter_map(question).take(ROUNDS * 4).collect();
    if qs.is_empty() {
        return Err("Die KI-Antwort enthält keine gültige Frage.".into());
    }
    Ok(qs)
}

fn rng(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
/// `n` Fragen aus der festen Bank (duel_fallback_data); fach 255 = gemischt. Deterministisch je `seed`.
/// Reihenfolge: passendes Fach zuerst, dann der Rest; Wiederholung erst, wenn die Bank leer ist.
pub fn fallback(fach: u8, n: usize, seed: u64) -> Vec<Question> {
    let mut s = seed;
    let mut idx: Vec<usize> = (0..FALLBACK.len()).collect();
    for i in (1..idx.len()).rev() {
        let j = (rng(&mut s) % (i as u64 + 1)) as usize;
        idx.swap(i, j);
    }
    idx.sort_by_key(|&i| fach != 255 && FALLBACK[i].fach != fach);
    idx.iter()
        .cycle()
        .take(n)
        .map(|&i| {
            let f = &FALLBACK[i];
            Question { q: f.q.into(), options: f.options.map(String::from), correct: f.correct, explanation: f.explanation.into() }
        })
        .collect()
}

/// Schlüssel, der gefahrlos in die curl-Konfiguration passt: druckbares ASCII ohne Anführungszeichen und Backslash.
fn key_ok(k: &str) -> bool {
    !k.is_empty() && k.len() <= 300 && k.chars().all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '\\'))
}
fn model_ok(m: &str) -> bool {
    (1..=80).contains(&m.len()) && m.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | ':' | '-'))
}
/// curl-Konfiguration für `curl -K -` (geht über stdin). Reine Funktion; nur hier steht der Schlüssel.
fn curl_config(cfg: &KiCfg, thema: &str, kontext: &str) -> Result<String, String> {
    if cfg.key.is_empty() {
        return Err("Kein OpenRouter-Schlüssel hinterlegt.".into());
    }
    if !key_ok(&cfg.key) {
        return Err("Der OpenRouter-Schlüssel enthält unzulässige Zeichen.".into());
    }
    if !(cfg.url.starts_with("https://") || cfg.url.starts_with("http://")) || !cfg.url.chars().all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\')) {
        return Err("Die OpenRouter-Adresse ist ungültig.".into());
    }
    let thema = if thema.trim().is_empty() { "Allgemeinwissen" } else { thema.trim() };
    let kontext = cut(kontext, 400);
    let zusatz = if kontext.is_empty() { String::new() } else { format!(" Halte dich an diese Inhalte aus dem Lernhelfer: „{kontext}“.") };
    let auftrag = format!(
        "Erstelle genau {ROUNDS} Multiple-Choice-Quizfragen auf Deutsch zum Thema „{thema}“ auf Universitätsniveau.{zusatz} \
         Antworte ausschließlich mit JSON in genau dieser Form: \
         {{\"questions\":[{{\"q\":\"Fragetext (höchstens 110 Zeichen)\",\"options\":[\"A) ...\",\"B) ...\",\"C) ...\",\"D) ...\"],\"correct\":0,\"explanation\":\"Ein kurzer Satz (höchstens 90 Zeichen)\"}}]}}. \
         Je Frage genau 4 Optionen (je höchstens 40 Zeichen), genau eine ist richtig; \"correct\" ist ihr Index 0 bis 3 als Zahl. \
         Verteile die richtige Stelle gleichmäßig, keine doppelten Fragen, fachlich korrekt und jugendfrei. \
         Formeln schreibst du als LaTeX zwischen $…$."
    );
    let body = json!({
        "model": cfg.model,
        "response_format": { "type": "json_object" },
        "temperature": 0.7,
        "messages": [
            { "role": "system", "content": "Du bist ein sorgfältiger Quiz-Autor für Schülerinnen, Schüler und Studierende. Du antwortest nur mit gültigem JSON." },
            { "role": "user", "content": auftrag }
        ]
    })
    .to_string();
    // curl entschlüsselt in "…" nur \\ und \" (sowie \t \n \r \v) – serde_json liefert keine rohen Steuerzeichen
    let esc = body.replace('\\', "\\\\").replace('"', "\\\"");
    Ok(format!("url = \"{}\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata = \"{esc}\"\n", cfg.url, cfg.key))
}
/// Blockiert (nur in spawn_blocking aufrufen). Fehlertexte enthalten weder stderr noch die Konfiguration.
fn run_curl(conf: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut ch = Command::new("curl").args(["-sS", "--max-time", "6", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| "curl ließ sich nicht starten.".to_string())?;
    if let Some(mut si) = ch.stdin.take() {
        let _ = si.write_all(conf.as_bytes()); // schlägt das fehl, meldet curl gleich selbst einen Fehler
    }
    let out = ch.wait_with_output().map_err(|_| "curl wurde abgebrochen.".to_string())?;
    if !out.status.success() {
        return Err(match out.status.code() {
            Some(28) => "OpenRouter hat nicht rechtzeitig geantwortet.".to_string(),
            Some(c) => format!("Keine Verbindung zu OpenRouter (curl-Fehler {c})."),
            None => "curl wurde abgebrochen.".to_string(),
        });
    }
    String::from_utf8(out.stdout).map_err(|_| "Die OpenRouter-Antwort ist kein Text.".to_string())
}
/// OpenRouter-Antwort → Fragen: `choices[0].message.content`, etwaige ```-Zäune fallen weg (nur der Teil von { bis }).
fn parse_answer(body: &str) -> Result<Vec<Question>, String> {
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| "OpenRouter hat kein JSON geliefert.".to_string())?;
    if let Some(e) = v.get("error") {
        let m = e.get("message").and_then(Value::as_str).unwrap_or("unbekannter Fehler");
        return Err(format!("OpenRouter meldet: {}", cut(m, 120)));
    }
    let c = v.pointer("/choices/0/message/content").and_then(Value::as_str).ok_or_else(|| "Die OpenRouter-Antwort enthält keinen Text.".to_string())?;
    match (c.find('{'), c.rfind('}')) {
        (Some(a), Some(b)) if a < b => parse_questions(&c[a..=b]),
        _ => Err("Die KI-Antwort ist kein gültiges JSON.".into()),
    }
}
/// Ein OpenRouter-Aufruf über curl (Schlüssel nur über stdin), höchstens TIMEOUT_MS. Hält nie die Spiel-Sperre.
pub async fn fetch(cfg: KiCfg, thema: String, kontext: String) -> Result<Vec<Question>, String> {
    let conf = curl_config(&cfg, &thema, &kontext)?;
    let job = tokio::task::spawn_blocking(move || run_curl(&conf));
    let res = match tokio::time::timeout(std::time::Duration::from_millis(TIMEOUT_MS), job).await {
        Err(_) => Err("OpenRouter hat nicht rechtzeitig geantwortet.".to_string()),
        Ok(Err(_)) => Err("Der KI-Auftrag wurde abgebrochen.".to_string()),
        Ok(Ok(r)) => r.and_then(|body| parse_answer(&body)),
    };
    // Gürtel und Hosenträger: sollte ein fremder Fehlertext den Schlüssel spiegeln, fällt er hier heraus
    res.map_err(|e| e.replace(&cfg.key, "…"))
}

fn nz(o: Option<String>) -> Option<String> {
    o.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl Game {
    /// (Schlüssel, Quelle): Verwaltung vor Umgebung.
    fn ki_key(&self) -> (String, &'static str) {
        match (nz(self.cfg("openrouter_key")), nz(std::env::var("OPENROUTER_API_KEY").ok())) {
            (Some(k), _) => (k, "verwaltung"),
            (None, Some(k)) => (k, "umgebung"),
            _ => (String::new(), "keine"),
        }
    }
    pub fn ki_cfg(&self) -> KiCfg {
        let model = nz(self.cfg("openrouter_model")).or_else(|| nz(std::env::var("OPENROUTER_MODEL").ok())).filter(|m| model_ok(m)).unwrap_or_else(|| DEFAULT_MODEL.to_string());
        KiCfg { key: self.ki_key().0, model, url: nz(self.cfg("openrouter_url")).unwrap_or_else(|| DEFAULT_URL.to_string()) }
    }
    /// Legt Fragen ab (Doppelte je Thema/Frage fallen weg; die alte Spalte `stufe` bleibt immer 0); liefert die Zahl der neuen Zeilen.
    pub fn cache_put(&self, thema: &str, qs: &[Question]) -> R<usize> {
        let (t, now) = (normalize_topic(thema), self.now());
        let mut st = self.db.prepare_cached("insert or ignore into duel_question_cache (topic, stufe, q, o0, o1, o2, o3, correct, explanation, used, ts) values (?, 0, ?, ?, ?, ?, ?, ?, ?, 0, ?)")?;
        let mut n = 0;
        for q in qs.iter().filter(|q| q.correct <= 3) {
            n += st.execute(params![t, q.q, q.options[0], q.options[1], q.options[2], q.options[3], q.correct, q.explanation, now])?;
        }
        Ok(n)
    }
    /// Bis zu `n` Fragen, die am seltensten benutzten zuerst; erhöht deren Zähler.
    pub fn cache_take(&self, thema: &str, n: usize) -> R<Vec<Question>> {
        let (t, n) = (normalize_topic(thema), n as i64);
        let qs = self
            .db
            .prepare_cached("select q, o0, o1, o2, o3, correct, explanation from duel_question_cache where topic = ?1 order by used, id limit ?2")?
            .query_map(params![t, n], |r| Ok(Question { q: r.get(0)?, options: [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?], correct: r.get(5)?, explanation: r.get(6)? }))?
            .collect::<Result<Vec<_>, _>>()?;
        self.db
            .prepare_cached("update duel_question_cache set used = used + 1 where id in (select id from duel_question_cache where topic = ?1 order by used, id limit ?2)")?
            .execute(params![t, n])?;
        Ok(qs)
    }
    fn cache_unused(&self, thema: &str) -> usize {
        self.db.query_row("select count(*) from duel_question_cache where topic = ? and used = 0", params![normalize_topic(thema)], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize
    }
    fn cache_count(&self) -> i64 {
        self.db.query_row("select count(*) from duel_question_cache", [], |r| r.get(0)).unwrap_or(0)
    }
    /// Startet bei Bedarf den KI-Auftrag für dieses Duell. true = Auftrag läuft, das Duell darf bis TIMEOUT_MS warten.
    pub fn questions_request(&mut self, duel: u32, thema: &str, kontext: &str) -> bool {
        let cfg = self.ki_cfg();
        if cfg.key.is_empty() || self.cache_unused(thema) >= ROUNDS {
            return false;
        }
        let Ok(h) = tokio::runtime::Handle::try_current() else { return false };
        let (tx, thema, kontext) = (self.jobs_tx.clone(), thema.to_string(), kontext.to_string());
        h.spawn(async move {
            let res = fetch(cfg, thema.clone(), kontext.clone()).await;
            let _ = tx.send(Job::Fragen { duel, thema, kontext, res });
        });
        true
    }
    /// Sofort genau ROUNDS Fragen: Cache, aufgefüllt aus der festen Bank. Blockiert nie.
    pub fn questions_now(&mut self, thema: &str, fach: u8) -> Vec<Question> {
        let mut qs = self.cache_take(thema, ROUNDS).unwrap_or_default();
        qs.truncate(ROUNDS);
        // zuerst ohne Doppelte, im zweiten Durchgang notfalls mit Wiederholung
        let bank = fallback(fach, ROUNDS * 2, self.now() as u64);
        for doppelt in [false, true] {
            for q in &bank {
                if qs.len() < ROUNDS && (doppelt || !qs.iter().any(|x| x.q == q.q)) {
                    qs.push(q.clone());
                }
            }
        }
        while qs.len() < ROUNDS {
            // nur bei leerer Bank und leerem Cache
            qs.push(Question { q: "Wie viel ist 7 · 8?".into(), options: ["54", "56", "63", "48"].map(String::from), correct: 1, explanation: "7 · 8 = 56.".into() });
        }
        qs
    }
    /// Im Takt: Ergebnis eines Auftrags eintragen (Cache) und an das Duell geben (duel_questions).
    pub fn job_done(&mut self, j: Job) {
        let Job::Fragen { duel, thema, res, .. } = j;
        let qs = match res {
            Ok(qs) => {
                // ablegen und gleich als benutzt zählen (sonst spielte das nächste Duell dieselben Fragen ohne neue Anfrage)
                let _ = self.cache_put(&thema, &qs);
                match self.cache_take(&thema, ROUNDS) {
                    Ok(c) if c.len() >= qs.len().min(ROUNDS) => c,
                    _ => qs,
                }
            }
            Err(e) => {
                eprintln!("duel_ai: {e}");
                Vec::new()
            }
        };
        self.duel_questions(duel, qs);
    }
    pub fn ki_get(&mut self) -> R<Value> {
        let (key, quelle) = self.ki_key();
        let n = key.chars().count();
        // nur die letzten 4 Zeichen, und die auch nur bei Schlüsseln, die lang genug sind
        let maske = if n >= 12 { format!("…{}", key.chars().skip(n - 4).collect::<String>()) } else if n > 0 { "…".to_string() } else { String::new() };
        Ok(json!({ "key_gesetzt": n > 0, "key_maske": maske, "quelle": quelle, "modell": self.ki_cfg().model, "modelle": MODELLE, "cache": self.cache_count() }))
    }
    pub fn ki_post(&mut self, b: &Value) -> R<Value> {
        let feld = |k: &str| match b.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.trim().to_string())),
            _ => no(400, "Schlüssel und Modell müssen Text sein."),
        };
        let (key, modell) = (feld("key")?, feld("modell")?);
        if key.as_deref().is_some_and(|k| !k.is_empty() && !key_ok(k)) {
            return no(400, "Der Schlüssel enthält unzulässige Zeichen (Leerzeichen, Anführungszeichen oder Backslash).");
        }
        if modell.as_deref().is_some_and(|m| !m.is_empty() && !model_ok(m)) {
            return no(400, "Der Modellname darf nur Buchstaben, Ziffern und . _ / : - enthalten (höchstens 80 Zeichen).");
        }
        for (k, v) in [("openrouter_key", key), ("openrouter_model", modell)] {
            match v {
                Some(v) if v.is_empty() => {
                    self.db.execute("delete from server_config where k = ?", [k])?;
                }
                Some(v) => self.cfg_set(k, &v)?,
                None => {}
            }
        }
        self.ki_get()
    }
    pub fn ki_test_done(&mut self, thema: &str, res: Result<Vec<Question>, String>) -> R<Value> {
        let res = res.and_then(|qs| if qs.is_empty() { Err("Die KI hat keine Frage geliefert.".to_string()) } else { Ok(qs) });
        Ok(match res {
            Ok(qs) => {
                self.cache_put(thema, &qs)?;
                let f = &qs[0];
                json!({ "ok": true, "anzahl": qs.len(), "frage": { "q": f.q, "antworten": f.options, "richtig": f.correct, "erklaerung": f.explanation }, "cache": self.cache_count() })
            }
            Err(e) => json!({ "ok": false, "fehler": e, "cache": self.cache_count() }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::game;

    fn frage(i: usize) -> Question {
        Question { q: format!("Frage {i}?"), options: ["a", "b", "c", "d"].map(String::from), correct: (i % 4) as u8, explanation: "Darum.".into() }
    }
    const KEY: &str = "sk-or-v1-streng-geheim-1234abcd";

    #[test]
    fn parse_gueltig() {
        let qs = parse_questions(r#"{"questions":[{"q":"Wie viel ist 2 + 2?","options":["A) 3","B) 4","c) 5","D) 6"],"correct":1,"explanation":"2 + 2 = 4."}]}"#).unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].q, "Wie viel ist 2 + 2?");
        assert_eq!(qs[0].options, ["3", "4", "5", "6"].map(String::from));
        assert_eq!(qs[0].correct, 1);
        assert_eq!(qs[0].explanation, "2 + 2 = 4.");
        // ein Buchstabe, der nicht zur Stelle passt, ist kein Vorsatz; fehlende Erklärung ist erlaubt
        let qs = parse_questions(r#"{"questions":[{"q":"x?","options":["B) eins","zwei","drei","vier"],"correct":0}]}"#).unwrap();
        assert_eq!(qs[0].options[0], "B) eins");
        assert_eq!(qs[0].explanation, "");
    }
    #[test]
    fn parse_verwirft_ungueltige() {
        let gut = r#"{"q":"gut?","options":["1","2","3","4"],"correct":3,"explanation":"e"}"#;
        let schlecht = [
            r#"{"q":"drei?","options":["1","2","3"],"correct":0,"explanation":"e"}"#,
            r#"{"q":"fuenf?","options":["1","2","3","4","5"],"correct":0,"explanation":"e"}"#,
            r#"{"q":"vier?","options":["1","2","3","4"],"correct":4,"explanation":"e"}"#,
            r#"{"q":"text?","options":["1","2","3","4"],"correct":"1","explanation":"e"}"#,
            r#"{"q":"minus?","options":["1","2","3","4"],"correct":-1,"explanation":"e"}"#,
            r#"{"q":"fehlt?","options":["1","2","3","4"],"explanation":"e"}"#,
            r#"{"options":["1","2","3","4"],"correct":0,"explanation":"e"}"#,
            r#"{"q":"leer?","options":["1","","3","4"],"correct":0,"explanation":"e"}"#,
            r#"{"q":"zahl?","options":["1",2,"3","4"],"correct":0,"explanation":"e"}"#,
            r#"{"q":"nur vorsatz?","options":["A)","B) b","C) c","D) d"],"correct":0,"explanation":"e"}"#,
        ];
        for s in schlecht {
            assert!(parse_questions(&format!(r#"{{"questions":[{s}]}}"#)).is_err(), "{s}");
            let qs = parse_questions(&format!(r#"{{"questions":[{s},{gut}]}}"#)).unwrap();
            assert_eq!(qs.len(), 1, "{s}");
            assert_eq!(qs[0].q, "gut?");
        }
        for s in ["", "kein json", "[]", r#"{"questions":[]}"#, r#"{"questions":"x"}"#, r#"{"fragen":[]}"#] {
            assert!(parse_questions(s).is_err(), "{s}");
        }
    }
    #[test]
    fn parse_entschaerft_und_kappt() {
        let lang = "x".repeat(300);
        let qs = parse_questions(&json!({ "questions": [{ "q": format!("<script>alert(1)</script>{lang}"), "options": ["<b>fett</b>", lang, "c\u{7}c", "d"], "correct": 0, "explanation": format!("<img src=x>{lang}") }] }).to_string()).unwrap();
        let q = &qs[0];
        for t in [&q.q, &q.explanation, &q.options[0], &q.options[1], &q.options[2]] {
            assert!(!t.contains('<') && !t.contains('>') && !t.chars().any(char::is_control), "{t}");
        }
        assert_eq!(q.q.chars().count(), 110);
        assert_eq!(q.options[1].chars().count(), 60);
        assert_eq!(q.explanation.chars().count(), 90);
        assert_eq!(q.options[2], "cc");
    }
    #[test]
    fn thema_vereinheitlicht() {
        assert_eq!(normalize_topic("  Kalter   KRIEG \n"), "kalter krieg");
        assert_eq!(normalize_topic("<b>Photosynthese</b>"), "‹b›photosynthese‹/b›");
        assert_eq!(normalize_topic(&"ä".repeat(200)).chars().count(), 60);
        assert_eq!(normalize_topic("   "), "");
    }
    #[test]
    fn bank_und_fallback() {
        for fach in 0..4u8 {
            {
                assert!(FALLBACK.iter().filter(|f| f.fach == fach).count() >= ROUNDS, "Fach {fach}");
                let qs = fallback(fach, ROUNDS, 7);
                assert_eq!(qs.len(), ROUNDS);
                // alle aus dem passenden Fach und der passenden Stufe, keine Doppelten
                for (i, q) in qs.iter().enumerate() {
                    assert!(FALLBACK.iter().any(|f| f.fach == fach && f.q == q.q), "{}", q.q);
                    assert!(q.correct <= 3 && q.options.iter().all(|o| !o.is_empty()));
                    assert!(!qs[..i].iter().any(|x| x.q == q.q));
                }
            }
        }
        assert!(FALLBACK.iter().all(|f| f.correct <= 3 && f.fach <= 3 && f.stufe <= 2));
        assert_eq!(fallback(2, ROUNDS, 42), fallback(2, ROUNDS, 42));
        assert!((0..20).any(|s| fallback(2, ROUNDS, s) != fallback(2, ROUNDS, 42)));
        // gemischt: ohne Doppelte, solange die Bank reicht; darüber hinaus Wiederholung statt Lücke
        let alle = fallback(255, FALLBACK.len(), 3);
        let mut namen: Vec<&str> = alle.iter().map(|q| q.q.as_str()).collect();
        namen.sort_unstable();
        namen.dedup();
        assert_eq!(namen.len(), FALLBACK.len());
        assert_eq!(fallback(0, FALLBACK.len() + 5, 3).len(), FALLBACK.len() + 5);
        assert!(fallback(9, ROUNDS, 1).len() == ROUNDS);
    }
    #[test]
    fn cache_ablegen_und_holen() {
        let g = game();
        let qs: Vec<Question> = (0..8).map(frage).collect();
        assert_eq!(g.cache_put("Kalter  Krieg", &qs).unwrap(), 8);
        assert_eq!(g.cache_put("kalter krieg", &qs).unwrap(), 0, "Doppelte nicht doppelt");
        assert_eq!(g.cache_unused("KALTER KRIEG"), 8);
        assert_eq!(g.cache_take("kalter krieg", 8).unwrap(), qs);
        assert_eq!(g.cache_unused("kalter krieg"), 0);
        // am seltensten benutzte zuerst: die neue Frage kommt vor den schon gespielten
        assert_eq!(g.cache_put("kalter krieg", &[frage(99)]).unwrap(), 1);
        assert_eq!(g.cache_take("kalter krieg", 3).unwrap()[0], frage(99));
        let t = std::time::Instant::now();
        for _ in 0..100 {
            assert_eq!(g.cache_take("kalter krieg", 8).unwrap().len(), 8);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / 100.0;
        assert!(ms < 1.0, "Cache-Zugriff im Mittel {ms:.3} ms");
    }
    #[test]
    fn ohne_schluessel_und_ohne_laufzeit() {
        let mut g = game();
        // kein tokio in diesem Test: selbst mit Schlüssel wird nichts gestartet und nichts blockiert
        assert!(!g.questions_request(1, "bruchrechnung", ""));
        g.cfg_set("openrouter_key", KEY).unwrap();
        assert!(!g.questions_request(1, "bruchrechnung", ""));
        let t = std::time::Instant::now();
        let qs = g.questions_now("bruchrechnung", 0);
        assert!(t.elapsed().as_millis() < 500);
        assert_eq!(qs.len(), ROUNDS);
        for (i, q) in qs.iter().enumerate() {
            assert!(!qs[..i].iter().any(|x| x.q == q.q));
        }
        // Cache geht vor, die Bank füllt auf
        g.cache_put("bruchrechnung", &[frage(1), frage(2)]).unwrap();
        let qs = g.questions_now("Bruchrechnung", 255);
        assert_eq!(qs.len(), ROUNDS);
        assert_eq!(qs[..2], [frage(1), frage(2)]);
        assert_eq!(g.questions_now("", 255).len(), ROUNDS);
    }
    #[test]
    fn auftrag_fuellt_den_cache() {
        let mut g = game();
        g.job_done(Job::Fragen { duel: 77, thema: "optik".into(), kontext: String::new(), res: Ok((0..8).map(frage).collect()) });
        assert_eq!(g.cache_count(), 8);
        assert_eq!(g.cache_unused("optik"), 0, "an das Duell gegebene Fragen zählen als benutzt");
        g.job_done(Job::Fragen { duel: 77, thema: "optik".into(), kontext: String::new(), res: Err("aus".into()) });
        assert_eq!(g.cache_count(), 8);
    }
    #[test]
    fn verwaltung_zeigt_den_schluessel_nie() {
        let mut g = game();
        let v = g.ki_post(&json!({ "key": KEY, "modell": "openai/gpt-4o-mini" })).unwrap();
        assert!(!v.to_string().contains(KEY) && !v.to_string().contains("geheim"));
        assert_eq!(v["key_gesetzt"], true);
        assert_eq!(v["key_maske"], "…abcd");
        assert_eq!(v["quelle"], "verwaltung");
        assert_eq!(v["modell"], "openai/gpt-4o-mini");
        assert_eq!(v["modelle"][0], DEFAULT_MODEL);
        assert!(v["modelle"].as_array().unwrap().len() >= 4);
        assert_eq!(g.ki_cfg().key, KEY);
        assert_eq!(g.ki_cfg().url, DEFAULT_URL);
        // ohne Feld „key“ bleibt der Schlüssel; leeres Modell = Standard
        let v = g.ki_post(&json!({ "modell": "" })).unwrap();
        assert_eq!(v["key_maske"], "…abcd");
        assert!(g.cfg("openrouter_model").is_none());
        // ungültige Eingaben ändern nichts
        for b in [json!({ "key": "ab\"cd" }), json!({ "key": "ab\\cd" }), json!({ "key": "ab cd" }), json!({ "key": 5 }), json!({ "modell": "böse modell" }), json!({ "key": "neu-und-gut-123456", "modell": "x y" })] {
            assert!(matches!(g.ki_post(&b), Err(crate::E(400, _))), "{b}");
        }
        assert_eq!(g.ki_cfg().key, KEY);
        // Test-Ergebnis: Frage und Zähler, nie der Schlüssel
        let v = g.ki_test_done("Optik", Ok(vec![frage(1), frage(2)])).unwrap();
        assert_eq!((v["ok"].clone(), v["anzahl"].clone(), v["cache"].clone()), (json!(true), json!(2), json!(2)));
        assert_eq!(v["frage"]["antworten"].as_array().unwrap().len(), 4);
        assert_eq!(v["frage"]["richtig"], 1);
        let v = g.ki_test_done("Optik", Err("kaputt".into())).unwrap();
        assert_eq!((v["ok"].clone(), v["fehler"].clone(), v["cache"].clone()), (json!(false), json!("kaputt"), json!(2)));
        assert_eq!(g.ki_get().unwrap()["cache"], 2);
        // leerer Schlüssel löscht
        let v = g.ki_post(&json!({ "key": "" })).unwrap();
        assert!(g.cfg("openrouter_key").is_none());
        assert_ne!(v["quelle"], "verwaltung");
        assert!(!v.to_string().contains(KEY));
    }
    #[test]
    fn curl_konfiguration() {
        let cfg = KiCfg { key: KEY.into(), model: DEFAULT_MODEL.into(), url: DEFAULT_URL.into() };
        let c = curl_config(&cfg, "sagt \"hallo\" \\ ende", "").unwrap();
        assert_eq!(c.matches(KEY).count(), 1, "Schlüssel nur in der Kopfzeile");
        let z: Vec<&str> = c.lines().collect();
        assert_eq!(z.len(), 4, "keine rohen Zeilenumbrüche in den Daten");
        assert_eq!(z[0], format!("url = \"{DEFAULT_URL}\""));
        assert_eq!(z[1], format!("header = \"Authorization: Bearer {KEY}\""));
        assert_eq!(z[2], "header = \"Content-Type: application/json\"");
        // Daten so entschlüsseln, wie curl es tut, und als JSON lesen
        let roh = z[3].strip_prefix("data = \"").and_then(|s| s.strip_suffix('"')).unwrap();
        let (mut body, mut esc) = (String::new(), false);
        for ch in roh.chars() {
            if esc || ch != '\\' {
                assert!(esc || ch != '"', "Anführungszeichen ohne Schutz");
                body.push(ch);
                esc = false;
            } else {
                esc = true;
            }
        }
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], DEFAULT_MODEL);
        assert_eq!(v["response_format"]["type"], "json_object");
        let auftrag = v["messages"][1]["content"].as_str().unwrap();
        assert!(auftrag.contains("sagt \"hallo\" \\ ende") && auftrag.contains("Universitätsniveau") && auftrag.contains("genau 8"));
        // unbrauchbare Schlüssel und Adressen werden abgelehnt, ohne sie zu zitieren
        for k in ["", "ab\"cd", "ab\\cd", "ab\ncd", "ab cd", "ab'cd"] {
            let e = curl_config(&KiCfg { key: k.into(), ..cfg.clone() }, "x", "").unwrap_err();
            assert!(k.is_empty() || !e.contains(k));
        }
        for u in ["", "ftp://x", "file:///etc/passwd", "https://x/\"\nheader = \"X: y"] {
            assert!(curl_config(&KiCfg { url: u.into(), ..cfg.clone() }, "x", "").is_err(), "{u}");
        }
    }
    #[test]
    fn antwort_lesen() {
        let inhalt = "```json\n{\"questions\":[{\"q\":\"a?\",\"options\":[\"A) 1\",\"B) 2\",\"C) 3\",\"D) 4\"],\"correct\":2,\"explanation\":\"e\"}]}\n```";
        let qs = parse_answer(&json!({ "choices": [{ "message": { "content": inhalt } }] }).to_string()).unwrap();
        assert_eq!((qs.len(), qs[0].correct, qs[0].options[2].as_str()), (1, 2, "3"));
        assert!(parse_answer(&json!({ "error": { "message": "No auth credentials found" } }).to_string()).unwrap_err().contains("No auth"));
        assert!(parse_answer(&json!({ "choices": [] }).to_string()).is_err());
        assert!(parse_answer(&json!({ "choices": [{ "message": { "content": "leider nein" } }] }).to_string()).is_err());
        assert!(parse_answer("<html>").is_err());
    }
    #[tokio::test]
    async fn fetch_ohne_gegenstelle_endet_rechtzeitig() {
        let t = std::time::Instant::now();
        let e = fetch(KiCfg { key: KEY.into(), model: DEFAULT_MODEL.into(), url: "http://127.0.0.1:9/".into() }, "optik".into(), String::new()).await.unwrap_err();
        assert!((t.elapsed().as_millis() as u64) < TIMEOUT_MS);
        assert!(!e.contains(KEY) && !e.is_empty());
        // ohne Schlüssel wird curl gar nicht erst gestartet
        assert!(fetch(KiCfg { key: String::new(), model: DEFAULT_MODEL.into(), url: DEFAULT_URL.into() }, "optik".into(), String::new()).await.is_err());
    }
}
