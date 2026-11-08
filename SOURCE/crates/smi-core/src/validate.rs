//! Contrôle qualité indépendant en 9 étapes et statut du modèle.
//!
//! Règle n°15 : aucune validation sans preuve. Un contrôle obligatoire non exécuté laisse le modèle
//! en NEEDS REVIEW ; un contrôle obligatoire en échec le met en VALIDATION FAILED. VALIDATED exige
//! que TOUS les contrôles obligatoires aient été exécutés et réussis (simulation et reproductibilité
//! comprises). READY FOR EXPORT n'est posé que par une action de l'utilisateur sur un modèle VALIDATED.
//!
//! Les tolérances sont dans [`Tolerances`] (réglages de l'application, documentées dans DECISIONS.md D-09).

use crate::dist::Distribution;
use crate::nature::Nature;
use crate::sim::SimResult;
use crate::stats::ExactStats;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tolerances {
    /// Écart absolu toléré sur le RTP (fraction : 1e-6 = 0,0001 point).
    pub rtp_abs: f64,
    /// Écart relatif toléré sur le hit rate visé.
    pub hit_rel: f64,
    /// Écart relatif toléré sur la fréquence des bonus.
    pub bonus_freq_rel: f64,
    /// Écart absolu toléré sur la part du RTP venue des bonus.
    pub bonus_share_abs: f64,
    /// Écart relatif toléré sur la fréquence du max win.
    pub max_prob_rel: f64,
    /// Écart relatif toléré sur l'écart-type visé (avertissement seulement).
    pub sd_rel: f64,
    /// Nombre minimal de spins simulés pour l'étape 7.
    pub sim_min_spins: u64,
    /// |z| maximal entre RTP / hit rate simulés et exacts.
    pub sim_z_max: f64,
    /// p-valeur minimale du khi-deux des bandes.
    pub chi2_p_min: f64,
    /// Écart maximal entre les sommations de la clôture.
    pub closure_abs: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            rtp_abs: 1e-6,
            hit_rel: 0.01,
            bonus_freq_rel: 0.01,
            bonus_share_abs: 0.005,
            max_prob_rel: 0.01,
            sd_rel: 0.15,
            sim_min_spins: 1_000_000,
            sim_z_max: 3.89,
            chi2_p_min: 1e-4,
            closure_abs: 1e-9,
        }
    }
}

/// Objectifs déclarés du modèle (tous facultatifs : un objectif absent n'est pas contrôlé).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Targets {
    pub rtp: Option<f64>,
    pub max_win: Option<f64>,
    pub max_win_one_in: Option<f64>,
    pub hit_rate: Option<f64>,
    pub bonus_one_in: Option<f64>,
    pub bonus_share: Option<f64>,
    pub sd_target: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    NotRun,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelStatus {
    Draft,
    NeedsReview,
    ValidationFailed,
    Validated,
    ReadyForExport,
}

impl ModelStatus {
    pub fn label(self) -> &'static str {
        match self {
            ModelStatus::Draft => "DRAFT",
            ModelStatus::NeedsReview => "NEEDS REVIEW",
            ModelStatus::ValidationFailed => "VALIDATION FAILED",
            ModelStatus::Validated => "VALIDATED",
            ModelStatus::ReadyForExport => "READY FOR EXPORT",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Check {
    pub step: u8,
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub mandatory: bool,
    pub expected: String,
    pub observed: String,
    pub detail: String,
    pub nature: Nature,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Reproducibility {
    pub seed: u64,
    pub spins: u64,
    pub hash_a: String,
    pub hash_b: String,
    pub threads_a: usize,
    pub threads_b: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidationReport {
    pub status: ModelStatus,
    pub checks: Vec<Check>,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub not_run: usize,
    pub fingerprint: String,
    pub engine_version: String,
    pub tolerances: Tolerances,
    pub generated_at: String,
}

pub const STEPS: [&str; 9] = [
    "Probabilités",
    "RTP",
    "Max win",
    "Bonus",
    "Distributions",
    "Fréquences",
    "Simulation",
    "Théorie / simulation",
    "Rapport final",
];

fn pct(v: f64) -> String {
    format!("{:.6} %", v * 100.0)
}

fn one_in(v: Option<f64>) -> String {
    v.map_or("—".into(), |n| format!("1 sur {}", crate::stats::group(n.round() as u64)))
}

pub fn validate(
    d: &Distribution,
    s: &ExactStats,
    t: &Targets,
    sim: Option<&SimResult>,
    repro: Option<&Reproducibility>,
    tol: &Tolerances,
    design: &crate::design::DesignRules,
    generated_at: &str,
) -> ValidationReport {
    let mut c: Vec<Check> = Vec::new();
    let mut add = |step: u8, id: &str, label: &str, status: CheckStatus, mandatory: bool, expected: String, observed: String, detail: String, nature: Nature| {
        c.push(Check { step, id: id.into(), label: label.into(), status, mandatory, expected, observed, detail, nature });
    };
    use CheckStatus::*;
    let ok = |b: bool| if b { Pass } else { Fail };
    let warn = |b: bool| if b { Pass } else { Warn };

    // 1. Probabilités
    let w = d.total_weight();
    add(1, "1.1", "Poids total strictement positif", ok(w > 0), true, "> 0".into(), w.to_string(), "Somme entière des poids de la table.".into(), Nature::Exact);
    let sum_p: f64 = d.atoms.iter().map(|a| a.weight as f64 / w as f64).sum();
    add(1, "1.2", "Somme des probabilités = 100 %", ok((sum_p - 1.0).abs() < 1e-12), true, "1".into(), format!("{sum_p:.15}"), "Σ w_i / W en flottant ; en entiers l'égalité est exacte par construction.".into(), Nature::Exact);
    add(1, "1.3", "Aucune probabilité ni aucun gain négatif", Pass, true, "≥ 0".into(), "≥ 0".into(), "Poids et paiements sont des entiers non signés ; une valeur négative est refusée à la lecture avec son numéro de ligne.".into(), Nature::Exact);
    add(1, "1.4", "Issues tirables", ok(!d.atoms.is_empty()), true, "≥ 1".into(), format!("{} issues ; {:.0} issues effectives ; issue la plus lourde 1 sur {:.0}", d.atoms.len(), s.effective_outcomes, 1.0 / s.heaviest_prob), "Issues effectives = (Σw)² / Σw².".into(), Nature::Exact);

    // 2. RTP
    match t.rtp {
        Some(r) => add(2, "2.1", "RTP conforme à la cible", ok((s.rtp - r).abs() <= tol.rtp_abs), true, pct(r), pct(s.rtp), format!("Tolérance ±{} point(s).", tol.rtp_abs * 100.0), Nature::Exact),
        None => add(2, "2.1", "RTP conforme à la cible", NotRun, false, "—".into(), pct(s.rtp), "Aucune cible déclarée.".into(), Nature::Exact),
    }
    add(
        2,
        "2.2",
        "Clôture : le RTP par issues, classes, bandes, catégories et base + bonus coïncide",
        ok(s.closure.max_abs_diff <= tol.closure_abs),
        true,
        format!("écart ≤ {:e}", tol.closure_abs),
        format!("écart max {:.2e}", s.closure.max_abs_diff),
        "Cinq sommations indépendantes (Witchcraft §16.3).".into(),
        Nature::Exact,
    );

    // 3. Max win
    let max_x = s.max_win.value;
    if let Some(cap) = d.cap {
        let over = d.atoms.iter().filter(|a| a.payout > cap).count();
        add(3, "3.1", "Aucun gain au-dessus du plafond", ok(over == 0), true, format!("≤ {}×", cap as f64 / 100.0), format!("max {}× ; {} issue(s) au-dessus", max_x, over), "Contrôle de chaque issue.".into(), Nature::Exact);
    } else {
        add(3, "3.1", "Aucun gain au-dessus du plafond", NotRun, t.max_win.is_some(), "plafond déclaré".into(), "aucun plafond".into(), "La loi ne déclare pas de plafond.".into(), Nature::Exact);
    }
    match t.max_win {
        Some(m) => add(3, "3.2", "Max win atteignable et égal à la cible", ok((max_x - m).abs() < 1e-9 && s.max_win.prob > 0.0), true, format!("{m}×"), format!("{max_x}× (probabilité {:.3e})", s.max_win.prob), "Le gain maximal doit être tirable (poids > 0).".into(), Nature::Exact),
        None => add(3, "3.2", "Max win atteignable et égal à la cible", NotRun, false, "—".into(), format!("{max_x}×"), "Aucune cible déclarée.".into(), Nature::Exact),
    }
    if let Some(n) = t.max_win_one_in {
        let got = s.max_win.one_in.unwrap_or(f64::INFINITY);
        add(3, "3.3", "Fréquence du max win conforme", ok(((got - n) / n).abs() <= tol.max_prob_rel), true, one_in(Some(n)), one_in(s.max_win.one_in), format!("Tolérance relative ±{} %.", tol.max_prob_rel * 100.0), Nature::Exact);
    }
    let two = s.stake_checks.iter().find(|k| k.scale.starts_with("2★") && k.item.starts_with("Gain maximal"));
    if let Some(k) = two {
        add(3, "3.4", "Barème Stake 2★ : gain maximal ≤ 50 000×", warn(k.pass != Some(false)), false, "≤ 50 000×".into(), format!("{}×", k.value), "Non bloquant chez Stake, mais abaisse la mise maximale (préférence du studio : 2★ et 3★ au vert).".into(), Nature::Exact);
    }

    // 4. Bonus
    let bonus = s.bonus.as_ref();
    let wants_bonus = t.bonus_one_in.is_some() || t.bonus_share.is_some();
    match (bonus, wants_bonus) {
        (None, true) => add(4, "4.1", "Classes de bonus présentes", Fail, true, "classes bonus".into(), "aucune".into(), "Des objectifs de bonus sont déclarés mais la loi ne distingue pas les bonus.".into(), Nature::Exact),
        (None, false) => add(4, "4.1", "Classes de bonus présentes", NotRun, false, "—".into(), "aucune".into(), "Loi sans information de bonus (ex. lookup table publique) : statistiques de bonus non disponibles.".into(), Nature::Exact),
        (Some(b), _) => {
            add(4, "4.1", "Classes de bonus présentes", Pass, wants_bonus, "classes bonus".into(), format!("{} ; espérance conditionnelle {:.2}×", one_in(b.one_in), b.mean.unwrap_or(0.0)), String::new(), Nature::Exact);
            if let Some(n) = t.bonus_one_in {
                let got = b.one_in.unwrap_or(f64::INFINITY);
                add(4, "4.2", "Fréquence des bonus conforme", ok(((got - n) / n).abs() <= tol.bonus_freq_rel), true, one_in(Some(n)), one_in(b.one_in), format!("Tolérance relative ±{} %.", tol.bonus_freq_rel * 100.0), Nature::Exact);
            }
            if let Some(sh) = t.bonus_share {
                add(4, "4.3", "Part du RTP venue des bonus conforme", ok((b.bonus_share - sh).abs() <= tol.bonus_share_abs), true, format!("{:.2} %", sh * 100.0), format!("{:.4} %", b.bonus_share * 100.0), format!("Tolérance ±{} point(s).", tol.bonus_share_abs * 100.0), Nature::Exact);
            }
            let zero_bonus: f64 = s.classes.iter().filter(|k| k.kind.is_bonus()).map(|k| k.prob * k.p_no_win.unwrap_or(0.0)).sum();
            add(4, "4.4", "Un bonus paie toujours quelque chose", warn(zero_bonus == 0.0), false, "P(bonus à 0) = 0".into(), format!("{:.3e}", zero_bonus), "Avertissement de conception (un bonus nul frustre).".into(), Nature::Exact);
            let p10 = b.p10.unwrap_or(0.0) / d.cost;
            add(4, "4.5", "Bonus qui paient (repère studio)", warn(p10 >= 5.0), false, "P10 du bonus ≥ 5× prix".into(), format!("P10 {:.2}× ; médiane {:.2}×", p10, b.median.unwrap_or(0.0) / d.cost), "Repère du studio : mesurer la part des bonus sous 15× (« un bonus doit payer »).".into(), Nature::Exact);
            // Contrôles de conception (D-17) applicables à une loi cible.
            let mut bonus_atoms: Vec<(u64, u64)> = d.atoms.iter().filter(|a| a.weight > 0 && d.classes.get(a.class as usize).is_some_and(|k| k.kind.is_bonus())).map(|a| (a.payout, a.weight)).collect();
            bonus_atoms.sort_unstable();
            if let Some(&(floor, _)) = bonus_atoms.first() {
                let tot: u128 = bonus_atoms.iter().map(|&(_, w)| w as u128).sum();
                let mut acc = 0u128;
                let med = bonus_atoms.iter().find(|&&(_, w)| {
                    acc += w as u128;
                    acc * 2 >= tot
                }).map_or(floor, |&(x, _)| x);
                let near: u128 = bonus_atoms.iter().filter(|&&(x, _)| x as f64 <= floor as f64 * 1.1).map(|&(_, w)| w as u128).sum();
                let near = near as f64 / tot as f64;
                let has_floor = floor > 0 && floor as f64 >= design.floor_detect * med as f64;
                let ratio = if floor > 0 { med as f64 / floor as f64 } else { f64::INFINITY };
                add(
                    4,
                    "4.6",
                    "Plancher du bonus loin de sa médiane (conception)",
                    if !has_floor { NotRun } else { warn(ratio >= design.floor_median_ratio_min && near <= design.floor_near_max) },
                    false,
                    format!("médiane ≥ {:.1} × plancher ; ≤ {:.0} % des bonus à +10 % du plancher", design.floor_median_ratio_min, design.floor_near_max * 100.0),
                    format!("plancher {:.2}×, médiane {:.2}× ({:.2} × le plancher), {:.1} % à +10 %", floor as f64 / 100.0, med as f64 / 100.0, ratio, near * 100.0),
                    if has_floor { "Un hidden ou un bonus à gain minimal ne doit pas faire tomber la moitié des rounds juste au-dessus du plancher.".into() } else { "Pas de plancher voulu (minimum trop bas face à la médiane).".into() },
                    Nature::Exact,
                );
            }
            if d.cost <= 1.0 + 1e-9 {
                add(4, "4.7", "Part du RTP des bonus (conception)", warn(b.bonus_share <= design.bonus_share_base_max), false, format!("≤ {:.0} %", design.bonus_share_base_max * 100.0), format!("{:.2} %", b.bonus_share * 100.0), "Seuil réglable (Settings → design) ; valeur de départ à confirmer.".into(), Nature::Exact);
            }
        }
    }

    // 5. Distributions
    let class_sum: f64 = s.classes.iter().map(|k| k.prob).sum();
    add(5, "5.1", "Les classes forment une partition", ok((class_sum - 1.0).abs() < 1e-12), true, "Σ P(classe) = 1".into(), format!("{class_sum:.15}"), "Chaque issue appartient à exactement une classe.".into(), Nature::Exact);
    let cond_mean: f64 = s.classes.iter().map(|k| k.prob * k.mean.unwrap_or(0.0)).sum();
    add(5, "5.2", "Lois conditionnelles cohérentes", ok((cond_mean - s.mean).abs() <= 1e-9 * s.mean.max(1.0)), true, format!("Σ P(classe)·E[X|classe] = {:.9}", s.mean), format!("{cond_mean:.9}"), "Formule des probabilités totales.".into(), Nature::Exact);
    add(5, "5.3", "Pas de trou sous le gain maximal", warn(s.gaps.is_empty()), false, "aucune bande vide".into(), if s.gaps.is_empty() { "aucune".into() } else { s.gaps.join(", ") }, "Une bande vide entre les gains courants et le max crée un « saut » visible pour les joueurs et sur StakeCruncher.".into(), Nature::Exact);
    add(5, "5.4", "Gains sur la grille de 0,1×", warn(s.off_grid_prob == 0.0), false, "0".into(), format!("{:.3e}", s.off_grid_prob), "Probabilité portée par des gains non multiples de 0,1×.".into(), Nature::Exact);
    let thin: Vec<String> = s.bands.iter().filter(|b| !b.is_zero && !b.is_max && b.outcomes > 0 && b.outcomes < design.min_books_per_band).map(|b| format!("{} : {} issue(s)", b.label, b.outcomes)).collect();
    add(5, "5.5", "Issues distinctes par tranche (conception)", warn(thin.is_empty()), false, format!("≥ {} par tranche", design.min_books_per_band), if thin.is_empty() { "aucune tranche mince".into() } else { thin.join(" ; ") }, "Le jeu final devra avoir au moins autant de books par tranche ; une tranche mince dans la loi cible le sera aussi dans le jeu.".into(), Nature::Exact);

    // 6. Fréquences
    if let Some(h) = t.hit_rate {
        add(6, "6.1", "Hit rate conforme", ok(((s.hit_rate - h) / h).abs() <= tol.hit_rel), true, pct(h), pct(s.hit_rate), format!("Tolérance relative ±{} %.", tol.hit_rel * 100.0), Nature::Exact);
    }
    if let Some(sd) = t.sd_target {
        add(6, "6.2", "Volatilité (σ / prix) proche de la cible", warn(((s.sd_per_price - sd) / sd).abs() <= tol.sd_rel), false, format!("{sd:.2}"), format!("{:.2}", s.sd_per_price), format!("Cible souple : tolérance relative ±{} %.", tol.sd_rel * 100.0), Nature::Exact);
    }
    for (scale, label) in [("2★", "6.3"), ("3★", "6.4")] {
        let fails: Vec<String> = s.stake_checks.iter().filter(|k| k.scale.starts_with(scale) && k.pass == Some(false)).map(|k| format!("{} = {:.4} > {}", k.item, k.value, k.limit.unwrap_or(0.0))).collect();
        add(6, label, &format!("Barèmes Stake {scale}"), warn(fails.is_empty()), false, "tous au vert".into(), if fails.is_empty() { "au vert".into() } else { fails.join(" ; ") }, "Non bloquants chez Stake (abaissent la mise maximale).".into(), Nature::Exact);
    }

    // 7. Simulation
    match sim {
        None => {
            add(7, "7.1", "Simulation exécutée", NotRun, true, format!("≥ {} spins", crate::stats::group(tol.sim_min_spins)), "aucune".into(), "Lancer une simulation (Spin Simulator).".into(), Nature::MonteCarlo);
        }
        Some(r) => {
            add(7, "7.1", "Simulation exécutée et complète", ok(r.completed && r.spins_done >= tol.sim_min_spins), true, format!("≥ {} spins, complète", crate::stats::group(tol.sim_min_spins)), format!("{} spins, graine {}, {}", crate::stats::group(r.spins_done), r.config.seed, if r.completed { "complète" } else { "interrompue" }), String::new(), Nature::MonteCarlo);
            add(7, "7.2", "Simulation faite sur CETTE version du modèle", ok(r.fingerprint == s.fingerprint), true, s.fingerprint.clone(), r.fingerprint.clone(), "Empreinte de la loi simulée.".into(), Nature::Exact);
        }
    }
    match repro {
        None => add(7, "7.3", "Reproductibilité vérifiée", NotRun, true, "mêmes compteurs".into(), "non vérifiée".into(), "Deux exécutions de même graine avec des nombres de threads différents.".into(), Nature::MonteCarlo),
        Some(rp) => add(7, "7.3", "Reproductibilité vérifiée", ok(rp.hash_a == rp.hash_b), true, rp.hash_a.clone(), rp.hash_b.clone(), format!("Graine {}, {} spins, {} puis {} threads.", rp.seed, crate::stats::group(rp.spins), rp.threads_a, rp.threads_b), Nature::MonteCarlo),
    }

    // 8. Théorie / simulation
    match sim {
        None => add(8, "8.1", "RTP simulé compatible avec le RTP exact", NotRun, true, "|z| ≤ seuil".into(), "—".into(), "Pas de simulation.".into(), Nature::MonteCarlo),
        Some(r) => {
            let k = &r.comparison;
            add(8, "8.1", "RTP simulé compatible avec le RTP exact", ok(k.rtp_z.abs() <= tol.sim_z_max), true, format!("|z| ≤ {}", tol.sim_z_max), format!("{} vs {} ; z = {:.2}", pct(k.rtp_sim), pct(k.rtp_theory), k.rtp_z), "z = (RTP simulé − RTP exact) / (σ/prix/√N), σ exact.".into(), Nature::MonteCarlo);
            add(8, "8.2", "Hit rate simulé compatible", ok(k.hit_z.abs() <= tol.sim_z_max), true, format!("|z| ≤ {}", tol.sim_z_max), format!("{} vs {} ; z = {:.2}", pct(k.hit_sim), pct(k.hit_theory), k.hit_z), String::new(), Nature::MonteCarlo);
            match &k.chi2 {
                Some(ch) => add(8, "8.3", "Bandes de gain : khi-deux", ok(ch.p_value >= tol.chi2_p_min), true, format!("p ≥ {:e}", tol.chi2_p_min), format!("χ² = {:.1}, ddl {}, p = {:.4}", ch.stat, ch.df, ch.p_value), "Bandes fusionnées jusqu'à 5 attendus par case.".into(), Nature::MonteCarlo),
                None => add(8, "8.3", "Bandes de gain : khi-deux", NotRun, false, "—".into(), "trop peu de bandes".into(), String::new(), Nature::MonteCarlo),
            }
            let bad: Vec<String> = k.thresholds.iter().filter(|t| t.p_value < 1e-5).map(|t| format!("≥ {}× : {} vus / {:.1} attendus", t.mult_price, t.observed, t.expected)).collect();
            add(8, "8.4", "Seuils de gain (≥ k × prix)", warn(bad.is_empty()), false, "p ≥ 1e-5 pour chaque seuil".into(), if bad.is_empty() { "conformes".into() } else { bad.join(" ; ") }, "Tests multiples : avertissement seulement.".into(), Nature::MonteCarlo);
            let mw = &k.max_win;
            add(8, "8.5", "Max win : événement rare", Pass, false, format!("{:.2} attendu(s)", mw.expected), format!("{} observé(s)", mw.observed), format!("P(aucun en {} spins) = {:.1} %. La probabilité exacte fait foi ; une absence en simulation ne prouve rien.", crate::stats::group(r.spins_done), mw.p_none * 100.0), Nature::ExactFormula);
            add(8, "8.6", "Aucune anomalie de simulation", ok(r.anomalies.is_empty()), true, "aucune".into(), if r.anomalies.is_empty() { "aucune".into() } else { r.anomalies.join(" ; ") }, String::new(), Nature::MonteCarlo);
        }
    }

    // 9. Rapport final
    add(9, "9.1", "Rapport final généré", Pass, false, "—".into(), generated_at.into(), format!("Moteur smi-core {}.", crate::ENGINE_VERSION), Nature::Specification);

    let failed = c.iter().filter(|k| k.mandatory && k.status == Fail).count();
    let not_run = c.iter().filter(|k| k.mandatory && k.status == NotRun).count();
    let status = if failed > 0 {
        ModelStatus::ValidationFailed
    } else if not_run > 0 {
        ModelStatus::NeedsReview
    } else {
        ModelStatus::Validated
    };
    ValidationReport {
        status,
        passed: c.iter().filter(|k| k.status == Pass).count(),
        warnings: c.iter().filter(|k| k.status == Warn || (!k.mandatory && k.status == Fail)).count(),
        failed: c.iter().filter(|k| k.status == Fail).count(),
        not_run: c.iter().filter(|k| k.status == NotRun).count(),
        checks: c,
        fingerprint: s.fingerprint.clone(),
        engine_version: crate::ENGINE_VERSION.into(),
        tolerances: tol.clone(),
        generated_at: generated_at.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{generate, GeneratorSpec};
    use crate::sim::{run, Control, SimConfig};
    use crate::stats::{compute, StatsOptions};
    use crate::volatility::VolatilityConfig;

    fn targets(spec: &GeneratorSpec, one_in: f64) -> Targets {
        Targets {
            rtp: Some(spec.rtp),
            max_win: Some(spec.max_win),
            max_win_one_in: Some(one_in),
            hit_rate: Some(spec.hit_rate),
            bonus_one_in: Some(spec.bonus_one_in),
            bonus_share: Some(spec.bonus_share),
            sd_target: None,
        }
    }

    #[test]
    fn never_validated_without_simulation() {
        let spec = GeneratorSpec { max_win_one_in: Some(400_000.0), ..Default::default() };
        let m = generate(&spec, &VolatilityConfig::default()).unwrap();
        let s = compute(&m.distribution, &StatsOptions::default());
        let r = validate(&m.distribution, &s, &targets(&spec, 400_000.0), None, None, &Tolerances::default(), &crate::design::DesignRules::default(), "test");
        assert_eq!(r.status, ModelStatus::NeedsReview);
        assert_eq!(r.failed, 0, "{:#?}", r.checks.iter().filter(|c| c.status == CheckStatus::Fail).collect::<Vec<_>>());
    }

    #[test]
    fn full_pipeline_validates() {
        let spec = GeneratorSpec { max_win_one_in: Some(400_000.0), ..Default::default() };
        let m = generate(&spec, &VolatilityConfig::default()).unwrap();
        let opt = StatsOptions::default();
        let s = compute(&m.distribution, &opt);
        let cfg = SimConfig { spins: 1_000_000, seed: 11, threads: 4, chunk_size: 1 << 18 };
        let sim = run(&m.distribution, &cfg, &Control::default(), &opt, &|_, _| {});
        let rp = crate::sim::reproducibility(&m.distribution, 11, 200_000);
        let r = validate(&m.distribution, &s, &targets(&spec, 400_000.0), Some(&sim), Some(&rp), &Tolerances::default(), &crate::design::DesignRules::default(), "test");
        assert_eq!(r.status, ModelStatus::Validated, "{:#?}", r.checks.iter().filter(|c| c.status != CheckStatus::Pass).collect::<Vec<_>>());
    }

    #[test]
    fn wrong_target_fails() {
        let spec = GeneratorSpec::default();
        let m = generate(&spec, &VolatilityConfig::default()).unwrap();
        let s = compute(&m.distribution, &StatsOptions::default());
        let mut t = targets(&spec, 1.0 / m.params.p_max);
        t.rtp = Some(0.97);
        let r = validate(&m.distribution, &s, &t, None, None, &Tolerances::default(), &crate::design::DesignRules::default(), "test");
        assert_eq!(r.status, ModelStatus::ValidationFailed);
    }
}
