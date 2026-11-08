//! `smi` — Slot Math Intelligence en ligne de commande.
//!
//! ```text
//! smi init                         crée l'arborescence, la base, importe les données de départ
//! smi stats <table.csv> [prix]     statistiques EXACTES d'une lookup table (JSON)
//! smi generate <spec.json>         génère une loi et l'enregistre comme modèle
//! smi revise <modèle> <spec.json> [note]   nouvelle version d'un modèle
//! smi simulate <modèle> [spins] [graine]
//! smi validate <modèle>            simulation + reproductibilité + 9 étapes
//! smi prompt <modèle>              écrit le prompt dans EXPORTS/prompts
//! smi docs                         intègre les documents déposés dans DOCUMENTS
//! smi collect                      collecte StakeCruncher (si activée dans les réglages)
//! smi market                       résumé du marché chargé
//! smi books <dossier> [rapport.md]  contrôles de conception d'un jeu publié (lecture seule du dossier)
//! ```

use smi_core::dist::Distribution;
use smi_core::generator::{generate, GeneratorSpec};
use smi_core::sim::{self, Control, SimConfig};
use smi_core::stats::compute;
use smi_data::settings::AppSettings;
use smi_data::{documents, market, models, Db, Paths};
use std::path::Path;

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e:#}");
        std::process::exit(1);
    }
}

fn open() -> anyhow::Result<(Paths, Db, AppSettings)> {
    let paths = Paths::detect();
    paths.ensure()?;
    let db = Db::open(&paths.database)?;
    let s = AppSettings::load(&db);
    Ok((paths, db, s))
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "init" => {
            // `smi init [dossier-du-pack]` : même installation que le premier lancement de l'application.
            let (paths, db, s) = open()?;
            println!("Projet : {}", paths.root.display());
            let pack = args.get(1).map(std::path::PathBuf::from);
            let rep = smi_data::starter::install(&db, &paths, &s, pack.as_deref())?;
            println!("{} fichier(s) copié(s), {} déjà présent(s)", rep.copied, rep.kept);
            for m in &rep.messages {
                println!("{m}");
            }
            println!("{} erreur(s)", rep.errors.len());
            for e in &rep.errors {
                println!("  {e}");
            }
            s.save(&db)?;
        }
        "stats" => {
            let file = args.get(1).ok_or_else(|| anyhow::anyhow!("chemin de la table attendu"))?;
            let cost: f64 = args.get(2).map(|v| v.parse()).transpose()?.unwrap_or(1.0);
            let text = std::fs::read_to_string(file)?;
            let (d, info) = Distribution::from_lut_csv(file, cost, None, &text).map_err(|e| anyhow::anyhow!("{e}"))?;
            let st = compute(&d, &AppSettings::default().stats);
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({"lecture": info, "stats": st}))?);
        }
        "generate" => {
            let (_, db, s) = open()?;
            let spec: GeneratorSpec = serde_json::from_str(std::fs::read_to_string(args.get(1).ok_or_else(|| anyhow::anyhow!("spec.json attendu"))?)?.trim_start_matches('\u{feff}'))?;
            let g = generate(&spec, &s.volatility)?;
            let (id, v) = models::create_model(&db, &g, &["CLI".into()], "Créé en ligne de commande")?;
            let st = compute(&g.distribution, &s.stats);
            println!("modèle {id} v{v} : RTP {:.6} %, hit {:.4} %, σ/prix {:.3}, max 1 sur {:.0}", st.rtp * 100.0, st.hit_rate * 100.0, st.sd_per_price, st.max_win.one_in.unwrap_or(0.0));
            for n in &g.notes {
                println!("  note : {n}");
            }
        }
        "revise" => {
            let (_, db, s) = open()?;
            let id = args.get(1).ok_or_else(|| anyhow::anyhow!("identifiant du modèle attendu"))?;
            let spec: GeneratorSpec = serde_json::from_str(std::fs::read_to_string(args.get(2).ok_or_else(|| anyhow::anyhow!("spec.json attendu"))?)?.trim_start_matches('\u{feff}'))?;
            let note = args.get(3).cloned().unwrap_or_else(|| "Révision".into());
            let g = generate(&spec, &s.volatility)?;
            let v = models::add_version(&db, id, &g, &note)?;
            let st = compute(&g.distribution, &s.stats);
            println!("modèle {id} v{v} : RTP {:.6} %, hit {:.4} %, σ/prix {:.3}, bonus médian {:.2}×", st.rtp * 100.0, st.hit_rate * 100.0, st.sd_per_price, st.bonus.as_ref().and_then(|b| b.median).unwrap_or(0.0));
            for n in &g.notes {
                println!("  note : {n}");
            }
        }
        "simulate" | "validate" => {
            let (_, db, s) = open()?;
            let id = args.get(1).ok_or_else(|| anyhow::anyhow!("identifiant du modèle attendu"))?;
            let mv = models::get_version(&db, id, None)?;
            let spins: u64 = args.get(2).map(|v| v.parse()).transpose()?.unwrap_or(10_000_000);
            let seed: u64 = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(s.default_seed);
            let cfg = SimConfig { spins, seed, threads: s.threads(), chunk_size: 1 << 20 };
            let r = sim::run(&mv.distribution, &cfg, &Control::default(), &s.stats, &|d, t| eprint!("\r{:.1} %", d as f64 * 100.0 / t as f64));
            eprintln!();
            println!("{} spins en {} ms ({:.0} spins/s) — RTP simulé {:.4} % (z = {:.2})", r.spins_done, r.elapsed_ms, r.spins_per_sec, r.comparison.rtp_sim * 100.0, r.comparison.rtp_z);
            models::save_simulation(&db, "model", Some(id), Some(mv.version), &format!("{} v{}", mv.name, mv.version), &serde_json::to_value(&cfg)?, &serde_json::to_value(&r)?, r.spins_done, Some(r.comparison.rtp_sim), r.completed, r.elapsed_ms, Some(&r.fingerprint))?;
            if cmd == "validate" {
                let st = compute(&mv.distribution, &s.stats);
                let rp = sim::reproducibility(&mv.distribution, seed, 200_000);
                let t = targets(&mv);
                let rep = smi_core::validate::validate(&mv.distribution, &st, &t, Some(&r), Some(&rp), &s.tolerances, &s.design, &smi_data::now());
                models::save_validation(&db, id, mv.version, &rep)?;
                println!("statut : {}", rep.status.label());
                for c in rep.checks.iter().filter(|c| c.status != smi_core::validate::CheckStatus::Pass) {
                    println!("  {:?} {} {} — attendu {}, observé {}", c.status, c.id, c.label, c.expected, c.observed);
                }
            }
        }
        "docs" => {
            let (paths, db, _) = open()?;
            let (n, errs) = documents::sync(&db, &paths)?;
            println!("{n} document(s) intégrés ; {} erreur(s)", errs.len());
        }
        "collect" => {
            let (paths, db, s) = open()?;
            let r = smi_data::collector::collect(&db, &paths, &s, &|m, p| eprintln!("{:>5.1} % {m}", p * 100.0), &std::sync::atomic::AtomicBool::new(false))?;
            println!("{}", r.message);
        }
        "market" => {
            let (_, db, s) = open()?;
            let (rows, basis) = market::market_rows(&db, &market::MarketFilter { kinds: vec!["base".into()], top_n: Some(s.market.reference_size), ..Default::default() })?;
            println!("{} modes de base (classement : {basis})", rows.len());
            for a in market::aggregates(&rows, s.market.min_sample) {
                println!("{:<32} n={:<4} moyenne={:<12} médiane={:<12} P25={:<12} P75={:<12}", a.label, a.agg.n, f(a.agg.mean), f(a.agg.median), f(a.agg.q1), f(a.agg.q3));
            }
        }
        "prompt" => {
            let (paths, db, s) = open()?;
            let id = args.get(1).ok_or_else(|| anyhow::anyhow!("identifiant du modèle attendu"))?;
            let mv = models::get_version(&db, id, None)?;
            let st = compute(&mv.distribution, &s.stats);
            let md = build_prompt(&db, &s, &mv, &st)?;
            let path = paths.exports.join("prompts").join(format!("{}_v{}.md", market::slugify(&mv.name), mv.version));
            std::fs::write(&path, &md)?;
            models::save_prompt(&db, id, mv.version, &md, Some(&path.display().to_string()))?;
            println!("{}", path.display());
        }
        "books" => {
            let dir = args.get(1).ok_or_else(|| anyhow::anyhow!("dossier publié attendu (celui qui contient index.json)"))?;
            let (_, db, s) = open()?;
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let (id, rep) = smi_data::books::analyze_folder(&db, &s, Path::new(dir), &|f, m| eprint!("\r{:5.1} % {m}          ", f * 100.0), &cancel)?;
            eprintln!();
            let md = smi_data::books::report_markdown(&rep);
            match args.get(2) {
                Some(out) => {
                    std::fs::write(out, &md)?;
                    println!("rapport {id} écrit dans {out}");
                }
                None => println!("{md}"),
            }
        }
        _ => {
            println!("{}", include_str!("main.rs").lines().skip(2).take(13).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
        }
    }
    Ok(())
}

fn f(v: Option<f64>) -> String {
    v.map_or("n.d.".into(), |x| format!("{x:.6}"))
}

pub fn targets(mv: &models::ModelVersion) -> smi_core::validate::Targets {
    smi_core::validate::Targets {
        rtp: Some(mv.spec.rtp),
        max_win: Some(mv.spec.max_win),
        max_win_one_in: mv.params.as_ref().map(|p| 1.0 / p.p_max),
        hit_rate: Some(mv.spec.hit_rate),
        bonus_one_in: Some(mv.spec.bonus_one_in),
        bonus_share: Some(mv.spec.bonus_share),
        sd_target: mv.params.as_ref().and_then(|p| p.sd_target),
    }
}

fn build_prompt(db: &Db, s: &AppSettings, mv: &models::ModelVersion, st: &smi_core::ExactStats) -> anyhow::Result<String> {
    let base = mv.distribution.conditional("Jeu de base", |c| c.kind == smi_core::ClassKind::Base).map(|d| compute(&d, &s.stats));
    let bonus = mv.distribution.conditional("Bonus", |c| c.kind.is_bonus()).map(|d| compute(&d, &s.stats));
    let vol = smi_core::volatility::classify(&smi_core::volatility::VolInputs::from_stats(st), &s.volatility);
    let validation = models::latest_validation(db, &mv.model_id, mv.version)?;
    let sim = models::latest_model_sim(db, &mv.model_id, mv.version, &mv.fingerprint)?;
    let market = market::market_context(db, s, st, mv.spec.max_win, mv.spec.price)?;
    Ok(smi_core::prompt::build(&smi_core::prompt::PromptInput {
        model_id: &mv.model_id,
        version: mv.version,
        generated_at: &smi_data::now(),
        spec: &mv.spec,
        params: mv.params.as_ref(),
        notes: &mv.notes,
        stats: st,
        base: base.as_ref(),
        bonus: bonus.as_ref(),
        vol: Some(&vol),
        validation: validation.as_ref(),
        sim: sim.as_ref(),
        market: market.as_ref(),
        design: Some(&s.design),
    }))
}

#[allow(dead_code)]
fn exists(p: &Path) -> bool {
    p.exists()
}
