//! Simulateur de spins : tirages au hasard d'une loi, comme le RGS de Stake Engine (un round = une
//! issue tirée avec une probabilité proportionnelle à son poids). Nature : MONTE CARLO.
//!
//! - **Reproductible** : la simulation est découpée en lots de `chunk_size` spins ; le lot `i` utilise
//!   le flux `Xoshiro256pp::stream(seed, i)`. Les compteurs sont des entiers additionnés : le
//!   résultat est identique quel que soit le nombre de threads (vérifié par l'empreinte `counts_hash`).
//! - **Mémoire maîtrisée** : un compteur `u32` par issue et par thread, rien par spin.
//! - **Pilotable** : progression, pause, reprise et annulation via [`Control`].
//! - **Comparaison théorie / simulation** : z du RTP (σ exact), Wilson du hit rate, khi-deux des
//!   bandes, seuils (z ou Poisson), max win (espérance N·p et P(aucune) = (1−p)^N : une absence
//!   n'est jamais une erreur), bonus.

use crate::dist::{Distribution, CENTS};
use crate::rng::Xoshiro256pp;
use crate::special::{chi2_sf, two_sided_p, wilson};
use crate::stats::{compute, ExactStats, StatsOptions};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Plafond d'une simulation (les compteurs par issue sont des u32).
pub const MAX_SPINS: u64 = 4_000_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SimConfig {
    pub spins: u64,
    pub seed: u64,
    pub threads: usize,
    pub chunk_size: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self { spins: 1_000_000, seed: 20_261_106, threads: 4, chunk_size: 1 << 20 }
    }
}

#[derive(Default)]
pub struct Control {
    pub cancel: AtomicBool,
    pub pause: AtomicBool,
    pub done: AtomicU64,
}

/// Table alias de Vose : tirage O(1) d'une issue.
pub struct Alias {
    prob: Vec<f64>,
    alias: Vec<u32>,
}

impl Alias {
    pub fn new(weights: &[u64]) -> Self {
        let n = weights.len();
        let total: f64 = weights.iter().map(|&w| w as f64).sum();
        let mut scaled: Vec<f64> = weights.iter().map(|&w| w as f64 * n as f64 / total).collect();
        let mut prob = vec![0.0; n];
        let mut alias = vec![0u32; n];
        let (mut small, mut large): (Vec<usize>, Vec<usize>) = (0..n).partition(|&i| scaled[i] < 1.0);
        while let (Some(s), Some(&l)) = (small.pop(), large.last()) {
            prob[s] = scaled[s];
            alias[s] = l as u32;
            scaled[l] = (scaled[l] + scaled[s]) - 1.0;
            if scaled[l] < 1.0 {
                large.pop();
                small.push(l);
            }
        }
        for i in large.into_iter().chain(small) {
            prob[i] = 1.0;
            alias[i] = i as u32;
        }
        Self { prob, alias }
    }

    #[inline]
    pub fn sample(&self, rng: &mut Xoshiro256pp) -> usize {
        let i = rng.below(self.prob.len() as u64) as usize;
        if rng.next_f64() < self.prob[i] {
            i
        } else {
            self.alias[i] as usize
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThresholdCheck {
    pub mult_price: f64,
    pub prob: f64,
    pub expected: f64,
    pub observed: u64,
    /// p-valeur bilatérale (z si N·p ≥ 10, Poisson sinon).
    pub p_value: f64,
    /// Probabilité de n'en voir AUCUN en N spins : (1 − p)^N.
    pub p_none: f64,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Chi2 {
    pub stat: f64,
    pub df: f64,
    pub p_value: f64,
    pub bins: Vec<(String, f64, u64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comparison {
    pub rtp_theory: f64,
    pub rtp_sim: f64,
    pub rtp_se: f64,
    pub rtp_ci95: (f64, f64),
    pub rtp_z: f64,
    pub rtp_p_value: f64,
    pub hit_theory: f64,
    pub hit_sim: f64,
    pub hit_ci95: (f64, f64),
    pub hit_z: f64,
    pub chi2: Option<Chi2>,
    pub thresholds: Vec<ThresholdCheck>,
    pub max_win: ThresholdCheck,
    pub bonus_freq: Option<(f64, f64, f64)>,
    pub bonus_mean: Option<(f64, f64, f64)>,
    pub bonus_median: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SimResult {
    pub config: SimConfig,
    pub spins_done: u64,
    pub completed: bool,
    pub reproducible: bool,
    pub elapsed_ms: u64,
    pub spins_per_sec: f64,
    pub fingerprint: String,
    pub counts_hash: String,
    pub biggest_win: f64,
    pub empirical: ExactStats,
    pub comparison: Comparison,
    /// (spins, RTP cumulé) dans l'ordre des lots.
    pub convergence: Vec<(u64, f64)>,
    pub anomalies: Vec<String>,
}

fn poisson_two_sided(k: u64, lambda: f64) -> f64 {
    // P(X ≤ k) et P(X ≥ k) pour X ~ Poisson(λ), sommés terme à terme (k petit dans ce cas d'usage).
    if lambda <= 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    let mut term = (-lambda).exp();
    let mut cdf = 0.0;
    for i in 0..=k {
        if i > 0 {
            term *= lambda / i as f64;
        }
        cdf += term;
    }
    let pmf_k = term;
    let le = cdf.min(1.0);
    let ge = (1.0 - cdf + pmf_k).clamp(0.0, 1.0);
    (2.0 * le.min(ge)).min(1.0)
}

fn fnv(counts: &[u64]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in counts {
        for b in c.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// Exécute une simulation. `progress` est appelé environ toutes les 100 ms avec (spins faits, total).
pub fn run(d: &Distribution, cfg: &SimConfig, ctl: &Control, opt: &StatsOptions, progress: &(dyn Fn(u64, u64) + Sync)) -> SimResult {
    let spins = cfg.spins.clamp(1, MAX_SPINS);
    let chunk = cfg.chunk_size.max(1024);
    let n_chunks = spins.div_ceil(chunk);
    let threads = cfg.threads.clamp(1, 256).min(n_chunks as usize);
    let k = d.atoms.len();
    let weights: Vec<u64> = d.atoms.iter().map(|a| a.weight).collect();
    let payouts: Vec<u64> = d.atoms.iter().map(|a| a.payout).collect();
    let alias = Alias::new(&weights);
    let next = AtomicU64::new(0);
    let chunk_sums: Mutex<Vec<(u64, u64, u64)>> = Mutex::new(Vec::with_capacity(n_chunks as usize));
    let totals: Mutex<Vec<u64>> = Mutex::new(vec![0u64; k]);
    let started = Instant::now();
    let interrupted = AtomicBool::new(false);
    ctl.done.store(0, Ordering::Relaxed);

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut counts = vec![0u32; k];
                loop {
                    let ci = next.fetch_add(1, Ordering::Relaxed);
                    if ci >= n_chunks || ctl.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let len = chunk.min(spins - ci * chunk);
                    let mut rng = Xoshiro256pp::stream(cfg.seed, ci);
                    let mut sum: u64 = 0;
                    let mut j = 0u64;
                    let mut aborted = false;
                    while j < len {
                        let block = (len - j).min(65_536);
                        for _ in 0..block {
                            let idx = alias.sample(&mut rng);
                            counts[idx] += 1;
                            sum += payouts[idx];
                        }
                        j += block;
                        ctl.done.fetch_add(block, Ordering::Relaxed);
                        while ctl.pause.load(Ordering::Relaxed) && !ctl.cancel.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        if ctl.cancel.load(Ordering::Relaxed) {
                            aborted = true;
                            break;
                        }
                    }
                    if aborted {
                        interrupted.store(true, Ordering::Relaxed);
                        chunk_sums.lock().unwrap().push((ci, sum, j));
                        break;
                    }
                    chunk_sums.lock().unwrap().push((ci, sum, len));
                }
                let mut t = totals.lock().unwrap();
                for (a, b) in t.iter_mut().zip(&counts) {
                    *a += *b as u64;
                }
            });
        }
        // Fil de progression.
        scope.spawn(|| loop {
            let done = ctl.done.load(Ordering::Relaxed);
            progress(done.min(spins), spins);
            if done >= spins || ctl.cancel.load(Ordering::Relaxed) || next.load(Ordering::Relaxed) >= n_chunks + threads as u64 {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        });
    });
    let elapsed = started.elapsed();
    let counts = totals.into_inner().unwrap();
    let mut sums = chunk_sums.into_inner().unwrap();
    sums.sort_by_key(|s| s.0);
    let spins_done: u64 = counts.iter().sum();
    let completed = spins_done == spins && !interrupted.load(Ordering::Relaxed);
    progress(spins_done, spins);

    // Convergence : RTP cumulé dans l'ordre des lots (≤ 200 points).
    let mut convergence = Vec::new();
    let (mut acc_s, mut acc_n) = (0u128, 0u64);
    let step = (sums.len() / 200).max(1);
    for (i, (_, s, n)) in sums.iter().enumerate() {
        acc_s += *s as u128;
        acc_n += n;
        if (i + 1) % step == 0 || i + 1 == sums.len() {
            convergence.push((acc_n, acc_s as f64 / acc_n.max(1) as f64 / CENTS as f64 / d.cost));
        }
    }

    let theory = compute(d, opt);
    let empirical = if spins_done > 0 { compute(&d.with_weights(&counts), opt) } else { theory.clone() };
    let comparison = compare(d, &theory, &empirical, &counts, spins_done);
    let biggest_win = counts.iter().zip(&payouts).filter(|(c, _)| **c > 0).map(|(_, p)| *p).max().unwrap_or(0) as f64 / CENTS as f64;

    let mut anomalies = Vec::new();
    if spins_done == 0 {
        anomalies.push("Aucun spin exécuté (simulation annulée avant le premier lot).".into());
    }
    if comparison.rtp_z.abs() > 4.0 {
        anomalies.push(format!("RTP simulé à {:.2} écarts-types de la théorie (|z| > 4)", comparison.rtp_z));
    }
    if comparison.hit_z.abs() > 4.0 {
        anomalies.push(format!("Hit rate simulé à {:.2} écarts-types de la théorie", comparison.hit_z));
    }
    if let Some(c) = &comparison.chi2 {
        if c.p_value < 1e-4 {
            anomalies.push(format!("Khi-deux des bandes : p = {:.2e} (< 1e-4)", c.p_value));
        }
    }
    for t in &comparison.thresholds {
        if t.p_value < 1e-5 {
            anomalies.push(format!("Seuil ≥ {}× prix : {} observés pour {:.1} attendus (p = {:.1e})", t.mult_price, t.observed, t.expected, t.p_value));
        }
    }
    if let Some(cap) = d.cap {
        if biggest_win * CENTS as f64 > cap as f64 + 0.5 {
            anomalies.push("Un gain simulé dépasse le plafond".into());
        }
    }

    SimResult {
        config: SimConfig { spins, chunk_size: chunk, threads, ..cfg.clone() },
        spins_done,
        completed,
        reproducible: completed,
        elapsed_ms: elapsed.as_millis() as u64,
        spins_per_sec: spins_done as f64 / elapsed.as_secs_f64().max(1e-9),
        fingerprint: d.fingerprint(),
        counts_hash: fnv(&counts),
        biggest_win,
        empirical,
        comparison,
        convergence,
        anomalies,
    }
}

fn threshold_check(mult: f64, p: f64, observed: u64, n: u64) -> ThresholdCheck {
    let nf = n as f64;
    let expected = nf * p;
    let p_none = crate::special::at_least_once(p, nf);
    let p_none = 1.0 - p_none;
    let p_value = if expected >= 10.0 && p < 1.0 {
        let z = (observed as f64 - expected) / (nf * p * (1.0 - p)).sqrt();
        two_sided_p(z)
    } else {
        poisson_two_sided(observed, expected)
    };
    let note = if expected < 3.0 {
        format!(
            "Événement rare : {:.2} attendu(s) en {} spins ; P(aucun) = {:.1} %. Une absence ne prouve rien : la probabilité exacte fait foi.",
            expected,
            crate::stats::group(n),
            p_none * 100.0
        )
    } else {
        String::new()
    };
    ThresholdCheck { mult_price: mult, prob: p, expected, observed, p_value, p_none, note }
}

fn compare(d: &Distribution, th: &ExactStats, em: &ExactStats, counts: &[u64], n: u64) -> Comparison {
    let nf = n.max(1) as f64;
    let se = th.sd_per_price / nf.sqrt();
    let rtp_z = if se > 0.0 { (em.rtp - th.rtp) / se } else { 0.0 };
    let hits: u64 = counts.iter().zip(&d.atoms).filter(|(_, a)| a.payout > 0).map(|(c, _)| *c).sum();
    let h = th.hit_rate;
    let hit_z = if h > 0.0 && h < 1.0 { (em.hit_rate - h) / (h * (1.0 - h) / nf).sqrt() } else { 0.0 };

    // Khi-deux sur les bandes de la loi exacte (fusion depuis le haut jusqu'à 5 attendus par case).
    let max_payout = d.max_payout();
    let chi2 = {
        let mut bins: Vec<(String, f64, u64)> = Vec::new();
        for tb in &th.bands {
            if tb.prob <= 0.0 {
                continue;
            }
            let lo = (tb.lo * CENTS as f64).round() as u64;
            let hi = tb.hi.map(|h| (h * CENTS as f64).round() as u64);
            let observed: u64 = counts
                .iter()
                .zip(&d.atoms)
                .filter(|(_, a)| {
                    if tb.is_zero {
                        a.payout == 0
                    } else if tb.is_max {
                        a.payout == max_payout
                    } else {
                        a.payout > 0 && a.payout != max_payout && a.payout >= lo && hi.map_or(true, |h| a.payout < h)
                    }
                })
                .map(|(c, _)| *c)
                .sum();
            bins.push((tb.label.clone(), tb.prob * nf, observed));
        }
        let mut merged: Vec<(String, f64, u64)> = Vec::new();
        let mut carry: Option<(String, f64, u64)> = None;
        for b in bins.into_iter().rev() {
            let cur = match carry.take() {
                Some((l, e, o)) => (format!("{} + {}", b.0, l), b.1 + e, b.2 + o),
                None => b,
            };
            if cur.1 >= 5.0 {
                merged.push(cur);
            } else {
                carry = Some(cur);
            }
        }
        if let Some(c) = carry {
            if let Some(last) = merged.last_mut() {
                last.0 = format!("{} + {}", c.0, last.0);
                last.1 += c.1;
                last.2 += c.2;
            }
        }
        merged.reverse();
        if merged.len() >= 2 {
            let stat: f64 = merged.iter().map(|(_, e, o)| (*o as f64 - e).powi(2) / e).sum();
            let df = (merged.len() - 1) as f64;
            Some(Chi2 { stat, df, p_value: chi2_sf(stat, df), bins: merged })
        } else {
            None
        }
    };

    let thresholds = th
        .thresholds
        .iter()
        .map(|t| {
            let cut = t.mult_price * d.cost * CENTS as f64;
            let obs: u64 = counts.iter().zip(&d.atoms).filter(|(_, a)| a.payout as f64 >= cut - 1e-9).map(|(c, _)| *c).sum();
            threshold_check(t.mult_price, t.prob, obs, n)
        })
        .collect();
    let obs_max: u64 = counts.iter().zip(&d.atoms).filter(|(_, a)| a.payout == max_payout).map(|(c, _)| *c).sum();
    let max_win = threshold_check(th.max_win.value_price, th.max_win.prob, obs_max, n);

    let (bonus_freq, bonus_mean, bonus_median) = match (&th.bonus, &em.bonus) {
        (Some(tb), Some(eb)) => {
            let fz = if tb.prob > 0.0 && tb.prob < 1.0 { (eb.prob - tb.prob) / (tb.prob * (1.0 - tb.prob) / nf).sqrt() } else { 0.0 };
            // σ conditionnel exact des gains de bonus, pour le z de la moyenne.
            let (mut w, mut s1, mut s2) = (0f64, 0f64, 0f64);
            for a in d.atoms.iter().filter(|a| d.classes[a.class as usize].kind.is_bonus()) {
                let x = a.payout as f64 / CENTS as f64;
                w += a.weight as f64;
                s1 += a.weight as f64 * x;
                s2 += a.weight as f64 * x * x;
            }
            let var = if w > 0.0 { s2 / w - (s1 / w).powi(2) } else { 0.0 };
            let nb = eb.prob * nf;
            let mz = match (tb.mean, eb.mean) {
                (Some(t), Some(e)) if nb > 0.0 && var > 0.0 => Some((t, e, (e - t) / (var / nb).sqrt())),
                _ => None,
            };
            let md = match (tb.median, eb.median) {
                (Some(t), Some(e)) => Some((t, e)),
                _ => None,
            };
            (Some((tb.prob, eb.prob, fz)), mz, md)
        }
        _ => (None, None, None),
    };

    Comparison {
        rtp_theory: th.rtp,
        rtp_sim: em.rtp,
        rtp_se: se,
        rtp_ci95: (em.rtp - 1.959_964 * se, em.rtp + 1.959_964 * se),
        rtp_z,
        rtp_p_value: two_sided_p(rtp_z),
        hit_theory: h,
        hit_sim: em.hit_rate,
        hit_ci95: wilson(hits, n),
        hit_z,
        chi2,
        thresholds,
        max_win,
        bonus_freq,
        bonus_mean,
        bonus_median,
    }
}

/// Contrôle de reproductibilité : même graine, 1 puis 3 threads, compteurs identiques attendus.
pub fn reproducibility(d: &Distribution, seed: u64, spins: u64) -> crate::validate::Reproducibility {
    let opt = StatsOptions { scales: vec![], ..Default::default() };
    let cfg = SimConfig { spins, seed, threads: 1, chunk_size: (spins / 8).max(1024) };
    let a = run(d, &cfg, &Control::default(), &opt, &|_, _| {});
    let b = run(d, &SimConfig { threads: 3, ..cfg.clone() }, &Control::default(), &opt, &|_, _| {});
    crate::validate::Reproducibility { seed, spins, hash_a: a.counts_hash, hash_b: b.counts_hash, threads_a: 1, threads_b: b.config.threads }
}

// ---------------------------------------------------------------------------------------------
// Sessions (Monte Carlo) : retour d'une session de N rounds et survie d'une bankroll.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionConfig {
    pub rounds: u64,
    pub sessions: u64,
    pub seed: u64,
    pub threads: usize,
    /// Bankroll de départ en multiples du prix : la session s'arrête quand on ne peut plus payer un round.
    pub bankroll: Option<f64>,
    /// « Gros gain » en multiples du prix, pour le contrôle exact / simulé.
    pub big_win: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionResult {
    pub config: SessionConfig,
    pub mean_return: f64,
    pub mean_ci95: (f64, f64),
    /// (q, valeur, IC bas, IC haut) — IC par statistiques d'ordre.
    pub percentiles: Vec<(f64, f64, f64, f64)>,
    pub p_profit: f64,
    pub p_profit_ci95: (f64, f64),
    pub p_below_half: f64,
    pub big_win_exact: f64,
    pub big_win_sim: f64,
    pub big_win_z: f64,
    /// (rounds joués k, part des joueurs encore en jeu) si bankroll.
    pub survival: Vec<(u64, f64)>,
    pub elapsed_ms: u64,
}

pub fn run_sessions(d: &Distribution, cfg: &SessionConfig, ctl: &Control) -> SessionResult {
    let started = Instant::now();
    let weights: Vec<u64> = d.atoms.iter().map(|a| a.weight).collect();
    let alias = Alias::new(&weights);
    let price_c = d.cost * CENTS as f64;
    let big_c = cfg.big_win * price_c;
    let n = cfg.sessions.max(1);
    let rounds = cfg.rounds.max(1);
    let returns: Mutex<Vec<(u64, f32, bool, u64)>> = Mutex::new(Vec::with_capacity(n as usize));
    let next = AtomicU64::new(0);
    let threads = cfg.threads.clamp(1, 256);
    ctl.done.store(0, Ordering::Relaxed);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n || ctl.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut rng = Xoshiro256pp::stream(cfg.seed ^ 0x5E55_1011, i);
                    let mut won: f64 = 0.0;
                    let mut big = false;
                    let mut played = 0u64;
                    let mut balance = cfg.bankroll.map(|b| b * price_c);
                    for _ in 0..rounds {
                        if let Some(bal) = balance {
                            if bal + 1e-9 < price_c {
                                break;
                            }
                        }
                        let x = d.atoms[alias.sample(&mut rng)].payout as f64;
                        won += x;
                        played += 1;
                        if x >= big_c - 1e-9 {
                            big = true;
                        }
                        if let Some(bal) = balance.as_mut() {
                            *bal += x - price_c;
                        }
                    }
                    let r = if played > 0 { won / (played as f64 * price_c) } else { 0.0 };
                    local.push((i, r as f32, big, played));
                    ctl.done.fetch_add(1, Ordering::Relaxed);
                }
                returns.lock().unwrap().extend(local);
            });
        }
    });
    let mut v = returns.into_inner().unwrap();
    v.sort_by_key(|r| r.0);
    let m = v.len().max(1) as f64;
    let mean: f64 = v.iter().map(|r| r.1 as f64).sum::<f64>() / m;
    let var: f64 = v.iter().map(|r| (r.1 as f64 - mean).powi(2)).sum::<f64>() / (m - 1.0).max(1.0);
    let half = 1.959_964 * (var / m).sqrt();
    let profit = v.iter().filter(|r| r.1 >= 1.0).count() as u64;
    let below = v.iter().filter(|r| r.1 < 0.5).count() as f64 / m;
    let big = v.iter().filter(|r| r.2).count() as f64 / m;

    // Probabilité exacte d'au moins un gros gain en N rounds (sans bankroll : N fixe).
    let p_big = {
        let w: u128 = d.total_weight();
        let wb: u128 = d.atoms.iter().filter(|a| a.payout as f64 >= big_c - 1e-9).map(|a| a.weight as u128).sum();
        wb as f64 / w as f64
    };
    let exact_big = crate::special::at_least_once(p_big, rounds as f64);
    let big_z = if exact_big > 0.0 && exact_big < 1.0 && cfg.bankroll.is_none() {
        (big - exact_big) / (exact_big * (1.0 - exact_big) / m).sqrt()
    } else {
        0.0
    };

    let mut sorted: Vec<f32> = v.iter().map(|r| r.1).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |rank: f64| sorted[(rank.round().max(0.0) as usize).min(sorted.len().saturating_sub(1))] as f64;
    let percentiles = [0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95]
        .iter()
        .filter(|_| !sorted.is_empty())
        .map(|&q| {
            let c = q * m;
            let h = 1.959_964 * (m * q * (1.0 - q)).sqrt();
            (q, at(c - 1.0), at(c - h - 1.0), at(c + h - 1.0))
        })
        .collect();

    let survival = if cfg.bankroll.is_some() {
        let marks: Vec<u64> = [1u64, 10, 25, 50, 100, 200, 300, 500, 750, 1_000, 2_000, 5_000, 10_000]
            .into_iter()
            .filter(|&k| k <= rounds)
            .collect();
        marks.iter().map(|&k| (k, v.iter().filter(|r| r.3 >= k).count() as f64 / m)).collect()
    } else {
        vec![]
    };

    SessionResult {
        config: cfg.clone(),
        mean_return: mean,
        mean_ci95: (mean - half, mean + half),
        percentiles,
        p_profit: profit as f64 / m,
        p_profit_ci95: wilson(profit, v.len() as u64),
        p_below_half: below,
        big_win_exact: exact_big,
        big_win_sim: big,
        big_win_z: big_z,
        survival,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist() -> Distribution {
        Distribution::from_pairs("t", 1.0, Some(100_000), [(0, 7_000), (50, 2_000), (200, 900), (5_000, 99), (100_000, 1)]).unwrap()
    }

    #[test]
    fn deterministic_across_thread_counts() {
        let d = dist();
        let opt = StatsOptions::default();
        let base = SimConfig { spins: 300_000, seed: 99, threads: 1, chunk_size: 50_000 };
        let a = run(&d, &base, &Control::default(), &opt, &|_, _| {});
        let b = run(&d, &SimConfig { threads: 5, ..base.clone() }, &Control::default(), &opt, &|_, _| {});
        assert_eq!(a.counts_hash, b.counts_hash);
        assert_eq!(a.empirical.rtp, b.empirical.rtp);
        assert!(a.completed && b.completed);
        let c = run(&d, &SimConfig { seed: 100, ..base }, &Control::default(), &opt, &|_, _| {});
        assert_ne!(a.counts_hash, c.counts_hash);
    }

    #[test]
    fn converges_to_theory() {
        let d = dist();
        let r = run(&d, &SimConfig { spins: 2_000_000, seed: 7, threads: 4, chunk_size: 1 << 18 }, &Control::default(), &StatsOptions::default(), &|_, _| {});
        assert!(r.comparison.rtp_z.abs() < 4.5, "z = {}", r.comparison.rtp_z);
        assert!(r.comparison.hit_z.abs() < 4.5);
        assert!(r.comparison.chi2.as_ref().unwrap().p_value > 1e-5);
        assert!(r.anomalies.is_empty(), "{:?}", r.anomalies);
        assert_eq!(r.spins_done, 2_000_000);
    }

    #[test]
    fn alias_matches_weights() {
        let w = [1u64, 2, 3, 4];
        let a = Alias::new(&w);
        let mut rng = Xoshiro256pp::seed_from_u64(1);
        let mut c = [0u64; 4];
        for _ in 0..1_000_000 {
            c[a.sample(&mut rng)] += 1;
        }
        for (i, &k) in c.iter().enumerate() {
            let e = 1_000_000.0 * w[i] as f64 / 10.0;
            assert!((k as f64 - e).abs() < 5.0 * e.sqrt(), "{i}: {k} vs {e}");
        }
    }

    #[test]
    fn cancel_stops_early() {
        let d = dist();
        let ctl = Control::default();
        ctl.cancel.store(true, Ordering::Relaxed);
        let r = run(&d, &SimConfig { spins: 10_000_000, ..Default::default() }, &ctl, &StatsOptions::default(), &|_, _| {});
        assert!(!r.completed);
        assert!(r.spins_done < 10_000_000);
    }

    #[test]
    fn sessions_match_exact_big_win() {
        let d = dist();
        let cfg = SessionConfig { rounds: 200, sessions: 40_000, seed: 3, threads: 4, bankroll: None, big_win: 50.0 };
        let r = run_sessions(&d, &cfg, &Control::default());
        assert!(r.big_win_z.abs() < 4.0, "z = {}", r.big_win_z);
        // RTP de la loi de test : 0,875.
        assert!((r.mean_return - 0.875).abs() < 0.05, "{}", r.mean_return);
    }
}
