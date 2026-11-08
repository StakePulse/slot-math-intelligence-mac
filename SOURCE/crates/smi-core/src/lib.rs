//! # smi-core — le moteur mathématique de Slot Math Intelligence
//!
//! Tout chiffre produit ici porte sa **nature** (voir [`nature::Nature`]) :
//! - `Exact` : somme pondérée finie sur la loi (table de poids entiers), comme une lookup table Stake Engine ;
//! - `ExactFormula` : formule fermée appliquée à des valeurs exactes (`1-(1-p)^n`, attente médiane…) ;
//! - `MonteCarlo` : estimation par tirages, toujours avec son intervalle de confiance et sa graine ;
//! - `Configuration` / `Specification` : entrées et cibles de conception, jamais des mesures.
//!
//! Modules :
//! - [`dist`] : la loi d'un mode (paiements en centièmes de mise de base × poids entiers, classes de résultat) ;
//! - [`stats`] : toutes les statistiques exactes d'une loi (RTP, moments, percentiles, bandes, classes, attentes…) ;
//! - [`stake`] : les barèmes 2★/3★ de Stake Engine (configurables) ;
//! - [`volatility`] : classification reproductible Low → Very High ;
//! - [`generator`] : construction d'une loi à partir d'objectifs (RTP, max win, hit, bonus, volatilité) ;
//! - [`sim`] : simulateur multithread à graines (tirage alias, flux par lot, résultats indépendants du nombre de threads) ;
//! - [`reels`] : moteur de règles générique (rouleaux, ways/lignes, scatters, tours gratuits) avec calcul exact du jeu de base ;
//! - [`validate`] : contrôle qualité en 9 étapes et statut du modèle ;
//! - [`market`] : agrégats du marché (moyenne, médiane, pondérées, quartiles, valeurs extrêmes) ;
//! - [`prompt`] : le prompt destiné à Claude.

pub mod design;
pub mod dist;
pub mod generator;
pub mod market;
pub mod nature;
pub mod prompt;
pub mod reels;
pub mod rng;
pub mod sim;
pub mod special;
pub mod stake;
pub mod stats;
pub mod validate;
pub mod volatility;

pub use dist::{Atom, ClassDef, ClassKind, Distribution};
pub use stats::ExactStats;

/// Version du moteur, écrite dans chaque export pour la traçabilité.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
