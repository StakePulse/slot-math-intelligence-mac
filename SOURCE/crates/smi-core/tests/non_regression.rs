//! Non-régression sur de VRAIES tables publiées : FINDERS KEEPERS (2026-11-05).
//!
//! Les chiffres de référence viennent d'un calcul indépendant (script `analyse.py` du rapport de
//! redistribution, en Python, autre code) : notre moteur doit les retrouver. Si les fichiers ne sont
//! pas présents (dépôt copié ailleurs), le test le dit et passe sans rien affirmer.

use smi_core::dist::Distribution;
use smi_core::stats::{compute, StatsOptions};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../starter/DATABASE/imports/our_games/finders-keepers_2026-11-05")
}

fn load(mode: &str, cost: f64) -> Option<Distribution> {
    let p = data_dir().join(format!("lookUpTable_{mode}_0.csv"));
    let text = std::fs::read_to_string(&p).ok()?;
    let (d, info) = Distribution::from_lut_csv(mode, cost, Some(500_000), &text).expect("table lisible");
    assert!(info.ids_contiguous, "ids non contigus dans {mode}");
    Some(d)
}

fn reference() -> Option<serde_json::Value> {
    let t = std::fs::read_to_string(data_dir().join("reference_figures.json")).ok()?;
    serde_json::from_str(&t).ok()
}

fn close(a: f64, b: f64, rel: f64, what: &str) {
    let tol = rel * b.abs().max(1e-300);
    assert!((a - b).abs() <= tol, "{what} : moteur {a} vs référence {b}");
}

#[test]
fn finders_keepers_all_modes_match_independent_analysis() {
    let Some(r) = reference() else {
        eprintln!("données FINDERS KEEPERS absentes : test non exécuté");
        return;
    };
    for (mode, cost) in [("base", 1.0), ("nid", 100.0), ("grand_nid", 200.0), ("nid_or", 300.0), ("spin_tresor", 400.0)] {
        let d = load(mode, cost).expect("table présente");
        let s = compute(&d, &StatsOptions::default());
        let f = &r["modes"][mode];
        let g = |k: &str| f[k].as_f64().unwrap();
        close(s.rtp, g("rtp"), 1e-12, &format!("{mode} RTP"));
        close(s.hit_rate, g("gain"), 1e-12, &format!("{mode} hit"));
        close(s.sd_per_price, g("ecart_type_prix"), 1e-9, &format!("{mode} σ/prix"));
        close(s.p_ge_price, g("rend_le_prix"), 1e-12, &format!("{mode} P(≥ prix)"));
        close(s.max_win.value, g("max"), 1e-12, &format!("{mode} max"));
        close(s.max_win.prob, g("p_max"), 1e-9, &format!("{mode} P(max)"));
        close(s.cvar_001, g("cvar_abs"), 1e-9, &format!("{mode} CVaR"));
        close(s.etl_40 + 1e-300, g("etl40") + 1e-300, 1e-9, &format!("{mode} ETL40"));
        assert!(s.closure.max_abs_diff < 1e-12, "{mode} clôture {}", s.closure.max_abs_diff);
    }
}

#[test]
fn finders_keepers_base_published_figures() {
    // Chiffres du rapport PDF (page « Le tableau complet ») : base 96,50 %, 28,1 % de gains, max 1 sur 849 747.
    let Some(d) = load("base", 1.0) else { return };
    let s = compute(&d, &StatsOptions::default());
    assert!((s.rtp - 0.965).abs() < 1e-12);
    assert!((s.hit_rate * 1000.0).round() == 281.0);
    assert_eq!(s.max_win.one_in.unwrap().round(), 849_747.0);
    assert_eq!(s.outcomes > 1000, true);
}
