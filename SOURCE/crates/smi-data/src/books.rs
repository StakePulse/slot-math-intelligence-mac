//! Lecture d'un dossier PUBLIÉ Stake Engine (`index.json`, `lookUpTable_<mode>_0.csv`,
//! `books_<mode>.jsonl[.zst]`) pour les contrôles de conception de `smi_core::design` (D-17).
//!
//! LECTURE SEULE : le dossier analysé n'est jamais modifié (aucune écriture, aucun fichier créé à côté) ;
//! seul le rapport est enregistré dans la base de Slot Math Intelligence.
//!
//! Ce qui est lu dans chaque book (format du SDK de Stake et de nos moteurs) :
//! - `id` (rapproché de la lookup table : poids et paiement font foi), `criteria` ;
//! - événements de révélation (`board`, contexte `gameType` ou `context`) : symboles affichés ;
//! - événements de gain (`winInfo` par défaut) : `wins[].symbol` et `amount` (ou `win`) ;
//! - événement de déclenchement (liste réglable + tout type contenant « trigger ») : `positions` des scatters
//!   et nom du `bonus`.

use crate::settings::AppSettings;
use crate::Db;
use anyhow::Context;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smi_core::design::{analyze, tier_of, BookRow, DesignReport, DesignRules, MarketRef, ModeDesign, ModeInput};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PublishedMode {
    pub name: String,
    pub cost: f64,
    pub events: PathBuf,
    pub weights: PathBuf,
}

/// Modes déclarés par `index.json` (format Stake Engine : `{"modes": [{name, cost, events, weights}]}`).
pub fn read_index(dir: &Path) -> anyhow::Result<Vec<PublishedMode>> {
    let p = dir.join("index.json");
    let text = std::fs::read_to_string(&p).with_context(|| format!("index.json introuvable dans {}", dir.display()))?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).context("index.json illisible")?;
    let modes = v["modes"].as_array().ok_or_else(|| anyhow::anyhow!("index.json : liste `modes` absente"))?;
    let mut out = vec![];
    for m in modes {
        let name = m["name"].as_str().ok_or_else(|| anyhow::anyhow!("index.json : mode sans nom"))?.to_string();
        let cost = m["cost"].as_f64().ok_or_else(|| anyhow::anyhow!("index.json : mode {name} sans coût"))?;
        let events = dir.join(m["events"].as_str().ok_or_else(|| anyhow::anyhow!("index.json : mode {name} sans fichier de books"))?);
        let weights = dir.join(m["weights"].as_str().ok_or_else(|| anyhow::anyhow!("index.json : mode {name} sans lookup table"))?);
        out.push(PublishedMode { name, cost, events, weights });
    }
    if out.is_empty() {
        anyhow::bail!("index.json ne déclare aucun mode");
    }
    Ok(out)
}

/// Lookup table : id → (poids, paiement en centièmes).
fn read_lut(path: &Path) -> anyhow::Result<BTreeMap<u64, (u64, u64)>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("lookup table illisible : {}", path.display()))?;
    let mut m = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let f: Vec<&str> = line.trim().trim_start_matches('\u{feff}').split(',').map(str::trim).collect();
        if f.len() < 3 || f[0].parse::<u64>().is_err() {
            continue;
        }
        let (id, w, p) = (f[0].parse::<u64>()?, f[1].parse::<u64>().with_context(|| format!("ligne {} : poids", i + 1))?, f[2].parse::<u64>().with_context(|| format!("ligne {} : paiement", i + 1))?);
        m.insert(id, (w, p));
    }
    if m.is_empty() {
        anyhow::bail!("lookup table vide : {}", path.display());
    }
    Ok(m)
}

/// Lecteur zstd qui enchaîne les trames (un fichier peut en contenir plusieurs).
struct MultiFrame<R: BufRead> {
    state: Option<State<R>>,
}

enum State<R: BufRead> {
    Raw(R),
    Frame(ruzstd::decoding::StreamingDecoder<R, ruzstd::decoding::FrameDecoder>),
}

impl<R: BufRead> Read for MultiFrame<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.state.take() {
                None => return Ok(0),
                Some(State::Frame(mut d)) => {
                    let n = d.read(buf)?;
                    if n > 0 {
                        self.state = Some(State::Frame(d));
                        return Ok(n);
                    }
                    self.state = Some(State::Raw(d.into_parts().0));
                }
                Some(State::Raw(mut r)) => {
                    if r.fill_buf()?.is_empty() {
                        return Ok(0);
                    }
                    let d = ruzstd::decoding::StreamingDecoder::new(r).map_err(|e| std::io::Error::other(format!("trame zstd illisible : {e}")))?;
                    self.state = Some(State::Frame(d));
                }
            }
        }
    }
}

fn open_lines(path: &Path) -> anyhow::Result<Box<dyn BufRead>> {
    let f = std::fs::File::open(path).with_context(|| format!("books introuvables : {}", path.display()))?;
    let r = BufReader::with_capacity(1 << 20, f);
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zst")) {
        Ok(Box::new(BufReader::with_capacity(1 << 20, MultiFrame { state: Some(State::Raw(r)) })))
    } else {
        Ok(Box::new(r))
    }
}

/// Empreinte FNV-1a d'une valeur JSON sous forme canonique (clés triées), sans les champs `index`.
fn content_hash(v: &Value) -> u64 {
    fn feed(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h ^= *b as u64;
            *h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    fn walk(h: &mut u64, v: &Value) {
        match v {
            Value::Null => feed(h, b"n"),
            Value::Bool(b) => feed(h, if *b { b"t" } else { b"f" }),
            Value::Number(n) => {
                feed(h, b"#");
                feed(h, n.to_string().as_bytes());
            }
            Value::String(s) => {
                feed(h, b"\"");
                feed(h, s.as_bytes());
                feed(h, b"\"");
            }
            Value::Array(a) => {
                feed(h, b"[");
                for x in a {
                    walk(h, x);
                    feed(h, b",");
                }
                feed(h, b"]");
            }
            Value::Object(o) => {
                feed(h, b"{");
                let mut keys: Vec<&String> = o.keys().filter(|k| k.as_str() != "index").collect();
                keys.sort();
                for k in keys {
                    feed(h, k.as_bytes());
                    feed(h, b":");
                    walk(h, &o[k]);
                    feed(h, b",");
                }
                feed(h, b"}");
            }
        }
    }
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    walk(&mut h, v);
    h
}

fn symbol_name(cell: &Value) -> Option<&str> {
    cell.as_str().or_else(|| cell.get("name").and_then(Value::as_str)).or_else(|| cell.get("symbol").and_then(Value::as_str))
}

fn context_is_base(e: &Value, triggered: bool) -> bool {
    for k in ["gameType", "context", "game_type"] {
        if let Some(s) = e.get(k).and_then(Value::as_str) {
            let s = s.to_ascii_lowercase();
            return s.contains("base");
        }
    }
    !triggered
}

/// Lit un mode : chaque book est réduit à une [`BookRow`] ; plateaux et gains sont agrégés par poids.
pub fn read_mode(m: &PublishedMode, rules: &DesignRules, progress: &dyn Fn(f64), cancel: &AtomicBool) -> anyhow::Result<(ModeInput, Vec<String>)> {
    let lut = read_lut(&m.weights)?;
    let total = lut.len().max(1) as f64;
    let mut inp = ModeInput { name: m.name.clone(), cost: m.cost, ..Default::default() };
    let mut notes = vec![];
    let mut criteria: BTreeMap<String, (u64, bool)> = BTreeMap::new();
    let mut symbols: BTreeMap<String, u64> = BTreeMap::new();
    let (mut missing, mut mismatch, mut n) = (0u64, 0u64, 0u64);
    let mut seen = std::collections::BTreeSet::new();
    let reader = open_lines(&m.events)?;
    for line in reader.lines() {
        let line = line.with_context(|| format!("lecture de {}", m.events.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("analyse annulée");
        }
        let b: Value = serde_json::from_str(&line).with_context(|| format!("{} : book n° {} illisible", m.events.display(), n + 1))?;
        n += 1;
        if n % 5_000 == 0 {
            progress(n as f64 / total);
        }
        let Some(id) = b.get("id").and_then(Value::as_u64) else {
            missing += 1;
            continue;
        };
        let Some(&(weight, payout)) = lut.get(&id) else {
            missing += 1;
            continue;
        };
        seen.insert(id);
        if let Some(pm) = b.get("payoutMultiplier").and_then(Value::as_f64) {
            if (pm - payout as f64).abs() > 0.5 {
                mismatch += 1;
            }
        }
        let crit = b.get("criteria").and_then(Value::as_str).unwrap_or("").to_string();
        let w = weight as u128;
        let (mut triggered, mut group, mut scatters) = (false, None::<String>, None::<u32>);
        let events = b.get("events").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut base_ctx = true;
        for e in &events {
            let t = e.get("type").and_then(Value::as_str).unwrap_or("");
            let is_trigger = (rules.trigger_events.iter().any(|x| x == t) || t.to_ascii_lowercase().contains("trigger")) && e.get("positions").and_then(Value::as_array).is_some();
            if is_trigger {
                if !triggered && base_ctx {
                    scatters = e.get("positions").and_then(Value::as_array).map(|a| a.len() as u32);
                    group = e.get("bonus").and_then(Value::as_str).map(String::from);
                }
                triggered = true;
                inp.has_triggers = true;
                continue;
            }
            if let Some(board) = e.get("board").and_then(Value::as_array) {
                base_ctx = context_is_base(e, triggered);
                if base_ctx {
                    inp.has_boards = true;
                    for reel in board {
                        for cell in reel.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                            if let Some(s) = symbol_name(cell) {
                                let tier = tier_of(s, rules);
                                inp.board[tier.idx()] += w;
                                *symbols.entry(s.to_string()).or_default() += 1;
                            }
                        }
                    }
                }
                continue;
            }
            if rules.win_events.iter().any(|x| x == t) {
                if let Some(wins) = e.get("wins").and_then(Value::as_array) {
                    inp.has_win_events = true;
                    let ctx = if base_ctx { 0 } else { 1 };
                    for win in wins {
                        let Some(s) = win.get("symbol").and_then(Value::as_str) else { continue };
                        let amount = win.get("amount").or_else(|| win.get("win")).and_then(Value::as_f64).unwrap_or(0.0);
                        let tier = tier_of(s, rules);
                        let slot = &mut inp.wins[ctx][tier.idx()];
                        slot.0 += w;
                        slot.1 += w * (amount.max(0.0) * 100.0).round() as u128;
                    }
                }
            }
        }
        let bonus = triggered || (!crit.is_empty() && !rules.no_bonus_criteria.iter().any(|c| c.eq_ignore_ascii_case(&crit)));
        let e = criteria.entry(crit.clone()).or_insert((0, bonus));
        e.0 += 1;
        e.1 |= bonus;
        inp.rows.push(BookRow {
            weight,
            payout,
            bonus,
            group: group.unwrap_or_else(|| crit.clone()),
            scatters,
            content: bonus.then(|| content_hash(&Value::Array(events))),
        });
    }
    progress(1.0);
    let absent = lut.keys().filter(|id| !seen.contains(id)).count();
    if missing > 0 {
        notes.push(format!("{}: {missing} book(s) sans id ou absent(s) de la lookup table (ignorés)", m.name));
    }
    if absent > 0 {
        notes.push(format!("{}: {absent} ligne(s) de la lookup table sans book", m.name));
    }
    if mismatch > 0 {
        notes.push(format!("{}: {mismatch} book(s) dont `payoutMultiplier` diffère de la table (la table fait foi)", m.name));
    }
    inp.criteria = criteria.into_iter().map(|(k, (c, b))| (k, c, b)).collect();
    inp.symbols = symbols.into_iter().map(|(s, c)| (s.clone(), tier_of(&s, rules), c)).collect();
    Ok((inp, notes))
}

/// Références du marché (pairs au max comparable) pour un mode de base.
fn market_ref(db: &Db, s: &AppSettings, inp: &ModeInput) -> Option<MarketRef> {
    let max_cents = inp.rows.iter().filter(|r| r.weight > 0).map(|r| r.payout).max()?;
    let (peers, basis) = crate::market::peers(db, s, max_cents as f64 / 100.0, inp.cost).ok()?;
    if peers.is_empty() {
        return None;
    }
    let mut hits: Vec<f64> = peers.iter().filter_map(|r| r.hit).collect();
    hits.sort_by(|a, b| a.total_cmp(b));
    let mut maxes: Vec<f64> = peers.iter().filter_map(|r| r.p_max).filter(|p| *p > 0.0).map(|p| 1.0 / p).collect();
    maxes.sort_by(|a, b| a.total_cmp(b));
    let q = smi_core::market::quantile_sorted;
    Some(MarketRef { n: peers.len(), hit_median: q(&hits, 0.5), hit_p25: q(&hits, 0.25), hit_p75: q(&hits, 0.75), max_one_in_median: q(&maxes, 0.5), basis })
}

/// Nom du jeu : premier dossier parent qui n'est pas un nom générique de sortie.
pub fn game_name(dir: &Path) -> String {
    const GENERIC: [&str; 8] = ["publish_files", "publish", "library", "math", "out", "output", "files", "books"];
    dir.ancestors()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
        .find(|n| !GENERIC.iter().any(|g| g.eq_ignore_ascii_case(n)))
        .unwrap_or("jeu")
        .to_string()
}

/// Analyse complète d'un dossier publié, enregistrée dans la base. Renvoie (id du rapport, rapport).
pub fn analyze_folder(db: &Db, s: &AppSettings, dir: &Path, progress: &dyn Fn(f64, &str), cancel: &AtomicBool) -> anyhow::Result<(i64, DesignReport)> {
    let modes = read_index(dir)?;
    let k = modes.len() as f64;
    let mut out: Vec<ModeDesign> = vec![];
    let mut notes = vec![];
    for (i, m) in modes.iter().enumerate() {
        let label = format!("mode {} ({}/{})", m.name, i + 1, modes.len());
        progress(i as f64 / k, &label);
        let (inp, mut n) = read_mode(m, &s.design, &|f| progress((i as f64 + f) / k, &label), cancel)?;
        notes.append(&mut n);
        let mk = if inp.cost <= 1.0 + 1e-9 && inp.rows.iter().any(|r| !r.bonus) { market_ref(db, s, &inp) } else { None };
        out.push(analyze(&inp, &s.design, mk.as_ref()));
    }
    let game = game_name(dir);
    let mut rep = DesignReport::new(&game, &dir.display().to_string(), &crate::now(), &s.design, out);
    if !notes.is_empty() {
        for m in rep.modes.iter_mut() {
            for n in notes.iter().filter(|n| n.starts_with(&format!("{}:", m.name))) {
                m.checks.push(smi_core::design::DesignCheck {
                    key: "lecture".into(),
                    label: "Lecture des books".into(),
                    status: smi_core::design::DStatus::Warn,
                    observed: n.splitn(2, ": ").nth(1).unwrap_or(n).to_string(),
                    target: "books et table cohérents".into(),
                    detail: String::new(),
                });
            }
        }
        rep = DesignReport::new(&rep.game, &rep.source, &rep.generated_at, &rep.rules, rep.modes);
    }
    let id = save_report(db, &rep)?;
    db.log("contrôles", &format!("Contrôles de conception de {} : {} réussis, {} à revoir, {} échecs", rep.game, rep.passed, rep.warnings, rep.failed));
    Ok((id, rep))
}

pub fn save_report(db: &Db, rep: &DesignReport) -> anyhow::Result<i64> {
    let c = db.conn();
    c.execute(
        "INSERT INTO design_reports(created_at, game, source, passed, warnings, failed, report) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![rep.generated_at, rep.game, rep.source, rep.passed as i64, rep.warnings as i64, rep.failed as i64, serde_json::to_string(rep)?],
    )?;
    Ok(c.last_insert_rowid())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReportRow {
    pub id: i64,
    pub created_at: String,
    pub game: String,
    pub source: String,
    pub passed: i64,
    pub warnings: i64,
    pub failed: i64,
}

pub fn list_reports(db: &Db) -> anyhow::Result<Vec<ReportRow>> {
    let c = db.conn();
    let mut st = c.prepare("SELECT id, created_at, game, source, passed, warnings, failed FROM design_reports ORDER BY id DESC")?;
    let rows = st
        .query_map([], |r| Ok(ReportRow { id: r.get(0)?, created_at: r.get(1)?, game: r.get(2)?, source: r.get(3)?, passed: r.get(4)?, warnings: r.get(5)?, failed: r.get(6)? }))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_report(db: &Db, id: i64) -> anyhow::Result<DesignReport> {
    let text: Option<String> = db.conn().query_row("SELECT report FROM design_reports WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
    Ok(serde_json::from_str(&text.ok_or_else(|| anyhow::anyhow!("rapport {id} introuvable"))?)?)
}

/// Rapport lisible (Markdown), à transmettre tel quel.
pub fn report_markdown(r: &DesignReport) -> String {
    use smi_core::prompt::{pct, xm};
    use std::fmt::Write;
    let mut o = String::new();
    let st = |s: smi_core::design::DStatus| match s {
        smi_core::design::DStatus::Pass => "OK",
        smi_core::design::DStatus::Warn => "À REVOIR",
        smi_core::design::DStatus::Fail => "ÉCHEC",
        smi_core::design::DStatus::NotRun => "non mesurable",
    };
    let _ = writeln!(o, "# Contrôles de conception — {}\n", r.game);
    let _ = writeln!(o, "Source : `{}` · {} · moteur {} · {} OK, {} à revoir, {} échecs, {} non mesurables. Mesures EXACTES sur les poids des lookup tables (aucune simulation).\n", r.source, r.generated_at, r.engine_version, r.passed, r.warnings, r.failed, r.not_run);
    for m in &r.modes {
        let _ = writeln!(o, "## {} ({}, prix {})\n", m.name, m.kind, xm(m.cost));
        let _ = writeln!(o, "{} books · RTP {} · {} de gains · max {} le prix ({})\n", smi_core::stats::group(m.books), pct(m.rtp, 4), pct(m.hit, 2), xm(m.max_price), m.max_one_in.map_or("—".into(), |v| format!("1 sur {}", smi_core::stats::group(v.round() as u64))));
        let _ = writeln!(o, "| Contrôle | Statut | Mesuré | Cible |\n|---|---|---|---|");
        for c in &m.checks {
            let _ = writeln!(o, "| {} | {} | {} | {} |", c.label, st(c.status), c.observed.replace('|', "/"), c.target.replace('|', "/"));
        }
        let _ = writeln!(o);
        let sc: Vec<_> = m.scatters.iter().collect();
        if !sc.is_empty() {
            let _ = writeln!(o, "Bonus selon le nombre de scatters :\n\n| Bonus | Scatters | Books | Fréquence | Moyenne | Médiane |\n|---|---|---|---|---|---|");
            for s in sc {
                let _ = writeln!(o, "| {} | {} | {}{} | {} | {} | {} |", s.group, s.scatters, smi_core::stats::group(s.books), if s.small { " (trop peu)" } else { "" }, s.one_in.map_or("—".into(), |v| format!("1 sur {}", smi_core::stats::group(v.round() as u64))), xm(s.mean), xm(s.median));
            }
            let _ = writeln!(o);
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use smi_core::design::Tier;

    fn tmp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("smi-books-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }

    /// Petit jeu publié au format Stake (books non compressés) : 4 rounds sans bonus, 2 bonus.
    fn write_game(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("index.json"), r#"{"modes":[{"name":"base","cost":1.0,"events":"books_base.jsonl","weights":"lookUpTable_base_0.csv"}]}"#).unwrap();
        std::fs::write(dir.join("lookUpTable_base_0.csv"), "1,700,0\n2,200,50\n3,50,300\n4,1,400\n5,1,500\n").unwrap();
        let books = [
            r#"{"id":1,"payoutMultiplier":0,"criteria":"basegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[[{"name":"L1"},{"name":"L2"}],[{"name":"H1"},{"name":"S"}]]}]}"#,
            r#"{"id":2,"payoutMultiplier":50,"criteria":"basegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[[{"name":"L1"},{"name":"L1"}],[{"name":"L1"},{"name":"H2"}]]},{"index":1,"type":"winInfo","wins":[{"symbol":"L1","win":0.5}]}]}"#,
            r#"{"id":3,"payoutMultiplier":300,"criteria":"basegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[[{"name":"H1"},{"name":"H1"}],[{"name":"H1"},{"name":"L3"}]]},{"index":1,"type":"winInfo","wins":[{"symbol":"H1","win":3.0}]}]}"#,
            r#"{"id":4,"payoutMultiplier":400,"criteria":"freegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[[{"name":"S"},{"name":"L1"}],[{"name":"S"},{"name":"S"}]]},{"index":1,"type":"freeSpinTrigger","positions":[[0,0],[1,0],[1,1]]},{"index":2,"type":"reveal","gameType":"freegame","board":[[{"name":"H1"}]]},{"index":3,"type":"winInfo","wins":[{"symbol":"H1","win":4.0}]}]}"#,
            r#"{"id":5,"payoutMultiplier":500,"criteria":"freegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[[{"name":"S"},{"name":"L1"}],[{"name":"S"},{"name":"S"}]]},{"index":1,"type":"freeSpinTrigger","positions":[[0,0],[1,0],[1,1]]},{"index":2,"type":"reveal","gameType":"freegame","board":[[{"name":"H1"}]]},{"index":3,"type":"winInfo","wins":[{"symbol":"H1","win":5.0}]}]}"#,
        ];
        std::fs::write(dir.join("books_base.jsonl"), books.join("\n")).unwrap();
    }

    #[test]
    fn reads_a_published_folder() {
        let dir = tmp("jeu").join("mon-jeu").join("publish_files");
        write_game(&dir);
        assert_eq!(game_name(&dir), "mon-jeu");
        let modes = read_index(&dir).unwrap();
        let (inp, notes) = read_mode(&modes[0], &DesignRules::default(), &|_| {}, &AtomicBool::new(false)).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(inp.rows.len(), 5);
        assert_eq!(inp.rows.iter().filter(|r| r.bonus).count(), 2);
        assert!(inp.rows.iter().filter(|r| r.bonus).all(|r| r.scatters == Some(3) && r.group == "freegame"));
        // Plateaux du jeu de base seulement (le plateau du tour gratuit est exclu).
        let board_total: u128 = inp.board.iter().sum();
        assert_eq!(board_total, 4 * (700 + 200 + 50 + 1 + 1));
        // Gains du jeu de base : L1 (poids 200) et H1 (poids 50) ; en bonus : H1 (poids 1 + 1).
        assert_eq!(inp.wins[0][Tier::Low.idx()], (200, 200 * 50));
        assert_eq!(inp.wins[0][Tier::High.idx()], (50, 50 * 300));
        assert_eq!(inp.wins[1][Tier::High.idx()].0, 2);
        // Les deux bonus ont des contenus différents (gains 4 et 5).
        let c: Vec<_> = inp.rows.iter().filter_map(|r| r.content).collect();
        assert_eq!(c.len(), 2);
        assert_ne!(c[0], c[1]);
    }

    #[test]
    fn analyzes_and_stores_report() {
        let dir = tmp("rapport").join("jeu-test").join("library").join("publish_files");
        write_game(&dir);
        let root = tmp("racine");
        let paths = crate::Paths::at(&root);
        paths.ensure().unwrap();
        let db = Db::open(&paths.database).unwrap();
        let (id, rep) = analyze_folder(&db, &AppSettings::default(), &dir, &|_, _| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(rep.game, "jeu-test");
        assert_eq!(rep.modes.len(), 1);
        let m = &rep.modes[0];
        assert_eq!(m.kind, "base");
        assert!(m.checks.iter().any(|c| c.key == "connexions_hauts"));
        let back = get_report(&db, id).unwrap();
        // Relu depuis la base (JSON) : mêmes contrôles, même texte (les flottants peuvent différer au dernier bit).
        assert_eq!(back.modes[0].checks, rep.modes[0].checks);
        assert_eq!((back.passed, back.warnings, back.failed, back.not_run), (rep.passed, rep.warnings, rep.failed, rep.not_run));
        assert_eq!(list_reports(&db).unwrap()[0].id, id);
        assert!(report_markdown(&rep).contains("Contrôles de conception — jeu-test"));
        // Le dossier analysé n'a pas été modifié : toujours les 3 mêmes fichiers.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
    }
}
