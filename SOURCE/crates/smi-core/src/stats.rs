//! Statistiques EXACTES d'une loi : chaque chiffre est une somme pondérée finie sur les issues.
//!
//! Formules (X = gain d'un round en mises de base, c = prix du mode, p_i = w_i / W) :
//! - RTP = Σ p_i·x_i / c ; contribution d'une issue = p_i·x_i / c ;
//! - hit rate = P(X > 0) ; P(E) = Σ_{i∈E} p_i ; fréquence = 1 / P(E) ;
//! - variance = Σ p_i (x_i − μ)² ; skewness = μ₃/σ³ ; kurtosis en excès = μ₄/σ⁴ − 3
//!   (moments du gain en mises de base : sans dimension, donc indépendants de l'unité — piège des
//!   fichiers moteur relevé dans Witchcraft §16.4) ;
//! - percentile Pq = plus petit x tel que P(X ≤ x) ≥ q ;
//! - VI90 = 1,645·σ/c ; VI95 = 1,960·σ/c ; CV = σ/μ ;
//! - attente médiane = ⌈ln 0,5 / ln(1−p)⌉ ; « au moins une fois en n » = 1 − (1−p)^n ; dépense par événement = c / p ;
//! - issues effectives = (Σw)² / Σw² ;
//! - CVaR 0,1 % = moyenne des 0,1 % meilleurs rounds ; ETL(s) = E[X·1{X > s}] / c.
//!
//! La **clôture** recalcule le RTP par quatre sommations indépendantes (issues, classes, bandes,
//! base + bonus) : elles doivent coïncider (contrôle de Witchcraft §16.3).

use crate::dist::{ClassKind, Distribution, CENTS};
use crate::special::{at_least_once, median_wait};
use crate::stake::{self, StakeCheck, StakeInputs, StakeScale};
use serde::{Deserialize, Serialize};

pub const PERCENTILES: [f64; 11] = [0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99, 0.995, 0.999, 0.9999, 0.99999];
pub const PRICE_LADDER: [f64; 17] =
    [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0, 20_000.0, 50_000.0, 100_000.0];
pub const CONCENTRATION: [f64; 6] = [0.000_01, 0.000_1, 0.001, 0.01, 0.1, 0.5];
/// Bornes basses des bandes de gain, en centièmes de mise de base (la dernière borne est l'infini).
pub const BAND_EDGES: [u64; 21] = [
    1, 50, 100, 200, 300, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 200_000, 500_000, 1_000_000, 2_000_000,
    5_000_000, 10_000_000, 25_000_000, u64::MAX,
];
const STAKE_TAIL: [f64; 5] = [5_000.0, 10_000.0, 25_000.0, 50_000.0, 100_000.0];

/// Catégories de gains en multiples du PRIX (configurables, documentées dans DECISIONS.md D-08).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WinCategories {
    pub small_below: f64,
    pub medium_below: f64,
    pub big_below: f64,
}

impl Default for WinCategories {
    fn default() -> Self {
        Self { small_below: 2.0, medium_below: 20.0, big_below: 500.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StatsOptions {
    pub categories: WinCategories,
    pub scales: Vec<StakeScale>,
}

impl Default for StatsOptions {
    fn default() -> Self {
        Self { categories: WinCategories::default(), scales: vec![StakeScale::two_star(), StakeScale::three_star()] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Percentile {
    pub q: f64,
    /// En mises de base.
    pub value: f64,
    /// En multiples du prix.
    pub value_price: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Threshold {
    /// Seuil en multiples du prix.
    pub mult_price: f64,
    pub prob: f64,
    pub one_in: Option<f64>,
    pub median_wait: Option<f64>,
    pub at_least_once_100: f64,
    pub at_least_once_1000: f64,
    pub at_least_once_10000: f64,
    /// Dépense moyenne par événement, en mises de base.
    pub spend_per_event: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Band {
    pub label: String,
    /// Bornes en mises de base (`hi` exclue ; `None` = sans borne).
    pub lo: f64,
    pub hi: Option<f64>,
    pub is_zero: bool,
    pub is_max: bool,
    pub prob: f64,
    pub one_in: Option<f64>,
    pub per_10000: f64,
    /// Contribution au RTP, en fraction du prix (×100 = points de RTP).
    pub rtp: f64,
    pub mean: Option<f64>,
    /// P(X ≥ borne basse).
    pub survival: f64,
    pub outcomes: u64,
    pub books: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClassStat {
    pub key: String,
    pub label: String,
    pub kind: ClassKind,
    pub prob: f64,
    pub one_in: Option<f64>,
    pub rtp: f64,
    pub share_of_rtp: f64,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub p10: Option<f64>,
    pub p90: Option<f64>,
    pub p_no_win: Option<f64>,
    pub max: Option<f64>,
    /// P(gain maximal du mode | classe), en « 1 sur N ».
    pub max_win_one_in: Option<f64>,
    pub outcomes: u64,
    pub books: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub key: String,
    pub label: String,
    pub prob: f64,
    pub one_in: Option<f64>,
    pub rtp: f64,
    pub mean: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MaxWin {
    /// Gain maximal tirable, en mises de base.
    pub value: f64,
    pub value_price: f64,
    pub prob: f64,
    pub one_in: Option<f64>,
    pub rtp: f64,
    pub outcomes: u64,
    pub books: u64,
    pub cap: Option<f64>,
    pub reaches_cap: Option<bool>,
    pub median_wait: Option<f64>,
    pub spend_per_max: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BonusStats {
    pub prob: f64,
    pub one_in: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub p10: Option<f64>,
    pub p90: Option<f64>,
    /// Points de RTP (fraction du prix) venus des bonus (classes Bonus + MaxWin).
    pub rtp_bonus: f64,
    pub rtp_base: f64,
    pub bonus_share: f64,
    pub median_wait: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Closure {
    pub by_outcomes: f64,
    pub by_classes: f64,
    pub by_bands: f64,
    pub by_base_plus_bonus: f64,
    pub by_categories: f64,
    pub max_abs_diff: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExactStats {
    pub name: String,
    pub fingerprint: String,
    pub cost: f64,
    pub total_weight: String,
    pub outcomes: u64,
    pub books: u64,
    pub rtp: f64,
    pub mean: f64,
    pub variance: f64,
    pub sd: f64,
    pub sd_per_price: f64,
    pub cv: Option<f64>,
    pub vi90: f64,
    pub vi95: f64,
    pub skewness: f64,
    pub excess_kurtosis: f64,
    pub hit_rate: f64,
    pub hit_one_in: Option<f64>,
    pub zero_rate: f64,
    pub p_ge_price: f64,
    pub median: f64,
    pub percentiles: Vec<Percentile>,
    pub thresholds: Vec<Threshold>,
    pub bands: Vec<Band>,
    /// Bandes vides sous le gain maximal (« trous » de la distribution).
    pub gaps: Vec<String>,
    pub classes: Vec<ClassStat>,
    pub categories: Vec<Category>,
    pub concentration: Vec<(f64, f64)>,
    pub max_win: MaxWin,
    pub bonus: Option<BonusStats>,
    pub effective_outcomes: f64,
    pub heaviest_prob: f64,
    pub cvar_001: f64,
    pub etl_40: f64,
    pub etl_10000: f64,
    /// Part de la probabilité portée par des gains non multiples de 0,1×.
    pub off_grid_prob: f64,
    pub stake_checks: Vec<StakeCheck>,
    pub closure: Closure,
}

fn inv(p: f64) -> Option<f64> {
    if p > 0.0 {
        Some(1.0 / p)
    } else {
        None
    }
}

fn x_of(payout: u64) -> f64 {
    payout as f64 / CENTS as f64
}

fn band_label(lo: u64, hi: u64) -> String {
    let f = |c: u64| {
        let v = c as f64 / CENTS as f64;
        if v >= 1_000.0 {
            group(v.round() as u64)
        } else if (v - v.round()).abs() < 1e-9 {
            format!("{}", v.round() as u64)
        } else {
            format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').replace('.', ",")
        }
    };
    if hi == u64::MAX {
        format!("{}× et plus", f(lo))
    } else {
        format!("{}× – {}×", f(lo), f(hi - 1))
    }
}

pub fn group(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// Quantile conditionnel sur une suite d'issues triées par paiement : plus petit x avec P(X ≤ x) ≥ q.
fn quantile<'a>(atoms: impl Iterator<Item = &'a crate::dist::Atom> + Clone, total: u128, q: f64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    let target = q * total as f64;
    let mut cum: u128 = 0;
    let mut last = None;
    for a in atoms {
        cum += a.weight as u128;
        last = Some(a.payout);
        if cum as f64 >= target * (1.0 - 1e-15) {
            return Some(x_of(a.payout));
        }
    }
    last.map(x_of)
}

pub fn compute(d: &Distribution, opt: &StatsOptions) -> ExactStats {
    let atoms = &d.atoms;
    let w_total: u128 = d.total_weight();
    let wf = w_total as f64;
    let c = d.cost;
    let price_cents = c * CENTS as f64;

    // --- RTP et moments (sommes entières, puis moments centrés en f64) ---
    let s1: u128 = atoms.iter().map(|a| a.weight as u128 * a.payout as u128).sum();
    let mean = s1 as f64 / wf / CENTS as f64;
    let rtp = mean / c;
    let (mut m2, mut m3, mut m4) = (0.0f64, 0.0f64, 0.0f64);
    let mut sum_w2: f64 = 0.0;
    let mut heaviest: u64 = 0;
    for a in atoms {
        let p = a.weight as f64 / wf;
        let dlt = x_of(a.payout) - mean;
        let d2 = dlt * dlt;
        m2 += p * d2;
        m3 += p * d2 * dlt;
        m4 += p * d2 * d2;
        sum_w2 += (a.weight as f64) * (a.weight as f64);
        heaviest = heaviest.max(a.weight);
    }
    let variance = m2;
    let sd = variance.sqrt();
    let (skewness, excess_kurtosis) = if sd > 0.0 { (m3 / (sd * sd * sd), m4 / (variance * variance) - 3.0) } else { (0.0, 0.0) };

    let w_win: u128 = atoms.iter().filter(|a| a.payout > 0).map(|a| a.weight as u128).sum();
    let hit_rate = w_win as f64 / wf;
    let w_ge_price: u128 = atoms.iter().filter(|a| a.payout as f64 >= price_cents - 1e-9).map(|a| a.weight as u128).sum();

    // Suffixes : poids et paiements des issues ≥ i (pour les seuils et les queues).
    let n = atoms.len();
    let mut suf_w = vec![0u128; n + 1];
    let mut suf_s = vec![0u128; n + 1];
    for i in (0..n).rev() {
        suf_w[i] = suf_w[i + 1] + atoms[i].weight as u128;
        suf_s[i] = suf_s[i + 1] + atoms[i].weight as u128 * atoms[i].payout as u128;
    }
    let first_ge = |cents: f64| -> usize { atoms.partition_point(|a| (a.payout as f64) < cents - 1e-9) };
    let first_gt = |cents: f64| -> usize { atoms.partition_point(|a| (a.payout as f64) <= cents + 1e-9) };
    let p_ge = |cents: f64| -> f64 { suf_w[first_ge(cents)] as f64 / wf };

    let max_payout = d.max_payout();
    let max_x = x_of(max_payout);

    // --- Percentiles ---
    let percentiles = PERCENTILES
        .iter()
        .map(|&q| {
            let v = quantile(atoms.iter(), w_total, q).unwrap_or(0.0);
            Percentile { q, value: v, value_price: v / c }
        })
        .collect::<Vec<_>>();
    let median = quantile(atoms.iter(), w_total, 0.5).unwrap_or(0.0);

    // --- Seuils en multiples du prix ---
    let thresholds = PRICE_LADDER
        .iter()
        .filter(|&&k| k * c <= max_x * (1.0 + 1e-12))
        .map(|&k| {
            let p = p_ge(k * price_cents);
            Threshold {
                mult_price: k,
                prob: p,
                one_in: inv(p),
                median_wait: if p > 0.0 { Some(median_wait(p)) } else { None },
                at_least_once_100: at_least_once(p, 100.0),
                at_least_once_1000: at_least_once(p, 1_000.0),
                at_least_once_10000: at_least_once(p, 10_000.0),
                spend_per_event: inv(p).map(|v| v * c),
            }
        })
        .collect::<Vec<_>>();

    // --- Bandes (en mises de base) + ligne du maximum ---
    let mut bands = Vec::new();
    let mut band_s_total: u128 = 0;
    {
        let zero_w: u128 = atoms.iter().filter(|a| a.payout == 0).map(|a| a.weight as u128).sum();
        let zero_n = atoms.iter().filter(|a| a.payout == 0 && a.weight > 0).count() as u64;
        let zero_b: u64 = atoms.iter().filter(|a| a.payout == 0).map(|a| a.books as u64).sum();
        let p = zero_w as f64 / wf;
        bands.push(Band {
            label: "0 (pas de gain)".into(),
            lo: 0.0,
            hi: Some(0.0),
            is_zero: true,
            is_max: false,
            prob: p,
            one_in: inv(p),
            per_10000: p * 10_000.0,
            rtp: 0.0,
            mean: None,
            survival: 1.0,
            outcomes: zero_n,
            books: zero_b,
        });
        for win in BAND_EDGES.windows(2) {
            let (lo, hi) = (win[0], win[1]);
            if lo > max_payout {
                break;
            }
            let sel = atoms.iter().filter(|a| a.weight > 0 && a.payout >= lo && a.payout < hi && a.payout > 0 && a.payout != max_payout);
            let (mut w, mut s, mut k, mut b) = (0u128, 0u128, 0u64, 0u64);
            for a in sel {
                w += a.weight as u128;
                s += a.weight as u128 * a.payout as u128;
                k += 1;
                b += a.books as u64;
            }
            band_s_total += s;
            let p = w as f64 / wf;
            bands.push(Band {
                label: band_label(lo, hi),
                lo: x_of(lo),
                hi: if hi == u64::MAX { None } else { Some(x_of(hi)) },
                is_zero: false,
                is_max: false,
                prob: p,
                one_in: inv(p),
                per_10000: p * 10_000.0,
                rtp: s as f64 / wf / CENTS as f64 / c,
                mean: if w > 0 { Some(s as f64 / w as f64 / CENTS as f64) } else { None },
                survival: p_ge(lo as f64),
                outcomes: k,
                books: b,
            });
        }
        if max_payout > 0 {
            let (mut w, mut k, mut b) = (0u128, 0u64, 0u64);
            for a in atoms.iter().filter(|a| a.payout == max_payout && a.weight > 0) {
                w += a.weight as u128;
                k += 1;
                b += a.books as u64;
            }
            let s = w * max_payout as u128;
            band_s_total += s;
            let p = w as f64 / wf;
            bands.push(Band {
                label: format!("{}× (max)", group(max_x.round() as u64)),
                lo: max_x,
                hi: None,
                is_zero: false,
                is_max: true,
                prob: p,
                one_in: inv(p),
                per_10000: p * 10_000.0,
                rtp: s as f64 / wf / CENTS as f64 / c,
                mean: Some(max_x),
                survival: p,
                outcomes: k,
                books: b,
            });
        }
    }
    let gaps = {
        let first = bands.iter().position(|b| !b.is_zero && b.prob > 0.0);
        match first {
            // Seules les bandes qui commencent sous le maximum peuvent être des « trous ».
            Some(f) => bands[f..].iter().filter(|b| !b.is_max && b.prob == 0.0 && b.lo < max_x - 1e-9).map(|b| b.label.clone()).collect(),
            None => vec![],
        }
    };

    // --- Classes de résultat (probabilités et stats conditionnelles) ---
    let mut class_s_total: u128 = 0;
    let (mut s_bonus, mut s_base): (u128, u128) = (0, 0);
    let classes = d
        .classes
        .iter()
        .enumerate()
        .map(|(ci, def)| {
            let sel = || atoms.iter().filter(move |a| a.class as usize == ci && a.weight > 0);
            let (mut w, mut s, mut k, mut b, mut w0, mut wmax) = (0u128, 0u128, 0u64, 0u64, 0u128, 0u128);
            let mut mx: Option<u64> = None;
            for a in sel() {
                w += a.weight as u128;
                s += a.weight as u128 * a.payout as u128;
                k += 1;
                b += a.books as u64;
                if a.payout == 0 {
                    w0 += a.weight as u128;
                }
                if a.payout == max_payout {
                    wmax += a.weight as u128;
                }
                mx = Some(mx.map_or(a.payout, |m: u64| m.max(a.payout)));
            }
            class_s_total += s;
            if def.kind.is_bonus() {
                s_bonus += s;
            } else {
                s_base += s;
            }
            let p = w as f64 / wf;
            let r = s as f64 / wf / CENTS as f64 / c;
            ClassStat {
                key: def.key.clone(),
                label: def.label.clone(),
                kind: def.kind,
                prob: p,
                one_in: inv(p),
                rtp: r,
                share_of_rtp: if rtp > 0.0 { r / rtp } else { 0.0 },
                mean: if w > 0 { Some(s as f64 / w as f64 / CENTS as f64) } else { None },
                median: quantile(sel(), w, 0.5),
                p10: quantile(sel(), w, 0.10),
                p90: quantile(sel(), w, 0.90),
                p_no_win: if w > 0 { Some(w0 as f64 / w as f64) } else { None },
                max: mx.map(x_of),
                max_win_one_in: if wmax > 0 { Some(w as f64 / wmax as f64) } else { None },
                outcomes: k,
                books: b,
            }
        })
        .collect::<Vec<_>>();

    // --- Catégories petits / moyens / gros / très gros / max (multiples du prix) ---
    let cat = &opt.categories;
    let mut cat_s_total: u128 = 0;
    let mut categories = Vec::new();
    {
        let mut push = |key: &str, label: String, f: &dyn Fn(u64) -> bool| {
            let (mut w, mut s) = (0u128, 0u128);
            for a in atoms.iter().filter(|a| f(a.payout)) {
                w += a.weight as u128;
                s += a.weight as u128 * a.payout as u128;
            }
            cat_s_total += s;
            let p = w as f64 / wf;
            categories.push(Category {
                key: key.into(),
                label,
                prob: p,
                one_in: inv(p),
                rtp: s as f64 / wf / CENTS as f64 / c,
                mean: if w > 0 { Some(s as f64 / w as f64 / CENTS as f64) } else { None },
            });
        };
        let pc = price_cents;
        let (a1, a2, a3) = (cat.small_below * pc, cat.medium_below * pc, cat.big_below * pc);
        push("lose", "Spins perdants (0)".into(), &|x| x == 0);
        push("small", format!("Petits gains (0 < gain < {}× prix)", cat.small_below), &|x| x > 0 && (x as f64) < a1 && x != max_payout);
        push("medium", format!("Gains moyens ({}× – {}× prix)", cat.small_below, cat.medium_below), &|x| (x as f64) >= a1 && (x as f64) < a2 && x != max_payout);
        push("big", format!("Gros gains ({}× – {}× prix)", cat.medium_below, cat.big_below), &|x| (x as f64) >= a2 && (x as f64) < a3 && x != max_payout);
        push("huge", format!("Très gros gains (≥ {}× prix, hors max)", cat.big_below), &|x| (x as f64) >= a3 && x != max_payout);
        push("max", "Gain maximal".into(), &|x| x == max_payout && x > 0);
    }

    // --- Concentration du retour ---
    let concentration = CONCENTRATION
        .iter()
        .map(|&f| {
            let target = f * wf;
            let (mut acc_w, mut acc_s) = (0.0f64, 0.0f64);
            for a in atoms.iter().rev() {
                if acc_w >= target {
                    break;
                }
                let take = (a.weight as f64).min(target - acc_w);
                acc_w += take;
                acc_s += take * a.payout as f64;
            }
            (f, if s1 > 0 { acc_s / s1 as f64 } else { 0.0 })
        })
        .collect::<Vec<_>>();

    // CVaR 0,1 % : moyenne des 0,1 % meilleurs rounds (mises de base).
    let cvar_001 = {
        let target = 0.001 * wf;
        let (mut acc_w, mut acc_s) = (0.0f64, 0.0f64);
        for a in atoms.iter().rev() {
            if acc_w >= target {
                break;
            }
            let take = (a.weight as f64).min(target - acc_w);
            acc_w += take;
            acc_s += take * a.payout as f64;
        }
        if acc_w > 0.0 {
            acc_s / acc_w / CENTS as f64
        } else {
            0.0
        }
    };
    let etl = |threshold_cents: f64| suf_s[first_gt(threshold_cents)] as f64 / wf / CENTS as f64 / c;
    let etl_40 = etl(40.0 * price_cents);
    let etl_10000 = etl(10_000.0 * CENTS as f64);

    // --- Max win ---
    let (w_max, k_max, b_max) = atoms.iter().filter(|a| a.payout == max_payout && a.weight > 0).fold((0u128, 0u64, 0u64), |acc, a| {
        (acc.0 + a.weight as u128, acc.1 + 1, acc.2 + a.books as u64)
    });
    let p_max = w_max as f64 / wf;
    let max_win = MaxWin {
        value: max_x,
        value_price: max_x / c,
        prob: p_max,
        one_in: inv(p_max),
        rtp: p_max * max_x / c,
        outcomes: k_max,
        books: b_max,
        cap: d.cap.map(x_of),
        reaches_cap: d.cap.map(|cap| max_payout == cap),
        median_wait: if p_max > 0.0 { Some(median_wait(p_max)) } else { None },
        spend_per_max: inv(p_max).map(|v| v * c),
    };

    // --- Bonus (seulement si la loi distingue des classes de bonus) ---
    let bonus = if d.has_bonus_classes() {
        let sel = || atoms.iter().filter(|a| d.classes[a.class as usize].kind.is_bonus());
        let wb: u128 = sel().map(|a| a.weight as u128).sum();
        let p = wb as f64 / wf;
        let rb = s_bonus as f64 / wf / CENTS as f64 / c;
        let rbase = s_base as f64 / wf / CENTS as f64 / c;
        Some(BonusStats {
            prob: p,
            one_in: inv(p),
            mean: if wb > 0 { Some(s_bonus as f64 / wb as f64 / CENTS as f64) } else { None },
            median: quantile(sel(), wb, 0.5),
            p10: quantile(sel(), wb, 0.1),
            p90: quantile(sel(), wb, 0.9),
            rtp_bonus: rb,
            rtp_base: rbase,
            bonus_share: if rtp > 0.0 { rb / rtp } else { 0.0 },
            median_wait: if p > 0.0 { Some(median_wait(p)) } else { None },
        })
    } else {
        None
    };

    // --- Barèmes Stake ---
    let tail: Vec<(f64, f64)> = STAKE_TAIL.iter().map(|&t| (t, p_ge(t * CENTS as f64))).collect();
    let inputs = StakeInputs { cost: c, max_payout: max_x, sd_per_price: sd / c, tail, cvar_absolute: cvar_001, etl_40, etl_10000 };
    let stake_checks = opt.scales.iter().flat_map(|s| stake::check(s, &inputs)).collect();

    // --- Grille 0,1× ---
    let off_grid: u128 = atoms.iter().filter(|a| a.payout % 10 != 0).map(|a| a.weight as u128).sum();

    // --- Clôture ---
    let conv = |s: u128| s as f64 / wf / CENTS as f64 / c;
    let by = [conv(s1), conv(class_s_total), conv(band_s_total), conv(s_base + s_bonus), conv(cat_s_total)];
    let max_abs_diff = by.iter().map(|v| (v - by[0]).abs()).fold(0.0, f64::max);
    let closure = Closure {
        by_outcomes: by[0],
        by_classes: by[1],
        by_bands: by[2],
        by_base_plus_bonus: by[3],
        by_categories: by[4],
        max_abs_diff,
    };

    ExactStats {
        name: d.name.clone(),
        fingerprint: d.fingerprint(),
        cost: c,
        total_weight: w_total.to_string(),
        outcomes: atoms.iter().filter(|a| a.weight > 0).count() as u64,
        books: atoms.iter().map(|a| a.books as u64).sum(),
        rtp,
        mean,
        variance,
        sd,
        sd_per_price: sd / c,
        cv: if mean > 0.0 { Some(sd / mean) } else { None },
        vi90: 1.644_853_626_951_472_2 * sd / c,
        vi95: 1.959_963_984_540_054 * sd / c,
        skewness,
        excess_kurtosis,
        hit_rate,
        hit_one_in: inv(hit_rate),
        zero_rate: 1.0 - hit_rate,
        p_ge_price: w_ge_price as f64 / wf,
        median,
        percentiles,
        thresholds,
        bands,
        gaps,
        classes,
        categories,
        concentration,
        max_win,
        bonus,
        effective_outcomes: wf * wf / sum_w2,
        heaviest_prob: heaviest as f64 / wf,
        cvar_001,
        etl_40,
        etl_10000,
        off_grid_prob: off_grid as f64 / wf,
        stake_checks,
        closure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist::{Atom, ClassDef};

    fn toy() -> Distribution {
        // 0 : 70 % ; 0,5× : 20 % ; 2× : 9 % ; 50× (bonus) : 0,9 % ; 1 000× (max) : 0,1 %.
        let classes = vec![
            ClassDef::new("no_win", "Pas de gain", ClassKind::NoWin),
            ClassDef::new("base", "Gains de base", ClassKind::Base),
            ClassDef::new("bonus", "Bonus", ClassKind::Bonus),
            ClassDef::new("max", "Max win", ClassKind::MaxWin),
        ];
        let atoms = vec![
            Atom { payout: 0, weight: 700, class: 0, books: 1 },
            Atom { payout: 50, weight: 200, class: 1, books: 1 },
            Atom { payout: 200, weight: 90, class: 1, books: 1 },
            Atom { payout: 5_000, weight: 9, class: 2, books: 1 },
            Atom { payout: 100_000, weight: 1, class: 3, books: 1 },
        ];
        Distribution::new("toy", 1.0, Some(100_000), classes, atoms).unwrap()
    }

    #[test]
    fn rtp_and_moments_by_hand() {
        let s = compute(&toy(), &StatsOptions::default());
        // E[X] = 0,2·0,5 + 0,09·2 + 0,009·50 + 0,001·1000 = 0,1 + 0,18 + 0,45 + 1 = 1,73
        assert!((s.mean - 1.73).abs() < 1e-12);
        assert!((s.rtp - 1.73).abs() < 1e-12);
        let e2 = 0.2 * 0.25 + 0.09 * 4.0 + 0.009 * 2500.0 + 0.001 * 1e6;
        assert!((s.variance - (e2 - 1.73 * 1.73)).abs() < 1e-9);
        assert!((s.hit_rate - 0.3).abs() < 1e-15);
        assert_eq!(s.median, 0.0);
        assert!((s.max_win.prob - 0.001).abs() < 1e-15);
        assert!((s.max_win.rtp - 1.0).abs() < 1e-12);
        let b = s.bonus.unwrap();
        assert!((b.prob - 0.01).abs() < 1e-15);
        assert!((b.rtp_bonus - 1.45).abs() < 1e-12);
        assert!((b.bonus_share - 1.45 / 1.73).abs() < 1e-12);
        assert!(s.closure.max_abs_diff < 1e-12);
    }

    #[test]
    fn percentile_convention() {
        let s = compute(&toy(), &StatsOptions::default());
        let get = |q: f64| s.percentiles.iter().find(|p| (p.q - q).abs() < 1e-12).unwrap().value;
        assert_eq!(get(0.5), 0.0); // P(X ≤ 0) = 0,7 ≥ 0,5
        assert_eq!(get(0.75), 0.5); // P(X ≤ 0,5) = 0,9 ≥ 0,75
        assert_eq!(get(0.95), 2.0); // P(X ≤ 2) = 0,99 ≥ 0,95
        assert_eq!(get(0.995), 50.0);
        assert_eq!(get(0.9999), 1000.0);
    }

    #[test]
    fn thresholds_and_bands() {
        let s = compute(&toy(), &StatsOptions::default());
        let t = |k: f64| s.thresholds.iter().find(|t| (t.mult_price - k).abs() < 1e-12).unwrap().prob;
        assert!((t(1.0) - 0.1).abs() < 1e-15);
        assert!((t(50.0) - 0.01).abs() < 1e-15);
        assert!((t(1000.0) - 0.001).abs() < 1e-15);
        let total_p: f64 = s.bands.iter().map(|b| b.prob).sum();
        assert!((total_p - 1.0).abs() < 1e-12);
        let total_rtp: f64 = s.bands.iter().map(|b| b.rtp).sum();
        assert!((total_rtp - s.rtp).abs() < 1e-12);
        assert!(!s.gaps.is_empty(), "des bandes vides entre 50× et 1 000× doivent être signalées");
    }

    #[test]
    fn price_scaling_keeps_shape_statistics() {
        // Même loi vendue 10× : RTP divisé par 10, skewness/kurtosis inchangés (sans dimension).
        let mut d = toy();
        let a = compute(&d, &StatsOptions::default());
        d.cost = 10.0;
        let b = compute(&d, &StatsOptions::default());
        assert!((b.rtp - a.rtp / 10.0).abs() < 1e-12);
        assert!((b.skewness - a.skewness).abs() < 1e-9);
        assert!((b.excess_kurtosis - a.excess_kurtosis).abs() < 1e-6);
        assert!((b.sd_per_price - a.sd_per_price / 10.0).abs() < 1e-12);
    }
}
