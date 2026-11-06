//! Classification de volatilité reproductible (Low → Very High), jamais « à l'impression ».
//!
//! Score composite dans [0, 1] = moyenne pondérée de 5 indicateurs ramenés sur [0, 1] entre deux
//! ancres FIXES (indépendantes de l'échantillon de marché, donc stables d'une collecte à l'autre) :
//!
//! | indicateur | sens | ancres par défaut |
//! |---|---|---|
//! | écart-type / prix (échelle log) | ↑ = plus volatil | 3 → 100 |
//! | fréquence des pertes P(X = 0) | ↑ | 0,50 → 0,97 |
//! | part du RTP venue des gains ≥ 100× le prix | ↑ | 0,10 → 0,85 |
//! | part du retour portée par le top 1 % des rounds | ↑ | 0,40 → 0,98 |
//! | P(gain ≥ 1 000× le prix), échelle log, sens inverse : plus c'est rare, plus c'est volatil | ↑ | 1e-3 → 1e-7 |
//!
//! Les ancres, les poids et les coupures de classes sont dans les réglages (`VolatilityConfig`) et
//! documentés dans `PROJECT/DECISIONS.md` (D-07). Calibration de départ : instantané du marché du
//! 2026-11-03 (133 modes de base : σ/prix médian 36, quartiles 29–47).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VolClass {
    #[serde(rename = "Low")]
    Low,
    #[serde(rename = "Medium-Low")]
    MediumLow,
    #[serde(rename = "Medium")]
    Medium,
    #[serde(rename = "Medium-High")]
    MediumHigh,
    #[serde(rename = "High")]
    High,
    #[serde(rename = "Very High")]
    VeryHigh,
}

impl VolClass {
    pub const ALL: [VolClass; 6] = [VolClass::Low, VolClass::MediumLow, VolClass::Medium, VolClass::MediumHigh, VolClass::High, VolClass::VeryHigh];

    pub fn label(self) -> &'static str {
        match self {
            VolClass::Low => "Low",
            VolClass::MediumLow => "Medium-Low",
            VolClass::Medium => "Medium",
            VolClass::MediumHigh => "Medium-High",
            VolClass::High => "High",
            VolClass::VeryHigh => "Very High",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let k = s.to_lowercase().replace([' ', '_'], "-");
        Self::ALL.into_iter().find(|c| c.label().to_lowercase().replace(' ', "-") == k)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Indicator {
    pub key: String,
    pub weight: f64,
    pub lo: f64,
    pub hi: f64,
    pub log: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VolatilityConfig {
    pub indicators: Vec<Indicator>,
    /// Coupures du score entre les 6 classes (5 valeurs croissantes).
    pub cuts: [f64; 5],
    /// Plage d'écart-type / prix visée par le générateur pour chaque classe (bornes basses, 6 valeurs).
    pub sd_targets: [f64; 6],
}

impl Default for VolatilityConfig {
    fn default() -> Self {
        let ind = |key: &str, weight, lo, hi, log| Indicator { key: key.into(), weight, lo, hi, log };
        Self {
            indicators: vec![
                ind("sd_per_price", 0.40, 3.0, 100.0, true),
                ind("loss_rate", 0.15, 0.50, 0.97, false),
                ind("rtp_share_100x", 0.15, 0.10, 0.85, false),
                ind("top1_share", 0.20, 0.40, 0.98, false),
                ind("p_1000x", 0.10, 1e-3, 1e-7, true),
            ],
            cuts: [0.25, 0.40, 0.55, 0.70, 0.85],
            sd_targets: [0.0, 8.0, 15.0, 25.0, 40.0, 60.0],
        }
    }
}

/// Valeurs d'entrée de la classification (toutes calculables exactement sur une loi).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VolInputs {
    pub sd_per_price: f64,
    pub loss_rate: f64,
    pub rtp_share_100x: f64,
    pub top1_share: f64,
    pub p_1000x: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VolResult {
    pub class: VolClass,
    pub score: f64,
    /// (indicateur, valeur, valeur normalisée sur [0, 1], poids).
    pub components: Vec<(String, f64, f64, f64)>,
}

impl VolInputs {
    pub fn from_stats(s: &crate::stats::ExactStats) -> Self {
        let share_100 = s
            .bands
            .iter()
            .filter(|b| b.lo >= 100.0 * s.cost - 1e-9 || b.is_max)
            .map(|b| b.rtp)
            .sum::<f64>();
        let share = if s.rtp > 0.0 { (share_100 / s.rtp).min(1.0) } else { 0.0 };
        let p1000 = s.thresholds.iter().find(|t| (t.mult_price - 1000.0).abs() < 1e-9).map(|t| t.prob).unwrap_or(0.0);
        let top1 = s.concentration.iter().find(|(f, _)| (*f - 0.01).abs() < 1e-12).map(|(_, v)| *v).unwrap_or(0.0);
        Self { sd_per_price: s.sd_per_price, loss_rate: s.zero_rate, rtp_share_100x: share, top1_share: top1, p_1000x: p1000 }
    }

    fn get(&self, key: &str) -> Option<f64> {
        match key {
            "sd_per_price" => Some(self.sd_per_price),
            "loss_rate" => Some(self.loss_rate),
            "rtp_share_100x" => Some(self.rtp_share_100x),
            "top1_share" => Some(self.top1_share),
            "p_1000x" => Some(self.p_1000x),
            _ => None,
        }
    }
}

fn normalize(v: f64, ind: &Indicator) -> f64 {
    let t = if ind.log {
        let floor = 1e-12;
        let (lv, llo, lhi) = (v.max(floor).ln(), ind.lo.max(floor).ln(), ind.hi.max(floor).ln());
        (lv - llo) / (lhi - llo)
    } else {
        (v - ind.lo) / (ind.hi - ind.lo)
    };
    t.clamp(0.0, 1.0)
}

pub fn classify(inp: &VolInputs, cfg: &VolatilityConfig) -> VolResult {
    let mut num = 0.0;
    let mut den = 0.0;
    let mut components = Vec::new();
    for ind in &cfg.indicators {
        if let Some(v) = inp.get(&ind.key) {
            // Un « 0 » pour P(≥1000×) veut dire « impossible » : indicateur au maximum de rareté.
            let n = normalize(v, ind);
            num += n * ind.weight;
            den += ind.weight;
            components.push((ind.key.clone(), v, n, ind.weight));
        }
    }
    let score = if den > 0.0 { num / den } else { 0.0 };
    VolResult { class: class_of_score(score, cfg), score, components }
}

pub fn class_of_score(score: f64, cfg: &VolatilityConfig) -> VolClass {
    let idx = cfg.cuts.iter().filter(|&&c| score >= c).count();
    VolClass::ALL[idx.min(5)]
}

/// Plage d'écart-type / prix visée par le générateur pour une classe : [bas, haut).
pub fn sd_range(class: VolClass, cfg: &VolatilityConfig) -> (f64, Option<f64>) {
    let i = VolClass::ALL.iter().position(|c| *c == class).unwrap();
    (cfg.sd_targets[i], cfg.sd_targets.get(i + 1).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extremes() {
        let cfg = VolatilityConfig::default();
        let calm = VolInputs { sd_per_price: 2.0, loss_rate: 0.4, rtp_share_100x: 0.0, top1_share: 0.3, p_1000x: 0.0 };
        // p_1000x = 0 compte comme « rare » : la classe reste basse grâce aux 4 autres indicateurs.
        assert!(classify(&calm, &cfg).class <= VolClass::MediumLow);
        let wild = VolInputs { sd_per_price: 120.0, loss_rate: 0.95, rtp_share_100x: 0.8, top1_share: 0.95, p_1000x: 1e-6 };
        assert_eq!(classify(&wild, &cfg).class, VolClass::VeryHigh);
    }

    #[test]
    fn parse_labels() {
        assert_eq!(VolClass::parse("medium-high"), Some(VolClass::MediumHigh));
        assert_eq!(VolClass::parse("Very High"), Some(VolClass::VeryHigh));
        assert_eq!(VolClass::parse("nope"), None);
    }
}
