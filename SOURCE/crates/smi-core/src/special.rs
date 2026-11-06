//! Fonctions spéciales pour les tests statistiques (loi normale, khi-deux, intervalles de Wilson).
//! Implémentations classiques, testées contre des valeurs de référence.

/// Fonction d'erreur complémentaire (Numerical Recipes, erfc Chebyshev : erreur relative < 1,2e-7).
pub fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07 + t * (-1.135_203_98 + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

/// Fonction de répartition de la loi normale centrée réduite.
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

/// p-valeur bilatérale d'un score z.
pub fn two_sided_p(z: f64) -> f64 {
    (2.0 * (1.0 - normal_cdf(z.abs()))).clamp(0.0, 1.0)
}

/// ln Γ(x) (Lanczos, g = 7, n = 9).
pub fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Fonction gamma incomplète régularisée supérieure Q(a, x) = 1 - P(a, x).
pub fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        // Série pour P.
        let mut sum = 1.0 / a;
        let mut del = sum;
        let mut ap = a;
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        1.0 - sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        // Fraction continue de Lentz pour Q.
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-15 {
                break;
            }
        }
        (-x + a * x.ln() - ln_gamma(a)).exp() * h
    }
}

/// p-valeur d'un khi-deux à `df` degrés de liberté : P(χ² ≥ stat).
pub fn chi2_sf(stat: f64, df: f64) -> f64 {
    gamma_q(df / 2.0, stat / 2.0).clamp(0.0, 1.0)
}

/// Intervalle de Wilson à 95 % pour une proportion observée `k / n`.
pub fn wilson(k: u64, n: u64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let z = 1.959_963_984_540_054;
    let n_f = n as f64;
    let p = k as f64 / n_f;
    let den = 1.0 + z * z / n_f;
    let centre = (p + z * z / (2.0 * n_f)) / den;
    let half = z * ((p * (1.0 - p) / n_f + z * z / (4.0 * n_f * n_f)).sqrt()) / den;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// P(au moins une occurrence en n rounds indépendants) = 1 - (1 - p)^n, calculé sans perte de précision.
pub fn at_least_once(p: f64, n: f64) -> f64 {
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    -((n * (-p).ln_1p()).exp_m1())
}

/// Attente médiane : plus petit n tel que P(au moins une fois en n) ≥ 50 %, soit ⌈ln 0,5 / ln(1 - p)⌉.
pub fn median_wait(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::INFINITY;
    }
    if p >= 0.5 {
        return 1.0;
    }
    (0.5f64.ln() / (-p).ln_1p()).ceil()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_values() {
        assert!((normal_cdf(1.959_964) - 0.975).abs() < 1e-6);
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((two_sided_p(1.959_964) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn chi2_values() {
        // Tables : χ²(0,95 ; 1) = 3,841 ; χ²(0,95 ; 10) = 18,307 ; χ²(0,99 ; 20) = 37,566.
        assert!((chi2_sf(3.841_459, 1.0) - 0.05).abs() < 1e-6);
        assert!((chi2_sf(18.307_04, 10.0) - 0.05).abs() < 1e-6);
        assert!((chi2_sf(37.566_23, 20.0) - 0.01).abs() < 1e-6);
    }

    #[test]
    fn waits_match_witchcraft() {
        // Witchcraft, BASE : bonus 1 sur 175 → attente médiane 121 spins ; win ≥ 100× (1 sur 857) → 594.
        assert_eq!(median_wait(1.0 / 175.0), 121.0);
        assert_eq!(median_wait(1.0 / 857.0), 594.0);
        // « Au moins une fois en 1 000 rounds » pour 1 sur 857 ≈ 68,9 %.
        assert!((at_least_once(1.0 / 857.0, 1000.0) - 0.689).abs() < 5e-4);
    }

    #[test]
    fn wilson_contains_p() {
        let (lo, hi) = wilson(220, 1000);
        assert!(lo < 0.22 && hi > 0.22);
        assert!((hi - lo) < 0.06);
    }
}
