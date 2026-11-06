//! Exports : prompt pour Claude, rapport de validation, modèle (JSON + lookup table CSV).

use crate::db::Db;
use crate::market::{market_context, slugify};
use crate::models::{latest_model_sim, latest_validation, ModelVersion};
use crate::settings::AppSettings;
use crate::Paths;
use smi_core::dist::ClassKind;
use smi_core::prompt::{build, PromptInput};
use smi_core::stats::{compute, ExactStats};
use smi_core::validate::{CheckStatus, Targets, ValidationReport, STEPS};
use smi_core::volatility::{classify, VolInputs};
use std::fmt::Write;
use std::path::PathBuf;

pub fn targets(mv: &ModelVersion) -> Targets {
    Targets {
        rtp: Some(mv.spec.rtp),
        max_win: Some(mv.spec.max_win),
        max_win_one_in: mv.params.as_ref().map(|p| 1.0 / p.p_max),
        hit_rate: Some(mv.spec.hit_rate),
        bonus_one_in: Some(mv.spec.bonus_one_in),
        bonus_share: Some(mv.spec.bonus_share),
        sd_target: mv.params.as_ref().and_then(|p| p.sd_target),
    }
}

/// Statistiques exactes de la loi, du jeu de base seul et des bonus seuls.
pub fn model_stats(mv: &ModelVersion, s: &AppSettings) -> (ExactStats, Option<ExactStats>, Option<ExactStats>) {
    let st = compute(&mv.distribution, &s.stats);
    let base = mv.distribution.conditional("Jeu de base (rounds gagnants sans bonus)", |c| c.kind == ClassKind::Base).map(|d| compute(&d, &s.stats));
    let bonus = mv.distribution.conditional("Bonus (rounds avec bonus)", |c| c.kind.is_bonus()).map(|d| compute(&d, &s.stats));
    (st, base, bonus)
}

pub fn prompt_for(db: &Db, s: &AppSettings, mv: &ModelVersion) -> anyhow::Result<String> {
    let (st, base, bonus) = model_stats(mv, s);
    let vol = classify(&VolInputs::from_stats(&st), &s.volatility);
    let validation = latest_validation(db, &mv.model_id, mv.version)?;
    let sim = latest_model_sim(db, &mv.model_id, mv.version, &mv.fingerprint)?;
    let market = market_context(db, s, &st, mv.spec.max_win, mv.spec.price)?;
    Ok(build(&PromptInput {
        model_id: &mv.model_id,
        version: mv.version,
        generated_at: &crate::now(),
        spec: &mv.spec,
        params: mv.params.as_ref(),
        notes: &mv.notes,
        stats: &st,
        base: base.as_ref(),
        bonus: bonus.as_ref(),
        vol: Some(&vol),
        validation: validation.as_ref(),
        sim: sim.as_ref(),
        market: market.as_ref(),
    }))
}

pub fn validation_markdown(name: &str, version: u32, r: &ValidationReport) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Rapport de validation — {name} v{version}\n");
    let _ = writeln!(o, "Statut : **{}** · {} réussis · {} avertissements · {} échecs · {} non exécutés · généré le {} · moteur {} · empreinte `{}`\n", r.status.label(), r.passed, r.warnings, r.failed, r.not_run, r.generated_at, r.engine_version, r.fingerprint);
    for (i, step) in STEPS.iter().enumerate() {
        let _ = writeln!(o, "## {}. {}\n", i + 1, step);
        let _ = writeln!(o, "| Contrôle | Statut | Obligatoire | Attendu | Observé | Nature | Détail |\n|---|---|---|---|---|---|---|");
        for c in r.checks.iter().filter(|c| c.step as usize == i + 1) {
            let st = match c.status {
                CheckStatus::Pass => "OK",
                CheckStatus::Warn => "AVERTISSEMENT",
                CheckStatus::Fail => "ÉCHEC",
                CheckStatus::NotRun => "NON EXÉCUTÉ",
            };
            let _ = writeln!(o, "| {} {} | {} | {} | {} | {} | {} | {} |", c.id, c.label, st, if c.mandatory { "oui" } else { "non" }, c.expected, c.observed, c.nature.label(), c.detail.replace('|', "/"));
        }
        let _ = writeln!(o);
    }
    let _ = writeln!(o, "## Tolérances\n\n```json\n{}\n```", serde_json::to_string_pretty(&r.tolerances).unwrap_or_default());
    o
}

/// Exporte la loi d'un modèle au format lookup table (`id,weight,payout`) + un JSON complet.
pub fn export_model(paths: &Paths, s: &AppSettings, mv: &ModelVersion) -> anyhow::Result<(PathBuf, PathBuf)> {
    let dir = paths.exports.join("models");
    std::fs::create_dir_all(&dir)?;
    let base = format!("{}_v{}", slugify(&mv.name), mv.version);
    let lut = dir.join(format!("{base}_lookUpTable.csv"));
    let mut csv = String::with_capacity(mv.distribution.atoms.len() * 24);
    for (i, a) in mv.distribution.atoms.iter().enumerate() {
        let _ = writeln!(csv, "{},{},{}", i + 1, a.weight, a.payout);
    }
    std::fs::write(&lut, csv)?;
    let (st, b, bo) = model_stats(mv, s);
    let json_path = dir.join(format!("{base}.json"));
    let v = serde_json::json!({
        "exported_at": crate::now(),
        "engine": smi_core::ENGINE_VERSION,
        "nature": {"spec": "SPECIFICATION", "stats": "EXACT"},
        "model": mv,
        "stats": st,
        "base_conditional": b,
        "bonus_conditional": bo,
        "lookup_table": lut.display().to_string(),
    });
    std::fs::write(&json_path, serde_json::to_string_pretty(&v)?)?;
    Ok((lut, json_path))
}
