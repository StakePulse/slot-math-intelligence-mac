//! Réglages de l'application, enregistrés dans la table `settings` (clé `app`, JSON).
//! Tout seuil utilisé par un calcul est ici, modifiable et documenté (DECISIONS.md).

use serde::{Deserialize, Serialize};
use smi_core::design::DesignRules;
use smi_core::stats::StatsOptions;
use smi_core::validate::Tolerances;
use smi_core::volatility::VolatilityConfig;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceMode {
    Eco,
    Balanced,
    Performance,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CollectorSettings {
    /// Collecte automatique sur l'API de StakeCruncher. FAUX par défaut : `robots.txt` du site
    /// interdit `/tracker-api/` aux robots ; l'activer est une décision de l'utilisateur.
    pub enabled: bool,
    /// Pause minimale entre deux requêtes (≥ 1 000 ms).
    pub delay_ms: u64,
    /// Nombre de jeux classés lus (pages de 100) pour établir le classement.
    pub ranking_depth: usize,
    /// Nombre de slots dont on télécharge les lookup tables (les plus jouées, au nombre de bets sur 30 j).
    pub top_n: usize,
    pub fetch_luts: bool,
    pub user_agent: String,
    /// Actualisation automatique toutes les N heures (application ouverte) ; `None` = manuelle.
    pub auto_refresh_hours: Option<u32>,
    pub accepted_at: Option<String>,
}

impl Default for CollectorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: 1_500,
            ranking_depth: 400,
            top_n: 50,
            fetch_luts: true,
            user_agent: "SlotMathIntelligence/0.1 (outil d'analyse local)".into(),
            auto_refresh_hours: None,
            accepted_at: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MarketSettings {
    /// Nombre de slots de référence (30 à 50 conseillé).
    pub reference_size: usize,
    /// En dessous de n jeux, un agrégat est signalé « échantillon insuffisant ».
    pub min_sample: usize,
    /// Plafonds comparables : rapport maximal entre deux max win.
    pub peer_factor: f64,
}

impl Default for MarketSettings {
    fn default() -> Self {
        Self { reference_size: 50, min_sample: 10, peer_factor: 2.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    pub performance_mode: PerformanceMode,
    /// Plafond absolu de threads (0 = automatique).
    pub max_threads: usize,
    pub default_seed: u64,
    pub collector: CollectorSettings,
    pub market: MarketSettings,
    pub volatility: VolatilityConfig,
    pub tolerances: Tolerances,
    pub stats: StatsOptions,
    /// Limite de l'énumération exhaustive des rouleaux (combinaisons).
    pub enumeration_limit: f64,
    /// Seuils des contrôles de conception d'un jeu terminé (DECISIONS.md D-17).
    pub design: DesignRules,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            performance_mode: PerformanceMode::Balanced,
            max_threads: 0,
            default_seed: 20_261_106,
            collector: CollectorSettings::default(),
            market: MarketSettings::default(),
            volatility: VolatilityConfig::default(),
            tolerances: Tolerances::default(),
            stats: StatsOptions::default(),
            enumeration_limit: 5e7,
            design: DesignRules::default(),
        }
    }
}

impl AppSettings {
    pub fn load(db: &crate::Db) -> Self {
        db.setting_get("app").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, db: &crate::Db) -> anyhow::Result<()> {
        db.setting_set("app", &serde_json::to_string(self)?)
    }

    /// Threads à utiliser : Éco = 1/4 des cœurs, Équilibré = 1/2, Performance = tous sauf un ;
    /// jamais plus que `max_threads` s'il est fixé, jamais tous les cœurs.
    pub fn threads(&self) -> usize {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let by_mode = match self.performance_mode {
            PerformanceMode::Eco => (cores / 4).max(1),
            PerformanceMode::Balanced => (cores / 2).max(1),
            PerformanceMode::Performance => cores.saturating_sub(1).max(1),
        };
        let capped = if self.max_threads > 0 { by_mode.min(self.max_threads) } else { by_mode };
        capped.clamp(1, cores.saturating_sub(1).max(1))
    }
}
