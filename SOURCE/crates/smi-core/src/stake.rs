//! Barèmes de vérification mathématique de Stake Engine (colonnes 2★ et 3★ du tableau de bord).
//!
//! Source : relevés du tableau de bord Stake Engine (2026-09-24) et de la documentation
//! `approval-guidelines/math-verification` (2026-10-17), notés dans les fiches du studio. Ce sont des
//! tests NON BLOQUANTS côté Stake : chaque classe ratée abaisse la mise maximale du jeu. Les seuils
//! sont modifiables dans les réglages ; une limite absente (`None`) n'est pas contrôlée et est
//! affichée « seuil non renseigné » — jamais inventée.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakeScale {
    pub name: String,
    /// Gain maximal par round, en mises de base.
    pub max_payout: Option<f64>,
    /// Écart-type du gain d'un round par unité de prix.
    pub sd_per_price: Option<f64>,
    /// Coût maximal d'un mode, en mises de base.
    pub max_cost: Option<f64>,
    /// P(gain ≥ seuil en mises de base) ≤ limite.
    pub tail: Vec<(f64, f64)>,
    /// Moyenne des 0,1 % meilleurs rounds, par unité de prix.
    pub cvar_per_stake: Option<f64>,
    /// Moyenne des 0,1 % meilleurs rounds, en mises de base.
    pub cvar_absolute: Option<f64>,
    /// E[X·1{X > 40 × prix}] / prix.
    pub etl_40: Option<f64>,
    /// E[X·1{X > 10 000}] / prix.
    pub etl_10000: Option<f64>,
    pub etl_sum: Option<f64>,
}

impl StakeScale {
    pub fn two_star() -> Self {
        Self {
            name: "2★ (100max_1cent)".into(),
            max_payout: Some(50_000.0),
            sd_per_price: Some(50.0),
            max_cost: Some(1_000.0),
            tail: vec![(5_000.0, 0.01), (10_000.0, 0.005), (25_000.0, 0.002), (50_000.0, 0.001), (100_000.0, 0.000_5)],
            cvar_per_stake: Some(700.0),
            cvar_absolute: Some(20_000.0),
            etl_40: Some(0.8),
            etl_10000: Some(0.6),
            etl_sum: Some(1.3),
        }
    }

    pub fn three_star() -> Self {
        Self {
            name: "3★ (500max_1cent)".into(),
            max_payout: Some(100_000.0),
            sd_per_price: Some(60.0),
            max_cost: Some(5_000.0),
            tail: vec![(5_000.0, 0.05), (10_000.0, 0.01), (25_000.0, 0.005), (50_000.0, 0.001)],
            cvar_per_stake: None,
            cvar_absolute: Some(50_000.0),
            etl_40: Some(0.9),
            etl_10000: Some(0.8),
            etl_sum: Some(1.5),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakeCheck {
    pub scale: String,
    pub item: String,
    pub value: f64,
    pub limit: Option<f64>,
    /// `None` : seuil non renseigné, rien n'est affirmé.
    pub pass: Option<bool>,
}

/// Valeurs d'un mode nécessaires aux barèmes (calculées exactement dans `stats`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakeInputs {
    pub cost: f64,
    pub max_payout: f64,
    pub sd_per_price: f64,
    /// (seuil en mises de base, P(gain ≥ seuil)).
    pub tail: Vec<(f64, f64)>,
    pub cvar_absolute: f64,
    pub etl_40: f64,
    pub etl_10000: f64,
}

pub fn check(scale: &StakeScale, v: &StakeInputs) -> Vec<StakeCheck> {
    let mut out = Vec::new();
    let mut push = |item: String, value: f64, limit: Option<f64>| {
        out.push(StakeCheck { scale: scale.name.clone(), item, value, limit, pass: limit.map(|l| value <= l * (1.0 + 1e-12)) });
    };
    push("Gain maximal (mises de base)".into(), v.max_payout, scale.max_payout);
    push("Écart-type / prix".into(), v.sd_per_price, scale.sd_per_price);
    push("Coût du mode (mises de base)".into(), v.cost, scale.max_cost);
    for (t, lim) in &scale.tail {
        let p = v.tail.iter().find(|(x, _)| (x - t).abs() < 1e-9).map(|(_, p)| *p).unwrap_or(0.0);
        push(format!("P(gain ≥ {}×)", fmt_int(*t)), p, Some(*lim));
    }
    push("CVaR 0,1 % par unité de prix".into(), v.cvar_absolute / v.cost, scale.cvar_per_stake);
    push("CVaR 0,1 % absolue (mises de base)".into(), v.cvar_absolute, scale.cvar_absolute);
    push("ETL > 40 × prix".into(), v.etl_40, scale.etl_40);
    push("ETL > 10 000×".into(), v.etl_10000, scale.etl_10000);
    push("ETL somme".into(), v.etl_40 + v.etl_10000, scale.etl_sum);
    out
}

fn fmt_int(v: f64) -> String {
    let s = format!("{}", v.round() as i64);
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}
