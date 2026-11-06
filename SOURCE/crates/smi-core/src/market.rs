//! Statistiques de marché : jamais une moyenne aveugle.
//!
//! Pour une métrique sur un ensemble de modes, on donne TOUJOURS ensemble : n, moyenne arithmétique,
//! médiane, moyenne pondérée par les bets (si connus), moyenne pondérée par le turnover (si connu),
//! écart-type, min / max, quartiles, P5 / P10 / P90 / P95, valeurs extrêmes (règle de Tukey,
//! 1,5 × IQR) et un drapeau « échantillon insuffisant » sous `min_n`.
//!
//! Quantiles : interpolation linéaire entre statistiques d'ordre (type 7 de Hyndman & Fan, celui de
//! R et de NumPy par défaut).
//!
//! Ces chiffres décrivent un ÉCHANTILLON DE JEUX ; ils ne sont jamais des probabilités d'un jeu, et
//! les volumes commerciaux (bets, turnover sur 30 jours) ne sont jamais confondus avec le RTP.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aggregate {
    pub n: usize,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub sd: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub p5: Option<f64>,
    pub p10: Option<f64>,
    pub q1: Option<f64>,
    pub q3: Option<f64>,
    pub p90: Option<f64>,
    pub p95: Option<f64>,
    pub mean_weighted_bets: Option<f64>,
    pub n_with_bets: usize,
    pub mean_weighted_turnover: Option<f64>,
    pub n_with_turnover: usize,
    /// Indices (dans l'entrée) des valeurs hors des barrières de Tukey.
    pub outliers: Vec<usize>,
    pub insufficient: bool,
    /// Histogramme : (borne basse, borne haute, effectif).
    pub histogram: Vec<(f64, f64, usize)>,
    pub log_scale: bool,
}

/// Quantile type 7 sur des valeurs TRIÉES.
pub fn quantile_sorted(v: &[f64], q: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let h = (v.len() - 1) as f64 * q.clamp(0.0, 1.0);
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    Some(v[lo] + (h - lo as f64) * (v[hi] - v[lo]))
}

fn weighted_mean(values: &[f64], weights: &[Option<f64>]) -> (Option<f64>, usize) {
    let mut num = 0.0;
    let mut den = 0.0;
    let mut n = 0;
    for (v, w) in values.iter().zip(weights) {
        if let Some(w) = w {
            if w.is_finite() && *w > 0.0 && v.is_finite() {
                num += v * w;
                den += w;
                n += 1;
            }
        }
    }
    (if den > 0.0 { Some(num / den) } else { None }, n)
}

/// Agrège une métrique. `bets` / `turnover` : poids facultatifs alignés sur `values`.
pub fn aggregate(values: &[f64], bets: &[Option<f64>], turnover: &[Option<f64>], min_n: usize, bins: usize, log_scale: bool) -> Aggregate {
    let idx: Vec<usize> = (0..values.len()).filter(|&i| values[i].is_finite()).collect();
    let mut v: Vec<f64> = idx.iter().map(|&i| values[i]).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let mean = if n > 0 { Some(v.iter().sum::<f64>() / n as f64) } else { None };
    let sd = match (mean, n) {
        (Some(m), n) if n > 1 => Some((v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()),
        _ => None,
    };
    let q = |p: f64| quantile_sorted(&v, p);
    let (q1, q3) = (q(0.25), q(0.75));
    let outliers = match (q1, q3) {
        (Some(a), Some(b)) if n >= 4 => {
            let iqr = b - a;
            let (lo, hi) = (a - 1.5 * iqr, b + 1.5 * iqr);
            idx.iter().copied().filter(|&i| values[i] < lo || values[i] > hi).collect()
        }
        _ => vec![],
    };
    let fv: Vec<f64> = idx.iter().map(|&i| values[i]).collect();
    let fb: Vec<Option<f64>> = idx.iter().map(|&i| bets.get(i).copied().flatten()).collect();
    let ft: Vec<Option<f64>> = idx.iter().map(|&i| turnover.get(i).copied().flatten()).collect();
    let (mb, nb) = weighted_mean(&fv, &fb);
    let (mt, nt) = weighted_mean(&fv, &ft);

    let histogram = if n > 0 && bins > 0 {
        let use_log = log_scale && v[0] > 0.0;
        let tr = |x: f64| if use_log { x.log10() } else { x };
        let inv = |x: f64| if use_log { 10f64.powf(x) } else { x };
        let (a, b) = (tr(v[0]), tr(v[n - 1]));
        let width = if b > a { (b - a) / bins as f64 } else { 1.0 };
        let mut h: Vec<(f64, f64, usize)> = (0..bins).map(|k| (inv(a + k as f64 * width), inv(a + (k + 1) as f64 * width), 0)).collect();
        for x in &v {
            let k = (((tr(*x) - a) / width).floor() as usize).min(bins - 1);
            h[k].2 += 1;
        }
        h
    } else {
        vec![]
    };

    Aggregate {
        n,
        mean,
        median: q(0.5),
        sd,
        min: v.first().copied(),
        max: v.last().copied(),
        p5: q(0.05),
        p10: q(0.10),
        q1,
        q3,
        p90: q(0.90),
        p95: q(0.95),
        mean_weighted_bets: mb,
        n_with_bets: nb,
        mean_weighted_turnover: mt,
        n_with_turnover: nt,
        outliers,
        insufficient: n < min_n,
        histogram,
        log_scale,
    }
}

/// Deux plafonds sont « comparables » s'ils sont dans un rapport ≤ `factor` (2 par défaut : un
/// ×5 000 se compare aux ×2 500 – ×10 000).
pub fn comparable_max(target: f64, other: f64, factor: f64) -> bool {
    target > 0.0 && other > 0.0 && other >= target / factor && other <= target * factor
}

/// Position d'une valeur dans un échantillon (rang centile, 0 à 100).
pub fn percentile_rank(sorted: &[f64], x: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let below = sorted.iter().filter(|&&v| v < x).count() as f64;
    let equal = sorted.iter().filter(|&&v| v == x).count() as f64;
    Some(100.0 * (below + 0.5 * equal) / sorted.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_type7() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile_sorted(&v, 0.5), Some(2.5));
        assert_eq!(quantile_sorted(&v, 0.25), Some(1.75));
        assert_eq!(quantile_sorted(&v, 1.0), Some(4.0));
    }

    #[test]
    fn weighted_means_and_outliers() {
        let values = [1.0, 2.0, 3.0, 4.0, 100.0];
        let bets = [Some(1.0), Some(1.0), Some(1.0), Some(1.0), Some(96.0)];
        let a = aggregate(&values, &bets, &[None; 5], 10, 4, false);
        assert_eq!(a.n, 5);
        assert_eq!(a.median, Some(3.0));
        assert!((a.mean.unwrap() - 22.0).abs() < 1e-12);
        assert!((a.mean_weighted_bets.unwrap() - 96.1).abs() < 1e-9);
        assert_eq!(a.mean_weighted_turnover, None);
        assert_eq!(a.outliers, vec![4]);
        assert!(a.insufficient);
        assert_eq!(a.histogram.iter().map(|h| h.2).sum::<usize>(), 5);
    }

    #[test]
    fn comparable() {
        assert!(comparable_max(5_000.0, 10_000.0, 2.0));
        assert!(!comparable_max(5_000.0, 10_001.0, 2.0));
    }
}
