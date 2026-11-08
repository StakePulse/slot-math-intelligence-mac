//! PROMPT GENERATOR — le livrable final : un cahier des charges mathématique complet pour Claude.
//!
//! Le prompt ne contient que des chiffres produits par ce moteur (loi exacte, simulation à graine,
//! références de marché datées et sourcées), chacun avec sa nature. Il demande un modèle VÉRIFIABLE
//! (books + lookup tables, audit indépendant, clôture des RTP), jamais des nombres arbitraires.

use crate::generator::{GenParams, GeneratorSpec};
use crate::market::Aggregate;
use crate::sim::SimResult;
use crate::stats::{group, ExactStats};
use crate::validate::{CheckStatus, ValidationReport};
use crate::volatility::VolResult;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PeerRow {
    pub name: String,
    pub studio: String,
    pub mode: String,
    pub kind: String,
    pub max_mult: f64,
    pub rtp: Option<f64>,
    pub hit: Option<f64>,
    pub sd_per_price: Option<f64>,
    pub max_one_in: Option<f64>,
    pub median: Option<f64>,
    pub q90: Option<f64>,
    pub p100: Option<f64>,
    pub share_100x: Option<f64>,
    pub bets_30d: Option<f64>,
    pub turnover_share: Option<f64>,
    pub rank: Option<u32>,
    pub vol_class: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MarketMetric {
    pub key: String,
    pub label: String,
    pub agg: Aggregate,
    pub ours: Option<f64>,
    pub ours_rank_pct: Option<f64>,
    /// Format d'affichage : "pct", "x", "one_in", "num".
    pub fmt: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProviderRow {
    pub studio: String,
    pub slots: usize,
    pub median_sd: Option<f64>,
    pub median_max: Option<f64>,
    pub median_hit: Option<f64>,
    pub median_rtp: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MarketContext {
    pub source: String,
    pub fetched: String,
    pub selection_rule: String,
    pub peers: Vec<PeerRow>,
    pub metrics: Vec<MarketMetric>,
    pub providers: Vec<ProviderRow>,
    /// Données non disponibles dans la source (affichées telles quelles).
    pub missing: Vec<String>,
}

pub struct PromptInput<'a> {
    pub model_id: &'a str,
    pub version: u32,
    pub generated_at: &'a str,
    pub spec: &'a GeneratorSpec,
    pub params: Option<&'a GenParams>,
    pub notes: &'a [String],
    pub stats: &'a ExactStats,
    pub base: Option<&'a ExactStats>,
    pub bonus: Option<&'a ExactStats>,
    pub vol: Option<&'a VolResult>,
    pub validation: Option<&'a ValidationReport>,
    pub sim: Option<&'a SimResult>,
    pub market: Option<&'a MarketContext>,
    /// Seuils des contrôles de conception du jeu terminé (D-17) ; `None` = valeurs par défaut.
    pub design: Option<&'a crate::design::DesignRules>,
}

pub fn fr(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let s = format!("{:.*}", digits, v.abs());
    let (int, dec) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
    let int = group(int.parse::<u64>().unwrap_or(0));
    let sign = if v < 0.0 { "−" } else { "" };
    if dec.is_empty() {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int},{dec}")
    }
}

pub fn pct(v: f64, digits: usize) -> String {
    format!("{} %", fr(v * 100.0, digits))
}

pub fn one_in(v: Option<f64>) -> String {
    match v {
        Some(n) if n.is_finite() => {
            if n < 100.0 {
                format!("1 sur {}", fr(n, 2))
            } else {
                format!("1 sur {}", group(n.round() as u64))
            }
        }
        _ => "jamais".into(),
    }
}

pub fn xm(v: f64) -> String {
    if v >= 100.0 {
        format!("{}×", fr(v, 0))
    } else if v >= 10.0 {
        format!("{}×", fr(v, 1))
    } else {
        format!("{}×", fr(v, 2))
    }
}

fn opt<T>(v: Option<T>, f: impl Fn(T) -> String) -> String {
    v.map_or("non disponible".into(), f)
}

fn bands_table(out: &mut String, s: &ExactStats) {
    let _ = writeln!(out, "| Gain (× mise de base) | Probabilité | Fréquence | Pour 10 000 rounds | Points de RTP | Moyenne dans la bande | Issues |");
    let _ = writeln!(out, "|---|---:|---:|---:|---:|---:|---:|");
    for b in s.bands.iter().filter(|b| b.prob > 0.0) {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} |",
            b.label,
            pct(b.prob, 5),
            one_in(b.one_in),
            fr(b.per_10000, 2),
            fr(b.rtp * 100.0, 3),
            b.mean.map_or("—".into(), xm),
            group(b.outcomes)
        );
    }
}

fn percentiles_line(s: &ExactStats) -> String {
    s.percentiles
        .iter()
        .map(|p| {
            let q = format!("{}", (p.q * 100.0 * 1000.0).round() / 1000.0).replace('.', ",");
            format!("P{q} = {}", xm(p.value))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn build(i: &PromptInput) -> String {
    let s = i.stats;
    let spec = i.spec;
    let mut o = String::new();
    let w = &mut o;

    let _ = writeln!(w, "# Cahier des charges mathématique — {}", spec.name);
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "> Généré par Slot Math Intelligence (moteur smi-core {}) le {} · modèle `{}` v{} · empreinte de la loi `{}`.",
        crate::ENGINE_VERSION,
        i.generated_at,
        i.model_id,
        i.version,
        s.fingerprint
    );
    let _ = writeln!(w, "> Natures des chiffres : **EXACT** (somme pondérée finie sur la loi cible), **EXACT FORMULA** (formule fermée sur valeurs exactes), **MONTE CARLO** (tirages à graine fixe, IC 95 %), **SPECIFICATION** (objectif de conception), **IMPORTÉ** (source tierce datée).");
    let _ = writeln!(w);

    let _ = writeln!(w, "## 1. Ta mission");
    let _ = writeln!(w);
    let _ = writeln!(w, "Tu es l'ingénieur mathématique du jeu. Conçois puis implémente en **Rust** le modèle mathématique complet de **{}** : règles, bandes ou tables, simulation des books, lookup tables (format Stake Engine : `id,weight,payout`), et un audit indépendant. Le modèle doit reproduire la **loi cible** ci-dessous (section 3) aux tolérances de la section 7.", spec.name);
    let _ = writeln!(w);
    let _ = writeln!(w, "Règles impératives :");
    let _ = writeln!(w, "- Ne propose **aucun nombre arbitraire** : chaque chiffre annoncé doit être calculé par ton moteur et recalculable par l'audit. Étiquette chaque chiffre EXACT / MONTE CARLO / SPECIFICATION.");
    let _ = writeln!(w, "- Les gains doivent **sortir des règles du jeu** (rouleaux, symboles, bonus) ; les poids de la lookup table ne servent qu'à régler les fréquences (comme Stake Engine : les books disent ce qui PEUT arriver, les poids disent À QUELLE FRÉQUENCE).");
    let _ = writeln!(w, "- Calcule exactement tout ce qui peut l'être (sommes pondérées sur la lookup table, énumérations) ; réserve le Monte Carlo à ce qui n'a pas de forme fermée et donne alors la graine et l'intervalle de confiance.");
    let _ = writeln!(w, "- Un événement rare (max win) ne se valide pas par simulation : donne sa probabilité exacte sur la table, son espérance de comptage N·p et P(aucun) = (1−p)^N.");
    let _ = writeln!(w, "- Si un objectif est incompatible avec les autres, **dis-le** avec le calcul qui le prouve et propose l'arbitrage ; ne le masque pas.");
    let _ = writeln!(w);

    // 2. Informations générales
    let _ = writeln!(w, "## 2. Informations générales (SPECIFICATION)");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Paramètre | Valeur |");
    let _ = writeln!(w, "|---|---|");
    let _ = writeln!(w, "| Nom | {} |", spec.name);
    let _ = writeln!(w, "| Type de jeu | {} |", spec.game_type);
    let _ = writeln!(w, "| Mécaniques | {} |", if spec.mechanics.is_empty() { "—".into() } else { spec.mechanics.join(", ") });
    let _ = writeln!(w, "| Prix du mode | {} mise(s) de base |", fr(spec.price, 2));
    let _ = writeln!(w, "| RTP cible | {} |", pct(spec.rtp, 3));
    let _ = writeln!(w, "| Max win (plafond par round) | {} la mise de base |", xm(spec.max_win));
    let _ = writeln!(w, "| Fréquence du max win | {} |", one_in(s.max_win.one_in));
    let _ = writeln!(w, "| Volatilité souhaitée | {}{} |", spec.volatility, i.params.and_then(|p| p.sd_target).map_or(String::new(), |t| format!(" (σ/prix visé ≈ {})", fr(t, 2))));
    let _ = writeln!(w, "| Hit rate souhaité | {} ({}) |", pct(spec.hit_rate, 2), one_in(Some(1.0 / spec.hit_rate)));
    let _ = writeln!(w, "| Fréquence des bonus | {} |", one_in(Some(spec.bonus_one_in)));
    let _ = writeln!(w, "| Part du RTP venue des bonus | {} |", pct(spec.bonus_share, 1));
    let _ = writeln!(w);
    if !i.notes.is_empty() {
        let _ = writeln!(w, "Remarques du générateur :");
        for n in i.notes {
            let _ = writeln!(w, "- {n}");
        }
        let _ = writeln!(w);
    }

    // 3. Loi cible
    let _ = writeln!(w, "## 3. Loi cible (EXACT sur la loi générée)");
    let _ = writeln!(w);
    let _ = writeln!(w, "### 3.1 Indicateurs clés");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Indicateur | Valeur |");
    let _ = writeln!(w, "|---|---|");
    let _ = writeln!(w, "| RTP | {} |", pct(s.rtp, 4));
    let _ = writeln!(w, "| Gain moyen | {} |", xm(s.mean));
    let _ = writeln!(w, "| Hit rate P(gain > 0) | {} ({}) |", pct(s.hit_rate, 3), one_in(s.hit_one_in));
    let _ = writeln!(w, "| Spins perdants | {} |", pct(s.zero_rate, 3));
    let _ = writeln!(w, "| P(gain ≥ prix) | {} |", pct(s.p_ge_price, 3));
    let _ = writeln!(w, "| Médiane | {} |", xm(s.median));
    let _ = writeln!(w, "| Écart-type | {} mises de base · σ/prix = {} |", fr(s.sd, 3), fr(s.sd_per_price, 3));
    let _ = writeln!(w, "| VI 90 % / VI 95 % | {} / {} |", fr(s.vi90, 2), fr(s.vi95, 2));
    let _ = writeln!(w, "| Skewness / kurtosis en excès | {} / {} |", fr(s.skewness, 2), fr(s.excess_kurtosis, 1));
    let _ = writeln!(w, "| Max win | {} · probabilité {:.3e} · {} · {} points de RTP |", xm(s.max_win.value), s.max_win.prob, one_in(s.max_win.one_in), fr(s.max_win.rtp * 100.0, 3));
    let _ = writeln!(w, "| Attente médiane du max win | {} rounds |", s.max_win.median_wait.map_or("—".into(), |v| group(v as u64)));
    let _ = writeln!(w, "| Top 1 % des rounds | porte {} du retour |", s.concentration.iter().find(|c| (c.0 - 0.01).abs() < 1e-12).map_or("—".into(), |c| pct(c.1, 1)));
    let _ = writeln!(w, "| CVaR 0,1 % | {} |", xm(s.cvar_001));
    if let Some(v) = i.vol {
        let _ = writeln!(w, "| Classe de volatilité (score composite) | {} (score {}) |", v.class.label(), fr(v.score, 3));
    }
    let _ = writeln!(w);
    let _ = writeln!(w, "Percentiles du gain d'un round : {}.", percentiles_line(s));
    let _ = writeln!(w);

    let _ = writeln!(w, "### 3.2 Répartition par classe de résultat (partition des rounds)");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Classe | Probabilité | Fréquence | Points de RTP | Part du RTP | Moyenne | Médiane | P10 – P90 |");
    let _ = writeln!(w, "|---|---:|---:|---:|---:|---:|---:|---|");
    for k in &s.classes {
        let _ = writeln!(
            w,
            "| {} | {} | {} | {} | {} | {} | {} | {} – {} |",
            k.label,
            pct(k.prob, 4),
            one_in(k.one_in),
            fr(k.rtp * 100.0, 3),
            pct(k.share_of_rtp, 1),
            k.mean.map_or("—".into(), xm),
            k.median.map_or("—".into(), xm),
            k.p10.map_or("—".into(), xm),
            k.p90.map_or("—".into(), xm)
        );
    }
    let _ = writeln!(w);
    if let Some(b) = &s.bonus {
        let _ = writeln!(
            w,
            "Répartition du RTP : jeu de base {} points, bonus (max win compris) {} points → {} du RTP vient des bonus. Bonus : {}, espérance {}, médiane {}, attente médiane {} spins.",
            fr(b.rtp_base * 100.0, 3),
            fr(b.rtp_bonus * 100.0, 3),
            pct(b.bonus_share, 1),
            one_in(b.one_in),
            b.mean.map_or("—".into(), xm),
            b.median.map_or("—".into(), xm),
            b.median_wait.map_or("—".into(), |v| group(v as u64))
        );
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "### 3.3 Petits, moyens, gros et très gros gains");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Catégorie | Probabilité | Fréquence | Points de RTP | Gain moyen |");
    let _ = writeln!(w, "|---|---:|---:|---:|---:|");
    for c in &s.categories {
        let _ = writeln!(w, "| {} | {} | {} | {} | {} |", c.label, pct(c.prob, 4), one_in(c.one_in), fr(c.rtp * 100.0, 3), c.mean.map_or("—".into(), xm));
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "### 3.4 Distribution des gains par bande (tous rounds)");
    let _ = writeln!(w);
    bands_table(w, s);
    let _ = writeln!(w);
    if let Some(b) = i.base {
        let _ = writeln!(w, "### 3.5 Distribution du jeu de base (conditionnelle : rounds gagnants sans bonus)");
        let _ = writeln!(w);
        let _ = writeln!(w, "Moyenne {} · médiane {} · {}.", xm(b.mean), xm(b.median), percentiles_line(b));
        let _ = writeln!(w);
        bands_table(w, b);
        let _ = writeln!(w);
    }
    if let Some(b) = i.bonus {
        let _ = writeln!(w, "### 3.6 Distribution des bonus (conditionnelle : un bonus est déclenché)");
        let _ = writeln!(w);
        let _ = writeln!(w, "Moyenne {} · médiane {} · {}.", xm(b.mean), xm(b.median), percentiles_line(b));
        let _ = writeln!(w);
        bands_table(w, b);
        let _ = writeln!(w);
    }

    let _ = writeln!(w, "### 3.7 Seuils, fréquences et attentes (EXACT FORMULA)");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Gain ≥ | Probabilité | Fréquence | Attente médiane | Au moins 1 fois en 100 / 1 000 / 10 000 rounds |");
    let _ = writeln!(w, "|---|---:|---:|---:|---|");
    for t in &s.thresholds {
        let _ = writeln!(
            w,
            "| {} le prix | {} | {} | {} | {} / {} / {} |",
            xm(t.mult_price),
            pct(t.prob, 5),
            one_in(t.one_in),
            t.median_wait.map_or("—".into(), |v| group(v as u64)),
            pct(t.at_least_once_100, 1),
            pct(t.at_least_once_1000, 1),
            pct(t.at_least_once_10000, 1)
        );
    }
    let _ = writeln!(w);

    let _ = writeln!(w, "### 3.8 Barèmes Stake Engine (EXACT)");
    let _ = writeln!(w);
    let _ = writeln!(w, "| Barème | Contrôle | Valeur | Limite | Statut |");
    let _ = writeln!(w, "|---|---|---:|---:|---|");
    for k in &s.stake_checks {
        let status = match k.pass {
            Some(true) => "au vert",
            Some(false) => "**ROUGE**",
            None => "seuil non renseigné",
        };
        let _ = writeln!(w, "| {} | {} | {} | {} | {} |", k.scale, k.item, fr(k.value, 4), k.limit.map_or("—".into(), |l| fr(l, 4)), status);
    }
    let _ = writeln!(w);

    // 4. Simulation
    let _ = writeln!(w, "## 4. Résultats de simulation (MONTE CARLO)");
    let _ = writeln!(w);
    match i.sim {
        None => {
            let _ = writeln!(w, "Aucune simulation n'a encore été faite sur cette version : ne considère pas le modèle comme vérifié.");
        }
        Some(r) => {
            let c = &r.comparison;
            let _ = writeln!(w, "{} spins tirés de la loi cible (graine {}, {} thread(s), lots de {} spins ; empreinte des compteurs `{}`).", group(r.spins_done), r.config.seed, r.config.threads, group(r.config.chunk_size), r.counts_hash);
            let _ = writeln!(w);
            let _ = writeln!(w, "| Mesure | Exact | Simulé | Écart |");
            let _ = writeln!(w, "|---|---:|---:|---|");
            let _ = writeln!(w, "| RTP | {} | {} | z = {} (IC 95 % {} – {}) |", pct(c.rtp_theory, 4), pct(c.rtp_sim, 4), fr(c.rtp_z, 2), pct(c.rtp_ci95.0, 3), pct(c.rtp_ci95.1, 3));
            let _ = writeln!(w, "| Hit rate | {} | {} | z = {} |", pct(c.hit_theory, 3), pct(c.hit_sim, 3), fr(c.hit_z, 2));
            if let Some((t, e, z)) = c.bonus_freq {
                let _ = writeln!(w, "| Fréquence des bonus | {} | {} | z = {} |", one_in(Some(1.0 / t)), one_in(Some(1.0 / e)), fr(z, 2));
            }
            if let Some((t, e, z)) = c.bonus_mean {
                let _ = writeln!(w, "| Gain moyen des bonus | {} | {} | z = {} |", xm(t), xm(e), fr(z, 2));
            }
            if let Some((t, e)) = c.bonus_median {
                let _ = writeln!(w, "| Médiane des bonus | {} | {} | — |", xm(t), xm(e));
            }
            let _ = writeln!(w, "| Plus gros gain observé | {} (max) | {} | — |", xm(s.max_win.value), xm(r.biggest_win));
            let _ = writeln!(w, "| Max win | {} attendu(s) | {} observé(s) | P(aucun) = {} |", fr(c.max_win.expected, 2), c.max_win.observed, pct(c.max_win.p_none, 1));
            if let Some(ch) = &c.chi2 {
                let _ = writeln!(w, "| Bandes (khi-deux) | — | χ² = {} (ddl {}) | p = {} |", fr(ch.stat, 1), ch.df, fr(ch.p_value, 4));
            }
            let _ = writeln!(w);
            if !r.anomalies.is_empty() {
                let _ = writeln!(w, "Anomalies : {}", r.anomalies.join(" ; "));
                let _ = writeln!(w);
            }
        }
    }

    // 5. Marché
    let _ = writeln!(w, "## 5. Références du marché");
    let _ = writeln!(w);
    match i.market {
        None => {
            let _ = writeln!(w, "Aucune référence de marché chargée : ne justifie pas les choix par « le marché ».");
        }
        Some(m) => {
            let _ = writeln!(w, "Source : {} · collecte du {} · règle de sélection : {}.", m.source, m.fetched, m.selection_rule);
            let _ = writeln!(w, "Ces chiffres décrivent un échantillon de jeux (IMPORTÉ / EXACT sur leurs tables publiées) : ce sont des **points de comparaison**, pas des objectifs à recopier.");
            let _ = writeln!(w);
            if !m.metrics.is_empty() {
                let _ = writeln!(w, "| Indicateur (jeux comparables) | n | Moyenne | Médiane | Pondérée bets | Pondérée turnover | P25 – P75 | Notre jeu | Rang centile |");
                let _ = writeln!(w, "|---|---:|---:|---:|---:|---:|---|---:|---:|");
                for mm in &m.metrics {
                    let f = |v: Option<f64>| -> String {
                        match (v, mm.fmt.as_str()) {
                            (None, _) => "n.d.".into(),
                            (Some(x), "pct") => pct(x, 2),
                            (Some(x), "x") => xm(x),
                            (Some(x), "one_in") => one_in(Some(1.0 / x)),
                            (Some(x), _) => fr(x, 2),
                        }
                    };
                    let _ = writeln!(
                        w,
                        "| {}{} | {} | {} | {} | {} | {} | {} – {} | {} | {} |",
                        mm.label,
                        if mm.agg.insufficient { " ⚠ échantillon insuffisant" } else { "" },
                        mm.agg.n,
                        f(mm.agg.mean),
                        f(mm.agg.median),
                        f(mm.agg.mean_weighted_bets),
                        f(mm.agg.mean_weighted_turnover),
                        f(mm.agg.q1),
                        f(mm.agg.q3),
                        f(mm.ours),
                        mm.ours_rank_pct.map_or("—".into(), |r| format!("{}", fr(r, 0)))
                    );
                }
                let _ = writeln!(w);
            }
            if !m.peers.is_empty() {
                let _ = writeln!(w, "Jeux comparables (les plus joués d'abord) :");
                let _ = writeln!(w);
                let _ = writeln!(w, "| Rang | Jeu | Studio | Mode | Max | RTP | Hit | σ/prix | Max win | Médiane | Part RTP ≥ 100× | Volatilité |");
                let _ = writeln!(w, "|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|");
                for p in m.peers.iter().take(25) {
                    let _ = writeln!(
                        w,
                        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                        p.rank.map_or("—".into(), |r| r.to_string()),
                        p.name,
                        p.studio,
                        p.mode,
                        xm(p.max_mult),
                        opt(p.rtp, |v| pct(v, 2)),
                        opt(p.hit, |v| pct(v, 1)),
                        opt(p.sd_per_price, |v| fr(v, 1)),
                        one_in(p.max_one_in),
                        opt(p.median, xm),
                        opt(p.share_100x, |v| pct(v, 0)),
                        p.vol_class.clone().unwrap_or_else(|| "—".into())
                    );
                }
                let _ = writeln!(w);
            }
            if !m.providers.is_empty() {
                let _ = writeln!(w, "Studios comparables : {}.", m.providers.iter().take(10).map(|p| format!("{} ({} jeu(x), σ/prix médian {}, max médian {})", p.studio, p.slots, opt(p.median_sd, |v| fr(v, 1)), opt(p.median_max, xm))).collect::<Vec<_>>().join(" ; "));
                let _ = writeln!(w);
            }
            if !m.missing.is_empty() {
                let _ = writeln!(w, "Non disponible dans la source (ne pas inventer) : {}.", m.missing.join(", "));
                let _ = writeln!(w);
            }
        }
    }
    let _ = writeln!(w, "Justification attendue : pour chaque écart notable entre notre loi et la médiane des jeux comparables (hit rate, σ/prix, fréquence du max win, part du RTP au-delà de 100×), explique le choix de conception et son effet sur le joueur (sessions, attentes, frustration), chiffres à l'appui.");
    let _ = writeln!(w);

    // 6. Contraintes techniques
    let _ = writeln!(w, "## 6. Contraintes techniques");
    let _ = writeln!(w);
    let _ = writeln!(w, "- **Architecture** : workspace Rust (`crates/<jeu>-core` règles + simulation des books, `crates/<jeu>-audit` audit indépendant qui n'importe RIEN du moteur). Books au format Stake Engine (`books_<mode>.jsonl.zst`), lookup tables `lookUpTable_<mode>_0.csv` (`id,weight,payout`, paiement en centièmes de mise), `index.json`. Tirages pondérés en itérant des collections TRIÉES (jamais une `HashMap`), graines fixes, flux par lot.");
    let _ = writeln!(w, "- **Règles de paiement** : paiements sur une grille de 0,1× ; le gain d'un round est la somme de ses gains ; aucune issue > {} ; une issue qui atteint le plafond s'arrête et paie exactement {}.", xm(spec.max_win), xm(spec.max_win));
    let _ = writeln!(w, "- **Bonus** : déclenchement {} par round (max win compris), part du RTP {} ; la loi conditionnelle du bonus doit suivre la section 3.6 (médiane, P10, P90) — pas seulement sa moyenne. Relances plafonnées et documentées.", one_in(Some(spec.bonus_one_in)), pct(spec.bonus_share, 1));
    let _ = writeln!(w, "- **Multiplicateurs** : toute valeur de multiplicateur et sa probabilité doivent figurer dans la configuration ; le plafond s'applique APRÈS multiplicateurs.");
    let _ = writeln!(w, "- **Plafond de gain** : {} par round ; probabilité visée {} ; donne le nombre de books distincts au plafond (variété : éviter qu'un même max win soit rejoué).", xm(spec.max_win), one_in(s.max_win.one_in));
    let _ = writeln!(w, "- **Méthode de vérification** : (1) ré-évaluer chaque book par l'audit indépendant ; (2) comparer ids et paiements book ↔ table ; (3) recalculer le RTP par issues, classes, bandes et base + bonus (clôture < 1e-9) ; (4) recalculer EXACTEMENT sur la table chaque chiffre de la section 3 ; (5) simuler ≥ 10 M rounds à graine fixe et comparer (z, khi-deux) ; (6) barèmes Stake 2★ et 3★.");
    let _ = writeln!(w);
    let dr = i.design.cloned().unwrap_or_default();
    let _ = writeln!(w, "### 6.1 Contrôles de conception du jeu terminé (mesurés sur les books publiés)");
    let _ = writeln!(w);
    let _ = writeln!(w, "Slot Math Intelligence relira le dossier publié (`index.json`, tables, books) et vérifiera chaque point ci-dessous, exactement sur les poids. Conçois le jeu pour les respecter dès le départ :");
    let _ = writeln!(w);
    let _ = writeln!(w, "1. **Bas et hauts équilibrés** : sur les plateaux du jeu de base, au moins {} de symboles HAUTS parmi les bas + hauts affichés.", pct(dr.high_board_min, 0));
    let _ = writeln!(w, "2. **Pas que des connexions de bas** : au moins {} des connexions gagnantes du jeu de base portées par des symboles hauts (bandes, piles et table de paiement à régler en conséquence).", pct(dr.high_wins_min, 0));
    let _ = writeln!(w, "3. **Plancher loin de la médiane** : pour tout bonus, hidden ou achat à gain minimal garanti, médiane ≥ {} × le plancher et au plus {} des rounds à moins de +10 % du plancher (contre-exemple : plancher 1 000×, médiane 1 100×).", fr(dr.floor_median_ratio_min, 1), pct(dr.floor_near_max, 0));
    let _ = writeln!(w, "4. **Plus de scatters = plus de gain** : à 6 scatters le bonus paie plus qu'à 5, à 5 plus qu'à 4, à 4 plus qu'à 3 — en moyenne ET en médiane, tous bonus confondus ET dans chaque bonus. Le nombre de scatters doit donc changer le contenu du bonus (tours, multiplicateurs, départ), pas seulement sa fréquence.");
    let _ = writeln!(w, "5. **Books par tranche** : au moins {} books distincts dans chaque tranche de gain, du plus petit gain au max, séparément pour le jeu de base et pour les bonus ; aucune tranche vide sous le max.", dr.min_books_per_band);
    let _ = writeln!(w, "6. **Pas de doublons** : chance de revoir un bonus identique en 10 bonus ≤ {} ; deux bonus au même multiplicateur ≤ {} ; aucun book au contenu identique à un autre.", pct(dr.repeat_in_10_max, 0), pct(dr.same_multi_max, 0));
    let _ = writeln!(w, "7. **Profil** : hit rate du jeu de base au moins au premier quartile des jeux comparables (section 5){} ; donner la répartition mort / sous la mise / ≥ mise.", dr.studio_hit_target.map_or(String::new(), |t| format!(", et ≥ {} (cible du studio)", pct(t, 1))));
    let _ = writeln!(w, "8. **Max win** : ≤ {} du prix (barème Stake 2★), fréquence de la section 3.", xm(dr.max_win_2star));
    let _ = writeln!(w, "9. **Part du RTP des bonus** : ≤ {} dans le mode de base, ≤ {} dans un Ante (un Ante qui double la fréquence des bonus à loi égale fait monter cette part : la compenser).", pct(dr.bonus_share_base_max, 0), pct(dr.bonus_share_ante_max, 0));
    let _ = writeln!(w);
    let _ = writeln!(w, "Format attendu des books pour que ces contrôles soient mesurables : champ `criteria` qui distingue les rounds sans bonus, plateaux dans les événements de révélation (`board`, avec le contexte jeu de base / bonus), gains détaillés (`winInfo.wins[].symbol` et `amount`), événement de déclenchement avec les `positions` des scatters et le nom du `bonus`.");
    let _ = writeln!(w);

    // 7. Objectifs de validation
    let _ = writeln!(w, "## 7. Objectifs de validation (critères d'acceptation)");
    let _ = writeln!(w);
    let tol = i.validation.map(|v| v.tolerances.clone()).unwrap_or_default();
    let _ = writeln!(w, "| Critère | Cible | Tolérance |");
    let _ = writeln!(w, "|---|---|---|");
    let _ = writeln!(w, "| RTP (sur la table) | {} | ±{} point |", pct(spec.rtp, 4), fr(tol.rtp_abs * 100.0, 6));
    let _ = writeln!(w, "| Hit rate | {} | ±{} % relatif |", pct(s.hit_rate, 3), fr(tol.hit_rel * 100.0, 1));
    if let Some(b) = &s.bonus {
        let _ = writeln!(w, "| Fréquence des bonus | {} | ±{} % relatif |", one_in(b.one_in), fr(tol.bonus_freq_rel * 100.0, 1));
        let _ = writeln!(w, "| Part du RTP des bonus | {} | ±{} point |", pct(b.bonus_share, 2), fr(tol.bonus_share_abs * 100.0, 2));
    }
    let _ = writeln!(w, "| Max win | {} à {} | ±{} % relatif sur la fréquence |", xm(s.max_win.value), one_in(s.max_win.one_in), fr(tol.max_prob_rel * 100.0, 1));
    let _ = writeln!(w, "| σ / prix | {} | ±{} % relatif (cible souple) |", fr(s.sd_per_price, 2), fr(tol.sd_rel * 100.0, 0));
    let _ = writeln!(w, "| Médiane et P90 des bonus | {} / {} | même bande de gain |", s.bonus.as_ref().and_then(|b| b.median).map_or("—".into(), xm), s.bonus.as_ref().and_then(|b| b.p90).map_or("—".into(), xm));
    let _ = writeln!(w, "| Simulation | ≥ 10 000 000 rounds | |z| ≤ {} sur RTP et hit rate, khi-deux p ≥ {} |", fr(tol.sim_z_max, 2), tol.chi2_p_min);
    let _ = writeln!(w, "| Audit | 0 défaut sur tous les books | — |");
    let _ = writeln!(w);
    if let Some(v) = i.validation {
        let _ = writeln!(w, "État de la loi cible dans Slot Math Intelligence : **{}** ({} contrôles réussis, {} avertissements, {} échecs, {} non exécutés).", v.status.label(), v.passed, v.warnings, v.failed, v.not_run);
        for c in v.checks.iter().filter(|c| c.status == CheckStatus::Fail || c.status == CheckStatus::Warn) {
            let _ = writeln!(w, "- {} {} — {} : attendu {}, observé {}.", if c.status == CheckStatus::Fail { "ÉCHEC" } else { "Avertissement" }, c.id, c.label, c.expected, c.observed);
        }
        let _ = writeln!(w);
    }

    // 8. Livrables
    let _ = writeln!(w, "## 8. Livrables attendus");
    let _ = writeln!(w);
    let _ = writeln!(w, "1. Les règles complètes (plateau, symboles, table de paiement, bonus, multiplicateurs, plafond) et la configuration de simulation (bandes, tables), étiquetées CONFIGURATION.");
    let _ = writeln!(w, "2. Le code Rust du moteur et de l'audit, avec tests unitaires et vecteurs de référence.");
    let _ = writeln!(w, "3. Les fichiers publiés (books, lookup tables, index) et leurs empreintes SHA-256.");
    let _ = writeln!(w, "4. Un rapport mathématique sur le modèle de « Witchcraft — Mathematical Model » : indicateurs par mode, classes, bandes, percentiles, seuils et attentes, max win, sessions (Monte Carlo avec IC), vérification (intégrité, ré-évaluation, clôture).");
    let _ = writeln!(w, "5. Le tableau de conformité aux critères de la section 7, chiffre par chiffre, avec les écarts expliqués.");
    let _ = writeln!(w, "6. Le rapport « Contrôles de conception » de Slot Math Intelligence sur le dossier publié (section 6.1), chaque point au vert ou son écart expliqué.");
    let _ = writeln!(w);

    // Annexe JSON
    let _ = writeln!(w, "## Annexe — objectifs lisibles par machine");
    let _ = writeln!(w);
    let annex = serde_json::json!({
        "name": spec.name,
        "price": spec.price,
        "rtp": s.rtp,
        "max_win": s.max_win.value,
        "max_win_one_in": s.max_win.one_in,
        "hit_rate": s.hit_rate,
        "bonus_one_in": s.bonus.as_ref().and_then(|b| b.one_in),
        "bonus_share": s.bonus.as_ref().map(|b| b.bonus_share),
        "bonus_median": s.bonus.as_ref().and_then(|b| b.median),
        "sd_per_price": s.sd_per_price,
        "classes": s.classes.iter().map(|k| serde_json::json!({"key": k.key, "prob": k.prob, "rtp": k.rtp, "mean": k.mean, "median": k.median})).collect::<Vec<_>>(),
        "fingerprint": s.fingerprint,
    });
    let _ = writeln!(w, "```json\n{}\n```", serde_json::to_string_pretty(&annex).unwrap_or_default());
    o
}
