//! Nature d'un chiffre : la règle n°5 du projet (« les calculs théoriques doivent être séparés des
//! simulations ») et la convention des modèles Witchcraft / Temple Drop.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Nature {
    /// Somme pondérée finie sur la loi publiée ou générée : exacte (arrondie à l'affichage).
    Exact,
    /// Formule fermée appliquée à des valeurs exactes, ex. `1 - (1 - p)^n`.
    ExactFormula,
    /// Estimation par tirages au hasard : graine et intervalle de confiance à 95 % fournis.
    MonteCarlo,
    /// Entrée de simulation (bandes, tables) : ne fixe pas les fréquences.
    Configuration,
    /// Règle ou cible de conception.
    Specification,
    /// Chiffre importé d'une source tierce (non recalculé par ce moteur).
    Imported,
}

impl Nature {
    pub fn label(self) -> &'static str {
        match self {
            Nature::Exact => "EXACT",
            Nature::ExactFormula => "EXACT FORMULA",
            Nature::MonteCarlo => "MONTE CARLO",
            Nature::Configuration => "CONFIGURATION",
            Nature::Specification => "SPECIFICATION",
            Nature::Imported => "IMPORTÉ",
        }
    }
}
