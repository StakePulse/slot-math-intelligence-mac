//! MATH LIBRARY : modèles versionnés, simulations, validations, prompts.
//!
//! Chaque modèle a un identifiant unique (UUID v4), une date de création, des versions immuables
//! (spécification, paramètres, loi complète, empreinte), un historique (note de changement par
//! version), un statut de validation et ses sources. On compare deux versions champ par champ.

use crate::db::Db;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use smi_core::dist::Distribution;
use smi_core::generator::{GenParams, GeneratedModel, GeneratorSpec};
use smi_core::sim::SimResult;
use smi_core::stats::ExactStats;
use smi_core::validate::{ModelStatus, ValidationReport};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub current_version: u32,
    pub status: ModelStatus,
    pub sources: Vec<String>,
    pub versions: i64,
    pub simulations: i64,
    pub max_win: Option<f64>,
    pub rtp: Option<f64>,
    pub volatility: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelVersion {
    pub model_id: String,
    pub name: String,
    pub version: u32,
    pub created_at: String,
    pub spec: GeneratorSpec,
    pub params: Option<GenParams>,
    pub notes: Vec<String>,
    pub distribution: Distribution,
    pub fingerprint: String,
    pub change_note: String,
    pub status: ModelStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VersionInfo {
    pub version: u32,
    pub created_at: String,
    pub change_note: String,
    pub status: ModelStatus,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SimSummary {
    pub id: i64,
    pub created_at: String,
    pub target_kind: String,
    pub model_id: Option<String>,
    pub version: Option<u32>,
    pub label: String,
    pub spins: i64,
    pub rtp: Option<f64>,
    pub completed: bool,
    pub elapsed_ms: i64,
    pub fingerprint: Option<String>,
}

fn status_str(s: ModelStatus) -> String {
    serde_json::to_value(s).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "DRAFT".into())
}

fn parse_status(s: &str) -> ModelStatus {
    serde_json::from_value(serde_json::Value::String(s.into())).unwrap_or(ModelStatus::Draft)
}

pub fn create_model(db: &Db, g: &GeneratedModel, sources: &[String], note: &str) -> anyhow::Result<(String, u32)> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::now();
    db.conn().execute(
        "INSERT INTO models(id, name, created_at, updated_at, current_version, status, sources) VALUES (?1, ?2, ?3, ?3, 0, 'DRAFT', ?4)",
        params![id, g.spec.name, now, serde_json::to_string(sources)?],
    )?;
    let v = add_version(db, &id, g, if note.is_empty() { "Création" } else { note })?;
    db.log("modèle", &format!("Modèle « {} » créé ({id})", g.spec.name));
    Ok((id, v))
}

pub fn add_version(db: &Db, model_id: &str, g: &GeneratedModel, change_note: &str) -> anyhow::Result<u32> {
    let c = db.conn();
    let current: u32 = c.query_row("SELECT current_version FROM models WHERE id = ?1", params![model_id], |r| r.get(0))?;
    let v = current + 1;
    let now = crate::now();
    c.execute(
        "INSERT INTO model_versions(model_id, version, created_at, spec, params, notes, distribution, fingerprint, change_note, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'DRAFT')",
        params![
            model_id,
            v,
            now,
            serde_json::to_string(&g.spec)?,
            serde_json::to_string(&g.params)?,
            serde_json::to_string(&g.notes)?,
            serde_json::to_string(&g.distribution)?,
            g.distribution.fingerprint(),
            change_note
        ],
    )?;
    c.execute("UPDATE models SET current_version = ?2, updated_at = ?3, status = 'DRAFT', name = ?4 WHERE id = ?1", params![model_id, v, now, g.spec.name])?;
    Ok(v)
}

pub fn list_models(db: &Db) -> anyhow::Result<Vec<ModelSummary>> {
    let c = db.conn();
    let mut st = c.prepare(
        "SELECT m.id, m.name, m.created_at, m.updated_at, m.current_version, m.status, m.sources,
                (SELECT COUNT(*) FROM model_versions WHERE model_id = m.id),
                (SELECT COUNT(*) FROM simulations WHERE model_id = m.id),
                (SELECT spec FROM model_versions WHERE model_id = m.id AND version = m.current_version)
         FROM models m ORDER BY m.updated_at DESC",
    )?;
    let rows = st
        .query_map([], |r| {
            let spec: Option<GeneratorSpec> = r.get::<_, Option<String>>(9)?.and_then(|s| serde_json::from_str(&s).ok());
            Ok(ModelSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                current_version: r.get(4)?,
                status: parse_status(&r.get::<_, String>(5)?),
                sources: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                versions: r.get(7)?,
                simulations: r.get(8)?,
                max_win: spec.as_ref().map(|s| s.max_win),
                rtp: spec.as_ref().map(|s| s.rtp),
                volatility: spec.map(|s| s.volatility),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_version(db: &Db, model_id: &str, version: Option<u32>) -> anyhow::Result<ModelVersion> {
    let c = db.conn();
    let v: u32 = match version {
        Some(v) => v,
        None => c.query_row("SELECT current_version FROM models WHERE id = ?1", params![model_id], |r| r.get(0))?,
    };
    let row = c.query_row(
        "SELECT mv.created_at, mv.spec, mv.params, mv.notes, mv.distribution, mv.fingerprint, mv.change_note, mv.status, m.name
         FROM model_versions mv JOIN models m ON m.id = mv.model_id WHERE mv.model_id = ?1 AND mv.version = ?2",
        params![model_id, v],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ))
        },
    )?;
    Ok(ModelVersion {
        model_id: model_id.into(),
        name: row.8,
        version: v,
        created_at: row.0,
        spec: serde_json::from_str(&row.1)?,
        params: row.2.and_then(|s| serde_json::from_str(&s).ok()),
        notes: serde_json::from_str(&row.3).unwrap_or_default(),
        distribution: serde_json::from_str(&row.4)?,
        fingerprint: row.5,
        change_note: row.6,
        status: parse_status(&row.7),
    })
}

pub fn versions(db: &Db, model_id: &str) -> anyhow::Result<Vec<VersionInfo>> {
    let c = db.conn();
    let mut st = c.prepare("SELECT version, created_at, change_note, status, fingerprint FROM model_versions WHERE model_id = ?1 ORDER BY version DESC")?;
    let rows = st
        .query_map(params![model_id], |r| {
            Ok(VersionInfo { version: r.get(0)?, created_at: r.get(1)?, change_note: r.get(2)?, status: parse_status(&r.get::<_, String>(3)?), fingerprint: r.get(4)? })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Statut d'une version. READY FOR EXPORT n'est accepté que sur une version VALIDATED.
pub fn set_status(db: &Db, model_id: &str, version: u32, status: ModelStatus) -> anyhow::Result<()> {
    let c = db.conn();
    if status == ModelStatus::ReadyForExport {
        let cur: String = c.query_row("SELECT status FROM model_versions WHERE model_id = ?1 AND version = ?2", params![model_id, version], |r| r.get(0))?;
        if parse_status(&cur) != ModelStatus::Validated && parse_status(&cur) != ModelStatus::ReadyForExport {
            anyhow::bail!("READY FOR EXPORT exige une version VALIDATED (statut actuel : {cur})");
        }
    }
    let s = status_str(status);
    c.execute("UPDATE model_versions SET status = ?3 WHERE model_id = ?1 AND version = ?2", params![model_id, version, s])?;
    c.execute("UPDATE models SET status = ?3, updated_at = ?4 WHERE id = ?1 AND current_version = ?2", params![model_id, version, s, crate::now()])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn save_simulation(
    db: &Db,
    target_kind: &str,
    model_id: Option<&str>,
    version: Option<u32>,
    label: &str,
    config: &serde_json::Value,
    result: &serde_json::Value,
    spins: u64,
    rtp: Option<f64>,
    completed: bool,
    elapsed_ms: u64,
    fingerprint: Option<&str>,
) -> anyhow::Result<i64> {
    let c = db.conn();
    c.execute(
        "INSERT INTO simulations(created_at, target_kind, model_id, version, label, config, result, spins, rtp, completed, elapsed_ms, fingerprint)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![crate::now(), target_kind, model_id, version, label, config.to_string(), result.to_string(), spins as i64, rtp, completed as i64, elapsed_ms as i64, fingerprint],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn list_simulations(db: &Db, model_id: Option<&str>, limit: usize) -> anyhow::Result<Vec<SimSummary>> {
    let c = db.conn();
    let mut st = c.prepare(
        "SELECT id, created_at, target_kind, model_id, version, label, spins, rtp, completed, elapsed_ms, fingerprint FROM simulations
         WHERE (?1 IS NULL OR model_id = ?1) ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = st
        .query_map(params![model_id, limit as i64], |r| {
            Ok(SimSummary {
                id: r.get(0)?,
                created_at: r.get(1)?,
                target_kind: r.get(2)?,
                model_id: r.get(3)?,
                version: r.get(4)?,
                label: r.get(5)?,
                spins: r.get(6)?,
                rtp: r.get(7)?,
                completed: r.get::<_, i64>(8)? != 0,
                elapsed_ms: r.get(9)?,
                fingerprint: r.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_simulation(db: &Db, id: i64) -> anyhow::Result<(serde_json::Value, serde_json::Value, String)> {
    let (c, r, l): (String, String, String) = db.conn().query_row("SELECT config, result, label FROM simulations WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok((serde_json::from_str(&c)?, serde_json::from_str(&r)?, l))
}

/// Dernière simulation COMPLÈTE de cette version exacte du modèle (même empreinte de loi).
pub fn latest_model_sim(db: &Db, model_id: &str, version: u32, fingerprint: &str) -> anyhow::Result<Option<SimResult>> {
    let r: Option<String> = db
        .conn()
        .query_row(
            "SELECT result FROM simulations WHERE model_id = ?1 AND version = ?2 AND fingerprint = ?3 AND completed = 1 AND target_kind = 'model' ORDER BY spins DESC, id DESC LIMIT 1",
            params![model_id, version, fingerprint],
            |r| r.get(0),
        )
        .optional()?;
    Ok(r.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn save_validation(db: &Db, model_id: &str, version: u32, report: &ValidationReport) -> anyhow::Result<()> {
    db.conn().execute(
        "INSERT INTO validations(model_id, version, created_at, status, report) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![model_id, version, crate::now(), status_str(report.status), serde_json::to_string(report)?],
    )?;
    set_status(db, model_id, version, report.status)?;
    Ok(())
}

pub fn latest_validation(db: &Db, model_id: &str, version: u32) -> anyhow::Result<Option<ValidationReport>> {
    let r: Option<String> = db
        .conn()
        .query_row("SELECT report FROM validations WHERE model_id = ?1 AND version = ?2 ORDER BY id DESC LIMIT 1", params![model_id, version], |r| r.get(0))
        .optional()?;
    Ok(r.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn save_prompt(db: &Db, model_id: &str, version: u32, markdown: &str, path: Option<&str>) -> anyhow::Result<i64> {
    let c = db.conn();
    c.execute("INSERT INTO prompts(model_id, version, created_at, markdown, path) VALUES (?1, ?2, ?3, ?4, ?5)", params![model_id, version, crate::now(), markdown, path])?;
    Ok(c.last_insert_rowid())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiffRow {
    pub field: String,
    pub a: String,
    pub b: String,
    pub changed: bool,
}

/// Comparaison de deux versions : objectifs (SPECIFICATION) puis résultats exacts.
pub fn diff(a: &ModelVersion, b: &ModelVersion, sa: &ExactStats, sb: &ExactStats) -> Vec<DiffRow> {
    let mut out = Vec::new();
    let mut row = |field: &str, x: String, y: String| {
        let changed = x != y;
        out.push(DiffRow { field: field.into(), a: x, b: y, changed });
    };
    let p = |v: f64| format!("{:.4} %", v * 100.0);
    let n = |v: f64| format!("{v:.4}");
    let oi = |v: Option<f64>| v.map_or("—".into(), |x| format!("1 sur {}", smi_core::stats::group(x.round() as u64)));
    row("Nom", a.spec.name.clone(), b.spec.name.clone());
    row("Mécaniques", a.spec.mechanics.join(", "), b.spec.mechanics.join(", "));
    row("RTP cible", p(a.spec.rtp), p(b.spec.rtp));
    row("Max win cible", n(a.spec.max_win), n(b.spec.max_win));
    row("Hit rate cible", p(a.spec.hit_rate), p(b.spec.hit_rate));
    row("Bonus 1 sur", n(a.spec.bonus_one_in), n(b.spec.bonus_one_in));
    row("Part bonus cible", p(a.spec.bonus_share), p(b.spec.bonus_share));
    row("Volatilité demandée", a.spec.volatility.clone(), b.spec.volatility.clone());
    row("RTP exact", p(sa.rtp), p(sb.rtp));
    row("Hit rate exact", p(sa.hit_rate), p(sb.hit_rate));
    row("σ / prix", n(sa.sd_per_price), n(sb.sd_per_price));
    row("Médiane", n(sa.median), n(sb.median));
    row("Max win", oi(sa.max_win.one_in), oi(sb.max_win.one_in));
    row("Bonus", oi(sa.bonus.as_ref().and_then(|x| x.one_in)), oi(sb.bonus.as_ref().and_then(|x| x.one_in)));
    row("Médiane des bonus", sa.bonus.as_ref().and_then(|x| x.median).map_or("—".into(), n), sb.bonus.as_ref().and_then(|x| x.median).map_or("—".into(), n));
    row("Issues", sa.outcomes.to_string(), sb.outcomes.to_string());
    row("Empreinte", a.fingerprint.clone(), b.fingerprint.clone());
    out
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Dashboard {
    pub slots: i64,
    pub providers: i64,
    pub modes: i64,
    pub models: i64,
    pub simulations: i64,
    pub documents: i64,
    pub facts_to_validate: i64,
    pub last_collection: Option<(String, String, String, String)>,
    pub activity: Vec<(String, String, String)>,
    pub alerts: Vec<(i64, String, String, String, String)>,
    pub recent_models: Vec<ModelSummary>,
    pub schema_version: i64,
}

pub fn dashboard(db: &Db) -> anyhow::Result<Dashboard> {
    let count = |sql: &str| -> i64 { db.conn().query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    let last_collection = db
        .conn()
        .query_row(
            "SELECT COALESCE(c.finished_at, c.started_at), s.name, c.status, COALESCE(c.message, '') FROM collections c JOIN sources s ON s.id = c.source_id ORDER BY c.id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let activity = {
        let c = db.conn();
        let mut st = c.prepare("SELECT at, kind, message FROM activity ORDER BY id DESC LIMIT 15")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        v
    };
    let alerts = {
        let c = db.conn();
        let mut st = c.prepare("SELECT id, created_at, level, scope, message FROM alerts WHERE resolved = 0 ORDER BY id DESC LIMIT 30")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<Vec<_>, _>>()?;
        v
    };
    let mut recent_models = list_models(db)?;
    recent_models.truncate(6);
    Ok(Dashboard {
        slots: count("SELECT COUNT(*) FROM slots"),
        providers: count("SELECT COUNT(*) FROM providers"),
        modes: count("SELECT COUNT(*) FROM modes"),
        models: count("SELECT COUNT(*) FROM models"),
        simulations: count("SELECT COUNT(*) FROM simulations"),
        documents: count("SELECT COUNT(*) FROM documents"),
        facts_to_validate: count("SELECT COUNT(*) FROM extracted_facts WHERE status = 'À VALIDER'"),
        last_collection,
        activity,
        alerts,
        recent_models,
        schema_version: db.schema_version(),
    })
}

pub fn resolve_alert(db: &Db, id: i64) -> anyhow::Result<()> {
    db.conn().execute("UPDATE alerts SET resolved = 1 WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use smi_core::generator::generate;
    use smi_core::volatility::VolatilityConfig;

    #[test]
    fn versions_and_status_rules() {
        let db = Db::open_in_memory().unwrap();
        let g = generate(&GeneratorSpec::default(), &VolatilityConfig::default()).unwrap();
        let (id, v1) = create_model(&db, &g, &["test".into()], "").unwrap();
        assert_eq!(v1, 1);
        let mut spec2 = g.spec.clone();
        spec2.hit_rate = 0.3;
        let g2 = generate(&spec2, &VolatilityConfig::default()).unwrap();
        let v2 = add_version(&db, &id, &g2, "hit 30 %").unwrap();
        assert_eq!(v2, 2);
        assert_eq!(versions(&db, &id).unwrap().len(), 2);
        let a = get_version(&db, &id, Some(1)).unwrap();
        assert_eq!(a.distribution, g.distribution);
        assert!(set_status(&db, &id, 2, ModelStatus::ReadyForExport).is_err(), "pas d'export sans validation");
        let models = list_models(&db).unwrap();
        assert_eq!(models[0].current_version, 2);
        assert_eq!(models[0].status, ModelStatus::Draft);
    }
}
