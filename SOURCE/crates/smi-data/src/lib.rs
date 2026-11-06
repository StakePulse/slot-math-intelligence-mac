//! # smi-data — persistance et sources de données
//!
//! - [`paths`] : où vit le projet (dossier `SLOT-MATH-INTELLIGENCE`) ;
//! - [`db`] : base SQLite (schéma versionné, migrations) ;
//! - [`settings`] : réglages de l'application (performance, collecte, seuils) ;
//! - [`market`] : imports (instantané tiers, lookup tables locales), classements, agrégats, pairs ;
//! - [`collector`] : collecte StakeCruncher — DÉSACTIVÉE par défaut (voir DECISIONS.md D-03) ;
//! - [`models`] : bibliothèque mathématique (modèles, versions, simulations, validations, prompts) ;
//! - [`documents`] : bibliothèque de documents (import, classement, extraction, faits à valider).

pub mod collector;
pub mod db;
pub mod documents;
pub mod market;
pub mod models;
pub mod paths;
pub mod report;
pub mod settings;
pub mod starter;

pub use db::Db;
pub use paths::Paths;

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
