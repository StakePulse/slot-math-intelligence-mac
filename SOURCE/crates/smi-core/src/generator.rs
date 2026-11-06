//! MATH GENERATOR — construit une loi complète à partir des objectifs d'un nouveau jeu.
//!
//! La loi produite a la forme d'une lookup table Stake Engine (paiements sur une grille de 0,1×,
//! poids entiers) et quatre classes de résultat :
//!
//! | classe | probabilité | espérance (mises de base) |
//! |---|---|---|
//! | pas de gain | 1 − hit | 0 |
//! | gains du jeu de base | hit − 1/N_bonus | (1 − part bonus) · RTP · prix |
//! | bonus (corps) | 1/N_bonus − p_max | part bonus · RTP · prix − p_max · max |
//! | max win | p_max | p_max · max |
//!
//! Dans chaque classe de gain, la loi conditionnelle est une **loi de puissance discrétisée**
//! (densité ∝ x^−a sur une grille de pas croissants : 0,1× jusqu'à 10×, 0,5× jusqu'à 100×, 5×
//! jusqu'à 1 000×, 50× jusqu'à 10 000×, 500× au-delà). L'exposant de chaque classe est résolu par
//! dichotomie pour donner EXACTEMENT l'espérance de la classe (la moyenne est strictement
//! décroissante en a). Le plancher du bonus (gain minimal d'un bonus) est le second degré de
//! liberté : il règle la volatilité (plus il est haut, plus le bonus est régulier et l'écart-type
//! bas) ; il est cherché par dichotomie pour viser l'écart-type / prix de la classe de volatilité
//! demandée. Si la cible est hors d'atteinte avec ces contraintes, le générateur le DIT et donne la
//! plage atteignable — il ne triche pas sur les autres objectifs.
//!
//! Toutes les contraintes sont vérifiées avant construction (probabilités positives, espérances
//! atteignables) avec un message explicite. La loi finale est ensuite mesurée EXACTEMENT par
//! `stats::compute` et contrôlée par `validate`.

use crate::dist::{Atom, ClassDef, ClassKind, Distribution, CENTS};
use crate::volatility::{sd_range, VolClass, VolatilityConfig};
use serde::{Deserialize, Serialize};

/// Somme des poids d'une loi générée : 10^15 (probabilités au 1e-15 près, RTP au 1e-12 près).
pub const WEIGHT_SCALE: f64 = 1e15;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeneratorSpec {
    pub name: String,
    pub game_type: String,
    pub mechanics: Vec<String>,
    /// Prix du mode en mises de base (1 = jeu de base).
    pub price: f64,
    /// RTP cible (0,965 = 96,5 %).
    pub rtp: f64,
    /// Gain maximal en mises de base (plafond par round).
    pub max_win: f64,
    /// Fréquence visée du max win (1 sur N). `None` : déduite de `max_rtp_share`.
    pub max_win_one_in: Option<f64>,
    /// Part du RTP portée par le max win quand `max_win_one_in` n'est pas donné.
    pub max_rtp_share: f64,
    /// Hit rate visé : P(gain > 0).
    pub hit_rate: f64,
    /// Fréquence des bonus (1 sur N rounds), max win compris.
    pub bonus_one_in: f64,
    /// Part du RTP venue des bonus (max win compris).
    pub bonus_share: f64,
    pub volatility: String,
    /// Écart-type / prix visé ; `None` : milieu de la plage de la classe de volatilité.
    pub sd_target: Option<f64>,
    /// Gain maximal du jeu de base hors bonus ; `None` : min(max, 1 000× prix).
    pub base_max_win: Option<f64>,
    /// Plancher du bonus imposé ; `None` : cherché pour la volatilité.
    pub bonus_floor: Option<f64>,
    /// Plancher minimal du bonus, en multiples du prix, quand il est cherché (« un bonus doit payer »).
    /// Abaissé automatiquement à la moitié de l'espérance du bonus si celle-ci est trop basse.
    pub bonus_floor_min: f64,
    /// Pas de la grille des paiements (0,1× par défaut).
    pub granularity: f64,
}

impl Default for GeneratorSpec {
    fn default() -> Self {
        Self {
            name: "Nouveau jeu".into(),
            game_type: "Video slot".into(),
            mechanics: vec!["Ways".into(), "Free Spins".into(), "Scatter Features".into()],
            price: 1.0,
            rtp: 0.965,
            max_win: 5_000.0,
            max_win_one_in: None,
            max_rtp_share: 0.015,
            hit_rate: 0.25,
            bonus_one_in: 200.0,
            bonus_share: 0.45,
            volatility: "High".into(),
            sd_target: None,
            base_max_win: None,
            bonus_floor: None,
            bonus_floor_min: 10.0,
            granularity: 0.1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GenParams {
    pub p_zero: f64,
    pub p_base: f64,
    pub p_bonus_body: f64,
    pub p_max: f64,
    pub mean_base: f64,
    pub mean_bonus_body: f64,
    pub alpha_base: f64,
    pub beta_bonus: f64,
    pub base_max: f64,
    pub bonus_floor: f64,
    pub sd_target: Option<f64>,
    pub sd_achieved: f64,
    /// Plage d'écart-type / prix atteignable en faisant varier le plancher du bonus.
    pub sd_feasible: (f64, f64),
    pub support_points: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GeneratedModel {
    pub spec: GeneratorSpec,
    pub distribution: Distribution,
    pub params: GenParams,
    /// Avertissements (cibles non atteignables, ajustements) — à lire avant toute validation.
    pub notes: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GenError {
    #[error("objectif invalide : {0}")]
    Invalid(String),
    #[error("objectifs incompatibles : {0}")]
    Infeasible(String),
}

/// Grille des paiements (en mises de base) dans [lo, hi], avec la largeur de case de chaque point.
pub fn grid(lo: f64, hi: f64, g: f64) -> Vec<(f64, f64)> {
    let segments = [(10.0, g), (100.0, 0.5f64.max(g)), (1_000.0, 5.0f64.max(g)), (10_000.0, 50.0f64.max(g)), (100_000.0, 500.0f64.max(g)), (f64::INFINITY, 5_000.0f64.max(g))];
    let mut out = Vec::new();
    let mut start: f64 = 0.0;
    for (end, step) in segments {
        let first = ((start.max(lo) / step).ceil() * step).max(step);
        let mut k = (first / step).round() as u64;
        loop {
            let x = k as f64 * step;
            if x > end + 1e-9 || x > hi + 1e-9 {
                break;
            }
            if x >= lo - 1e-9 && (out.last().map_or(true, |&(p, _): &(f64, f64)| x > p + 1e-9)) {
                out.push((round_grid(x, g), step));
            }
            k += 1;
        }
        if end >= hi {
            break;
        }
        start = end;
    }
    out
}

fn round_grid(x: f64, g: f64) -> f64 {
    ((x / g).round() * g * 1e6).round() / 1e6
}

/// Poids (non normalisés) d'une loi de puissance x^−a sur la grille, et sa moyenne.
fn power_weights(pts: &[(f64, f64)], a: f64) -> Vec<f64> {
    // Normalisation en log pour éviter les débordements : x^−a · Δ = exp(−a ln x + ln Δ − c).
    let logs: Vec<f64> = pts.iter().map(|&(x, dx)| -a * x.ln() + dx.ln()).collect();
    let m = logs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    logs.iter().map(|l| (l - m).exp()).collect()
}

fn moments(pts: &[(f64, f64)], w: &[f64]) -> (f64, f64) {
    let s: f64 = w.iter().sum();
    let m1: f64 = pts.iter().zip(w).map(|(p, w)| p.0 * w).sum::<f64>() / s;
    let m2: f64 = pts.iter().zip(w).map(|(p, w)| p.0 * p.0 * w).sum::<f64>() / s;
    (m1, m2)
}

/// Exposant `a` tel que la moyenne de la loi x^−a sur `pts` vaille `target`.
fn solve_exponent(pts: &[(f64, f64)], target: f64) -> Result<f64, GenError> {
    let mean = |a: f64| moments(pts, &power_weights(pts, a)).0;
    let (mut lo, mut hi) = (-20.0, 400.0);
    let (m_lo, m_hi) = (mean(lo), mean(hi));
    if !(target < m_lo && target > m_hi) {
        return Err(GenError::Infeasible(format!(
            "espérance conditionnelle {:.4}× hors de la plage atteignable [{:.4}× ; {:.4}×] sur [{}× ; {}×]",
            target,
            m_hi,
            m_lo,
            pts.first().map_or(0.0, |p| p.0),
            pts.last().map_or(0.0, |p| p.0)
        )));
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mean(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

struct Built {
    base: Vec<(f64, f64)>,
    bonus: Vec<(f64, f64)>,
    alpha: f64,
    beta: f64,
    floor: f64,
    sd_price: f64,
}

fn build(spec: &GeneratorSpec, pr: &Probs, base_pts: &[(f64, f64)], floor: f64) -> Result<Built, GenError> {
    let g = spec.granularity;
    let alpha = solve_exponent(base_pts, pr.mean_base)?;
    let wb = power_weights(base_pts, alpha);
    let (b1, b2) = moments(base_pts, &wb);
    let bonus_pts = grid(floor, spec.max_win - g, g);
    if bonus_pts.len() < 2 {
        return Err(GenError::Infeasible(format!("plancher du bonus {floor}× trop proche du max")));
    }
    let beta = solve_exponent(&bonus_pts, pr.mean_bonus)?;
    let wbo = power_weights(&bonus_pts, beta);
    let (o1, o2) = moments(&bonus_pts, &wbo);
    let mean = pr.p_base * b1 + pr.p_bonus * o1 + pr.p_max * spec.max_win;
    let e2 = pr.p_base * b2 + pr.p_bonus * o2 + pr.p_max * spec.max_win * spec.max_win;
    let sd = (e2 - mean * mean).max(0.0).sqrt();
    let sb: f64 = wb.iter().sum();
    let so: f64 = wbo.iter().sum();
    Ok(Built {
        base: base_pts.iter().zip(&wb).map(|(p, w)| (p.0, w / sb)).collect(),
        bonus: bonus_pts.iter().zip(&wbo).map(|(p, w)| (p.0, w / so)).collect(),
        alpha,
        beta,
        floor,
        sd_price: sd / spec.price,
    })
}

struct Probs {
    p_zero: f64,
    p_base: f64,
    p_bonus: f64,
    p_max: f64,
    mean_base: f64,
    mean_bonus: f64,
}

fn probs(spec: &GeneratorSpec) -> Result<(Probs, Vec<String>), GenError> {
    let mut notes = Vec::new();
    let s = spec;
    let check = |ok: bool, msg: String| if ok { Ok(()) } else { Err(GenError::Invalid(msg)) };
    check(s.price > 0.0, format!("prix {} ≤ 0", s.price))?;
    check(s.rtp > 0.0 && s.rtp < 1.5, format!("RTP {} hors de ]0 ; 1,5[", s.rtp))?;
    check(s.max_win > s.price, "le gain maximal doit dépasser le prix du mode".into())?;
    check(s.hit_rate > 0.0 && s.hit_rate <= 1.0, format!("hit rate {} hors de ]0 ; 1]", s.hit_rate))?;
    check(s.bonus_one_in >= 1.0, "la fréquence des bonus doit être « 1 sur N » avec N ≥ 1".into())?;
    check(s.bonus_share > 0.0 && s.bonus_share < 1.0, "la part du RTP venue des bonus doit être dans ]0 ; 1[".into())?;
    check(s.granularity > 0.0, "pas de grille ≤ 0".into())?;
    let total = s.rtp * s.price;
    let p_max = match s.max_win_one_in {
        Some(n) => {
            check(n >= 1.0, "max win « 1 sur N » avec N < 1".into())?;
            1.0 / n
        }
        None => {
            let p = s.max_rtp_share * total / s.max_win;
            notes.push(format!(
                "Fréquence du max win déduite : {:.2} % du RTP au plafond → 1 sur {:.0}.",
                s.max_rtp_share * 100.0,
                1.0 / p
            ));
            p
        }
    };
    let p_bonus_all = 1.0 / s.bonus_one_in;
    let p_bonus = p_bonus_all - p_max;
    let p_base = s.hit_rate - p_bonus_all;
    let e_bonus_all = s.bonus_share * total;
    let e_bonus = e_bonus_all - p_max * s.max_win;
    let e_base = total - e_bonus_all;
    if p_bonus <= 0.0 {
        return Err(GenError::Infeasible(format!(
            "le max win (1 sur {:.0}) est plus fréquent que les bonus (1 sur {:.0}) : il doit venir des bonus",
            1.0 / p_max,
            s.bonus_one_in
        )));
    }
    if p_base <= 0.0 {
        return Err(GenError::Infeasible(format!("hit rate {:.2} % ≤ fréquence des bonus {:.3} %", s.hit_rate * 100.0, p_bonus_all * 100.0)));
    }
    if e_bonus <= 0.0 {
        return Err(GenError::Infeasible(format!(
            "le max win seul porte {:.2} % du RTP, plus que la part bonus visée ({:.1} %)",
            p_max * s.max_win / total * 100.0,
            s.bonus_share * 100.0
        )));
    }
    Ok((
        Probs { p_zero: 1.0 - s.hit_rate, p_base, p_bonus, p_max, mean_base: e_base / p_base, mean_bonus: e_bonus / p_bonus },
        notes,
    ))
}

pub fn generate(spec: &GeneratorSpec, vol_cfg: &VolatilityConfig) -> Result<GeneratedModel, GenError> {
    let (pr, mut notes) = probs(spec)?;
    let g = spec.granularity;
    let base_max = spec.base_max_win.unwrap_or_else(|| (1_000.0 * spec.price).min(spec.max_win - g)).min(spec.max_win - g);
    let base_pts = grid(g, base_max, g);
    if pr.mean_base <= g || pr.mean_base >= base_max {
        return Err(GenError::Infeasible(format!(
            "gain moyen du jeu de base {:.3}× hors de ]{g}× ; {base_max}×[ : revoir hit rate, part bonus ou gain max de base",
            pr.mean_base
        )));
    }

    let sd_target = match spec.sd_target {
        Some(v) => Some(v),
        None => VolClass::parse(&spec.volatility).map(|c| {
            let (lo, hi) = sd_range(c, vol_cfg);
            match hi {
                Some(h) if lo > 0.0 => (lo * h).sqrt(),
                Some(h) => h * 0.6,
                None => lo * 1.25,
            }
        }),
    };

    // Plage du plancher : de la grille jusqu'à 95 % de la moyenne conditionnelle du bonus.
    let floor_lo = g.max((spec.bonus_floor_min * spec.price).min(pr.mean_bonus * 0.5));
    if floor_lo < spec.bonus_floor_min * spec.price - 1e-9 {
        notes.push(format!(
            "Plancher du bonus abaissé à {:.1}× (la moitié de son espérance {:.1}×) : le minimum demandé ({}× prix) n'est pas compatible avec la part de RTP des bonus.",
            floor_lo, pr.mean_bonus, spec.bonus_floor_min
        ));
    }
    let floor_hi = (pr.mean_bonus * 0.95).max(floor_lo + g);
    let try_floor = |f: f64| build(spec, &pr, &base_pts, round_grid(f, g).max(g));
    // Plus haut plancher réalisable (la grille peut rendre la moyenne inatteignable près de 95 %).
    let floor_hi = if try_floor(floor_hi).is_ok() {
        floor_hi
    } else {
        let (mut lo, mut hi) = (floor_lo.ln(), floor_hi.ln());
        for _ in 0..50 {
            let mid = 0.5 * (lo + hi);
            if try_floor(mid.exp()).is_ok() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo.exp()
    };
    let built = match spec.bonus_floor {
        Some(f) => try_floor(f)?,
        None => {
            let a = try_floor(floor_lo);
            let b = try_floor(floor_hi);
            match (a, b, sd_target) {
                (Ok(a), Ok(b), Some(target)) => {
                    // L'écart-type décroît quand le plancher monte.
                    let (sd_max, sd_min) = (a.sd_price, b.sd_price);
                    if target >= sd_max {
                        notes.push(format!(
                            "Volatilité visée σ/prix = {target:.2} hors d'atteinte : au plus {sd_max:.2} avec ces objectifs (plancher du bonus au minimum). Pour monter : max win plus fréquent, part bonus plus haute ou bonus plus rares."
                        ));
                        a
                    } else if target <= sd_min {
                        notes.push(format!(
                            "Volatilité visée σ/prix = {target:.2} hors d'atteinte : au moins {sd_min:.2} avec ces objectifs (le max win et sa fréquence imposent ce plancher). Pour descendre : max win plus rare ou plafond plus bas."
                        ));
                        b
                    } else {
                        let (mut lo, mut hi) = (floor_lo.ln(), floor_hi.ln());
                        let mut best = a;
                        for _ in 0..60 {
                            let mid = 0.5 * (lo + hi);
                            match try_floor(mid.exp()) {
                                Ok(m) => {
                                    if m.sd_price > target {
                                        lo = mid;
                                    } else {
                                        hi = mid;
                                    }
                                    if (m.sd_price - target).abs() < (best.sd_price - target).abs() {
                                        best = m;
                                    }
                                }
                                Err(_) => hi = mid,
                            }
                        }
                        best
                    }
                }
                (Ok(a), _, None) => a,
                (Ok(a), Err(_), Some(_)) => a,
                (Err(e), _, _) => return Err(e),
            }
        }
    };
    let sd_feasible = {
        let a = try_floor(floor_lo).map(|b| b.sd_price).unwrap_or(f64::NAN);
        let b = try_floor(floor_hi).map(|b| b.sd_price).unwrap_or(f64::NAN);
        (b.min(a), a.max(b))
    };

    // --- Poids entiers ---
    let classes = vec![
        ClassDef::new("no_win", "Pas de gain", ClassKind::NoWin),
        ClassDef::new("base", "Gains du jeu de base", ClassKind::Base),
        ClassDef::new("bonus", "Bonus", ClassKind::Bonus),
        ClassDef::new("max", "Max win", ClassKind::MaxWin),
    ];
    let to_c = |x: f64| (x * CENTS as f64).round() as u64;
    let mut atoms = Vec::with_capacity(built.base.len() + built.bonus.len() + 2);
    let mut dropped = 0usize;
    let mut push = |payout: u64, p: f64, class: u16| {
        let w = (p * WEIGHT_SCALE).round() as u64;
        if w == 0 {
            dropped += 1;
        } else {
            atoms.push(Atom { payout, weight: w, class, books: 1 });
        }
    };
    push(0, pr.p_zero, 0);
    for &(x, q) in &built.base {
        push(to_c(x), pr.p_base * q, 1);
    }
    for &(x, q) in &built.bonus {
        push(to_c(x), pr.p_bonus * q, 2);
    }
    push(to_c(spec.max_win), pr.p_max, 3);
    if dropped > 0 {
        notes.push(format!("{dropped} point(s) de grille de probabilité < 1e-15 retirés (effet sur le RTP < 1e-12)."));
    }
    let support_points = atoms.len();
    let distribution = Distribution::new(&spec.name, spec.price, Some(to_c(spec.max_win)), classes, atoms).map_err(|e| GenError::Invalid(e.to_string()))?;
    Ok(GeneratedModel {
        spec: spec.clone(),
        distribution,
        params: GenParams {
            p_zero: pr.p_zero,
            p_base: pr.p_base,
            p_bonus_body: pr.p_bonus,
            p_max: pr.p_max,
            mean_base: pr.mean_base,
            mean_bonus_body: pr.mean_bonus,
            alpha_base: built.alpha,
            beta_bonus: built.beta,
            base_max,
            bonus_floor: built.floor,
            sd_target,
            sd_achieved: built.sd_price,
            sd_feasible,
            support_points,
        },
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{compute, StatsOptions};

    #[test]
    fn grid_is_ordered_and_on_step() {
        let g = grid(0.1, 2_000.0, 0.1);
        assert!(g.windows(2).all(|w| w[1].0 > w[0].0));
        assert_eq!(g.first().unwrap().0, 0.1);
        assert_eq!(g.last().unwrap().0, 2_000.0);
        assert!(g.iter().all(|(x, _)| ((x * 10.0).round() - x * 10.0).abs() < 1e-6));
    }

    #[test]
    fn hits_every_target_exactly() {
        let spec = GeneratorSpec { max_win: 5_000.0, max_win_one_in: Some(800_000.0), hit_rate: 0.27, bonus_one_in: 180.0, bonus_share: 0.42, volatility: "Medium-High".into(), ..Default::default() };
        let m = generate(&spec, &VolatilityConfig::default()).unwrap();
        let s = compute(&m.distribution, &StatsOptions::default());
        assert!((s.rtp - 0.965).abs() < 1e-9, "rtp {}", s.rtp);
        assert!((s.hit_rate - 0.27).abs() < 1e-9);
        let b = s.bonus.as_ref().unwrap();
        assert!((b.one_in.unwrap() - 180.0).abs() < 1e-6);
        assert!((b.bonus_share - 0.42).abs() < 1e-9);
        assert!((s.max_win.one_in.unwrap() - 800_000.0).abs() / 800_000.0 < 1e-6);
        assert_eq!(s.max_win.value, 5_000.0);
        assert!(s.closure.max_abs_diff < 1e-12);
        // σ visé : milieu géométrique de [25 ; 40[ ≈ 31,6.
        let target = m.params.sd_target.unwrap();
        if m.notes.iter().all(|n| !n.contains("hors d'atteinte")) {
            assert!((s.sd_per_price - target).abs() / target < 0.02, "sd {} vs {}", s.sd_per_price, target);
        }
    }

    #[test]
    fn reports_infeasible_targets() {
        let spec = GeneratorSpec { hit_rate: 0.001, bonus_one_in: 200.0, ..Default::default() };
        assert!(matches!(generate(&spec, &VolatilityConfig::default()), Err(GenError::Infeasible(_))));
    }

    #[test]
    fn higher_volatility_class_gives_higher_sd() {
        let mk = |v: &str| {
            let spec = GeneratorSpec { volatility: v.into(), max_win: 10_000.0, max_win_one_in: Some(2_000_000.0), ..Default::default() };
            generate(&spec, &VolatilityConfig::default()).unwrap().params.sd_achieved
        };
        assert!(mk("Medium") < mk("High"));
    }
}
