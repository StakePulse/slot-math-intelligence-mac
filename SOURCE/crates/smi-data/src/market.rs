//! MARKET INTELLIGENCE côté données : imports, classements, filtres, agrégats, pairs.
//!
//! Sources possibles d'une statistique de mode (colonne `nature`) :
//! - `EXACT` : calculée par NOTRE moteur sur une lookup table (fichier local ou collecte) ;
//! - `IMPORTÉ` : chiffres d'un tiers (ex. instantané slotlab du 2026-11-03), non recalculables ici.
//!
//! Popularité : le classement de référence se fait au **nombre de bets sur 30 jours** ; le turnover
//! est affiché à côté (jamais fusionné). Quand une source ne donne pas les bets (instantané slotlab),
//! le classement retombe sur le turnover et le DIT (`basis`).

use crate::db::Db;
use crate::settings::AppSettings;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use smi_core::dist::Distribution;
use smi_core::market::{aggregate, comparable_max, percentile_rank, Aggregate};
use smi_core::prompt::{MarketContext, MarketMetric, PeerRow, ProviderRow};
use smi_core::stats::{compute, ExactStats};
use smi_core::volatility::{classify, VolInputs};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MarketRow {
    pub slot_id: i64,
    pub mode_id: i64,
    pub slug: String,
    pub name: String,
    pub provider: String,
    pub stake_slug: Option<String>,
    pub ours: bool,
    pub mode: String,
    pub version: String,
    pub kind: String,
    pub cost: f64,
    pub nature: String,
    pub source: String,
    pub collected_at: String,
    pub collection_id: i64,
    pub rtp: Option<f64>,
    pub hit: Option<f64>,
    pub sd_per_price: Option<f64>,
    pub max_mult: Option<f64>,
    pub p_max: Option<f64>,
    pub median: Option<f64>,
    pub q25: Option<f64>,
    pub q75: Option<f64>,
    pub q90: Option<f64>,
    pub share_100x: Option<f64>,
    pub top1: Option<f64>,
    pub p_1000x: Option<f64>,
    pub vol_class: Option<String>,
    pub vol_score: Option<f64>,
    pub bets_30d: Option<f64>,
    pub turnover_30d: Option<f64>,
    pub turnover_share: Option<f64>,
    pub rank_bets: Option<i64>,
    pub rank_turnover: Option<i64>,
    pub players_current: Option<i64>,
    pub has_lut: bool,
    pub figures: serde_json::Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MarketFilter {
    pub providers: Vec<String>,
    pub kinds: Vec<String>,
    pub vol_classes: Vec<String>,
    pub max_min: Option<f64>,
    pub max_max: Option<f64>,
    pub rtp_min: Option<f64>,
    pub rtp_max: Option<f64>,
    /// Garder les N slots les plus populaires (bets 30 j, sinon turnover).
    pub top_n: Option<usize>,
    pub include_ours: bool,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ImportReport {
    pub collection_id: i64,
    pub slots: usize,
    pub modes: usize,
    pub errors: Vec<String>,
    pub message: String,
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for ch in s.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Nature d'un mode d'après son prix et sa part de rounds nuls (règle reprise de l'outil slotlab).
pub fn kind_of(cost: f64, zero_rate: f64) -> &'static str {
    if (cost - 1.0).abs() < 1e-9 {
        "base"
    } else if cost < 10.0 {
        "ante"
    } else if zero_rate >= 0.25 {
        "bust"
    } else {
        "buy"
    }
}

pub fn upsert_provider(db: &Db, name: &str) -> anyhow::Result<i64> {
    let name = name.trim();
    let slug = slugify(name);
    let c = db.conn();
    c.execute("INSERT OR IGNORE INTO providers(slug, name) VALUES (?1, ?2)", params![slug, name])?;
    Ok(c.query_row("SELECT id FROM providers WHERE slug = ?1", params![slug], |r| r.get(0))?)
}

pub fn upsert_slot(db: &Db, slug: &str, name: &str, provider: &str, stake_slug: Option<&str>, ours: bool) -> anyhow::Result<i64> {
    let pid = upsert_provider(db, if provider.trim().is_empty() { "Inconnu" } else { provider })?;
    let now = crate::now();
    let c = db.conn();
    c.execute(
        "INSERT INTO slots(slug, name, provider_id, stake_slug, ours, first_seen, last_seen) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(slug) DO UPDATE SET name = excluded.name, provider_id = excluded.provider_id,
           stake_slug = COALESCE(excluded.stake_slug, slots.stake_slug), ours = MAX(slots.ours, excluded.ours), last_seen = excluded.last_seen",
        params![slug, name.trim(), pid, stake_slug, ours as i64, now],
    )?;
    Ok(c.query_row("SELECT id FROM slots WHERE slug = ?1", params![slug], |r| r.get(0))?)
}

pub fn upsert_mode(db: &Db, slot_id: i64, version: &str, name: &str, cost: f64, kind: &str) -> anyhow::Result<i64> {
    let c = db.conn();
    c.execute(
        "INSERT INTO modes(slot_id, version, name, cost, kind) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(slot_id, version, name) DO UPDATE SET cost = excluded.cost, kind = excluded.kind",
        params![slot_id, version, name, cost, kind],
    )?;
    Ok(c.query_row("SELECT id FROM modes WHERE slot_id = ?1 AND version = ?2 AND name = ?3", params![slot_id, version, name], |r| r.get(0))?)
}

#[allow(clippy::too_many_arguments)]
pub fn insert_popularity(
    db: &Db,
    slot_id: i64,
    collection_id: i64,
    period: &str,
    bets: Option<f64>,
    turnover: Option<f64>,
    share: Option<f64>,
    rank_bets: Option<i64>,
    rank_turnover: Option<i64>,
    players: (Option<i64>, Option<i64>),
    rtp_displayed: Option<f64>,
) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO popularity(slot_id, collection_id, period, bets, turnover, turnover_share, rank_bets, rank_turnover, players_current, players_peak, rtp_displayed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![slot_id, collection_id, period, bets, turnover, share, rank_bets, rank_turnover, players.0, players.1, rtp_displayed],
    )?;
    Ok(())
}

/// Chiffres compacts d'un mode, au même format quelle que soit la source.
pub fn compact_figures(s: &ExactStats) -> serde_json::Value {
    let ladder: serde_json::Map<String, serde_json::Value> = s
        .thresholds
        .iter()
        .map(|t| (format!("p{}", t.mult_price), serde_json::json!(t.prob)))
        .collect();
    serde_json::json!({
        "rtp": s.rtp, "hit": s.hit_rate, "zero": s.zero_rate, "vol": s.sd_per_price, "maxMult": s.max_win.value_price,
        "max": s.max_win.value, "pmax": s.max_win.prob, "median": s.median / s.cost,
        "q25": s.percentiles.iter().find(|p| (p.q - 0.25).abs() < 1e-9).map(|p| p.value_price),
        "q75": s.percentiles.iter().find(|p| (p.q - 0.75).abs() < 1e-9).map(|p| p.value_price),
        "q90": s.percentiles.iter().find(|p| (p.q - 0.90).abs() < 1e-9).map(|p| p.value_price),
        "q99": s.percentiles.iter().find(|p| (p.q - 0.99).abs() < 1e-9).map(|p| p.value_price),
        "ladder": ladder,
        "cvar": s.cvar_001, "etl40": s.etl_40, "skew": s.skewness, "kurt": s.excess_kurtosis,
        "outcomes": s.outcomes, "effective": s.effective_outcomes,
        "bands": s.bands.iter().filter(|b| b.prob > 0.0).map(|b| serde_json::json!({"label": b.label, "lo": b.lo, "prob": b.prob, "rtp": b.rtp, "max": b.is_max, "zero": b.is_zero})).collect::<Vec<_>>(),
        "stake": s.stake_checks.iter().map(|k| serde_json::json!({"scale": k.scale, "item": k.item, "value": k.value, "limit": k.limit, "pass": k.pass})).collect::<Vec<_>>(),
    })
}

/// Enregistre les statistiques EXACTES d'un mode calculées par notre moteur sur sa table.
#[allow(clippy::too_many_arguments)]
pub fn store_exact_mode(
    db: &Db,
    settings: &AppSettings,
    collection_id: i64,
    slot_id: i64,
    version: &str,
    mode: &str,
    d: &Distribution,
    source: &str,
    lut_path: Option<&str>,
) -> anyhow::Result<(i64, ExactStats)> {
    let s = compute(d, &settings.stats);
    let kind = kind_of(d.cost, s.zero_rate);
    let mode_id = upsert_mode(db, slot_id, version, mode, d.cost, kind)?;
    let vi = VolInputs::from_stats(&s);
    let vr = classify(&vi, &settings.volatility);
    let fig = compact_figures(&s);
    db.conn().execute(
        "INSERT OR REPLACE INTO mode_stats(mode_id, collection_id, nature, source, rtp, hit, sd_per_price, max_mult, p_max, median, q25, q75, q90,
            share_100x, top1, p_1000x, vol_class, vol_score, figures, lut_path, lut_fingerprint)
         VALUES (?1, ?2, 'EXACT', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            mode_id,
            collection_id,
            source,
            s.rtp,
            s.hit_rate,
            s.sd_per_price,
            s.max_win.value_price,
            s.max_win.prob,
            s.median / s.cost,
            fig["q25"].as_f64(),
            fig["q75"].as_f64(),
            fig["q90"].as_f64(),
            vi.rtp_share_100x,
            vi.top1_share,
            vi.p_1000x,
            vr.class.label(),
            vr.score,
            fig.to_string(),
            lut_path,
            s.fingerprint
        ],
    )?;
    Ok((mode_id, s))
}

/// Importe une lookup table locale (`id,weight,payout`) : copie compressée dans DATABASE/luts,
/// statistiques EXACTES, slot marqué « notre jeu » si `ours`.
#[allow(clippy::too_many_arguments)]
pub fn import_lut_file(
    db: &Db,
    paths: &crate::Paths,
    settings: &AppSettings,
    file: &Path,
    slot_name: &str,
    provider: &str,
    mode: &str,
    cost: f64,
    ours: bool,
) -> anyhow::Result<ImportReport> {
    let text = std::fs::read_to_string(file)?;
    let (d, info) = Distribution::from_lut_csv(mode, cost, None, &text).map_err(|e| anyhow::anyhow!("{}", e))?;
    let src = db.source_id("fichier", "Lookup tables locales", None, Some("Tables id,weight,payout importées à la main"))?;
    let coll = db.start_collection(src, Some(&crate::today()))?;
    let slug = slugify(&format!("{provider}-{slot_name}"));
    let slot_id = upsert_slot(db, &slug, slot_name, provider, None, ours)?;
    let dir = paths.luts.join(&slug);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}.csv.gz", slugify(mode)));
    {
        use std::io::Write;
        let f = std::fs::File::create(&dest)?;
        let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        gz.write_all(text.as_bytes())?;
        gz.finish()?;
    }
    let source = format!("lookup table locale « {} » ({} lignes, {} poids nuls)", file.display(), info.rows, info.zero_weight_rows);
    store_exact_mode(db, settings, coll, slot_id, "local", mode, &d, &source, Some(&dest.display().to_string()))?;
    db.finish_collection(coll, "OK", 1, 0, &source, None)?;
    db.log("import", &format!("Table {mode} de {slot_name} importée ({} issues distinctes)", d.atoms.len()));
    Ok(ImportReport { collection_id: coll, slots: 1, modes: 1, errors: vec![], message: source })
}

/// Recharge la loi d'un mode depuis sa table compressée (si on l'a).
pub fn load_mode_distribution(db: &Db, mode_id: i64) -> anyhow::Result<Option<Distribution>> {
    let row: Option<(Option<String>, f64, String)> = db
        .conn()
        .query_row(
            "SELECT ms.lut_path, m.cost, m.name FROM mode_stats ms JOIN modes m ON m.id = ms.mode_id WHERE ms.mode_id = ?1 ORDER BY ms.collection_id DESC LIMIT 1",
            params![mode_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((Some(path), cost, name)) = row else { return Ok(None) };
    let f = std::fs::File::open(&path)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(f), &mut text)?;
    let (d, _) = Distribution::from_lut_csv(&name, cost, None, &text).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(Some(d))
}

/// Importe l'instantané tiers de l'outil slotlab (`market.json`) : chiffres IMPORTÉS, datés, sourcés.
pub fn import_slotlab_snapshot(db: &Db, settings: &AppSettings, file: &Path) -> anyhow::Result<ImportReport> {
    let text = std::fs::read_to_string(file)?;
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let fetched = v["fetched"].as_str().unwrap_or("date inconnue").to_string();
    let rule = v["rule"].as_str().unwrap_or("").to_string();
    let name = format!("Instantané slotlab {fetched}");
    let src = db.source_id(
        "instantané tiers",
        &name,
        Some("https://stakecruncher.com"),
        Some(&format!("Fichier market.json de l'outil tiers slotlab ; chiffres calculés par ce tiers sur les lookup tables publiées. Règle : {rule}")),
    )?;
    // Un même instantané ne s'importe qu'une fois.
    let already: Option<i64> = db.conn().query_row("SELECT id FROM collections WHERE source_id = ?1 AND status = 'OK'", params![src], |r| r.get(0)).optional()?;
    if let Some(id) = already {
        return Ok(ImportReport { collection_id: id, slots: 0, modes: 0, errors: vec![], message: format!("{name} déjà importé (collection {id})") });
    }
    let coll = db.start_collection(src, Some(&fetched))?;
    let modes = v["modes"].as_array().cloned().unwrap_or_default();
    let mut errors = Vec::new();
    let mut slots = std::collections::BTreeSet::new();
    let mut n = 0usize;
    let source = format!("slotlab market.json ({fetched}) — IMPORTÉ, non recalculé");
    for m in &modes {
        let f = |k: &str| m[k].as_f64();
        let (Some(slug), Some(name), Some(mode)) = (m["slug"].as_str(), m["name"].as_str(), m["mode"].as_str()) else {
            errors.push("ligne sans slug / nom / mode".into());
            continue;
        };
        let studio = m["studio"].as_str().unwrap_or("Inconnu");
        let res: anyhow::Result<()> = (|| {
            let slot_id = upsert_slot(db, slug, name, studio, m["stake"].as_str(), false)?;
            if slots.insert(slot_id) {
                insert_popularity(db, slot_id, coll, "30d", None, None, f("share"), None, m["rank"].as_i64(), (None, None), None)?;
            }
            let cost = f("cost").unwrap_or(1.0);
            let kind = m["kind"].as_str().unwrap_or_else(|| kind_of(cost, f("zero").unwrap_or(0.0)));
            let mode_id = upsert_mode(db, slot_id, "instantané", mode, cost, kind)?;
            let vi = VolInputs {
                sd_per_price: f("vol").unwrap_or(f64::NAN),
                loss_rate: f("zero").unwrap_or(f64::NAN),
                rtp_share_100x: f("vs100").unwrap_or(f64::NAN),
                top1_share: f("top1").unwrap_or(f64::NAN),
                p_1000x: f("p1000").unwrap_or(0.0),
            };
            let (vc, vs) = if [vi.sd_per_price, vi.loss_rate, vi.rtp_share_100x, vi.top1_share].iter().all(|x| x.is_finite()) {
                let r = classify(&vi, &settings.volatility);
                (Some(r.class.label().to_string()), Some(r.score))
            } else {
                (None, None)
            };
            db.conn().execute(
                "INSERT OR REPLACE INTO mode_stats(mode_id, collection_id, nature, source, rtp, hit, sd_per_price, max_mult, p_max, median, q25, q75, q90,
                    share_100x, top1, p_1000x, vol_class, vol_score, figures)
                 VALUES (?1, ?2, 'IMPORTÉ', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    mode_id,
                    coll,
                    source,
                    f("rtp"),
                    f("hit"),
                    f("vol"),
                    f("maxMult"),
                    f("pmax"),
                    f("median"),
                    f("q25"),
                    f("q75"),
                    f("q90"),
                    f("vs100"),
                    f("top1"),
                    f("p1000"),
                    vc,
                    vs,
                    m.to_string()
                ],
            )?;
            Ok(())
        })();
        match res {
            Ok(()) => n += 1,
            Err(e) => errors.push(format!("{slug}/{mode} : {e}")),
        }
    }
    let msg = format!("{} : {} slots, {} modes importés ({} erreurs). Bets non fournis par cette source : classement au turnover.", name, slots.len(), n, errors.len());
    db.finish_collection(coll, if errors.is_empty() { "OK" } else { "PARTIEL" }, n as i64, errors.len() as i64, &msg, None)?;
    db.log("import", &msg);
    Ok(ImportReport { collection_id: coll, slots: slots.len(), modes: n, errors, message: msg })
}

/// Toutes les lignes de marché (dernière statistique de chaque mode, dernière popularité 30 j du slot).
pub fn market_rows(db: &Db, filter: &MarketFilter) -> anyhow::Result<(Vec<MarketRow>, String)> {
    let c = db.conn();
    let mut st = c.prepare(
        "SELECT s.id, m.id, s.slug, s.name, COALESCE(p.name, 'Inconnu'), s.stake_slug, s.ours, m.name, m.version, m.kind, m.cost,
                ms.nature, ms.source, COALESCE(col.data_date, col.started_at), col.id,
                ms.rtp, ms.hit, ms.sd_per_price, ms.max_mult, ms.p_max, ms.median, ms.q25, ms.q75, ms.q90, ms.share_100x, ms.top1, ms.p_1000x,
                ms.vol_class, ms.vol_score, ms.lut_path, ms.figures,
                pop.bets, pop.turnover, pop.turnover_share, pop.rank_bets, pop.rank_turnover, pop.players_current
         FROM mode_stats ms
         JOIN modes m ON m.id = ms.mode_id
         JOIN slots s ON s.id = m.slot_id
         LEFT JOIN providers p ON p.id = s.provider_id
         JOIN collections col ON col.id = ms.collection_id
         LEFT JOIN popularity pop ON pop.id = (SELECT id FROM popularity WHERE slot_id = s.id AND period = '30d' ORDER BY collection_id DESC LIMIT 1)
         WHERE ms.id = (SELECT id FROM mode_stats WHERE mode_id = m.id ORDER BY collection_id DESC LIMIT 1)",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(MarketRow {
                slot_id: r.get(0)?,
                mode_id: r.get(1)?,
                slug: r.get(2)?,
                name: r.get(3)?,
                provider: r.get::<_, String>(4)?.trim().to_string(),
                stake_slug: r.get(5)?,
                ours: r.get::<_, i64>(6)? != 0,
                mode: r.get(7)?,
                version: r.get(8)?,
                kind: r.get(9)?,
                cost: r.get(10)?,
                nature: r.get(11)?,
                source: r.get(12)?,
                collected_at: r.get(13)?,
                collection_id: r.get(14)?,
                rtp: r.get(15)?,
                hit: r.get(16)?,
                sd_per_price: r.get(17)?,
                max_mult: r.get(18)?,
                p_max: r.get(19)?,
                median: r.get(20)?,
                q25: r.get(21)?,
                q75: r.get(22)?,
                q90: r.get(23)?,
                share_100x: r.get(24)?,
                top1: r.get(25)?,
                p_1000x: r.get(26)?,
                vol_class: r.get(27)?,
                vol_score: r.get(28)?,
                has_lut: r.get::<_, Option<String>>(29)?.is_some(),
                figures: serde_json::from_str(&r.get::<_, String>(30)?).unwrap_or(serde_json::Value::Null),
                bets_30d: r.get(31)?,
                turnover_30d: r.get(32)?,
                turnover_share: r.get(33)?,
                rank_bets: r.get(34)?,
                rank_turnover: r.get(35)?,
                players_current: r.get(36)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(st);
    drop(c);
    Ok(apply_filter(rows, filter))
}

/// Popularité d'un slot pour le tri : (bets connus ?, valeur).
fn popularity_key(r: &MarketRow) -> (i32, f64) {
    match (r.bets_30d, r.turnover_share, r.rank_turnover) {
        (Some(b), _, _) => (2, b),
        (None, Some(s), _) => (1, s),
        (None, None, Some(rank)) => (1, -(rank as f64)),
        _ => (0, 0.0),
    }
}

pub fn apply_filter(mut rows: Vec<MarketRow>, f: &MarketFilter) -> (Vec<MarketRow>, String) {
    let has_bets = rows.iter().any(|r| r.bets_30d.is_some());
    let basis = if has_bets { "nombre de bets sur 30 jours".to_string() } else { "turnover sur 30 jours (la source ne fournit pas le nombre de bets)".to_string() };
    rows.retain(|r| {
        (f.include_ours || !r.ours || f.providers.iter().any(|p| p.eq_ignore_ascii_case(&r.provider)))
            && (f.providers.is_empty() || f.providers.iter().any(|p| p.eq_ignore_ascii_case(&r.provider)))
            && (f.kinds.is_empty() || f.kinds.iter().any(|k| k == &r.kind))
            && (f.vol_classes.is_empty() || r.vol_class.as_ref().is_some_and(|v| f.vol_classes.contains(v)))
            && f.max_min.is_none_or(|m| r.max_mult.is_some_and(|x| x * r.cost >= m))
            && f.max_max.is_none_or(|m| r.max_mult.is_some_and(|x| x * r.cost <= m))
            && f.rtp_min.is_none_or(|m| r.rtp.is_some_and(|x| x >= m))
            && f.rtp_max.is_none_or(|m| r.rtp.is_some_and(|x| x <= m))
            && f.search.as_ref().is_none_or(|q| {
                let q = q.to_lowercase();
                r.name.to_lowercase().contains(&q) || r.provider.to_lowercase().contains(&q)
            })
    });
    rows.sort_by(|a, b| popularity_key(b).partial_cmp(&popularity_key(a)).unwrap_or(std::cmp::Ordering::Equal).then(a.name.cmp(&b.name)).then(a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal)));
    if let Some(n) = f.top_n {
        let mut keep = Vec::new();
        for r in &rows {
            if !keep.contains(&r.slot_id) {
                if keep.len() >= n {
                    break;
                }
                keep.push(r.slot_id);
            }
        }
        rows.retain(|r| keep.contains(&r.slot_id));
    }
    (rows, basis)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MetricAggregate {
    pub key: String,
    pub label: String,
    pub fmt: String,
    pub agg: Aggregate,
}

pub const METRICS: [(&str, &str, &str, bool); 9] = [
    ("rtp", "RTP", "pct", false),
    ("hit", "Hit rate", "pct", false),
    ("sd_per_price", "Écart-type / prix", "num", true),
    ("max_mult", "Max win (× prix)", "x", true),
    ("p_max", "Fréquence du max win", "one_in", true),
    ("share_100x", "Part du RTP ≥ 100× prix", "pct", false),
    ("top1", "Retour porté par le top 1 %", "pct", false),
    ("p_1000x", "P(gain ≥ 1 000× prix)", "one_in", true),
    ("q90", "P90 du gain (× prix)", "x", false),
];

pub fn metric_of(r: &MarketRow, key: &str) -> Option<f64> {
    match key {
        "rtp" => r.rtp,
        "hit" => r.hit,
        "sd_per_price" => r.sd_per_price,
        "max_mult" => r.max_mult,
        "p_max" => r.p_max,
        "share_100x" => r.share_100x,
        "top1" => r.top1,
        "p_1000x" => r.p_1000x.filter(|v| *v > 0.0),
        "q90" => r.q90,
        _ => None,
    }
}

/// Agrégats complets de chaque métrique sur des lignes de marché (un mode = une valeur).
pub fn aggregates(rows: &[MarketRow], min_n: usize) -> Vec<MetricAggregate> {
    METRICS
        .iter()
        .map(|(key, label, fmt, log)| {
            let sel: Vec<&MarketRow> = rows.iter().filter(|r| metric_of(r, key).is_some()).collect();
            let values: Vec<f64> = sel.iter().map(|r| metric_of(r, key).unwrap()).collect();
            let bets: Vec<Option<f64>> = sel.iter().map(|r| r.bets_30d).collect();
            let turnover: Vec<Option<f64>> = sel.iter().map(|r| r.turnover_30d.or(r.turnover_share)).collect();
            MetricAggregate { key: (*key).into(), label: (*label).into(), fmt: (*fmt).into(), agg: aggregate(&values, &bets, &turnover, min_n, 12, *log) }
        })
        .collect()
}

pub fn providers_summary(rows: &[MarketRow]) -> Vec<ProviderRow> {
    let mut by: std::collections::BTreeMap<String, Vec<&MarketRow>> = std::collections::BTreeMap::new();
    for r in rows.iter().filter(|r| r.kind == "base") {
        by.entry(r.provider.clone()).or_default().push(r);
    }
    let med = |v: Vec<f64>| {
        let mut v = v;
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        smi_core::market::quantile_sorted(&v, 0.5)
    };
    let mut out: Vec<ProviderRow> = by
        .into_iter()
        .map(|(studio, rs)| ProviderRow {
            slots: rs.iter().map(|r| r.slot_id).collect::<std::collections::BTreeSet<_>>().len(),
            median_sd: med(rs.iter().filter_map(|r| r.sd_per_price).collect()),
            median_max: med(rs.iter().filter_map(|r| r.max_mult).collect()),
            median_hit: med(rs.iter().filter_map(|r| r.hit).collect()),
            median_rtp: med(rs.iter().filter_map(|r| r.rtp).collect()),
            studio,
        })
        .collect();
    out.sort_by(|a, b| b.slots.cmp(&a.slots).then(a.studio.cmp(&b.studio)));
    out
}

/// Pairs d'un nouveau jeu : modes de même nature, plafond comparable, les plus joués d'abord.
pub fn peers(db: &Db, settings: &AppSettings, max_win: f64, price: f64) -> anyhow::Result<(Vec<MarketRow>, String)> {
    let kind = kind_of(price, 0.0);
    let filter = MarketFilter { kinds: vec![kind.to_string()], ..Default::default() };
    let (rows, basis) = market_rows(db, &filter)?;
    let target = max_win / price;
    let mut sel: Vec<MarketRow> = rows
        .into_iter()
        .filter(|r| !r.ours && r.max_mult.is_some_and(|m| comparable_max(target, m, settings.market.peer_factor)))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    sel.retain(|r| seen.insert(r.slot_id));
    sel.truncate(settings.market.reference_size);
    Ok((sel, basis))
}

/// Contexte de marché pour le prompt : pairs, agrégats, place de notre loi.
pub fn market_context(db: &Db, settings: &AppSettings, ours: &ExactStats, max_win: f64, price: f64) -> anyhow::Result<Option<MarketContext>> {
    let (peers, basis) = peers(db, settings, max_win, price)?;
    if peers.is_empty() {
        return Ok(None);
    }
    let vi = VolInputs::from_stats(ours);
    let our_val = |k: &str| -> Option<f64> {
        match k {
            "rtp" => Some(ours.rtp),
            "hit" => Some(ours.hit_rate),
            "sd_per_price" => Some(ours.sd_per_price),
            "max_mult" => Some(ours.max_win.value_price),
            "p_max" => Some(ours.max_win.prob),
            "share_100x" => Some(vi.rtp_share_100x),
            "top1" => Some(vi.top1_share),
            "p_1000x" => Some(vi.p_1000x),
            "q90" => ours.percentiles.iter().find(|p| (p.q - 0.9).abs() < 1e-9).map(|p| p.value_price),
            _ => None,
        }
    };
    let metrics = aggregates(&peers, settings.market.min_sample)
        .into_iter()
        .map(|m| {
            let mut v: Vec<f64> = peers.iter().filter_map(|r| metric_of(r, &m.key)).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let ours_v = our_val(&m.key);
            MarketMetric { ours_rank_pct: ours_v.and_then(|x| percentile_rank(&v, x)), ours: ours_v, key: m.key, label: m.label, agg: m.agg, fmt: m.fmt }
        })
        .collect();
    let sources: std::collections::BTreeSet<String> = peers.iter().map(|r| format!("{} [{}]", r.source, r.nature)).collect();
    let dates: std::collections::BTreeSet<String> = peers.iter().map(|r| r.collected_at.clone()).collect();
    let mut missing = vec!["fréquence et distribution des bonus des jeux du marché (une lookup table ne dit pas quel round est un bonus)".to_string(), "scatters et anticipation (books non publiés)".to_string()];
    if peers.iter().all(|r| r.bets_30d.is_none()) {
        missing.push("nombre de bets (popularité au turnover)".into());
    }
    Ok(Some(MarketContext {
        source: sources.into_iter().collect::<Vec<_>>().join(" ; "),
        fetched: dates.into_iter().collect::<Vec<_>>().join(", "),
        selection_rule: format!(
            "modes de même nature (prix {}×), plafond entre {}× et {}× le prix, classés par {} ; {} slots",
            price,
            (max_win / price / settings.market.peer_factor).round(),
            (max_win / price * settings.market.peer_factor).round(),
            basis,
            peers.len()
        ),
        providers: providers_summary(&peers),
        peers: peers
            .iter()
            .map(|r| PeerRow {
                name: r.name.clone(),
                studio: r.provider.clone(),
                mode: r.mode.clone(),
                kind: r.kind.clone(),
                max_mult: r.max_mult.unwrap_or(0.0),
                rtp: r.rtp,
                hit: r.hit,
                sd_per_price: r.sd_per_price,
                max_one_in: r.p_max.filter(|p| *p > 0.0).map(|p| 1.0 / p),
                median: r.median,
                q90: r.q90,
                p100: r.figures.get("p100").and_then(|v| v.as_f64()),
                share_100x: r.share_100x,
                bets_30d: r.bets_30d,
                turnover_share: r.turnover_share,
                rank: r.rank_bets.or(r.rank_turnover).map(|v| v as u32),
                vol_class: r.vol_class.clone(),
            })
            .collect(),
        metrics,
        missing,
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RankRow {
    pub slot_id: i64,
    pub name: String,
    pub provider: String,
    pub bets_30d: Option<f64>,
    pub bets_7d: Option<f64>,
    pub bets_90d: Option<f64>,
    pub turnover_30d: Option<f64>,
    pub turnover_share: Option<f64>,
    pub rank_bets: Option<i64>,
    pub rank_turnover: Option<i64>,
    pub prev_rank_bets: Option<i64>,
    pub players_current: Option<i64>,
    pub players_peak: Option<i64>,
    pub collected_at: String,
    pub source: String,
    pub base_vol_class: Option<String>,
    pub base_max: Option<f64>,
}

/// Classement des slots (dernière collecte de chaque slot) avec l'évolution du rang aux bets.
pub fn ranking(db: &Db) -> anyhow::Result<Vec<RankRow>> {
    let c = db.conn();
    let mut st = c.prepare(
        "SELECT s.id, s.name, COALESCE(p.name,'Inconnu'), pop.bets, pop.turnover, pop.turnover_share, pop.rank_bets, pop.rank_turnover,
                pop.players_current, pop.players_peak, COALESCE(col.data_date, col.started_at), src.name, pop.collection_id,
                (SELECT bets FROM popularity WHERE slot_id = s.id AND collection_id = pop.collection_id AND period = '7d'),
                (SELECT bets FROM popularity WHERE slot_id = s.id AND collection_id = pop.collection_id AND period = '90d'),
                (SELECT rank_bets FROM popularity WHERE slot_id = s.id AND period = '30d' AND collection_id < pop.collection_id ORDER BY collection_id DESC LIMIT 1),
                (SELECT ms.vol_class FROM modes m JOIN mode_stats ms ON ms.mode_id = m.id WHERE m.slot_id = s.id AND m.kind = 'base' ORDER BY ms.collection_id DESC LIMIT 1),
                (SELECT ms.max_mult FROM modes m JOIN mode_stats ms ON ms.mode_id = m.id WHERE m.slot_id = s.id AND m.kind = 'base' ORDER BY ms.collection_id DESC LIMIT 1)
         FROM slots s
         LEFT JOIN providers p ON p.id = s.provider_id
         JOIN popularity pop ON pop.id = (SELECT id FROM popularity WHERE slot_id = s.id AND period = '30d' ORDER BY collection_id DESC LIMIT 1)
         JOIN collections col ON col.id = pop.collection_id
         JOIN sources src ON src.id = col.source_id",
    )?;
    let mut rows = st
        .query_map([], |r| {
            Ok(RankRow {
                slot_id: r.get(0)?,
                name: r.get(1)?,
                provider: r.get::<_, String>(2)?.trim().to_string(),
                bets_30d: r.get(3)?,
                turnover_30d: r.get(4)?,
                turnover_share: r.get(5)?,
                rank_bets: r.get(6)?,
                rank_turnover: r.get(7)?,
                players_current: r.get(8)?,
                players_peak: r.get(9)?,
                collected_at: r.get(10)?,
                source: r.get(11)?,
                bets_7d: r.get(13)?,
                bets_90d: r.get(14)?,
                prev_rank_bets: r.get(15)?,
                base_vol_class: r.get(16)?,
                base_max: r.get(17)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by(|a, b| {
        let ka = (a.bets_30d.is_some(), a.bets_30d.unwrap_or(0.0), -(a.rank_turnover.unwrap_or(i64::MAX) as f64));
        let kb = (b.bets_30d.is_some(), b.bets_30d.unwrap_or(0.0), -(b.rank_turnover.unwrap_or(i64::MAX) as f64));
        kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(rows)
}

/// Où ranger une table téléchargée.
pub fn lut_path(paths: &crate::Paths, slug: &str, version: &str, mode: &str) -> PathBuf {
    paths.luts.join(slug).join(format!("v{version}")).join(format!("{}.csv.gz", slugify(mode)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_and_kind() {
        assert_eq!(slugify("Paperclip Gaming — Candy Rush!"), "paperclip-gaming-candy-rush");
        assert_eq!(kind_of(1.0, 0.8), "base");
        assert_eq!(kind_of(1.5, 0.8), "ante");
        assert_eq!(kind_of(100.0, 0.0), "buy");
        assert_eq!(kind_of(200.0, 0.6), "bust");
    }

    #[test]
    fn import_snapshot_and_query() {
        let db = Db::open_in_memory().unwrap();
        let s = AppSettings::default();
        let dir = std::env::temp_dir().join(format!("smi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("market.json");
        std::fs::write(
            &f,
            r#"{"fetched":"2026-11-03","rule":"test","modes":[
              {"rank":1,"slug":"a","name":"A","studio":"S1","share":0.3,"mode":"base","cost":1.0,"kind":"base","rtp":0.965,"hit":0.2,"zero":0.8,"vol":30.0,"maxMult":5000,"pmax":1e-6,"median":0,"q25":0,"q75":0,"q90":0.5,"vs100":0.5,"top1":0.8,"p1000":5e-5},
              {"rank":1,"slug":"a","name":"A","studio":"S1","share":0.3,"mode":"bonus","cost":100.0,"kind":"buy","rtp":0.965,"hit":1.0,"zero":0.0,"vol":3.0,"maxMult":50,"pmax":1e-3,"median":0.5,"q25":0.2,"q75":1.0,"q90":2.0,"vs100":0.0,"top1":0.1,"p1000":0},
              {"rank":7,"slug":"b","name":"B","studio":"S2","share":0.1,"mode":"base","cost":1.0,"kind":"base","rtp":0.96,"hit":0.3,"zero":0.7,"vol":15.0,"maxMult":10000,"pmax":2e-7,"median":0,"q25":0,"q75":0.2,"q90":1.0,"vs100":0.4,"top1":0.7,"p1000":3e-5}
            ]}"#,
        )
        .unwrap();
        let r = import_slotlab_snapshot(&db, &s, &f).unwrap();
        assert_eq!((r.slots, r.modes), (2, 3));
        let again = import_slotlab_snapshot(&db, &s, &f).unwrap();
        assert_eq!(again.modes, 0, "pas de doublon");
        let (rows, basis) = market_rows(&db, &MarketFilter { kinds: vec!["base".into()], ..Default::default() }).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "A", "le plus joué d'abord");
        assert!(basis.contains("turnover"));
        assert!(rows.iter().all(|r| r.nature == "IMPORTÉ" && r.vol_class.is_some()));
        let ag = aggregates(&rows, 10);
        let hit = ag.iter().find(|a| a.key == "hit").unwrap();
        assert_eq!(hit.agg.n, 2);
        assert!(hit.agg.insufficient);
        let rank = ranking(&db).unwrap();
        assert_eq!(rank.len(), 2);
        assert_eq!(rank[0].name, "A");
    }
}
